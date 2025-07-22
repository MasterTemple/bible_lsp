use std::ops::{Deref, DerefMut};

use regex::Regex;
use tower_lsp::lsp_types::{Position, Range};

use crate::{autocompletion::AutocompleteState, bible_api::BibleAPI, re};

/// - This is a single chapter/verse reference
/// - Ex: `1:2` in `John 1:2`
#[derive(Clone, Debug)]
pub struct ChapterVerse {
    pub chapter: usize,
    pub verse: usize,
}

/// - This is a range of verse references within a single chapter
/// - Ex: `1:2-3` `John 1:2-3`
#[derive(Clone, Debug)]
pub struct ChapterRange {
    pub chapter: usize,
    pub start_verse: usize,
    pub end_verse: usize,
}

/// - This is a range of verse references across a multiple chapters
/// - Ex: `1:2-3:4` in `John 1:2-3:4`
#[derive(Clone, Debug)]
pub struct BookRange {
    pub start_chapter: usize,
    pub end_chapter: usize,
    pub start_verse: usize,
    pub end_verse: usize,
}

/// Remember, these correspond to
/// ```
///                `Ephesians 1:1-4,5-7,2:2-3:4,6`
///                          |     |   |       | |
///                ----------+     |   |       | |
/// ChapterRange:  `1:1-4`         |   |       | |
///                ----------------+   |       | |
/// ChapterRange:  `1:5-7`             |       | |
///                --------------------+       | |
/// BookRange:     `2:2-3:4`                   | |
///                ----------------------------+ |
/// ChatperVerse:  `3:6`                         |
///                ------------------------------+
/// ```
/// These should be grouped into a single reference
///
#[derive(Clone, Debug)]
pub enum BookReferenceSegment {
    /// - This is a single chapter/verse reference
    /// - Ex: `1:2` in `John 1:2`
    ChapterVerse(ChapterVerse),
    /// - This is a range of verse references within a single chapter
    /// - Ex: `1:2-3` `John 1:2-3`
    ChapterRange(ChapterRange),
    /// - This is a range of verse references across a multiple chapters
    /// - Ex: `John 1:2-3:4`
    BookRange(BookRange),
}

#[derive(Clone, Debug)]
pub struct BookReferenceSegments(pub Vec<BookReferenceSegment>);

impl BookReferenceSegments {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn parse(segment_input: &str) -> Self {
        parse_reference_segments(segment_input)
    }

    pub fn label(&self) -> String {
        let mut previous_chapter: Option<usize> = None;
        let mut label_segments: Vec<String> = vec![];
        // let mut label_str = String::new();
        for seg in self.0.iter() {
            let next_seg = match seg {
                BookReferenceSegment::ChapterVerse(chapter_verse) => {
                    if previous_chapter.is_some_and(|prev| prev == chapter_verse.chapter) {
                        format!("{}", chapter_verse.verse)
                    } else {
                        format!("{}:{}", chapter_verse.chapter, chapter_verse.verse)
                    }
                }
                BookReferenceSegment::ChapterRange(chapter_range) => {
                    if previous_chapter.is_some_and(|prev| prev == chapter_range.chapter) {
                        format!("{}-{}", chapter_range.start_verse, chapter_range.end_verse)
                    } else {
                        format!(
                            "{}:{}-{}",
                            chapter_range.chapter,
                            chapter_range.start_verse,
                            chapter_range.end_verse
                        )
                    }
                }
                BookReferenceSegment::BookRange(book_range) => {
                    if previous_chapter.is_some_and(|prev| prev == book_range.start_chapter) {
                        format!(
                            "{}-{}:{}",
                            book_range.start_verse, book_range.end_chapter, book_range.end_verse
                        )
                    } else {
                        format!(
                            "{}:{}-{}:{}",
                            book_range.start_chapter,
                            book_range.start_verse,
                            book_range.end_chapter,
                            book_range.end_verse
                        )
                    }
                }
            };
            let ending_chapter = seg.get_ending_chapter();
            // // if new chapter, add '; '
            // if previous_chapter.is_some_and(|prev| prev != ending_chapter) {
            //     label_segments.push(String::from("; "));
            // }
            // // if same chapter, add ','
            // else {
            //     label_segments.push(String::from(","));
            // }
            if let Some(prev) = previous_chapter {
                match prev == ending_chapter {
                    // if same chapter, add ','
                    true => label_segments.push(String::from(",")),
                    // if new chapter, add '; '
                    false => label_segments.push(String::from("; ")),
                }
            }
            label_segments.push(next_seg);
            previous_chapter = Some(ending_chapter);
        }
        label_segments.join("")
    }
}

impl Deref for BookReferenceSegments {
    type Target = Vec<BookReferenceSegment>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BookReferenceSegments {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BookReferenceSegment {
    pub fn get_starting_verse(&self) -> usize {
        match self {
            BookReferenceSegment::ChapterVerse(chapter_verse) => chapter_verse.verse,
            BookReferenceSegment::ChapterRange(chapter_range) => chapter_range.start_verse,
            BookReferenceSegment::BookRange(book_range) => book_range.start_verse,
        }
    }

    pub fn get_starting_chapter(&self) -> usize {
        match self {
            BookReferenceSegment::ChapterVerse(chapter_verse) => chapter_verse.chapter,
            BookReferenceSegment::ChapterRange(chapter_range) => chapter_range.chapter,
            BookReferenceSegment::BookRange(book_range) => book_range.start_chapter,
        }
    }

    pub fn get_ending_verse(&self) -> usize {
        match self {
            BookReferenceSegment::ChapterVerse(chapter_verse) => chapter_verse.verse,
            BookReferenceSegment::ChapterRange(chapter_range) => chapter_range.end_verse,
            BookReferenceSegment::BookRange(book_range) => book_range.end_verse,
        }
    }

    pub fn get_ending_chapter(&self) -> usize {
        match self {
            BookReferenceSegment::ChapterVerse(chapter_verse) => chapter_verse.chapter,
            BookReferenceSegment::ChapterRange(chapter_range) => chapter_range.chapter,
            BookReferenceSegment::BookRange(book_range) => book_range.end_chapter,
        }
    }
}

const DIGITS_ONLY_MSG: &'static str =
    "Only digits in a capture group should always parse to an usize.";

/// - This function is meant to parse the `1:1-4,5-7,2:2-3:4,6` in `Ephesians 1:1-4,5-7,2:2-3:4,6`
/// - Don't pass it anything else please :)
/**
Passing `1` will result in
```no_run
[src/main.rs:27:5] parse_reference_segments("1") = [
    ChapterVerse(
        ChapterVerse {
            chapter: 1,
            verse: 1,
        },
    ),
]
```
Passing `1:` will result in
```no_run
[src/main.rs:28:5] parse_reference_segments("1:") = [
    ChapterVerse(
        ChapterVerse {
            chapter: 1,
            verse: 1,
        },
    ),
]
```
*/
fn parse_reference_segments(segment_input: &str) -> BookReferenceSegments {
    // swap weird hyphens with normal dash
    let input = &segment_input.replace("–", "-");
    // input now only contains the following characters: [\d,:;-]
    let input = re::non_segment_characters()
        .replace_all(&input, "")
        .to_string();

    // removing trailing non-digits (leading shouldn't exist)
    let input = re::trailing_non_digits()
        .replace_all(&input, "")
        .to_string();

    // split at , or ; (because there is no uniform standard)
    // now I only have ranges (or a single verse)
    let ranges: Vec<&str> = re::segment_splitters().split(input.as_str()).collect();
    // ALWAYS UPDATE THE CHAPTER SO I CAN USE IT WHEN ONLY VERSES ARE PROVIDED
    let mut chapter = 1;
    let mut segments: Vec<BookReferenceSegment> = Vec::new();
    for range in ranges {
        // if it is a range
        if let Some((left, right)) = range.split_once("-") {
            match (left.split_once(":"), right.split_once(":")) {
                // `ch1:v1 - ch2:v2`
                (Some((ch1, v1)), Some((ch2, v2))) => {
                    chapter = ch2.parse().expect(DIGITS_ONLY_MSG);
                    segments.push(BookReferenceSegment::BookRange(BookRange {
                        start_chapter: ch1.parse().expect(DIGITS_ONLY_MSG),
                        end_chapter: chapter,
                        start_verse: v1.parse().expect(DIGITS_ONLY_MSG),
                        end_verse: v2.parse().expect(DIGITS_ONLY_MSG),
                    }));
                }
                // `ch1:v1 - v2`
                (Some((ch1, v1)), None) => {
                    chapter = ch1.parse().expect(DIGITS_ONLY_MSG);
                    segments.push(BookReferenceSegment::ChapterRange(ChapterRange {
                        chapter,
                        start_verse: v1.parse().expect(DIGITS_ONLY_MSG),
                        end_verse: right.parse().expect(DIGITS_ONLY_MSG),
                    }));
                }
                // `v1 - ch2:v2`
                (None, Some((ch2, v2))) => {
                    let start_chapter = chapter;
                    chapter = ch2.parse().expect(DIGITS_ONLY_MSG);
                    segments.push(BookReferenceSegment::BookRange(BookRange {
                        start_chapter,
                        end_chapter: chapter,
                        start_verse: left.parse().expect(DIGITS_ONLY_MSG),
                        end_verse: v2.parse().expect(DIGITS_ONLY_MSG),
                    }));
                }
                // `v1 - v2`
                (None, None) => segments.push(BookReferenceSegment::ChapterRange(ChapterRange {
                    chapter,
                    start_verse: left.parse().expect(DIGITS_ONLY_MSG),
                    end_verse: right.parse().expect(DIGITS_ONLY_MSG),
                })),
            };
        }
        // else it is not a range, either `ch:v` or `v`
        else {
            // handle `ch:v`
            if let Some((ch, v)) = range.split_once(":") {
                chapter = ch.parse().expect(DIGITS_ONLY_MSG);
                segments.push(BookReferenceSegment::ChapterVerse(ChapterVerse {
                    chapter,
                    verse: v.parse().expect(DIGITS_ONLY_MSG),
                }))
            }
            // handle `v`
            else {
                let v = range.parse().expect(DIGITS_ONLY_MSG);
                segments.push(BookReferenceSegment::ChapterVerse(ChapterVerse {
                    chapter,
                    verse: v,
                }))
            }
        }
    }
    BookReferenceSegments(segments)
}
