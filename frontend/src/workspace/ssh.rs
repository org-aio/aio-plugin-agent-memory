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
                Button {
                    disabled: state.read().busy || state.read().ssh_devices.is_empty(),
                    onclick: move |_| creating.set(true),
                    Plus { size: "16px" }
                    "添加主机"
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
                        on_delete: move |value| deleting.set(Some(value)),
                        on_key: move |value| public_key.set(Some(value)),
                    }
                }
            }
        }
        if creating() {
            SshHostDialog { on_close: move |_| creating.set(false) }
        }
        if let Some(host) = editing() {
            SshHostDialog { existing: Some(host), on_close: move |_| editing.set(None) }
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
    on_delete: EventHandler<SshHost>,
    on_key: EventHandler<String>,
) -> Element {
    let mut state = use_context::<Signal<MemoryState>>();
    let mut busy = use_signal(|| false);
    let id = host.id.clone();
    let edit = host.clone();
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
                    onclick: {
                        let id = id.clone();
                        move |_| {
                            let id = id.clone(); busy.set(true);
                            spawn(async move {
                                if let Ok(result) = state::apply_ssh_host(state, id).await { state.write().notice = Some(result.message); }
                                busy.set(false);
                            });
                        }
                    },
                    Server { size: "15px" }
                    if busy() { "写入中" } else { "写入设备" }
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
    let mut identity = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.identity_file.clone())
            .unwrap_or_else(|| "id_ed25519".into())
    });
    let mut device = use_signal(|| {
        existing
            .as_ref()
            .map(|host| host.device_id.clone())
            .unwrap_or_else(|| {
                state
                    .read()
                    .ssh_devices
                    .first()
                    .map(|device| device.id.clone())
                    .unwrap_or_default()
            })
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
            DialogDescription { "保存后点击“写入设备”，配置会写入目标设备的 ~/.ssh/config 托管块。" }
            div { class: "memory-ssh-form",
                TextInput { label: "Alias", value: alias(), on_change: move |value| alias.set(value), placeholder: Some("okm".into()) }
                TextInput { label: "主机地址", value: hostname(), on_change: move |value| hostname.set(value), placeholder: Some("61.163.60.13".into()) }
                div { class: "memory-ssh-form__split",
                    TextInput { label: "用户", value: user(), on_change: move |value| user.set(value), placeholder: Some("root".into()) }
                    TextInput { label: "端口", value: port(), input_type: "number", on_change: move |value| port.set(value) }
                }
                label { class: "memory-ssh-form__field", "配对设备"
                    select { value: device(), onchange: move |event| device.set(event.value()),
                        for item in devices { option { value: "{item.id}", "{item.label} · {item.platform}" } }
                    }
                }
                label { class: "memory-ssh-form__field", "身份文件"
                    select { value: identity().replace(".pub", ""), onchange: move |event| identity.set(event.value()),
                        option { value: "id_ed25519", "~/.ssh/id_ed25519" }
                        option { value: "id_rsa", "~/.ssh/id_rsa" }
                        option { value: "id_ecdsa", "~/.ssh/id_ecdsa" }
                    }
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
                    let draft = SshHostDraft {
                        alias: alias(), hostname: hostname(), user: user(), port: parsed,
                        identity_file: identity().replace(".pub", ""), device_id: device(), version,
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
        "error" => "需要处理",
        _ => "待写入",
    }
}
