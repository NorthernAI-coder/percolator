# Phase 0 — State-invariant catalog (Pillar F source of truth)

The user-expected STATE invariants U1..Un that any committed Ok-state must
satisfy. Phase 1 (validator completeness audit) checks `validate_shape` /
`validate_with_market` AGAINST this list and proves `validate_*==Ok => Ui`.
Each row: the invariant, the validator clause that should enforce it (file:fn),
and the spec §0 requirement it derives from.

Scope: engine public API only (see docs/roadmap/ROADMAP.md §0). These are
state-shape invariants, NOT liveness (Pillar L) or per-op value flow (Pillar S).

| id | invariant (predicate over committed state) | enforcing validator clause | spec req | status |
|----|---------------------------------------------|----------------------------|----------|--------|
| U1 | `vault <= MAX_VAULT_TVL` | validate_shape (vault bound) | config | PROVEN-SOUND (validator_sound_bound_and_config) |
| U2 | `c_tot <= vault && insurance <= vault` (junior never exceeds vault) | validate_shape | 6 | PROVEN-SOUND (validator_sound_scalar_invariants) |
| U3 | senior stack covered: `c_tot + insurance + backing_provider_earnings <= vault` | validate_header_aggregate_totals (senior) | 6 | PROVEN-SOUND (validator_sound_senior_stack_within_vault) |
| U4 | senior + fresh backing covered: `c_tot + insurance + earnings + source_fresh_backing/BOUND_SCALE <= vault` | validate_header_aggregate_totals (senior_with_backing) | 6 | CLAUSE-PRESENT |
| U5 | insurance reservations within pool: `source_insurance_credit_reserved_total_atoms <= insurance && insurance_domain_budget_remaining_total <= insurance` | validate_header_aggregate_totals | 6/11 | CLAUSE-PRESENT |
| U6 | matured PnL <= positive PnL: `pnl_matured_pos_tot <= pnl_pos_tot` | validate_shape | 14 | PROVEN-SOUND (proof_v16_validator_sound_pnl_aggregates) |
| U7 | bound >= positive PnL: `pnl_pos_bound_tot >= pnl_pos_tot` and `== derived_bound` | validate_shape (derived_bound) | 17 | PROVEN-SOUND (proof_v16_validator_sound_pnl_aggregates) |
| U8 | bound-num never understates exact: `pnl_pos_bound_tot_num >= exact_bound_num` and `>= source_claim_bound_total_num` | validate_shape / aggregate | 17 | FUZZ-DEFERRED (Kani-intractable: symbolic bound_num forces validate_shape's amount_from_bound_num u128-division to bit-blast → P6) |
| U9 | clock monotone: `slot_last <= current_slot`, `last_asset_activation_slot <= current_slot` | validate_shape | 29/33 | PROVEN-SOUND (validator_sound_scalar_invariants) |
| U10 | market-id nonzero / next_market_id != 0 | validate_shape | 4 | PROVEN-SOUND (validator_sound_bound_and_config) |
| U11 | payout-snapshot gate: resolved ledger non-EMPTY only if snapshot captured | validate_shape | 22 | CLAUSE-PRESENT |
| U12 | aggregate totals == recomputed slot sums (earnings, claim-bound, fresh-backing, insurance-reserved, budget-remaining, blocker-count) | validate_shape_full_audit_scan / compute_aggregate_totals_and_validate_slots | 6/17/11 | CLAUSE-PRESENT (audit-scan feature) |
| U13 | global junior bound >= Σ per-domain source claims | aggregate totals (junior-bound vs claims) | 6 | VERIFY-SOUNDNESS (see proof_v16_validate_shape_rejects_global_junior_bound_below_domain_claims) |
| U14 | every active leg bound to a live asset + matching epoch/side | validate_with_market (leg loop: market_id, lifecycle, epoch binding) | 4/36 | CLAUSE-PRESENT |
| U15 | canonical per-asset leg: no duplicate asset_index across active legs | validate_with_market (seen_assets dedup) | 36 | CLAUSE-PRESENT |
| U16 | active-bitmap matches leg.active; inactive slots are EMPTY | validate_with_market (bit!=leg.active, is_empty) | 32/34 | CLAUSE-PRESENT |
| U17 | leg shape valid: a_basis in range, loss_weight >= loss_weight_for_basis, <= SOCIAL_LOSS_DEN, b_rem < DEN | validate_active_leg | 1/14 | CLAUSE-PRESENT |
| U18 | account provenance/version/layout bound to this market group | validate_with_market (provenance) | 4 | CLAUSE-PRESENT |
| U19 | reserved_pnl <= positive PnL; residual_spent <= residual_crystallized | validate_with_market | 19/24 | CLAUSE-PRESENT |
| U20 | source-credit domains well-shaped (occupied/tagged, configured, deduped) | validate_source_credit_shape_with_market | 9/36 | CLAUSE-PRESENT |
| U21 | resolved payout receipt static-valid (present/face/paid shape) | validate_resolved_payout_receipt_static | 22 | CLAUSE-PRESENT |
| U22 | close-progress ledger residual equation holds (gross+drift == progress+residual) | validate_close_progress_ledger_with_market | 22/24 | CLAUSE-PRESENT |
| U23 | bucket status lifecycle shape (Empty value-free; Fresh funded; Expired/Impaired residue-only) | validate_backing_bucket_static | 8/16 | CLAUSE-PRESENT (closure proven) |
| U24 | positive-PnL source attribution: claims <= attributed PnL | validate_positive_pnl_source_attribution | 9/15 | CLAUSE-PRESENT |

## Holes flagged for Phase 1 (no clause OR soundness not yet proven)
- U13 soundness: prove `validate_shape==Ok => junior_bound >= Σ domain claims`
  as a pure predicate proof (a partial witness exists; lift to a soundness
  theorem over symbolic per-domain claims).
- U3/U4/U5 soundness: prove the senior-coverage clauses are sound over symbolic
  slot sums (today proven via the audit-scan recompute equality U12, which is
  strong; restate as standalone soundness lemmas the wrapper can import).
- U12 is gated behind the `audit-scan` feature — confirm it runs in the
  validate_shape path used at every public Ok-exit (boundary_audit covers
  reachability; confirm the audit-scan variant is the one on the hot path or
  document the two-tier shape).

## Acceptance (Phase 1 closes against this catalog)
Every Ui is either (a) a validator clause with a machine-checked soundness lemma
`validate_*==Ok => Ui`, or (b) listed out-of-validator with a compensating
control. No Ui left UNMAPPED.
