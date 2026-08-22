//! Exact completion-receipt custody replay at final image boundaries.

use std::collections::BTreeSet;

use psi_core::ClaimId;
use psi_terminal::CompletionReceipt;

/// Replay the verifier's claim uniqueness and canonical receipt ordering.
///
/// Argument bounds are owned by the surrounding object or installation row;
/// this check prevents a serialized/native artifact from duplicating one
/// caller claim across several arguments or reordering an otherwise valid set.
pub(super) fn completion_receipts_have_exact_custody(receipts: &[CompletionReceipt]) -> bool {
    let mut claims = BTreeSet::<ClaimId>::new();
    receipts.windows(2).all(|pair| pair[0] < pair[1])
        && receipts.iter().all(|receipt| claims.insert(receipt.claim))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(claim: u64, argument_index: u32) -> CompletionReceipt {
        CompletionReceipt {
            claim: ClaimId::new(claim).expect("claim"),
            argument_index,
        }
    }

    #[test]
    fn exact_completion_receipt_custody_requires_canonical_unique_claims() {
        assert!(completion_receipts_have_exact_custody(&[]));
        assert!(completion_receipts_have_exact_custody(&[receipt(1, 0)]));
        assert!(completion_receipts_have_exact_custody(&[
            receipt(1, 1),
            receipt(2, 0),
        ]));

        assert!(!completion_receipts_have_exact_custody(&[
            receipt(2, 0),
            receipt(1, 1),
        ]));
        assert!(!completion_receipts_have_exact_custody(&[
            receipt(1, 0),
            receipt(1, 1),
        ]));
        assert!(!completion_receipts_have_exact_custody(&[
            receipt(1, 0),
            receipt(1, 0),
        ]));
    }
}
