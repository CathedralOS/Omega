//! Immutable shape selected by an exact proof-check identity rule.

use omega_optimization_unit::ProofCertifiedScalarIdentityKind;
use psi_core::{IntegerType, OperationId, ValueId};

#[derive(Debug, Clone, Copy)]
pub(in crate::rules::passes::proof_check_elision) struct ProofCertifiedScalarIdentityShape {
    pub(in crate::rules::passes::proof_check_elision) source_operation: OperationId,
    pub(in crate::rules::passes::proof_check_elision) result: ValueId,
    pub(in crate::rules::passes::proof_check_elision) replacement: ValueId,
    pub(in crate::rules::passes::proof_check_elision) identity_operand: ValueId,
    pub(in crate::rules::passes::proof_check_elision) scalar_type: IntegerType,
    pub(in crate::rules::passes::proof_check_elision) identity: ProofCertifiedScalarIdentityKind,
}
