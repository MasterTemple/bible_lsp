use crate::bible_lsp::BibleLSP;

fn main() {
    let json_path = "/home/dgmastertemple/Development/rust/bible_api/esv.json";
    let lsp = BibleLSP::new(json_path);
    let contents = std::fs::read_to_string("/home/dgmastertemple/christian_commons.txt").unwrap();
    let references = lsp.find_book_references(&contents).unwrap();
    for r in references {
        println!("{r}");
    }
}
