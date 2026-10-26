//! Ordered normal-edge positions for residence scheduling. Argument capture
//! precedes cleanup; destination aliases bind after the complete action stream.
//! Scratch coordinates are analysis positions, never emitted IR identities.

use super::{LoweringError, Motion, PlaceId, Residence, unsupported};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalMachine, Terminator,
};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowEdge, DescriptorFlowStep, DescriptorLocation, DescriptorPosition,
};
use terminal_semantics::descriptor_results::{DescriptorAnalysis, FlowFeasibility};

pub(super) struct Gap {
    pub(super) position: DescriptorPosition,
    block: usize,
    roster: usize,
    ordinal: usize,
}

pub(super) struct Action {
    pub(super) position: DescriptorPosition,
    pub(super) action: TerminalAffineCleanupAction,
    block: usize,
    roster: usize,
    ordinal: usize,
}

pub(super) struct NormalEdges {
    pub(super) flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    pub(super) gaps: Vec<Gap>,
    pub(super) actions: Vec<Action>,
    pub(super) returned: BTreeMap<DescriptorPosition, PlaceId>,
}

impl NormalEdges {
    /// Every round observes one complete schedule. Relocated deaths replace
    /// old observations; parents may shorten their intervals monotonically.
    pub(super) fn scheduled_flow(
        &self,
        motions: &[Motion],
    ) -> BTreeMap<DescriptorPosition, DescriptorFlowStep> {
        let mut flow = self.flow.clone();
        for action in &self.actions {
            if let TerminalAffineCleanupAction::RetireHome(place) = action.action
                && motions.iter().any(|motion| {
                    motion.place == place && !motion.retire.contains(&action.position)
                })
            {
                flow.get_mut(&action.position).unwrap().inputs.clear();
            }
        }
        for gap in &self.gaps {
            flow.get_mut(&gap.position).unwrap().inputs.extend(
                motions
                    .iter()
                    .filter(|motion| motion.retire.contains(&gap.position))
                    .map(|motion| DescriptorLocation::whole(motion.place)),
            );
        }
        flow
    }

    pub(super) fn new(
        machine: &TerminalMachine,
        types: &[StructuralTypeDeclaration],
        analysis: &mut DescriptorAnalysis<'_>,
        mut flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        residence_count: usize,
    ) -> Result<Self, LoweringError> {
        let mut gaps = Vec::new();
        let mut actions = Vec::new();
        let mut returned = BTreeMap::new();
        let feasibility = analysis.flow_feasibility(machine.id);
        for (block_position, block) in machine.blocks.iter().enumerate() {
            let terminator = (
                block.id,
                u32::try_from(block.operations.len()).map_err(|_| {
                    LoweringError::Unsupported("descriptor residence block exceeds u32")
                })?,
            );
            // Returns have cleanup rosters but no successor edge to filter.
            // An independently proved impossible terminator cannot contribute
            // a physical retirement frontier on a live path. Keep its emitted
            // roster untouched; unknown flow retains the ordinary proof gates.
            if feasibility.get(&terminator) == Some(&FlowFeasibility::ProvedImpossible) {
                continue;
            }
            if let Terminator::ReturnStructural { source, .. } = &block.terminator {
                returned.insert(terminator, *source);
            }
            if let Terminator::StructuralCase { source, path, .. } = &block.terminator {
                // A tag read needs its physical subject even without a
                // descriptor payload observation. Missing payload is not dead.
                let (location, opaque) = storage_observation(machine, types, *source, path);
                let canonical = analysis.location(machine.id, terminator, &location);
                let Some(step) = flow.get_mut(&terminator) else {
                    return unsupported("descriptor residence case has no terminator");
                };
                step.inputs.push(location);
                step.inputs.extend(canonical);
                step.may_escape |= opaque;
            }
            let rosters = block.terminator.cleanup_rosters().collect::<Vec<_>>();
            let Some(original) = flow.get(&terminator) else {
                return unsupported("descriptor residence graph lost a terminator");
            };
            let edges = original.successors.clone();
            if !edges.is_empty() && edges.len() != rosters.len() {
                return unsupported("descriptor residence cleanup and successor rosters disagree");
            }
            let mut next = terminator.1;
            let mut starts = Vec::new();
            for (roster, (_, cleanup)) in rosters.iter().enumerate() {
                if !edges.is_empty()
                    && analysis.edge_feasibility(machine.id, terminator, roster)
                        == FlowFeasibility::ProvedImpossible
                {
                    continue;
                }
                let mut positions = Vec::new();
                for ordinal in 0..=cleanup.len() {
                    // At most one death per candidate home on this edge. This
                    // finite domain permits every child-before-parent order
                    // without relying on production or solver traversal order.
                    for _ in 0..residence_count {
                        let position = next_position(block.id, &mut next)?;
                        positions.push(position);
                        flow.insert(position, DescriptorFlowStep::default());
                        gaps.push(Gap {
                            position,
                            block: block_position,
                            roster,
                            ordinal,
                        });
                    }
                    if let Some(action) = cleanup.get(ordinal) {
                        let position = next_position(block.id, &mut next)?;
                        positions.push(position);
                        let mut step = DescriptorFlowStep::default();
                        let (location, opaque) =
                            storage_observation(machine, types, action.place(), action.path());
                        step.inputs.push(location.clone());
                        if let Some(canonical) =
                            analysis.location(machine.id, terminator, &location)
                            && canonical != location
                        {
                            step.inputs.push(canonical);
                        }
                        // The existing nominal action remains an independent
                        // observation. Newly inserted deaths precede this step
                        // and never acquire its opaque escape classification.
                        step.may_escape = opaque
                            || matches!(action, TerminalAffineCleanupAction::InvokeNominal(_));
                        flow.insert(position, step);
                        actions.push(Action {
                            position,
                            action: action.clone(),
                            block: block_position,
                            roster,
                            ordinal,
                        });
                    }
                }
                for pair in positions.windows(2) {
                    flow.get_mut(&pair[0])
                        .unwrap()
                        .successors
                        .push(plain_edge(pair[1]));
                }
                if let Some(edge) = edges.get(roster) {
                    flow.get_mut(positions.last().unwrap())
                        .unwrap()
                        .successors
                        .push(edge.clone());
                }
                starts.push(plain_edge(positions[0]));
            }
            if !rosters.is_empty() {
                flow.get_mut(&terminator).unwrap().successors = starts;
            }
        }
        Ok(Self {
            flow,
            gaps,
            actions,
            returned,
        })
    }

    pub(super) fn install(
        &self,
        machine: &mut TerminalMachine,
        residences: &[Residence],
        motions: &[Motion],
    ) {
        for (block_position, block) in machine.blocks.iter_mut().enumerate() {
            for (roster, actions) in cleanup_rosters_mut(&mut block.terminator)
                .into_iter()
                .enumerate()
            {
                let old = std::mem::take(actions);
                for ordinal in 0..=old.len() {
                    for gap in self.gaps.iter().filter(|gap| {
                        gap.block == block_position
                            && gap.roster == roster
                            && gap.ordinal == ordinal
                    }) {
                        let mut additions = motions
                            .iter()
                            .filter(|motion| motion.retire.contains(&gap.position))
                            .collect::<Vec<_>>();
                        // Dependencies occupy distinct ordered positions;
                        // independent simultaneous deaths use reverse birth.
                        additions.sort_by_key(|motion| {
                            std::cmp::Reverse(
                                residences
                                    .iter()
                                    .find(|home| home.place == motion.place)
                                    .unwrap()
                                    .producer,
                            )
                        });
                        actions.extend(
                            additions.into_iter().map(|motion| {
                                TerminalAffineCleanupAction::RetireHome(motion.place)
                            }),
                        );
                    }
                    if let Some(action) = old.get(ordinal) {
                        let actual = self.actions.iter().find(|actual| {
                            actual.block == block_position
                                && actual.roster == roster
                                && actual.ordinal == ordinal
                        });
                        let removed = actual.is_some_and(|actual| matches!(action, TerminalAffineCleanupAction::RetireHome(place)
                            if motions.iter().any(|motion| motion.place == *place && !motion.retire.contains(&actual.position))));
                        if !removed {
                            actions.push(action.clone());
                        }
                    }
                }
            }
        }
    }
}

fn next_position(
    block: semantic_vocabulary::BlockId,
    next: &mut u32,
) -> Result<DescriptorPosition, LoweringError> {
    *next = next.checked_add(1).ok_or(LoweringError::Unsupported(
        "descriptor residence cleanup coordinates exceed u32",
    ))?;
    Ok((block, *next))
}

fn plain_edge(target: DescriptorPosition) -> DescriptorFlowEdge {
    DescriptorFlowEdge {
        target,
        parameters: Vec::new(),
        bindings: Vec::new(),
    }
}

fn storage_observation(
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> (DescriptorLocation, bool) {
    if let Some(ty) = place_type(machine, place)
        && let Some(location) = terminal_semantics::descriptor_lifetimes::descriptor_location(
            types, place, ty, path, None,
        )
    {
        return (location, false);
    }
    // A runtime-selected or unavailable coordinate cannot prove a smaller
    // lifetime. Observe the entire named carrier and preserve its interference;
    // ordinary typed cleanup validation still decides whether it is admitted.
    (DescriptorLocation::whole(place), true)
}

fn place_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            machine
                .structural_places
                .iter()
                .find(|declaration| declaration.id == place)
                .and_then(|declaration| declaration.kind.produced_structural_type())
        })
}

fn cleanup_rosters_mut(terminator: &mut Terminator) -> Vec<&mut Vec<TerminalAffineCleanupAction>> {
    match terminator {
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
        } => vec![
            &mut when_true.cleanup_actions,
            &mut when_false.cleanup_actions,
        ],
        Terminator::StructuralCase { cases, .. } => cases
            .iter_mut()
            .map(|case| &mut case.cleanup_actions)
            .collect(),
        Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => Vec::new(),
    }
}
