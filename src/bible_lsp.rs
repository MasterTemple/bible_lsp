use std::io::Write;
use std::{
    fs::{self, OpenOptions},
    io,
};

use tower_lsp::lsp_types::{Position, Range};

use crate::{
    autocompletion::{
        suggest_all_books, AutocompleteState, AutocompletionEndingOperator, BibleCompletion,
        BookNameCompletion,
    },
    bible_api::BibleAPI,
    book_reference::BookReference,
    book_reference_segment::{self, BookReferenceSegments},
    re,
};

#[derive(Clone, Debug)]
pub struct BibleLSP {
    pub api: BibleAPI,
}

fn calculate_position(newline_indexes: &Vec<usize>, start_index: usize, end_index: usize) -> Range {
    // If there is one line or match is on the first line
    if newline_indexes.len() == 0 || start_index < newline_indexes[0] {
        return Range {
            start: Position {
                line: 0,
                character: start_index as u32,
            },
            end: Position {
                line: 0,
                character: end_index as u32,
            },
        };
    }

    // If the match is on the last line
    if *newline_indexes
        .last()
        .expect("Previous if statement guarantees len > 0")
        < start_index
    {
        let line = newline_indexes.len() as u32;
        let line_start_index = *newline_indexes
            .last()
            .expect("Previous if statement guarantees len > 0");
        // im sure the off-by-one error is from cr-lf \r\n
        let start_character = (start_index - 1 - line_start_index) as u32;
        let end_character = (end_index - 1 - line_start_index) as u32;
        return Range {
            start: Position {
                line,
                character: start_character,
            },
            end: Position {
                line,
                character: end_character,
            },
        };
    }

    // With the above cases out of the way, at any given index (1..len()-1) I can just the
    // adjacent one and it is guaranteed to be in bounds
    let mut bottom = 1;
    let mut top = newline_indexes.len() - 1;
    let mut mid = top / bottom;

    while top != bottom {
        // okay, maybe i want to just remove if the first one is it and then just always
        // check left
        // the below case may handle the end one, but i dont want to think about it right now so i
        // will be content to let it handle it as its own case if it wants to
        if newline_indexes[mid - 1] < start_index && start_index < newline_indexes[mid] {
            break;
        } else if start_index < newline_indexes[mid] {
            top = mid;
        } else {
            bottom = mid;
        }
        mid = bottom + ((top - bottom) / 2);
    }

    let line = mid as u32;
    let line_start_index = newline_indexes[mid - 1];
    let start_character = (start_index - 1 - line_start_index) as u32;
    let end_character = (end_index - 1 - line_start_index) as u32;
    return Range {
        start: Position {
            line,
            character: start_character,
        },
        end: Position {
            line,
            character: end_character,
        },
    };
}

const NOTHING: (Option<usize>, Option<usize>, Option<usize>) = (None, None, None);
/**
Returns current book id, current chapter, and current verse
*/
fn parse_current_state(api: &BibleAPI, text_before_cursor: &str) -> AutocompleteState {
    let mut progress = AutocompleteState::BooksOnly;
    let Some(book_match) = api
        .book_abbreviation_regex()
        .find_iter(text_before_cursor)
        .last()
    else {
        return progress;
    };
    let everything_after_book_name = &text_before_cursor[book_match.end()..];
    if everything_after_book_name.len() == 0 {
        return progress;
    }
    let Some(book_id) = api.get_book_id(book_match.as_str()) else {
        return progress;
    };
    // progress.0 = Some(book_id);
    progress = AutocompleteState::ChaptersOnly { book_id };
    // if there is a space after the book, they probably want to now type chapter
    if everything_after_book_name == " " {
        return progress;
    }

    // match segment characters
    let Some(segment_match) = re::segment_characters().find(everything_after_book_name) else {
        return progress;
    };

    // if they segment characters ends before the end of the input, it means the user started
    // typing something else
    // maybe i need a -1
    // if segment_match.end() < text_before_cursor.len() {
    //     return progress;
    // }

    // before parsing segments, must make sure they have at least 1 valid reference
    // segment parsing function assumes there is at least 1 valid segment, so a partial segment
    // like `1` or `1:` will return incorrect results
    //
    if let Some(cap) = re::incomplete_segment_start().captures(everything_after_book_name) {
        if let (Some(chapter_number), Some(colon)) = (cap.get(1), cap.get(2)) {
            // colon signifies i have typed chapter, so now it is time to suggest verse
            progress = AutocompleteState::VersesOnly {
                book_id,
                chapter: chapter_number
                    .as_str()
                    .parse()
                    .expect("Regex only matches number"),
            };
            // progress.1 = Some(
            // chapter
            //     .as_str()
            //     .parse()
            //     .expect("Regex only matches number"),
            // );
            return progress;
        }
        // this is guaranteed
        else if let Some(chapter_number) = cap.get(1) {
            // I am still suggesting chapters at this point because colon signifies I have chosen one,
            // no colon means i am still typing a chapter
            return progress;
        }
    }

    let segments = BookReferenceSegments::parse(segment_match.as_str());

    let operator = match segment_match
        .as_str()
        .trim()
        .chars()
        .last()
        .expect("I think if there wasn't an ending char it would not have gotten this far")
    {
        ':' => AutocompletionEndingOperator::Chapter,
        ',' | ';' => AutocompletionEndingOperator::Break,
        '-' | '–' => AutocompletionEndingOperator::Through,
        _ => AutocompletionEndingOperator::None,
    };
    let last_segment = segments
        .last()
        .expect("There is guaranteed a segment parse");
    // progress.1 = Some(last_segment.get_ending_chapter());

    // progress = AutocompleteState::ChaptersOrVerses {
    //     book_id,
    //     chapter: last_segment.get_ending_chapter(),
    //     verse: last_segment.get_ending_verse(),
    //     segments,
    //     operator,
    // };

    let last_chapter = re::chapter()
        .captures_iter(segment_match.as_str())
        .last()
        .expect("There is at least one chapter if I made it this far.")
        .get(1)
        .expect("Required group")
        .as_str()
        .parse()
        .expect("Digit capture group");

    let last_verse = re::verse()
        .captures_iter(segment_match.as_str())
        .last()
        .expect("There is at least one verse if I made it this far.")
        .get(1)
        .expect("Required group")
        .as_str()
        .parse()
        .expect("Digit capture group");

    progress = AutocompleteState::ChaptersOrVerses {
        book_id,
        chapter: last_chapter,
        verse: last_verse,
        segments,
        operator,
    };

    // if let Some(cap) = re::autocomplete_ending()
    //     .captures_iter(segment_match.as_str())
    //     .next()
    // {
    //     let ending = cap.as_str();
    //     if ending.ends_with(":") {}
    // }

    progress
}

// given current context (book, chapter, verse, and another number)
// suggest all possible results of what that number could be:
// - all chapters from book > chapter..=another_number
// - all chapters from book > chapter > verse..=another_number
// but the range isn't too another number, but what it starts with
// so it is actually multiple ranges: given Ephesians 1:1-2 -> verse 2, verses 20-29, and verses
// 200-299 (until i pass verses)

impl BibleLSP {
    pub fn new(json_path: &str) -> Self {
        BibleLSP {
            api: BibleAPI::new(json_path),
        }
    }

    pub fn find_book_references(&self, input: &str) -> Option<Vec<BookReference>> {
        /*
        Calculate the newline indexes so that I can convert the string index into line and column number for LSP (tower_lsp::Range)
        */
        let newline_indexes = input
            // .char_indices()
            .chars()
            .filter(|ch| *ch != '\r')
            .enumerate()
            .filter(|(_, ch)| *ch == '\n')
            .map(|(idx, _)| idx)
            .collect::<Vec<usize>>();
        /*
        Break the input into segments where each segment starts with a book of the Bible
        Also record the len of each book, so that I can efficiently split the segment into the book name and remaining text
        (which includes both the reference segments, such as `1:1-2:2` and everything after that up until the next book name)
        */
        let pat = self.api.book_abbreviation_regex();
        let mut iter = pat.find_iter(input).peekable();
        let mut prev: Option<usize> = None;
        let mut book_lens = vec![];
        // saving the start index of the capture so I can get a slice of the input later and do
        // only 1 .clone() at the end
        let mut start_indexes = vec![];
        // this is a vec of slices that correspond to the entire segment (start of one book or
        // abbreviation to right before the start of the next)
        let mut segment_matches = vec![];
        while let Some(cap) = iter.next() {
            start_indexes.push(cap.start());
            book_lens.push(cap.end() - cap.start());
            // store the previous start up until the start of this book
            // wait until the next iteration to store the segment of the current iteration
            if let Some(prev_start) = prev {
                segment_matches.push(&input[prev_start..cap.start()]);
            }
            prev = Some(cap.start());
            // if at the last element, segment goes to the end
            if iter.peek().is_none() {
                segment_matches.push(&input[cap.start()..]);
            }
        }
        /*
        - Iterate together over the previous recorded data
        - Parse reference segments (`1:1-2:2,3:4`)
        - Organize all data into a [`BookReference`]
        */
        let mut book_references = vec![];
        for ((seg, book_len), start_index) in segment_matches
            .into_iter()
            .zip(book_lens)
            .zip(start_indexes)
        {
            // find the reference segments (`1:1-2:2,3:4`) in the text segment if it is right after
            // the book name/abbreviation
            if let Some(segment_match) =
                re::post_book_valid_reference_segment_characters().find(&seg[book_len..])
            {
                let book_name = &seg[0..book_len];
                let book_id = self
                    .api
                    .get_book_id(&book_name)
                    .expect("The book_name slice already passed the RegEx of valid books.");
                let segment_chars = segment_match.as_str();
                let end_index = start_index + book_name.len() + segment_chars.len();
                let range = calculate_position(&newline_indexes, start_index, end_index);
                let book_reference = BookReference::new(book_id, range, segment_chars);
                book_references.push(book_reference);
            }
        }
        Some(book_references)
    }

    // /// Suggest autocomplete:
    // /// - book name: with book information
    // /// - chapter: with chapter information and verse preview
    // /// - verse: preview current verse (with surrounding context)
    // /// for chapter and verse, after the data specific to that suggestion, include the hover
    // /// contents of the entire reference so far as it is typed
    // pub fn suggest_auto_complete(&self, line: &str) -> Option<Vec<BibleCompletion>> {
    //     // Check current line for last book of the Bible mentioned
    //     // If there is no book at all
    //     let Some(book_match) = self.api.book_abbreviation_regex().find_iter(line).last() else {
    //         // Suggest all books
    //         return Some(suggest_all_books());
    //     };
    //
    //     // If there is a last book that goes to the end of the line, they have typed part of a book and it is validly matched by the pattern but they want to keep typing the full name
    //     if book_match.end() == line.len() {}
    //
    //     // If everything after the last book is not segments
    //     let contents_after_last_book = &line[book_match.end()..];
    //     let has_segment_characters_to_the_end = re::segment_characters()
    //         .find(contents_after_last_book)
    //         .is_some_and(|contents| contents.end() == line.len());
    //
    //     if !has_segment_characters_to_the_end {
    //         // Suggest all books
    //         return Some(suggest_all_books());
    //     }
    //
    //     let book_name = &line[book_match.start()..book_match.end()];
    //     // dbg!(&book_name);
    //     let book_id = self
    //         .api
    //         .get_book_id(book_name)
    //         .expect("The book_name slice already passed the RegEx of valid books.");
    //
    //     // if book_match.end() == line.len() - 1 && &line[line.len() - 1] {}
    //
    //     // get segments that follow right after
    //     let reference_segment_portion =
    //         re::post_book_valid_reference_segment_characters().find(contents_after_last_book)?;
    //     // dbg!(reference_segment_portion);
    //     let book_reference_segments = parse_reference_segments(reference_segment_portion.as_str());
    //     dbg!(&book_reference_segments);
    //     None
    // }

    // trim input to everything before cursor
    //
    // if (the end is segments):
    //     if before segments is a book:
    //         parse segments and suggest chapter/verse
    //     else:
    //         suggest book
    // else:
    //     suggest book
    //
    // OR
    //
    // if the last book is followed by only segments until the cursor:
    //  parse segments
    // else:
    //  suggest books
    // pub fn suggest_auto_completion(&self, line: &str) -> Option<Vec<BibleCompletion>> {
    //     // if there is no book, i can early return
    //     let Some(book_match) = self.api.book_abbreviation_regex().find_iter(line).last() else {
    //         // Suggest all books
    //         return Some(suggest_all_books());
    //     };
    // }

    // /// - Find all Scripture references, their start and stop locations, and their references parsed
    // /// - This can be used for diagnostics or hover or go to definition
    // /// - I can cache this
    // /// - On a hover or go to definition request, I can just use the information in the cached result because it I can search by line number
    // fn find_everything(&self, text: &str) -> () {
    //     todo!()
    // }
    //
    pub fn suggest_auto_completion(&self, line: &str) -> Vec<BibleCompletion> {
        let state = parse_current_state(&self.api, line);
        // let mut file = OpenOptions::new()
        //     .write(true)
        //     .append(true)
        //     .open("~/bible_lsp.log")
        //     .unwrap();
        // write!(file, format!("{:#?}", &state));
        // append_log(format!("{}\n{:#?}\n\n", line, &state));
        // format!("{:#?}", &state);
        let result = state.give_suggestions(&self.api);
        append_log(format!("result={:#?}\n\n", &result));
        result
    }
}

pub fn append_log(content: impl AsRef<str>) {
    _ = append_to_file("/home/dgmastertemple/bible_lsp.log", content.as_ref());
}

pub fn append_to_file(filename: &str, content: &str) -> Result<(), io::Error> {
    // Open the file in append mode. Create it if it doesn't exist.
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)?;

    // Write the content to the file.
    writeln!(file, "{}", content)?;

    Ok(())
}
