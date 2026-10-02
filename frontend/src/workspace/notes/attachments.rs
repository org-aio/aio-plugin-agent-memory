use az_memory_model::{AttachmentData, AttachmentDraft, AttachmentSummary};
use dioxus::prelude::dioxus_elements::FileData;
use dioxus::prelude::*;
use dioxus_icons::lucide::{Image, X};
use wasm_bindgen::JsCast as _;

pub(super) const MAX_IMAGE_BYTES: usize = 700_000;
pub(super) const MAX_TOTAL_DATA_URL_BYTES: usize = 960_000;
pub(super) const MAX_IMAGES: usize = 4;
const MAX_EDGE: f64 = 1600.0;

#[derive(Clone, PartialEq)]
pub(crate) struct DraftAttachment {
    pub filename: String,
    pub content_type: String,
    pub data_url: String,
}

impl DraftAttachment {
    pub fn request(&self) -> AttachmentDraft {
        AttachmentDraft {
            filename: self.filename.clone(),
            content_type: self.content_type.clone(),
            data_url: self.data_url.clone(),
        }
    }
}

/// 从文件选择结果读取图片；过大时在浏览器内等比压缩，仍超限则保留明确错误。
pub(super) async fn read_image(file: FileData) -> Result<DraftAttachment, String> {
    let filename = file.name();
    let bytes = file
        .read_bytes()
        .await
        .map_err(|_| "无法读取所选图片".to_owned())?;
    let bytes = bytes.to_vec();
    let content_type = file.content_type();
    let inferred = infer_type(&filename, content_type.as_deref(), &bytes)
        .ok_or_else(|| "仅支持 PNG、JPEG、GIF 或 WebP 图片".to_owned())?;
    if bytes.len() <= MAX_IMAGE_BYTES {
        return Ok(attachment(filename, inferred.to_owned(), &bytes));
    }
    if inferred == "image/gif" {
        return Err("GIF 不能自动压缩，请选择不超过 700 KB 的图片".into());
    }
    let compressed = compress_to_data_url(&bytes, inferred).await?;
    let encoded = compressed
        .split_once(',')
        .map(|(_, value)| value)
        .ok_or_else(|| "图片压缩结果无效".to_owned())?;
    let size = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
        .map_err(|_| "图片压缩结果无效".to_owned())?
        .len();
    if size > MAX_IMAGE_BYTES {
        return Err("图片过大，压缩后仍超过 700 KB，请先裁剪".into());
    }
    Ok(attachment(
        filename,
        "image/jpeg".into(),
        compressed.as_bytes(),
    ))
}

fn attachment(filename: String, content_type: String, value: &[u8]) -> DraftAttachment {
    let data_url = match value.starts_with(b"data:") {
        true => String::from_utf8_lossy(value).into_owned(),
        false => format!(
            "data:{content_type};base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, value)
        ),
    };
    DraftAttachment {
        filename,
        content_type,
        data_url,
    }
}

fn infer_type<'a>(filename: &str, content_type: Option<&'a str>, bytes: &[u8]) -> Option<&'a str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    match content_type.unwrap_or_default() {
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" => content_type,
        _ if filename.to_ascii_lowercase().ends_with(".png") => Some("image/png"),
        _ if filename.to_ascii_lowercase().ends_with(".jpg")
            || filename.to_ascii_lowercase().ends_with(".jpeg") =>
        {
            Some("image/jpeg")
        }
        _ if filename.to_ascii_lowercase().ends_with(".gif") => Some("image/gif"),
        _ if filename.to_ascii_lowercase().ends_with(".webp") => Some("image/webp"),
        _ => None,
    }
}

async fn compress_to_data_url(bytes: &[u8], content_type: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or_else(|| "当前环境没有浏览器窗口".to_owned())?;
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(content_type);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(
        &js_sys::Array::of1(&js_sys::Uint8Array::from(bytes)).into(),
        &options,
    )
    .map_err(|_| "无法准备图片".to_owned())?;
    let bitmap = wasm_bindgen_futures::JsFuture::from(
        window
            .create_image_bitmap_with_blob(&blob)
            .map_err(|_| "无法解析图片".to_owned())?,
    )
    .await
    .map_err(|_| "无法解析图片".to_owned())?
    .dyn_into::<web_sys::ImageBitmap>()
    .map_err(|_| "图片格式无效".to_owned())?;
    let (width, height) = (bitmap.width() as f64, bitmap.height() as f64);
    let scale = (MAX_EDGE / width.max(height)).min(1.0);
    let canvas = window
        .document()
        .ok_or_else(|| "当前环境没有浏览器文档".to_owned())?
        .create_element("canvas")
        .map_err(|_| "无法创建图片画布".to_owned())?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| "无法创建图片画布".to_owned())?;
    canvas.set_width((width * scale).round().max(1.0) as u32);
    canvas.set_height((height * scale).round().max(1.0) as u32);
    let context = canvas
        .get_context("2d")
        .map_err(|_| "无法创建图片画布".to_owned())?
        .ok_or_else(|| "无法创建图片画布".to_owned())?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(|_| "无法创建图片画布".to_owned())?;
    context
        .draw_image_with_image_bitmap_and_dw_and_dh(
            &bitmap,
            0.0,
            0.0,
            canvas.width() as f64,
            canvas.height() as f64,
        )
        .map_err(|_| "无法绘制图片".to_owned())?;
    canvas
        .to_data_url_with_type_and_encoder_options("image/jpeg", &0.82.into())
        .map_err(|_| "无法导出压缩图片".to_owned())
}

#[component]
fn DraftAttachmentItem(
    index: usize,
    image: DraftAttachment,
    images: Vec<DraftAttachment>,
    disabled: bool,
    on_change: EventHandler<Vec<DraftAttachment>>,
) -> Element {
    rsx! {
        figure { key: "{index}-{image.filename}", class: "memory-attachment",
            img { src: "{image.data_url}", alt: "{image.filename}" }
            figcaption { "{image.filename}" }
            button { r#type: "button", title: "移除图片", aria_label: "移除图片", disabled,
                onclick: move |_| {
                    let mut next = images.clone();
                    next.remove(index);
                    on_change.call(next);
                }, X { size: 14 } }
        }
    }
}

#[component]
pub(super) fn AttachmentPicker(
    images: Vec<DraftAttachment>,
    disabled: bool,
    on_change: EventHandler<Vec<DraftAttachment>>,
) -> Element {
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let remaining = MAX_IMAGES.saturating_sub(images.len());
    let picker_images = images.clone();
    let attachment_items = images.iter().cloned().enumerate().collect::<Vec<_>>();
    rsx! {
        div { class: "memory-attachments",
            div { class: "memory-attachments__head",
                span { "图片" }
                label { class: "memory-attachments__pick",
                    Image { size: 15 }
                    span { "添加图片" }
                    input {
                        r#type: "file", r#accept: "image/png,image/jpeg,image/gif,image/webp", multiple: remaining > 1,
                        disabled: disabled || busy() || remaining == 0,
                        onchange: move |event: FormEvent| {
                            let selected = event.files().into_iter().take(remaining).collect::<Vec<_>>();
                            if selected.is_empty() { return; }
                            let mut current = picker_images.clone();
                            busy.set(true); error.set(None);
                            spawn(async move {
                                for file in selected {
                                    match read_image(file).await {
                                        Ok(image) => {
                                        let total = current.iter().map(|item| item.data_url.len()).sum::<usize>();
                                        if total.saturating_add(image.data_url.len()) > MAX_TOTAL_DATA_URL_BYTES {
                                            error.set(Some("图片总量超过约 700 KB，请减少图片或先裁剪".into()));
                                            break;
                                        }
                                        current.push(image);
                                    },
                                        Err(message) => { error.set(Some(message)); break; }
                                    }
                                }
                                on_change.call(current);
                                busy.set(false);
                            });
                        },
                    }
                }
                if busy() { span { class: "memory-attachments__hint", "正在处理图片…" } }
                else { span { class: "memory-attachments__hint", "最多 {MAX_IMAGES} 张，单张不超过 700 KB" } }
            }
            if !images.is_empty() {
                div { class: "memory-attachments__grid",
                    for (index, image) in attachment_items {
                        DraftAttachmentItem { index, image, images: images.clone(), disabled, on_change }
                    }
                }
            }
            if let Some(message) = error() { p { class: "memory-attachments__error", role: "alert", "{message}" } }
        }
    }
}

#[component]
pub(super) fn AttachmentViewer(id: String, attachments: Vec<AttachmentSummary>) -> Element {
    if attachments.is_empty() {
        return rsx! {};
    }
    let mut full = use_signal(|| None::<AttachmentData>);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        div { class: "memory-attachments__grid",
            for attachment in attachments {
                figure { key: "{attachment.id}", class: "memory-attachment",
                    button { r#type: "button", class: "memory-attachment__open", title: "查看原图",
                        onclick: { let id = id.clone(); let attachment = attachment.clone(); move |_| {
                            let id = id.clone(); let attachment_id = attachment.id.clone();
                            spawn(async move {
                                match crate::transport::request::<AttachmentData>("GET", &format!("/sources/{id}/attachments/{attachment_id}"), serde_json::Value::Null).await {
                                    Ok(data) => { error.set(None); full.set(Some(data)); }
                                    Err(message) => error.set(Some(message)),
                                }
                            });
                        } },
                        span { class: "memory-attachment__thumb", alt: "{attachment.filename}", "{attachment.filename.chars().next().unwrap_or('图')}" }
                    }
                    figcaption { "{attachment.filename}" }
                }
            }
        }
        if let Some(message) = error() { p { class: "memory-attachments__error", role: "alert", "{message}" } }
        if let Some(data) = full() {
            div { class: "memory-attachment__lightbox", role: "dialog", aria_label: "原图预览", onclick: move |_| full.set(None),
                img { src: "{data.data_url}", alt: "{data.filename}" }
                span { "{data.filename}" }
            }
        }
    }
}
