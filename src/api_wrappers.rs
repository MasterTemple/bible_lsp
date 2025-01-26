use crate::{bible_api::BibleAPI, book_reference::BookReference};

pub struct APIBookReference<'a> {
    pub api: &'a BibleAPI,
    pub book_reference: BookReference,
}

impl<'a> APIBookReference<'a> {
    /// Ex: `Ephesians 1:1-2; 2:3-3:4,6`
    pub fn full_ref_label(&self) -> String {
        self.book_reference.full_ref_label(&self.api)
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
    pub fn format_content(&self) -> String {
        self.book_reference.format_content(&self.api)
    }

    /// provides markdown for LSP hover preview
    pub fn lsp_hover(&self) -> String {
        let reference = self.book_reference.full_ref_label(&self.api);
        let content = self.book_reference.format_content(&self.api);
        format!("### {reference}\n\n{content}")
    }

    /// provides text for LSP diagnostic
    pub fn lsp_diagnostic(&self) -> Option<String> {
        self.book_reference.format_diagnostic(&self.api)
    }
}
