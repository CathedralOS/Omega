//! Whole byte observations resolve the source's canonical arrival or producer.
//! A state edge changes the name of incoming storage, not its permissions:
//! owner length is metadata, content observation requires readable access,
//! and replacement still requires an immutable payload source. Control flow
//! independently checks availability and currentness.

use crate::validation::{
    IntegerSign, IntegerType, ModuleError, PlaceId, ScalarType, StructuralAccess,
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
    if let Some(parameter) = terminal_semantics::arrival_parameter(machine, source)
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
    if terminal_semantics::arrival_parameter(machine, source)
        .is_some_and(|parameter| parameter.access == StructuralAccess::MutableBorrow)
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

/// The home of a declared descriptor local: a claim-free
/// `EstablishStructuralLocal` result of standalone byte-view type. Whole
/// descriptor stores replace it; reads observe it as a shared view. Its
/// qualifications, if any, are primitive byte predicates of that carrier:
/// the declared invariant every store proves and every read may rely on.
pub(in crate::validation) fn descriptor_home<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let home = operation.result.structural()?;
    (matches!(
        operation.kind,
        terminal_psi::OperationKind::EstablishStructuralLocal {}
    ) && home.place == place
        && descriptor_home_predicates(module, home).is_some())
    .then_some(home)
}

/// The byte predicates a descriptor home declares, when `home` has the shape
/// one may: an unrestricted or affine, claim-free result of standalone shared
/// byte-view type whose every qualification is a primitive byte predicate of
/// that carrier.
pub(in crate::validation) fn descriptor_home_predicates(
    module: &TerminalModule,
    home: &terminal_psi::StructuralOperationResult,
) -> Option<terminal_psi::ByteSequencePredicateSet> {
    (matches!(
        home.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) && home.projected_qualifications.is_empty()
        && home.qualification_establishments.is_empty()
        && home.claims.is_empty()
        && module.structural_types.iter().any(|row| {
            row.id == home.structural_type
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow),
                        }
                    )
                )
        }))
    .then(|| {
        terminal_semantics::byte_qualification_predicates_in(
            module.structural_domains.iter(),
            home.structural_type,
            &home.qualifications,
        )
    })
    .flatten()
}

/// Whole immutable views retain one exact source for every observation. A
/// declared descriptor home is such a view only where a whole descriptor
/// store reaches the observation on every path: an unestablished local
/// cannot be read.
pub(in crate::validation) fn validate_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    invalid: impl Fn() -> ModuleError,
) -> Result<(), ModuleError> {
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)
        .ok_or_else(&invalid)?;
    let structural_type = match place.kind {
        StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. } => {
            let parameter =
                terminal_semantics::arrival_parameter(machine, source).ok_or_else(&invalid)?;
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
                || (owned
                    && matches!(
                        parameter.access,
                        StructuralAccess::Owned | StructuralAccess::MutableBorrow
                    )))
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || terminal_semantics::byte_parameter_predicates(module, parameter).is_none()
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
            if descriptor_home(module, machine, source).is_some() {
                if !crate::validation::structural::byte_sequence_fields::descriptor_home_is_established(
                    machine,
                    operation.id,
                    source,
                ) {
                    return Err(ModuleError::ByteSequenceViewNotEstablished {
                        operation: operation.id,
                        place: source,
                    });
                }
                // `descriptor_home` already authenticated its view carrier.
                return Ok(());
            }
            crate::validation::byte_sequence::subslice::borrowed_result_in_module(
                module, machine, source,
            )
            .or_else(|| super::concatenation::owned_result(machine, source))
            .or_else(|| terminal_semantics::byte_owned_call_result(module, machine, source))
            .or_else(|| terminal_semantics::byte_view_call_result(module, machine, source))
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
                || terminal_semantics::byte_owned_call_result(module, machine, source).is_some()
                || terminal_semantics::arrival_parameter(machine, source).is_some())
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
/// An owned bounded formal supplies its own value, not an attenuated loan;
/// availability and overlapping access remain checked by the custody pass.
pub(in crate::validation) fn validate_replacement_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    invalid: impl Fn() -> ModuleError,
) -> Result<(), ModuleError> {
    if terminal_semantics::arrival_parameter(machine, source).is_some_and(|parameter| {
        parameter.access != StructuralAccess::SharedBorrow
            && !(parameter.access == StructuralAccess::Owned
                && module.structural_types.iter().any(|row| {
                    row.id == parameter.structural_type
                        && matches!(
                            row.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                }))
    }) {
        return Err(invalid());
    }
    validate_source(module, machine, operation, source, invalid)
}
