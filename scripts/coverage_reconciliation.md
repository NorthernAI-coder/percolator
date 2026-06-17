# Phase 2 — Coverage reconciliation matrix

Maps every Phase-0 catalog entry to EXISTING artifacts so later phases work only
the gaps. Status: PROVEN (machine-checked, non-vacuous) | PARTIAL (some coverage,
gap noted) | MISSING | SUPERSEDED. PARTIAL/MISSING rows are the Phase 3-7 backlog.

Re-derivation note: every "PROVEN" row inherited from before the vacuity audit
must still pass cover_vacuity_gate.py + symbolic_assert_audit.py; rows marked
(gate-clean) were swept 2026-06-16.

## Pillar S — value safety
| catalog | existing artifact(s) | status | gap → phase |
|---------|----------------------|--------|-------------|
| S-T1 position delta | trade_position_delta_preserves_oi_symmetry; attach/clear/resize kernels; **kernel_classify_position_delta (PROVEN: exact Attach/Clear/Flip/Resize route, production dispatches on it)** | PROVEN | exact signed-delta-per-route via the leg kernels |
| S-T2 realized PnL on close | negative_pnl_settlement_consumes_principal; **trade-arith conformance (notional_floor/risk_ceil vs native ref, Tier-A+B)** | PARTIAL→strengthened | full mark close PnL still P6 FUZZ-B |
| S-T3 fee exact+ceil | trade_fee_helper_* (2); rounding_residue_fuzz; **trade_fee_conformance (ceil vs native ref, Tier-A exhaustive + Tier-B 4k)** | PROVEN + CONFORMANT | — |
| S-T4 OI/weight conservation | kernel_attach/clear/resize contracts; composition_attach/clear value (PROVEN) | PROVEN (gate-clean) | extend AXIOM whole-body to trade body → P5 |
| S-T5 trader value conserved mod fee | TokenValueFlowProofV16 (runtime) + flow contracts | PARTIAL | whole-op AXIOM composition → P5; FUZZ-B → P6 |
| S-T6 batch == fold | contract_check_kernel_accumulate_batch_trade; proof_v16_batch_outcome_accumulator_is_exact… | PROVEN (gate-clean) | — |
| S-L1 no stranded loss | proof_v16_liquidation_cannot_leave_uncovered_loss_with_other_open_risk | PROVEN (gate-clean) | — |
| S-L2 draw == deficit | **kernel_settle_principal (principal layer) + kernel_consume_insurance_layer (insurance layer, capped by domain budget) — both PROVEN** | PARTIAL→strengthened | remaining social layer: `kernel_social_loss_distribute` → P3 |
| S-L3 position reduced | **kernel_reduce_position_delta (PROVEN: STRICT progress — pre>0&&req>0 => closed>0 && post_abs<pre; never over-closes/flips; full close clears)** — now called by BOTH rebalance AND liquidate_account | PROVEN (real liquidation route) | — |
| S-L4 route to recovery | proof_v16_liquidation_preflight_routes_insufficient_residual_capacity_to_recovery | PROVEN (gate-clean) | — |
| S-A1 Σ debits == deficit | kernel_settle_principal; **kernel_social_loss_chunk_cap (PROVEN: booked<=residual & <=cap)** + social_loss_book_split conformance (delta_b*ws+rem==num, rem<ws, Tier-A+B) | PROVEN (cap) + CONFORMANT (split) | — |
| S-A2 debit ∝ loss weight | social_loss_book_split conformance (delta_b==numerator/weight_sum, delta_b*ws+rem==num, rem<ws, delta_b>0 IFF num>=ws — Tier-A+B); **contract_check_bresidual_chunk_conservation PROVES the booking shell: booked==engine_chunk, remaining+booked==residual, explicit_loss==0, delta_b>0, SIDE ISOLATION (non-loss side untouched), monotonicity** | CONFORMANT (split) + PROVEN (shell: conservation + side isolation) | resolved-mode explicit-loss path covered by kernel_bresidual_step (B.2 composition) |
| S-A3 ADL rounds to zero | rounding_residue_fuzz (ADL direction) | PARTIAL | P6 Tier-A bound |
| S-C1 paid <= face | proof_v16_resolved_receipt_payment_cannot_exceed_terminal_claim | PROVEN (gate-clean) | — |
| S-C2 pro-rata, total<=pool | claimable_is_rate_monotone; payout_topup_pays_min_claimable; **kernel_resolved_payout_step (PROVEN: payout==min(claimable,vault), vault conserved)** | PROVEN | claimable's wide rate-div covered by P6 conformance |
| S-C3 order-independent | proof_v16_two_resolved_receipts_are_order_independent… (clean-room fixed) | PROVEN (gate-clean) | — |
| S-C4 no double-claim | closure layer; backing_double_claim_fuzz | PARTIAL | P6 Tier-A/B split |
| S-C5 bound never understates | proof_v16_public_resolved_bound_refinement_is_monotone_and_value_neutral | PROVEN (gate-clean) | — |
| S-C6 terminal realization | proof_v16_insolvent_resolved_receipt_clears_at_terminal_rate; resolved_winddown_* (3) | PARTIAL (Kani-intractable whole-body) | P6 FUZZ-B |
| S-U1 cures counted once | flow support_to_account_capital (==3 sources) | PROVEN (gate-clean) | — |
| S-U2 cure needs consume+burn | realize/consume gate proofs | PARTIAL | gate kernel → P3 |
| S-U3 cure rejects no-close | proof_v16_cure_and_cancel_close_rejects_without_active_close | PROVEN (gate-clean) | — |

## Pillar L — liveness / rank progress
| catalog | existing artifact(s) | status | gap → phase |
|---------|----------------------|--------|-------------|
| A1 stale (sel/dec) | proof_v16_equity_active_accrual_with_progress_commits_one_bounded_segment | PARTIAL | route-reachability `select_progress_witness_stale` → P3/P4 |
| A2 b-stale (sel/dec) | liveness_b_stale_leg_has_advancing_chunk; kernel_advance_leg_b_snap | PROVEN-AT-KERNEL | route through body → P4 |
| A3 pending close (sel/dec) | liveness_pending_close_has_rank_decreasing_advance; kernel_advance_close_ledger | PROVEN-AT-KERNEL | route through body → P4 |
| A4 expired close | proof_v16_expired_close_progress_declares_recovery… (PUBLIC ROUTE) | PROVEN (gate-clean) | — |
| A5 liquidatable | preflight accept + route proofs | PARTIAL | dec via S-L2/S-L3 → P3/P4 |
| A6 recovery-eligible | proof_v16_permissionless_recovery_crank_is_accounting_neutral | PROVEN (gate-clean) | — |
| A7 resolved winner | resolved_winddown_* + terminal suite; kernel_resolved_close_progress (close-step rank) + build_resolved_close_rank FIDELITY-BUILDER (each rank flag == real per-component predicate: b-stale/neg-PnL/live-leg/capital/receipt/recovery) | PARTIAL (rank classifier + rank-summary builder both FAITHFUL) | remaining ONLY: wire close_resolved branch dispatch through the built rank (route proof) + P6 FUZZ-B terminal realization |
| NB1 valid trade not blocked | ALL leaves PRODUCTION-FAITHFUL: build_trade_request_guard_summary (request); kernel_trade_preflight_admits (preflight); kernel_cert_is_current (accounts_current == ensure_favorable_action_current_certificate); kernel_initial/locked_margin_gate (final gates); kernel_trade_admit (composition) | PARTIAL (every leaf FAITHFUL) | remaining ONLY: the composing route proof (EconomicallyValidTrade => public route reaches fill) over the real two-account body |
| NB2 finite crank progress | unwind(40) bounds (req 33); permissionless-crank proofs; clock-advance | PARTIAL | per-continuation bounded-work + rank/terminal artifact for EVERY selected crank action → P4 |
| L.sel selector (all A-classes) | select_progress_witness (contract_check_select_progress_witness, overlap-safe priority) — NOW PRODUCTION-WIRED via permissionless_auto_crank_not_atomic: build_actionable_summary classifies real state (each flag == its production eligibility predicate, mode-gated), actionable_summary_from_signals assembles it (contract_check_actionable_summary_from_signals), the proven selector picks the continuation, the auto-crank dispatches to the matching proven entrypoint. Dispatch soundness validated by 5 TDD integration tests (v16_auto_crank_*: stale→refresh→clean, multi-step stale→liquidate→fixed-point, expired-close→terminal recovery, b-stale→SettleB, resolved_winner-without-snapshot) which surfaced+fixed 5 classifier soundness bugs — EACH flag must match what its dispatch entrypoint actually requires: (1) liquidatable needs open risk (non-empty bitmap); (2) pending_close is NOT proactively classified — settle_account_b_chunk does not advance the close-ledger residual (only the owning liquidation does), so AdvanceClose→SettleB would not progress; the with-leg case is liquidatable; (3) recovery_eligible is not Resolved-dispatchable (declare_permissionless_recovery rejects Resolved); driven by expired_close instead; (4) resolved_winner must NOT gate on payout_snapshot_captured — close_resolved is the sole lazy capturer, so gating would deadlock the first winner; (5) liquidatable stale-cert deficit needs re-cert (handled by refresh-first priority) | PROVEN (selector) + PRODUCTION-FAITHFUL (classifier flags by real predicate; assembly proven; Live path tested end-to-end) | SCOPE: auto-crank fully drives LIVE-mode liveness; Resolved-mode winddown (winner realization, unattributed-insolvency terminal recovery) partially covered (winner classified, close_resolved realizes lazily) — full Resolved drive + per-flag symbolic biconditional over monolith interior → P4 |

## Pillar F — state floor (see state_invariant_catalog.md)
| catalog | existing artifact(s) | status | gap → phase |
|---------|----------------------|--------|-------------|
| U2,U3,U6,U7,U9 | validator_sound_senior_stack / _scalar_invariants / _pnl_aggregates (PROVEN-SOUND) | PROVEN | — |
| U1,U8,U10-U12,U14-U24 clauses | validate_shape/validate_with_market clauses; boundary_audit 56/56 | PARTIAL (clause present, soundness lemma pending) | remaining soundness lemmas → P1 |
| U13 junior bound | proof_v16_validate_shape_rejects_global_junior_bound_below_domain_claims | PROVEN (gate-clean, rejection direction) | soundness direction → P1 |

## Backlog after reconciliation (what Phases 3-7 actually build)
MISSING: S-L2, S-L3, S-A2 (kernels); the route-reachability `select_progress_witness_*`
kernels for A1/A2/A3/A5; NB1 "valid-admitted" direction.
PARTIAL→strengthen: S-T1/S-T2 (kernel_apply_fill), S-C2 (kernel_resolved_payout),
S-A1 (global conservation), Pillar-F soundness lemmas (P1), the FUZZ rows → P6
Tier-A/Tier-B.
PROVEN (no new work, keep gate-clean): S-T3,S-T4,S-T6,S-L1,S-L4,S-C1,S-C3,S-C5,
S-U1,S-U3,A4,A6,U13(reject).

## Roadmap exhaustion status (tractable items complete)

The cleanly-tractable roadmap items are exhausted. Established this effort:

**Conservation chain (workstream B, no-LoF) — bankruptcy-residual, 5 proven layers:**
apply_bankruptcy_residual_chunk_to_loss_side (leaf: conservation + side isolation
+ monotonicity) -> kernel_bresidual_step (booking-step composition) ->
closure_close_ledger_absorbs_booking_outcome (ledger residual == booking
remaining) -> kernel_settle_resolved_pnl_after_booking (negative PnL reduced by
exactly the absorbed loss, never over-cleared) -> insurance-draw vault-neutrality
(kernel_consume_insurance_layer scalar conservation + flow-proof Ok-IFF-vault-flat
biconditional). All proven without the view-symbolic wall.

**Self-classifying crank (workstream A/C) — production-wired liveness selector,
5 TDD-surfaced soundness bugs fixed; classifier flags all FAITHFUL + mode-gated.**

**Certification gates (workstream E) — COMPLETE, all green:** route_fidelity_roster
(29 summary fields tied to production), no_lof_entrypoint_roster (56 entrypoints,
21 FLOW/56 FLOOR/16 KERNEL), dead_kernel_check (28 kernels: 23 called / 1
entrypoint / 4 verified fidelity models), guard_mutation_sensitivity (all fields
consulted), liveness_roster_enforcement, arithmetic_axiom_manifest_check,
cover_vacuity_gate, symbolic_assert_audit.

**Remaining = documented permanent walls + out-of-scope (NOT tractable-undone):**
- view-symbolic / monolith-interior route proofs (A1..A7 reaching kernels through
  the real public body; NB1 composing route; the 4 fidelity models wired into
  production; close_resolved terminal composition; single_step_capacity wide-
  divisor division + declare_recovery validate_shape over the symbolic view).
- per-op B.1 value THEOREMS for all 56 entrypoints (35 are FLOOR-only — rely on
  the Pillar-F validator floor + Phase-1 soundness, not a per-op value-flow proof).
- wrapper proof consumption + LiteSVM CU/journeys (workstream D, Phase 7) —
  OUT OF SCOPE (finding 7, engine-only).
- SCHED (external scheduler liveness) — named external assumption.
