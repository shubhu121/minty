use unicode_segmentation::UnicodeSegmentation;
use whatlang::detect;

pub struct LangInfo {
    pub lang: String,
}

pub fn detect_language(text: &str) -> Option<LangInfo> {
    detect(text).map(|info| LangInfo {
        lang: info.lang().eng_name().to_string(),
    })
}

pub fn token_count_multilingual(text: &str) -> usize {
    let count = text.unicode_words().count();
    (count as f64 * 1.3).round() as usize
}

pub fn split_sentences_multilingual(text: &str) -> Vec<&str> {
    text.unicode_sentences().collect()
}
