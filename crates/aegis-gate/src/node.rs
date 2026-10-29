//! An `aegis-server` booted from the binary under test with a scrubbed environment: 127.0.0.1 on
//! a free port, persistence and the built-in vault in a scratch data dir, a one-run admin from
//! `AEGIS_ADMIN_USERNAME` / `AEGIS_ADMIN_PASSWORD`, no peers, no TLS, no external Vault. Stopped
//! with SIGTERM (the server saves to disk on it) so a restart on the same data dir can be checked.

use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// ── node ──

pub struct Node {
    pub port: u16,
    pub log: PathBuf,
    child: Child,
}

impl Node {
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(mut self) -> Result<(), String> {
        let pid = self.child.id() as libc::pid_t;
        if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
            return Err(format!(
                "SIGTERM to {pid}: {}",
                std::io::Error::last_os_error()
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            match self.child.try_wait() {
                Ok(Some(s)) if s.success() => return Ok(()),
                Ok(Some(s)) => return Err(format!("exited {s} after SIGTERM")),
                Ok(None) if Instant::now() > deadline => {
                    return Err("still running 45 s after SIGTERM".into())
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Run {
    pub bin: PathBuf,
    pub dir: PathBuf,
    pub out: PathBuf,
    pub user: String,
    pub pass: String,
}

fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("free port: {e}"))?;
    l.local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("free port: {e}"))
}

pub fn one_run_secret() -> Result<String, String> {
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .map_err(|e| format!("/dev/urandom: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

impl Run {
    pub fn new(bin: &Path, out: &Path) -> Result<Self, String> {
        let dir = out.join("run");
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["home", "data"] {
            let d = dir.join(sub);
            std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        Ok(Self {
            bin: bin.to_path_buf(),
            dir,
            out: out.to_path_buf(),
            user: "aegis-gate".into(),
            pass: one_run_secret()?,
        })
    }

    pub fn boot(&self, label: &str) -> Result<Node, String> {
        let port = free_port()?;
        let log = self.out.join(format!("aegis-server-{label}.log"));
        let stderr = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
        let stdout = stderr.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&self.bin);
        cmd.args(["--host", "127.0.0.1", "--port", &port.to_string()])
            .arg("--data-dir")
            .arg(self.dir.join("data"))
            .args(["--node-id", "aegis-gate", "--node-name", "aegis-gate"])
            .env_clear()
            .env("HOME", self.dir.join("home"))
            .env("RUST_LOG", "info")
            .env("AEGIS_ADMIN_USERNAME", &self.user)
            .env("AEGIS_ADMIN_PASSWORD", &self.pass)
            .env("AEGIS_RATE_LIMIT_PER_MINUTE", "100000")
            .env("AEGIS_LOGIN_RATE_LIMIT_PER_MINUTE", "1000")
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        if let Some(path) = std::env::var_os("PATH") {
            cmd.env("PATH", path);
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("start {}: {e}", self.bin.display()))?;
        let mut n = Node { port, log, child };
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if matches!(request(port, "GET", "/health", "", None), Ok(r) if r.status == 200) {
                return Ok(n);
            }
            if !n.alive() {
                return Err(format!(
                    "aegis-server exited before answering /health (see {})",
                    n.log.display()
                ));
            }
            if Instant::now() > deadline {
                return Err(format!("aegis-server not healthy on :{port} after 60 s"));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

// ── http ──

pub struct Reply {
    pub status: u16,
    pub body: String,
}

impl Reply {
    pub fn json(&self) -> Result<Value, String> {
        serde_json::from_str(&self.body)
            .map_err(|e| format!("not JSON ({e}): {}", head(&self.body)))
    }
}

pub fn head(s: &str) -> String {
    s.chars().take(200).collect()
}

pub fn request(
    port: u16,
    method: &str,
    path: &str,
    body: &str,
    token: Option<&str>,
) -> Result<Reply, String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("{path}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(40)))
        .map_err(|e| e.to_string())?;
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !body.is_empty() {
        req.push_str("Content-Type: application/json\r\n");
    }
    if let Some(t) = token {
        req.push_str(&format!("Authorization: Bearer {t}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes())
        .map_err(|e| format!("{path}: {e}"))?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw)
        .map_err(|e| format!("{path}: {e}"))?;
    parse_reply(&raw).ok_or_else(|| format!("{path}: malformed HTTP reply"))
}

fn parse_reply(raw: &[u8]) -> Option<Reply> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status = head.split_whitespace().nth(1)?.parse().ok()?;
    let chunked = head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_string()
    };
    Some(Reply { status, body })
}

fn dechunk(mut s: &str) -> Option<String> {
    let mut out = String::new();
    loop {
        let (size, rest) = s.split_once("\r\n")?;
        let n = usize::from_str_radix(size.split(';').next()?.trim(), 16).ok()?;
        if n == 0 {
            return Some(out);
        }
        out.push_str(rest.get(..n)?);
        s = rest.get(n..)?.strip_prefix("\r\n")?;
    }
}

pub fn log_problems(log: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.lines()
        .filter(|l| l.contains("panicked") || l.contains("RUST_BACKTRACE"))
        .map(|l| format!("{}: {}", log.display(), head(l)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replies_parse_status_body_and_chunks() {
        let r = parse_reply(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\n\r\n{}").unwrap();
        assert_eq!(r.status, 401);
        assert_eq!(r.json().unwrap(), json!({}));
        let c = parse_reply(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(c.json().unwrap(), json!({ "a": 1 }));
        assert!(parse_reply(b"garbage").is_none());
    }

    #[test]
    fn secrets_are_fresh_hex_long_enough_for_the_admin() {
        let a = one_run_secret().unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, one_run_secret().unwrap());
    }
}
