//! v13 account-local risk engine foundation.
//!
//! This module implements the v13 architectural primitives that are independent
//! of the legacy v12 global slab: authenticated portfolio accounts, bounded
//! per-account refresh, stale/B-stale fail-closed checks, and deterministic
//! h-min/h-max selection from committed market/account state.

use crate::wide_math::{checked_mul_div_ceil_u256, U256};
use crate::{
    ADL_ONE, MAX_ORACLE_PRICE, MAX_POSITION_ABS_Q, MAX_VAULT_TVL, MIN_A_SIDE, POS_SCALE,
    SOCIAL_LOSS_DEN, SOCIAL_WEIGHT_SCALE,
};

pub const V13_MAX_PORTFOLIO_ASSETS_N: usize = 16;
pub const V13_LAYOUT_DISCRIMINATOR: u16 = 13;
pub const V13_ACCOUNT_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V13Error {
    InvalidConfig,
    ArithmeticOverflow,
    ProvenanceMismatch,
    HiddenLeg,
    InvalidLeg,
    Stale,
    BStale,
    LockActive,
    NonProgress,
    RecoveryRequired,
    CounterOverflow,
    CounterUnderflow,
}

pub type V13Result<T> = core::result::Result<T, V13Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HLockLaneV13 {
    HMin,
    HMax,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideV13 {
    Long,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideModeV13 {
    Normal,
    DrainOnly,
    ResetPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessRecoveryReasonV13 {
    BelowProgressFloor,
    BlockedSegmentHeadroomOrRepresentability,
    AccountBSettlementCannotProgress,
    BIndexHeadroomExhausted,
    ActiveBankruptCloseCannotProgress,
    ExplicitLossOrDustAuditOverflow,
    OracleOrTargetUnavailableByAuthenticatedPolicy,
    CounterOrEpochOverflowDeclaredRecovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvenanceHeaderV13 {
    pub market_group_id: [u8; 32],
    pub portfolio_account_id: [u8; 32],
    pub owner: [u8; 32],
    pub version: u16,
    pub layout_discriminator: u16,
}

impl ProvenanceHeaderV13 {
    pub const fn new(
        market_group_id: [u8; 32],
        portfolio_account_id: [u8; 32],
        owner: [u8; 32],
    ) -> Self {
        Self {
            market_group_id,
            portfolio_account_id,
            owner,
            version: V13_ACCOUNT_VERSION,
            layout_discriminator: V13_LAYOUT_DISCRIMINATOR,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V13Config {
    pub max_portfolio_assets: u8,
    pub h_min: u64,
    pub h_max: u64,
    pub maintenance_margin_bps: u64,
    pub initial_margin_bps: u64,
    pub max_trading_fee_bps: u64,
    pub max_account_b_settlement_chunks: u64,
    pub max_bankrupt_close_chunks: u64,
    pub public_b_chunk_atoms: u128,
    pub permissionless_recovery_enabled: bool,
    pub stale_certificate_penalty_enabled: bool,
    pub full_refresh_required_for_favorable_actions: bool,
    pub public_liveness_profile_crank_forward: bool,
}

impl V13Config {
    pub const fn public_user_fund(max_portfolio_assets: u8, h_min: u64, h_max: u64) -> Self {
        Self {
            max_portfolio_assets,
            h_min,
            h_max,
            maintenance_margin_bps: 10_000,
            initial_margin_bps: 10_000,
            max_trading_fee_bps: 0,
            max_account_b_settlement_chunks: 1,
            max_bankrupt_close_chunks: 1,
            public_b_chunk_atoms: MAX_VAULT_TVL,
            permissionless_recovery_enabled: true,
            stale_certificate_penalty_enabled: true,
            full_refresh_required_for_favorable_actions: true,
            public_liveness_profile_crank_forward: true,
        }
    }

    pub fn validate_public_user_fund(&self) -> V13Result<()> {
        if self.max_portfolio_assets == 0
            || self.max_portfolio_assets as usize > V13_MAX_PORTFOLIO_ASSETS_N
        {
            return Err(V13Error::InvalidConfig);
        }
        if self.h_max == 0 || self.h_min > self.h_max {
            return Err(V13Error::InvalidConfig);
        }
        if self.maintenance_margin_bps > self.initial_margin_bps
            || self.initial_margin_bps > 10_000
            || self.max_trading_fee_bps > 10_000
            || self.max_account_b_settlement_chunks == 0
            || self.max_bankrupt_close_chunks == 0
            || self.public_b_chunk_atoms == 0
        {
            return Err(V13Error::InvalidConfig);
        }
        if !self.permissionless_recovery_enabled
            || !self.stale_certificate_penalty_enabled
            || !self.full_refresh_required_for_favorable_actions
            || !self.public_liveness_profile_crank_forward
        {
            return Err(V13Error::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetStateV13 {
    pub raw_oracle_target_price: u64,
    pub effective_price: u64,
    pub fund_px_last: u64,
    pub a_long: u128,
    pub a_short: u128,
    pub k_long: i128,
    pub k_short: i128,
    pub f_long_num: i128,
    pub f_short_num: i128,
    pub b_long_num: u128,
    pub b_short_num: u128,
    pub b_epoch_start_long_num: u128,
    pub b_epoch_start_short_num: u128,
    pub oi_eff_long_q: u128,
    pub oi_eff_short_q: u128,
    pub stored_pos_count_long: u64,
    pub stored_pos_count_short: u64,
    pub stale_account_count_long: u64,
    pub stale_account_count_short: u64,
    pub loss_weight_sum_long: u128,
    pub loss_weight_sum_short: u128,
    pub social_loss_remainder_long_num: u128,
    pub social_loss_remainder_short_num: u128,
    pub social_loss_dust_long_num: u128,
    pub social_loss_dust_short_num: u128,
    pub explicit_unallocated_loss_long: u128,
    pub explicit_unallocated_loss_short: u128,
    pub epoch_long: u64,
    pub epoch_short: u64,
    pub mode_long: SideModeV13,
    pub mode_short: SideModeV13,
}

impl Default for AssetStateV13 {
    fn default() -> Self {
        Self {
            raw_oracle_target_price: 1,
            effective_price: 1,
            fund_px_last: 1,
            a_long: ADL_ONE,
            a_short: ADL_ONE,
            k_long: 0,
            k_short: 0,
            f_long_num: 0,
            f_short_num: 0,
            b_long_num: 0,
            b_short_num: 0,
            b_epoch_start_long_num: 0,
            b_epoch_start_short_num: 0,
            oi_eff_long_q: 0,
            oi_eff_short_q: 0,
            stored_pos_count_long: 0,
            stored_pos_count_short: 0,
            stale_account_count_long: 0,
            stale_account_count_short: 0,
            loss_weight_sum_long: 0,
            loss_weight_sum_short: 0,
            social_loss_remainder_long_num: 0,
            social_loss_remainder_short_num: 0,
            social_loss_dust_long_num: 0,
            social_loss_dust_short_num: 0,
            explicit_unallocated_loss_long: 0,
            explicit_unallocated_loss_short: 0,
            epoch_long: 0,
            epoch_short: 0,
            mode_long: SideModeV13::Normal,
            mode_short: SideModeV13::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortfolioLegV13 {
    pub active: bool,
    pub side: SideV13,
    pub basis_pos_q: i128,
    pub a_basis: u128,
    pub k_snap: i128,
    pub f_snap: i128,
    pub epoch_snap: u64,
    pub loss_weight: u128,
    pub b_snap: u128,
    pub b_rem: u128,
    pub b_epoch_snap: u64,
    pub b_stale: bool,
    pub stale: bool,
}

impl PortfolioLegV13 {
    pub const EMPTY: Self = Self {
        active: false,
        side: SideV13::Long,
        basis_pos_q: 0,
        a_basis: ADL_ONE,
        k_snap: 0,
        f_snap: 0,
        epoch_snap: 0,
        loss_weight: 0,
        b_snap: 0,
        b_rem: 0,
        b_epoch_snap: 0,
        b_stale: false,
        stale: false,
    };
}

impl Default for PortfolioLegV13 {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HealthCertV13 {
    pub certified_equity: i128,
    pub certified_initial_req: u128,
    pub certified_maintenance_req: u128,
    pub certified_liq_deficit: u128,
    pub certified_worst_case_loss: u128,
    pub cert_oracle_epoch: u64,
    pub cert_funding_epoch: u64,
    pub cert_risk_epoch: u64,
    pub active_bitmap_at_cert: u32,
    pub valid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortfolioAccountV13 {
    pub provenance_header: ProvenanceHeaderV13,
    pub owner: [u8; 32],
    pub capital: u128,
    pub pnl: i128,
    pub reserved_pnl: u128,
    pub fee_credits: i128,
    pub active_bitmap: u32,
    pub legs: [PortfolioLegV13; V13_MAX_PORTFOLIO_ASSETS_N],
    pub health_cert: HealthCertV13,
    pub stale_state: bool,
    pub b_stale_state: bool,
    pub rebalance_lock: bool,
    pub liquidation_lock: bool,
}

impl PortfolioAccountV13 {
    pub const fn empty(header: ProvenanceHeaderV13) -> Self {
        Self {
            provenance_header: header,
            owner: header.owner,
            capital: 0,
            pnl: 0,
            reserved_pnl: 0,
            fee_credits: 0,
            active_bitmap: 0,
            legs: [PortfolioLegV13::EMPTY; V13_MAX_PORTFOLIO_ASSETS_N],
            health_cert: HealthCertV13 {
                certified_equity: 0,
                certified_initial_req: 0,
                certified_maintenance_req: 0,
                certified_liq_deficit: 0,
                certified_worst_case_loss: 0,
                cert_oracle_epoch: 0,
                cert_funding_epoch: 0,
                cert_risk_epoch: 0,
                active_bitmap_at_cert: 0,
                valid: false,
            },
            stale_state: false,
            b_stale_state: false,
            rebalance_lock: false,
            liquidation_lock: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketGroupV13 {
    pub market_group_id: [u8; 32],
    pub config: V13Config,
    pub vault: u128,
    pub insurance: u128,
    pub c_tot: u128,
    pub pnl_pos_tot: u128,
    pub pnl_matured_pos_tot: u128,
    pub materialized_portfolio_count: u64,
    pub stale_certificate_count: u64,
    pub b_stale_account_count: u64,
    pub negative_pnl_account_count: u64,
    pub risk_epoch: u64,
    pub oracle_epoch: u64,
    pub funding_epoch: u64,
    pub slot_last: u64,
    pub current_slot: u64,
    pub assets: [AssetStateV13; V13_MAX_PORTFOLIO_ASSETS_N],
    pub bankruptcy_hlock_active: bool,
    pub threshold_stress_active: bool,
    pub active_bankrupt_close_present: bool,
    pub loss_stale_active: bool,
    pub recovery_reason: Option<PermissionlessRecoveryReasonV13>,
}

impl MarketGroupV13 {
    pub fn new(market_group_id: [u8; 32], config: V13Config) -> V13Result<Self> {
        config.validate_public_user_fund()?;
        Ok(Self {
            market_group_id,
            config,
            vault: 0,
            insurance: 0,
            c_tot: 0,
            pnl_pos_tot: 0,
            pnl_matured_pos_tot: 0,
            materialized_portfolio_count: 0,
            stale_certificate_count: 0,
            b_stale_account_count: 0,
            negative_pnl_account_count: 0,
            risk_epoch: 0,
            oracle_epoch: 0,
            funding_epoch: 0,
            slot_last: 0,
            current_slot: 0,
            assets: [AssetStateV13::default(); V13_MAX_PORTFOLIO_ASSETS_N],
            bankruptcy_hlock_active: false,
            threshold_stress_active: false,
            active_bankrupt_close_present: false,
            loss_stale_active: false,
            recovery_reason: None,
        })
    }

    pub fn validate_portfolio_account_provenance(
        &self,
        account: &PortfolioAccountV13,
    ) -> V13Result<()> {
        let h = account.provenance_header;
        if h.market_group_id != self.market_group_id
            || h.owner != account.owner
            || h.version != V13_ACCOUNT_VERSION
            || h.layout_discriminator != V13_LAYOUT_DISCRIMINATOR
        {
            return Err(V13Error::ProvenanceMismatch);
        }
        Ok(())
    }

    pub fn validate_account_shape(&self, account: &PortfolioAccountV13) -> V13Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        validate_non_min_i128(account.pnl)?;
        validate_fee_credits(account.fee_credits)?;
        if account.reserved_pnl > account.pnl.max(0) as u128 {
            return Err(V13Error::InvalidLeg);
        }

        let n = self.config.max_portfolio_assets as usize;
        for i in 0..V13_MAX_PORTFOLIO_ASSETS_N {
            let bit = ((account.active_bitmap >> i) & 1) != 0;
            let leg = account.legs[i];
            if i >= n {
                if bit || leg != PortfolioLegV13::default() {
                    return Err(V13Error::HiddenLeg);
                }
                continue;
            }

            if bit != leg.active {
                return Err(V13Error::HiddenLeg);
            }
            if !leg.active {
                if leg != PortfolioLegV13::EMPTY {
                    return Err(V13Error::HiddenLeg);
                }
            } else {
                validate_active_leg(leg)?;
            }
        }
        Ok(())
    }

    pub fn create_portfolio_account(&mut self, account: &PortfolioAccountV13) -> V13Result<()> {
        self.validate_account_shape(account)?;
        self.materialized_portfolio_count = self
            .materialized_portfolio_count
            .checked_add(1)
            .ok_or(V13Error::CounterOverflow)?;
        Ok(())
    }

    pub fn close_portfolio_account(&mut self, account: &PortfolioAccountV13) -> V13Result<()> {
        self.validate_account_shape(account)?;
        if account.active_bitmap != 0
            || account.capital != 0
            || account.pnl != 0
            || account.reserved_pnl != 0
            || account.fee_credits != 0
            || account.stale_state
            || account.b_stale_state
        {
            return Err(V13Error::LockActive);
        }
        self.materialized_portfolio_count = self
            .materialized_portfolio_count
            .checked_sub(1)
            .ok_or(V13Error::CounterUnderflow)?;
        Ok(())
    }

    pub fn mark_account_stale(&mut self, account: &mut PortfolioAccountV13) -> V13Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if !account.stale_state {
            account.stale_state = true;
            account.health_cert.valid = false;
            self.stale_certificate_count = self
                .stale_certificate_count
                .checked_add(1)
                .ok_or(V13Error::CounterOverflow)?;
        }
        Ok(())
    }

    pub fn clear_account_stale(&mut self, account: &mut PortfolioAccountV13) -> V13Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if account.stale_state {
            account.stale_state = false;
            self.stale_certificate_count = self
                .stale_certificate_count
                .checked_sub(1)
                .ok_or(V13Error::CounterUnderflow)?;
        }
        Ok(())
    }

    pub fn mark_account_b_stale(&mut self, account: &mut PortfolioAccountV13) -> V13Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if !account.b_stale_state {
            account.b_stale_state = true;
            account.health_cert.valid = false;
            self.b_stale_account_count = self
                .b_stale_account_count
                .checked_add(1)
                .ok_or(V13Error::CounterOverflow)?;
        }
        Ok(())
    }

    pub fn clear_account_b_stale(&mut self, account: &mut PortfolioAccountV13) -> V13Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if has_b_stale_leg(account) {
            return Err(V13Error::BStale);
        }
        if account.b_stale_state {
            account.b_stale_state = false;
            self.b_stale_account_count = self
                .b_stale_account_count
                .checked_sub(1)
                .ok_or(V13Error::CounterUnderflow)?;
        }
        Ok(())
    }

    pub fn attach_leg(
        &mut self,
        account: &mut PortfolioAccountV13,
        asset_index: usize,
        side: SideV13,
        basis_pos_q: i128,
    ) -> V13Result<()> {
        self.validate_portfolio_account_provenance(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V13Error::InvalidLeg);
        }
        if account.legs[asset_index].active || ((account.active_bitmap >> asset_index) & 1) != 0 {
            return Err(V13Error::InvalidLeg);
        }
        validate_basis(basis_pos_q)?;

        let asset = self.assets[asset_index];
        let (a_basis, k_snap, f_snap, b_snap, epoch_snap) = match side {
            SideV13::Long => (
                asset.a_long,
                asset.k_long,
                asset.f_long_num,
                asset.b_long_num,
                asset.epoch_long,
            ),
            SideV13::Short => (
                asset.a_short,
                asset.k_short,
                asset.f_short_num,
                asset.b_short_num,
                asset.epoch_short,
            ),
        };
        if !(MIN_A_SIDE..=ADL_ONE).contains(&a_basis) {
            return Err(V13Error::InvalidLeg);
        }
        let loss_weight = loss_weight_for_basis(basis_pos_q.unsigned_abs(), a_basis)?;
        if loss_weight == 0 {
            return Err(V13Error::InvalidLeg);
        }

        let asset = &mut self.assets[asset_index];
        match side {
            SideV13::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_add(1)
                    .ok_or(V13Error::CounterOverflow)?;
                asset.oi_eff_long_q = asset
                    .oi_eff_long_q
                    .checked_add(basis_pos_q.unsigned_abs())
                    .ok_or(V13Error::ArithmeticOverflow)?;
                asset.loss_weight_sum_long = asset
                    .loss_weight_sum_long
                    .checked_add(loss_weight)
                    .ok_or(V13Error::ArithmeticOverflow)?;
            }
            SideV13::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_add(1)
                    .ok_or(V13Error::CounterOverflow)?;
                asset.oi_eff_short_q = asset
                    .oi_eff_short_q
                    .checked_add(basis_pos_q.unsigned_abs())
                    .ok_or(V13Error::ArithmeticOverflow)?;
                asset.loss_weight_sum_short = asset
                    .loss_weight_sum_short
                    .checked_add(loss_weight)
                    .ok_or(V13Error::ArithmeticOverflow)?;
            }
        }
        account.legs[asset_index] = PortfolioLegV13 {
            active: true,
            side,
            basis_pos_q,
            a_basis,
            k_snap,
            f_snap,
            epoch_snap,
            loss_weight,
            b_snap,
            b_rem: 0,
            b_epoch_snap: epoch_snap,
            b_stale: false,
            stale: false,
        };
        account.active_bitmap |= 1u32 << asset_index;
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    pub fn clear_leg(
        &mut self,
        account: &mut PortfolioAccountV13,
        asset_index: usize,
    ) -> V13Result<()> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V13Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active || leg.b_stale || leg.stale {
            return Err(V13Error::InvalidLeg);
        }
        let asset = &mut self.assets[asset_index];
        match leg.side {
            SideV13::Long => {
                asset.stored_pos_count_long = asset
                    .stored_pos_count_long
                    .checked_sub(1)
                    .ok_or(V13Error::CounterUnderflow)?;
                asset.oi_eff_long_q = asset
                    .oi_eff_long_q
                    .checked_sub(leg.basis_pos_q.unsigned_abs())
                    .ok_or(V13Error::CounterUnderflow)?;
                asset.loss_weight_sum_long = asset
                    .loss_weight_sum_long
                    .checked_sub(leg.loss_weight)
                    .ok_or(V13Error::CounterUnderflow)?;
            }
            SideV13::Short => {
                asset.stored_pos_count_short = asset
                    .stored_pos_count_short
                    .checked_sub(1)
                    .ok_or(V13Error::CounterUnderflow)?;
                asset.oi_eff_short_q = asset
                    .oi_eff_short_q
                    .checked_sub(leg.basis_pos_q.unsigned_abs())
                    .ok_or(V13Error::CounterUnderflow)?;
                asset.loss_weight_sum_short = asset
                    .loss_weight_sum_short
                    .checked_sub(leg.loss_weight)
                    .ok_or(V13Error::CounterUnderflow)?;
            }
        }
        account.legs[asset_index] = PortfolioLegV13::EMPTY;
        account.active_bitmap &= !(1u32 << asset_index);
        account.health_cert.valid = false;
        self.validate_account_shape(account)
    }

    pub fn mark_leg_b_stale(
        &mut self,
        account: &mut PortfolioAccountV13,
        asset_index: usize,
    ) -> V13Result<()> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize
            || !account.legs[asset_index].active
        {
            return Err(V13Error::InvalidLeg);
        }
        account.legs[asset_index].b_stale = true;
        self.mark_account_b_stale(account)
    }

    pub fn h_lock_lane(
        &self,
        account: Option<&PortfolioAccountV13>,
        instruction_bankruptcy_candidate: bool,
    ) -> V13Result<HLockLaneV13> {
        if let Some(account) = account {
            self.validate_portfolio_account_provenance(account)?;
            if account.stale_state || account.b_stale_state {
                return Ok(HLockLaneV13::HMax);
            }
        }

        if self.threshold_stress_active
            || self.bankruptcy_hlock_active
            || instruction_bankruptcy_candidate
            || self.loss_stale_active
            || self.active_bankrupt_close_present
        {
            return Ok(HLockLaneV13::HMax);
        }

        Ok(HLockLaneV13::HMin)
    }

    pub fn select_h_lock(
        &self,
        account: Option<&PortfolioAccountV13>,
        instruction_bankruptcy_candidate: bool,
    ) -> V13Result<u64> {
        match self.h_lock_lane(account, instruction_bankruptcy_candidate)? {
            HLockLaneV13::HMin => Ok(self.config.h_min),
            HLockLaneV13::HMax => Ok(self.config.h_max),
        }
    }

    pub fn full_account_refresh(
        &mut self,
        account: &mut PortfolioAccountV13,
        effective_prices: &[u64; V13_MAX_PORTFOLIO_ASSETS_N],
    ) -> V13Result<HealthCertV13> {
        self.validate_account_shape(account)?;
        if account.stale_state || account.b_stale_state {
            return Err(if account.b_stale_state {
                V13Error::BStale
            } else {
                V13Error::Stale
            });
        }

        let n = self.config.max_portfolio_assets as usize;
        let mut initial_req = 0u128;
        let mut maintenance_req = 0u128;
        let mut worst_case_loss = 0u128;
        for i in 0..n {
            if !account.legs[i].active {
                continue;
            }
            let price = effective_prices[i];
            if price == 0 || price > MAX_ORACLE_PRICE {
                return Err(V13Error::InvalidConfig);
            }
            let risk_notional =
                risk_notional_ceil(account.legs[i].basis_pos_q.unsigned_abs(), price)?;
            initial_req = initial_req
                .checked_add(risk_notional)
                .ok_or(V13Error::ArithmeticOverflow)?;
            maintenance_req = maintenance_req
                .checked_add(risk_notional)
                .ok_or(V13Error::ArithmeticOverflow)?;
            worst_case_loss = worst_case_loss
                .checked_add(risk_notional)
                .ok_or(V13Error::ArithmeticOverflow)?;
        }

        let equity = account_equity(account)?;
        let certified_liq_deficit = if equity < 0 {
            equity.unsigned_abs()
        } else {
            let e = equity as u128;
            maintenance_req.saturating_sub(e)
        };
        let cert = HealthCertV13 {
            certified_equity: equity,
            certified_initial_req: initial_req,
            certified_maintenance_req: maintenance_req,
            certified_liq_deficit,
            certified_worst_case_loss: worst_case_loss,
            cert_oracle_epoch: self.oracle_epoch,
            cert_funding_epoch: self.funding_epoch,
            cert_risk_epoch: self.risk_epoch,
            active_bitmap_at_cert: account.active_bitmap,
            valid: true,
        };
        account.health_cert = cert;
        Ok(cert)
    }

    pub fn ensure_favorable_action_allowed(
        &self,
        account: &PortfolioAccountV13,
    ) -> V13Result<()> {
        self.validate_account_shape(account)?;
        if self.h_lock_lane(Some(account), false)? == HLockLaneV13::HMax {
            return Err(V13Error::LockActive);
        }
        if !account.health_cert.valid
            || account.health_cert.cert_oracle_epoch != self.oracle_epoch
            || account.health_cert.cert_funding_epoch != self.funding_epoch
            || account.health_cert.cert_risk_epoch != self.risk_epoch
            || account.health_cert.active_bitmap_at_cert != account.active_bitmap
        {
            return Err(V13Error::Stale);
        }
        Ok(())
    }

    pub fn account_b_settlement_chunk(
        &self,
        account: &PortfolioAccountV13,
        asset_index: usize,
        endpoint_delta_budget: u128,
    ) -> V13Result<AccountBSettlementChunkV13> {
        self.validate_account_shape(account)?;
        if asset_index >= self.config.max_portfolio_assets as usize {
            return Err(V13Error::InvalidLeg);
        }
        let leg = account.legs[asset_index];
        if !leg.active {
            return Err(V13Error::InvalidLeg);
        }
        let target = self.b_target_for_leg(asset_index, leg)?;
        if target < leg.b_snap {
            return Err(V13Error::RecoveryRequired);
        }
        let b_remaining = target - leg.b_snap;
        if b_remaining == 0 {
            return Ok(AccountBSettlementChunkV13 {
                delta_b: 0,
                loss: 0,
                new_remainder: leg.b_rem,
                remaining_after: 0,
            });
        }
        if leg.loss_weight == 0 || endpoint_delta_budget == 0 {
            return Err(V13Error::RecoveryRequired);
        }

        let limit = self.config.public_b_chunk_atoms;
        let max_num = limit
            .checked_add(1)
            .and_then(|v| v.checked_mul(SOCIAL_LOSS_DEN))
            .and_then(|v| v.checked_sub(1))
            .ok_or(V13Error::ArithmeticOverflow)?;
        if leg.b_rem > max_num {
            return Err(V13Error::RecoveryRequired);
        }
        let max_delta_by_loss = (max_num - leg.b_rem) / leg.loss_weight;
        let delta_b = b_remaining
            .min(max_delta_by_loss)
            .min(endpoint_delta_budget);
        if delta_b == 0 {
            return Err(V13Error::RecoveryRequired);
        }
        let num = leg
            .loss_weight
            .checked_mul(delta_b)
            .and_then(|v| v.checked_add(leg.b_rem))
            .ok_or(V13Error::ArithmeticOverflow)?;
        let loss = num / SOCIAL_LOSS_DEN;
        let new_remainder = num % SOCIAL_LOSS_DEN;
        Ok(AccountBSettlementChunkV13 {
            delta_b,
            loss,
            new_remainder,
            remaining_after: b_remaining - delta_b,
        })
    }

    pub fn settle_account_b_chunk(
        &mut self,
        account: &mut PortfolioAccountV13,
        asset_index: usize,
        endpoint_delta_budget: u128,
    ) -> V13Result<AccountBSettlementChunkV13> {
        let chunk = self.account_b_settlement_chunk(account, asset_index, endpoint_delta_budget)?;
        if chunk.delta_b == 0 {
            if !has_b_stale_leg(account) {
                self.clear_account_b_stale(account)?;
            }
            return Ok(chunk);
        }
        let old_pnl = account.pnl;
        let loss_i128 = i128::try_from(chunk.loss).map_err(|_| V13Error::ArithmeticOverflow)?;
        let new_pnl = old_pnl
            .checked_sub(loss_i128)
            .ok_or(V13Error::ArithmeticOverflow)?;

        {
            let leg = &mut account.legs[asset_index];
            leg.b_snap = leg
                .b_snap
                .checked_add(chunk.delta_b)
                .ok_or(V13Error::ArithmeticOverflow)?;
            leg.b_rem = chunk.new_remainder;
            leg.b_stale = chunk.remaining_after != 0;
        }
        self.set_account_pnl(account, new_pnl)?;
        if !has_b_stale_leg(account) {
            self.clear_account_b_stale(account)?;
        }
        account.health_cert.valid = false;
        self.validate_account_shape(account)?;
        Ok(chunk)
    }

    pub fn risk_score(&self, account: &PortfolioAccountV13) -> V13Result<RiskScoreV13> {
        self.validate_account_shape(account)?;
        if !account.health_cert.valid {
            return Err(V13Error::Stale);
        }
        Ok(RiskScoreV13 {
            certified_liq_deficit: account.health_cert.certified_liq_deficit,
            unsettled_b_loss_bound: account_b_loss_bound(account)?,
            stale_loss_bound: if account.stale_state { 1 } else { 0 },
            gross_risk_notional: account.health_cert.certified_worst_case_loss,
            active_leg_count: account.active_bitmap.count_ones(),
        })
    }

    pub fn validate_liquidation_progress(
        &self,
        before: &PortfolioAccountV13,
        after: &PortfolioAccountV13,
    ) -> V13Result<()> {
        let before_score = self.risk_score(before)?;
        let after_score = self.risk_score(after)?;
        if after_score.strictly_reduces_from(before_score)
            || after_score.certified_liq_deficit < before_score.certified_liq_deficit
        {
            Ok(())
        } else {
            Err(V13Error::NonProgress)
        }
    }

    pub fn declare_permissionless_recovery(
        &mut self,
        reason: PermissionlessRecoveryReasonV13,
    ) -> V13Result<PermissionlessProgressOutcomeV13> {
        if !self.config.permissionless_recovery_enabled {
            return Err(V13Error::InvalidConfig);
        }
        self.recovery_reason = Some(reason);
        Ok(PermissionlessProgressOutcomeV13::RecoveryDeclared(reason))
    }

    pub fn assert_public_invariants(&self) -> V13Result<()> {
        if self.vault > MAX_VAULT_TVL {
            return Err(V13Error::InvalidConfig);
        }
        let senior = self
            .c_tot
            .checked_add(self.insurance)
            .ok_or(V13Error::ArithmeticOverflow)?;
        if self.c_tot > self.vault || self.insurance > self.vault || senior > self.vault {
            return Err(V13Error::InvalidConfig);
        }
        if self.pnl_matured_pos_tot > self.pnl_pos_tot {
            return Err(V13Error::InvalidConfig);
        }
        if self.slot_last > self.current_slot {
            return Err(V13Error::InvalidConfig);
        }
        for i in 0..self.config.max_portfolio_assets as usize {
            let asset = self.assets[i];
            if asset.effective_price == 0
                || asset.effective_price > MAX_ORACLE_PRICE
                || asset.raw_oracle_target_price == 0
                || asset.raw_oracle_target_price > MAX_ORACLE_PRICE
                || asset.fund_px_last == 0
                || asset.fund_px_last > MAX_ORACLE_PRICE
                || asset.oi_eff_long_q > crate::MAX_OI_SIDE_Q
                || asset.oi_eff_short_q > crate::MAX_OI_SIDE_Q
                || asset.loss_weight_sum_long > SOCIAL_LOSS_DEN
                || asset.loss_weight_sum_short > SOCIAL_LOSS_DEN
                || asset.social_loss_remainder_long_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_remainder_short_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_dust_long_num >= SOCIAL_LOSS_DEN
                || asset.social_loss_dust_short_num >= SOCIAL_LOSS_DEN
            {
                return Err(V13Error::InvalidConfig);
            }
        }
        Ok(())
    }

    fn b_target_for_leg(&self, asset_index: usize, leg: PortfolioLegV13) -> V13Result<u128> {
        let asset = self.assets[asset_index];
        let (current_b, epoch_start_b, side_epoch, mode) = match leg.side {
            SideV13::Long => (
                asset.b_long_num,
                asset.b_epoch_start_long_num,
                asset.epoch_long,
                asset.mode_long,
            ),
            SideV13::Short => (
                asset.b_short_num,
                asset.b_epoch_start_short_num,
                asset.epoch_short,
                asset.mode_short,
            ),
        };
        if leg.b_epoch_snap == side_epoch {
            Ok(current_b)
        } else if mode == SideModeV13::ResetPending
            && leg.b_epoch_snap.checked_add(1) == Some(side_epoch)
        {
            Ok(epoch_start_b)
        } else {
            Err(V13Error::InvalidLeg)
        }
    }

    fn set_account_pnl(
        &mut self,
        account: &mut PortfolioAccountV13,
        new_pnl: i128,
    ) -> V13Result<()> {
        validate_non_min_i128(new_pnl)?;
        let old_pos = account.pnl.max(0) as u128;
        let new_pos = new_pnl.max(0) as u128;
        if new_pos >= old_pos {
            self.pnl_pos_tot = self
                .pnl_pos_tot
                .checked_add(new_pos - old_pos)
                .ok_or(V13Error::ArithmeticOverflow)?;
        } else {
            self.pnl_pos_tot = self
                .pnl_pos_tot
                .checked_sub(old_pos - new_pos)
                .ok_or(V13Error::CounterUnderflow)?;
            self.pnl_matured_pos_tot = self.pnl_matured_pos_tot.min(self.pnl_pos_tot);
        }

        let old_negative = account.pnl < 0;
        let new_negative = new_pnl < 0;
        match (old_negative, new_negative) {
            (false, true) => {
                self.negative_pnl_account_count = self
                    .negative_pnl_account_count
                    .checked_add(1)
                    .ok_or(V13Error::CounterOverflow)?;
            }
            (true, false) => {
                self.negative_pnl_account_count = self
                    .negative_pnl_account_count
                    .checked_sub(1)
                    .ok_or(V13Error::CounterUnderflow)?;
            }
            _ => {}
        }
        account.pnl = new_pnl;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountBSettlementChunkV13 {
    pub delta_b: u128,
    pub loss: u128,
    pub new_remainder: u128,
    pub remaining_after: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RiskScoreV13 {
    pub certified_liq_deficit: u128,
    pub unsettled_b_loss_bound: u128,
    pub stale_loss_bound: u128,
    pub gross_risk_notional: u128,
    pub active_leg_count: u32,
}

impl RiskScoreV13 {
    pub fn strictly_reduces_from(self, before: Self) -> bool {
        self < before
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionlessProgressOutcomeV13 {
    AccountBChunk(AccountBSettlementChunkV13),
    RecoveryDeclared(PermissionlessRecoveryReasonV13),
}

pub fn risk_notional_ceil(abs_pos_q: u128, price: u64) -> V13Result<u128> {
    if abs_pos_q == 0 {
        return Ok(0);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(abs_pos_q),
        U256::from_u128(price as u128),
        U256::from_u128(POS_SCALE),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V13Error::ArithmeticOverflow)
}

pub fn account_equity(account: &PortfolioAccountV13) -> V13Result<i128> {
    validate_non_min_i128(account.pnl)?;
    validate_fee_credits(account.fee_credits)?;
    let capital = i128::try_from(account.capital).map_err(|_| V13Error::ArithmeticOverflow)?;
    let fee_debt =
        i128::try_from(fee_debt_u128(account)?).map_err(|_| V13Error::ArithmeticOverflow)?;
    capital
        .checked_add(account.pnl)
        .and_then(|v| v.checked_sub(fee_debt))
        .ok_or(V13Error::ArithmeticOverflow)
}

fn validate_non_min_i128(v: i128) -> V13Result<()> {
    if v == i128::MIN {
        return Err(V13Error::ArithmeticOverflow);
    }
    Ok(())
}

fn validate_fee_credits(v: i128) -> V13Result<()> {
    validate_non_min_i128(v)?;
    if v > 0 {
        return Err(V13Error::InvalidLeg);
    }
    Ok(())
}

fn fee_debt_u128(account: &PortfolioAccountV13) -> V13Result<u128> {
    validate_fee_credits(account.fee_credits)?;
    Ok(account.fee_credits.unsigned_abs())
}

fn validate_basis(basis_pos_q: i128) -> V13Result<()> {
    if basis_pos_q == 0
        || basis_pos_q == i128::MIN
        || basis_pos_q.unsigned_abs() > MAX_POSITION_ABS_Q
    {
        return Err(V13Error::InvalidLeg);
    }
    Ok(())
}

fn validate_active_leg(leg: PortfolioLegV13) -> V13Result<()> {
    validate_basis(leg.basis_pos_q)?;
    validate_non_min_i128(leg.k_snap)?;
    validate_non_min_i128(leg.f_snap)?;
    if !(MIN_A_SIDE..=ADL_ONE).contains(&leg.a_basis)
        || leg.loss_weight == 0
        || leg.loss_weight != loss_weight_for_basis(leg.basis_pos_q.unsigned_abs(), leg.a_basis)?
        || leg.b_rem >= SOCIAL_LOSS_DEN
        || leg.b_epoch_snap != leg.epoch_snap
    {
        return Err(V13Error::InvalidLeg);
    }
    Ok(())
}

fn loss_weight_for_basis(abs_basis_q: u128, a_basis: u128) -> V13Result<u128> {
    if a_basis == 0 {
        return Err(V13Error::InvalidLeg);
    }
    checked_mul_div_ceil_u256(
        U256::from_u128(abs_basis_q),
        U256::from_u128(SOCIAL_WEIGHT_SCALE),
        U256::from_u128(a_basis),
    )
    .and_then(|v| v.try_into_u128())
    .ok_or(V13Error::ArithmeticOverflow)
}

fn has_b_stale_leg(account: &PortfolioAccountV13) -> bool {
    account
        .legs
        .iter()
        .any(|leg| leg.active && leg.b_stale)
}

fn account_b_loss_bound(account: &PortfolioAccountV13) -> V13Result<u128> {
    let mut bound = 0u128;
    for leg in account.legs.iter() {
        if leg.active && leg.b_stale {
            bound = bound
                .checked_add(leg.loss_weight)
                .ok_or(V13Error::ArithmeticOverflow)?;
        }
    }
    Ok(bound)
}
