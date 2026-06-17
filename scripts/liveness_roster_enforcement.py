#!/usr/bin/env python3
"""ROADMAP Phase 4 acceptance: enforce the liveness (Pillar L) roster — every
ActionableState class has BOTH halves accounted, and every named artifact
exists:

  L.sel — a witness that the actionable class admits a public continuation
          (selection / route-reachability).
  L.dec — a witness that the continuation strictly decreases the well-founded
          rank R or records terminal recovery.

Plus the "not blocked" obligations NB1/NB2. Status per half:
  PROVEN          — a machine-checked artifact (named fn present here).
  PROVEN-AT-KERNEL— the rank step is proven at the kernel boundary; route
                    through the monolith interior is the backstopped half.
  BACKSTOPPED     — validator + fuzz + gate evidence, not a single theorem
                    (the documented intractable interior).
This script FAILS the build if any named artifact is missing (renamed/deleted)
or any class lacks a recorded disposition for both halves — turning the
liveness catalog into an enforced invariant. It does NOT re-run proofs.

Derived from scripts/liveness_obligation_catalog.md + no-dos-liveness.md.
"""
import re
import sys

PROOFS = "tests/proofs_v16.rs"
HARNESS = "src/v16_proofs.rs"

PROVEN = "PROVEN"
PROVEN_AT_KERNEL = "PROVEN-AT-KERNEL"
PARTIAL = "PARTIAL"
BACKSTOPPED = "BACKSTOPPED"

# class -> {sel:(status, file, [fns]), dec:(status, file, [fns])}
ROSTER = {
    "A1 stale account": {
        "sel": (PROVEN_AT_KERNEL, HARNESS, ["contract_check_select_progress_witness"]),  # overlap-safe selection (route through refresh monolith remains backstopped)
        "dec": (PROVEN, PROOFS, ["proof_v16_equity_active_accrual_with_progress_commits_one_bounded_segment"]),
    },
    "A2 b-stale leg": {
        "sel": (PROVEN_AT_KERNEL, HARNESS, ["liveness_b_stale_leg_has_advancing_chunk"]),
        "dec": (PROVEN, HARNESS, ["contract_check_kernel_advance_leg_b_snap"]),
    },
    "A3 pending close residual": {
        "sel": (PROVEN_AT_KERNEL, HARNESS, ["liveness_pending_close_has_rank_decreasing_advance"]),
        "dec": (PROVEN, HARNESS, ["closure_kernel_advance_close_ledger_rank_witness"]),
    },
    "A4 expired close": {
        "sel": (PROVEN, PROOFS, ["proof_v16_expired_close_progress_declares_recovery_without_value_mutation"]),
        "dec": (PROVEN, PROOFS, ["proof_v16_expired_close_progress_declares_recovery_without_value_mutation"]),
    },
    "A5 liquidatable": {
        "sel": (PROVEN, PROOFS, ["proof_v16_liquidation_preflight_accepts_only_fully_durable_residual",
                                 "proof_v16_liquidation_preflight_routes_insufficient_residual_capacity_to_recovery"]),
        # dec: insurance-draw + risk-reduction kernels, both production-called by liquidate_account (strict progress)
        "dec": (PROVEN_AT_KERNEL, HARNESS, ["contract_check_kernel_consume_insurance_layer", "contract_check_kernel_reduce_position_delta"]),
    },
    "A6 recovery-eligible": {
        "sel": (PROVEN, PROOFS, ["proof_v16_permissionless_recovery_crank_is_accounting_neutral"]),
        "dec": (PROVEN, PROOFS, ["proof_v16_permissionless_recovery_crank_is_accounting_neutral"]),
    },
    "A7 resolved winner": {
        "sel": (PROVEN, PROOFS, ["proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault"]),
        "dec": (PROVEN_AT_KERNEL, HARNESS, ["contract_check_kernel_resolved_close_progress"]),  # close-step progress proven; terminal realization arithmetic -> P6 fuzz
    },
}

# "not blocked" obligations. PARTIAL (per /tmp/proofs.md 3B.0): the final gate is
# proven, but NB1/NB2 stay PARTIAL until the FULL guard stack is covered — NB1
# cannot be PROVEN on the margin gate alone (it also needs request validation,
# refresh/currentness, loss-stale scope, target/effective lag, pending-domain
# barrier, fee-affordability, and oracle/funding-envelope guard proofs); NB2
# needs a bounded-work AND a rank/terminal artifact for EVERY selected crank
# continuation, not just clock advance.
NB = {
    # NB1 admission is now PROVEN-AT-KERNEL: kernel_economically_valid_trade_admits
    # proves admit IFF EconomicallyValidTradeV16 (over production inputs) — every
    # rejection maps to a false economic precondition, so no economically-valid
    # trade is internally DoSed at the guard composition. The guard summary is
    # faithful to production (route-fidelity roster); the production trade-body
    # ROUTE to this guard stack over the symbolic two-account body is the
    # documented backstopped half.
    "NB1 valid trade admitted (admit IFF economically valid; full guard stack + fidelity)":
        (PROVEN_AT_KERNEL, HARNESS,
         ["contract_check_kernel_economically_valid_trade_admits",
          "contract_check_kernel_trade_admit",
          "contract_check_kernel_initial_margin_gate"]),
    # NB2 is now PROVEN-AT-KERNEL: select_progress_witness is proven TOTAL over
    # actionable summaries, and the nb2_continuation_matrix gate pins every one of
    # its 6 continuations to (a) an auto-crank dispatch arm, (b) a present rank /
    # terminal artifact, (c) the static per-account scan bound
    # (V16_MAX_PORTFOLIO_ASSETS_N). The rank-decrease over the full symbolic
    # monolith body and the max-shape CU envelope (wrapper/LiteSVM) are the
    # documented backstopped half.
    "NB2 finite crank progress (selector-total + per-continuation dispatch/rank/bounded)":
        (PROVEN_AT_KERNEL, HARNESS,
         ["contract_check_select_progress_witness",
          "closure_kernel_advance_close_ledger_rank_witness"]),
}

_cache = {}
def fns_in(path):
    if path not in _cache:
        _cache[path] = set(re.findall(r"\bfn\s+([A-Za-z0-9_]+)\s*\(", open(path).read()))
    return _cache[path]

# Composed L.sel oracle (3A.4): select_progress_witness proves that for ANY
# actionable summary a non-blocked continuation is deterministically selected,
# resolving overlapping classes — the overlap-safe gate-reachability existential.
# 3C step 4: this selector is now PRODUCTION-WIRED — permissionless_auto_crank_
# not_atomic builds the summary from real state (build_actionable_summary, each
# flag == its production eligibility predicate, mode-gated), assembles it via the
# proven actionable_summary_from_signals kernel, runs this selector, and
# dispatches the chosen continuation to the matching proven entrypoint.
COMPOSED_SELECTOR = ("src/v16_proofs.rs", "contract_check_select_progress_witness")
# 3C step 4 classifier-assembly fidelity (production self-classifying crank).
CLASSIFIER_ASSEMBLY = ("src/v16_proofs.rs", "contract_check_actionable_summary_from_signals")

missing = []
def check(label, status, path, fns):
    if status == BACKSTOPPED:
        return
    for fn in fns:
        if fn not in fns_in(path):
            missing.append((label, path, fn))

for cls, halves in ROSTER.items():
    for half in ("sel", "dec"):
        st, path, fns = halves[half]
        check(f"{cls} L.{half}", st, path, fns)
for label, (st, path, fns) in NB.items():
    check(label, st, path, fns)
check("composed L.sel selector", PROVEN, COMPOSED_SELECTOR[0], [COMPOSED_SELECTOR[1]])
check("classifier assembly fidelity", PROVEN, CLASSIFIER_ASSEMBLY[0], [CLASSIFIER_ASSEMBLY[1]])

if missing:
    print("LIVENESS ROSTER GAP(S):")
    for label, path, fn in missing:
        print(f"  {label}: missing artifact `{fn}` in {path}")
    sys.exit(1)

# rollup
tally = {}
for cls, halves in ROSTER.items():
    for half in ("sel", "dec"):
        tally[halves[half][0]] = tally.get(halves[half][0], 0) + 1

print(f"liveness roster OK: all {len(ROSTER)} ActionableState classes have both "
      f"L.sel and L.dec recorded with present artifacts; NB1/NB2 present.")
print(f"  halves: {tally.get(PROVEN,0)} PROVEN, {tally.get(PROVEN_AT_KERNEL,0)} "
      f"PROVEN-AT-KERNEL, {tally.get(BACKSTOPPED,0)} BACKSTOPPED")
print()
for cls, halves in ROSTER.items():
    s, d = halves["sel"][0], halves["dec"][0]
    print(f"  {cls:28s} L.sel={s:16s} L.dec={d}")
print()
for label, (st, _p, _f) in NB.items():
    print(f"  {label:28s} {st}")
print()
print("BACKSTOPPED halves are the documented intractable interior (route through "
      "the monolith / terminal realization) — validator+fuzz+gate evidence, not a "
      "single theorem. SCHED (external submission) remains a named assumption.")
