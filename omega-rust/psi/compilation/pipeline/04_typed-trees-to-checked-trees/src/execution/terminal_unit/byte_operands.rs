//! Prepare one byte operand independently of the operation consuming it.
//!
//! A comparison, predicate scan and descriptor replacement observe the same
//! current place or completed view expression. The caller supplies its actual
//! parameter and selector namespaces; preparation never fabricates a call or
//! replays a local initializer as current storage. Lifetime and predicate
//! obligations remain with the consuming operation.
use super::{calls, types};
use checked_trees::{
    CheckedOperatorFacts, CheckedRuntimeIndex, CheckedScalarExpressionPlans,
    CheckedStructuralAccess, CheckedSubsliceSite, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan as Source,
};
use typed_trees::{expression::ExpressionHandle, types::TypeReferenceHandle};

#[allow(clippy::too_many_arguments)]
pub(crate) fn plan(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    pure: &CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    borrow: &checked_trees::BorrowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    operand: ExpressionHandle,
    target: TypeReferenceHandle,
    dense_parameter: &dyn Fn(usize) -> Option<u32>,
    selector: &dyn Fn(ExpressionHandle) -> Option<CheckedRuntimeIndex>,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if let Some(literal) = calls::byte_sequence_literal_argument(program, target, operand) {
        return Some(literal);
    }
    if let Some(subject) = place(
        program,
        machine,
        state,
        statement_index,
        operand,
        target,
        dense_parameter,
        selector,
    ) {
        return Some(subject);
    }
    let parameters = if machine.attached_data.is_some() {
        calls::ambient_self_scalar_signature(program, machine, state)?.0
    } else {
        calls::free_structural_scalar_signature(
            program,
            &mut types::ShapeCollector::new(program),
            state,
            &[],
        )?
        .0
    };
    calls::view_subslice::admit_resolved(
        program,
        operators,
        pure,
        computations,
        borrow,
        machine,
        state,
        &parameters,
        target,
        operand,
        statement_index,
        CheckedSubsliceSite::Expression {
            expression: operand,
        },
    )
    .map(|resolution| resolution.argument())
}

#[allow(clippy::too_many_arguments)]
fn place(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    operand: ExpressionHandle,
    target: TypeReferenceHandle,
    dense_parameter: &dyn Fn(usize) -> Option<u32>,
    selector: &dyn Fn(ExpressionHandle) -> Option<CheckedRuntimeIndex>,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        operand,
    )?;
    if !validation::place_has_builtin_coordinates(program, machine, Some(state), operand) {
        return None;
    }
    let root = place.root.as_symbol()?;
    let source = if let Some(position) =
        typed_trees::signature::state_parameter_position(program.state_parameters(state), root)
    {
        Source::Parameter {
            parameter_index: dense_parameter(position)?,
        }
    } else {
        Source::StructuralLocal { symbol: root }
    };
    let (projected, path) = calls::projected_selected_path(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        &place,
        &selector,
    )?;
    // An explicit readable borrow observes its referent's current bytes.
    // The source place supplies that storage directly; the predicate does
    // not need a freshly materialized pointer wrapper to scan the same bytes.
    let observed = if program.normalized_type_identity(projected)
        == program.normalized_type_identity(target)
    {
        target
    } else {
        match validation::reference_shell(program, target) {
            Some(shell) if shell.access != language_semantics::ReferenceAccess::WriteOnly => {
                shell.referee
            }
            Some(_) => return None,
            None => target,
        }
    };
    if program.normalized_type_identity(projected) != program.normalized_type_identity(observed) {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity: validation::stored_structural_type_identity(program, projected, &[], &[]),
        access: CheckedStructuralAccess::SharedBorrow,
    })
}

/// Descriptor expressions can produce a standalone carrier even when only an
/// inline record field mentions their type. Register the authored expression's
/// exact carrier in the same vocabulary as call arguments and scalar operands.
/// This records types only; source custody and evaluation remain at the store.
pub(super) fn store_operand_type_identities(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut types::ShapeCollector<'_>,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> Vec<typed_trees::type_identity::TypeIdentityId> {
    let Some(value) = operation.stored_bytes() else {
        return Vec::new();
    };
    let Some(machine) = program.machine(machine) else {
        return Vec::new();
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.symbol == state)
    else {
        return Vec::new();
    };
    value
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            let checked_trees::CheckedByteSequenceValue::Descriptor { expression, source } = node
            else {
                return None;
            };
            // Literal descriptors use the immutable literal producer's carrier.
            if matches!(source.source, Source::ByteSequenceLiteral { .. }) {
                return None;
            }
            let reference =
                validation::expression_result_type_reference(program, machine, state, *expression)?;
            let identity = shapes.add_type(reference, &[], &[])?;
            (identity == source.type_identity).then_some(identity)
        })
        .collect()
}
