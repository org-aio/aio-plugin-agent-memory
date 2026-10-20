use super::{MemoryError, Result, access, spaces::require_node_space};
use crate::{model::MemoryContext, service::MemoryService};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use az_memory_model::{ShareLink, ShareRequest};
use base64::{Engine, engine::general_purpose::STANDARD};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use regex::Regex;
use sqlx::Row;
use std::{sync::Arc, sync::LazyLock};

const DEFAULT_SECONDS: i64 = 86_400;
const MIN_SECONDS: i64 = 300;
const MAX_SECONDS: i64 = 31_536_000;

static SECRET_REFERENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[secret:[a-f0-9]{32}\]\]").expect("固定秘密引用正则有效"));
static WIKI_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]\n]{1,160})\]\]").expect("固定 wiki 链接正则有效"));

/// 创建或替换一条笔记的唯一外链；重新生成会立即让旧地址失效。
pub async fn create(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
    Json(request): Json<ShareRequest>,
) -> Result<Json<ShareLink>> {
    if context.worker() {
        return Err(MemoryError::Access);
    }
    let seconds = request.expires_in_seconds.unwrap_or(DEFAULT_SECONDS);
    if !(MIN_SECONDS..=MAX_SECONDS).contains(&seconds) {
        return Err(MemoryError::Input(
            "分享有效期必须在 5 分钟到 365 天之间".into(),
        ));
    }
    let created_at = access::now();
    let expires_at = created_at
        .checked_add(seconds.saturating_mul(1000))
        .ok_or_else(|| MemoryError::Input("分享有效期无效".into()))?;
    let mut transaction = service.pool.begin().await?;
    require_node_space(&mut transaction, &context, &id, true).await?;
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM plugin_memory_sources WHERE id=$1 FOR UPDATE",
    )
    .bind(&id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(MemoryError::Missing)?;
    match status.as_str() {
        "deleted" => return Err(MemoryError::Missing),
        "quarantined" => return Err(MemoryError::Input("保密暂存内容不能分享".into())),
        _ => {}
    }
    let space_id: String =
        sqlx::query_scalar("SELECT space_id FROM plugin_memory_sources WHERE id=$1")
            .bind(&id)
            .fetch_one(&mut *transaction)
            .await?;
    let token = share_token();
    sqlx::query("INSERT INTO plugin_memory_shares(source_id,token,space_id,created_by,created_at,expires_at) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(source_id) DO UPDATE SET token=EXCLUDED.token,space_id=EXCLUDED.space_id,created_by=EXCLUDED.created_by,created_at=EXCLUDED.created_at,expires_at=EXCLUDED.expires_at")
        .bind(&id)
        .bind(&token)
        .bind(&space_id)
        .bind(&context.user_id)
        .bind(created_at)
        .bind(expires_at)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(Json(link(&service, token, expires_at)))
}

/// 当前有效外链；过期或从未创建时返回 null。
pub async fn get(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<Json<Option<ShareLink>>> {
    let mut transaction = service.pool.begin().await?;
    require_node_space(&mut transaction, &context, &id, true).await?;
    let row = sqlx::query(
        "SELECT token,expires_at FROM plugin_memory_shares WHERE source_id=$1 AND expires_at>$2",
    )
    .bind(&id)
    .bind(access::now())
    .fetch_optional(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Json(
        row.map(|row| {
            let token: String = row.try_get(0)?;
            let expires_at: i64 = row.try_get(1)?;
            Ok::<_, sqlx::Error>(link(&service, token, expires_at))
        })
        .transpose()?,
    ))
}

pub async fn revoke(
    State(service): State<Arc<MemoryService>>,
    Path(id): Path<String>,
    context: MemoryContext,
) -> Result<StatusCode> {
    if context.worker() {
        return Err(MemoryError::Access);
    }
    let mut transaction = service.pool.begin().await?;
    require_node_space(&mut transaction, &context, &id, true).await?;
    sqlx::query("DELETE FROM plugin_memory_shares WHERE source_id=$1")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 匿名只读页面；令牌只换取一条已净化笔记，不能访问其他接口。
pub async fn page(
    service: &MemoryService,
    ingress: &crate::transport::Ingress,
    headers: &axum::http::HeaderMap,
    token: &str,
) -> Response {
    if !crate::transport::public_ingress(ingress, headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !valid_token(token) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match public_page(service, token).await {
        Ok(PublicPage::Found(html)) => page_response(html),
        Ok(PublicPage::Expired) => page_response(expired_page()),
        Ok(PublicPage::Missing) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            eprintln!("Memory 分享页读取失败: {error:#}");
            page_response(error_page())
        }
    }
}

enum PublicPage {
    Found(String),
    Expired,
    Missing,
}

async fn public_page(service: &MemoryService, token: &str) -> anyhow::Result<PublicPage> {
    let mut transaction = service.pool.begin().await?;
    let row = sqlx::query(
        "SELECT sh.source_id,sh.space_id,sh.expires_at,s.status,n.title,n.content,n.updated_at
         FROM plugin_memory_shares sh
         JOIN plugin_memory_sources s ON s.id=sh.source_id
         JOIN plugin_memory_nodes n ON n.id=s.id
         WHERE sh.token=$1",
    )
    .bind(token)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(row) = row else {
        return Ok(PublicPage::Missing);
    };
    let source_id: String = row.try_get(0)?;
    let space_id: String = row.try_get(1)?;
    let expires_at: i64 = row.try_get(2)?;
    let status: String = row.try_get(3)?;
    if expires_at <= access::now() {
        return Ok(PublicPage::Expired);
    }
    if matches!(status.as_str(), "deleted" | "quarantined") {
        return Ok(PublicPage::Missing);
    }
    let title: String = row.try_get(4)?;
    let content: String = row.try_get(5)?;
    let updated_at: i64 = row.try_get(6)?;
    let attachments = sqlx::query(
        "SELECT id,filename,content_type,ciphertext FROM plugin_memory_attachments WHERE source_id=$1 ORDER BY created_at,id",
    )
    .bind(&source_id)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;

    let mut images = Vec::new();
    for attachment in attachments {
        let id: String = attachment.try_get(0)?;
        let filename: String = attachment.try_get(1)?;
        let content_type: String = attachment.try_get(2)?;
        let ciphertext: Vec<u8> = attachment.try_get(3)?;
        let bytes = service
            .crypto
            .open(&format!("{space_id}/attachment/{id}"), &ciphertext)
            .await?;
        images.push((filename, content_type, STANDARD.encode(bytes)));
    }
    let title = if title.trim().is_empty() {
        "未命名笔记".to_owned()
    } else {
        title
    };
    Ok(PublicPage::Found(render_page(
        &title, &content, &images, updated_at, expires_at,
    )))
}

fn share_token() -> String {
    format!("{}{}", access::id(), access::id())
}

fn valid_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn link(service: &MemoryService, token: String, expires_at: i64) -> ShareLink {
    ShareLink {
        path: format!("{}/s/{token}", service.public_base.trim_end_matches('/')),
        expires_at,
    }
}

fn page_response(body: String) -> Response {
    let mut response = Html(body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "sandbox allow-top-navigation-by-user-activation; default-src 'none'; img-src data:; style-src 'unsafe-inline'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

fn render_page(
    title: &str,
    content: &str,
    images: &[(String, String, String)],
    updated_at: i64,
    expires_at: i64,
) -> String {
    let body = markdown(&public_text(content));
    let gallery = images
        .iter()
        .map(|(filename, content_type, encoded)| {
            format!(
                "<figure><img src=\"data:{content_type};base64,{encoded}\" alt=\"{}\"><figcaption>{}</figcaption></figure>",
                escape(filename),
                escape(filename)
            )
        })
        .collect::<String>();
    format!(
        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"robots\" content=\"noindex, nofollow\"><title>{}</title><style>{}</style></head><body><main><header><p class=\"eyebrow\">智能体记忆 · 分享</p><h1>{}</h1><p class=\"meta\">更新于 {}</p></header><article>{body}</article>{gallery}<footer>此链接有效期至 {}，到期后自动失效。</footer></main></body></html>",
        escape(title),
        STYLE,
        escape(title),
        format_timestamp(updated_at),
        format_timestamp(expires_at),
    )
}

fn expired_page() -> String {
    simple_page(
        "分享链接已过期",
        "此分享链接已过期。请联系分享者重新生成一条有效链接。",
    )
}

fn error_page() -> String {
    simple_page("暂时无法打开", "分享内容读取失败，请稍后重试。")
}

fn simple_page(title: &str, detail: &str) -> String {
    format!(
        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"robots\" content=\"noindex, nofollow\"><title>{}</title><style>{}</style></head><body><main><header><p class=\"eyebrow\">智能体记忆 · 分享</p><h1>{}</h1></header><p>{}</p></main></body></html>",
        escape(title),
        STYLE,
        escape(title),
        escape(detail),
    )
}

fn public_text(text: &str) -> String {
    let text = SECRET_REFERENCE.replace_all(text, "［保密字段］");
    WIKI_LINK.replace_all(&text, "$1").into_owned()
}

fn markdown(source: &str) -> String {
    let mut events = Vec::new();
    let mut unsafe_link = 0usize;
    for event in Parser::new_ext(source, Options::empty()) {
        match event {
            // 原始 HTML 一律按文本转义，公开页不执行用户输入的结构。
            Event::Html(value) | Event::InlineHtml(value) => events.push(Event::Text(value)),
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                if safe_link(&dest_url) {
                    events.push(Event::Start(Tag::Link {
                        link_type,
                        dest_url,
                        title,
                        id,
                    }));
                } else {
                    unsafe_link += 1;
                }
            }
            Event::End(TagEnd::Link) if unsafe_link > 0 => unsafe_link -= 1,
            _ => events.push(event),
        }
    }
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}

fn safe_link(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("https://") || value.starts_with("http://") || value.starts_with("mailto:")
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_timestamp(timestamp: i64) -> String {
    let seconds = timestamp.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year}-{month:02}-{day:02} {:02}:{:02} UTC",
        day_seconds / 3600,
        (day_seconds % 3600) / 60
    )
}

/// Howard Hinnant 的 civil_from_days；避免为一行时间戳引入日期库。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

const STYLE: &str = r#"
:root{color-scheme:light dark;--bg:#f6f7f9;--card:#fff;--text:#1f2328;--muted:#656d76;--border:#d8dee4;--link:#0969da}
@media(prefers-color-scheme:dark){:root{--bg:#0d1117;--card:#161b22;--text:#e6edf3;--muted:#8d96a0;--border:#30363d;--link:#58a6ff}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--text);font:16px/1.75 ui-sans-serif,-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif}
main{max-width:760px;margin:0 auto;padding:48px 24px 64px}
header{margin-bottom:32px;padding-bottom:20px;border-bottom:1px solid var(--border)}
.eyebrow{margin:0 0 8px;color:var(--muted);font-size:13px;letter-spacing:.04em}
h1{margin:0;font-size:30px;line-height:1.3;font-weight:650}
.meta{margin:10px 0 0;color:var(--muted);font-size:13px}
article{overflow-wrap:anywhere}
article>:first-child{margin-top:0}
article h1,article h2,article h3{line-height:1.35;margin:1.4em 0 .6em}
article h1{font-size:24px}article h2{font-size:20px}article h3{font-size:17px}
article p,article ul,article ol,article blockquote,article pre,article table{margin:0 0 1em}
article a{color:var(--link);text-underline-offset:2px}
article code{padding:.15em .35em;border-radius:4px;background:color-mix(in srgb,var(--muted) 15%,transparent);font:.9em ui-monospace,SFMono-Regular,Menlo,monospace}
article pre{overflow:auto;padding:14px;border:1px solid var(--border);border-radius:8px;background:var(--card)}
article pre code{padding:0;background:none}
article blockquote{margin-left:0;padding-left:14px;border-left:3px solid var(--border);color:var(--muted)}
article img{max-width:100%;height:auto;border-radius:8px}
article table{border-collapse:collapse;width:100%}
article th,article td{padding:6px 10px;border:1px solid var(--border);text-align:left}
figure{margin:24px 0;padding:12px;border:1px solid var(--border);border-radius:10px;background:var(--card)}
figure img{display:block;max-width:100%;height:auto;margin:0 auto;border-radius:6px}
figcaption{margin-top:8px;overflow:hidden;color:var(--muted);font-size:12px;text-overflow:ellipsis;white-space:nowrap;text-align:center}
footer{margin-top:40px;padding-top:16px;border-top:1px solid var(--border);color:var(--muted);font-size:12px}
@media(max-width:600px){main{padding:28px 16px 48px}h1{font-size:24px}}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_text_hides_secret_references_and_wiki_syntax() {
        let text = "正文 [[secret:0123456789abcdef0123456789abcdef]] 与 [[读书笔记]]";
        let value = public_text(text);
        assert!(value.contains("［保密字段］"));
        assert!(value.contains("读书笔记"));
        assert!(!value.contains("secret:"));
        assert!(!value.contains("[["));
    }

    #[test]
    fn markdown_escapes_raw_html_and_rejects_unsafe_links() {
        let value = markdown(
            "<script>alert(1)</script>\n\n[危险](javascript:alert(1)) [安全](https://example.com)",
        );
        assert!(value.contains("&lt;script&gt;"));
        assert!(!value.contains("<script>"));
        assert!(!value.contains("javascript:"));
        assert!(value.contains("https://example.com"));
        assert!(value.contains("危险"));
    }

    #[test]
    fn timestamp_uses_utc_civil_date() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_timestamp(1_767_225_600_000), "2026-01-01 00:00 UTC");
    }
}
