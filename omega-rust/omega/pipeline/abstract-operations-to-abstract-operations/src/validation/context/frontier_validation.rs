//! Surviving obligation-owner and structural-frontier custody.

use super::super::{BTreeMap, O};
use super::{OptimizationUnitValidationError, PsiOptimizationUnit};
use crate::validation::member_blocks::invariant_member_parameters;
use crate::validation::place_observations::invariant_member_place_parameters;
use optimization_unit::{OptimizationFact, PsiOptimizationFunction, PsiProvenance};
pub(super) fn validate_surviving_frontiers(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    components: &[optimization_unit::OptimizerCycleComponent],
) -> Result<(), OptimizationUnitValidationError> {
    let context = input.context();
    let reconstructed = context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| (row.obligation.id, row))
        .collect::<BTreeMap<_, _>>();
    let accepted = context
        .accepted_facts()
        .iter()
        .map(|fact| (fact.obligation, fact))
        .collect::<BTreeMap<_, _>>();
    for function in &unit.functions {
        validate_surviving_byte_operations(context.module(), function, components)?;
        let Some(frontiers) = context.structural_frontiers().machine(function.machine) else {
            return Err(
                OptimizationUnitValidationError::MissingStructuralFrontierMachine(function.machine),
            );
        };
        for fact in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner_matches = reconstructed.get(obligation).is_some_and(|row| {
                row.owner
                    == terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            if !owner_matches || !accepted.contains_key(obligation) {
                return Err(
                    OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                );
            }
        }
        for site in function.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .flat_map(|node| node.provenance.iter().copied())
        }) {
            match site {
                PsiProvenance::Operation(operation)
                    if frontiers.operation_entry(operation).is_none()
                        || frontiers.operation_exit(operation).is_none() =>
                {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralOperationFrontier {
                            machine: function.machine,
                            operation,
                        },
                    );
                }
                PsiProvenance::Edge(edge) if frontiers.edge_entry(edge).is_none() => {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                            machine: function.machine,
                            edge,
                        },
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_surviving_byte_operations(
    module: &terminal_psi::TerminalModule,
    function: &PsiOptimizationFunction,
    components: &[optimization_unit::OptimizerCycleComponent],
) -> Result<(), OptimizationUnitValidationError> {
    // Member structural parameters whose reaching edges agree on one
    // representative root — reconstructed here from the transformed unit's own
    // structural bindings. A surviving byte observation may re-spell its root
    // as that representative; every other place stays byte-exact. The scalar
    // analog covers a relocated byte read's `index`/`length` operands: each
    // may re-spell an invariant member scalar parameter as its agreed
    // representative, and the obligation stays byte-exact. The run-covered
    // root set is empty: this unit is already transformed, so a root whose
    // producer relocated is produced in the preheader, not by a member, and
    // a still-member-produced root is genuinely loop-carried here.
    let representatives: BTreeMap<semantic_vocabulary::PlaceId, semantic_vocabulary::PlaceId> =
        components
            .iter()
            .filter(|component| component.id.machine == function.machine)
            .flat_map(|component| {
                invariant_member_place_parameters(
                    function,
                    component,
                    &std::collections::BTreeSet::new(),
                )
            })
            .collect();
    let scalar_representatives: BTreeMap<
        semantic_vocabulary::ValueId,
        semantic_vocabulary::ValueId,
    > = components
        .iter()
        .filter(|component| component.id.machine == function.machine)
        .flat_map(|component| invariant_member_parameters(function, component))
        .collect();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .map(|node| &node.operation)
    {
        let (psi_operation, obligation) = match operation {
            O::ByteSequenceRead {
                psi_operation,
                obligation,
                ..
            }
            | O::ByteSequenceSubslice {
                psi_operation,
                obligation,
                ..
            }
            | O::ByteSequenceWrite {
                psi_operation,
                obligation,
                ..
            }
            | O::StructuralByteSequenceFieldStore {
                psi_operation,
                obligation,
                ..
            }
            | O::StructuralByteSequenceFieldByteStore {
                psi_operation,
                obligation,
                ..
            } => (*psi_operation, Some(*obligation)),
            O::ByteSequenceLength { psi_operation, .. }
            | O::StructuralByteSequenceFieldLength { psi_operation, .. } => (*psi_operation, None),
            O::ElementViewRead {
                psi_operation,
                obligation,
                ..
            }
            | O::IndexedPrimitiveRead {
                psi_operation,
                obligation,
                ..
            }
            | O::ElementViewSubslice {
                psi_operation,
                obligation,
                ..
            } => (*psi_operation, Some(*obligation)),
            O::EstablishElementView { psi_operation, .. }
            | O::ElementViewLength { psi_operation, .. } => (*psi_operation, None),
            _ => continue,
        };
        let original = module
            .machines
            .iter()
            .find(|machine| machine.id == function.machine)
            .and_then(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == psi_operation)
            });
        let matches = original.is_some_and(|original| {
            let (result, kind) = match operation {
                O::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    index,
                    value,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Unit,
                    terminal_psi::OperationKind::StructuralByteSequenceFieldByteStore {
                        destination: *destination,
                        path: path.clone(),
                        field: *field,
                        index: *index,
                        value: *value,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Unit,
                    terminal_psi::OperationKind::StructuralByteSequenceFieldStore {
                        destination: *destination,
                        path: path.clone(),
                        field: *field,
                        source: *source,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::ByteSequenceWrite {
                    destination,
                    index,
                    value,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Unit,
                    terminal_psi::OperationKind::ByteSequenceWrite {
                        destination: *destination,
                        index: *index,
                        value: *value,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::ByteSequenceSubslice {
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Structural(result.clone()),
                    terminal_psi::OperationKind::ByteSequenceSubslice {
                        source: *source,
                        start: *start,
                        end: *end,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::ByteSequenceRead {
                    result,
                    source,
                    index,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        qualifications: Default::default(),
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::ByteSequenceRead {
                        source: *source,
                        index: *index,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::StructuralByteSequenceFieldLength {
                    result,
                    source,
                    path,
                    field,
                    ..
                } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        qualifications: Default::default(),
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::StructuralByteSequenceFieldLength {
                        source: *source,
                        path: path.clone(),
                        field: *field,
                    },
                ),
                O::ByteSequenceLength { result, source, .. } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        qualifications: Default::default(),
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::ByteSequenceLength { source: *source },
                ),
                O::EstablishElementView {
                    result,
                    destination,
                    source,
                    element,
                    ..
                } => (
                    terminal_psi::OperationResult::Structural(result.clone()),
                    terminal_psi::OperationKind::EstablishElementView {
                        destination: *destination,
                        source: source.clone(),
                        element: *element,
                    },
                ),
                O::ElementViewLength { result, source, .. } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        qualifications: Default::default(),
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::ElementViewLength { source: *source },
                ),
                O::ElementViewRead {
                    result,
                    source,
                    index,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        qualifications: Default::default(),
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::ElementViewRead {
                        source: *source,
                        index: *index,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                O::IndexedPrimitiveRead {
                    result,
                    source,
                    path,
                    index,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                        qualifications: Default::default(),
                        id: result.value,
                        scalar_type: result.scalar_type,
                    }),
                    terminal_psi::OperationKind::IndexedPrimitiveRead {
                        source: *source,
                        path: path.clone(),
                        index: index.value,
                        obligation: *obligation,
                    },
                ),
                O::ElementViewSubslice {
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                    ..
                } => (
                    terminal_psi::OperationResult::Structural(result.clone()),
                    terminal_psi::OperationKind::ElementViewSubslice {
                        source: *source,
                        start: *start,
                        end: *end,
                        length: *length,
                        obligation: *obligation,
                    },
                ),
                _ => return false,
            };
            original.result == result
                && byte_operation_kind_matches(
                    &original.kind,
                    &kind,
                    &representatives,
                    &scalar_representatives,
                )
        });
        if !matches {
            return Err(if let Some(obligation) = obligation {
                OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                    machine: function.machine,
                    operation: psi_operation,
                    obligation,
                }
            } else {
                OptimizationUnitValidationError::StructuralCatalogMismatch {
                    machine: Some(function.machine),
                }
            });
        }
    }
    Ok(())
}

/// Whether `actual` is `expected` modulo the substitutions an admitted
/// relocation may perform: an observation root rebound to its member
/// structural parameter's invariant representative, and — for
/// `ByteSequenceRead` and `ByteSequenceSubslice` — scalar operands
/// re-spelled as the representatives their invariant member scalar
/// parameters resolve to. Only
/// the four byte-observation kinds admitted for root rebinds tolerate the
/// place substitution — `ByteSequenceLength`'s whole root,
/// `StructuralByteSequenceFieldLength`'s root with its projection path and
/// field still byte-exact, `ByteSequenceRead`'s root with its obligation
/// still byte-exact, and `ByteSequenceSubslice`'s root with its endpoints
/// and obligation — and only when the expected root resolves to the actual
/// one. A subslice's structural result is compared outside this matcher, so
/// the fresh view place stays byte-exact. Every other kind, place, scalar,
/// and payload stays byte-exact.
fn byte_operation_kind_matches(
    expected: &terminal_psi::OperationKind,
    actual: &terminal_psi::OperationKind,
    representatives: &BTreeMap<semantic_vocabulary::PlaceId, semantic_vocabulary::PlaceId>,
    scalar_representatives: &BTreeMap<semantic_vocabulary::ValueId, semantic_vocabulary::ValueId>,
) -> bool {
    if expected == actual {
        return true;
    }
    let root_matches = |expected: semantic_vocabulary::PlaceId,
                        actual: semantic_vocabulary::PlaceId| {
        expected == actual || representatives.get(&expected) == Some(&actual)
    };
    let operand_matches = |expected: semantic_vocabulary::ValueId,
                           actual: semantic_vocabulary::ValueId| {
        expected == actual || scalar_representatives.get(&expected) == Some(&actual)
    };
    match (expected, actual) {
        (
            terminal_psi::OperationKind::ByteSequenceLength { source: expected },
            terminal_psi::OperationKind::ByteSequenceLength { source: actual },
        ) => root_matches(*expected, *actual),
        (
            terminal_psi::OperationKind::StructuralByteSequenceFieldLength {
                source: expected_source,
                path: expected_path,
                field: expected_field,
            },
            terminal_psi::OperationKind::StructuralByteSequenceFieldLength {
                source: actual_source,
                path: actual_path,
                field: actual_field,
            },
        ) => {
            expected_path == actual_path
                && expected_field == actual_field
                && root_matches(*expected_source, *actual_source)
        }
        (
            terminal_psi::OperationKind::ByteSequenceRead {
                source: expected_source,
                index: expected_index,
                length: expected_length,
                obligation: expected_obligation,
            },
            terminal_psi::OperationKind::ByteSequenceRead {
                source: actual_source,
                index: actual_index,
                length: actual_length,
                obligation: actual_obligation,
            },
        ) => {
            expected_obligation == actual_obligation
                && root_matches(*expected_source, *actual_source)
                && operand_matches(*expected_index, *actual_index)
                && operand_matches(*expected_length, *actual_length)
        }
        (
            terminal_psi::OperationKind::ByteSequenceSubslice {
                source: expected_source,
                start: expected_start,
                end: expected_end,
                length: expected_length,
                obligation: expected_obligation,
            },
            terminal_psi::OperationKind::ByteSequenceSubslice {
                source: actual_source,
                start: actual_start,
                end: actual_end,
                length: actual_length,
                obligation: actual_obligation,
            },
        ) => {
            expected_obligation == actual_obligation
                && root_matches(*expected_source, *actual_source)
                && operand_matches(*expected_start, *actual_start)
                && operand_matches(*expected_end, *actual_end)
                && operand_matches(*expected_length, *actual_length)
        }
        (
            terminal_psi::OperationKind::ElementViewLength { source: expected },
            terminal_psi::OperationKind::ElementViewLength { source: actual },
        ) => root_matches(*expected, *actual),
        (
            terminal_psi::OperationKind::ElementViewRead {
                source: expected_source,
                index: expected_index,
                length: expected_length,
                obligation: expected_obligation,
            },
            terminal_psi::OperationKind::ElementViewRead {
                source: actual_source,
                index: actual_index,
                length: actual_length,
                obligation: actual_obligation,
            },
        ) => {
            expected_obligation == actual_obligation
                && root_matches(*expected_source, *actual_source)
                && operand_matches(*expected_index, *actual_index)
                && operand_matches(*expected_length, *actual_length)
        }
        (
            terminal_psi::OperationKind::IndexedPrimitiveRead {
                source: expected_source,
                path: expected_path,
                index: expected_index,
                obligation: expected_obligation,
            },
            terminal_psi::OperationKind::IndexedPrimitiveRead {
                source: actual_source,
                path: actual_path,
                index: actual_index,
                obligation: actual_obligation,
            },
        ) => {
            expected_obligation == actual_obligation
                && expected_path == actual_path
                && root_matches(*expected_source, *actual_source)
                && operand_matches(*expected_index, *actual_index)
        }
        (
            terminal_psi::OperationKind::ElementViewSubslice {
                source: expected_source,
                start: expected_start,
                end: expected_end,
                length: expected_length,
                obligation: expected_obligation,
            },
            terminal_psi::OperationKind::ElementViewSubslice {
                source: actual_source,
                start: actual_start,
                end: actual_end,
                length: actual_length,
                obligation: actual_obligation,
            },
        ) => {
            expected_obligation == actual_obligation
                && root_matches(*expected_source, *actual_source)
                && operand_matches(*expected_start, *actual_start)
                && operand_matches(*expected_end, *actual_end)
                && operand_matches(*expected_length, *actual_length)
        }
        _ => false,
    }
}
