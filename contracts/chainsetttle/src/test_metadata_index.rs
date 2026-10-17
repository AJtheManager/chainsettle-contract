#![cfg(test)]

extern crate std;

use super::*;
use crate::test_common::{create_standard_shipment, setup, TestSetup};
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, String, Symbol, TryFromVal};

fn create(t: &TestSetup, client: &ChainSettleContractClient, id: &str) -> String {
    let shipment_id = String::from_str(&t.env, id);
    create_standard_shipment(
        client,
        &t.env,
        &shipment_id,
        &t.buyer,
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        1_000_000,
    );
    shipment_id
}

fn po(t: &TestSetup) -> Symbol {
    Symbol::new(&t.env, "po_number")
}

fn s(t: &TestSetup, v: &str) -> String {
    String::from_str(&t.env, v)
}

fn has_event(t: &TestSetup, name: &str) -> bool {
    let wanted = Symbol::new(&t.env, name);
    t.env.events().all().iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .and_then(|v| Symbol::try_from_val(&t.env, &v).ok())
            .map(|sym| sym == wanted)
            .unwrap_or(false)
    })
}

// ── Admin configuration ─────────────────────────────────────────────────────

#[test]
fn test_metadata_key_not_indexed_by_default() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    assert!(!client.is_metadata_key_indexed(&po(&t)));
}

#[test]
fn test_admin_can_enable_and_disable_indexing() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);

    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    assert!(has_event(&t, "metadata_key_indexed"));
    assert!(client.is_metadata_key_indexed(&po(&t)));

    let log = client.get_admin_log();
    let last = log.get(log.len() - 1).unwrap();
    assert_eq!(last.action, Symbol::new(&t.env, "set_meta_indexed"));

    client.set_metadata_key_indexed(&t.buyer, &po(&t), &false);
    assert!(!client.is_metadata_key_indexed(&po(&t)));
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_non_admin_cannot_configure_indexing() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let stranger = Address::generate(&t.env);
    client.set_metadata_key_indexed(&stranger, &po(&t), &true);
}

#[test]
fn test_configure_indexing_requires_admin_signature() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    t.env.set_auths(&[]);
    assert!(client
        .try_set_metadata_key_indexed(&t.buyer, &po(&t), &true)
        .is_err());
}

// ── Lookup ──────────────────────────────────────────────────────────────────

#[test]
fn test_find_shipments_by_indexed_metadata() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);

    let a = create(&t, &client, "MI-A");
    let b = create(&t, &client, "MI-B");
    let c = create(&t, &client, "MI-C");

    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    client.set_shipment_metadata(&t.supplier, &b, &po(&t), &s(&t, "PO-1"));
    client.set_shipment_metadata(&t.buyer, &c, &po(&t), &s(&t, "PO-2"));

    let found = client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1"));
    assert_eq!(found.len(), 2);
    assert_eq!(found.get(0).unwrap(), a);
    assert_eq!(found.get(1).unwrap(), b);

    let found = client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-2"));
    assert_eq!(found.len(), 1);
    assert_eq!(found.get(0).unwrap(), c);

    assert_eq!(
        client
            .find_shipments_by_metadata(&po(&t), &s(&t, "PO-unknown"))
            .len(),
        0
    );
}

#[test]
fn test_setting_same_value_twice_does_not_duplicate() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    let a = create(&t, &client, "MI-DUP");

    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));

    assert_eq!(
        client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1")).len(),
        1
    );
    assert_eq!(client.get_shipment_metadata_keys(&a).len(), 1);
}

#[test]
fn test_changing_value_moves_shipment_between_index_buckets() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    let a = create(&t, &client, "MI-MOVE");

    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-OLD"));
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-NEW"));

    assert_eq!(
        client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-OLD")).len(),
        0
    );
    let found = client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-NEW"));
    assert_eq!(found.len(), 1);
    assert_eq!(found.get(0).unwrap(), a);
}

#[test]
#[should_panic(expected = "metadata key not indexed")]
fn test_unindexed_key_is_not_searchable() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let a = create(&t, &client, "MI-NOIDX");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1"));
}

#[test]
#[should_panic(expected = "metadata key not indexed")]
fn test_disabling_index_makes_key_unsearchable() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    let a = create(&t, &client, "MI-OFF");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &false);
    client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1"));
}

#[test]
fn test_values_set_before_indexing_are_not_backfilled() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let a = create(&t, &client, "MI-LATE");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));

    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    assert_eq!(
        client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1")).len(),
        0
    );

    // Re-setting the value indexes it.
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    assert_eq!(
        client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1")).len(),
        1
    );
}

#[test]
fn test_stale_entries_are_filtered_after_reindexing() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    let a = create(&t, &client, "MI-STALE");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));

    // While indexing is off the value changes; the index is cleaned on the
    // way out of the old bucket, and lookups double-check the live value.
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &false);
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-2"));
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);

    assert_eq!(
        client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1")).len(),
        0
    );
    assert_eq!(
        client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-2")).len(),
        0
    );
}

#[test]
fn test_other_keys_do_not_pollute_index() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    let a = create(&t, &client, "MI-OTHER");
    let cost_center = Symbol::new(&t.env, "cost_center");

    client.set_shipment_metadata(&t.buyer, &a, &cost_center, &s(&t, "PO-1"));
    assert_eq!(
        client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1")).len(),
        0
    );
}

#[test]
#[should_panic(expected = "metadata value must not be empty")]
fn test_find_rejects_empty_value() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    client.find_shipments_by_metadata(&po(&t), &s(&t, ""));
}

#[test]
#[should_panic(expected = "metadata value too long to index")]
fn test_set_rejects_oversized_value_for_indexed_key() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    let a = create(&t, &client, "MI-LONG");
    let long = "x".repeat(constants::MAX_METADATA_INDEX_VALUE_LEN as usize + 1);
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, &long));
}

// ── Removal ─────────────────────────────────────────────────────────────────

#[test]
fn test_remove_metadata_updates_index_and_audit_log() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_metadata_key_indexed(&t.buyer, &po(&t), &true);
    let a = create(&t, &client, "MI-RM");
    let b = create(&t, &client, "MI-RM2");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    client.set_shipment_metadata(&t.buyer, &b, &po(&t), &s(&t, "PO-1"));

    client.remove_shipment_metadata(&t.supplier, &a, &po(&t));
    assert!(has_event(&t, "shipment_metadata_removed"));

    assert!(client.get_shipment_metadata(&a, &po(&t)).is_none());
    assert_eq!(client.get_shipment_metadata_keys(&a).len(), 0);
    let found = client.find_shipments_by_metadata(&po(&t), &s(&t, "PO-1"));
    assert_eq!(found.len(), 1);
    assert_eq!(found.get(0).unwrap(), b);

    let shipment = client.get_shipment(&a);
    let last = shipment.audit_log.get(shipment.audit_log.len() - 1).unwrap();
    assert_eq!(last.action, Symbol::new(&t.env, "metadata_removed"));
    assert_eq!(last.caller, t.supplier);
}

#[test]
fn test_set_metadata_writes_audit_log() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let a = create(&t, &client, "MI-AUD");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    let shipment = client.get_shipment(&a);
    let last = shipment.audit_log.get(shipment.audit_log.len() - 1).unwrap();
    assert_eq!(last.action, Symbol::new(&t.env, "metadata_set"));
}

#[test]
#[should_panic(expected = "metadata key not found")]
fn test_remove_missing_key_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let a = create(&t, &client, "MI-RM-MISS");
    client.remove_shipment_metadata(&t.buyer, &a, &po(&t));
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_remove_by_non_party_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let a = create(&t, &client, "MI-RM-AUTH");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    let stranger = Address::generate(&t.env);
    client.remove_shipment_metadata(&stranger, &a, &po(&t));
}

#[test]
fn test_remove_requires_signature() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let a = create(&t, &client, "MI-RM-SIG");
    client.set_shipment_metadata(&t.buyer, &a, &po(&t), &s(&t, "PO-1"));
    t.env.set_auths(&[]);
    assert!(client
        .try_remove_shipment_metadata(&t.buyer, &a, &po(&t))
        .is_err());
}
