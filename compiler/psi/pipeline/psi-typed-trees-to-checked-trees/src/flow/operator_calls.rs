use super::*;

pub(super) struct ResolvedOperatorStatementCall<'program> {
    pub(super) operator: &'program psi_typed_trees::operator::OperatorDefinition,
    pub(super) receiver_is_value: bool,
}

/// Resolve the operator/boundary definition a named statement call dispatches
/// to. Statement operators do not produce ordinary borrow-call facts, so every
/// consumer (ownership, mutation invalidation, and postcondition flow) must use
/// this one resolution rule rather than guessing independently.
pub(super) fn resolve_operator_statement_call<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    call: &psi_typed_trees::statement::TableCall,
) -> Option<ResolvedOperatorStatementCall<'program>> {
    // A valid receiver symbol that resolves to a typed value (a local,
    // parameter, or field) is a method-form receiver place; a static path
    // receiver (`Text::validate(...)`) names no runtime value.
    let receiver_is_value = call.receiver_symbol.is_valid()
        && symbol_type_symbol(program, call.receiver_symbol).is_some();
    let receiver_segments: Vec<&str> = if receiver_is_value {
        Vec::new()
    } else {
        program
            .statement_table
            .name_path_members(call.receiver)
            .iter()
            .map(|identifier| identifier.as_str())
            .collect()
    };
    let argument_count = program
        .statement_table
        .expression_handles(call.arguments)
        .len();

    let static_receiver_segments =
        (!receiver_segments.is_empty()).then_some(receiver_segments.as_slice());
    resolve_operator_for_call(
        program,
        call.target_symbol,
        static_receiver_segments,
        call.target.as_str(),
        argument_count,
        receiver_is_value,
    )
    .map(|operator| ResolvedOperatorStatementCall {
        operator,
        receiver_is_value,
    })
}

/// Resolve an operator call from its already-separated path and arity facts.
/// Expression-call ownership and statement-call flow share this exact rule.
pub(super) fn resolve_operator_for_call<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
    static_receiver_segments: Option<&[&str]>,
    target_name: &str,
    argument_count: usize,
    has_value_receiver: bool,
) -> Option<&'program psi_typed_trees::operator::OperatorDefinition> {
    psi_typed_trees::operator::resolve_named_call(
        program,
        target_symbol,
        static_receiver_segments,
        target_name,
        argument_count,
        has_value_receiver,
    )
}

#[derive(Debug, Clone)]
struct OperatorStatementOperand {
    parameter_symbol: SymbolHandle,
    is_mutable: bool,
    label: String,
    place: Option<CanonicalPlace>,
}

fn operator_statement_operands<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call: &psi_typed_trees::statement::TableCall,
) -> Option<(
    &'program psi_typed_trees::operator::OperatorDefinition,
    Vec<OperatorStatementOperand>,
)> {
    let resolved = resolve_operator_statement_call(program, call)?;
    let parameters = program.operator_parameters(resolved.operator);
    let receiver_parameter_index = parameters
        .iter()
        .position(|parameter| parameter.is_self)
        .or_else(|| {
            resolved
                .receiver_is_value
                .then(|| parameters.iter().position(|parameter| !parameter.is_self))
                .flatten()
        });
    let receiver_place = receiver_parameter_index.and_then(|_| {
        crate::flow::mutation::canonical_receiver_place_for_call_site(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            &CallSite::Statement(call),
        )
    });
    let arguments = program.statement_table.expression_handles(call.arguments);
    let mut argument_index = 0usize;
    let mut operands = Vec::with_capacity(parameters.len());

    for (parameter_index, parameter) in parameters.iter().enumerate() {
        let (place, label) = if receiver_parameter_index == Some(parameter_index) {
            let label = receiver_place.as_ref().map_or_else(
                || {
                    psi_typed_trees::expression::display_name_path(
                        program.statement_table.name_path_members(call.receiver),
                        "::",
                    )
                },
                |place| {
                    crate::labels::canonical_place_label_from_parts(
                        program,
                        place.root,
                        &place.segments,
                    )
                },
            );
            (receiver_place.clone(), label)
        } else {
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            let place = argument.and_then(|expression| {
                canonical_place_from_expression_in_state(
                    program,
                    caller_state_symbol,
                    statement_index,
                    expression,
                )
                .or_else(|| canonical_place_from_expression(program, expression))
            });
            let label = argument.map_or_else(
                || parameter.name.to_string(),
                |expression| program.expression_table.display_name(expression),
            );
            (place, label)
        };
        operands.push(OperatorStatementOperand {
            parameter_symbol: parameter.symbol,
            is_mutable: parameter.is_mutable,
            label,
            place,
        });
    }

    Some((resolved.operator, operands))
}

/// A named boundary/operator call has no ordinary borrow-call fact. Its mutable
/// operands still invalidate every fact depending on those places before the
/// operator's postconditions are introduced.
pub(super) fn operator_statement_call_mutated_places(
    program: &psi_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call: &psi_typed_trees::statement::TableCall,
) -> Vec<CanonicalPlace> {
    operator_statement_operands(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        statement_index,
        call,
    )
    .map(|(_, operands)| {
        operands
            .into_iter()
            .filter_map(|operand| operand.is_mutable.then_some(operand.place).flatten())
            .collect()
    })
    .unwrap_or_default()
}

/// Introduce domain memberships guaranteed by a named boundary/operator call
/// after its mutable operands have invalidated the pre-call context. The fact's
/// place is instantiated from the operator parameter onto the exact caller
/// operand, including any relative field/index segments.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_operator_statement_ensures(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call: &psi_typed_trees::statement::TableCall,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let Some((operator, operands)) = operator_statement_operands(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        statement_index,
        call,
    ) else {
        return;
    };
    let point = ProgramPoint::Statement {
        machine_symbol: caller_machine_symbol,
        state_symbol: caller_state_symbol,
        statement_index,
    };
    let parameters = program.operator_parameters(operator);
    let operand_labels = operands
        .iter()
        .map(|operand| operand.label.clone())
        .collect::<Vec<_>>();
    let mut refs = HandleSpan::empty();

    for contract in program
        .signature_contracts
        .span_or_empty(operator.contracts)
    {
        if contract.kind != psi_typed_trees::signature::SignatureContractKind::Ensures {
            continue;
        }
        for offset in 0..contract.facts.count() {
            let fact_handle = Handle::from_parts(
                contract
                    .facts
                    .start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("operator contract fact handle overflow"),
                contract.facts.start().generation(),
            );
            match program.proof_facts.get(fact_handle) {
                psi_typed_trees::domain::ProofFact::Membership(membership) => {
                    let Some(relative) =
                        operator_contract_relative_place(program, operator, membership.value)
                    else {
                        continue;
                    };
                    let psi_facts::PlaceRoot::Symbol(parameter_symbol) = relative.root else {
                        continue;
                    };
                    let Some(operand) = operands
                        .iter()
                        .find(|operand| operand.parameter_symbol == parameter_symbol)
                    else {
                        continue;
                    };
                    let Some(mut place) = operand.place.clone() else {
                        continue;
                    };
                    place.extend_segments(&relative.segments);
                    let place = crate::semantic_places::append_place_with_segments(
                        semantic,
                        place.root,
                        &place.segments,
                    );
                    let payload = FactPayload::ContractDomainMembership {
                        kind: semantic_contract_fact_kind(ContractProofFactKind::Ensures),
                        fact: fact_handle,
                        value: membership.value,
                        domain: membership.domain,
                        domain_symbol: membership.domain_symbol,
                    };
                    let fact = semantic.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin: FactOrigin::OperatorEnsures {
                            operator_symbol: operator.symbol,
                        },
                        evidence: crate::qualification_evidence::operator_contract_evidence(
                            program,
                            operator.symbol,
                            payload,
                        ),
                        payload,
                    });
                    semantic.append_ref(&mut refs, fact);
                }
                psi_typed_trees::domain::ProofFact::Expression(expression) => {
                    let label =
                        crate::labels::instantiate_operator_contract_expression_label_with_labels(
                            program,
                            parameters,
                            &operand_labels,
                            *expression,
                        );
                    let instantiated = semantic.append_instantiated_expression(label);
                    let mut dependencies = operands
                        .iter()
                        .filter_map(|operand| operand.place.clone())
                        .collect::<Vec<_>>();
                    dependencies.dedup();

                    if dependencies.is_empty() {
                        let fact = semantic.append_fact(Fact {
                            place: FactPlace::Unknown,
                            point,
                            origin: FactOrigin::OperatorEnsures {
                                operator_symbol: operator.symbol,
                            },
                            evidence: QualificationEvidence::default(),
                            payload: FactPayload::ContractBooleanExpression {
                                kind: semantic_contract_fact_kind(ContractProofFactKind::Ensures),
                                fact: fact_handle,
                                expression: *expression,
                                instantiated,
                            },
                        });
                        semantic.append_ref(&mut refs, fact);
                    } else {
                        for dependency in dependencies {
                            let place = crate::semantic_places::append_place_with_segments(
                                semantic,
                                dependency.root,
                                &dependency.segments,
                            );
                            let fact = semantic.append_fact(Fact {
                                place: FactPlace::Place(place),
                                point,
                                origin: FactOrigin::OperatorEnsures {
                                    operator_symbol: operator.symbol,
                                },
                                evidence: QualificationEvidence::default(),
                                payload: FactPayload::ContractBooleanExpression {
                                    kind: semantic_contract_fact_kind(
                                        ContractProofFactKind::Ensures,
                                    ),
                                    fact: fact_handle,
                                    expression: *expression,
                                    instantiated,
                                },
                            });
                            semantic.append_ref(&mut refs, fact);
                        }
                    }
                }
                psi_typed_trees::domain::ProofFact::Proposition(application) => {
                    let place = semantic.append_symbol_place(application.proposition);
                    let binder_labels = application
                        .binder_arguments
                        .iter()
                        .map(|argument| argument.display_name())
                        .collect::<Vec<_>>();
                    let argument_labels = program
                        .expression_table
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|argument| {
                            crate::labels::instantiate_operator_contract_expression_label_with_labels(
                                program,
                                parameters,
                                &operand_labels,
                                *argument,
                            )
                        })
                        .collect::<Vec<_>>();
                    let instantiated = program
                        .normalize_proposition_application_with_labels(
                            application,
                            &binder_labels,
                            &argument_labels,
                        )
                        .map(|formula| {
                            semantic.append_instantiated_expression(formula.identity_label())
                        })
                        .unwrap_or_else(psi_arena::Handle::invalid);
                    let fact = semantic.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin: FactOrigin::OperatorEnsures {
                            operator_symbol: operator.symbol,
                        },
                        evidence: QualificationEvidence::default(),
                        payload: FactPayload::ContractPropositionApplication {
                            kind: semantic_contract_fact_kind(ContractProofFactKind::Ensures),
                            fact: fact_handle,
                            proposition: application.proposition,
                            instantiated,
                        },
                    });
                    semantic.append_ref(&mut refs, fact);
                }
            }
        }
    }

    if refs.is_empty() {
        return;
    }
    let context = semantic.append_context(point, refs);
    let mut next_contexts =
        clone_flow_contexts(&mut ctx.contexts.semantic_context_refs, *active_contexts);
    ctx.contexts
        .semantic_context_refs
        .append_to_span(&mut next_contexts, FlowSemanticContextRef { context });
    *active_contexts = next_contexts;
    let mut next_constraints =
        clone_constraint_refs(&mut ctx.contexts.constraint_refs, *active_constraints);
    append_constraint_ref(
        &mut ctx.contexts.constraint_refs,
        &mut next_constraints,
        FlowConstraintKind::SemanticContext { context },
    );
    *active_constraints = next_constraints;
}

/// Canonicalize a place spelled inside an operator contract relative to that
/// operator's parameters. Contract name paths are intentionally not resolved
/// as caller symbols, so the parameter's authored name is the fallback root.
fn operator_contract_relative_place(
    program: &psi_typed_trees::TypedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            operator_contract_relative_place(program, operator, *inner)
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let head = members.first()?.as_str();
            let parameter = program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| {
                    parameter.name.as_str() == head
                        || (path.head_symbol.is_valid() && parameter.symbol == path.head_symbol)
                        || (path.symbol.is_valid() && parameter.symbol == path.symbol)
                })?;
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let mut segments = Vec::with_capacity(members.len().saturating_sub(1));
            for offset in 1..members.len() {
                let symbol = member_symbols
                    .get(offset)
                    .copied()
                    .filter(|symbol| symbol.is_valid())?;
                crate::flow::push_field_place_segments(program, &mut segments, symbol);
            }
            Some(CanonicalPlace {
                root: psi_facts::PlaceRoot::Symbol(parameter.symbol),
                segments,
            })
        }
        ExpressionNode::Member(member) => {
            let mut place = operator_contract_relative_place(program, operator, member.receiver)?;
            let symbol = member
                .member_symbol
                .is_valid()
                .then_some(member.member_symbol)?;
            crate::flow::push_field_place_segments(program, &mut place.segments, symbol);
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place =
                operator_contract_relative_place(program, operator, indexed.collection)?;
            place
                .segments
                .push(crate::flow::index_place_segment(program, indexed.index));
            Some(place)
        }
        _ => None,
    }
}
