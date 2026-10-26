//! End an existing copyable descriptor residence at its actual normal lifetime.
//!
//! The body-local custody pass authorizes typed retirement and reconstructs
//! availability. It cannot know which callees retain a reference to a carrier.
//! Once the complete closure exists, the shared borrowed-storage replay supplies
//! those exact value/address cohorts. This owner only moves an already selected
//! RetireHome; it grants no disposal, claim, referent or restoration authority.
//! Unknown flow, escapes and ambiguous residence frontiers keep the original
//! schedule. Parameter homes retain their checked declaration continuation;
//! this completion owns operation-produced homes, including anonymous children.

use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{OperationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralMultiplicity, TerminalAffineCleanupAction, TerminalMachine, TerminalModule,
};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, DescriptorResidenceRules,
    descriptor_residence_live_positions,
};
use terminal_semantics::descriptor_results::DescriptorAnalysis;

mod normal_edges;

use normal_edges::NormalEdges;

struct Residence {
    place: PlaceId,
    producer: OperationId,
    birth: DescriptorPosition,
}

/// Prepare immutable type/flow interpretation once at the completed closure.
/// Ordinary reference slots and byte/element view slots share one inventory;
/// an address to the carrier itself must remain an observation of its home.
pub(crate) fn complete(module: &mut TerminalModule) -> Result<(), LoweringError> {
    let copyable_types =
        super::affine_cleanup::copyable_descriptor_type_ids(&module.structural_types);
    if copyable_types.is_empty() {
        return Ok(());
    }
    let graph = terminal_semantics::descriptor_results::terminal_borrowed_storage_graph(module);
    let rules = module
        .machines
        .iter()
        .map(|machine| {
            terminal_semantics::descriptor_results::terminal_residence_rules(module, machine)
        })
        .collect::<Vec<_>>();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    for (machine, rules) in module.machines.iter_mut().zip(rules) {
        let residences = residences(machine, &copyable_types)?;
        if residences.is_empty() {
            continue;
        }
        let Some(mut rules) = rules else {
            continue;
        };
        let Some(flow) = analysis.residence_flow(machine.id, &mut rules) else {
            // Incomplete return/effect coverage is not a dead-lifetime proof.
            continue;
        };
        let normal = NormalEdges::new(
            machine,
            &module.structural_types,
            &mut analysis,
            flow,
            residences.len(),
        )?;
        let mut motions = Vec::<Motion>::new();
        loop {
            let scheduled = normal.scheduled_flow(&motions);
            let mut additions = Vec::new();
            for residence in &residences {
                if let Some(motion) =
                    plan_residence(machine, &normal, &scheduled, &rules, residence, &motions)?
                {
                    let previous = motions
                        .iter()
                        .find(|previous| previous.place == motion.place);
                    // Moving a dependent's death earlier may shorten a parent's
                    // interval. Accept only monotone decreases in this finite
                    // position domain; no producer/pass ordering is assumed.
                    if previous.is_none_or(|previous| {
                        motion.live.is_subset(&previous.live) && motion.live != previous.live
                    }) {
                        additions.push(motion);
                    }
                }
            }
            if additions.is_empty() {
                break;
            }
            for motion in additions {
                motions.retain(|previous| previous.place != motion.place);
                motions.push(motion);
            }
        }
        normal.install(machine, &residences, &motions);
    }
    Ok(())
}

fn residences(
    machine: &TerminalMachine,
    copyable_types: &[semantic_vocabulary::StructuralTypeId],
) -> Result<Vec<Residence>, LoweringError> {
    let mut residences = Vec::new();
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let Some(result) = operation.result.structural() else {
                continue;
            };
            if result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.claims.is_empty()
                || !copyable_types.contains(&result.structural_type)
            {
                continue;
            }
            let actions = machine
                .blocks
                .iter()
                .flat_map(|block| block.terminator.cleanup_rosters())
                .flat_map(|(_, actions)| actions)
                .filter(|action| action.place() == result.place)
                .collect::<Vec<_>>();
            // Keep independently selected regional/nominal dispositions with
            // their original owner. Geometry cannot replace those obligations.
            if actions.is_empty()
                || actions
                    .iter()
                    .any(|action| !matches!(action, TerminalAffineCleanupAction::RetireHome(_)))
            {
                continue;
            }
            if residences
                .iter()
                .any(|residence: &Residence| residence.place == result.place)
            {
                return unsupported("descriptor residence has repeated operation births");
            }
            residences.push(Residence {
                place: result.place,
                producer: operation.id,
                birth: (
                    block.id,
                    u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("descriptor residence ordinal exceeds u32")
                    })?,
                ),
            });
        }
    }
    Ok(residences)
}

struct Motion {
    place: PlaceId,
    /// Exactly the cleanup gaps reached with this live physical occurrence.
    retire: BTreeSet<DescriptorPosition>,
    live: BTreeSet<DescriptorPosition>,
}

const UNBORN: u8 = 1;
const LIVE: u8 = 2;
const RETIRED: u8 = 4;

fn plan_residence(
    machine: &TerminalMachine,
    normal: &NormalEdges,
    scheduled: &BTreeMap<
        DescriptorPosition,
        terminal_semantics::descriptor_lifetimes::DescriptorFlowStep,
    >,
    rules: &DescriptorResidenceRules,
    residence: &Residence,
    motions: &[Motion],
) -> Result<Option<Motion>, LoweringError> {
    let descriptor = DescriptorLocation::whole(residence.place);
    let previous = motions
        .iter()
        .find(|motion| motion.place == residence.place);
    let retirements = previous
        .map(|motion| motion.retire.clone())
        .unwrap_or_else(|| {
            normal
                .actions
                .iter()
                .filter(|action| {
                    matches!(action.action,
            TerminalAffineCleanupAction::RetireHome(place) if place == residence.place)
                })
                .map(|action| action.position)
                .collect()
        });
    let live = descriptor_residence_live_positions(
        scheduled,
        rules,
        residence.birth,
        &descriptor,
        |position, input| !(retirements.contains(&position) && input == &descriptor),
    );
    if live.contains(&residence.birth) {
        // A surviving alias reaches reformation. Do not use one static birth
        // coordinate to end a previous dynamic occurrence.
        return Ok(None);
    }
    let baseline = residence_frontiers(machine, normal, residence, &BTreeSet::new())?;
    let cuts = normal
        .gaps
        .iter()
        .filter(|gap| baseline.get(&gap.position) == Some(&LIVE) && !live.contains(&gap.position))
        .map(|gap| gap.position)
        .collect::<BTreeSet<_>>();
    let planned = residence_frontiers(machine, normal, residence, &cuts)?;
    let mut retire = BTreeSet::new();
    for gap in &normal.gaps {
        let state = planned.get(&gap.position).copied().unwrap_or(0);
        if cuts.contains(&gap.position) {
            match state {
                LIVE => {
                    retire.insert(gap.position);
                }
                UNBORN | RETIRED | 0 => {}
                _ => {
                    // One path already ended the home. A plain RetireHome
                    // cannot stand in for a required selected residence test.
                    return Ok(None);
                }
            }
        }
    }
    for action in &normal.actions {
        if matches!(action.action, TerminalAffineCleanupAction::RetireHome(place) if place == residence.place)
        {
            match planned.get(&action.position).copied().unwrap_or(0) {
                LIVE => {
                    retire.insert(action.position);
                }
                RETIRED => {}
                _ => return Ok(None),
            }
        }
    }
    Ok(Some(Motion {
        place: residence.place,
        retire,
        live,
    }))
}

/// A finite union of physical occurrence states. Fixed cut points kill only
/// Live, so iteration is monotone; a mixed frontier is never permission to
/// retire a home which is absent on another incoming edge.
fn residence_frontiers(
    machine: &TerminalMachine,
    normal: &NormalEdges,
    residence: &Residence,
    cuts: &BTreeSet<DescriptorPosition>,
) -> Result<BTreeMap<DescriptorPosition, u8>, LoweringError> {
    let entry = (machine.entry, 0);
    let mut before = normal
        .flow
        .keys()
        .map(|position| (*position, 0))
        .collect::<BTreeMap<_, _>>();
    let Some(initial) = before.get_mut(&entry) else {
        return unsupported("descriptor residence graph has no entry");
    };
    *initial = UNBORN;
    let mut pending = vec![entry];
    while let Some(position) = pending.pop() {
        let step = &normal.flow[&position];
        let mut state = before[&position];
        if position == residence.birth {
            state = LIVE;
        }
        if cuts.contains(&position) {
            state = retire(state);
        }
        if normal.actions.iter().any(|action| {
            action.position == position
                && matches!(action.action, TerminalAffineCleanupAction::RetireHome(place) if place == residence.place)
        }) || normal.returned.get(&position) == Some(&residence.place)
        {
            state = retire(state);
        }
        for edge in &step.successors {
            let Some(target) = before.get_mut(&edge.target) else {
                return unsupported("descriptor residence graph lost a successor");
            };
            let joined = *target | state;
            if joined != *target {
                *target = joined;
                pending.push(edge.target);
            }
        }
    }
    Ok(before)
}

fn retire(state: u8) -> u8 {
    (state & !LIVE) | if state & LIVE != 0 { RETIRED } else { 0 }
}
