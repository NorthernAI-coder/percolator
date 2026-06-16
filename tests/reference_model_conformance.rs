//! ROADMAP Phase 6 — reference-model conformance fuzz (Pillar S, no-LoF value).
//!
//! For economics the engine computes with wide (U256) arithmetic that Kani
//! cannot symex, the honest substitute is differential conformance against an
//! INDEPENDENT reference model — but only over a STATED domain. This file pins
//! the resolved-payout `claimable` arithmetic:
//!     claimable(receipt, ledger) == floor(face * rate_num / rate_den) - paid
//! against a native-u128 reference that shares no code with the engine's
//! `wide_mul_div_floor_u128`.
//!
//! Two tiers, per the roadmap:
//!   TIER A — bounded EXHAUSTIVE subdomain: enumerated, so engine==reference is
//!            a proof-by-enumeration WITHIN the bound.
//!   TIER B — adversarial sampling over the STATED no-overflow domain
//!            (face*rate_num <= u128::MAX), with an explicit seed corpus of
//!            denominator/numerator/rounding/boundary edges.
//!
//! STATED LIMIT (not hidden): the reference is exact only where face*rate_num
//! fits u128. The product>u128 regime is the engine's U256 path and is OUT of
//! this reference's bound — covered by the engine's own arithmetic, not claimed
//! here. This is fuzz, not proof, outside Tier A.

use percolator::v16::*;
use percolator::BOUND_SCALE;
use proptest::prelude::*;

/// Independent reference: native u128, no shared code with the engine helper.
/// Valid over the no-overflow domain face*rate_num <= u128::MAX (caller-ensured).
/// Returns Err(()) exactly when gross < paid (engine returns Err there).
fn ref_claimable(face: u128, rate_num: u128, rate_den: u128, paid: u128) -> Result<u128, ()> {
    assert!(rate_den > 0, "reference domain requires rate_den > 0");
    let gross = face
        .checked_mul(rate_num)
        .expect("reference domain bounds the product to u128")
        / rate_den;
    gross.checked_sub(paid).ok_or(())
}

/// A receipt that passes `validate_resolved_payout_receipt_value`:
/// present, exact_num(face*BOUND_SCALE) <= prior_bound, paid <= face,
/// finalized iff paid == face.
fn valid_receipt(face: u128, paid: u128) -> ResolvedPayoutReceiptV16 {
    ResolvedPayoutReceiptV16 {
        present: true,
        prior_bound_contribution_num: face
            .checked_mul(BOUND_SCALE)
            .expect("face*BOUND_SCALE within domain"),
        live_released_face_at_receipt: 0,
        terminal_positive_claim_face: face,
        paid_effective: paid,
        finalized: paid == face,
    }
}

fn ledger(rate_num: u128, rate_den: u128) -> ResolvedPayoutLedgerV16 {
    ResolvedPayoutLedgerV16 {
        snapshot_residual: 0,
        terminal_claim_exact_receipts_num: 0,
        terminal_claim_bound_unreceipted_num: 0,
        current_payout_rate_num: rate_num,
        current_payout_rate_den: rate_den,
        snapshot_slot: 1,
        payout_halted: false,
        finalized: false,
    }
}

fn check(face: u128, rate_num: u128, rate_den: u128, paid: u128) {
    let engine = MarketGroupV16ViewMut::<u64>::kani_resolved_receipt_claimable_against_ledger(
        valid_receipt(face, paid),
        ledger(rate_num, rate_den),
    );
    match ref_claimable(face, rate_num, rate_den, paid) {
        Ok(expect) => assert_eq!(
            engine,
            Ok(expect),
            "engine != reference at face={face} rn={rate_num} rd={rate_den} paid={paid}"
        ),
        Err(()) => assert!(
            engine.is_err(),
            "reference says gross<paid (Err) but engine returned {engine:?}"
        ),
    }
}

#[test]
fn tier_a_exhaustive_small_domain() {
    // Proof-by-enumeration of engine==reference over a small bounded domain.
    const B: u128 = 6;
    let mut n = 0u64;
    for face in 0..=B {
        for paid in 0..=face {
            for rate_num in 0..=B {
                for rate_den in 1..=B {
                    check(face, rate_num, rate_den, paid);
                    n += 1;
                }
            }
        }
    }
    eprintln!("tier_a_exhaustive: {n} cases (engine == reference, fully enumerated)");
}

#[test]
fn tier_b_seed_corpus_edges() {
    // Explicit seed corpus: denominator/numerator/rounding/boundary edges.
    let big = 1u128 << 50;
    let corpus: &[(u128, u128, u128, u128)] = &[
        (0, 0, 1, 0),
        (1, 1, 1, 0),
        (1, 1, 1, 1),                  // gross == paid -> 0
        (big, big, 1, 0),              // large product, den 1
        (big, 1, big, 0),              // floors to 0
        (7, 3, 4, 0),                  // floor(21/4) = 5
        (7, 3, 4, 5),                  // gross 5, paid 5 -> 0
        (100, 999, 1000, 0),           // floor(99900/1000) = 99 (one-less edge)
        (big, big, big, big - 1),      // gross ~ big, paid just below
        (big, big, big, big + 1),      // paid > gross -> Err
    ];
    for &(f, rn, rd, p) in corpus {
        if f.checked_mul(rn).is_none() || f.checked_mul(BOUND_SCALE).is_none() {
            continue; // outside the stated reference domain
        }
        check(f, rn, rd, p.min(f));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// Tier B sampling over the stated no-overflow domain. face,rate_num,rate_den
    /// up to 2^56 (product < 2^112 < u128), paid drawn in [0, face].
    #[test]
    fn tier_b_sampled_no_overflow_domain(
        face in 0u128..(1u128 << 56),
        rate_num in 0u128..(1u128 << 56),
        rate_den in 1u128..(1u128 << 56),
        paid_frac in 0u128..=1_000_000u128,
    ) {
        // Guard the stated domain (rarely trips for these bounds, but explicit).
        prop_assume!(face == 0 || rate_num <= u128::MAX / face);
        prop_assume!(face == 0 || face <= u128::MAX / BOUND_SCALE);
        let paid = face.saturating_mul(paid_frac) / 1_000_000; // paid in [0, face]
        check(face, rate_num, rate_den, paid);
    }
}
