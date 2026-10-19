//! Whole plain-owned occurrences share source identity across records and sums.
//! This classifies the named storage only; flow checking must establish its
//! availability and each selected transfer's residual ownership independently.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

/// A qualification or erasure of the same stored structural carrier keeps
/// its operand's ownership. Compare full applications, not nominal heads;
/// representation conversion, arithmetic conversion and borrow recasts do not
/// supply this relation. Membership checking remains independent.
pub fn value_preserving_cast_operand(
    program: &dyn crate::CheckingInput,
    state: SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let cast = program.expression_table.cast_node(expression)?;
    if cast.form.is_recast()
        || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
        || program.primitive_type_reference(cast.target_type).is_some()
    {
        return None;
    }
    let actual = crate::expression_type_reference_in_state(program, state, statement, cast.value)?;
    let actual_identity = crate::stored_structural_type_identity(program, actual, &[], &[]);
    let target_identity =
        crate::stored_structural_type_identity(program, cast.target_type, &[], &[]);
    (actual_identity.is_valid() && actual_identity == target_identity).then_some(cast.value)
}

/// The evaluated owner beneath representation-preserving qualifications. This
/// does not replay an initializer or turn a computed value into an lvalue;
/// it identifies which exact operand's permission events a consumer must join.
pub fn ownership_source_expression(
    program: &dyn crate::CheckingInput,
    state: SymbolHandle,
    statement: usize,
    mut expression: ExpressionHandle,
) -> ExpressionHandle {
    while let Some(operand) = value_preserving_cast_operand(program, state, statement, expression) {
        expression = operand;
    }
    expression
}

/// Resolve a whole local or parameter without erasing a borrow, qualification,
/// or nominal type. A projection is not its containing owner's whole value.
pub fn plain_owned_value_source(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    whole_owned_value_source(program, expression, reference).filter(|_| {
        crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::PlainOwned,
        )
    })
}

/// The same whole-place resolution for an affine root, plain or carrying
/// linear claims: the named storage is the whole value, and the caller's
/// claim accounting (not this predicate) decides which consumed claims the
/// transfer names. `has_contents` keeps the same finite owned
/// walls as the plain rule — no loans, slices, nominal cleanup, or recursive
/// storage — while letting a `[linear]` member remain a tracked claim.
pub fn affine_owned_value_source(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    whole_owned_value_source(program, expression, reference).filter(|_| {
        program.type_multiplicity(reference) == language_semantics::Multiplicity::Affine
            && crate::has_contents(
                program,
                reference,
                &[],
                crate::value_custody::storage_contents::ContentsRequirement::LinearOwned,
            )
    })
}

/// The same whole-place resolution for a linear carrier: the named storage is
/// the whole value, and the caller's claim accounting (not this predicate)
/// decides whether the move is available on every path that needs it.
pub fn linear_owned_value_source(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    whole_owned_value_source(program, expression, reference).filter(|_| {
        program.type_multiplicity(reference) == language_semantics::Multiplicity::Linear
            && crate::has_contents(
                program,
                reference,
                &[],
                crate::value_custody::storage_contents::ContentsRequirement::LinearOwned,
            )
    })
}

/// A whole local or parameter name whose declared type is exactly `reference`.
/// Contents and multiplicity remain the caller's question: a shared borrow,
/// a linear claim, and a plain affine value all resolve to the same place.
pub fn whole_owned_value_source(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let path = program.expression_table.name_path_node(expression)?;
    if path.symbol != path.head_symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || !matches!(
            program.symbols.get(path.symbol).kind,
            SymbolKind::Local | SymbolKind::Parameter
        )
    {
        return None;
    }
    let actual = crate::value_custody::expression_types::named_value_type_reference(program, path)?;
    (program.normalized_type_identity(actual) == program.normalized_type_identity(reference))
        .then_some(path.symbol)
}
