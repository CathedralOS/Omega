/*
We derive the validated optimized physical evidence scope and its surviving
operator and boundary occurrence roster from an optimized abstract operation
plan and Terminal boundary-application coverage.
*/

use boundary_applications::TerminalBoundaryApplicationCoverage;
use optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use semantic_vocabulary::OperationId;
use terminal_psi::TerminalPsiIdentity;
use verified_optimization_input::publication::EmissionProgramFacts;

use super::{
    ValidatedOptimizedNativePhysicalEvidenceScope, canonical_usize, native_optimization_projection,
    optimized_boundary_occurrence, optimized_operator_occurrence,
    validated_optimized_native_physical_evidence_scope,
};
use semantic_vocabulary::InvocationSite;

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
    final_plan: &EmissionProgramFacts,
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
    final_plan: &EmissionProgramFacts,
    d29_references: &[D29ReferenceCoordinate],
    boundary_application_coverage_identity: [u8; 32],
) -> Result<ValidatedOptimizedNativePhysicalEvidenceScope, &'static str> {
    if final_plan.psi != authority.terminal {
        return Err("optimized physical scope is detached from its validated Terminal identity");
    }

    let mut d29_operations = Vec::with_capacity(d29_references.len());
    for reference in d29_references {
        if reference.terminal != authority.terminal {
            return Err("optimized physical scope contains a detached D29 reference");
        }
        d29_operations.push(reference.operation);
    }
    d29_operations.sort_unstable();
    if d29_operations.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("optimized physical scope contains duplicate D29 references");
    }

    // Ordinary operation numbers belong to their machine. D29 demands have a
    // stricter module-wide unique-source contract, established by Terminal
    // admission; keep that constraint when deriving surviving occurrences.
    let mut final_operations = Vec::new();
    let mut surviving_d29 = vec![false; d29_operations.len()];
    for function in &final_plan.functions {
        for operation in function.source_occurrences.iter() {
            let Some(psi_operation) = operation.operation else {
                continue;
            };
            final_operations.push((function.machine, psi_operation));
            if let Ok(ordinal) = d29_operations.binary_search(&psi_operation) {
                if surviving_d29[ordinal] {
                    return Err("optimized physical scope repeats a surviving D29 operation");
                }
                surviving_d29[ordinal] = true;
            }
        }
    }
    final_operations.sort_unstable();
    if final_operations.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("optimized physical scope contains duplicate final operation identities");
    }

    let mut operator_occurrences = Vec::new();
    let mut boundary_occurrences = Vec::new();
    for function in &final_plan.functions {
        for (operation_ordinal, operation) in function.source_occurrences.iter().enumerate() {
            if let Some(psi_operation) = operation.operation
                && d29_operations.binary_search(&psi_operation).is_ok()
            {
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
            let invocation = operation.boundary;
            if let Some((boundary, site)) = invocation {
                let identity = optimized_boundary_occurrence_identity(
                    authority,
                    function.machine,
                    site,
                    boundary,
                    operation_ordinal,
                );
                boundary_occurrences.push(optimized_boundary_occurrence(
                    authority.terminal,
                    function.machine,
                    site,
                    boundary,
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
    operator_occurrences: &[super::OptimizedOperatorOccurrence],
    boundary_occurrences: &[super::OptimizedBoundaryOccurrence],
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
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(OPTIMIZED_SCOPE_IDENTITY_DOMAIN);
    digest.raw(&validated_authority_bytes(authority));
    digest.raw(&boundary_application_coverage);
    digest.raw(&projection.bytes());
    digest.finish()
}

fn optimized_operator_occurrence_identity(
    authority: ValidatedProjectionCoordinates,
    machine: semantic_vocabulary::MachineId,
    operation: OperationId,
    operation_ordinal: usize,
) -> OptimizedOperatorOccurrenceIdentity {
    let mut canonical = validated_authority_bytes(authority);
    super::identity_bytes::append_operator_occurrence_identity(
        &mut canonical,
        machine,
        operation,
        operation_ordinal,
    );
    OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical)
}

fn optimized_boundary_occurrence_identity(
    authority: ValidatedProjectionCoordinates,
    machine: semantic_vocabulary::MachineId,
    site: impl Into<InvocationSite>,
    boundary: semantic_vocabulary::BoundaryMachineId,
    operation_ordinal: usize,
) -> OptimizedBoundaryOccurrenceIdentity {
    let mut canonical = validated_authority_bytes(authority);
    super::identity_bytes::append_boundary_occurrence_identity(
        &mut canonical,
        machine,
        site,
        boundary,
        operation_ordinal,
    );
    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(&canonical)
}

fn validated_authority_bytes(authority: ValidatedProjectionCoordinates) -> Vec<u8> {
    let mut canonical = Vec::with_capacity(104);
    canonical.extend_from_slice(&super::identity_bytes::terminal_identity_bytes(
        authority.terminal,
    ));
    canonical.extend_from_slice(&authority.validation.bytes());
    canonical.extend_from_slice(&authority.final_unit.bytes());
    canonical
}

#[cfg(test)]
mod tests {
    use super::{D29ReferenceCoordinate, ValidatedProjectionCoordinates, derive_optimized_scope};
    use abstract_operations::AbstractFunctionResult;
    use optimization_core::{OptimizationUnitIdentity, OptimizedAbstractPlanProjectionIdentity};
    use semantic_vocabulary::{BoundaryMachineId, InvocationSite, MachineId, OperationId};
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
    use verified_optimization_input::publication::{
        EmissionFunctionFacts, EmissionOperationProvenance, EmissionProgramFacts,
    };

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

    fn final_plan(terminal: TerminalPsiIdentity) -> EmissionProgramFacts {
        let machine = MachineId::new(1).unwrap();
        EmissionProgramFacts {
            psi: terminal,
            entry: machine,
            functions: vec![EmissionFunctionFacts {
                machine,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                source_occurrences: vec![
                    EmissionOperationProvenance {
                        operation: Some(operation(10)),
                        boundary: None,
                    },
                    EmissionOperationProvenance {
                        operation: Some(operation(11)),
                        boundary: Some((
                            BoundaryMachineId::new(30).unwrap(),
                            InvocationSite::Operation(operation(11)),
                        )),
                    },
                    EmissionOperationProvenance {
                        operation: None,
                        boundary: None,
                    },
                ],
                requires_graph_storage_replay: false,
                descriptors: Vec::new(),
                primitive_locals: Vec::new(),
                scalar_call_results: Vec::new(),
                ram_obligations: Vec::new(),
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
            Some(operation(11))
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
    fn optimized_projection_scopes_ordinary_operation_numbers_to_their_machine() {
        let psi = terminal(1);
        let mut plan = final_plan(psi);
        let mut second = plan.functions[0].clone();
        second.machine = MachineId::new(2).unwrap();
        plan.functions.push(second);
        let scope = derive_optimized_scope(authority(psi), &plan, &[], [7; 32])
            .expect("distinct machines may reuse ordinary operation numbers");
        let boundaries = scope.projection().boundary_occurrences();
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].operation(), boundaries[1].operation());
        assert_ne!(boundaries[0].machine(), boundaries[1].machine());
        assert_ne!(boundaries[0].identity(), boundaries[1].identity());
    }

    #[test]
    fn optimized_projection_rejects_duplicate_operations_in_one_machine() {
        let psi = terminal(1);
        let mut plan = final_plan(psi);
        let duplicate = plan.functions[0].source_occurrences[0].clone();
        plan.functions[0].source_occurrences.push(duplicate);
        assert_eq!(
            derive_optimized_scope(authority(psi), &plan, &[], [7; 32]).unwrap_err(),
            "optimized physical scope contains duplicate final operation identities",
        );
    }

    #[test]
    fn optimized_projection_retains_unique_d29_source_among_machine_local_operations() {
        let psi = terminal(1);
        let mut plan = final_plan(psi);
        let mut second = plan.functions[0].clone();
        second.machine = MachineId::new(2).unwrap();
        second.source_occurrences[0].operation = Some(operation(12));
        plan.functions.push(second);
        let references = [D29ReferenceCoordinate {
            terminal: psi,
            operation: operation(12),
        }];
        let scope = derive_optimized_scope(authority(psi), &plan, &references, [7; 32])
            .expect("unique D29 source survives alongside reused ordinary numbers");
        let operators = scope.projection().operator_occurrences();
        assert_eq!(operators.len(), 1);
        assert_eq!(operators[0].machine(), MachineId::new(2).unwrap());
        assert_eq!(operators[0].operation(), operation(12));
        assert_eq!(operators[0].operation_ordinal(), 0);
    }

    #[test]
    fn optimized_projection_rejects_ambiguous_d29_sources_across_machines() {
        let psi = terminal(1);
        let mut plan = final_plan(psi);
        let mut second = plan.functions[0].clone();
        second.machine = MachineId::new(2).unwrap();
        plan.functions.push(second);
        let references = [D29ReferenceCoordinate {
            terminal: psi,
            operation: operation(10),
        }];
        assert_eq!(
            derive_optimized_scope(authority(psi), &plan, &references, [7; 32]).unwrap_err(),
            "optimized physical scope repeats a surviving D29 operation",
        );
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
