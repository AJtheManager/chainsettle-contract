#![cfg(test)]

extern crate std;

use super::*;
use crate::test_common::{setup, TestSetup};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::{Address, Symbol, TryFromVal};

fn advance_to(t: &TestSetup, ledger: u32) {
    t.env.ledger().with_mut(|l| l.sequence_number = ledger);
}

fn now(t: &TestSetup) -> u32 {
    t.env.ledger().sequence()
}

#[test]
fn test_grant_without_expiry_behaves_as_before() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);

    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &None);
    assert!(client.has_role(&Role::Pauser, &pauser));
    assert_eq!(client.get_role_expiry(&Role::Pauser, &pauser), None);

    advance_to(&t, now(&t) + 1_000);
    assert!(client.has_role(&Role::Pauser, &pauser));
    client.pause(&pauser);
    assert!(client.is_paused());
}

#[test]
fn test_role_valid_until_expiry_ledger() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);
    let expiry = now(&t) + 100;

    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &Some(expiry));
    assert_eq!(client.get_role_expiry(&Role::Pauser, &pauser), Some(expiry));

    advance_to(&t, expiry - 1);
    assert!(client.has_role(&Role::Pauser, &pauser));
    client.pause(&pauser);
    client.unpause(&pauser);

    advance_to(&t, expiry);
    assert!(!client.has_role(&Role::Pauser, &pauser));
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_expired_role_is_rejected() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let fee_mgr = Address::generate(&t.env);
    let expiry = now(&t) + 50;

    client.grant_role(&t.buyer, &Role::FeeManager, &fee_mgr, &Some(expiry));
    client.set_fee_config(&fee_mgr, &100, &t.treasury);

    advance_to(&t, expiry + 1);
    client.set_fee_config(&fee_mgr, &200, &t.treasury);
}

#[test]
fn test_regrant_without_expiry_clears_previous_expiry() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let officer = Address::generate(&t.env);
    let expiry = now(&t) + 10;

    client.grant_role(&t.buyer, &Role::ComplianceOfficer, &officer, &Some(expiry));
    client.grant_role(&t.buyer, &Role::ComplianceOfficer, &officer, &None);
    assert_eq!(client.get_role_expiry(&Role::ComplianceOfficer, &officer), None);

    advance_to(&t, expiry + 100);
    assert!(client.has_role(&Role::ComplianceOfficer, &officer));
}

#[test]
fn test_regrant_extends_expired_role() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);
    let first = now(&t) + 10;

    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &Some(first));
    advance_to(&t, first + 5);
    assert!(!client.has_role(&Role::Pauser, &pauser));

    let second = now(&t) + 10;
    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &Some(second));
    assert!(client.has_role(&Role::Pauser, &pauser));
    assert_eq!(client.get_role_expiry(&Role::Pauser, &pauser), Some(second));
}

#[test]
fn test_revoke_clears_expiry() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);

    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &Some(now(&t) + 100));
    client.revoke_role(&t.buyer, &Role::Pauser, &pauser);
    assert!(!client.has_role(&Role::Pauser, &pauser));
    assert_eq!(client.get_role_expiry(&Role::Pauser, &pauser), None);
}

#[test]
fn test_expiry_is_scoped_per_role() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let operator = Address::generate(&t.env);
    let expiry = now(&t) + 10;

    client.grant_role(&t.buyer, &Role::Pauser, &operator, &Some(expiry));
    client.grant_role(&t.buyer, &Role::FeeManager, &operator, &None);

    advance_to(&t, expiry);
    assert!(!client.has_role(&Role::Pauser, &operator));
    assert!(client.has_role(&Role::FeeManager, &operator));
}

#[test]
fn test_primary_admin_is_unaffected_by_expiry() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let expiry = now(&t) + 10;

    client.grant_role(&t.buyer, &Role::Pauser, &t.buyer, &Some(expiry));
    advance_to(&t, expiry + 10);
    assert!(client.has_role(&Role::Pauser, &t.buyer));
}

#[test]
fn test_get_role_expiry_for_unknown_grant_is_none() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let nobody = Address::generate(&t.env);
    assert_eq!(client.get_role_expiry(&Role::ArbiterManager, &nobody), None);
}

#[test]
fn test_grant_with_expiry_emits_event_and_audit() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);
    let expiry = now(&t) + 100;

    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &Some(expiry));

    let wanted = Symbol::new(&t.env, "role_expiry_set");
    let mut found = false;
    for (_, topics, data) in t.env.events().all().iter() {
        let topic = Symbol::try_from_val(&t.env, &topics.get(0).unwrap());
        if topic.map(|s| s == wanted).unwrap_or(false) {
            let (role, addr, ledger): (Role, Address, u32) =
                <(Role, Address, u32)>::try_from_val(&t.env, &data).unwrap();
            assert_eq!(role, Role::Pauser);
            assert_eq!(addr, pauser);
            assert_eq!(ledger, expiry);
            found = true;
        }
    }
    assert!(found);

    let log = client.get_admin_log();
    let last = log.get(log.len() - 1).unwrap();
    assert_eq!(last.action, Symbol::new(&t.env, "grant_role"));
    assert_eq!(last.detail, Symbol::new(&t.env, "role_granted"));
}

#[test]
#[should_panic(expected = "role expiry must be in the future")]
fn test_grant_rejects_past_expiry() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);
    advance_to(&t, 500);
    client.grant_role(&t.buyer, &Role::Pauser, &pauser, &Some(500));
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_non_admin_cannot_grant_role_with_expiry() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let stranger = Address::generate(&t.env);
    client.grant_role(&stranger, &Role::Pauser, &stranger, &Some(now(&t) + 10));
}

#[test]
fn test_grant_role_requires_admin_signature() {
    let t = setup();
    let client = ChainSettleContractClient::new(&t.env, &t.contract_id);
    let pauser = Address::generate(&t.env);
    t.env.set_auths(&[]);
    assert!(client
        .try_grant_role(&t.buyer, &Role::Pauser, &pauser, &Some(1_000))
        .is_err());
}
