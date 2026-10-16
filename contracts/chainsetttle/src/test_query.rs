#![cfg(test)]

extern crate std;

use super::*;
use crate::test_common::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, String,
};

// ============================================================
// QUERY & READ-ONLY TESTS: COMPLETION PERCENTAGE
// ============================================================

// ============================================================
// COMPLETION PERCENTAGE: DOCUMENTED WEIGHT DERIVATION
// ------------------------------------------------------------
// These tests pin the behaviour documented in
// `docs/completion-percentage.md` and in the rustdoc on
// `ChainSettleContract::get_completion_percentage`, i.e. that the
// percentage is the settled share of the milestone payment weights:
//
//   settled = released_amount + total_advanced_amount
//   pct     = clamp(settled * 100 / total_amount, 0, 100)
//
// with each accumulation being one milestone weight
// (`total_amount * payment_percent / 100`, or `* splits_bps / 10_000`).
// ============================================================

/// Submits proof as the supplier and confirms the milestone as the buyer.
fn settle_milestone(
    client: &ChainSettleContractClient,
    t: &TestSetup,
    shipment_id: &String,
    milestone_index: u32,
) {
    client.submit_proof(
        &t.supplier,
        shipment_id,
        &milestone_index,
        &String::from_str(&t.env, "ipfs://proof"),
        &Symbol::new(&t.env, "ipfs"),
    );
    client.confirm_milestone(&t.buyer, shipment_id, &milestone_index);
}

/// The reading tracks escrow *settlement*, not supplier earnings: the contested share
/// of a partial dispute counts even when it is refunded to the buyer (§3.5 of the doc).
#[test]
fn test_get_completion_percentage_counts_buyer_refund_settlement() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);

    let shipment_id = String::from_str(&t.env, "SHIP-COMPL-REFUND");
    let total_amount: i128 = 1_000_000_000;

    create_standard_shipment(
        &client,
        &t.env,
        &shipment_id,
        &t.buyer,
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        total_amount,
    );

    client.submit_proof(
        &t.supplier,
        &shipment_id,
        &0,
        &String::from_str(&t.env, "ipfs://proof"),
        &Symbol::new(&t.env, "ipfs"),
    );

    // 40 % of milestone 0 contested → uncontested 60 % of 250_000_000 = 150_000_000 (15 %).
    client.raise_partial_dispute(&t.buyer, &shipment_id, &0, &40);
    assert_eq!(client.get_completion_percentage(&shipment_id), 15);

    // Arbiter sides with the buyer: the contested 100_000_000 is refunded, yet it has
    // still left escrow, so the full 25 % weight now reads as settled.
    client.resolve_dispute(&t.arbiter, &shipment_id, &0, &false, &None);

    assert_eq!(
        client.get_shipment(&shipment_id).released_amount,
        250_000_000
    );
    assert_eq!(client.get_completion_percentage(&shipment_id), 25);
}

// ============================================================
// ADDITIONAL QUERY TESTS: EDGE CASES & COMPLEX SCENARIOS
// ============================================================

/// Test completion percentage with multiple advances across different milestones.
#[test]
fn test_get_completion_percentage_multiple_advances() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);

    let shipment_id = String::from_str(&t.env, "SHIP-COMPL-MULTI-ADV");
    let total_amount: i128 = 1_000_000_000;

    create_standard_shipment(
        &client,
        &t.env,
        &shipment_id,
        &t.buyer,
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        total_amount,
    );

    // 10% advance on milestone 0 (weight 25%) = 25_000_000 = 2.5%
    client.request_advance(&t.supplier, &shipment_id, &0, &10);
    client.approve_advance(&t.buyer, &shipment_id, &0);
    assert_eq!(client.get_completion_percentage(&shipment_id), 2);

    // 20% advance on milestone 1 (weight 50%) = 100_000_000 = 10%
    client.request_advance(&t.supplier, &shipment_id, &1, &20);
    client.approve_advance(&t.buyer, &shipment_id, &1);
    // Total: 2.5% + 10% = 12.5% → floored to 12%
    assert_eq!(client.get_completion_percentage(&shipment_id), 12);

    // Settle milestone 0 - advance is absorbed into full weight
    settle_milestone(&client, &t, &shipment_id, 0);
    // 25% from milestone 0 + 10% from milestone 1 advance = 35%
    assert_eq!(client.get_completion_percentage(&shipment_id), 35);
}

