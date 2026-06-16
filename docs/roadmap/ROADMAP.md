# Roadmap: improving substantive user-expected operational coverage

Author: engine maintainer. Goal: close the gap between what the composite
no-LoF/no-DoS argument proves today and what a *user* expects an *operation* to
do. Revised after audit (7 findings incorporated): liveness/rank progress is now
a FIRST-CLASS pillar alongside value safety and the validator floor; the
invariant catalog is the source of truth that everything else audits against; an
already-proven/missing matrix prevents duplicate work; fuzz claims are bounded
honestly; the engine-API boundary is explicit.

This file is a plan, not a claim. Nothing is "done" until it has a
machine-checked artifact (or an explicitly-bounded fuzz artifact) that passes the
vacuity gates.

---

## 0. Scope boundary (explicit — finding 7)

This plan covers the ENGINE PUBLIC API ONLY: the 55 `pub fn *_not_atomic`
entrypoints of the v16 engine. It does NOT cover wrapper / SVM account routing,
authorization, account loading, or oracle/funding authentication — those live in
the wrapper and are out of scope here. Where the plan says "all 55 ops" it means
engine public API coverage, not wrapper coverage. The downstream goal ("the
wrapper can function-contract the engine and prove the rest") REQUIRES that what
we expose here is a clean, importable per-op contract surface — so every artifact
below should be phrased as a contract a wrapper proof could consume.

---

## 1. The three pillars (a no-LoF/no-DoS plan needs all three)

A complete operational-coverage argument is NOT value safety alone. Three
co-equal pillars, each with its own obligations and acceptance bar:

- PILLAR S — VALUE SAFETY (no-LoF): no op moves value it shouldn't; conservation
  and exact frames hold; encumbrance/lien ledger never leaks.
- PILLAR L — LIVENESS / RANK PROGRESS (no-DoS): every actionable
  unhealthy/closing/stale class has an EXISTING successful public transition
  that (a) is reachable through the real public route (not just at a kernel
  boundary), and (b) strictly decreases a well-founded rank or records terminal
  recovery; and economically-valid ops are not blocked by internal mark/fee
  guards.
- PILLAR F — STATE-INVARIANT FLOOR: every committed Ok-state satisfies the
  user-expected state invariants — proven by validator soundness + the 55/55
  boundary audit.

CRITICAL (finding 1): Pillar F (validator soundness + boundary audit) is a
SAFETY FLOOR ON SUCCESSFUL EXITS ONLY. It says committed states are well-formed.
It does NOT prove liveness — it says nothing about whether a stuck account can
make progress, or whether a needed transition is reachable. Pillar L must be
carried as its own track with its own artifacts; it cannot be inferred from F.

---

## 2. Where we are today (the gap, precisely)

Holds for ALL 55 engine public ops (Pillars S/F, structural):
- GlobalValidState floor at every Ok-exit (boundary_audit.py, 55/55).
- Inductive conservation / encumbrance & lien closure; identity independence;
  typed `TokenValueFlowProofV16` on value-moving paths.

Holds end-to-end (exact frames + conservation) only for the SIMPLE ops:
deposit/withdraw, domain-insurance, counterparty-backing, fees, oracle anchor,
asset lifecycle/admin.

Pillar L today: ActionableState 7-class disjunction with rank STEPS proven for
the two kernel-backed classes (B-advance, close-advance) and routing proofs for
A4/A5/A6 (expired-close, liquidation-preflight, recovery-crank). What is NOT yet
first-class: reaching the rank kernel through the monolithic body interior for
the terminal-route classes, and "economically-valid op is not blocked."

NOT an end-to-end theorem (only stage-kernels + validators + fuzz) for the
COMPLEX high-value ops: `apply_trade_after_refresh` / `execute_batch_with_fee`,
`liquidate_account`, `close_resolved_account` / `realize_source_backed_claims` /
`claim_resolved_payout_topup` / `refine_resolved_unreceipted_bound`,
`cure_and_cancel_close`, social-loss / ADL paths.

Intractability causes (measured — do not re-litigate): wide symbolic arithmetic
(u128 div/mul at 2^50+), large-struct symbolic (de)serialization (16-leg
accounts), leg-scan loops forcing unwind>=17.

---

## Guiding principles (lessons already paid for)

- KERNELS WORK: extract a pure/minimally-mutating stage, contract it over the
  full input domain. The one reliable way to convert intractable bodies into
  proven production code.
- ARITHMETIC-AXIOM RECIPE WORKS for value-conservation of thin-interior bodies
  (opaque-property stub of the wide-division helper + assert deltas + discharge
  exact arithmetic by fuzz). Does NOT scale to large-interior bodies (resize
  timed out three ways) — extract a thinner seam first.
- VALIDATORS ARE THE ANCHOR for Pillar F — but only F. Liveness needs its own
  reachable-progress artifacts.
- NO NEW VACUITY: every harness passes cover_vacuity_gate.py and
  symbolic_assert_audit.py and asserts a property that depends on its symbolic
  inputs through the real production path.
- BEHAVIOR > BODY: prove the user-facing property at the production seam that
  implements it, not by symexing the whole monolith.

---

## Phase 0 — Invariant + property + liveness-obligation CATALOG (source of truth; finding 4)

This is the SPEC for every later phase and MUST land first. Three catalogs,
checked in, each entry tagged with pillar (S/L/F), op(s), and a tractability tag
(kernel-provable / axiom-composable / bounded-exhaustive-fuzz / sampling-fuzz).

- `scripts/state_invariant_catalog.md` (Pillar F): the user-expected state
  invariants U1..Un (solvency shape, junior-bound >= Σ domain claims, insurance
  budget aggregation, leg/asset binding, no double-claim, ...). Phase 1 audits
  validators AGAINST this — so it must exist first.
- `scripts/op_property_catalog.md` (Pillar S): per complex op, the value/frame
  properties as predicates over (pre, args, post). Examples: trade fill/fee/
  margin exactness; liquidation insurance/social draw == deficit; resolved claim
  paid <= face, pro-rata, order-independent; ADL Σ debits == deficit.
- `scripts/liveness_obligation_catalog.md` (Pillar L): per ActionableState
  class, the obligation "class(S) && GlobalValidState(S) => EXISTS a public call
  C reachable through the real route s.t. C succeeds and decreases rank R / records
  terminal recovery", plus the well-founded rank R and the "not blocked by
  internal mark/fee guard" obligation for economically-valid ops.

Acceptance: the union of these three catalogs is the definition of "substantive
user-expected coverage." Everything downstream cites a catalog entry.

---

## Phase 1 — Validator completeness audit, AGAINST the Phase-0 catalog (Pillar F)

The floor already runs at all 55 Ok-exits; making it MEAN the Phase-0 state
invariants amplifies coverage everywhere (including intractable ops) for little
marginal cost — but ONLY for Pillar F (safety on successful exits), not liveness.

Deliverables:
1. `scripts/validator_invariant_map.md` — map every clause of `validate_shape` /
   `validate_with_market` to the U-invariant it enforces; flag U-invariants with
   NO validator clause (the holes the floor cannot catch).
2. Kani soundness proofs (pure predicate proofs over state, no monolith body →
   likely tractable): `validate_shape(s)==Ok => Ui(s)` for each catalog U.
3. Each hole: add the validator clause (engine change, TDD-first) or document
   the compensating control (per-op gate / fuzz).

Acceptance: every Phase-0 U-invariant is either (a) a validator clause proven
sound, or (b) listed out-of-validator with its compensating control. Explicitly
labeled: this strengthens Pillar F only.

---

## Phase 2 — Reconciliation matrix: already-proven / missing / superseded (finding 3)

Before extracting any kernel or writing any proof, map each Phase-0 catalog entry
to existing artifacts so we do not duplicate work. Local proof files already
carry partial coverage for validation, liquidation, resolved-receipt payout,
batch accumulators, signed trade deltas, and trade-preflight paths (e.g.
tests/proofs_v16.rs:6633 and the contract_check_* / proof_v16_liquidation_* /
proof_v16_*_resolved_* / kernel_accumulate_batch_trade / trade_signed_size_deltas
families).

Deliverable: `scripts/coverage_reconciliation.md` — a table:
`catalog entry | pillar | existing artifact(s) | status {PROVEN | PARTIAL |
MISSING | SUPERSEDED-BY} | gap`. Phases 3-7 only work the MISSING/PARTIAL rows.
Re-tag PARTIAL rows that are actually vacuity-suspect for re-derivation.

---

## Phase 3 — Kernel decomposition of complex-op economics (Pillars S + L)

Extend the kernel program to the MISSING/PARTIAL economic cores from Phase 2,
contract each over its full input domain. Includes BOTH value kernels (S) and
rank-progress kernels (L). Priority by value-at-risk:

S/value cores:
1. `kernel_apply_fill` — (position, fill) -> (new position, realized PnL delta,
   OI delta); exact deltas, sign/zero, overflow bounds.
2. `kernel_liquidation_loss_split` — deficit + coverage stack -> exact per-layer
   draw + no-uncovered-loss.
3. `kernel_social_loss_distribute` — deficit -> per-account debits ∝ loss weight;
   Σ debits == deficit, each <= capacity.
4. `kernel_resolved_payout` — lift existing claimable + order-independence to a
   contract: monotone in rate, capped by face, conserves the pool.

L/rank cores (peer priority, finding 1/6):
5. extend the proven rank kernels so EACH ActionableState class has a
   rank-decreasing production kernel (not only B-advance / close-advance), and a
   `select_progress_witness_*` kernel proving the actionable predicate admits a
   valid public continuation (the gate-reachability half).

Each kernel: production calls it (true refactor, suites green each step); contract
full-domain; passes both vacuity gates; cited back to its Phase-0/Phase-2 row.

---

## Phase 4 — Liveness / rank-progress as first-class obligations (Pillar L; findings 1, 6)

Discharge the Phase-0 liveness catalog. For each ActionableState class, two
machine-checked harnesses (the review's shape):
- `proof_actionable_i_selects_valid_witness`: class(S) && GlobalValidState(S) =>
  the witness-selection kernel returns a valid public continuation.
- `proof_witness_i_public_call_succeeds_and_decreases_rank`: the selected public
  path reaches the proven rank kernel / terminal route and strictly decreases R
  (or records terminal recovery).
Where reaching the kernel through the monolith interior is intractable, extract
the route/gate selection into a production kernel (Phase 3.5) and prove THAT,
making the public body a thin `validate -> select -> apply -> validate` wrapper.

ALSO (finding 6) the "not blocked" obligation: prove an economically-valid trade
(within margin, within the oracle/funding envelope, fee-affordable) is NOT
rejected by internal mark/fee/guard logic — i.e. the guards reject only genuinely
invalid ops. This is a no-DoS property distinct from value safety.

Acceptance: a top-level roster check (extend actionable_class_coverage.py) that
every class has BOTH harnesses (or a documented terminal-route witness), and the
"not blocked" property holds for trade and close.

---

## Phase 5 — Whole-body value composition where tractable (Pillar S)

For complex bodies with a CLEAN SEAM and THIN interior, compose the Phase-3
kernels via the arithmetic-axiom recipe into a whole-op value-conservation
theorem (stub wide helpers to opaque properties, stub_verified the kernels or run
real if division-free, assert conservation deltas, discharge exact arithmetic via
Phase 6 fuzz). For large interiors (resize/state-size wall): extract the seam
first, never symex an un-seamed monolith.

Acceptance (Pillar S): at least trade-fill and resolved-payout get a
machine-checked whole-op value-conservation theorem under the named axiom.
NOTE (finding 6): this is a NO-LoF acceptance bar only; the no-DoS acceptance
rows live in Phase 4, not here.

---

## Phase 6 — Reference-model conformance fuzz (rigorous substitute; finding 2)

For economics that stay Kani-intractable, fuzz is the trusted base — but fuzz is
NOT proof unless its domain is stated. Structure it as TWO tiers, never as "full
production range == proven":

TIER A — bounded EXHAUSTIVE subdomains: pick small but representative bounded
input domains (e.g. atoms in 0..=N, a few asset/leg configs) and enumerate them
exhaustively. Within the bound this IS a proof-by-enumeration.

TIER B — adversarial randomized sampling over the production range: an
independent bigint reference model, differentially fuzzed engine-vs-reference,
with an EXPLICIT checked-in seed corpus (denominator/numerator/overflow edges,
one-less/one-more rounding, boundary magnitudes), randomized high-volume cases,
realistic operation SEQUENCES, and STATED coverage targets (which fields/edges
are exercised). `log()` the bound and the sampling so it never reads as
exhaustive.

Acceptance: each intractable catalog entry maps to (Tier A bound that is proven
exhaustively) + (Tier B sampling with named seed corpus and coverage targets).
The claim is always "engine == reference over <Tier A domain>, sampled over
<Tier B domain with corpus C>", labeled fuzz, never proof of the full range.

---

## Phase 7 — Sequence / user-journey invariants (Pillars S + L)

Users run sequences. Extend the two-op witnesses to the journeys that matter,
each carrying both a value (S) and a progress (L) obligation:
- open -> accrue -> partial-close -> close: position/PnL correct; conservation
  across the journey; each step makes finite progress.
- open -> adverse move -> liquidate: no value stranded; account ends
  healthy/closed; insurance/social draw == deficit; liquidation makes progress.
- resolve -> multi-receipt payout under scarcity: total <= pool, pro-rata,
  order-independent; payout terminates.
Bounded Kani sequence proofs where tractable, else Phase-6 reference-model
sequence fuzz.

---

## Cross-cutting: keep the assurance honest

- Every new harness passes `cover_vacuity_gate.py` and `symbolic_assert_audit.py`.
- CI step runs both gates over captured per-harness logs.
- DURABLE welded-discriminant review artifact (finding 5): a checked-in
  `scripts/welded_discriminant_review.md` — per harness-family, a filled
  checklist confirming the asserted outcome VARIES with the symbolic input across
  the covered domain (i.e. no constant outcome welded by a sibling variable).
  Required for any harness asserting a constant/near-constant result. No new
  proof merges without its row. This converts tribal knowledge into a record.
- Maintain `lof_transition_class_roster.py`: update each op's strength tier as it
  graduates (KERNEL_VALUE -> WHOLE_BODY_COMPOSITION, gate -> reachable-route), so
  the roster reflects real strength, not aspiration.
- Keep the engine-API boundary (section 0) visible in every artifact, so the
  wrapper-contract goal stays well-defined.

---

## Definition of "substantive user-expected coverage" (acceptance bar)

For each complex op, ALL THREE pillars, not just value:

PILLAR S (no-LoF):
- each op_property_catalog entry is machine-proven at a kernel/seam (Phases 3-5)
  or Tier-A-exhaustive + Tier-B-sampled (Phase 6).
PILLAR L (no-DoS) — finding 6, peer rows:
- finite crank progress: every crank step does bounded work and advances a rank.
- terminal-route reachability: each terminal class reaches its recovery/close
  route through the real public path.
- liquidation/recovery progress: a liquidatable/stuck account has a successful
  continuation that reduces risk or records terminal recovery.
- not-blocked: an economically-valid trade/close executes without rejection by
  internal mark/fee/guard logic.
PILLAR F (state floor):
- the GlobalValidState floor is proven to enforce the relevant Phase-0
  U-invariants (Phase 1).
ALL pillars:
- at least one end-to-end user-journey covered (Phase 7); every supporting
  harness passes both vacuity gates and has its welded-discriminant row; the
  roster + theorem docs state the per-op, per-pillar strength tier honestly.

Order of attack: Phase 0 (catalog, source of truth) -> Phase 1 (validator floor)
+ Phase 2 (reconciliation) -> Phase 3 (value + rank kernels) -> Phase 4 (liveness
first-class) + Phase 6 (rigorous fuzz) -> Phases 5/7 (compose + journeys).

---

## Explicit non-goals / permanent limits

- A single Kani theorem over all public transitions, or whole-body symex of an
  un-seamed monolith with wide arithmetic: out of reach this prover generation.
  We decompose; we do not pretend to collapse.
- Validator soundness + boundary audit prove Pillar F (safety on successful
  exits) ONLY — never no-DoS liveness (finding 1). Liveness is Pillar L's job.
- "Proven correct vs user intent": proofs are relative to the Phase-0 catalogs +
  validators. Phase 0/1 make that relationship explicit; they do not make the
  catalog user-intent by fiat.
- Reference-model fuzz is NOT proof outside its Tier-A exhaustive bound; Tier-B
  is sampling, always labeled with its domain and corpus (finding 2).
- Engine public API only; wrapper / SVM routing is out of scope (finding 7).
