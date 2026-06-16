# Phase 1 — Validator → invariant map and soundness-lemma plan (Pillar F)

Inverts state_invariant_catalog.md: each validator clause → the U-invariant it
enforces, plus the soundness lemma `validate_*==Ok => Ui` to machine-check.
Pillar F only: this strengthens the safety floor on SUCCESSFUL exits; it is NOT
liveness.

## validate_shape clauses (src/v16.rs)
| clause | enforces | soundness lemma | tractability |
|--------|----------|-----------------|--------------|
| `vault <= MAX_VAULT_TVL` | U1 | trivial bound | TRACTABLE |
| `c_tot<=vault && insurance<=vault` | U2 | scalar | TRACTABLE |
| `pnl_matured_pos_tot<=pnl_pos_tot` | U6 | scalar | TRACTABLE |
| `pnl_pos_bound_tot>=pnl_pos_tot && ==derived_bound` | U7 | scalar | TRACTABLE |
| `pnl_pos_bound_tot_num>=exact_bound_num` | U8 | scalar | TRACTABLE |
| `slot_last<=current_slot`, activation-slot ord | U9 | scalar | TRACTABLE |
| `next_market_id!=0` | U10 | scalar | TRACTABLE |
| payout-snapshot gate | U11 | enum/bool | TRACTABLE |

## validate_header_aggregate_totals clauses
| clause | enforces | soundness lemma | tractability |
|--------|----------|-----------------|--------------|
| `c_tot+insurance+earnings <= vault` | U3 | scalar add (checked) | TRACTABLE |
| `+source_fresh_backing/BOUND_SCALE <= vault` | U4 | scalar add + 1 div by const | TRACTABLE |
| `reserved_atoms<=insurance && budget_remaining<=insurance` | U5 | scalar | TRACTABLE |
| `pnl_pos_bound_tot_num >= source_claim_bound_total_num` | U8/U13 | scalar | TRACTABLE |

## validate_shape_full_audit_scan (audit-scan feature)
| clause | enforces | soundness lemma | tractability |
|--------|----------|-----------------|--------------|
| recomputed slot sums == header totals (earnings, claim-bound, fresh-backing, insurance-reserved, budget-remaining, blocker-count) | U12 | recompute equality over slots | HEAVY (slot loop) — prove per-field over a bounded slot count |

## validate_with_market / validate_active_leg clauses
| clause | enforces | soundness lemma | tractability |
|--------|----------|-----------------|--------------|
| provenance/version/layout | U18 | scalar | TRACTABLE |
| leg market_id/lifecycle/epoch binding | U14 | per-leg (leg-loop) | MEDIUM (unwind bound) |
| seen_assets dedup | U15 | per-leg | MEDIUM |
| bit==leg.active, inactive EMPTY | U16 | per-leg | MEDIUM |
| validate_active_leg shape | U17 | scalar per leg | TRACTABLE (per-leg) |
| reserved_pnl/residual bounds | U19 | scalar | TRACTABLE |

## Soundness-proof plan
- TRACTABLE scalar lemmas: prove `validate_shape(s)==Ok => Ui` by building a
  state with the relevant fields SYMBOLIC and the rest a minimal valid fixture,
  assuming validate_shape()==Ok, asserting Ui. Batch the scalar Us into a few
  harnesses (one per validator function) to amortize fixture cost.
- U13 (junior bound) already has the REJECT direction
  (proof_v16_validate_shape_rejects_global_junior_bound_below_domain_claims);
  add the SOUND direction as `validate_shape==Ok => bound_num >= claim_total_num`.
- U12 (audit-scan recompute) is HEAVY; prove per-field over a bounded slot count
  (1-2 slots) and document the loop-bound generalization (req 33/34).
- Holes (U-invariant with NO clause): none found in the catalog pass — every
  U1..U24 maps to a clause. If a future U is added with no clause, it is a
  Phase-1 engine-change (add the clause, TDD-first).

## Status
First soundness lemma landed: see tests/proofs_v16.rs
`proof_v16_validator_sound_senior_stack_within_vault` (U3) — verified.
Remaining scalar lemmas batched as backlog; leg-loop (U14-U17) and audit-scan
(U12) lemmas are MEDIUM/HEAVY and scheduled after the Phase 3 kernels.
