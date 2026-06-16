#!/usr/bin/env python3
"""symbolic_assert_audit.py — heuristic auditor for Kani harnesses whose
assertions IGNORE their symbolic inputs (a vacuity class the cover gate cannot
catch: the cover is satisfiable but the asserted fact does not depend on what
the proof made symbolic).

Two heuristics, both producing CANDIDATES FOR MANUAL REVIEW (not verdicts):

  H1 NO-SYMBOLIC-ASSERT (taint): seed the symbolic identifiers (`let x = kani::
     any()` / `kani::any_where` / `: T = kani::any()`), propagate data-flow to a
     fixpoint through `let` bindings, struct-literal assignments and field/index
     mutations (`a.b = <tainted>`, `a[i] = <tainted>` taint `a`), then collect
     the identifiers used inside assert!/assert_eq!/assert_ne!. If NO assertion
     references any tainted identifier, the assertions hold independently of the
     symbolic inputs — the symbolic vars are decorative. (Real "X is always
     constant regardless of input" proofs land here too, so a hit is a prompt to
     read the harness, not proof of a defect.)

  H2 REFERENTIAL-TAUTOLOGY: an `assert_eq!(A, B)` whose two sides are the SAME
     call expression on the SAME argument identifiers with nothing mutated
     between -> true for any implementation (the #4 flavor).

Limitations (stated, not hidden): taint is textual/heuristic, so it MISSES
flavors where the assert DOES reference a symbolic-derived value but is welded
to a constant outcome by another variable (e.g. a welded discriminant bit) or
pins the wrong error. Those need property-level human judgment. This tool
narrows the manual surface; it does not replace it.

Usage: scripts/symbolic_assert_audit.py <file.rs> [<file.rs> ...]
Exit code = number of candidate harnesses flagged (0 = none).
"""
import re
import sys

HARNESS_FN = re.compile(r"\bfn\s+(proof_\w+|closure_\w+|contract_\w+|composition_\w+|liveness_\w+)\s*\(")
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
# symbolic sources: kani::any*(...) and the suite's kani_any*/any_*_fixture helpers
ANY = re.compile(r"\b(kani::any\w*|kani_any\w*|any_\w*_fixture|arbitrary)\s*(::<[^>]*>)?\s*\(")
LET = re.compile(r"^\s*let\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?::[^=]+)?=\s*(.*)$")
# a reassignment / field / index mutation:  <base>... = rhs ;
MUT = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:\.[A-Za-z0-9_]+|\[[^\]]*\])+\s*=\s*(.*)$")
ASSERT = re.compile(r"\bassert(?:_eq|_ne)?!\s*\(")

KEYWORDS = {"let", "mut", "if", "else", "match", "as", "true", "false", "Ok", "Err",
            "Some", "None", "return", "self", "unwrap", "get", "u128", "i128", "u64",
            "u32", "u8", "i64", "as_view", "try_to_runtime", "new", "from_runtime",
            "kani", "any", "assume", "cover", "assert", "assert_eq", "assert_ne"}


def split_harnesses(text):
    """Yield (name, body_text) for each harness fn via brace matching."""
    out = []
    for m in HARNESS_FN.finditer(text):
        name = m.group(1)
        i = text.find("{", m.end())
        if i < 0:
            continue
        depth, j = 0, i
        while j < len(text):
            c = text[j]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        out.append((name, text[i:j + 1]))
    return out


def idents(expr):
    return [w for w in IDENT.findall(expr) if w not in KEYWORDS]


def statements(body):
    """Split a harness body into logical statements at top-level ';' (depth 0
    w.r.t. () [] {}). Keeps multi-line struct literals / method chains intact."""
    out, buf, depth = [], "", 0
    for ch in body:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == ";" and depth <= 0:
            out.append(buf)
            buf = ""
        else:
            buf += ch
    if buf.strip():
        out.append(buf)
    return out


# `let`, `if let`, `while let`, `let ... else` bindings (pattern up to first `=`)
LET_BIND = re.compile(r"^\s*(?:if\s+let|while\s+let|let)\s+(?:mut\s+)?(.+?)\s*=\s*(.*)$", re.S)
MUT_BIND = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:\.[A-Za-z0-9_]+|\[[^\]]*\])+\s*=[^=].*$", re.S)


def bound_idents(lhs):
    """Idents introduced by a let pattern: handles `x`, `(a, b, c)`, `[a, b]`,
    `mut x`, type ascriptions stripped."""
    lhs = lhs.split(":")[0]  # drop type ascription on simple binds
    return [w for w in IDENT.findall(lhs) if w not in KEYWORDS]


def audit_harness(name, body):
    inner = body.strip()
    if inner.startswith("{") and inner.endswith("}"):
        inner = inner[1:-1]
    stmts = statements(inner)

    # --- taint seed + fixpoint over whole statements (multi-line aware) ---
    tainted = set()
    for s in stmts:
        if ANY.search(s):
            m = LET_BIND.match(s)
            if m:
                tainted.update(bound_idents(m.group(1)))
    changed = True
    while changed:
        changed = False
        for s in stmts:
            st = s.strip()
            if st.startswith("assert") or "kani::cover" in st:
                continue
            m = LET_BIND.match(s)
            if m:
                lhs, rhs = m.group(1), m.group(2)
                binds = bound_idents(lhs)
                if any(b not in tainted for b in binds) and any(t in idents(rhs) for t in tainted):
                    for b in binds:
                        tainted.add(b)
                    changed = True
                continue
            if MUT_BIND.match(s):
                base = MUT_BIND.match(s).group(1)
                rhs = s.split("=", 1)[1] if "=" in s else ""
                if base not in tainted and any(t in idents(rhs) for t in tainted):
                    tainted.add(base)
                    changed = True

    # --- collect assert argument identifiers (statements are multi-line) ---
    assert_idents = set()
    assert_calls = []
    for s in stmts:
        m = ASSERT.search(s)
        if not m:
            continue
        arg = s[m.end():]  # text after the opening assert(
        assert_calls.append(arg)
        for w in idents(arg):
            assert_idents.add(w)

    has_symbolic = bool(tainted)
    asserts_touch_symbolic = bool(assert_idents & tainted)

    flags = []
    # H1: symbolic inputs exist but no assertion depends on any of them
    if has_symbolic and assert_idents and not asserts_touch_symbolic:
        flags.append(("NO_SYMBOLIC_ASSERT",
                      f"{len(tainted)} symbolic-derived vars, but no assert references any "
                      f"(asserts use: {sorted(assert_idents)[:8]})"))

    # H2: referential-tautology assert_eq!(SAME_CALL(args), SAME_CALL(args))
    for a in assert_calls:
        # split top-level comma of assert_eq!
        if "," not in a:
            continue
        depth, sp = 0, None
        for k, ch in enumerate(a):
            if ch in "([{":
                depth += 1
            elif ch in ")]}":
                depth -= 1
            elif ch == "," and depth == 0:
                sp = k
                break
        if sp is None:
            continue
        lhs, rhs = a[:sp], a[sp + 1:]
        ln = re.sub(r"\s+", "", lhs)
        rn = re.sub(r"\s+", "", rhs)
        # a call expr on both sides, identical normalised text => tautology
        if "(" in ln and ln == rn and len(ln) > 12:
            flags.append(("REFERENTIAL_TAUTOLOGY",
                          f"assert_eq! compares an identical call to itself: {lhs.strip()[:60]}"))
            break
    return flags


# Harnesses manually reviewed and confirmed NON-vacuous — the flag is a known
# limitation of textual taint (the asserted values are read back from state an
# engine operation mutated, or built via conditional/struct flow), not a defect.
# Each line: harness -> why the assertion really does depend on the symbolic input.
REVIEWED_CLEAR = {
    "proof_v16_validate_shape_rejects_global_junior_bound_below_domain_claims":
        "rejection requires symbolic claim>0 (flows struct->from_runtime->field mutation)",
    "proof_v16_claim_resolved_payout_topup_preflight_rejects_unready":
        "symbolic live_mode selects which not-ready state; both reject LockActive (mutation in if-block)",
    "composition_attach_body_frame_division_stubbed":
        "frame over account state mutated by attach(symbolic basis) via the engine op",
    "composition_clear_leg_value_conservation":
        "oi/weight read back from asset mutated by clear(symbolic basis) via the engine op",
}


def main(argv):
    if len(argv) < 2:
        print("usage: symbolic_assert_audit.py <file.rs> [...]", file=sys.stderr)
        return 2
    total = 0
    cleared = 0
    for path in argv[1:]:
        text = open(path, errors="replace").read()
        for name, body in split_harnesses(text):
            flags = audit_harness(name, body)
            if not flags:
                continue
            if name in REVIEWED_CLEAR:
                cleared += 1
                continue
            for kind, detail in flags:
                total += 1
                print(f"[{kind}] {path}::{name}\n    {detail}")
    if cleared:
        print(f"(suppressed {cleared} harness(es) on the reviewed-clear allowlist — "
              f"taint false positives, see REVIEWED_CLEAR)")
    if total == 0:
        print("symbolic_assert_audit: no candidates flagged.")
    else:
        print(f"\nsymbolic_assert_audit: {total} candidate(s) for MANUAL review "
              f"(heuristic — a hit is a prompt to read the harness, not a verdict).")
    return total


if __name__ == "__main__":
    sys.exit(main(sys.argv))
