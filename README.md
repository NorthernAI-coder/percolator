# Percolator

**EDUCATIONAL RESEARCH PROJECT — NOT PRODUCTION READY. NOT AUDITED. Do NOT use with real funds.**

Current normative spec: [`spec.md`](spec.md), **v16.9.0**.

Percolator is a perpetual-futures risk-engine library for account-local,
permissionless risk progress. v16 keeps the slab-free account model and adds
source-domain realizable credit: positive PnL from one source domain is usable
only up to conservatively proven counterparty or insurance backing for that
domain.

The core promise is narrower and more realistic than global auto-discovery:
if an honest crank supplies a valid account hint, the engine can make bounded
progress on that account, while omitted or stale accounts cannot extract value
or increase risk using optimistic health.

## Three Invariants

1. **Realizable credit:** protected principal is senior, positive PnL is junior, and source-domain positive credit cannot exceed realizable backing reserved for that domain.
2. **Account-local safety:** every favorable action refreshes the account's full active portfolio first; hidden, stale, or B-stale legs fail closed.
3. **Bounded progress:** cranks and recovery paths are account-local and incremental; no public instruction needs to evaluate the whole market.

## Account-Local v16

Each `PortfolioAccountV16` carries provenance:

```text
market_group_id
portfolio_account_id
owner
version/layout discriminator
```

The engine rejects any account whose provenance does not match the
`MarketGroupV16`. Active positions are defined only by the canonical active
bitmap and bounded leg array. There is no hidden slab slot and no global account
table to scan.

The account-local bounded work unit is a full portfolio refresh over at most
`MAX_PORTFOLIO_ASSETS_N` configured legs. A fresh health certificate is required
for user-favorable actions. If an account is stale, B-stale, under h-max/stress,
or loss-stale, favorable paths must reject or use conservative no-positive-credit
lanes.

## H-Lock

Capital is senior. Profit is junior. `h_min` may be zero while the market is
healthy, but h-lock selection is state-derived and permissionless:

- `h_min` is used only when no h-max condition is active.
- `h_max` is used under threshold stress, bankruptcy h-lock, instruction-local
  bankruptcy candidates, loss-stale catchup, stale/B-stale account state, or
  active bankrupt close state.

Wrappers do not choose h-lock from an oracle. They supply authenticated market
inputs; the engine selects the lane from committed market/account state.

## Positive Payouts

Live positive PnL is an ordinary junior lane only while the market group is
`Live` and no resolved payout ledger exists. Once the market resolves, positive
claims move to a single resolved payout ledger: exact account receipts replace
scaled unreceipted bounds, payouts track `paid_effective`, and later bound
refinements can only increase claimable top-ups.

## A/K/F/B

v16 keeps the lazy index model but makes bankruptcy residuals explicit:

- **A** scales effective quantity for side-level quantity ADL.
- **K/F** represent mark and funding settlement.
- **B** books bankruptcy residual loss through account-local chunks.

Account-local B settlement is bounded. A public endpoint must either apply the
engine-determined positive chunk, leave the account B-stale, or route to
permissionless recovery if no positive chunk is representable. B-stale accounts
cannot withdraw, close favorably, convert/release PnL, use hedge credit, increase
risk, or receive resolved payout.

## Crank And Recovery

Public user-fund markets are `CrankForward`. An account-free equity-active crank
is forbidden unless it also commits bounded protective progress. Candidate lists
are hints, not proofs of completeness: missing accounts do not make a crank
unsafe, and hinted unhealthy accounts must either make bounded progress or route
to recovery.

If ordinary bounded progress cannot continue, the public recovery API records a
deterministic recovery reason. The caller does not choose a recovery price.

## Proofs

The current v16 proof suite is intentionally account-local and runs over the
production v16 methods:

```bash
cargo install --locked kani-verifier
cargo kani setup
scripts/run_kani_full_audit.sh
```

The latest checked timing sweep is in:

```text
kani_audit_full.tsv
kani_audit_final.tsv
scripts/proof-strength-audit-results.md
```

The old slab proof inventory was retired with the v16 cutover because it no
longer applies to the architecture.

### No-LoF / No-DoS: what is proven, and under what assumptions

The precise claim is a **conditional 10/10**: 10/10 *under this decomposition and
the named assumptions below* — not an unconditional "the full engine and wrapper
are proven end-to-end". It is a composed proof, not a single all-transitions
theorem (that query is intractable for this prover generation), and several
no-DoS obligations are **proven at the kernel/dispatch boundary**, not as a full
route-level theorem over the symbolic monolith body — accepted/backstopped by the
decomposition, not a soundness gap.

- **No-LoF.** `GlobalValidState` (`validate_shape` + per-touched-account
  `validate_with_market`) holds at every committed `Ok` exit of all 56 public
  `*_not_atomic` entrypoints (`scripts/boundary_audit.py`, 56/56). Each entrypoint
  maps to a stronger per-class proof source (`scripts/lof_transition_class_roster.py`),
  and `scripts/no_lof_strength_roster.py` enforces **zero value-bearing FLOOR-only**
  entrypoints (51 per-op THEOREM, 5 non-value-bearing FLOOR). Wide arithmetic is
  abstracted to opaque spec values in Kani and discharged by Tier-A/Tier-B
  differential fuzz (`scripts/arithmetic_axiom_manifest.md`).
- **No-DoS.** `scripts/actionable_class_coverage.py` covers the 7 ActionableState
  classes. **NB1** is PROVEN-AT-KERNEL (`kernel_economically_valid_trade_admits`:
  admit IFF economically valid). **NB2** is PROVEN-AT-KERNEL (the selector is
  proven total; `scripts/nb2_continuation_matrix.py` pins each continuation to a
  dispatch arm + rank/terminal artifact + the static scan bound). The
  order-insensitive public auto-crank is engine-selected: `first_actionable_slot`
  (bounded asset self-selection) and `select_auto_crank_plan` (totality + priority
  + engine-selected asset) are proven; the liquidation fee is config-derived; a
  step with no matching observation returns a clean `NonProgress` without mutation.

Assumptions (the trusted base, also in `scripts/engine_contracts.json`): SVM
rollback on `Err`; scheduler fairness (SCHED); the named arithmetic axiom
manifest; and wrapper routing/auth/oracle/CU obligations (the engine does not
certify the wrapper). The full-monolith-route rank/admission and max-shape CU are
the PROVEN-AT-KERNEL-vs-full-route boundary, not soundness gaps.

### Certification gates

`python3 scripts/final_ci_gate.py [--with-conformance]` runs the engine
certification suite and fails on any gap (boundary audit, no-LoF entrypoint +
strength rosters, transition-class roster, route-fidelity roster, guard
mutation-sensitivity, actionable-class coverage, liveness roster with NB1/NB2 not
PARTIAL, NB2 continuation matrix, dead-kernel check, arithmetic-axiom manifest,
identity-independence + symbolic-assert audits, engine contract manifest).
`scripts/cover_vacuity_gate.py <kani-log-dirs>` is an explicit condition run over
captured per-harness Kani logs in CI (it fails any harness whose `kani::cover!`
witness is unsatisfiable). The engine exports `scripts/engine_contracts.json` for
wrapper proofs to import.


## Tests

```bash
cargo test                                   # default unit + spec suite
cargo test --features fuzz                   # + reference/differential conformance + fuzz
python3 scripts/final_ci_gate.py --with-conformance   # engine certification gate suite
```

The Kani proof sweep (`scripts/run_kani_full_audit.sh`) is separate and slower;
the cover-vacuity gate runs over its captured per-harness logs.

## Scope

This repository is a pure risk-engine library. It does not define an on-chain
program id, account decoder, persisted market registry, or deployment manifest.
Wrappers own authorization, account loading, oracle/funding authentication,
fee-schedule policy, and raw-state layout migration.

## License

Apache-2.0.
