//! Trace exposed storage and scalar formals through the value that supplies a native address.
use semantic_vocabulary::ValueId;
use terminal_psi::{OperationKind, StructuralArgument, TerminalMachine};

pub struct NativeAddressOrigin<S> {
    /// Storage whose observed address contributed to this scalar, including at joins.
    pub sources: Vec<S>,
    /// Scalar formals whose actual addresses must be resolved at a direct call.
    pub parameters: Vec<ValueId>,
    /// Every route supplies one unmodified storage address or scalar formal.
    pub exact: bool,
}

/// The scalar definition visible at this stage of the compiler.
pub enum AddressDefinition<S> {
    Storage(S),
    Parameter,
    Forward(ValueId),
    Adjust(ValueId, ValueId),
    Incoming(Vec<Option<ValueId>>),
    Opaque,
}

pub fn trace_address<S: PartialEq>(
    value: ValueId,
    mut definition: impl FnMut(ValueId) -> AddressDefinition<S>,
) -> NativeAddressOrigin<S> {
    let mut work = vec![value];
    let mut visited = std::collections::HashSet::new();
    let mut parameters = Vec::new();
    let mut sources = Vec::new();
    let mut exact = true;
    while let Some(value) = work.pop() {
        if !visited.insert(value) {
            continue;
        }
        match definition(value) {
            AddressDefinition::Storage(source) => {
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
            AddressDefinition::Parameter => parameters.push(value),
            AddressDefinition::Forward(operand) => work.push(operand),
            AddressDefinition::Adjust(left, right) => {
                exact = false;
                work.extend([left, right]);
            }
            AddressDefinition::Incoming(incoming) => {
                exact &= !incoming.is_empty() && incoming.iter().all(Option::is_some);
                work.extend(incoming.into_iter().flatten());
            }
            AddressDefinition::Opaque => exact = false,
        }
    }
    parameters.sort_unstable();
    NativeAddressOrigin {
        exact: exact && sources.len() + parameters.len() == 1,
        sources,
        parameters,
    }
}

pub fn address_origin(
    machine: &TerminalMachine,
    value: ValueId,
) -> NativeAddressOrigin<&StructuralArgument> {
    trace_address(value, |value| {
        if let Some(operation) = machine.operations().find(|operation| {
            operation
                .result
                .as_scalar()
                .is_some_and(|result| result.id == value)
        }) {
            return match &operation.kind {
                OperationKind::StorageAddress { source } => AddressDefinition::Storage(source),
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
    })
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
