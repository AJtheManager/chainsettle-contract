#![cfg(test)]

//! Keeps docs/events.md in sync with the contract: every literal event topic
//! passed to `events().publish` must be documented.

extern crate std;

use std::format;
use std::string::String;
use std::vec::Vec;

const CONTRACT_SOURCE: &str = include_str!("lib.rs");
const EVENTS_DOC: &str = include_str!("../../../docs/events.md");

/// Literal `Symbol::new` topics inside each `publish((...), ...)` topic tuple.
fn published_topics(src: &str) -> Vec<String> {
    let mut topics = Vec::new();
    for (start, _) in src.match_indices(".publish(") {
        let call = &src[start + ".publish(".len()..];
        let Some(tuple) = call.trim_start().strip_prefix('(') else { continue };
        // The topic tuple ends at its matching ')'.
        let mut depth = 1;
        let mut end = 0;
        for (i, c) in tuple.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                end = i;
                break;
            }
        }
        for part in tuple[..end].split("Symbol::new(").skip(1) {
            let Some(comma) = part.find(',') else { continue };
            let Some(literal) = part[comma + 1..].trim_start().strip_prefix('"') else {
                continue;
            };
            if let Some(close) = literal.find('"') {
                topics.push(String::from(&literal[..close]));
            }
        }
    }
    topics
}

#[test]
fn test_every_published_topic_is_documented() {
    let topics = published_topics(CONTRACT_SOURCE);
    assert!(topics.len() > 100, "topic scan found only {} topics", topics.len());
    let missing: Vec<String> = topics
        .iter()
        .filter(|t| !EVENTS_DOC.contains(&format!("`{}`", t)))
        .cloned()
        .collect();
    assert!(
        missing.is_empty(),
        "topics missing from docs/events.md: {:?}",
        missing
    );
}
