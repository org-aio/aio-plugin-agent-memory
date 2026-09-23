use regex::Regex;
use serde_json::{Map, Value};

#[derive(Clone, Debug)]
pub struct IsolatedSecret {
    pub id: String,
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug)]
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
        if let Some(existing) = secrets
            .iter()
            .find(|secret: &&IsolatedSecret| secret.value == value)
        {
            return format!("[[secret:{}]]", existing.id);
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        secrets.push(IsolatedSecret {
            id: id.clone(),
            label: label.to_owned(),
            value: value.to_owned(),
        });
        format!("[[secret:{id}]]")
    };
    let mut text = protect_private_keys(&checked, &mut protect);
    text = protect_assignments(&text, &mut protect, &uncertain);
    text = protect_bearer(&text, &mut protect);
    text = protect_known_tokens(&text, &mut protect);
    let structured = serde_json::from_str::<Value>(checked.trim())
        .ok()
        .map(|value| visit(value, &mut protect));
    let initial = structured.as_ref().map(Value::to_string).unwrap_or(text);
    if serde_json::from_str::<Value>(checked.trim()).is_err()
        && (checked.trim_start().starts_with('{') || checked.trim_start().starts_with('['))
        && assignment().is_match(&checked.to_ascii_lowercase())
    {
        uncertain.set(true);
    }
    let mut safe = initial;
    let mut ordered = secrets.clone();
    ordered.sort_by(|left, right| right.value.len().cmp(&left.value.len()));
    for secret in &ordered {
        safe =
            replace_outside_references(&safe, &secret.value, &format!("[[secret:{}]]", secret.id));
    }
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

fn visit(mut value: Value, protect: &mut impl FnMut(&str, &str) -> String) -> Value {
    match &mut value {
        Value::Object(object) => {
            let entries = std::mem::take(object);
            let mut next = Map::new();
            for (key, value) in entries {
                let protected = if sensitive(&key) {
                    Value::String(protect(&key, &scalar(&value)))
                } else {
                    visit(value, protect)
                };
                next.insert(key, protected);
            }
            Value::Object(next)
        }
        Value::Array(values) => {
            for value in values {
                *value = visit(value.take(), protect);
            }
            value
        }
        Value::String(value) => Value::String(protect("secret", value.as_str())),
        _ => value,
    }
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
            let value = captures.get(2).map(|value| value.as_str()).unwrap_or("");
            let value = value.trim_matches(|ch| ch == '"' || ch == '\'');
            if value.is_empty() {
                uncertain.set(true);
            }
            format!("{}: {}", &captures[1], protect(&captures[1], value))
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

fn sensitive(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|value| value.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    [
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
    ]
    .iter()
    .any(|item| normalized.ends_with(item))
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
}
