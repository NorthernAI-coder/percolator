#!/usr/bin/env python3
"""Workstream E certification gate: NO-LoF ENTRYPOINT ROSTER.

Every public `*_not_atomic` engine entrypoint must reach SOME machine-checkable
no-LoF evidence — directly in its body OR through the fns it calls (thin
entrypoints delegate to validating `_core` fns). Evidence kinds:

  FLOW   — a TokenValueFlowProofV16 double-entry value-flow conservation (with
           vault-delta typing), the strongest per-op no-LoF tie.
  FLOOR  — commits through validate_shape / validate_with_market / *_audit_scan
           (the Pillar-F GlobalValidState floor on the Ok exit).
  KERNEL — routes value math through a proven conservation kernel (kernel_* / the
           named conservation kernels).

The gate FAILS if any entrypoint reaches NO evidence within the call closure (an
unprotected value-moving surface). This is the COVERAGE roster (it prevents an
entrypoint shipping with zero no-LoF assurance); the per-op no-LoF THEOREMS are
workstream B.1 + Phase-1 validator soundness. Exit code = #unprotected (0=clean).
"""
import re
import sys
from collections import deque
from pathlib import Path

SRC = "src/v16.rs"
ENTRY = re.compile(r"^\s*pub fn ([a-z_]+_not_atomic)\s*[(<]")
FN_ANY = re.compile(r"^\s*(?:pub(?:\(crate\))?\s+)?fn\s+([A-Za-z0-9_]+)\s*[(<]")
FLOW = re.compile(r"TokenValueFlowProofV16|validate_\w*_to_\w+")
FLOOR = re.compile(
    r"validate_shape|validate_with_market|validate_account_audit_scan|validate_shape_audit_scan"
    # config-level / empty-activation validity floors used by structural setup ops
    r"|validate_public_user_fund_shape|validate_public_\w+_shape"
    r"|is_empty_for_activation|asset_state_is_empty|validate_unconfigured_market_tail"
)
KERNEL = re.compile(r"\b(kernel_[a-z_]+|apply_bankruptcy_residual_chunk_to_loss_side|social_loss_book_split)\s*\(")
CALL = re.compile(r"\b([a-z_][a-z0-9_]+)\s*\(")
MAX_DEPTH = 5


def all_fn_bodies(lines):
    bodies = {}
    i, n = 0, len(lines)
    while i < n:
        m = FN_ANY.match(lines[i])
        if not m:
            i += 1
            continue
        name = m.group(1)
        depth, started, body, j = 0, False, [], i
        while j < n:
            line = lines[j]
            body.append(line)
            depth += line.count("{") - line.count("}")
            if "{" in line:
                started = True
            if started and depth <= 0:
                break
            j += 1
        bodies.setdefault(name, "\n".join(body))
        i = j + 1
    return bodies


def main():
    lines = Path(SRC).read_text().splitlines()
    bodies = all_fn_bodies(lines)

    def kinds(body):
        k = []
        if FLOW.search(body):
            k.append("FLOW")
        if FLOOR.search(body):
            k.append("FLOOR")
        if KERNEL.search(body):
            k.append("KERNEL")
        return k

    direct = {name: kinds(body) for name, body in bodies.items()}
    calls = {name: set(c for c in CALL.findall(body) if c in bodies and c != name)
             for name, body in bodies.items()}

    entrypoints = sorted(m.group(1) for m in (ENTRY.match(l) for l in lines) if m)
    rows, unprotected = [], []
    for ep in entrypoints:
        # BFS the call closure; collect the evidence kinds reachable.
        seen, q, found = {ep}, deque([(ep, 0)]), set()
        while q:
            cur, d = q.popleft()
            found.update(direct.get(cur, []))
            if d < MAX_DEPTH:
                for c in calls.get(cur, ()):
                    if c not in seen:
                        seen.add(c)
                        q.append((c, d + 1))
        ev = [k for k in ("FLOW", "FLOOR", "KERNEL") if k in found]
        rows.append((ep, ev))
        if not ev:
            unprotected.append(ep)

    if unprotected:
        print("NO-LoF COVERAGE GAP(S): entrypoint reaching NO no-LoF evidence in its call closure:")
        for u in unprotected:
            print(f"  `{u}`")
        return len(unprotected)

    flow = sum(1 for _, e in rows if "FLOW" in e)
    floor = sum(1 for _, e in rows if "FLOOR" in e)
    kern = sum(1 for _, e in rows if "KERNEL" in e)
    print(f"no-LoF entrypoint roster OK: all {len(rows)} *_not_atomic entrypoints reach "
          f"no-LoF evidence ({flow} FLOW, {floor} FLOOR, {kern} KERNEL; overlaps counted).")
    for name, ev in rows:
        print(f"  {name:62s} {'+'.join(ev)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
