//! Fresh instance storage, lexical symbol remapping, and retained source custody.
use super::{
    Diagnostic, ExpressionHandle, ExpressionNode, HandleSpan, ProofFact, StatementNode,
    SymbolHandle, SymbolKind, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
};
use crate::monomorphization::body_rewriting::{
    cloned_expression_roots, rebind_state_scoped_range_endpoints,
    reject_runtime_bound_static_occurrences, remap_machine_argument_symbols, rewrite_cloned_calls,
    statement_span_handles, substitute_cloned_type_parameters,
};
use crate::monomorphization::{
    Candidate, candidate_conformance_fingerprint_arguments,
    closed_operator_realizations_for_machine, collect_statement_expression_trees, const_arguments,
    const_values, remapped_symbol, resolve_specialized_receiver_calls,
    specialization_selection_report_fingerprint, specialized_attached_data,
};

pub(super) fn clone_specialized_machine(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    ordinal: usize,
    template_contract_report_fingerprint: u64,
    template_contract_commitment: typed_trees::typed_trees::MachineTemplateCommitment,
    canonical_template_contract_bytes: Vec<u8>,
    normalized_template_identity: String,
    accepted_template_commitment: Option<String>,
) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Diagnostic> {
    const_arguments::validate_bindings(source.unwrap_or(program), candidate)?;
    let source_machine =
        source.unwrap_or(program).machines()[candidate.template.machine_index].clone();
    let source_states = source
        .unwrap_or(program)
        .machine_states(&source_machine)
        .to_vec();
    let source_owned = source
        .unwrap_or(program)
        .machine_owned_data(&source_machine)
        .to_vec();
    let specialized_attached_data =
        specialized_attached_data(source.unwrap_or(program), candidate, &source_machine);
    let inherited_field_names = specialized_attached_data
        .as_ref()
        .map(|(name, _)| name)
        .into_iter()
        .flat_map(|attached_data| {
            source
                .unwrap_or(program)
                .data_definitions()
                .iter()
                .filter(move |data| data.name == *attached_data)
        })
        .flat_map(|data| source.unwrap_or(program).data_members(data))
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field.name.as_str().to_owned()),
            typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    // Arena indices are one-based, so the clone region begins one past the
    // last pre-existing node; bounding substitutions at count()+1 keeps an
    // already-materialized endpoint or binder sentinel out of the sweep.
    let type_start = program.type_reference_table.type_reference_count() + 1;
    let expression_start = program.expression_table.iter_expressions().count() + 1;

    let type_arguments: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| {
            source
                .unwrap_or(program)
                .display_type_reference(binding.expect("complete specialization"))
        })
        .collect();
    let type_identities: Vec<String> = candidate
        .type_bindings
        .iter()
        .map(|binding| {
            source
                .unwrap_or(program)
                .normalized_type_identity(binding.expect("complete specialization"))
                .into_string()
        })
        .collect();
    let const_arguments: Vec<String> = candidate
        .const_bindings
        .iter()
        .map(|binding| {
            source
                .unwrap_or(program)
                .display_type_reference(binding.expect("complete specialization"))
        })
        .collect();
    let const_identities: Vec<String> = candidate
        .const_bindings
        .iter()
        .map(|binding| {
            source
                .unwrap_or(program)
                .normalized_type_identity(binding.expect("complete specialization"))
                .into_string()
        })
        .collect();
    let machine_paths: Vec<String> = candidate
        .machine_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete specialization")
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect();
    let evidence_paths =
        candidate_conformance_fingerprint_arguments(source.unwrap_or(program), candidate);
    let selection_report_fingerprint = specialization_selection_report_fingerprint(
        &candidate.template.template_name,
        &type_identities,
        &const_identities,
        &machine_paths,
        &evidence_paths,
    );
    let generated_name = format!(
        "{}$specialized${selection_report_fingerprint:016x}${ordinal}",
        candidate.template.template_name
    );
    let machine_symbol = program.symbols.insert_generated_root_from(
        source_machine.symbol,
        SymbolKind::Machine,
        &generated_name,
    );

    let machine_children = program.symbols.insert_generated_children(
        machine_symbol,
        inherited_field_names
            .iter()
            .map(|name| (SymbolKind::Field, name.as_str()))
            .chain(
                source_owned
                    .iter()
                    .map(|item| (SymbolKind::Field, item.name.as_str())),
            )
            .chain(
                source_states
                    .iter()
                    .map(|state| (SymbolKind::State, state.name.as_str())),
            ),
    );
    let machine_children: Vec<SymbolHandle> =
        symbols::SymbolTableBuilder::child_handles(machine_children).collect();
    let mut next_child = machine_children.into_iter();
    let mut symbol_map = vec![(source_machine.symbol, machine_symbol)];
    let source_machine_children = source_machine
        .symbol
        .is_valid()
        .then(|| {
            source
                .unwrap_or(program)
                .symbols
                .child_handles(source_machine.symbol)
        })
        .flatten()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    for field_name in &inherited_field_names {
        let cloned_field = next_child.next().expect("inherited-field clone symbol");
        let source_fields = source_machine_children
            .iter()
            .copied()
            .filter(|symbol| {
                source.unwrap_or(program).symbols.get(*symbol).kind == SymbolKind::Field
                    && source.unwrap_or(program).symbols.name(*symbol) == field_name
            })
            .collect::<Vec<_>>();
        if let [source_field] = source_fields.as_slice() {
            symbol_map.push((*source_field, cloned_field));
        }
    }
    for item in &source_owned {
        symbol_map.push((
            item.symbol,
            next_child.next().expect("owned-data clone symbol"),
        ));
    }
    let state_symbols: Vec<(SymbolHandle, SymbolHandle)> = source_states
        .iter()
        .map(|state| (state.symbol, next_child.next().expect("state clone symbol")))
        .collect();
    symbol_map.extend(state_symbols.iter().copied());

    // A `Value` binder bound to a runtime argument is realized as an ordinary
    // trailing parameter on every cloned state: executable occurrences remap
    // to that parameter, and each rewritten call site appends the subject as
    // an ordinary argument. Static `const` and closed `Value` arguments keep
    // the literal substitution path and add no parameter.
    let realized_parameters: Vec<(
        SymbolHandle,
        typed_trees::name::Identifier,
        TypeReferenceHandle,
    )> = candidate
        .template
        .const_parameters
        .iter()
        .zip(&candidate.runtime_value_bindings)
        .filter_map(|((symbol, name, declared_type), runtime)| {
            runtime.as_ref().map(|_| {
                (
                    *symbol,
                    typed_trees::name::Identifier::generated(name.clone()),
                    *declared_type,
                )
            })
        })
        .collect();
    let mut state_realized_parameters: Vec<Vec<(SymbolHandle, SymbolHandle)>> = Vec::new();

    for (source_state, (_, state_symbol)) in source_states.iter().zip(state_symbols.iter()) {
        let parameters = source
            .unwrap_or(program)
            .state_parameters(source_state)
            .to_vec();
        let locals: Vec<_> = source
            .unwrap_or(program)
            .statement_table
            .statements(source_state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) => Some(local.clone()),
                _ => None,
            })
            .collect();
        let children = program.symbols.insert_generated_children(
            *state_symbol,
            parameters
                .iter()
                .map(|parameter| (SymbolKind::Parameter, parameter.name.as_str()))
                .chain(
                    realized_parameters
                        .iter()
                        .map(|(_, name, _)| (SymbolKind::Parameter, name.as_str())),
                )
                .chain(
                    locals
                        .iter()
                        .map(|local| (SymbolKind::Local, local.name.as_str())),
                ),
        );
        let mut children = symbols::SymbolTableBuilder::child_handles(children);
        for parameter in parameters {
            symbol_map.push((
                parameter.symbol,
                children.next().expect("state-parameter clone symbol"),
            ));
        }
        state_realized_parameters.push(
            realized_parameters
                .iter()
                .map(|(binder, _, _)| {
                    (
                        *binder,
                        children.next().expect("realized-parameter clone symbol"),
                    )
                })
                .collect(),
        );
        for local in locals {
            symbol_map.push((local.symbol, children.next().expect("local clone symbol")));
        }
    }

    let mut cloned = source_machine.clone();
    cloned.is_public = false;
    cloned.symbol = machine_symbol;
    cloned.name = typed_trees::name::Identifier::generated(generated_name);
    if let Some((attached_data, attached_data_symbol)) = specialized_attached_data {
        cloned.attached_data = Some(attached_data);
        cloned.attached_data_symbol = attached_data_symbol;
    } else {
        cloned.attached_data = None;
        cloned.attached_data_symbol = SymbolHandle::invalid();
    }
    cloned.type_parameters = HandleSpan::empty();
    // A newly concrete attachment owns concrete field identities. Its old
    // generic application cannot be used to project those fields. Otherwise
    // copy the retained application into the same substitution watermark as
    // the body, so it cannot keep the template's unspecialized binders.
    cloned.attached_data_application =
        if cloned.attached_data_symbol == source_machine.attached_data_symbol {
            copy_type_reference(
                source,
                program,
                source_machine.attached_data_application,
                &symbol_map,
            )
        } else {
            TypeReferenceHandle::invalid()
        };
    cloned.conformance_bounds.clear();
    cloned.owned_data = HandleSpan::empty();
    cloned.satisfies = HandleSpan::empty();
    cloned.invokes = HandleSpan::empty();
    for mut invocation in source
        .unwrap_or(program)
        .machine_invokes(&source_machine)
        .to_vec()
    {
        invocation.target = match invocation.target {
            typed_trees::signature::AuthoredInvocationTarget::Unresolved => {
                typed_trees::signature::AuthoredInvocationTarget::Unresolved
            }
            typed_trees::signature::AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                typed_trees::signature::AuthoredInvocationTarget::Parameter {
                    ordinal,
                    symbol: remapped_symbol(symbol, &symbol_map),
                }
            }
            typed_trees::signature::AuthoredInvocationTarget::Service(symbol) => {
                typed_trees::signature::AuthoredInvocationTarget::Service(remapped_symbol(
                    symbol,
                    &symbol_map,
                ))
            }
        };
        program.push_machine_invoke(&mut cloned, invocation);
    }
    let ranking_subjects = typed_trees::ranking::resolve_machine_witness_subjects(
        source.unwrap_or(program),
        &source_machine,
    )
    .unwrap_or_default()
    .into_iter()
    .map(|expression| copy_expression(source, program, expression, &symbol_map))
    .collect::<Vec<_>>();
    let ranking_view_arguments = typed_trees::ranking::resolve_machine_witness_view_arguments(
        source.unwrap_or(program),
        &source_machine,
    )
    .unwrap_or_default()
    .into_iter()
    .map(|expression| copy_expression(source, program, expression, &symbol_map))
    .collect::<Vec<_>>();
    let ranking_range = source
        .unwrap_or(program)
        .ranking_expression_custody_for(source_machine.symbol)
        .and_then(|custody| custody.rank_range)
        .map(|expression| copy_expression(source, program, expression, &symbol_map));
    if !ranking_subjects.is_empty() || !ranking_view_arguments.is_empty() || ranking_range.is_some()
    {
        program
            .ranking_expression_custody
            .push(typed_trees::ranking::RankingExpressionCustody {
                machine: machine_symbol,
                subjects: ranking_subjects,
                view_arguments: ranking_view_arguments,
                rank_range: ranking_range,
            });
    }
    cloned.contracts = HandleSpan::empty();
    cloned.states = HandleSpan::empty();

    let owned_symbol_offset = 1;
    for (index, source_item) in source_owned.iter().enumerate() {
        let mut item = source_item.clone();
        item.symbol = symbol_map[owned_symbol_offset + index].1;
        item.type_reference =
            copy_type_reference(source, program, source_item.type_reference, &symbol_map);
        item.initial_value =
            copy_expression(source, program, source_item.initial_value, &symbol_map);
        program.push_machine_owned_data(&mut cloned, item);
    }
    for conformance in source
        .unwrap_or(program)
        .machine_trait_conformances(&source_machine)
        .to_vec()
    {
        program.push_machine_trait_conformance(&mut cloned, conformance.clone());
    }
    for contract in source
        .unwrap_or(program)
        .machine_contracts(&source_machine)
        .to_vec()
    {
        let contract = copy_signature_contract(source, program, contract.clone(), &symbol_map);
        // A machine-level contract reads entry-state parameters, so a
        // runtime-bound `Value` binder inside its fact expressions denotes the
        // entry state's realized trailing parameter.
        if let Some(realized) = state_realized_parameters.first() {
            remap_contract_value_subjects(program, &contract, realized);
        }
        program.push_machine_contract(&mut cloned, contract);
    }

    for (state_index, (source_state, (_, fresh_symbol))) in
        source_states.iter().zip(state_symbols.iter()).enumerate()
    {
        let mut state = source_state.clone();
        state.symbol = *fresh_symbol;
        state.parameters = HandleSpan::empty();
        state.contracts = HandleSpan::empty();
        state.return_type =
            copy_type_reference(source, program, source_state.return_type, &symbol_map);
        state.statement_nodes = copy_statements(source, program, source_state.statement_nodes);
        {
            let tables = &mut program.tables;
            tables.statement_table.remap_symbols_in(
                state.statement_nodes,
                &mut tables.expression_table,
                &mut tables.type_reference_table,
                &symbol_map,
            );
        }
        // Executable uses of a runtime-bound `Value` binder read the realized
        // parameter. The plain map intentionally leaves binder symbols in type
        // and contract positions alone: those are static contexts, and the
        // substitution pass rejects them with a dedicated diagnostic.
        let realized = &state_realized_parameters[state_index];
        if !realized.is_empty() {
            for handle in statement_span_handles(state.statement_nodes) {
                let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                else {
                    continue;
                };
                for argument in call.machine_arguments.iter_mut() {
                    remap_machine_argument_symbols(argument, realized);
                }
            }
            for statement in program
                .statement_table
                .statements(state.statement_nodes)
                .to_vec()
            {
                let mut roots = Vec::new();
                collect_statement_expression_trees(program, &statement, &mut roots);
                for root in roots {
                    program.expression_table.remap_symbols_in(root, realized);
                }
            }
        }
        for source_parameter in source
            .unwrap_or(program)
            .state_parameters(source_state)
            .to_vec()
        {
            let mut parameter = source_parameter.clone();
            parameter.symbol = remapped_symbol(parameter.symbol, &symbol_map);
            parameter.type_reference = copy_type_reference(
                source,
                program,
                source_parameter.type_reference,
                &symbol_map,
            );
            program.push_state_parameter(&mut state, parameter);
        }
        for ((_, name, declared_type), (_, realized_symbol)) in realized_parameters
            .iter()
            .zip(state_realized_parameters[state_index].iter())
        {
            let type_reference = copy_type_reference(source, program, *declared_type, &symbol_map);
            program.push_state_parameter(
                &mut state,
                typed_trees::signature::StateParameter {
                    symbol: *realized_symbol,
                    name: name.clone(),
                    type_reference,
                    is_const: false,
                    is_mutable: false,
                    is_self: false,
                    relevance: language_core::BindingRelevance::Relevant,
                },
            );
        }
        rebind_realized_member_symbols(program, &state, &state_realized_parameters[state_index]);
        for contract in source
            .unwrap_or(program)
            .state_contracts(source_state)
            .to_vec()
        {
            let contract = copy_signature_contract(source, program, contract.clone(), &symbol_map);
            // A state-level contract reads this state's parameters; a
            // runtime-bound `Value` binder inside its fact expressions denotes
            // this state's realized trailing parameter.
            remap_contract_value_subjects(
                program,
                &contract,
                &state_realized_parameters[state_index],
            );
            program.push_state_contract(&mut state, contract);
        }
        program.push_machine_state(&mut cloned, state);
    }

    let proof_output_calls = source
        .unwrap_or(program)
        .proof_output_calls
        .iter()
        .filter(|call| call.machine_symbol == source_machine.symbol)
        .cloned()
        .collect::<Vec<_>>();
    for mut call in proof_output_calls {
        call.machine_symbol = cloned.symbol;
        call.state_symbol = remapped_symbol(call.state_symbol, &symbol_map);
        call.call = copy_expression(source, program, call.call, &symbol_map);
        program.proof_output_calls.push(call);
    }
    for state in program.machine_states(&cloned).to_vec() {
        for handle in statement_span_handles(state.statement_nodes) {
            let StatementNode::Call(call) = program.statement_table.statement(handle) else {
                continue;
            };
            let mut arguments = call.machine_arguments.clone();
            copy_static_argument_type_payloads(source, program, &mut arguments, &symbol_map);
            let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
                unreachable!();
            };
            call.machine_arguments = arguments;
        }
    }
    copy_cloned_expression_type_payloads(source, program, expression_start, &symbol_map);
    let cloned_expression_roots = cloned_expression_roots(program, &cloned);
    const_values::substitute(
        program,
        candidate,
        Some(expression_start),
        &cloned_expression_roots,
    )?;
    substitute_cloned_type_parameters(source, program, candidate, type_start)?;
    // Range endpoints are value positions, not static positions: a binder
    // surviving in one names the realized trailing parameter so the clone's
    // declared `-> u64[0..=Bound]` keeps qualifying the captured subject.
    rebind_state_scoped_range_endpoints(program, &cloned, &state_realized_parameters);
    reject_runtime_bound_static_occurrences(program, candidate, &cloned)?;
    // A transition between cloned states forwards the containing state's
    // realized `Value` subjects, in telescope order, as trailing ordinary
    // arguments — the same subjects a rewritten call site appends.
    let state_transition_subjects: Vec<Vec<(typed_trees::name::Identifier, SymbolHandle)>> =
        state_realized_parameters
            .iter()
            .map(|realized| {
                realized
                    .iter()
                    .zip(realized_parameters.iter())
                    .map(|((_, symbol), (_, name, _))| (name.clone(), *symbol))
                    .collect()
            })
            .collect();
    rewrite_cloned_calls(
        source,
        program,
        candidate,
        &state_symbols,
        &state_transition_subjects,
        expression_start,
        cloned.states,
    );
    resolve_specialized_receiver_calls(program, &cloned);
    let instance_symbol = cloned.symbol;
    let authored_service_reach_rows = source
        .unwrap_or(program)
        .authored_service_reach_rows_for(source_machine.symbol)
        .map(|row| typed_trees::signature::AuthoredServiceReachRow {
            owner: instance_symbol,
            keyword_source_spans: row.keyword_source_spans.clone(),
            targets: row
                .targets
                .iter()
                .map(
                    |target| typed_trees::signature::AuthoredServiceReachTarget {
                        service: remapped_symbol(target.service, &symbol_map),
                        source_span: target.source_span,
                    },
                )
                .collect(),
            installation_bound: row.installation_bound,
        })
        .collect::<Vec<_>>();
    program
        .authored_service_reach_rows
        .extend(authored_service_reach_rows);
    // Evidence forwarding is erased from executable statements before this
    // phase. Its lexical owner must follow the clone just like the body;
    // retaining only the template row leaves named outputs unassigned.
    let evidence_forwardings = source
        .unwrap_or(program)
        .evidence_forwardings
        .iter()
        .filter(|forwarding| forwarding.machine_symbol == source_machine.symbol)
        .map(|forwarding| {
            let mut forwarding = forwarding.clone();
            forwarding.machine_symbol = instance_symbol;
            forwarding.state_symbol = remapped_symbol(forwarding.state_symbol, &symbol_map);
            forwarding.source_conformance = forwarding
                .source_conformance
                .map(|symbol| remapped_symbol(symbol, &symbol_map));
            forwarding
        })
        .collect::<Vec<_>>();
    program.evidence_forwardings.extend(evidence_forwardings);
    program.push_machine(cloned);
    let operator_realizations = closed_operator_realizations_for_machine(program, instance_symbol)?;
    program
        .machine_specializations
        .push(typed_trees::typed_trees::MachineSpecialization {
            template: candidate.template.template_symbol,
            instance: instance_symbol,
            template_parameters: source_machine.type_parameters,
            type_arguments,
            const_arguments,
            type_argument_identities: type_identities,
            const_argument_identities: const_identities,
            machine_arguments: candidate
                .machine_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                .collect(),
            conformance_arguments: candidate
                .evidence_bindings
                .iter()
                .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                .collect(),
            inferred_conformance_arguments: candidate.inferred_conformance_arguments.clone(),
            conformance_applications: candidate
                .evidence_bindings
                .iter()
                .map(|binding| {
                    crate::conformance::conformance_applications::close_conformance_application(
                        source.unwrap_or(program),
                        binding.as_ref().expect("complete specialization"),
                    )
                    .expect("validated closed conformance application")
                })
                .chain(candidate.selected_bound_applications.iter().cloned())
                .collect(),
            operator_realizations,
            template_contract_report_fingerprint,
            template_contract_commitment,
            canonical_template_contract_bytes,
            normalized_template_identity,
            accepted_template_commitment,
            machine_argument_contract_report_fingerprints: Vec::new(),
            machine_argument_contract_commitments: Vec::new(),
            conformance_argument_report_fingerprints: Vec::new(),
            report_fingerprint: 0,
            commitment: typed_trees::typed_trees::MachineSpecializationCommitment::default(),
        });

    Ok(state_symbols)
}

// Existing cross-table copiers retain recursive graph shape and source spans.
// Staging visits only the selected graph, never the complete program arenas.
pub(super) fn copy_statements(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    statements: HandleSpan<StatementNode>,
) -> HandleSpan<StatementNode> {
    if let Some(source) = source {
        let tables = &mut program.tables;
        return tables.statement_table.copy_statement_nodes_deep_from(
            &source.statement_table,
            &source.expression_table,
            &mut tables.expression_table,
            &source.type_reference_table,
            &mut tables.type_reference_table,
            statements,
        );
    }
    let mut staging = typed_trees::typed_trees::TypedTreeTables::default();
    let staged = staging.statement_table.copy_statement_nodes_deep_from(
        &program.statement_table,
        &program.expression_table,
        &mut staging.expression_table,
        &program.type_reference_table,
        &mut staging.type_reference_table,
        statements,
    );
    let tables = &mut program.tables;
    tables.statement_table.copy_statement_nodes_deep_from(
        &staging.statement_table,
        &staging.expression_table,
        &mut tables.expression_table,
        &staging.type_reference_table,
        &mut tables.type_reference_table,
        staged,
    )
}

pub(super) fn copy_expression(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> ExpressionHandle {
    if !expression.is_valid() {
        return ExpressionHandle::invalid();
    }
    let copied = if let Some(source) = source {
        program
            .expression_table
            .copy_from(&source.expression_table, expression)
    } else {
        let mut staging = typed_trees::expression::ExpressionTable::default();
        let staged = staging.copy_from(&program.expression_table, expression);
        program.expression_table.copy_from(&staging, staged)
    };
    program.expression_table.remap_symbols_in(copied, symbols);
    copied
}

/// `ExpressionTable::copy_from` owns expression recursion but deliberately
/// cannot clone handles from the separate type-reference table. A specialized
/// machine is a new semantic graph, so copy cast/zero-value type payloads here
/// before binder substitution; otherwise a cloned cast would still point into
/// the authored template's type nodes, and substitution would mutate both.
pub(super) fn copy_cloned_expression_type_payloads(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    expression_start: usize,
    symbols: &[(SymbolHandle, SymbolHandle)],
) {
    let calls = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .filter_map(|(handle, expression)| match expression {
            ExpressionNode::Call(call) => Some((handle, call.machine_arguments.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (handle, mut arguments) in calls {
        copy_static_argument_type_payloads(source, program, &mut arguments, symbols);
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            unreachable!();
        };
        call.machine_arguments = arguments;
    }
    let cast_payloads = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            Some((
                handle,
                cast.target_type,
                cast.result_type,
                cast.semantic_domain_arguments,
            ))
        })
        .collect::<Vec<_>>();
    for (handle, target_type, result_type, arguments) in cast_payloads {
        let target_type = copy_type_reference(source, program, target_type, symbols);
        let result_type = copy_type_reference(source, program, result_type, symbols);
        let copied_arguments = source
            .unwrap_or(program)
            .type_reference_table
            .type_reference_handles(arguments)
            .to_vec()
            .into_iter()
            .map(|argument| copy_type_reference(source, program, argument, symbols))
            .collect::<Vec<_>>();
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles(copied_arguments);
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
            unreachable!("collected cast changed kind")
        };
        cast.target_type = target_type;
        cast.result_type = result_type;
        cast.semantic_domain_arguments = arguments;
    }

    let zero_values = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .filter_map(|(handle, expression)| {
            let ExpressionNode::ZeroValue(type_reference) = expression else {
                return None;
            };
            Some((handle, *type_reference))
        })
        .collect::<Vec<_>>();
    for (handle, type_reference) in zero_values {
        let type_reference = copy_type_reference(source, program, type_reference, symbols);
        let ExpressionNode::ZeroValue(current) = program.expression_table.expression_mut(handle)
        else {
            unreachable!("collected zero-value expression changed kind")
        };
        *current = type_reference;
    }
}

fn copy_static_argument_type_payloads(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    arguments: &mut [typed_trees::expression::StaticMachineArgument],
    symbols: &[(SymbolHandle, SymbolHandle)],
) {
    for argument in arguments {
        if argument.type_reference.is_valid() {
            argument.type_reference =
                copy_type_reference(source, program, argument.type_reference, symbols);
        }
        if let Some(application) = &mut argument.application {
            copy_static_argument_type_payloads(
                source,
                program,
                &mut application.arguments,
                symbols,
            );
        }
    }
}

pub(super) fn copy_type_reference(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> TypeReferenceHandle {
    if !type_reference.is_valid() {
        return TypeReferenceHandle::invalid();
    }
    let copied = if let Some(source) = source {
        let tables = &mut program.tables;
        tables.type_reference_table.copy_from(
            &source.type_reference_table,
            &source.expression_table,
            &mut tables.expression_table,
            type_reference,
        )
    } else {
        let mut staging = typed_trees::typed_trees::TypedTreeTables::default();
        let staged = staging.type_reference_table.copy_from(
            &program.type_reference_table,
            &program.expression_table,
            &mut staging.expression_table,
            type_reference,
        );
        let tables = &mut program.tables;
        tables.type_reference_table.copy_from(
            &staging.type_reference_table,
            &staging.expression_table,
            &mut tables.expression_table,
            staged,
        )
    };
    {
        let tables = &mut program.tables;
        tables
            .type_reference_table
            .remap_symbols_in(copied, &mut tables.expression_table, symbols);
    }
    copied
}

pub(super) fn copy_signature_contract(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    contract: typed_trees::signature::SignatureContract,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> typed_trees::signature::SignatureContract {
    let original_facts = contract.facts;
    let mut copied = contract;
    copied.facts = HandleSpan::empty();
    for (offset, fact) in source
        .unwrap_or(program)
        .proof_facts
        .span_or_empty(original_facts)
        .to_vec()
        .iter()
        .enumerate()
    {
        let source_fact = arena::Handle::from_parts(
            original_facts
                .start()
                .arena_index()
                .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                .expect("proof fact source handle overflow"),
            original_facts.start().generation(),
        );
        let source_span = source
            .unwrap_or(program)
            .proof_fact_source_span(source_fact);
        let fact = match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                typed_trees::domain::ProofFact::Expression(copy_expression(
                    source,
                    program,
                    *expression,
                    symbols,
                ))
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let source_arguments = source
                    .unwrap_or(program)
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .to_vec();
                let domain_arguments = if source_arguments.len()
                    != membership.domain_arguments.len()
                {
                    // Retain invalidity until the fallible instance refresh;
                    // never turn a stale span into an empty valid application.
                    arena::HandleSpan::from_parts(
                        arena::Handle::invalid(),
                        membership.domain_arguments.count(),
                    )
                } else {
                    let arguments = source_arguments
                        .iter()
                        .map(|argument| copy_type_reference(source, program, *argument, symbols))
                        .collect::<Vec<_>>();
                    program
                        .type_reference_table
                        .insert_type_reference_handles(arguments)
                };
                typed_trees::domain::ProofFact::Membership(
                    typed_trees::domain::ProofMembershipFact {
                        value: copy_expression(source, program, membership.value, symbols),
                        domain: {
                            let members = source
                                .unwrap_or(program)
                                .domain_path_members(membership.domain)
                                .to_vec();
                            program.domain_path_members.insert_many(members)
                        },
                        domain_symbol: remapped_symbol(membership.domain_symbol, symbols),
                        domain_arguments,
                        semantic_domain: membership.semantic_domain,
                        authored_domain_selection: membership.authored_domain_selection,
                    },
                )
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                let arguments = source
                    .unwrap_or(program)
                    .expression_table
                    .expression_handles(application.arguments)
                    .to_vec()
                    .into_iter()
                    .map(|argument| copy_expression(source, program, argument, symbols))
                    .collect::<Vec<_>>();
                let arguments = program
                    .expression_table
                    .insert_expression_handles(arguments);
                typed_trees::domain::ProofFact::Proposition(
                    typed_trees::proposition::PropositionApplication {
                        proposition: remapped_symbol(application.proposition, symbols),
                        name: application.name.clone(),
                        binder_arguments: application
                            .binder_arguments
                            .iter()
                            .map(
                                |argument| typed_trees::proposition::PropositionBinderArgument {
                                    kind: argument.kind,
                                    path: argument.path.clone(),
                                    const_literal: argument.const_literal.clone(),
                                    evidence_projection: argument.evidence_projection.clone(),
                                    symbol: remapped_symbol(argument.symbol, symbols),
                                },
                            )
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        arguments,
                    },
                )
            }
        };
        let copied_fact = program.proof_facts.append_to_span(&mut copied.facts, fact);
        if let Some(source_span) = source_span {
            program.set_proof_fact_source_span(copied_fact, source_span);
        }
    }
    copied
}

/// Retarget a copied contract's fact value expressions from each runtime-bound
/// `Value` binder to the owning scope's realized trailing parameter. A binder
/// occurrence inside a fact expression is an ordinary value read, not a static
/// position: `requires Count <= 10` on the clone becomes a precondition on the
/// parameter the rewritten call site fills with the captured subject, so the
/// ordinary call-requires machinery re-derives the obligation on that subject
/// (including under a caller guard). Type-position occurrences — parameter
/// type references, membership domain arguments — are untouched and still
/// reject under `reject_runtime_bound_static_occurrences`.
pub(super) fn remap_contract_value_subjects(
    program: &mut TypedTrees,
    contract: &typed_trees::signature::SignatureContract,
    realized: &[(SymbolHandle, SymbolHandle)],
) {
    if realized.is_empty() {
        return;
    }
    let roots: Vec<ExpressionHandle> = program
        .proof_facts
        .span_or_empty(contract.facts)
        .iter()
        .flat_map(|fact| match fact {
            ProofFact::Expression(expression) => vec![*expression],
            ProofFact::Membership(membership) => vec![membership.value],
            ProofFact::Proposition(application) => program
                .expression_table
                .expression_handles(application.arguments)
                .to_vec(),
        })
        .collect();
    for root in roots {
        program.expression_table.remap_symbols_in(root, realized);
    }
}

/// Rebind member projections rooted at a realized `Value` subject. The
/// template types `V.field` through the binder's declared carrier but leaves
/// `member.member_symbol` unresolved: a binder is a type parameter, not a data
/// symbol. Realizing the subject as an ordinary parameter makes the
/// projection an ordinary field read, so bind the same member identity an
/// authored parameter read would have carried.
fn rebind_realized_member_symbols(
    program: &mut TypedTrees,
    state: &typed_trees::state::State,
    realized: &[(SymbolHandle, SymbolHandle)],
) {
    if realized.is_empty() {
        return;
    }
    let mut roots = Vec::new();
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
    {
        collect_statement_expression_trees(program, statement, &mut roots);
    }
    let mut members = Vec::new();
    for root in roots {
        crate::monomorphization::collect_expression_tree(program, root, &mut members);
    }
    for handle in members {
        let ExpressionNode::Member(member) = program.expression_table.expression(handle) else {
            continue;
        };
        if member.member_symbol.is_valid() || member.case_variant.is_some() {
            continue;
        }
        let receiver = member.receiver;
        let name = member.member.clone();
        let Some(field) =
            realized_receiver_field(program, state, realized, receiver, name.as_str())
        else {
            continue;
        };
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(handle) else {
            unreachable!();
        };
        member.member_symbol = field.symbol;
    }
}

/// Resolve the exact field `name` on the declared carrier of a realized
/// `Value` receiver. Only receivers rooted at a realized parameter qualify —
/// an unresolved member on any other place keeps its authored identity.
fn realized_receiver_field(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    realized: &[(SymbolHandle, SymbolHandle)],
    receiver: ExpressionHandle,
    name: &str,
) -> Option<typed_trees::data::DataField> {
    let reference = realized_receiver_type(program, state, realized, receiver, 0)?;
    carrier_field(program, reference, name)
}

/// The declared type of a member-projection receiver rooted at a realized
/// `Value` parameter. Nested projections walk each field's declared type in
/// turn so `V.outer.inner` resolves against the same carriers an authored
/// parameter chain would.
fn realized_receiver_type(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    realized: &[(SymbolHandle, SymbolHandle)],
    expression: ExpressionHandle,
    depth: usize,
) -> Option<TypeReferenceHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            realized_receiver_type(program, state, realized, inner.target, depth + 1)
        }
        ExpressionNode::Name(path) => {
            let symbol = path.symbol;
            if !realized.iter().any(|(_, realized)| *realized == symbol) {
                return None;
            }
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .map(|parameter| parameter.type_reference)
        }
        ExpressionNode::Member(member) => {
            if member.case_variant.is_some() {
                return None;
            }
            let receiver =
                realized_receiver_type(program, state, realized, member.receiver, depth + 1)?;
            carrier_field(program, receiver, member.member.as_str())
                .map(|field| field.type_reference)
        }
        _ => None,
    }
}

/// The one field `name` declares on the record `reference` names: peel the
/// borrow/constraint shells a realized parameter's declared carrier may wear,
/// then require a single data definition so the field identity is exact.
fn carrier_field(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
) -> Option<typed_trees::data::DataField> {
    let mut reference = reference;
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => {
                let mut definitions = program
                    .data_definitions()
                    .iter()
                    .filter(|data| data.symbol == *symbol);
                let definition = definitions.next()?;
                if definitions.next().is_some() {
                    return None;
                }
                let mut fields = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| match member {
                        typed_trees::data::DataMember::Field(field)
                            if field.name.as_str() == name =>
                        {
                            Some(field)
                        }
                        _ => None,
                    });
                let field = fields.next()?;
                if fields.next().is_some() || !field.symbol.is_valid() {
                    return None;
                }
                return Some(field.clone());
            }
            _ => return None,
        }
    }
}
