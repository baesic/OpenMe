use std::slice;

#[no_mangle]
pub extern "C" fn openme_version() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn openme_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

#[no_mangle]
pub extern "C" fn openme_dealloc(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, len);
    }
}

#[no_mangle]
pub extern "C" fn openme_match_score(
    text_ptr: *const u8,
    text_len: usize,
    query_ptr: *const u8,
    query_len: usize,
) -> i32 {
    let text = read_utf8(text_ptr, text_len);
    let query = read_utf8(query_ptr, query_len);
    score_match(&text, &query)
}

#[no_mangle]
pub extern "C" fn openme_should_show_tag(
    tags_ptr: *const u8,
    tags_len: usize,
    active_ptr: *const u8,
    active_len: usize,
) -> i32 {
    let tags = read_utf8(tags_ptr, tags_len);
    let active = read_utf8(active_ptr, active_len);
    if should_show_for_tag(&tags, &active) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn openme_next_theme_is_dark(current_ptr: *const u8, current_len: usize) -> i32 {
    let current = read_utf8(current_ptr, current_len);
    if next_theme_is_dark(&current) {
        1
    } else {
        0
    }
}

pub fn score_match(text: &str, query: &str) -> i32 {
    let terms = query
        .split_whitespace()
        .map(|term| term.trim().to_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return 1;
    }

    let haystack = text.to_lowercase();
    let mut score = 0;
    for term in terms {
        if haystack.contains(&term) {
            score += 10;
        }
        if haystack
            .split(|ch: char| !ch.is_alphanumeric())
            .any(|word| word == term)
        {
            score += 5;
        }
    }
    score
}

pub fn should_show_for_tag(csv_tags: &str, active: &str) -> bool {
    let active = active.trim();
    active.is_empty() || csv_tags.split(',').map(str::trim).any(|tag| tag == active)
}

pub fn next_theme_is_dark(current: &str) -> bool {
    current.trim() != "dark"
}

fn read_utf8(ptr: *const u8, len: usize) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    let bytes = unsafe { slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_matching_terms() {
        assert!(score_match("Rust WebAssembly blog", "rust wasm") > 0);
        assert_eq!(score_match("Rust WebAssembly blog", "python"), 0);
    }

    #[test]
    fn tag_filter_logic() {
        assert!(should_show_for_tag("rust,wasm", "rust"));
        assert!(!should_show_for_tag("rust,wasm", "ai"));
        assert!(should_show_for_tag("rust,wasm", ""));
    }

    #[test]
    fn theme_toggle_logic() {
        assert!(next_theme_is_dark("light"));
        assert!(!next_theme_is_dark("dark"));
    }
}
