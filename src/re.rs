use cached::proc_macro::cached;
use regex::Regex;

/// - This matches reference segments if they are at the start of the String
/// - The purpose is so that only what is right after a book name is matched
/// - This is designed to be used in segments that start with a book and go to the next
/// book, but after you slice out the book name `segment[book_name_len()..]`
///
/// Matches like the following:
/// ```text
/// Ephesians 1:1-4,5-7,2:2-3:4,6
///          |------------------|
///
/// eph. 1:1-4,5-7,2:2-3:4,6
///    |-------------------|
///
/// I read Ephesians 4:28, and it changed how I thought about money
///                 |---|
/// ```
/// - Note: the period is part of this match because otherwise it would be part of the name,
/// but I don't want to have to deal with that
/// - This works because I get rid of all [`non_segment_characters`] when parsing this data
/// - I make sure this ends with a number, so it won't match `Ephesians 4:28,` when it is a
/// grammatical comma and not part of the reference (like `Ephesians 4:28,30`)
#[cached(size = 1)]
pub fn post_book_valid_reference_segment_characters() -> Regex {
    // Regex::new(r"\.? *\d+:\d+[ \d,:;\-–]+").unwrap()
    // Regex::new(r"^ *\d+:\d+([ \d,:;\-–]+\d+)?").unwrap()
    // Regex::new(r"^ *\d+:(\d+ *[,:;\-–] *)?\d+").unwrap()
    Regex::new(r"^ *\d+:\d+( *[,:;\-–] *\d+)*").unwrap()
}

#[cached(size = 1)]
pub fn segment_characters() -> Regex {
    Regex::new(r"\.?[ \d,:;\-–]+").unwrap()
}

// #[cached(size = 1)]
// pub fn segment_characters() -> Regex {
//     Regex::new(r"\.?( *\d+[,:;\-–] *)+\d+").unwrap()
// }

/**
when autocompleting
i should extract
- book
- first chapter
- colon
- remaining segments (with first chapter and colon) up to the last digit
- the last symbol

*/
#[cached(size = 1)]
pub fn verse_auto_complete_segment() -> Regex {
    Regex::new(r"^ *\d+:\d+( *[,:;\-–] *\d+)*").unwrap()
}

#[cached(size = 1)]
pub fn incomplete_segment_start() -> Regex {
    Regex::new(r"^ *(\d+)(:)? *$").unwrap()
}

#[cached(size = 1)]
pub fn ends_with_segment_characters() -> Regex {
    Regex::new(r"\.?[ \d,:;\-–]+$").unwrap()
}

#[cached(size = 1)]
pub fn non_segment_characters() -> Regex {
    Regex::new(r"[^\d,:;-]+").unwrap()
}

#[cached(size = 1)]
pub fn trailing_non_digits() -> Regex {
    Regex::new(r"(\D+$)").unwrap()
}

#[cached(size = 1)]
pub fn segment_splitters() -> Regex {
    Regex::new("(,|;)").unwrap()
}

// match_all_completed_segments + this
#[cached(size = 1)]
pub fn remove_incomplete_segments() -> Regex {
    Regex::new(r"((?:)(\d+:)|(\d+[\-–]))$").unwrap()
}

/// - for sure matches a chapter
/// - purpose is to find last one (so just use)
#[cached(size = 1)]
pub fn chapter() -> Regex {
    Regex::new(r"(\d+)(:|$)").unwrap()
}

/// for sure matches a verse
#[cached(size = 1)]
pub fn verse() -> Regex {
    Regex::new(r"(\d+)([^:]|$)").unwrap()
}

/// for sure matches a verse
#[cached(size = 1)]
pub fn at_least_one_segment() -> Regex {
    Regex::new(r"\d+:\d+").unwrap()
}

/// for sure matches a verse
/// this will only match
#[cached(size = 1)]
pub fn non_segment_state() -> Regex {
    Regex::new(r"^ *(\d+)?(:)?(\d+)?$").unwrap()
}
