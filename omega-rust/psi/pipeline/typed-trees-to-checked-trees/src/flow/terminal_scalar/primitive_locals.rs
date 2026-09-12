//! Local referents demanded by checked computation and ordinary Unit calls.

use checked_trees::{
    CheckedScalarBinding, CheckedScalarBindingDestination, CheckedScalarBindingValue,
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole, CheckedScalarPrimitiveLocalPlan, CheckedStructuralAccess,
    CheckedUnitStructuralArgumentSourcePlan,
};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, statement::StatementNode, types::TypeReferenceNode};

pub(in crate::flow) fn collect(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expressions: &CheckedScalarExpressionPlans,
    computations: &CheckedScalarComputationPlans,
    bindings: &[CheckedScalarBinding],
) -> Option<Vec<CheckedScalarPrimitiveLocalPlan>> {
    let mut locals = Vec::new();
    // Failed or folded computation construction can leave orphan arena nodes.
    // Only retained roots can demand storage in this source state.
    for (_, root) in computations
        .roots
        .iter()
        .filter(|(_, root)| root.state == state.symbol)
    {
        if root.machine != machine.symbol
            || computations
                .root_at(state.symbol, root.statement_ordinal, root.role)
                .is_none()
        {
            return None;
        }
        let mut pending = vec![root.root];
        let mut visited = Vec::new();
        while let Some(handle) = pending.pop() {
            if !computations.nodes.is_valid(handle) {
                return None;
            }
            if visited.contains(&handle) {
                continue;
            }
            visited.push(handle);
            match &computations.nodes.get(handle).kind {
                CheckedScalarComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Place(_)
                        | checked_trees::CheckedScalarComputationStructuralArgument::Array { .. },
                    ..
                } => {}
                CheckedScalarComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                    ..
                } => {
                    pending.extend(
                        computations
                            .case_fields
                            .span(subject.fields)?
                            .iter()
                            .map(|field| field.value),
                    );
                }
                CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                    pending.extend([*left, *right])
                }
                CheckedScalarComputationKind::Qualification { operand, .. } => {
                    pending.push(*operand)
                }
                CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                    pending.push(*subject);
                    for arm in computations.dispatch_arms.span(*arms)? {
                        if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) =
                            arm.pattern
                        {
                            pending.push(pattern);
                        }
                        pending.push(arm.value);
                    }
                }
                CheckedScalarComputationKind::Value(_)
                | CheckedScalarComputationKind::StructuralField { .. } => {}
                CheckedScalarComputationKind::Call {
                    arguments,
                    structural_arguments,
                    ..
                } => {
                    pending.extend_from_slice(computations.operands.span(*arguments)?);
                    for argument in computations
                        .structural_arguments
                        .span(*structural_arguments)?
                    {
                        let argument = match argument {
                            checked_trees::CheckedScalarComputationStructuralArgument::Case(
                                subject,
                            ) => {
                                pending.extend(
                                    computations
                                        .case_fields
                                        .span(subject.fields)?
                                        .iter()
                                        .map(|field| field.value),
                                );
                                continue;
                            }
                            checked_trees::CheckedScalarComputationStructuralArgument::Place(
                                argument,
                            ) => argument,
                            checked_trees::CheckedScalarComputationStructuralArgument::Array {
                                elements,
                                ..
                            } => {
                                pending.extend_from_slice(computations.operands.span(*elements)?);
                                continue;
                            }
                        };
                        let CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } =
                            argument.source
                        else {
                            continue;
                        };
                        if !argument.path.is_empty()
                            || argument.access == CheckedStructuralAccess::Owned
                        {
                            return None;
                        }
                        let local = local_plan(
                            program,
                            machine,
                            state,
                            expressions,
                            computations,
                            bindings,
                            symbol,
                            root.statement_ordinal,
                        )?;
                        if local.type_identity != argument.type_identity {
                            return None;
                        }
                        if !locals.contains(&local) {
                            locals.push(local);
                        }
                    }
                }
                CheckedScalarComputationKind::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    pending.extend([*condition, *when_true, *when_false]);
                }
                CheckedScalarComputationKind::Apply { operands, .. } => {
                    pending.extend_from_slice(computations.operands.span(*operands)?);
                }
            }
        }
    }
    // A statement call requires the same real referent as a computation call.
    // The completed Unit operation and receiving source replay independently
    // rejoin the exact target signature and checked borrow occurrence.
    for (ordinal, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Call(call) = statement else {
            continue;
        };
        for expression in program.statement_table.expression_handles(call.arguments) {
            let typed_trees::expression::ExpressionNode::Borrow(borrow) =
                program.expression_table.expression(*expression)
            else {
                continue;
            };
            let typed_trees::expression::ExpressionNode::Name(name) =
                program.expression_table.expression(borrow.target)
            else {
                continue;
            };
            if !program.statement_table.statements(state.statement_nodes).iter().any(|statement|
                matches!(statement, StatementNode::LocalData(local) if local.symbol == name.symbol && local.is_mutable)) { continue; }
            let local = local_plan(
                program,
                machine,
                state,
                expressions,
                computations,
                bindings,
                name.symbol,
                u32::try_from(ordinal).ok()?,
            )?;
            if !locals.contains(&local) {
                locals.push(local);
            }
        }
    }
    locals.sort_by_key(|local| local.statement_ordinal);
    Some(locals)
}

fn local_plan(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expressions: &CheckedScalarExpressionPlans,
    computations: &CheckedScalarComputationPlans,
    bindings: &[CheckedScalarBinding],
    symbol: SymbolHandle,
    use_statement_ordinal: u32,
) -> Option<CheckedScalarPrimitiveLocalPlan> {
    if !symbol.is_valid() {
        return None;
    }
    let mut declarations = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
            _ => None,
        });
    let (ordinal, local) = declarations.next()?;
    let statement_ordinal = u32::try_from(ordinal).ok()?;
    if declarations.next().is_some()
        || statement_ordinal >= use_statement_ordinal
        || !local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Named { .. }
        )
        || crate::checks::type_multiplicity(program, local.type_reference)
            != language_semantics::Multiplicity::Unrestricted
    {
        return None;
    }
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    let mut initializers = bindings
        .iter()
        .filter(|binding| binding.statement_ordinal == statement_ordinal);
    let initializer = initializers.next()?;
    if initializers.next().is_some()
        || initializer.destination
            != (CheckedScalarBindingDestination::StorageInitialize { symbol })
        || initializer.primitive_type != primitive_type
    {
        return None;
    }
    let role = CheckedScalarExpressionRole::StorageInitializer;
    match initializer.value {
        CheckedScalarBindingValue::Computation => {
            let root = computations.root_at(state.symbol, statement_ordinal, role)?;
            if root.machine != machine.symbol || !computations.nodes.is_valid(root.root) {
                return None;
            }
            let computation = computations.nodes.get(root.root);
            if computation.authored_root != local.initial_value
                || computation.primitive_type != primitive_type
            {
                return None;
            }
        }
        CheckedScalarBindingValue::Expression => {
            let (source, value) =
                expressions.bound_expression_at(state.symbol, statement_ordinal, role)?;
            if source.destination != symbol
                || source.expression != local.initial_value
                || crate::values::scalar_expression_type(value) != Some(primitive_type)
            {
                return None;
            }
        }
        CheckedScalarBindingValue::DirectCall { .. } => return None,
    }
    Some(CheckedScalarPrimitiveLocalPlan {
        statement_ordinal,
        symbol,
        primitive_type,
        // This is ShapeCollector's normalization for an already plain primitive:
        // no reference stripping, generic substitution, or invented nominal name.
        type_identity: program
            .normalized_type_identity_with_binders_and_substitutions(local.type_reference, &[], &[])
            .into_string(),
    })
}
