//! 语言表、文字系统识别、翻译方向推断。
pub struct Lang {
    pub code: &'static str,
    pub zh: &'static str,
    pub en: &'static str,
    pub script: &'static str,
}

pub const LANGUAGES: &[Lang] = &[
    Lang { code: "zh", zh: "中文", en: "Chinese", script: "han" },
    Lang { code: "ko", zh: "韩语", en: "Korean", script: "hangul" },
    Lang { code: "ja", zh: "日语", en: "Japanese", script: "kana" },
    Lang { code: "en", zh: "英语", en: "English", script: "latin" },
    Lang { code: "th", zh: "泰语", en: "Thai", script: "thai" },
    Lang { code: "vi", zh: "越南语", en: "Vietnamese", script: "latin" },
    Lang { code: "id", zh: "印尼语", en: "Indonesian", script: "latin" },
    Lang { code: "ru", zh: "俄语", en: "Russian", script: "cyrillic" },
    Lang { code: "es", zh: "西班牙语", en: "Spanish", script: "latin" },
    Lang { code: "fr", zh: "法语", en: "French", script: "latin" },
    Lang { code: "de", zh: "德语", en: "German", script: "latin" },
    Lang { code: "pt", zh: "葡萄牙语", en: "Portuguese", script: "latin" },
    Lang { code: "ar", zh: "阿拉伯语", en: "Arabic", script: "arabic" },
    Lang { code: "hi", zh: "印地语", en: "Hindi", script: "devanagari" },
];

pub fn language_name(code: &str) -> String {
    LANGUAGES
        .iter()
        .find(|l| l.code == code)
        .map(|l| l.zh.to_string())
        .unwrap_or_else(|| "原文语言".to_string())
}

fn count_in(text: &str, ranges: &[(u32, u32)]) -> usize {
    text.chars()
        .filter(|c| {
            let code = *c as u32;
            ranges.iter().any(|(lo, hi)| code >= *lo && code <= *hi)
        })
        .count()
}

fn script_of(text: &str) -> Option<&'static str> {
    let hangul = count_in(text, &[(0xAC00, 0xD7A3), (0x1100, 0x11FF), (0x3130, 0x318F)]);
    if hangul > 0 {
        return Some("hangul");
    }
    let kana = count_in(text, &[(0x3040, 0x309F), (0x30A0, 0x30FF)]);
    if kana > 0 {
        return Some("kana");
    }
    let thai = count_in(text, &[(0x0E00, 0x0E7F)]);
    if thai > 0 {
        return Some("thai");
    }
    let arabic = count_in(text, &[(0x0600, 0x06FF), (0x0750, 0x077F)]);
    if arabic > 0 {
        return Some("arabic");
    }
    let deva = count_in(text, &[(0x0900, 0x097F)]);
    if deva > 0 {
        return Some("devanagari");
    }
    let cyr = count_in(text, &[(0x0400, 0x04FF)]);
    if cyr > 0 {
        return Some("cyrillic");
    }
    let han = count_in(text, &[(0x4E00, 0x9FFF), (0x3400, 0x4DBF), (0xF900, 0xFAFF)]);
    let latin = count_in(text, &[(0x0041, 0x005A), (0x0061, 0x007A), (0x00C0, 0x024F)]);
    if han > 0 && han >= (latin as f64 * 0.3) as usize {
        return Some("han");
    }
    if latin > 0 {
        return Some("latin");
    }
    None
}

/// 识别语种：拉丁文暂按英语处理（后续可接 fasttext）。
pub fn detect_language(text: &str) -> Option<&'static str> {
    match script_of(text)? {
        "hangul" => Some("ko"),
        "kana" => Some("ja"),
        "thai" => Some("th"),
        "arabic" => Some("ar"),
        "devanagari" => Some("hi"),
        "cyrillic" => Some("ru"),
        "han" => Some("zh"),
        "latin" => Some("en"),
        _ => None,
    }
}

/// 返回 (源语言, 目标语言)
pub fn resolve_direction(text: &str, mine: &str, partners: &[String]) -> (String, String) {
    let partner = partners.first().cloned().unwrap_or_else(|| "en".to_string());
    let detected = detect_language(text).unwrap_or(mine);
    if detected == mine {
        (mine.to_string(), partner)
    } else {
        (detected.to_string(), mine.to_string())
    }
}
