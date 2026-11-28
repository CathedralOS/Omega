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
        OperationKind::MoveStructuralValue { source }
            if source.access == StructuralAccess::Owned =>
        {
            (&source.place, &source.path)
        }
        OperationKind::MoveStructuralField {
            source,
            path,
            field: None,
        } => (source, path),
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
        OperationKind::MoveStructuralValue { .. } | OperationKind::MoveStructuralField { .. } => {
            let public = declaration
                .field_type
                .as_structural()
                .ok_or("moved package field is not structural")?;
            if !schema
                .witnesses
                .iter()
                .any(|&witness| fields::nominal_uses(module, public, witness))
            {
                return Err("a consuming package projection requires a witness-indexed carrier");
            }
            let value = operation
                .result
                .structural()
                .ok_or("moved package field has no owner result")?;
            (
                Projected::Owner(value.place),
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

/// Read the stored reference of this package or an alias of that same home.
pub(super) fn subject_reference(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    opening: &TerminalPackageOpening,
    arg: &StructuralArgument,
) -> Option<(StructuralTypeId, StructuralAccess)> {
    let (StructuralPathSegment::Referent, stored) = arg.path.split_last()? else {
        return None;
    };
    for (i, segment) in stored.iter().enumerate() {
        let StructuralPathSegment::Field(id) = segment else {
            continue;
        };
        if !same_subject_at(module, machine, operation, opening, arg.place, &stored[..i]) {
            continue;
        }
        let field = fields(module, opening.record)
            .ok()?
            .iter()
            .find(|field| field.identity == *id && !field.relevance.is_erased())?;
        let reference = crate::inline_descriptor_type(
            module.types.structural_types.iter(),
            field.field_type.as_structural()?,
            &stored[i + 1..],
        )?;
        if let StructuralTypeShape::Reference { referent, access } = reference.shape {
            return Some((referent, access));
        }
        return None;
    }
    None
}

/// A returned owner gets the descriptor's opening when the call's complete
/// public result, including the custody at every field path, matches it.
pub(super) fn returned_owner(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    index: u32,
) -> Option<Projected> {
    let call = operation.kind.callable_invocation()?;
    let result = operation.result.structural()?;
    let opened = opening(module, machine.id, index).ok()?;
    let contract = module.execution.callables.contract(call.contract)?;
    (contract.result.matches_operation_result(&operation.result)
        && opened
            .witnesses
            .iter()
            .any(|&witness| fields::nominal_uses(module, result.structural_type, witness)))
    .then_some(Projected::Owner(result.place))
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
        if opened
            .witnesses
            .iter()
            .any(|&witness| fields::nominal_uses(module, formal.structural_type, witness))
        {
            if arg.access != StructuralAccess::Owned
                || formal.access != StructuralAccess::Owned
                || !arg.path.is_empty()
                || machine.structural_place_type(arg.place) != Some(formal.structural_type)
            {
                return Err("package consuming call requires its complete opened owner");
            }
            let (origin, _) = owned::origin(module, machine, projected, arg.place, operation.id)
                .ok_or("package consuming call lacks its owner projection")?;
            if origin != index {
                return Err(
                    "package consuming call uses an owner from a different or stale opening",
                );
            }
            continue;
        }
        let Some(position) = opened
            .witnesses
            .iter()
            .position(|ty| *ty == formal.structural_type)
        else {
            continue;
        };
        let (referent, access) = subject_reference(module, machine, operation, opened, arg)
            .ok_or("package argument comes from another opening")?;
        if referent != schema.witnesses[position]
            || !terminal_psi::structural_access_covers(access, arg.access)
            || arg.access != formal.access
        {
            return Err("package argument differs from its opened parameter");
        }
        checked
            .borrowed_arguments
            .insert((machine.id, operation.id, i), referent);
    }
    Ok(*contract)
}
