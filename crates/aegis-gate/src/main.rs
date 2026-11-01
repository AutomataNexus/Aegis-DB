//! The Aegis-DB release gate, run by NexusForge after fmt / clippy -D warnings / test and before the
//! attested release (`nexusforge-gates.toml`): cargo audit over Cargo.lock, then the `aegis-server`
//! binary under test booted on loopback with a scratch data dir and its query and document-store
//! suite run over HTTP (auth refusals, SQL DDL / DML / params / errors, collections, documents,
//! filters, sort, cursor paging, patch / replace / delete), then the server stopped with SIGTERM,
//! booted again on the same data dir, and the data read back. Nothing leaves 127.0.0.1.
//! Exit 0 = green.

mod node;
mod suite;

use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
aegis-gate --bin PATH [--out DIR] [--workspace .] [--audit 1] [--audit-ignore ID,ID]
  --bin           the aegis-server binary under test (target/release/aegis-server)
  --out           server logs, audit.txt and report.json (target/nexusforge-gate/aegis)
  --workspace     the Aegis-DB checkout (Cargo.lock)
  --audit 0       skip cargo audit (offline runs only; NexusForge runs it)
  --audit-ignore  advisory IDs cargo audit ignores (each one reasoned in nexusforge-gates.toml)";

// ── args ──

struct Args(HashMap<String, String>);

impl Args {
    fn parse(argv: &[String]) -> Result<Self, String> {
        let mut m = HashMap::new();
        let mut it = argv.iter();
        while let Some(a) = it.next() {
            let k = a
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected argument {a}"))?;
            let v = it.next().ok_or_else(|| format!("--{k} needs a value"))?;
            m.insert(k.to_string(), v.clone());
        }
        Ok(Self(m))
    }

    fn get(&self, k: &str) -> Option<&str> {
        self.0.get(k).map(String::as_str)
    }

    fn path(&self, k: &str, default: &str) -> PathBuf {
        PathBuf::from(self.get(k).unwrap_or(default))
    }

    fn num(&self, k: &str, default: u64) -> Result<u64, String> {
        self.get(k).map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("--{k} must be a number"))
        })
    }

    fn list(&self, k: &str) -> Vec<String> {
        self.get(k)
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ── steps ──

pub struct Step {
    pub name: String,
    pub problems: Vec<String>,
}

fn print_step(s: &Step) {
    if s.problems.is_empty() {
        println!("  PASS  {}", s.name);
    } else {
        println!("  FAIL  {}", s.name);
        for p in &s.problems {
            println!("          - {p}");
        }
    }
}

fn audit_args(lock: &Path, ignore: &[String]) -> Vec<String> {
    let mut v: Vec<String> = ["audit", "--deny", "warnings", "--file"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    v.push(lock.display().to_string());
    for id in ignore {
        v.push("--ignore".to_string());
        v.push(id.clone());
    }
    v
}

fn audit(workspace: &Path, out: &Path, ignore: &[String]) -> Step {
    let lock = workspace.join("Cargo.lock");
    let log = out.join("audit.txt");
    let name = if ignore.is_empty() {
        "cargo audit --deny warnings".to_string()
    } else {
        format!(
            "cargo audit --deny warnings (ignoring {})",
            ignore.join(", ")
        )
    };
    let run = Command::new("cargo")
        .args(audit_args(&lock, ignore))
        .current_dir(workspace)
        .output();
    let problems = match run {
        Ok(o) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            let _ = std::fs::write(&log, &text);
            if o.status.success() {
                Vec::new()
            } else {
                let mut p = advisories(&text);
                p.push(format!("full output: {}", log.display()));
                p
            }
        }
        Err(e) => vec![format!(
            "cargo audit did not run ({e}); install it: cargo install cargo-audit"
        )],
    };
    Step { name, problems }
}

fn advisories(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut krate, mut version, mut kind) = (String::new(), String::new(), String::new());
    for l in text.lines() {
        let field = |p: &str| l.strip_prefix(p).map(|v| v.trim().to_string());
        if let Some(v) = field("Crate:") {
            krate = v;
        } else if let Some(v) = field("Version:") {
            version = v;
        } else if let Some(v) = field("Warning:") {
            kind = v;
        } else if let Some(v) = field("ID:") {
            let tag = if kind.is_empty() {
                "vulnerability".to_string()
            } else {
                std::mem::take(&mut kind)
            };
            out.push(format!("{v} {krate} {version} ({tag})"));
        } else if l.starts_with("error:") {
            out.push(l.to_string());
        }
    }
    out
}

fn gate(a: &Args) -> Result<bool, String> {
    let bin = a.path("bin", "target/release/aegis-server");
    let bin = std::fs::canonicalize(&bin)
        .ok()
        .filter(|b| b.is_file())
        .ok_or_else(|| format!("no aegis-server binary at {}", bin.display()))?;
    let workspace =
        std::fs::canonicalize(a.path("workspace", ".")).map_err(|e| format!("--workspace: {e}"))?;
    let out = a.path("out", "target/nexusforge-gate/aegis");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let out = std::fs::canonicalize(&out).map_err(|e| e.to_string())?;
    println!("aegis gate: {} → {}", bin.display(), out.display());
    let mut steps = Vec::new();

    if a.num("audit", 1)? != 0 {
        println!("[1/3] dependency audit");
        let s = audit(&workspace, &out, &a.list("audit-ignore"));
        print_step(&s);
        steps.push(s);
    } else {
        println!("[1/3] dependency audit skipped (--audit 0)");
    }

    println!("[2/3] aegis-server on loopback: auth, SQL, document store");
    let run = node::Run::new(&bin, &out)?;
    let first = suite::first_boot(&run, &mut |s| {
        print_step(&s);
        steps.push(s);
    });

    println!("[3/3] SIGTERM, boot again on the same data dir, read back");
    if first {
        suite::second_boot(&run, &mut |s| {
            print_step(&s);
            steps.push(s);
        });
    } else {
        let s = Step {
            name: "restart on the same data dir".into(),
            problems: vec!["not run: the first boot did not finish".into()],
        };
        print_step(&s);
        steps.push(s);
    }
    finish(&bin, &out, &steps)
}

fn finish(bin: &Path, out: &Path, steps: &[Step]) -> Result<bool, String> {
    let failed = steps.iter().filter(|s| !s.problems.is_empty()).count();
    let report = json!({
        "binary": bin.display().to_string(),
        "checks": steps.len(),
        "failed": failed,
        "steps": steps.iter().map(|s| json!({ "name": s.name, "problems": s.problems })).collect::<Vec<_>>(),
    });
    let path = out.join("report.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    println!(
        "{} checks, {failed} failed · {}",
        steps.len(),
        path.display()
    );
    Ok(failed == 0)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match Args::parse(&argv).and_then(|a| gate(&a)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("aegis-gate: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_take_flag_value_pairs_only() {
        let a = Args::parse(&["--bin".into(), "x".into(), "--audit".into(), "0".into()]).unwrap();
        assert_eq!(a.get("bin"), Some("x"));
        assert_eq!(a.num("audit", 1).unwrap(), 0);
        assert_eq!(a.path("out", "d"), PathBuf::from("d"));
        assert!(Args::parse(&["stray".into()]).is_err());
        assert!(Args::parse(&["--bin".into()]).is_err());
        assert!(Args::parse(&["--audit".into(), "x".into()])
            .unwrap()
            .num("audit", 1)
            .is_err());
    }

    #[test]
    fn audit_ignore_list_becomes_ignore_flags() {
        let a = Args::parse(&["--audit-ignore".into(), "A-1, B-2,".into()]).unwrap();
        let ids = a.list("audit-ignore");
        assert_eq!(ids, vec!["A-1".to_string(), "B-2".to_string()]);
        let want = "audit --deny warnings --file Cargo.lock --ignore A-1 --ignore B-2";
        assert_eq!(audit_args(Path::new("Cargo.lock"), &ids).join(" "), want);
        assert!(Args::parse(&[]).unwrap().list("audit-ignore").is_empty());
    }

    #[test]
    fn audit_output_becomes_one_line_per_advisory() {
        let text = "Crate:     h2\nVersion:   0.4.14\nTitle:     x\nID:        RUSTSEC-2026-0258\n\
                    Crate:     instant\nVersion:   0.1.13\nWarning:   unmaintained\nTitle:     y\nID:        RUSTSEC-2024-0384\n\
                    error: 1 vulnerability found!\n";
        assert_eq!(
            advisories(text),
            vec![
                "RUSTSEC-2026-0258 h2 0.4.14 (vulnerability)",
                "RUSTSEC-2024-0384 instant 0.1.13 (unmaintained)",
                "error: 1 vulnerability found!",
            ]
        );
    }
}
