#![cfg(test)]

extern crate std;

use super::*;
use crate::test_common::{build_milestones, default_options, setup, single_buyer_vec};
use soroban_sdk::{testutils::Ledger as _, token, vec, String, Symbol};

fn sid(env: &Env, id: &str) -> String {
    String::from_str(env, id)
}

fn confirm_all_milestones(
    client: &ChainSettleContractClient,
    t: &crate::test_common::TestSetup,
    shipment_id: &String,
) {
    for i in 0..3 {
        client.submit_proof(
            &t.supplier,
            shipment_id,
            &i,
            &String::from_str(&t.env, "proof_hash"),
            &Symbol::new(&t.env, "ipfs"),
        );
        client.confirm_milestone(&t.buyer, shipment_id, &i);
    }
}

// ============================================================
// LOCKING COLLATERAL AT CREATION
// ============================================================

#[test]
fn test_collateral_larger_than_shipment_total() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let mint_client = token::StellarAssetClient::new(&t.env, &t.token_id);

    // Collateral bigger than the shipment's total escrow amount.
    mint_client.mint(&t.supplier, &5_000_000_000);

    let shipment_id = sid(&t.env, "collat-large");
    let mut opts = default_options(&t.env);
    opts.supplier_collateral = 2_000_000_000; // larger than total_amount below

    client.create_shipment(
        &shipment_id,
        &single_buyer_vec(&t.env, &t.buyer),
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        &1_000_000_000i128,
        &build_milestones(&t.env),
        &opts,
    );

    let shipment = client.get_shipment(&shipment_id);
    assert_eq!(shipment.status, ShipmentStatus::Active);
}

#[test]
#[should_panic]
fn test_collateral_creation_panics_on_insufficient_supplier_balance() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    // Supplier has zero balance and no mint; requiring collateral must fail the transfer.

    let shipment_id = sid(&t.env, "collat-insufficient");
    let mut opts = default_options(&t.env);
    opts.supplier_collateral = 50_000_000;

    client.create_shipment(
        &shipment_id,
        &single_buyer_vec(&t.env, &t.buyer),
        &t.supplier,
        &t.logistics,
        &t.arbiter,
        &t.token_id,
        &1_000_000_000i128,
        &build_milestones(&t.env),
        &opts,
    );
}

// ============================================================
// RETURN OF COLLATERAL ON COMPLETION
// ============================================================

// ============================================================
// FORFEITURE OF COLLATERAL ON BUYER CANCELLATION
// ============================================================

// ============================================================
// FORFEITURE OF COLLATERAL ON DEADLINE-BASED REFUND (#164)
// ============================================================

// ============================================================
// STATE ISOLATION ACROSS SHIPMENTS
// ============================================================

