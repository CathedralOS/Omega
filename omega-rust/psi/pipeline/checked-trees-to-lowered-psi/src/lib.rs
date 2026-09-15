#![forbid(unsafe_code)]

//! Checked trees to unsealed, target-neutral Psi.
//!
//! Output retains source custody, proof and debug companions for later stages.
//! Unsupported constructs fail closed; this stage does not optimize or publish.
//!
//! Start at `machine_lowering.rs`: it selects the checked machine, dispatches it
//! through `machine_lowering::machine_dispatch` to the plan family that owns its shape, and
//! sequences the custody, evidence, validation and debug work every selected
//! module needs. The plan families beneath it are:
//!
//! - [`unit`]: attached, dynamic composed, structural-control and cleanup Unit machines.
//! - [`returns`]: affine, boundary-scalar, payloadless and structural return machines.
//! - [`scalar_graph`]: scalar-graph preparation, call closure and module assembly.
//! - [`expression_preparation`]: source-bound expressions, bindings and independent replay.
//! - [`emission`]: operation and store emission shared by Unit and scalar bodies.
//! - [`retention`]: checked custody installed on the assembled module.
//! - [`proofs`]: propositions, contracts, certificates and evidence artifacts.
//!
//! Beside the route, `producer_result` defines source ownership and completion modes.
//! `lowering_error` and `terminal_identities` carry the
//! failure and identity vocabulary every producer shares and `debug_map`
//! presents the Terminal debug companion.

mod emission;
mod expression_preparation {
    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::lowering_error::{LoweringError, unsupported};
    use checked_trees::types::PrimitiveType;
    use checked_trees::{
        CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
        CheckedScalarExpression, CheckedScalarExpressionRole, CheckedScalarSuccessor, CheckedTrees,
        CheckedUnitStructuralPathSegment,
    };
    use language_semantics::Multiplicity;
    use numerics::arithmetic::ArithmeticDomain;
    use semantic_vocabulary::{
        IntegerSign, IntegerValue, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    };

    #[path = "bindings/bind_values.rs"]
    pub(crate) mod bindings;
    pub(crate) mod computation_graph;
    pub(crate) mod prepare_expression;
    pub(crate) mod qualifications;
    #[path = "source_custody/replay_source.rs"]
    pub(crate) mod source_custody;
}
mod lowering_error;
mod machine_lowering;
mod producer_result;
mod proofs;
mod retention;
mod returns;
mod scalar_graph;
mod terminal_identities;
mod unit;

pub use lowering_error::LoweringError;
pub use machine_lowering::machine_dispatch::{
    select_terminal_machine, select_terminal_machine_by_symbol,
};
pub use machine_lowering::{
    lower_bounded_callback_identity_machine, lower_machine, lower_machine_by_symbol,
};
pub use proofs::content_conservation::{
    LoweredContentConservation, LoweredContentIdentityReshuffles,
    LoweredContentPartitionComposition, LoweredContentPartitionCompositions,
    lower_boundary_content_guarantees, lower_content_conservation_plan,
    lower_content_identity_reshuffles, lower_content_partition_compositions,
};
pub use proofs::float_meaning_projection::{
    FloatMeaningProjectionLoweringError, lower_float_meaning_equality,
    lower_float_meaning_projection,
};
pub use proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
pub use proofs::quotient_correspondence::install_non_executable_quotient_correspondences;

#[cfg(test)]
mod tests;
