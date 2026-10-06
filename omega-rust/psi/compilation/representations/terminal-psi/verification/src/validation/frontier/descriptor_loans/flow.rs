//! Shared current-body descriptor transport with independent Terminal cleanup
//! observations. Slot replacement, alias writeback and call effects come from
//! the same prepared graph as backing-origin replay.
use semantic_vocabulary::PlaceId;
use std::collections::BTreeMap;
use terminal_psi::{TerminalAffineCleanupAction, TerminalMachine, TerminalModule, Terminator};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowEdge, DescriptorFlowStep, DescriptorLocation, DescriptorPosition as Position,
};
use terminal_semantics::descriptor_results::DescriptorAnalysis;

pub(super) fn reconstruct(
    module: &TerminalModule,
    machine: &TerminalMachine,
    analysis: &mut DescriptorAnalysis<'_>,
) -> BTreeMap<Position, DescriptorFlowStep> {
    let mut steps = analysis
        .flow(machine.id)
        .unwrap_or_else(|| unknown_flow(machine));
    for block in &machine.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            if let Some(step) = steps.get_mut(&(block.id, index as u32)) {
                step.may_escape |=
                    super::accesses::may_escape(module, machine, operation, analysis);
            }
        }
        let position = (block.id, block.operations.len() as u32);
        let Some(step) = steps.get_mut(&position) else {
            continue;
        };
        let mut observe = |place| {
            let raw = DescriptorLocation::whole(place);
            step.inputs.push(raw.clone());
            if let Some(location) = analysis.location(machine.id, position, &raw) {
                step.inputs.push(location);
            }
        };
        let disposal = |place| !descriptor_disposal(module, machine, place);
        let rosters = match &block.terminator {
            Terminator::Jump {
                cleanup_actions, ..
            }
            | Terminator::Return {
                cleanup_actions, ..
            }
            | Terminator::ReturnUnit {
                cleanup_actions, ..
            }
            | Terminator::ReturnStructural {
                cleanup_actions, ..
            } => vec![cleanup_actions],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![&when_true.cleanup_actions, &when_false.cleanup_actions],
            Terminator::StructuralCase { source, cases } => {
                observe(*source);
                cases.iter().map(|edge| &edge.cleanup_actions).collect()
            }
            Terminator::Crash { .. } => Vec::new(),
        };
        for actions in rosters {
            for action in actions {
                match action {
                    TerminalAffineCleanupAction::DiscardRoot(place) if disposal(*place) => {
                        observe(*place)
                    }
                    TerminalAffineCleanupAction::DiscardRoot(_) => {}
                    TerminalAffineCleanupAction::DiscardResidual(discard) => observe(discard.place),
                    TerminalAffineCleanupAction::InvokeNominal(cleanup) => observe(cleanup.place),
                }
            }
        }
    }
    steps
}

// Unresolved provenance never retires a cohort or grants a new carrier's
// authority. Preserve control reachability and pin every surviving old name.
fn unknown_flow(machine: &TerminalMachine) -> BTreeMap<Position, DescriptorFlowStep> {
    let mut steps = BTreeMap::new();
    for block in &machine.blocks {
        for index in 0..block.operations.len() {
            steps.insert(
                (block.id, index as u32),
                DescriptorFlowStep {
                    inputs: machine
                        .structural_places
                        .iter()
                        .map(|place| DescriptorLocation::whole(place.id))
                        .collect(),
                    may_escape: true,
                    successors: vec![DescriptorFlowEdge {
                        target: (block.id, index as u32 + 1),
                        parameters: vec![],
                        bindings: vec![],
                    }],
                    ..DescriptorFlowStep::default()
                },
            );
        }
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            _ => vec![],
        };
        steps.insert(
            (block.id, block.operations.len() as u32),
            DescriptorFlowStep {
                inputs: machine
                    .structural_places
                    .iter()
                    .map(|place| DescriptorLocation::whole(place.id))
                    .collect(),
                may_escape: true,
                successors: targets
                    .into_iter()
                    .map(|target| DescriptorFlowEdge {
                        target: (target, 0),
                        parameters: vec![],
                        bindings: vec![],
                    })
                    .collect(),
                ..DescriptorFlowStep::default()
            },
        );
    }
    steps
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

pub(super) fn location(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<DescriptorLocation> {
    terminal_semantics::descriptor_lifetimes::descriptor_location(
        &module.structural_types,
        root,
        super::accesses::root_type(machine, root)?,
        path,
        field,
    )
}
