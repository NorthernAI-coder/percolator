#!/usr/bin/env python3
"""Workstream E certification gate: ROUTE-FIDELITY ROSTER.

Every field of a compact decision summary (the *_Summary / *_Rank / *_Guard
structs the proven kernels consume) must be tied to production: it must appear in
a Kani fidelity contract in src/v16_proofs.rs, where the contract proves that
field EQUALS its real production predicate/guard (so the compact kernel is a
faithful representation of the monolith decision, not a proof-only model that
could drift from production).

A summary field that appears in NO fidelity contract is an un-tied field — the
kernel could compute it from anything and no proof would catch the divergence —
and FAILS the gate. This converts "every compact summary field maps to
production code / external assumption / PARTIAL" into an enforced invariant.

Exit code = number of un-tied fields (0 = clean).
"""
import re
import sys
from pathlib import Path

SRC = "src/v16.rs"
PROOFS = "src/v16_proofs.rs"
STRUCT = re.compile(r"^pub struct (\w+(?:Summary|Rank|Guard)\w*V16)\s*\{")
FIELD = re.compile(r"^\s*pub ([a-z_][a-z0-9_]*)\s*:")

# TradeGuardSummaryV16 is a COMPOSITION summary: each field is the output of a
# separately-proven leaf kernel (not a single `.field ==` builder ensures). The
# tie is the named leaf's fidelity contract; this maps field -> the harness that
# proves it equals its real production guard.
COMPOSED_TIE = {
    "TradeGuardSummaryV16": {
        "request_valid": "contract_check_build_trade_request_guard_summary",
        "accounts_current": "contract_check_kernel_cert_is_current",
        "no_loss_stale_block": "contract_check_kernel_trade_preflight_admits",
        "no_adverse_lag": "contract_check_kernel_trade_preflight_admits",
        "no_barrier_touch": "contract_check_kernel_trade_preflight_admits",
        "margin_ok": "contract_check_kernel_initial_margin_gate",
        "locked_lane_ok": "contract_check_kernel_locked_margin_gate",
    },
}

# Fields whose fidelity is structural / proven elsewhere (documented exceptions).
EXEMPT = set()


def summary_fields(lines):
    out = {}
    i, n = 0, len(lines)
    while i < n:
        m = STRUCT.match(lines[i])
        if not m:
            i += 1
            continue
        name = m.group(1)
        fields = []
        j = i + 1
        while j < n and not lines[j].startswith("}"):
            fm = FIELD.match(lines[j])
            if fm:
                fields.append(fm.group(1))
            j += 1
        out[name] = fields
        i = j + 1
    return out


def main():
    lines = Path(SRC).read_text().splitlines()
    # The fidelity `kani::ensures` clauses live on the builder/kernel in src/v16.rs
    # (e.g. `r.stale == stale`); the harness in v16_proofs.rs only calls them.
    # Search both for the contract-equality signature `.<field> ==`.
    haystack = Path(SRC).read_text() + "\n" + (Path(PROOFS).read_text() if Path(PROOFS).exists() else "")
    summaries = summary_fields(lines)

    violations = []
    rows = []
    for sname, fields in sorted(summaries.items()):
        tied, untied = [], []
        for f in fields:
            # a fidelity tie is an ensures equality on the field: `.<f> ==`
            # (matches r.<f> ==, result.<f> ==), NOT struct construction (`<f>:`).
            pat = re.compile(r"\." + re.escape(f) + r"\s*==")
            leaf = COMPOSED_TIE.get(sname, {}).get(f)
            if f in EXEMPT or pat.search(haystack):
                tied.append(f)
            elif leaf and leaf in haystack:
                tied.append(f)  # composition field, tied via the named leaf harness
            else:
                untied.append(f)
                violations.append((sname, f))
        rows.append((sname, len(fields), tied, untied))

    if violations:
        print("ROUTE-FIDELITY GAP(S): compact summary field with NO fidelity contract tie:")
        for sname, f in violations:
            print(f"  {sname}.{f} — appears in no fidelity contract in {PROOFS}")
        return len(violations)

    total = sum(n for _, n, _, _ in rows)
    print(f"route-fidelity roster OK: all {total} fields across {len(rows)} compact "
          f"summary struct(s) are tied to a production predicate by a fidelity contract.")
    for sname, n, tied, _ in rows:
        print(f"  {sname:30s} {n} field(s) tied")
    return 0


if __name__ == "__main__":
    sys.exit(main())
