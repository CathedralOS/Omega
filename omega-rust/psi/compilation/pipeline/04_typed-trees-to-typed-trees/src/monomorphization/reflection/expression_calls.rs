//! A reflection expression invokes an ordinary Unit body at its original
//! evaluation point. The existing statement elaborator owns its field calls;
//! lifting the sequence into a callable keeps conditional arms conditional and
//! evaluates each actual once, without hoisting a mutable loan across a guard.
use super::*;
use typed_trees::{machine::Machine, signature::StateParameter, state::State};

#[allow(clippy::too_many_arguments)]
pub(super) fn materialize(
    program: &mut TypedTrees,
    owner_symbol: SymbolHandle,
    source_state: SymbolHandle,
    expression: ExpressionHandle,
    subject: TypeReferenceHandle,
    context: TypeReferenceHandle,
    inspection: bool,
    access: ReferenceAccess,
) -> Result<(), Vec<Diagnostic>> {
    let ExpressionNode::Call(original) = expressions_of(program).expression(expression).clone()
    else {
        return Err(error("typed field query lost its expression invocation"));
    };
    let owner = program
        .machine_by_symbol(owner_symbol)
        .cloned()
        .ok_or_else(|| error("typed field query lost its lexical machine"))?;
    let actuals = expressions_of(program)
        .expression_handles(original.arguments)
        .to_vec();
    let context_type = reference(program, context, ReferenceAccess::Mutable);
    let mut parameters = Vec::new();
    if !inspection {
        parameters.push(("value", reference(program, subject, access), access));
    }
    parameters.push(("context", context_type, ReferenceAccess::Mutable));
    if actuals.len() != parameters.len() {
        return Err(error("typed field query has invalid expression operands"));
    }
    let helper_name = format!(
        "__reflect_query#{}#{}",
        source_state.arena_index(),
        expression.arena_index()
    );
    // The derivation retains the lexical author, not the core query's package.
    // The next round therefore checks exactly the original structural authority.
    let helper_symbol =
        program
            .symbols
            .insert_generated_root_from(source_state, SymbolKind::Machine, &helper_name);
    let entry_symbol = program
        .symbols
        .insert_generated_children(helper_symbol, [(SymbolKind::State, "entry")])
        .start();
    let mut helper = Machine {
        symbol: helper_symbol,
        name: Identifier::generated(&helper_name),
        service_reach_row: owner.service_reach_row,
        service_reach_is_installation_bound: owner.service_reach_is_installation_bound,
        target: owner.target.clone(),
        authored_target: owner.authored_target.clone(),
        suspends: owner.suspends,
        blocks: owner.blocks,
        body_is_present: true,
        body_status: owner.body_status,
        ..Default::default()
    };
    let mut entry = State {
        symbol: entry_symbol,
        name: Identifier::generated("entry"),
        // Match an ordinary source machine with no authored result. Its Unit
        // completion uses the zero return handle; manufacturing a declared
        // value type here makes Unit-valued match arms appear heterogeneous.
        return_type: TypeReferenceHandle::invalid(),
        ..Default::default()
    };
    let symbols = program.symbols.insert_generated_children(
        entry_symbol,
        parameters
            .iter()
            .map(|(label, _, _)| (SymbolKind::Parameter, *label)),
    );
    let mut operands = Vec::new();
    let mut arguments = Vec::new();
    for (position, ((label, parameter_type, access), actual)) in
        parameters.into_iter().zip(actuals).enumerate()
    {
        let symbol = symbols.handle_at(position as u32);
        program.push_state_parameter(
            &mut entry,
            StateParameter {
                symbol,
                name: Identifier::generated(label),
                type_reference: parameter_type,
                ..Default::default()
            },
        );
        operands.push(name(program, symbol));
        arguments.push(reborrow_operand(program, actual, access));
    }
    let source_span = original
        .authored_call_selection
        .and_then(|occurrence| program.authored_declaration_selections().get(occurrence))
        .map_or_else(
            || expressions_of(program).source_span(expression),
            |selection| selection.source_span(),
        );
    let query = StatementNode::Call(TableCall {
        target_symbol: original.target_symbol,
        target: original.target.clone(),
        authored_machine_application: original.authored_machine_application,
        static_machine_parameter: original.static_machine_parameter,
        static_requirement_dispatch: original.static_requirement_dispatch.clone(),
        machine_arguments: original.machine_arguments.clone(),
        arguments: program.statement_table.insert_expression_handles(operands),
        evidence_arguments: original.evidence_arguments.clone(),
        operational_acknowledgement: original.operational_acknowledgement,
        authored_call_selection: original.authored_call_selection,
        source_span,
        ..Default::default()
    });
    entry.statement_nodes = program.statement_table.insert_statements([query]);
    program.push_machine_state(&mut helper, entry);
    program.push_machine(helper);
    let arguments = expressions_of_mut(program).insert_expression_handles(arguments);
    let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(expression) else {
        return Err(error("typed field query lost its receiving expression"));
    };
    call.target_symbol = entry_symbol;
    call.target = Identifier::generated("entry");
    call.receiver = ExpressionHandle::invalid();
    call.machine_arguments = Box::default();
    // The retained query owns its static callback application. The generated
    // Unit entry has no generic telescope: replaying that application against
    // this entry would incorrectly demand a callback for every possible Kind
    // instead of the actual field applications the shared elaborator checks.
    call.authored_machine_application = Default::default();
    call.static_machine_parameter = SymbolHandle::invalid();
    call.static_requirement_dispatch = None;
    call.arguments = arguments;
    Ok(())
}
