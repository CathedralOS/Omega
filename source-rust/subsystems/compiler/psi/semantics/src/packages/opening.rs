//! Verify that the evaluated descriptor and its operands still describe the same subject.
use super::*;

pub(super) fn projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    index: u32,
    field: StructuralFieldId,
    checked: &mut CheckedPackageSites,
) -> Result<ValueId, &'static str> {
    let opened = opening(module, machine.id, index)?;
    let schema = schema(module, opened.record)?;
    let declaration = fields(module, opened.record)?
        .iter()
        .find(|decl| decl.id == field && !decl.relevance.is_erased())
        .ok_or("package projection names an absent field")?;
    let OperationKind::PrimitiveScalarRead { source, path } = &operation.kind else {
        return Err("package descriptor projection requires a scalar field read");
    };
    let Some((StructuralPathSegment::Field(identity), parent)) = path.split_last() else {
        return Err("package projection does not read a field");
    };
    if *source != opened.subject.place
        || !crate::structural_paths_select_same_location(parent, &opened.subject.path)
        || *identity != declaration.identity
    {
        return Err("package projection reads a field of another subject");
    }
    let value = operation
        .result
        .scalar()
        .ok_or("package projection lacks a scalar result")?;
    let actual = StructuralFieldType::Scalar(value.scalar_type);
    let substitution = Substitution {
        public: &schema.witnesses,
        actual: &opened.witnesses,
    };
    if !matches!(value.scalar_type, ScalarType::Callable(_))
        || !substitution.admits(module, &declaration.field_type, &actual)
    {
        return Err("package projection publishes the wrong opened callable");
    }
    checked
        .fields
        .insert((machine.id, operation.id, field), actual);
    checked
        .projected_fields
        .insert((machine.id, operation.id), field);
    let ScalarType::Callable(contract) = value.scalar_type else {
        return Err("projected value is not callable");
    };
    checked
        .descriptors
        .insert((machine.id, operation.id), (index, contract));
    Ok(value.id)
}

fn subject_field<'a>(
    module: &'a TerminalModule,
    opening: &TerminalPackageOpening,
    arg: &StructuralArgument,
) -> Option<&'a StructuralFieldDeclaration> {
    if arg.place != opening.subject.place || arg.path.len() != opening.subject.path.len() + 2 {
        return None;
    }
    let (parent, tail) = arg.path.split_at(opening.subject.path.len());
    let [
        StructuralPathSegment::Field(field),
        StructuralPathSegment::Referent,
    ] = tail
    else {
        return None;
    };
    if !crate::structural_paths_select_same_location(parent, &opening.subject.path) {
        return None;
    }
    fields(module, opening.record)
        .ok()?
        .iter()
        .find(|decl| decl.identity == *field && !decl.relevance.is_erased())
}

// A saved descriptor may be reused across blocks or intervening writes when
// every incoming path retains this package's storage. Reuse the descriptor
// replay that already accounts for field overlap, call write frames and loans.
pub(super) fn same_storage_version(
    module: &TerminalModule,
    machine: &TerminalMachine,
    subject: &StructuralArgument,
    projection: OperationId,
    call: OperationId,
) -> bool {
    use crate::descriptors::descriptor_lifetimes::descriptor_location;
    use crate::descriptors::descriptor_results::{
        DescriptorPayloadTarget, require_unchanged_observed_storage,
        terminal_borrowed_storage_graph_from,
    };
    let position = |id| {
        machine.blocks.iter().find_map(|block| {
            let index = block.operations.iter().position(|op| op.id == id)?;
            Some((block.id, u32::try_from(index).ok()?))
        })
    };
    let (Some(read), Some(use_site)) = (position(projection), position(call)) else {
        return false;
    };
    if read.0 == use_site.0 && read.1.checked_add(1) == Some(use_site.1) {
        return true;
    }
    let Some(root) = machine.structural_place_type(subject.place) else {
        return false;
    };
    let Some(location) = descriptor_location(
        &module.types.structural_types,
        subject.place,
        root,
        &subject.path,
        None,
    ) else {
        return false;
    };
    let Ok(graph) = terminal_borrowed_storage_graph_from(module, [machine.id], &[]) else {
        return false;
    };
    require_unchanged_observed_storage(
        &module.types.structural_types,
        &graph,
        machine.id,
        read,
        use_site,
        &DescriptorPayloadTarget::current(location),
    )
    .is_ok()
}

pub(super) fn invocation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    index: u32,
    projected: &BTreeMap<(MachineId, ValueId), (u32, OperationId)>,
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
    let Some(&(origin, producer)) = projected.get(&(machine.id, *callee)) else {
        return Err("package invocation has no projected descriptor");
    };
    if origin != index
        || !same_storage_version(module, machine, &opened.subject, producer, operation.id)
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
        let field = subject_field(module, opened, arg)
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
