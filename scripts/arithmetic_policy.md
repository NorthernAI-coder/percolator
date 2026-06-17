# Arithmetic Policy (roadmap Phase 5)

The engine proof surface treats the wide (U256-width) division/multiplication
primitives as **named arithmetic axioms**: in Kani their outputs are opaque
bounded/nonzero witnesses, and their exact semantics are discharged by
differential reference-model fuzz. This file states the policy that bounds the
final 10/10 claim.

## Stance: conditional 10/10 under the named arithmetic axiom manifest

We adopt **Option 1** (conditional). The 10/10 no-LoF/no-DoS claim is stated as
"10/10 **under the named arithmetic axiom manifest**", never unconditional.

Mandatory, CI-enforced:

1. Every `#[kani::stub(<wide-helper>, <axiom>)]` has a row in
   `scripts/arithmetic_axiom_manifest.md` and a present discharge artifact
   (`scripts/arithmetic_axiom_manifest_check.py`). Current axioms:
   - `kani_any_loss_weight` / `axiom_loss_weight_nonzero` → `loss_weight_for_basis`
     (ceil-division weight), discharged by `loss_weight_*` reference tests.
   - `axiom_social_loss_book_split` → `social_loss_book_split`
     (the `engine_chunk*SOCIAL_LOSS_DEN/weight_sum` booking division), discharged
     by `social_loss_split_tier_a_exhaustive` + `social_loss_split_sampled`.
2. Differential discharge is BOTH Tier-A exhaustive (a small fully-enumerated
   domain) AND Tier-B sampled (a stated wide domain + seed-corpus edges) against
   an INDEPENDENT native reference implementation (`tests/reference_model_
   conformance.rs`). Each test states its domain.
3. Boundary vectors: explicit max/min vectors for notional, fee bps (0 and the
   10_000 cap), and social-loss split at the `weight_sum`/`SOCIAL_LOSS_DEN`
   boundaries are pinned in `arithmetic_boundary_vectors_*` so the discharge is
   not only random-sampled but exact at the extremes.

The axiom is sound for the proofs that consume it because those proofs assert
FRAME / CONSERVATION / RANK shells that hold for ANY value the wide helper returns
(e.g. the social-loss booking shell conserves `booked + explicit + remaining ==
residual` regardless of the exact `delta_b`); the helper's exact VALUE is never
re-derived in Kani (that would reintroduce the intractable wide arithmetic).

## Tracked separately: Option 2 (remove the axiom)

To reach UNCONDITIONAL arithmetic correctness (not pursued now, tracked as
"remove arithmetic axiom"):

- Verify the U256 div/mul kernels once in a prover/backend that handles wide
  division (or replace them with a separately verified arithmetic library), then
- Import that verified result into Kani as a lemma in place of the opaque axiom.

This removes the single biggest conditional in the final claim but needs a
stronger backend; it is out of scope for the conditional-10/10 milestone.

## What this policy does NOT cover

- SVM rollback semantics, SCHED (scheduler fairness), and wrapper routing/auth/
  oracle are separate named assumptions (see `scripts/engine_contracts.json`
  `assumptions`), not arithmetic.
