//! SQL injection detection engine.
//!
//! Uses a library of compiled regex patterns to score incoming SQL strings
//! for injection risk.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// A single detection rule.
struct InjectionPattern {
    name: &'static str,
    regex: Regex,
    score: u32,
    #[allow(dead_code)]
    description: &'static str,
}

/// Result of analyzing a query string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlInjectionResult {
    pub is_suspicious: bool,
    pub score: u32,
    pub matched_patterns: Vec<String>,
}

/// The SQL injection detector holds pre-compiled patterns.
pub struct SqlInjectionDetector {
    patterns: Vec<InjectionPattern>,
}

impl Default for SqlInjectionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlInjectionDetector {
    /// Build the detector with all built-in patterns.
    pub fn new() -> Self {
        let patterns = vec![
            // 1 - UNION SELECT injection
            InjectionPattern {
                name: "union_select",
                regex: Regex::new(r#"(?i)\bUNION\s+(ALL\s+)?SELECT\b"#).expect("regex"),
                score: 90,
                description: "UNION SELECT injection",
            },
            // 2 - OR 1=1 always-true condition
            InjectionPattern {
                name: "or_always_true",
                regex: Regex::new(r#"(?i)\bOR\s+['"]?\d+['"]?\s*=\s*['"]?\d+['"]?"#)
                    .expect("regex"),
                score: 85,
                description: "OR always-true condition",
            },
            // 3 - Stacked queries DROP
            InjectionPattern {
                name: "stacked_drop",
                regex: Regex::new(r#"(?i);\s*DROP\s+"#).expect("regex"),
                score: 95,
                description: "Stacked queries with DROP",
            },
            // 4 - Stacked queries DELETE
            InjectionPattern {
                name: "stacked_delete",
                regex: Regex::new(r#"(?i);\s*DELETE\s+"#).expect("regex"),
                score: 95,
                description: "Stacked queries with DELETE",
            },
            // 5 - Stacked queries INSERT
            InjectionPattern {
                name: "stacked_insert",
                regex: Regex::new(r#"(?i);\s*INSERT\s+"#).expect("regex"),
                score: 90,
                description: "Stacked queries with INSERT",
            },
            // 6 - Stacked queries UPDATE
            InjectionPattern {
                name: "stacked_update",
                regex: Regex::new(r#"(?i);\s*UPDATE\s+"#).expect("regex"),
                score: 90,
                description: "Stacked queries with UPDATE",
            },
            // 7 - Single-line comment injection
            InjectionPattern {
                name: "comment_dash",
                regex: Regex::new(r#"--\s*$"#).expect("regex"),
                score: 60,
                description: "Trailing comment injection (--)",
            },
            // 8 - Block comment injection
            InjectionPattern {
                name: "comment_block",
                regex: Regex::new(r#"/\*.*?\*/"#).expect("regex"),
                score: 60,
                description: "Block comment injection",
            },
            // 9 - Hash comment
            InjectionPattern {
                name: "comment_hash",
                regex: Regex::new(r#"#\s*$"#).expect("regex"),
                score: 60,
                description: "Hash comment injection",
            },
            // 10 - Nested comments
            InjectionPattern {
                name: "nested_comment",
                regex: Regex::new(r#"/\*.*?/\*"#).expect("regex"),
                score: 65,
                description: "Nested comment injection",
            },
            // 11 - SLEEP time-based
            InjectionPattern {
                name: "sleep_fn",
                regex: Regex::new(r#"(?i)\bSLEEP\s*\("#).expect("regex"),
                score: 80,
                description: "SLEEP-based timing attack",
            },
            // 12 - BENCHMARK time-based
            InjectionPattern {
                name: "benchmark_fn",
                regex: Regex::new(r#"(?i)\bBENCHMARK\s*\("#).expect("regex"),
                score: 80,
                description: "BENCHMARK-based timing attack",
            },
            // 13 - WAITFOR DELAY
            InjectionPattern {
                name: "waitfor_delay",
                regex: Regex::new(r#"(?i)\bWAITFOR\s+DELAY\b"#).expect("regex"),
                score: 80,
                description: "WAITFOR DELAY timing attack",
            },
            // 14 - LOAD_FILE
            InjectionPattern {
                name: "load_file",
                regex: Regex::new(r#"(?i)\bLOAD_FILE\s*\("#).expect("regex"),
                score: 90,
                description: "LOAD_FILE file read attempt",
            },
            // 15 - INTO OUTFILE
            InjectionPattern {
                name: "into_outfile",
                regex: Regex::new(r#"(?i)\bINTO\s+(OUT|DUMP)FILE\b"#).expect("regex"),
                score: 90,
                description: "INTO OUTFILE/DUMPFILE write attempt",
            },
            // 16 - CHAR obfuscation
            InjectionPattern {
                name: "char_obfuscation",
                regex: Regex::new(r#"(?i)\bCHAR\s*\(\s*\d+(\s*,\s*\d+)+\s*\)"#).expect("regex"),
                score: 70,
                description: "CHAR() string obfuscation",
            },
            // 17 - CONCAT obfuscation
            InjectionPattern {
                name: "concat_obfuscation",
                regex: Regex::new(r#"(?i)\bCONCAT\s*\("#).expect("regex"),
                score: 70,
                description: "CONCAT() string obfuscation",
            },
            // 18 - Hex encoding 0x
            InjectionPattern {
                name: "hex_encoding",
                regex: Regex::new(r#"0x[0-9a-fA-F]{8,}"#).expect("regex"),
                score: 75,
                description: "Hex-encoded string attack",
            },
            // 19 - Boolean-based AND 1=1
            InjectionPattern {
                name: "boolean_and",
                regex: Regex::new(r#"(?i)\bAND\s+['"]?\d+['"]?\s*=\s*['"]?\d+['"]?"#)
                    .expect("regex"),
                score: 70,
                description: "Boolean-based blind injection (AND x=x)",
            },
            // 20 - String termination with quote + semicolon
            InjectionPattern {
                name: "string_termination",
                regex: Regex::new(r#"['"]\s*;\s*(DROP|DELETE|INSERT|UPDATE|ALTER|CREATE|EXEC)\b"#)
                    .expect("regex"),
                score: 90,
                description: "String termination followed by SQL command",
            },
            // 21 - EXEC/EXECUTE procedure
            InjectionPattern {
                name: "exec_proc",
                regex: Regex::new(r#"(?i)\bEXEC(UTE)?\s+(xp_|sp_)"#).expect("regex"),
                score: 90,
                description: "EXEC stored procedure call",
            },
            // 22 - xp_cmdshell
            InjectionPattern {
                name: "xp_cmdshell",
                regex: Regex::new(r#"(?i)\bxp_cmdshell\b"#).expect("regex"),
                score: 95,
                description: "xp_cmdshell command execution",
            },
            // 23 - INFORMATION_SCHEMA
            InjectionPattern {
                name: "information_schema",
                regex: Regex::new(r#"(?i)\bINFORMATION_SCHEMA\b"#).expect("regex"),
                score: 75,
                description: "INFORMATION_SCHEMA metadata access",
            },
            // 24 - pg_sleep (PostgreSQL timing)
            InjectionPattern {
                name: "pg_sleep",
                regex: Regex::new(r#"(?i)\bpg_sleep\s*\("#).expect("regex"),
                score: 80,
                description: "pg_sleep timing attack",
            },
            // 25 - HAVING clause injection
            InjectionPattern {
                name: "having_injection",
                regex: Regex::new(r#"(?i)\bHAVING\s+\d+\s*=\s*\d+"#).expect("regex"),
                score: 70,
                description: "HAVING clause injection",
            },
            // 26 - ORDER BY enumeration
            InjectionPattern {
                name: "order_by_enum",
                regex: Regex::new(r#"(?i)\bORDER\s+BY\s+\d{2,}"#).expect("regex"),
                score: 60,
                description: "ORDER BY column enumeration",
            },
            // 27 - GROUP BY with HAVING
            InjectionPattern {
                name: "group_by_having",
                regex: Regex::new(r#"(?i)\bGROUP\s+BY\s+.+\bHAVING\b"#).expect("regex"),
                score: 50,
                description: "GROUP BY with HAVING injection",
            },
            // 28 - EXTRACTVALUE/UPDATEXML
            InjectionPattern {
                name: "xml_extract",
                regex: Regex::new(r#"(?i)\b(EXTRACTVALUE|UPDATEXML)\s*\("#).expect("regex"),
                score: 85,
                description: "XML function injection",
            },
            // 29 - CONVERT/CAST with suspicious usage
            InjectionPattern {
                name: "convert_cast",
                regex: Regex::new(r#"(?i)\b(CONVERT|CAST)\s*\(.+\bAS\b.+\)"#).expect("regex"),
                score: 40,
                description: "CONVERT/CAST type coercion",
            },
            // 30 - Double-encoded percent
            InjectionPattern {
                name: "double_encode",
                regex: Regex::new(r#"%25(27|22|3[bB])"#).expect("regex"),
                score: 75,
                description: "Double URL encoding attack",
            },
            // 31 - Unicode encoding
            InjectionPattern {
                name: "unicode_encode",
                regex: Regex::new(r#"\\u0027|\\u0022|%u0027|%u0022"#).expect("regex"),
                score: 75,
                description: "Unicode encoding attack",
            },
            // 32 - INTO variable
            InjectionPattern {
                name: "into_var",
                regex: Regex::new(r#"(?i)\bINTO\s+@"#).expect("regex"),
                score: 70,
                description: "INTO variable assignment",
            },
            // 33 - ALTER TABLE
            InjectionPattern {
                name: "alter_table",
                regex: Regex::new(r#"(?i);\s*ALTER\s+TABLE\b"#).expect("regex"),
                score: 90,
                description: "Stacked ALTER TABLE",
            },
            // 34 - CREATE TABLE
            InjectionPattern {
                name: "create_stacked",
                regex: Regex::new(r#"(?i);\s*CREATE\s+(TABLE|DATABASE|USER)\b"#).expect("regex"),
                score: 90,
                description: "Stacked CREATE statement",
            },
            // 35 - SHUTDOWN
            InjectionPattern {
                name: "shutdown_cmd",
                regex: Regex::new(r#"(?i)\bSHUTDOWN\b"#).expect("regex"),
                score: 95,
                description: "SHUTDOWN command",
            },
            // 36 - Tautology with string
            InjectionPattern {
                name: "tautology_string",
                regex: Regex::new(r#"(?i)['"]?\s*OR\s+['"][^'"]+['"]\s*=\s*['"][^'"]+['"]"#)
                    .expect("regex"),
                score: 85,
                description: "String tautology (OR 'a'='a')",
            },
            // 37 - IF-based blind
            InjectionPattern {
                name: "if_blind",
                regex: Regex::new(r#"(?i)\bIF\s*\(.+,.+,.+\)"#).expect("regex"),
                score: 70,
                description: "IF-based blind injection",
            },
            // 38 - LIKE-based wildcard abuse
            InjectionPattern {
                name: "like_wildcard",
                regex: Regex::new(r#"(?i)\bLIKE\s+['"]%['"]"#).expect("regex"),
                score: 30,
                description: "LIKE wildcard abuse",
            },
        ];

        Self { patterns }
    }

    /// Analyze an input string for SQL injection patterns.
    /// Returns the combined score (capped at 100) and matched pattern names.
    pub fn analyze(&self, input: &str) -> SqlInjectionResult {
        let mut total_score: u32 = 0;
        let mut matched = Vec::new();

        for pat in &self.patterns {
            if pat.regex.is_match(input) {
                total_score = total_score.saturating_add(pat.score);
                matched.push(pat.name.to_string());
            }
        }

        let capped = total_score.min(100);
        SqlInjectionResult {
            is_suspicious: capped > 0,
            score: capped,
            matched_patterns: matched,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> SqlInjectionDetector {
        SqlInjectionDetector::new()
    }

    #[test]
    fn test_clean_query() {
        let result = detector().analyze("SELECT id, name FROM users WHERE id = 42");
        assert!(!result.is_suspicious || result.score < 40);
    }

    #[test]
    fn test_union_select() {
        let result = detector().analyze("SELECT * FROM users UNION SELECT password FROM admins");
        assert!(result.is_suspicious);
        assert!(result
            .matched_patterns
            .contains(&"union_select".to_string()));
        assert!(result.score >= 90);
    }

    #[test]
    fn test_or_1_eq_1() {
        let result = detector().analyze("SELECT * FROM users WHERE name='admin' OR 1=1");
        assert!(result.is_suspicious);
        assert!(result
            .matched_patterns
            .contains(&"or_always_true".to_string()));
    }

    #[test]
    fn test_stacked_drop() {
        let result = detector().analyze("SELECT 1; DROP TABLE users");
        assert!(result.is_suspicious);
        assert!(result
            .matched_patterns
            .contains(&"stacked_drop".to_string()));
        assert!(result.score >= 90);
    }

    #[test]
    fn test_sleep_injection() {
        let result = detector().analyze("SELECT * FROM users WHERE id=1 AND SLEEP(5)");
        assert!(result.is_suspicious);
        assert!(result.matched_patterns.contains(&"sleep_fn".to_string()));
    }

    #[test]
    fn test_load_file() {
        let result = detector().analyze("SELECT LOAD_FILE('/etc/passwd')");
        assert!(result.is_suspicious);
        assert!(result.score >= 90);
    }

    #[test]
    fn test_string_termination() {
        let result = detector().analyze("'; DROP TABLE users");
        assert!(result.is_suspicious);
        assert!(result
            .matched_patterns
            .contains(&"string_termination".to_string()));
    }

    #[test]
    fn test_score_capped_at_100() {
        let result = detector().analyze(
            "' OR 1=1; DROP TABLE x; DELETE FROM y; UNION SELECT * FROM z; SLEEP(5); LOAD_FILE('/etc/passwd')"
        );
        assert!(result.is_suspicious);
        assert!(result.score <= 100);
    }

    #[test]
    fn test_xp_cmdshell() {
        let result = detector().analyze("EXEC xp_cmdshell 'dir'");
        assert!(result.is_suspicious);
        assert!(result.matched_patterns.contains(&"xp_cmdshell".to_string()));
    }

    #[test]
    fn test_hex_encoding() {
        let result = detector().analyze("SELECT * FROM users WHERE name=0x61646D696E");
        assert!(result.is_suspicious);
        assert!(result
            .matched_patterns
            .contains(&"hex_encoding".to_string()));
    }
}
