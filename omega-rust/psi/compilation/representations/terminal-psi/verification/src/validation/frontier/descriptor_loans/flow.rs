//! Descriptor value-instance transport reconstructed from Terminal operations.
//! Derivations keep every captured cohort; simultaneous edges rename values
//! from the pre-edge state. Cleanup that can observe payload keeps it live;
//! no-code disposal of an exact borrowed descriptor observes no referent.
//! Affine frontier replay still requires the carrier's complete disposition.

use std::collections::BTreeMap;

use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind};
use terminal_psi::{
    Operation, OperationKind, RecordFieldValue, StructuralArgument, TerminalAffineCleanupAction,
    TerminalMachine, TerminalModule, Terminator,
};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowEdge, DescriptorFlowStep, DescriptorPosition as Position,
};

pub(super) fn reconstruct(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> BTreeMap<Position, DescriptorFlowStep> {
    let mut steps = BTreeMap::new();
    for block in &machine.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let index = u32::try_from(index).expect("validated Terminal operation index fits u32");
            let definition = result_definition(machine, operation);
            let derivations = definition
                .map(|result| match &operation.kind {
                    OperationKind::EstablishElementView { source, .. }
                    | OperationKind::EstablishMutableElementView { source, .. }
                    | OperationKind::EstablishReference { source } => vec![(source.place, result)],
                    OperationKind::ElementViewSubslice { source, .. }
                    | OperationKind::ByteSequenceSubslice { source, .. } => vec![(*source, result)],
                    OperationKind::StructuralLeafCopy { source, .. }
                        if operation.result.structural().is_some_and(|result|
                            terminal_semantics::descriptor_results::is_shared_descriptor(module.structural_types.iter(), result.structural_type)) => vec![(*source, result)],
                    OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. } =>
                        terminal_semantics::descriptor_results::call_origins(module, machine, operation)
                            .into_iter().flatten().filter_map(|origin| match origin {
                                terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, .. } => Some((source, result)),
                                terminal_semantics::descriptor_results::DescriptorCallOrigin::Literal { .. } => None,
                            }).collect(),
                    OperationKind::EstablishRecord { fields }
                    | OperationKind::EstablishStructuralCase { fields, .. } => fields
                        .iter()
                        .filter_map(|field| match &field.value {
                            RecordFieldValue::Structural(argument) => {
                                Some((argument.place, result))
                            }
                            RecordFieldValue::Scalar { .. } => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                })
                .unwrap_or_default();
            let mut inputs = Vec::new();
            operation.kind.clone().map_structural_uses(&mut |place| {
                inputs.push(place);
                place
            });
            steps.insert(
                (block.id, index),
                DescriptorFlowStep {
                    definitions: definition.into_iter().collect(),
                    authority_derivations: if matches!(
                        operation.kind,
                        OperationKind::EstablishElementView { .. }
                            | OperationKind::EstablishMutableElementView { .. }
                            | OperationKind::EstablishReference { .. }
                            | OperationKind::ElementViewSubslice { .. }
                            | OperationKind::ByteSequenceSubslice { .. }
                            | OperationKind::StructuralLeafCopy { .. }
                            | OperationKind::CallStructural { .. }
                            | OperationKind::CallStructuralWithScalarArguments { .. }
                    ) {
                        derivations.clone()
                    } else {
                        Vec::new()
                    },
                    derivations,
                    inputs,
                    may_escape: super::accesses::may_escape(module, machine, operation),
                    successors: vec![DescriptorFlowEdge {
                        target: (block.id, index + 1),
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    }],
                },
            );
        }
        let mut step = DescriptorFlowStep::default();
        match &block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                step.inputs
                    .extend(structural_arguments.iter().map(|argument| argument.place));
                step.inputs.extend(
                    trivial_affine_discards
                        .iter()
                        .copied()
                        .filter(|place| !descriptor_disposal(module, machine, *place)),
                );
                step.inputs
                    .extend(residual_affine_discards.iter().map(|discard| discard.place));
                add_edge(machine, &mut step, *target, structural_arguments);
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    step.inputs.extend(
                        edge.structural_arguments
                            .iter()
                            .map(|argument| argument.place),
                    );
                    step.inputs.extend(
                        edge.trivial_affine_discards
                            .iter()
                            .copied()
                            .filter(|place| !descriptor_disposal(module, machine, *place)),
                    );
                    add_edge(machine, &mut step, edge.target, &edge.structural_arguments);
                }
            }
            Terminator::StructuralCase { source, cases } => {
                step.inputs.push(*source);
                for case in cases {
                    step.inputs.extend(
                        case.trivial_affine_discards
                            .iter()
                            .copied()
                            .filter(|place| !descriptor_disposal(module, machine, *place)),
                    );
                    // Case payload bindings are scalar in this vocabulary;
                    // no structural cohort is manufactured from field IDs.
                    add_edge(machine, &mut step, case.target, &[]);
                }
            }
            Terminator::ReturnStructural {
                source,
                trivial_affine_discards,
                ..
            } => {
                step.inputs.push(*source);
                step.inputs.extend(
                    trivial_affine_discards
                        .iter()
                        .copied()
                        .filter(|place| !descriptor_disposal(module, machine, *place)),
                );
                step.may_escape = true;
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                step.inputs
                    .extend(cleanup_actions.iter().filter_map(|action| match action {
                        TerminalAffineCleanupAction::DiscardRoot(place) => {
                            (!descriptor_disposal(module, machine, *place)).then_some(*place)
                        }
                        TerminalAffineCleanupAction::DiscardResidual(discard) => {
                            Some(discard.place)
                        }
                        TerminalAffineCleanupAction::InvokeNominal(cleanup) => Some(cleanup.place),
                    }));
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => {
                step.inputs.extend(
                    trivial_affine_discards
                        .iter()
                        .copied()
                        .filter(|place| !descriptor_disposal(module, machine, *place)),
                );
            }
            Terminator::ReturnUnitPartialAffine {
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                step.inputs.extend(
                    trivial_affine_discards
                        .iter()
                        .copied()
                        .filter(|place| !descriptor_disposal(module, machine, *place)),
                );
                step.inputs
                    .extend(residual_affine_discards.iter().map(|discard| discard.place));
            }
            Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                step.inputs
                    .extend(cleanups.iter().map(|cleanup| cleanup.place));
            }
            Terminator::Crash { .. } => {}
        }
        steps.insert(
            (
                block.id,
                u32::try_from(block.operations.len())
                    .expect("validated Terminal operation index fits u32"),
            ),
            step,
        );
    }
    steps
}

fn result_definition(machine: &TerminalMachine, operation: &Operation) -> Option<PlaceId> {
    let result = operation.result.structural()?;
    machine
        .structural_places
        .iter()
        .any(|declaration| {
            declaration.id == result.place
                && declaration.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    }
        })
        .then_some(result.place)
}

fn add_edge(
    machine: &TerminalMachine,
    step: &mut DescriptorFlowStep,
    target: BlockId,
    arguments: &[StructuralArgument],
) {
    let destination = machine.blocks.iter().find(|block| block.id == target);
    let parameters = destination
        .into_iter()
        .flat_map(|block| &block.structural_parameters)
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    if destination.is_some() && parameters.len() == arguments.len() {
        step.successors.push(DescriptorFlowEdge {
            target: (target, 0),
            bindings: arguments
                .iter()
                .zip(&parameters)
                .map(|(argument, parameter)| (argument.place, *parameter))
                .collect(),
            parameters,
        });
    } else {
        // Structural validation rejects this edge independently. Until then,
        // never truncate a zip and silently retire a possibly carried cohort:
        // retain every old name and every possible destination, and pin escape.
        step.may_escape = true;
        step.inputs
            .extend(machine.structural_places.iter().map(|place| place.id));
        step.successors.push(DescriptorFlowEdge {
            target: (target, 0),
            parameters: Vec::new(),
            bindings: machine
                .structural_places
                .iter()
                .flat_map(|place| {
                    parameters
                        .iter()
                        .map(move |parameter| (place.id, *parameter))
                })
                .collect(),
        });
    }
}

/// Only an authenticated affine view result has no-code carrier disposal.
/// Aggregate, residual and nominal cleanup may observe payload and stay live.
fn descriptor_disposal(module: &TerminalModule, machine: &TerminalMachine, place: PlaceId) -> bool {
    crate::validation::element_view::view_loan_result_in_module(module, machine, place).is_some_and(
        |(result, _)| {
            result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && module.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && (matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        ) || declaration.shape.is_borrowed_byte_view())
                })
        },
    )
}
