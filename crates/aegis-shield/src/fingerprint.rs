//! Request fingerprinting and scanner detection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Classification of the user-agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserAgentClass {
    Browser,
    ApiClient,
    Bot,
    Scanner,
    Unknown,
}

/// Result of fingerprinting a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestFingerprint {
    pub user_agent_class: UserAgentClass,
    pub suspicion_score: u32,
}

/// Known scanner signatures (lowercased for matching).
const SCANNER_SIGNATURES: &[&str] = &[
    "sqlmap",
    "nikto",
    "nmap",
    "masscan",
    "gobuster",
    "dirbuster",
    "wfuzz",
    "hydra",
    "burpsuite",
    "burp suite",
    "owasp zap",
    "zaproxy",
    "w3af",
    "arachni",
    "skipfish",
    "havij",
    "acunetix",
    "nessus",
    "openvas",
];

const BOT_SIGNATURES: &[&str] = &[
    "googlebot",
    "bingbot",
    "baiduspider",
    "yandexbot",
    "duckduckbot",
    "slurp",
    "ia_archiver",
    "facebot",
    "twitterbot",
    "linkedinbot",
    "semrushbot",
    "ahrefsbot",
    "mj12bot",
    "dotbot",
    "petalbot",
];

const BROWSER_SIGNATURES: &[&str] = &["mozilla", "chrome", "safari", "firefox", "edge", "opera"];

const API_CLIENT_SIGNATURES: &[&str] = &[
    "curl",
    "wget",
    "httpie",
    "postman",
    "insomnia",
    "axios",
    "python-requests",
    "go-http-client",
    "java/",
    "okhttp",
];

/// Fingerprints incoming requests based on user-agent and headers.
pub struct RequestFingerprinter {
    // reserved for future extensibility
    _private: (),
}

impl RequestFingerprinter {
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Analyze a request's user-agent and headers to produce a fingerprint.
    pub fn analyze(
        &self,
        user_agent: Option<&str>,
        headers: &HashMap<String, String>,
    ) -> RequestFingerprint {
        let mut score: u32 = 0;

        let ua = match user_agent {
            Some(ua) if !ua.trim().is_empty() => ua,
            _ => {
                // Missing or empty user-agent is suspicious
                return RequestFingerprint {
                    user_agent_class: UserAgentClass::Unknown,
                    suspicion_score: 40,
                };
            }
        };

        let ua_lower = ua.to_lowercase();

        // Check scanner signatures first (highest priority)
        for sig in SCANNER_SIGNATURES {
            if ua_lower.contains(sig) {
                return RequestFingerprint {
                    user_agent_class: UserAgentClass::Scanner,
                    suspicion_score: 90,
                };
            }
        }

        // Check bots
        for sig in BOT_SIGNATURES {
            if ua_lower.contains(sig) {
                return RequestFingerprint {
                    user_agent_class: UserAgentClass::Bot,
                    suspicion_score: 20,
                };
            }
        }

        // Classify as browser or API client
        let mut class = UserAgentClass::Unknown;

        for sig in BROWSER_SIGNATURES {
            if ua_lower.contains(sig) {
                class = UserAgentClass::Browser;
                break;
            }
        }

        if class == UserAgentClass::Unknown {
            for sig in API_CLIENT_SIGNATURES {
                if ua_lower.contains(sig) {
                    class = UserAgentClass::ApiClient;
                    break;
                }
            }
        }

        // Heuristic checks
        if class == UserAgentClass::Unknown {
            score += 15; // unrecognized user-agent
        }

        // Very short user-agent
        if ua.len() < 10 {
            score += 10;
        }

        // Suspicious headers
        if headers.contains_key("x-forwarded-for") {
            // Multiple forwards can indicate proxy chains
            if let Some(val) = headers.get("x-forwarded-for") {
                let count = val.matches(',').count();
                if count > 3 {
                    score += 15;
                }
            }
        }

        // Missing common browser headers when claiming to be a browser
        if class == UserAgentClass::Browser {
            if !headers.contains_key("accept") && !headers.contains_key("Accept") {
                score += 10;
            }
        }

        RequestFingerprint {
            user_agent_class: class,
            suspicion_score: score.min(100),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp() -> RequestFingerprinter {
        RequestFingerprinter::new()
    }

    #[test]
    fn test_scanner_detection_sqlmap() {
        let result = fp().analyze(Some("sqlmap/1.6"), &HashMap::new());
        assert_eq!(result.user_agent_class, UserAgentClass::Scanner);
        assert!(result.suspicion_score >= 90);
    }

    #[test]
    fn test_scanner_detection_nikto() {
        let result = fp().analyze(Some("Nikto/2.1.6"), &HashMap::new());
        assert_eq!(result.user_agent_class, UserAgentClass::Scanner);
    }

    #[test]
    fn test_bot_detection() {
        let result = fp().analyze(
            Some("Mozilla/5.0 (compatible; Googlebot/2.1)"),
            &HashMap::new(),
        );
        assert_eq!(result.user_agent_class, UserAgentClass::Bot);
    }

    #[test]
    fn test_browser_detection() {
        let result = fp().analyze(
            Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0"),
            &HashMap::new(),
        );
        assert_eq!(result.user_agent_class, UserAgentClass::Browser);
    }

    #[test]
    fn test_api_client_detection() {
        let result = fp().analyze(Some("curl/7.81.0"), &HashMap::new());
        assert_eq!(result.user_agent_class, UserAgentClass::ApiClient);
    }

    #[test]
    fn test_missing_user_agent() {
        let result = fp().analyze(None, &HashMap::new());
        assert_eq!(result.user_agent_class, UserAgentClass::Unknown);
        assert!(result.suspicion_score >= 30);
    }

    #[test]
    fn test_empty_user_agent() {
        let result = fp().analyze(Some(""), &HashMap::new());
        assert_eq!(result.user_agent_class, UserAgentClass::Unknown);
        assert!(result.suspicion_score >= 30);
    }
}
