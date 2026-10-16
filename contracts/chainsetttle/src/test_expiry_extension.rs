#![cfg(test)]

extern crate std;

use super::*;
use crate::test_common::{default_options, setup, single_buyer_vec, TestSetup};
use soroban_sdk::testutils::{Events as _, Ledger as _};
use soroban_sdk::{vec, String, Symbol, TryFromVal, TryIntoVal};

/// Creates a shipment expiring `expires_in` ledgers from now and returns its id.
fn create_expiring(t: &TestSetup, client: &ChainSettleContractClient, id: &str, expires_in: u32) -> String {
    create_expiring_with_buyers(t, client, id, expires_in, false)
}

/// As `create_expiring`, optionally with a second co-buyer on the shipment.
fn create_expiring_with_buyers(
    t: &TestSetup,
    client: &ChainSettleContractClient,
    id: &str,
    expires_in: u32,
    two_buyers: bool,
) -> String {
    let shipment_id = String::from_str(&t.env, id);
    let mut opts = default_options(&t.env);
    opts.expires_at_ledger = Some(t.env.ledger().sequence() + expires_in);
    let buyers = if two_buyers {
        vec![&t.env, t.buyer.clone(), t.buyer2.clone()]
    } else {
        single_buyer_vec(&t.env, &t.buyer)
    };
    client.create_shipment(
        &shipment_id,
        &buyers,
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        &1_000_000,
        &crate::test_common::build_milestones(&t.env),
        &opts,
    );
    shipment_id
}

fn advance(t: &TestSetup, ledgers: u32) {
    t.env.ledger().with_mut(|l| l.sequence_number += ledgers);
}

fn expiry_of(client: &ChainSettleContractClient, shipment_id: &String) -> Option<u32> {
    client.get_shipment(shipment_id).expires_at_ledger
}

#[test]
fn test_max_extension_defaults_to_zero_and_is_settable() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    assert_eq!(client.get_max_expiry_extension_ledgers(), 0);
    client.set_max_expiry_extension_ledgers(&t.buyer, &500u32);
    assert_eq!(client.get_max_expiry_extension_ledgers(), 500);
}

#[test]
fn test_buyer_proposal_approved_by_supplier() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring(&t, &client, "b2s", 1_000);

    client.propose_expiry_extension(&t.buyer, &shipment_id, &2_000u32);
    client.approve_expiry_extension(&t.supplier, &shipment_id);

    assert_eq!(expiry_of(&client, &shipment_id), Some(2_000));
    assert_eq!(client.get_total_expiry_extended(&shipment_id), 1_000);
    assert!(client.get_pending_expiry_extension(&shipment_id).is_none());
}

#[test]
#[should_panic(expected = "cannot approve own expiry extension")]
fn test_proposer_cannot_self_approve() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring(&t, &client, "self", 1_000);
    client.propose_expiry_extension(&t.supplier, &shipment_id, &2_000u32);
    client.approve_expiry_extension(&t.supplier, &shipment_id);
}

#[test]
#[should_panic(expected = "expiry extension must be approved by the supplier")]
fn test_other_buyer_cannot_supply_supplier_approval() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring_with_buyers(&t, &client, "multi", 1_000, true);
    // Two buyers on the shipment: a second buyer proposing still needs the supplier.
    client.propose_expiry_extension(&t.buyer2, &shipment_id, &2_000u32);
    client.approve_expiry_extension(&t.buyer, &shipment_id);
}

#[test]
#[should_panic(expected = "no pending expiry extension")]
fn test_approval_without_proposal_fails() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring(&t, &client, "nop", 1_000);
    client.approve_expiry_extension(&t.supplier, &shipment_id);
}

#[test]
#[should_panic(expected = "new expiry must be later than current expiry")]
fn test_extension_to_earlier_expiry_rejected() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring(&t, &client, "back", 1_000);
    client.propose_expiry_extension(&t.supplier, &shipment_id, &900u32);
}

#[test]
#[should_panic(expected = "shipment has no expiry")]
fn test_shipment_without_expiry_cannot_be_extended() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = String::from_str(&t.env, "noexp");
    client.create_shipment(
        &shipment_id,
        &single_buyer_vec(&t.env, &t.buyer),
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        &1_000_000,
        &crate::test_common::build_milestones(&t.env),
        &default_options(&t.env),
    );
    client.propose_expiry_extension(&t.supplier, &shipment_id, &9_999u32);
}

#[test]
#[should_panic(expected = "shipment has already expired")]
fn test_expired_shipment_cannot_be_extended() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring(&t, &client, "exp", 100);
    advance(&t, 100);
    client.propose_expiry_extension(&t.supplier, &shipment_id, &5_000u32);
}

#[test]
#[should_panic(expected = "expiry extension exceeds maximum allowed")]
fn test_extension_beyond_admin_maximum_rejected() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_max_expiry_extension_ledgers(&t.buyer, &100u32);
    let shipment_id = create_expiring(&t, &client, "max", 1_000);
    client.propose_expiry_extension(&t.supplier, &shipment_id, &1_200u32);
}

#[test]
#[should_panic(expected = "pending expiry extension is stale")]
fn test_stale_proposal_is_rejected_after_resume() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring(&t, &client, "stale", 1_000);
    // A pause/resume cycle shifts the shipment's expiry, invalidating the
    // proposal made against the pre-pause base.
    client.propose_expiry_extension(&t.supplier, &shipment_id, &2_000u32);
    client.request_shipment_pause(&t.buyer, &shipment_id);
    client.approve_shipment_pause(&t.supplier, &shipment_id);
    advance(&t, 100);
    client.resume_shipment(&t.supplier, &shipment_id);
    client.resume_shipment(&t.buyer, &shipment_id);
    client.approve_expiry_extension(&t.buyer, &shipment_id);
}

#[test]
fn test_audit_log_records_proposal_and_approval() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let shipment_id = create_expiring(&t, &client, "audit", 1_000);
    let before = client.get_shipment(&shipment_id).audit_log.len();

    client.propose_expiry_extension(&t.supplier, &shipment_id, &2_000u32);
    client.approve_expiry_extension(&t.buyer, &shipment_id);

    let log = client.get_shipment(&shipment_id).audit_log;
    assert_eq!(log.len(), before + 2);
    assert_eq!(log.get(before).unwrap().action, Symbol::new(&t.env, "expiry_ext_proposed"));
    assert_eq!(log.get(before + 1).unwrap().action, Symbol::new(&t.env, "expiry_extended"));
}

