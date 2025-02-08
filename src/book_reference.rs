use tower_lsp::lsp_types::Range;

use crate::{
    api_wrappers::APIBookReference, bible_api::BibleAPI,
    book_reference_segment::BookReferenceSegments,
};

#[derive(Clone, Debug)]
pub struct BookReference {
    pub range: Range,
    pub book_id: usize,
    pub segments: BookReferenceSegments,
}

impl<'a> BookReference {
    pub fn apid(self, api: &'a BibleAPI) -> APIBookReference<'a> {
        APIBookReference {
            api,
            book_reference: self,
        }
    }
}

impl BookReference {
    /// This should only be called after finding a match in a range
    pub fn new(book_id: usize, range: Range, segment_input: &str) -> Self {
        // split into book name and segments
        // get book id
        let segments = BookReferenceSegments::parse(segment_input);
        Self {
            range,
            book_id,
            segments,
        }
    }

    /// Formats into something like `Ephesians 1:1-4, 5-7, 2:2-3:4, 6`
    pub fn full_ref_label(&self, api: &BibleAPI) -> String {
        let book_name = api
            .get_book_name(self.book_id)
            .expect("A BookReference struct should not be created if the book_id is invalid.");
        format!("{} {}", book_name, self.segments.label())
    }

    /**
    Returns text like the following:

    ```text
    [1:1] Paul, an apostle of Christ Jesus by the will of God, To the saints who are in Ephesus, and are faithful in Christ Jesus:
    [1:2] Grace to you and peace from God our Father and the Lord Jesus Christ.
    [1:3] Blessed be the God and Father of our Lord Jesus Christ, who has blessed us in Christ with every spiritual blessing in the heavenly places,
    [1:4] even as he chose us in him before the foundation of the world, that we should be holy and blameless before him. In love
    ```
    */
    pub fn format_content(&self, api: &BibleAPI) -> String {
        self.segments
            .iter()
            .map(|seg| {
                let mut contents = vec![];
                for chapter in seg.get_starting_chapter()..=seg.get_ending_chapter() {
                    for verse in seg.get_starting_verse()..=seg.get_ending_verse() {
                        if let Some(content) = api.get_bible_contents(self.book_id, chapter, verse)
                        {
                            contents.push(format!("[{}:{}] {}", chapter, verse, content));
                        }
                    }
                }
                contents.join("\n")
            })
            .collect::<Vec<String>>()
            .join("\n\n")
    }

    pub fn format(&self, api: &BibleAPI) -> String {
        let reference = self.full_ref_label(api);
        let content = self.format_content(api);
        format!("### {reference}\n\n{content}")
    }

    pub fn format_insert(&self, api: &BibleAPI) -> String {
        let reference = self.full_ref_label(api);
        let content = self.format_content(api);
        format!("\n{content}")
    }

    pub fn format_replace(&self, api: &BibleAPI) -> String {
        let reference = self.full_ref_label(api);
        let content = self
            .format_content(api)
            .replace("\n\n", "\n")
            .replace("\n", " ");
        format!("> {content} - {reference}")
    }

    pub fn format_diagnostic(&self, api: &BibleAPI) -> Option<String> {
        let first_segment = self.segments.first()?;
        // .expect("This would not have matched as a book reference if there were not segments");
        let content = api.get_bible_contents(
            self.book_id,
            first_segment.get_starting_chapter(),
            first_segment.get_starting_verse(),
        )?;
        Some(content)
    }
}
