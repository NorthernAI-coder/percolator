#![cfg(kani)]

use percolator::v13::{
    account_equity, HLockLaneV13, MarketGroupV13, PortfolioAccountV13, ProvenanceHeaderV13,
    SideV13, V13Config, V13Error, V13_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::SOCIAL_LOSS_DEN;

fn symbolic_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    let market: [u8; 32] = kani::any();
    let account: [u8; 32] = kani::any();
    let owner: [u8; 32] = kani::any();
    (market, account, owner)
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_hlock_is_exactly_hmin_or_hmax() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let mut group =
        MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.threshold_stress_active = kani::any();
    group.bankruptcy_hlock_active = kani::any();
    group.loss_stale_active = kani::any();
    group.active_bankrupt_close_present = kani::any();
    account.stale_state = kani::any();
    account.b_stale_state = kani::any();
    let instruction_bankruptcy_candidate: bool = kani::any();

    kani::cover!(
        !group.threshold_stress_active
            && !group.bankruptcy_hlock_active
            && !group.loss_stale_active
            && !group.active_bankrupt_close_present
            && !account.stale_state
            && !account.b_stale_state
            && !instruction_bankruptcy_candidate,
        "v13 h-min lane reachable"
    );
    kani::cover!(
        group.threshold_stress_active
            || group.bankruptcy_hlock_active
            || group.loss_stale_active
            || group.active_bankrupt_close_present
            || account.stale_state
            || account.b_stale_state
            || instruction_bankruptcy_candidate,
        "v13 h-max lane reachable"
    );

    let selected = group
        .select_h_lock(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    assert!(selected == 0 || selected == h_max as u64);

    let lane = group
        .h_lock_lane(Some(&account), instruction_bankruptcy_candidate)
        .unwrap();
    if lane == HLockLaneV13::HMax {
        assert_eq!(selected, h_max as u64);
    } else {
        assert_eq!(selected, 0);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_hmin_zero_remains_available_when_no_lock_state_exists() {
    let h_max: u8 = kani::any();
    kani::assume(h_max > 0);
    let (market, account_id, owner) = symbolic_ids();
    let group =
        MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, h_max as u64)).unwrap();
    let account = PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    assert_eq!(group.h_lock_lane(Some(&account), false), Ok(HLockLaneV13::HMin));
    assert_eq!(group.select_h_lock(Some(&account), false), Ok(0));
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_stale_counter_transitions_are_idempotent() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.mark_account_stale(&mut account).unwrap();
    group.mark_account_stale(&mut account).unwrap();
    kani::cover!(account.stale_state, "v13 stale state reachable");
    assert_eq!(group.stale_certificate_count, 1);

    group.clear_account_stale(&mut account).unwrap();
    group.clear_account_stale(&mut account).unwrap();
    kani::cover!(!account.stale_state, "v13 stale clear reachable");
    assert_eq!(group.stale_certificate_count, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_account_equity_rejects_i128_min_persistent_pnl() {
    let (market, account_id, owner) = symbolic_ids();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.pnl = i128::MIN;
    assert_eq!(account_equity(&account), Err(V13Error::ArithmeticOverflow));
}

fn concrete_ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_hidden_leg_rejected_by_bitmap_authority() {
    let (market, account_id, owner) = concrete_ids();
    let group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    account.legs[0].active = true;
    kani::cover!(
        account.active_bitmap == 0 && account.legs[0].active,
        "v13 hidden active leg reachable"
    );
    assert_eq!(
        group.validate_account_shape(&account),
        Err(V13Error::HiddenLeg)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_attach_then_clear_leg_restores_account_local_counters_for_long() {
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));

    group.attach_leg(&mut account, 0, SideV13::Long, 7).unwrap();
    assert_eq!(account.active_bitmap, 1);
    assert_eq!(account.legs[0].basis_pos_q, 7);
    assert_eq!(group.assets[0].oi_eff_long_q, 7);

    group.clear_leg(&mut account, 0).unwrap();
    assert_eq!(account.active_bitmap, 0);
    assert_eq!(group.assets[0].oi_eff_long_q, 0);
    assert_eq!(group.assets[0].oi_eff_short_q, 0);
    assert_eq!(group.assets[0].stored_pos_count_long, 0);
    assert_eq!(group.assets[0].stored_pos_count_short, 0);
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_account_b_chunk_either_advances_or_fails_closed() {
    let target_units: u8 = kani::any();
    let budget_units: u8 = kani::any();
    kani::assume(target_units <= 4);
    kani::assume(budget_units <= 4);
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    group.attach_leg(&mut account, 0, SideV13::Long, 1).unwrap();
    group.assets[0].b_long_num = (target_units as u128) * SOCIAL_LOSS_DEN;
    group.mark_leg_b_stale(&mut account, 0).unwrap();

    let before_snap = account.legs[0].b_snap;
    let before_remaining = group.assets[0].b_long_num - before_snap;
    let budget = (budget_units as u128) * SOCIAL_LOSS_DEN;
    let result = group.settle_account_b_chunk(&mut account, 0, budget);

    if before_remaining == 0 {
        assert!(result.is_ok());
        assert_eq!(account.legs[0].b_snap, before_snap);
    } else if budget == 0 {
        assert_eq!(result, Err(V13Error::RecoveryRequired));
        assert_eq!(account.legs[0].b_snap, before_snap);
    } else {
        let chunk = result.unwrap();
        kani::cover!(chunk.delta_b > 0, "v13 B chunk progress reachable");
        assert!(chunk.delta_b > 0);
        assert!(account.legs[0].b_snap > before_snap);
        assert!(chunk.remaining_after < before_remaining);
    }
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_liquidation_progress_rejects_non_reducing_scores() {
    let deficit: u8 = kani::any();
    let (market, account_id, owner) = symbolic_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut before =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    let mut after = before;
    group
        .full_account_refresh(&mut before, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    group
        .full_account_refresh(&mut after, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    before.health_cert.certified_liq_deficit = deficit as u128;
    after.health_cert.certified_liq_deficit = deficit as u128;

    assert_eq!(
        group.validate_liquidation_progress(&before, &after),
        Err(V13Error::NonProgress)
    );
}

#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn proof_v13_favorable_action_requires_current_full_refresh() {
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV13::new(market, V13Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner));
    account.capital = 2;

    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V13Error::Stale)
    );
    group.full_account_refresh(&mut account, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    assert_eq!(group.ensure_favorable_action_allowed(&account), Ok(()));
    group.oracle_epoch += 1;
    assert_eq!(
        group.ensure_favorable_action_allowed(&account),
        Err(V13Error::Stale)
    );
}
