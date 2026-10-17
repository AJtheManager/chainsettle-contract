#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, vec, Address, Env, String,
};

// ============================================================
// TEST SETUP & SHARED FIXTURES
// ============================================================

pub struct TestSetup {
    pub env: Env,
    pub contract_id: Address,
    pub token_id: Address,
    pub buyer: Address,
    pub buyer2: Address,
    pub supplier: Address,
    pub logistics: Address,
    pub arbiter: Address,
    pub treasury: Address,
}

pub fn setup() -> TestSetup {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ChainSettleContract, ());

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_client = token::StellarAssetClient::new(&env, &token_id);

    let buyer = Address::generate(&env);
    let buyer2 = Address::generate(&env);
    let supplier = Address::generate(&env);
    let logistics = Address::generate(&env);
    let arbiter = Address::generate(&env);
    let treasury = Address::generate(&env);

    token_client.mint(&buyer, &10_000_000_000);
    token_client.mint(&buyer2, &10_000_000_000);

    let client = ChainSettleContractClient::new(&env, &contract_id);
    client.init(&buyer);

    TestSetup {
        env,
        contract_id,
        token_id,
        buyer,
        buyer2,
        supplier,
        logistics,
        arbiter,
        treasury,
    }
}

pub fn build_milestones(env: &Env) -> soroban_sdk::Vec<Milestone> {
    vec![
        env,
        Milestone {
            name: String::from_str(env, "Goods Dispatched"),
            payment_percent: 25,
            proof_hash: String::from_str(env, ""),
            status: MilestoneStatus::Pending,
            release_after_ledger: 0,
            proof_submitted_ledger: None,
            dispute_opened_ledger: None,
            deadline_ledger: 0,
            penalty_bps_per_ledger: 0,
        },
        Milestone {
            name: String::from_str(env, "In Transit"),
            payment_percent: 50,
            proof_hash: String::from_str(env, ""),
            status: MilestoneStatus::Pending,
            release_after_ledger: 0,
            proof_submitted_ledger: None,
            dispute_opened_ledger: None,
            deadline_ledger: 0,
            penalty_bps_per_ledger: 0,
        },
        Milestone {
            name: String::from_str(env, "Delivered"),
            payment_percent: 25,
            proof_hash: String::from_str(env, ""),
            status: MilestoneStatus::Pending,
            release_after_ledger: 0,
            proof_submitted_ledger: None,
            dispute_opened_ledger: None,
            deadline_ledger: 0,
            penalty_bps_per_ledger: 0,
        },
    ]
}

pub fn single_buyer_vec(env: &Env, buyer: &Address) -> soroban_sdk::Vec<Address> {
    vec![env, buyer.clone()]
}

pub fn default_options(_env: &Env) -> ShipmentOptions {
    ShipmentOptions {
        response_deadline: 0,
        penalty_bps: 0,
        milestone_mode: MilestoneMode::Parallel,
        holdback_ledgers: 0,
        dispute_cooldown_ledgers: 0,
        late_penalty_bps_per_ledger: 0,
        auto_confirm_ledgers: 0,
        dispute_bond_amount: 0,
        dispute_bond_bps: 0,
        arbiter_fee_bps: 0,
        logistics_fee_bps: 0,
        supplier_collateral: 0,
        expires_at_ledger: None,
        metadata_hash: None,
        referrer: None,
        buyer_cancel_fee_bps: 0,
        early_bonus_pool: 0,
        review_window_ledgers: None,
        milestone_splits: vec![_env],
        deadlines: vec![_env],
        dispute_timeout_seconds: 0,
        default_resolution: Resolution::Buyer,
        backup_arbiter: None,
        confirmation_cooldown_ledgers: None,
        arbiter_panel: vec![_env],
        jurisdiction: None,
        grace_period_ledgers: 0,
        quality_grades: vec![_env],
        milestone_quantities: vec![_env],
        retainage_bps: 0,
        warranty_bps: 0,
        warranty_ledgers: 0,
        inspector: None,
        inspected_milestones: vec![_env],
        proof_submitters: vec![_env],
        require_dual_attestation: false,
        fund_from_vault: false,
        milestone_suppliers: vec![_env],
        refund_recipient: None,
    }
}

/// Create a standard shipment with no deadline, no penalty, parallel mode, no holdback, no cooldown.
pub fn create_standard_shipment(
    client: &ChainSettleContractClient,
    env: &Env,
    shipment_id: &String,
    buyer: &Address,
    supplier: &Address,
    logistics: &Address,
    arbiter: &Address,
    token_id: &Address,
    total_amount: i128,
) {
    client.create_shipment(
        shipment_id,
        &single_buyer_vec(env, buyer),
        supplier,
        logistics,
        arbiter,
        token_id,
        &total_amount,
        &build_milestones(env),
        &default_options(env),
    );
}

// ============================================================
// 100 COMPREHENSIVE TESTS FOR CHAINSETTLE CONTRACT
// ============================================================

/// Event as returned by `env.events().all()`: (contract, topics, data).
pub type CapturedEvent = (Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val);

/// Runs `f` and returns only the events emitted by the contract invocation it
/// makes.
///
/// `env.events().all()` only reports the most recent top-level invocation, so
/// reading it after several calls silently drops the earlier calls' events.
/// Wrap exactly one client call in `f` and inspect the returned events instead.
/// `f` must make exactly one invocation: with none, the previous call's events
/// would be returned; with several, only the last call's events are returned.
pub fn capture_events<F: FnOnce()>(env: &Env, f: F) -> soroban_sdk::Vec<CapturedEvent> {
    use soroban_sdk::testutils::Events as _;
    f();
    env.events().all()
}

/// Returns true when any event in `events` has `topic` as its first topic.
pub fn has_event_topic(env: &Env, events: &soroban_sdk::Vec<CapturedEvent>, topic: &str) -> bool {
    use soroban_sdk::TryFromVal;
    let expected = Symbol::new(env, topic);
    events.iter().any(|(_, topics, _)| {
        topics
            .get(0)
            .and_then(|t| Symbol::try_from_val(env, &t).ok())
            .map_or(false, |t| t == expected)
    })
}

/// Submit a proof for `milestone_index` as the setup's supplier.
pub fn submit_proof_for_milestone(setup: &TestSetup, shipment_id: &String, milestone_index: u32) {
    let client = ChainSettleContractClient::new(&setup.env, &setup.contract_id);
    let proof = String::from_str(&setup.env, "proof_hash_123");
    client.submit_proof(
        &setup.supplier,
        shipment_id,
        &milestone_index,
        &proof,
        &Symbol::new(&setup.env, "ipfs"),
    );
}
