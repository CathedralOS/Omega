//! Follow the complete owned field through the moves and local assignments
//! that implement a consuming record pattern. Each link must be the reaching
//! definition on every path to its next use.
use super::*;
use crate::descriptor_lifetimes::{DescriptorPosition, descriptor_location};
use crate::descriptor_results::{
    DescriptorAnalysis, terminal_borrowed_storage_graph_from,
    unchanged_stored_projection_sources_in,
};
use semantic_vocabulary::PlaceId;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    Opening(u32, OperationId),
    Move(PlaceId),
}

fn write(
    machine: &TerminalMachine,
    at: DescriptorPosition,
    origins: &ProjectedOrigins,
) -> Option<(PlaceId, Source)> {
    let op = machine.block(&at.0)?.operations.get(at.1 as usize)?;
    if let Some(result) = op.result.structural()
        && let Some(&(opening, producer)) =
            origins.get(&(machine.id, Projected::Owner(result.place)))
        && producer == op.id
    {
        return Some((result.place, Source::Opening(opening, producer)));
    }
    let (destination, input) = match &op.kind {
        OperationKind::MoveStructuralValue { source } => (op.result.structural()?.place, source),
        OperationKind::StoreStructuralField {
            destination,
            path,
            field: None,
            value,
            ..
        } if path.is_empty()
            && matches!(
                machine.producing_operation(*destination)?.0.kind,
                OperationKind::EstablishStructuralLocal {}
            ) =>
        {
            (*destination, value)
        }
        _ => return None,
    };
    (input.access == StructuralAccess::Owned
        && input.path.is_empty()
        && machine.structural_place_type(destination)?
            == machine.structural_place_type(input.place)?)
    .then_some((destination, Source::Move(input.place)))
}

pub(super) fn origin(
    module: &TerminalModule,
    machine: &TerminalMachine,
    origins: &ProjectedOrigins,
    mut place: PlaceId,
    use_site: OperationId,
) -> Option<(u32, OperationId)> {
    let graph = terminal_borrowed_storage_graph_from(module, [machine.id], &[]).ok()?;
    let types = &module.types.structural_types;
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, &graph);
    let mut cut = machine.blocks.iter().find_map(|block| {
        let index = block.operations.iter().position(|op| op.id == use_site)?;
        Some((block.id, u32::try_from(index).ok()?))
    })?;
    let location =
        |root| descriptor_location(types, root, machine.structural_place_type(root)?, &[], None);
    let mut seen = Vec::new();
    while !seen.contains(&(place, cut)) {
        seen.push((place, cut));
        let sources = unchanged_stored_projection_sources_in(
            types,
            &graph,
            &mut analysis,
            machine.id,
            cut,
            &location(place)?,
            |at| {
                write(machine, at, origins)
                    .and_then(|(destination, source)| Some((location(destination)?, (at, source))))
                    .into_iter()
                    .collect()
            },
        )?;
        let &[(at, source)] = sources.as_slice() else {
            return None;
        };
        match source {
            Source::Opening(index, producer) => return Some((index, producer)),
            Source::Move(input) => {
                place = input;
                cut = at;
            }
        }
    }
    None
}
