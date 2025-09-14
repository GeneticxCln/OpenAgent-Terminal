//! Pure helpers for composer text editing and word navigation.

pub fn is_word_char(c: char) -> bool {
    // Treat alphanumerics and '_' and '-' as wordish; adjust as needed
    c.is_alphanumeric() || c == '_' || c == '-'
}

pub fn move_word_left(text: &str, cursor: usize) -> usize {
    if cursor == 0 { return 0; }
    let mut idx = cursor;
    // Move left skipping spaces first
    while idx > 0 {
        let mut it = text[..idx].chars();
        let ch = it.next_back().unwrap_or('\0');
        if ch.is_whitespace() { idx -= ch.len_utf8(); } else { break; }
    }
    // Then skip the word characters
    while idx > 0 {
        let mut it = text[..idx].chars();
        let ch = it.next_back().unwrap_or('\0');
        if is_word_char(ch) { idx -= ch.len_utf8(); } else { break; }
    }
    idx
}

pub fn move_word_right(text: &str, cursor: usize) -> usize {
    if cursor >= text.len() { return text.len(); }
    let mut idx = cursor;
    let iter = text[cursor..].char_indices();
    // Skip current/next word run
    let mut in_word = None;
    for (off, ch) in iter {
        let at = cursor + off;
        let w = is_word_char(ch);
        match in_word {
            None => in_word = Some(w),
            Some(state) if state && !w => { return at; },
            Some(state) if !state && w => { // start of next word; keep going to its end
                in_word = Some(true);
            },
            _ => {}
        }
        idx = at + ch.len_utf8();
    }
    idx
}

#[allow(dead_code)]
pub fn delete_word_left(text: &str, cursor: usize) -> (String, usize) {
    let start = move_word_left(text, cursor);
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[cursor..]);
    (out, start)
}

#[allow(dead_code)]
pub fn delete_word_right(text: &str, cursor: usize) -> (String, usize) {
    let end = move_word_right(text, cursor);
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..cursor]);
    out.push_str(&text[end..]);
    (out, cursor)
}

#[allow(dead_code)]
pub fn delete_to_start(text: &str, cursor: usize) -> (String, usize) {
    let mut out = String::new();
    out.push_str(&text[cursor..]);
    (out, 0)
}

#[allow(dead_code)]
pub fn delete_to_end(text: &str, cursor: usize) -> (String, usize) {
    let mut out = String::with_capacity(cursor);
    out.push_str(&text[..cursor]);
    (out, cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_nav_basic() {
        let s = "git checkout main";
        // Move-left from end should land at the start index of the previous word ("main"),
        // which is character index 13 in this string.
        assert_eq!(move_word_left(s, s.len()), 13);
        assert!(move_word_right(s, 0) > 0);
    }

    #[test]
    fn delete_word_edges() {
        let s = "hello world";
        let (out, cur) = delete_word_left(s, s.len());
        assert_eq!(out, "hello ");
        assert!(cur < s.len());
        let (out2, cur2) = delete_word_right("rm  -rf", 3);
        assert!(cur2 <= out2.len());
    }
}