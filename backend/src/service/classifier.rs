use regex::Regex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatIntent {
    Greeting,
    Save,
    Recall,
    Model,
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub intent: ChatIntent,
    pub query: String,
}

pub fn classify(sanitized: &str) -> Decision {
    let text = sanitized.trim();
    let greeting = text
        .trim_end_matches(['!', '！', '?', '？', '.', '。', '~', '～'])
        .to_ascii_lowercase();
    if [
        "hi",
        "hello",
        "hey",
        "你好",
        "您好",
        "嗨",
        "哈喽",
        "在吗",
        "早上好",
        "下午好",
        "晚上好",
    ]
    .contains(&greeting.as_str())
    {
        return Decision {
            intent: ChatIntent::Greeting,
            query: text.to_owned(),
        };
    }
    let reference = Regex::new(r"\[\[secret:[a-f0-9]{32}\]\]").expect("固定引用正则有效");
    let question = reference.replace_all(text, "").trim().to_owned();
    let comparable = question.to_ascii_lowercase();
    let complex = Regex::new(r"(分析|比较|对比|总结|归纳|解释|为什么|为何|怎么|如何|建议|规划|设计|推理|翻译|生成|撰写|编写|\b(analy[sz]e|compare|summari[sz]e|explain|why|how|recommend|plan|write|translate)\b)")
        .expect("固定复杂意图正则有效");
    if complex.is_match(&comparable) {
        return Decision {
            intent: ChatIntent::Model,
            query: question,
        };
    }
    if !reference.is_match(text)
        && !question.contains('\n')
        && !question.contains(';')
        && !question.contains('；')
        && question.chars().count() <= 180
    {
        if let Some(subject) = lookup_subject(&question) {
            return Decision {
                intent: ChatIntent::Recall,
                query: subject,
            };
        }
    }
    let structured =
        question.trim_start().starts_with('{') || question.trim_start().starts_with('[');
    let save = Regex::new(r"^(?:请)?(?:帮我)?(?:记下|记住|记录|保存|备忘|收下)(?:来)?[\s:：，,][\s\S]+|[\s\S]+[，,。\s](?:请)?(?:帮我)?(?:记下|记住|保存|记录)(?:来)?[。!！]*$|^(?:remember|save|note)\s+[\s\S]+")
        .expect("固定保存正则有效");
    if save.is_match(&comparable)
        || structured
        || (reference.is_match(text) && !question.contains('?') && !question.contains('？'))
    {
        return Decision {
            intent: ChatIntent::Save,
            query: question,
        };
    }
    Decision {
        intent: ChatIntent::Model,
        query: question,
    }
}

fn lookup_subject(question: &str) -> Option<String> {
    let lower = question.to_ascii_lowercase();
    let prefixes = [
        "请",
        "帮我",
        "查找",
        "搜索",
        "检索",
        "查一下",
        "查下",
        "找一下",
        "找下",
        "找出",
        "查询",
        "查阅",
        "查看",
    ];
    let mut subject = None;
    for prefix in prefixes {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let rest = rest.trim_start_matches([':', '：', ' ']);
            if !rest.is_empty() {
                subject = Some(rest.to_owned());
                break;
            }
        }
    }
    if subject.is_none() {
        for prefix in ["find", "search", "search for", "look up", "lookup", "show"] {
            if let Some(rest) = lower.strip_prefix(prefix) {
                let rest = rest.trim_start();
                if !rest.is_empty() {
                    subject = Some(rest.to_owned());
                    break;
                }
            }
        }
    }
    let fact = Regex::new(
        r"^(.{2,80}?)(?:是什么|是啥|在哪里|在哪儿|在哪|是多少|是哪天|是什么时候)[？?。!！]*$",
    )
    .expect("固定事实问句正则有效");
    if subject.is_none() {
        subject = fact
            .captures(&lower)
            .and_then(|capture| capture.get(1).map(|value| value.as_str().to_owned()));
    }
    subject
        .map(|value| {
            value
                .trim()
                .trim_end_matches(['?', '？', '。', '!', '！'])
                .to_owned()
        })
        .filter(|value| {
            !value.is_empty()
                && !value.contains([',', '，', '。', '？', '?'])
                && !Regex::new(r"然后|并且|顺便|以及|\band\b")
                    .expect("固定复合词正则有效")
                    .is_match(value)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_greeting_lookup_save_and_model() {
        assert_eq!(classify("你好").intent, ChatIntent::Greeting);
        assert_eq!(classify("查找 Aurora").intent, ChatIntent::Recall);
        assert_eq!(classify("项目是什么").intent, ChatIntent::Recall);
        assert_eq!(classify("记住 明天开会").intent, ChatIntent::Save);
        assert_eq!(classify("分析一下资料").intent, ChatIntent::Model);
    }
}
