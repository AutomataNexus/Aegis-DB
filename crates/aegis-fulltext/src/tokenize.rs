//! Text tokenization: lowercase, split on non-alphanumeric, drop stopwords and
//! very short tokens. Deterministic so indexing and querying stay consistent.

/// A small English stopword list. Common, low-signal words that BM25 would
/// otherwise waste IDF mass on.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "been", "but", "by", "for", "from", "had", "has",
    "have", "he", "i", "if", "in", "into", "is", "it", "its", "no", "not", "of", "on", "or", "our",
    "she", "such", "that", "the", "their", "then", "there", "these", "they", "this", "to", "us",
    "was", "we", "were", "will", "with", "you", "your",
];

fn is_stopword(token: &str) -> bool {
    STOPWORDS.binary_search(&token).is_ok()
}

/// Tokenize text into normalized terms: lowercased, ASCII-alphanumeric runs,
/// stopwords and 1-char tokens removed.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            cur.extend(ch.to_lowercase());
        } else if !cur.is_empty() {
            push_token(&mut out, std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, cur);
    }
    out
}

fn push_token(out: &mut Vec<String>, token: String) {
    if token.len() > 1 && !is_stopword(&token) {
        out.push(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopwords_are_sorted_for_binary_search() {
        let mut sorted = STOPWORDS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, STOPWORDS, "STOPWORDS must stay sorted");
    }

    #[test]
    fn tokenizes_and_filters() {
        let toks = tokenize("The Quick, brown FOX! jumps over the lazy dog.");
        // 'the' (stopword) and 'over' kept? 'over' not in list -> kept.
        assert_eq!(
            toks,
            vec!["quick", "brown", "fox", "jumps", "over", "lazy", "dog"]
        );
    }

    #[test]
    fn handles_numbers_and_unicode() {
        let toks = tokenize("Aegis-DB v0.4 — café 2024!");
        assert_eq!(toks, vec!["aegis", "db", "v0", "café", "2024"]);
    }
}
