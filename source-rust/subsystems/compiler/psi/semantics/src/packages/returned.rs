//! A closed package can return a reference borrowed from a concrete input.
//! Recover its referent from the initialized field that reaches the return.
use super::*;
use crate::descriptor_lifetimes::{
    DescriptorLocation, DescriptorPathSegment, DescriptorPosition, descriptor_location,
    descriptor_path,
};
use crate::descriptor_results::{
    DescriptorAnalysis, DescriptorResultSlot, DescriptorStoredOrigin, projected_stored_sources_in,
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
    let types = &module.types.structural_types;
    let checked = std::cell::OnceCell::new();
    for slot in slots {
        let Some(path) = descriptor_path(types, result.structural_type, &slot.path) else {
            continue;
        };
        if fields::hidden_reference_field(module, result.structural_type, &path).is_none() {
            continue;
        }
        let Some(checked) = checked
            .get_or_init(|| check_package_sites(module).ok())
            .as_ref()
        else {
            break;
        };
        for origin in &slot.origins {
            let Ok(input) =
                descriptor_input_origin_path(types, &machine.structural_parameters, origin)
            else {
                continue;
            };
            let request = Lender {
                machine: id,
                field: path.clone(),
                input: input.clone(),
            };
            if let Some(ty) = lender_type(module, graph, analysis, checked, request) {
                referents
                    .0
                    .entry(path.clone())
                    .or_default()
                    .insert(input, ty);
            }
        }
    }
    referents
}

/// Each dependency is a returned field of a particular callee and the
/// formal lender whose actual argument reached the current return.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Lender {
    machine: MachineId,
    field: Vec<StructuralPathSegment>,
    input: Input,
}

impl Lender {
    fn is_among(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        origins: &[DescriptorStoredOrigin],
    ) -> bool {
        origins.iter().any(|origin| {
            descriptor_input_origin_path(
                &module.types.structural_types,
                &machine.structural_parameters,
                origin,
            )
            .is_ok_and(|input| input == self.input)
        })
    }

    /// Substitute each callee lender separately. A branch choosing Socket
    /// cannot establish File for a different input of the caller.
    fn forwarded_lenders(
        &self,
        module: &TerminalModule,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        callee: MachineId,
        suffix: &[DescriptorPathSegment],
    ) -> Option<Vec<Self>> {
        let caller = module.machine(self.machine)?;
        let callee = module.machine(callee)?;
        let types = &module.types.structural_types;
        let field = descriptor_path(types, callee.result.structural()?.structural_type, suffix)?;
        let mut lenders = Vec::new();
        for (formal, actuals) in analysis.forwarded_origins(caller.id, position, suffix)? {
            if self.is_among(module, caller, &actuals) {
                let input =
                    descriptor_input_origin_path(types, &callee.structural_parameters, &formal)
                        .ok()?;
                lenders.push(Self {
                    machine: callee.id,
                    field: field.clone(),
                    input,
                });
            }
        }
        (!lenders.is_empty()).then_some(lenders)
    }
}

fn lender_type(
    module: &TerminalModule,
    graph: DescriptorRowView<'_>,
    analysis: &mut DescriptorAnalysis<'_>,
    checked: &CheckedPackageSites,
    root: Lender,
) -> Option<StructuralTypeId> {
    let types = &module.types.structural_types;
    let mut requests = vec![root.clone()];
    let mut positions = BTreeMap::from([(root, 0)]);
    let mut rows = Vec::new();
    while let Some(request) = requests.get(rows.len()).cloned() {
        let machine = module.machine(request.machine)?;
        let result = machine.result.structural()?.structural_type;
        let member = fields::hidden_reference_field(module, result, &request.field)?;
        let initializers = field_initializers(module, machine, checked, &member);
        let row = graph.find(machine.id)?;
        let mut fixed = Vec::new();
        let mut dependencies = Vec::new();
        for (at, returned) in row.returns.as_ref()? {
            let location = descriptor_location(types, returned.root, result, &request.field, None)?;
            let inputs = analysis.returned_origins(machine.id, *at, &location)?;
            if !request.is_among(module, machine, &inputs) {
                continue;
            }
            match reaching_source(
                types,
                machine.id,
                graph,
                analysis,
                &initializers,
                *at,
                location,
            )? {
                Source::Initializer(ty) => fixed.push(ty),
                Source::Call(position, suffix) => {
                    let callee = row.direct_call(position)?.callee()?;
                    for child in
                        request.forwarded_lenders(module, analysis, position, callee, &suffix)?
                    {
                        let index = *positions.entry(child.clone()).or_insert_with(|| {
                            requests.push(child);
                            requests.len() - 1
                        });
                        dependencies.push(Some(index));
                    }
                }
                Source::Transfer(..) => return None,
            }
        }
        if fixed.is_empty() && dependencies.is_empty() {
            return None;
        }
        rows.push(graphs::reach::ReachRow {
            fixed,
            dependencies,
        });
    }
    let resolved = graphs::reach::resolve(rows).ok()?;
    let [ty] = resolved.first()?.as_slice() else {
        return None;
    };
    Some(*ty)
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    Initializer(StructuralTypeId),
    Transfer(DescriptorPosition, DescriptorLocation),
    Call(DescriptorPosition, Vec<DescriptorPathSegment>),
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

/// Follow the actual value into this caller slot, then substitute the checked
/// return field's formal lenders with the arguments of that particular call.
pub(super) fn local_result_homes(
    module: &TerminalModule,
    caller: &TerminalMachine,
    graph: DescriptorRowView<'_>,
    analysis: &mut DescriptorAnalysis<'_>,
    at: DescriptorPosition,
    slot: &DescriptorLocation,
) -> Option<Vec<(semantic_vocabulary::PlaceId, Vec<StructuralPathSegment>)>> {
    let types = &module.types.structural_types;
    let Source::Call(position, suffix) = reaching_source(
        types,
        caller.id,
        graph,
        analysis,
        &Initializers::new(),
        at,
        slot.clone(),
    )?
    else {
        return None;
    };
    let operation = caller
        .blocks
        .iter()
        .find(|block| block.id == position.0)?
        .operations
        .get(usize::try_from(position.1).ok()?)?;
    let invocation = operation.declared_invocation()?;
    let terminal_psi::TargetMachineCallable::Machine(callee) = invocation.callee else {
        return None;
    };
    let result = operation.result.structural()?;
    let field = descriptor_path(types, result.structural_type, &suffix)?;
    let contents = analysis.result_contents(module, callee)?;
    let lenders = contents.package_referents.0.get(&field)?;
    let mut actual_homes = BTreeSet::new();
    for ((parameter, input), referent) in lenders {
        let argument = invocation.structural_arguments.get(*parameter)?;
        let mut path = argument.path.clone();
        for projection in input {
            let terminal_psi::ReferenceOriginProjection::Exact(segment) = projection else {
                return None;
            };
            path.push(segment.clone());
        }
        if path.iter().any(|segment| segment.runtime_index().is_some()) {
            return None;
        }
        let argument_slot = descriptor_location(
            types,
            argument.place,
            caller.structural_place_type(argument.place)?,
            &path,
            None,
        )?;
        for origin in analysis
            .captured_storage_family(caller.id, position, &argument_slot)
            .ok()?
        {
            let crate::descriptor_results::DescriptorCallOrigin::Argument { source, projection } =
                origin
            else {
                return None;
            };
            let path = projection.captured_storage_path()?;
            if path.iter().any(|segment| segment.runtime_index().is_some()) {
                return None;
            }
            let ty = caller.structural_place_type(source)?;
            if crate::runtime_structural_path_tip(types.iter(), ty, &path)?.id != *referent {
                return None;
            }
            actual_homes.insert((source, path));
        }
    }
    let homes = analysis.local_reference_homes(caller.id, at, slot)?;
    homes
        .iter()
        .all(|home| actual_homes.contains(home))
        .then_some(homes)
}

fn reaching_source(
    types: &[terminal_psi::StructuralTypeDeclaration],
    machine: MachineId,
    graph: DescriptorRowView<'_>,
    analysis: &mut DescriptorAnalysis<'_>,
    initializers: &Initializers,
    mut at: DescriptorPosition,
    mut location: DescriptorLocation,
) -> Option<Source> {
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
                if let Some(call) = row.direct_call(position)
                    && let Some(destination) = &call.result
                {
                    return vec![(destination.clone(), Source::Call(position, Vec::new()))];
                }
                step.value_transfers
                    .iter()
                    .map(|(from, to)| (to.clone(), Source::Transfer(position, from.clone())))
                    .collect()
            },
            |source, suffix| match source {
                Source::Initializer(_) => suffix.is_empty().then_some(source),
                Source::Call(position, _) => Some(Source::Call(position, suffix.to_vec())),
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
            Source::Initializer(_) | Source::Call(..) => return Some(source.clone()),
            Source::Transfer(position, input) => {
                at = *position;
                location = input.clone();
            }
        }
    }
    None
}
