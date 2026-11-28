use super::{DescriptorDefinition, DescriptorMachine, DescriptorPlace};
use arena::NumericId;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use std::borrow::Cow;
use terminal_psi::{
    Block, OperationKind, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultDeclaration, TerminalMachine,
};

/// Dense identities address the body-local vector directly. Sparse or repeated
/// identities need a sorted lookup. Both representations retain every match.
struct DefinitionRows<Id, T> {
    rows: Vec<(Id, T)>,
    dense_start: Option<u64>,
}

impl<Id: NumericId, T> DefinitionRows<Id, T> {
    fn new(mut rows: Vec<(Id, T)>) -> Self {
        let dense_start = rows.first().map(|row| row.0.get()).filter(|start| {
            rows.iter()
                .enumerate()
                .all(|(index, (id, _))| id.get().checked_sub(*start) == Some(index as u64))
        });
        if dense_start.is_none() {
            // Preserve declaration order among owners of the same place.
            rows.sort_by_key(|row| row.0);
        }
        Self { rows, dense_start }
    }

    fn matches(&self, id: Id) -> &[(Id, T)] {
        if let Some(start) = self.dense_start {
            return id
                .get()
                .checked_sub(start)
                .and_then(|offset| usize::try_from(offset).ok())
                .and_then(|index| self.rows.get(index))
                .map_or(&[], std::slice::from_ref);
        }
        let start = self.rows.partition_point(|row| row.0 < id);
        let tail = &self.rows[start..];
        &tail[..tail.partition_point(|row| row.0 == id)]
    }
}

struct Definitions<'a> {
    result: Option<&'a StructuralResultDeclaration>,
    parameters: DefinitionRows<PlaceId, (Option<usize>, &'a StructuralParameterDeclaration)>,
    blocks: DefinitionRows<BlockId, &'a Block>,
    producers: DefinitionRows<OperationId, Option<&'a StructuralOperationResult>>,
    literals: DefinitionRows<PlaceId, ()>,
}

impl<'a> Definitions<'a> {
    fn new(machine: &'a TerminalMachine) -> Self {
        let parameters = machine
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(ordinal, parameter)| (parameter.place, (Some(ordinal), parameter)))
            .chain(
                machine
                    .block_structural_parameters()
                    .map(|parameter| (parameter.place, (None, parameter))),
            )
            .collect();
        let mut producers = Vec::new();
        let mut literals = Vec::new();
        let needs_producer = machine
            .structural_places
            .iter()
            .any(|row| matches!(row.kind, StructuralPlaceKind::OperationResult { .. }));
        let needs_literal = machine
            .structural_places
            .iter()
            .any(|row| matches!(row.kind, StructuralPlaceKind::ByteSequenceLiteral { .. }));
        if needs_producer || needs_literal {
            for operation in machine.operations() {
                #[cfg(test)]
                OPERATION_VISITS.set(OPERATION_VISITS.get() + 1);
                if needs_producer {
                    // A scalar or unit producer with the same ID also makes a result ambiguous.
                    producers.push((operation.id, operation.result.structural()));
                }
                if needs_literal
                    && let OperationKind::EstablishByteSequenceLiteral { destination, .. } =
                        operation.kind
                {
                    literals.push((destination, ()));
                }
            }
        }
        Self {
            result: machine.result.structural(),
            parameters: DefinitionRows::new(parameters),
            blocks: DefinitionRows::new(
                machine
                    .blocks
                    .iter()
                    .map(|block| (block.id, block))
                    .collect(),
            ),
            producers: DefinitionRows::new(producers),
            literals: DefinitionRows::new(literals),
        }
    }

    fn resolve(
        &self,
        place: &StructuralPlaceDeclaration,
    ) -> Option<(DescriptorDefinition, Option<StructuralTypeId>)> {
        let owners = self.parameters.matches(place.id);
        let definition = match place.kind {
            StructuralPlaceKind::Parameter { position } => {
                let mut matching = owners.iter().filter_map(|(_, (ordinal, parameter))| {
                    ordinal
                        .filter(|_| parameter.position == position)
                        .map(|ordinal| (ordinal, parameter))
                });
                let (position, parameter) = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                DescriptorDefinition::Parameter {
                    position,
                    access: parameter.access,
                    structural_type: parameter.structural_type,
                }
            }
            StructuralPlaceKind::BlockParameter { block, position } => {
                let [(_, owner)] = self.blocks.matches(block) else {
                    return None;
                };
                let parameter = owner.structural_parameters.get(position as usize)?;
                if parameter.place != place.id {
                    return None;
                }
                DescriptorDefinition::Arrival {
                    access: parameter.access,
                }
            }
            StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                let [_] = self.literals.matches(place.id) else {
                    return None;
                };
                DescriptorDefinition::Literal
            }
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                let [(_, Some(result))] = self.producers.matches(producer) else {
                    return None;
                };
                if result.place != place.id || result.structural_type != structural_type {
                    return None;
                }
                DescriptorDefinition::Local
            }
            _ => DescriptorDefinition::Local,
        };
        let ty = owners
            .first()
            .map(|(_, (_, parameter))| parameter.structural_type)
            .or_else(|| match place.kind {
                StructuralPlaceKind::Result => self
                    .result
                    .filter(|result| result.place == place.id)
                    .map(|result| result.structural_type),
                _ => place.kind.produced_structural_type(),
            });
        Some((definition, ty))
    }
}

pub(super) fn adapt_places<'a>(
    machine: &'a TerminalMachine,
    graph: &mut DescriptorMachine,
) -> Option<Cow<'a, [StructuralPlaceDeclaration]>> {
    let mut places = Cow::Borrowed(machine.structural_places.as_slice());
    if places.is_empty() {
        return Some(places);
    }
    if !places.is_sorted_by_key(|place| place.id) {
        places.to_mut().sort_unstable_by_key(|place| place.id);
    }
    if places.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return None;
    }
    let definitions = Definitions::new(machine);
    graph.places.reserve(places.len());
    for place in places.iter() {
        let (definition, ty) = definitions.resolve(place)?;
        graph.places.push(DescriptorPlace {
            producer: None,
            place: place.id,
            definition,
        });
        if let Some(ty) = ty {
            graph.place_types.insert(place.id, ty);
        }
    }
    Some(places)
}

#[cfg(test)]
thread_local! {
    static OPERATION_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests;
