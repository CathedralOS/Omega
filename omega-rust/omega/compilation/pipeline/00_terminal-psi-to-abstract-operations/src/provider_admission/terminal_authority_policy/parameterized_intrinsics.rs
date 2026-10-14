//! Parameterized compiler policy families cannot be enumerated by nominal children.
//! The committed family assigns host-authority classes only. A member's seal,
//! signature, live caller loan, address and execution are independently admitted.
//! This closed read/write family grants no host resource and no RAM backing.
use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerPlacedScalarOperation, TerminalAuthorityDisposition,
};

pub(super) const PARAMETERIZED_POLICY_FAMILY_COUNT: u32 = 1;
const PLACED_CALLER_RAM_SCALAR_POLICY_V1: &[u8] =
    b"omega.compiler-policy.placed-caller-ram-scalar.v1\0";

pub(super) fn classify(
    mechanism: CompilerIntrinsicExecutionIdentity,
) -> Option<TerminalAuthorityDisposition> {
    match mechanism {
        CompilerIntrinsicExecutionIdentity::PlacedScalarAccess {
            contract,
            placement_commitment,
            operation: CompilerPlacedScalarOperation::Read | CompilerPlacedScalarOperation::Write,
            ..
        } if contract != [0; 32] && placement_commitment != [0; 32] => {
            Some(TerminalAuthorityDisposition::from_classes([]))
        }
        _ => None,
    }
}

/// Commit the complete family predicate and its empty host-class assignment.
/// Read and Write are the only members; destructive Take is not classified.
pub(super) fn encode_commitment(hasher: &mut crate::digest_encoder::DomainDigestEncoder) {
    hasher.raw(&(PLACED_CALLER_RAM_SCALAR_POLICY_V1.len() as u32).to_be_bytes());
    hasher.raw(PLACED_CALLER_RAM_SCALAR_POLICY_V1);
    // Exact identity child axes are opaque, nonzero 32-byte contract and
    // placement commitments, a u32 slot, and one of these operation tags.
    hasher.raw(&[32, 32, 4, 2, 0, 2]);
    // Zero host classes. The checked caller RAM loan is never a host class.
    hasher.raw(&0_u32.to_be_bytes());
}
