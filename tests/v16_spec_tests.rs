use percolator::v16::{
    v16_domain_count_for_market_slots, BackingBucketStatusV16, BackingBucketV16,
    BackingBucketV16Account, EngineAssetSlotV16Account, Market, MarketGroupV16HeaderAccount,
    MarketGroupV16ViewMut, PortfolioAccountV16Account, PortfolioSourceDomainV16Account,
    PortfolioV16ViewMut, ProvenanceHeaderV16, ProvenanceHeaderV16Account, SourceCreditStateV16,
    SourceCreditStateV16Account, TradeRequestV16, V16Config, V16Error, V16PodI128, V16PodU128,
    V16PodU64,
};
use percolator::{BOUND_SCALE, CREDIT_RATE_SCALE, POS_SCALE};

fn ids() -> ([u8; 32], [u8; 32], [u8; 32]) {
    ([1; 32], [2; 32], [3; 32])
}

fn market_fixture(
    market_slots: u32,
    init_price: u64,
) -> (MarketGroupV16HeaderAccount, Vec<Market<u64>>) {
    let (market_id, _, _) = ids();
    let cfg =
        V16Config::public_user_fund_with_market_slots(market_slots as u16, market_slots, 0, 10);
    let mut header =
        MarketGroupV16HeaderAccount::new_dynamic(market_id, cfg, market_slots, 0).unwrap();
    let mut markets = (0..market_slots)
        .map(|i| Market::new(i as u64, EngineAssetSlotV16Account::default()))
        .collect::<Vec<_>>();
    {
        let mut view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        for i in 0..market_slots as usize {
            view.activate_empty_market_not_atomic(i as u32, init_price, (i + 1) as u64)
                .unwrap();
        }
        view.validate_shape().unwrap();
    }
    (header, markets)
}

fn account_fixture(
    market_slots: u32,
    account_seed: u8,
) -> (
    PortfolioAccountV16Account,
    Vec<PortfolioSourceDomainV16Account>,
) {
    let (market_id, _, owner) = ids();
    let header = ProvenanceHeaderV16Account::from_runtime(&ProvenanceHeaderV16::new(
        market_id,
        [account_seed; 32],
        owner,
    ));
    let account = PortfolioAccountV16Account::try_empty(header).unwrap();
    let domains = vec![
        PortfolioSourceDomainV16Account::default();
        v16_domain_count_for_market_slots(market_slots).unwrap()
    ];
    (account, domains)
}

#[test]
fn v16_view_deposit_and_withdraw_are_the_tested_paths() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let (mut account_header, mut source_domains) = account_fixture(1, 2);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);

    market_view
        .deposit_not_atomic(&mut account_view, 11)
        .unwrap();
    market_view
        .withdraw_not_atomic(&mut account_view, 4)
        .unwrap();

    assert_eq!(account_view.header.capital.get(), 7);
    assert_eq!(market_view.header.c_tot.get(), 7);
    assert_eq!(market_view.header.vault.get(), 7);
    market_view.validate_shape().unwrap();
    account_view
        .validate_with_market(&market_view.as_view())
        .unwrap();
}

#[test]
fn v16_view_fee_sync_settles_flat_loss_before_fee() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let (mut account_header, mut source_domains) = account_fixture(1, 4);
    header.vault = V16PodU128::new(100);
    header.c_tot = V16PodU128::new(100);
    header.negative_pnl_account_count = V16PodU64::new(1);
    header.current_slot = V16PodU64::new(10);
    header.slot_last = V16PodU64::new(10);
    account_header.capital = V16PodU128::new(100);
    account_header.pnl = V16PodI128::new(-40);

    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    let charged = market_view
        .sync_account_fee_to_slot_not_atomic(&mut account_view, 10, 10)
        .unwrap();

    assert_eq!(charged, 60);
    assert_eq!(account_view.header.pnl.get(), 0);
    assert_eq!(account_view.header.capital.get(), 0);
    assert_eq!(market_view.header.c_tot.get(), 0);
    assert_eq!(market_view.header.insurance.get(), 60);
    assert_eq!(market_view.header.vault.get(), 100);
    assert_eq!(market_view.header.negative_pnl_account_count.get(), 0);
}

#[test]
fn v16_view_dynamic_market_slots_can_be_activated_without_runtime_vec_engine() {
    let (mut header, mut markets) = market_fixture(3, 100);
    let view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    view.validate_shape().unwrap();

    assert_eq!(
        view.header
            .config
            .try_to_runtime()
            .unwrap()
            .max_market_slots,
        3
    );
    assert_eq!(view.markets.len(), 3);
    assert_eq!(view.markets[2].engine.asset.market_id.get(), 3);
    assert_eq!(view.markets[2].engine.asset.effective_price.get(), 100);
}

#[test]
fn v16_view_rejects_overwithdraw_without_mutation() {
    let (mut header, mut markets) = market_fixture(1, 100);
    let (mut account_header, mut source_domains) = account_fixture(1, 6);
    let mut market_view = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut account_view = PortfolioV16ViewMut::new(&mut account_header, &mut source_domains);
    market_view
        .deposit_not_atomic(&mut account_view, 3)
        .unwrap();

    let before_vault = market_view.header.vault.get();
    let before_c_tot = market_view.header.c_tot.get();
    let before_capital = account_view.header.capital.get();
    let err = market_view.withdraw_not_atomic(&mut account_view, 4);

    assert_eq!(err, Err(V16Error::LockActive));
    assert_eq!(market_view.header.vault.get(), before_vault);
    assert_eq!(market_view.header.c_tot.get(), before_c_tot);
    assert_eq!(account_view.header.capital.get(), before_capital);
}

#[test]
fn v16_risk_increasing_trade_creates_source_credit_lien_for_im() {
    let (mut header, mut markets) = market_fixture(1, 1);
    let (mut long_header, mut long_domains) = account_fixture(1, 8);
    let (mut short_header, mut short_domains) = account_fixture(1, 9);
    let claim = 100u128;
    let claim_num = claim * BOUND_SCALE;
    long_header.pnl = V16PodI128::new(claim as i128);
    long_domains[0].source_claim_market_id = V16PodU64::new(1);
    long_domains[0].source_claim_bound_num = V16PodU128::new(claim_num);
    header.pnl_pos_tot = V16PodU128::new(claim);
    header.pnl_pos_bound_tot_num = V16PodU128::new(claim_num);
    header.pnl_pos_bound_tot = V16PodU128::new(claim);
    markets[0].engine.source_credit_long =
        SourceCreditStateV16Account::from_runtime(&SourceCreditStateV16 {
            positive_claim_bound_num: claim_num,
            exact_positive_claim_num: claim_num,
            fresh_reserved_backing_num: claim_num,
            credit_rate_num: CREDIT_RATE_SCALE,
            ..SourceCreditStateV16::EMPTY
        });
    markets[0].engine.backing_long = BackingBucketV16Account::from_runtime(&BackingBucketV16 {
        market_id: 1,
        fresh_unliened_backing_num: claim_num,
        expiry_slot: 100,
        status: BackingBucketStatusV16::Fresh,
        ..BackingBucketV16::EMPTY
    });
    {
        let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
        let mut short = PortfolioV16ViewMut::new(&mut short_header, &mut short_domains);
        market.deposit_not_atomic(&mut short, 1_000).unwrap();
    }

    let mut market = MarketGroupV16ViewMut::new(&mut header, &mut markets);
    let mut long = PortfolioV16ViewMut::new(&mut long_header, &mut long_domains);
    let mut short = PortfolioV16ViewMut::new(&mut short_header, &mut short_domains);
    market
        .execute_trade_with_fee_in_place_not_atomic(
            &mut long,
            &mut short,
            TradeRequestV16 {
                asset_index: 0,
                size_q: 10 * POS_SCALE,
                exec_price: 1,
                fee_bps: 0,
            },
        )
        .expect("risk-increasing trade should atomically lien backed source credit for IM");

    assert_eq!(long.header.capital.get(), 0);
    assert_eq!(
        long.source_domains[0].source_claim_liened_num.get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        long.source_domains[0].source_lien_effective_reserved.get(),
        10
    );
    assert_eq!(
        long.source_domains[0]
            .source_lien_counterparty_backing_num
            .get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        market.markets[0]
            .engine
            .source_credit_long
            .valid_liened_backing_num
            .get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        market.markets[0]
            .engine
            .backing_long
            .valid_liened_backing_num
            .get(),
        10 * BOUND_SCALE
    );
    assert_eq!(
        market.markets[0]
            .engine
            .backing_long
            .fresh_unliened_backing_num
            .get(),
        90 * BOUND_SCALE
    );
    market.validate_shape().unwrap();
    long.validate_with_market(&market.as_view()).unwrap();
    short.validate_with_market(&market.as_view()).unwrap();
}
