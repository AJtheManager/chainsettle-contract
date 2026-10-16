// #663 batch arbiter onboarding, #664 percentage split resolution,
// #665 automatic panel escalation, #666 batch proof submission.

#![cfg(test)]

extern crate std;

use super::*;
use crate::test_common::{build_milestones, default_options, setup, single_buyer_vec, TestSetup};
use soroban_sdk::testutils::{Address as _, Events, Ledger as _};
use soroban_sdk::{token, vec, Address, BytesN, Env, String, Symbol, TryFromVal, Vec};

const TOTAL: i128 = 1_000_000;
/// Milestone 0 of `build_milestones` is 25% of `TOTAL`.
const M0: i128 = 250_000;

fn sid(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

fn ipfs(env: &Env) -> Symbol {
    Symbol::new(env, "ipfs")
}

fn proof(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

fn create(
    t: &TestSetup,
    client: &ChainSettleContractClient,
    id: &str,
    options: &ShipmentOptions,
) -> String {
    let shipment_id = sid(&t.env, id);
    client.create_shipment(
        &shipment_id,
        &single_buyer_vec(&t.env, &t.buyer),
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        &TOTAL,
        &build_milestones(&t.env),
        options,
    );
    shipment_id
}

fn balance(t: &TestSetup, who: &Address) -> i128 {
    token::Client::new(&t.env, &t.token_id).balance(who)
}

fn count_events(t: &TestSetup, name: &str) -> u32 {
    let target = Symbol::new(&t.env, name);
    let events = t.env.events().all();
    let mut n = 0u32;
    for i in 0..events.len() {
        let (_id, topics, _data) = events.get(i).unwrap();
        if let Some(first) = topics.get(0) {
            if let Ok(topic) = Symbol::try_from_val(&t.env, &first) {
                if topic == target {
                    n += 1;
                }
            }
        }
    }
    n
}

fn has_audit_action(client: &ChainSettleContractClient, id: &String, action: &Symbol) -> bool {
    let log = client.get_shipment(id).audit_log;
    for i in 0..log.len() {
        if log.get(i).unwrap().action == *action {
            return true;
        }
    }
    false
}

fn milestone_status(client: &ChainSettleContractClient, id: &String, idx: u32) -> MilestoneStatus {
    client.get_shipment(id).milestones.get(idx).unwrap().status
}

fn submit(t: &TestSetup, client: &ChainSettleContractClient, id: &String, idx: u32) {
    client.submit_proof(&t.supplier, id, &idx, &proof(&t.env, "QmProof"), &ipfs(&t.env));
}

/// Shipment with milestone 0 under a full dispute raised by the buyer.
fn disputed(t: &TestSetup, client: &ChainSettleContractClient, id: &str) -> String {
    let shipment_id = create(t, client, id, &default_options(&t.env));
    submit(t, client, &shipment_id, 0);
    client.raise_dispute(&t.buyer, &shipment_id, &0u32);
    shipment_id
}

fn generated(env: &Env, n: u32) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..n {
        v.push_back(Address::generate(env));
    }
    v
}

// ============================================================
// #666 batch_submit_proofs
// ============================================================

#[test]
fn test_batch_submit_proofs_happy_path() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-1", &default_options(&t.env));

    let entries = vec![
        &t.env,
        (0u32, proof(&t.env, "Qm0"), ipfs(&t.env)),
        (2u32, proof(&t.env, "Qm2"), ipfs(&t.env)),
    ];
    client.batch_submit_proofs(&t.supplier, &id, &entries);
    assert_eq!(count_events(&t, "batch_proofs_submitted"), 1);
    assert_eq!(count_events(&t, "proof_submitted"), 2);

    let shipment = client.get_shipment(&id);
    let m0 = shipment.milestones.get(0).unwrap();
    let m2 = shipment.milestones.get(2).unwrap();
    assert_eq!(m0.status, MilestoneStatus::ProofSubmitted);
    assert_eq!(m0.proof_hash, proof(&t.env, "Qm0"));
    assert_eq!(m2.status, MilestoneStatus::ProofSubmitted);
    assert_eq!(m2.proof_hash, proof(&t.env, "Qm2"));
    assert_eq!(shipment.milestones.get(1).unwrap().status, MilestoneStatus::Pending);
    assert!(has_audit_action(
        &client,
        &id,
        &Symbol::new(&t.env, "batch_proofs_submitted")
    ));
}

#[test]
fn test_batch_submit_proofs_invalid_entry_reverts_batch() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-2", &default_options(&t.env));

    let entries = vec![
        &t.env,
        (0u32, proof(&t.env, "Qm0"), ipfs(&t.env)),
        (9u32, proof(&t.env, "Qm9"), ipfs(&t.env)),
    ];
    assert!(client
        .try_batch_submit_proofs(&t.supplier, &id, &entries)
        .is_err());
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Pending);
}

#[test]
fn test_batch_submit_proofs_non_pending_entry_reverts_batch() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-3", &default_options(&t.env));
    submit(&t, &client, &id, 1);

    let entries = vec![
        &t.env,
        (0u32, proof(&t.env, "Qm0"), ipfs(&t.env)),
        (1u32, proof(&t.env, "Qm1"), ipfs(&t.env)),
    ];
    assert!(client
        .try_batch_submit_proofs(&t.supplier, &id, &entries)
        .is_err());
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Pending);
}

#[test]
fn test_batch_submit_proofs_sequential_mode_cannot_skip_ahead() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let mut options = default_options(&t.env);
    options.milestone_mode = MilestoneMode::Sequential;
    let id = create(&t, &client, "BP-4", &options);

    let entries = vec![
        &t.env,
        (0u32, proof(&t.env, "Qm0"), ipfs(&t.env)),
        (1u32, proof(&t.env, "Qm1"), ipfs(&t.env)),
    ];
    assert!(client
        .try_batch_submit_proofs(&t.supplier, &id, &entries)
        .is_err());
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Pending);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_batch_submit_proofs_rejects_non_submitter() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-5", &default_options(&t.env));
    let entries = vec![&t.env, (0u32, proof(&t.env, "Qm0"), ipfs(&t.env))];
    client.batch_submit_proofs(&t.buyer, &id, &entries);
}

#[test]
fn test_batch_submit_proofs_requires_auth() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-6", &default_options(&t.env));
    let entries = vec![&t.env, (0u32, proof(&t.env, "Qm0"), ipfs(&t.env))];

    t.env.set_auths(&[]);
    assert!(client
        .try_batch_submit_proofs(&t.supplier, &id, &entries)
        .is_err());
}

#[test]
#[should_panic(expected = "duplicate milestone index in batch")]
fn test_batch_submit_proofs_rejects_duplicate_index() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-7", &default_options(&t.env));
    let entries = vec![
        &t.env,
        (0u32, proof(&t.env, "Qm0"), ipfs(&t.env)),
        (0u32, proof(&t.env, "Qm0b"), ipfs(&t.env)),
    ];
    client.batch_submit_proofs(&t.supplier, &id, &entries);
}

#[test]
#[should_panic(expected = "batch is empty")]
fn test_batch_submit_proofs_rejects_empty_batch() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-8", &default_options(&t.env));
    let entries: Vec<(u32, String, Symbol)> = Vec::new(&t.env);
    client.batch_submit_proofs(&t.supplier, &id, &entries);
}

#[test]
#[should_panic(expected = "batch too large")]
fn test_batch_submit_proofs_rejects_oversized_batch() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "BP-9", &default_options(&t.env));
    let mut entries: Vec<(u32, String, Symbol)> = Vec::new(&t.env);
    for i in 0..(constants::MAX_BATCH_PROOF_SUBMISSIONS + 1) {
        entries.push_back((i, proof(&t.env, "Qm"), ipfs(&t.env)));
    }
    client.batch_submit_proofs(&t.supplier, &id, &entries);
}

// ============================================================
// #663 batch_add_arbiters_to_pool
// ============================================================

#[test]
fn test_batch_add_arbiters_happy_path() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let arbiters = generated(&t.env, 3);

    client.batch_add_arbiters_to_pool(&t.buyer, &arbiters);
    assert_eq!(count_events(&t, "arbiter_pool_updated"), 3);
    assert_eq!(count_events(&t, "arbiters_batch_added"), 1);

    let pool = client.get_arbiter_pool();
    assert_eq!(pool.len(), 3);
    for i in 0..arbiters.len() {
        assert!(pool.contains(&arbiters.get(i).unwrap()));
    }
}

#[test]
fn test_batch_add_arbiters_requires_admin_role() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let stranger = Address::generate(&t.env);
    assert!(client
        .try_batch_add_arbiters_to_pool(&stranger, &generated(&t.env, 2))
        .is_err());
    assert_eq!(client.get_arbiter_pool().len(), 0);
}

#[test]
fn test_batch_add_arbiters_requires_auth() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let arbiters = generated(&t.env, 2);
    t.env.set_auths(&[]);
    assert!(client
        .try_batch_add_arbiters_to_pool(&t.buyer, &arbiters)
        .is_err());
}

#[test]
fn test_batch_add_arbiters_duplicate_in_batch_reverts() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let a = Address::generate(&t.env);
    let b = Address::generate(&t.env);
    let arbiters = vec![&t.env, a.clone(), b, a];
    assert!(client
        .try_batch_add_arbiters_to_pool(&t.buyer, &arbiters)
        .is_err());
    assert_eq!(client.get_arbiter_pool().len(), 0);
}

#[test]
fn test_batch_add_arbiters_already_in_pool_reverts() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.add_arbiter_to_pool(&t.buyer, &t.arbiter);
    let arbiters = vec![&t.env, Address::generate(&t.env), t.arbiter.clone()];
    assert!(client
        .try_batch_add_arbiters_to_pool(&t.buyer, &arbiters)
        .is_err());
    assert_eq!(client.get_arbiter_pool().len(), 1);
}

#[test]
fn test_batch_add_arbiters_blacklisted_reverts() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let bad = Address::generate(&t.env);
    client.blacklist_address(&t.buyer, &bad, &BytesN::from_array(&t.env, &[7u8; 32]));
    let arbiters = vec![&t.env, Address::generate(&t.env), bad];
    assert!(client
        .try_batch_add_arbiters_to_pool(&t.buyer, &arbiters)
        .is_err());
    assert_eq!(client.get_arbiter_pool().len(), 0);
}

#[test]
#[should_panic(expected = "batch is empty")]
fn test_batch_add_arbiters_rejects_empty_batch() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.batch_add_arbiters_to_pool(&t.buyer, &Vec::new(&t.env));
}

#[test]
#[should_panic(expected = "batch too large")]
fn test_batch_add_arbiters_rejects_oversized_batch() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let arbiters = generated(&t.env, constants::MAX_BATCH_ADD_ARBITERS + 1);
    client.batch_add_arbiters_to_pool(&t.buyer, &arbiters);
}

// ============================================================
// #664 resolve_dispute_split
// ============================================================

#[test]
fn test_split_full_dispute_pays_exact_shares() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = disputed(&t, &client, "SP-1");
    let buyer_before = balance(&t, &t.buyer);

    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &3_000u32);
    assert_eq!(count_events(&t, "dispute_split_resolved"), 1);
    assert_eq!(count_events(&t, "dispute_split_settled"), 1);

    let supplier_gain = balance(&t, &t.supplier);
    let buyer_gain = balance(&t, &t.buyer) - buyer_before;
    assert_eq!(supplier_gain, 75_000);
    assert_eq!(buyer_gain, 175_000);
    assert_eq!(supplier_gain + buyer_gain, M0);

    let shipment = client.get_shipment(&id);
    assert_eq!(shipment.milestones.get(0).unwrap().status, MilestoneStatus::Resolved);
    assert_eq!(shipment.released_amount, M0);
    assert_eq!(shipment.open_dispute_count, 0);
    assert_eq!(client.get_dispute_split(&id, &0u32), Some(3_000u32));
    assert!(has_audit_action(
        &client,
        &id,
        &Symbol::new(&t.env, "dispute_split_resolved")
    ));
}

#[test]
fn test_split_shares_sum_exactly_with_rounding() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    // 25% of 1_000_004 = 250_001, which no 33.33% split divides evenly.
    let id = sid(&t.env, "SP-2");
    client.create_shipment(
        &id,
        &single_buyer_vec(&t.env, &t.buyer),
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        &1_000_004i128,
        &build_milestones(&t.env),
        &default_options(&t.env),
    );
    submit(&t, &client, &id, 0);
    client.raise_dispute(&t.buyer, &id, &0u32);
    let buyer_before = balance(&t, &t.buyer);

    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &3_333u32);

    let contested: i128 = 250_001;
    let supplier_gain = balance(&t, &t.supplier);
    let buyer_gain = balance(&t, &t.buyer) - buyer_before;
    assert_eq!(supplier_gain, (contested * 3_333) / 10_000);
    assert_eq!(buyer_gain, contested - (contested * 3_333) / 10_000);
    assert_eq!(supplier_gain + buyer_gain, contested);
    assert_eq!(client.get_shipment(&id).released_amount, contested);
}

#[test]
fn test_split_partial_dispute_splits_contested_share_only() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "SP-3", &default_options(&t.env));
    submit(&t, &client, &id, 0);
    // 40% contested: 150_000 released now, 100_000 held for the arbiter.
    client.raise_partial_dispute(&t.buyer, &id, &0u32, &40u32);
    assert_eq!(balance(&t, &t.supplier), 150_000);
    let buyer_before = balance(&t, &t.buyer);

    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &2_500u32);

    assert_eq!(balance(&t, &t.supplier), 150_000 + 25_000);
    assert_eq!(balance(&t, &t.buyer) - buyer_before, 75_000);
    let shipment = client.get_shipment(&id);
    assert_eq!(shipment.milestones.get(0).unwrap().status, MilestoneStatus::Resolved);
    assert_eq!(shipment.released_amount, M0);
}

#[test]
fn test_split_bounds_are_accepted() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = disputed(&t, &client, "SP-4");
    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &10_000u32);
    assert_eq!(balance(&t, &t.supplier), M0);

    let id2 = disputed(&t, &client, "SP-5");
    let buyer_before = balance(&t, &t.buyer);
    client.resolve_dispute_split(&t.arbiter, &id2, &0u32, &0u32);
    assert_eq!(balance(&t, &t.buyer) - buyer_before, M0);
    assert_eq!(milestone_status(&client, &id2, 0), MilestoneStatus::Resolved);
}

#[test]
#[should_panic(expected = "supplier_bps must be at most 10000")]
fn test_split_rejects_bps_above_10000() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = disputed(&t, &client, "SP-6");
    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &10_001u32);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_split_rejects_non_arbiter() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = disputed(&t, &client, "SP-7");
    client.resolve_dispute_split(&t.buyer, &id, &0u32, &5_000u32);
}

#[test]
fn test_split_requires_auth() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = disputed(&t, &client, "SP-8");
    t.env.set_auths(&[]);
    assert!(client
        .try_resolve_dispute_split(&t.arbiter, &id, &0u32, &5_000u32)
        .is_err());
}

#[test]
#[should_panic(expected = "milestone is not in disputed status")]
fn test_split_rejects_undisputed_milestone() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let id = create(&t, &client, "SP-9", &default_options(&t.env));
    submit(&t, &client, &id, 0);
    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &5_000u32);
}

#[test]
fn test_split_parks_while_appealable_then_finalizes() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    client.set_appeal_window_ledgers(&t.buyer, &50u32);
    let id = disputed(&t, &client, "SP-10");
    let buyer_before = balance(&t, &t.buyer);

    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &6_000u32);
    assert_eq!(
        milestone_status(&client, &id, 0),
        MilestoneStatus::ResolvedPendingFinality
    );
    assert_eq!(balance(&t, &t.supplier), 0);
    assert!(client
        .try_finalize_dispute_resolution(&id, &0u32)
        .is_err());

    t.env.ledger().with_mut(|l| l.sequence_number += 51);
    client.finalize_dispute_resolution(&id, &0u32);
    assert_eq!(balance(&t, &t.supplier), 150_000);
    assert_eq!(balance(&t, &t.buyer) - buyer_before, 100_000);
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Resolved);
    // The appeal window is over once the split has paid out.
    assert!(client.try_appeal_dispute(&t.buyer, &id, &0u32).is_err());
}

#[test]
fn test_split_appeal_reopens_without_double_payment() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let arbiter2 = Address::generate(&t.env);
    client.add_arbiter_to_pool(&t.buyer, &t.arbiter);
    client.add_arbiter_to_pool(&t.buyer, &arbiter2);
    client.set_appeal_window_ledgers(&t.buyer, &50u32);
    let id = disputed(&t, &client, "SP-11");
    let buyer_before = balance(&t, &t.buyer);

    // Buyer-favoured split, parked while appealable.
    client.resolve_dispute_split(&t.arbiter, &id, &0u32, &3_000u32);
    client.appeal_dispute(&t.supplier, &id, &0u32);
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Disputed);
    let appeal_arbiter = client.get_shipment(&id).arbiter;
    assert_eq!(appeal_arbiter, arbiter2);

    // Supplier-favoured appeal split: final, paid immediately, once.
    client.resolve_dispute_split(&appeal_arbiter, &id, &0u32, &8_000u32);
    assert_eq!(balance(&t, &t.supplier), 200_000);
    assert_eq!(balance(&t, &t.buyer) - buyer_before, 50_000);
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Resolved);
    // The majority side flipped, so the original arbiter was overturned.
    assert_eq!(client.get_arbiter_stats(&t.arbiter).overturned_count, 1);
}

// ============================================================
// #665 automatic panel escalation
// ============================================================

fn pool_of(t: &TestSetup, client: &ChainSettleContractClient, n: u32) -> Vec<Address> {
    let arbiters = generated(&t.env, n);
    client.batch_add_arbiters_to_pool(&t.buyer, &arbiters);
    arbiters
}

#[test]
fn test_panel_escalation_config_roundtrip() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    assert_eq!(client.get_panel_escalation(&t.token_id), None);
    client.set_panel_escalation(&t.buyer, &t.token_id, &100_000i128, &3u32);
    assert_eq!(count_events(&t, "panel_escalation_set"), 1);
    assert_eq!(
        client.get_panel_escalation(&t.token_id),
        Some((100_000i128, 3u32))
    );
    client.clear_panel_escalation(&t.buyer, &t.token_id);
    assert_eq!(client.get_panel_escalation(&t.token_id), None);
}

#[test]
fn test_panel_escalation_requires_admin() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let stranger = Address::generate(&t.env);
    assert!(client
        .try_set_panel_escalation(&stranger, &t.token_id, &100_000i128, &3u32)
        .is_err());
    assert!(client
        .try_clear_panel_escalation(&stranger, &t.token_id)
        .is_err());
    t.env.set_auths(&[]);
    assert!(client
        .try_set_panel_escalation(&t.buyer, &t.token_id, &100_000i128, &3u32)
        .is_err());
}

#[test]
fn test_panel_escalation_validates_inputs() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    assert!(client
        .try_set_panel_escalation(&t.buyer, &t.token_id, &0i128, &3u32)
        .is_err());
    assert!(client
        .try_set_panel_escalation(&t.buyer, &t.token_id, &100_000i128, &2u32)
        .is_err());
}

#[test]
fn test_dispute_above_threshold_escalates_to_panel() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pool = pool_of(&t, &client, 4);
    client.set_panel_escalation(&t.buyer, &t.token_id, &100_000i128, &3u32);

    let id = disputed(&t, &client, "PE-1");
    assert_eq!(count_events(&t, "dispute_panel_escalated"), 1);
    assert!(client.is_dispute_panel_escalated(&id, &0u32));
    let panel = client.get_arbiter_panel(&id);
    assert_eq!(panel.len(), 3);
    for i in 0..panel.len() {
        assert!(pool.contains(&panel.get(i).unwrap()));
    }
    assert!(has_audit_action(
        &client,
        &id,
        &Symbol::new(&t.env, "dispute_panel_escalated")
    ));

    // The single arbiter can no longer decide it, in either form.
    assert!(client
        .try_resolve_dispute(&t.arbiter, &id, &0u32, &true, &None)
        .is_err());
    assert!(client
        .try_resolve_dispute_split(&t.arbiter, &id, &0u32, &5_000u32)
        .is_err());

    // A panel majority resolves it.
    client.cast_dispute_vote(&panel.get(0).unwrap(), &id, &0u32, &true);
    client.cast_dispute_vote(&panel.get(1).unwrap(), &id, &0u32, &true);
    assert_ne!(milestone_status(&client, &id, 0), MilestoneStatus::Disputed);
    assert!(!client.is_dispute_panel_escalated(&id, &0u32));
}

#[test]
fn test_dispute_at_or_below_threshold_is_unchanged() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    pool_of(&t, &client, 4);
    // Threshold equal to the milestone value: not strictly above it.
    client.set_panel_escalation(&t.buyer, &t.token_id, &M0, &3u32);

    let id = disputed(&t, &client, "PE-2");
    assert!(!client.is_dispute_panel_escalated(&id, &0u32));
    assert_eq!(client.get_arbiter_panel(&id).len(), 0);
    client.resolve_dispute(&t.arbiter, &id, &0u32, &true, &None);
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Resolved);
}

#[test]
fn test_escalation_falls_back_to_single_arbiter_when_pool_too_small() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    pool_of(&t, &client, 2);
    client.set_panel_escalation(&t.buyer, &t.token_id, &100_000i128, &3u32);

    let id = disputed(&t, &client, "PE-3");
    assert_eq!(count_events(&t, "panel_escalation_fallback"), 1);
    assert!(!client.is_dispute_panel_escalated(&id, &0u32));
    client.resolve_dispute(&t.arbiter, &id, &0u32, &true, &None);
    assert_eq!(milestone_status(&client, &id, 0), MilestoneStatus::Resolved);
}

#[test]
fn test_partial_dispute_escalates_on_contested_share() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    pool_of(&t, &client, 4);
    // 30% of 250_000 = 75_000 contested.
    client.set_panel_escalation(&t.buyer, &t.token_id, &100_000i128, &3u32);
    let id = create(&t, &client, "PE-4", &default_options(&t.env));
    submit(&t, &client, &id, 0);
    client.raise_partial_dispute(&t.buyer, &id, &0u32, &30u32);
    assert!(!client.is_dispute_panel_escalated(&id, &0u32));

    client.set_panel_escalation(&t.buyer, &t.token_id, &50_000i128, &3u32);
    let id2 = create(&t, &client, "PE-5", &default_options(&t.env));
    submit(&t, &client, &id2, 0);
    client.raise_partial_dispute(&t.buyer, &id2, &0u32, &30u32);
    assert!(client.is_dispute_panel_escalated(&id2, &0u32));
}

#[test]
fn test_escalation_is_per_token() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    pool_of(&t, &client, 4);
    let other_token = Address::generate(&t.env);
    client.set_panel_escalation(&t.buyer, &other_token, &1i128, &3u32);

    let id = disputed(&t, &client, "PE-6");
    assert!(!client.is_dispute_panel_escalated(&id, &0u32));
}
