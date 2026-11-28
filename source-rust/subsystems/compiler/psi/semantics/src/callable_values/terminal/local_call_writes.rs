//! Keep a caller's other callback fields when every location a lent
//! package may modify can be placed in the caller's live storage.
use super::*;
use crate::descriptor_lifetimes::{descriptor_location, descriptor_path};
use crate::descriptor_results::DescriptorContentsQuery;
use crate::{SharedMutation, packages::CheckedPackageSites, shared_mutation};
use semantic_vocabulary::{CanonicalStructuralPathSegment as FieldPath, ScalarType};
use terminal_psi::{
    Operation, StructuralPathSegment as Step, StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(super) fn refine_package_writes(
    module: &TerminalModule,
    graph: &mut [DescriptorMachine],
    packages: &CheckedPackageSites,
) {
    if module.execution.packages.schemas.is_empty() {
        return;
    }
    // Read every cut against the original conservative graph. One refinement
    // must never become the evidence for another.
    let mut changes = Vec::new();
    let mut references =
        DescriptorAnalysis::with_reference_slots(&module.types.structural_types, &*graph);
    for (ordinal, flow) in graph.iter().enumerate() {
        let Some(machine) = module.machine(flow.id) else {
            continue;
        };
        for (&at, writes) in &flow.payload_invalidations {
            if !writes.iter().any(Option::is_none) || flow.unresolved_contents_writes.contains(&at)
            {
                continue;
            }
            let operation = machine
                .blocks
                .iter()
                .find(|b| b.id == at.0)
                .and_then(|b| b.operations.get(at.1 as usize));
            if let Some(footprint) = operation.and_then(|op| {
                if op.kind.callable_invocation().is_some() {
                    local_opened_callback(module, packages, machine, op, at, &mut references)
                } else {
                    local_package_arguments(module, packages, machine, op, at, &mut references)
                }
            }) {
                changes.push((ordinal, at, footprint));
            }
        }
    }
    for (ordinal, at, footprint) in changes {
        graph[ordinal].payload_invalidations.insert(at, footprint);
    }
}

/// A data-only lender can vary in value or layout without containing
/// another address. A reference, a native address, a view, or unknown
/// storage would need its own account.
fn numeric_storage(module: &TerminalModule, id: StructuralTypeId) -> bool {
    fn plain(scalar: ScalarType) -> bool {
        match scalar {
            ScalarType::Boolean | ScalarType::IeeeFloat(_) => true,
            ScalarType::Integer(integer) => !integer.is_address(),
            ScalarType::Callable(_) => false,
        }
    }
    match module.structural_type(id).map(|ty| &ty.shape) {
        Some(StructuralTypeShape::PrimitiveScalar(scalar)) => plain(*scalar),
        Some(StructuralTypeShape::Record { fields, .. }) => fields.iter().all(|field| {
            field.relevance.is_erased() || field.field_type.scalar_type().is_some_and(plain)
        }),
        _ => false,
    }
}

fn read_only_numeric_storage(module: &TerminalModule, id: StructuralTypeId) -> bool {
    numeric_storage(module, id)
        && shared_mutation(module.types.structural_types.iter(), id, &[]) == SharedMutation::None
}

fn closed_callback(envelope: &terminal_psi::TerminalCallableEnvelope) -> bool {
    envelope.service_reach.is_empty()
        && envelope.invocations.is_empty()
        && !envelope.suspends
        && !envelope.blocks
}

/// Recover the checked lender of every opened input. A mutable borrow of
/// local data-only storage can write through that lender but cannot follow
/// another reference. A shared input also needs a read-only storage policy.
fn local_opened_callback(
    module: &TerminalModule,
    packages: &CheckedPackageSites,
    machine: &TerminalMachine,
    operation: &Operation,
    at: DescriptorPosition,
    references: &mut DescriptorAnalysis<'_>,
) -> Option<Vec<Option<DescriptorContentsQuery>>> {
    let call = operation.kind.callable_invocation()?;
    let contract = packages.invocation_contract(machine.id, operation.id)?;
    if call.contract != contract
        || !closed_callback(&module.execution.callables.contract(contract)?.envelope)
        || !call.arguments.is_empty()
        || !call.claim_transfers.is_empty()
        || call.structural_arguments.is_empty()
    {
        return None;
    }
    let homes = crate::packages::opened_call_homes(
        module,
        packages,
        machine.id,
        operation.id,
        at,
        references,
    );
    if homes.len() != call.structural_arguments.len() {
        return None;
    }
    let mut writes = Vec::new();
    for (actual, (_, home)) in call.structural_arguments.iter().zip(homes) {
        let root = machine.structural_place_type(home.root)?;
        let types = &module.types.structural_types;
        let path = descriptor_path(types, root, &home.path)?;
        let concrete = crate::runtime_structural_path_tip(types.iter(), root, &path)?;
        match actual.access {
            StructuralAccess::SharedBorrow if read_only_numeric_storage(module, concrete.id) => {}
            StructuralAccess::MutableBorrow
                if machine.structural_parameter(home.root).is_none()
                    && path
                        .iter()
                        .all(|step| matches!(step, Step::Field(_) | Step::FixedIndex(_)))
                    && numeric_storage(module, concrete.id) =>
            {
                writes.push(Some(DescriptorContentsQuery::from_location(&home)));
            }
            _ => return None,
        }
    }
    Some(writes)
}

/// Cover every possible backing of the live output view. A fixed array of
/// data-only elements cannot lead the callee to another writable location.
fn local_view_backings(
    module: &TerminalModule,
    machine: &TerminalMachine,
    at: DescriptorPosition,
    references: &mut DescriptorAnalysis<'_>,
    view: &DescriptorLocation,
    view_type: StructuralTypeId,
) -> Option<Vec<DescriptorLocation>> {
    let types = &module.types.structural_types;
    let mut regions = Vec::new();
    for (source, path) in references.local_reference_homes(machine.id, at, view)? {
        if machine.structural_parameter(source).is_some()
            || !path
                .iter()
                .all(|step| matches!(step, Step::Field(_) | Step::FixedIndex(_)))
        {
            return None;
        }
        let root = machine.structural_place_type(source)?;
        let array = crate::runtime_structural_path_tip(types.iter(), root, &path)?;
        let StructuralTypeShape::FixedArray { element, .. } = array.shape else {
            return None;
        };
        if !numeric_storage(module, element)
            || crate::collection_backing(types.iter(), view_type, element).is_none()
        {
            return None;
        }
        regions.push(descriptor_location(types, source, root, &path, None)?);
    }
    Some(regions)
}

fn local_package_arguments(
    module: &TerminalModule,
    packages: &CheckedPackageSites,
    machine: &TerminalMachine,
    operation: &Operation,
    at: DescriptorPosition,
    references: &mut DescriptorAnalysis<'_>,
) -> Option<Vec<Option<DescriptorContentsQuery>>> {
    let call = terminal_psi::direct_call_parts(&operation.kind)?;
    let envelope = module.execution.callables.machine_envelope(call.callee)?;
    if !envelope.service_reach.is_empty()
        || envelope
            .invocations
            .iter()
            .any(|permission| permission.service().is_some())
        || envelope.suspends
        || envelope.blocks
        || !call.arguments.is_empty()
        || call.structural_arguments.is_empty()
    {
        return None;
    }
    let types = &module.types.structural_types;
    let mut regions = Vec::new();
    let mut borrows_package = false;
    for actual in call.structural_arguments {
        if actual.access != StructuralAccess::MutableBorrow {
            return None;
        }
        let root = machine.structural_place_type(actual.place)?;
        let record = crate::runtime_structural_path_tip(types.iter(), root, &actual.path)?;
        let local = descriptor_location(types, actual.place, root, &actual.path, None)?;
        let homes = references.storage_origins(machine.id, at, &local)?;
        if homes.as_slice() != std::slice::from_ref(&local) {
            return None;
        }
        if record.shape.is_view_descriptor() {
            if !actual.path.is_empty() || machine.structural_parameter(actual.place).is_some() {
                return None;
            }
            regions.extend(local_view_backings(
                module, machine, at, references, &local, record.id,
            )?);
            regions.push(local);
            continue;
        }
        if !module
            .execution
            .packages
            .schemas
            .iter()
            .any(|schema| schema.record == record.id)
        {
            return None;
        }
        borrows_package = true;
        regions.push(local);
        let StructuralTypeShape::Record { fields, .. } = &record.shape else {
            return None;
        };
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            if let Some(scalar) = field.field_type.scalar_type() {
                match scalar {
                    ScalarType::Integer(integer) if integer.is_address() => return None,
                    ScalarType::Callable(id) => {
                        let callback = &module.execution.callables.contract(id)?.envelope;
                        if !closed_callback(callback) {
                            return None;
                        }
                    }
                    _ => {}
                }
                continue;
            }
            if shared_mutation(types.iter(), record.id, &[FieldPath::Field(field.id)])
                != SharedMutation::Unbounded
            {
                // An unexamined structural field can contain a raw address.
                if !numeric_storage(module, field.field_type.as_structural()?) {
                    return None;
                }
                continue;
            }
            let mut path = actual.path.clone();
            path.push(Step::Field(field.identity));
            for (source, path) in crate::packages::local_mutable_field_homes(
                module,
                machine,
                at,
                references,
                actual.place,
                &path,
                Some(packages),
            )? {
                // A runtime index needs the selector's original evaluation point.
                if path.iter().any(|step| step.runtime_index().is_some()) {
                    return None;
                }
                let root = machine.structural_place_type(source)?;
                let concrete = crate::runtime_structural_path_tip(types.iter(), root, &path)?;
                if !numeric_storage(module, concrete.id) {
                    return None;
                }
                regions.push(descriptor_location(types, source, root, &path, None)?);
            }
        }
    }
    if !borrows_package {
        return None;
    }
    regions.sort_unstable();
    regions.dedup();
    Some(
        regions
            .iter()
            .map(|region| Some(DescriptorContentsQuery::from_location(region)))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralRecordStorage, StructuralTypeDeclaration, TerminalIdentityIndex,
    };

    #[test]
    fn a_shared_callback_can_mutate_an_atomic_numeric_field() {
        let mut module = TerminalModule::for_entry(semantic_vocabulary::MachineId::new(1).unwrap());
        let id = StructuralTypeId::new(1).unwrap();
        let record = |ty| StructuralTypeDeclaration {
            id,
            identity: TerminalIdentityIndex(1),
            declaration_identity: Default::default(),
            requires: vec![],
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            multiplicity: StructuralMultiplicity::Unrestricted,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(2),
                    declaration_identity: Default::default(),
                    inline_carrier: None,
                    relevance: language_core::BindingRelevance::Relevant,
                    storage: None,
                    field_type: ty,
                }],
                storage: StructuralRecordStorage::Packed,
            },
        };
        module
            .types
            .structural_types
            .push(record(StructuralFieldType::Scalar(ScalarType::Boolean)));
        assert!(read_only_numeric_storage(&module, id));
        module.types.structural_types[0] = record(StructuralFieldType::Atomic(ScalarType::Boolean));
        assert!(numeric_storage(&module, id));
        assert!(!read_only_numeric_storage(&module, id));
    }
}
