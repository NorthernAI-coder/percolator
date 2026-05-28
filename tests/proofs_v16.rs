#![cfg(kani)]

use percolator::v16::{
    EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount, MarketGroupV16ViewMut,
    PortfolioAccountV16Account, PortfolioLegV16, PortfolioLegV16Account,
    PortfolioSourceDomainV16Account, PortfolioV16ViewMut, ProvenanceHeaderV16,
    ProvenanceHeaderV16Account, SideV16, V16Config, V16Error, V16PodI128, V16PodU128, V16PodU64,
};
use percolator::{ADL_ONE, POS_SCALE};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn one_market_view_fixture() -> (
    MarketGroupV16HeaderAccount,
    [Market<u64>; 1],
    PortfolioAccountV16Account,
    [PortfolioSourceDomainV16Account; 2],
) {
    let (market_id, account_id, owner) = ids();
    let cfg = V16Config::public_user_fund_with_market_slots(1, 1, 0, 10);
    let mut header = MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, 1, 0).unwrap();
    let mut markets = [Market::new(0u64, EngineAssetSlotV16Account::default())];
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        view.activate_empty_market_not_atomic(0, 100, 1).unwrap();
    }
    let account_header =
        PortfolioAccountV16Account::try_empty(ProvenanceHeaderV16Account::from_runtime(
            &ProvenanceHeaderV16::new(market_id, account_id, owner),
        ))
        .unwrap();
    let source_domains = [PortfolioSourceDomainV16Account::default(); 2];
    (header, markets, account_header, source_domains)
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_deposit_preserves_c_tot_vault_capital_sum() {
    let amount_raw: u16 = kani::any();
    kani::assume(amount_raw <= 1_000);
    let amount = amount_raw as u128;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    market.deposit_not_atomic(&mut account, amount).unwrap();

    kani::cover!(amount > 0, "view deposit covers nonzero amount");
    assert_eq!(account.header.capital.get(), amount);
    assert_eq!(market.header.c_tot.get(), amount);
    assert_eq!(market.header.vault.get(), amount);
    assert_eq!(market.validate_shape(), Ok(()));
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_overwithdraw_rejects_without_mutation() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    market.deposit_not_atomic(&mut account, 3).unwrap();
    let before_vault = market.header.vault;
    let before_c_tot = market.header.c_tot;
    let before_capital = account.header.capital;

    let result = market.withdraw_not_atomic(&mut account, 4);

    kani::cover!(
        result == Err(V16Error::LockActive),
        "view overwithdraw lock branch reachable"
    );
    assert_eq!(result, Err(V16Error::LockActive));
    assert_eq!(market.header.vault, before_vault);
    assert_eq!(market.header.c_tot, before_c_tot);
    assert_eq!(account.header.capital, before_capital);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_fee_sync_settles_negative_pnl_before_fee() {
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(100);
    header.c_tot = V16PodU128::new(100);
    header.negative_pnl_account_count = V16PodU64::new(1);
    header.current_slot = V16PodU64::new(10);
    header.slot_last = V16PodU64::new(10);
    account_header.capital = V16PodU128::new(100);
    account_header.pnl = V16PodI128::new(-40);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let charged = market
        .sync_account_fee_to_slot_not_atomic(&mut account, 10, 10)
        .unwrap();

    kani::cover!(
        charged == 60 && account.header.pnl.get() == 0,
        "view fee sync settles realized loss before fee"
    );
    assert_eq!(charged, 60);
    assert_eq!(account.header.pnl.get(), 0);
    assert_eq!(account.header.capital.get(), 0);
    assert_eq!(market.header.c_tot.get(), 0);
    assert_eq!(market.header.insurance.get(), 60);
    assert_eq!(market.header.vault.get(), 100);
}

#[kani::proof]
#[kani::unwind(48)]
#[kani::solver(cadical)]
fn proof_v16_view_domain_budget_caps_bankruptcy_insurance_spend() {
    let budget_raw: u8 = kani::any();
    kani::assume(budget_raw <= 5);
    let budget = budget_raw as u128;
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(10);
    header.insurance = V16PodU128::new(10);
    header.negative_pnl_account_count = V16PodU64::new(1);
    markets[0].engine.insurance_domain_budget_short = V16PodU128::new(budget);
    account_header.pnl = V16PodI128::new(-5);
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    let used = market
        .kani_consume_domain_insurance_for_negative_pnl(0, SideV16::Long, &mut account)
        .unwrap();

    kani::cover!(budget == 0 && used == 0, "zero domain budget spend branch");
    kani::cover!(
        budget > 0 && used == budget,
        "positive domain budget spend branch"
    );
    assert_eq!(used, budget);
    assert_eq!(market.header.insurance.get(), 10 - budget);
    assert_eq!(
        market.markets[0].engine.insurance_domain_spent_short.get(),
        budget
    );
    assert_eq!(account.header.pnl.get(), -5 + budget as i128);
}

#[kani::proof]
#[kani::unwind(64)]
#[kani::solver(cadical)]
fn proof_v16_view_funding_target_signs_match_long_short_sides() {
    let positive_funding: bool = kani::any();
    let (mut header, mut markets, mut account_header, mut source_domains) =
        one_market_view_fixture();
    header.vault = V16PodU128::new(10);
    header.c_tot = V16PodU128::new(10);
    account_header.capital = V16PodU128::new(10);
    if positive_funding {
        markets[0].engine.asset.f_long_num = V16PodI128::new(-(ADL_ONE as i128));
        markets[0].engine.asset.f_short_num = V16PodI128::new(ADL_ONE as i128);
    } else {
        markets[0].engine.asset.f_long_num = V16PodI128::new(ADL_ONE as i128);
        markets[0].engine.asset.f_short_num = V16PodI128::new(-(ADL_ONE as i128));
    }
    account_header.active_bitmap[0] = V16PodU64::new(1);
    account_header.legs[0] = PortfolioLegV16Account::from_runtime(&PortfolioLegV16 {
        active: true,
        asset_index: 0,
        market_id: 1,
        side: SideV16::Long,
        basis_pos_q: POS_SCALE as i128,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: POS_SCALE,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    });
    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    market
        .full_account_refresh_not_atomic(&mut account)
        .unwrap();

    kani::cover!(
        positive_funding && account.header.capital.get() == 9 && account.header.pnl.get() == 0,
        "positive funding charges long"
    );
    kani::cover!(
        !positive_funding && account.header.capital.get() == 10 && account.header.pnl.get() == 1,
        "negative funding pays long"
    );
    if positive_funding {
        assert_eq!(account.header.capital.get(), 9);
        assert_eq!(account.header.pnl.get(), 0);
    } else {
        assert_eq!(account.header.capital.get(), 10);
        assert_eq!(account.header.pnl.get(), 1);
    }
}
