//! A closed package can return a reference borrowed from a concrete input.
//! Recover its referent from the initialized field that reaches the return.
use super::*;
use crate::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPosition, descriptor_location, descriptor_path,
};
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorResultSlot, projected_stored_sources_in,
};
use crate::module_analyses::DescriptorRowView;
use crate::references::reference_origins::descriptor_regions::descriptor_input_origin_path;
use std::collections::BTreeSet;

type Input = (usize, Vec<terminal_psi::ReferenceOriginProjection>);

/// A concrete referent belongs to the actual input that reached one field,
/// even when separate returns of the same package close over different types.
#[derive(Clone, Default)]
pub(crate) struct ReturnedReferences(
    BTreeMap<Vec<StructuralPathSegment>, BTreeMap<Input, StructuralTypeId>>,
);

impl ReturnedReferences {
    pub(crate) fn referent(
        &self,
        field: &[StructuralPathSegment],
        parameter: usize,
        path: &[terminal_psi::ReferenceOriginProjection],
    ) -> Option<StructuralTypeId> {
        self.0.get(field)?.get(&(parameter, path.to_vec())).copied()
    }
}

pub(crate) fn returned_referents(
    module: &TerminalModule,
    id: MachineId,
    graph: DescriptorRowView<'_>,
    analysis: &mut DescriptorAnalysis<'_>,
    slots: &[DescriptorResultSlot],
) -> ReturnedReferences {
    let mut referents = ReturnedReferences::default();
    if module.execution.packages.schemas.is_empty() {
        return referents;
    }
    let Some(machine) = module.machine(id) else {
        return referents;
    };
    let Some(result) = machine.result.structural() else {
        return referents;
    };
    let Some(returns) = graph.find(id).and_then(|row| row.returns.as_ref()) else {
        return referents;
    };
    let types = &module.types.structural_types;
    let checked = std::cell::OnceCell::new();
    for slot in slots {
        let Some(path) = descriptor_path(types, result.structural_type, &slot.path) else {
            continue;
        };
        let Some(member) = fields::hidden_reference_field(module, result.structural_type, &path)
        else {
            continue;
        };
        let Some(checked) = checked
            .get_or_init(|| check_package_sites(module).ok())
            .as_ref()
        else {
            break;
        };
        let initializers = field_initializers(module, machine, checked, &member);
        if initializers.is_empty() {
            continue;
        }
        let mut choices = BTreeMap::new();
        let complete = returns.iter().try_for_each(|(at, returned)| {
            let location =
                descriptor_location(types, returned.root, result.structural_type, &path, None)?;
            let ty = initializer_referent(
                types,
                id,
                graph,
                analysis,
                &initializers,
                *at,
                location.clone(),
            )?;
            for origin in analysis.returned_origins(id, *at, &location)? {
                let input =
                    descriptor_input_origin_path(types, &machine.structural_parameters, &origin)
                        .ok()?;
                if choices
                    .insert(input, ty)
                    .is_some_and(|previous| previous != ty)
                {
                    return None;
                }
            }
            Some(())
        });
        if complete.is_some() {
            referents.0.insert(path, choices);
        }
    }
    referents
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    Initializer(StructuralTypeId),
    Transfer(DescriptorPosition, DescriptorLocation),
}

type Initializers = BTreeMap<DescriptorPosition, (DescriptorLocation, StructuralTypeId)>;

fn field_initializers(
    module: &TerminalModule,
    machine: &TerminalMachine,
    checked: &CheckedPackageSites,
    member: &fields::HiddenReferenceField<'_>,
) -> Initializers {
    let types = &module.types.structural_types;
    let mut writes = Initializers::new();
    let Ok(declaration) = fields(module, member.record) else {
        return writes;
    };
    let Some(field) = declaration.iter().find(|field| field.id == member.field) else {
        return writes;
    };
    let mut path = vec![StructuralPathSegment::Field(field.identity)];
    path.extend_from_slice(member.path);
    for block in &machine.blocks {
        for (index, op) in block.operations.iter().enumerate() {
            let Some(concrete) = checked
                .field_type(machine.id, op.id, member.field)
                .and_then(StructuralFieldType::as_structural)
            else {
                continue;
            };
            let Some(result) = op.result.structural() else {
                continue;
            };
            if result.structural_type != member.record
                || !matches!(op.kind, OperationKind::EstablishRecord { .. })
            {
                continue;
            }
            let Some(reference) =
                crate::inline_descriptor_type(types.iter(), concrete, member.path)
            else {
                continue;
            };
            if let StructuralTypeShape::Reference { referent, access } = reference.shape
                && access == member.access
                && let Some(stored) =
                    descriptor_location(types, result.place, member.record, &path, None)
                && let Ok(index) = u32::try_from(index)
            {
                writes.insert((block.id, index), (stored, referent));
            }
        }
    }
    writes
}

fn initializer_referent(
    types: &[terminal_psi::StructuralTypeDeclaration],
    machine: MachineId,
    graph: DescriptorRowView<'_>,
    analysis: &mut DescriptorAnalysis<'_>,
    initializers: &Initializers,
    mut at: DescriptorPosition,
    mut location: DescriptorLocation,
) -> Option<StructuralTypeId> {
    let row = graph.find(machine)?;
    let mut visited = BTreeSet::new();
    while visited.insert((at, location.clone())) {
        let sources = projected_stored_sources_in(
            types,
            graph,
            analysis,
            machine,
            at,
            &location,
            |position| {
                let Some(step) = row.flow.get(&position) else {
                    return Vec::new();
                };
                if let Some((stored, referent)) = initializers.get(&position) {
                    return vec![(stored.clone(), Source::Initializer(*referent))];
                }
                step.value_transfers
                    .iter()
                    .map(|(from, to)| (to.clone(), Source::Transfer(position, from.clone())))
                    .collect()
            },
            |source, suffix| match source {
                Source::Initializer(_) => suffix.is_empty().then_some(source),
                Source::Transfer(position, mut input) => {
                    input.path.extend_from_slice(suffix);
                    Some(Source::Transfer(position, input))
                }
            },
        )?;
        let [source] = sources.as_slice() else {
            return None;
        };
        match source {
            Source::Initializer(ty) => return Some(*ty),
            Source::Transfer(position, input) => {
                at = *position;
                location = input.clone();
            }
        }
    }
    None
}
