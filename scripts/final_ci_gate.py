#!/usr/bin/env python3
"""Phase 6 (/tmp/proofs.md) FINAL CI GATE.

Single entrypoint that runs every ENGINE-side certification gate and fails if any
does not pass. This is the conditional-10/10 signoff for the engine repo; the
wrapper-side conditions (12-14: wrapper pin matches certified commit, wrapper Kani
imports engine contracts, max-shape LiteSVM/CU) live in percolator-prog and are
reported here as out-of-engine-scope.

Run from the repo root:  python3 scripts/final_ci_gate.py
Add --with-conformance to also run the reference/differential fuzz suite (slower).

Exit code = number of failing engine gates (0 = engine signoff clean).
"""
import subprocess
import sys

# (label, argv) for argless-or-fixed-arg engine gates.
GATES = [
    ("boundary_audit (Ok-exit floor)", ["python3", "scripts/boundary_audit.py"]),
    ("no_lof_entrypoint_roster", ["python3", "scripts/no_lof_entrypoint_roster.py"]),
    ("no_lof_strength_roster (THEOREM vs FLOOR)", ["python3", "scripts/no_lof_strength_roster.py"]),
    ("lof_transition_class_roster", ["python3", "scripts/lof_transition_class_roster.py"]),
    ("route_fidelity_roster", ["python3", "scripts/route_fidelity_roster.py"]),
    ("guard_mutation_sensitivity", ["python3", "scripts/guard_mutation_sensitivity.py"]),
    ("actionable_class_coverage", ["python3", "scripts/actionable_class_coverage.py"]),
    ("liveness_roster_enforcement (NB1/NB2 not PARTIAL)", ["python3", "scripts/liveness_roster_enforcement.py"]),
    ("nb2_continuation_matrix", ["python3", "scripts/nb2_continuation_matrix.py"]),
    ("dead_kernel_check", ["python3", "scripts/dead_kernel_check.py"]),
    ("arithmetic_axiom_manifest_check", ["python3", "scripts/arithmetic_axiom_manifest_check.py"]),
    ("identity_independence_audit", ["python3", "scripts/identity_independence_audit.py"]),
    ("symbolic_assert_audit (proof sources)",
     ["python3", "scripts/symbolic_assert_audit.py", "src/v16_proofs.rs", "tests/proofs_v16.rs"]),
    ("engine_contract_manifest (wrapper surface)", ["python3", "scripts/engine_contract_manifest.py"]),
]

# Engine gates that require captured Kani per-harness logs (run in CI after the
# Kani run, over the log dir) — reported, not run here.
LOG_GATES = [
    "cover_vacuity_gate.py <kani-log-dirs>  (run over captured Kani harness logs)",
]

# Wrapper-side conditions (percolator-prog) — out of engine scope (finding 7).
WRAPPER = [
    "wrapper engine pin matches the certified engine commit",
    "wrapper Kani proofs import engine_contracts.json, prove only wrapper boundaries",
    "max-shape LiteSVM / CU tests for normal ops + crank continuations",
]


def main():
    with_conf = "--with-conformance" in sys.argv
    failed = []
    print("=== ENGINE CERTIFICATION GATES ===")
    for label, argv in GATES:
        r = subprocess.run(argv, capture_output=True, text=True)
        ok = r.returncode == 0
        print(f"  [{'PASS' if ok else 'FAIL'}] {label}")
        if not ok:
            failed.append(label)
            tail = (r.stdout + r.stderr).strip().splitlines()[-4:]
            for ln in tail:
                print(f"          {ln}")

    if with_conf:
        print("  running reference/differential conformance suite ...")
        r = subprocess.run(
            ["cargo", "test", "--features", "fuzz", "--test", "reference_model_conformance"],
            capture_output=True, text=True)
        ok = r.returncode == 0
        print(f"  [{'PASS' if ok else 'FAIL'}] reference_model_conformance")
        if not ok:
            failed.append("reference_model_conformance")

    print("\n=== ENGINE GATES REQUIRING CAPTURED KANI LOGS (run in CI) ===")
    for g in LOG_GATES:
        print(f"  - {g}")
    print("\n=== WRAPPER-SIDE CONDITIONS (percolator-prog, out of engine scope) ===")
    for w in WRAPPER:
        print(f"  - {w}")

    print()
    if failed:
        print(f"FINAL CI GATE: FAIL — {len(failed)} engine gate(s) failed: {failed}")
        return len(failed)
    print("FINAL CI GATE: engine signoff CLEAN — all engine certification gates pass.")
    print("Conditional 10/10 holds UNDER: SVM rollback, SCHED, the named arithmetic")
    print("axiom manifest, and wrapper routing/auth/oracle (see arithmetic_policy.md,")
    print("engine_contracts.json). NB1/NB2 are PROVEN-AT-KERNEL; full-monolith routes")
    print("and max-shape CU remain the documented backstopped/ wrapper halves.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
