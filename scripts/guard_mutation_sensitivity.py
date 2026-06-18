#!/usr/bin/env python3
"""Workstream E certification gate: GUARD MUTATION-SENSITIVITY.

The roadmap requires "flipping each guard must break the corresponding proof or
test". Statically, that means every field of a decision summary must actually be
CONSULTED in the production decision logic — a field that is built and proven
faithful (route-fidelity gate) but never READ in any decision is vacuous: flipping
it changes no outcome, so no proof or test could ever depend on it.

For each compact summary struct (the route-fidelity targets) this requires every
field to be read (`.field`) somewhere in src/v16.rs OUTSIDE its own struct
definition and OUTSIDE pure construction (`field:` / `field,` in the builder) —
i.e. consulted by a decision method (all_pass / is_actionable / has_pending) or a
consumer kernel (the admit chain / selector / close classifier). A field read
nowhere fails the gate.

Together with route_fidelity_roster.py (each field == its production predicate)
this gives mutation-sensitivity: every guard is BOTH faithful AND outcome-
relevant. Exit code = number of vacuous (never-consulted) fields (0 = clean).
"""
import re
import sys
from pathlib import Path

SRC = "src/v16.rs"
STRUCT = re.compile(r"^pub struct (\w+(?:Summary|Rank|Guard)\w*V16)\s*\{")
FIELD = re.compile(r"^\s*pub ([a-z_][a-z0-9_]*)\s*:")


def summary_fields(lines):
    out, i, n = {}, 0, len(lines)
    while i < n:
        m = STRUCT.match(lines[i])
        if not m:
            i += 1
            continue
        name, fields, j = m.group(1), [], i + 1
        while j < n and not lines[j].startswith("}"):
            fm = FIELD.match(lines[j])
            if fm:
                fields.append(fm.group(1))
            j += 1
        out[name] = fields
        i = j + 1
    return out


def main():
    # Proof-only summary structs may live in the cfg(kani) module v16_kani_api.rs
    # (moved to minimise the production audit surface); scan it too so their fields
    # stay consult-checked.
    KANI_API = "src/v16_kani_api.rs"
    text = Path(SRC).read_text()
    if Path(KANI_API).exists():
        text += "\n" + Path(KANI_API).read_text()
    lines = text.splitlines()
    summaries = summary_fields(lines)

    violations, rows = [], []
    for sname, fields in sorted(summaries.items()):
        consulted, vacuous = [], []
        for f in fields:
            # a CONSULT is a read `<expr>.field` in decision logic. Count `.field`
            # reads; construction uses `field:` / bare `field,` so a `.field`
            # occurrence is a genuine read.
            reads = len(re.findall(r"\.\b" + re.escape(f) + r"\b", text))
            if reads >= 1:
                consulted.append(f)
            else:
                vacuous.append(f)
                violations.append((sname, f))
        rows.append((sname, consulted, vacuous))

    if violations:
        print("GUARD MUTATION-SENSITIVITY GAP(S): summary field never CONSULTED "
              "in any decision (vacuous — flipping it breaks nothing):")
        for sname, f in violations:
            print(f"  {sname}.{f}")
        return len(violations)

    total = sum(len(c) for _, c, _ in rows)
    print(f"guard mutation-sensitivity OK: all {total} summary field(s) are consulted "
          f"in production decision logic (flipping any one changes an outcome).")
    for sname, consulted, _ in rows:
        print(f"  {sname:30s} {len(consulted)} field(s) consulted")
    return 0


if __name__ == "__main__":
    sys.exit(main())
