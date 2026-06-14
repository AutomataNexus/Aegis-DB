//! Inverted index with Okapi BM25 ranking.
//!
//! `postings[term][doc] = term-frequency`, plus per-document lengths and the
//! running total so the average document length (needed by BM25's length
//! normalization) is O(1). Documents are removed exactly by re-applying their
//! token list, so search results are exact (no tombstones).

use std::collections::HashMap;

/// BM25 free parameters. `k1` controls term-frequency saturation, `b` controls
/// document-length normalization. The classic defaults are k1=1.2, b=0.75.
#[derive(Debug, Clone, Copy)]
pub struct Bm25Params {
    pub k1: f32,
    pub b: f32,
}
impl Default for Bm25Params {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

#[derive(Default)]
pub struct InvertedIndex {
    postings: HashMap<String, HashMap<u32, u32>>,
    doc_len: HashMap<u32, u32>,
    total_len: u64,
    params: Bm25Params,
}

impl InvertedIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn doc_count(&self) -> usize {
        self.doc_len.len()
    }

    fn avgdl(&self) -> f32 {
        let n = self.doc_len.len();
        if n == 0 {
            0.0
        } else {
            self.total_len as f32 / n as f32
        }
    }

    /// Add a document's tokens to the index.
    pub fn add(&mut self, doc: u32, tokens: &[String]) {
        for t in tokens {
            *self
                .postings
                .entry(t.clone())
                .or_default()
                .entry(doc)
                .or_insert(0) += 1;
        }
        self.total_len += tokens.len() as u64;
        self.doc_len.insert(doc, tokens.len() as u32);
    }

    /// Remove a document by re-applying its token list. No-op if absent.
    pub fn remove(&mut self, doc: u32, tokens: &[String]) {
        if let Some(len) = self.doc_len.remove(&doc) {
            self.total_len -= len as u64;
            // Use the unique terms to avoid redundant lookups.
            let mut seen = std::collections::HashSet::new();
            for t in tokens {
                if seen.insert(t.as_str()) {
                    if let Some(plist) = self.postings.get_mut(t) {
                        plist.remove(&doc);
                        if plist.is_empty() {
                            self.postings.remove(t);
                        }
                    }
                }
            }
        }
    }

    /// BM25-score every document that matches at least one query term.
    /// Returns `doc_id -> score`.
    pub fn score(&self, query_tokens: &[String]) -> HashMap<u32, f32> {
        let n = self.doc_len.len() as f32;
        let avgdl = self.avgdl();
        let mut scores: HashMap<u32, f32> = HashMap::new();
        if n == 0.0 || avgdl == 0.0 {
            return scores;
        }
        let mut seen = std::collections::HashSet::new();
        for term in query_tokens {
            if !seen.insert(term.as_str()) {
                continue;
            }
            let Some(plist) = self.postings.get(term) else {
                continue;
            };
            let df = plist.len() as f32;
            let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
            let k1 = self.params.k1;
            let b = self.params.b;
            for (&doc, &tf) in plist {
                let dl = self.doc_len.get(&doc).copied().unwrap_or(0) as f32;
                let tf = tf as f32;
                let denom = tf + k1 * (1.0 - b + b * dl / avgdl);
                *scores.entry(doc).or_insert(0.0) += idf * (tf * (k1 + 1.0)) / denom;
            }
        }
        scores
    }
}
