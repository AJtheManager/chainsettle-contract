#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token, vec, Address, BytesN, String, TryFromVal, Vec,
};
use crate::test_common::*;

#[test]
fn test_revoke_role_takes_effect_immediately() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);

    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &None);
    assert!(client.has_role(&Role::Pauser, &pauser));

    // Pauser can pause
    client.pause(&pauser);
    assert!(client.is_paused());
    client.unpause(&pauser);
    assert!(!client.is_paused());

    // Revoke
    client.revoke_role(&t.buyer, &Role::Pauser, &pauser);
    assert!(!client.has_role(&Role::Pauser, &pauser));
}

#[test]
fn test_fee_manager_can_manage_fees() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let fee_mgr = Address::generate(&t.env);

    client.grant_role(&t.buyer, &Role::FeeManager, &fee_mgr, &None);

    // set_fee_config
    client.set_fee_config(&fee_mgr, &250, &t.treasury);
    assert_eq!(client.get_fee_config().unwrap().fee_bps, 250);

    // schedule & cancel fee holiday
    client.schedule_fee_holiday(&fee_mgr, &100, &200);
    client.cancel_fee_holiday(&fee_mgr);

    // set_fee_recipients
    let recipients = vec![
        &t.env,
        FeeRecipient {
            recipient: t.treasury.clone(),
            share_bps: 10_000,
        },
    ];
    client.set_fee_recipients(&fee_mgr, &recipients);

    // set_referral_fee_bps
    client.set_referral_fee_bps(&fee_mgr, &300);
    assert_eq!(client.get_referral_fee_bps(), 300);

    // set_fee_tiers
    let tiers = vec![
        &t.env,
        FeeTier {
            min_lifetime_volume: 1_000_000,
            fee_bps: 50,
        },
    ];
    client.set_fee_tiers(&fee_mgr, &tiers);
}

#[test]
fn test_compliance_officer_can_manage_compliance() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let compliance = Address::generate(&t.env);
    let bad_actor = Address::generate(&t.env);
    let supplier = Address::generate(&t.env);

    client.grant_role(&t.buyer, &Role::ComplianceOfficer, &compliance, &None);

    // Blacklist and unblacklist
    client.blacklist_address(&compliance, &bad_actor, &BytesN::from_array(&t.env, &[2u8; 32]));
    assert!(client.is_blacklisted(&bad_actor));
    client.remove_from_blacklist(&compliance, &bad_actor);
    assert!(!client.is_blacklisted(&bad_actor));

    // Whitelist and remove
    client.add_to_whitelist(&compliance, &supplier);
    client.remove_from_whitelist(&compliance, &supplier);

    // Mutual pre-approval
    client.set_require_mutual_preapproval(&compliance, &true);
    assert!(client.get_require_mutual_preapproval());
    client.set_require_mutual_preapproval(&compliance, &false);
    assert!(!client.get_require_mutual_preapproval());

    // Max allowed tokens
    client.set_max_allowed_tokens(&compliance, &10);
    assert_eq!(client.get_max_allowed_tokens(), 10);
}

#[test]
fn test_grant_revoke_audit_log_and_events() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let user = Address::generate(&t.env);

    client.grant_role(&t.buyer, &Role::Pauser, &user, &None);
    client.revoke_role(&t.buyer, &Role::Pauser, &user);

    let log = client.get_admin_log();
    assert!(log.len() >= 2);
    let last_revoked = log.get(log.len() - 1).unwrap();
    let last_granted = log.get(log.len() - 2).unwrap();

    assert_eq!(last_granted.action, Symbol::new(&t.env, "grant_role"));
    assert_eq!(last_granted.detail, Symbol::new(&t.env, "role_granted"));
    assert_eq!(last_revoked.action, Symbol::new(&t.env, "revoke_role"));
    assert_eq!(last_revoked.detail, Symbol::new(&t.env, "role_revoked"));
}
