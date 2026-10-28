//! Symbolic scalar reads through checked representation views.
//!
//! A view preserves a captured bit pattern, independently of the target's
//! numeric equality (notably NaNs and signed zero). This owner records that
//! interpretation and composes inverse views; recast admission still owns
//! geometry, validity, lifetime and access before any value is observed.

use super::StructuralTerm;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(in crate::proof_contracts::contract_entailment) fn scalar_representation_term(
    program: &dyn crate::CheckingInput,
    cast: &typed_trees::expression::TableCastExpression,
    source: TypeReferenceHandle,
    value: StructuralTerm,
) -> Option<StructuralTerm> {
    if cast.form.recast_access() != Some(language_semantics::ReferenceAccess::Shared) {
        return None;
    }
    if !super::scalar_operand_is_observed(program, source, &value) {
        return None;
    }
    // The source borrow may enclose this recast, so its operand is the
    // selected place's owned carrier. An already borrowed operand must carry
    // its own authenticated shared observation.
    let source_carrier = match crate::reference_shell(program, source) {
        Some(shell) if shell.access == language_semantics::ReferenceAccess::Shared => shell.referee,
        Some(_) => return None,
        None => source,
    };
    let source = program.primitive_type_reference(source_carrier)?;
    // A recast stores the referent carrier separately from its loan access.
    // The prepared expression type owns the resulting descriptor shell.
    let target = program.primitive_type_reference(cast.target_type)?;
    let scalar = |primitive: PrimitiveType| {
        primitive.is_fixed_integer() || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
    };
    if !scalar(source) || !scalar(target) || source.scalar_byte_size() != target.scalar_byte_size()
    {
        return None;
    }
    Some(normalize_scalar_representation(source, target, value))
}

pub(in crate::proof_contracts::contract_entailment) fn normalize_scalar_representation(
    source: PrimitiveType,
    target: PrimitiveType,
    value: StructuralTerm,
) -> StructuralTerm {
    if source == target {
        return value;
    }
    if let StructuralTerm::ScalarRepresentation {
        source: original,
        target: intermediate,
        value: original_value,
    } = value.semantic_value()
        && *intermediate == source
    {
        return normalize_scalar_representation(*original, target, *original_value.clone());
    }
    StructuralTerm::ScalarRepresentation {
        source,
        target,
        value: Box::new(value),
    }
}
