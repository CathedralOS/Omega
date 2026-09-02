use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};
use omega_boundary_applications::TerminalBoundaryApplicationCoverage;
use omega_optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use psi_core::OperationId;
use psi_terminal::TerminalPsiIdentity;
use sha2::{Digest, Sha256};

use super::model::{
    ValidatedOptimizedNativePhysicalEvidenceScope, native_optimization_projection,
    optimized_boundary_occurrence, optimized_operator_occurrence,
    validated_optimized_native_physical_evidence_scope,
};

const OPTIMIZED_SCOPE_IDENTITY_DOMAIN: &[u8] =
    b"omega.native-artifact.validated-optimized-physical-scope.sha256.v1\0";

#[derive(Clone, Copy)]
struct ValidatedProjectionCoordinates {
    terminal: TerminalPsiIdentity,
    validation: OptimizedAbstractPlanProjectionIdentity,
    final_unit: OptimizationUnitIdentity,
}

#[derive(Clone, Copy)]
struct D29ReferenceCoordinate {
    terminal: TerminalPsiIdentity,
    operation: OperationId,
}

pub(crate) fn derive_validated_optimization_scope(
    final_plan: &AbstractOperationPlan,
    terminal: TerminalPsiIdentity,
    validation: OptimizedAbstractPlanProjectionIdentity,
    final_unit: OptimizationUnitIdentity,
    boundary_application_coverage: &TerminalBoundaryApplicationCoverage,
    boundary_application_coverage_identity: [u8; 32],
) -> Result<ValidatedOptimizedNativePhysicalEvidenceScope, &'static str> {
    boundary_application_coverage
        .validate_for_terminal(terminal)
        .map_err(|_| "optimized physical scope has invalid D29 coverage")?;
    let references = boundary_application_coverage
        .references()
        .iter()
        .map(|reference| D29ReferenceCoordinate {
            terminal: reference.terminal(),
            operation: reference.terminal_operation(),
        })
        .collect::<Vec<_>>();
    derive_optimized_scope(
        ValidatedProjectionCoordinates {
            terminal,
            validation,
            final_unit,
        },
        final_plan,
        &references,
        boundary_application_coverage_identity,
    )
}

fn derive_optimized_scope(
    authority: ValidatedProjectionCoordinates,
    final_plan: &AbstractOperationPlan,
    d29_references: &[D29ReferenceCoordinate],
    boundary_application_coverage_identity: [u8; 32],
) -> Result<ValidatedOptimizedNativePhysicalEvidenceScope, &'static str> {
    if final_plan.psi != authority.terminal {
        return Err("optimized physical scope is detached from its validated Terminal identity");
    }

    let mut d29_operations = BTreeSet::new();
    for reference in d29_references {
        if reference.terminal != authority.terminal {
            return Err("optimized physical scope contains a detached D29 reference");
        }
        if !d29_operations.insert(reference.operation) {
            return Err("optimized physical scope contains duplicate D29 references");
        }
    }

    // Terminal operation identities are globally unique. Requiring that
    // invariant again on the final projection makes every retained D29 row an
    // exact join and prevents one reference from selecting multiple survivors.
    let mut final_operations = BTreeMap::new();
    for function in &final_plan.functions {
        for (operation_ordinal, operation) in function.operations.iter().enumerate() {
            let Some(psi_operation) = abstract_operation_psi_operation(operation) else {
                continue;
            };
            if final_operations
                .insert(psi_operation, (function.machine, operation_ordinal))
                .is_some()
            {
                return Err(
                    "optimized physical scope contains duplicate final operation identities",
                );
            }
        }
    }

    let mut operator_occurrences = Vec::new();
    let mut boundary_occurrences = Vec::new();
    for function in &final_plan.functions {
        for (operation_ordinal, operation) in function.operations.iter().enumerate() {
            let Some(psi_operation) = abstract_operation_psi_operation(operation) else {
                continue;
            };
            if d29_operations.contains(&psi_operation) {
                let identity = optimized_operator_occurrence_identity(
                    authority,
                    function.machine,
                    psi_operation,
                    operation_ordinal,
                );
                operator_occurrences.push(optimized_operator_occurrence(
                    authority.terminal,
                    function.machine,
                    psi_operation,
                    operation_ordinal,
                    identity,
                ));
            }
            if let AbstractOperation::BoundaryCall {
                boundary,
                psi_operation,
                ..
            } = operation
            {
                let identity = optimized_boundary_occurrence_identity(
                    authority,
                    function.machine,
                    *psi_operation,
                    *boundary,
                    operation_ordinal,
                );
                boundary_occurrences.push(optimized_boundary_occurrence(
                    authority.terminal,
                    function.machine,
                    *psi_operation,
                    *boundary,
                    operation_ordinal,
                    identity,
                ));
            }
        }
    }

    let projection_identity = optimized_projection_identity(
        authority,
        boundary_application_coverage_identity,
        &operator_occurrences,
        &boundary_occurrences,
    );
    let projection = native_optimization_projection(
        authority.terminal,
        operator_occurrences,
        boundary_occurrences,
        projection_identity,
    );
    let scope_identity = optimized_scope_identity(
        authority,
        boundary_application_coverage_identity,
        projection_identity,
    );
    Ok(validated_optimized_native_physical_evidence_scope(
        authority.validation,
        authority.final_unit,
        boundary_application_coverage_identity,
        projection,
        scope_identity,
    ))
}

fn optimized_projection_identity(
    authority: ValidatedProjectionCoordinates,
    boundary_application_coverage: [u8; 32],
    operator_occurrences: &[super::model::OptimizedOperatorOccurrence],
    boundary_occurrences: &[super::model::OptimizedBoundaryOccurrence],
) -> NativeOptimizationProjectionIdentity {
    let mut canonical = validated_authority_bytes(authority);
    canonical.extend_from_slice(&boundary_application_coverage);
    canonical.push(1);
    canonical.extend_from_slice(&canonical_usize(operator_occurrences.len()));
    for occurrence in operator_occurrences {
        canonical.extend_from_slice(&occurrence.identity().bytes());
    }
    canonical.push(2);
    canonical.extend_from_slice(&canonical_usize(boundary_occurrences.len()));
    for occurrence in boundary_occurrences {
        canonical.extend_from_slice(&occurrence.identity().bytes());
    }
    NativeOptimizationProjectionIdentity::from_canonical_bytes(&canonical)
}

fn optimized_scope_identity(
    authority: ValidatedProjectionCoordinates,
    boundary_application_coverage: [u8; 32],
    projection: NativeOptimizationProjectionIdentity,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(OPTIMIZED_SCOPE_IDENTITY_DOMAIN);
    digest.update(validated_authority_bytes(authority));
    digest.update(boundary_application_coverage);
    digest.update(projection.bytes());
    digest.finalize().into()
}

fn optimized_operator_occurrence_identity(
    authority: ValidatedProjectionCoordinates,
    machine: psi_core::MachineId,
    operation: OperationId,
    operation_ordinal: usize,
) -> OptimizedOperatorOccurrenceIdentity {
    let mut canonical = validated_authority_bytes(authority);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical)
}

fn optimized_boundary_occurrence_identity(
    authority: ValidatedProjectionCoordinates,
    machine: psi_core::MachineId,
    operation: OperationId,
    boundary: psi_core::BoundaryMachineId,
    operation_ordinal: usize,
) -> OptimizedBoundaryOccurrenceIdentity {
    let mut canonical = validated_authority_bytes(authority);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&boundary.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(&canonical)
}

fn validated_authority_bytes(authority: ValidatedProjectionCoordinates) -> Vec<u8> {
    let mut canonical = Vec::with_capacity(104);
    canonical.extend_from_slice(&terminal_identity_bytes(authority.terminal));
    canonical.extend_from_slice(&authority.validation.bytes());
    canonical.extend_from_slice(&authority.final_unit.bytes());
    canonical
}

fn abstract_operation_psi_operation(operation: &AbstractOperation) -> Option<OperationId> {
    match operation {
        AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. }
        | AbstractOperation::StructuralScalarFieldStore { psi_operation, .. }
        | AbstractOperation::EstablishPayloadlessCase { psi_operation, .. }
        | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
        | AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
        | AbstractOperation::CallUnit { psi_operation, .. }
        | AbstractOperation::CallStructuralScalar { psi_operation, .. }
        | AbstractOperation::CallStructuralScalarWithDynamicArguments { psi_operation, .. }
        | AbstractOperation::CallDynamicScalar { psi_operation, .. }
        | AbstractOperation::CallDynamicParameterScalar { psi_operation, .. }
        | AbstractOperation::CallStructural { psi_operation, .. }
        | AbstractOperation::BoundaryCall { psi_operation, .. }
        | AbstractOperation::PortWrite { psi_operation, .. }
        | AbstractOperation::Call { psi_operation, .. }
        | AbstractOperation::IntegerConstant { psi_operation, .. }
        | AbstractOperation::IeeeFloatConstant { psi_operation, .. }
        | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. }
        | AbstractOperation::BooleanConstant { psi_operation, .. }
        | AbstractOperation::BooleanStructuralField { psi_operation, .. }
        | AbstractOperation::IntegerStructuralField { psi_operation, .. }
        | AbstractOperation::BooleanNot { psi_operation, .. }
        | AbstractOperation::BooleanEqual { psi_operation, .. }
        | AbstractOperation::IntegerEqual { psi_operation, .. }
        | AbstractOperation::IntegerLessThan { psi_operation, .. }
        | AbstractOperation::IntegerLessOrEqual { psi_operation, .. }
        | AbstractOperation::IntegerBitwiseNot { psi_operation, .. }
        | AbstractOperation::IntegerWiden { psi_operation, .. }
        | AbstractOperation::IntegerExactCast { psi_operation, .. }
        | AbstractOperation::IntegerBitwiseAnd { psi_operation, .. }
        | AbstractOperation::IntegerBitwiseOr { psi_operation, .. }
        | AbstractOperation::IntegerBitwiseXor { psi_operation, .. }
        | AbstractOperation::WrappingIntegerShiftLeft { psi_operation, .. }
        | AbstractOperation::WrappingIntegerShiftRight { psi_operation, .. }
        | AbstractOperation::ExactIntegerShiftLeft { psi_operation, .. }
        | AbstractOperation::ExactIntegerShiftRight { psi_operation, .. }
        | AbstractOperation::WrappingIntegerAdd { psi_operation, .. }
        | AbstractOperation::ExactIntegerAdd { psi_operation, .. }
        | AbstractOperation::SaturatingIntegerAdd { psi_operation, .. }
        | AbstractOperation::WrappingIntegerSubtract { psi_operation, .. }
        | AbstractOperation::ExactIntegerSubtract { psi_operation, .. }
        | AbstractOperation::SaturatingIntegerSubtract { psi_operation, .. }
        | AbstractOperation::WrappingIntegerMultiply { psi_operation, .. }
        | AbstractOperation::ExactIntegerMultiply { psi_operation, .. }
        | AbstractOperation::ExactIntegerDivide { psi_operation, .. }
        | AbstractOperation::ExactIntegerRemainder { psi_operation, .. }
        | AbstractOperation::WrappingIntegerDivide { psi_operation, .. }
        | AbstractOperation::WrappingIntegerRemainder { psi_operation, .. }
        | AbstractOperation::SaturatingIntegerDivide { psi_operation, .. }
        | AbstractOperation::SaturatingIntegerRemainder { psi_operation, .. }
        | AbstractOperation::SaturatingIntegerMultiply { psi_operation, .. } => {
            Some(*psi_operation)
        }
        AbstractOperation::DynamicDescriptorParameter { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. }
        | AbstractOperation::Return { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::Crash { .. } => None,
    }
}

fn terminal_identity_bytes(terminal: TerminalPsiIdentity) -> Vec<u8> {
    let mut canonical = Vec::with_capacity(40);
    canonical.extend_from_slice(&terminal.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(terminal.program_fingerprint.as_bytes());
    canonical
}

fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("native optimized projection length fits u64")
        .to_le_bytes()
}

#[cfg(test)]
mod tests {
    use omega_abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractResult,
    };
    use omega_optimization_core::{
        OptimizationUnitIdentity, OptimizedAbstractPlanProjectionIdentity,
    };
    use psi_core::{
        BlockId, BoundaryMachineId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;

    fn terminal(marker: u8) -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([marker; 32]),
        }
    }

    fn authority(terminal: TerminalPsiIdentity) -> ValidatedProjectionCoordinates {
        ValidatedProjectionCoordinates {
            terminal,
            validation: OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(
                b"validated projection",
            ),
            final_unit: OptimizationUnitIdentity::from_canonical_bytes(b"final unit"),
        }
    }

    fn operation(value: u64) -> OperationId {
        OperationId::new(value).expect("nonzero operation")
    }

    fn final_plan(terminal: TerminalPsiIdentity) -> AbstractOperationPlan {
        let machine = MachineId::new(1).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        AbstractOperationPlan {
            psi: terminal,
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: BlockId::new(2).unwrap(),
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: operation(10),
                        result: ValueId::new(20).unwrap(),
                        scalar_type,
                        value: IntegerValue::Signed(7),
                    },
                    AbstractOperation::BoundaryCall {
                        psi_operation: operation(11),
                        result: Some(AbstractResult {
                            value: ValueId::new(21).unwrap(),
                            scalar_type,
                        }),
                        boundary: BoundaryMachineId::new(30).unwrap(),
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(40).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        }
    }

    #[test]
    fn optimized_projection_requires_children_only_for_final_survivors() {
        let psi = terminal(1);
        let references = [
            D29ReferenceCoordinate {
                terminal: psi,
                operation: operation(10),
            },
            // This exact Terminal D29 occurrence was proven eliminated and
            // therefore is absent from the final abstract plan.
            D29ReferenceCoordinate {
                terminal: psi,
                operation: operation(12),
            },
        ];
        let scope = derive_optimized_scope(authority(psi), &final_plan(psi), &references, [7; 32])
            .expect("validated final survivor projection");

        assert_eq!(scope.projection().operator_occurrences().len(), 1);
        assert_eq!(
            scope.projection().operator_occurrences()[0].operation(),
            operation(10)
        );
        assert_eq!(
            scope.projection().operator_occurrences()[0].operation_ordinal(),
            0
        );
        assert_eq!(scope.projection().boundary_occurrences().len(), 1);
        assert_eq!(
            scope.projection().boundary_occurrences()[0].operation(),
            operation(11)
        );
        assert_eq!(
            scope.projection().boundary_occurrences()[0].operation_ordinal(),
            1
        );
    }

    #[test]
    fn optimized_occurrence_identity_binds_validation_and_final_unit() {
        let psi = terminal(1);
        let references = [D29ReferenceCoordinate {
            terminal: psi,
            operation: operation(10),
        }];
        let baseline =
            derive_optimized_scope(authority(psi), &final_plan(psi), &references, [7; 32]).unwrap();
        let mut changed_authority = authority(psi);
        changed_authority.final_unit =
            OptimizationUnitIdentity::from_canonical_bytes(b"different final unit");
        let changed =
            derive_optimized_scope(changed_authority, &final_plan(psi), &references, [7; 32])
                .unwrap();

        assert_ne!(
            baseline.projection().operator_occurrences()[0].identity(),
            changed.projection().operator_occurrences()[0].identity(),
        );
        assert_ne!(baseline.identity(), changed.identity());
    }

    #[test]
    fn optimized_projection_rejects_duplicate_and_detached_d29_references() {
        let psi = terminal(1);
        let duplicate = D29ReferenceCoordinate {
            terminal: psi,
            operation: operation(10),
        };
        assert_eq!(
            derive_optimized_scope(
                authority(psi),
                &final_plan(psi),
                &[duplicate, duplicate],
                [7; 32],
            )
            .unwrap_err(),
            "optimized physical scope contains duplicate D29 references",
        );
        assert_eq!(
            derive_optimized_scope(
                authority(psi),
                &final_plan(psi),
                &[D29ReferenceCoordinate {
                    terminal: terminal(2),
                    operation: operation(10),
                }],
                [7; 32],
            )
            .unwrap_err(),
            "optimized physical scope contains a detached D29 reference",
        );
    }

    #[test]
    fn optimized_projection_rejects_a_detached_final_plan() {
        let psi = terminal(1);
        assert_eq!(
            derive_optimized_scope(authority(psi), &final_plan(terminal(2)), &[], [7; 32])
                .unwrap_err(),
            "optimized physical scope is detached from its validated Terminal identity",
        );
    }
}
