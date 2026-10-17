#![cfg(test)]

//! Guards Soroban's 32-character limit on symbols. An overlong
//! `Symbol::new` literal or contract function name only fails at runtime on
//! the code path that uses it, so this scans the source instead.

extern crate std;

use std::format;
use std::string::String;
use std::vec::Vec;

const MAX_SYMBOL_LEN: usize = 32;
const CONTRACT_SOURCE: &str = include_str!("lib.rs");

fn line_of(src: &str, offset: usize) -> usize {
    src[..offset].matches('\n').count() + 1
}

/// Every `Symbol::new(<env>, "...")` literal and `pub fn` name longer than
/// 32 characters, as "line N: ..." messages.
fn symbol_limit_violations(src: &str) -> Vec<String> {
    let mut violations = Vec::new();

    let marker = "Symbol::new(";
    let mut search_from = 0;
    while let Some(found) = src[search_from..].find(marker) {
        let args_start = search_from + found + marker.len();
        search_from = args_start;
        // Second argument, tolerating whitespace and line breaks around it.
        let Some(comma) = src[args_start..].find(',') else { break };
        let rest = &src[args_start + comma + 1..];
        let trimmed = rest.trim_start();
        let Some(literal) = trimmed.strip_prefix('"') else { continue };
        let Some(close) = literal.find('"') else { continue };
        let value = &literal[..close];
        if value.len() > MAX_SYMBOL_LEN {
            let literal_offset = src.len() - literal.len();
            violations.push(format!(
                "line {}: Symbol::new literal \"{}\" is {} characters",
                line_of(src, literal_offset),
                value,
                value.len()
            ));
        }
    }

    for (i, line) in src.lines().enumerate() {
        let Some(after) = line.trim_start().strip_prefix("pub fn ") else { continue };
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.len() > MAX_SYMBOL_LEN {
            violations.push(format!(
                "line {}: pub fn {} is {} characters",
                i + 1,
                name,
                name.len()
            ));
        }
    }

    violations
}

fn assert_no_symbol_limit_violations(src: &str) {
    let violations = symbol_limit_violations(src);
    assert!(
        violations.is_empty(),
        "symbols longer than {} characters:\n{}",
        MAX_SYMBOL_LEN,
        violations.join("\n")
    );
}

#[test]
fn test_contract_symbols_within_limit() {
    assert_no_symbol_limit_violations(CONTRACT_SOURCE);
}

#[test]
fn test_scanner_reports_every_violation_with_its_line() {
    let src = "fn a(env: Env) {\n\
               \x20   env.events().publish((Symbol::new(&env, \"this_event_topic_is_far_too_long_x\"),), ());\n\
               \x20   let ok = Symbol::new(&env, \"exactly_thirty_two_characters_ok\");\n\
               \x20   let multi = Symbol::new(\n\
               \x20       env,\n\
               \x20       \"another_overlong_topic_spanning_lines\",\n\
               \x20   );\n\
               }\n\
               pub fn this_function_name_is_much_too_long_x() {}\n\
               pub fn short_name() {}\n";
    let violations = symbol_limit_violations(src);
    assert_eq!(violations.len(), 3, "{:?}", violations);
    assert!(violations[0].starts_with("line 2: "), "{}", violations[0]);
    assert!(violations[1].starts_with("line 6: "), "{}", violations[1]);
    assert!(violations[2].starts_with("line 9: "), "{}", violations[2]);
}

#[test]
#[should_panic(expected = "symbols longer than 32 characters")]
fn test_deliberately_overlong_symbol_fails() {
    assert_no_symbol_limit_violations(
        "let s = Symbol::new(&env, \"a_deliberately_overlong_symbol_name\");",
    );
}
