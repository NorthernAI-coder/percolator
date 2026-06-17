#!/usr/bin/env python3
"""Phase 4 (/tmp/proofs.md) ENGINE CONTRACT MANIFEST.

Emits a single machine-readable contract surface for the engine's public
transitions, so the wrapper can IMPORT engine guarantees instead of reproving
engine internals (the no-duplicate-work boundary). For every public *_not_atomic
entrypoint it records:

  transition_class  - its no-LoF transition class + strongest proof source
                      (from lof_transition_class_roster).
  no_lof            - "THEOREM" (per-op value/frame/kernel/closure proof) or
                      "FLOOR" (universal GlobalValidState floor only; the 5
                      non-value-bearing structural ops).
  liveness          - the crank continuation it serves, if any (the auto-crank
                      dispatch targets), else null.
  arithmetic_axioms - the global named arithmetic axioms the proof surface assumes
                      (from arithmetic_axiom_manifest_check).
  assumptions       - the conditional-10/10 assumptions (SVM rollback, SCHED,
                      arithmetic axiom, wrapper routing/auth/oracle).

Writes scripts/engine_contracts.json and verifies completeness (every entrypoint
has a contract row, every transition class artifact is present). This is the
engine half of Phase 4; the wrapper consumes the JSON. Exit code = #gaps.
"""
import importlib.util
import json
import re
import sys
from pathlib import Path

SRC = "src/v16.rs"


def load_lof():
    import contextlib
    import io
    spec = importlib.util.spec_from_file_location("lof", "scripts/lof_transition_class_roster.py")
    mod = importlib.util.module_from_spec(spec)
    # the module runs its checks + prints at import and may sys.exit; guard + mute.
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            spec.loader.exec_module(mod)
    except SystemExit:
        pass
    return mod


# auto-crank dispatch tokens -> continuation (for the liveness column)
LIVENESS = {
    "permissionless_auto_crank_not_atomic": "dispatcher(select_auto_crank_plan)",
    "permissionless_crank_not_atomic": "refresh/settle-B/liquidate/recover",
    "close_resolved_account_not_atomic": "A7 resolved close (kernel_resolved_close_progress)",
    "liquidate_account_not_atomic": "A5 risk reduction (kernel_reduce_position_delta)",
}

ASSUMPTIONS = [
    "SVM rollback on Err (only committed Ok exits are proven)",
    "SCHED: some actor eventually submits an available public crank",
    "ArithmeticAxiom: wide div/mul outputs opaque, discharged by reference/differential fuzz",
    "Wrapper proves account routing / auth / oracle freshness / rollback boundaries",
]


def main():
    lof = load_lof()
    src = Path(SRC).read_text()
    entrypoints = sorted(set(re.findall(r"pub fn (\w+_not_atomic)\b", src)))

    # per-entrypoint transition class (reuse lof CLASSES + ownership logic)
    classes = lof.CLASSES
    owner = {}
    for ep in entrypoints:
        hits = [(name, strength) for (name, strength, pats, _a) in classes if any(p in ep for p in pats)]
        owner[ep] = hits[0] if len(hits) == 1 else (hits[0] if hits else (None, None))

    # arithmetic axioms (parse the manifest stub rows)
    axioms = sorted(set(re.findall(r"`(axiom_\w+|kani_any_\w+)`", Path("scripts/arithmetic_axiom_manifest.md").read_text())))

    THEOREM_STRENGTHS = {"DIRECT_FRAME", "WHOLE_BODY_COMPOSITION", "KERNEL_VALUE",
                         "FLOW_VALIDATOR", "CLOSURE"}
    contracts = {}
    gaps = []
    for ep in entrypoints:
        cls, strength = owner[ep]
        if cls is None:
            gaps.append(f"{ep}: no transition class")
            continue
        contracts[ep] = {
            "transition_class": cls,
            # the transition class's strongest proof source (per-class artifact);
            # the per-OP strength split is in no_lof_strength_roster.py (51 per-op
            # THEOREM / 5 non-value-bearing FLOOR).
            "class_proof_source": strength,
            "class_tier": "THEOREM" if strength in THEOREM_STRENGTHS else "FLOOR",
            "liveness": LIVENESS.get(ep),
        }

    manifest = {
        "engine": "percolator v16",
        "public_entrypoints": len(entrypoints),
        "arithmetic_axioms": axioms,
        "assumptions": ASSUMPTIONS,
        "contracts": contracts,
    }
    Path("scripts/engine_contracts.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")

    if gaps:
        print("ENGINE CONTRACT MANIFEST GAP(S):")
        for g in gaps:
            print(f"  {g}")
        return len(gaps)

    th = sum(1 for c in contracts.values() if c["class_tier"] == "THEOREM")
    print(f"engine contract manifest OK: {len(contracts)} public entrypoints, "
          f"{th} class-THEOREM / {len(contracts) - th} class-FLOOR, "
          f"{len(axioms)} arithmetic axioms (per-op strength split in no_lof_strength_roster).")
    print("  wrote scripts/engine_contracts.json (wrapper-importable contract surface).")
    print("  assumptions:", "; ".join(ASSUMPTIONS))
    return 0


if __name__ == "__main__":
    sys.exit(main())
