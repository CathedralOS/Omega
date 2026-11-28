//! Trace exposed storage and scalar formals through the value that supplies a native address.
mod call_inputs;
mod returned_addresses;
pub use call_inputs::receives_expired_address;
mod returned_storage;
pub use super::borrowed_storage::Position;
pub use returned_addresses::AddressReturnBody;
pub use returned_storage::{ReturnedStorage, StorageHome};
use semantic_vocabulary::{MachineId, ValueId};
use terminal_psi::{OperationKind, StructuralArgument, TerminalMachine};

pub(crate) fn tracks_native_addresses(module: &terminal_psi::TerminalModule) -> bool {
    module.machines.iter().any(|body| {
        body.operations()
            .any(|operation| matches!(operation.kind, OperationKind::BorrowAt { .. }))
    })
}

pub struct NativeAddressOrigin<S> {
    /// Storage whose observed address contributed to this scalar, including at joins.
    pub sources: Vec<S>,
    /// Each storage observation retains the cut at which its address was read.
    pub observations: Vec<((semantic_vocabulary::BlockId, u32), usize)>,
    /// Scalar formals whose actual addresses must be resolved at a direct call.
    pub parameters: Vec<ValueId>,
    /// An incoming route can supply an address with no known storage or formal.
    pub opaque: bool,
    /// Some contributing storage belonged to an invocation that has returned.
    pub expired: bool,
    /// Every route supplies one unmodified storage address or scalar formal.
    pub exact: bool,
}

/// The scalar definition visible at this stage of the compiler.
impl<S> NativeAddressOrigin<S> {
    pub fn only_expired_storage(&self) -> bool {
        self.expired && !self.opaque && self.sources.is_empty() && self.parameters.is_empty()
    }

    pub fn observed_sources(
        &self,
    ) -> impl Iterator<Item = ((semantic_vocabulary::BlockId, u32), &S)> {
        self.observations
            .iter()
            .map(|(at, index)| (*at, &self.sources[*index]))
    }
}

#[derive(Clone)]
pub enum AddressDefinition<S> {
    Storage(S, (semantic_vocabulary::BlockId, u32)),
    Parameter,
    Forward(ValueId),
    Call(AddressCall<S>),
    Adjust(ValueId, ValueId),
    Incoming(Vec<Option<ValueId>>),
    Opaque,
}

#[derive(Clone)]
pub struct AddressCall<S> {
    pub callee: MachineId,
    pub scalars: Vec<ValueId>,
    pub storage: Vec<S>,
    pub at: Position,
}

pub fn trace_address(
    value: ValueId,
    definition: impl FnMut(ValueId) -> AddressDefinition<StructuralArgument>,
    mut returned: impl FnMut(MachineId) -> Option<AddressReturnBody>,
) -> NativeAddressOrigin<StructuralArgument> {
    let mut calls = returned_addresses::ReturnSources::default();
    trace_local(value, definition, |call| calls.actuals(call, &mut returned))
}

fn trace_local<S: PartialEq>(
    value: ValueId,
    mut definition: impl FnMut(ValueId) -> AddressDefinition<S>,
    mut returned: impl FnMut(&AddressCall<S>) -> returned_addresses::ReturnedActuals<S>,
) -> NativeAddressOrigin<S> {
    let mut work = vec![value];
    let mut visited = std::collections::HashSet::new();
    let mut parameters = Vec::new();
    let mut observations = Vec::new();
    let mut sources = Vec::new();
    let mut exact = true;
    let mut opaque = Vec::new();
    let mut expired = false;
    let mut observe = |source: S, at| {
        let index = sources
            .iter()
            .position(|known| known == &source)
            .unwrap_or_else(|| {
                sources.push(source);
                sources.len() - 1
            });
        observations.push((at, index));
    };
    // A merged value may be opaque on any incoming edge. For arithmetic,
    // both operands must lack a backing to produce an independent opaque
    // address; a plain integer offset does not create a second address.
    let mut dependents = Vec::new();
    while let Some(value) = work.pop() {
        if !visited.insert(value) {
            continue;
        }
        match definition(value) {
            AddressDefinition::Storage(source, at) => observe(source, at),
            AddressDefinition::Parameter => parameters.push(value),
            AddressDefinition::Call(call) => {
                let inputs = returned(&call);
                exact &= inputs.exact;
                expired |= inputs.expired;
                if inputs.opaque {
                    opaque.push(value);
                }
                for source in inputs.storage {
                    observe(source, call.at);
                }
                for input in inputs.values {
                    dependents.push((input, value, None));
                    work.push(input);
                }
            }
            AddressDefinition::Forward(operand) => {
                dependents.push((operand, value, None));
                work.push(operand);
            }
            AddressDefinition::Adjust(left, right) => {
                exact = false;
                dependents.push((left, value, Some(right)));
                dependents.push((right, value, Some(left)));
                work.extend([left, right]);
            }
            AddressDefinition::Incoming(incoming) => {
                if incoming.is_empty() || incoming.iter().any(Option::is_none) {
                    exact = false;
                    opaque.push(value);
                }
                for operand in incoming.into_iter().flatten() {
                    dependents.push((operand, value, None));
                    work.push(operand);
                }
            }
            AddressDefinition::Opaque => {
                exact = false;
                opaque.push(value);
            }
        }
    }
    let mut independent = std::collections::HashSet::new();
    dependents.sort_unstable();
    while let Some(operand) = opaque.pop() {
        if !independent.insert(operand) {
            continue;
        }
        let first = dependents.partition_point(|(source, _, _)| *source < operand);
        for &(_, result, other) in dependents[first..]
            .iter()
            .take_while(|(source, _, _)| *source == operand)
        {
            if other.is_none_or(|other| independent.contains(&other)) {
                opaque.push(result);
            }
        }
    }
    parameters.sort_unstable();
    NativeAddressOrigin {
        expired,
        opaque: independent.contains(&value)
            || (observations.is_empty() && parameters.is_empty() && !expired),
        exact: exact && observations.len() + parameters.len() == 1,
        observations,
        sources,
        parameters,
    }
}

pub fn address_origin(
    module: &terminal_psi::TerminalModule,
    machine: &TerminalMachine,
    value: ValueId,
) -> NativeAddressOrigin<StructuralArgument> {
    trace_address(
        value,
        |v| terminal_definition(machine, v),
        |callee| {
            let body = module.machine(callee)?;
            let results = body
                .blocks
                .iter()
                .filter_map(|block| match &block.terminator {
                    terminal_psi::Terminator::Return { value, .. } => Some(*value),
                    _ => None,
                })
                .collect();
            Some(AddressReturnBody::new(
                body.parameters.iter().map(|p| p.id).collect(),
                results,
                |v| terminal_definition(body, v),
                |source, at| {
                    ReturnedStorage::classify(
                        &module.types.structural_types,
                        source,
                        at,
                        |place, cut| terminal_home(body, place, cut),
                    )
                },
            ))
        },
    )
}

fn terminal_home(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    at: Position,
) -> StorageHome<'_> {
    if let Some((ordinal, formal)) = machine
        .structural_parameters
        .iter()
        .enumerate()
        .find(|(_, p)| p.place == place)
    {
        return StorageHome::Formal(ordinal, formal.access, formal.structural_type);
    }
    let Some((producer, ty)) = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)
        .and_then(|row| row.kind.as_operation_result())
    else {
        return StorageHome::Other;
    };
    let Some(operation) = machine.operation(producer) else {
        return StorageHome::Other;
    };
    match &operation.kind {
        OperationKind::EstablishPrimitiveLocal { .. }
        | OperationKind::EstablishStructuralLocal { .. }
        | OperationKind::EstablishZeroedStructural { .. }
        | OperationKind::EstablishRecord { .. }
        | OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishStructuralArray { .. } => StorageHome::Local(ty),
        OperationKind::EstablishReference {
            source,
            storage: None,
        } => {
            let Some(index) = at.1.checked_sub(1) else {
                return StorageHome::Other;
            };
            if machine
                .block(&at.0)
                .and_then(|block| block.operations.get(index as usize))
                .is_some_and(|previous| previous.id == producer)
            {
                StorageHome::FreshReference(source, (at.0, index))
            } else {
                StorageHome::Other
            }
        }
        _ => StorageHome::Other,
    }
}

fn terminal_definition(
    machine: &TerminalMachine,
    value: ValueId,
) -> AddressDefinition<StructuralArgument> {
    if let Some((operation, at)) = machine.blocks.iter().find_map(|block| {
        block
            .operations
            .iter()
            .enumerate()
            .find_map(|(index, operation)| {
                operation
                    .result
                    .as_scalar()
                    .filter(|result| result.id == value)?;
                Some((operation, (block.id, u32::try_from(index).ok()?)))
            })
    }) {
        if let Some(call) = operation.module_invocation() {
            return AddressDefinition::Call(AddressCall {
                callee: call.callee,
                scalars: call.arguments.to_vec(),
                storage: call.structural_arguments.to_vec(),
                at,
            });
        }
        return match &operation.kind {
            OperationKind::StorageAddress { source } => {
                AddressDefinition::Storage(source.clone(), at)
            }
            OperationKind::ScalarRepresentationView { operand, .. }
            | OperationKind::IntegerWiden { operand }
            | OperationKind::IntegerExactCast { operand, .. } => {
                AddressDefinition::Forward(*operand)
            }
            OperationKind::ExactIntegerAdd { left, right, .. }
            | OperationKind::WrappingIntegerAdd { left, right }
            | OperationKind::SaturatingIntegerAdd { left, right }
            | OperationKind::ExactIntegerSubtract { left, right, .. }
            | OperationKind::WrappingIntegerSubtract { left, right }
            | OperationKind::SaturatingIntegerSubtract { left, right } => {
                AddressDefinition::Adjust(*left, *right)
            }
            _ => AddressDefinition::Opaque,
        };
    }
    if machine.parameters.iter().any(|formal| formal.id == value) {
        return AddressDefinition::Parameter;
    }
    let Some((block, position)) = machine.blocks.iter().find_map(|block| {
        block
            .parameters
            .iter()
            .position(|parameter| parameter.id == value)
            .map(|position| (block.id, position))
    }) else {
        return AddressDefinition::Opaque;
    };
    AddressDefinition::Incoming(
        machine
            .blocks
            .iter()
            .flat_map(|predecessor| predecessor.terminator.scalar_arguments_to(block))
            .map(|arguments| arguments.get(position).copied())
            .collect(),
    )
}

/// The typed operation determines its authority even when the source omitted a
/// reach annotation. Product and dependency admission use the installed core's
/// identity for each listed atom.
pub fn required_authority(
    module: &terminal_psi::TerminalModule,
    operation: &terminal_psi::Operation,
) -> Result<
    Option<(
        &'static str,
        &'static [semantic_vocabulary::dependency_reach::UnsafeAtom],
    )>,
    &'static str,
> {
    use semantic_vocabulary::dependency_reach::UnsafeAtom::*;
    use terminal_psi::{StructuralAccess, StructuralTypeShape};
    match operation.kind {
        OperationKind::UpgradeMutability { .. } => {
            Ok(Some(("upgrade_mutability", &[BorrowMutabilityUpgrade])))
        }
        OperationKind::BorrowAt { .. } => {
            let result = operation
                .result
                .as_structural()
                .and_then(|result| module.structural_type(result.structural_type))
                .ok_or("native address borrow has no reference result")?;
            match result.shape {
                StructuralTypeShape::Reference {
                    access: StructuralAccess::SharedBorrow,
                    ..
                } => Ok(Some(("borrow_at", &[RawMemoryRead]))),
                StructuralTypeShape::Reference {
                    access: StructuralAccess::MutableBorrow,
                    ..
                } => Ok(Some((
                    "borrow_mutable_at",
                    &[RawMemoryRead, RawMemoryWrite],
                ))),
                _ => Err("native address borrow has invalid result access"),
            }
        }
        _ => Ok(None),
    }
}

/// The concrete service row of the operation, including the reporting parent.
/// The module supplies a complete bundled declaration for every required atom.
pub fn required_services(
    module: &terminal_psi::TerminalModule,
    operation: &terminal_psi::Operation,
) -> Result<Vec<semantic_vocabulary::ServiceId>, String> {
    let mut services = Vec::new();
    if let Some((_, atoms)) = required_authority(module, operation)? {
        for &atom in atoms {
            let source = module.core_unsafe_service(atom)?;
            services.push(source.id);
            services.extend_from_slice(&source.parents);
        }
        services.sort_unstable();
        services.dedup();
    }
    Ok(services)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_an_opaque_incoming_address_from_an_integer_offset() {
        let value = |id| ValueId::new(id).unwrap();
        let address = value(1);
        let unknown = value(2);
        let adjusted = value(3);
        let joined = value(4);
        let cycle = value(5);
        let trace = |root| {
            trace_address(
                root,
                |id| match id {
                    id if id == address => AddressDefinition::Parameter,
                    id if id == adjusted => AddressDefinition::Adjust(address, unknown),
                    id if id == joined => {
                        AddressDefinition::Incoming(vec![Some(adjusted), Some(unknown)])
                    }
                    id if id == cycle => {
                        AddressDefinition::Incoming(vec![Some(cycle), Some(joined)])
                    }
                    _ => AddressDefinition::Opaque,
                },
                |_| None,
            )
        };
        assert!(!trace(address).opaque);
        assert!(!trace(adjusted).opaque);
        assert!(!trace(adjusted).exact);
        assert!(trace(unknown).opaque);
        assert!(trace(joined).opaque);
        assert!(trace(cycle).opaque);
        assert_eq!(trace(cycle).parameters, vec![address]);
    }
}
