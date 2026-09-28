use crate::contracts::parameter_domains::{build_domain_membership_contract, domain_constraints};
use crate::contracts::proof_facts::lower_proof_facts;
use crate::expressions::statement::lower_statement_node;
use crate::lowerer::Lowerer;
use crate::signatures::parameters::lower_state_parameter;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_state(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
) -> Result<typed::state::State, Diagnostic> {
    let mut typed_state = typed::state::State {
        symbol: state.symbol,
        name: crate::lowerer::name::lower_name(&state.name),
        parameters: Default::default(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(typed::types::TypeReferenceHandle::invalid),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };

    // #66/DOM1/P1a: a constrained parameter is an obligation on exactly this
    // callable state boundary. Keeping the synthesized contract state-local is
    // essential for graph machines: a qualification introduced in one state
    // must not become a prerequisite of the machine's unrelated entry state.
    let mut domain_constrained_parameters: Vec<(
        symbols::SymbolHandle,
        typed::name::Identifier,
        symbols::SymbolHandle,
        String,
        Vec<typed::types::TypeReferenceHandle>,
        language_semantics::SemanticDomainId,
    )> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(state.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        for (domain_symbol, domain_full_name, domain_arguments, semantic_domain) in
            domain_constraints(&lowerer.typed_trees, parameter.type_reference)
        {
            domain_constrained_parameters.push((
                parameter.symbol,
                parameter.name.clone(),
                domain_symbol,
                domain_full_name,
                domain_arguments,
                semantic_domain,
            ));
        }
        lowerer
            .typed_trees
            .push_state_parameter(&mut typed_state, parameter);
    }

    for contract in lowerer.source_trees.signature_contracts(state.contracts) {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_state_contract(
            &mut typed_state,
            typed::signature::SignatureContract {
                kind: match &contract.kind {
                    resolved::signature::SignatureContractKind::Requires => {
                        typed::signature::SignatureContractKind::Requires
                    }
                    resolved::signature::SignatureContractKind::Ensures => {
                        typed::signature::SignatureContractKind::Ensures
                    }
                    resolved::signature::SignatureContractKind::EnsuresForResultCase {
                        result_data,
                        result_case,
                    } => typed::signature::SignatureContractKind::EnsuresForResultCase {
                        result_data: *result_data,
                        result_case: *result_case,
                    },
                    resolved::signature::SignatureContractKind::Crashes { cause } => {
                        typed::signature::SignatureContractKind::Crashes {
                            cause: match cause {
                                resolved::signature::CrashCause::Trap => {
                                    typed::signature::CrashCause::Trap
                                }
                                resolved::signature::CrashCause::Abort => {
                                    typed::signature::CrashCause::Abort
                                }
                            },
                        }
                    }
                },
                keyword_source_span: contract.keyword_source_span,
                binding: contract
                    .binding
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                facts,
                token_count: contract.token_count,
            },
        );
    }

    for (
        param_symbol,
        param_name,
        domain_symbol,
        domain_full_name,
        domain_arguments,
        semantic_domain,
    ) in domain_constrained_parameters
    {
        let contract = build_domain_membership_contract(
            lowerer,
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
            domain_arguments,
            semantic_domain,
        );
        lowerer
            .typed_trees
            .push_state_contract(&mut typed_state, contract);
    }

    let statements = lowerer
        .source_trees
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes);
    for (index, statement) in statements.iter().enumerate() {
        let closes_run = !matches!(
            statements.get(index + 1),
            Some(resolved::statement::StatementNode::Transition(_))
        );
        if let resolved::statement::StatementNode::ProofOutputBindingStatement(package) = statement
        {
            let call =
                crate::expressions::expression::lower_expression_handle(lowerer, package.call)?;
            let runtime_call_statement_index = package
                .bindings
                .iter()
                .any(|binding| {
                    binding.output_field.as_str() == "value" && binding.binding.as_str() != "_"
                })
                .then(|| {
                    usize::try_from(typed_state.statement_nodes.count())
                        .expect("typed statement count fits usize")
                        .checked_sub(1)
                        .expect("a proof-output Type value has its synthesized local first")
                });
            let runtime_call_statement_index = if runtime_call_statement_index.is_some() {
                runtime_call_statement_index
            } else if proof_output_call_requires_execution(lowerer, call) {
                let statement_index = usize::try_from(typed_state.statement_nodes.count())
                    .expect("typed statement count fits usize");
                let call = proof_output_runtime_call(lowerer, call)?;
                lowerer.typed_trees.statement_table.push_statement(
                    &mut typed_state.statement_nodes,
                    typed::statement::StatementNode::Call(call),
                );
                Some(statement_index)
            } else {
                None
            };
            lowerer
                .typed_trees
                .proof_output_calls
                .push(typed::typed_trees::ProofOutputCall {
                    machine_symbol: package.machine_symbol,
                    state_symbol: package.state_symbol,
                    statement_index: typed_state.statement_nodes.count() as usize,
                    source_statement_index: package.statement_index,
                    runtime_call_statement_index,
                    bindings: package
                        .bindings
                        .iter()
                        .map(|binding| typed::typed_trees::ProofOutputSelector {
                            output_field: crate::lowerer::name::lower_name(&binding.output_field),
                            binding: crate::lowerer::name::lower_name(&binding.binding),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    call,
                });
            continue;
        }
        let statement = lower_statement_node(lowerer, attached_data, state, statement, closes_run)?;
        lowerer
            .typed_trees
            .statement_table
            .push_statement(&mut typed_state.statement_nodes, statement);
    }

    Ok(typed_state)
}

fn proof_output_call_requires_execution(
    lowerer: &Lowerer,
    call: typed::expression::ExpressionHandle,
) -> bool {
    let typed::expression::ExpressionNode::Call(call) =
        lowerer.typed_trees.expression_table.expression(call)
    else {
        return true;
    };
    let Some((machine, state)) = lowerer.source_trees.machines.iter().find_map(|machine| {
        lowerer
            .source_trees
            .machine_state_handles(machine.states)
            .iter()
            .map(|state| lowerer.source_trees.machine_state(*state))
            .find(|state| state.symbol == call.target_symbol)
            .map(|state| (machine, state))
    }) else {
        return true;
    };

    state.return_type.is_none()
        && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || lowerer
                .source_trees
                .machine_state_handles(machine.states)
                .len()
                != 1
            || !state.statement_nodes.is_empty())
}

fn proof_output_runtime_call(
    lowerer: &mut Lowerer,
    call_handle: typed::expression::ExpressionHandle,
) -> Result<typed::statement::TableCall, Diagnostic> {
    let source_span = lowerer
        .typed_trees
        .expression_table
        .source_span(call_handle);
    let typed::expression::ExpressionNode::Call(call) = lowerer
        .typed_trees
        .expression_table
        .expression(call_handle)
        .clone()
    else {
        return Err(Diagnostic::error(
            "proof-output binding requires a direct call",
        ));
    };
    let mut call_occurrences = Vec::new();
    for occurrence in lowerer
        .typed_trees
        .expression_table
        .authored_selection_occurrences(call_handle)
    {
        let Some(selection) = lowerer
            .typed_trees
            .authored_declaration_selections()
            .get(occurrence)
        else {
            return Err(Diagnostic::error(format!(
                "proof-output call retains unknown authored selection occurrence {}",
                occurrence.ordinal(),
            )));
        };
        if selection.kind()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
        {
            call_occurrences.push(occurrence);
        }
    }
    let authored_call_selection = match call.operational_acknowledgement.origin {
        language_semantics::CallOperationalAcknowledgementOrigin::Source => {
            let [occurrence] = call_occurrences.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "proof-output call retains {} authored call selections; expected one",
                    call_occurrences.len(),
                )));
            };
            Some(*occurrence)
        }
        language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => None,
    };
    let (receiver_root_symbol, receiver_symbol, receiver_members) =
        proof_output_receiver_parts(&lowerer.typed_trees, call.receiver)
            .ok_or_else(|| Diagnostic::error("proof-output call receiver must be a name path"))?;
    let mut receiver = arena::HandleSpan::empty();
    for member in receiver_members {
        lowerer
            .typed_trees
            .statement_table
            .push_name_path_member(&mut receiver, member);
    }
    // Expression and statement calls own distinct argument-list arenas. Keep
    // the expression identities, but allocate the list in its new owner.
    let arguments = lowerer
        .typed_trees
        .tables
        .statement_table
        .insert_expression_handles(
            lowerer
                .typed_trees
                .tables
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        );
    Ok(typed::statement::TableCall {
        receiver_root_symbol,
        receiver_symbol,
        target_symbol: call.target_symbol,
        receiver,
        target: call.target,
        static_machine_parameter: call.static_machine_parameter,
        static_requirement_dispatch: None,
        machine_arguments: call.machine_arguments,
        arguments,
        evidence_arguments: call.evidence_arguments,
        operational_acknowledgement: call.operational_acknowledgement,
        discards_result: false,
        source_span,
        authored_call_selection,
    })
}

fn proof_output_receiver_parts(
    program: &typed::TypedTrees,
    expression: typed::expression::ExpressionHandle,
) -> Option<(
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    Vec<typed::name::Identifier>,
)> {
    if !expression.is_valid() {
        return Some((
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::invalid(),
            Vec::new(),
        ));
    }
    match program.expression_table.expression(expression) {
        typed::expression::ExpressionNode::Borrow(inner) => {
            proof_output_receiver_parts(program, inner.target)
        }
        typed::expression::ExpressionNode::Name(path) => Some((
            path.head_symbol,
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .to_vec(),
        )),
        typed::expression::ExpressionNode::Member(member) => {
            let (root, _, mut members) = proof_output_receiver_parts(program, member.receiver)?;
            members.push(member.member.clone());
            Some((root, member.member_symbol, members))
        }
        _ => None,
    }
}
