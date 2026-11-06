//! Citation checker: every claim in an agent-written packet must point at a real record field.
//!
//! SPEC: tokens look like `[CC-25-004812:entry_point]` or `[PWN-000123:serial]` or with an
//! expected value `[CC-25-004812:entry_point=sliding_door]`. Valid record ids are case ids and pawn
//! ids in the dataset. Valid fields: any top-level field of Case/PawnRecord, plus `mo.<field>`,
//! `facts.<field>`, `items[<item id>].<field>`. With `=value`, compare against the serialized
//! JSON value (strings compared case-insensitively, enums by snake_case name). Report each invalid
//! token with a precise problem. `uncited_sentences`: sentences in the text that contain a factual
//! claim marker (a digit, a case id, or words like "same", "matches", "linked") but no token.
//!
//! Tokens are bracketed segments that contain a `:`; other brackets are ignored. A token whose
//! record id is not in the dataset is reported as an unknown record.
use crate::types::*;
use serde_json::Value;

const CLAIM_WORDS: [&str; 6] = ["same", "match", "linked", "identical", "both", "similar"];

/// Walks `path` (dot-separated, with `items[<id>]` selecting an item by id) into `root`.
fn resolve<'a>(root: &'a Value, path: &str) -> Result<&'a Value, String> {
    let mut cur = root;
    for seg in path.split('.') {
        if seg.is_empty() {
            return Err("empty field segment".into());
        }
        let (name, idx) = match seg.find('[') {
            Some(i) if seg.ends_with(']') => (&seg[..i], Some(&seg[i + 1..seg.len() - 1])),
            Some(_) => return Err(format!("malformed segment '{}'", seg)),
            None => (seg, None),
        };
        cur = cur.get(name).ok_or_else(|| format!("no field '{}'", name))?;
        if let Some(id) = idx {
            let arr = cur.as_array().ok_or_else(|| format!("'{}' is not a list", name))?;
            cur = arr
                .iter()
                .find(|e| e.get("id").and_then(Value::as_str) == Some(id))
                .ok_or_else(|| format!("no element with id '{}' in '{}'", id, name))?;
        }
    }
    Ok(cur)
}

fn value_matches(actual: &Value, expected: &str) -> bool {
    let expected = expected.trim();
    match actual {
        Value::String(s) => s.eq_ignore_ascii_case(expected),
        Value::Null => expected.eq_ignore_ascii_case("null"),
        other => *other.to_string() == *expected,
    }
}

fn check_token(root_of: &dyn Fn(&str) -> Option<Value>, body: &str) -> Result<(), String> {
    let Some((id, rest)) = body.split_once(':') else {
        return Err("missing ':' between record id and field".into());
    };
    let (field, expected) = match rest.split_once('=') {
        Some((f, v)) => (f.trim(), Some(v)),
        None => (rest.trim(), None),
    };
    let Some(root) = root_of(id.trim()) else {
        return Err(format!("unknown record id '{}'", id.trim()));
    };
    if field.is_empty() {
        return Err("empty field".into());
    }
    let actual = resolve(&root, field).map_err(|e| format!("invalid field '{}': {}", field, e))?;
    if let Some(exp) = expected
        && !value_matches(actual, exp)
    {
        return Err(format!("value mismatch for '{}': record has {} but cited {}", field, actual, exp.trim()));
    }
    Ok(())
}

/// Splits `text` into sentences on . ! ? (not when the dot sits between alphanumerics or inside
/// a bracketed token), returning (sentence, tokens found in it).
fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0usize);
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        cur.push(c);
        let inner_dot = c == '.'
            && i > 0
            && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric())
            && chars[i - 1].is_alphanumeric();
        if matches!(c, '.' | '!' | '?') && depth == 0 && !inner_dot {
            out.push(std::mem::take(&mut cur));
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn tokens_in(s: &str) -> Vec<&str> {
    // Outermost bracket pairs; nested `[..]` (items[ID]) stay inside the token.
    let mut out = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    for (i, c) in s.char_indices() {
        match c {
            '[' => {
                if depth == 0 {
                    start = i + 1;
                }
                depth += 1;
            }
            ']' if depth > 0 => {
                depth -= 1;
                if depth == 0 && s[start..i].contains(':') {
                    out.push(&s[start..i]);
                }
            }
            _ => {}
        }
    }
    out
}

pub fn check(ds: &Dataset, text: &str) -> CitationReport {
    let root_of = |id: &str| -> Option<Value> {
        if let Some(c) = ds.cases.iter().find(|c| c.id == id) {
            return serde_json::to_value(c).ok();
        }
        ds.pawn.iter().find(|p| p.id == id).and_then(|p| serde_json::to_value(p).ok())
    };
    let case_ids: Vec<&str> = ds.cases.iter().map(|c| c.id.as_str()).collect();
    let (mut total, mut valid) = (0usize, 0usize);
    let mut problems = Vec::new();
    let mut uncited = Vec::new();
    for sent in sentences(text) {
        let toks = tokens_in(&sent);
        for body in &toks {
            total += 1;
            match check_token(&root_of, body) {
                Ok(()) => valid += 1,
                Err(problem) => problems.push(CitationProblem { token: format!("[{}]", body), problem }),
            }
        }
        if toks.is_empty() {
            let lower = sent.to_lowercase();
            let claim = sent.chars().any(|c| c.is_ascii_digit())
                || case_ids.iter().any(|id| sent.contains(id))
                || CLAIM_WORDS.iter().any(|w| lower.contains(w));
            if claim {
                uncited.push(sent);
            }
        }
    }
    CitationReport { total, valid, problems, uncited_sentences: uncited }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linkage::test_support::case;

    fn ds() -> Dataset {
        let mut c = case("CC-25-001", 47.6, -122.3, 0, 2);
        c.items.push(StolenItem {
            id: "I1".into(),
            category: PropertyType::Laptop,
            brand: None,
            model: None,
            serial: Some("AB123".into()),
            description: "laptop".into(),
            markings: vec![],
            value_usd: 500,
        });
        Dataset {
            cases: vec![c],
            pawn: vec![PawnRecord {
                id: "PWN-1".into(),
                dealer: "D".into(),
                received_at: 0,
                location: GeoPoint { lat: 0.0, lon: 0.0 },
                category: PropertyType::Laptop,
                brand: None,
                model: None,
                serial: Some("AB123".into()),
                description: "laptop".into(),
                markings: vec![],
                seller_ref: "S".into(),
            }],
        }
    }

    #[test]
    fn plain_and_nested_fields_are_valid() {
        let r = check(&ds(), "Entry [CC-25-001:mo.entry_point] and [PWN-1:serial] and [CC-25-001:items[I1].serial] and [CC-25-001:facts.witness].");
        assert_eq!((r.total, r.valid), (4, 4), "{:?}", r.problems);
        assert!(r.uncited_sentences.is_empty());
    }

    #[test]
    fn expected_values_compare() {
        let d = ds();
        let ok = check(&d, "[CC-25-001:mo.entry_point=REAR_DOOR] [CC-25-001:items[I1].serial=ab123] [CC-25-001:facts.witness=false]");
        assert_eq!((ok.total, ok.valid), (3, 3), "{:?}", ok.problems);
        let bad = check(&d, "[CC-25-001:mo.entry_point=window]");
        assert_eq!(bad.valid, 0);
        assert!(bad.problems[0].problem.contains("mismatch"));
    }

    #[test]
    fn invalid_tokens_reported() {
        let r = check(&ds(), "[CC-25-001:nope] [CC-25-999:block] [CC-25-001:items[ZZ].serial] [PWN-1:mo.x]");
        assert_eq!((r.total, r.valid), (4, 0));
        assert_eq!(r.problems.len(), 4);
        assert!(r.problems[1].problem.contains("unknown record"));
    }

    #[test]
    fn uncited_claim_sentences_flagged() {
        let r = check(&ds(), "The laptop was stolen at 3pm. Looks fine overall. Same entry as CC-25-001 [CC-25-001:mo.entry_point]. Both cases are linked.");
        assert_eq!(r.uncited_sentences, vec!["The laptop was stolen at 3pm.", "Both cases are linked."]);
    }
}
