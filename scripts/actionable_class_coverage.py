#!/usr/bin/env python3
"""no-DoS liveness: every ActionableState class is covered by >=1 named,
machine-checked witness proof (the review's "roster check" — /tmp/proofs.md
step 6 / line 291).

ActionableState is the 7-class disjunction in scripts/no-dos-liveness.md. For
no-DoS to be "every actionable state has a successful bounded continuation",
each class must map to at least one EXISTING proof artifact that witnesses its
continuation:

  - the two kernel-backed classes (A2 b-stale, A3 pending close) have a
    machine-checked EXISTENTIAL (ActionableClass(S) => exists a successful
    rank-decreasing call) — the gate-reachability half the review asked for;
  - the five terminal-route classes (A1 stale, A4 expired, A5 liquidatable,
    A6 recovery, A7 resolved) have a suite success-witness proving the public
    continuation succeeds and records its terminal/protective effect.

This script fails if any class loses its witness (e.g. a proof is renamed or
deleted) — turning the class->continuation table into an enforced invariant
rather than prose. It does NOT re-run the proofs; it checks the roster is
intact at the current tree. Run the Kani/suite layers for the proofs themselves.
"""
import re
import sys

# class -> (kind, file, list of fn names that must ALL be present)
COVERAGE = {
    "A1 stale account": (
        "terminal/protective suite witness",
        "tests/proofs_v16.rs",
        ["proof_v16_equity_active_accrual_with_progress_commits_one_bounded_segment"],
    ),
    "A2 b-stale leg": (
        "machine-checked existential (rank-decreasing chunk)",
        "src/v16_proofs.rs",
        ["liveness_b_stale_leg_has_advancing_chunk"],
    ),
    "A3 pending close residual": (
        "machine-checked existential (rank-decreasing advance)",
        "src/v16_proofs.rs",
        ["liveness_pending_close_has_rank_decreasing_advance"],
    ),
    "A4 expired close": (
        "terminal-route suite witness",
        "tests/proofs_v16.rs",
        ["proof_v16_expired_close_progress_declares_recovery_without_value_mutation"],
    ),
    "A5 liquidatable": (
        "terminal-route suite witness (accept + route)",
        "tests/proofs_v16.rs",
        [
            "proof_v16_liquidation_preflight_accepts_only_fully_durable_residual",
            "proof_v16_liquidation_preflight_routes_insufficient_residual_capacity_to_recovery",
        ],
    ),
    "A6 recovery-eligible": (
        "terminal-route suite witness (accounting-neutral)",
        "tests/proofs_v16.rs",
        ["proof_v16_permissionless_recovery_crank_is_accounting_neutral"],
    ),
    "A7 resolved winner": (
        "terminal-realization suite witness",
        "tests/proofs_v16.rs",
        [
            "proof_v16_resolved_winddown_releases_liened_source_claim",
            "proof_v16_public_resolved_close_flat_account_pays_only_capital_and_vault",
        ],
    ),
}

_cache = {}


def fns_in(path):
    if path not in _cache:
        text = open(path).read()
        _cache[path] = set(re.findall(r"\bfn\s+([A-Za-z0-9_]+)\s*\(", text))
    return _cache[path]


missing = []
for cls, (kind, path, fns) in COVERAGE.items():
    present = fns_in(path)
    for fn in fns:
        if fn not in present:
            missing.append((cls, path, fn))

if missing:
    print("ACTIONABLE-CLASS COVERAGE GAP(S):")
    for cls, path, fn in missing:
        print(f"  {cls}: missing witness `{fn}` in {path}")
    sys.exit(1)

print(f"actionable-class coverage OK: all {len(COVERAGE)} ActionableState classes")
print("have a present, named machine-checked witness:")
for cls, (kind, path, fns) in COVERAGE.items():
    print(f"  {cls:28s} -> {kind}")
    for fn in fns:
        print(f"       {path}::{fn}")
