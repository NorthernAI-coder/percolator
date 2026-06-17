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

The engine carries a decomposed, machine-checked argument for two safety
properties at the pure-engine boundary. The precise claim is a **conditional
10/10**: 10/10 *under this decomposition and the named assumptions below* — not
an unconditional "the full engine and wrapper are proven end-to-end". It is a
composed proof, not a single all-transitions theorem (that one query is
intractable for this prover generation), and several no-DoS obligations are
**proven at the kernel/dispatch boundary** rather than as a full route-level
theorem over the symbolic monolith body (the distinction is stated explicitly
below — it is accepted/backstopped by the architecture, not a soundness gap).

No loss of funds (no-LoF):

- `GlobalValidState` — `validate_shape` plus per-touched-account
  `validate_with_market` — is preserved at every committed `Ok` exit of all 56
  public `*_not_atomic` entrypoints, checked transitively by
  `scripts/boundary_audit.py` (56/56). `Err` paths fully revert at the execution
  boundary, so they need no preservation.
- Every public entrypoint is mapped to a stronger no-LoF proof source by an
  enforced partition, `scripts/lof_transition_class_roster.py` (10 transition
  classes; build fails on any unclassified entrypoint or missing artifact):
  exact whole-state frames, whole-body frame+value composition (attach/clear),
  production kernel-contract value deltas, typed `TokenValueFlowProofV16`
  validation, and inductive encumbrance/lien closure proofs.
- A per-entrypoint **strength roster**, `scripts/no_lof_strength_roster.py`,
  classifies each entrypoint THEOREM (per-op value/frame/kernel/closure proof) vs
  FLOOR (validator floor only) and **enforces zero value-bearing FLOOR-only
  entrypoints**: 51 THEOREM, 5 FLOOR (all verified non-value-bearing structural
  setup ops). The bankruptcy-residual chain (booking leaf → step → ledger
  absorption → PnL settlement → insurance-draw vault-neutrality) is a worked
  example of the per-op value theorems.
- Value-moving arithmetic is proven via the arithmetic-axiom recipe (policy in
  `scripts/arithmetic_policy.md`): the wide division/multiplication helpers are
  abstracted to opaque spec values inside Kani, and their exact forms are
  discharged by Tier-A-exhaustive + Tier-B-sampled differential fuzz (with
  explicit boundary vectors) against independent reimplementations. Kani never
  executes wide arithmetic.

No denial of service (no-DoS / liveness):

- `ActionableState` is a 7-class disjunction; every class has a present, named
  machine-checked witness, classified by strength and enforced by
  `scripts/actionable_class_coverage.py`. A well-founded lexicographic rank
  decreases on each continuation; the B-advance and close-advance rank steps are
  machine-proven production kernels.
- **NB1 (valid trade admitted)** is PROVEN-AT-KERNEL:
  `kernel_economically_valid_trade_admits` proves *admit IFF economically valid*
  over the production trade inputs (`EconomicallyValidTradeV16`), so no
  economically-valid trade is internally DoSed and every rejection maps to a
  concrete false precondition; the guard summary is proven faithful to production
  (`scripts/route_fidelity_roster.py`).
- **NB2 (finite crank progress)** is PROVEN-AT-KERNEL: the selector is proven
  total over actionable summaries, and `scripts/nb2_continuation_matrix.py` pins
  each continuation to a dispatch arm + a present rank/terminal artifact + the
  static per-account scan bound.
- The order-insensitive public auto-crank (`permissionless_auto_crank_not_atomic`)
  is **engine-selected, not caller-directed**: the keeper submits bounded oracle
  observations + a liquidation budget, and the ENGINE classifies the account,
  selects the highest-priority step, and self-selects the asset. Two pure kernels
  carry the proof: `first_actionable_slot` (the bounded leg scan returns an
  in-range, actionable, first-match slot and is complete) and
  `select_auto_crank_plan` (totality + priority determinism + the plan carries the
  engine-selected asset). The liquidation fee is config-derived, never a caller
  hint; a step whose observation is absent returns a clean `NonProgress` without
  mutation, so stale keeper transactions are order-insensitive.
- The **PROVEN-AT-KERNEL vs full-route distinction (explicit):** for NB1/NB2 and
  the A1/A2/A3/A5/A7 classes, the step kernel, the dispatcher route fidelity, and
  the guard/summary fidelity are each machine-checked, but the rank-decrease /
  admission *over the full symbolic two-account or monolith body* is **not** a
  single route-level theorem — it is accepted/backstopped by this decomposition
  (kernel proof + dispatch fidelity + validator floor), and the max-shape CU
  envelope is a wrapper/LiteSVM obligation. This is the honest boundary of the
  current claim, not a soundness gap.

Assumptions and named boundaries (the trusted base — also in
`scripts/engine_contracts.json`):

- `ArithmeticAxiom` + differential fuzz: the stubbed wide div/mul helpers equal
  their spec; only this narrow, helper-specific arithmetic is assumed, never a
  global arithmetic operator. See `scripts/arithmetic_policy.md` (conditional
  Option 1; unconditional Option 2 tracked separately).
- Execution-boundary atomicity: a rejected (`Err`) public call fully reverts.
- External scheduler / fairness (SCHED): the engine proves a successful bounded
  continuation *exists* for every actionable state; it does not prove an external
  actor *submits* it. Permissionless cranks make every continuation callable by
  any actor.
- Wrapper obligations: account routing, signer/auth, oracle freshness, rollback
  boundaries, and instruction serialization are the wrapper's to prove; the
  wrapper imports `scripts/engine_contracts.json` rather than reproving engine
  internals (the engine repo does not certify the wrapper).
- Tool-generation limits (not soundness gaps): a single Kani query over all
  public transitions at once, and whole-body value composition / full route-level
  rank for large-interior bodies, are intractable due to bit-precise wide
  arithmetic and large-struct symbolic state. The rosters above are the sound
  decomposition.

### Certification gates

The engine signoff is gate-enforced. `python3 scripts/final_ci_gate.py
[--with-conformance]` runs the full engine certification suite and fails on any
gap: boundary audit, no-LoF entrypoint + strength rosters, lof transition-class
roster, route-fidelity roster, guard mutation-sensitivity, actionable-class
coverage, liveness roster (NB1/NB2 must not be PARTIAL), NB2 continuation matrix,
dead-kernel check, arithmetic-axiom manifest, identity-independence and
symbolic-assert audits, and the engine contract manifest. The
**cover-vacuity gate** (`scripts/cover_vacuity_gate.py <kani-log-dirs>`) is an
explicit condition run over **captured per-harness Kani logs** in CI — it fails
any harness whose `kani::cover!` witness is unsatisfiable, so a vacuous-but-green
proof cannot pass. (It is a log reader, so it runs after the Kani sweep produces
fresh logs for the audited commit; it is not exercised by the source-only gates.)

Full detail: `scripts/no-steal-theorem.md` (no-LoF), `scripts/no-dos-liveness.md`
(no-DoS), `scripts/proof-frontier-closure.md` (the goal-by-goal index), and
`scripts/coverage_reconciliation.md` (the per-spec matrix).

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
