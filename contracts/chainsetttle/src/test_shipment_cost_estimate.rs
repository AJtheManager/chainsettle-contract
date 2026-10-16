// Tests for `estimate_shipment_costs` query.
//
// Verifies `estimate_shipment_costs` accurately simulates shipment-level
// fees, payouts, dispute bond, and supplier collateral before creation,
// and respects active waivers, fee holidays, tiers, and custom splits.

#![cfg(test)]

extern crate std;

use super::*;
use crate::test_common::{build_milestones, default_options, setup, single_buyer_vec};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    vec, Address, Env, String, Vec,
};

#[test]
fn test_estimate_reflects_fee_holiday() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);

    // 200 bps platform fee
    client.set_fee_config(&t.buyer, &200u32, &t.treasury);

    let milestones = build_milestones(&t.env);
    let mut opts = default_options(&t.env);
    opts.logistics_fee_bps = 100;

    // Before holiday: 200 bps fee
    let est_before = client.estimate_shipment_costs(
        &t.buyer,
        &t.token_id,
        &1_000_000i128,
        &milestones,
        &opts,
    );
    assert_eq!(est_before.applied_fee_bps, 200);
    assert_eq!(est_before.total_platform_fee, 20_000);
    assert_eq!(est_before.total_logistics_fee, 10_000);
    assert_eq!(est_before.total_net_amount, 970_000);

    // Schedule fee holiday covering current ledger (sequence 0..100)
    let cur_ledger = t.env.ledger().sequence();
    client.schedule_fee_holiday(&t.buyer, &cur_ledger, &(cur_ledger + 50));

    assert!(client.is_fee_holiday_active());

    // During holiday: platform fee waived (0 bps)
    let est_holiday = client.estimate_shipment_costs(
        &t.buyer,
        &t.token_id,
        &1_000_000i128,
        &milestones,
        &opts,
    );
    assert_eq!(est_holiday.applied_fee_bps, 0);
    assert_eq!(est_holiday.total_platform_fee, 0);
    assert_eq!(est_holiday.total_logistics_fee, 10_000);
    assert_eq!(est_holiday.total_net_amount, 990_000);

    // Cancel holiday -> platform fee restored
    client.cancel_fee_holiday(&t.buyer);
    assert!(!client.is_fee_holiday_active());

    let est_after = client.estimate_shipment_costs(
        &t.buyer,
        &t.token_id,
        &1_000_000i128,
        &milestones,
        &opts,
    );
    assert_eq!(est_after.applied_fee_bps, 200);
    assert_eq!(est_after.total_platform_fee, 20_000);
}

#[test]
#[should_panic(expected = "amount must be greater than zero")]
fn test_estimate_zero_amount_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let milestones = build_milestones(&t.env);
    let opts = default_options(&t.env);

    client.estimate_shipment_costs(&t.buyer, &t.token_id, &0i128, &milestones, &opts);
}

#[test]
#[should_panic(expected = "milestones cannot be empty")]
fn test_estimate_empty_milestones_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let empty_milestones: Vec<Milestone> = Vec::new(&t.env);
    let opts = default_options(&t.env);

    client.estimate_shipment_costs(
        &t.buyer,
        &t.token_id,
        &1_000_000i128,
        &empty_milestones,
        &opts,
    );
}

#[test]
#[should_panic(expected = "milestone percentages must sum to 100")]
fn test_estimate_invalid_percentage_sum_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let mut milestones = build_milestones(&t.env);
    let mut m0 = milestones.get(0).unwrap();
    m0.payment_percent = 90; // sum becomes 90 + 50 + 25 = 165
    milestones.set(0, m0);
    let opts = default_options(&t.env);

    client.estimate_shipment_costs(&t.buyer, &t.token_id, &1_000_000i128, &milestones, &opts);
}

#[test]
#[should_panic(expected = "InvalidSplitConfiguration")]
fn test_estimate_invalid_splits_sum_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let milestones = build_milestones(&t.env);
    let mut opts = default_options(&t.env);
    opts.milestone_splits = vec![&t.env, 3000u32, 3000u32, 3000u32]; // sum = 9000 != 10000

    client.estimate_shipment_costs(&t.buyer, &t.token_id, &1_000_000i128, &milestones, &opts);
}

#[test]
#[should_panic(expected = "dispute_bond_bps exceeds maximum allowed")]
fn test_estimate_dispute_bond_bps_cap_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let milestones = build_milestones(&t.env);
    let mut opts = default_options(&t.env);
    opts.dispute_bond_bps = 5001; // exceeds default max 5000 bps

    client.estimate_shipment_costs(&t.buyer, &t.token_id, &1_000_000i128, &milestones, &opts);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_estimate_blacklisted_buyer_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);

    client.blacklist_address(&t.buyer, &t.buyer, &BytesN::from_array(&t.env, &[0u8; 32]));

    let milestones = build_milestones(&t.env);
    let opts = default_options(&t.env);

    client.estimate_shipment_costs(&t.buyer, &t.token_id, &1_000_000i128, &milestones, &opts);
}

#[test]
#[should_panic(expected = "token is not in the approved whitelist")]
fn test_estimate_unapproved_token_panics() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);

    let other_token = Address::generate(&t.env);
    client.add_allowed_token(&other_token);

    let milestones = build_milestones(&t.env);
    let opts = default_options(&t.env);

    // t.token_id is not other_token
    client.estimate_shipment_costs(&t.buyer, &t.token_id, &1_000_000i128, &milestones, &opts);
}
