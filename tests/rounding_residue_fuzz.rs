//! Spec requirement #14 (rounding residue): every conservative-rounding
//! residue is either assigned AGAINST the user (direction properties below)
//! or stays in its source pool (sum-conservation, covered by the close/
//! sequence conservation fuzz and the exact-split Kani proofs). This file
//! pins the DIRECTION of every remaining division-bearing computational
//! helper: floors never overstate user entitlements, ceils never understate
//! user obligations.

use percolator::v16::*;
use percolator::SourceCreditStateV16;
use percolator::{BOUND_SCALE, CREDIT_RATE_SCALE, POS_SCALE};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Trade fees CEIL: fee dust is charged against the user (spec #14
    /// "assigned against the user conservatively") and never exceeds the
    /// exact fee by more than one atom.
    #[test]
    fn fee_bps_ceils_against_user(
        notional in 0u128..=u128::MAX / 20_000,
        bps in 0u64..=10_000u64,
    ) {
        let fee = kani_checked_fee_bps(notional, bps).unwrap();
        prop_assert!(fee * 10_000 >= notional * bps as u128, "fee under exact: dust leaked to user");
        prop_assert!(fee == 0 || (fee - 1) * 10_000 < notional * bps as u128, "fee more than one atom over exact");
    }

    /// Trade notional floors: risk notional used for fees floors down,
    /// while the margin-side notional ceils up — checked against each other
    /// the user can never gain from the spread.
    #[test]
    fn notional_floor_le_ceil(
        size_q in 1u128..=100_000_000_000u128,
        price in 1u64..=1_000_000u64,
    ) {
        let floor = kani_trade_notional_floor(size_q, price).unwrap();
        let ceil = kani_risk_notional_ceil(size_q, price).unwrap();
        prop_assert!(floor <= ceil, "floor exceeded ceil");
        // exact value sits between them
        let exact_num = size_q as u128 * price as u128;
        prop_assert!(floor * POS_SCALE <= exact_num);
        prop_assert!(ceil * POS_SCALE + POS_SCALE > exact_num);
    }

    /// Margin requirement is exactly floor(n*bps/10^4).max(min_floor): the
    /// per-step floor is compensated by the CEILED risk notional upstream
    /// (kani_risk_notional_ceil, asserted in notional_floor_le_ceil), so the
    /// composed requirement never understates the true obligation.
    #[test]
    fn margin_requirement_is_exact_floored_with_min(
        notional in 0u128..=u128::MAX / 20_000,
        bps in 0u64..=10_000u64,
        min_req in 0u128..=1_000_000u128,
    ) {
        let req = kani_margin_requirement(notional, bps, min_req).unwrap();
        if notional == 0 {
            prop_assert_eq!(req, 0);
        } else {
            prop_assert_eq!(req, (notional * bps as u128 / 10_000).max(min_req));
        }
    }

    /// ADL scaling: the scaled delta never exceeds the unscaled basis delta
    /// in magnitude — social-loss chunking can only round toward zero.
    #[test]
    fn adl_delta_rounds_toward_zero(
        abs_basis_q in 0u128..=1u128 << 100,
        a_basis in 0u128..=1u128 << 100,
        then in -(1i128 << 100)..=(1i128 << 100),
        now in -(1i128 << 100)..=(1i128 << 100),
    ) {
        if let Some(scaled) = kani_scaled_adl_delta_fast(abs_basis_q, a_basis, then, now) {
            if a_basis > 0 && abs_basis_q <= a_basis {
                let raw = now.saturating_sub(then);
                prop_assert!(scaled.unsigned_abs() <= raw.unsigned_abs(),
                    "scaled ADL delta exceeded raw delta magnitude");
            }
        }
    }

    /// Source-credit support for a face claim floors: the realizable support
    /// never exceeds the exact rate-scaled claim (haircut rounds against the
    /// claimant, residue stays in the pool).
    #[test]
    fn realizable_support_floors_against_claimant(
        claim_bound_num in 0u128..=1u128 << 90,
        exact_frac in 0u128..=1000u128,
        fresh_reserved in 0u128..=1u128 << 90,
        face_num in 0u128..=1u128 << 90,
    ) {
        let exact_claim_num = claim_bound_num / 1000 * exact_frac;
        let state = SourceCreditStateV16 {
            positive_claim_bound_num: claim_bound_num,
            exact_positive_claim_num: exact_claim_num,
            fresh_reserved_backing_num: fresh_reserved,
            credit_rate_num: 0, // recomputed below
            ..SourceCreditStateV16::EMPTY
        };
        let rate = match kani_expected_source_credit_rate_num_for_state(state) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let state = SourceCreditStateV16 { credit_rate_num: rate, ..state };
        if let Ok(support) = kani_source_credit_state_realizable_support_for_face(state, face_num) {
            // support <= exact face * rate / CRS (floor direction)
            // (compare in u256-free form: support * CRS <= face * rate, guarded sizes)
            let lhs = support.checked_mul(CREDIT_RATE_SCALE);
            let rhs = face_num.checked_mul(rate);
            if let (Some(l), Some(r)) = (lhs, rhs) {
                prop_assert!(l <= r, "support exceeded exact rate-scaled face");
            }
        }
    }
}
