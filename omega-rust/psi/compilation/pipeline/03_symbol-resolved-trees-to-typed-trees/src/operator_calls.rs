//! Close an operator occurrence to its ordinary declaration, after operand
//! qualifications have settled. The selected machine retains its full contract;
//! this pass supplies no checking evidence and changes no argument evaluation.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Late,
    AuthoredDeclarationSelectionTarget as Target,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionNode, TableCallExpression};

/// One declaration/type query shared by call preparation and domain-cast result
/// accounting. Unknown nonliteral types never act as wildcard operands.
pub(crate) fn selection(
    program: &TypedTrees,
    types: &mut crate::typing_queries::DeclaredExpressionTypes,
    owner: symbols::SymbolHandle,
    handle: typed_trees::expression::ExpressionHandle,
) -> Result<Option<symbols::SymbolHandle>, Diagnostic> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(handle) else {
        return Ok(None);
    };
    if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
        return Ok(None);
    }
    let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator) else {
        return Ok(None);
    };
    let mut operands = Vec::with_capacity(2);
    for operand in [binary.left, binary.right] {
        let reference = types.get(program, owner, operand);
        if reference.is_none()
            && !typed_trees::closed_numeric::has_anonymous_numeric_results(program, operand)
        {
            return Ok(None);
        }
        operands.push(reference);
    }
    let span = program.expression_table.source_span(handle);
    selection_for_operands(program, spelling, &operands, span)
}

pub(crate) fn selection_for_operands(
    program: &TypedTrees,
    spelling: language_core::operator_spelling::OperatorSpelling,
    operands: &[Option<typed_trees::types::TypeReferenceHandle>],
    span: source::SourceSpan,
) -> Result<Option<symbols::SymbolHandle>, Diagnostic> {
    // An anonymous arithmetic tree has no declared home. Like the existing
    // builtin-meaning query, keep it builtin rather than letting every family
    // match a tuple of wildcard operands.
    if operands.iter().all(Option::is_none) {
        return Ok(None);
    }
    let candidates =
        typed_trees::operator::resolve_spelling_for_operands(program, spelling, operands, span);
    let selected = match candidates.as_slice() {
        [] => return Ok(None),
        [selected] => selected.operator.symbol,
        _ => {
            return Err(
                Diagnostic::error("ambiguous operator declarations for these operands")
                    .with_source_span(span),
            );
        }
    };
    // This connector serves ordinary closed bodies. An unapplied generic or
    // bodyless requirement must never acquire a fabricated direct call.
    Ok(program
        .machine_by_symbol(selected)
        .filter(|machine| {
            machine.type_parameters.is_empty()
                && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        })
        .map(|machine| machine.symbol))
}

pub(crate) fn prepare(program: &mut TypedTrees, frontier: usize) -> Result<(), Diagnostic> {
    let sites = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Binary(binary)
                if handle.arena_index() as usize > frontier
                    && binary.policy == numerics::arithmetic::ArithmeticDomain::Exact =>
            {
                typed_trees::operator::binary_operator_spelling(binary.operator)
                    .map(|_| (handle, *binary))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if sites.is_empty() {
        return Ok(());
    }
    // One shared declaration-owner inventory, rather than a program scan for
    // every operand. The spelling query uses the prepared family index.
    let owners = typed_trees::references::collect_program_operand_owners(program);
    let mut types = crate::typing_queries::DeclaredExpressionTypes::default();
    for (handle, binary) in sites {
        let span = program.expression_table.source_span(handle);
        let mut answer = None;
        for owner in owners.expression_owners(handle) {
            let selected = selection(program, &mut types, owner, handle)?;
            if answer.is_some_and(|previous| previous != selected) {
                return Err(Diagnostic::error(
                    "operator occurrence has conflicting declaration contexts",
                )
                .with_source_span(span));
            }
            answer = Some(selected);
        }
        let Some(Some(target)) = answer else {
            continue;
        };
        // Generic and bodyless requirements still require their own genuine
        // application/selection producer. Never turn one into a direct call.
        let Some(machine) = program.machine_by_symbol(target) else {
            continue;
        };
        if !machine.type_parameters.is_empty()
            || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        {
            continue;
        }
        let label = machine.name.clone();
        let selections = program
            .expression_table
            .authored_selection_occurrences(handle)
            .filter(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(*occurrence)
                    .is_some_and(|selection| {
                        selection.kind() == Kind::Operator && selection.source_span() == span
                    })
            })
            .collect::<Vec<_>>();
        let [occurrence] = selections.as_slice() else {
            return Err(
                Diagnostic::error("operator call lost its unique authored selection")
                    .with_source_span(span),
            );
        };
        let selection = program
            .authored_declaration_selections()
            .get(*occurrence)
            .unwrap();
        match selection.target() {
            Target::LateBound(Late::CheckedOperator) => {
                program
                    .finalize_late_bound_authored_declaration_selection(
                        *occurrence,
                        Late::CheckedOperator,
                        target,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!("operator choice cannot be retained: {error:?}"))
                            .with_source_span(span)
                    })?;
            }
            Target::Resolved(selected) if selected.selected_symbol() == target => {}
            _ => {
                return Err(Diagnostic::error(
                    "operator choice contradicts its authored selection",
                )
                .with_source_span(span));
            }
        }
        let arguments = program
            .expression_table
            .insert_expression_handles([binary.left, binary.right]);
        *program.expression_table.expression_mut(handle) =
            ExpressionNode::Call(TableCallExpression {
                runtime_call: None,
                authored_call_selection: Some(*occurrence),
                authored_call_binding: None,
                authored_machine_application: Default::default(),
                build_selection: Default::default(),
                exact_coercion_target: Default::default(),
                requirement_application: Default::default(),
                receiver: Default::default(),
                target_symbol: target,
                static_machine_parameter: Default::default(),
                target: label,
                static_requirement_dispatch: None,
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                installed_interrupt_operation: None,
                layout_query: None,
                native_code_layout: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            });
    }
    Ok(())
}
