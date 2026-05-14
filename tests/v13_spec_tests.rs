use percolator::v13::{
    account_equity, risk_notional_ceil, HLockLaneV13, MarketGroupV13,
    PermissionlessProgressOutcomeV13, PermissionlessRecoveryReasonV13, PortfolioAccountV13,
    PortfolioLegV13, ProvenanceHeaderV13, SideV13, V13Config, V13Error,
    V13_MAX_PORTFOLIO_ASSETS_N,
};
use percolator::{ADL_ONE, SOCIAL_LOSS_DEN};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn group() -> MarketGroupV13 {
    let (market, _, _) = ids();
    MarketGroupV13::new(market, V13Config::public_user_fund(4, 0, 10)).unwrap()
}

fn account() -> PortfolioAccountV13 {
    let (market, account_id, owner) = ids();
    PortfolioAccountV13::empty(ProvenanceHeaderV13::new(market, account_id, owner))
}

fn active_leg(side: SideV13, basis_pos_q: i128) -> PortfolioLegV13 {
    PortfolioLegV13 {
        active: true,
        side,
        basis_pos_q,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: basis_pos_q.unsigned_abs(),
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    }
}

#[test]
fn v13_hlock_is_permissionless_state_not_oracle_input() {
    let mut g = group();
    let mut a = account();

    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV13::HMin));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(0));

    g.threshold_stress_active = true;
    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV13::HMax));
    assert_eq!(g.select_h_lock(Some(&a), false), Ok(10));

    g.threshold_stress_active = false;
    assert_eq!(g.h_lock_lane(Some(&a), true), Ok(HLockLaneV13::HMax));

    a.b_stale_state = true;
    assert_eq!(g.h_lock_lane(Some(&a), false), Ok(HLockLaneV13::HMax));
}

#[test]
fn v13_provenance_binds_account_to_market_owner_and_layout() {
    let g = group();
    let mut a = account();
    assert_eq!(g.validate_portfolio_account_provenance(&a), Ok(()));

    a.provenance_header.market_group_id = [9; 32];
    assert_eq!(
        g.validate_portfolio_account_provenance(&a),
        Err(V13Error::ProvenanceMismatch)
    );
}

#[test]
fn v13_active_bitmap_is_the_only_active_leg_authority() {
    let g = group();
    let mut a = account();
    a.legs[0] = active_leg(SideV13::Long, 1);
    assert_eq!(g.validate_account_shape(&a), Err(V13Error::HiddenLeg));

    a.active_bitmap = 1;
    assert_eq!(g.validate_account_shape(&a), Ok(()));

    a.legs[5] = active_leg(SideV13::Short, -1);
    a.active_bitmap |= 1 << 5;
    assert_eq!(g.validate_account_shape(&a), Err(V13Error::HiddenLeg));
}

#[test]
fn v13_stale_and_b_stale_counters_are_exact_and_idempotent() {
    let mut g = group();
    let mut a = account();

    g.mark_account_stale(&mut a).unwrap();
    g.mark_account_stale(&mut a).unwrap();
    assert!(a.stale_state);
    assert_eq!(g.stale_certificate_count, 1);

    g.clear_account_stale(&mut a).unwrap();
    g.clear_account_stale(&mut a).unwrap();
    assert!(!a.stale_state);
    assert_eq!(g.stale_certificate_count, 0);

    g.mark_account_b_stale(&mut a).unwrap();
    g.mark_account_b_stale(&mut a).unwrap();
    assert!(a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 1);

    g.clear_account_b_stale(&mut a).unwrap();
    g.clear_account_b_stale(&mut a).unwrap();
    assert!(!a.b_stale_state);
    assert_eq!(g.b_stale_account_count, 0);
}

#[test]
fn v13_favorable_action_requires_current_full_account_refresh() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    g.attach_leg(&mut a, 0, SideV13::Long, 1_000_000)
        .unwrap();
    let mut prices = [1u64; V13_MAX_PORTFOLIO_ASSETS_N];
    prices[0] = 100;

    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V13Error::Stale));

    let cert = g.full_account_refresh(&mut a, &prices).unwrap();
    assert!(cert.valid);
    assert_eq!(cert.certified_maintenance_req, 100);
    assert_eq!(g.ensure_favorable_action_allowed(&a), Ok(()));

    g.oracle_epoch += 1;
    assert_eq!(g.ensure_favorable_action_allowed(&a), Err(V13Error::Stale));
}

#[test]
fn v13_b_stale_blocks_refresh_and_favorable_actions_without_scanning_market() {
    let mut g = group();
    let mut a = account();
    a.capital = 100;
    g.attach_leg(&mut a, 0, SideV13::Long, 1_000_000)
        .unwrap();
    let prices = [100u64; V13_MAX_PORTFOLIO_ASSETS_N];

    g.mark_account_b_stale(&mut a).unwrap();
    assert_eq!(g.full_account_refresh(&mut a, &prices), Err(V13Error::BStale));
    assert_eq!(
        g.ensure_favorable_action_allowed(&a),
        Err(V13Error::LockActive)
    );
}

#[test]
fn v13_public_init_rejects_unbounded_portfolio_width() {
    let (market, _, _) = ids();
    let cfg = V13Config::public_user_fund((V13_MAX_PORTFOLIO_ASSETS_N + 1) as u8, 0, 10);
    assert_eq!(MarketGroupV13::new(market, cfg), Err(V13Error::InvalidConfig));
}

#[test]
fn v13_risk_notional_and_equity_use_exact_conservative_shapes() {
    assert_eq!(risk_notional_ceil(1, 1), Ok(1));
    assert_eq!(risk_notional_ceil(1, 1_000_001), Ok(2));

    let mut a = account();
    a.capital = 100;
    a.pnl = -25;
    a.fee_credits = -10;
    assert_eq!(account_equity(&a), Ok(65));
}

#[test]
fn v13_attach_and_clear_leg_update_only_bounded_account_and_asset_state() {
    let mut g = group();
    let mut a = account();

    g.attach_leg(&mut a, 1, SideV13::Short, -7).unwrap();
    assert_eq!(a.active_bitmap, 1 << 1);
    assert_eq!(g.assets[1].stored_pos_count_short, 1);
    assert_eq!(g.assets[1].oi_eff_short_q, 7);
    assert_eq!(g.assets[1].loss_weight_sum_short, 7);

    g.clear_leg(&mut a, 1).unwrap();
    assert_eq!(a.active_bitmap, 0);
    assert_eq!(g.assets[1].stored_pos_count_short, 0);
    assert_eq!(g.assets[1].oi_eff_short_q, 0);
    assert_eq!(g.assets[1].loss_weight_sum_short, 0);
}

#[test]
fn v13_account_b_chunk_makes_strict_account_local_progress_or_requires_recovery() {
    let mut g = group();
    let mut a = account();
    g.attach_leg(&mut a, 0, SideV13::Long, 1).unwrap();
    g.assets[0].b_long_num = SOCIAL_LOSS_DEN * 2;
    g.mark_leg_b_stale(&mut a, 0).unwrap();

    let chunk = g.settle_account_b_chunk(&mut a, 0, SOCIAL_LOSS_DEN).unwrap();
    assert!(chunk.delta_b > 0);
    assert!(a.legs[0].b_snap > 0);
    assert_eq!(a.health_cert.valid, false);

    let mut blocked = account();
    g.attach_leg(&mut blocked, 1, SideV13::Long, 1).unwrap();
    g.assets[1].b_long_num = 1;
    g.mark_leg_b_stale(&mut blocked, 1).unwrap();
    assert_eq!(
        g.settle_account_b_chunk(&mut blocked, 1, 0),
        Err(V13Error::RecoveryRequired)
    );
}

#[test]
fn v13_liquidation_progress_requires_strict_risk_score_reduction() {
    let mut g = group();
    let mut before = account();
    let mut after = account();
    g.full_account_refresh(&mut before, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();
    g.full_account_refresh(&mut after, &[1; V13_MAX_PORTFOLIO_ASSETS_N])
        .unwrap();

    before.health_cert.certified_liq_deficit = 10;
    after.health_cert.certified_liq_deficit = 10;
    assert_eq!(
        g.validate_liquidation_progress(&before, &after),
        Err(V13Error::NonProgress)
    );

    after.health_cert.certified_liq_deficit = 9;
    assert_eq!(g.validate_liquidation_progress(&before, &after), Ok(()));
}

#[test]
fn v13_permissionless_recovery_is_declared_by_reason_not_caller_price() {
    let mut g = group();
    let reason = PermissionlessRecoveryReasonV13::AccountBSettlementCannotProgress;
    assert_eq!(
        g.declare_permissionless_recovery(reason),
        Ok(PermissionlessProgressOutcomeV13::RecoveryDeclared(reason))
    );
    assert_eq!(g.recovery_reason, Some(reason));
}
