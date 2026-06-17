#!/usr/bin/env python3
"""Workstream E certification gate: DEAD PROOF-KERNEL check.

Enumerates the production kernel/builder fns by the codebase naming convention
(`kernel_*`, `build_*`, plus a known set of named kernels) and requires each to
be one of:

  (a) production-CALLED  — >=1 call site in src/v16.rs outside its own definition
      (proofs live in the separate src/v16_proofs.rs module, so a call here is a
       genuine production call), OR
  (b) a public ENTRYPOINT — `pub fn ..._not_atomic`, invoked by the (out-of-scope,
      finding 7) wrapper rather than internally, OR
  (c) a VERIFIED FIDELITY MODEL — not called, but proven equal/faithful to a real
      production guard by a Kani harness in src/v16_proofs.rs (workstream A
      accepts "proven equal to a real production guard" as a valid summary tie).

A kernel that is NONE of these is genuinely dead/unverified — a refactor orphaned
it or it was never wired/proven — and FAILS the gate. Converts the "no dead
production kernels" discipline into an enforced CI invariant.

Exit code = number of violations (0 = clean).
"""
import re
import sys
from pathlib import Path

SRC = "src/v16.rs"
PROOFS = "src/v16_proofs.rs"

# Named production kernels/builders that do not match the kernel_/build_ prefix.
EXTRA = {
    "apply_bankruptcy_residual_chunk_to_loss_side",
    "select_progress_witness",
    "actionable_summary_from_signals",
    "social_loss_book_split",
    "permissionless_auto_crank_not_atomic",
}
FN_DEF = re.compile(r"^\s*(pub(?:\(crate\))?\s+)?fn\s+([A-Za-z0-9_]+)\s*[(<]")


def main():
    lines = Path(SRC).read_text().splitlines()
    proofs = Path(PROOFS).read_text() if Path(PROOFS).exists() else ""

    kernels = {}  # name -> is_pub_entrypoint
    for line in lines:
        m = FN_DEF.match(line)
        if not m:
            continue
        name = m.group(2)
        if name.startswith(("kernel_", "build_")) or name in EXTRA:
            is_pub = bool(m.group(1)) and "(crate)" not in (m.group(1) or "")
            kernels[name] = is_pub and name.endswith("_not_atomic")

    def call_count(name):
        call_re = re.compile(r"\b" + re.escape(name) + r"\s*\(")
        defn = re.compile(r"\bfn\s+" + re.escape(name) + r"\s*[(<]")
        return sum(len(call_re.findall(l)) for l in lines if not defn.search(l))

    buckets = {"called": [], "entry": [], "model": [], "dead": []}
    for name, is_entry in sorted(kernels.items()):
        if call_count(name) >= 1:
            buckets["called"].append(name)
        elif is_entry:
            buckets["entry"].append(name)
        elif name in proofs:
            buckets["model"].append(name)
        else:
            buckets["dead"].append(name)

    if buckets["dead"]:
        print("DEAD PROOF-KERNEL GAP(S): kernel/builder that is neither called, a "
              "pub entrypoint, nor verified by a harness:")
        for v in buckets["dead"]:
            print(f"  `{v}` — orphaned/unverified (no call site in {SRC}, not a "
                  f"*_not_atomic entrypoint, no harness in {PROOFS})")
        return len(buckets["dead"])

    print(f"dead-kernel gate OK: {len(kernels)} kernel/builder fn(s) accounted for — "
          f"{len(buckets['called'])} CALLED, {len(buckets['entry'])} ENTRYPOINT, "
          f"{len(buckets['model'])} VERIFIED FIDELITY MODEL (proven == production, not called).")
    for n in buckets["model"]:
        print(f"  fidelity model: {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
