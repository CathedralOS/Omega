/*
Static call-tuple selection and binding inference (`selection`), compile-time
reflection and active-case schema queries (`reflection`), and inferred result
local refreshing (`result_locals`).
*/

pub(crate) mod selection {
    use crate::tables::expressions_of;
    use const_arguments::collect_type_expressions;
    use typed_trees::operator::declaration_by_symbol;
    use validation::collect_expression_tree;
    use validation::collect_statement_expression_trees;
    use validation::expression_result_type_reference;

    pub(crate) mod callee_proposals {
        use const_arguments::resolve_runtime_subject;
        use const_integer::classify;
        use validation::expression_traversal::{
            collect_proof_fact_expression_trees, collect_statement_expression_trees,
        };

        use crate::monomorphization::selection::proposals::{BindingContext, StaticProposals};
        use crate::monomorphization::selection::static_bindings::infer_static_bindings;
        use crate::monomorphization::{
            CallSite, CalleeState, Candidate, ExpressionHandle, StaticMachineArgument,
            SymbolHandle, SymbolKind, const_arguments,
        };
        use crate::tables::expressions_of;
        use typed_trees::const_integer::{self, ConstIntegerExpression};
        use typed_trees::types::TypeReferenceNode;
        use validation::runtime_field_subject;

        pub(crate) struct StaticArgumentOverflow;

        pub(crate) fn collect_machine_proposals_for_callee(
            program: &dyn ::validation::CheckingInput,
            candidates: &[Candidate],
            callee: &CalleeState,
            machine_arguments: &[StaticMachineArgument],
            runtime_scope: Option<(&typed_trees::state::State, usize)>,
            proposals: &mut StaticProposals,
        ) -> Result<(), StaticArgumentOverflow> {
            let candidate = &candidates[callee.candidate_index];
            let context = BindingContext::new(
                program,
                &candidate.template.type_parameters,
                &candidate.template.const_parameters,
                callee.candidate_index,
            );
            let mut type_index = 0usize;
            let mut const_index = 0usize;
            let mut machine_index = 0usize;
            let mut evidence_index = 0usize;
            for selected in machine_arguments {
                // Retained source types preserve an implementation's scope;
                // the operand still fills its declared evidence slot.
                if selected.names_implementation(&program.symbols) {
                    if evidence_index >= candidate.template.evidence_parameters.len() {
                        return Err(StaticArgumentOverflow);
                    }
                    proposals.evidence.push((
                        callee.candidate_index,
                        evidence_index,
                        selected.clone(),
                    ));
                    evidence_index += 1;
                    continue;
                }
                let value_slot = const_index < candidate.template.const_parameters.len()
                    && candidate
                        .template
                        .value_const_parameters
                        .contains(&const_index);
                let runtime_expression =
                    const_arguments::runtime_subject_expression(program, selected);
                if !(value_slot && runtime_expression.is_some())
                    && let Some(expression) = const_expression_argument(program, selected)
                {
                    if const_index >= candidate.template.const_parameters.len() {
                        return Err(StaticArgumentOverflow);
                    }
                    match classify(expressions_of(program), expression, &|symbol, _| {
                        const_binder(program, symbol)
                    }) {
                        Some(ConstIntegerExpression::Closed(value)) => {
                            if let Some((handle, _, _)) = program
                                .type_reference_table
                                .named_references()
                                .find(|(_, symbol, name)| {
                                    !symbol.is_valid() && *name == value.to_string()
                                })
                            {
                                proposals.constants.push((
                                    callee.candidate_index,
                                    const_index,
                                    handle,
                                ));
                            }
                            const_index += 1;
                            continue;
                        }
                        Some(ConstIntegerExpression::Open) | None => {
                            if !typed_trees::domain::index_argument_is_closed(
                                program,
                                selected.type_reference,
                            ) {
                                proposals.constants.push((
                                    callee.candidate_index,
                                    const_index,
                                    selected.type_reference,
                                ));
                            }
                            const_index += 1;
                            continue;
                        }
                    }
                }
                let runtime_field = runtime_scope.and_then(|(state, _)| {
                    selected
                        .evidence_projection
                        .as_ref()
                        .and_then(|projection| {
                            runtime_field_subject(
                                program,
                                state,
                                &projection.term,
                                &projection.member,
                            )
                        })
                });
                if selected.type_reference.is_valid()
                    && !(value_slot && runtime_expression.is_some())
                {
                    if type_index >= candidate.template.type_parameters.len() {
                        return Err(StaticArgumentOverflow);
                    }
                    proposals.types.push((
                        callee.candidate_index,
                        type_index,
                        selected.type_reference,
                    ));
                    type_index += 1;
                    continue;
                }
                if const_arguments::forwarded_type(program, selected).is_valid() {
                    if const_index >= candidate.template.const_parameters.len() {
                        return Err(StaticArgumentOverflow);
                    }
                    if let Some(binding) = program
                        .type_reference_table
                        .find_named_type_reference(selected.symbol)
                    {
                        proposals
                            .constants
                            .push((callee.candidate_index, const_index, binding));
                    }
                    const_index += 1;
                    continue;
                }
                if let Some(literal) = const_arguments::spelling(program, selected) {
                    if const_index >= candidate.template.const_parameters.len() {
                        return Err(StaticArgumentOverflow);
                    }
                    if let Some((handle, _, _)) = program
                        .type_reference_table
                        .named_references()
                        .find(|(_, symbol, name)| !symbol.is_valid() && *name == literal)
                    {
                        proposals
                            .constants
                            .push((callee.candidate_index, const_index, handle));
                    }
                    const_index += 1;
                    continue;
                }
                if const_index >= candidate.template.const_parameters.len()
                    && (runtime_expression.is_some()
                        || runtime_field.is_some()
                        || runtime_scope.is_some_and(|(scope_state, scope_limit)| {
                            resolve_runtime_subject(program, scope_state, scope_limit, selected)
                                .is_some()
                        }))
                {
                    return Err(StaticArgumentOverflow);
                }
                if value_slot {
                    if let Some((scope_state, scope_limit)) = runtime_scope
                        && let Some(subject) =
                            resolve_runtime_subject(program, scope_state, scope_limit, selected)
                    {
                        let mut subject_argument = selected.clone();
                        subject_argument.symbol = subject;
                        proposals.runtime_values.push((
                            callee.candidate_index,
                            const_index,
                            crate::monomorphization::RuntimeValueBinding::Authored(
                                subject_argument,
                            ),
                        ));
                        const_index += 1;
                        continue;
                    }
                    if runtime_expression.is_some() || runtime_field.is_some() {
                        proposals.runtime_values.push((
                            callee.candidate_index,
                            const_index,
                            crate::monomorphization::RuntimeValueBinding::Authored(
                                selected.clone(),
                            ),
                        ));
                        const_index += 1;
                        continue;
                    }
                }
                if !selected.symbol.is_valid() {
                    continue;
                }
                let kind = program.symbols.get(selected.symbol).kind;
                if matches!(
                    kind,
                    SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                ) {
                    if type_index >= candidate.template.type_parameters.len() {
                        return Err(StaticArgumentOverflow);
                    }
                    if let Some(handle) = program
                        .type_reference_table
                        .find_named_type_reference(selected.symbol)
                    {
                        proposals
                            .types
                            .push((callee.candidate_index, type_index, handle));
                    }
                    type_index += 1;
                    continue;
                }
                if !matches!(kind, SymbolKind::State | SymbolKind::MachineParameter) {
                    continue;
                }
                if machine_index >= candidate.template.machine_parameters.len() {
                    return Err(StaticArgumentOverflow);
                }
                proposals
                    .machines
                    .push((callee.candidate_index, machine_index, selected.clone()));
                let requirement = &candidate.template.machine_parameters[machine_index].2;
                machine_index += 1;
                let Some(actual_state) = state_by_symbol(program, selected.symbol) else {
                    continue;
                };
                let first_type_proposal = proposals.types.len();
                let first_const_proposal = proposals.constants.len();
                for (required, actual) in program
                    .state_signature_parameters(requirement)
                    .iter()
                    .zip(program.state_parameters(actual_state))
                {
                    let actual_type =
                        validation::unwrapped_type_reference(program, actual.type_reference)
                            .unwrap_or(actual.type_reference);
                    infer_static_bindings(context, required.type_reference, actual_type, proposals);
                }
                infer_static_bindings(
                    context,
                    requirement.return_type,
                    actual_state.return_type,
                    proposals,
                );
                /*
                    The machine argument's parameters were unwrapped above, so a
                    domain qualification such as `Token in Carry::AcrossSuspend`
                    is not part of what they propose. When the caller already
                    bound that slot explicitly to the qualified form of the same
                    carrier, the derived proposal only restates the explicit
                    binding without its qualification; keep the explicit one.
                    A derived proposal naming a different carrier still
                    conflicts, and the machine argument's own parameter
                    contract is still checked against the explicit binding.
                */
                let mut ordinal = 0usize;
                let explicit_types = proposals.types[..first_type_proposal].to_vec();
                proposals.types.retain(|&(candidate_index, slot, binding)| {
                    let derived = ordinal >= first_type_proposal;
                    ordinal += 1;
                    !derived
                        || !explicit_types.iter().any(|&(explicit_candidate, explicit_slot, explicit)| {
                            explicit_candidate == candidate_index
                                && explicit_slot == slot
                                && !crate::monomorphization::selection::static_bindings::same_type_identity(program, explicit, binding)
                                && crate::monomorphization::selection::static_bindings::same_type_identity(
                                    program,
                                    validation::unwrapped_type_reference(program, explicit)
                                        .unwrap_or(explicit),
                                    binding,
                                )
                        })
                });
                let provider = program.machine_containing_state(selected.symbol);
                if let Some(provider) = provider {
                    let provider_binders: Vec<_> = program
                        .machine_type_parameters(provider)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .collect();
                    let owns_binder = |binding| {
                        crate::monomorphization::contextual_calls::names_any(
                            program.typed_program(),
                            binding,
                            &provider_binders,
                        )
                    };
                    let mut ordinal = 0usize;
                    proposals.types.retain(|(_, _, binding)| {
                        let keep = ordinal < first_type_proposal || !owns_binder(*binding);
                        ordinal += 1;
                        keep
                    });
                    let mut ordinal = 0usize;
                    proposals.constants.retain(|(_, _, binding)| {
                        let keep = ordinal < first_const_proposal || !owns_binder(*binding);
                        ordinal += 1;
                        keep
                    });
                }
            }
            Ok(())
        }

        pub(crate) fn resolve_callee<'callee>(
            program: &dyn ::validation::CheckingInput,
            callee_states: &'callee [CalleeState],
            target_symbol: SymbolHandle,
        ) -> Option<&'callee CalleeState> {
            if !target_symbol.is_valid() {
                return None;
            }
            let entry = program
                .machine_by_symbol(target_symbol)
                .and_then(|machine| program.machine_states(machine).first())
                .map_or(target_symbol, |state| state.symbol);
            callee_states.iter().find(|callee| callee.symbol == entry)
        }

        pub(crate) fn state_by_symbol(
            program: &dyn ::validation::CheckingInput,
            symbol: SymbolHandle,
        ) -> Option<&typed_trees::state::State> {
            program.state_by_symbol(symbol)
        }

        pub(crate) struct ContractExpressionScope<'program> {
            pub machine: &'program typed_trees::machine::Machine,
            pub state: Option<&'program typed_trees::state::State>,
            pub expression: ExpressionHandle,
        }

        pub(crate) fn contract_expression_scopes(
            program: &dyn ::validation::CheckingInput,
        ) -> Vec<ContractExpressionScope<'_>> {
            let mut scopes = Vec::new();
            for machine in program.machines() {
                let entry = program.machine_states(machine).first();
                let mut expressions = Vec::new();
                collect_proof_fact_expression_trees(
                    program,
                    program
                        .machine_contracts(machine)
                        .iter()
                        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts)),
                    &mut expressions,
                );
                scopes.extend(
                    expressions
                        .into_iter()
                        .map(|expression| ContractExpressionScope {
                            machine,
                            state: entry,
                            expression,
                        }),
                );
                for state in program.machine_states(machine) {
                    let mut expressions = Vec::new();
                    collect_proof_fact_expression_trees(
                        program,
                        program
                            .state_contracts(state)
                            .iter()
                            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts)),
                        &mut expressions,
                    );
                    scopes.extend(expressions.into_iter().map(|expression| {
                        ContractExpressionScope {
                            machine,
                            state: Some(state),
                            expression,
                        }
                    }));
                }
            }
            scopes.sort_unstable_by_key(|scope| {
                (
                    scope.expression.arena_index(),
                    scope.expression.generation(),
                    scope.machine.symbol.arena_index(),
                    scope.machine.symbol.generation(),
                    scope
                        .state
                        .map(|state| (state.symbol.arena_index(), state.symbol.generation())),
                )
            });
            scopes.dedup_by_key(|scope| {
                (
                    scope.expression,
                    scope.machine.symbol,
                    scope.state.map(|state| state.symbol),
                )
            });
            scopes
        }

        pub(crate) fn contract_expression_handles(
            program: &dyn ::validation::CheckingInput,
        ) -> Vec<ExpressionHandle> {
            collect_contract_expression_handles(
                program,
                contract_expression_scopes(program)
                    .into_iter()
                    .map(|scope| scope.expression),
            )
        }

        pub(crate) fn contract_expression_handles_from_scopes(
            program: &dyn ::validation::CheckingInput,
            scopes: &[ContractExpressionScope<'_>],
        ) -> Vec<ExpressionHandle> {
            collect_contract_expression_handles(
                program,
                scopes.iter().map(|scope| scope.expression),
            )
        }

        fn collect_contract_expression_handles(
            program: &dyn ::validation::CheckingInput,
            expressions: impl Iterator<Item = ExpressionHandle>,
        ) -> Vec<ExpressionHandle> {
            let mut handles = expressions.collect::<Vec<_>>();
            collect_proof_fact_expression_trees(
                program,
                program.proof_facts.iter().map(|(_, fact)| fact),
                &mut handles,
            );
            handles.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
            handles.dedup();
            handles
        }

        pub(crate) fn enclosing_statement_ordinal(
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            site: CallSite,
        ) -> Option<usize> {
            match site {
                CallSite::Statement(handle) => {
                    let start = state.statement_nodes.start().arena_index() as usize;
                    let ordinal = handle.arena_index() as usize;
                    (ordinal >= start && ordinal < start + state.statement_nodes.count() as usize)
                        .then_some(ordinal - start)
                }
                CallSite::Expression(handle) => program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                    .find_map(|(offset, statement)| {
                        let mut roots = Vec::new();
                        collect_statement_expression_trees(program, statement, &mut roots);
                        roots.contains(&handle).then_some(offset)
                    }),
            }
        }

        fn const_expression_argument(
            program: &dyn ::validation::CheckingInput,
            argument: &StaticMachineArgument,
        ) -> Option<ExpressionHandle> {
            if !argument.type_reference.is_valid() {
                return None;
            }
            match program
                .type_reference_table
                .type_reference(argument.type_reference)
            {
                TypeReferenceNode::ConstExpression(expression) => Some(*expression),
                _ => None,
            }
        }

        fn const_binder(program: &dyn ::validation::CheckingInput, symbol: SymbolHandle) -> bool {
            symbol.is_valid()
                && program
                    .data_type_parameter_by_symbol(symbol)
                    .is_some_and(|parameter| parameter.kind.is_const_or_value())
        }
    }
    pub(crate) mod proposals {
        use super::{StaticMachineArgument, SymbolHandle, TypeReferenceHandle};

        pub(crate) type ValueProposal = (usize, usize, StaticMachineArgument);

        pub(crate) type TypeProposal = (usize, usize, TypeReferenceHandle);

        #[derive(Default)]
        pub(crate) struct StaticProposals {
            pub(crate) types: Vec<TypeProposal>,
            pub(crate) constants: Vec<TypeProposal>,
            pub(crate) machines: Vec<ValueProposal>,
            pub(crate) evidence: Vec<ValueProposal>,
            pub(crate) runtime_values:
                Vec<(usize, usize, crate::monomorphization::RuntimeValueBinding)>,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct BindingContext<'context> {
            pub(crate) program: &'context dyn validation::CheckingInput,
            pub(crate) type_parameters: &'context [(SymbolHandle, String)],
            pub(crate) const_parameters: &'context [(SymbolHandle, String, TypeReferenceHandle)],
            pub(crate) candidate_index: usize,
        }

        impl<'context> BindingContext<'context> {
            pub(crate) fn new(
                program: &'context dyn validation::CheckingInput,
                type_parameters: &'context [(SymbolHandle, String)],
                const_parameters: &'context [(SymbolHandle, String, TypeReferenceHandle)],
                candidate_index: usize,
            ) -> Self {
                Self {
                    program,
                    type_parameters,
                    const_parameters,
                    candidate_index,
                }
            }
        }
    }
    pub(crate) mod selection_assembly {
        use crate::monomorphization::selection::StaticProposals;
        use crate::monomorphization::selection::callee_proposals::{
            enclosing_statement_ordinal, resolve_callee,
        };
        use crate::monomorphization::selection::collect_call_proposals;
        use crate::monomorphization::selection::static_bindings::{
            same_const_identity, same_type_identity,
        };
        use crate::monomorphization::{
            CallSelection, CallSite, CalleeState, Candidate, ExpressionHandle, SpecializationKey,
            StaticMachineArgument, SymbolHandle, SymbolKind, TypeParameterKind,
            TypeReferenceHandle, TypeReferenceNode,
        };
        use crate::tables::expressions_of;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn selection_for_call(
            program: &dyn ::validation::CheckingInput,
            caller_machine: &typed_trees::machine::Machine,
            caller_state: &typed_trees::state::State,
            candidates: &[Candidate],
            callee_states: &[CalleeState],
            site: CallSite,
            target_symbol: SymbolHandle,
            machine_arguments: &[StaticMachineArgument],
            arguments: &[ExpressionHandle],
            expected_return: Option<TypeReferenceHandle>,
            caller_is_generic: bool,
        ) -> Option<CallSelection> {
            let query = match site {
                CallSite::Expression(handle) => expressions_of(program)
                    .call_node(handle)
                    .and_then(|call| call.native_code_layout.as_ref()),
                CallSite::Statement(_) => None,
            };
            let effective_target = if let Some(query) = query {
                program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == query.operand.symbol)
                    .and_then(|machine| program.machine_states(machine).first())
                    .map(|state| state.symbol)
                    .unwrap_or(query.operand.symbol)
            } else {
                target_symbol
            };
            let callee = resolve_callee(program, callee_states, effective_target)?;
            let scope_limit = if !candidates[callee.candidate_index]
                .template
                .value_const_parameters
                .is_empty()
                || !machine_arguments.is_empty()
            {
                enclosing_statement_ordinal(program, caller_state, site).unwrap_or(usize::MAX)
            } else {
                usize::MAX
            };
            selection_for_call_with_local_prefix(
                program,
                caller_machine,
                caller_state,
                candidates,
                callee_states,
                site,
                target_symbol,
                machine_arguments,
                arguments,
                expected_return,
                caller_is_generic,
                scope_limit,
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn selection_for_call_with_local_prefix(
            program: &dyn ::validation::CheckingInput,
            caller_machine: &typed_trees::machine::Machine,
            caller_state: &typed_trees::state::State,
            candidates: &[Candidate],
            callee_states: &[CalleeState],
            site: CallSite,
            target_symbol: SymbolHandle,
            machine_arguments: &[StaticMachineArgument],
            arguments: &[ExpressionHandle],
            expected_return: Option<TypeReferenceHandle>,
            caller_is_generic: bool,
            scope_limit: usize,
        ) -> Option<CallSelection> {
            let query = match site {
                CallSite::Expression(handle) => expressions_of(program)
                    .call_node(handle)
                    .and_then(|call| call.native_code_layout.as_ref()),
                CallSite::Statement(_) => None,
            };
            let (target_symbol, machine_arguments, arguments, expected_return) =
                if let Some(query) = query {
                    let selected = program
                        .machines()
                        .iter()
                        .find(|machine| machine.symbol == query.operand.symbol)
                        .and_then(|machine| program.machine_states(machine).first())
                        .map(|state| state.symbol)
                        .unwrap_or(query.operand.symbol);
                    (
                        selected,
                        query
                            .operand
                            .application
                            .as_ref()
                            .map(|application| application.arguments.as_ref())
                            .unwrap_or(&[]),
                        &[][..],
                        None,
                    )
                } else {
                    (target_symbol, machine_arguments, arguments, expected_return)
                };
            let callee = resolve_callee(program, callee_states, target_symbol)?;
            let candidate = &candidates[callee.candidate_index];
            let mut proposals = StaticProposals::default();
            let explicit_arguments = collect_call_proposals(
                program,
                caller_machine,
                caller_state,
                candidates,
                callee_states,
                site,
                target_symbol,
                machine_arguments,
                arguments,
                expected_return,
                scope_limit,
                &mut proposals,
            );
            if let CallSite::Expression(expression) = site {
                for (_, _, reference) in &mut proposals.types {
                    *reference = program.statically_refined_type_reference(expression, *reference);
                }
            }
            let mut selection = selection_from_proposals(
                program,
                site,
                callee,
                candidate,
                caller_is_generic,
                proposals,
            );
            selection.explicit_argument_overflow =
                explicit_arguments == super::ExplicitArgumentCapacity::Exceeded;
            selection.lexical_owner = caller_machine.symbol;
            selection.lexical_state = caller_state.symbol;
            Some(selection)
        }

        pub(crate) fn selection_from_proposals(
            program: &dyn ::validation::CheckingInput,
            site: CallSite,
            callee: &CalleeState,
            candidate: &Candidate,
            caller_is_generic: bool,
            proposals: StaticProposals,
        ) -> CallSelection {
            let bindings = proposal_bindings(program, candidate, proposals);
            CallSelection {
                site,
                lexical_owner: SymbolHandle::invalid(),
                lexical_state: SymbolHandle::invalid(),
                callee_symbol: callee.symbol,
                candidate_index: callee.candidate_index,
                caller_is_generic,
                explicit_argument_overflow: false,
                unresolved_type_parameters: bindings.unresolved_type_parameters,
                unresolved_machine_parameters: bindings.unresolved_machine_parameters,
                unresolved_evidence_parameters: bindings.unresolved_evidence_parameters,
                unresolved_const_parameters: bindings.unresolved_const_parameters,
                type_bindings: bindings.type_bindings,
                const_bindings: bindings.const_bindings,
                runtime_value_bindings: bindings.runtime_value_bindings,
                machine_bindings: bindings.machine_bindings,
                evidence_bindings: bindings.evidence_bindings,
                conflicted: bindings.conflicted,
                conflict_reason: bindings.conflict_reason,
            }
        }

        pub(crate) struct ProposalBindings {
            pub(crate) unresolved_type_parameters: bool,
            pub(crate) unresolved_machine_parameters: bool,
            pub(crate) unresolved_evidence_parameters: bool,
            pub(crate) unresolved_const_parameters: bool,
            pub(crate) type_bindings: Vec<Option<TypeReferenceHandle>>,
            pub(crate) const_bindings: Vec<Option<TypeReferenceHandle>>,
            pub(crate) runtime_value_bindings:
                Vec<Option<crate::monomorphization::RuntimeValueBinding>>,
            pub(crate) machine_bindings: Vec<Option<StaticMachineArgument>>,
            pub(crate) evidence_bindings: Vec<Option<StaticMachineArgument>>,
            pub(crate) conflicted: bool,
            pub(crate) conflict_reason: Option<String>,
        }

        pub(crate) fn proposal_bindings(
            program: &dyn ::validation::CheckingInput,
            candidate: &Candidate,
            proposals: StaticProposals,
        ) -> ProposalBindings {
            let StaticProposals {
                types: type_proposals,
                constants: const_proposals,
                machines: machine_proposals,
                evidence: evidence_proposals,
                runtime_values: runtime_value_proposals,
            } = proposals;
            let mut selection = ProposalBindings {
                unresolved_type_parameters: false,
                unresolved_machine_parameters: false,
                unresolved_evidence_parameters: false,
                unresolved_const_parameters: false,
                type_bindings: vec![None; candidate.template.type_parameters.len()],
                const_bindings: vec![None; candidate.template.const_parameters.len()],
                runtime_value_bindings: vec![None; candidate.template.const_parameters.len()],
                machine_bindings: vec![None; candidate.template.machine_parameters.len()],
                evidence_bindings: vec![None; candidate.template.evidence_parameters.len()],
                conflicted: false,
                conflict_reason: None,
            };
            for (_, parameter, argument) in runtime_value_proposals {
                selection.const_bindings[parameter] =
                    Some(candidate.template.const_parameters[parameter].2);
                selection.runtime_value_bindings[parameter] = Some(argument);
            }
            for (_, parameter, binding) in type_proposals {
                if type_reference_is_any_generic_parameter(program, binding) {
                    selection.unresolved_type_parameters = true;
                }
                match selection.type_bindings[parameter] {
                    None => selection.type_bindings[parameter] = Some(binding),
                    Some(existing) if !same_type_identity(program, existing, binding) => {
                        selection.conflicted = true;
                        selection.conflict_reason.get_or_insert_with(|| {
                            format!(
                                "Type slot `{}` conflicts: {} versus {}",
                                candidate.template.type_parameters[parameter].1,
                                describe_binding(program, existing),
                                describe_binding(program, binding),
                            )
                        });
                    }
                    Some(_) => {}
                }
            }
            for (_, parameter, binding) in const_proposals {
                if selection.runtime_value_bindings[parameter].is_some() {
                    if program
                        .static_const_literal_from_type_reference(binding)
                        .is_some()
                    {
                        let placeholder = candidate.template.const_parameters[parameter].2;
                        match selection.const_bindings[parameter] {
                            Some(existing)
                                if !same_const_identity(program, existing, placeholder)
                                    && !same_const_identity(program, existing, binding) =>
                            {
                                selection.conflicted = true;
                                selection.conflict_reason.get_or_insert_with(|| {
                                    format!(
                                        "Const slot `{}` conflicts: {} versus {}",
                                        candidate.template.const_parameters[parameter].1,
                                        describe_binding(program, existing),
                                        describe_binding(program, binding),
                                    )
                                });
                            }
                            _ => {
                                selection.const_bindings[parameter] = Some(binding);
                                selection.runtime_value_bindings[parameter] = None;
                            }
                        }
                    }
                    continue;
                }
                if !binding.is_valid() {
                    selection.unresolved_const_parameters = true;
                    continue;
                }
                if !typed_trees::domain::index_argument_is_closed(program, binding) {
                    selection.unresolved_const_parameters = true;
                }
                match selection.const_bindings[parameter] {
                    None => selection.const_bindings[parameter] = Some(binding),
                    Some(existing) if !same_const_identity(program, existing, binding) => {
                        selection.conflicted = true;
                        selection.conflict_reason.get_or_insert_with(|| {
                            format!(
                                "Const slot `{}` conflicts: {} versus {}",
                                candidate.template.const_parameters[parameter].1,
                                describe_binding(program, existing),
                                describe_binding(program, binding),
                            )
                        });
                    }
                    Some(_) => {}
                }
            }
            for (_, parameter, binding) in machine_proposals {
                if program.symbols.get(binding.symbol).kind == SymbolKind::MachineParameter
                    || binding.application.is_some()
                {
                    selection.unresolved_machine_parameters = true;
                }
                match &selection.machine_bindings[parameter] {
                    None => selection.machine_bindings[parameter] = Some(binding),
                    Some(existing) if existing.symbol != binding.symbol => {
                        selection.conflicted = true;
                        selection.conflict_reason.get_or_insert_with(|| {
                            format!(
                                "Machine slot `{}` conflicts: {} versus {}",
                                candidate.template.machine_parameters[parameter].1,
                                existing.display_name(),
                                binding.display_name(),
                            )
                        });
                    }
                    Some(_) => {}
                }
            }
            for (_, parameter, binding) in evidence_proposals {
                if crate::monomorphization::contextual_calls::argument_is_open(
                    program.typed_program(),
                    &binding,
                ) {
                    selection.unresolved_evidence_parameters = true;
                }
                match &selection.evidence_bindings[parameter] {
                    None => selection.evidence_bindings[parameter] = Some(binding),
                    Some(existing)
                        if existing.symbol != binding.symbol
                            || existing.display_name() != binding.display_name() =>
                    {
                        selection.conflicted = true;
                        selection.conflict_reason.get_or_insert_with(|| {
                            format!(
                                "evidence slot `{}` conflicts: {} versus {}",
                                candidate.template.evidence_parameters[parameter].binder_name,
                                existing.display_name(),
                                binding.display_name(),
                            )
                        });
                    }
                    Some(_) => {}
                }
            }
            selection
        }

        fn describe_binding(
            program: &dyn validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> String {
            let owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            format!(
                "{} [identity {}; supplier {}]",
                program.display_type_reference(reference),
                program
                    .static_const_type_reference_identity(reference)
                    .unwrap_or_else(|| program.normalized_type_identity_text(reference)),
                if owner.is_valid() {
                    program.symbols.display_path(owner, "::")
                } else {
                    "unretained".into()
                },
            )
        }

        pub(crate) fn type_reference_is_any_generic_parameter(
            program: &dyn ::validation::CheckingInput,
            binding: TypeReferenceHandle,
        ) -> bool {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(binding)
            else {
                return false;
            };
            if symbol.is_valid() && program.symbols.get(*symbol).kind == SymbolKind::TypeParameter {
                return true;
            }
            program.machines().iter().any(|machine| {
                program
                    .machine_type_parameters(machine)
                    .iter()
                    .any(|parameter| {
                        matches!(
                            parameter.kind,
                            TypeParameterKind::Type
                                | TypeParameterKind::Const { .. }
                                | TypeParameterKind::Value { .. }
                        ) && (parameter.symbol == *symbol
                            || (!parameter.symbol.is_valid()
                                && !symbol.is_valid()
                                && parameter.name.as_str() == name.as_str()))
                    })
            })
        }

        fn bound_count<T>(bindings: &[Option<T>]) -> usize {
            bindings.iter().filter(|item| item.is_some()).count()
        }

        pub(crate) fn upsert_selection(
            selections: &mut Vec<CallSelection>,
            selection: CallSelection,
        ) {
            if let Some(existing) = selections
                .iter_mut()
                .find(|existing| existing.site == selection.site)
            {
                let explicit_argument_overflow =
                    existing.explicit_argument_overflow || selection.explicit_argument_overflow;
                let existing_evidence = bound_count(&existing.type_bindings)
                    + bound_count(&existing.const_bindings)
                    + bound_count(&existing.machine_bindings)
                    + bound_count(&existing.evidence_bindings);
                let new_evidence = bound_count(&selection.type_bindings)
                    + bound_count(&selection.const_bindings)
                    + bound_count(&selection.machine_bindings)
                    + bound_count(&selection.evidence_bindings);
                if new_evidence >= existing_evidence {
                    *existing = selection;
                }
                existing.explicit_argument_overflow = explicit_argument_overflow;
            } else {
                selections.push(selection);
            }
        }

        pub(crate) fn unique_complete_selections(
            program: &dyn ::validation::CheckingInput,
            selections: &[CallSelection],
            candidate_index: usize,
        ) -> Result<Vec<(SpecializationKey, Vec<usize>)>, diagnostics::Diagnostic> {
            let mut groups: Vec<(SpecializationKey, Vec<usize>)> = Vec::new();
            for (selection_index, selection) in selections.iter().enumerate() {
                if selection.candidate_index != candidate_index
                    || !selection.is_complete()
                    || selection
                        .type_bindings
                        .iter()
                        .chain(&selection.const_bindings)
                        .flatten()
                        .any(|reference| {
                            crate::monomorphization::contextual_calls::operand_is_open(
                                program.typed_program(),
                                *reference,
                            )
                        })
                {
                    continue;
                }
                let key = SpecializationKey {
                    type_arguments: selection
                        .type_bindings
                        .iter()
                        .map(|binding| {
                            program
                                .normalized_type_identity_text(binding.expect("complete selection"))
                        })
                        .collect(),
                    const_arguments: selection
                        .const_bindings
                        .iter()
                        .map(|binding| {
                            program
                                .normalized_type_identity_text(binding.expect("complete selection"))
                        })
                        .collect(),
                    machine_arguments: selection
                        .machine_bindings
                        .iter()
                        .map(|binding| binding.as_ref().expect("complete selection").symbol)
                        .collect(),
                    evidence_arguments: selection
                        .evidence_bindings
                        .iter()
                        .map(|binding| {
                            validation::conformance_applications::close_conformance_application(
                                program,
                                binding.as_ref().expect("complete selection"),
                            )
                            .map(|application| application.report_fingerprint)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                };
                if let Some((_, members)) = groups.iter_mut().find(|(existing, _)| *existing == key)
                {
                    members.push(selection_index);
                } else {
                    groups.push((key, vec![selection_index]));
                }
            }
            Ok(groups)
        }
    }
    pub(crate) mod static_bindings {
        use super::proposals::{BindingContext, StaticProposals};
        use crate::monomorphization::{
            HandleSpan, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };
        use typed_trees::types::DomainConstraint;

        pub(crate) fn names_type_binder(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            let mut pending = vec![reference];
            let mut visited = Vec::new();
            while let Some(reference) = pending.pop() {
                if !reference.is_valid() || visited.contains(&reference) {
                    continue;
                }
                visited.push(reference);
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                        if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter {
                            return true;
                        }
                        if let Some(application) = program
                            .data_definition_by_symbol(*symbol)
                            .and_then(|definition| definition.generic_instance)
                        {
                            pending.push(application);
                        }
                    }
                    TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments)
                            .iter()
                            .copied(),
                    ),
                    TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                    TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                    _ => {}
                }
            }
            false
        }

        pub(crate) fn infer_static_bindings(
            context: BindingContext<'_>,
            required: TypeReferenceHandle,
            actual: TypeReferenceHandle,
            proposals: &mut StaticProposals,
        ) {
            let program = context.program;
            if !required.is_valid() || !actual.is_valid() {
                return;
            }
            if let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(required)
                && let Some(index) =
                    context
                        .type_parameters
                        .iter()
                        .position(|(parameter_symbol, parameter_name)| {
                            parameter_symbol == symbol
                                || (!parameter_symbol.is_valid()
                                    && !symbol.is_valid()
                                    && parameter_name == name.as_str())
                                || parameter_name == name.as_str()
                        })
            {
                proposals
                    .types
                    .push((context.candidate_index, index, actual));
                return;
            }
            if let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(required)
                && let Some(index) = context.const_parameters.iter().position(
                    |(parameter_symbol, parameter_name, _)| {
                        parameter_symbol == symbol
                            || (!parameter_symbol.is_valid()
                                && !symbol.is_valid()
                                && parameter_name == name.as_str())
                            || parameter_name == name.as_str()
                    },
                )
            {
                proposals
                    .constants
                    .push((context.candidate_index, index, actual));
                return;
            }

            match (
                program.type_reference_table.type_reference(required),
                program.type_reference_table.type_reference(actual),
            ) {
                (TypeReferenceNode::Generic { .. }, TypeReferenceNode::Named { symbol, .. })
                    if symbol.is_valid() =>
                {
                    if let Some(application) = program
                        .data_definition_by_symbol(*symbol)
                        .and_then(|definition| definition.generic_instance)
                        .map(|application| {
                            validation::unwrapped_type_reference(program, application)
                                .unwrap_or(application)
                        })
                        .filter(|application| {
                            matches!(
                                program.type_reference_table.type_reference(*application),
                                TypeReferenceNode::Generic { .. }
                            )
                        })
                    {
                        infer_static_bindings(context, required, application, proposals);
                    }
                }
                (
                    TypeReferenceNode::Reference {
                        referee: required, ..
                    },
                    TypeReferenceNode::Reference {
                        referee: actual, ..
                    },
                ) => infer_static_bindings(context, *required, *actual, proposals),
                (
                    TypeReferenceNode::Constrained {
                        base_type: required_base,
                        constraints: required_constraints,
                    },
                    TypeReferenceNode::Constrained {
                        base_type: actual_base,
                        constraints: actual_constraints,
                    },
                ) => {
                    infer_static_bindings(context, *required_base, *actual_base, proposals);
                    infer_domain_argument_bindings(
                        context,
                        *required_constraints,
                        *actual_constraints,
                        proposals,
                    );
                }
                (TypeReferenceNode::Constrained { base_type, .. }, _) => infer_static_bindings(
                    context,
                    *base_type,
                    validation::unwrapped_type_reference(program, actual).unwrap_or(actual),
                    proposals,
                ),
                (_, TypeReferenceNode::Constrained { base_type, .. }) => infer_static_bindings(
                    context,
                    validation::unwrapped_type_reference(program, required).unwrap_or(required),
                    *base_type,
                    proposals,
                ),
                (
                    TypeReferenceNode::Slice {
                        element_type: required,
                    },
                    TypeReferenceNode::Slice {
                        element_type: actual,
                    }
                    | TypeReferenceNode::FixedArray {
                        element_type: actual,
                        ..
                    },
                ) => infer_static_bindings(context, *required, *actual, proposals),
                (
                    TypeReferenceNode::FixedArray {
                        element_type: required_element,
                        length: required_length,
                    },
                    TypeReferenceNode::FixedArray {
                        element_type: actual_element,
                        length: actual_length,
                    },
                ) => {
                    infer_fixed_array_length_binding(
                        context,
                        required_length,
                        actual_length,
                        proposals,
                    );
                    infer_static_bindings(context, *required_element, *actual_element, proposals);
                }
                (
                    TypeReferenceNode::Generic {
                        base_symbol: required_base,
                        base_name: required_name,
                        arguments: required_arguments,
                        ..
                    },
                    TypeReferenceNode::Generic {
                        base_symbol: actual_base,
                        base_name: actual_name,
                        arguments: actual_arguments,
                        ..
                    },
                ) if ((required_base.is_valid()
                    && actual_base.is_valid()
                    && required_base == actual_base)
                    || required_name == actual_name)
                    && program
                        .type_reference_table
                        .type_reference_handles(*required_arguments)
                        .len()
                        == program
                            .type_reference_table
                            .type_reference_handles(*actual_arguments)
                            .len() =>
                {
                    for (required, actual) in program
                        .type_reference_table
                        .type_reference_handles(*required_arguments)
                        .iter()
                        .zip(
                            program
                                .type_reference_table
                                .type_reference_handles(*actual_arguments),
                        )
                    {
                        infer_static_bindings(context, *required, *actual, proposals);
                    }
                }
                _ => {}
            }
        }

        pub(crate) fn infer_fixed_array_length_binding(
            context: BindingContext<'_>,
            required: &typed_trees::types::FixedArrayLength,
            actual: &typed_trees::types::FixedArrayLength,
            proposals: &mut StaticProposals,
        ) {
            let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = required
            else {
                return;
            };
            let Some(parameter_index) = context.const_parameters.iter().position(
                |(parameter_symbol, parameter_name, _)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                },
            ) else {
                return;
            };
            let program = context.program;
            let binding = match actual {
                typed_trees::types::FixedArrayLength::Literal(value) => {
                    let value = value.to_string();
                    program
                        .type_reference_table
                        .named_references()
                        .find(|(_, candidate_symbol, candidate_name)| {
                            !candidate_symbol.is_valid() && *candidate_name == value
                        })
                        .map(|(handle, _, _)| handle)
                }
                typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } => program
                    .type_reference_table
                    .named_references()
                    .find(|(_, candidate_symbol, candidate_name)| {
                        candidate_symbol == symbol
                            || (!candidate_symbol.is_valid()
                                && !symbol.is_valid()
                                && *candidate_name == name.as_str())
                    })
                    .map(|(handle, _, _)| handle),
                typed_trees::types::FixedArrayLength::Expression(_)
                | typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
            };
            if let Some(binding) = binding {
                proposals
                    .constants
                    .push((context.candidate_index, parameter_index, binding));
            }
        }

        pub(crate) fn infer_domain_argument_bindings(
            context: BindingContext<'_>,
            required_constraints: HandleSpan<TypeConstraintNode>,
            actual_constraints: HandleSpan<TypeConstraintNode>,
            proposals: &mut StaticProposals,
        ) {
            let program = context.program;
            for required in program
                .type_reference_table
                .constraints(required_constraints)
            {
                let TypeConstraintNode::Domain(required) = required else {
                    continue;
                };
                let Some(actual) = program
                    .type_reference_table
                    .constraints(actual_constraints)
                    .iter()
                    .find_map(|constraint| {
                        let TypeConstraintNode::Domain(actual) = constraint else {
                            return None;
                        };
                        same_domain_family(required, actual).then_some(actual)
                    })
                else {
                    continue;
                };
                for (required, actual) in required.arguments.iter().zip(&actual.arguments) {
                    infer_static_bindings(context, *required, *actual, proposals);
                }
            }
        }

        pub(crate) fn same_domain_family(
            left: &DomainConstraint,
            right: &DomainConstraint,
        ) -> bool {
            if left.symbol.is_valid() && right.symbol.is_valid() {
                return left.symbol == right.symbol;
            }
            left.name == right.name
                || left.name.as_str().rsplit("::").next() == right.name.as_str().rsplit("::").next()
        }

        pub(crate) fn same_type_identity(
            program: &dyn ::validation::CheckingInput,
            left: TypeReferenceHandle,
            right: TypeReferenceHandle,
        ) -> bool {
            program.normalized_type_identity(left) == program.normalized_type_identity(right)
        }

        pub(crate) fn same_const_identity(
            program: &dyn ::validation::CheckingInput,
            left: TypeReferenceHandle,
            right: TypeReferenceHandle,
        ) -> bool {
            program
                .static_const_type_reference_identity(left)
                .zip(program.static_const_type_reference_identity(right))
                .map_or_else(
                    || same_type_identity(program, left, right),
                    |(left, right)| left == right,
                )
        }
    }

    pub(crate) use callee_proposals::{
        ContractExpressionScope, collect_machine_proposals_for_callee, contract_expression_handles,
        contract_expression_handles_from_scopes, contract_expression_scopes, resolve_callee,
        state_by_symbol,
    };
    pub(crate) use proposals::{BindingContext, StaticProposals};
    pub(crate) use selection_assembly::{
        proposal_bindings, selection_for_call, selection_from_proposals,
        unique_complete_selections, upsert_selection,
    };
    pub(crate) use static_bindings::{
        infer_static_bindings, names_type_binder, same_const_identity,
    };

    use crate::monomorphization::{CallSelection, CalleeState, Candidate, const_arguments};
    use crate::monomorphization::{
        CallSite, ExpressionHandle, ExpressionNode, StatementNode, StaticMachineArgument,
        SymbolHandle, TypeReferenceHandle,
    };
    use std::collections::HashSet;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ExplicitArgumentCapacity {
        WithinBounds,
        Exceeded,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn collect_call_proposals(
        program: &dyn ::validation::CheckingInput,
        caller_machine: &typed_trees::machine::Machine,
        caller_state: &typed_trees::state::State,
        candidates: &[Candidate],
        callee_states: &[CalleeState],
        site: CallSite,
        target_symbol: SymbolHandle,
        machine_arguments: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
        expected_return: Option<TypeReferenceHandle>,
        scope_limit: usize,
        proposals: &mut StaticProposals,
    ) -> ExplicitArgumentCapacity {
        let Some(callee) = resolve_callee(program, callee_states, target_symbol) else {
            return ExplicitArgumentCapacity::WithinBounds;
        };
        let explicit_arguments = collect_machine_proposals_for_callee(
            program,
            candidates,
            callee,
            machine_arguments,
            Some((caller_state, scope_limit)),
            proposals,
        );
        if explicit_arguments.is_err() {
            return ExplicitArgumentCapacity::Exceeded;
        }
        let explicit_type_count = proposals.types.len();
        let mut inferred = StaticProposals::default();

        let candidate = &candidates[callee.candidate_index];
        let context = BindingContext::new(
            program,
            &candidate.template.type_parameters,
            &candidate.template.const_parameters,
            callee.candidate_index,
        );
        for (position, (argument, required)) in arguments
            .iter()
            .zip(callee.parameter_types.iter())
            .enumerate()
        {
            let Some(actual) =
                expression_result_type_reference(program, caller_machine, caller_state, *argument)
            else {
                continue;
            };
            if !program
                .typed_program()
                .callable_body_has_open_parameters(caller_machine)
                && static_bindings::names_type_binder(program, actual)
            {
                continue;
            }
            let first = inferred.constants.len();
            infer_static_bindings(context, *required, actual, &mut inferred);
            let descriptors = program.data_application_value_fields(actual);
            for &(_, binder, binding) in &inferred.constants[first..] {
                if !candidate.template.value_const_parameters.contains(&binder)
                    || program
                        .static_const_literal_from_type_reference(binding)
                        .is_some()
                {
                    continue;
                }
                if let Some((_, field, _)) =
                    descriptors.iter().find(|(_, _, index)| *index == binding)
                {
                    inferred.runtime_values.push((
                        callee.candidate_index,
                        binder,
                        crate::monomorphization::RuntimeValueBinding::CapturedArgument {
                            site,
                            position: u32::try_from(position).expect("argument ordinal"),
                            source: *argument,
                            descriptor: field.symbol,
                        },
                    ));
                }
            }
        }
        if let Some(actual) = expected_return {
            let result_context = BindingContext::new(
                program,
                &candidate.template.type_parameters,
                &candidate.template.const_parameters,
                callee.candidate_index,
            );
            infer_static_bindings(result_context, callee.return_type, actual, &mut inferred);
        }
        for proposal in inferred.types {
            if !proposals.types[..explicit_type_count]
                .iter()
                .any(|explicit| explicit.0 == proposal.0 && explicit.1 == proposal.1)
            {
                proposals.types.push(proposal);
            }
        }
        proposals.constants.extend(inferred.constants);
        for proposal in inferred.runtime_values {
            if !proposals
                .runtime_values
                .iter()
                .any(|explicit| explicit.0 == proposal.0 && explicit.1 == proposal.1)
            {
                proposals.runtime_values.push(proposal);
            }
        }
        ExplicitArgumentCapacity::WithinBounds
    }

    pub(crate) fn collect_contract_call_selections(
        program: &dyn ::validation::CheckingInput,
        scopes: &[ContractExpressionScope<'_>],
        candidates: &[Candidate],
        callee_states: &[CalleeState],
    ) -> Vec<CallSelection> {
        let mut selections = Vec::new();
        for occurrences in scopes.chunk_by(|left, right| left.expression == right.expression) {
            let [scope] = occurrences else {
                continue;
            };
            let Some(state) = scope.state else {
                continue;
            };
            let Some(call) = expressions_of(program).call_node(scope.expression) else {
                continue;
            };
            if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
                continue;
            }
            if let Some(selection) = selection_assembly::selection_for_call_with_local_prefix(
                program,
                scope.machine,
                state,
                candidates,
                callee_states,
                CallSite::Expression(scope.expression),
                call.target_symbol,
                &call.machine_arguments,
                expressions_of(program).expression_handles(call.arguments),
                None,
                program
                    .typed_program()
                    .callable_body_has_open_parameters(scope.machine),
                0,
            ) && selection.has_bound_arguments()
                && !const_arguments::has_pending_type_arguments(program, &selection.type_bindings)
            {
                upsert_selection(&mut selections, selection);
            }
        }
        selections
    }

    pub(crate) fn collect_call_selections(
        program: &dyn ::validation::CheckingInput,
        candidates: &[Candidate],
        callee_states: &[CalleeState],
        contract_expressions: &[ExpressionHandle],
    ) -> Vec<CallSelection> {
        let mut selections = Vec::new();
        let contract_expressions: HashSet<ExpressionHandle> =
            contract_expressions.iter().copied().collect();
        let mut covered_expressions = HashSet::new();
        let expected_calls: std::collections::HashMap<ExpressionHandle, TypeReferenceHandle> =
            validation::collect_expected_expression_calls(program)
                .into_iter()
                .collect();

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for handle in state.statement_nodes.handles() {
                    if program
                        .typed_program()
                        .static_statement_type_context_is_unreachable(handle)
                    {
                        continue;
                    }
                    match program.statement_table.statement(handle) {
                        StatementNode::Call(call)
                            if declaration_by_symbol(program, call.target_symbol).is_none() =>
                        {
                            if let Some(selection) = selection_for_call(
                                program,
                                machine,
                                state,
                                candidates,
                                callee_states,
                                CallSite::Statement(handle),
                                call.target_symbol,
                                &call.machine_arguments,
                                program.statement_table.expression_handles(call.arguments),
                                None,
                                program
                                    .typed_program()
                                    .callable_body_has_open_parameters(machine),
                            ) {
                                upsert_selection(&mut selections, selection);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut expressions = Vec::new();
                let mut visited_types = Vec::new();
                for reference in std::iter::once(state.return_type)
                    .chain(
                        program
                            .state_parameters(state)
                            .iter()
                            .map(|parameter| parameter.type_reference),
                    )
                    .chain(
                        program
                            .machine_owned_data(machine)
                            .iter()
                            .map(|owned| owned.type_reference),
                    )
                {
                    collect_type_expressions(
                        program,
                        reference,
                        &mut visited_types,
                        &mut expressions,
                    );
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(local) = statement.as_local_data() {
                        collect_type_expressions(
                            program,
                            local.type_reference,
                            &mut visited_types,
                            &mut expressions,
                        );
                    }
                    if !matches!(statement, StatementNode::AssemblyFact(_)) {
                        collect_statement_expression_trees(program, statement, &mut expressions);
                    }
                }
                for call in &program.proof_output_calls {
                    if call.machine_symbol == machine.symbol && call.state_symbol == state.symbol {
                        collect_expression_tree(program, call.call, &mut expressions);
                    }
                }
                for expression in expressions {
                    if covered_expressions.contains(&expression)
                        || contract_expressions.contains(&expression)
                    {
                        continue;
                    }

                    let Some(call) = expressions_of(program).call_node(expression) else {
                        continue;
                    };
                    if typed_trees::operator::resolve_named_expression_call(program, call).is_some()
                    {
                        continue;
                    }
                    covered_expressions.insert(expression);
                    if program
                        .typed_program()
                        .static_type_context_is_unreachable(expression)
                    {
                        continue;
                    }
                    if let Some(selection) = selection_for_call(
                        program,
                        machine,
                        state,
                        candidates,
                        callee_states,
                        CallSite::Expression(expression),
                        call.target_symbol,
                        &call.machine_arguments,
                        expressions_of(program).expression_handles(call.arguments),
                        expected_calls.get(&expression).copied().filter(|expected| {
                            program
                                .typed_program()
                                .callable_body_has_open_parameters(machine)
                                || !static_bindings::names_type_binder(program, *expected)
                        }),
                        program
                            .typed_program()
                            .callable_body_has_open_parameters(machine),
                    ) {
                        upsert_selection(&mut selections, selection);
                    }
                }
            }
        }

        let mut source_recipes = Vec::new();
        for declaration in program.const_declarations() {
            if declaration.authored_initializer.is_valid() {
                collect_expression_tree(
                    program,
                    declaration.authored_initializer,
                    &mut source_recipes,
                );
            }
        }
        for (expression, _) in expressions_of(program).iter_expressions() {
            let authored = expressions_of(program).materialization_source(expression);
            if authored.is_valid() {
                collect_expression_tree(program, authored, &mut source_recipes);
            }
        }
        let source_recipes: HashSet<ExpressionHandle> = source_recipes.into_iter().collect();

        for (handle, expression) in expressions_of(program).iter_expressions() {
            if covered_expressions.contains(&handle)
                || contract_expressions.contains(&handle)
                || source_recipes.contains(&handle)
            {
                continue;
            }
            let ExpressionNode::Call(call) = expression else {
                continue;
            };
            if typed_trees::operator::resolve_named_expression_call(program, call).is_some() {
                continue;
            }
            let Some(callee) = resolve_callee(program, callee_states, call.target_symbol) else {
                continue;
            };
            let candidate = &candidates[callee.candidate_index];
            let mut proposals = StaticProposals::default();
            let explicit_arguments = collect_machine_proposals_for_callee(
                program,
                candidates,
                callee,
                &call.machine_arguments,
                None,
                &mut proposals,
            );
            let lexical_owner =
                typed_trees::references::StaticArgumentCall::Expression(handle, call)
                    .lexical_owner(program);
            let caller_is_generic = program
                .machine_by_symbol(lexical_owner)
                .is_some_and(|owner| {
                    program
                        .typed_program()
                        .callable_body_has_open_parameters(owner)
                });
            let mut selection = selection_from_proposals(
                program,
                CallSite::Expression(handle),
                callee,
                candidate,
                caller_is_generic,
                proposals,
            );
            selection.explicit_argument_overflow = explicit_arguments.is_err();
            selection.lexical_owner = lexical_owner;
            upsert_selection(&mut selections, selection);
        }

        selections
    }
}

pub(crate) mod reflection {
    use crate::monomorphization::{
        CallSelection, CallSite, Candidate, Diagnostic, SymbolHandle, TypedTrees,
    };
    use crate::tables::{expressions_of, expressions_of_mut};
    use arena::HandleSpan;
    use language_core::ReferenceAccess;
    use symbols::{SymbolKind, SymbolTableBuilder};
    use typed_trees::data::{DataDefinition, DataField, DataMember};
    pub(crate) mod active_case {
        use super::*;
        use typed_trees::expression::{
            BinaryOperator, StaticMachineArgument, TableBinaryExpression,
        };
        use typed_trees::statement::{TableTransition, TransitionGuardNode, TransitionTargetNode};
        use typed_trees::{
            domain::ProofFact,
            machine::Machine,
            signature::{SignatureContract, SignatureContractKind, StateParameter, StateSignature},
            state::State,
        };

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn emit(
            program: &mut TypedTrees,
            owner: &Machine,
            source_state: SymbolHandle,
            declaration: SymbolHandle,
            subject: TypeReferenceHandle,
            context: TypeReferenceHandle,
            subject_data: &DataDefinition,
            graph: &validation::reflection::SemanticSchemaGraph,
            field_info: &DataDefinition,
            info_type: TypeReferenceHandle,
            owner_identity: &str,
            owner_application: &str,
            access: ReferenceAccess,
            captured_subject: SymbolHandle,
            captured_context: SymbolHandle,
            field_callback: &StaticMachineArgument,
            case_callback: &StaticMachineArgument,
            case_signature: &StateSignature,
            query: typed_trees::statement::StatementHandle,
            acknowledgement: language_semantics::CallOperationalAcknowledgement,
            replacement: &mut Vec<StatementNode>,
        ) -> Result<(), Vec<Diagnostic>> {
            let parameter = program
                .state_signature_parameters(case_signature)
                .get(1)
                .ok_or_else(|| error("active visitation lost its exact case description"))?;
            let case_type = program
                .type_reference_table
                .dereferenced_type_reference(parameter.type_reference);
            let case_info = program
                .closed_data_definition(case_type)
                .filter(|data| {
                    reflection_declaration(program, data.symbol) == Some(language_core::core_vocabulary::ReflectionDeclaration::CaseInfo)
                })
                .cloned()
                .ok_or_else(|| error("active visitation requires its exact CaseInfo vocabulary"))?;
            let case_info_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: case_info.symbol,
                    name: case_info.name.clone(),
                });
            let helper_name = format!(
                "__reflect_active#{}#{}",
                source_state.arena_index(),
                query.arena_index()
            );
            let helper_symbol = program.symbols.insert_generated_root_from(
                source_state,
                SymbolKind::Machine,
                &helper_name,
            );
            let variants = program
                .data_members(subject_data)
                .iter()
                .filter_map(|member| member.as_variant().cloned())
                .collect::<Vec<_>>();
            if variants.is_empty() {
                return Err(error("active visitation requires a sum subject"));
            }
            let state_names = std::iter::once("entry".to_owned())
                .chain(
                    variants
                        .iter()
                        .enumerate()
                        .map(|(index, _)| format!("case#{index}")),
                )
                .collect::<Vec<_>>();
            let symbols = program.symbols.insert_generated_children(
                helper_symbol,
                state_names
                    .iter()
                    .map(|name| (SymbolKind::State, name.as_str())),
            );
            let mut helper = Machine {
                symbol: helper_symbol,
                name: Identifier::generated(&helper_name),
                service_reach_row: owner.service_reach_row,
                service_reach_is_installation_bound: owner.service_reach_is_installation_bound,
                target: owner.target.clone(),
                authored_target: owner.authored_target.clone(),
                body_is_present: true,
                body_status: owner.body_status,
                ..Default::default()
            };
            let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
            let subject_type = reference(program, subject, access);
            let context_type = reference(program, context, ReferenceAccess::Mutable);
            let mut entry = State {
                symbol: symbols.handle_at(0),
                name: Identifier::generated("entry"),
                return_type: unit,
                ..Default::default()
            };
            let (entry_subject, entry_context) =
                parameters(program, &mut entry, subject_type, context_type);
            let mut dispatch = Vec::new();
            let marker_type = program
                .data_members(&case_info)
                .iter()
                .find_map(|member| {
                    member
                        .as_field()
                        .filter(|field| field.name.as_str() == "stable_number")
                        .map(|field| field.type_reference)
                })
                .ok_or_else(|| error("active visitation lost its marker type"))?;
            let marker_value = integer(program, 0)?;
            local(
                program,
                entry.symbol,
                declaration,
                "query_origin",
                marker_type,
                marker_value,
                &mut dispatch,
            )?;
            if let Some(StatementNode::LocalData(local)) = dispatch.last_mut() {
                local.reflected_query_origin = Some(query);
            }
            let mut cases = Vec::new();
            for (index, variant) in variants.iter().enumerate() {
                let symbol = symbols.handle_at(index as u32 + 1);
                let mut state = State {
                    symbol,
                    name: Identifier::generated(&state_names[index + 1]),
                    return_type: unit,
                    ..Default::default()
                };
                let (value, context) = parameters(program, &mut state, subject_type, context_type);
                let guard = case_guard(program, value, variant.symbol);
                let mut facts = HandleSpan::empty();
                program
                    .proof_facts
                    .append_to_span(&mut facts, ProofFact::Expression(guard));
                program.push_state_contract(
                    &mut state,
                    SignatureContract {
                        kind: SignatureContractKind::Requires,
                        facts,
                        ..Default::default()
                    },
                );
                let description = graph
                    .cases()
                    .find(|(_, case)| case.name == variant.name.as_str())
                    .map(|(_, case)| case.clone())
                    .ok_or_else(|| error("active visitation lost its selected case description"))?;
                let mut body = Vec::new();
                let mut fields = Vec::new();
                for (key, text) in [
                    ("member_identity", description.member_identity.as_str()),
                    ("owner_identity", owner_identity),
                    ("owner_application", owner_application),
                    ("name", description.name.as_str()),
                ] {
                    let value = expressions_of_mut(program)
                        .insert(ExpressionNode::String(text.as_bytes().into()));
                    fields.push(info_field(program, &case_info, key, value)?);
                }
                for (key, number) in [
                    (
                        "declaration_position",
                        description.declaration_position as u64,
                    ),
                    ("stable_number", description.stable_number.unwrap_or(0)),
                ] {
                    let value = integer(program, number)?;
                    fields.push(info_field(program, &case_info, key, value)?);
                }
                let has_number = expressions_of_mut(program)
                    .insert(ExpressionNode::Boolean(description.stable_number.is_some()));
                fields.push(info_field(
                    program,
                    &case_info,
                    "has_stable_number",
                    has_number,
                )?);
                let fields = expressions_of_mut(program).insert_struct_fields(fields);
                let info = expressions_of_mut(program).insert(ExpressionNode::StructLiteral(
                    TableStructLiteral {
                        type_name: case_info.name.clone(),
                        type_symbol: case_info.symbol,
                        fields,
                        ..Default::default()
                    },
                ));
                let info = local(
                    program,
                    state.symbol,
                    declaration,
                    "case_info",
                    case_info_type,
                    info,
                    &mut body,
                )?;
                let context_value = name(program, context);
                let context_value = borrow(program, context_value, ReferenceAccess::Mutable);
                let info_value = name(program, info);
                let info_value = borrow(program, info_value, ReferenceAccess::Shared);
                let arguments = program
                    .statement_table
                    .insert_expression_handles(vec![context_value, info_value]);
                body.push(StatementNode::Call(TableCall {
                    target_symbol: case_callback.symbol,
                    target: case_callback
                        .path
                        .last()
                        .cloned()
                        .ok_or_else(|| error("active case callback lost its exact operation"))?,
                    machine_arguments: case_callback
                        .application
                        .as_ref()
                        .map_or_else(Vec::new, |application| application.arguments.to_vec())
                        .into_boxed_slice(),
                    arguments,
                    operational_acknowledgement: acknowledgement,
                    reflected_field_origin: Some(
                        typed_trees::statement::ReflectedFieldCallOrigin {
                            query,
                            callback: case_callback.clone(),
                        },
                    ),
                    ..Default::default()
                }));
                let mut members = program
                    .data_members(subject_data)
                    .iter()
                    .enumerate()
                    .filter_map(|(position, member)| {
                        let DataMember::Field(field) = member else {
                            return None;
                        };
                        Some(
                            graph
                                .fields()
                                .find(|(_, description)| {
                                    description.declaration_position as usize == position
                                })
                                .map(|(_, description)| {
                                    (position, field.clone(), description.clone(), None, "")
                                }),
                        )
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| error("active visitation lost its common member"))?;
                let payload = program.data_payload_fields(variant).to_vec();
                for (position, field) in payload.into_iter().enumerate() {
                    let description_field = description
                        .payload_fields
                        .iter()
                        .find_map(|handle| {
                            let validation::reflection::SchemaNode::Field(field) =
                                graph.node(*handle)
                            else {
                                return None;
                            };
                            (field.declaration_position as usize == position)
                                .then_some(field.clone())
                        })
                        .ok_or_else(|| error("active visitation lost its exact payload member"))?;
                    members.push((
                        position,
                        field,
                        description_field,
                        Some(variant.name.clone()),
                        description.member_identity.as_str(),
                    ));
                }
                emit_fields(
                    program,
                    members,
                    state.symbol,
                    declaration,
                    field_info,
                    info_type,
                    owner_identity,
                    owner_application,
                    false,
                    access,
                    Some(value),
                    context,
                    field_callback,
                    query,
                    acknowledgement,
                    &mut body,
                )?;
                state.statement_nodes = program.statement_table.insert_statements(body);
                cases.push(state);
                let condition = case_guard(program, entry_subject, variant.symbol);
                let value = name(program, entry_subject);
                let value = borrow(program, value, access);
                let context = name(program, entry_context);
                let context = borrow(program, context, ReferenceAccess::Mutable);
                let arguments = program
                    .statement_table
                    .insert_expression_handles(vec![value, context]);
                let mut path = HandleSpan::empty();
                program.statement_table.push_name_path_member(
                    &mut path,
                    Identifier::generated(&state_names[index + 1]),
                );
                let target =
                    program
                        .statement_table
                        .insert_transition_target(TransitionTargetNode::Named {
                            operational_acknowledgement: acknowledgement,
                            path: typed_trees::statement::TableNamePath {
                                members: path,
                                head_symbol: symbol,
                                symbol,
                                ..Default::default()
                            },
                            static_machine_parameter: SymbolHandle::invalid(),
                            arguments,
                            evidence_arguments: Box::default(),
                            source_span: Default::default(),
                            authored_call_selection: None,
                        });
                dispatch.push(StatementNode::Transition(TableTransition {
                    guard: TransitionGuardNode::When(condition),
                    target,
                    ..Default::default()
                }));
            }
            entry.statement_nodes = program.statement_table.insert_statements(dispatch);
            program.push_machine_state(&mut helper, entry.clone());
            for case in cases {
                program.push_machine_state(&mut helper, case);
            }
            program.push_machine(helper);
            let value = name(program, captured_subject);
            let value = borrow(program, value, access);
            let context = name(program, captured_context);
            let context = borrow(program, context, ReferenceAccess::Mutable);
            let arguments = program
                .statement_table
                .insert_expression_handles(vec![value, context]);
            replacement.push(StatementNode::Call(TableCall {
                target_symbol: entry.symbol,
                target: Identifier::generated("entry"),
                arguments,
                operational_acknowledgement: acknowledgement,
                reflected_field_origin: Some(typed_trees::statement::ReflectedFieldCallOrigin {
                    query,
                    callback: StaticMachineArgument {
                        symbol: entry.symbol,
                        path: vec![
                            Identifier::generated(&helper_name),
                            Identifier::generated("entry"),
                        ]
                        .into_boxed_slice(),
                        ..Default::default()
                    },
                }),
                ..Default::default()
            }));
            Ok(())
        }
        fn parameters(
            program: &mut TypedTrees,
            state: &mut State,
            subject_type: TypeReferenceHandle,
            context_type: TypeReferenceHandle,
        ) -> (SymbolHandle, SymbolHandle) {
            let symbols = program.symbols.insert_generated_children(
                state.symbol,
                [
                    (SymbolKind::Parameter, "value"),
                    (SymbolKind::Parameter, "context"),
                ],
            );
            for (symbol, parameter_type, label) in [
                (symbols.handle_at(0), subject_type, "value"),
                (symbols.handle_at(1), context_type, "context"),
            ] {
                program.push_state_parameter(
                    state,
                    StateParameter {
                        symbol,
                        name: Identifier::generated(label),
                        type_reference: parameter_type,
                        ..Default::default()
                    },
                );
            }
            (symbols.handle_at(0), symbols.handle_at(1))
        }
        fn case_guard(
            program: &mut TypedTrees,
            subject: SymbolHandle,
            variant: SymbolHandle,
        ) -> ExpressionHandle {
            let left = name(program, subject);
            let right = name(program, variant);
            expressions_of_mut(program).insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left,
                operator: BinaryOperator::CaseMembership,
                right,
            }))
        }
    }
    pub(crate) mod expression_calls {
        use super::*;
        use typed_trees::{machine::Machine, signature::StateParameter, state::State};

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn materialize(
            program: &mut TypedTrees,
            owner_symbol: SymbolHandle,
            source_state: SymbolHandle,
            expression: ExpressionHandle,
            subject: TypeReferenceHandle,
            context: TypeReferenceHandle,
            inspection: bool,
            access: ReferenceAccess,
        ) -> Result<(), Vec<Diagnostic>> {
            let ExpressionNode::Call(original) =
                expressions_of(program).expression(expression).clone()
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
            let helper_symbol = program.symbols.insert_generated_root_from(
                source_state,
                SymbolKind::Machine,
                &helper_name,
            );
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
            let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(expression)
            else {
                return Err(error("typed field query lost its receiving expression"));
            };
            call.target_symbol = entry_symbol;
            call.target = Identifier::generated("entry");
            call.receiver = ExpressionHandle::invalid();
            call.machine_arguments = Box::default();
            call.authored_machine_application = Default::default();
            call.static_machine_parameter = SymbolHandle::invalid();
            call.static_requirement_dispatch = None;
            call.arguments = arguments;
            Ok(())
        }
    }
    pub(crate) mod owned_schema {
        use super::*;

        pub(crate) fn elaborate(
            program: &mut TypedTrees,
            candidate: &Candidate,
            selection: &CallSelection,
        ) -> Result<(), Vec<Diagnostic>> {
            let operands_are_empty = match selection.site {
                CallSite::Expression(expression) => {
                    let ExpressionNode::Call(call) = expressions_of(program).expression(expression)
                    else {
                        return Err(error("semantic schema lost its selected call occurrence"));
                    };
                    !call.receiver.is_valid()
                        && call.arguments.is_empty()
                        && call.evidence_arguments.is_empty()
                }
                CallSite::Statement(statement) => {
                    let StatementNode::Call(call) = program.statement_table.statement(statement)
                    else {
                        return Err(error("semantic schema lost its selected call occurrence"));
                    };
                    call.arguments.is_empty() && call.evidence_arguments.is_empty()
                }
            };
            if !operands_are_empty {
                return Err(error(
                    "semantic schema query accepts no runtime or evidence operands",
                ));
            }
            let [Some(subject)] = candidate.type_bindings.as_slice() else {
                return Err(error("semantic schema requires its exact selected subject"));
            };
            let subject = program
                .closed_data_definition(*subject)
                .cloned()
                .ok_or_else(|| {
                    error("semantic schema requires a closed semantic data application")
                })?;
            let author = selection.lexical_owner;
            let author_span = program
                .symbols
                .symbol_provenance_source_span(author)
                .ok_or_else(|| error("semantic schema requires its lexical source authority"))?;
            if !program
                .symbols
                .source_reference_may_select_symbol(author_span, subject.symbol)
            {
                return Err(error(
                    "semantic schema cannot select the subject from its lexical author's scope",
                ));
            }
            let requester_identity = program
                .normalized_hermetic_symbol_identity(author)
                .map_err(|message| error(format!("semantic schema author identity: {message}")))?;
            let authority = if program
                .symbols
                .same_symbol_source_package(author, subject.symbol)
            {
                validation::reflection::SchemaQueryAuthority::OwningScope { requester_identity }
            } else {
                validation::reflection::SchemaQueryAuthority::ForeignScope { requester_identity }
            };
            let graph = validation::reflection::construct_semantic_schema_graph(
                program,
                subject.symbol,
                authority,
            )
            .map_err(error)?;
            let query = &program.machines()[candidate.template.machine_index];
            let result = program
                .machine_states(query)
                .first()
                .ok_or_else(|| error("semantic schema lost its declared result"))?
                .return_type;
            let result = program
                .closed_data_definition(result)
                .cloned()
                .filter(|definition| {
                    reflection_declaration(program, definition.symbol) == Some(language_core::core_vocabulary::ReflectionDeclaration::TypeSchema)
                })
                .ok_or_else(|| {
                    error("semantic schema requires its toolchain-owned result vocabulary")
                })?;
            let bytes = expressions_of_mut(program)
                .insert(ExpressionNode::String(graph.runtime_image().into()));
            let field = info_field(program, &result, "storage", bytes)?;
            let fields = expressions_of_mut(program).insert_struct_fields([field]);
            let value = ExpressionNode::StructLiteral(TableStructLiteral {
                type_name: result.name.clone(),
                type_symbol: result.symbol,
                fields,
                ..Default::default()
            });
            let authored = authored_query(program, selection.site)?;
            let table = expressions_of_mut(program);
            let materialized = table.insert(value);
            table.set_source_span(materialized, table.source_span(authored));
            let occurrences = table
                .authored_selection_occurrences(authored)
                .collect::<Vec<_>>();
            table.attach_authored_selection_occurrences(materialized, occurrences);
            table.retain_materialization_source(materialized, authored);
            match selection.site {
                CallSite::Expression(expression) => {
                    table.replace_with_existing(expression, materialized)
                }
                CallSite::Statement(statement) => {
                    *program.statement_table.statement_mut(statement) =
                        StatementNode::Expression(materialized);
                }
            }
            Ok(())
        }

        fn authored_query(
            program: &mut TypedTrees,
            site: CallSite,
        ) -> Result<ExpressionHandle, Vec<Diagnostic>> {
            match site {
                CallSite::Expression(expression) => {
                    Ok(expressions_of_mut(program).insert_copy(expression))
                }
                CallSite::Statement(statement) => {
                    let Some(call) = program
                        .statement_table
                        .statement(statement)
                        .as_call()
                        .cloned()
                    else {
                        return Err(error("semantic schema lost its authored statement query"));
                    };
                    let arguments = program
                        .statement_table
                        .expression_handles(call.arguments)
                        .to_vec();
                    let table = expressions_of_mut(program);
                    let arguments = table.insert_expression_handles(arguments);
                    let occurrence = call.authored_call_selection;
                    let source_span = call.source_span;
                    let expression = table.insert(ExpressionNode::Call(
                        typed_trees::expression::TableCallExpression {
                            authored_call_selection: occurrence,
                            authored_call_binding: None,
                            authored_machine_application: call.authored_machine_application,
                            build_selection: Default::default(),
                            exact_coercion_target: Default::default(),
                            requirement_application: Default::default(),
                            receiver: Default::default(),
                            target_symbol: call.target_symbol,
                            target: call.target,
                            static_machine_parameter: call.static_machine_parameter,
                            static_requirement_dispatch: call.static_requirement_dispatch,
                            machine_arguments: call.machine_arguments,
                            quotient_operation: None,
                            private_layout_operation: None,
                            installed_interrupt_operation: None,
                            layout_query: None,
                            native_code_layout: None,
                            arguments,
                            evidence_arguments: call.evidence_arguments,
                            operational_acknowledgement: call.operational_acknowledgement,
                        },
                    ));
                    table.set_source_span(expression, source_span);
                    table.attach_authored_selection_occurrences(expression, occurrence);
                    Ok(expression)
                }
            }
        }
    }
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, TableBorrowExpression, TableMemberExpression,
        TableNamePath, TableStructLiteral, TableStructLiteralField,
    };
    use typed_trees::name::Identifier;
    use typed_trees::statement::{StatementNode, TableCall, TableLocalData};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    fn error(message: impl Into<String>) -> Vec<Diagnostic> {
        vec![Diagnostic::error(message)]
    }

    /*
        The `reflect.omg` declaration `symbol` is, from the core vocabulary
        catalog. Reflection elaboration only rewrites calls and types this
        names; a lookalike user declaration stays ordinary.
    */
    fn reflection_declaration(
        program: &TypedTrees,
        symbol: SymbolHandle,
    ) -> Option<language_core::core_vocabulary::ReflectionDeclaration> {
        match program.symbols.core_vocabulary(symbol) {
            Some(language_core::core_vocabulary::CoreVocabulary::Reflection(declaration)) => {
                Some(declaration)
            }
            _ => None,
        }
    }

    pub(crate) fn elaborate_calls(
        program: &mut TypedTrees,
        candidates: &[Candidate],
        selections: &[CallSelection],
        changed_owners: &mut Vec<SymbolHandle>,
    ) -> Result<bool, Vec<Diagnostic>> {
        for selection in selections
            .iter()
            .filter(|selection| selection.is_complete())
        {
            let candidate = crate::monomorphization::candidate_for_selection(
                program,
                &candidates[selection.candidate_index],
                selection,
            );
            let declaration = candidate.template.template_symbol;
            let Some(reflection) = reflection_declaration(program, declaration) else {
                continue;
            };
            if reflection == language_core::core_vocabulary::ReflectionDeclaration::Schema {
                owned_schema::elaborate(program, &candidate, selection)?;
                changed_owners.push(selection.lexical_owner);
                return Ok(true);
            }
            let inspection = reflection == language_core::core_vocabulary::ReflectionDeclaration::InspectRecordFields;
            let active = matches!(
                reflection,
                language_core::core_vocabulary::ReflectionDeclaration::VisitActiveFields | language_core::core_vocabulary::ReflectionDeclaration::VisitActiveFieldsMut
            );
            let field_access = match reflection {
                language_core::core_vocabulary::ReflectionDeclaration::VisitRuntimeFields
                | language_core::core_vocabulary::ReflectionDeclaration::VisitActiveFields
                | language_core::core_vocabulary::ReflectionDeclaration::InspectRecordFields => ReferenceAccess::Shared,
                language_core::core_vocabulary::ReflectionDeclaration::VisitRuntimeFieldsMut | language_core::core_vocabulary::ReflectionDeclaration::VisitActiveFieldsMut => {
                    ReferenceAccess::Mutable
                }
                _ => continue,
            };
            let [Some(subject), Some(context)] = candidate.type_bindings.as_slice() else {
                return Err(error(
                    "typed field visitation requires its exact subject and context types",
                ));
            };
            let site = match selection.site {
                CallSite::Statement(site) => site,
                CallSite::Expression(expression) => {
                    expression_calls::materialize(
                        program,
                        selection.lexical_owner,
                        selection.lexical_state,
                        expression,
                        *subject,
                        *context,
                        inspection,
                        field_access,
                    )?;
                    changed_owners.push(selection.lexical_owner);
                    return Ok(true);
                }
            };
            let StatementNode::Call(call) = program.statement_table.statement(site).clone() else {
                return Err(error("typed field visitation lost its call occurrence"));
            };
            let (case_callback, callback) = match candidate.machine_bindings.as_slice() {
                [Some(case_callback), Some(callback)] if active => (Some(case_callback), callback),
                [Some(callback)] if !active => (None, callback),
                _ => {
                    return Err(error(
                        "typed field visitation requires its explicitly selected callbacks",
                    ));
                }
            };
            let Some(subject_data) = program.closed_data_definition(*subject).cloned() else {
                return Err(error(
                    "typed field visitation requires a closed semantic data application",
                ));
            };
            let author_span = program
                .symbols
                .symbol_provenance_source_span(selection.lexical_owner)
                .ok_or_else(|| {
                    error("typed field visitation requires its lexical source authority")
                })?;
            if !program
                .symbols
                .source_reference_may_select_symbol(author_span, subject_data.symbol)
                || (!program
                    .symbols
                    .same_symbol_source_package(selection.lexical_owner, subject_data.symbol)
                    && !subject_data.is_public)
            {
                return Err(error(
                    "typed field visitation cannot select the subject from its lexical author's scope",
                ));
            }
            if !program
                .symbols
                .same_symbol_source_package(selection.lexical_owner, subject_data.symbol)
            {
                return Err(error(
                    "typed field visitation requires its subject owner; delegate an owning operation instead of reflecting a foreign carrier",
                ));
            }
            let owner_identity = program
                .normalized_hermetic_symbol_identity(subject_data.symbol)
                .map_err(|message| {
                    error(format!(
                        "typed field visitation subject identity: {message}"
                    ))
                })?;
            let requester_identity = program
                .normalized_hermetic_symbol_identity(selection.lexical_owner)
                .map_err(|message| {
                    error(format!("typed field visitation author identity: {message}"))
                })?;
            let authority = if program
                .symbols
                .same_symbol_source_package(selection.lexical_owner, subject_data.symbol)
            {
                validation::reflection::SchemaQueryAuthority::OwningScope { requester_identity }
            } else {
                validation::reflection::SchemaQueryAuthority::ForeignScope { requester_identity }
            };
            let graph = validation::reflection::construct_semantic_schema_graph(
                program,
                subject_data.symbol,
                authority,
            )
            .map_err(error)?;
            if !active && graph.cases().next().is_some() {
                return Err(error(
                    "sum visitation requires an explicitly selected active-case callback",
                ));
            }
            let owner_application = program.package_qualified_type_identity_text(*subject);
            let members = program.data_members(&subject_data).to_vec();
            let signature = &candidate.template.machine_parameters[usize::from(active)].2;
            let metadata = program
                .state_signature_parameters(signature)
                .get(1)
                .ok_or_else(|| error("typed field visitation lost its metadata parameter"))?;
            let metadata_type = program
                .type_reference_table
                .dereferenced_type_reference(metadata.type_reference);
            let field_info = program
                .closed_data_definition(metadata_type)
                .filter(|definition| {
                    reflection_declaration(program, definition.symbol) == Some(language_core::core_vocabulary::ReflectionDeclaration::FieldInfo)
                })
                .cloned()
                .ok_or_else(|| {
                    error("typed field visitation requires its exact FieldInfo vocabulary")
                })?;
            let owner = program
                .machine_by_symbol(selection.lexical_owner)
                .cloned()
                .ok_or_else(|| error("typed field visitation lost its lexical machine"))?;
            let state_index = program
                .machine_states(&owner)
                .iter()
                .position(|state| state.symbol == selection.lexical_state)
                .ok_or_else(|| error("typed field visitation lost its lexical state"))?;
            let state = program.machine_states(&owner)[state_index].clone();
            let operands = program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec();
            let (subject_operand, context_operand) = match operands.as_slice() {
                [context] if inspection => (None, *context),
                [subject, context] if !inspection => (Some(*subject), *context),
                _ => return Err(error("typed field query has invalid operands")),
            };
            let mut replacement = Vec::new();
            let captured_subject = if let Some(subject_operand) = subject_operand {
                let subject_type = reference(program, *subject, field_access);
                let subject_operand = reborrow_operand(program, subject_operand, field_access);
                Some(local(
                    program,
                    state.symbol,
                    declaration,
                    "subject",
                    subject_type,
                    subject_operand,
                    &mut replacement,
                )?)
            } else {
                None
            };
            let context_type = reference(program, *context, ReferenceAccess::Mutable);
            let context_operand =
                reborrow_operand(program, context_operand, ReferenceAccess::Mutable);
            let captured_context = local(
                program,
                state.symbol,
                declaration,
                "context",
                context_type,
                context_operand,
                &mut replacement,
            )?;
            let Some(StatementNode::LocalData(context_capture)) = replacement.last_mut() else {
                return Err(error("typed field query lost its context capture"));
            };
            context_capture.reflected_query_origin = Some(site);
            let info_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: field_info.symbol,
                    name: field_info.name.clone(),
                });
            let fields = members
                .iter()
                .enumerate()
                .filter_map(|(position, member)| {
                    let DataMember::Field(field) = member else {
                        return None;
                    };
                    let description = graph
                        .fields()
                        .find(|(_, description)| {
                            description.declaration_position as usize == position
                        })
                        .map(|(_, description)| description.clone());
                    Some(
                        description
                            .map(|description| (position, field.clone(), description, None, "")),
                    )
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| error("typed field visitation lost its exact schema member"))?;
            if let Some(case_callback) = case_callback {
                active_case::emit(
                    program,
                    &owner,
                    state.symbol,
                    declaration,
                    *subject,
                    *context,
                    &subject_data,
                    &graph,
                    &field_info,
                    info_type,
                    &owner_identity,
                    &owner_application,
                    field_access,
                    captured_subject.ok_or_else(|| error("active visitation needs its subject"))?,
                    captured_context,
                    callback,
                    case_callback,
                    &candidate.template.machine_parameters[0].2,
                    site,
                    call.operational_acknowledgement,
                    &mut replacement,
                )?;
            } else {
                emit_fields(
                    program,
                    fields,
                    state.symbol,
                    declaration,
                    &field_info,
                    info_type,
                    &owner_identity,
                    &owner_application,
                    inspection,
                    field_access,
                    captured_subject,
                    captured_context,
                    callback,
                    site,
                    call.operational_acknowledgement,
                    &mut replacement,
                )?;
            }
            let statements = program
                .statement_table
                .statements(state.statement_nodes)
                .to_vec();
            let mut body = HandleSpan::empty();
            for (position, statement) in statements.into_iter().enumerate() {
                let original = state.statement_nodes.handle_at(position as u32);
                if original == site {
                    for generated in replacement.drain(..) {
                        program.statement_table.push_statement(&mut body, generated);
                    }
                } else {
                    program.statement_table.push_statement(&mut body, statement);
                    let target = body.handle_at(body.count() - 1);
                    expressions_of_mut(program)
                        .relocate_statement_operand_captures(original, target);
                }
            }
            program.machine_states_mut(&owner)[state_index].statement_nodes = body;
            changed_owners.push(owner.symbol);
            return Ok(true);
        }
        Ok(false)
    }

    fn reborrow_operand(
        program: &mut TypedTrees,
        operand: ExpressionHandle,
        access: ReferenceAccess,
    ) -> ExpressionHandle {
        if let ExpressionNode::Borrow(loan) = expressions_of(program).expression(operand) {
            return if loan.access == ReferenceAccess::Mutable && access == ReferenceAccess::Shared {
                borrow(program, loan.target, ReferenceAccess::Shared)
            } else {
                operand
            };
        }
        if validation::canonical_place_from_expression(program, operand)
            .is_some_and(|place| place.root.as_symbol().is_some())
        {
            borrow(program, operand, access)
        } else {
            operand
        }
    }

    fn reference(
        program: &mut TypedTrees,
        referee: TypeReferenceHandle,
        access: ReferenceAccess,
    ) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access,
                lifetime: None,
            })
    }

    fn name(program: &mut TypedTrees, symbol: SymbolHandle) -> ExpressionHandle {
        let identifier = Identifier::generated(program.symbols.name(symbol));
        let mut members = HandleSpan::empty();
        expressions_of_mut(program).push_name_path_member(&mut members, identifier);
        let mut member_symbols = HandleSpan::empty();
        expressions_of_mut(program).push_name_path_member_symbol(&mut member_symbols, symbol);
        expressions_of_mut(program).insert(ExpressionNode::Name(TableNamePath::new(
            members,
            member_symbols,
            symbol,
            symbol,
        )))
    }

    fn borrow(
        program: &mut TypedTrees,
        target: ExpressionHandle,
        access: ReferenceAccess,
    ) -> ExpressionHandle {
        expressions_of_mut(program).insert(ExpressionNode::Borrow(TableBorrowExpression {
            target,
            access,
        }))
    }

    fn local(
        program: &mut TypedTrees,
        state: SymbolHandle,
        origin: SymbolHandle,
        role: &str,
        type_reference: TypeReferenceHandle,
        initial_value: ExpressionHandle,
        statements: &mut Vec<StatementNode>,
    ) -> Result<SymbolHandle, Vec<Diagnostic>> {
        let local_name = format!(
            "__reflect_{role}#{}",
            expressions_of(program).expression_count()
        );
        let children = program.symbols.append_generated_children_from(
            state,
            [(origin, SymbolKind::Local, local_name.as_str())],
        );
        let symbol = SymbolTableBuilder::child_handles(children)
            .next()
            .ok_or_else(|| {
                error("typed field visitation could not allocate its operand capture")
            })?;
        statements.push(StatementNode::LocalData(TableLocalData {
            symbol,
            name: Identifier::generated(local_name),
            type_reference,
            initial_value,
            ..Default::default()
        }));
        Ok(symbol)
    }

    fn info_field(
        program: &TypedTrees,
        definition: &typed_trees::data::DataDefinition,
        field_name: &str,
        value: ExpressionHandle,
    ) -> Result<TableStructLiteralField, Vec<Diagnostic>> {
        let field = program
            .data_members(definition)
            .iter()
            .find_map(|member| {
                member
                    .as_field()
                    .filter(|field| field.name.as_str() == field_name)
            })
            .ok_or_else(|| {
                error("typed field visitation has an incomplete FieldInfo vocabulary")
            })?;
        Ok(TableStructLiteralField {
            name: field.name.clone(),
            field_symbol: field.symbol,
            value,
        })
    }

    fn integer(program: &mut TypedTrees, value: u64) -> Result<ExpressionHandle, Vec<Diagnostic>> {
        let literal = numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            &value.to_string(),
        )
        .map_err(error)?;
        Ok(expressions_of_mut(program).insert(ExpressionNode::Integer(literal)))
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_fields(
        program: &mut TypedTrees,
        fields: Vec<(
            usize,
            DataField,
            validation::reflection::FieldDescription,
            Option<Identifier>,
            &str,
        )>,
        state: SymbolHandle,
        declaration: SymbolHandle,
        field_info: &DataDefinition,
        info_type: TypeReferenceHandle,
        owner_identity: &str,
        owner_application: &str,
        inspection: bool,
        field_access: ReferenceAccess,
        captured_subject: Option<SymbolHandle>,
        captured_context: SymbolHandle,
        callback: &typed_trees::expression::StaticMachineArgument,
        site: typed_trees::statement::StatementHandle,
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
        replacement: &mut Vec<StatementNode>,
    ) -> Result<(), Vec<Diagnostic>> {
        for (position, field, description, case_variant, case_identity) in fields {
            if field.relevance.is_erased() && !inspection {
                continue;
            }
            let text_values = [
                ("member_identity", description.member_identity.as_str()),
                ("owner_identity", owner_identity),
                ("owner_application", owner_application),
                ("name", description.name.as_str()),
                ("qualified_type", description.qualified_type.as_str()),
                ("case_identity", case_identity),
            ];
            let mut info_fields = Vec::new();
            for (field_name, text) in text_values {
                let value = expressions_of_mut(program)
                    .insert(ExpressionNode::String(text.as_bytes().into()));
                info_fields.push(info_field(program, field_info, field_name, value)?);
            }
            let position_value = integer(program, position as u64)?;
            info_fields.push(info_field(
                program,
                field_info,
                "declaration_position",
                position_value,
            )?);
            let has_number = expressions_of_mut(program)
                .insert(ExpressionNode::Boolean(field.identity.is_some()));
            info_fields.push(info_field(
                program,
                field_info,
                "has_stable_number",
                has_number,
            )?);
            let erased = expressions_of_mut(program)
                .insert(ExpressionNode::Boolean(field.relevance.is_erased()));
            info_fields.push(info_field(program, field_info, "is_erased", erased)?);
            let number = integer(program, field.identity.unwrap_or(0))?;
            info_fields.push(info_field(program, field_info, "stable_number", number)?);
            let fields = expressions_of_mut(program).insert_struct_fields(info_fields);
            let info = expressions_of_mut(program).insert(ExpressionNode::StructLiteral(
                TableStructLiteral {
                    type_name: field_info.name.clone(),
                    type_symbol: field_info.symbol,
                    fields,
                    ..Default::default()
                },
            ));
            let info = local(
                program,
                state,
                declaration,
                "field_info",
                info_type,
                info,
                replacement,
            )?;
            let context_value = name(program, captured_context);
            let context_value = borrow(program, context_value, ReferenceAccess::Mutable);
            let info_value = name(program, info);
            let info_value = borrow(program, info_value, ReferenceAccess::Shared);
            let mut values = vec![context_value, info_value];
            if let Some(captured_subject) = captured_subject {
                let subject_value = name(program, captured_subject);
                let field_value = expressions_of_mut(program).insert(ExpressionNode::Member(
                    TableMemberExpression {
                        receiver: subject_value,
                        member_symbol: field.symbol,
                        member: field.name.clone(),
                        case_variant,
                    },
                ));
                let field_loan_type = reference(program, field.type_reference, field_access);
                let field_loan = borrow(program, field_value, field_access);
                let field_loan = local(
                    program,
                    state,
                    declaration,
                    "field_loan",
                    field_loan_type,
                    field_loan,
                    replacement,
                )?;
                values.push(name(program, field_loan));
            }
            let arguments = program.statement_table.insert_expression_handles(values);
            let mut machine_arguments = callback
                .application
                .as_ref()
                .map_or_else(Vec::new, |application| application.arguments.to_vec());
            machine_arguments.push(
                program
                    .static_operand_from_type_reference(field.type_reference)
                    .map_err(error)?,
            );
            replacement.push(StatementNode::Call(TableCall {
                target_symbol: callback.symbol,
                target: callback
                    .path
                    .last()
                    .cloned()
                    .ok_or_else(|| error("typed field callback has no selected name"))?,
                machine_arguments: machine_arguments.into_boxed_slice(),
                arguments,
                operational_acknowledgement,
                reflected_field_origin: Some(typed_trees::statement::ReflectedFieldCallOrigin {
                    query: site,
                    callback: callback.clone(),
                }),
                ..Default::default()
            }));
        }
        Ok(())
    }
}

pub(crate) mod result_locals {
    use crate::tables::expressions_of;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::references::DeclarationSelection;
    use typed_trees::statement::{StatementNode, TableLocalData};
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::type_identity::applied_type_identity;
    use typed_trees::types::TypeReferenceHandle;
    use validation::expression_result_type_reference;

    pub(crate) mod dispatch_subjects {

        use crate::tables::{expressions_of, expressions_of_mut};
        use arena::HandleSpan;
        use language_core::{LocalBindingRole, Multiplicity, ReferenceAccess};
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::expression::{
            ExpressionHandle, ExpressionNode, TableBorrowExpression, TableMemberExpression,
            TableNamePath,
        };
        use typed_trees::statement::{StatementNode, StatementRootRole};
        use typed_trees::types::TypeReferenceNode;
        use validation::expression_traversal::collect_expression_tree;
        use validation::has_anonymous_operator_meaning;

        pub(crate) fn materialize(program: &mut TypedTrees) -> Vec<SymbolHandle> {
            let mut pending = Vec::new();
            let mut aliases = Vec::new();
            let mut guards = Vec::new();
            let mut logical = Vec::new();
            let mut proof_only = None;
            for machine in program.machines() {
                for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                    for (offset, local) in super::local_bindings(program, state.statement_nodes) {
                        if local.role != LocalBindingRole::DispatchSubject {
                            continue;
                        }
                        let actual = completed_subject_type(
                            program,
                            machine,
                            state,
                            local.initial_value,
                            &mut Vec::new(),
                        );
                        if actual.is_none()
                            && validation::evaluate_anonymous_numeric_expression(
                                program,
                                local.initial_value,
                                &[],
                                |expression| has_anonymous_operator_meaning(program, expression),
                            )
                            .is_some()
                        {
                            logical.push((
                                machine.clone(),
                                state_index,
                                state.statement_nodes,
                                offset,
                                local.symbol,
                                local.initial_value,
                            ));
                            continue;
                        }
                        let Some(actual) = actual else {
                            continue;
                        };
                        let storage = validation::canonical_place_from_expression_in_state(
                            program,
                            state.symbol,
                            offset,
                            local.initial_value,
                        )
                        .is_some_and(|place| place.root.as_symbol().is_some());
                        let case_storage = program.pattern_component(local.initial_value)
                            == Some(language_core::PatternComponent::Case)
                            && validation::reference_shell(program, actual).is_none();
                        // A case view observes the same home even when the containing
                        // data is copyable. An existing reference keeps its descriptor.
                        let noncopy_storage = storage
                            && (program.type_multiplicity(actual) != Multiplicity::Unrestricted
                                || case_storage);
                        let (mathematical, logical_machine) = if noncopy_storage {
                            let classification = proof_only
                                .get_or_insert_with(|| typed_trees::proof_only::classify(program));
                            let mathematical =
                                classification.proof_only_mention(program, actual).is_some();
                            (
                                mathematical,
                                mathematical && classification.is_proof_machine(program, machine),
                            )
                        } else {
                            (false, false)
                        };
                        if logical_machine
                            && !validation::type_carries_linear_obligation(program, actual)
                            && validation::place_has_immutable_denotation(
                                program,
                                machine,
                                state,
                                offset,
                                local.initial_value,
                            )
                        {
                            logical.push((
                                machine.clone(),
                                state_index,
                                state.statement_nodes,
                                offset,
                                local.symbol,
                                local.initial_value,
                            ));
                            continue;
                        }
                        let observation = if storage {
                            (noncopy_storage
                                && !mathematical
                                && validation::place_has_builtin_coordinates(
                                    program,
                                    machine,
                                    Some(state),
                                    local.initial_value,
                                ))
                            .then_some(actual)
                        } else {
                            None
                        };
                        if observation.is_some() {
                            aliases.push((local.symbol, local.initial_value));
                        }
                        pending.push((
                            machine.symbol,
                            state.statement_nodes,
                            offset,
                            local.initial_value,
                            observation,
                        ));
                    }
                }
            }
            if pending.is_empty() && logical.is_empty() {
                return Vec::new();
            }
            if !aliases.is_empty() {
                for machine in program.machines() {
                    for state in program.machine_states(machine) {
                        if !pending.iter().any(|(_, body, _, _, observation)| {
                            *body == state.statement_nodes && observation.is_some()
                        }) {
                            continue;
                        }
                        for statement in program.statement_table.statements(state.statement_nodes) {
                            for (role, root) in program.tables.expression_roots(statement) {
                                if role == StatementRootRole::TransitionGuard {
                                    let mut observed = Vec::new();
                                    collect_expression_tree(program, root, &mut observed);
                                    guards.extend(observed);
                                }
                            }
                        }
                    }
                }
            }
            aliases.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
            guards.sort_by_key(|expression| (expression.arena_index(), expression.generation()));
            guards.dedup();
            replace_place_aliases(program, &aliases, &guards);
            let logical_aliases = logical
                .iter()
                .map(|(_, _, _, _, symbol, initial)| (*symbol, *initial))
                .collect::<Vec<_>>();
            let logical_aliases = normalized_place_aliases(program, &logical_aliases);
            replace_place_aliases(program, &logical_aliases, &[]);
            let mut changed = Vec::new();
            for (owner, body, offset, initial, observation) in pending {
                let observed = observation.map(|referee| {
                    let expression = expressions_of_mut(program).insert(ExpressionNode::Borrow(
                        TableBorrowExpression {
                            target: initial,
                            access: ReferenceAccess::Shared,
                        },
                    ));
                    let span = expressions_of(program).source_span(initial);
                    expressions_of_mut(program).set_source_span(expression, span);
                    let reference =
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::Reference {
                                referee,
                                access: ReferenceAccess::Shared,
                                lifetime: None,
                            });
                    (expression, reference)
                });
                let StatementNode::LocalData(local) =
                    &mut program.statement_table.statements_mut(body)[offset]
                else {
                    unreachable!()
                };
                local.role = if observed.is_some() {
                    LocalBindingRole::DispatchObservation
                } else {
                    LocalBindingRole::Value
                };
                if let Some((expression, reference)) = observed {
                    local.initial_value = expression;
                    local.type_reference = reference;
                }
                changed.push(owner);
            }
            for (machine, state_index, old_body, _, _, _) in &logical {
                if program.machine_states(machine)[*state_index].statement_nodes != *old_body {
                    continue;
                }
                let statements = program.statement_table.statements(*old_body).to_vec();
                let mut body = HandleSpan::empty();
                for (offset, statement) in statements.into_iter().enumerate() {
                    if !logical.iter().any(|(_, _, candidate, position, _, _)| {
                        candidate == old_body && *position == offset
                    }) {
                        program.statement_table.push_statement(&mut body, statement);
                    }
                }
                program.machine_states_mut(machine)[*state_index].statement_nodes = body;
                changed.push(machine.symbol);
            }
            changed
        }

        fn completed_subject_type(
            program: &TypedTrees,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            expression: ExpressionHandle,
            seen: &mut Vec<ExpressionHandle>,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            if !expression.is_valid() || seen.contains(&expression) {
                return None;
            }
            seen.push(expression);
            let actual =
                validation::expression_result_type_reference(program, machine, state, expression)?;
            if matches!(
                program.type_reference_table.type_reference(actual),
                typed_trees::types::TypeReferenceNode::Unit
            ) && let ExpressionNode::Name(path) = expressions_of(program).expression(expression)
                && let Some((_, local)) = path
                    .resolved_symbol()
                    .and_then(|symbol| program.state_local_declaration(state, symbol))
                && local.type_is_inferred
                && matches!(
                    program
                        .type_reference_table
                        .type_reference(local.type_reference),
                    typed_trees::types::TypeReferenceNode::Unit
                )
            {
                return completed_subject_type(program, machine, state, local.initial_value, seen);
            }
            Some(actual)
        }

        fn replace_place_aliases(
            program: &mut TypedTrees,
            aliases: &[(SymbolHandle, ExpressionHandle)],
            guards: &[ExpressionHandle],
        ) {
            if aliases.is_empty() {
                return;
            }
            let occurrences = expressions_of(program)
                .expression_entries()
                .filter_map(|(expression, _)| {
                    if guards
                        .binary_search_by_key(
                            &(expression.arena_index(), expression.generation()),
                            |guard| (guard.arena_index(), guard.generation()),
                        )
                        .is_ok()
                    {
                        return None;
                    }
                    let (path, source) = place_alias_replacement(program, aliases, expression)?;
                    Some((expression, path, source))
                })
                .collect::<Vec<_>>();
            for (expression, path, source) in occurrences {
                replace_place_alias(program, expression, path, source);
            }
        }

        fn normalized_place_aliases(
            program: &mut TypedTrees,
            aliases: &[(SymbolHandle, ExpressionHandle)],
        ) -> Vec<(SymbolHandle, ExpressionHandle)> {
            let mut normalized = Vec::with_capacity(aliases.len());
            for &(symbol, source) in aliases {
                let source = expressions_of_mut(program).insert_copy(source);
                let mut nodes = Vec::new();
                collect_expression_tree(program, source, &mut nodes);
                for expression in nodes {
                    if let Some((path, source)) =
                        place_alias_replacement(program, &normalized, expression)
                    {
                        replace_place_alias(program, expression, path, source);
                    }
                }
                let position = normalized
                    .binary_search_by_key(
                        &(symbol.arena_index(), symbol.generation()),
                        |(candidate, _)| (candidate.arena_index(), candidate.generation()),
                    )
                    .unwrap_or_else(|position| position);
                normalized.insert(position, (symbol, source));
            }
            normalized
        }

        fn place_alias_replacement(
            program: &TypedTrees,
            aliases: &[(SymbolHandle, ExpressionHandle)],
            expression: ExpressionHandle,
        ) -> Option<(TableNamePath, ExpressionHandle)> {
            let ExpressionNode::Name(path) = expressions_of(program).expression(expression) else {
                return None;
            };
            let symbol = path.resolved_symbol()?;
            let index = aliases
                .binary_search_by_key(
                    &(symbol.arena_index(), symbol.generation()),
                    |(candidate, _)| (candidate.arena_index(), candidate.generation()),
                )
                .ok()?;
            Some((*path, aliases[index].1))
        }

        fn replace_place_alias(
            program: &mut TypedTrees,
            expression: ExpressionHandle,
            path: TableNamePath,
            source: ExpressionHandle,
        ) {
            // The place substitution preserves this occurrence's component, including
            // a reference-local alias whose declaration already retained a case view.
            let Some(component) = program.pattern_component(expression) else {
                return;
            };
            // Reuse the ordinary graph copier, then compose projections on that exact
            // captured source. Index operands remain typed occurrences; no index is
            // encoded in a generated name or evaluated again while adding the suffix.
            let mut source = if path.members.count() > 1 {
                expressions_of_mut(program).insert_copy(source)
            } else {
                source
            };
            let occurrences = expressions_of(program)
                .authored_selection_occurrences(source)
                .collect::<Vec<_>>();
            for offset in 1..path.members.count() as usize {
                let member =
                    expressions_of(program).name_path_members(path.members)[offset].clone();
                let member_symbol = expressions_of(program)
                    .name_path_member_symbols(path.member_symbols)
                    .get(offset)
                    .copied()
                    .unwrap_or_default();
                source = expressions_of_mut(program).insert(ExpressionNode::Member(
                    TableMemberExpression {
                        receiver: source,
                        member_symbol,
                        member,
                        case_variant: None,
                    },
                ));
            }
            expressions_of_mut(program).attach_authored_selection_occurrences(source, occurrences);
            expressions_of_mut(program).replace_with_existing(expression, source);
            expressions_of_mut(program).set_pattern_component(expression, component);
        }

        pub(crate) fn require_complete(
            program: &TypedTrees,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            let mut errors = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        let Some(local) = statement.as_local_data() else {
                            continue;
                        };
                        if local.role == LocalBindingRole::DispatchSubject {
                            errors.push(
                            diagnostics::Diagnostic::error(format!(
                                "cannot determine the dispatch subject storage type in `{}::{}`",
                                machine.name.as_str(),
                                state.name.as_str(),
                            ))
                            .with_source_span(
                                expressions_of(program).source_span(local.initial_value),
                            ),
                        );
                        }
                    }
                }
            }
            if errors.is_empty() {
                Ok(())
            } else {
                let mut formation_errors = Vec::new();
                validation::validate_anonymous_numeric_formation(program, &mut formation_errors);
                let symbols = validation::TopLevelSymbols::build(program, &mut Vec::new());
                for machine in program.machines() {
                    for state in program.machine_states(machine) {
                        let statements = program.statement_table.statements(state.statement_nodes);
                        for (index, statement) in statements.iter().enumerate() {
                            let Some(initial) = statement.as_local_data().and_then(|local| {
                                (local.role == LocalBindingRole::DispatchSubject)
                                    .then_some(local.initial_value)
                            }) else {
                                continue;
                            };
                            validation::expression_traversal::record_unresolved_subject_source_obligations(
                        program,
                        machine,
                        state,
                        &statements[..index],
                        &symbols,
                        initial,
                        &mut formation_errors,
                    );
                        }
                    }
                }
                if formation_errors.is_empty() {
                    Err(errors)
                } else {
                    Err(formation_errors)
                }
            }
        }
    }
    pub(crate) mod operand_captures {
        use super::InferredUpdate;
        use crate::tables::expressions_of;
        use language_core::operator_spelling::OperatorSpelling;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::operator;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
        use validation::expression_traversal::collect_expression_tree;

        struct Capture {
            symbol: SymbolHandle,
            raw_type: TypeReferenceHandle,
            selected: Option<TypeReferenceHandle>,
            disagrees: bool,
        }

        pub(crate) fn adapt(program: &TypedTrees, updates: &mut [InferredUpdate]) {
            if updates.is_empty() {
                return;
            }
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    let statements = program.statement_table.statements(state.statement_nodes);
                    let mut captures = statements
                        .iter()
                        .enumerate()
                        .filter_map(|(offset, statement)| {
                            let local = statement.as_local_data()?;
                            if !local.is_operand_capture || !local.type_is_inferred {
                                return None;
                            }
                            let raw_type = updates
                                .iter()
                                .find(|update| {
                                    update.body == state.statement_nodes && update.offset == offset
                                })
                                .map(|update| update.type_reference)
                                .or_else(|| {
                                    validation::expression_result_type_reference(
                                        program,
                                        machine,
                                        state,
                                        local.initial_value,
                                    )
                                })?;
                            Some(Capture {
                                symbol: local.symbol,
                                raw_type,
                                selected: None,
                                disagrees: false,
                            })
                        })
                        .collect::<Vec<_>>();
                    if captures.is_empty() {
                        continue;
                    }
                    let mut expressions = Vec::new();
                    for statement in statements {
                        for (_, root) in program.tables.expression_roots(statement) {
                            if root.is_valid() {
                                collect_expression_tree(program, root, &mut expressions);
                            }
                        }
                    }
                    let mut operand_uses = Vec::new();
                    for &expression in &expressions {
                        let ExpressionNode::Indexed(indexed) =
                            expressions_of(program).expression(expression)
                        else {
                            continue;
                        };
                        let (spelling, operands) =
                            match expressions_of(program).expression(indexed.index) {
                                ExpressionNode::Range(range) => (
                                    OperatorSpelling::Range,
                                    vec![indexed.collection, range.start, range.end],
                                ),
                                _ => (
                                    OperatorSpelling::Index,
                                    vec![indexed.collection, indexed.index],
                                ),
                            };
                        let operand_types = operands
                            .iter()
                            .map(|&operand| {
                                capture_index(program, operand, &captures)
                                    .map(|index| captures[index].raw_type)
                                    .or_else(|| {
                                        operand
                                            .is_valid()
                                            .then(|| {
                                                validation::expression_result_type_reference(
                                                    program, machine, state, operand,
                                                )
                                            })
                                            .flatten()
                                    })
                            })
                            .collect::<Vec<_>>();
                        for (position, &operand) in operands.iter().enumerate().skip(1) {
                            let Some(index) = capture_index(program, operand, &captures) else {
                                continue;
                            };
                            operand_uses.push(operand);
                            let selected = consumed_type(
                                program,
                                machine.symbol,
                                expression,
                                spelling,
                                &operand_types,
                                position,
                                captures[index].raw_type,
                            );
                            let capture = &mut captures[index];
                            if capture
                                .selected
                                .is_some_and(|previous| previous != selected)
                            {
                                capture.disagrees = true;
                            }
                            capture.selected = Some(selected);
                        }
                    }
                    for &expression in &expressions {
                        if let Some(index) = capture_index(program, expression, &captures)
                            && !operand_uses.contains(&expression)
                        {
                            captures[index].disagrees = true;
                        }
                    }
                    for update in updates
                        .iter_mut()
                        .filter(|update| update.body == state.statement_nodes)
                    {
                        let Some(local) = statements[update.offset].as_local_data() else {
                            continue;
                        };
                        if let Some(capture) = captures
                            .iter()
                            .find(|capture| capture.symbol == local.symbol)
                            && !capture.disagrees
                            && let Some(selected) = capture.selected
                        {
                            update.type_reference = selected;
                        }
                    }
                }
            }
        }

        fn capture_index(
            program: &TypedTrees,
            expression: ExpressionHandle,
            captures: &[Capture],
        ) -> Option<usize> {
            if !expression.is_valid() {
                return None;
            }
            let ExpressionNode::Name(path) = expressions_of(program).expression(expression) else {
                return None;
            };
            captures
                .iter()
                .position(|capture| capture.symbol == path.symbol)
        }

        fn consumed_type(
            program: &TypedTrees,
            owner: SymbolHandle,
            expression: ExpressionHandle,
            spelling: OperatorSpelling,
            operands: &[Option<TypeReferenceHandle>],
            position: usize,
            actual: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            let trait_meanings =
                operator::selected_trait_operator_meanings(program, owner, spelling, operands);
            let expected = if trait_meanings.is_empty() {
                operator::resolve_indexed_spelling_for_operands(
                    program,
                    spelling,
                    operands,
                    expressions_of(program).source_span(expression),
                )
                .into_iter()
                .filter_map(|candidate| {
                    program
                        .operator_parameters(candidate.operator)
                        .get(position)
                        .map(|parameter| parameter.type_reference)
                })
                .collect::<Vec<_>>()
            } else {
                trait_meanings
                    .into_iter()
                    .filter_map(|meaning| {
                        program
                            .state_signature_parameters(meaning.requirement)
                            .get(position)
                            .map(|parameter| parameter.type_reference)
                    })
                    .collect::<Vec<_>>()
            };
            if let Some(&first) = expected.first() {
                let selected = operator::scalar_operand_read_type(program, actual, first);
                return if expected.iter().all(|&expected| {
                    operator::scalar_operand_read_type(program, actual, expected) == selected
                }) {
                    selected
                } else {
                    actual
                };
            }
            if !operator::has_builtin_spelled_expression_meaning(
                program, owner, expression, spelling, operands,
            ) {
                return actual;
            }
            let builtin_collection = operands
                .first()
                .copied()
                .flatten()
                .and_then(|collection| validation::unwrapped_type_reference(program, collection))
                .is_some_and(|collection| {
                    matches!(
                        program.type_reference_table.type_reference(collection),
                        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
                    )
                });
            let Some(reference) = validation::reference_shell(program, actual) else {
                return actual;
            };
            if !builtin_collection
                || !matches!(
                    program.primitive_type_reference(reference.referee),
                    Some(
                        PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                            | PrimitiveType::I8
                            | PrimitiveType::I16
                            | PrimitiveType::I32
                            | PrimitiveType::I64
                    )
                )
            {
                return actual;
            }
            operator::scalar_operand_read_type(program, actual, reference.referee)
        }
    }
    pub(crate) mod reference_observations {
        use crate::tables::{expressions_of, expressions_of_mut};
        use arena::HandleSpan;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::TypedTrees;
        use typed_trees::expression::ExpressionNode;
        use typed_trees::operator::scalar_operand_read_type;
        use typed_trees::statement::{StatementNode, TableLocalData};
        use validation::expression_result_type_reference;

        pub(crate) fn materialize(
            program: &mut TypedTrees,
        ) -> Result<Vec<SymbolHandle>, Vec<diagnostics::Diagnostic>> {
            let mut pending = Vec::new();
            for machine in program.machines() {
                for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                    let mut insertions = Vec::new();
                    for (offset, local) in super::local_bindings(program, state.statement_nodes) {
                        if local.relevance.is_erased()
                            || local.role != language_core::LocalBindingRole::Value
                            || program
                                .primitive_type_reference(local.type_reference)
                                .is_none()
                            || !matches!(
                                expressions_of(program).expression(local.initial_value),
                                ExpressionNode::Call(_)
                            )
                        {
                            continue;
                        }
                        let Some(actual) = expression_result_type_reference(
                            program,
                            machine,
                            state,
                            local.initial_value,
                        ) else {
                            continue;
                        };
                        if validation::reference_shell(program, actual).is_some()
                            && scalar_operand_read_type(program, actual, local.type_reference)
                                != actual
                        {
                            insertions.push((offset, actual));
                        }
                    }
                    if !insertions.is_empty() {
                        pending.push((machine.clone(), state_index, state.clone(), insertions));
                    }
                }
            }
            let mut changed = Vec::new();
            for (machine, state_index, state, insertions) in pending {
                let statements = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .to_vec();
                let mut body = HandleSpan::empty();
                for (offset, mut statement) in statements.into_iter().enumerate() {
                    if let Some((_, actual)) =
                        insertions.iter().find(|(position, ..)| *position == offset)
                    {
                        let StatementNode::LocalData(local) = &mut statement else {
                            unreachable!();
                        };
                        let name = format!("__reference_result#{}", local.symbol.arena_index());
                        let children = program.symbols.append_generated_children_from(
                            state.symbol,
                            [(local.symbol, SymbolKind::Local, name.as_str())],
                        );
                        let symbol = symbols::SymbolTableBuilder::child_handles(children)
                            .next()
                            .expect("one generated reference result");
                        let name = typed_trees::name::Identifier::generated(name);
                        let reference =
                            crate::monomorphization::body_rewriting::insert_subject_name(
                                expressions_of_mut(program),
                                name.clone(),
                                symbol,
                            );
                        let source_span = expressions_of(program).source_span(local.initial_value);
                        expressions_of_mut(program).set_source_span(reference, source_span);
                        program.statement_table.push_statement(
                            &mut body,
                            StatementNode::LocalData(TableLocalData {
                                symbol,
                                name,
                                type_reference: *actual,
                                initial_value: local.initial_value,
                                type_is_inferred: true,
                                ..Default::default()
                            }),
                        );
                        local.initial_value = reference;
                    }
                    program.statement_table.push_statement(&mut body, statement);
                }
                program.machine_states_mut(&machine)[state_index].statement_nodes = body;
                changed.push(machine.symbol);
            }
            Ok(changed)
        }
    }
    pub(crate) use dispatch_subjects::materialize as materialize_dispatch_subjects;
    pub(crate) use dispatch_subjects::require_complete as require_complete_dispatch_subjects;
    pub(crate) use reference_observations::materialize as materialize_reference_observations;

    fn local_bindings(
        program: &TypedTrees,
        statements: arena::HandleSpan<StatementNode>,
    ) -> impl Iterator<Item = (usize, &TableLocalData)> {
        program
            .statement_table
            .indexed_statements(statements)
            .filter_map(|(offset, statement)| Some((offset, statement.as_local_data()?)))
    }

    #[derive(Clone, Copy)]
    pub(crate) struct InferredUpdate {
        owner: SymbolHandle,
        body: arena::HandleSpan<StatementNode>,
        offset: usize,
        type_reference: TypeReferenceHandle,
    }

    fn apply_updates(
        program: &mut TypedTrees,
        mut updates: Vec<InferredUpdate>,
    ) -> Vec<SymbolHandle> {
        operand_captures::adapt(program, &mut updates);
        let mut changed_owners = Vec::new();
        for update in updates {
            let Some(local) =
                program.statement_table.statements(update.body)[update.offset].as_local_data()
            else {
                continue;
            };
            if local.type_reference == update.type_reference
                || applied_type_identity(program, local.type_reference, update.owner, &[])
                    .zip(applied_type_identity(
                        program,
                        update.type_reference,
                        update.owner,
                        &[],
                    ))
                    .is_some_and(|(previous, current)| previous == current)
            {
                continue;
            }
            if let StatementNode::LocalData(local) =
                &mut program.statement_table.statements_mut(update.body)[update.offset]
            {
                local.type_reference = update.type_reference;
                changed_owners.push(update.owner);
            }
        }
        changed_owners
    }

    pub(crate) fn refresh_generic_call_results(
        program: &mut TypedTrees,
        candidates: &[crate::monomorphization::Candidate],
        selections: &[crate::monomorphization::CallSelection],
    ) -> Result<Vec<SymbolHandle>, Vec<diagnostics::Diagnostic>> {
        let mut updates = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (offset, local) in local_bindings(program, state.statement_nodes) {
                    if expressions_of(program)
                        .call_node(local.initial_value)
                        .is_some_and(|call| call.native_code_layout.is_some())
                    {
                        continue;
                    }
                    if !local.type_is_inferred || !local.role.has_ordinary_initializer() {
                        continue;
                    }
                    let Some(selection) = selections.iter().find(|selection| {
                        selection.site
                            == crate::monomorphization::CallSite::Expression(local.initial_value)
                            && selection.caller_is_generic
                    }) else {
                        continue;
                    };
                    if !selection.has_bound_arguments() {
                        continue;
                    }
                    let Some(callee) = crate::monomorphization::selection::state_by_symbol(
                        program,
                        selection.callee_symbol,
                    ) else {
                        continue;
                    };
                    if callee.return_type.is_valid() {
                        updates.push((
                            state.statement_nodes,
                            offset,
                            local.type_reference,
                            local.is_operand_capture,
                            callee.return_type,
                            selection.clone(),
                        ));
                    }
                }
            }
        }
        let mut inferred_updates = Vec::new();
        for (body, offset, previous_type, is_operand_capture, return_type, selection) in updates {
            let candidate = crate::monomorphization::candidate_for_selection(
                program,
                &candidates[selection.candidate_index],
                &selection,
            );
            let mut forwarded_symbols = Vec::new();
            for (ordinal, ((parameter, _, _), binding)) in candidate
                .template
                .const_parameters
                .iter()
                .zip(&candidate.const_bindings)
                .enumerate()
            {
                if candidate.runtime_value_bindings[ordinal].is_some() {
                    continue;
                }
                if let Some(binding) = binding
                    && let Ok(argument) = program.static_operand_from_type_reference(*binding)
                    && argument.symbol.is_valid()
                {
                    forwarded_symbols.push((*parameter, argument.symbol));
                }
            }
            let substitutions = candidate
                .template
                .type_parameters
                .iter()
                .zip(&candidate.type_bindings)
                .filter_map(|((symbol, _), binding)| binding.map(|binding| (*symbol, binding)))
                .chain(
                    candidate
                        .template
                        .const_parameters
                        .iter()
                        .zip(&candidate.const_bindings)
                        .filter_map(|((symbol, _, _), binding)| {
                            binding.map(|binding| (*symbol, binding))
                        }),
                )
                .collect::<Vec<_>>();
            if !is_operand_capture
                && program.package_qualified_type_identity(previous_type)
                    == program.type_identity(TypeIdentityRequest {
                        substitutions: &substitutions,
                        ..TypeIdentityRequest::package_qualified(return_type)
                    })
            {
                continue;
            }
            let type_start = program.type_reference_table.type_reference_count() + 1;
            let expression_start = expressions_of(program).expression_count() + 1;
            let selected_return = program
                .apply_static_application_type(
                    return_type,
                    candidate.binding_application,
                    candidate.template.template_symbol,
                )
                .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
            let tables = &mut program.tables;
            tables.type_reference_table.remap_symbols_in(
                selected_return,
                &mut tables.expression_table,
                &forwarded_symbols,
            );
            let evidence_rewrites =
                crate::monomorphization::body_rewriting::evidence_argument_rewrites(
                    program, &candidate,
                );
            let static_rewrites =
                crate::monomorphization::body_rewriting::forwarded_static_argument_rewrites(
                    program, &candidate,
                );
            let expressions = expressions_of(program)
                .iter_expressions_from(expression_start)
                .map(|(expression, _)| expression)
                .collect::<Vec<_>>();
            for expression in expressions {
                crate::monomorphization::body_rewriting::rewrite_index_algebra(
                    program,
                    expression,
                    &evidence_rewrites,
                    &static_rewrites,
                );
            }
            crate::monomorphization::const_values::substitute(
                program,
                &candidate,
                Some(expression_start),
                &[],
            )
            .map_err(|error| vec![error])?;
            crate::monomorphization::body_rewriting::substitute_cloned_type_parameters(
                None,
                program,
                &candidate,
                type_start,
                expression_start,
                &[],
                SymbolHandle::invalid(),
            )
            .map_err(|error| vec![error])?;
            inferred_updates.push(InferredUpdate {
                owner: selection.lexical_owner,
                body,
                offset,
                type_reference: selected_return,
            });
        }
        let changed_owners = apply_updates(program, inferred_updates);
        if !changed_owners.is_empty() {
            let roots = DeclarationSelection::from_symbols(changed_owners.iter().copied());
            crate::monomorphization::finalize_domain_instance_identities(program, &roots)
                .map_err(|diagnostic| vec![diagnostic])?;
        }
        Ok(changed_owners)
    }

    pub(crate) fn refresh(
        program: &mut TypedTrees,
        unavailable: &[crate::constructed_results::UnavailableValueType],
    ) -> Result<Vec<SymbolHandle>, Vec<diagnostics::Diagnostic>> {
        let mut updates = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (offset, local) in local_bindings(program, state.statement_nodes) {
                    if !local.type_is_inferred || !local.role.has_ordinary_initializer() {
                        continue;
                    }
                    let is_call = expressions_of(program)
                        .call_node(local.initial_value)
                        .is_some();
                    let needs_inference = matches!(
                        program
                            .type_reference_table
                            .type_reference(local.type_reference),
                        typed_trees::types::TypeReferenceNode::Unit
                    );
                    if !is_call
                        && !local.is_operand_capture
                        && !needs_inference
                        && local.role != language_core::LocalBindingRole::DispatchSubject
                    {
                        continue;
                    }
                    let reference = if is_call {
                        if let Some(unavailable) = unavailable
                            .iter()
                            .find(|unavailable| unavailable.expression == local.initial_value)
                        {
                            return Err(unavailable.diagnostics.clone());
                        }
                        let retained = expressions_of(program)
                            .constructed_value_type_reference(local.initial_value);
                        retained.is_valid().then_some(retained)
                    } else {
                        expression_result_type_reference(
                            program,
                            machine,
                            state,
                            local.initial_value,
                        )
                    };
                    let Some(type_reference) = reference else {
                        continue;
                    };
                    if !is_call
                        && !local.is_operand_capture
                        && !needs_inference
                        && local.role != language_core::LocalBindingRole::DispatchSubject
                        && validation::reference_shell(program, type_reference).is_none()
                    {
                        continue;
                    }
                    let receivers = if is_call {
                        Vec::new()
                    } else {
                        receiver_application_bindings(program, machine, state, local.initial_value)
                    };
                    updates.push((
                        InferredUpdate {
                            owner: machine.symbol,
                            body: state.statement_nodes,
                            offset,
                            type_reference,
                        },
                        receivers,
                    ));
                }
            }
        }
        // A field's declared type is written in its data declaration's binders.
        // Read through `binding: Binding<Actors>`, `binding.source` has type
        // `Source<Actors>` in the reader's binders: substitution follows
        // declaration identity through field types, const and value arguments
        // included (spec/language/generics.md, closed applications). Instantiate
        // from the innermost receiver outward. `instantiate` writes new rows only
        // where a binder changes, so no declaration's own field type is rewritten;
        // a binder it cannot express (a const array length) keeps the declared type.
        let updates = updates
            .into_iter()
            .map(|(mut update, receivers)| {
                for bindings in receivers {
                    update.type_reference = program
                        .type_reference_table
                        .instantiate(update.type_reference, &bindings)
                        .unwrap_or(update.type_reference);
                }
                update
            })
            .collect();
        Ok(apply_updates(program, updates))
    }

    /// For a field read `receiver.field` (possibly nested), each receiver's data
    /// application bindings, innermost receiver first: its data's type, const
    /// and value binders paired with the receiver type's arguments.
    fn receiver_application_bindings(
        program: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> Vec<Vec<(SymbolHandle, TypeReferenceHandle)>> {
        let mut chain = Vec::new();
        let mut current = expression;
        while let ExpressionNode::Member(member) = expressions_of(program).expression(current) {
            let receiver = member.receiver;
            let Some(receiver_type) =
                expression_result_type_reference(program, machine, state, receiver)
            else {
                break;
            };
            chain.push(data_application_bindings(program, receiver_type));
            current = receiver;
        }
        chain
    }

    fn data_application_bindings(
        program: &TypedTrees,
        receiver: TypeReferenceHandle,
    ) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
        let receiver = program.type_reference_table.peeled_type_reference(receiver);
        let typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program.type_reference_table.type_reference(receiver)
        else {
            return Vec::new();
        };
        let Some(data) = validation::unique_data_definition_by_symbol(program, *base_symbol) else {
            return Vec::new();
        };
        let parameters = program.data_type_parameters(data);
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        if parameters.len() != arguments.len() {
            return Vec::new();
        }
        parameters
            .iter()
            .zip(arguments)
            .filter(|(parameter, _)| {
                matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Type
                        | typed_trees::data::TypeParameterKind::Const { .. }
                        | typed_trees::data::TypeParameterKind::Value { .. }
                )
            })
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect()
    }
}
