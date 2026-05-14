# Kani Proof Strength Audit Results

Generated: 2026-05-14

Source prompt: `scripts/audit-proof-strength.md`.

Kani version: `cargo-kani 0.66.0`.

## Full Kani Timing Sweep

Command:

```text
scripts/run_kani_full_audit.sh
```

The v13 cutover removed the v12 slab and retired the v12 proof inventory. The
sweep now parses the remaining `tests/proofs_*.rs` files and runs each v13
harness one-by-one with exact harness selection and a `600s` timeout.

```text
SUMMARY: 9 passed, 0 failed/timeout (0 timeout) out of 9
```

Timing artifacts:

```text
kani_audit_full.tsv
kani_audit_final.tsv
```

Aggregate timing:

| Metric | Value |
|---|---:|
| Harnesses | 9 |
| Pass | 9 |
| Fail | 0 |
| Timeout | 0 |
| Total wall-clock harness time | 197s |
| Slowest harness | `proof_v13_account_b_chunk_either_advances_or_fails_closed` |
| Slowest harness time | 135s |

## Harness Timings

| Harness | Time | Status |
|---|---:|---|
| `proof_v13_account_b_chunk_either_advances_or_fails_closed` | 135s | PASS |
| `proof_v13_attach_then_clear_leg_restores_account_local_counters_for_long` | 41s | PASS |
| `proof_v13_liquidation_progress_rejects_non_reducing_scores` | 6s | PASS |
| `proof_v13_favorable_action_requires_current_full_refresh` | 5s | PASS |
| `proof_v13_hlock_is_exactly_hmin_or_hmax` | 3s | PASS |
| `proof_v13_hmin_zero_remains_available_when_no_lock_state_exists` | 3s | PASS |
| `proof_v13_stale_counter_transitions_are_idempotent` | 3s | PASS |
| `proof_v13_hidden_leg_rejected_by_bitmap_authority` | 1s | PASS |
| `proof_v13_account_equity_rejects_i128_min_persistent_pnl` | 0s | PASS |

## Static Strength Scan

Inventory by file:

| File | Harnesses |
|---|---:|
| `tests/proofs_v13.rs` | 9 |

Strength indicators:

| Check | Result |
|---|---:|
| Harnesses over v13 production code | 9 |
| Harnesses with symbolic inputs or symbolic branch choices | 6 |
| Harnesses with `kani::cover!` reachability checks | 4 |
| Explicit `kani::assume(false)` / `assume(false)` findings | 0 |
| Confirmed vacuous harnesses | 0 |
| Confirmed weak harnesses | 0 |

Current classification:

| Classification | Status |
|---|---|
| Non-vacuity | No confirmed vacuous harnesses found. Cover checks exercise h-min/h-max, stale set/clear, hidden-leg rejection, and B-chunk progress paths. |
| Weak proofs | No confirmed weak proofs in the v13 inventory. |
| Inductive strength | The stale-counter proof is close to an account-local inductive transition proof. The remaining proofs are strong production-code safety/liveness harnesses, not a complete arbitrary-state inductive proof system. |
| Practical proof boundary | The suite proves key v13 account-local invariants over the real production methods: h-lock state selection, provenance/hidden-leg fail-closed behavior, stale counter idempotence, i128::MIN rejection, B-chunk progress/fail-closed behavior, full-refresh gating, and monotonic liquidation-score rejection. |

## Rust Test Matrix

| Command | Result |
|---|---|
| `cargo test` | PASS |
| `cargo test --features test` | PASS |
| `cargo test --features small` | PASS |
| `cargo test --features medium` | PASS |

The Rust suite currently covers 50 wide-math unit tests and 12 v13 spec tests.

## Audit Conclusion

All v13 Kani proofs pass within the 10-minute per-harness cap, and no weak or
vacuous proof was identified in this pass. The proof boundary is intentionally
v13 account-local: the retired v12 slab proofs no longer apply after the
architectural cutover.
