/// This is meant to be used only to create the initial data structure for reading in the JSON file
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JSONTranslation {
    pub name: String,
    pub language: String,
    pub abbreviation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JSONBook {
    /// book id where Genesis = 1
    pub id: usize,
    /// the name of the book as it is displayed
    pub book: String,
    /// all abbreviations (any case), not necessarily including the book name
    pub abbreviations: Vec<String>,
    pub content: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JSONBible {
    pub translation: JSONTranslation,
    pub bible: Vec<JSONBook>,
}
