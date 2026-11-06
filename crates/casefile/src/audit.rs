//! Append-only, hash-chained audit log (data/audit.jsonl). Each line:
//! {"seq":n,"ts":"...","actor":"...","action":"...","subject":"...","detail":{...},"engine":"...",
//!  "prev":"<hex>","hash":"<hex>"} where hash = sha256(prev || json-without-hash).
//! Tampering with any line breaks its hash and every later link: `verify` reports the first bad seq.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub seq: u64,
    pub ts: String,
    pub actor: String,
    pub action: String,
    pub subject: String,
    pub detail: serde_json::Value,
    pub engine: String,
    pub prev: String,
    #[serde(default)]
    pub hash: String,
}

fn digest(e: &Entry) -> String {
    let mut c = e.clone();
    c.hash = String::new();
    let body = serde_json::to_string(&c).expect("serializable");
    let mut h = Sha256::new();
    h.update(e.prev.as_bytes());
    h.update(body.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn open(root: &Path) -> Self {
        Self { path: root.join("data/audit.jsonl") }
    }

    pub fn entries(&self) -> Vec<Entry> {
        std::fs::read_to_string(&self.path)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    pub fn append(
        &self,
        actor: &str,
        action: &str,
        subject: &str,
        detail: serde_json::Value,
    ) -> anyhow::Result<Entry> {
        let all = self.entries();
        let (seq, prev) =
            all.last().map(|e| (e.seq + 1, e.hash.clone())).unwrap_or((0, "genesis".into()));
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();
        let mut e = Entry {
            seq,
            ts,
            actor: actor.into(),
            action: action.into(),
            subject: subject.into(),
            detail,
            engine: casefile_core::ENGINE_VERSION.into(),
            prev,
            hash: String::new(),
        };
        e.hash = digest(&e);
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{}", serde_json::to_string(&e)?)?;
        Ok(e)
    }

    /// Ok(n) = chain of n entries intact; Err(seq) = first broken entry.
    pub fn verify(&self) -> Result<usize, u64> {
        let mut prev = "genesis".to_string();
        let all = self.entries();
        for e in &all {
            if e.prev != prev || digest(e) != e.hash {
                return Err(e.seq);
            }
            prev = e.hash.clone();
        }
        Ok(all.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_detects_tampering() {
        let dir = std::env::temp_dir().join(format!("casefile-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let log = AuditLog::open(&dir);
        log.append("sgt.demo", "approve_assignment", "CC-1", serde_json::json!({"ok": true})).unwrap();
        log.append("det.demo", "reject_step", "CC-1", serde_json::json!({"reason": "not lawful"}))
            .unwrap();
        assert_eq!(log.verify(), Ok(2));
        let p = dir.join("data/audit.jsonl");
        let text = std::fs::read_to_string(&p).unwrap().replace("not lawful", "lawful");
        std::fs::write(&p, text).unwrap();
        assert_eq!(log.verify(), Err(1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
