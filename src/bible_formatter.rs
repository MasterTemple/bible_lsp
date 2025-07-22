use crate::book_reference_segment::BookReferenceSegment;

struct PassageFormatter {
    // can use book, chapter, verse, content
    verse: String,

    // the text that joins all verses together
    join_verses: String,

    // can use verses
    segment: String,

    // the text that joins all segments together
    join_segment: String,

    // can use book, label/reference, segments
    text: String,

    // insert, replace, all, ...
    code_actions: Vec<String>,
}

fn literal_word() -> PassageFormatter {
    PassageFormatter {
        verse: "{content}".to_string(),
        join_verses: " ".to_string(),
        segment: "{verses}".to_string(),
        join_segment: " ".to_string(),
        text: "> {segments}\n— {reference}".to_string(),
        code_actions: vec![],
    }
}

struct BibleFormatter {
    book_format: String,
    chapter_format: String,
    verse_format: String,
}

struct ItemFormatting {
    left_content: String,
    right_content: String,
    hide: bool,
}

/**
i think the best method is to have the 3 sections that i can format, but each can use its parents data
so in the verse formatter, i can use the chapter or book
in the chapter formatter, i can use the book

example book formatter:
# {book}
{chapters}

example chapter formatter:
{book} {chapter}
{verses}

example verse formatter:
[{chapter}:{verse}] {content}

automatically join the children with nothing in between, it is up for the child to specify

the best way to use the template is to do a find and replace, but track the following, where is the `{chapter}` variable located, how long is it, subtract that from the contents that are going there, then do the same for the other variables

okay but how do i represent something like adding extra space between non-contiguous verses?
*/

/*
maybe these should be my formatting groups:
    ChapterVerse: book, chapter, verse
        ### {book} {chapter}
        ---
        [{verse}] {content}
    ChapterRange: book, chapter, verse
        ### {book} {chapter}
        ---
        {[{verse}] {content}\n}\n // add the extra newline so that disconnected segments have space in between
    BookRange

but i need 2 different methods
one for formatting by itself
ex:
    ChapterRange: book, chapter, verse
        ### {book} {chapter}
        ---
        {[{verse}] {content}\n}\n
and one for formatting in a vector
    ChapterRange: book, chapter, verse
        {[{verse}] {content}\n}\n

so have an optional/separate heading format
as well as a heading format for book grouped segments

REMEMBER, THE ABOVE ARE ALL BOOK SEGMENTS
*/

impl BibleFormatter {
    /**
    `Ephesians 1:1-4,5-7,2:3-4` yields
    ```text
    ### Ephesians

    [1:1] Paul, an apostle of Christ Jesus by the will of God, To the saints who are in Ephesus, and are faithful in Christ Jesus:
    [1:2] Grace to you and peace from God our Father and the Lord Jesus Christ.
    [1:3] Blessed be the God and Father of our Lord Jesus Christ, who has blessed us in Christ with every spiritual blessing in the heavenly places,
    [1:4] even as he chose us in him before the foundation of the world, that we should be holy and blameless before him. In love

    [1:5] he predestined us for adoption to himself as sons through Jesus Christ, according to the purpose of his will,
    [1:6] to the praise of his glorious grace, with which he has blessed us in the Beloved.
    [1:7] In him we have redemption through his blood, the forgiveness of our trespasses, according to the riches of his grace,

    [2:3] among whom we all once lived in the passions of our flesh, carrying out the desires of the body and the mind, and were by nature children of wrath, like the rest of mankind.
    [2:4] But God, being rich in mercy, because of the great love with which he loved us,
    ```
    */
    fn format_segments(&self, segments: Vec<BookReferenceSegment>) -> String {
        String::new()
    }
}
