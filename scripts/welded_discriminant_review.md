# Welded-discriminant review log (durable; cross-cutting)

The two automated gates catch the mechanically-detectable vacuity classes:
- `cover_vacuity_gate.py` — dead covers (UNSATISFIABLE/UNREACHABLE).
- `symbolic_assert_audit.py` — assertions that ignore symbolic inputs (taint).

They CANNOT catch the welded-discriminant class: a harness whose cover is
satisfiable and whose assert DOES reference a symbolic-derived value, but whose
asserted OUTCOME is welded to a constant by ANOTHER variable (the original
bankruptcy-hlock defect: `lane==HMax` was true for all inputs because a welded
bit forced HMax). This is the residual class that needs human eyes.

POLICY: any harness asserting a constant or near-constant outcome (a fixed error
variant, a fixed enum, a fixed numeric) MUST have a row here confirming the
asserted outcome VARIES with the symbolic input across the covered domain — i.e.
there is a reachable symbolic assignment producing a DIFFERENT outcome, or the
constancy is the genuine property (and why). No new proof merges without its row.

Checklist per harness/family:
1. List the symbolic inputs.
2. State the asserted outcome.
3. Is the outcome constant across all symbolic assignments? If yes → is that the
   intended property, or is a sibling variable welding it? (welded => DEFECT)
4. If it should vary: name a reachable assignment that produces a different
   outcome, and confirm a cover witnesses it.

---

## Reviewed families

### bankruptcy_hlock (proof_v16_bankruptcy_hlock_selects_hmax_before_source_backed_value_exit)
1. symbolic: claim, hlock_active, instruction_candidate.
2. assert: `lane == (if hlock_active||instruction_candidate {HMax} else {HMin})`.
3. NOT constant — the asserted lane VARIES with (hlock_active, instruction_candidate).
4. covers witness HMax-via-bit, HMax-via-candidate, AND the HMin arm
   (!hlock && !candidate). Outcome genuinely varies. CLEAN (clean-room fixed).

### reused_asset_slot (proof_v16_reused_asset_slot_rejects_stale_market_id_leg)
1. symbolic: stale_market_id_raw, units, is_short, market_id_is_current.
2. assert: `if current {Ok(())} else {Err(HiddenLeg)}`.
3. NOT constant — outcome flips on market_id_is_current; cover witnesses the
   Ok baseline AND the HiddenLeg rejection at multi-unit basis. CLEAN.

### expired_backing (proof_v16_expired_backing_yields_zero_realizable_support_after_expiry)
1. symbolic: fully_backed selector.
2. assert: post-expiry zeroed state (forfeiture) — constant ACROSS the backing
   ratio BY DESIGN (the lapsed backing is zeroed regardless of how much it was).
3. Constant outcome is the GENUINE property (forfeiture is ratio-independent);
   the selector exists to make both covers (backing<claim, backing==claim)
   satisfiable, not to vary the post-state. Documented as intentional. CLEAN.

### two_resolved_receipts (order independence)
1. symbolic: a_claim, b_claim, vault.
2. assert: total_a_first == total_b_first; per-receipt equal when funded.
3. NOT a tautology — vault scarcity makes intermediate per-receipt payouts
   DIFFER by order; the asserted equality is the non-trivial property. cover
   witnesses scarce (vault < ca+cb) and funded. CLEAN.

### insurance_lien_consume (domain isolation)
1. symbolic: atoms_long, atoms_short.
2. assert: Long consumed; Short sibling unchanged; pool == atoms_short.
3. NOT constant — the surviving Short value (atoms_short) is symbolic; the
   isolation assertion is over symbolic sibling state. CLEAN.

---

## Pending (fill as roadmap harnesses land)
Each new Phase 3-7 harness that asserts a constant outcome adds its row before
merge. CI may grep new harnesses for `assert_eq!(..., Err(` / `assert_eq!(...,
<Enum>::` with no varying-outcome cover and require a matching row here.
