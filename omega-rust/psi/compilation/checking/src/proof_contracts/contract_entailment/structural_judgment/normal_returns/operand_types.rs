//! Receiving carriers for the shared ordered child roster.
//!
//! A contextual collection literal has no standalone type reference. Capturing
//! it under a call's result type discarded that entire normal-return path,
//! including unrelated scalar facts needed by later arguments. Calls and
//! constructors instead supply their exact selected operand declarations.
//! These are typing contexts, not evidence of argument admission, referent
//! origins or post-call preservation. Those judgments retain their owners.

use super::super::super::specification_calls::{
    SelectedCallContract, call_parameter_scope, owner_leading_operands, selected_call_contract,
};
use crate::frozen_program::expression_table;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::TypeReferenceHandle;

/// Positions match `expression_traversal::children`, including its receiver
/// slot even when absent. No expression-handle search may select a receiving
/// position: the same handle can occur on more than one operand edge.
pub(super) fn receiving_types(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    expression: ExpressionHandle,
    node: &ExpressionNode,
    receiving: TypeReferenceHandle,
) -> Option<Vec<TypeReferenceHandle>> {
    match node {
        ExpressionNode::Call(call) => {
            let mut owner = selected_call_contract(program, caller.symbol, call)?;
            // A static generic qualifier owns its public application just as
            // a dynamic slot owns its instantiated data telescope. Keep that
            // scope; an implementation's private parameter types are not it.
            if let SelectedCallContract::Signature(signature) = owner
                && program
                    .closed_requirement_application(signature.symbol)
                    .is_none()
                && call.requirement_application.is_valid()
            {
                let (declaration, _) = program.trait_machine_signature(signature.symbol)?;
                let application = program
                    .applied_requirement_scope(caller.symbol, call.requirement_application)?
                    .declaring_scope(program, declaration.symbol)?;
                // Preparation recursively instantiated composite carriers.
                // A shallow substitution cannot turn &[T] into &[u8]. Keep
                // a closed instance's own method-local arguments above, and
                // require this exact prepared public view for an open owner.
                owner = SelectedCallContract::Signature(
                    program.applied_callable_signature(&application, signature.symbol)?,
                );
            }
            let parameters = call_parameter_scope(
                program,
                caller.symbol,
                &owner,
                crate::CallSite::Expression { expression, call },
            )?;
            let leading = owner_leading_operands(&owner);
            if expression_table(program)
                .expression_handles(call.arguments)
                .len()
                != leading + parameters.len()
            {
                return None;
            }
            // Dispatch-prefix values are independently typed. Only data
            // arguments receive the public callable's parameter carriers.
            let mut references = vec![TypeReferenceHandle::invalid(); 1 + leading];
            references.extend(parameters.iter().map(|parameter| parameter.type_reference));
            Some(references)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            // Each element receives the element carrier, not the aggregate.
            // This also preserves contextual literals nested inside literals.
            let element = crate::value_custody::places::collection_element_type_reference(
                program, receiving,
            )?;
            Some(vec![
                element;
                expression_table(program)
                    .expression_handles(*elements)
                    .len()
            ])
        }
        ExpressionNode::StructLiteral(literal) => Some(
            expression_table(program)
                .struct_fields(literal.fields)
                .iter()
                .map(|field| {
                    crate::value_custody::struct_literals::selected_construction_field_type(
                        program,
                        expression,
                        literal,
                        field.field_symbol,
                    )
                    .unwrap_or_else(TypeReferenceHandle::invalid)
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Context for an authenticated ordered statement prefix. Argument positions
/// are read from its complete selected telescope, rather than by finding an
/// expression handle in a callee's parameter list. This matters for literals
/// without standalone types and for applied argument qualifications.
pub(super) fn prefix_receiving_types(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    roots: &[ExpressionHandle],
) -> Option<Vec<TypeReferenceHandle>> {
    use super::super::super::specification_calls::selected_call_site_contract;
    use typed_trees::statement::StatementNode;
    let statement = program
        .statement_table
        .statement_at(state, statement_index)?;
    match statement {
        StatementNode::LocalData(local) => return Some(vec![local.type_reference; roots.len()]),
        StatementNode::Assignment(assignment) => {
            let reference =
                crate::assignment_stored_value_type(program, caller, state, assignment)?;
            return Some(vec![reference; roots.len()]);
        }
        StatementNode::Expression(_) => return Some(vec![state.return_type; roots.len()]),
        _ => {}
    }
    let mut selected = None;
    for site in crate::machine_calls::calls::collect_call_sites_in_statement(
        program,
        caller,
        state,
        statement_index,
    ) {
        // Calls nested inside a root supply their own operand types during
        // evaluation. Only a statement/transition telescope owns these roots.
        if matches!(site, crate::CallSite::Expression { .. }) {
            continue;
        }
        let arguments = crate::call_site_argument_expressions(program, &site);
        if !arguments.starts_with(roots) {
            continue;
        }
        let (parameters, leading) = if site.is_internal_state_transfer(program, caller.symbol)
            || matches!(site, crate::CallSite::TransitionNamed { .. })
                && site.target_symbol(program) == caller.symbol
        {
            // Internal arrivals retain this machine's already resolved state
            // telescope. They have no public callable application to fabricate.
            (
                std::borrow::Cow::Borrowed(site.parameters(program, caller.symbol)?),
                0,
            )
        } else {
            let owner = selected_call_site_contract(program, caller.symbol, site)?;
            let parameters = call_parameter_scope(program, caller.symbol, &owner, site)?;
            (parameters, owner_leading_operands(&owner))
        };
        if arguments.len() != leading + parameters.len() {
            return None;
        }
        let references = roots
            .iter()
            .enumerate()
            .map(|(position, root)| {
                if position < leading {
                    crate::expression_result_type_reference(program, caller, state, *root)
                } else {
                    parameters
                        .get(position - leading)
                        .map(|parameter| parameter.type_reference)
                }
            })
            .collect::<Option<Vec<_>>>()?;
        // Ambiguous supplied roots cannot choose between different owners.
        if selected.replace(references).is_some() {
            return None;
        }
    }
    if selected.is_some() {
        return selected;
    }
    // Value and guard targets have their own semantic receiving roles. An
    // unavailable named telescope cannot fall through to the state's result.
    let [root] = roots else {
        return None;
    };
    let inventory = program.tables.expression_roots(statement);
    let mut owners = inventory
        .iter()
        .filter(|(_, expression)| expression == root);
    let (role, _) = owners.next()?;
    if owners.next().is_some() {
        return None;
    }
    let reference = match role {
        typed_trees::statement::StatementRootRole::TransitionValue => state.return_type,
        typed_trees::statement::StatementRootRole::TransitionGuard => {
            crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                program,
                symbols::BuiltinTypeAtom::Bool,
            )?
        }
        _ => return None,
    };
    Some(vec![reference])
}
