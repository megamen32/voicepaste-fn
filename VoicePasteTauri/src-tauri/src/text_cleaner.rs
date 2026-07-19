/// Strip common subtitle-channel boilerplate from the end of transcripts.

const UNWANTED_SUFFIXES: &[&str] = &[
    "продолжение следует",
    "субтитры сделал dimatorzok",
    "субтитры сделаны dimatorzok",
    "subtitles by dimatorzok",
    "subtitles made by dimatorzok",
    "to be continued",
    "thanks for watching",
];

const TRAILING_PUNCT: &[char] = &['.', '!', '?', '*', ';', ':'];

pub struct TextCleaner;

impl TextCleaner {
    pub fn clean(text: &str) -> String {
        let mut result = text.trim().to_string();
        if result.is_empty() {
            return result;
        }

        let lower = result.to_lowercase();

        for suffix in UNWANTED_SUFFIXES {
            let lower_suffix = suffix.to_lowercase();
            if lower_suffix.is_empty() {
                continue;
            }

            // Try bare suffix first
            let mut matched = lower.ends_with(&lower_suffix);

            // Try with trailing punctuation stripped
            if !matched {
                let mut probe = lower.clone();
                for _ in 0..5 {
                    if probe.ends_with(&lower_suffix) {
                        matched = true;
                        break;
                    }
                    if let Some(c) = probe.chars().last() {
                        if TRAILING_PUNCT.contains(&c) {
                            probe.pop();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }

            if !matched {
                continue;
            }

            // Find cut point in original-case text
            // Walk backwards over trailing punctuation
            let mut cut = result.len();
            while cut > 0 {
                let ch = result[..cut].chars().last().unwrap();
                if TRAILING_PUNCT.contains(&ch) {
                    cut -= ch.len_utf8();
                } else {
                    break;
                }
            }
            // Walk backwards over the suffix itself
            let suffix_byte_len = suffix.len();
            if cut >= suffix_byte_len {
                cut -= suffix_byte_len;
            } else {
                cut = 0;
            }

            result = result[..cut].trim().to_string();
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_suffix_returns_trimmed() {
        assert_eq!(TextCleaner::clean("  hello world  "), "hello world");
    }

    #[test]
    fn test_strips_continuation() {
        assert_eq!(
            TextCleaner::clean("Hello world. Продолжение следует"),
            "Hello world."
        );
    }

    #[test]
    fn test_strips_with_trailing_dots() {
        assert_eq!(
            TextCleaner::clean("Hello world. Продолжение следует..."),
            "Hello world."
        );
    }

    #[test]
    fn test_strips_thanks_for_watching() {
        assert_eq!(
            TextCleaner::clean("Some text. Thanks for watching!"),
            "Some text."
        );
    }

    #[test]
    fn test_strips_to_be_continued() {
        assert_eq!(
            TextCleaner::clean("Some text. To be continued..."),
            "Some text."
        );
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(TextCleaner::clean(""), "");
    }

    #[test]
    fn test_only_suffix() {
        assert_eq!(TextCleaner::clean("Thanks for watching"), "");
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(
            TextCleaner::clean("Text. THANKS FOR WATCHING!!!"),
            "Text."
        );
    }
}
