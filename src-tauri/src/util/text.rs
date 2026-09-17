pub fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn collapse_blank_lines(input: &str) -> String {
    let mut output = String::new();
    let mut blank_count = 0usize;
    for line in normalize_newlines(input).lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                output.push('\n');
            }
        } else {
            blank_count = 0;
            output.push_str(trimmed);
            output.push('\n');
        }
    }
    output.trim().to_owned()
}

pub fn estimate_tokens(text: &str) -> i64 {
    // Conservative estimate for English/Chinese mixed text. It is only used for
    // display and scheduling, never for billing or hard limits.
    let characters = text.chars().count() as i64;
    ((characters + 3) / 4).max(1)
}

pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_collapses() {
        let value = normalize_newlines("a\r\n\r\n\r\nb");
        assert_eq!(value, "a\n\n\nb");
        assert_eq!(collapse_blank_lines("a\r\n\r\n\r\nb"), "a\n\nb");
    }
}
