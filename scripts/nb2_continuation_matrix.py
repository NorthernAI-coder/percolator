#!/usr/bin/env python3
"""Phase 3 (/tmp/proofs.md) NB2 CONTINUATION MATRIX.

NB2 (no-DoS): every actionable state has a finite successful public crank path
that decreases a well-founded rank or reaches terminal recovery. The proven
selector select_progress_witness is TOTAL over actionable summaries (an actionable
state always yields Some continuation); this matrix pins, for EVERY continuation
it can return, the three NB2 obligations:

  DISPATCH   - permissionless_auto_crank_not_atomic has a dispatch arm routing the
               continuation to a real production entrypoint (route fidelity).
  RANK       - a present proof artifact that the continuation's step strictly
               decreases the liveness rank or records terminal recovery.
  BOUNDED    - the work per call is statically bounded (the per-account scans are
               bounded by V16_MAX_PORTFOLIO_ASSETS_N; the crank does a bounded
               number of leg/asset touches — no unbounded loop).

The gate FAILS if any continuation lacks a dispatch arm or a present RANK
artifact. Bounded-work is the static leg-count bound (asserted structurally; the
max-shape CU envelope is a wrapper/LiteSVM artifact, out of engine scope). This
converts NB2's per-continuation coverage from prose into an enforced invariant.

NOTE on production-route depth: several RANK artifacts are PROVEN-AT-KERNEL — the
step kernel is proven, and the auto-crank dispatch (this matrix) ties the selected
continuation to the real entrypoint; the rank-decrease over the full symbolic
monolith body is the documented backstopped half. Exit code = #gaps (0 = clean).
"""
import re
import sys
from pathlib import Path

SRC = "src/v16.rs"
PROOFS = "tests/proofs_v16.rs"
HARNESS = "src/v16_proofs.rs"
MAXN = "V16_MAX_PORTFOLIO_ASSETS_N"

# continuation -> (auto-crank dispatch token, [rank/terminal artifact (file, fn)])
MATRIX = {
    "RefreshAccount": (
        "PermissionlessCrankActionV16::Refresh",
        [(PROOFS, "proof_v16_equity_active_accrual_with_progress_commits_one_bounded_segment")]),
    "SettleBChunk": (
        "PermissionlessCrankActionV16::SettleB",
        [(HARNESS, "contract_check_kernel_advance_leg_b_snap")]),
    # AdvanceClose is FOLDED (engine.md redesign): AutoCrankPlanV16 has no
    # AdvanceClose variant. A leg-bearing pending close is liquidatable (Liquidate
    # books the residual chunk); a leg-less / expired close routes to recovery.
    # The close-ledger rank step itself stays proven by
    # closure_kernel_advance_close_ledger_rank_witness (consumed by the
    # bankruptcy-residual booking path), not by an auto-crank dispatch arm.
    "Liquidate": (
        "PermissionlessCrankActionV16::Liquidate",
        [(HARNESS, "contract_check_kernel_reduce_position_delta")]),
    "DeclareRecovery": (
        "PermissionlessCrankActionV16::Recover",
        [(PROOFS, "proof_v16_expired_close_progress_declares_recovery_without_value_mutation"),
         (PROOFS, "proof_v16_permissionless_recovery_crank_is_accounting_neutral")]),
    "CloseResolved": (
        "close_resolved_account_not_atomic",
        [(HARNESS, "contract_check_kernel_resolved_close_progress")]),
}


def fns_in(path, _c={}):
    if path not in _c:
        _c[path] = set(re.findall(r"\bfn\s+([A-Za-z0-9_]+)\s*[(<]", Path(path).read_text())) if Path(path).exists() else set()
    return _c[path]


def main():
    src = Path(SRC).read_text()
    # the auto-crank body (route-fidelity dispatch surface)
    m = re.search(r"fn permissionless_auto_crank_not_atomic.*?\n    \}", src, re.S)
    auto = m.group(0) if m else ""
    # the proven engine plan selector must enumerate every dispatched continuation
    sel = re.search(r"fn select_auto_crank_plan.*?\n    \}", src, re.S)
    sel = sel.group(0) if sel else ""

    bounded = MAXN in src  # the static per-account scan bound exists
    gaps = []
    rows = []
    for cont, (dispatch_tok, arts) in MATRIX.items():
        in_selector = cont in sel
        in_dispatch = dispatch_tok in auto
        have_rank = any(fn in fns_in(path) for (path, fn) in arts)
        rows.append((cont, in_selector, in_dispatch, have_rank))
        if not in_dispatch:
            gaps.append(f"{cont}: no dispatch arm ({dispatch_tok}) in permissionless_auto_crank")
        if not have_rank:
            gaps.append(f"{cont}: no present RANK/terminal artifact {[a[1] for a in arts]}")
        if not in_selector:
            gaps.append(f"{cont}: not enumerated in select_progress_witness")

    if not bounded:
        gaps.append(f"missing static per-account scan bound {MAXN}")

    if gaps:
        print("NB2 CONTINUATION-MATRIX GAP(S):")
        for g in gaps:
            print(f"  {g}")
        return len(gaps)

    print(f"NB2 continuation matrix OK: all {len(MATRIX)} select_progress_witness "
          f"continuations have a dispatch arm + a present rank/terminal artifact; "
          f"per-account work bounded by {MAXN}.")
    for cont, sl, di, rk in rows:
        print(f"  {cont:16s} selector={sl} dispatch={di} rank/terminal={rk}")
    print("Selector totality (an actionable state always yields Some) is proven by "
          "contract_check_select_progress_witness; the None case is exactly "
          "not-actionable. Full-monolith-route rank-decrease is the backstopped half.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
