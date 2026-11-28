//! Verify that the evaluated descriptor and its operands still describe the same subject.
use super::*;

pub(super) fn projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    index: u32,
    field: StructuralFieldId,
    checked: &mut CheckedPackageSites,
) -> Result<Projected, &'static str> {
    let opened = opening(module, machine.id, index)?;
    let schema = schema(module, opened.record)?;
    let declaration = fields(module, opened.record)?
        .iter()
        .find(|decl| decl.id == field && !decl.relevance.is_erased())
        .ok_or("package projection names an absent field")?;
    let (source, path) = match &operation.kind {
        OperationKind::PrimitiveScalarRead { source, path }
        | OperationKind::StructuralLeafCopy { source, path } => (source, path),
        _ => return Err("package projection requires a typed field read"),
    };
    let Some((StructuralPathSegment::Field(identity), parent)) = path.split_last() else {
        return Err("package projection does not read a field");
    };
    if *identity != declaration.identity
        || !same_subject_at(module, machine, operation, opened, *source, parent)
    {
        return Err("package projection reads a field of another subject");
    }
    let (projected, actual) = match &operation.kind {
        OperationKind::PrimitiveScalarRead { .. } => {
            let value = operation
                .result
                .scalar()
                .ok_or("package descriptor projection lacks a scalar result")?;
            let ScalarType::Callable(contract) = value.scalar_type else {
                return Err("projected value is not callable");
            };
            checked
                .descriptors
                .insert((machine.id, operation.id), (index, contract));
            (
                Projected::Descriptor(value.id),
                StructuralFieldType::Scalar(value.scalar_type),
            )
        }
        OperationKind::StructuralLeafCopy { .. } => {
            let value = operation
                .result
                .structural()
                .ok_or("package field copy lacks a structural result")?;
            let ty = module
                .structural_type(value.structural_type)
                .ok_or("opened reference has no type")?;
            if !matches!(
                ty.shape,
                StructuralTypeShape::Reference {
                    access: StructuralAccess::SharedBorrow,
                    ..
                }
            ) {
                return Err("only shared references can be copied from an opened package");
            }
            (
                Projected::Reference(value.place),
                StructuralFieldType::Structural(value.structural_type),
            )
        }
        _ => unreachable!("checked package projection operation"),
    };
    let substitution = Substitution {
        public: &schema.witnesses,
        actual: &opened.witnesses,
    };
    if !substitution.admits(module, &declaration.field_type, &actual) {
        return Err("package projection publishes a type outside its opening");
    }
    checked
        .fields
        .insert((machine.id, operation.id, field), actual);
    checked
        .projected_fields
        .insert((machine.id, operation.id), field);
    Ok(projected)
}

/// Follow ordinary checked reference formation at the operation's prestate.
/// Equal sets of possible targets do not prove that two references select
/// the same one. Only one resolved storage location can match the anchor.
pub(super) fn same_subject_at(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    opened: &TerminalPackageOpening,
    place: semantic_vocabulary::PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    use crate::descriptor_results::{DescriptorAnalysis, terminal_borrowed_storage_graph_from};
    use crate::descriptors::descriptor_lifetimes::{DescriptorLocation, descriptor_location};
    let anchor = &opened.subject;
    if anchor.place == place && crate::structural_paths_select_same_location(&anchor.path, path) {
        return true;
    }
    if path
        .iter()
        .chain(&anchor.path)
        .any(|step| matches!(step, StructuralPathSegment::RuntimeIndex { .. }))
    {
        return false;
    }
    let Some(position) = machine.blocks.iter().find_map(|block| {
        let offset = block
            .operations
            .iter()
            .position(|op| op.id == operation.id)?;
        Some((block.id, u32::try_from(offset).ok()?))
    }) else {
        return false;
    };
    let Ok(graph) = terminal_borrowed_storage_graph_from(module, [machine.id], &[]) else {
        return false;
    };
    let types = &module.types.structural_types;
    let mut replay = DescriptorAnalysis::with_borrowed_storage_slots(types, &graph);
    let mut address = |place, path: &[StructuralPathSegment]| {
        let root = machine.structural_place_type(place)?;
        let ty = crate::runtime_structural_path_tip(types.iter(), root, path)?;
        if ty.id != opened.record {
            return None;
        }
        let coordinate = descriptor_location(types, place, root, path, None)?;
        let resolved = replay.location(machine.id, position, &coordinate)?;
        DescriptorLocation::captured(resolved.root, &resolved.path)
    };
    address(anchor.place, &anchor.path)
        .is_some_and(|recorded| address(place, path) == Some(recorded))
}

pub(super) fn subject_field<'a>(
    module: &'a TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    opening: &TerminalPackageOpening,
    arg: &StructuralArgument,
) -> Option<&'a StructuralFieldDeclaration> {
    let [
        parent @ ..,
        StructuralPathSegment::Field(field),
        StructuralPathSegment::Referent,
    ] = arg.path.as_slice()
    else {
        return None;
    };
    if !same_subject_at(module, machine, operation, opening, arg.place, parent) {
        return None;
    }
    fields(module, opening.record)
        .ok()?
        .iter()
        .find(|decl| decl.identity == *field && !decl.relevance.is_erased())
}

pub(super) fn invocation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    index: u32,
    projected: &ProjectedOrigins,
    checked: &mut CheckedPackageSites,
) -> Result<CallableContractId, &'static str> {
    let opened = opening(module, machine.id, index)?;
    let OperationKind::CallMachine {
        callee,
        contract,
        structural_arguments,
        ..
    } = &operation.kind
    else {
        return Err("package invocation is not an evaluated machine call");
    };
    let Some(&(origin, producer)) = projected.get(&(machine.id, Projected::Descriptor(*callee)))
    else {
        return Err("package invocation has no projected descriptor");
    };
    if origin != index
        || !versions::same_storage_version(module, machine, opened, producer, operation.id, None)
    {
        return Err("package invocation has a different opening or stale descriptor");
    }
    let interface = module
        .execution
        .callables
        .contract(*contract)
        .ok_or("package invocation lacks a callable contract")?;
    if scalar_type(machine, *callee) != Some(ScalarType::Callable(*contract))
        || interface.structural_parameters.len() != structural_arguments.len()
    {
        return Err("package call differs from its projected callable");
    }
    let schema = schema(module, opened.record)?;
    for (i, (formal, arg)) in interface
        .structural_parameters
        .iter()
        .zip(structural_arguments)
        .enumerate()
    {
        let Some(position) = opened
            .witnesses
            .iter()
            .position(|ty| *ty == formal.structural_type)
        else {
            continue;
        };
        let field = subject_field(module, machine, operation, opened, arg)
            .ok_or("package argument comes from another opening")?;
        let Some(StructuralTypeShape::Reference { referent, access }) = field
            .field_type
            .as_structural()
            .and_then(|ty| module.structural_type(ty))
            .map(|ty| &ty.shape)
        else {
            return Err("package argument is not a borrowed field");
        };
        if *referent != schema.witnesses[position]
            || !terminal_psi::structural_access_covers(*access, arg.access)
            || arg.access != formal.access
        {
            return Err("package argument differs from its opened parameter");
        }
        checked
            .borrowed_arguments
            .insert((machine.id, operation.id, i), *referent);
    }
    Ok(*contract)
}
