//! Fuzz/integration coverage for the residual/backing double-claim class.
//!
//! Recoverable counterparty backing principal is provider-withdrawable with no
//! mode or payout-snapshot gate, so it must never be counted in residual(), the
//! junior payout pool. If it is, the resolved payout snapshot promises winners
//! the same vault atoms the provider can still withdraw, and whichever party
//! moves second is robbed or stranded. The Kani proof
//! `proof_v16_residual_excludes_recoverable_counterparty_backing_principal`
//! pins the residual() primitive; these randomized tests drive the real
//! end-to-end resolved close + provider withdrawal in BOTH orders and assert
//! the two claims never overlap.

use percolator::BOUND_SCALE;
use percolator::{
    BackingBucketStatusV16, BackingBucketV16, BackingBucketV16Account, EngineAssetSlotV16Account,
    Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut, PortfolioAccountV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account,
    ResolvedCloseOutcomeV16, SourceCreditStateV16, SourceCreditStateV16Account, V16Config,
    V16PodI128, V16PodU128, V16PodU64, CREDIT_RATE_SCALE,
};
use proptest::prelude::*;

fn market_id() -> [u8; 32] {
    [1u8; 32]
}

fn empty_account() -> PortfolioAccountV16Account {
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id(),
        [2u8; 32],
        [2u8; 32],
    ));
    let mut account = PortfolioAccountV16Account::default();
    account.init_empty_in_place(header).unwrap();
    account
}

/// Resolved single-winner haircut market with `backing` atoms of recoverable
/// counterparty backing principal sitting in the vault alongside the winner's
/// capital and the junior residual.
fn resolved_market_with_backing(
    capital: u128,
    pnl: u128,
    residual: u128,
    backing: u128,
) -> (MarketGroupV16HeaderAccount, [Market<u64>; 1]) {
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id(), cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    header
        .activate_empty_asset_slot_not_atomic(0, &mut markets[0].engine, 100, 1)
        .unwrap();
    header.mode = 1; // Resolved
    header.resolved_slot = V16PodU64::new(1);
    header.current_slot = V16PodU64::new(1);
    header.vault = V16PodU128::new(capital + residual + backing);
    header.c_tot = V16PodU128::new(capital);
    header.pnl_pos_tot = V16PodU128::new(pnl);
    header.pnl_matured_pos_tot = V16PodU128::new(pnl);
    header.pnl_pos_bound_tot = V16PodU128::new(pnl);
    header.pnl_pos_bound_tot_num = V16PodU128::new(pnl * BOUND_SCALE);
    if backing != 0 {
        let backing_num = backing * BOUND_SCALE;
        header.source_fresh_backing_total_num = V16PodU128::new(backing_num);
        let engine_market_id = markets[0].engine.asset.market_id.get();
        markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
            market_id: engine_market_id,
            fresh_unliened_backing_num: backing_num,
            expiry_slot: 100,
            status: BackingBucketStatusV16::Fresh,
            ..BackingBucketV16::EMPTY
        });
        markets[0].engine.source_credit_long =
            SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
                fresh_reserved_backing_num: backing_num,
                credit_rate_num: CREDIT_RATE_SCALE,
                ..SourceCreditStateV16::EMPTY
            });
    }
    (header, markets)
}

fn winner_account(capital: u128, pnl: u128) -> PortfolioAccountV16Account {
    let mut account_header = empty_account();
    account_header.capital = V16PodU128::new(capital);
    account_header.pnl = V16PodI128::new(pnl as i128);
    account_header.last_fee_slot = V16PodU64::new(1);
    account_header
}

/// Close the winner, then (optionally first) withdraw the provider principal.
/// Returns (winner_payout, vault_after_everything).
fn run_order(
    capital: u128,
    pnl: u128,
    residual: u128,
    backing: u128,
    provider_first: bool,
) -> (u128, u128) {
    let (mut header, mut markets) = resolved_market_with_backing(capital, pnl, residual, backing);
    let mut account_header = winner_account(capital, pnl);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header);
    assert_eq!(market.validate_shape(), Ok(()));
    assert_eq!(account.validate_with_market(&market.as_view()), Ok(()));

    let vault_before = market.header.vault.get();
    if provider_first {
        market
            .withdraw_fresh_counterparty_backing_not_atomic(0, backing)
            .expect("provider principal must be withdrawable before the winner closes");
    }
    let outcome = market
        .close_resolved_account_not_atomic(&mut account, 0)
        .expect("winner close must not revert");
    let closed = matches!(outcome, ResolvedCloseOutcomeV16::Closed { .. });
    assert!(closed, "winner did not fully close");
    if !provider_first {
        market
            .withdraw_fresh_counterparty_backing_not_atomic(0, backing)
            .expect("provider principal must remain withdrawable after the winner closes");
    }
    assert_eq!(market.validate_shape(), Ok(()));
    let vault_after = market.header.vault.get();
    let winner_payout = vault_before - vault_after - backing;
    (winner_payout, vault_after)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// The winner's resolved payout and the provider's principal must be funded
    /// by DISJOINT vault atoms: the winner is paid capital + haircut residual
    /// (never the provider's backing), and the provider can recover the full
    /// principal regardless of whether the withdrawal happens before or after
    /// the payout snapshot is captured by the winner's close.
    #[test]
    fn winner_payout_and_provider_principal_never_overlap(
        capital in 0u128..=1_000_000u128,
        pnl in 2u128..=1_000_000u128,
        residual_frac in 1u128..=999u128,
        backing in 1u128..=1_000_000u128,
    ) {
        // haircut: residual strictly below the winner's junior bound.
        let residual = (pnl.saturating_mul(residual_frac) / 1000).max(1).min(pnl - 1);
        prop_assume!(residual < pnl);

        let (payout_after, vault_after) =
            run_order(capital, pnl, residual, backing, false);
        let (payout_first, vault_first) =
            run_order(capital, pnl, residual, backing, true);

        // The winner gets exactly its capital plus the honest junior residual...
        prop_assert_eq!(payout_after, capital + residual);
        // ...identically in both orders (the snapshot must not depend on whether
        // the provider already recovered principal)...
        prop_assert_eq!(payout_first, payout_after);
        // ...and nothing else leaks: the vault drains to zero in both orders.
        prop_assert_eq!(vault_after, 0);
        prop_assert_eq!(vault_first, 0);
    }
}
