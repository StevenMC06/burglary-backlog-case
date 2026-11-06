//! Citation checker: every claim in an agent-written packet must point at a real record field.
//!
//! SPEC: tokens look like `[CC-25-004812:entry_point]` or `[PWN-000123:serial]` or with an
//! expected value `[CC-25-004812:entry_point=sliding_door]`. Valid record ids are case ids and pawn
//! ids in the dataset. Valid fields: any top-level field of Case/PawnRecord, plus `mo.<field>`,
//! `facts.<field>`, `items[<item id>].<field>`. With `=value`, compare against the serialized
//! JSON value (strings compared case-insensitively, enums by snake_case name). Report each invalid
//! token with a precise problem. `uncited_sentences`: sentences in the text that contain a factual
//! claim marker (a digit, a case id, or words like "same", "matches", "linked") but no token.
use crate::types::*;

pub fn check(ds: &Dataset, text: &str) -> CitationReport {
    let _ = (ds, text);
    todo!("citations::check")
}
