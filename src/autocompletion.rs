use std::fmt::Display;

use cached::proc_macro::cached;
use tower_lsp::lsp_types::CompletionItem;

use crate::{
    bible_api::BibleAPI,
    book_reference_segment::{
        BookReferenceSegment, BookReferenceSegments, ChapterRange, ChapterVerse,
    },
    re,
};

#[derive(Clone, Debug)]
pub enum PartialReferenceSegment {
    /// previous chapter
    Range(usize),
}

#[derive(Clone, Debug)]
pub struct AutocompletionSegments {
    full_segments: BookReferenceSegments,
    partial_segment: Option<PartialReferenceSegment>,
}

// impl AutocompletionSegments {
//     pub fn from_segment_str(segment_input: &str) -> Self {
//         match re::autocomplete_ending().find(segment_input) {
//             Some(cap) => {
//                 let full_segments = BookReferenceSegments::parse(&segment_input[..cap.start()]);
//                 Self {
//                     full_segments,
//                     partial_segment: None,
//                 }
//             }
//             None => {
//                 let full_segments = BookReferenceSegments::parse(segment_input);
//                 Self {
//                     full_segments,
//                     partial_segment: None,
//                 }
//             }
//         }
//     }
// }

#[derive(Copy, Clone, Debug)]
pub enum AutocompletionEndingOperator {
    // when ends with a number
    None,
    // probably a ':'
    Chapter,
    /// Usually represented by ',' or ';'
    Break,
    /// Usually represented by '-' or '–'
    Through,
}

/**
- Do not look at the last digits being typed if it is touching the cursor without a word boundary
- This is because LSP will filter all options

Example:

When given
```text
Ephesians 1:1_
```
I do not need to filter my suggestions to say `Ephesians 1:1` or `Ephesians 1:10..19`
because the LSP will do that for me

*/
#[derive(Clone, Debug)]
pub enum AutocompleteState {
    /// when BooksOnly is found
    BooksOnly,
    /// only known after "{book} "
    ChaptersOnly { book_id: usize },
    /// only known after ":"
    VersesOnly { book_id: usize, chapter: usize },
    /// all other cases
    /// - the verse is the previous verse found, this IS NOT what the user is typing
    /// - given `Ephesians 1:2-`, the chapter and verse tell me information such as I should only
    ///   suggest verses `3..=23` and chapters `2..=6`
    ChaptersOrVerses {
        book_id: usize,
        chapter: usize,
        verse: usize,
        segments: BookReferenceSegments,
        operator: AutocompletionEndingOperator,
    },
}

impl AutocompleteState {
    pub fn give_suggestions(&self, api: &BibleAPI) -> Vec<BibleCompletion> {
        match self.clone() {
            AutocompleteState::BooksOnly => suggest_all_books(),
            AutocompleteState::ChaptersOnly { book_id } => {
                let chapter_count = api.get_book_chapter_count(book_id).expect("Valid book id");
                (1..=chapter_count)
                    .map(|chapter| BibleCompletion::Chapter(ChapterCompletion { book_id, chapter }))
                    .collect()
            }
            AutocompleteState::VersesOnly { book_id, chapter } => {
                let Some(verse_count) = api.get_chapter_verse_count(book_id, chapter) else {
                    // if chapter is invalid (out of bounds), I will return empty list
                    return vec![];
                };
                (1..=verse_count)
                    .map(|verse| {
                        BibleCompletion::Verse(VerseCompletion {
                            book_id,
                            chapter,
                            verse,
                            segments: BookReferenceSegments::new(),
                            operator: AutocompletionEndingOperator::Chapter,
                        })
                    })
                    .collect()
            }
            AutocompleteState::ChaptersOrVerses {
                book_id,
                chapter,
                verse,
                segments,
                operator,
            } => {
                let chapter_count = api.get_book_chapter_count(book_id).expect("Valid book id");
                let chapter_completions: Vec<BibleCompletion> = ((chapter + 1)..=chapter_count)
                    .map(|chapter| BibleCompletion::Chapter(ChapterCompletion { book_id, chapter }))
                    .collect();

                let Some(verse_count) = api.get_chapter_verse_count(book_id, chapter) else {
                    // if chapter is invalid (out of bounds), I will return empty list
                    return vec![];
                };
                let mut verse_completions: Vec<BibleCompletion> = ((verse + 1)..=verse_count)
                    .map(|verse| {
                        BibleCompletion::Verse(VerseCompletion {
                            book_id,
                            chapter,
                            verse,
                            segments: segments.clone(),
                            operator,
                        })
                    })
                    .collect();
                verse_completions.extend(chapter_completions);
                verse_completions
            }
        }
    }
    // fn format_preview(&self, api: &BibleAPI, book_reference: &BookReference) {
    //     let label = book_reference.format_reference(api);
    //     format!("### {label}")
    //     match self {
    //         AutocompleteState::BooksOnly => todo!(),
    //         AutocompleteState::ChaptersOnly { book_id } => todo!(),
    //         AutocompleteState::VersesOnly { book_id, chapter } => todo!(),
    //         AutocompleteState::ChaptersOrVerses { book_id, chapter, verse } => todo!(),
    //     }
    // }
}

#[derive(Clone, Debug)]
pub struct BookNameCompletion {
    pub book_id: usize,
}

#[derive(Clone, Debug)]
pub struct ChapterCompletion {
    pub book_id: usize,
    pub chapter: usize,
}

#[derive(Clone, Debug)]
pub struct VerseCompletion {
    pub book_id: usize,
    pub chapter: usize,
    pub verse: usize,
    pub segments: BookReferenceSegments,
    pub operator: AutocompletionEndingOperator,
}

// figure out how to use these when formatting
// pub segments: Box<Vec<BookReferenceSegment>>,

/*
NOTE: all of these LSP events might not be given to the LSP server

Alright, here is some big brain moves:

Check current line for last book of the Bible mentioned

If there is no book at all OR everything after the last book is not segments
    return: suggest books of the bible

If there is a last book that goes to the end of the line, they have typed part of a book and it is validly matched by the pattern but they want to keep typing the full name
    return: suggest books of the bible

If there is a last book followed by a space only or by a space and numbers only
    return: suggest chapters

Else parse segments for context (see later)
    If last non-digit character is ':'
        return: suggest verses
    Else:
        return: suggest verses greater than previous number and less than verses in current chapter PLUS all chapters after the last chapter

ALGORITHM 2

trim input to everything before cursor

if (the end is segments):
    if before segments is a book:
        parse segments and suggest chapter/verse
    else:
        suggest book
else:
    suggest book

*/
#[derive(Clone, Debug)]
pub enum BibleCompletion {
    BookName(BookNameCompletion),
    Chapter(ChapterCompletion),
    Verse(VerseCompletion),
}

impl BibleCompletion {
    pub fn print(&self, api: &BibleAPI) -> String {
        let display = match &self {
            BibleCompletion::BookName(BookNameCompletion { book_id }) => {
                format!("{}", api.get_book_name(*book_id).unwrap())
            }
            BibleCompletion::Chapter(ChapterCompletion { book_id, chapter }) => {
                format!("{} {}", api.get_book_name(*book_id).unwrap(), chapter)
            }
            BibleCompletion::Verse(VerseCompletion {
                book_id,
                chapter,
                verse,
                segments,
                operator,
            }) => {
                format!(
                    "{} {}:{}",
                    api.get_book_name(*book_id).unwrap(),
                    chapter,
                    verse
                )
            }
        };
        // println!("{}", display);
        display
    }

    pub fn label(&self, api: &BibleAPI) -> String {
        match self.clone() {
            BibleCompletion::BookName(BookNameCompletion { book_id }) => {
                let book_name = api.get_book_name(book_id).unwrap();
                // format!("{book_name}")
                book_name
            }
            BibleCompletion::Chapter(ChapterCompletion { book_id, chapter }) => {
                let book_name = api.get_book_name(book_id).unwrap();
                format!("{book_name} {chapter}")
            }
            BibleCompletion::Verse(VerseCompletion {
                book_id,
                chapter,
                verse,
                mut segments,
                operator,
            }) => {
                // segments.push(BookReferenceSegment::ChapterVerse(ChapterVerse {
                //     chapter,
                //     verse,
                // }));
                match operator {
                    AutocompletionEndingOperator::None => {
                        // removing last element because it is incomplete
                        // let _ = segments.pop();
                    }
                    AutocompletionEndingOperator::Chapter => (),
                    AutocompletionEndingOperator::Break => {
                        segments.push(BookReferenceSegment::ChapterVerse(ChapterVerse {
                            chapter,
                            verse,
                        }));
                    }
                    AutocompletionEndingOperator::Through => {
                        let start_verse = segments
                            .last()
                            .expect("I'm pretty sure it always has a segment")
                            .get_ending_verse();
                        // remove last segment because it is a single
                        // ChapteVerse but it really is an incomplete range
                        let _ = segments.pop();
                        segments.push(BookReferenceSegment::ChapterRange(ChapterRange {
                            chapter,
                            start_verse,
                            end_verse: verse,
                        }));
                    }
                };
                format!(
                    "{} {}",
                    api.get_book_name(book_id).unwrap(),
                    segments.label()
                )
            }
        }
    }
    pub fn lsp_preview(&self, api: &BibleAPI) -> String {
        // return format!("```rust\n{self:?}\n```");
        match self.clone() {
            BibleCompletion::BookName(BookNameCompletion { book_id }) => {
                let book_name = api.get_book_name(book_id).unwrap();
                format!("### {book_name}")
            }
            BibleCompletion::Chapter(ChapterCompletion { book_id, chapter }) => {
                let book_name = api.get_book_name(book_id).unwrap();
                let content = api
                    .get_all_verses(book_id, chapter)
                    .expect("Valid book id")
                    .filter_map(|verse| {
                        api.get_bible_contents(book_id, chapter, verse)
                            .map(|content| format!("[{}:{}] {}", chapter, verse, content))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("### {book_name} {chapter}\n\n{content}")
            }
            BibleCompletion::Verse(VerseCompletion {
                book_id,
                chapter,
                verse,
                mut segments,
                operator,
            }) => {
                // ! this should be based on the type of the segment if it is , or -
                match operator {
                    AutocompletionEndingOperator::None => {
                        // removing last element because it is incomplete
                        // let _ = segments.pop();
                    }
                    AutocompletionEndingOperator::Chapter => (),
                    AutocompletionEndingOperator::Break => {
                        segments.push(BookReferenceSegment::ChapterVerse(ChapterVerse {
                            chapter,
                            verse,
                        }));
                    }
                    AutocompletionEndingOperator::Through => {
                        let start_verse = segments
                            .last()
                            .expect("I'm pretty sure it always has a segment")
                            .get_ending_verse();
                        // remove last segment because it is a single
                        // ChapteVerse but it really is an incomplete range
                        let _ = segments.pop();
                        segments.push(BookReferenceSegment::ChapterRange(ChapterRange {
                            chapter,
                            start_verse,
                            end_verse: verse,
                        }));
                    }
                };
                // segments.push(BookReferenceSegment::ChapterVerse(ChapterVerse {
                //     chapter,
                //     verse,
                // }));
                let label = format!(
                    "{} {}",
                    api.get_book_name(book_id).unwrap(),
                    segments.label()
                );
                let content = segments
                    .iter()
                    .map(|seg| {
                        let mut contents = vec![];
                        for chapter in seg.get_starting_chapter()..=seg.get_ending_chapter() {
                            for verse in seg.get_starting_verse()..=seg.get_ending_verse() {
                                if let Some(content) =
                                    api.get_bible_contents(book_id, chapter, verse)
                                {
                                    contents.push(format!("[{}:{}] {}", chapter, verse, content));
                                }
                            }
                        }
                        contents.join("\n")
                    })
                    .collect::<Vec<String>>()
                    .join("\n\n");
                format!("### {label}\n\n{content}")
            }
        }
    }
}

/// It is probably more valuable to cache the one that actually formats everything, but oh well
#[cached(size = 1)]
pub fn suggest_all_books() -> Vec<BibleCompletion> {
    (1..=66)
        .map(|book_id| BibleCompletion::BookName(BookNameCompletion { book_id }))
        .collect()
}

// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_autocomplete() {
//         let json_path = "/home/dgmastertemple/Development/rust/bible_api/esv.json";
//         let api = BibleAPI::new(json_path);
//         // let suggestions = AutocompleteState::BooksOnly.give_suggestions(&api);
//         // let suggestions = AutocompleteState::ChaptersOnly { book_id: 49 }.give_suggestions(&api);
//         // let suggestions = AutocompleteState::VersesOnly {
//         //     book_id: 49,
//         //     chapter: 2,
//         // }
//         let suggestions = AutocompleteState::ChaptersOrVerses {
//             book_id: 49,
//             chapter: 2,
//             verse: 3,
//             segments: BookReferenceSegments::new(),
//             operator: AutocompletionEndingOperator::Through,
//         }
//         .give_suggestions(&api);
//         for sug in suggestions {
//             sug.print(&api);
//         }
//     }
// }

fn get_last_chapter_and_verse(segment_input: &str) -> (Option<usize>, Option<usize>) {
    let last_chapter = re::chapter()
        .captures_iter(segment_input)
        .last()
        .map(|cap| cap.get(1).expect("Required group"));

    let last_verse = re::verse()
        .captures_iter(segment_input)
        .last()
        .map(|cap| cap.get(1).expect("Required group"));

    let (chapter, verse) = match (last_chapter, last_verse) {
        // book name is the only thing typed
        (None, None) => (None, None),
        // these cases can't exist, because the only case in which one would exist is
        // when only the chapter is typed, but both actually match
        // which is why i am doing what i do below
        (None, Some(_)) | (Some(_), None) => (None, None),
        (Some(chapter), Some(verse)) => {
            // there is only one overlapping case for the regex, and that is if there is
            // one set of digits touching the end (which is the chapter)
            if chapter.start() == verse.start() {
                (Some(chapter), None)
            }
            // the last verse comes before the last chapter
            // meaning we don't know the last verse
            else if chapter.start() > verse.start() {
                (Some(chapter), None)
            } else {
                (Some(chapter), Some(chapter))
            }
        }
    };
    let chapter = chapter.map(|c| c.as_str().parse::<usize>().expect("Digits capture group"));
    let verse = verse.map(|v| v.as_str().parse::<usize>().expect("Digits capture group"));

    (chapter, verse)
}

pub enum CompletionJoiner {
    Range,
    Break,
}

pub struct CompletionSegmentsState {
    pub segments: BookReferenceSegments,
    pub current_chapter: Option<usize>,
    pub current_verse: Option<usize>,
    pub joiner: CompletionJoiner,
}

impl CompletionSegmentsState {
    /// hey now, only call me if there real segments to parse :D :D :D
    pub fn parse(segment_input: &str) -> CompletionSegmentsState {
        let full_segments_input = re::remove_incomplete_segments().replace(segment_input, "");
        // gotta make sure there are valid segments before passing it to the parse function
        let segments = if re::at_least_one_segment().is_match(segment_input) {
            BookReferenceSegments::parse(&full_segments_input)
        } else {
            BookReferenceSegments::new()
        };

        let (current_chapter, current_verse) = get_last_chapter_and_verse(segment_input);
        // so given current chapter and verse, i need to suggest a number
        // that number is either a chapter or a verse
        // as well as if they are joined by a range (-) or if they are disconnected
        //
        let joiner = match segment_input
            .chars()
            .last()
            // .expect("I think if there wasn't an ending char it would not have gotten this far")
        {
            Some('-') | Some('–') => CompletionJoiner::Range,
            _ => CompletionJoiner::Break,
        };

        Self {
            segments,
            current_chapter,
            current_verse,
            joiner,
        }
    }
}

pub struct APICompletionSegment<'a> {
    api: &'a BibleAPI,
    book_id: usize,
    segment_state: CompletionSegmentsState,
}

pub struct CompletionItemData {
    label: String,
    documentation: String,
}

impl<'a> APICompletionSegment<'a> {
    // pub fn lsp_label(&self) -> String {
    //     self.segment_state.segments.label()
    // }
    // pub fn lsp_preview
    pub fn completion_items(&self) -> Vec<CompletionItemData> {
        let last_chapter = self.segment_state.current_chapter;
        let last_verse = self.segment_state.current_verse;

        vec![]
    }
}

/*
alright here is the algorithm

if no book => suggest_all_books()

if re::at_least_one_segment() => parse segments
else segments = vec![]

match re::non_segment_state() {
    group 1 only => suggest chapters only
    group 1 and 2 => suggest verses only
}

determine last chapter and verse

match ending {
    ":" => suggest verses only
    _ => suggest chapters or verses
}

save also relation (range or break)

*/

mod tests {
    use super::*;

    #[test]
    fn test_autocomplete() {
        // basically assert the suggest_function() results .len() == what i expect
        // assert_eq!()
    }
}
