use regex::Regex;
use serde_json::{Map, Value};

#[derive(Clone)]
pub struct IsolatedSecret {
    pub id: String,
    pub label: String,
    pub value: String,
}

#[derive(Clone)]
pub struct Isolation {
    pub text: String,
    pub secrets: Vec<IsolatedSecret>,
    pub quarantined: bool,
}

pub fn isolate(input: &str, allowed: &[String]) -> Isolation {
    let mut checked = input.to_owned();
    for id in allowed {
        checked = checked.replace(&format!("[[secret:{id}]]"), "[protected]");
    }
    let uncertain = std::cell::Cell::new(checked.contains("[[secret:"));
    let mut secrets = Vec::new();
    let mut protect = |label: &str, value: &str| {
        if value.trim().is_empty() {
            uncertain.set(true);
            return "[protected]".to_owned();
        }
        // 已授权引用由原生调用方确认；不能再次作为明文秘密抽取。
        if allowed.iter().any(|id| value == format!("[[secret:{id}]]")) {
            return value.to_owned();
        }
        if let Some(existing) = secrets
            .iter()
            .find(|secret: &&IsolatedSecret| secret.value == value)
        {
            return format!("[[secret:{}]]", existing.id);
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        secrets.push(IsolatedSecret {
            id: id.clone(),
            label: secret_label(label).to_owned(),
            value: value.to_owned(),
        });
        format!("[[secret:{id}]]")
    };
    // JSON 先解码再逐字段净化，普通字符串只扫描已知秘密格式。
    let structured = input.trim_start().starts_with('{') || input.trim_start().starts_with('[');
    let json = structured
        .then(|| serde_json::from_str::<Value>(input.trim()).ok())
        .flatten();
    let initial = match json {
        Some(value) => visit(value, &mut protect, &uncertain, allowed).to_string(),
        None => protect_text(input, &mut protect, &uncertain),
    };
    if serde_json::from_str::<Value>(checked.trim()).is_err()
        && (checked.trim_start().starts_with('{') || checked.trim_start().starts_with('['))
        && incomplete_assignment().is_match(&checked)
    {
        uncertain.set(true);
    }
    let mut ordered = secrets.clone();
    ordered.sort_by_key(|secret| std::cmp::Reverse(secret.value.len()));
    // JSON 解码后替换字符串和字段名，防止引号转义使同值备注漏掉净化。
    let safe = if structured {
        match serde_json::from_str::<Value>(&initial) {
            Ok(value) => redact_json(value, &ordered).to_string(),
            Err(_) => redact(&initial, &ordered),
        }
    } else {
        redact(&initial, &ordered)
    };
    Isolation {
        text: if uncertain.get() {
            "[资料已保密暂存，等待补充说明]".into()
        } else {
            safe
        },
        secrets,
        quarantined: uncertain.get(),
    }
}

fn visit(
    mut value: Value,
    protect: &mut impl FnMut(&str, &str) -> String,
    uncertain: &std::cell::Cell<bool>,
    allowed: &[String],
) -> Value {
    match &mut value {
        Value::Object(object) => {
            let entries = std::mem::take(object);
            let mut next = Map::new();
            for (key, value) in entries {
                let protected = if sensitive(&key) {
                    Value::String(protect(&key, &scalar(&value)))
                } else {
                    visit(value, protect, uncertain, allowed)
                };
                next.insert(protect_text(&key, protect, uncertain), protected);
            }
            Value::Object(next)
        }
        Value::Array(values) => {
            for value in values {
                *value = visit(value.take(), protect, uncertain, allowed);
            }
            value
        }
        // 整理结果的 secretIds 是服务端已确认的公开字段 ID，不是未标注凭据。
        Value::String(value) if allowed.contains(value) => Value::String(value.clone()),
        Value::String(value) => Value::String(protect_text(value, protect, uncertain)),
        _ => value,
    }
}

fn protect_text(
    text: &str,
    protect: &mut impl FnMut(&str, &str) -> String,
    uncertain: &std::cell::Cell<bool>,
) -> String {
    // 引用内部的 secret: 不是密码赋值；未知引用已在入口被隔离。
    let references = Regex::new(r"\[\[secret:[a-f0-9]{32}\]\]").expect("固定引用正则有效");
    let mut result = String::new();
    let mut position = 0;
    for reference in references.find_iter(text) {
        result.push_str(&protect_plain_text(
            &text[position..reference.start()],
            protect,
            uncertain,
        ));
        result.push_str(reference.as_str());
        position = reference.end();
    }
    result.push_str(&protect_plain_text(&text[position..], protect, uncertain));
    let remaining = references.replace_all(&result, "[protected]");
    let opaque = Regex::new(r"^[A-Za-z0-9_+/=-]{32,}$").expect("固定不透明值正则有效");
    if remaining.lines().any(|line| opaque.is_match(line.trim()))
        || remaining.contains("-----BEGIN")
    {
        uncertain.set(true);
    }
    for assignment in incomplete_assignment().find_iter(&remaining) {
        if !remaining[assignment.end()..]
            .trim_start()
            .starts_with("[protected]")
        {
            uncertain.set(true);
        }
    }
    result
}

fn protect_plain_text(
    text: &str,
    protect: &mut impl FnMut(&str, &str) -> String,
    uncertain: &std::cell::Cell<bool>,
) -> String {
    let text = protect_private_keys(text, protect);
    let text = protect_assignments(&text, protect, uncertain);
    let text = protect_bearer(&text, protect);
    let text = Regex::new(r"([a-zA-Z][a-zA-Z0-9+.-]*://[^\s/@:]+:)([^\s/@]+)(@)")
        .expect("固定 URL 凭据正则有效")
        .replace_all(&text, |captures: &regex::Captures<'_>| {
            format!(
                "{}{}{}",
                &captures[1],
                protect("url_password", &captures[2]),
                &captures[3]
            )
        });
    protect_known_tokens(&text, protect)
}

fn scalar(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn protect_private_keys(text: &str, protect: &mut impl FnMut(&str, &str) -> String) -> String {
    Regex::new(r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----")
        .expect("固定私钥正则有效")
        .replace_all(text, |captures: &regex::Captures<'_>| {
            protect("private_key", &captures[0])
        })
        .into_owned()
}

fn protect_assignments(
    text: &str,
    protect: &mut impl FnMut(&str, &str) -> String,
    uncertain: &std::cell::Cell<bool>,
) -> String {
    let regex = assignment();
    regex
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let raw = captures.get(2).map(|value| value.as_str()).unwrap_or("");
            let end = captures
                .get(0)
                .map(|value| value.end())
                .unwrap_or(text.len());
            let tail = text[end..]
                .split(['\n', ',', ';', '，', '；'])
                .next()
                .unwrap_or("")
                .trim();
            if !raw.starts_with(['"', '\'']) && !tail.is_empty() && !regex.is_match(tail) {
                uncertain.set(true);
            }
            let value = if raw.starts_with('"') {
                serde_json::from_str::<String>(raw).unwrap_or_else(|_| {
                    uncertain.set(true);
                    raw.to_owned()
                })
            } else {
                raw.trim_matches('\'').to_owned()
            };
            if matches!(value.as_str(), "" | "|" | ">" | "|-" | "|+" | ">-" | ">+")
                || value.starts_with("!!")
            {
                uncertain.set(true);
            }
            format!("{}: {}", &captures[1], protect(&captures[1], &value))
        })
        .into_owned()
}

fn protect_bearer(text: &str, protect: &mut impl FnMut(&str, &str) -> String) -> String {
    Regex::new(r"(?i)\bbearer\s+([A-Za-z0-9._~+/-]+=*)")
        .expect("固定 bearer 正则有效")
        .replace_all(text, |captures: &regex::Captures<'_>| {
            format!("Bearer {}", protect("authorization", &captures[1]))
        })
        .into_owned()
}

fn protect_known_tokens(text: &str, protect: &mut impl FnMut(&str, &str) -> String) -> String {
    Regex::new(r"(?:sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)")
        .expect("固定 token 正则有效")
        .replace_all(text, |captures: &regex::Captures<'_>| protect("token", &captures[0]))
        .into_owned()
}

fn replace_outside_references(text: &str, value: &str, replacement: &str) -> String {
    if value.is_empty() {
        return text.to_owned();
    }
    let reference = Regex::new(r"\[\[secret:[a-f0-9]{32}\]\]").expect("固定引用正则有效");
    let mut result = String::new();
    let mut position = 0;
    for capture in reference.find_iter(text) {
        result.push_str(&text[position..capture.start()].replace(value, replacement));
        result.push_str(capture.as_str());
        position = capture.end();
    }
    result.push_str(&text[position..].replace(value, replacement));
    result
}

fn assignment() -> Regex {
    Regex::new(r#"(?i)(password|passwd|pwd|passphrase|(?:access[_ -]?|refresh[_ -]?)?token|api[_ -]?key|(?:client[_ -]?|app[_ -]?)?secret|secret[_ -]?key|private[_ -]?key|authorization|cookie|密码|口令|密钥|秘钥|令牌)\s*[:=：是为]\s*("(?:[^"\\]|\\.)*"|'[^']*'|[^\s,;，；]+)"#)
        .expect("固定赋值正则有效")
}

fn incomplete_assignment() -> Regex {
    Regex::new(
        r"(?i)(password|passwd|pwd|token|secret|api[_ -]?key|密码|密钥|秘钥|令牌)\s*[:=：是为]",
    )
    .expect("固定不完整赋值正则有效")
}

fn normalized_name(name: &str) -> String {
    name.chars()
        .filter(|value| value.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

const SECRET_NAMES: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "passphrase",
    "token",
    "apikey",
    "secret",
    "privatekey",
    "authorization",
    "cookie",
    "密码",
    "口令",
    "密钥",
    "秘钥",
    "令牌",
    "accesstoken",
    "refreshtoken",
    "clientsecret",
    "appsecret",
    "secretkey",
];

fn secret_label(name: &str) -> &str {
    let normalized = normalized_name(name);
    SECRET_NAMES
        .iter()
        .filter(|item| normalized.ends_with(**item))
        .max_by_key(|item| item.len())
        .copied()
        .unwrap_or("secret")
}

fn sensitive(name: &str) -> bool {
    let normalized = normalized_name(name);
    SECRET_NAMES.iter().any(|item| {
        normalized == *item || (item.chars().count() >= 5 && normalized.ends_with(item))
    })
}

fn redact(text: &str, secrets: &[IsolatedSecret]) -> String {
    let mut safe = text.to_owned();
    for secret in secrets {
        safe =
            replace_outside_references(&safe, &secret.value, &format!("[[secret:{}]]", secret.id));
    }
    safe
}

fn redact_json(value: Value, secrets: &[IsolatedSecret]) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (redact(&key, secrets), redact_json(value, secrets)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| redact_json(value, secrets))
                .collect(),
        ),
        Value::String(value) => Value::String(redact(&value, secrets)),
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolates_assignments_and_reuses_references() {
        let result = isolate("password: canary\n再次 canary", &[]);
        assert!(!result.quarantined);
        assert_eq!(result.secrets.len(), 1);
        assert!(!result.text.contains("canary"));
        assert!(result.text.contains("[[secret:"));
    }

    #[test]
    fn quarantines_unknown_secret_references() {
        let result = isolate("[[secret:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]]", &[]);
        assert!(result.quarantined);
    }

    #[test]
    fn quarantines_ambiguous_secrets_without_partial_disclosure() {
        for input in [
            "password:",
            "-----BEGIN PRIVATE KEY-----\npartial",
            "password: canary has spaces",
            "authorization: Bearer opaque-value",
            "password: |\n  multiline-canary",
            "password: !!str canary",
            &"R".repeat(48),
        ] {
            let result = isolate(input, &[]);
            assert!(result.quarantined, "{input}");
            assert_eq!(result.text, "[资料已保密暂存，等待补充说明]");
        }
        for input in [
            "帮我找一下网站的密码",
            "记一下：下周整理项目资料",
            "APP_ID=visible-app",
        ] {
            let result = isolate(input, &[]);
            assert!(!result.quarantined);
            assert_eq!(result.text, input);
        }
    }

    #[test]
    fn protects_url_passwords_escaped_copies_and_metadata() -> Result<(), serde_json::Error> {
        let result = isolate("postgres://user:canary-password@db.example/data", &[]);
        assert!(!result.quarantined);
        assert!(!result.text.contains("canary-password"));
        let result = isolate(
            r#"{"password":"canary\"quoted","note":"canary\"quoted","canary\"quoted":"ordinary"}"#,
            &[],
        );
        assert!(!result.quarantined);
        assert!(!result.text.contains("canary"));
        let value: Value = serde_json::from_str(&result.text)?;
        assert_eq!(value["password"], value["note"]);
        let result = isolate(r#"{"canary_password":"canary"}"#, &[]);
        assert!(!result.quarantined);
        assert!(!result.secrets[0].label.contains("canary"));
        Ok(())
    }
    #[test]
    fn json_preserves_ordinary_fields_and_replaces_duplicate_secrets() {
        let result = isolate(
            r#"{"username":"alice","password":"canary","note":"again canary","nested":[{"title":"project","body":"hello"}]}"#,
            &[],
        );
        assert!(!result.quarantined);
        assert_eq!(result.secrets.len(), 1);
        assert_eq!(result.secrets[0].value, "canary");
        let safe: Value = serde_json::from_str(&result.text).unwrap();
        assert_eq!(safe["username"], "alice");
        assert_eq!(safe["nested"][0]["body"], "hello");
        assert!(!result.text.contains("canary"));
    }

    #[test]
    fn nested_strings_scan_secrets_without_hiding_ordinary_text() {
        let result = isolate(
            r#"{"notes":["hello","password: hidden","Bearer bearer-value"],"escaped":"api_key: \"another-value\""}"#,
            &[],
        );
        assert!(!result.quarantined);
        assert_eq!(result.secrets.len(), 3);
        assert!(result.text.contains("hello"));
        assert!(!result.text.contains("hidden"));
        assert!(!result.text.contains("bearer-value"));
        assert!(!result.text.contains("another-value"));
    }

    #[test]
    fn compilation_can_keep_authorized_references_without_creating_secrets() {
        let id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        let reference = format!("[[secret:{id}]]");
        let value = serde_json::json!([{"draft":{"title":"account","content":format!("password: {reference}")},"secretIds":[id]}]);
        let result = isolate(&value.to_string(), &[id]);
        assert!(!result.quarantined);
        assert!(result.secrets.is_empty());
        assert!(result.text.contains(&reference));
        let nested = serde_json::json!({"draft":{"content":serde_json::json!({"password":reference,"note":"alice"}).to_string()}});
        let result = isolate(
            &nested.to_string(),
            &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()],
        );
        assert!(!result.quarantined);
        assert!(result.secrets.is_empty());
        assert!(result.text.contains(&reference));
    }
}
