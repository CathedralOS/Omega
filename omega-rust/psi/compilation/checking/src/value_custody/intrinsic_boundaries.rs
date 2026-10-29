//! Source-call classification for the sealed hosted intrinsic vocabulary.
//!
//! A declared boundary keeps its exact signature and requirement edge. This
//! module does not issue provider authority: Omega joins the sealed immutable
//! catalog, complete contract and explicit target-keyed Build choice before
//! realization. Native locators belong to Build requests, never source leaves.

use symbols::SymbolHandle;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// The exact boundary requirement a direct call to a bodyless external leaf
/// delegates to, with the leaf's attached provider (the invalid symbol for a
/// free leaf). Checking and lowering use this join for every external leaf;
/// the checked interpreter keeps `exact_compiler_intrinsic_boundary_requirement`
/// because it hosts only the intrinsic catalog.
pub fn exact_external_leaf_boundary_requirement(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    exact_compiler_intrinsic_boundary_requirement(program, target_state_symbol)
}

/// The immutable catalogue's exact requirement edge, independently of target
/// selection, native permission or accepted package execution authority.
pub fn exact_compiler_intrinsic_boundary_requirement(
    program: &dyn crate::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    let machine = program.machine_holding_state(target_state_symbol)?;
    let catalogue = program.hosted_intrinsic_declaration(machine.symbol)?;
    let [entry] = program.machine_states(machine) else {
        return None;
    };
    (entry.symbol == target_state_symbol)
        .then_some((catalogue.requirement.symbol, catalogue.code_owner))
}

/// A `CheckedShape` data declaration resolved through a type reference's
/// exact named head — the admission prologue hosted intrinsic result
/// contracts share. `None` for anything but a plain (no lifetime or type
/// parameters, generic instance, quotient, where-facts, zero-gate, carry or
/// Linear multiplicity) `CheckedShape` data named `name`. The returned
/// symbol is the type reference's head, which the declaration resolved to.
pub fn checked_shape_data_declaration<'program>(
    program: &'program dyn crate::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
    name: &str,
) -> Option<(&'program typed_trees::data::DataDefinition, SymbolHandle)> {
    let symbol = &program
        .type_reference_table
        .type_reference(result_type)
        .as_named_symbol()?;
    let data = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
    if !symbol.is_valid()
        || data.name.as_str() != name
        || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || !data.lifetime_parameters.is_empty()
        || !program.data_type_parameters(data).is_empty()
        || data.generic_instance.is_some()
        || data.quotient.is_some()
        || !data.where_facts.is_empty()
        || data.zero_gated
        || data.properties.carry.is_some()
        || data.properties.multiplicity == language_semantics::Multiplicity::Linear
    {
        return None;
    }
    Some((data, *symbol))
}

/// The supported byte-input result, resolved by declaration identity. This is
/// typed shape validation, not selected-package or native execution admission.
pub fn exact_byte_read_result_type(
    program: &dyn crate::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    use typed_trees::data::DataMember;

    let (data, symbol) = checked_shape_data_declaration(program, result_type, "ByteRead")?;
    let [DataMember::Variant(eof), DataMember::Variant(byte)] = program.data_members(data) else {
        return None;
    };
    if eof.name.as_str() != "Eof"
        || !eof.payload.is_empty()
        || !eof.where_facts.is_empty()
        || byte.name.as_str() != "Byte"
    {
        return None;
    }
    let [field] = program.data_payload_fields(byte) else {
        return None;
    };
    // The host operation stores one input byte in a bare `i32` carrier. A
    // qualified carrier names a domain this operation cannot establish.
    if field.name.as_str() != "value"
        || field.relevance.is_erased()
        || !matches!(
            program
                .type_reference_table
                .type_reference(field.type_reference),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(field.type_reference) != Some(PrimitiveType::I32)
    {
        return None;
    }
    // The case's `where` facts may narrow the payload but must admit every
    // input byte, so only interval facts containing 0..=255 qualify.
    crate::case_where_field_intervals(program, byte)?
        .iter()
        .all(|(_, minimum, maximum)| {
            minimum.is_none_or(|minimum| minimum <= 0)
                && maximum.is_none_or(|maximum| maximum >= 255)
        })
        .then_some(symbol)
}
/// Completion belongs to the admitted requirement declaration, independently
/// of the provider selected to realize it. The join itself is typed-program
/// vocabulary; checking forwards its `CheckingInput` view.
pub fn boundary_requirement_completion(
    program: &dyn crate::CheckingInput,
    requirement: SymbolHandle,
) -> semantic_vocabulary::BoundaryCompletion {
    typed_trees::boundary_completions::boundary_requirement_completion(
        program.typed_program(),
        requirement,
    )
}

/// The core terminal-event identity used by both checking and interpretation.
pub fn is_canonical_process_exit_requirement(
    program: &dyn crate::CheckingInput,
    trait_symbol: SymbolHandle,
    requirement: SymbolHandle,
) -> bool {
    typed_trees::boundary_completions::is_canonical_process_exit_requirement(
        program.typed_program(),
        trait_symbol,
        requirement,
    )
}

/// Rejoin a semantic-role receipt to the exact package-owned declaration.
/// Consumer policy supplies the role; declaration spelling alone cannot.
pub fn valid_boundary_completion_declaration(
    program: &dyn crate::CheckingInput,
    declaration: &typed_trees::BoundaryCompletionDeclaration,
) -> bool {
    typed_trees::boundary_completions::valid_boundary_completion_declaration(
        program.typed_program(),
        declaration,
    )
}

/// Malformed source-role input must fail admission, never weaken an external
/// completion promise into an ordinary return by losing its exact join.
pub fn validate_boundary_completion_declarations(
    program: &dyn crate::CheckingInput,
) -> Result<(), &'static str> {
    typed_trees::boundary_completions::validate_boundary_completion_declarations(
        program.typed_program(),
    )
}
