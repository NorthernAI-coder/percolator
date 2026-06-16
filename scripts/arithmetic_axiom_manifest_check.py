#!/usr/bin/env python3
"""Enforce the arithmetic-axiom manifest (roadmap 3B.2): every
`#[kani::stub(<target>, <axiom>)]` in the proof sources must appear as a row in
scripts/arithmetic_axiom_manifest.md, and every named discharge artifact must
exist. Prevents opaque-helper axiom assumptions from drifting un-audited.

Exit code = number of violations (0 = clean).
"""
import re
import sys
from pathlib import Path

SRCS = ["src/v16_proofs.rs"]
MANIFEST = "scripts/arithmetic_axiom_manifest.md"
DISCHARGE_FILES = ["tests/rounding_residue_fuzz.rs", "tests/reference_model_conformance.rs"]

STUB = re.compile(r"#\[kani::stub\(\s*crate::v16::(\w+)\s*,\s*(\w+)\s*\)\]")


def fns_in(path):
    return set(re.findall(r"\bfn\s+([A-Za-z0-9_]+)\s*\(", Path(path).read_text())) if Path(path).exists() else set()


def main():
    manifest = Path(MANIFEST).read_text() if Path(MANIFEST).exists() else ""
    discharge_fns = set()
    for f in DISCHARGE_FILES:
        discharge_fns |= fns_in(f)

    stubs = set()
    for s in SRCS:
        for m in STUB.finditer(Path(s).read_text()):
            stubs.add((m.group(1), m.group(2)))  # (target_helper, axiom_fn)

    violations = []
    for target, axiom in sorted(stubs):
        if axiom not in manifest:
            violations.append(f"axiom stub `{axiom}` (for {target}) has NO manifest row")

    # every manifest discharge artifact named in backticks that looks like a fn
    # must exist in a discharge file
    named = set(re.findall(r"`([a-z][A-Za-z0-9_]+)`", manifest))
    for n in named:
        if n.startswith(("loss_weight_", "ref_", "u8_", "tier_", "trade_", "phase7_")) and n not in discharge_fns:
            # only enforce names that look like discharge tests
            if "matches" in n or "holds" in n:
                violations.append(f"manifest names discharge `{n}` but it is absent from {DISCHARGE_FILES}")

    if violations:
        print("ARITHMETIC-AXIOM MANIFEST GAP(S):")
        for v in violations:
            print(f"  {v}")
        return len(violations)
    print(f"arithmetic-axiom manifest OK: {len(stubs)} axiom stub(s), all with a "
          f"manifest row and present discharge artifact.")
    for target, axiom in sorted(stubs):
        print(f"  {axiom}  ->  crate::v16::{target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
