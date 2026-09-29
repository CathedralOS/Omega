//! Exact whole immutable byte-view observation; control flow owns availability.

use crate::validation::{
    IntegerSign, IntegerType, ModuleError, OperationKind, PlaceId, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralPlaceKind, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};
pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return Err(ModuleError::ByteSequenceLengthRequiresU64Result(
            operation.id,
        ));
    }
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        && matches!(
            parameter.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        )
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && terminal_semantics::byte_parameter_predicates(module, parameter).is_some()
        && module.structural_types.iter().any(|row| {
            row.id == parameter.structural_type
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
        && !machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == source)
        && !machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source)
    {
        // Owner length is metadata; observing it does not read any byte
        // through a write-only loan.
        return Ok(());
    }
    if machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|parameter| {
            parameter.place == source && parameter.access == StructuralAccess::MutableBorrow
        })
    {
        return crate::validation::byte_sequence::write::validate_destination(
            module, machine, operation, source,
        );
    }
    validate_source(module, machine, operation, source, || {
        ModuleError::InvalidByteSequenceLengthSource {
            operation: operation.id,
            source,
        }
    })
}

/// Whole immutable views retain one exact source for every observation.
pub(in crate::validation) fn validate_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    _operation: &terminal_psi::Operation,
    source: PlaceId,
    invalid: impl Fn() -> ModuleError,
) -> Result<(), ModuleError> {
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)
        .ok_or_else(&invalid)?;
    let structural_type = match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let parameter = machine
                .structural_parameters
                .iter()
                .find(|parameter| {
                    parameter.place == source
                        && parameter.position == position
                        && parameter.is_self == is_self
                })
                .ok_or_else(&invalid)?;
            let owned = module.structural_types.iter().any(|row| {
                row.id == parameter.structural_type
                    && matches!(
                        row.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            });
            if !(parameter.access == StructuralAccess::SharedBorrow
                || (owned && parameter.access == StructuralAccess::MutableBorrow))
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || terminal_semantics::byte_parameter_predicates(module, parameter).is_none()
            {
                return Err(invalid());
            }
            parameter.structural_type
        }
        StructuralPlaceKind::BlockParameter { .. } => {
            let parameter =
                crate::validation::block_views::parameter(machine, source).ok_or_else(&invalid)?;
            if parameter.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            parameter.structural_type
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } => {
            // Foundation validation binds exactly one establishment to this
            // declaration. The shared view-use pass checks its dominance.
            structural_type
        }
        StructuralPlaceKind::OperationResult { .. } => {
            crate::validation::byte_sequence::subslice::borrowed_result(machine, source)
                .or_else(|| super::concatenation::owned_result(machine, source))
                .or_else(|| borrowed_call_result(machine, source))
                .ok_or_else(&invalid)?
                .structural_type
        }
        _ => return Err(invalid()),
    };
    if !module.structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && (matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                )
            ) || ((super::concatenation::owned_result(machine, source).is_some()
                || machine
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == source))
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )))
    }) {
        return Err(invalid());
    }
    Ok(())
}

/// Replacement consumes an immutable payload. Readable owner metadata and
/// concatenation inputs do not authorize a mutable owner as a copy source.
pub(in crate::validation) fn validate_replacement_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    invalid: impl Fn() -> ModuleError,
) -> Result<(), ModuleError> {
    if machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|parameter| {
            parameter.place == source && parameter.access != StructuralAccess::SharedBorrow
        })
    {
        return Err(invalid());
    }
    validate_source(module, machine, operation, source, invalid)
}

/// A call result can also publish a whole immutable byte view: when a callee
/// returns an Unrestricted borrowed view, its operation-result place observes
/// the same stable source a subslice or concatenation would. The roster and
/// claim filters mirror `borrowed_result`; the producer is a call instead.
fn borrowed_call_result<'a>(
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if operation.id != producer
                || !matches!(
                    operation.kind,
                    OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                        | OperationKind::BoundaryCall { .. }
                )
            {
                return None;
            }
            operation.result.structural().filter(|result| {
                result.place == place
                    && result.structural_type == structural_type
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
        })
}
