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
production v16 methods. It has two kinds of harness, which take different flags:

- **Plain `#[kani::proof]` harnesses** (the bulk, 258) assert behavior directly
  and carry `kani::cover!` witnesses. Run them with `--features fuzz` only.
- **`#[kani::proof_for_contract]` harnesses** (51, all in `src/v16_proofs.rs`)
  verify a function against its `#[kani::ensures]`/`#[kani::requires]` contract.
  They exist only under the `contracts` feature and need `-Z function-contracts`.

```bash
cargo install --locked kani-verifier
cargo kani setup

# Plain proofs — authoritative gate, must report 0 failures:
cargo kani --tests --features fuzz

# Contract proofs — adds the 51 proof_for_contract harnesses:
cargo kani --tests --features fuzz,contracts -Z function-contracts
```

Do **not** collapse these into one command. The `contracts` + `-Z function-contracts`
run also re-executes the plain proofs, and ~31 of them report **spurious
cover-reachability failures** under the `contracts` feature (the feature flips
which branches a `kani::cover!` can reach). Those are flag artifacts, not engine
or proof defects: every one re-verifies cleanly under `--features fuzz` alone
(confirm a single one with `cargo kani --tests --features fuzz --harness NAME`).
So `--features fuzz` is authoritative for plain proofs, and the `contracts` run is
authoritative for the 51 contract proofs.

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
  `validate_with_market`) holds at every committed `Ok` exit of all 55 public
  `*_not_atomic` entrypoints, and every value-bearing entrypoint has a per-op
  frame/value/kernel/closure theorem (no value-bearing operation rests on the
  validator floor alone). Wide arithmetic is abstracted to opaque spec values in
  Kani and discharged by Tier-A/Tier-B differential fuzz against an independent
  reference implementation (the `cargo test --features fuzz` conformance suite).
- **No-DoS.** Each of the 7 ActionableState classes has a named machine-checked
  witness. **NB1** is PROVEN-AT-KERNEL (`kernel_economically_valid_trade_admits`:
  admit IFF economically valid). **NB2** is PROVEN-AT-KERNEL (the selector is
  proven total; each continuation has a dispatch arm + a rank/terminal artifact +
  the static scan bound). The order-insensitive public auto-crank is
  engine-selected: `first_actionable_slot` (bounded asset self-selection) and
  `select_auto_crank_plan` (totality + priority + engine-selected asset) are
  proven; the liquidation fee is config-derived; a step with no matching
  observation returns a clean `NonProgress` without mutation.

Assumptions (the trusted base): SVM rollback on `Err`; scheduler fairness (SCHED);
the named arithmetic axiom manifest; and wrapper routing/auth/oracle/CU
obligations (the engine does not certify the wrapper). The full-monolith-route
rank/admission and max-shape CU are the PROVEN-AT-KERNEL-vs-full-route boundary,
not soundness gaps.

### Permissionless crank: one public route

There is exactly **one** public crank entrypoint, `permissionless_auto_crank_not_
atomic`. It is built for a swarm of opportunistic keepers landing out of order:
the caller submits authenticated oracle **observations** (asset, price, funding)
plus a liquidation budget, and the **engine** — not the caller — classifies the
account, selects the highest-priority step *and its asset* from current on-chain
state, derives the liquidation fee from config, and dispatches one bounded
primitive. One instruction = one step (never loop to a fixed point).

The per-action primitives (refresh / settle-B / liquidate / recover /
close-resolved) are **internal** dispatch targets, not wrapper entrypoints —
caller-chosen actions go stale under out-of-order landing. A step whose
observation is absent returns `NonProgress` with no mutation, so a stale/late tx
is a clean no-op (SVM rollback) and arbitrary landing order is safe.

Wrapper call: decode the instruction → authenticate clock/observations → build
`AutoCrankWorkV16` → call once → mirror token/custody movement keyed off
`result.selected` (`AutoCrankPlanV16`). See the doc comment on
`permissionless_auto_crank_not_atomic`.

## Verify

The verification is all `cargo`:

```bash
# Behavior + differential conformance (fast):
cargo test                  # unit + spec behavior tests
cargo test --features fuzz  # + reference-model differential conformance + property fuzz
                            #   (this is the discharge for the wide-arithmetic axiom)

# Formal proofs (Kani) — flags depend on harness type (see "Proofs" above):
cargo kani --tests --features fuzz                                  # all plain proofs — must be 0 failures
cargo kani --tests --features fuzz,contracts -Z function-contracts  # + the 51 contract proofs
cargo kani --tests --features fuzz --harness NAME                   # one plain proof
```

The single `fuzz,contracts` sweep reports ~31 spurious cover-reachability
failures on plain proofs (a flag artifact, not a defect — see "Proofs"); each
re-verifies under `--features fuzz`. Run harnesses one at a time (`--harness`) if
the full sweep is flaky; kill any stray `cbmc` between runs.

## Scope

This repository is a pure risk-engine library. It does not define an on-chain
program id, account decoder, persisted market registry, or deployment manifest.
Wrappers own authorization, account loading, oracle/funding authentication,
fee-schedule policy, and raw-state layout migration.

## License

Apache-2.0.
