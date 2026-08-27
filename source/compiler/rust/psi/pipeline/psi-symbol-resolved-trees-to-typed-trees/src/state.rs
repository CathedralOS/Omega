use crate::domain::lower_proof_facts;
use crate::lowerer::Lowerer;
use crate::statement::lower_statement_node;
use crate::type_reference::lower_type_reference_into_table;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_state(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
) -> Result<typed::state::State, Diagnostic> {
    let mut typed_state = typed::state::State {
        symbol: state.symbol,
        name: crate::name::lower_name(&state.name),
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
        psi_symbols::SymbolHandle,
        typed::name::Identifier,
        psi_symbols::SymbolHandle,
        String,
    )> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(state.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        for (domain_symbol, domain_full_name) in
            domain_constraints(&lowerer.typed_trees, parameter.type_reference)
        {
            domain_constrained_parameters.push((
                parameter.symbol,
                parameter.name.clone(),
                domain_symbol,
                domain_full_name,
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
                binding: contract.binding.as_ref().map(crate::name::lower_name),
                facts,
                token_count: contract.token_count,
            },
        );
    }

    for (param_symbol, param_name, domain_symbol, domain_full_name) in domain_constrained_parameters
    {
        let contract = build_domain_membership_contract(
            lowerer,
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
        );
        lowerer
            .typed_trees
            .push_state_contract(&mut typed_state, contract);
    }

    for statement in lowerer
        .source_trees
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    {
        if let resolved::statement::StatementNode::ProofOutputBindingStatement(package) = statement
        {
            let call = crate::expression::lower_expression_handle(lowerer, package.call)?;
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
                            output_field: crate::name::lower_name(&binding.output_field),
                            binding: crate::name::lower_name(&binding.binding),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    call,
                });
            continue;
        }
        let statement = lower_statement_node(lowerer, attached_data, state, statement)?;
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
        && (machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
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
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
        {
            call_occurrences.push(occurrence);
        }
    }
    let authored_call_selection = match call.operational_acknowledgement.origin {
        psi_language_semantics::CallOperationalAcknowledgementOrigin::Source => {
            let [occurrence] = call_occurrences.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "proof-output call retains {} authored call selections; expected one",
                    call_occurrences.len(),
                )));
            };
            Some(*occurrence)
        }
        psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => None,
    };
    let (receiver_symbol, receiver_members) =
        proof_output_receiver_parts(&lowerer.typed_trees, call.receiver)
            .ok_or_else(|| Diagnostic::error("proof-output call receiver must be a name path"))?;
    let mut receiver = psi_arena::HandleSpan::empty();
    for member in receiver_members {
        lowerer
            .typed_trees
            .statement_table
            .push_name_path_member(&mut receiver, member);
    }
    Ok(typed::statement::TableCall {
        receiver_symbol,
        target_symbol: call.target_symbol,
        receiver,
        target: call.target,
        machine_arguments: call.machine_arguments,
        arguments: call.arguments,
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
) -> Option<(psi_symbols::SymbolHandle, Vec<typed::name::Identifier>)> {
    if !expression.is_valid() {
        return Some((psi_symbols::SymbolHandle::invalid(), Vec::new()));
    }
    match program.expression_table.expression(expression) {
        typed::expression::ExpressionNode::Borrow(inner) => {
            proof_output_receiver_parts(program, inner.target)
        }
        typed::expression::ExpressionNode::Name(path) => Some((
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .to_vec(),
        )),
        typed::expression::ExpressionNode::Member(member) => {
            let (_, mut members) = proof_output_receiver_parts(program, member.receiver)?;
            members.push(member.member.clone());
            Some((member.member_symbol, members))
        }
        _ => None,
    }
}

pub(crate) fn lower_state_signature(
    lowerer: &mut Lowerer,
    signature: &resolved::signature::StateSignature,
) -> Result<typed::signature::StateSignature, Diagnostic> {
    let mut typed_signature = typed::signature::StateSignature {
        symbol: signature.symbol,
        name: crate::name::lower_name(&signature.name),
        spelling: signature.spelling,
        lifetime_parameters: signature
            .lifetime_parameters
            .iter()
            .map(crate::name::lower_name)
            .collect(),
        type_parameters: Default::default(),
        is_default: signature.is_default,
        parameters: Default::default(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(typed::types::TypeReferenceHandle::invalid),
        invokes: Default::default(),
        service_reach_row: signature.service_reach_row,
        service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
        suspends: signature.suspends,
        blocks: signature.blocks,
        contracts: Default::default(),
        // The final subject-bearing guarantee is normalized after all typed
        // domains, contracts, and requirements are available.
        termination_guarantee: if signature.terminates_guarantee {
            psi_language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            }
        } else {
            psi_language_semantics::TerminationGuarantee::NoGuarantee
        },
    };

    for parameter in lowerer
        .source_trees
        .data_type_parameters(signature.type_parameters)
    {
        let parameter = crate::data::lower_type_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .data_type_parameters
            .append_to_span(&mut typed_signature.type_parameters, parameter);
    }

    // #66/DOM1/P1a: collect every declared domain on constrained parameters.
    // Each desugars below into its own implicit `requires <param> in <domain>`
    // membership contract. Predicate-bearing domains discharge by proof;
    // bodyless domains require retained qualification evidence.
    let mut domain_constrained_parameters: Vec<(
        psi_symbols::SymbolHandle,
        typed::name::Identifier,
        psi_symbols::SymbolHandle,
        String,
    )> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(signature.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        for (domain_symbol, domain_full_name) in
            domain_constraints(&lowerer.typed_trees, parameter.type_reference)
        {
            domain_constrained_parameters.push((
                parameter.symbol,
                parameter.name.clone(),
                domain_symbol,
                domain_full_name,
            ));
        }
        lowerer
            .typed_trees
            .push_state_signature_parameter(&mut typed_signature, parameter);
    }

    for binding in lowerer.source_trees.signature_invokes(signature.invokes) {
        lowerer
            .typed_trees
            .push_state_signature_invoke(&mut typed_signature, crate::name::lower_name(binding));
    }

    for contract in lowerer
        .source_trees
        .signature_contracts(signature.contracts)
    {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_state_signature_contract(
            &mut typed_signature,
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
                binding: contract.binding.as_ref().map(crate::name::lower_name),
                facts,
                token_count: contract.token_count,
            },
        );
    }

    // #66/DOM1/P1a: desugar each declared domain into an implicit `requires
    // <param> in <domain>` membership contract (here on a trait/platform
    // signature; the regular-machine path is `lower_machine`).
    for (param_symbol, param_name, domain_symbol, domain_full_name) in domain_constrained_parameters
    {
        let contract = build_domain_membership_contract(
            lowerer,
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
        );
        lowerer
            .typed_trees
            .push_state_signature_contract(&mut typed_signature, contract);
    }

    Ok(typed_signature)
}

/// Every normalized declared domain on a parameter type, looking through a
/// leading reference. Arithmetic policy is represented by a distinct
/// constraint node and does not become a membership contract.
pub(crate) fn domain_constraints(
    typed_trees: &typed::TypedTrees,
    type_reference: typed::types::TypeReferenceHandle,
) -> Vec<(psi_symbols::SymbolHandle, String)> {
    match typed_trees
        .type_reference_table
        .type_reference(type_reference)
    {
        typed::types::TypeReferenceNode::Reference { referee, .. } => {
            domain_constraints(typed_trees, *referee)
        }
        typed::types::TypeReferenceNode::Constrained { constraints, .. } => typed_trees
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                typed::types::TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                    typed_trees
                        .domain_definitions()
                        .iter()
                        .find(|definition| definition.symbol == domain.symbol)
                        .map(|definition| (domain.symbol, definition.name.as_str().to_owned()))
                }
                typed::types::TypeConstraintNode::Domain(domain)
                    if psi_language_semantics::CarryPermission::from_name(domain.name.as_str())
                        .is_some() =>
                {
                    Some((
                        psi_symbols::SymbolHandle::invalid(),
                        domain.name.as_str().to_owned(),
                    ))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Build the implicit `requires <param> in <domain>` membership contract for a
/// normalized declared-domain constraint. The caller attaches the contract at
/// the right level (machine vs trait signature).
pub(crate) fn build_domain_membership_contract(
    lowerer: &mut Lowerer,
    param_symbol: psi_symbols::SymbolHandle,
    param_name: typed::name::Identifier,
    domain_symbol: psi_symbols::SymbolHandle,
    domain_full_name: &str,
) -> typed::signature::SignatureContract {
    let mut members = psi_arena::HandleSpan::empty();
    lowerer
        .typed_trees
        .expression_table
        .push_name_path_member(&mut members, param_name);
    let mut member_symbols = psi_arena::HandleSpan::empty();
    lowerer
        .typed_trees
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, param_symbol);
    let value =
        lowerer
            .typed_trees
            .expression_table
            .insert(typed::expression::ExpressionNode::Name(
                typed::expression::TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: param_symbol,
                    symbol: param_symbol,
                },
            ));

    let mut domain = psi_arena::HandleSpan::empty();
    for part in domain_full_name.split("::") {
        lowerer
            .typed_trees
            .domain_path_members
            .append_to_span(&mut domain, typed::name::Identifier::generated(part));
    }

    let mut facts = psi_arena::HandleSpan::empty();
    lowerer.typed_trees.proof_facts.append_to_span(
        &mut facts,
        typed::domain::ProofFact::Membership(typed::domain::ProofMembershipFact {
            value,
            domain,
            domain_symbol,
        }),
    );

    typed::signature::SignatureContract {
        kind: typed::signature::SignatureContractKind::Requires,
        keyword_source_span: None,
        binding: None,
        facts,
        token_count: 0,
    }
}

pub(crate) fn lower_state_parameter(
    lowerer: &mut Lowerer,
    parameter: &resolved::signature::StateParameter,
) -> Result<typed::signature::StateParameter, Diagnostic> {
    let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
    crate::domain_constraints::normalize_domain_constraints_for_type(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
    )?;
    Ok(typed::signature::StateParameter {
        symbol: parameter.symbol,
        name: crate::name::lower_name(&parameter.name),
        type_reference,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    })
}
