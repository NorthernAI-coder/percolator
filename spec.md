# Risk Engine Spec (Source of Truth) — v14.12.0 Global Cross Margin

**Design:** protected principal + junior profit claims + full global cross-margin account solvency + haircut-bounded positive support + deterministic support allocation + leg-attributed bankruptcy loss + domain-budgeted insurance + preemptible close ownership + durable close progress ledger + pending-domain-loss barriers + immutable drift anchor + close-drift reserves + market-side B domains.  
**Scope:** one Percolator market group for one quote-token vault, with up to `N` configured assets per `PortfolioAccount` and unbounded global account count.  
**Status:** normative source-of-truth draft. Terms **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

This revision supersedes v14.11.0. Its goal is:

```text
preserve global account solvency
without allowing global loss contagion
```

v14.12 keeps true global cross-margin account health: current conservative PnL from one leg MAY support losses and maintenance on another leg in the same `PortfolioAccount`. It does **not** create a global bad-debt pool. Bankruptcy residuals are attributed only to the asset-side exposure that generated them. Insurance is domain-budgeted or hard-capped protocol first-loss capital. Positive PnL used to cure bankruptcy loss is haircut-bounded by global junior solvency; when global junior claims are impaired, maintenance credit is also haircut-bounded. No leg-local paper profit may become unbooked senior value.

Every top-level instruction is atomic. Any failed precondition, checked arithmetic guard, missing authenticated proof, context-capacity overflow, non-progressing lock, lock-order violation, accrual/lock incompatibility, stale close snapshot, or conservative-failure condition MUST roll back every mutation performed by that instruction. Before commit, every successful top-level instruction MUST leave all global, asset, account, certificate, close-state, insurance, and attribution invariants true.

-------------------------------------------------------------------------------
0. Non-negotiable v14.12 requirements
-------------------------------------------------------------------------------

1. **Global account solvency:** each `PortfolioAccount` is evaluated as one portfolio. Current conservative PnL across all active legs may support account maintenance.
2. **No global loss contagion:** bankruptcy residual MUST be charged only to the asset-side loss domain whose exposure generated the residual.
3. **No double-socialized close progress:** any durable B booking, quantity ADL, insurance spend, support consumption, or explicit loss assignment MUST also durably advance close-local progress accounting in the same atomic step. Preemption/restart MUST resume from remaining loss, not recompute from the original gross loss.
4. **No orphaned durable chunks:** durable B/ADL/insurance/support progress MUST remain attributed to `(account, close_id, asset, side/domain)` until the account close finalizes or recovery reconciles it.
5. **No pending residual escape:** any unfinalized close ledger entry with `residual_remaining > 0` creates a pending-domain-loss barrier. Domain participants MUST NOT reduce/clear weight, withdraw domain-dependent positive credit, or otherwise escape the pending loss before it is B-booked, backed, or recovered.
6. **Immutable close lifecycle:** a `close_id` and its ledger entries persist across preemption, unwind, restart, and recovery until finalized. A restart MUST reuse the same `close_id`; opening a new close for the same account before reconciling the old one is forbidden.
7. **No close deadlock:** close ownership MUST be preemptible and globally ordered. A lower-priority close MUST fully unwind reversible staged state and release its domains before a higher-priority conflicting close proceeds. Hold-and-wait cycles are forbidden.
8. **Close lock scope is minimal and current-step only:** a close may reserve only domains currently required for its staged residual/B/insurance/OI/A mutations. It MUST NOT reserve speculative worst-case future domains for the entire close lifetime.
9. **Accrual is non-exclusive but close-bounded:** domain locks MUST NOT freeze oracle price/funding accrual for an asset. Accrual may advance global K/F/price/time state while close snapshots are recomputed or conservatively re-aged before use, but a bankrupt close must reserve bounded adverse close drift so it cannot chase a moving loss forever.
10. **Close drift is bounded:** after a bankrupt close starts, post-start adverse price/funding/K/F drift for the closing account MUST be covered by a precomputed close-drift reserve, recovered, or fail. A close MUST NOT rely on unbounded future cranks while its loss target grows.
11. **Immutable close drift anchor:** `drift_reference_slot`, `gross_loss_at_close_start`, and `max_close_slot` are set once at close start and MUST NOT move on recompute, restart, or preemption. Drift is always total adverse drift from `drift_reference_slot` to now.
12. **Positive support is globally haircut-bounded:** positive PnL used to cure losses, avoid residual booking, pay fees, withdraw, release, transfer, convert, or increase risk MUST be valued through conservative global junior solvency bounds, not at face value.
13. **Effective support burns face junior claims:** consuming haircut-valued positive support MUST burn or lock the corresponding face junior claim amount, `ceil(effective_support * g.den / g.num)`, and update account/global junior aggregates in the same atomic step.
14. **Junior impairment is fail-closed:** when `Residual < PNL_pos_bound_tot`, positive PnL is globally impaired. Maintenance, support, trade, withdrawal, fee-paying, and close lanes MUST use haircut-bounded positive credit until impairment clears.
15. **No stale close-snapshot use:** multi-instruction close vectors, support pools, and insurance allocations MUST be recomputed or conservatively re-aged before every continuation that consumes them.
16. **No insurance contagion by default:** one bad asset MUST NOT be able to drain all insurance unless that insurance is explicitly configured as protocol-owned global first-loss capital with hard per-domain caps and no user claim seniority.
17. **No zero-weight loss clearing:** a bankruptcy residual MUST NOT be cleared into non-claim audit state merely because the target domain has no current weight. If no eligible B weight or domain insurance exists, the affected domain/account MUST enter recovery unless protocol-owned capital fully backs the loss without violating senior invariants.
18. **No partial-socialization escape:** a leg residual MUST NOT be socialized while the same account still has eligible support or closable value that can cure it. Socialization is normally a terminal/account-close path.
19. **No market-wide close hostage:** one pathological close MUST NOT block unrelated accounts or unrelated asset-side domains.
20. **Protected principal is senior:** positive PnL is junior and haircut-limited. Junior claims MUST NOT outrank capital, insurance, or durable loss recognition.
21. **PnL support is not extractability:** positive unrealized PnL MAY support maintenance under conservative rules, but MUST NOT become withdrawable, releasable, transferable, fee-paying, loss-curing, or usable for risk increase unless every contributing leg is current and every positive-credit gate passes.
22. **Leg provenance is mandatory:** every PnL, support, loss, fee, stale penalty, close step, residual, insurance debit, B charge, quantity ADL, and close-progress entry MUST retain deterministic asset-side provenance.
23. **No caller-chosen loss domain:** liquidation order, support allocation, insurance allocation, and residual attribution MUST be deterministic and independent of caller ordering.
24. **Hedge-credit proof must be adversarial:** non-identical oracle-family offsets MUST assume configured adverse basis divergence, not correlation-as-safety.
25. **Dead-leg exit is mandatory:** public markets MUST give owners a bounded way to forfeit/detach a recovery-mode leg so unrelated collateral is not permanently hostage.
26. **Hints are discovery only:** omitted or stale positions MUST NOT improve account health.
27. **Full account refresh is bounded by `N`:** any user-favorable operation MUST refresh the full active portfolio first.
28. **Stale profitable legs fail closed:** stale, lagged, B-stale, loss-stale, locked, partial-refresh, or recovery-mode profitable legs provide zero or conservative haircut credit, never optimistic credit.
29. **Account-free equity-active accrual is forbidden** on exposed live markets unless the operation also commits bounded protective progress for an affected account or affected domain.
30. **No full-market atomic work:** public instructions MUST NOT scan all accounts or all opposing accounts.
31. **Crank-forward public markets:** any state that only a privileged actor can advance is non-compliant for public user-fund markets.
32. **Fees never outrank losses:** uncollectible fees are dropped or forgiven, never paid from insurance or socialized through B.
33. **Residual durability before clearing exposure:** basis, OI, PnL, and side weights for a bankrupt close MUST NOT be freed until residuals are durably booked or assigned only to fully backed explicit non-claim loss state.
34. **No ADL/finalization split:** quantity ADL, closing-account exposure clear, and close-progress ledger advancement MUST be atomic or protected by a non-preemptible finalization barrier. A close MUST NOT release domains after ADL while the closing account still appears open.
35. **No self-dealing extraction:** realizing positive PnL MUST NOT increase net withdrawable senior claims unless matching portfolio/counterparty losses and fees are durably recognized and settled.
36. **Canonical per-asset leg:** each account has at most one canonical leg per configured asset; a same-asset long/short pair MUST be represented as one net signed position, not two independent support sources.

-------------------------------------------------------------------------------
1. Units, bounds, and configuration
-------------------------------------------------------------------------------

Persistent economic quantities use `u128` or `i128`. Persistent signed fields MUST NOT equal `i128::MIN`. Transient products involving price, position, A/K/F/B, weights, fees, penalties, support allocation, residual attribution, insurance allocation, re-aging bounds, and remainders MUST use an exact domain at least 256 bits wide.

```text
POS_SCALE                    = 1_000_000
ADL_ONE                      = 1_000_000_000_000_000
FUNDING_DEN                  = 1_000_000_000
SOCIAL_WEIGHT_SCALE          = ADL_ONE
SOCIAL_LOSS_DEN              = 1_000_000_000_000_000_000_000
STRESS_CONSUMPTION_SCALE     = 1_000_000_000
MAX_BPS                      = 10_000
```

Every live, resolved, raw target, effective engine, recovery, and fallback price MUST satisfy:

```text
0 < price <= MAX_ORACLE_PRICE
```

```text
RiskNotional(asset, account) =
    0 if effective_pos_q == 0
    else ceil(abs(effective_pos_q) * conservative_effective_price / POS_SCALE)

trade_notional =
    floor(abs(size_q) * exec_price / POS_SCALE)
```

### 1.1 Hard bounds

```text
MAX_VAULT_TVL                         = 10_000_000_000_000_000
MAX_ORACLE_PRICE                      = 1_000_000_000_000
MAX_POSITION_ABS_Q_PER_ASSET          = 100_000_000_000_000
MAX_TRADE_SIZE_Q                      = MAX_POSITION_ABS_Q_PER_ASSET
MAX_OI_SIDE_Q_PER_ASSET               = 100_000_000_000_000
MAX_ACCOUNT_NOTIONAL_PER_ASSET        = 100_000_000_000_000_000_000
MAX_PORTFOLIO_ASSETS_N                = implementation/config bounded
MAX_PROTOCOL_FEE_ABS                  = 1_000_000_000_000_000_000_000_000_000_000_000_000
GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT   = 10_000
MAX_WARMUP_SLOTS                      = u64::MAX
MAX_RESOLVE_PRICE_DEVIATION_BPS       = 10_000
MIN_A_SIDE                            = 100_000_000_000_000
```

`N` MUST be small enough that full account refresh, global health computation, liquidation validation, close-vector re-aging, residual attribution, resolved close, and proof packing fit within target runtime limits. A public market with `N` too large for bounded full-account refresh is non-compliant and MUST reject initialization before user funds are accepted.

### 1.2 Immutable configuration

Initialization MUST validate:

```text
0 < cfg_min_nonzero_mm_req < cfg_min_nonzero_im_req
0 <= cfg_maintenance_bps <= cfg_initial_bps <= MAX_BPS
0 <= cfg_max_trading_fee_bps <= MAX_BPS
0 <= cfg_liquidation_fee_bps <= MAX_BPS
0 <= cfg_min_liquidation_abs <= cfg_liquidation_fee_cap <= MAX_PROTOCOL_FEE_ABS
0 <= cfg_h_min <= cfg_h_max <= MAX_WARMUP_SLOTS
cfg_h_max > 0
0 <= cfg_resolve_price_deviation_bps <= MAX_RESOLVE_PRICE_DEVIATION_BPS
0 < cfg_max_accrual_dt_slots
0 <= cfg_max_abs_funding_e9_per_slot <= GLOBAL_MAX_ABS_FUNDING_E9_PER_SLOT
0 < cfg_max_price_move_bps_per_slot
0 < initial_oracle_price(asset) <= MAX_ORACLE_PRICE for every configured asset
0 < cfg_max_portfolio_assets <= MAX_PORTFOLIO_ASSETS_N
for every asset side: cfg_max_active_weight_per_side <= SOCIAL_LOSS_DEN
```

Public user-fund markets MUST satisfy:

```text
cfg_margin_mode == FullGlobalCrossMargin
cfg_bankruptcy_mode == LegAttributedMarketSideB
cfg_positive_support_mode == GlobalHaircutBounded
cfg_insurance_mode in {DomainBudgeted, GlobalProtocolFirstLossWithCaps}
cfg_public_liveness_profile == CrankForward
cfg_permissionless_recovery_enabled == true
cfg_recovery_fallback_price_enabled == true
cfg_owner_dead_leg_forfeit_enabled == true
cfg_full_refresh_required_for_favorable_actions == true
cfg_stale_certificate_penalty_enabled == true
cfg_deterministic_portfolio_liquidation_enabled == true
cfg_close_state_scope == AccountLocalWithPreemptibleDomainLocks
cfg_close_conflict_policy == DeterministicPreemptivePriority
cfg_no_global_B_index == true
cfg_no_cross_asset_residual_socialization == true
cfg_public_b_chunk_atoms > 0
cfg_max_account_b_settlement_chunks > 0
cfg_max_bankrupt_close_chunks > 0
cfg_max_bankrupt_close_lifetime_slots > 0
cfg_close_drift_reserve_enabled == true
cfg_close_drift_anchor_mode == ImmutableReferenceSlot
cfg_close_progress_after_drift_positive == true
```

The immutable recovery fallback price policy MUST be deterministic, bounded, representable, and conservative against the owner of the stale leg. It MAY use last effective price only if configured at initialization and haircut/penalty rules make the fallback non-extractive.

If `cfg_insurance_mode == GlobalProtocolFirstLossWithCaps`, initialization MUST prove that global insurance is protocol-owned risk capital, not a senior user claim, and that immutable per-domain loss caps prevent one asset from consuming more than its configured share before recovery/drain-only mode.

`cfg_public_b_chunk_atoms` MUST be large enough to make bounded progress under the vault and B-headroom limits. No public caller may choose a smaller residual chunk than the engine-determined chunk.

Initialization MUST prove a close-progress envelope in exact arithmetic. The envelope is measured from the immutable `drift_reference_slot`, not from a recomputed working-plan snapshot:

```text
max_close_drift_loss(account, close_lifetime) =
    worst adverse price/funding/K/F/stale/thin-market movement
    over cfg_max_bankrupt_close_lifetime_slots
    for every allowed portfolio and close domain set

min_close_progress_per_continuation =
    minimum residual/insurance/B/recovery progress guaranteed by a valid crank
    net of chunking, representability, and domain budget constraints

require min_close_progress_per_continuation > max_adverse_drift_per_continuation
require cfg_max_bankrupt_close_chunks * min_close_progress_per_continuation
        covers max close residual plus max_close_drift_loss
```

If this cannot be proven for a market/configuration, the public market MUST reject initialization or lower bounds until the envelope holds.


### 1.3 Solvency and portfolio offset envelopes

For each asset, initialization MUST prove in exact wide arithmetic:

```text
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
cfg_min_funding_lifetime_slots >= cfg_max_accrual_dt_slots
ADL_ONE * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_min_funding_lifetime_slots <= i128::MAX
```

For every integer `1 <= X <= MAX_ACCOUNT_NOTIONAL_PER_ASSET`, initialization MUST prove:

```text
price_budget_bps      = cfg_max_price_move_bps_per_slot * cfg_max_accrual_dt_slots
funding_budget_num    = cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots * 10_000
loss_budget_num       = price_budget_bps * FUNDING_DEN + funding_budget_num
price_funding_loss_X  = ceil(X * loss_budget_num / (10_000 * FUNDING_DEN))
worst_liq_notional_X  = ceil(X * (10_000 + price_budget_bps) / 10_000)
liq_fee_raw_X         = ceil(worst_liq_notional_X * cfg_liquidation_fee_bps / 10_000)
liq_fee_X             = min(max(liq_fee_raw_X, cfg_min_liquidation_abs), cfg_liquidation_fee_cap)
mm_req_X              = max(floor(X * cfg_maintenance_bps / 10_000), cfg_min_nonzero_mm_req)
require price_funding_loss_X + liq_fee_X <= mm_req_X
```

For global cross-margin offsets, initialization MUST prove that all hedge buckets, concentration caps, stale penalties, thin-market penalties, and correlation offsets cannot reduce maintenance below the worst-case bounded loss envelope for any allowed portfolio.

Hedge buckets are classified as:

```text
SameUnderlyingExact
    Same canonical price source or deterministic 1:1 conversion with no independent depeg risk.

ExplicitFamilyWithGap
    Distinct but configured related assets. The proof MUST assume simultaneous adverse movement
    plus configured basis/depeg gap against the account.
```

For `ExplicitFamilyWithGap`, hedge credit MUST be no greater than the provable minimum residual risk reduction after applying:
- each leg's own bounded price/funding move,
- the configured adverse basis gap,
- stale/oracle uncertainty,
- thin-market penalty,
- liquidation cost,
- no benefit from historical correlation.

No production config may use correlation, covariance, or empirical co-movement as a safety assumption unless reduced to deterministic adverse-gap caps.

-------------------------------------------------------------------------------
2. State
-------------------------------------------------------------------------------

### 2.1 MarketGroup

```text
MarketGroup {
    V                         // quote vault balance
    I                         // total insurance capital
    C_tot                     // total senior capital
    PNL_pos_tot               // exact known positive junior claims
    PNL_pos_bound_tot         // conservative upper bound including stale/partial accounts
    PNL_matured_pos_tot
    materialized_portfolio_count_unbounded_counter

    risk_epoch
    oracle_epoch
    funding_epoch
    current_slot

    assets[0..N)
    domain_locks[(asset, side)]
    insurance_ledger
    close_progress_ledger
    pending_domain_loss_barriers[(asset, side)]
    global_stale_penalty_params
    mode in {Live, Resolved, Recovery}
}
```

`PNL_pos_bound_tot` MUST be used for haircut denominator and payout readiness whenever any account may be stale, partial, unresolved, or otherwise not exactly current. It MUST remain conservative until exact account refresh lowers it by authenticated mutation.

There is no single global `active_portfolio_close`. Close state is account-local; only affected asset-side domains are locked.

### 2.2 InsuranceLedger

```text
InsuranceLedger {
    total_available
    domain_budget[(asset, side)]
    domain_spent[(asset, side)]
    staged_by_close_id[(asset, side)] optional
    global_protocol_budget optional
    global_protocol_spent
}
```

Invariants:

```text
total_available <= I
sum(domain_budget - domain_spent - staged_domain_debits) + remaining_global_protocol_budget <= total_available
domain_spent <= domain_budget
staged insurance is reserved exactly once by close_id
uncollectible fees are never insurance-eligible
```

A domain may spend only its remaining domain budget plus any configured global protocol first-loss allowance still available under that domain's cap. If a domain budget is exhausted, remaining residual routes to that domain's B or recovery; it MUST NOT borrow insurance from unrelated domains.

### 2.2.1 CloseProgressLedger

```text
CloseProgressLedger {
    entries[(account_id, close_id, asset_id, side_or_domain)] {
        gross_loss_at_close_start
        drift_reference_slot
        max_close_slot
        support_consumed                 // effective support value applied to residual
        junior_face_burned             // face junior claim burned/locked for support
        insurance_spent
        b_loss_booked
        explicit_loss_assigned
        quantity_adl_applied_q
        drift_consumed
        residual_remaining
        finalized
    }
}
```

Invariants:

```text
residual_remaining =
    gross_loss_at_close_start
  + drift_consumed
  - support_consumed
  - insurance_spent
  - b_loss_booked
  - explicit_loss_assigned

gross_loss_at_close_start, drift_reference_slot, max_close_slot, and close_id are immutable after close start
drift_consumed is the monotone maximum conservative adverse drift from drift_reference_slot to now
support_consumed is effective support value; junior_face_burned is the face junior claim amount removed or locked
support_consumed <= floor(junior_face_burned * g.num / g.den) at the time support is consumed
all progress fields are monotone except residual_remaining, which may increase only when drift_consumed increases
durable B booking, support consumption, and quantity ADL MUST have exactly one matching ledger advance
a finalized entry cannot be mutated except by audit reconciliation that preserves totals
```

The ledger is close-local durable accounting. It is not a user claim and cannot create withdrawable value. It exists so preemption, restart, and recovery can resume from remaining residual instead of recomputing gross loss and double-booking already durable chunks. Recompute may refresh working plan snapshots, but it MUST NOT move the ledger's `drift_reference_slot`, `gross_loss_at_close_start`, or `max_close_slot`.

Any unfinalized ledger entry with `residual_remaining > 0` creates a `PendingDomainLossBarrier` for its bankruptcy domain. The barrier:
- blocks weight-reducing exits, leg clears, positive-credit withdrawals, and new attaches that would dilute or avoid the pending domain loss;
- does not block higher-priority liquidation/close/recovery progress in that domain;
- is released only when the residual is durably B-booked, covered by eligible domain/protocol backing, assigned to explicit fully-backed non-claim loss, or reconciled by recovery.

A preempted close may release domain locks, but it MUST NOT erase pending-domain-loss barriers for unbooked residual. This prevents participants from escaping a known pending residual while allowing higher-priority closes to progress.



### 2.3 Asset state

Each asset has independent time and price state.

```text
Asset {
    raw_oracle_target_price
    effective_price
    fund_px_last
    slot_last

    A_long, A_short
    K_long, K_short
    F_long_num, F_short_num

    B_long_num, B_short_num
    B_epoch_start_long_num, B_epoch_start_short_num
    K_epoch_start_long, K_epoch_start_short
    F_epoch_start_long_num, F_epoch_start_short_num
    A_epoch_start_long, A_epoch_start_short

    OI_eff_long, OI_eff_short
    stored_pos_count_long, stored_pos_count_short
    stale_account_count_long, stale_account_count_short

    loss_weight_sum_long, loss_weight_sum_short
    social_loss_remainder_long_num, social_loss_remainder_short_num
    social_loss_dust_long_num, social_loss_dust_short_num
    explicit_unallocated_loss_long, explicit_unallocated_loss_short

    epoch_long, epoch_short
    mode_long, mode_short in {Normal, DrainOnly, ResetPending}
}
```

B state is always per `(asset, side)`. There is no global B accumulator.

### 2.4 DomainLock and close ownership

```text
DomainLock {
    locked_by_close_id optional
    close_priority
    staged_residual
    staged_insurance_debit
    staged_b_booking
    phase
    last_progress_slot
    progress_nonce
}
```

A domain lock blocks only operations that would mutate or depend on that exact asset-side's B residual booking, quantity ADL, A-side scaling, OI, weights, staged residual, staged insurance, exposure clear, or positive-credit eligibility. It MUST NOT block unrelated accounts or unrelated domains.

A domain lock MUST NOT block authenticated asset-wide price/funding accrual. K/F accrual is append-only global market state; locked close snapshots are not allowed to rely on old K/F/price/time values and MUST be recomputed or conservatively re-aged before use.

Close ownership discipline:

```text
ClosePriority =
    (
        higher liquidating deficit first,
        then higher total_abs_risk_notional,
        then older snapshot_slot,
        then deterministic close_id
    )
```

A close may reserve only domains currently required for:
- staged residual booking,
- staged insurance,
- B booking,
- quantity ADL,
- A/OI/weight mutation,
- exposure clear,
- deterministic current-step close progress.

Worst-case future domains from speculative re-aging MUST NOT be reserved up front.

Conflict resolution is deterministic and preemptive:

```text
if incoming_close_priority > existing_close_priority:
    existing close MUST fully unwind reversible staged state,
    preserve durable close ledger entries and pending-domain-loss barriers,
    release all held domains,
    and restart from refreshed state under the same close_id before further progress

else:
    incoming close makes no mutation and returns conflict/progress-required
```

Unwind MUST:
- reverse unstabilized staged insurance,
- reverse unstabilized staged residual metadata,
- preserve already durable B booking,
- preserve already durable support consumption and junior_face_burned,
- preserve already durable quantity ADL,
- preserve pending-domain-loss barriers for unbooked residual,
- preserve all senior solvency invariants,
- leave the account conservatively refreshable under the same close_id.

Hold-and-wait across close continuations is forbidden. A close continuation MUST NOT hold one domain while waiting to acquire another domain owned by a lower-priority close. The lower-priority close is preempted instead.

A domain lock may be created only by an instruction that also commits deterministic progress. Every later successful continuation MUST increase `progress_nonce`, reduce staged residual, advance phase, release a fully-settled domain, or route to recovery. Equal-state lock churn MUST revert.

### 2.5 PortfolioAccount

```text
PortfolioAccount {
    owner
    market_group_id
    config_hash_at_open

    C_i
    PNL_i
    R_i
    fee_credits_i <= 0 and != i128::MIN

    active_bitmap              // one bit per configured asset
    legs[0..N)                 // one canonical signed net leg per asset

    health_cert
    stale_state
    positive_credit_lock
    rebalance_lock
    liquidation_lock
    portfolio_close_state optional {
        close_id
        required_domain_set
        snapshot_slot
        drift_reference_slot
        max_close_slot
        close_drift_reserve
        drift_consumed
        progress_measure
        close_progress_ledger_keys[0..bounded]
    }
}
```

Each configured asset has at most one active signed leg. Same-asset opposite exposure MUST net into the single leg before health, support, hedge, liquidation, or residual attribution. Implementations MUST NOT represent simultaneous long and short legs for the same account/asset as independent PnL/support sources.

A flat inactive leg MUST be canonical:

```text
basis_pos_q = 0
a_basis = ADL_ONE
k_snap = f_snap = 0
epoch_snap = 0
loss_weight = b_snap = b_rem = b_epoch_snap = 0
```

A nonzero leg MUST satisfy:

```text
basis_pos_q != 0
a_basis >= MIN_A_SIDE
loss_weight = ceil(abs(basis_pos_q) * SOCIAL_WEIGHT_SCALE / a_basis) > 0
b_rem < SOCIAL_LOSS_DEN
b_epoch_snap == epoch_snap
b_epoch_snap == side_epoch or (side_mode == ResetPending and b_epoch_snap + 1 == side_epoch)
```

### 2.6 LegRefresh

Each active leg has a transient refresh record:

```text
LegRefresh {
    asset_id
    side
    signed_pos_q
    conservative_pnl
    positive_pnl_current
    negative_pnl_current
    positive_support_value       // globally haircut-bounded positive value
    mm_req
    im_req
    stale_penalty
    thin_market_penalty
    b_stale
    loss_stale
    oracle_current
    funding_current
    domain_locked
    eligible_for_maintenance_positive_credit
    eligible_for_positive_credit
    eligible_for_withdraw_credit
    bankruptcy_domain = (asset_id, opposing_side)
}
```

A bankrupt long residual is charged to that asset's short-side B domain. A bankrupt short residual is charged to that asset's long-side B domain.

### 2.7 HealthCert

```text
HealthCert {
    certified_equity_maint
    certified_equity_initial
    certified_equity_trade
    certified_equity_withdraw
    certified_initial_req
    certified_maintenance_req
    certified_liq_deficit
    certified_worst_case_loss

    cert_market_group_id
    cert_config_hash
    cert_oracle_epoch
    cert_funding_epoch
    cert_risk_epoch
    cert_asset_slot_vector_hash
    cert_effective_price_vector_hash
    active_bitmap_at_cert
    stale_penalty_accumulator
    positive_credit_mask
}
```

Certificate invariants:

```text
certified_equity_maint    <= exact_conservative_maintenance_equity(account)
certified_equity_initial  <= exact_conservative_initial_equity(account)
certified_equity_trade    <= exact_conservative_trade_equity(account)
certified_equity_withdraw <= exact_conservative_withdraw_equity(account)
certified_initial_req     >= exact_initial_requirement(account)
certified_maintenance_req >= exact_maintenance_requirement(account)
certified_liq_deficit     >= exact_liquidation_deficit(account)
```

If exactness is uncertain, the engine MUST round against the account.

-------------------------------------------------------------------------------
3. Global invariants
-------------------------------------------------------------------------------

```text
C_tot <= V <= MAX_VAULT_TVL
I <= V
V >= C_tot + I
PNL_matured_pos_tot <= PNL_pos_tot <= PNL_pos_bound_tot
0 < effective_price(asset) <= MAX_ORACLE_PRICE
0 < fund_px_last(asset) <= MAX_ORACLE_PRICE
asset.slot_last <= current_slot
insurance_ledger.total_available <= I
```

For every asset side:

```text
0 < A_side <= ADL_ONE
if side is Normal and has current-epoch stored positions: A_side >= MIN_A_SIDE
0 <= OI_eff_side <= MAX_OI_SIDE_Q_PER_ASSET
if Live: OI_eff_long == OI_eff_short for each asset
if OI_eff_side > 0 and side is not ResetPending: loss_weight_sum_side > 0
if loss_weight_sum_side == 0: no residual may be cleared to that B domain except fully backed protocol-owned explicit loss
0 <= loss_weight_sum_side <= SOCIAL_LOSS_DEN
social_loss_remainder_side_num < SOCIAL_LOSS_DEN
social_loss_dust_side_num < SOCIAL_LOSS_DEN
```

K/F/B headroom:

```text
abs(K_side) + A_side * MAX_ORACLE_PRICE <= i128::MAX
abs(F_side_num) + A_side * MAX_ORACLE_PRICE * cfg_max_abs_funding_e9_per_slot * cfg_max_accrual_dt_slots <= i128::MAX
B_side_num <= u128::MAX
```

B epoch rules:

```text
Normal/DrainOnly side:
    B_side_num is current-epoch B and loss_weight_sum_side is the sum of current-epoch nonzero-basis weights.

ResetPending side:
    A/K/F/B_epoch_start are terminal targets for stale prior-epoch accounts.
    Current A/K/F/B and loss_weight_sum_side are new-epoch state and may be zero.
```

Explicit/unallocated loss buckets are non-redeemable audit/reconciliation state. They may trigger h-max or recovery while live, but are not user liabilities and MUST NOT block terminal market close after all accounts are closed.

-------------------------------------------------------------------------------
4. Claims, haircuts, equity lanes, and positive support
-------------------------------------------------------------------------------

```text
Residual = V - (C_tot + I)
PosPNL_i = max(PNL_i, 0)
FeeDebt_i = max(-fee_credits_i, 0)
ReleasedPos_i = PosPNL_i - R_i on Live; PosPNL_i on Resolved
```

Haircuts use conservative global junior bounds:

```text
if PNL_matured_pos_tot == 0: h = (1,1)
else h = (min(Residual, PNL_matured_pos_tot), PNL_matured_pos_tot)

if PNL_pos_bound_tot == 0: g = (1,1)
else g = (min(Residual, PNL_pos_bound_tot), PNL_pos_bound_tot)
```

Global cross-margin has two distinct positive-PnL concepts:

```text
junior_impaired =
    PNL_pos_bound_tot > Residual

maintenance_positive_credit:
    current conservative positive PnL that may help avoid liquidation only while
    junior_impaired is false. When junior_impaired is true, maintenance positive
    credit MUST use the same haircut-bounded support value as stricter lanes.

loss_curing_positive_support:
    positive PnL consumed to settle losses, reduce residuals,
    pay fees, withdraw, release, transfer, convert, or support risk increase.
    This MUST be globally haircut-bounded by g.
```

Per-leg positive support value:

```text
leg_positive_support_value =
    0 if leg is not eligible_for_positive_credit
    else floor(positive_pnl_current * g.num / g.den)
```

Per-leg maintenance positive value:

```text
leg_maintenance_positive_value =
    0 if leg is not eligible_for_maintenance_positive_credit
    else if junior_impaired:
        leg_positive_support_value
    else:
        positive_pnl_current after conservative leg-local haircuts
```

If a leg-specific junior-solvency bound is stricter than global `g`, the stricter bound MUST be used. The engine MUST NOT use face-value positive PnL to cure residuals, create senior value, or prevent liquidation when `junior_impaired == true`.

When effective positive support `S` is consumed, the engine MUST burn or lock face junior claim:

```text
if g.num == 0: S must be 0
else junior_face_burn = ceil(S * g.den / g.num)
```

`PNL_i`, `PNL_pos_tot`, `PNL_pos_bound_tot`, `PNL_matured_pos_tot` when applicable, and the close ledger's `junior_face_burned` MUST be updated conservatively in the same atomic instruction. Burning only `S` face value when `g < 1` is forbidden.

Equity lanes:

```text
Eq_maint_i =
    C_i + conservative_negative_leg_pnl
        + sum(maintenance_eligible leg_maintenance_positive_value)
        - FeeDebt_i - penalties

Eq_initial_i  =
    C_i + conservative_negative_leg_pnl
        + sum(initial_eligible leg_positive_support_value)
        - FeeDebt_i - penalties

Eq_trade_i    =
    C_i + conservative_negative_leg_pnl
        + sum(trade_eligible leg_positive_support_value)
        - FeeDebt_i - penalties

Eq_withdraw_i =
    C_i + floor(ReleasedPos_i * h.num / h.den)
        + min(PNL_i,0) - FeeDebt_i - penalties

Eq_no_positive_credit_i =
    C_i + conservative_sum_negative_leg_pnl - FeeDebt_i - penalties
```

Maintenance equity is the broadest lane and may use current conservative account PnL to avoid unnecessary liquidation. Initial, trade, withdraw, fee-paying, support, and residual-cure lanes are strict and globally haircut-bounded. Positive PnL does not become senior capital merely because it supports maintenance.

A domain-locked profitable leg contributes zero positive credit to initial, trade, withdraw, fee-paying, release, transfer, and loss-curing lanes. It may contribute to maintenance only under a conservative adverse-price shadow check that would not create or worsen a residual if the locked domain resolves against the account.

### 4.1 PositiveCreditAction

A `PositiveCreditAction` is any action whose approval, payout, transfer, withdrawal, reserve release, conversion, fee payment, support allocation, residual reduction, or risk increase depends on positive PnL increasing effective equity.

PositiveCreditActions include:

```text
withdrawal above no-positive-credit equity
risk-increasing trade approval
reserve release or acceleration
PNL conversion to capital
favorable close payout
resolved positive payout
fee payment from positive PnL
cross-account transfer of value
positive-PnL SupportPool contribution
residual reduction before insurance/B booking
```

A `PositiveCreditAction` MUST reject or use `Eq_no_positive_credit_i` if any contributing leg is:

```text
stale certificate
oracle stale
funding stale
loss-stale catchup incomplete
target/effective price lagged without conservative dual-price pass
B-stale
partially B-settled
partial-refresh
active close touching the leg's domain
pending-domain-loss barrier touching the leg's domain
recovery mode
thin-market locked
hmax/stress locked
```

### 4.2 Matched/self-dealing realization rule

Realizing positive PnL MUST NOT increase net withdrawable senior claims unless all matching portfolio/counterparty losses, fees, stale penalties, and support consumption are durably recognized in the same atomic instruction or were already current.

For matched trades, both sides that receive realized or newly reusable value MUST be refreshed, settled, and checked under final no-positive-credit/haircut state. If any loss side cannot settle, all corresponding gains remain junior, locked, or haircut-limited.

This rule applies across intermediate accounts when the same instruction or transaction creates economically linked gain/loss legs. Wrappers MUST NOT split matched settlement into separate extractable phases.

-------------------------------------------------------------------------------
5. Global cross-margin health
-------------------------------------------------------------------------------

A full portfolio refresh MUST compute conservative health from all active legs.

```text
gross_mm = sum(asset_mm_leg)
gross_im = sum(asset_im_leg)

portfolio_maintenance_req =
    gross_mm
    - hedge_credit
    + stale_penalty
    + concentration_penalty
    + thin_market_penalty
    + unsettled_loss_penalty
    + target_effective_lag_penalty
    + domain_lock_penalty

portfolio_initial_req =
    gross_im
    - initial_hedge_credit
    + stricter_penalties
```

Hedge credit is optional and conservative:

```text
hedge_credit <= min(offset_leg_risks) * cfg_max_offset_bps / 10_000
```

Hedge credit is allowed only for configured buckets:

```text
SameUnderlyingExact, or ExplicitFamilyWithGap proven under §1.3
current oracle/funding/risk epochs
no unsettled B loss on either leg
no stale certificate
no target/effective lag unless conservative dual-price checked
no recovery mode
no active close barrier on either domain
```

The account is:

```text
initial-healthy      if certified_equity_initial >= certified_initial_req
maintenance-healthy  if certified_equity_maint   >= certified_maintenance_req
liquidatable         if certified_liq_deficit > 0 after full refresh
```

Global cross margin means all active legs contribute to `certified_equity_maint` under conservative eligibility. It does not mean all markets share bankruptcy losses, insurance, or senior claims.

-------------------------------------------------------------------------------
6. Stale certificate decay
-------------------------------------------------------------------------------

A certificate is fresh only if its market group, config hash, epochs, active bitmap, asset slot vector, and effective price vector remain valid under the configured envelope.

When stale, compute a conservative penalty:

```text
stale_loss_bound =
    sum_abs_notional_at_cert * max_price_move_since_cert
    + max_funding_move_since_cert
    + fee_bound
    + configured_oracle_uncertainty_bound
    + thin_market_bound
    + domain_lock_bound
```

Then:

```text
current_certified_equity_maint = old_certified_equity_maint - stale_loss_bound
current_certified_maintenance  = old_certified_maintenance + stale_risk_penalty
```

Stale accounts MUST NOT withdraw, close favorably, convert/release PnL, increase risk, use hedge credit, use positive PnL for approval/support, or receive resolved positive payout.

A stale account MAY be refreshed, defensively rebalanced, liquidated, moved toward recovery, or have a dead leg forfeited/detached under §14.2.

-------------------------------------------------------------------------------
7. Canonical settlement helpers
-------------------------------------------------------------------------------

Every `C_i`, `PNL_i`, position, B, fee, close-state, insurance, and support-allocation mutation MUST use aggregate-updating helpers or equivalent proofs.

### 7.1 Attach and clear leg

`attach_leg(account, asset, side, new_eff)` requires old side effects settled, side mode permits current-epoch attach, full account refresh context, no active close barrier on that domain, and no existing nonzero same-asset opposite leg. Same-asset exposure changes MUST net into the canonical signed leg.

`clear_leg(account, asset)` requires A/K/F/B settled to the required target. It quarantines side remainder, transfers local `b_rem` to scaled dust, subtracts current-epoch weight, clears local fields, and MUST NOT mutate OI unless called by the OI-changing transition that proves the matching OI change.

### 7.2 Side remainder quarantine

Before changing `loss_weight_sum_side`:

```text
if social_loss_remainder_side_num != 0:
    transfer it to social_loss_dust_side_num
    social_loss_remainder_side_num = 0
```

While a close has staged residual against a side, weight-set changes on that side are forbidden.

### 7.3 Combined side-effect settlement

Authoritative touch prepares A/K/F and B together before principal loss settlement. The engine MUST NOT drain capital for B before same-touch K/F gains/losses are included.

For a nonzero leg, B target is current `B_side_num` if in the current epoch, else `B_epoch_start_side_num` under `ResetPending`.

A full settlement computes:

```text
ΔB = B_target - b_snap
num = b_rem + loss_weight * ΔB
B_loss = floor(num / SOCIAL_LOSS_DEN)
b_rem_new = num % SOCIAL_LOSS_DEN
KF_pnl_delta = exact signed-floor A/K/F settlement
net_pnl_delta = KF_pnl_delta - B_loss
```

If full B settlement is too large or not representable, partial B settlement is allowed. A user-favorable endpoint MUST stop after partial B settlement and return progress-required.

### 7.4 Account-local B settlement chunk

Let:

```text
B_remaining = B_target - b_snap
w = loss_weight
r = b_rem
L = per_touch_B_loss_limit
```

```text
max_num = (L + 1) * SOCIAL_LOSS_DEN - 1
if r > max_num: max_delta_by_loss = 0
else:           max_delta_by_loss = floor((max_num - r) / w)

delta_B_settle = min(B_remaining, max_delta_by_loss, endpoint_or_engine_delta_budget)
```

A successful chunk requires `delta_B_settle > 0` and writes:

```text
num = r + w * delta_B_settle
B_loss_chunk = floor(num / SOCIAL_LOSS_DEN)
b_rem = num % SOCIAL_LOSS_DEN
b_snap += delta_B_settle
```

If `B_remaining > 0`, the account remains B-stale and no user-favorable action may continue.

### 7.5 No rounding profit from dust

Dust and remainders are audit state, not user-credit state. Any remainder quarantine, B settlement, reset, or resolved close MUST round against the account and MUST NOT create positive PnL, withdrawable capital, support value, or fee-paying value.

-------------------------------------------------------------------------------
8. A/K/F/B mechanics
-------------------------------------------------------------------------------

### 8.1 Per-asset accrual

`accrue_asset_to(asset, now_slot, effective_price, funding_rate)` requires live mode, authenticated time, valid price, and bounded funding rate. It is asset-wide and non-exclusive with respect to domain locks.

A domain lock MUST NOT block updates to `effective_price`, `fund_px_last`, `slot_last`, `K_long`, `K_short`, `F_long_num`, or `F_short_num`. If accrual occurs while a close snapshot exists, that snapshot becomes stale until recomputed or conservatively re-aged under §10.3. Locked closes MUST NOT consume pre-accrual side-effect values.

Accrual MUST NOT mutate B, A, OI, weights, staged residuals, staged insurance, quantity ADL, or exposure-clear state for a locked domain unless the required domain set is atomically held by that close.

```text
dt = now_slot - asset.slot_last
funding_active = dt > 0 && funding_rate != 0 && OI_eff_long != 0 && OI_eff_short != 0 && fund_px_last > 0
price_move_active = effective_price != previous_effective_price && (OI_eff_long != 0 || OI_eff_short != 0)
```

If active, require `dt <= cfg_max_accrual_dt_slots`. If price moves:

```text
abs(effective_price - previous_effective_price) * 10_000
    <= cfg_max_price_move_bps_per_slot * dt * previous_effective_price
```

K/F/stress candidates are computed in exact wide arithmetic and validated before any persistent write. If validation fails, no K/F/stress/price/slot field is written.

### 8.2 B residual booking

`book_bankruptcy_residual_chunk(asset, side, residual_remaining)` is O(1). It requires eligible opposing weight, eligible domain insurance, or fully backed protocol-owned explicit loss state. A residual MUST NOT be cleared merely because `W == 0`.

Let `H = u128::MAX - B_side_num`, `W = loss_weight_sum_side`, and `R = social_loss_remainder_side_num`.

```text
max_scaled = (H + 1) * W - 1
if R > max_scaled: max_chunk_by_B = 0
else:              max_chunk_by_B = floor((max_scaled - R) / SOCIAL_LOSS_DEN)

engine_chunk = min(residual_remaining, max_chunk_by_B, cfg_public_b_chunk_atoms)

delta_B = floor((engine_chunk * SOCIAL_LOSS_DEN + R) / W)
new_remainder = (engine_chunk * SOCIAL_LOSS_DEN + R) % W
```

A successful B booking requires `W > 0`, `engine_chunk > 0`, `delta_B > 0`, and `B_side_num + delta_B <= u128::MAX`. The same atomic step MUST increment `CloseProgressLedger.b_loss_booked` for the closing account/domain by `engine_chunk`, reduce `residual_remaining`, and update or release the pending-domain-loss barrier; otherwise the B booking MUST revert.

If `W == 0`, residual may be cleared only by already-reserved eligible domain insurance or explicit protocol-owned capital that preserves `V >= C_tot + I` and all senior invariants. Otherwise the close cannot clear exposure and MUST enter permissionless recovery for the affected account/domain. A caller MUST NOT choose a smaller chunk than the engine-determined chunk. If no positive chunk is representable while residual remains, enter permissionless recovery.

### 8.3 Quantity ADL after residual durability

Bankruptcy residual is B-indexed; K is not used for bankruptcy residual. Quantity ADL is staged and applied exactly once after residual durability.

For a full close of `q_close_q`:

```text
OI_eff_liq_side -= q_close_q
OI_eff_opp_side -= q_close_q
```

If opposing OI remains, compute:

```text
A_candidate = floor(A_opp * OI_opp_after / OI_opp_before)
```

If `A_candidate == 0`, zero both sides and schedule reset. Otherwise set `A_opp = A_candidate`, update OI, and enter `DrainOnly` if `A_opp < MIN_A_SIDE`.

This step MUST NOT change local loss weights. The same atomic step MUST advance `CloseProgressLedger.quantity_adl_applied_q` exactly once for the closing account/domain. Quantity ADL MUST be paired atomically with closing-account exposure clear and finalization, or the account MUST enter a non-preemptible `ADLAppliedFinalizationBarrier` that permits only idempotent finalization/recovery. Domains MUST NOT be released to ordinary operations while ADL is durable but the closing exposure still appears open. Failure rolls back finalization.

### 8.4 Side reset

`begin_full_drain_reset(asset, side)` requires zero OI and no active close barrier on that side. It snapshots A/K/F/B epoch-start state, quarantines remainder, resets current A/K/F/B and weights, increments epoch, sets `A_side = ADL_ONE`, and enters `ResetPending`. Stale accounts settle against epoch-start snapshots.

-------------------------------------------------------------------------------
9. Deterministic liquidation and residual attribution
-------------------------------------------------------------------------------

Liquidation is triggered by global portfolio health:

```text
certified_equity_maint < certified_maintenance_req
```

Residual charging is leg-attributed and market-local.

### 9.1 PortfolioLiquidationPlan

A liquidation instruction MUST refresh the full account and build a deterministic `PortfolioLiquidationPlan` from all active legs.

The plan MUST include:

```text
LegCloseCandidate[0..N)
SupportPool
LossVector
SupportAllocationVector
InsuranceAllocationVector
ResidualAttributionVector
RequiredDomainSet                 // currently required domains for durable staged mutations
CloseDriftReserve
DriftReferenceSlot
MaxCloseSlot
snapshot_slot
snapshot_oracle_epoch
snapshot_funding_epoch
snapshot_price_vector_hash
ProgressScoreBefore
ProgressScoreAfter
```

The plan order MUST be deterministic, for example:

```text
1. legs with highest conservative risk contribution
2. then largest liquidation deficit
3. then asset_id ascending
4. then side order Long before Short
```

The caller may provide hints, but hints cannot choose attribution, skip toxic legs, or improve the account.

Before any close state is staged, the engine MUST derive the current-step `RequiredDomainSet` for the mutations it will actually perform in that instruction and reserve it under §2.4. Speculative future domains from re-aging are excluded. A conflict with a lower-priority close preempts and unwinds that close's reversible staged state; a conflict with a higher-priority close performs no mutation. Continuation may acquire additional currently-required domains only through deterministic preemption/unwind; hold-and-wait is forbidden. The lock model is minimal-current-step, not maximal-future-set.

The initial close plan MUST also set immutable `DriftReferenceSlot = snapshot_slot`, immutable `MaxCloseSlot = DriftReferenceSlot + cfg_max_bankrupt_close_lifetime_slots`, and `CloseDriftReserve`, the maximum adverse price/funding/K/F/stale/thin-market loss for the closing account from `DriftReferenceSlot` through `MaxCloseSlot`. Later recomputed working plans may get fresh `snapshot_slot` values, but they MUST reuse the ledger's immutable `DriftReferenceSlot` and `MaxCloseSlot`. The reserve is not user-credit state and cannot create withdrawable value. It exists only to prevent a multi-crank close from under-booking a loss while asset-wide accrual continues.

### 9.2 SupportPool and no partial-socialization escape

Before any residual is socialized, the engine computes account-wide support.

`reserve_required_for_remaining_open_risk` is defined as:

```text
reserve_required_for_remaining_open_risk =
    minimum additional conservative equity required
    so that every remaining open leg not included in the
    current close candidate set still satisfies
    maintenance after:
        worst bounded price move,
        worst bounded funding move,
        stale penalties,
        thin-market penalties,
        target/effective lag penalties,
        pending-domain-loss penalties,
        and liquidation costs.
```

The reserve MUST be computed from exact conservative post-close state and MUST round against the account. It MUST NOT depend on optimistic future liquidation success, future positive PnL realization, or unproven hedge correlation.

```text
SupportPool =
    max(0,
        available senior C_i
      + eligible current realized nonjunior gains
      + sum(eligible leg_positive_support_value)
      - fee debt
      - stale penalties
      - required locks
      - pending-domain-loss reserves
      - reserve_required_for_remaining_open_risk)
```

`leg_positive_support_value` is globally haircut-bounded by `g` under §4. The engine MUST NOT use face-value positive PnL in `SupportPool` when `g < 1`.

Positive PnL support is not converted into capital. It is consumed as junior support against losses. Any consumed positive support MUST burn or lock the required face junior claim under §4 and update global positive-PnL aggregates or bounds conservatively in the same atomic step.

A public instruction MUST NOT B-book a residual from a partial liquidation while the same account has remaining eligible support, closable positive value, or open risk that can be deterministically closed to reduce the residual. If a close would create residual while other legs remain, the engine MUST either:
- expand to a deterministic bankrupt portfolio close;
- prove no additional eligible support exists and remaining open risk is not closable within bounds; or
- route to recovery.

### 9.3 LossVector

For each losing close candidate, compute:

```text
LegLoss_j = max(0, loss_to_close_leg_j + liquidation_cost_j + side_effect_loss_j)
Domain_j  = (asset_j, opposing_side_j)
```

Losses are computed after A/K/F/B settlement required for that leg and after subtracting durable close progress already recorded in `CloseProgressLedger` for the same `(account, close_id, asset, domain)`. Any unbooked `residual_remaining` from the ledger remains a pending-domain-loss barrier. Losses MUST include stale, catchup, thin-market, target/effective-lag, domain-lock, close-drift, pending-domain-loss, and liquidation penalties when applicable.

### 9.4 Caller-independent support allocation

Support allocation MUST be deterministic and caller-independent.

Allowed allocation methods:

```text
A. pro-rata by LegLoss_j among all losing legs, with deterministic dust assignment;
B. lexicographic by deterministic liquidation plan order;
C. configured fixed priority proven not to increase extraction or contagion.
```

The chosen method is immutable config. A public market SHOULD use pro-rata allocation because it prevents the caller from pushing all residual into the most favorable B domain.

For pro-rata allocation:

```text
TotalLoss = sum(LegLoss_j)
SupportToLeg_j = floor(SupportPool * LegLoss_j / TotalLoss)
remaining_support_dust assigned by asset_id/side deterministic order
UncuredLoss_j = LegLoss_j - SupportToLeg_j
```

### 9.5 Domain-budgeted insurance allocation

Insurance allocation MUST be deterministic, staged exactly once, and domain-budgeted.

```text
InsuranceBudget_j =
    remaining_domain_budget[Domain_j]
  + permitted_global_protocol_first_loss_for_domain[Domain_j]
```

```text
InsuranceToLeg_j <= min(UncuredLoss_j, InsuranceBudget_j)
PostInsuranceLoss_j = UncuredLoss_j - InsuranceToLeg_j
Residual_j = max(0, PostInsuranceLoss_j)
```

The engine MUST reserve `InsuranceToLeg_j` under the close id before finalizing any exposure clear. Staged insurance cannot be spent by another close. Uncollectible fees are not insurance-eligible.

### 9.6 No cross-asset residual mutation

The following are forbidden:

```text
book ETH residual to SOL B
book one asset residual to all shorts
book residual to all profitable accounts
book residual to a global B index
pay uncollectible fees from insurance
borrow insurance budget from unrelated domains
use face-value junior PnL to reduce residual when g < 1
charge unrelated market principal
clear losing exposure before Residual_j is durable
```

`Residual_j` MUST be booked only to `Domain_j`.

### 9.7 Partial liquidation progress

A successful liquidation/rebalance of an unhealthy account MUST strictly reduce a deterministic lexicographic risk score:

```text
RiskScore = (
    is_liquidatable,
    certified_liq_deficit,
    total_abs_risk_notional,
    stale_penalty_accumulator,
    count_unsettled_B_legs,
    count_active_close_domains,
    count_active_legs,
    deterministic_tiebreak
)
```

or strictly reduce the liquidating deficit. Equal-score churn MUST revert.

-------------------------------------------------------------------------------
10. Bankrupt portfolio close and snapshot re-aging
-------------------------------------------------------------------------------

`begin_or_continue_bankrupt_portfolio_close(account, now_slot, budget)` is bounded, deterministic, and account-local. It may lock only domains in the current-step `RequiredDomainSet`, and it MUST reserve that set under the preemptive lock rules before staging any residual.

Minimum phases:

```text
Touched
FullPortfolioSideEffectsPartiallySettled
PortfolioLossVectorComputed
SupportPoolComputed
SupportAllocated
InsuranceAllocated
ResidualsPartiallyBooked
ResidualsBooked
QuantityADLApplied
AccountFinalized
```

Before `ResidualsBooked`, the account's basis, weight, OI, PnL, and slot are not freed or cleared except under staged, durable, exactly-once semantics.

### 10.1 Durable close progress and preemption/restart

Any durable close progress MUST be recorded in `CloseProgressLedger` in the same atomic instruction that makes the external mutation durable.

Durable progress includes:

```text
support consumed
junior face claim burned/locked
insurance spent
B loss booked
explicit loss assigned
quantity ADL applied
drift consumed
```

Preemption/unwind may reverse only unstabilized staged state. It MUST NOT erase durable close progress, pending-domain-loss barriers, or the immutable close_id. Restart/recompute MUST load the durable ledger under the same close_id and compute each domain's remaining residual as:

```text
remaining_residual =
    gross_loss_at_close_start
  + total_adverse_drift_from(drift_reference_slot, now)
  - support_consumed
  - insurance_spent
  - b_loss_booked
  - explicit_loss_assigned
```

`drift_consumed` MUST be set to the monotone maximum of `total_adverse_drift_from(drift_reference_slot, now)` and the prior `drift_consumed`. It MUST NOT be computed from the working plan's fresh `snapshot_slot` after recompute/restart.

A restarted close MUST NOT open a new close_id or recompute from the original gross leg loss without subtracting ledgered progress and total adverse drift from the immutable `drift_reference_slot`. A durable B booking, support consumption, junior face burn, or quantity ADL with no matching close-progress ledger entry is a critical invariant failure and MUST route to recovery before any further favorable action.

Recovery MUST reconcile the ledger, B index movement, ADL movement, insurance debits, and final account state exactly once. It MUST NOT double-book an already ledgered B loss or re-apply an already ledgered quantity ADL.

### 10.2 Close drift reserve and moving-loss prohibition

A close may span multiple cranks only if the initial close plan stages a conservative `CloseDriftReserve`, immutable `DriftReferenceSlot`, and immutable `MaxCloseSlot`.

```text
CloseDriftReserve >=
    max adverse price/funding/K/F/stale/thin-market loss
    from immutable DriftReferenceSlot through immutable MaxCloseSlot
    for all active legs in the close under the configured envelopes
```

Post-start favorable movement MUST NOT increase support, payout, or withdrawable value for the closing account. Post-start adverse movement is measured cumulatively from `DriftReferenceSlot`; it consumes `CloseDriftReserve` or increases residual inside the close's current-step domains. Recompute/restart MUST NOT reset this measurement window.

Every successful continuation MUST strictly reduce:

```text
CloseProgressMeasure =
    residual_remaining
  + unbooked_adverse_drift_bound
  + unsettled_B_loss
  + unsettled_insurance_staging
```

after adding worst-case drift since the previous continuation. If the measure does not decrease, the instruction MUST revert or route to recovery.

If `now_slot > MaxCloseSlot`, if `CloseDriftReserve` is exhausted, or if adverse drift can outpace guaranteed close progress, ordinary close continuation MUST stop and route to permissionless recovery. `MaxCloseSlot` is ledger-immutable and MUST NOT be refreshed by plan recompute or preemption restart. A close MUST NOT remain live indefinitely while fresh accrual grows its loss target.

### 10.3 Close snapshot validity

Any stored `LossVector`, `SupportPool`, `SupportAllocationVector`, `InsuranceAllocationVector`, or `ResidualAttributionVector` is a close snapshot, not permanently valid truth.

Before any continuation consumes a prior snapshot, the engine MUST either:

```text
A. recompute the full portfolio close plan from current state; or
B. conservatively re-age the snapshot from snapshot_slot to now_slot.
```

Re-aging MUST:
- reduce positive support by the maximum adverse price/funding/stale/thin-market move since snapshot;
- increase losses by the maximum adverse price/funding/stale/thin-market move since snapshot;
- incorporate all K/F/price/slot accrual that occurred after the working snapshot;
- compute adverse post-start movement cumulatively from immutable `drift_reference_slot` to now, not from the working `snapshot_slot`;
- set `drift_consumed` to the monotone maximum total adverse drift from `drift_reference_slot`;
- charge adverse post-start movement against `CloseDriftReserve` before reducing residuals;
- ignore favorable post-start movement for support/extraction;
- apply any new domain locks, B-stale state, recovery state, or target/effective lag;
- recompute `g` from current `Residual` and `PNL_pos_bound_tot`;
- re-apply global haircut-bounded positive support;
- re-evaluate whether `junior_impaired` forces maintenance positive credit to haircut value;
- invalidate any support from a leg that is no longer eligible;
- recompute or conservatively increase residuals after any staged insurance;
- release and restage insurance atomically if the old allocation is no longer conservative, without allowing double-spend;
- fail or route to recovery if the re-aged snapshot is not representable or no longer conservative;
- if re-aging requires additional domains, acquisition MUST follow deterministic preemption/unwind rules before mutation;
- fail/recover if close drift reserve is exhausted, expired by immutable `max_close_slot`, or no longer conservative.
- fail if a recompute attempts to move `drift_reference_slot`, `gross_loss_at_close_start`, or `max_close_slot`.

A continuation MUST NOT consume a stale face-value SupportPool. A close snapshot that cannot be made conservative is unusable.

### 10.4 Close rules

1. All active legs are considered, not just hinted legs.
2. The current-step `RequiredDomainSet` is reserved before residual, insurance, B, OI, A, weight, or exposure-clear state is staged.
3. Within one current step, partial domain acquisition is forbidden; across continuations, additional domains are acquired only through deterministic preemption/unwind.
4. `close_id`, `gross_loss_at_close_start`, `drift_reference_slot`, and `max_close_slot` are immutable until finalization or recovery reconciliation.
5. Every durable B booking, insurance spend, effective support consumption, junior face burn/lock, explicit loss assignment, and quantity ADL MUST advance `CloseProgressLedger` in the same atomic step.
6. Any unfinalized ledger residual creates a pending-domain-loss barrier until booked, backed, assigned to fully-backed explicit loss, finalized, or recovered.
7. Restart/recompute MUST reuse the same close_id and subtract ledgered durable progress from gross loss before computing remaining residual.
8. A conservative `CloseDriftReserve`, immutable `DriftReferenceSlot`, and immutable `MaxCloseSlot` are staged before multi-crank close progress.
9. Eligible support is consumed before insurance or B booking.
10. Positive support is globally haircut-bounded by current/re-aged `g`; consuming effective support burns/locks the required face junior claim.
11. Partial liquidation cannot socialize a residual while deterministic account-close support remains.
12. Insurance is domain-budgeted, staged exactly once, and cannot be double-spent across active closes.
13. Residuals are attributed per losing leg to that leg's `Domain_j`.
14. A residual with no eligible domain weight cannot be silently cleared; it must be backed by eligible domain/protocol capital or routed to recovery.
15. B booking is chunked per domain.
16. Quantity ADL and closing-account exposure finalization are atomic or protected by a non-preemptible finalization barrier.
17. Close-blocking fee debt is forgiven, not socialized.
18. If ordinary progress cannot continue, drift reserve expires, or close progress is non-decreasing after drift, the next bounded action MUST enter permissionless terminal recovery for that account or affected domain.
19. Unrelated accounts and domains may continue unless they conflict with a currently held domain or pending-domain-loss barrier.

-------------------------------------------------------------------------------
11. User operations
-------------------------------------------------------------------------------

A user-favorable operation MUST:

1. authenticate owner/authority;
2. validate clock, oracle target, effective price, admission, and inputs;
3. continue conflicting active close, recover, detach/forfeit a dead leg, or fail before unrelated mutation;
4. refresh the full active portfolio account;
5. settle A/K/F/B side effects for touched legs;
6. settle losses before fees;
7. recompute `HealthCert`;
8. run candidate checks under final h-max/stale/B/loss-stale/domain-lock/pending-domain-loss/recovery state;
9. commit only if all invariants hold.

Deposits are pure capital paths. Deposits into B-stale/stale/locked accounts are loss-curing only and MUST NOT enable favorable actions before refresh clears locks.

Withdrawals use post-withdraw candidate state. Any `PositiveCreditAction` must pass strict positive-credit gates; otherwise withdrawal uses `Eq_no_positive_credit_i`.

Trades require:

```text
full portfolio refresh for both counterparties or verified maker/liquidator account
loss-current market state
current B/K/F settlement for touched legs
side-mode gating
OI/position bounds
candidate-slippage neutralization
no-positive-credit approval while h-max/stress/stale/B/loss/domain locks are active
matched-side loss recognition before gain extractability
exact charged fee supplied to and enforced by engine
```

Trades MUST NOT execute while bounded catchup remains incomplete unless purely risk-reducing and passing no-positive-credit conservative checks.

-------------------------------------------------------------------------------
12. Rebalance
-------------------------------------------------------------------------------

Rebalance may occur on user touch, crank touch, or liquidation.

Allowed:

```text
move support equity across active legs within the same PortfolioAccount
reduce risk by closing, shrinking, or collateral-shifting among legs
refresh certificates
consume globally haircut-bounded positive PnL support against portfolio losses without converting it to capital
forfeit/detach a dead recovery-mode leg under §14.2
```

Forbidden:

```text
double count collateral
treat positive PnL as senior capital
use stale profitable legs for credit
use B-stale, domain-locked, or pending-domain-loss-barrier legs for hedge credit
erase fee debt or bankruptcy loss
improve one account by worsening another except explicit liquidation transfer rules
move bankruptcy residual across asset domains
use face-value positive PnL as support when g < 1
```

Conservation rule:

```text
senior_claim_after
    <= senior_claim_before
       + realized_nonjunior_pnl
       - fees
       - realized_losses
```

For an unhealthy account, accepted rebalance/liquidation requires strict `RiskScore` or deficit progress.

-------------------------------------------------------------------------------
13. Keeper cranks and hints
-------------------------------------------------------------------------------

Keeper cranks are bounded and incremental.

Inputs:

```text
account hints
proposed rebalance/liquidation actions
oracle/funding proofs as required
optional recovery proof
```

Rules:

```text
candidate padding/missing/duplicates count against inspection budget
missing global accounts do not cause rollback merely because more accounts exist
if equity-active accrual is performed on an exposed market, protective progress must also commit
hints are never assumed complete
any hinted unhealthy account must be processable with bounded work or routed to recovery
domain locks and pending-domain-loss barriers must progress, settle, or route to recovery
close snapshots must be recomputed or conservatively re-aged before consumption
asset-wide K/F price/funding accrual is not blocked by domain locks
close continuations must make progress net of adverse close drift
```

If `authenticated_now_slot - asset.slot_last > cfg_max_accrual_dt_slots`, use subtraction-first bounded catchup segments. While catchup remains incomplete, the market is loss-stale:

```text
positive PnL uses h-max/no-positive-credit lanes
reserves do not release
conversion/auto-conversion is disabled
risk-increasing trades, nonflat withdrawals, and OI-increasing actions are blocked
keeper touch/revalidation and risk-reducing actions may continue
```

-------------------------------------------------------------------------------
14. Resolution, recovery, and dead-leg detach
-------------------------------------------------------------------------------

A public `CrankForward` market MUST expose permissionless terminal recovery for any state where ordinary bounded progress cannot continue, including:

```text
BelowProgressFloor
BlockedSegmentHeadroomOrRepresentability
AccountBSettlementCannotProgress
BIndexHeadroomExhausted
ActivePortfolioCloseCannotProgress
DomainLockCannotProgress
DomainInsuranceBudgetExhausted
PartialSocializationForbiddenButFullCloseCannotProgress
CloseSnapshotCannotBeConservativelyReaged
CloseDriftReserveExhaustedOrExpired
CloseDriftAnchorMismatch
MaxCloseSlotReanchorAttempted
CloseProgressCannotOutrunAccrualDrift
OracleOrTargetUnavailableByAuthenticatedPolicy
RecoveryFallbackRequired
CounterOrEpochOverflowDeclaredRecovery
NTooLargeForBoundedRefresh
```

The caller cannot choose the recovery price. Recovery uses deterministic authenticated recovery price when available and representable; otherwise it MUST use the immutable configured fallback. A public market with no always-representable fallback recovery price is non-compliant.

If recovery occurs while an account close or domain lock exists, recovery follows the same deterministic preemption discipline as ordinary close continuation. Recovery MUST NOT participate in hold-and-wait cycles. Recovery MUST preserve and reconcile `CloseProgressLedger` and pending-domain-loss barriers; it cannot erase ledgered B/ADL/support progress and then recompute gross loss. Recovery MUST complete, durably settle, or deterministically unwind that close/lock before clearing it. It MUST NOT wait on a partially-held external lock cycle, drop residuals, double-spend insurance, clear PnL without durable loss state, orphan a pending-domain-loss barrier, or leave booked B loss to be charged again.

### 14.1 Resolved close

Resolved close is permissionless and bounded. It refreshes one `PortfolioAccount`, settles terminal K/F/B, clears reserve metadata, settles negative PnL from principal then eligible domain insurance/unallocated audit loss, syncs fees to `resolved_slot`, and only then may pay, forgive fee debt, or free.

Positive payout readiness is tracked by exact aggregates and conservative junior bounds, not by scanning all accounts in one instruction. Payout snapshot is captured once after readiness and remains stable.

### 14.2 Owner dead-leg forfeit/detach

Public markets MUST expose an owner-callable, bounded `forfeit_recovery_leg(account, asset)` path for a leg whose asset is in `Recovery`, `DrainOnly`, unresolved oracle-unavailable recovery, or other configured terminal-dead state.

The path:
- refreshes the full account conservatively;
- settles or over-reserves all A/K/F/B losses for the leg;
- values positive PnL of the forfeited leg at zero;
- values negative PnL of the forfeited leg at the conservative fallback/recovery loss;
- burns/forfeits any junior claim associated with the leg;
- books any residual only to the leg's bankruptcy domain `(asset, opposing_side)` after eligible account support rules;
- clears the leg only after residual durability;
- leaves unrelated legs usable once the account is otherwise fresh and healthy.

Forfeit/detach MUST NOT let the owner choose a favorable price, evade losses, escape B settlement, or move residual to unrelated domains. Its purpose is liveness: a dead asset must not permanently hostage unrelated collateral.

-------------------------------------------------------------------------------
15. Wrapper obligations
-------------------------------------------------------------------------------

Wrappers own:

```text
authorization
oracle normalization
raw target storage
effective-price staircase policy
account proof packing
anti-spam economics
hint markets or off-chain discovery
thin-market guardrails
```

Public wrappers MUST NOT expose caller-controlled:

```text
admission
funding
threshold
future slot
B residual chunk size
account-B settlement chunk size
portfolio support allocation method
portfolio insurance allocation method
residual attribution method
domain insurance budget override
domain lock acquisition order, current-step required-domain set, preemption priority, pending-domain-loss barrier, or close-progress ledger interpretation
whether asset-wide K/F accrual is blocked by domain locks
close snapshot validity, re-aging parameters, close drift reserve, drift reference slot, close-progress ledger, or max close slot
recovery fallback price
favorable stale-certificate interpretation
```

Public user-fund wrappers MUST expose:

```text
full account refresh
hinted crank
bounded catchup
active portfolio-close continuation
account-B settlement continuation
domain-lock continuation
permissionless recovery
owner dead-leg forfeit/detach
rebalance-on-touch
```

Target/effective lag MUST not give users a free option. Extraction-sensitive actions reject or shadow-check; risk-increasing trades use dual-price/no-positive-credit checks.

No wrapper may treat a global accumulator as proof that a specific account is healthy. Account health is proven only by full account refresh or conservative certificate.

-------------------------------------------------------------------------------
16. Required proof and TDD coverage
-------------------------------------------------------------------------------

1. `global_cross_margin_all_legs_support_maintenance`.
2. `global_cross_margin_does_not_create_global_B_domain`.
3. `bad_asset_residual_charged_only_to_asset_side_domain`.
4. `positive_support_is_g_haircut_bounded_when_global_junior_insolvent`.
5. `support_pool_never_uses_face_value_positive_pnl_when_g_below_one`.
6. `bankrupt_close_snapshot_must_recompute_or_reage`.
7. `stale_support_snapshot_cannot_understate_residual`.
8. `junior_impairment_haircuts_maintenance_positive_credit`.
9. `zero_weight_domain_residual_cannot_clear_without_backing`.
10. `oi_positive_requires_loss_weight_or_recovery`.
11. `reaged_close_recomputes_or_restages_insurance`.
12. `single_active_close_does_not_block_unrelated_domains`.
13. `domain_lock_blocks_only_conflicting_mutations`.
14. `domain_lock_requires_progress_or_recovery`.
15. `current_step_locking_does_not_reintroduce_maximal_serialization`.
16. `lock_model_has_no_maximal_set_contradiction`.
17. `orphaned_b_increment_routes_to_recovery_before_favorable_action`.
18. `preempted_close_restart_cannot_double_book_residual`.
19. `durable_quantity_adl_requires_matching_close_progress_ledger_advance`.
20. `durable_b_booking_requires_matching_close_progress_ledger_advance`.
21. `preemption_preserves_durable_b_but_restart_subtracts_ledgered_loss`.
22. `pending_domain_loss_barrier_blocks_weight_exit_until_residual_durable`.
23. `preempted_close_releases_locks_but_not_pending_loss_barrier`.
24. `effective_support_consumption_burns_required_face_junior_claim`.
25. `support_consumed_cannot_exceed_g_value_of_face_claim_burned`.
26. `close_id_reused_across_preemption_restart_until_finalized`.
27. `new_close_id_for_unfinalized_account_reverts`.
28. `quantity_adl_and_account_finalization_atomic_or_barriered`.
29. `adl_applied_open_leg_cannot_release_domains_to_ordinary_ops`.
30. `forfeit_recovery_leg_books_to_bankruptcy_domain_not_same_side`.
31. `reserve_required_for_remaining_open_risk_rounds_against_account`.
32. `shared_asset_closes_can_progress_without_global_serialization`.
33. `close_conflict_does_not_freeze_unrelated_positive_credit`.
34. `lower_priority_close_unwinds_before_higher_priority_progress`.
35. `preemptive_close_priority_prevents_serialized_hold_and_wait`.
36. `partial_domain_lock_acquisition_reverts_without_mutation`.
37. `side_lock_does_not_freeze_unrelated_side_accrual`.
38. `domain_lock_blocks_b_a_oi_weight_but_not_time`.
39. `locked_close_cannot_consume_pre_accrual_side_effect_values`.
40. `accrual_during_close_forces_snapshot_reage_before_consumption`.
41. `close_drift_reserve_bounds_post_start_adverse_accrual`.
42. `drift_reference_slot_immutable_across_preemption_restart`.
43. `max_close_slot_immutable_across_recompute`.
44. `recompute_snapshot_cannot_drop_pre_snapshot_drift`.
45. `drift_consumed_total_from_reference_slot_not_working_snapshot`.
46. `repeated_preemption_cannot_extend_close_lifetime`.
47. `split_drift_anchor_underbooking_reverts_or_recovers`.
48. `bankrupt_close_progress_decreases_net_of_close_drift`.
49. `close_cannot_chase_moving_kf_loss_forever`.
50. `expired_close_drift_routes_to_recovery`.
51. `post_start_favorable_move_does_not_increase_closing_account_support`.
52. `initialization_rejects_if_close_progress_cannot_outrun_drift`.
53. `domain_lock_does_not_block_asset_wide_kf_accrual`.
54. `recovery_cannot_deadlock_on_partial_external_lock_cycle`.
55. `liquidation_order_cannot_choose_residual_domain`.
56. `portfolio_support_allocation_is_caller_independent`.
57. `portfolio_insurance_allocation_is_caller_independent`.
58. `domain_budgeted_insurance_prevents_bad_asset_global_insurance_drain`.
59. `staged_insurance_not_double_spent_across_active_closes`.
60. `partial_liquidation_cannot_socialize_while_account_support_remains`.
61. `oracle_family_hedge_credit_assumes_adverse_basis_gap`.
62. `same_underlying_exact_offset_distinct_from_oracle_family_offset`.
63. `recovery_fallback_price_required_for_public_markets`.
64. `dead_leg_forfeit_unfreezes_unrelated_collateral_without_value_escape`.
65. `canonical_single_leg_per_asset_no_same_asset_double_support`.
66. `positive_pnl_support_not_withdrawable_without_gates`.
67. `self_dealing_realization_forces_matching_loss_recognition`.
68. `fake_pnl_cannot_become_senior_claim`.
69. `domain_locked_profitable_leg_cannot_support_risk_increase`.
70. `stale_profitable_leg_cannot_support_risk_increase`.
71. `stale_profitable_leg_zero_or_penalty_credit_for_withdraw`.
72. `full_account_refresh_is_O_N_and_required_for_favorable_actions`.
73. `hinted_subset_cannot_hide_toxic_leg`.
74. `rebalance_conserves_senior_claims`.
75. `rebalance_cannot_double_count_collateral`.
76. `cross_margin_offset_cap_never_below_loss_envelope`.
77. `unhealthy_rebalance_requires_strict_lexicographic_progress`.
78. `cyclic_rescue_without_progress_reverts`.
79. `B_stale_blocks_withdraw_convert_close_and_risk_increase`.
80. `account_B_settlement_chunks_huge_delta_without_market_scan`.
81. `B_booking_exact_remainder_conservation`.
82. `bankrupt_portfolio_close_books_all_residuals_before_clear`.
83. `bankruptcy_residual_excludes_protocol_fees`.
84. `uncollectible_fees_forgiven_not_socialized`.
85. `account_free_equity_active_accrual_requires_protective_progress`.
86. `effective_price_raw_target_lag_no_free_option`.
87. `loss_stale_catchup_blocks_risk_increase_until_current`.
88. `resolved_close_one_account_bounded`.
89. `permissionless_recovery_no_caller_chosen_price`.
90. `explicit_loss_audit_overflow_does_not_trap_funds`.
91. `authoritatively_flat_account_never_receives_B_loss`.
92. `no_single_instruction_full_market_requirement`.
93. `global_accumulator_not_account_health_proof`.
94. `active_bitmap_canonical_no_hidden_legs`.
95. `N_too_large_rejects_public_initialization`.
96. `PNL_pos_bound_tot_prevents_lazy_positive_pnl_first_mover_overpay`.
97. `per_asset_slot_last_prevents_cross_asset_accrual_aliasing`.
98. `reset_pending_epoch_start_snapshots_prevent_prior_epoch_resurrection`.
99. `certificate_bound_to_market_config_asset_slots_and_prices`.
100. `resolved_payout_readiness_uses_exact_counters_and_bounds`.

-------------------------------------------------------------------------------
17. v14.12 audit summary: major issues fixed
-------------------------------------------------------------------------------

[FIXED] Haircut-bounded support and junior-face burn
    Positive support is valued through `g`; consuming effective support now burns/locks the corresponding face junior claim and updates aggregates atomically.

[FIXED] Durable close progress across preemption
    B booking, ADL, insurance spend, support consumption, and explicit loss assignment advance `CloseProgressLedger` in the same atomic step. Restart resumes from remaining residual under the same close_id.

[FIXED] Pending residual escape after preemption
    Releasing a preempted close's domain locks no longer lets domain participants exit before unbooked residual is charged. Pending-domain-loss barriers survive until the residual is booked, backed, finalized, or recovered.

[FIXED] Immutable drift anchor and close lifetime
    `drift_reference_slot`, `gross_loss_at_close_start`, and `max_close_slot` are immutable. Recompute/preemption cannot drop drift windows or extend close lifetime.

[FIXED] ADL/finalization split
    Quantity ADL is paired atomically with closing-account exposure finalization or protected by a non-preemptible finalization barrier.

[FIXED] Current-step preemptible lock model
    The normative lock model is minimal current-step locking plus deterministic preemption/unwind, not maximal future-domain hoarding.

[FIXED] Recovery and dead-leg attribution
    Recovery preserves close-progress and pending-loss ledgers; dead-leg residuals book only to `(asset, opposing_side)`.

[KEPT] Global account solvency without global loss contagion
    Portfolio maintenance is global; bankruptcy residuals, domain insurance, B booking, and pending loss barriers remain local to the asset-side domain that generated the loss.

-------------------------------------------------------------------------------
18. Honest remaining tradeoff
-------------------------------------------------------------------------------

v14.12 guarantee:

```text
one honest crank with a valid account hint can force bounded progress on that account;
all current conservative legs may support global account solvency;
positive support used to cure losses, and maintenance support during junior impairment, is globally haircut-bounded;
bankruptcy residuals remain market-side local;
insurance cannot be globally drained by one bad asset unless explicitly capped protocol capital absorbs that risk;
dead assets can be forfeited/detached without moving losses to unrelated domains;
one active close cannot freeze unrelated domains; concurrent closes cannot deadlock through partial or expanding domain-lock acquisition; and domain locks cannot freeze asset-wide price/funding time; and bankrupt closes cannot chase an unbounded moving loss target or drop drift across recompute/preemption restarts; and preemption cannot orphan pending residuals, double-book durable chunks, or let domain participants escape unbooked losses.
```

This enables:

```text
true global cross-margin account health
unbounded global account count
lazy evaluation
bounded per-account verification
market-local bad-debt containment
domain-budgeted insurance containment
permissionless hinted recovery
dead-leg liveness escape
no market-wide liveness hostage from one close
```

