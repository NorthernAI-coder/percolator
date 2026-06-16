# Phase 0 — Op-property catalog (Pillar S: value safety / no-LoF)

Per complex op, the user-expected VALUE/FRAME properties as predicates over
(pre, args, post). Each row tagged with a tractability plan:
- KERNEL: provable as a full-domain kernel contract (Phase 3)
- AXIOM: whole-op value-conservation composable under the arithmetic axiom (Phase 5)
- FUZZ-A: bounded-exhaustive subdomain (Phase 6 Tier A)
- FUZZ-B: adversarial sampling over production range (Phase 6 Tier B)

Phase 2 reconciles each row against existing artifacts before any new work.

## TRADE — apply_trade_after_refresh / execute_trade_with_fee / execute_batch_with_fee
| id | property | plan |
|----|----------|------|
| S-T1 | post.position == pre.position + filled_size (signed) | KERNEL (kernel_apply_fill) |
| S-T2 | realized PnL delta == exact mark-to-fill on the closed portion; sign correct | KERNEL + FUZZ-B (wide mark arithmetic) |
| S-T3 | fee == exact(notional, schedule), rounded against the user (ceil) | KERNEL (checked_fee_bps) + FUZZ-A rounding edges |
| S-T4 | OI/loss-weight sums move by exactly the leg delta (conservation) | KERNEL (attach/clear/resize — PROVEN) + AXIOM whole-body |
| S-T5 | trader value conserved modulo fee + realized PnL (no value created/destroyed) | AXIOM + FUZZ-B |
| S-T6 | batch outcome == exact fold of per-fill outcomes | KERNEL (kernel_accumulate_batch_trade — PROVEN) |

## LIQUIDATION — liquidate_account
| id | property | plan |
|----|----------|------|
| S-L1 | never strands uncovered loss while other open risk remains | KERNEL (liquidation_close_would_leave_uncovered_loss — PROVEN preflight) |
| S-L2 | insurance + social draw == exactly the maintenance deficit, not more | KERNEL (kernel_liquidation_loss_split — MISSING) |
| S-L3 | liquidated position is reduced or closed (no-op liquidation impossible) | KERNEL + Pillar L cross-ref |
| S-L4 | insufficient durable residual routes to recovery (no silent loss) | KERNEL (preflight routes-to-recovery — PROVEN) |

## SOCIAL LOSS / ADL
| id | property | plan |
|----|----------|------|
| S-A1 | Σ socialized debits == realized deficit (conservation) | KERNEL (kernel_social_loss_distribute — MISSING) |
| S-A2 | each account debit ∝ its loss weight (deterministic attribution) | KERNEL + FUZZ-B |
| S-A3 | ADL rounds toward zero (residue sink, req 14) | FUZZ-A (rounding direction — PARTIAL) |

## CLOSE / RESOLVE — close_resolved_account / realize_source_backed_claims / claim_resolved_payout_topup / refine_resolved_unreceipted_bound
| id | property | plan |
|----|----------|------|
| S-C1 | each resolved claim paid <= its terminal face | KERNEL (resolved_receipt_payment — PROVEN: cannot exceed claim) |
| S-C2 | payouts pro-rata under scarcity; total <= pool | KERNEL (kernel_resolved_payout — PARTIAL: claimable proven) |
| S-C3 | two-receipt payout order-independent | KERNEL (PROVEN: composition_… / proof_v16_two_resolved_receipts… clean-room fixed) |
| S-C4 | no double-claim across receipts/topups | CLOSURE + FUZZ-B (backing_double_claim_fuzz) |
| S-C5 | claim bounds never understate (refine monotone, value-neutral) | KERNEL (PROVEN: bound refinement monotone) |
| S-C6 | terminal realization at rate then dematerialize; junior residual = forfeited principal | FUZZ-B (terminal realization — intractable in Kani) |

## CURE — cure_and_cancel_close
| id | property | plan |
|----|----------|------|
| S-U1 | cures counted exactly once (req 12) | FLOW (support_to_account_capital == 3 sources — PROVEN) |
| S-U2 | cure requires lien consume + face burn (no unbacked cure, req 15) | KERNEL (realize/consume gates — PROVEN preflight) |
| S-U3 | cure on an account with no active close rejects before mutation | KERNEL (PROVEN: cure_rejects_without_active_close) |

## Notes
- "PROVEN" / "PARTIAL" tags here are PRELIMINARY; Phase 2's reconciliation
  matrix is authoritative and re-checks each against the actual artifact + the
  vacuity gates (some "PROVEN" rows may be vacuity-suspect and need re-derivation).
- Every property that is FUZZ-only must, in Phase 6, be split into a Tier-A
  bounded-exhaustive part and a Tier-B sampled part with a named seed corpus.
