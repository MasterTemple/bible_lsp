use std::ops::RangeInclusive;
use std::{collections::BTreeMap, sync::Mutex};

use once_cell::sync::Lazy;
use regex::Regex;

use crate::bible_json::{JSONBible, JSONTranslation};

/// map of abbreviations and actual name (all lowercase) to book id
pub type AbbreviationsToBookId = BTreeMap<String, usize>;

/// map of book id to book name
pub type BookIdToName = BTreeMap<usize, String>;

/// - 2D array to check if verse reference is valid
///   - each outer array corresponds to a book of the bible
///   - each inner array corresponds to each chapter of the book
///   - each element of the inner array is the number of verses in that chapter
pub type ReferenceArray = Vec<Vec<usize>>;

/// - 3D array to store content
///   - each outer array corresponds to a book of the bible
///   - each middle array corresponds to each chapter of the book
///   - each inner array corresponds to each verse of the chapter
pub type BibleContents = Vec<Vec<Vec<String>>>;

/// - This is a cache used to store a dynamically generated RegEx for matching books of the Bible based on the abbreviations by translation
/// - This **DOES NOT** match `1:1-4,5-7,2:2-3:4,6` in `eph 1:1-4,5-7,2:2-3:4,6`
/// - This would match `eph` for `Ephesians`
static BOOK_ABBREVIATION_REGEX_CACHE: Lazy<Mutex<Option<(String, Regex)>>> =
    Lazy::new(|| Mutex::new(None));

/// - This is a cache used to store a dynamically generated RegEx for matching books of the Bible AND reference content based on the abbreviations by translation
/// - This **DOES** match `eph 1:1-4,5-7,2:2-3:4,6` in `eph 1:1-4,5-7,2:2-3:4,6`
/// - This would match `eph` for `Ephesians`
static BOOK_REFERENCE_REGEX_CACHE: Lazy<Mutex<Option<(String, Regex)>>> =
    Lazy::new(|| Mutex::new(None));

#[derive(Clone, Debug)]
pub struct BibleAPI {
    pub translation: JSONTranslation,
    /// map of abbreviations and actual name (all lowercase) to book id
    pub abbreviations_to_book_id: AbbreviationsToBookId,
    /// map of book id to book name
    pub book_id_to_name: BookIdToName,
    /// - 2D array to check if verse reference is valid
    ///   - each outer array corresponds to a book of the bible
    ///   - each inner array corresponds to each chapter of the book
    ///   - each element of the inner array is the number of verses in that chapter
    pub reference_array: ReferenceArray,
    /// - 3D array to store content
    ///   - each outer array corresponds to a book of the bible
    ///   - each middle array corresponds to each chapter of the book
    ///   - each inner array corresponds to each verse of the chapter
    pub bible_contents: BibleContents,
}

impl BibleAPI {
    /// - This reads the JSON file and reformats it into optimized data structures to be used by
    /// the methods of this "API"
    pub fn new(json_path: &str) -> Self {
        let bible_json = std::fs::read_to_string(json_path)
            .expect(format!("Couldn't find the Bible JSON file at {json_path:?}.").as_str());
        let bible: JSONBible = serde_json::from_str(bible_json.as_str())
            .expect("Bible JSON file improperly formatted.");

        let mut abbreviations_to_book_id = AbbreviationsToBookId::new();
        let mut book_id_to_name = BookIdToName::new();
        let mut reference_array = ReferenceArray::new();
        let mut bible_contents = BibleContents::new();

        for book in bible.bible.iter() {
            let mut book_contents: Vec<Vec<String>> = vec![];
            book_id_to_name.insert(book.id, book.book.clone());
            abbreviations_to_book_id.insert(book.book.clone().to_lowercase(), book.id);
            for abbreviation in book.abbreviations.iter().cloned() {
                abbreviations_to_book_id.insert(abbreviation.to_lowercase(), book.id);
            }
            let mut chapter_array = Vec::new();
            for (_, verses) in book.content.iter().enumerate() {
                chapter_array.push(verses.len());
                book_contents.push(verses.clone());
            }
            reference_array.push(chapter_array);
            bible_contents.push(book_contents);
        }

        Self {
            translation: bible.translation,
            abbreviations_to_book_id,
            book_id_to_name,
            reference_array,
            bible_contents,
        }
    }

    pub fn is_valid_book_chapter(&self, book: usize, chapter: usize) -> bool {
        self.reference_array
            .get(book - 1)
            .is_some_and(|chapters| chapter <= chapters.len())
    }

    pub fn is_valid_reference(&self, book: usize, chapter: usize, verse: usize) -> bool {
        self.reference_array
            .get(book - 1)
            .and_then(|chapters| chapters.get(chapter - 1))
            .is_some_and(|verse_count| verse <= *verse_count)
    }

    /// gets the number of chapters in a book
    pub fn get_book_chapter_count(&self, book: usize) -> Option<usize> {
        Some(self.reference_array.get(book - 1)?.len())
    }

    /// gets the number of verses in a chapter
    pub fn get_chapter_verse_count(&self, book: usize, chapter: usize) -> Option<usize> {
        Some(
            self.reference_array
                .get(book - 1)?
                .get(chapter - 1)?
                .clone(),
        )
    }

    pub fn get_all_chapters(&self, book: usize) -> Option<RangeInclusive<usize>> {
        self.get_remaining_chapters(book, 0)
    }

    // get the remaining chapters in the book (does not include itself)
    pub fn get_remaining_chapters(
        &self,
        book: usize,
        chapter: usize,
    ) -> Option<RangeInclusive<usize>> {
        self.get_book_chapter_count(book)
            .map(|chapter_count| (chapter + 1)..=chapter_count)
    }

    pub fn get_all_verses(&self, book: usize, chapter: usize) -> Option<RangeInclusive<usize>> {
        self.get_remaining_verses(book, chapter, 0)
    }

    // get the remaining verses in the chapter (does not include itself)
    pub fn get_remaining_verses(
        &self,
        book: usize,
        chapter: usize,
        verse: usize,
    ) -> Option<RangeInclusive<usize>> {
        self.get_chapter_verse_count(book, chapter)
            .map(|verse_count| (verse + 1)..=verse_count)
    }

    pub fn get_bible_contents(&self, book: usize, chapter: usize, verse: usize) -> Option<String> {
        Some(
            self.bible_contents
                .get(book - 1)?
                .get(chapter - 1)?
                .get(verse - 1)?
                .clone(),
        )
    }

    pub fn get_bible_range_contents(
        &self,
        book_id: usize,
        start_chapter: usize,
        start_verse: usize,
        end_chapter: usize,
        end_verse: usize,
    ) -> Vec<String> {
        let mut contents = vec![];
        for chapter in start_chapter..=end_chapter {
            for verse in start_verse..=end_verse {
                if let Some(content) = self.get_bible_contents(book_id, chapter, verse) {
                    contents.push(content);
                }
            }
        }
        contents
    }

    pub fn get_book_id(&self, book: &str) -> Option<usize> {
        self.abbreviations_to_book_id
            .get(book.to_lowercase().trim_end_matches("."))
            // .get(&book.to_lowercase())
            .cloned()
    }

    pub fn get_book_name(&self, book: usize) -> Option<String> {
        self.book_id_to_name.get(&book).cloned()
    }

    /// - I added the period so that people can use it in abbreviations
    /// - The period is removed when calling [`BibleAPI::get_book_id`]
    pub fn book_abbreviation_regex(&self) -> Regex {
        let mut cache = BOOK_ABBREVIATION_REGEX_CACHE.lock().unwrap();
        if cache
            .as_ref()
            .is_some_and(|(version, _)| *version == self.translation.abbreviation)
        {
            cache.as_ref().unwrap().clone().1
        } else {
            let books_pattern: String = self
                .abbreviations_to_book_id
                .keys()
                .into_iter()
                .map(|key| key.to_string())
                .collect::<Vec<String>>()
                .join("|");
            // I added the period so that people can use it in abbreviations
            let pattern = Regex::new(format!(r"\b((?i){books_pattern})\b\.?").as_str())
                .expect("Failed to compile book_abbreviation_regex.");
            *cache = Some((self.translation.abbreviation.clone(), pattern.clone()));
            pattern
        }
    }
}
