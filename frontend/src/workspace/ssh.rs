use crate::{state, state::MemoryState};
use az_memory_model::{SshHost, SshHostDraft};
use az_ui_components::{
    admin::{EmptyState, StatusMessage},
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{Dialog, DialogDescription, DialogTitle},
    input::TextInput,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Copy, Pencil, Plus, Server, ShieldCheck, Terminal, Trash2};

#[component]
pub fn SshPanel() -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut editing = use_signal(|| None::<SshHost>);
    let mut creating = use_signal(|| false);
    let mut importing = use_signal(|| false);
    let mut configuring = use_signal(|| None::<SshHost>);
    let mut deleting = use_signal(|| None::<SshHost>);
    let mut public_key = use_signal(|| None::<String>);
    let hosts = state.read().ssh_hosts.clone();
    rsx! {
        section { class: "memory-ssh",
            header { class: "memory-ssh__header",
                div {
                    h2 { "SSH 连接" }
                    p { "配置通过已配对设备写入 ~/.ssh/config，私钥和密码不会上传。" }
                }
                div { class: "memory-ssh__header-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().busy || state.read().ssh_devices.is_empty(),
                        onclick: move |_| importing.set(true),
                        "从设备导入"
                    }
                    Button {
                        disabled: state.read().busy || state.read().ssh_devices.is_empty(),
                        onclick: move |_| creating.set(true),
                        Plus { size: "16px" }
                        "添加主机"
                    }
                }
            }
            if state.read().ssh_devices.is_empty() {
                StatusMessage { message: "没有已配对的 SSH 管理设备。请先升级并授权 worker。", error: true }
            }
            if hosts.is_empty() {
                EmptyState {
                    title: "暂无 SSH 主机",
                    detail: "添加 alias 后，在设备本机执行配置并验证免密登录。",
                }
            }
            div { class: "memory-ssh__list",
                for host in hosts {
                    SshHostRow {
                        key: "{host.id}",
                        host: host.clone(),
                        on_edit: move |value| editing.set(Some(value)),
                        on_configure: move |value| configuring.set(Some(value)),
                        on_delete: move |value| deleting.set(Some(value)),
                        on_key: move |value| public_key.set(Some(value)),
                    }
                }
            }
        }
        if creating() {
            SshHostDialog { on_close: move |_| creating.set(false) }
        }
        if importing() {
            SshImportDialog { on_close: move |_| importing.set(false) }
        }
        if let Some(host) = editing() {
            SshHostDialog { existing: Some(host), on_close: move |_| editing.set(None) }
        }
        if let Some(host) = configuring() {
            SshConfigureDialog { host, on_close: move |_| configuring.set(None), on_applied: move |_| configuring.set(None) }
        }
        if let Some(host) = deleting() {
            SshDeleteDialog { host, on_close: move |_| deleting.set(None) }
        }
        if let Some(value) = public_key() {
            PublicKeyDialog { value, on_close: move |_| public_key.set(None) }
        }
    }
}

#[component]
fn SshHostRow(
    host: SshHost,
    on_edit: EventHandler<SshHost>,
    on_configure: EventHandler<SshHost>,
    on_delete: EventHandler<SshHost>,
    on_key: EventHandler<String>,
) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut busy = use_signal(|| false);
    let id = host.id.clone();
    let edit = host.clone();
    let configure = host.clone();
    let remove = host.clone();
    rsx! {
        article { class: "memory-ssh__row",
            div { class: "memory-ssh__identity",
                div { class: "memory-ssh__alias", Terminal { size: "16px" } code { "ssh {host.alias}" } }
                div { class: "memory-ssh__target", "{host.user}@{host.hostname}:{host.port}" }
                div { class: "memory-ssh__device", Server { size: "14px" } "{host.device_label}" }
            }
            div { class: "memory-ssh__status",
                Badge {
                    variant: match host.status.as_str() { "verified" => BadgeVariant::Primary, "error" => BadgeVariant::Destructive, _ => BadgeVariant::Outline },
                    "{status_label(&host.status)}"
                }
                if let Some(error) = host.last_error.clone() { span { class: "memory-ssh__error", "{error}" } }
            }
            div { class: "memory-ssh__actions",
                Button {
                    variant: ButtonVariant::Outline, disabled: busy(),
                    title: "把这台记录的字段重写到设备托管块",
                    onclick: move |_| on_configure.call(configure.clone()),
                    Server { size: "15px" }
                    "重写配置"
                }
                Button {
                    variant: ButtonVariant::Outline, disabled: busy(),
                    onclick: {
                        let id = id.clone();
                        move |_| {
                            let id = id.clone(); busy.set(true);
                            spawn(async move {
                                if let Ok(result) = state::verify_ssh_host(state, id).await {
                                    if let Some(key) = result.public_key { on_key.call(key); }
                                }
                                busy.set(false);
                            });
                        }
                    },
                    ShieldCheck { size: "15px" }
                    "验证免密"
                }
                Button { variant: ButtonVariant::Ghost, size: ButtonSize::IconSm, title: "编辑", aria_label: "编辑 SSH 主机", disabled: busy(), onclick: move |_| on_edit.call(edit.clone()), Pencil { size: "15px" } }
                Button { variant: ButtonVariant::Ghost, size: ButtonSize::IconSm, title: "删除", aria_label: "删除 SSH 主机", disabled: busy(), onclick: move |_| on_delete.call(remove.clone()), Trash2 { size: "15px" } }
            }
        }
    }
}

#[component]
fn SshHostDialog(existing: Option<SshHost>, on_close: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut alias = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.alias.clone())
            .unwrap_or_default()
    });
    let mut hostname = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.hostname.clone())
            .unwrap_or_default()
    });
    let mut user = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.user.clone())
            .unwrap_or_default()
    });
    let mut port = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.port.to_string())
            .unwrap_or_else(|| "22".into())
    });
    // 新建时不预填身份文件，避免把某台设备的默认密钥误当作这台主机的选择。
    let mut identity = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.identity_file.clone())
            .unwrap_or_default()
    });
    let mut proxy_jump = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.proxy_jump.clone())
            .unwrap_or_default()
    });
    let mut proxy_command = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.proxy_command.clone())
            .unwrap_or_default()
    });
    // 配对设备必须由用户显式选择；默认选中第一台容易写错目标设备。
    let mut device = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.device_id.clone())
            .unwrap_or_default()
    });
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let id = existing.as_ref().map(|host| host.id.clone());
    let version = existing.as_ref().map(|host| host.version);
    let devices = state.read().ssh_devices.clone();
    let title = if existing.is_some() {
        "编辑 SSH 主机"
    } else {
        "添加 SSH 主机"
    };
    rsx! {
        Dialog { class: "memory-dialog memory-ssh-dialog", open: true, on_open_change: move |open: bool| if !open && !busy() { on_close.call(()); },
            DialogTitle { "{title}" }
            DialogDescription { "保存后可在列表点“重写配置”，把这条记录写入目标设备的 ~/.ssh/config 托管块。" }
            div { class: "memory-ssh-form",
                TextInput { label: "Alias", value: alias(), on_change: move |value| alias.set(value), placeholder: Some("例如 my-server".into()) }
                TextInput { label: "主机地址", value: hostname(), on_change: move |value| hostname.set(value), placeholder: Some("例如 server.example.com 或 192.0.2.10".into()) }
                div { class: "memory-ssh-form__split",
                    TextInput { label: "用户", value: user(), on_change: move |value| user.set(value), placeholder: Some("登录用户名".into()) }
                    TextInput { label: "端口", value: port(), input_type: "number", on_change: move |value| port.set(value) }
                }
                label { class: "memory-ssh-form__field", "配对设备"
                    select { value: device(), onchange: move |event| device.set(event.value()),
                        option { value: "", "请选择写入配置的设备" }
                        for item in devices { option { value: "{item.id}", "{item.label} · {item.platform}" } }
                    }
                }
                label { class: "memory-ssh-form__field", "身份文件"
                    select { value: identity().replace(".pub", ""), onchange: move |event| identity.set(event.value()),
                        option { value: "", "请选择或沿用设备已有的私钥" }
                        option { value: "id_ed25519", "~/.ssh/id_ed25519" }
                        option { value: "id_rsa", "~/.ssh/id_rsa" }
                        option { value: "id_ecdsa", "~/.ssh/id_ecdsa" }
                    }
                }
                // 跳板与代理互斥；留空表示直连，导入的现有配置会按原值回填。
                if proxy_command().is_empty() {
                    TextInput { label: "跳板主机（可选）", value: proxy_jump(), on_change: move |value| proxy_jump.set(value), placeholder: Some("例如 bastion 或 user@bastion:22".into()) }
                }
                if proxy_jump().is_empty() {
                    TextInput { label: "代理命令（可选）", value: proxy_command(), on_change: move |value| proxy_command.set(value), placeholder: Some("例如 cloudflared access ssh --hostname %h".into()) }
                }
                if let Some(message) = error() { StatusMessage { message, error: true } }
            }
            footer { class: "memory-dialog__footer",
                Button { variant: ButtonVariant::Outline, disabled: busy(), onclick: move |_| on_close.call(()), "取消" }
                Button { disabled: busy(), onclick: move |_| {
                    let parsed = match port().parse::<u16>() { Ok(value) => value, Err(_) => { error.set(Some("端口无效".into())); return; } };
                    if alias().trim().is_empty() || hostname().trim().is_empty() || user().trim().is_empty() || device().is_empty() {
                        error.set(Some("请填写 alias、主机、用户并选择设备".into())); return;
                    }
                    if identity().is_empty() {
                        error.set(Some("请选择设备上的身份文件".into())); return;
                    }
                    let draft = SshHostDraft {
                        alias: alias(), hostname: hostname(), user: user(), port: parsed,
                        identity_file: identity().replace(".pub", ""),
                        proxy_jump: proxy_jump(), proxy_command: proxy_command(),
                        device_id: device(), version,
                    };
                    let id = id.clone(); busy.set(true);
                    spawn(async move {
                        match state::save_ssh_host(state, id, draft).await {
                            Ok(_) => { busy.set(false); on_close.call(()); }
                            Err(message) => { error.set(Some(message)); busy.set(false); }
                        }
                    });
                }, if busy() { "保存中" } else { "保存" } }
            }
        }
    }
}

#[component]
fn SshImportDialog(on_close: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut device = use_signal(|| {
        state
            .read()
            .ssh_devices
            .first()
            .map(|item| item.id.clone())
            .unwrap_or_default()
    });
    let devices = state.read().ssh_devices.clone();
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        Dialog { class: "memory-dialog memory-ssh-dialog", open: true, on_open_change: move |open: bool| if !open && !busy() { on_close.call(()); },
            DialogTitle { "从设备导入 SSH 配置" }
            DialogDescription { "读取选定设备已有的 ~/.ssh/config 并登记为记忆主机。只保存别名、主机、用户、端口和私钥文件名，不上传私钥。" }
            div { class: "memory-simport",
                label { class: "memory-ssh-form__field", "读取设备"
                    select { value: device(), onchange: move |event| device.set(event.value()),
                        for item in devices { option { value: "{item.id}", "{item.label} · {item.platform}" } }
                    }
                }
                p { class: "memory-simport__note", "本机转发连接、已由本插件管理的条目和非法别名会跳过并列出原因；带跳板或代理的条目会原样登记。" }
                if let Some(message) = error() { StatusMessage { message, error: true } }
            }
            footer { class: "memory-dialog__footer",
                Button { variant: ButtonVariant::Outline, disabled: busy(), onclick: move |_| on_close.call(()), "取消" }
                Button { disabled: busy() || device().is_empty(), onclick: move |_| {
                    if device().is_empty() { error.set(Some("请选择要读取的设备".into())); return; }
                    let id = device();
                    busy.set(true);
                    spawn(async move {
                        match state::import_ssh_hosts(state, id).await {
                            Ok(_) => { busy.set(false); on_close.call(()); }
                            Err(message) => { error.set(Some(message)); busy.set(false); }
                        }
                    });
                }, if busy() { "读取中" } else { "读取并导入" } }
            }
        }
    }
}

/// 导入的现有配置默认只登记元数据；重写前必须明确确认，且携带版本做乐观锁。
#[component]
fn SshConfigureDialog(
    host: SshHost,
    on_close: EventHandler<()>,
    on_applied: EventHandler<()>,
) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        Dialog { class: "memory-confirm memory-ssh-configure", open: true, on_open_change: move |open: bool| if !open && !busy() { on_close.call(()); },
            DialogTitle { "重写设备配置？" }
            DialogDescription { "会把下列四项写进设备 ~/.ssh/config 的托管块；原有的跳板、代理和超时等选项不会保留。" }
            p { class: "memory-delete-title", "ssh {host.alias} · {host.user}@{host.hostname}:{host.port} · ~/.ssh/{host.identity_file}" }
            if let Some(message) = error() { StatusMessage { message, error: true } }
            footer { class: "memory-dialog__footer",
                Button { variant: ButtonVariant::Outline, disabled: busy(), onclick: move |_| on_close.call(()), "取消" }
                Button { disabled: busy(), onclick: move |_| {
                    busy.set(true); let id = host.id.clone();
                    spawn(async move {
                        match state::apply_ssh_host(state, id).await {
                            Ok(_) => { busy.set(false); on_applied.call(()); }
                            Err(message) => { error.set(Some(message)); busy.set(false); }
                        }
                    });
                }, if busy() { "写入中" } else { "确认写入" } }
            }
        }
    }
}

#[component]
fn SshDeleteDialog(host: SshHost, on_close: EventHandler<()>) -> Element {
    let state = use_context::<Signal<MemoryState>>();
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        Dialog { class: "memory-confirm", open: true, on_open_change: move |open: bool| if !open && !busy() { on_close.call(()); },
            DialogTitle { "删除 SSH 主机？" }
            DialogDescription { "设备上的托管配置块会一并删除；手工配置和其他主机不受影响。" }
            p { class: "memory-delete-title", "ssh {host.alias} · {host.user}@{host.hostname}" }
            if let Some(message) = error() { StatusMessage { message, error: true } }
            footer { class: "memory-dialog__footer",
                Button { variant: ButtonVariant::Outline, disabled: busy(), onclick: move |_| on_close.call(()), "取消" }
                Button { variant: ButtonVariant::Destructive, disabled: busy(), onclick: move |_| {
                    busy.set(true); let id = host.id.clone();
                    spawn(async move {
                        match state::delete_ssh_host(state, id).await {
                            Ok(_) => { busy.set(false); on_close.call(()); }
                            Err(message) => { error.set(Some(message)); busy.set(false); }
                        }
                    });
                }, if busy() { "删除中" } else { "确认删除" } }
            }
        }
    }
}

#[component]
fn PublicKeyDialog(value: String, on_close: EventHandler<()>) -> Element {
    let mut copied = use_signal(|| false);
    rsx! {
        Dialog { class: "memory-dialog memory-ssh-key", open: true, on_open_change: move |open: bool| if !open { on_close.call(()); },
            DialogTitle { "本机 SSH 公钥" }
            DialogDescription { "如果远端尚未安装此公钥，请在设备终端执行 ssh-copy-id 完成一次授权；密码不要录入本插件。" }
            label { class: "memory-ssh-form__field", "公钥"
                textarea { readonly: true, value: "{value}" }
            }
            if copied() { StatusMessage { message: "公钥已复制" } }
            footer { class: "memory-dialog__footer",
                Button { variant: ButtonVariant::Outline, onclick: {
                    let value = value.clone();
                    move |_| { let value = value.clone(); spawn(async move { if crate::transport::copy(value).await.is_ok() { copied.set(true); } }); }
                }, Copy { size: "15px" } "复制公钥" }
                Button { onclick: move |_| on_close.call(()), "关闭" }
            }
        }
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "verified" => "免密通过",
        "applied" => "已写入",
        "registered" => "已登记",
        "error" => "需要处理",
        _ => "待写入",
    }
}
