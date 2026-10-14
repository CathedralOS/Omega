//! Operations that establish a value: scalar arrays and cases, byte-sequence
//! literals, trivial affine locals, records and primitive locals.

use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralFieldType, StructuralPlaceDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalModule, find_structural_case,
};

pub(super) fn validate_establish_scalar_array(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishScalarArray { elements } = &operation.kind else {
        unreachable!("dispatched validate_establish_scalar_array")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("scalar array establishment has no structural result");
    };
    if !result.is_unrestricted_plain()
        || !matches!(
            machine.structural_place(result.place),
            Some(StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        )
        || terminal_semantics::scalar_array_leaf_shape(
            module.structural_types.iter(),
            result.structural_type,
        )
        .is_none_or(|(_, count)| u64::try_from(elements.len()).ok() != Some(count))
    {
        return malformed("scalar array establishment has an invalid result shape");
    }
    // This constructor carries no per-leaf receiving obligations. Empty
    // arrays instantiate no scalar; nonempty restricted arrays need a
    // constructor that retains each initialized value's predicate authority.
    if terminal_semantics::scalar_array_leaf_declaration(
        module.structural_types.iter(),
        result.structural_type,
    )
    .is_some_and(|(leaf, count)| count != 0 && leaf.scalar_restriction.is_some())
    {
        return malformed("scalar array initializer lacks receiving restriction obligations");
    }
    Ok(())
}

pub(super) fn validate_establish_scalar_case(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishScalarCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_establish_scalar_case")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("scalar case establishment has no structural result");
    };
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return malformed("scalar case establishment has an invalid result surface");
    }
    if !matches!(
        machine.structural_place(result.place),
        Some(StructuralPlaceDeclaration {
            kind: StructuralPlaceKind::OperationResult { producer, structural_type },
            ..
        }) if *producer == operation.id && *structural_type == result.structural_type
    ) {
        return malformed("scalar case establishment has no matching result place");
    }
    let Some(declaration) = module.structural_type(result.structural_type) else {
        return malformed("scalar case establishment has an unknown structural type");
    };
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return malformed("scalar case establishment requires a sum type");
    };
    let Some(selected) = find_structural_case(cases, *result_case) else {
        return malformed("scalar case establishment requires an exact member");
    };
    if selected.fields.len() != fields.len() {
        return malformed("scalar case establishment requires every selected field");
    }
    for (declaration, field) in selected.fields.iter().zip(fields) {
        if declaration.id != field.field
            || declaration.relevance.is_erased()
            || declaration.field_type.scalar_type().is_none()
            || terminal_semantics::scalar_field_requires_evidence(&declaration.field_type)
                != field.range_obligation.is_some()
        {
            return malformed("scalar case establishment has an invalid field binding");
        }
    }
    // Full module validation independently checks exact operand types,
    // dominance and declaration-derived obligations before acceptance.
    Ok(())
}

pub(super) fn validate_establish_structural_case(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishStructuralCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_establish_structural_case")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("structural case establishment has no structural result");
    };
    if !result.qualifications.is_empty() {
        return malformed("structural case establishment has an invalid result surface");
    }
    validate_constructor_claims(module, result)?;
    if !matches!(
        machine.structural_place(result.place),
        Some(StructuralPlaceDeclaration {
            kind: StructuralPlaceKind::OperationResult { producer, structural_type },
            ..
        }) if *producer == operation.id && *structural_type == result.structural_type
    ) {
        return malformed("structural case establishment has no matching result place");
    }
    let Some(declaration) = module.structural_type(result.structural_type) else {
        return malformed("structural case establishment has an unknown structural type");
    };
    // A mixed case binds its runtime common fields in declaration order, then
    // the selected payload; erased common members take no roster position,
    // so an all-erased union binds the payload alone as a closed sum does.
    // Common and payload members share the same exact initializer validation;
    // nested common storage has no selected-case segment in its logical path.
    let (common, cases) = match &declaration.shape {
        StructuralTypeShape::Sum { cases } => (&[][..], cases),
        StructuralTypeShape::Mixed { fields, cases }
            if fields.iter().all(|field| {
                matches!(field.field_type, StructuralFieldType::Erased { .. })
                    || !field.relevance.is_erased()
            }) =>
        {
            (fields.as_slice(), cases)
        }
        _ => return malformed("structural case establishment requires a sum type"),
    };
    let Some(selected) = find_structural_case(cases, *result_case) else {
        return malformed("structural case establishment requires an exact member");
    };
    // Erased payload members take no roster position either.
    let roster = common
        .iter()
        .chain(&selected.fields)
        .filter(|field| !matches!(field.field_type, StructuralFieldType::Erased { .. }))
        .collect::<Vec<_>>();
    if roster.len() != fields.len() {
        return malformed("structural case establishment requires every selected field");
    }
    for (declaration, field) in roster.into_iter().zip(fields) {
        if declaration.id != field.field || declaration.relevance.is_erased() {
            return malformed("structural case establishment has an invalid field binding");
        }
        validate_constructor_field(module, machine, field, &declaration.field_type)?;
    }
    // Full module validation independently checks exact operand types,
    // dominance and declaration-derived obligations before acceptance.
    Ok(())
}

pub(super) fn validate_establish_byte_sequence_literal(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishByteSequenceLiteral { destination, .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_byte_sequence_literal")
    };
    if operation.result != OperationResult::Unit {
        return malformed("byte-sequence literal establishment declares a scalar result");
    }
    let Some(StructuralPlaceDeclaration {
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        },
        ..
    }) = machine.structural_place(*destination)
    else {
        return malformed("byte-sequence literal establishment has no literal declaration");
    };
    let Some(declaration) = module.structural_type(*structural_type) else {
        return malformed("byte-sequence literal has an unknown structural type");
    };
    if !matches!(
        declaration.shape,
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView { .. })
    ) {
        return malformed("byte-sequence literal must use a borrowed byte-sequence type");
    }
    Ok(())
}

pub(super) fn validate_establish_zeroed_structural(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishZeroedStructural {} = &operation.kind else {
        unreachable!("dispatched validate_establish_zeroed_structural")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("zeroed structural establishment has no structural result");
    };
    if !matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !result.qualification_establishments.is_empty()
        || !matches!(
            machine.structural_place(result.place),
            Some(StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        )
        || !terminal_semantics::zeroed_structural_shape(
            module.structural_types.iter(),
            result.structural_type,
        )
    {
        return malformed("zeroed structural establishment has an invalid result shape");
    }
    Ok(())
}

pub(super) fn validate_establish_structural_local(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishStructuralLocal {} = &operation.kind else {
        unreachable!("dispatched validate_establish_structural_local")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("structural local establishment has no structural result");
    };
    // Only a standalone byte-view home may carry qualifications; the verifier
    // authenticates them as primitive byte predicates of that carrier.
    let byte_view_home = module.structural_types.iter().any(|declaration| {
        declaration.id == result.structural_type
            && declaration.shape.is_borrowed_view_byte_sequence()
    });
    if !matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) || (!result.qualifications.is_empty() && !byte_view_home)
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !result.qualification_establishments.is_empty()
        || !matches!(
            machine.structural_place(result.place),
            Some(StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type }, ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        )
    {
        return malformed("structural local establishment has an invalid result shape");
    }
    Ok(())
}

pub(super) fn validate_establish_trivial_affine_local(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishTrivialAffineLocal { destination } = &operation.kind else {
        unreachable!("dispatched validate_establish_trivial_affine_local")
    };
    if operation.result != OperationResult::Unit {
        return malformed("trivial affine local establishment declares a scalar result");
    }
    let Some(StructuralPlaceDeclaration {
        kind: StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        },
        ..
    }) = machine.structural_place(*destination)
    else {
        return malformed("trivial affine local establishment has no local declaration");
    };
    if terminal_semantics::contains_placed_carrier(module, *structural_type) {
        return malformed("ordinary local construction cannot establish placed custody");
    }
    let Some(declaration) = module.structural_type(*structural_type) else {
        return malformed("trivial affine local has an unknown structural type");
    };
    if !matches!(
        &declaration.shape,
        StructuralTypeShape::Record { fields, .. } if fields.is_empty()
    ) {
        return malformed("trivial affine local must have an empty record type");
    }
    Ok(())
}

pub(super) fn validate_establish_record(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        unreachable!("dispatched validate_establish_record")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("record has no structural result");
    };
    validate_constructor_claims(module, result)?;
    // Multiplicity does not describe the record's live claim set: an affine
    // record can carry disjoint linear descendants. Source custody owns their
    // identities and transfers; this codec checks the result's storage shape,
    // not a second ownership eligibility rule.
    if !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) {
        return malformed("record result custody is noncanonical");
    }
    let Some(declaration) = module.structural_type(result.structural_type) else {
        return malformed("record type is absent");
    };
    let StructuralTypeShape::Record {
        fields: declarations,
        ..
    } = &declaration.shape
    else {
        return malformed("record result is not a record");
    };
    let declarations = declarations
        .iter()
        .filter(|field| !field.relevance.is_erased());
    if fields.len() != declarations.clone().count() {
        return malformed("record field roster differs");
    }
    for (field, declaration) in fields.iter().zip(declarations) {
        if field.field != declaration.id
            || declaration.relevance != terminal_psi::BindingRelevance::Relevant
        {
            return malformed("record field identity differs");
        }
        validate_constructor_field(module, machine, field, &declaration.field_type)?;
    }
    Ok(())
}

/// Records and cases bind the same completed field operands. The selected
/// declaration namespace changes the roster, never the carrier contract.
fn validate_constructor_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    field: &terminal_psi::RecordFieldInitializer,
    field_type: &StructuralFieldType,
) -> Result<(), CodecError> {
    match (&field.value, field_type) {
        (
            terminal_psi::RecordFieldValue::Scalar {
                range_obligation, ..
            },
            field_type,
        ) if field_type.scalar_type().is_some() => {
            if range_obligation.is_some()
                != terminal_semantics::scalar_field_requires_evidence(field_type)
            {
                return malformed("constructor field range obligation differs");
            }
        }
        (terminal_psi::RecordFieldValue::Structural(argument), field_type)
            if (matches!(field_type, StructuralFieldType::Structural(_))
                || matches!(
                    field_type.byte_sequence_carrier(),
                    Some(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            | terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
                ))
                && argument.access == terminal_psi::StructuralAccess::Owned =>
        {
            validate_constructor_projection(module, machine, argument, field_type)?;
            // Predicate wrappers do not change whole descriptor custody.
            // Independent verification checks availability, partial moves,
            // backing lifetime and receiving predicates.
        }
        _ => return malformed("constructor field operand differs"),
    }
    Ok(())
}

/// Aggregate multiplicity and descendant custody are independent. Retain each
/// exact typed path here; current ownership replay authenticates its identity
/// and operand transport rather than treating the catalogue as fresh issuance.
fn validate_constructor_claims(
    module: &TerminalModule,
    result: &terminal_psi::StructuralOperationResult,
) -> Result<(), CodecError> {
    // An intermediate constructor can publish custody at its private arrival
    // rather than on this operation. Do not infer issuance from the root's
    // multiplicity or require another claim before that owning edge.
    if result
        .claims
        .windows(2)
        .any(|pair| pair[0].claim >= pair[1].claim)
    {
        return malformed("aggregate claim bindings disagree with canonical order");
    }
    for (position, claim) in result.claims.iter().enumerate() {
        super::validate_structural_path(module, result.structural_type, &claim.path)?;
        if result.claims[position + 1..]
            .iter()
            .any(|other| terminal_semantics::structural_paths_may_overlap(&claim.path, &other.path))
        {
            return malformed("aggregate claim bindings overlap");
        }
    }
    Ok(())
}

pub(super) fn validate_establish_primitive_local(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishPrimitiveLocal {
        value,
        range_obligation,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_establish_primitive_local")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("primitive local has no structural result");
    };
    if !result.is_unrestricted_plain()
        || !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && matches!(place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type)
        })
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && declaration.scalar_restriction.is_some() == range_obligation.is_some()
                && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(carrier)
                    if super::declared_scalar_type(machine, *value) == Some(carrier))
        })
    {
        return malformed("primitive local result custody is noncanonical");
    }
    Ok(())
}

pub(super) fn validate_establish_byte_sequence_concatenation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let Some(result) = operation.result.structural() else {
        return malformed("byte concatenation requires an owned structural result");
    };
    if !matches!(machine.structural_place(result.place).map(|row| row.kind),
        Some(StructuralPlaceKind::OperationResult { producer, structural_type })
            if producer == operation.id && structural_type == result.structural_type)
        || !module.structural_types.iter().any(|row| {
            row.id == result.structural_type && row.shape.is_bounded_owned_byte_sequence()
        })
    {
        return malformed("byte concatenation result differs from its owned carrier declaration");
    }
    Ok(())
}

/// Projection is already part of the canonical StructuralArgument vocabulary.
/// Constructor transfers select finite owned structural children, including
/// fields in a selected case namespace. A Case segment preserves that namespace;
/// it does not itself prove the case active or grant custody. Typed-path checking
/// below authenticates the geometry; consuming custody authenticates the active
/// case and partial transfer. Runtime selectors and reference crossings require
/// other authority and are not finite owned constructor paths.
/// Whole operands retain their existing independent carrier/predicate checks.
fn validate_constructor_projection(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &terminal_psi::StructuralArgument,
    field_type: &StructuralFieldType,
) -> Result<(), CodecError> {
    if argument.path.is_empty() {
        return Ok(());
    }
    let Some(expected) = field_type.as_structural() else {
        return malformed("projected constructor operand requires a structural field");
    };
    let Some(root) = super::structural_place_type(machine, argument.place) else {
        return malformed("projected constructor operand has no declared root");
    };
    let Some((_, selected)) = terminal_semantics::canonical_static_projection(
        module.structural_types.iter(),
        root,
        &argument.path,
    ) else {
        return malformed("projected constructor operand requires a finite owned path");
    };
    if selected.id != expected {
        return malformed("projected constructor operand differs from its field carrier");
    }
    Ok(())
}

pub(super) fn validate_establish_structural_array(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishStructuralArray { elements } = &operation.kind else {
        unreachable!("array dispatch")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("array has no structural result");
    };
    validate_constructor_claims(module, result)?;
    // Array custody is the disjoint frontier of its completed elements,
    // including linear descendants. Their claims are transported, not issued
    // again or forbidden by a claim-free construction eligibility rule.
    if !result.qualifications.is_empty()
        || !machine.structural_places.iter().any(|place| place.id == result.place && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type } if producer == operation.id && structural_type == result.structural_type)) {
        return malformed("array result custody is noncanonical");
    }
    let Some(StructuralTypeShape::FixedArray { element, length }) = module
        .structural_type(result.structural_type)
        .map(|row| &row.shape)
    else {
        return malformed("structural array result is not a fixed array");
    };
    if u64::try_from(elements.len()).ok() != Some(*length) {
        return malformed("array element count differs");
    }
    for argument in elements {
        if argument.access != terminal_psi::StructuralAccess::Owned
            || argument.place == result.place
        {
            return malformed("array element is not an independent owned source");
        }
        let Some(root) = super::structural_place_type(machine, argument.place) else {
            return malformed("array element source is absent");
        };
        if super::validate_structural_path(module, root, &argument.path)? != *element {
            return malformed("array element type differs");
        }
        validate_constructor_projection(
            module,
            machine,
            argument,
            &StructuralFieldType::Structural(*element),
        )?;
    }
    Ok(())
}
