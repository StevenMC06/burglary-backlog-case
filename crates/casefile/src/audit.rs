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
    /// Hosted demos cap the log so a public visitor cannot grow it without bound.
    cap: Option<usize>,
}

impl AuditLog {
    pub fn open(root: &Path) -> Self {
        Self { path: root.join("data/audit.jsonl"), cap: None }
    }

    /// Refuse appends once the log holds `cap` entries.
    pub fn with_cap(mut self, cap: usize) -> Self {
        self.cap = Some(cap);
        self
    }

    /// Missing logs are new logs; all other read/parse failures are errors.
    pub fn try_entries(&self) -> anyhow::Result<Vec<Entry>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.into()),
        };
        text.lines().enumerate().map(|(i, line)| {
            serde_json::from_str(line).map_err(|e| anyhow::anyhow!("invalid audit line {}: {e}", i + 1))
        }).collect()
    }

    #[cfg(test)]
    pub fn entries(&self) -> Vec<Entry> {
        self.try_entries().unwrap_or_default()
    }

    pub fn ensure_appendable(&self) -> anyhow::Result<()> {
        self.verify().map_err(|seq| anyhow::anyhow!("audit integrity failure at sequence {seq}"))?;
        let all = self.try_entries()?;
        if !all.is_empty() && !std::fs::read(&self.path)?.ends_with(b"\n") {
            anyhow::bail!("audit final entry is missing its newline; refusing append");
        }
        if self.cap.is_some_and(|cap| all.len() >= cap) {
            anyhow::bail!("demo audit log is full");
        }
        Ok(())
    }

    pub fn append(
        &self,
        actor: &str,
        action: &str,
        subject: &str,
        detail: serde_json::Value,
    ) -> anyhow::Result<Entry> {
        self.ensure_appendable()?;
        let all = self.try_entries()?;
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
        let all = self.try_entries().map_err(|_| 0_u64)?;
        for (index, e) in all.iter().enumerate() {
            if e.seq != index as u64 || e.prev != prev || digest(e) != e.hash {
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
    fn corruption_and_read_errors_fail_closed() {
        let dir = std::env::temp_dir().join(format!("casefile-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let log = AuditLog::open(&dir);
        log.append("det.demo", "accept_step", "CC-1", serde_json::json!({})).unwrap();
        let original = std::fs::read_to_string(&log.path).unwrap();
        for broken in [format!("{original}garbage\n"), format!("garbage\n{original}")] {
            std::fs::write(&log.path, &broken).unwrap();
            assert!(log.try_entries().is_err());
            assert!(log.verify().is_err());
            assert!(log.append("det.demo", "accept_step", "CC-1", serde_json::json!({})).is_err());
            assert_eq!(std::fs::read_to_string(&log.path).unwrap(), broken);
        }
        std::fs::write(&log.path, original.trim_end()).unwrap();
        let before = std::fs::read(&log.path).unwrap();
        assert!(log.append("demo", "x", "y", serde_json::json!({})).is_err());
        assert_eq!(std::fs::read(&log.path).unwrap(), before);
        let mut entry: Entry = serde_json::from_str(original.trim()).unwrap();
        entry.seq = 9;
        entry.hash = digest(&entry);
        std::fs::write(&log.path, serde_json::to_string(&entry).unwrap()).unwrap();
        assert!(log.verify().is_err(), "even rehashed entries need contiguous sequences");
        std::fs::remove_file(&log.path).unwrap();
        std::fs::create_dir(&log.path).unwrap();
        assert!(log.try_entries().is_err());
        assert!(log.verify().is_err());
        assert!(log.append("demo", "x", "y", serde_json::json!({})).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

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
