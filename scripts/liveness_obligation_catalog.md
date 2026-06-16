# Phase 0 — Liveness-obligation catalog (Pillar L: no-DoS / rank progress)

Liveness is a FIRST-CLASS pillar, NOT inferable from the validator floor
(Pillar F says committed states are well-formed; it says nothing about whether a
stuck account can progress). Derived from scripts/no-dos-liveness.md.

The well-founded rank (lexicographic, decreases on every listed continuation):
`R = (pending closes, Σ residual_remaining, Σ (b_target − b_snap), stale count)`.

For each ActionableState class, two obligations (the review's shape) + the
shared "not blocked" obligation:
- L.sel — `class(S) && GlobalValidState(S) => EXISTS a public continuation C
  reachable through the REAL public route, with C's preconditions satisfiable`.
- L.dec — `C(S) = Ok and post strictly decreases R, or records terminal recovery`.

| class | predicate | continuation C | L.sel status | L.dec status |
|-------|-----------|----------------|--------------|--------------|
| A1 stale account | `stale_state != 0` or cert invalid for epoch | accrual/refresh crank | MISSING (route-reachability) | PARTIAL (one-bounded-segment proven) |
| A2 b-stale leg | leg `b_stale` or `b_target > b_snap` | settle_account_b_chunk | PROVEN-AT-KERNEL (liveness_b_stale_leg_has_advancing_chunk) | PROVEN (kernel_advance_leg_b_snap) |
| A3 pending close residual | `close active && residual>0 && now<=max_close_slot` | advance_close_progress_ledger | PROVEN-AT-KERNEL (liveness_pending_close_has_rank_decreasing_advance) | PROVEN (kernel_advance_close_ledger) |
| A4 expired close | `close active && now>max_close_slot` | declare_permissionless_recovery | PROVEN-PUBLIC-ROUTE (ensure_close_progress_not_expired) | PROVEN (terminal recovery recorded, value-neutral) |
| A5 liquidatable | maintenance deficit with open risk | liquidate / route-to-recovery | PROVEN-PUBLIC-ROUTE (preflight accept+route) | PARTIAL (risk-reduction kernel = S-L3, MISSING) |
| A6 recovery-eligible | stuck class (B exhaustion, underbacking, lock/barrier) | permissionless recovery crank | PROVEN-PUBLIC-ROUTE (permissionless_crank) | PROVEN (accounting-neutral terminal) |
| A7 resolved winner | `mode==Resolved` with open claim/capital | close_resolved | PARTIAL (terminal suite witnesses) | PARTIAL (terminal realization — Kani-intractable, fuzz) |

## Gaps to close (Phase 3 L-cores + Phase 4)
1. A1/A2/A3 are PROVEN-AT-KERNEL (witness exhibited at the kernel boundary). The
   route-reachability half — that the public BODY routes the actionable state to
   its kernel without rejecting for an unrelated reason — is still backstopped
   for the monolith interior. Extract the route/gate selection into
   `select_progress_witness_*` kernels and prove L.sel as a contract.
2. A5.dec needs `kernel_liquidation_loss_split` (S-L2) + a risk-reduction lemma:
   the liquidation continuation strictly reduces open risk (rank component).
3. A7 terminal realization stays fuzz (intractable); make the fuzz Tier-A/Tier-B
   per Phase 6, and prove the GATE (mode==Resolved admits close_resolved).

## The "not blocked" obligation (finding 6 — distinct from value safety)
NB1 — an economically-valid trade (within IM) is NOT rejected by the margin
gate. PROVEN: kernel_initial_margin_gate's contract is a BICONDITIONAL
(Ok==>admit AND Err==>!admit); the Err==>!admit clause's contrapositive is
exactly "valid cert + equity>=IM ==> admitted", so the gate rejects ONLY
genuinely-invalid trades and cannot deny service to valid users. The
locked-lane gate (kernel_locked_margin_gate) is likewise a total decision.
Remaining for full NB1: the oracle/funding-envelope and fee-affordability
guards (trade_preflight_risk_gate is proven to block only unsafe increases).
NB2 — finite crank progress: every permissionless crank step does bounded work
(req 33/34 unwind bounds) AND advances R or is a no-op only when not actionable.


## Composed L.sel selector (3A.4 — overlap-safe gate-reachability)
select_progress_witness(ActionableSummaryV16) -> Option<ProgressContinuationV16>
is a PROVEN total/deterministic selector (contract_check_select_progress_witness):
for any actionable summary it returns Some continuation whose class is ACTUALLY
active (non-blocked), with a fixed priority resolving overlaps — so one active
class cannot invalidate the witness chosen for another (the per-class-witness
weakness the roadmap flagged). This composes the per-class L.dec rank kernels
into one overlap-safe L.sel.

## External assumption (named, out of engine scope)
SCHED — an external actor SUBMITS the continuation. The engine proves a
successful bounded continuation EXISTS and is callable by ANY actor
(permissionless, req 35); it does not prove submission. This stays an explicit
assumption, never claimed as proven.

## Acceptance (Pillar L)
Every class has L.sel proven as a reachable-route contract (or a documented
terminal-route witness), L.dec proven as a rank-decrease/terminal kernel, NB1/NB2
proven at the gate kernels, and SCHED named. A roster check (extend
actionable_class_coverage.py) enforces both halves per class.
