/*
    Helpers every checking step reads: `collections` (ordered first-occurrence
    rosters), `call_formals` (aligning a call's operands with its formals),
    `operators` (checked operator selection), `authored_selections`
    (authored declaration custody retained through specialization) and
    `machine_waves` (call-cycle components checked wave by wave in parallel).
*/

pub(crate) mod authored_selections {
    /*
        Retain authored declaration custody through specialization and final checking.

        Operator occurrences in static type endpoints may lack executable use facts.
        Their conservative candidate query uses independently known operand carriers,
        not runtime intervals or a guessed peer type. Validated literal suffixes can
        exclude unrelated operators; they cannot bypass a matching authored meaning
        or replace the final selection and package-admission checks.

        `finalization.rs` publishes checked selections, and
        `selection_collection.rs` collects proof/statement conclusions. Independent
        validation owns source environments, compiler operation classification and
        consistent occurrence resolution. Preparation owns early binding; this
        owner never rewrites the typed input.
    */
    mod build_selections {
        /*
            Source authority for designated product operands, independent of execution.

            The host preparation only observes identity and therefore borrows shared.
            The authored bind still requires mutable Build authority; this check retains
            that source obligation before the ordinary call weakens its implementation
            access. An integer ticket or a declared description type never proves issuance.
        */
        use diagnostics::Diagnostic;
        use language_semantics::ReferenceAccess;
        use language_semantics::declaration_selection::{
            BuildSelectionRequest, RootBindingOperand,
        };
        use typed_trees::expression::ExpressionNode;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::TypeReferenceNode;
        use validation::CheckingInput;
        use validation::compiler_operations::exact_build_prelude_data;
        use validation::compiler_operations::type_reference_names_exact_prelude_data;
        use validation::expression_owners::OwnerEnvironmentIndex;
        use validation::expression_type_reference_in_state;

        pub(super) fn validate(
            program: &dyn CheckingInput,
            owners: &OwnerEnvironmentIndex,
        ) -> Result<(), Diagnostic> {
            let expressions = &program.expression_table;
            let operations: Vec<_> = program
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary && definition.name.as_str() == "BuildBindingOperations"
                })
                .filter(|definition| {
                    program
                        .symbols
                        .symbol_source_span(definition.symbol)
                        .and_then(|span| program.symbols.source_file(span))
                        .is_some_and(|file| {
                            file.origin == source::SourceOrigin::Toolchain
                                && file.path == std::path::Path::new("<build-prelude>")
                        })
                })
                .flat_map(|definition| program.trait_machine_signatures(definition))
                .map(|signature| (signature.symbol, signature.name.as_str()))
                .collect();
            let template = |symbol| {
                program
                    .closed_requirement_application(symbol)
                    .map_or(symbol, |application| application.template)
            };
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        if let StatementNode::Call(call) = statement
                            && operations
                                .iter()
                                .any(|(symbol, _)| *symbol == template(call.target_symbol))
                        {
                            return Err(Diagnostic::error(
                                "Build binding protocol requires source-issued designated product operands",
                            ));
                        }
                    }
                }
            }
            for (expression, node) in expressions.iter_expressions() {
                let ExpressionNode::Call(call) = node else {
                    continue;
                };
                let operation = operations
                    .iter()
                    .find(|(symbol, _)| *symbol == template(call.target_symbol))
                    .map(|(_, name)| *name);
                if !call.build_selection.is_valid() {
                    if operation.is_some() {
                        return Err(Diagnostic::error(
                            "Build binding protocol requires source-issued designated product operands",
                        ));
                    }
                    continue;
                }
                let request = expressions
                    .build_selection_request(call.build_selection)
                    .ok_or_else(|| {
                        Diagnostic::error("Build selection lost its designated operands")
                    })?;
                let span = request.source_span();
                let operation = operation.ok_or_else(|| {
                    Diagnostic::error(
                        "Build selection lost its exact toolchain boundary requirement",
                    )
                    .with_source_span(span)
                })?;
                let arguments = expressions.expression_handles(call.arguments);
                let expected = match request {
                    BuildSelectionRequest::ProviderReadiness(request) => {
                        if !program.traits().iter().any(|definition| {
                            definition.symbol
                                == program
                                    .type_reference_table
                                    .type_symbol(request.requirement)
                                && definition.is_boundary
                        }) || request.backing.path.is_empty()
                            || request.backing.source_span.span.start
                                >= request.backing.source_span.span.end
                        {
                            return Err(Diagnostic::error(
                                "provider readiness lost its checked requirement or designated backing",
                            )
                            .with_source_span(span));
                        }
                        match operation {
                            "prepare" | "await_provider" => 1,
                            _ => {
                                return Err(Diagnostic::error(
                                    "provider readiness lost its checked suspension protocol",
                                )
                                .with_source_span(span));
                            }
                        }
                    }
                    BuildSelectionRequest::Exclusion(_) => match operation {
                        "prepare" => 1,
                        "exclude" => 2,
                        _ => {
                            return Err(Diagnostic::error(
                                "behavior exclusion lost its checked value protocol",
                            )
                            .with_source_span(span));
                        }
                    },
                    BuildSelectionRequest::Entry(binding) => {
                        if binding.slot.is_empty()
                            || matches!(&binding.implementation, RootBindingOperand::AuthoredPath(path) if path.is_empty())
                        {
                            return Err(Diagnostic::error("root-slot binding requires exactly one slot path and one implementation path or description expression").with_source_span(span));
                        }
                        match (&binding.implementation, operation) {
                            (_, "prepare") | (RootBindingOperand::AuthoredPath(_), "bind") => 1,
                            (RootBindingOperand::Description, "bind_description") => 2,
                            _ => {
                                return Err(Diagnostic::error(
                                    "root binding operation disagrees with its designated product operands",
                                )
                                .with_source_span(span));
                            }
                        }
                    }
                    BuildSelectionRequest::Declaration(binding) => {
                        binding
                            .validate_shape()
                            .map_err(|reason| Diagnostic::error(reason).with_source_span(span))?;
                        let preparation = if binding.operation.uses_tests_facet() {
                            "prepare_tests"
                        } else {
                            "prepare"
                        };
                        if operation == preparation {
                            1
                        } else if let Some((_, arity)) = binding
                            .operation
                            .product_recording_operation()
                            .filter(|(name, _)| *name == operation)
                        {
                            arity
                        } else {
                            return Err(Diagnostic::error(
                                "Product selection commit disagrees with its declaration operation",
                            )
                            .with_source_span(span));
                        }
                    }
                };
                if arguments.len() != expected {
                    return Err(Diagnostic::error(
                        "root binding operation lost its ordinary arguments",
                    )
                    .with_source_span(span));
                }
                for (machine, state, statement) in owners.containing_statement_sites(expression) {
                    let machine = &program.machines()[machine];
                    let state = &program.machine_states(machine)[state];
                    let argument_type = |argument| {
                        expression_type_reference_in_state(
                            program,
                            state.symbol,
                            statement,
                            argument,
                        )
                    };
                    if operation == "prepare" {
                        let mut receiver = arguments[0];
                        let mut explicit_mutable_borrow = false;
                        while let ExpressionNode::Borrow(borrow) = expressions.expression(receiver)
                        {
                            if borrow.access != ReferenceAccess::Mutable {
                                return Err(Diagnostic::error(
                                    "root binding requires a compiler-issued &mut Build place",
                                )
                                .with_source_span(span));
                            }
                            explicit_mutable_borrow = true;
                            receiver = borrow.target;
                        }
                        let has_authority = argument_type(receiver).is_some_and(|reference| {
                            let referee =
                                match program.type_reference_table.type_reference(reference) {
                                    TypeReferenceNode::Reference {
                                        access, referee, ..
                                    } if *access == ReferenceAccess::Mutable => *referee,
                                    _ if explicit_mutable_borrow => reference,
                                    _ => return false,
                                };
                            exact_build_prelude_data(
                                program,
                                program.type_reference_table.type_symbol(referee),
                                "Build",
                            )
                        });
                        if !has_authority {
                            return Err(Diagnostic::error(
                                "root binding requires a compiler-issued &mut Build receiver",
                            )
                            .with_source_span(span));
                        }
                    } else if operation == "prepare_tests"
                        && !argument_type(arguments[0]).is_some_and(|reference| {
                            let referee =
                                match program.type_reference_table.type_reference(reference) {
                                    TypeReferenceNode::Reference { referee, .. } => *referee,
                                    _ => reference,
                                };
                            exact_build_prelude_data(
                                program,
                                program.type_reference_table.type_symbol(referee),
                                "BuildTests",
                            )
                        })
                    {
                        // The evaluator authenticates the exact facet cell; the
                        // checker requires its compiler-owned type.
                        return Err(Diagnostic::error(
                            "test group registration requires the Build's compiler-owned `tests` facet",
                        )
                        .with_source_span(span));
                    } else if operation == "bind_description"
                        && !argument_type(arguments[1]).is_some_and(|reference| {
                            type_reference_names_exact_prelude_data(
                                program,
                                reference,
                                "ProductEntryRef",
                            )
                        })
                    {
                        return Err(Diagnostic::error("delegated root binding requires an operand of the compiler-owned ProductEntryRef description type").with_source_span(span));
                    }
                }
            }
            Ok(())
        }
    }
    mod call_targets {
        /*
            Checked call, conformance and name-path targets.
        */
        use super::member_targets::for_each_statement_containing;
        use crate::flow::expressions;
        use crate::flow::state_statements;
        use crate::published::places::place_at;
        use crate::shared::authored_selections::contract_resolution;
        use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus};
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionNode;
        use validation::selection_resolution::exact_named_operator_call;

        pub(crate) fn checked_operator_conformance_targets(
            facts: &CheckFacts,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Vec<SymbolHandle> {
            let mut targets = Vec::new();
            facts
                .operators
                .for_each_expression_use(expression, |_, operator_use| {
                    if operator_use.status != CheckedOperatorResolutionStatus::Resolved {
                        return;
                    }
                    let Some(candidate) = facts.operators.selected_candidate(operator_use) else {
                        return;
                    };
                    if candidate.is_trait_backed()
                        && !targets.contains(&candidate.conformance_symbol)
                    {
                        targets.push(candidate.conformance_symbol);
                    }
                });
            targets
        }

        pub(crate) fn checked_statement_call_target(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            statement_index: usize,
            authored_target: SymbolHandle,
        ) -> SymbolHandle {
            if authored_target.is_valid() {
                return authored_target;
            }
            let Some(state) = facts.flow.control.state(machine_symbol, state_symbol) else {
                return SymbolHandle::invalid();
            };
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                if call.statement_index != statement_index || !call.target_symbol.is_valid() {
                    continue;
                }
                if matches!(
                    validation::find_call_site(
                        program,
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        call.call_ordinal,
                    ),
                    Some(validation::CallSite::Statement(_))
                ) {
                    return call.target_symbol;
                }
            }
            SymbolHandle::invalid()
        }

        pub(crate) fn checked_call_target(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: typed_trees::expression::ExpressionHandle,
            authored_target: SymbolHandle,
            authored_source_span: source::SourceSpan,
        ) -> SymbolHandle {
            if authored_target.is_valid() {
                return authored_target;
            }
            let mut checked_named_target = None;
            for use_fact in facts
                .operators
                .named_uses()
                .filter(|use_fact| use_fact.expression == expression)
            {
                if !use_fact.selected_operator_symbol.is_valid()
                    || checked_named_target
                        .is_some_and(|target| target != use_fact.selected_operator_symbol)
                {
                    return SymbolHandle::invalid();
                }
                checked_named_target = Some(use_fact.selected_operator_symbol);
            }
            if let Some(target) = checked_named_target {
                return target;
            }
            if let Some(call) = program.expression_table.call_node(expression)
                && let Some(operator) = exact_named_operator_call(program, call)
            {
                return operator.symbol;
            }
            if let Some(call) = program.expression_table.call_node(expression)
                && let Some(operator) = contract_resolution::checked_named_operator_call(
                    program,
                    facts,
                    expression,
                    call,
                    authored_source_span,
                    None,
                )
            {
                return operator.symbol;
            }
            if let Some(call) = program.expression_table.call_node(expression)
                && let Some(target) =
                    validation::expression_owners::checked_machine_call_target_from_exact_owner(
                        program,
                        &facts.proof.contract_facts,
                        expression,
                        call,
                        None,
                    )
            {
                return target;
            }
            for (_, state) in facts.flow.control.states.iter() {
                for call in facts.flow.control.calls.span_or_empty(state.calls) {
                    if let Some(validation::CallSite::Expression {
                        expression: candidate,
                        ..
                    }) = validation::find_call_site(
                        program,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    ) && candidate == expression
                        && call.target_symbol.is_valid()
                    {
                        return call.target_symbol;
                    }
                }
            }
            let mut checked_source_target = None;
            for (_, state) in facts.flow.control.states.iter() {
                for call in facts.flow.control.calls.span_or_empty(state.calls) {
                    if !call.authored_source_custody_valid
                        || call.authored_source_span != Some(authored_source_span)
                        || !call.target_symbol.is_valid()
                    {
                        continue;
                    }
                    if checked_source_target.is_some_and(|target| target != call.target_symbol) {
                        return SymbolHandle::invalid();
                    }
                    checked_source_target = Some(call.target_symbol);
                }
            }
            if let Some(target) = checked_source_target {
                return target;
            }
            let mut checked_fact_target = None;
            for projection in &facts.fact_call_projections {
                if projection.call_expression != expression || !projection.target_state.is_valid() {
                    continue;
                }
                if checked_fact_target.is_some_and(|target| target != projection.target_state) {
                    return SymbolHandle::invalid();
                }
                checked_fact_target = Some(projection.target_state);
            }
            if let Some(target) = checked_fact_target {
                return target;
            }
            SymbolHandle::invalid()
        }

        pub(crate) fn checked_name_path_segment_target(
            program: &dyn ::validation::CheckingInput,
            expression: typed_trees::expression::ExpressionHandle,
            path: &typed_trees::expression::TableNamePath,
            target_index: usize,
        ) -> SymbolHandle {
            let direct = validation::resolve_name_path_member_symbol(program, path, target_index);
            if direct.is_valid() && target_index != 0 {
                return direct;
            }

            let mut contextual_root = None;
            let mut conflicting = false;
            for_each_statement_containing(
                program,
                expression,
                |machine, state, statement_index, _| {
                    let root = if direct.is_valid() {
                        direct
                    } else {
                        let Some(validation::CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(root),
                            ..
                        }) = place_at(program, state.symbol, statement_index, expression)
                        else {
                            return Some(());
                        };
                        root
                    };
                    let root =
                        authored_contextual_root(program, machine, state, statement_index, root);
                    if contextual_root.is_some_and(|candidate| candidate != root) {
                        conflicting = true;
                        return None;
                    }
                    contextual_root = Some(root);
                    Some(())
                },
            );
            if conflicting {
                return SymbolHandle::invalid();
            }

            let Some(mut selected) = contextual_root else {
                return direct;
            };
            if target_index == 0 {
                return selected;
            }
            for member in expressions(program)
                .name_path_members(path.members)
                .iter()
                .skip(1)
                .take(target_index)
            {
                selected = validation::symbol_type_symbol(program, selected)
                    .and_then(|type_symbol| {
                        validation::resolve_member_symbol_from_type_symbol(
                            program,
                            type_symbol,
                            member.as_str(),
                        )
                    })
                    .unwrap_or_else(SymbolHandle::invalid);
                if !selected.is_valid() {
                    break;
                }
            }
            selected
        }

        fn authored_contextual_root(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            selected: SymbolHandle,
        ) -> SymbolHandle {
            let Some(template_symbol) = program
                .machine_specializations
                .iter()
                .find(|specialization| {
                    specialization.instance == machine.symbol
                        && specialization.template != specialization.instance
                })
                .map(|specialization| specialization.template)
            else {
                return selected;
            };
            let Some(template) = program.machine_by_symbol(template_symbol) else {
                return selected;
            };
            let Some(state_ordinal) =
                typed_trees::state::state_position(program.machine_states(machine), state.symbol)
            else {
                return selected;
            };
            let Some(template_state) = program.machine_states(template).get(state_ordinal) else {
                return selected;
            };

            if let Some(parameter_ordinal) = typed_trees::signature::state_parameter_position(
                program.state_parameters(state),
                selected,
            ) {
                return program
                    .state_parameters(template_state)
                    .get(parameter_ordinal)
                    .map_or(selected, |parameter| parameter.symbol);
            }

            let template_statements = state_statements(program, template_state.statement_nodes);
            if let Some((ordinal, _)) = program
                .state_local_declaration(state, selected)
                .filter(|&(ordinal, _)| ordinal < statement_index)
            {
                return template_statements
                    .get(ordinal)
                    .and_then(|statement| match statement {
                        typed_trees::statement::StatementNode::LocalData(local) => {
                            Some(local.symbol)
                        }
                        _ => None,
                    })
                    .unwrap_or(selected);
            }
            selected
        }

        /*
            A call through a named implementation (`Ascending::before(left, right)`)
            selects the implementation: its visibility and package authority govern
            the call, and the row it reaches may be a synthesized default machine that
            has no separate authority of its own.
        */
        pub(crate) fn implementation_call_selection(
            program: &dyn ::validation::CheckingInput,
            receiver: SymbolHandle,
        ) -> Option<SymbolHandle> {
            (receiver.is_valid()
                && program.symbols.get(receiver).kind == symbols::SymbolKind::Conformance)
                .then_some(receiver)
        }

        pub(crate) fn expression_receiver_symbol(
            program: &dyn ::validation::CheckingInput,
            receiver: typed_trees::expression::ExpressionHandle,
        ) -> SymbolHandle {
            if !receiver.is_valid() {
                return SymbolHandle::invalid();
            }
            match program.expression_table.expression(receiver) {
                ExpressionNode::Name(path) => path.symbol,
                _ => SymbolHandle::invalid(),
            }
        }
    }
    mod contract_resolution {
        /*
            Checked reconstruction for declaration selections copied into proof/static
            contracts.

            These expressions can lack executable operator-use facts and can retain
            late-bound call symbols after contract cloning. Exact checked owners supply
            the missing parameter/result types; ambiguous reconstruction stays closed.
        */
        use checked_trees::{CheckFacts, ContractProofFactOwner};
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionNode;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum CheckedContractOperatorResolution {
            Declaration(SymbolHandle),
            Builtin,
        }

        /*
            `owner_index` is the caller's exact-owner index, when it holds one for this
            program and these facts; without it each operand query rescans the program.
        */
        pub(crate) fn checked_operator_resolution(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: typed_trees::expression::ExpressionHandle,
            node: &ExpressionNode,
            owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
        ) -> Option<CheckedContractOperatorResolution> {
            if validation::content_conservation_plans(program)
                .iter()
                .any(|plan| plan.source_expression == expression)
            {
                return Some(CheckedContractOperatorResolution::Builtin);
            }
            if checked_float_meaning_equality(program, facts, expression, node, owner_index)
                || checked_resultless_law_equality(program, facts, expression, node, owner_index)
            {
                return Some(CheckedContractOperatorResolution::Builtin);
            }
            checked_spelled_operator_resolution(program, facts, expression, node, owner_index)
        }

        pub(super) fn checked_operand_type(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            containing_expression: typed_trees::expression::ExpressionHandle,
            operand: typed_trees::expression::ExpressionHandle,
            owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            crate::shared::authored_selections::operator_targets::authored_operand_type(
                program, operand,
            )
            .or_else(|| {
                validation::expression_owners::checked_expression_type_reference_from_exact_owner(
                    program,
                    &facts.proof.contract_facts,
                    containing_expression,
                    operand,
                    owner_index,
                )
            })
            .or_else(|| {
                let call = program.expression_table.call_node(operand)?;
                validation::selection_resolution::exact_named_operator_call(program, call)
                    .or_else(|| {
                        checked_named_operator_call(
                            program,
                            facts,
                            operand,
                            call,
                            program.expression_table.source_span(operand),
                            owner_index,
                        )
                    })
                    .map(|operator| operator.return_type)
            })
        }

        pub(crate) fn checked_named_operator_call<'program>(
            program: &'program dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: typed_trees::expression::ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            source_span: source::SourceSpan,
            owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
        ) -> Option<&'program typed_trees::operator::OperatorDefinition> {
            let arguments = program.expression_table.expression_handles(call.arguments);
            let operand_types = arguments
                .iter()
                .map(|argument| {
                    crate::shared::authored_selections::operator_targets::authored_operand_type(
                        program, *argument,
                    )
                    .or_else(|| {
                        validation::expression_owners::checked_expression_type_reference_from_exact_owner(
                            program,
                            &facts.proof.contract_facts,
                            expression,
                            *argument,
                            owner_index,
                        )
                    })
                })
                .collect::<Vec<_>>();
            if operand_types.iter().all(Option::is_none) {
                return None;
            }

            let candidates = typed_trees::operator::named_expression_call_candidates(program, call)
                .into_iter()
                .filter(|operator| {
                    program
                        .symbols
                        .source_reference_can_see_symbol(source_span, operator.symbol)
                })
                .filter(|operator| {
                    let parameters = program.operator_parameters(operator);
                    parameters.len() == operand_types.len()
                        && parameters
                            .iter()
                            .zip(&operand_types)
                            .all(|(parameter, actual)| {
                                actual.is_none_or(|actual| {
                                    program.normalized_type_identity(actual)
                                        == program
                                            .normalized_type_identity(parameter.type_reference)
                                })
                            })
                })
                .collect::<Vec<_>>();
            let [selected] = candidates.as_slice() else {
                return None;
            };
            Some(*selected)
        }

        /*
            Proof/static contract expressions do not always produce an executable
            operator-use fact. When the checked owner supplies an operand type, replay
            ordinary operand-driven spelled dispatch: one candidate selects that
            declaration, no candidate means the already-validated compiler builtin,
            and ambiguity remains unresolved.
        */
        fn checked_spelled_operator_resolution(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: typed_trees::expression::ExpressionHandle,
            node: &ExpressionNode,
            owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
        ) -> Option<CheckedContractOperatorResolution> {
            let ExpressionNode::Binary(binary) = node else {
                return None;
            };
            let spelling = typed_trees::operator::binary_operator_spelling(binary.operator)?;
            let operand_types = [
                checked_operand_type(program, facts, expression, binary.left, owner_index),
                checked_operand_type(program, facts, expression, binary.right, owner_index),
            ];
            if operand_types.iter().all(Option::is_none) {
                return None;
            }
            let candidates = typed_trees::operator::resolve_spelling_for_operands(
                program,
                spelling,
                &operand_types,
                program.expression_table.source_span(expression),
            );
            match candidates.as_slice() {
                [] => Some(CheckedContractOperatorResolution::Builtin),
                [candidate] => Some(CheckedContractOperatorResolution::Declaration(
                    candidate.operator.symbol,
                )),
                _ => None,
            }
        }

        /*
            D40 equality is a proof-only compiler intrinsic, not an authored runtime
            operator. Admit only the sealed toolchain `FloatMeaning` carrier.
        */
        fn checked_float_meaning_equality(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: typed_trees::expression::ExpressionHandle,
            node: &ExpressionNode,
            owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
        ) -> bool {
            let ExpressionNode::Binary(binary) = node else {
                return false;
            };
            if binary.operator != typed_trees::expression::BinaryOperator::Equal {
                return false;
            }
            [binary.left, binary.right].into_iter().all(|operand| {
                checked_operand_type(program, facts, expression, operand, owner_index).is_some_and(
                    |type_reference| {
                        validation::is_exact_toolchain_float_meaning_type(program, type_reference)
                    },
                )
            })
        }

        /*
            A resultless signature contract is a theorem slot. Its `==` is proposition
            equality over retained terms, including copies on exact realizing machines.
        */
        fn checked_resultless_law_equality(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: typed_trees::expression::ExpressionHandle,
            node: &ExpressionNode,
            owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
        ) -> bool {
            if !matches!(
                node,
                ExpressionNode::Binary(binary)
                    if binary.operator == typed_trees::expression::BinaryOperator::Equal
            ) {
                return false;
            }

            let containing = match owner_index {
                Some(owner_index) => owner_index.containing_contract_facts(expression),
                None => facts
                    .proof
                    .contract_facts
                    .iter()
                    .filter(|(_, contract)| {
                        validation::expression_owners::proof_fact_contains_expression(
                            program,
                            contract.fact,
                            expression,
                        )
                    })
                    .map(|(handle, _)| handle)
                    .collect(),
            };
            let mut found = false;
            for handle in containing {
                let contract = facts.proof.contract_facts.get(handle);
                let resultless = match contract.owner {
                    ContractProofFactOwner::CallSignature { .. }
                    | ContractProofFactOwner::RuntimeCallableContract { .. } => {
                        crate::published::proof::contracts::call_signatures::signature(
                            program,
                            &facts.proof,
                            contract.owner,
                        )
                        .is_some_and(|signature| {
                            type_reference_is_unit(program, signature.return_type)
                        })
                    }
                    ContractProofFactOwner::StateSignature {
                        owner_symbol,
                        state_symbol,
                    } => resultless_state_signature(program, owner_symbol, state_symbol),
                    ContractProofFactOwner::MachineState {
                        machine_symbol,
                        state_symbol,
                    } => resultless_machine_state(program, machine_symbol, state_symbol),
                    ContractProofFactOwner::Machine { machine_symbol } => program
                        .machine_by_symbol(machine_symbol)
                        .and_then(|machine| program.machine_states(machine).first())
                        .is_some_and(|state| type_reference_is_unit(program, state.return_type)),
                    _ => false,
                };
                if !resultless {
                    return false;
                }
                found = true;
            }
            found
        }

        fn resultless_state_signature(
            program: &dyn ::validation::CheckingInput,
            owner_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> bool {
            let return_type = program
                .trait_definition_by_symbol(owner_symbol)
                .and_then(|definition| {
                    program
                        .trait_machine_signature_by_symbol(definition, state_symbol)
                        .map(|signature| signature.return_type)
                })
                .or_else(|| {
                    program
                        .machine_parameter_signature(state_symbol)
                        .map(|(_, signature)| signature.return_type)
                });
            return_type.is_some_and(|return_type| type_reference_is_unit(program, return_type))
        }

        fn resultless_machine_state(
            program: &dyn ::validation::CheckingInput,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> bool {
            program
                .machine_by_symbol(machine_symbol)
                .and_then(|machine| program.state_in_machine(machine, state_symbol))
                .is_some_and(|state| type_reference_is_unit(program, state.return_type))
        }

        fn type_reference_is_unit(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            matches!(
                program.type_reference_table.type_reference(type_reference),
                typed_trees::types::TypeReferenceNode::Unit
            )
        }
    }
    mod finalization {
        /*
            Finalizing checked authored selections.

            Every authored selection resolves to its checked target here, except a
            late-bound selection in compiler toolchain source, a target sibling's body
            or a build descriptor: those stay late-bound for the realization or build
            evaluation that binds them. Both package checkpoints apply this one rule.
        */
        use crate::shared::authored_selections::call_targets::{
            checked_call_target, checked_name_path_segment_target,
            checked_operator_conformance_targets, expression_receiver_symbol,
            implementation_call_selection,
        };
        use crate::shared::authored_selections::member_targets::checked_member_target;
        use crate::shared::authored_selections::operator_targets::{
            GenericOperatorValueOrigins, checked_generic_operator_target,
            checked_operator_target_for_occurrence, checked_structural_equality_call,
            typed_operator_has_no_authored_selection,
        };
        use crate::shared::authored_selections::selection_collection::{
            checked_struct_literal_type_symbol, collect_checked_proof_membership_selections,
            collect_checked_proof_view_call_selections, collect_checked_statement_selections,
        };
        use checked_trees::CheckFacts;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelection,
            AuthoredDeclarationSelectionIntrinsic as SelectionIntrinsic,
            AuthoredDeclarationSelectionKind as SelectionKind,
            AuthoredDeclarationSelectionLateBinding as LateBinding,
            AuthoredDeclarationSelectionOccurrenceId,
            AuthoredDeclarationSelectionTarget as SelectionTarget, AuthoredDeclarationSelections,
            BuildOperation,
        };
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::expression::ExpressionNode;
        use validation::selection_resolution::{
            AuthoredSelectionResolution, AuthoredSelectionResolutionTarget as ResolutionTarget,
            declaration_target, finalization_diagnostic, push_consistent_resolution,
        };

        /* One occurrence's inferred conformances and resolution, or its refusal. */
        type OccurrenceResolution = Result<
            (
                Vec<(
                    source::SourceSpan,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
                    SymbolHandle,
                )>,
                Option<AuthoredSelectionResolution>,
            ),
            Diagnostic,
        >;

        pub(crate) fn finalize_checked_authored_selections(
            program: &dyn ::validation::CheckingInput,
            facts: &mut CheckFacts,
        ) -> Result<AuthoredDeclarationSelections, Diagnostic> {
            let mut resolutions = Vec::new();
            let mut inferred_conformances = Vec::new();
            let expressions = &program.tables.expression_table;
            let owner_index = {
                validation::expression_owners::OwnerEnvironmentIndex::with_call_signatures(
                    program,
                    &facts.proof.contract_facts,
                    &facts.proof.call_contract_signatures,
                )
            };

            {
                super::build_selections::validate(program, &owner_index)?;
            }

            super::static_inputs::collect(program, &mut resolutions)?;

            // Materialized inputs were resolved from their checked arguments
            // above; descriptors remain owned by Build. Neither should be
            // inferred again from the call that records the Product request.
            let mut separately_owned_inputs =
                validation::execution_projection::product_request_operand_occurrences(program);
            separately_owned_inputs.extend(resolutions.iter().map(|row| row.occurrence));
            {
                let rows = expressions.iter_expressions().collect::<Vec<_>>();
                let generic_operator_values =
                    std::sync::OnceLock::<GenericOperatorValueOrigins>::new();
                let owner_index = &owner_index;
                let separately_owned_inputs = &separately_owned_inputs;
                let facts: &CheckFacts = facts;
                let resolved: Vec<Vec<OccurrenceResolution>> =
                    validation::checking_thread_pool().install(|| {
                        use rayon::prelude::*;
                        rows.par_iter()
                            .with_min_len(256)
                            .map(|&(expression, node)| {
                    let occurrences = expressions
                        .authored_selection_occurrences(expression)
                        .collect::<Vec<_>>();
                    let mut out = Vec::with_capacity(occurrences.len());
                    for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
                        let mut found = Vec::new();
                        let item = (|| -> OccurrenceResolution {
                        let Some(selection) = program.authored_declaration_selections().get(occurrence)
                        else {
                            return Err(Diagnostic::error(format!(
                                "expression retains unknown authored declaration selection occurrence {}",
                                occurrence.ordinal()
                            )));
                        };

                        let SelectionTarget::LateBound(binding) = selection.target() else {
                            return Ok((found, None));
                        };

                        if separately_owned_inputs.contains(&occurrence) {
                            return Ok((found, None));
                        }
                        if binding == LateBinding::CheckedOperator {
                            for selected_symbol in checked_operator_conformance_targets(facts, expression) {
                                let inferred = (
                                    selection.source_span(),
                                    selection.exposure(),
                                    selected_symbol,
                                );
                                found.push(inferred);
                            }
                        }
                        let request_target =
                            validation::compiler_operations::build_product_request_intrinsic(
                                program, expression, selection,
                            )
                            .map_err(|reason| {
                                Diagnostic::error(reason).with_source_span(selection.source_span())
                            })?;
                        let target = if let Some(intrinsic) = request_target {
                            Some(ResolutionTarget::Intrinsic(intrinsic))
                        } else {
                            match (binding, node) {
                        (
                            LateBinding::CheckedStaticArgument,
                            ExpressionNode::Call(call),
                        ) if BuildOperation::from_call_target(call.target.as_str())
                            .is_some_and(BuildOperation::selects_product_declarations) =>
                        {
                            let Some(argument_index) = selection.top_level_static_argument_position() else {
                                return Ok((found, None));
                            };
                            let argument = call.machine_arguments.get(argument_index);
                            if validation::compiler_operations::is_build_declaration_selection(program, expression) {
                                let selected = validation::compiler_operations::resolve_build_static_operand(
                                    program, expression, argument_index, selection.source_span(),
                                ).map_err(|message| Diagnostic::error(message)
                                    .with_source_span(selection.source_span()))?;
                                Some(selected)
                            } else {
                                declaration_target(argument.map(|argument| argument.symbol).unwrap_or_default())
                            }
                        }
                        (
                            LateBinding::CheckedCall,
                            ExpressionNode::Call(call),
                        ) if call.quotient_operation.is_some() => {
                            let request = call
                                .quotient_operation
                                .as_ref()
                                .expect("guarded sealed quotient request");
                            if call.target_symbol.is_valid() {
                                return Err(Diagnostic::error(
                                    "a sealed quotient request cannot also select an authored declaration",
                                )
                                .with_source_span(selection.source_span()));
                            }
                            Some(ResolutionTarget::Intrinsic(match request.kind {
                                typed_trees::expression::QuotientOperationKind::Define => {
                                    SelectionIntrinsic::QuotientDefine
                                }
                                typed_trees::expression::QuotientOperationKind::Lift => {
                                    SelectionIntrinsic::QuotientLift
                                }
                            }))
                        }
                        (
                            LateBinding::CheckedCall,
                            ExpressionNode::Call(call),
                        ) => {
                            program.expression_table.build_selection_request(call.build_selection)
                            .and_then(|request| match request {
                                language_semantics::declaration_selection::BuildSelectionRequest::Declaration(request) => Some(request.operation.intrinsic()),
                                language_semantics::declaration_selection::BuildSelectionRequest::ProviderReadiness(_) => Some(BuildOperation::ProviderReadiness.intrinsic()),
                                _ => None,
                            })
                            .or_else(|| validation::compiler_operations::classify_expression_call_intrinsic(program, call))
                            .map(ResolutionTarget::Intrinsic)
                            .or_else(|| {
                                declaration_target(
                                    implementation_call_selection(
                                        program,
                                        expression_receiver_symbol(program, call.receiver),
                                    )
                                    .unwrap_or_else(|| {
                                        checked_call_target(
                                            program,
                                            facts,
                                            expression,
                                            call.target_symbol,
                                            selection.source_span(),
                                        )
                                    }),
                                )
                            })
                        }
                        (
                            LateBinding::CheckedMember,
                            ExpressionNode::Member(member),
                        ) => {
                            checked_member_target(program, facts, expression, member, Some(owner_index))
                        }
                        (
                            LateBinding::CheckedStaticPathSegment,
                            ExpressionNode::Name(path),
                        ) => {
                            declaration_target(checked_name_path_segment_target(
                                program,
                                expression,
                                path,
                                late_binding_ordinal(program, &occurrences[..occurrence_offset], binding),
                            ))
                        }
                        (
                            LateBinding::CheckedStructLiteralType,
                            ExpressionNode::StructLiteral(literal),
                        ) => declaration_target(checked_struct_literal_type_symbol(
                            program,
                            literal,
                            selection.source_span(),
                        )),
                        (
                            LateBinding::CheckedStructLiteralCase,
                            ExpressionNode::StructLiteral(literal),
                        ) => declaration_target(literal.case_symbol.unwrap_or_else(SymbolHandle::invalid)),
                        (
                            LateBinding::CheckedStructLiteralField,
                            ExpressionNode::StructLiteral(literal),
                        ) => declaration_target(
                            expressions
                                .struct_fields(literal.fields)
                                .get(late_binding_ordinal(
                                    program,
                                    &occurrences[..occurrence_offset],
                                    binding,
                                ))
                                .map(|field| {
                                    if field.field_symbol.is_valid() {
                                        field.field_symbol
                                    } else {
                                        validation::resolve_member_symbol_from_type_symbol(
                                            program,
                                            checked_struct_literal_type_symbol(
                                                program,
                                                literal,
                                                selection.source_span(),
                                            ),
                                            field.name.as_str(),
                                        )
                                        .unwrap_or_else(SymbolHandle::invalid)
                                    }
                                })
                                .unwrap_or_else(SymbolHandle::invalid),
                        ),
                        (LateBinding::CheckedOperator, _)
                            if matches!(
                                node,
                                ExpressionNode::Binary(_)
                                    | ExpressionNode::Indexed(_)
                                    | ExpressionNode::Unary(_)
                            ) =>
                        {
                            checked_generic_operator_target(
                                program,
                                facts,
                                generic_operator_values.get_or_init(|| {
                                    GenericOperatorValueOrigins::index(program, facts)
                                }),
                                expression,
                            )?
                            .or_else(|| checked_operator_target_for_occurrence(
                                program,
                                facts,
                                expression,
                                node,
                                occurrence,
                                Some(owner_index),
                            ))
                            .or_else(|| {
                                typed_operator_has_no_authored_selection(program, expression).then_some(
                                    ResolutionTarget::Intrinsic(
                                        SelectionIntrinsic::BuiltinOperator,
                                    ),
                                )
                            })
                        }
                        (
                            LateBinding::CheckedOperator,
                            ExpressionNode::Boolean(_)
                            | ExpressionNode::Float(_)
                            | ExpressionNode::Integer(_),
                        ) => Some(ResolutionTarget::Intrinsic(
                            SelectionIntrinsic::BuiltinOperator,
                        )),
                        (
                            LateBinding::CheckedOperator,
                            ExpressionNode::Call(call),
                        ) if call.operational_acknowledgement.origin
                            == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized =>
                        {
                            checked_structural_equality_call(program, facts, expression, call)
                                .then_some(ResolutionTarget::Intrinsic(
                                    SelectionIntrinsic::BuiltinOperator,
                                ))
                                .or_else(|| {
                                    declaration_target(
                                        validation::token_bound_machine_call_target(program, call)
                                            .unwrap_or_else(SymbolHandle::invalid),
                                    )
                                })
                        }
                        _ => None,
                    }
                        };
                        Ok((
                            found,
                            target.map(|target| AuthoredSelectionResolution {
                                occurrence,
                                binding,
                                target,
                            }),
                        ))
                        })();
                        let refused = item.is_err();
                        out.push(item);
                        if refused {
                            break;
                        }
                    }
                    out
                            })
                            .collect()
                    });
                for item in resolved.into_iter().flatten() {
                    let (found, resolution) = item?;
                    for inferred in found {
                        crate::shared::collections::push_unique(
                            &mut inferred_conformances,
                            inferred,
                        );
                    }
                    if let Some(resolution) = resolution {
                        push_consistent_resolution(program, &mut resolutions, resolution)?;
                    }
                }
            }

            {
                collect_checked_statement_selections(program, facts, &mut resolutions)?;
            }
            {
                collect_checked_proof_membership_selections(program, facts, &mut resolutions)?;
            }
            let mut unoccurred_view_calls = Vec::new();
            {
                collect_checked_proof_view_call_selections(
                    program,
                    &mut resolutions,
                    &mut unoccurred_view_calls,
                )?;
            }

            let mut selections = program.authored_declaration_selections().clone();
            for resolution in resolutions {
                let result = match resolution.target {
                    ResolutionTarget::Declaration(selected) => selections.finalize_late_bound(
                        resolution.occurrence,
                        resolution.binding,
                        selected,
                    ),
                    ResolutionTarget::Intrinsic(intrinsic) => selections.finalize_intrinsic(
                        resolution.occurrence,
                        resolution.binding,
                        intrinsic,
                    ),
                };
                result.map_err(|error| finalization_diagnostic(resolution, error))?;
            }
            let mut finalized_selection_occurrences = arena::Arena::new();
            let mut minted_view_calls: Vec<(
                source::SourceSpan,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
                AuthoredDeclarationSelectionOccurrenceId,
            )> = Vec::new();
            for (source_span, exposure, expression) in unoccurred_view_calls {
                let occurrence =
                    match minted_view_calls
                        .iter()
                        .find(|(seen_span, seen_exposure, _)| {
                            *seen_span == source_span && *seen_exposure == exposure
                        }) {
                        Some((_, _, occurrence)) => *occurrence,
                        None => {
                            let occurrence = selections
                                .record_late_bound(
                                    source_span,
                                    exposure,
                                    SelectionKind::Call,
                                    LateBinding::CheckedCall,
                                )
                                .map_err(|error| {
                                    Diagnostic::error(format!(
                                        "failed to retain proof-view call selection: {error:?}"
                                    ))
                                    .with_source_span(source_span)
                                })?;
                            selections
                                .finalize_intrinsic(
                                    occurrence,
                                    LateBinding::CheckedCall,
                                    SelectionIntrinsic::ProofView,
                                )
                                .map_err(|error| {
                                    Diagnostic::error(format!(
                                        "failed to finalize proof-view call selection: {error:?}"
                                    ))
                                    .with_source_span(source_span)
                                })?;
                            minted_view_calls.push((source_span, exposure, occurrence));
                            occurrence
                        }
                    };
                if !finalized_selection_occurrences
                    .iter()
                    .any(|(_, retained)| *retained == (expression, Some(occurrence)))
                {
                    finalized_selection_occurrences.insert((expression, Some(occurrence)));
                }
            }
            for (source_span, exposure, selected_symbol) in inferred_conformances {
                let already_retained = selections
                    .resolved_occurrence(
                        source_span,
                        exposure,
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance,
                        selected_symbol,
                    )
                    .is_some();
                if !already_retained {
                    selections
                        .record_resolved(
                            source_span,
                            exposure,
                            language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance,
                            selected_symbol,
                        )
                        .map_err(|error| {
                            Diagnostic::error(format!(
                                "failed to retain checked authored conformance selection: {error:?}"
                            ))
                            .with_source_span(source_span)
                    })?;
                }
            }
            let descriptor_occurrences =
                validation::execution_projection::descriptor_selection_occurrences(program);
            if let Some(selection) = selections.iter().find(|selection| {
                if !matches!(selection.target(), SelectionTarget::LateBound(_))
                    || descriptor_occurrences.contains(&selection.occurrence_id())
                {
                    return false;
                }
                if selection_is_derived_wire_codec(program, **selection) {
                    return false;
                }
                program
                    .symbols
                    .source_file(selection.source_span())
                    .is_none_or(|source| source.origin != source::SourceOrigin::Toolchain)
            }) {
                let SelectionTarget::LateBound(binding) = selection.target() else {
                    unreachable!("guarded late-bound authored selection")
                };
                if selection.kind() == SelectionKind::Call
                    && binding == LateBinding::CheckedCall
                    && let Some(callee) = undeclared_checked_call_callee(program, *selection)
                {
                    return Err(Diagnostic::error(format!(
                        "call `{callee}` selects no declaration: `{callee}` is not a declared machine, data, or proof definition"
                    ))
                    .with_source_span(selection.source_span()));
                }
                let location = program
                    .symbols
                    .source_file(selection.source_span())
                    .map(|source| source.path.display().to_string())
                    .unwrap_or_else(|| "<no source file>".to_owned());
                return Err(Diagnostic::error(format!(
                    "authored {:?} declaration selection occurrence {} (`{}` in {location}) remained unresolved after successful checking ({binding:?})",
                    selection.kind(),
                    selection.occurrence_id().ordinal(),
                    program.symbols.source_text(selection.source_span()),
                ))
                .with_source_span(selection.source_span()));
            }
            facts.finalized_selection_occurrences = finalized_selection_occurrences;
            Ok(selections)
        }

        fn late_binding_ordinal(
            program: &dyn ::validation::CheckingInput,
            prior_occurrences: &[AuthoredDeclarationSelectionOccurrenceId],
            binding: LateBinding,
        ) -> usize {
            prior_occurrences
                .iter()
                .filter(|occurrence| {
                    program
                        .authored_declaration_selections()
                        .get(**occurrence)
                        .is_some_and(|selection| {
                            selection.target() == SelectionTarget::LateBound(binding)
                        })
                })
                .count()
        }

        fn selection_is_derived_wire_codec(
            program: &dyn ::validation::CheckingInput,
            selection: AuthoredDeclarationSelection,
        ) -> bool {
            program.machines().iter().any(|machine| {
                program.machine_states(machine).iter().any(|state| {
                    program
                        .statement_table
                        .iter_statements(state.statement_nodes)
                        .any(|(_, statement)| match statement {
                            typed_trees::statement::StatementNode::Call(call)
                                if call.authored_call_selection
                                    == Some(selection.occurrence_id()) =>
                            {
                                validation::compiler_operations::statement_wire_codec_intrinsic(
                                    program, call,
                                )
                                .is_some()
                            }
                            _ => false,
                        })
                })
            })
        }

        /*
            Names the receiverless authored call target that stayed late-bound because
            no declaration carries the name — the `Bag(items)` proof-view shape, where
            every declared-target resolution correctly found no candidate. Statement
            calls resolve through the same rule with their own selection row: a call
            whose callee was filtered out of the selected program (e.g. a
            foreign-target method) records no call fact, so only the call row's own
            selection occurrence still names it. Receiver calls, qualified paths, and
            already-bound targets keep the generic unresolved-selection diagnostic so
            selection ambiguity retains its ledger identity.
        */
        fn undeclared_checked_call_callee(
            program: &dyn ::validation::CheckingInput,
            selection: AuthoredDeclarationSelection,
        ) -> Option<String> {
            if let Some(name) = undeclared_statement_call_callee(program, selection) {
                return Some(name);
            }
            let expressions = &program.tables.expression_table;
            expressions
                .iter_expressions()
                .find_map(|(expression, node)| {
                    let ExpressionNode::Call(call) = node else {
                        return None;
                    };
                    if !expressions
                        .authored_selection_occurrences(expression)
                        .any(|occurrence| occurrence == selection.occurrence_id())
                    {
                        return None;
                    }
                    if call.target_symbol.is_valid() || call.receiver.is_valid() {
                        return None;
                    }
                    let name = call.target.as_str();
                    if name.contains("::") {
                        return None;
                    }
                    (!top_level_declaration_exists(program, name, selection.source_span()))
                        .then(|| name.to_string())
                })
        }

        /*
            The undeclared-callee check's declaration roster: every top-level kind
            that could still carry the call's target name in the selected program.
        */
        fn top_level_declaration_exists(
            program: &dyn ::validation::CheckingInput,
            name: &str,
            reference: source::SourceSpan,
        ) -> bool {
            program
                .symbols
                .find_top_level_declaration_or_operator_family_from_source(
                    name,
                    &[
                        SymbolKind::Machine,
                        SymbolKind::Data,
                        SymbolKind::MathematicalDefinition,
                        SymbolKind::Proposition,
                        SymbolKind::BuiltinFunction,
                        SymbolKind::Operator,
                        SymbolKind::Measure,
                        SymbolKind::Domain,
                    ],
                    reference,
                )
                .is_some()
        }

        /*
            The statement-call twin of `undeclared_checked_call_callee`. A statement
            call (`legs.acquire();`) carries its authored selection on the call row
            rather than the expression table, and when the callee's declaration was
            filtered out of the selected program the checked call facts record no row
            — the statement would otherwise compile as if absent. The callee name is
            `target`, so receiver calls are named here where the expression path
            skips them. The name counts as declared only when the selected program
            carries it somewhere — a top-level declaration, a machine, or any
            machine's state — so a surviving same-named method still suppresses the
            diagnostic and selection ambiguity keeps the generic ledger identity.
        */
        fn undeclared_statement_call_callee(
            program: &dyn ::validation::CheckingInput,
            selection: AuthoredDeclarationSelection,
        ) -> Option<String> {
            let call = program.machines().iter().find_map(|machine| {
                program.machine_states(machine).iter().find_map(|state| {
                    program
                        .statement_table
                        .iter_statements(state.statement_nodes)
                        .find_map(|(_, statement)| match statement {
                            typed_trees::statement::StatementNode::Call(call)
                                if call.authored_call_selection
                                    == Some(selection.occurrence_id()) =>
                            {
                                Some(call)
                            }
                            _ => None,
                        })
                })
            })?;
            if call.target_symbol.is_valid() {
                return None;
            }
            let name = call.target.as_str();
            if name.contains("::") {
                return None;
            }
            let declared = top_level_declaration_exists(program, name, selection.source_span())
                || program.machines().iter().any(|machine| {
                    machine.name.as_str() == name
                        || program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.name.as_str() == name)
                });
            (!declared).then(|| name.to_string())
        }
    }
    mod member_targets {
        /*
            Contextual domain, value and statement member targets.
        */
        use arena::Arena;
        use typed_trees::data::DataMember;
        use typed_trees::machine::Machine;
        use typed_trees::operator::OperandType;
        use typed_trees::operator::binary_operator_spelling;
        use typed_trees::operator::resolve_spelling_for_operand_types;
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::TypeReferenceHandle;
        use validation::expression_owners::OwnerEnvironmentIndex;
        use validation::expression_owners::OwnerMemberTarget;
        use validation::expression_owners::checked_member_target_from_exact_owner;
        use validation::expression_traversal::collect_statement_expression_trees;
        use validation::field_domains::data_definition_by_symbol_or_name;
        use validation::selection_resolution::declaration_target;
        use validation::selection_resolution::{
            AuthoredSelectionResolutionTarget as ResolutionTarget, authored_data_member,
        };

        use crate::flow::indexed_statements;
        use crate::shared::authored_selections::operator_targets::{
            authored_operand_descriptor, authored_operand_type,
        };
        use checked_trees::CheckFacts;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
        use validation::collection_operations::{MeasureReceiver, collection_measure_member};
        use validation::{
            data_member_symbol_by_name, declaration_type_reference, effective_member_symbol,
            expression_type_reference_in_state,
        };

        pub(crate) fn checked_member_target(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: ExpressionHandle,
            member: &TableMemberExpression,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<ResolutionTarget> {
            declaration_target(effective_member_symbol(program, member.receiver, member))
                .or_else(|| {
                    checked_member_target_from_exact_owner(
                        program,
                        &facts.proof.contract_facts,
                        expression,
                        member,
                        owner_index,
                    )
                    .map(|target| match target {
                        OwnerMemberTarget::Declaration(symbol) => {
                            ResolutionTarget::Declaration(symbol)
                        }
                        OwnerMemberTarget::CollectionMeasure(measure) => {
                            ResolutionTarget::Intrinsic(measure.intrinsic())
                        }
                    })
                })
                .or_else(|| {
                    let matching = facts
                        .fact_call_projections
                        .iter()
                        .filter(|projection| projection.projection_expression == expression)
                        .collect::<Vec<_>>();
                    let [projection] = matching.as_slice() else {
                        return None;
                    };
                    declaration_target(projection.field)
                })
                .or_else(|| checked_value_member_target(program, facts, member))
                .or_else(|| authored_member_target(program, member))
                .or_else(|| contextual_statement_member_target(program, expression, member))
                .map(|target| match target {
                    ResolutionTarget::Declaration(symbol) => {
                        ResolutionTarget::Declaration(authored_data_member(program, symbol))
                    }
                    target => target,
                })
        }

        fn checked_value_member_target(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            member: &TableMemberExpression,
        ) -> Option<ResolutionTarget> {
            let mut resolved = None;
            for (_, value) in facts.values.expression_values(member.receiver) {
                if !value.type_reference.is_valid() {
                    continue;
                }
                let target = member_symbol_from_type_reference(
                    program,
                    value.type_reference,
                    member.member.as_str(),
                )
                .and_then(declaration_target)
                .or_else(|| collection_measure_target(program, value.type_reference, member));
                let Some(target) = target else {
                    continue;
                };
                if resolved.is_some_and(|candidate| candidate != target) {
                    return None;
                }
                resolved = Some(target);
            }
            resolved
        }

        fn authored_member_target(
            program: &dyn ::validation::CheckingInput,
            member: &TableMemberExpression,
        ) -> Option<ResolutionTarget> {
            let receiver_type = authored_operand_type(program, member.receiver)?;
            member_symbol_from_type_reference(program, receiver_type, member.member.as_str())
                .and_then(declaration_target)
                .or_else(|| collection_measure_target(program, receiver_type, member))
        }

        /*
            Run `visit` on every `(state, statement_index, statement)` whose
            statement expression tree contains `needle`; `None` aborts the walk.
        */
        pub(super) fn for_each_statement_containing(
            program: &dyn ::validation::CheckingInput,
            needle: ExpressionHandle,
            mut visit: impl FnMut(&Machine, &State, usize, &StatementNode) -> Option<()>,
        ) -> Option<()> {
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for (statement_index, statement) in
                        indexed_statements(program, state.statement_nodes)
                    {
                        let mut expressions = Vec::new();
                        collect_statement_expression_trees(program, statement, &mut expressions);
                        if !expressions.contains(&needle) {
                            continue;
                        }
                        visit(machine, state, statement_index, statement)?;
                    }
                }
            }
            Some(())
        }

        fn contextual_statement_member_target(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            member: &TableMemberExpression,
        ) -> Option<ResolutionTarget> {
            let mut resolved = None;
            for_each_statement_containing(program, expression, |_, state, statement_index, _| {
                let receiver_type = expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    member.receiver,
                )?;
                let target = member_symbol_from_type_reference(
                    program,
                    receiver_type,
                    member.member.as_str(),
                )
                .and_then(declaration_target)
                .or_else(|| collection_measure_target(program, receiver_type, member))?;
                if resolved.is_some_and(|candidate| candidate != target) {
                    return None;
                }
                resolved = Some(target);
                Some(())
            })?;
            resolved
        }

        fn member_symbol_from_type_reference(
            program: &dyn ::validation::CheckingInput,
            type_reference: TypeReferenceHandle,
            member_name: &str,
        ) -> Option<SymbolHandle> {
            use typed_trees::types::TypeReferenceNode;

            let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    return member_symbol_from_type_reference(program, *referee, member_name);
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    return member_symbol_from_type_reference(program, *base_type, member_name);
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    ..
                } => (*base_symbol, base_name),
                TypeReferenceNode::Named { symbol, name } => (*symbol, name),
                TypeReferenceNode::Callable { .. } => return None,
                TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::FixedArray { .. }
                | TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::Unit => return None,
            };
            let data = data_definition_by_symbol_or_name(program, symbol, name)?;
            data_member_symbol_by_name(program, data, member_name).or_else(|| {
                program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) => program
                            .data_payload_fields(variant)
                            .iter()
                            .find_map(|field| {
                                (field.name.as_str() == member_name).then_some(field.symbol)
                            }),
                        _ => None,
                    })
            })
        }

        /*
            A `len` or `capacity` member on a fixed array or slice selects the
            compiler-owned standing measure of that name; neither is a package
            declaration. Anything else yields no intrinsic target.
        */
        fn collection_measure_target(
            program: &dyn ::validation::CheckingInput,
            type_reference: TypeReferenceHandle,
            member: &TableMemberExpression,
        ) -> Option<ResolutionTarget> {
            let measure = collection_measure_member(
                program,
                member,
                MeasureReceiver::Declared(type_reference),
            )?;
            Some(ResolutionTarget::Intrinsic(measure.intrinsic()))
        }

        /*
            With a walk's `owners` index the statements containing the expression come
            from its recorded sites instead of a whole-program rescan; the verdict
            (every containing statement types the operand as a primitive, and one
            exists) does not depend on the order they are visited in.
        */
        pub(crate) fn expression_is_contextual_statement_primitive(
            program: &dyn ::validation::CheckingInput,
            _facts: &CheckFacts,
            containing_expression: ExpressionHandle,
            operand: ExpressionHandle,
            owners: Option<&OwnerEnvironmentIndex>,
        ) -> bool {
            if let Some(owners) = owners {
                let sites = owners.containing_statement_sites(containing_expression);
                let mut found = false;
                for (machine, state, statement_index) in sites {
                    let machine = &program.machines()[machine];
                    let state = &program.machine_states(machine)[state];
                    let Some(type_reference) = expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        operand,
                    ) else {
                        return false;
                    };
                    if program.primitive_type_reference(type_reference).is_none() {
                        return false;
                    }
                    found = true;
                }
                return found;
            }
            let mut found = false;
            for_each_statement_containing(
                program,
                containing_expression,
                |_, state, statement_index, _| {
                    let type_reference = expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        operand,
                    )?;
                    program.primitive_type_reference(type_reference)?;
                    found = true;
                    Some(())
                },
            )
            .is_some()
                && found
        }

        /*
            `program_owners`, when a walk supplies one, is its index over no checked
            facts (`OwnerEnvironmentIndex::program_only`).
        */
        pub(crate) fn expression_is_intrinsic_primitive_without_origin(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            program_owners: Option<&OwnerEnvironmentIndex>,
        ) -> bool {
            let type_reference = match program.tables.expression_table.expression(expression) {
                ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Integer(_) => {
                    return true;
                }
                ExpressionNode::Name(path) => declaration_type_reference(program, path.symbol),
                ExpressionNode::Call(call) => program
                    .machines()
                    .iter()
                    .flat_map(|machine| program.machine_states(machine))
                    .find_map(|state| {
                        (state.symbol == call.target_symbol).then_some(state.return_type)
                    }),
                ExpressionNode::Cast(cast) => Some(cast.target_type),
                ExpressionNode::Member(member) => {
                    if matches!(
                        checked_member_target_from_exact_owner(
                            program,
                            &Arena::default(),
                            expression,
                            member,
                            program_owners,
                        ),
                        Some(OwnerMemberTarget::CollectionMeasure(_))
                    ) {
                        return true;
                    }
                    declaration_type_reference(
                        program,
                        effective_member_symbol(program, member.receiver, member),
                    )
                }
                ExpressionNode::Binary(binary)
                    if binary_operator_spelling(binary.operator).is_none() =>
                {
                    return true;
                }
                ExpressionNode::Binary(binary) => {
                    let Some(spelling) = binary_operator_spelling(binary.operator) else {
                        unreachable!("handled above")
                    };
                    let operand_types = [
                        authored_operand_descriptor(program, binary.left),
                        authored_operand_descriptor(program, binary.right),
                    ];
                    if operand_types.iter().all(|operand| operand.is_unknown()) {
                        return expression_is_anonymous_arithmetic(program, binary.left)
                            && expression_is_anonymous_arithmetic(program, binary.right);
                    }
                    if !resolve_spelling_for_operand_types(
                        program,
                        spelling,
                        &operand_types,
                        program.tables.expression_table.source_span(expression),
                    )
                    .is_empty()
                    {
                        return false;
                    }
                    return operand_types
                        .iter()
                        .any(|operand| matches!(operand, OperandType::Primitive(_)))
                        || expression_is_intrinsic_primitive_without_origin(
                            program,
                            binary.left,
                            program_owners,
                        )
                        || expression_is_intrinsic_primitive_without_origin(
                            program,
                            binary.right,
                            program_owners,
                        );
                }
                ExpressionNode::Unary(_) => return true,
                ExpressionNode::Borrow(inner) => {
                    return expression_is_intrinsic_primitive_without_origin(
                        program,
                        inner.target,
                        program_owners,
                    );
                }
                _ => None,
            };
            type_reference
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
                .is_some()
        }

        /*
            Whether an expression is anonymous arithmetic: a literal that has not
            landed (no width suffix and no captured carrier), or arithmetic whose
            operands are all anonymous. Such an expression carries no type of its own
            until it lands, so no authored operator spelling applies to it.
        */
        fn expression_is_anonymous_arithmetic(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            use typed_trees::expression::BinaryOperator;
            match program.tables.expression_table.expression(expression) {
                ExpressionNode::Float(literal) => literal.landing().is_none(),
                ExpressionNode::Integer(literal) => literal.landing().is_none(),
                ExpressionNode::Binary(binary) => {
                    matches!(
                        binary.operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                            | BinaryOperator::Modulo
                    ) && expression_is_anonymous_arithmetic(program, binary.left)
                        && expression_is_anonymous_arithmetic(program, binary.right)
                }
                _ => false,
            }
        }
    }
    mod operator_targets {
        /*
            Checked operator targets and authored operator selection candidates.
        */
        /* Handle-keyed maps take the integer hasher; none is iterated for output order. */
        type HashMap<K, V> = std::collections::HashMap<K, V, BuildSymbolHasher>;

        use crate::flow::expressions;
        use crate::shared::authored_selections::contract_resolution;
        use crate::shared::authored_selections::member_targets::{
            expression_is_contextual_statement_primitive,
            expression_is_intrinsic_primitive_without_origin,
        };
        use checked_trees::{
            CheckFacts, CheckedOperatorOccurrence,
            CheckedOperatorResolutionStatus as ResolutionStatus, CheckedValueOrigin,
        };
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionIntrinsic as SelectionIntrinsic,
            AuthoredDeclarationSelectionOccurrenceId,
        };
        use symbols::{BuildSymbolHasher, SymbolHandle};
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{
            ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
        };
        use typed_trees::operator::OperatorDefinition;
        use typed_trees::operator::SpelledOperator;
        use typed_trees::operator::binary_operator_spelling;
        use typed_trees::operator::resolve_spelling;
        use typed_trees::operator::resolve_spelling_for_operand_types;
        use typed_trees::operator::resolve_spelling_for_operands;
        use typed_trees::types::TypeReferenceHandle;
        use validation::expression_owners::{
            OwnerEnvironmentIndex, expression_contains, reachable_expressions,
        };
        use validation::selection_resolution::AuthoredSelectionResolutionTarget;
        use validation::selection_resolution::{declaration_target, exact_named_operator_call};
        use validation::{
            declaration_type_reference, effective_member_symbol,
            expression_type_reference_for_origin, generic_bound_operator_requirement,
        };

        fn intrinsic_operator_operand_is_primitive(
            program: &dyn ::validation::CheckingInput,
            node: &ExpressionNode,
            origin: CheckedValueOrigin,
        ) -> bool {
            let operand = match node {
                ExpressionNode::Binary(binary) => binary.left,
                ExpressionNode::Unary(unary) => unary.operand,
                _ => return false,
            };
            expression_type_reference_for_origin(program, operand, origin)
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
                .is_some()
                || expression_is_intrinsic_primitive_without_origin(program, operand, None)
        }

        /*
            The checked value facts whose expression contains each expression.

            The open-generic fallback below asks one containment question of every
            checked value fact, and selection finalization asks it once per authored
            operator occurrence. Neither the expression table nor the fact roster
            moves while selections finalize, so one walk per value fact answers every
            occurrence. Origins keep the fact order the repeated walks produced, and
            a fact naming an origin already recorded for that expression adds
            nothing, exactly as the receiving roster's own membership test did.

            Only a fact whose origin names a machine with conformance bounds can
            select a declared generic owner, so only those facts are walked. Every
            nested subexpression carries its own fact, so walking all of them
            visited each node once per enclosing expression: quadratic in an
            operator chain's length.
        */
        pub(crate) struct GenericOperatorValueOrigins {
            by_expression: HashMap<ExpressionHandle, Vec<CheckedValueOrigin>>,
        }

        impl GenericOperatorValueOrigins {
            pub(crate) fn index(
                program: &dyn ::validation::CheckingInput,
                facts: &CheckFacts,
            ) -> Self {
                let mut by_expression: HashMap<_, Vec<CheckedValueOrigin>> = HashMap::default();
                for (_, value) in facts.values.values.iter() {
                    if !origin_machine_symbol(value.origin)
                        .and_then(|machine_symbol| program.machine_by_symbol(machine_symbol))
                        .is_some_and(|machine| !machine.conformance_bounds.is_empty())
                    {
                        continue;
                    }
                    for reached in reachable_expressions(program, value.expression) {
                        let origins = by_expression.entry(reached).or_default();
                        crate::shared::collections::push_unique(&mut *origins, value.origin);
                    }
                }
                Self { by_expression }
            }

            fn origins(&self, expression: ExpressionHandle) -> &[CheckedValueOrigin] {
                self.by_expression
                    .get(&expression)
                    .map_or(&[][..], Vec::as_slice)
            }
        }

        fn origin_machine_symbol(origin: CheckedValueOrigin) -> Option<SymbolHandle> {
            match origin {
                CheckedValueOrigin::MachineDecrease { machine_symbol, .. }
                | CheckedValueOrigin::MachineOwnedDataInitializer { machine_symbol, .. }
                | CheckedValueOrigin::StateStatement { machine_symbol, .. } => Some(machine_symbol),
                CheckedValueOrigin::NestedExpression { .. } => None,
            }
        }

        pub(crate) fn checked_generic_operator_target(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            value_origins: &GenericOperatorValueOrigins,
            expression: ExpressionHandle,
        ) -> Result<Option<AuthoredSelectionResolutionTarget>, Diagnostic> {
            if let Some(operation) = program.expression_table.open_index_operation(expression)
                && let Some(target) = declaration_target(operation.operator)
            {
                return Ok(Some(target));
            }
            let mut selected = None;
            let mut uses = Vec::new();
            facts
                .operators
                .for_each_expression_use(expression, |_, operator_use| {
                    uses.push((operator_use.spelling, operator_use.origin));
                });
            if let Some(binary) = program.expression_table.binary_node(expression)
                && let Some(spelling) = binary_operator_spelling(binary.operator)
            {
                for origin in value_origins.origins(expression) {
                    let selected_use = (spelling, *origin);
                    crate::shared::collections::push_unique(&mut uses, selected_use);
                }
            }
            for (spelling, origin) in uses {
                let Some(machine_symbol) = origin_machine_symbol(origin) else {
                    continue;
                };
                let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                    continue;
                };
                if machine.conformance_bounds.is_empty() {
                    continue;
                }
                let operands = match program.expression_table.expression(expression) {
                    ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
                    ExpressionNode::Unary(unary) => vec![unary.operand],
                    ExpressionNode::Indexed(indexed) => vec![indexed.collection, indexed.index],
                    _ => continue,
                };
                let operand_types = operands
                    .into_iter()
                    .map(|operand| expression_type_reference_for_origin(program, operand, origin))
                    .collect::<Vec<_>>();
                let requirement =
                    generic_bound_operator_requirement(program, machine, spelling, &operand_types)
                        .map_err(Diagnostic::error)?;
                let Some(requirement) = requirement else {
                    continue;
                };
                let target = AuthoredSelectionResolutionTarget::Declaration(requirement.symbol);
                if selected.is_some_and(|previous| previous != target) {
                    return Err(Diagnostic::error(
                        "one authored operator occurrence has conflicting declared generic owners",
                    ));
                }
                selected = Some(target);
            }
            Ok(selected)
        }

        fn checked_operator_target(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: ExpressionHandle,
            node: &ExpressionNode,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<AuthoredSelectionResolutionTarget> {
            if matches!(
                node,
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        typed_trees::expression::BinaryOperator::And
                            | typed_trees::expression::BinaryOperator::BitwiseAnd
                            | typed_trees::expression::BinaryOperator::BitwiseOr
                            | typed_trees::expression::BinaryOperator::BitwiseXor
                            | typed_trees::expression::BinaryOperator::Or
                            | typed_trees::expression::BinaryOperator::ShiftLeft
                            | typed_trees::expression::BinaryOperator::ShiftRight
                    )
            ) {
                return Some(AuthoredSelectionResolutionTarget::Intrinsic(
                    SelectionIntrinsic::BuiltinOperator,
                ));
            }

            let mut uses = Vec::new();
            facts
                .operators
                .for_each_expression_use(expression, |_, operator_use| {
                    if operator_use.occurrence == CheckedOperatorOccurrence::Expression {
                        uses.push(*operator_use);
                    }
                });

            uses.iter()
                .find_map(|operator_use| {
                    (operator_use.status == ResolutionStatus::Resolved)
                        .then(|| declaration_target(operator_use.selected_operator_symbol))
                        .flatten()
                })
                .or_else(|| {
                    uses.iter()
                        .any(|operator_use| {
                            operator_use.status == ResolutionStatus::BuiltinFallback
                                || (operator_use.status == ResolutionStatus::Missing
                                    && intrinsic_operator_operand_is_primitive(
                                        program,
                                        node,
                                        operator_use.origin,
                                    ))
                        })
                        .then_some(AuthoredSelectionResolutionTarget::Intrinsic(
                            SelectionIntrinsic::BuiltinOperator,
                        ))
                })
                .or_else(|| {
                    contract_resolution::checked_operator_resolution(
                        program,
                        facts,
                        expression,
                        node,
                        owner_index,
                    )
                    .and_then(|resolution| match resolution {
                        contract_resolution::CheckedContractOperatorResolution::Declaration(
                            symbol,
                        ) => declaration_target(symbol),
                        contract_resolution::CheckedContractOperatorResolution::Builtin => {
                            Some(AuthoredSelectionResolutionTarget::Intrinsic(
                                SelectionIntrinsic::BuiltinOperator,
                            ))
                        }
                    })
                })
                .or_else(|| {
                    resolve_authored_operator_without_use_fact(program, expression, node)
                        .and_then(|operator| declaration_target(operator.symbol))
                })
                .or_else(|| {
                    let operand = match node {
                        ExpressionNode::Binary(binary) => binary.left,
                        ExpressionNode::Unary(unary) => unary.operand,
                        _ => return None,
                    };
                    (contract_resolution::checked_operand_type(
                        program,
                        facts,
                        expression,
                        operand,
                        owner_index,
                    )
                    .and_then(|type_reference| program.primitive_type_reference(type_reference))
                    .is_some()
                        || expression_is_intrinsic_primitive_without_origin(
                            program,
                            operand,
                            owner_index.map(|owners| owners.program_only(program)),
                        )
                        || checked_operator_expression_is_intrinsic_primitive(facts, operand)
                        || expression_is_contextual_statement_primitive(
                            program,
                            facts,
                            expression,
                            operand,
                            owner_index,
                        ))
                    .then_some(
                        AuthoredSelectionResolutionTarget::Intrinsic(
                            SelectionIntrinsic::BuiltinOperator,
                        ),
                    )
                })
                .or_else(|| {
                    operator_has_no_authored_spelling_candidate(program, expression, node)
                        .then_some(AuthoredSelectionResolutionTarget::Intrinsic(
                            SelectionIntrinsic::BuiltinOperator,
                        ))
                })
        }

        pub(crate) fn checked_operator_target_for_occurrence(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: ExpressionHandle,
            node: &ExpressionNode,
            occurrence: AuthoredDeclarationSelectionOccurrenceId,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<AuthoredSelectionResolutionTarget> {
            let is_rooted = |candidate| {
                owner_index.is_none_or(|owners| owners.contains_expression(candidate)) || {
                    let mut has_use = false;
                    facts.operators.for_each_expression_use(candidate, |_, _| {
                        has_use = true;
                    });
                    has_use
                }
            };
            let copied_target = || {
                expressions(program)
                    .iter_expressions()
                    .filter(|(candidate, _)| *candidate != expression)
                    .filter(|(candidate, _)| {
                        expressions(program)
                            .authored_selection_occurrences(*candidate)
                            .any(|retained| retained == occurrence)
                    })
                    .filter(|(candidate, _)| is_rooted(*candidate))
                    .find_map(|(candidate, candidate_node)| {
                        checked_operator_target(
                            program,
                            facts,
                            candidate,
                            candidate_node,
                            owner_index,
                        )
                    })
            };
            if !is_rooted(expression) {
                copied_target().or_else(|| {
                    checked_operator_target(program, facts, expression, node, owner_index)
                })
            } else {
                checked_operator_target(program, facts, expression, node, owner_index)
                    .or_else(copied_target)
            }
        }

        fn checked_operator_expression_is_intrinsic_primitive(
            facts: &CheckFacts,
            expression: ExpressionHandle,
        ) -> bool {
            let mut is_fallback = false;
            facts
                .operators
                .for_each_expression_use(expression, |_, operator_use| {
                    if operator_use.occurrence == CheckedOperatorOccurrence::Expression
                        && operator_use.status == ResolutionStatus::BuiltinFallback
                    {
                        is_fallback = true;
                    }
                });
            is_fallback
        }

        /*
            Whether a `==`/`!=` occurrence typed lowering rewrote into a call selects
            the `equals` row of an `Equatable` implementation, and checking resolved
            that call to exactly the same row. The operator occurrence then settles as
            the builtin operator: the implementation's authored row supplies its meaning.
        */
        pub(crate) fn checked_structural_equality_call(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            expression: ExpressionHandle,
            call: &TableCallExpression,
        ) -> bool {
            if !call.target_symbol.is_valid() {
                return false;
            }
            let selects_equality_row = program.conformances().iter().any(|conformance| {
                program.symbols.display_path(conformance.trait_symbol, "::") == "Equatable"
                    && conformance.rows.iter().any(|row| {
                        row.realization_state == call.target_symbol
                            && row.requirement_name.as_str() == "equals"
                    })
            });
            if !selects_equality_row {
                return false;
            }

            super::call_targets::checked_call_target(
                program,
                facts,
                expression,
                call.target_symbol,
                program.expression_table.source_span(expression),
            ) == call.target_symbol
        }

        /*
            Declaration contracts and other proof-static expressions are validated
            without an executable value origin, so they do not always produce a
            `CheckedOperatorUseFact`. Finalize their authored selection only when the
            typed operands select one exact declared operator; partial or ambiguous
            reconstruction remains unresolved and therefore rejects package custody.
        */
        fn resolve_authored_operator_without_use_fact<'program>(
            program: &'program dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            node: &ExpressionNode,
        ) -> Option<&'program OperatorDefinition> {
            use language_core::OperatorSpelling;

            let (spelling, operand_types) = match node {
                ExpressionNode::Binary(binary) => {
                    let spelling = binary_operator_spelling(binary.operator)?;
                    (
                        spelling,
                        vec![
                            Some(authored_operand_type(program, binary.left)?),
                            Some(authored_operand_type(program, binary.right)?),
                        ],
                    )
                }
                ExpressionNode::Indexed(indexed) => {
                    let collection = Some(authored_operand_type(program, indexed.collection)?);
                    match program.expression_table.expression(indexed.index) {
                        ExpressionNode::Range(range) => (
                            OperatorSpelling::Range,
                            vec![
                                collection,
                                authored_operand_type(program, range.start),
                                authored_operand_type(program, range.end),
                            ],
                        ),
                        _ => (
                            OperatorSpelling::Index,
                            vec![collection, authored_operand_type(program, indexed.index)],
                        ),
                    }
                }
                _ => return None,
            };
            let candidates = resolve_spelling_for_operands(
                program,
                spelling,
                &operand_types,
                program.expression_table.source_span(expression),
            );
            let [candidate] = candidates.as_slice() else {
                return None;
            };
            Some(candidate.operator)
        }

        /*
            The operand type operator selection must read here: the retained type
            reference when the operand names one, and otherwise the carrier the
            language fixes for a value that names no type at all.

            A comparison result and a boolean literal are `bool`, but neither points at
            a written `bool`. Reported as unknown they match every declared parameter
            as a wildcard, so `(nan32 == nan32) == false` kept a declared `Float::equal`
            as a candidate and could not be recognized as the builtin it is.
        */
        pub(crate) fn authored_operand_descriptor(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> typed_trees::operator::OperandType {
            use typed_trees::operator::OperandType;
            if let Some(type_reference) = authored_operand_type(program, expression) {
                return OperandType::Reference(type_reference);
            }
            authored_operand_carrier(program, expression)
                .map_or(OperandType::Unknown, OperandType::Primitive)
        }

        /*
            The primitive carrier an operand's VALUE has, for an operand that names no
            type reference. An operation's own result is such a value: a comparison
            produces `bool`, and an arithmetic or bitwise operation produces the
            carrier its operands share.

            Each answer holds only where no authored operator supplies a different
            meaning: a declared `less` may return a mask, a declared `add` may return
            another carrier entirely. Candidates that ALL return the same carrier
            settle it whichever one is selected, and no candidate at all leaves the
            builtin operation.
        */
        fn authored_operand_carrier(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<typed_trees::types::PrimitiveType> {
            use typed_trees::expression::{BinaryOperator, UnaryOperator};
            use typed_trees::operator::OperandType;
            use typed_trees::types::PrimitiveType;

            fn operand_carrier(
                program: &dyn ::validation::CheckingInput,
                operand: OperandType,
            ) -> Option<typed_trees::types::PrimitiveType> {
                match operand {
                    OperandType::Reference(reference) => {
                        program.primitive_type_reference(reference)
                    }
                    OperandType::Primitive(primitive) => Some(primitive),
                    OperandType::Unknown => None,
                }
            }

            match program.expression_table.expression(expression) {
                ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
                ExpressionNode::Binary(binary) => {
                    if matches!(
                        binary.operator,
                        BinaryOperator::And | BinaryOperator::Or | BinaryOperator::CaseMembership
                    ) {
                        return Some(PrimitiveType::Bool);
                    }
                    let left = authored_operand_descriptor(program, binary.left);
                    let right = authored_operand_descriptor(program, binary.right);
                    let carrier = match binary.operator {
                        BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual => PrimitiveType::Bool,
                        _ => match (
                            operand_carrier(program, left),
                            operand_carrier(program, right),
                        ) {
                            (Some(left), Some(right)) if left == right => left,
                            (Some(carrier), None) | (None, Some(carrier)) => carrier,
                            _ => return None,
                        },
                    };
                    candidate_operators(program, expression, &[left, right])
                        .iter()
                        .all(|candidate| {
                            program.primitive_type_reference(candidate.operator.return_type)
                                == Some(carrier)
                        })
                        .then_some(carrier)
                }
                ExpressionNode::Unary(unary) => match unary.operator {
                    UnaryOperator::LogicalNot => Some(PrimitiveType::Bool),
                    UnaryOperator::BitwiseNot => operand_carrier(
                        program,
                        authored_operand_descriptor(program, unary.operand),
                    ),
                },
                _ => None,
            }
        }

        pub(crate) fn authored_operand_type(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            if !expression.is_valid() {
                return None;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Integer(_) => {
                    program
                        .closed_integer_value_in(expression, SymbolHandle::invalid())?
                        .type_reference
                }
                ExpressionNode::Atomic(atomic) => authored_operand_type(program, atomic.value),
                ExpressionNode::Borrow(borrow) => authored_operand_type(program, borrow.target),
                ExpressionNode::Cast(cast) => Some(cast.target_type),
                ExpressionNode::Member(member) => declaration_type_reference(
                    program,
                    effective_member_symbol(program, member.receiver, member),
                ),
                ExpressionNode::Call(call) => {
                    exact_named_operator_call(program, call).map(|operator| operator.return_type)
                }
                ExpressionNode::Name(path) => declaration_type_reference(program, path.symbol)
                    .or_else(|| operator_contract_value_type(program, expression, path)),
                // An element of an array literal has the literal's element type,
                // which every element shares; a contract such as
                // `requires [3u64][0] <= 7` names no other typed operand.
                ExpressionNode::Indexed(indexed)
                    if !matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) =>
                {
                    let ExpressionNode::ArrayLiteral(elements) =
                        program.expression_table.expression(indexed.collection)
                    else {
                        return None;
                    };
                    program
                        .expression_table
                        .expression_handles(*elements)
                        .iter()
                        .find_map(|element| authored_operand_type(program, *element))
                }
                _ => None,
            }
        }

        fn operator_contract_value_type(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            path: &TableNamePath,
        ) -> Option<TypeReferenceHandle> {
            let name = expressions(program)
                .name_path_members(path.members)
                .last()?
                .as_str();
            let mut types = program.all_operators().filter_map(|operator| {
                let type_reference = if language_core::is_reserved_result(name) {
                    operator.return_type
                } else {
                    program
                        .operator_parameters(operator)
                        .iter()
                        .find(|parameter| parameter.name.as_str() == name)?
                        .type_reference
                };
                program
                    .operator_contracts(operator)
                    .iter()
                    .any(|contract| {
                        program
                            .proof_facts
                            .span_or_empty(contract.facts)
                            .iter()
                            .any(|fact| match fact {
                                ProofFact::Expression(root) => {
                                    expression_contains(program, *root, expression)
                                }
                                ProofFact::Membership(membership) => {
                                    expression_contains(program, membership.value, expression)
                                }
                                ProofFact::Proposition(_) => false,
                            })
                    })
                    .then_some(type_reference)
            });
            let first = types.next()?;
            types
                .all(|type_reference| type_reference == first)
                .then_some(first)
        }

        fn operator_has_no_authored_spelling_candidate(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            node: &ExpressionNode,
        ) -> bool {
            use language_core::OperatorSpelling;
            let spelling = match node {
                ExpressionNode::Binary(binary) => {
                    let Some(spelling) = binary_operator_spelling(binary.operator) else {
                        return true;
                    };
                    spelling
                }
                ExpressionNode::Indexed(indexed) => {
                    if matches!(
                        program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) {
                        OperatorSpelling::Range
                    } else {
                        OperatorSpelling::Index
                    }
                }
                ExpressionNode::Unary(_) => return true,
                _ => return false,
            };
            resolve_spelling(
                program,
                spelling,
                None,
                program.expression_table.source_span(expression),
            )
            .is_empty()
        }

        /*
            Return whether an early typed operator expression cannot select an authored
            operator declaration. Ordinary checked lowering remains responsible for
            rejecting a semantically invalid builtin use.
        */
        pub(crate) fn typed_operator_has_no_authored_selection(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            let node = program.expression_table.expression(expression);
            operator_has_no_authored_spelling_candidate(program, expression, node)
                || (matches!(node, ExpressionNode::Binary(_))
                    && expression_is_intrinsic_primitive_without_origin(program, expression, None))
        }

        pub(crate) fn typed_operator_authored_selection_candidates(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Vec<SymbolHandle> {
            candidate_operators(program, expression, &[])
                .into_iter()
                .map(|candidate| candidate.operator.symbol)
                .collect()
        }

        /*
            The declared operator candidates of `expression`, for a caller that
            already holds a BINARY expression's two operand descriptors. Deriving them
            again would walk each operand once per enclosing operation, which turns a
            nested arithmetic expression's carrier into an exponential walk.
        */
        fn candidate_operators<'program>(
            program: &'program dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            binary_operands: &[typed_trees::operator::OperandType],
        ) -> Vec<SpelledOperator<'program>> {
            use language_core::OperatorSpelling;

            let (spelling, operand_types) = match program.expression_table.expression(expression) {
                ExpressionNode::Binary(binary) => {
                    let Some(spelling) = binary_operator_spelling(binary.operator) else {
                        return Vec::new();
                    };
                    (
                        spelling,
                        match binary_operands {
                            [left, right] => vec![*left, *right],
                            _ => vec![
                                authored_operand_descriptor(program, binary.left),
                                authored_operand_descriptor(program, binary.right),
                            ],
                        },
                    )
                }
                ExpressionNode::Indexed(indexed) => {
                    let collection = authored_operand_descriptor(program, indexed.collection);
                    match program.expression_table.expression(indexed.index) {
                        ExpressionNode::Range(range) => (
                            OperatorSpelling::Range,
                            vec![
                                collection,
                                authored_operand_descriptor(program, range.start),
                                authored_operand_descriptor(program, range.end),
                            ],
                        ),
                        _ => (
                            OperatorSpelling::Index,
                            vec![
                                collection,
                                authored_operand_descriptor(program, indexed.index),
                            ],
                        ),
                    }
                }
                ExpressionNode::Unary(_) => return Vec::new(),
                _ => return Vec::new(),
            };

            resolve_spelling_for_operand_types(
                program,
                spelling,
                &operand_types,
                program.expression_table.source_span(expression),
            )
        }
    }
    mod selection_collection {
        /*
            Collecting statement, transition-target and proof membership
            selections.
        */
        use crate::flow::expressions;
        use crate::flow::indexed_statements;
        use crate::shared::authored_selections::call_targets::{
            checked_statement_call_target, implementation_call_selection,
        };
        use checked_trees::CheckFacts;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionIntrinsic,
            AuthoredDeclarationSelectionKind as SelectionKind,
            AuthoredDeclarationSelectionLateBinding as LateBinding,
            AuthoredDeclarationSelectionTarget as SelectionTarget,
        };
        use symbols::{SymbolHandle, SymbolKind};
        use validation::compiler_operations::{
            classify_statement_call_intrinsic, statement_wire_codec_intrinsic,
        };
        use validation::selection_resolution::{
            AuthoredSelectionResolution as SelectionResolution,
            AuthoredSelectionResolutionTarget as ResolutionTarget,
        };
        use validation::selection_resolution::{
            collect_checked_transition_target_selections, declaration_target,
            push_consistent_resolution,
        };

        pub(crate) fn checked_struct_literal_type_symbol(
            program: &dyn ::validation::CheckingInput,
            literal: &typed_trees::expression::TableStructLiteral,
            source_span: source::SourceSpan,
        ) -> SymbolHandle {
            if literal.type_symbol.is_valid() {
                return literal.type_symbol;
            }

            program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    literal.type_name.as_str(),
                    &[SymbolKind::Data],
                    source_span,
                )
                .unwrap_or_else(SymbolHandle::invalid)
        }

        pub(crate) fn collect_checked_proof_membership_selections(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            resolutions: &mut Vec<SelectionResolution>,
        ) -> Result<(), Diagnostic> {
            for (fact_handle, fact) in typed_trees::references::program_proof_facts(program) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                let Some(occurrence) = membership.authored_domain_selection else {
                    continue;
                };
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    return Err(Diagnostic::error(format!(
                        "proof membership retains unknown authored selection occurrence {}",
                        occurrence.ordinal(),
                    )));
                };
                if selection.kind() != SelectionKind::DomainMembership {
                    return Err(Diagnostic::error(
                        "proof membership retains mismatched authored domain-selection evidence",
                    )
                    .with_source_span(selection.source_span()));
                }
                if selection.target()
                    != SelectionTarget::LateBound(LateBinding::CheckedDomainMembership)
                {
                    continue;
                }

                let target = declaration_target(membership.domain_symbol).or_else(|| {
                    let mut permission = None;
                    for (_, checked_fact) in facts.semantic.facts.iter() {
                        let facts::FactPayload::ContractCarryPermission {
                            fact,
                            permission: candidate,
                            ..
                        } = checked_fact.payload
                        else {
                            continue;
                        };
                        if fact != fact_handle {
                            continue;
                        }
                        if permission.is_some_and(|retained| retained != candidate) {
                            return None;
                        }
                        permission = Some(candidate);
                    }
                    permission.map(|permission| {
                        ResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::CarryPermission(permission),
                        )
                    })
                });
                if let Some(target) = target {
                    push_consistent_resolution(
                        program,
                        resolutions,
                        SelectionResolution {
                            occurrence,
                            binding: LateBinding::CheckedDomainMembership,
                            target,
                        },
                    )?;
                }
            }
            Ok(())
        }

        /*
            Contract-position calls whose spelled target selects no declaration are
            admitted proof views: `Seq`/`Bag`/`Range`-style schematic atoms mint no
            machine realization, numeric interpretation, or equality semantics
            (validation/src/proof_contracts/contract_entailment/proof_view_tests.rs
            pins this admission). Their checked call occurrences still need an
            explicit ledger target, so they finalize as the compiler-owned ProofView
            intrinsic rather than staying unresolved after checking. Executable
            positions never reach this collector: an undeclared call there fails
            resolution before checking admits the program.

            The resolver deliberately mints no Call occurrence for a contract callee
            that names no visible declaration (the `unbound_contract_call` gate in
            syntax-trees-to-symbol-resolved-trees). Such spellings cannot be bound
            here because there is nothing to bind; instead they are reported through
            `unoccurred_view_calls` so finalization can mint a finalized ProofView row
            directly and retain its call-expression association in checked facts.
        */
        pub(crate) fn collect_checked_proof_view_call_selections(
            program: &dyn ::validation::CheckingInput,
            resolutions: &mut Vec<SelectionResolution>,
            unoccurred_view_calls: &mut Vec<(
                source::SourceSpan,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
                typed_trees::expression::ExpressionHandle,
            )>,
        ) -> Result<(), Diagnostic> {
            let mut fact_roots = Vec::new();
            for (_, fact) in typed_trees::references::program_proof_facts(program) {
                match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        fact_roots.push(*expression);
                    }
                    typed_trees::domain::ProofFact::Membership(membership) => {
                        fact_roots.push(membership.value);
                    }
                    typed_trees::domain::ProofFact::Proposition(application) => {
                        for argument in
                            expressions(program).expression_handles(application.arguments)
                        {
                            fact_roots.push(*argument);
                        }
                    }
                }
            }
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for statement in
                        crate::flow::state_statements(program, state.statement_nodes).iter()
                    {
                        let Some(fact) = statement.as_assembly_fact() else {
                            continue;
                        };
                        fact_roots.extend(program.tables.proof_fact_expression_roots(fact.fact));
                    }
                }
            }
            for root in fact_roots {
                let mut subtree = Vec::new();
                validation::expression_traversal::collect_expression_tree(
                    program,
                    root,
                    &mut subtree,
                );
                let subtree_exposure = subtree
                    .iter()
                    .flat_map(|expression| {
                        program.expression_table.authored_selection_occurrences(*expression)
                    })
                    .find_map(|occurrence| {
                        program
                            .authored_declaration_selections()
                            .get(occurrence)
                            .map(|selection| selection.exposure())
                    })
                    .unwrap_or(
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    );
                for expression in subtree {
                    let typed_trees::expression::ExpressionNode::Call(call) =
                        program.expression_table.expression(expression)
                    else {
                        continue;
                    };
                    if call.target_symbol.is_valid() || call.receiver.is_valid() {
                        continue;
                    }
                    let occurrences = expressions(program)
                        .authored_selection_occurrences(expression)
                        .collect::<Vec<_>>();
                    for occurrence in occurrences.iter().copied() {
                        if resolutions
                            .iter()
                            .any(|resolution| resolution.occurrence == occurrence)
                        {
                            continue;
                        }
                        let Some(selection) =
                            program.authored_declaration_selections().get(occurrence)
                        else {
                            return Err(Diagnostic::error(format!(
                                "proof view call retains unknown authored selection occurrence {}",
                                occurrence.ordinal(),
                            )));
                        };
                        if selection.kind() != SelectionKind::Call
                            || selection.target()
                                != SelectionTarget::LateBound(LateBinding::CheckedCall)
                        {
                            continue;
                        }
                        push_consistent_resolution(
                            program,
                            resolutions,
                            SelectionResolution {
                                occurrence,
                                binding: LateBinding::CheckedCall,
                                target: ResolutionTarget::Intrinsic(
                                    AuthoredDeclarationSelectionIntrinsic::ProofView,
                                ),
                            },
                        )?;
                    }
                    let has_call_occurrence = occurrences.iter().copied().any(|occurrence| {
                        program
                            .authored_declaration_selections()
                            .get(occurrence)
                            .is_some_and(|selection| selection.kind() == SelectionKind::Call)
                    });
                    if !has_call_occurrence {
                        unoccurred_view_calls.push((
                            program.expression_table.source_span(expression),
                            subtree_exposure,
                            expression,
                        ));
                    }
                }
            }
            Ok(())
        }

        pub(crate) fn collect_checked_statement_selections(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            resolutions: &mut Vec<SelectionResolution>,
        ) -> Result<(), Diagnostic> {
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for (statement_index, statement) in
                        indexed_statements(program, state.statement_nodes)
                    {
                        let Some(call) = statement.as_call() else {
                            continue;
                        };
                        if call.operational_acknowledgement.origin
                            != language_semantics::CallOperationalAcknowledgementOrigin::Source
                            || call.source_span.span.start >= call.source_span.span.end
                        {
                            continue;
                        }
                        let target = implementation_call_selection(program, call.receiver_symbol)
                            .unwrap_or_else(|| {
                                checked_statement_call_target(
                                    program,
                                    facts,
                                    machine.symbol,
                                    state.symbol,
                                    statement_index,
                                    call.target_symbol,
                                )
                            });
                        let Some(occurrence) = call.authored_call_selection else {
                            return Err(Diagnostic::error(
                                "source-authored checked statement call has no attached call selection",
                            )
                            .with_source_span(call.source_span));
                        };
                        let Some(selection) =
                            program.authored_declaration_selections().get(occurrence)
                        else {
                            return Err(Diagnostic::error(format!(
                                "statement call retains unknown authored selection occurrence {}",
                                occurrence.ordinal(),
                            ))
                            .with_source_span(call.source_span));
                        };
                        if selection.kind() != SelectionKind::Call
                            || selection.exposure()
                                != language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
                            || selection.source_span() != call.source_span
                        {
                            return Err(Diagnostic::error(
                                "statement call retains mismatched authored call-selection evidence",
                            )
                            .with_source_span(call.source_span));
                        }
                        let resolution_target = classify_statement_call_intrinsic(program, call)
                            .or_else(|| statement_wire_codec_intrinsic(program, call))
                            .map(ResolutionTarget::Intrinsic)
                            .or_else(|| {
                                crate::flow::resolved_operator_statement_symbol(program, call)
                                    .and_then(declaration_target)
                            })
                            .or_else(|| declaration_target(target));
                        if selection.target()
                            == SelectionTarget::LateBound(LateBinding::CheckedCall)
                            && let Some(target) = resolution_target
                        {
                            push_consistent_resolution(
                                program,
                                resolutions,
                                SelectionResolution {
                                    occurrence,
                                    binding: LateBinding::CheckedCall,
                                    target,
                                },
                            )?;
                        }
                    }
                }
            }
            collect_checked_transition_target_selections(program, resolutions)?;
            Ok(())
        }
    }
    mod static_inputs {
        /*
            Finalize source-issued static input occurrences after independent ordinary
            formation checks. Product root descriptors grant no authority here.
        */
        use crate::flow::expressions;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionKind as Kind,
            AuthoredDeclarationSelectionLateBinding as Binding,
            AuthoredDeclarationSelectionTarget as Target,
        };
        use validation::selection_resolution::{
            AuthoredSelectionResolution, AuthoredSelectionResolutionTarget,
            push_consistent_resolution,
        };

        pub(super) fn collect(
            program: &dyn validation::CheckingInput,
            resolutions: &mut Vec<AuthoredSelectionResolution>,
        ) -> Result<(), Diagnostic> {
            let typed = program.typed_program();
            for (_, node) in typed.expression_table.iter_expressions() {
                if let typed_trees::expression::ExpressionNode::Call(call) = node
                    && let Some(request) = &call.installed_interrupt_operation
                {
                    collect_registrar(typed, request, resolutions)?;
                }
            }
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for statement in
                        crate::flow::state_statements(program, state.statement_nodes).iter()
                    {
                        if let Some(call) = statement.as_call()
                            && let Some(request) = &call.installed_interrupt_operation
                        {
                            collect_registrar(typed, request, resolutions)?;
                        }
                    }
                }
            }
            for handle in typed.live_product_operand_applications() {
                let application = typed.product_operand_applications.get(handle);
                collect_inputs(
                    typed,
                    application.input_selections,
                    application
                        .argument
                        .application
                        .as_ref()
                        .map_or(&[][..], |application| application.arguments.as_ref()),
                    resolutions,
                )?;
            }
            if !expressions(typed)
                .static_machine_applications()
                .any(|(_, application)| !application.input_selections.is_empty())
            {
                return Ok(());
            }
            #[derive(Default)]
            struct Applications {
                handles: Vec<typed_trees::static_applications::StaticMachineApplicationHandle>,
                expressions: Vec<Vec<typed_trees::expression::ExpressionHandle>>,
            }
            impl Extend<symbols::SymbolHandle> for Applications {
                fn extend<Symbols: IntoIterator<Item = symbols::SymbolHandle>>(
                    &mut self,
                    _: Symbols,
                ) {
                }
            }
            impl typed_trees::references::TypedReferences for Applications {
                fn static_application(
                    &mut self,
                    handle: typed_trees::static_applications::StaticMachineApplicationHandle,
                ) {
                    if !self.handles.contains(&handle) {
                        self.handles.push(handle);
                    }
                }
                fn expression(
                    &mut self,
                    handle: typed_trees::expression::ExpressionHandle,
                ) -> bool {
                    let index = handle.arena_index() as usize;
                    if self.expressions.len() <= index {
                        self.expressions.resize_with(index + 1, Vec::new);
                    }
                    if self.expressions[index].contains(&handle) {
                        return false;
                    }
                    self.expressions[index].push(handle);
                    true
                }
            }
            let mut applications = Applications::default();
            typed_trees::references::visit_program(typed, &mut applications);
            for handle in applications.handles {
                let application = typed.expression_table.static_machine_application(handle);
                collect_inputs(
                    typed,
                    application.input_selections,
                    expressions(typed).static_machine_application_arguments(handle),
                    resolutions,
                )?;
            }
            Ok(())
        }

        fn collect_inputs(
            typed: &typed_trees::TypedTrees,
            inputs: arena::HandleSpan<typed_trees::product_operand::ProductInputSelection>,
            arguments: &[typed_trees::expression::StaticMachineArgument],
            resolutions: &mut Vec<AuthoredSelectionResolution>,
        ) -> Result<(), Diagnostic> {
            for retained in typed.product_input_selections.span_or_empty(inputs) {
                let occurrence = retained.occurrence.ok_or_else(|| {
                    Diagnostic::error("static input retains an absent selection occurrence")
                })?;
                let selection = typed
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "Product input retains an unknown authored selection occurrence",
                        )
                    })?;
                if selection.compiler_partition().is_some() {
                    return Err(Diagnostic::error(
                        "Product input source copy requires its exact semantic owner",
                    )
                    .with_source_span(selection.source_span()));
                }
                if !matches!(
                    selection.kind(),
                    Kind::StaticArgument
                        | Kind::Conformance
                        | Kind::TypeReference
                        | Kind::StaticPathSegment
                ) {
                    return Err(Diagnostic::error(
                        "Product input retains a non-input declaration selection",
                    )
                    .with_source_span(selection.source_span()));
                }
                let selected = retained
                    .selected_symbol(typed, arguments)
                    .map_err(|reason| {
                        Diagnostic::error(reason).with_source_span(selection.source_span())
                    })?;
                match selection.target() {
                    Target::LateBound(
                        binding @ (Binding::CheckedStaticArgument
                        | Binding::CheckedStaticPathSegment
                        | Binding::CheckedConformance),
                    ) => {
                        push_consistent_resolution(
                            typed,
                            resolutions,
                            AuthoredSelectionResolution {
                                occurrence,
                                binding,
                                target: AuthoredSelectionResolutionTarget::Declaration(selected),
                            },
                        )?;
                    }
                    Target::Resolved(target)
                        if typed.symbols.authored_origin(target.selected_symbol()) == selected => {}
                    _ => {
                        return Err(Diagnostic::error(
                            "Product input disagrees with its original authored declaration selection",
                        )
                        .with_source_span(selection.source_span()));
                    }
                }
            }
            Ok(())
        }

        /*
            Record the requirement's identity, not permission to install its callback.
            The complete application and slot remain separate installation obligations.
        */
        fn collect_registrar(
            typed: &typed_trees::TypedTrees,
            request: &typed_trees::expression::InstalledInterruptOperationRequest,
            resolutions: &mut Vec<AuthoredSelectionResolution>,
        ) -> Result<(), Diagnostic> {
            let Some(occurrence) = request.registrar_selection else {
                return Ok(());
            };
            let selection = typed
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| {
                    Diagnostic::error("interrupt registrar retains an unknown authored selection")
                })?;
            let mut argument = &request.registrar;
            while let Some(application) = &argument.application
                && application.member.is_valid()
            {
                argument = typed
                    .expression_table
                    .static_member(application.member)
                    .ok_or_else(|| {
                        Diagnostic::error("interrupt registrar lost its retained member")
                    })?;
            }
            if selection.kind() != Kind::StaticArgument || !argument.symbol.is_valid() {
                return Err(Diagnostic::error(
                    "interrupt registrar has no exact requirement declaration",
                )
                .with_source_span(selection.source_span()));
            }
            let selected = typed.symbols.authored_origin(argument.symbol);
            match selection.target() {
                Target::LateBound(Binding::CheckedStaticArgument) => push_consistent_resolution(
                    typed,
                    resolutions,
                    AuthoredSelectionResolution {
                        occurrence,
                        binding: Binding::CheckedStaticArgument,
                        target: AuthoredSelectionResolutionTarget::Declaration(selected),
                    },
                ),
                Target::Resolved(target)
                    if typed.symbols.authored_origin(target.selected_symbol()) == selected =>
                {
                    Ok(())
                }
                _ => Err(Diagnostic::error(
                    "interrupt registrar disagrees with its authored requirement selection",
                )
                .with_source_span(selection.source_span())),
            }
        }
    }

    pub(crate) use contract_resolution::{
        CheckedContractOperatorResolution, checked_named_operator_call, checked_operator_resolution,
    };
    pub(crate) use operator_targets::{
        typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
    };

    pub(crate) use crate::shared::authored_selections::finalization::finalize_checked_authored_selections;
    use checked_trees::CheckFacts;
    use symbols::SymbolHandle;

    pub(crate) fn derive_checked_nominal_call_target(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        expression: typed_trees::expression::ExpressionHandle,
        owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
    ) -> Option<SymbolHandle> {
        let call = program.expression_table.call_node(expression)?;
        validation::expression_owners::checked_machine_call_target_from_exact_owner(
            program,
            &facts.proof.contract_facts,
            expression,
            call,
            owner_index,
        )
    }

    /*
        Exact declaration for one late-bound `CheckedMember` occurrence, derived
        from the receiver's owner type in every checked environment that contains
        the expression, before checked binding has run. Case-payload projections
        synthesized from destructure patterns stay unresolved through typing; this
        is the same owner-type resolution checked binding applies to them. `None`
        means no exact owner is derivable, never that the spelling is free.
    */
    pub(crate) fn exact_owner_member_declaration(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let member = program.expression_table.member_node(expression)?;
        if member.member_symbol.is_valid() {
            return Some(member.member_symbol);
        }
        match validation::expression_owners::checked_member_target_from_exact_owner(
            program,
            &arena::Arena::default(),
            expression,
            member,
            None,
        )? {
            validation::expression_owners::OwnerMemberTarget::Declaration(symbol) => Some(symbol),
            validation::expression_owners::OwnerMemberTarget::CollectionMeasure(_) => None,
        }
    }
}
pub(crate) mod call_formals {
    /*
        Align a call's authored operands with the formals those operands answer.

        A selector call's first operand is the selector. Dispatch consumes it, and
        the requirement's formals — the only names a published crash route or
        contract predicate may use — begin at the next operand. A list that already
        has one operand per formal is left unchanged, including a slot roster whose
        own parameter 0 is the selector.
    */
    use symbols::SymbolHandle;

    pub(crate) fn arguments_aligned_to_formals<'a, T>(
        program: &dyn ::validation::CheckingInput,
        call_target: SymbolHandle,
        formal_count: usize,
        arguments: &'a [T],
    ) -> &'a [T] {
        let leading = usize::from(program.dynamic_slot_by_symbol(call_target).is_some());
        arguments
            .get(leading..)
            .filter(|aligned| aligned.len() == formal_count)
            .unwrap_or(arguments)
    }
}
pub(crate) mod collections {
    /*
        Ordered rosters: one owner for first-occurrence accumulation.

        Checked-tree builders accumulate ordered rosters — dependencies, origins,
        identities, dispositions, claims. The policy is order-preserving
        uniqueness: the first occurrence of a value fixes its position and every
        later occurrence is dropped, so a roster reads in the order the producing
        walk met its entries.

        Keeping that policy here lets a producer state what it accumulates instead
        of restating how uniqueness is decided. The per-site rescan it replaces
        (`if !roster.contains(&entry) { roster.push(entry); }`, its `fold`
        spelling, and the one-off `push_unique_*` wrappers) let one accumulator
        drift into a different rule — a `clone` on the insert path, a skipped
        validity check, a set that reordered the roster — without the rosters that
        consume it noticing.
    */
    use std::collections::HashMap;
    use std::hash::{DefaultHasher, Hash, Hasher};

    /*
        Append `value` unless an equal value is already present, keeping the
        first occurrence's position.
    */
    pub(crate) fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
        if !values.contains(&value) {
            values.push(value);
        }
    }

    /* Collect the first occurrence of each value, in iteration order. */
    pub(crate) fn unique_in_order<T: PartialEq>(values: impl IntoIterator<Item = T>) -> Vec<T> {
        let mut unique = Vec::new();
        for value in values {
            push_unique(&mut unique, value);
        }
        unique
    }

    /*
        The same first-occurrence roster for one that grows past a few dozen
        entries, where rescanning for each append makes accumulation quadratic.
        Equality still decides uniqueness; the hash only chooses which earlier
        entries to compare, and no value is cloned.
    */
    pub(crate) struct UniqueRoster<T> {
        values: Vec<T>,
        positions_by_hash: HashMap<u64, Vec<usize>>,
    }

    impl<T> Default for UniqueRoster<T> {
        fn default() -> Self {
            Self {
                values: Vec::new(),
                positions_by_hash: HashMap::new(),
            }
        }
    }

    impl<T: Hash + Eq> UniqueRoster<T> {
        pub(crate) fn push(&mut self, value: T) {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            let positions = self.positions_by_hash.entry(hasher.finish()).or_default();
            if positions
                .iter()
                .any(|position| self.values[*position] == value)
            {
                return;
            }
            positions.push(self.values.len());
            self.values.push(value);
        }

        pub(crate) fn as_slice(&self) -> &[T] {
            &self.values
        }

        pub(crate) fn into_vec(self) -> Vec<T> {
            self.values
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{UniqueRoster, push_unique, unique_in_order};
        use std::hash::{Hash, Hasher};

        #[test]
        fn push_unique_keeps_first_occurrence_order() {
            let mut values = vec![3, 1];
            push_unique(&mut values, 1);
            push_unique(&mut values, 7);
            push_unique(&mut values, 3);
            assert_eq!(values, vec![3, 1, 7]);
        }

        #[test]
        fn unique_in_order_drops_repeats() {
            assert_eq!(unique_in_order([2, 1, 2, 2, 3, 1]), vec![2, 1, 3]);
            assert!(unique_in_order(Vec::<u8>::new()).is_empty());
        }

        /* Every value hashes alike, so uniqueness must come from equality. */
        #[derive(Debug, PartialEq, Eq)]
        struct Colliding(u32);

        impl Hash for Colliding {
            fn hash<H: Hasher>(&self, state: &mut H) {
                0u8.hash(state);
            }
        }

        #[test]
        fn unique_roster_matches_push_unique_for_colliding_hashes() {
            let input = [4, 2, 4, 9, 2, 7, 9, 4];
            let mut roster = UniqueRoster::default();
            let mut expected = Vec::new();
            for value in input {
                roster.push(Colliding(value));
                push_unique(&mut expected, Colliding(value));
            }
            assert_eq!(roster.as_slice(), expected.as_slice());
            assert_eq!(roster.into_vec(), expected);
        }
    }
}
pub(crate) mod machine_waves {
    /*
        Machine call-cycle components and the waves that check them in parallel.

        A whole-program fixed point over machine summaries (termination, mutation,
        crash and reach summaries) only needs a machine's callees to be settled
        before the machine itself is derived. Machines that call each other, directly
        or around a longer cycle, form one *component*; a component's fixed point
        needs nothing but its own members and components it calls. Ordering the
        components so every callee comes first groups them into *waves*: every
        component of a wave calls only earlier waves, so the components of one wave
        are independent jobs.

        The caller supplies the read graph: for each machine (by roster position)
        the machines whose results its step reads. An edge that a step never reads
        only costs parallelism; a read without an edge would let the step see an
        unsettled value, so each phase derives its edges from the same lookups its
        step performs.

        Determinism: a step sees settled results of earlier waves and the initial
        values of everything else, never a result from its own wave, and its result
        is written back by roster position. The output therefore does not depend on
        the thread count or on which worker finishes first.
    */
    use rayon::prelude::*;

    pub(crate) struct MachineCallComponents {
        /* Each component's members in ascending roster order. */
        members: Vec<Vec<usize>>,
        /* Component indices per wave, callees' waves first. */
        callee_first_waves: Vec<Vec<usize>>,
        /* Component indices per wave, callers' waves first. */
        caller_first_waves: Vec<Vec<usize>>,
    }

    impl MachineCallComponents {
        /*
            `reads[machine]` lists the roster positions whose results the step for
            `machine` reads. Self edges and duplicates are allowed.
        */
        pub(crate) fn new(reads: &[Vec<usize>]) -> Self {
            let (component_of, count) = strongly_connected_components(reads);
            let mut members = vec![Vec::new(); count];
            for (machine, &component) in component_of.iter().enumerate() {
                members[component].push(machine);
            }
            let mut callee_level = vec![0usize; count];
            for component in 0..count {
                let mut level = 0;
                for &machine in &members[component] {
                    for &callee in &reads[machine] {
                        let callee = component_of[callee];
                        if callee != component {
                            level = level.max(callee_level[callee] + 1);
                        }
                    }
                }
                callee_level[component] = level;
            }
            let mut caller_level = vec![0usize; count];
            for component in (0..count).rev() {
                for &machine in &members[component] {
                    for &callee in &reads[machine] {
                        let callee = component_of[callee];
                        if callee != component {
                            caller_level[callee] =
                                caller_level[callee].max(caller_level[component] + 1);
                        }
                    }
                }
            }
            Self {
                members,
                callee_first_waves: waves(&callee_level),
                caller_first_waves: waves(&caller_level),
            }
        }

        /*
            Run `step` once per component, callees' components first. `step`
            receives the component's members (ascending roster order) and the
            results so far, and returns one result per member in the same order.
        */
        pub(crate) fn run_callee_first<T, F>(&self, results: &mut [T], step: F)
        where
            T: Send + Sync,
            F: Fn(&[usize], &[T]) -> Vec<T> + Sync,
        {
            self.run_waves(&self.callee_first_waves, results, step);
        }

        /*
            As `run_callee_first`, with callers' components first: for facts that
            flow from a caller into its private callee's entry. No phase uses this
            order yet; the flow inputs and private-entry passes are its consumers.
        */
        #[allow(dead_code)]
        pub(crate) fn run_caller_first<T, F>(&self, results: &mut [T], step: F)
        where
            T: Send + Sync,
            F: Fn(&[usize], &[T]) -> Vec<T> + Sync,
        {
            self.run_waves(&self.caller_first_waves, results, step);
        }

        fn run_waves<T, F>(&self, waves: &[Vec<usize>], results: &mut [T], step: F)
        where
            T: Send + Sync,
            F: Fn(&[usize], &[T]) -> Vec<T> + Sync,
        {
            for wave in waves {
                let settled: &[T] = results;
                let derived: Vec<Vec<T>> = if wave.len() == 1 {
                    vec![step(&self.members[wave[0]], settled)]
                } else {
                    validation::checking_thread_pool().install(|| {
                        wave.par_iter()
                            .map(|&component| step(&self.members[component], settled))
                            .collect()
                    })
                };
                for (&component, values) in wave.iter().zip(derived) {
                    let members = &self.members[component];
                    assert_eq!(
                        members.len(),
                        values.len(),
                        "a component step returns one result per member"
                    );
                    for (&machine, value) in members.iter().zip(values) {
                        results[machine] = value;
                    }
                }
            }
        }
    }

    fn waves(levels: &[usize]) -> Vec<Vec<usize>> {
        let depth = levels.iter().copied().max().map_or(0, |level| level + 1);
        let mut waves = vec![Vec::new(); depth];
        for (component, &level) in levels.iter().enumerate() {
            waves[level].push(component);
        }
        waves
    }

    /* Iterative Tarjan. A component's successors receive lower numbers. */
    fn strongly_connected_components(next: &[Vec<usize>]) -> (Vec<usize>, usize) {
        const UNVISITED: usize = usize::MAX;
        let count = next.len();
        let mut index = vec![UNVISITED; count];
        let mut low = vec![0; count];
        let mut on_stack = vec![false; count];
        let mut stack = Vec::new();
        let mut component = vec![UNVISITED; count];
        let (mut next_index, mut components) = (0usize, 0usize);
        let mut work: Vec<(usize, usize)> = Vec::new();
        for start in 0..count {
            if index[start] != UNVISITED {
                continue;
            }
            work.push((start, 0));
            while let Some(&mut (node, ref mut edge)) = work.last_mut() {
                if *edge == 0 && index[node] == UNVISITED {
                    index[node] = next_index;
                    low[node] = next_index;
                    next_index += 1;
                    stack.push(node);
                    on_stack[node] = true;
                }
                if let Some(&to) = next[node].get(*edge) {
                    *edge += 1;
                    if index[to] == UNVISITED {
                        work.push((to, 0));
                    } else if on_stack[to] {
                        low[node] = low[node].min(index[to]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[node]);
                }
                if low[node] == index[node] {
                    while let Some(member) = stack.pop() {
                        on_stack[member] = false;
                        component[member] = components;
                        if member == node {
                            break;
                        }
                    }
                    components += 1;
                }
            }
        }
        (component, components)
    }

    #[cfg(test)]
    mod tests {
        use super::MachineCallComponents;

        #[test]
        fn cycles_share_a_component_and_callees_run_first() {
            let reads = vec![vec![1], vec![2], vec![1, 3], vec![], vec![4]];
            let components = MachineCallComponents::new(&reads);
            assert_eq!(components.members.len(), 4);
            assert_eq!(components.callee_first_waves.len(), 3);
            let mut order = vec![usize::MAX; 5];
            let counter = std::sync::atomic::AtomicUsize::new(0);
            let mut seen = vec![0usize; 5];
            components.run_callee_first(&mut seen, |members, settled| {
                for &member in members {
                    for &callee in &reads[member] {
                        if !members.contains(&callee) {
                            assert_ne!(settled[callee], 0, "callee settled before caller");
                        }
                    }
                }
                members.iter().map(|_| 1).collect()
            });
            components.run_callee_first(&mut order, |members, _| {
                let at = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                members.iter().map(|_| at).collect()
            });
            assert_eq!(order[1], order[2]);
            assert!(order[3] < order[1] && order[1] < order[0]);

            let mut caller_order = vec![usize::MAX; 5];
            counter.store(0, std::sync::atomic::Ordering::SeqCst);
            components.run_caller_first(&mut caller_order, |members, _| {
                let at = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                members.iter().map(|_| at).collect()
            });
            assert!(caller_order[0] < caller_order[1] && caller_order[1] < caller_order[3]);
        }
    }
}
pub(crate) mod operators {
    /*
        Checked boundary operator applications and candidate evidence.

        Shared declaration/selection queries belong to semantic validation.
        This stage retains proof-backed applications and checked body candidates.
    */
    mod applications {
        use crate::flow::expressions;
        use checked_trees::{
            CheckedBoundaryOperatorApplicationArgument, CheckedBoundaryOperatorApplicationDemand,
            CheckedBoundaryOperatorApplicationUseSite, CheckedOperatorFacts,
            CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
            CheckedSymbolicBoundaryOperatorApplicationArgument,
            CheckedSymbolicBoundaryOperatorApplicationDemand, CheckedValueOrigin,
        };
        use typed_trees::expression::ExpressionNode;
        use typed_trees::operator::ClosedOperatorApplicationArgument;
        use typed_trees::types::TypeReferenceHandle;

        use validation::{expression_type_reference_for_origin, indexed_operand_types};

        /*
            The requirement-slot identity Omega 00 binds on one boundary application:
            the canonical operator overload identity, or the normalized machine
            overload when the operator is a token view of a requirement machine.
        */
        fn boundary_application_requirement_identity(
            program: &dyn ::validation::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
        ) -> typed_trees::type_identity::CallableIdentityId {
            program
                .typed_program()
                .boundary_operator_requirement_callable_identity(operator)
        }

        /* The exact callable identity for a direct requirement use. */
        fn requirement_machine_identity(
            program: &dyn ::validation::CheckingInput,
            requirement_symbol: symbols::SymbolHandle,
        ) -> typed_trees::type_identity::CallableIdentityId {
            program
                .typed_program()
                .machine_by_symbol(requirement_symbol)
                .map(|requirement| {
                    program
                        .typed_program()
                        .machine_callable_identity(requirement)
                })
                .unwrap_or_default()
        }

        pub(crate) fn bind_boundary_operator_application_demands(
            program: &dyn ::validation::CheckingInput,
            validated: &[validation::ValidatedBoundaryOperatorApplication],
            operators: &mut CheckedOperatorFacts,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            let mut diagnostics = Vec::new();
            let mut applications = Vec::new();
            let mut symbolic_applications = Vec::new();
            for (_, operator_use) in operators.uses.iter() {
                if operator_use.status != CheckedOperatorResolutionStatus::Resolved {
                    continue;
                }
                let Some(operator) = typed_trees::operator::declaration_by_symbol(
                    program,
                    operator_use.selected_operator_symbol,
                )
                .filter(|operator| operator.is_boundary) else {
                    continue;
                };
                let operand_types = spelled_operand_types(program, operator_use);
                match checked_spelled_boundary_application(
                    program,
                    operator,
                    operator_use,
                    &operand_types,
                ) {
                    Ok(Some(application)) => applications.push(application),
                    Ok(None) => {
                        if let Some(application) = spelled_symbolic_boundary_application(
                            program,
                            operator,
                            operator_use,
                            &operand_types,
                        ) {
                            symbolic_applications.push(application);
                        }
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }

            for application in validated {
                let Some(operator) =
                    typed_trees::operator::declaration_by_symbol(program, application.requirement)
                else {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "validated boundary application names no operator declaration",
                    ));
                    continue;
                };
                if !operator.is_boundary {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "validated boundary application names a non-boundary operator",
                    ));
                    continue;
                }
                match application.site {
                    validation::ValidatedBoundaryOperatorApplicationUseSite::Expression(
                        expression,
                    ) => {
                        let Some(call) = program.expression_table.call_node(expression) else {
                            diagnostics.push(diagnostics::Diagnostic::error(
                                "validated named operator application no longer names a call expression",
                            ));
                            continue;
                        };
                        if typed_trees::operator::resolve_named_expression_call(program, call)
                            .is_none_or(|selected| selected.symbol != application.requirement)
                        {
                            diagnostics.push(diagnostics::Diagnostic::error(
                                "validated named operator application does not rejoin its selected requirement",
                            ));
                            continue;
                        }
                        let matching_uses = operators
                            .named_uses
                            .iter()
                            .filter_map(|(_, operator_use)| {
                                (operator_use.expression == expression
                                    && operator_use.selected_operator_symbol
                                        == application.requirement)
                                    .then_some(operator_use)
                            })
                            .collect::<Vec<_>>();
                        if matching_uses.is_empty() {
                            let source_span = program.expression_table.source_span(expression);
                            let message = if program.symbols.source_reference_may_select_symbol(
                                source_span,
                                application.requirement,
                            ) {
                                format!(
                                    "the compiler lost the checked use of named operator `{}`",
                                    program.operator_path_string(operator.name),
                                )
                            } else {
                                format!(
                                    "named operator `{}` is not available in this source's import scope",
                                    program.operator_path_string(operator.name),
                                )
                            };
                            diagnostics.push(
                                diagnostics::Diagnostic::error(message)
                                    .with_source_span(source_span),
                            );
                            continue;
                        }
                        for operator_use in matching_uses {
                            let operand_types =
                                named_expression_operand_types(program, call, operator_use.origin);
                            if application.arguments.iter().any(|argument| {
                                matches!(
                                    argument,
                                    validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder { .. }
                                )
                            }) {
                                if let Some((machine_symbol, arguments)) =
                                    rejoin_validated_symbolic_arguments(
                                        program,
                                        operator,
                                        &application.arguments,
                                        &operand_types,
                                        operator_use.origin,
                                        &mut diagnostics,
                                    )
                                {
                                    symbolic_applications.push(
                                        CheckedSymbolicBoundaryOperatorApplicationDemand {
                                            site: CheckedBoundaryOperatorApplicationUseSite::Expression {
                                                expression,
                                                origin: operator_use.origin,
                                            },
                                            requirement_symbol: operator.symbol,
                                            machine_symbol,
                                            arguments,
                                        },
                                    );
                                }
                                continue;
                            }
                            if let Some(arguments) = rejoin_validated_arguments(
                                program,
                                operator,
                                &application.arguments,
                                Some(&operand_types),
                                &mut diagnostics,
                            ) {
                                applications.push(CheckedBoundaryOperatorApplicationDemand {
                                    site: CheckedBoundaryOperatorApplicationUseSite::Expression {
                                        expression,
                                        origin: operator_use.origin,
                                    },
                                    requirement_symbol: operator.symbol,
                                    requirement_identity: boundary_application_requirement_identity(
                                        program, operator,
                                    ),
                                    arguments,
                                    body_candidates: Default::default(),
                                });
                            }
                        }
                    }
                    validation::ValidatedBoundaryOperatorApplicationUseSite::Statement(
                        statement,
                    ) => {
                        let Some(call) = program.statement_table.statement(statement).as_call()
                        else {
                            diagnostics.push(diagnostics::Diagnostic::error(
                                "validated named operator application no longer names a call statement",
                            ));
                            continue;
                        };
                        if call.target_symbol != application.requirement {
                            diagnostics.push(diagnostics::Diagnostic::error(
                                "validated statement operator application does not rejoin its selected requirement",
                            ));
                            continue;
                        }
                        if application.arguments.iter().any(|argument| {
                            matches!(
                                argument,
                                validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder { .. }
                            )
                        }) {
                            diagnostics.push(diagnostics::Diagnostic::error(
                                "symbolic statement boundary applications are not yet supported",
                            ));
                            continue;
                        }
                        if let Some(arguments) = rejoin_validated_arguments(
                            program,
                            operator,
                            &application.arguments,
                            None,
                            &mut diagnostics,
                        ) {
                            applications.push(CheckedBoundaryOperatorApplicationDemand {
                                site: CheckedBoundaryOperatorApplicationUseSite::Statement(
                                    statement,
                                ),
                                requirement_symbol: operator.symbol,
                                requirement_identity: boundary_application_requirement_identity(
                                    program, operator,
                                ),
                                arguments,
                                body_candidates: Default::default(),
                            });
                        }
                    }
                }
            }
            for (_, requirement_use) in operators.named_requirement_uses.iter() {
                let ExpressionNode::Call(call) =
                    expressions(program).expression(requirement_use.expression)
                else {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "retained direct requirement use no longer names a call expression",
                    ));
                    continue;
                };
                let trait_rejoins = program
                    .trait_machine_signature(requirement_use.requirement_symbol)
                    .is_some_and(|(owner, signature)| {
                        owner.is_boundary
                            && signature.symbol == call.target_symbol
                            && program.trait_type_parameters(owner).is_empty()
                            && signature.type_parameters.is_empty()
                            && (!call.receiver.is_valid()
                                || matches!(program.expression_table.expression(call.receiver),
                                    ExpressionNode::Name(path) if path.symbol == owner.symbol))
                    });
                let rejoins = trait_rejoins
                    || program
                        .machine_by_symbol(requirement_use.requirement_symbol)
                        .is_some_and(|requirement| {
                            program
                                .machine_states(requirement)
                                .first()
                                .is_some_and(|entry| entry.symbol == call.target_symbol)
                        });
                if !rejoins {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "retained direct requirement use does not rejoin its exact callable declaration",
                    ));
                    continue;
                }
                if trait_rejoins {
                    continue;
                }
                applications.push(CheckedBoundaryOperatorApplicationDemand {
                    site: CheckedBoundaryOperatorApplicationUseSite::Expression {
                        expression: requirement_use.expression,
                        origin: requirement_use.origin,
                    },
                    requirement_symbol: requirement_use.requirement_symbol,
                    requirement_identity: requirement_machine_identity(
                        program,
                        requirement_use.requirement_symbol,
                    ),
                    arguments: Vec::new(),
                    body_candidates: Default::default(),
                });
            }
            if !diagnostics.is_empty() {
                return Err(diagnostics);
            }
            operators.boundary_applications = applications;
            operators.symbolic_boundary_applications = symbolic_applications;
            Ok(())
        }

        fn checked_spelled_boundary_application(
            program: &dyn ::validation::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            operator_use: &CheckedOperatorUseFact,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Result<Option<CheckedBoundaryOperatorApplicationDemand>, diagnostics::Diagnostic>
        {
            let bindings = if matches!(
                program.expression_table.expression(operator_use.expression),
                ExpressionNode::Indexed(_)
            ) {
                typed_trees::operator::closed_indexed_operator_application_for_operands(
                    program,
                    operator,
                    operand_types,
                )
            } else {
                typed_trees::operator::closed_operator_application_for_operands(
                    program,
                    operator,
                    operand_types,
                )
            };
            let Some(bindings) = bindings else {
                return Ok(None);
            };
            validation::validate_closed_operator_application(program, operator, &bindings)?;
            Ok(Some(checked_boundary_application_from_bindings(
                program,
                operator,
                operator_use.application_site(),
                bindings,
            )))
        }

        /*
            A resolved spelled use inside a generic machine cannot close its
            operator application until the machine's binders are substituted, but it
            still exports an open demand when every operator type binder maps
            directly onto one enclosing-machine type binder — the same symbolic
            cohort as named open applications. The row is demand only: it preserves
            the producer's transitive open obligation and authorizes no coverage
            until a consumer's final substitution closes the application. Implicit
            match-equality comparisons carry no authored operator token of their
            own, so they stay out of this cohort for now.
        */
        fn spelled_symbolic_boundary_application(
            program: &dyn ::validation::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            operator_use: &CheckedOperatorUseFact,
            operand_types: &[Option<TypeReferenceHandle>],
        ) -> Option<CheckedSymbolicBoundaryOperatorApplicationDemand> {
            if operator_use.occurrence != checked_trees::CheckedOperatorOccurrence::Expression {
                return None;
            }
            let machine_symbol = operator_use.origin.machine_symbol()?;
            let machine = program.machine_by_symbol(machine_symbol)?;
            let arguments = typed_trees::operator::symbolic_operator_type_application_for_operands(
                program,
                machine,
                operator,
                operand_types,
            )?;
            let arguments = arguments
                .into_iter()
                .enumerate()
                .map(|(ordinal, argument)| {
                    u32::try_from(ordinal).ok().map(|binder_ordinal| {
                        CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder {
                            binder_owner: operator.symbol,
                            binder_ordinal,
                            binder_symbol: argument.operator_binder_symbol,
                            machine_binder_ordinal: argument.machine_binder_ordinal,
                            machine_binder_symbol: argument.machine_binder_symbol,
                        }
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(CheckedSymbolicBoundaryOperatorApplicationDemand {
                site: operator_use.application_site(),
                requirement_symbol: operator.symbol,
                machine_symbol,
                arguments,
            })
        }

        fn checked_boundary_application_from_bindings(
            program: &dyn ::validation::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            site: CheckedBoundaryOperatorApplicationUseSite,
            bindings: Vec<ClosedOperatorApplicationArgument>,
        ) -> CheckedBoundaryOperatorApplicationDemand {
            let arguments = bindings
                .into_iter()
                .enumerate()
                .map(|(ordinal, argument)| {
                    let binder_ordinal =
                        u32::try_from(ordinal).expect("operator static telescope ordinal overflow");
                    match argument {
                        ClosedOperatorApplicationArgument::Type {
                            binder_symbol,
                            type_reference,
                        } => CheckedBoundaryOperatorApplicationArgument::Type {
                            binder_owner: operator.symbol,
                            binder_ordinal,
                            binder_symbol,
                            type_reference,
                        },
                        ClosedOperatorApplicationArgument::Const {
                            binder_symbol,
                            declared_carrier,
                            value,
                        } => CheckedBoundaryOperatorApplicationArgument::Const {
                            binder_owner: operator.symbol,
                            binder_ordinal,
                            binder_symbol,
                            declared_carrier,
                            value,
                        },
                    }
                })
                .collect();
            CheckedBoundaryOperatorApplicationDemand {
                site,
                requirement_symbol: operator.symbol,
                requirement_identity: boundary_application_requirement_identity(program, operator),
                arguments,
                body_candidates: Default::default(),
            }
        }

        fn rejoin_validated_arguments(
            program: &dyn ::validation::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            validated: &[validation::ValidatedBoundaryOperatorApplicationArgument],
            operand_types: Option<&[Option<TypeReferenceHandle>]>,
            diagnostics: &mut Vec<diagnostics::Diagnostic>,
        ) -> Option<Vec<CheckedBoundaryOperatorApplicationArgument>> {
            let parameters = program.operator_type_parameters(operator);
            if !operator.lifetime_parameters.is_empty()
                || parameters.iter().any(|parameter| {
                    !matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Type
                            | typed_trees::data::TypeParameterKind::Const { .. }
                    )
                })
                || validated.len() != parameters.len()
            {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "validated boundary application does not match the supported selected operator telescope",
                ));
                return None;
            }
            let mut closed = Vec::with_capacity(validated.len());
            let mut checked = Vec::with_capacity(validated.len());
            for (ordinal, (argument, parameter)) in validated.iter().zip(parameters).enumerate() {
                let (
                    binder_owner,
                    binder_ordinal,
                    binder_symbol,
                    closed_argument,
                    checked_argument,
                ) = match argument {
                    validation::ValidatedBoundaryOperatorApplicationArgument::Type {
                        binder_owner,
                        binder_ordinal,
                        binder_symbol,
                        type_reference,
                    } => (
                        *binder_owner,
                        *binder_ordinal,
                        *binder_symbol,
                        ClosedOperatorApplicationArgument::Type {
                            binder_symbol: *binder_symbol,
                            type_reference: *type_reference,
                        },
                        CheckedBoundaryOperatorApplicationArgument::Type {
                            binder_owner: *binder_owner,
                            binder_ordinal: *binder_ordinal,
                            binder_symbol: *binder_symbol,
                            type_reference: *type_reference,
                        },
                    ),
                    validation::ValidatedBoundaryOperatorApplicationArgument::Const {
                        binder_owner,
                        binder_ordinal,
                        binder_symbol,
                        declared_carrier,
                        value,
                    } => (
                        *binder_owner,
                        *binder_ordinal,
                        *binder_symbol,
                        ClosedOperatorApplicationArgument::Const {
                            binder_symbol: *binder_symbol,
                            declared_carrier: *declared_carrier,
                            value: value.clone(),
                        },
                        CheckedBoundaryOperatorApplicationArgument::Const {
                            binder_owner: *binder_owner,
                            binder_ordinal: *binder_ordinal,
                            binder_symbol: *binder_symbol,
                            declared_carrier: *declared_carrier,
                            value: value.clone(),
                        },
                    ),
                    validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder {
                        ..
                    } => {
                        diagnostics.push(diagnostics::Diagnostic::error(
                            "symbolic boundary application entered closed-application replay",
                        ));
                        return None;
                    }
                };
                if binder_owner != operator.symbol
                    || usize::try_from(binder_ordinal).ok() != Some(ordinal)
                    || binder_symbol != parameter.symbol
                {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "validated boundary application does not rejoin the selected operator binder",
                    ));
                    return None;
                }
                closed.push(closed_argument);
                checked.push(checked_argument);
            }
            if let Err(diagnostic) =
                validation::validate_closed_operator_application(program, operator, &closed)
            {
                diagnostics.push(diagnostic);
                return None;
            }
            if let Some(operand_types) = operand_types {
                let Some(rederived) =
                    typed_trees::operator::closed_operator_application_for_operands(
                        program,
                        operator,
                        operand_types,
                    )
                else {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "validated boundary application no longer closes from the selected use operands",
                    ));
                    return None;
                };
                if rederived != closed {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "validated boundary application differs from the application reconstructed from the selected use",
                    ));
                    return None;
                }
            }
            Some(checked)
        }

        fn rejoin_validated_symbolic_arguments(
            program: &dyn ::validation::CheckingInput,
            operator: &typed_trees::operator::OperatorDefinition,
            validated: &[validation::ValidatedBoundaryOperatorApplicationArgument],
            operand_types: &[Option<TypeReferenceHandle>],
            origin: CheckedValueOrigin,
            diagnostics: &mut Vec<diagnostics::Diagnostic>,
        ) -> Option<(
            symbols::SymbolHandle,
            Vec<CheckedSymbolicBoundaryOperatorApplicationArgument>,
        )> {
            let parameters = program.operator_type_parameters(operator);
            if !operator.lifetime_parameters.is_empty()
                || parameters.is_empty()
                || parameters.iter().any(|parameter| {
                    !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
                || parameters.len() != validated.len()
            {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "symbolic boundary application does not match the supported type-only operator telescope",
                ));
                return None;
            }

            let machine_symbol = validated.first().and_then(|argument| match argument {
                validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder {
                    machine_owner,
                    ..
                } => Some(*machine_owner),
                _ => None,
            })?;
            let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "symbolic boundary application lost its enclosing generic machine",
                ));
                return None;
            };
            if origin.machine_symbol() != Some(machine_symbol) {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "symbolic boundary application use does not belong to its enclosing generic machine",
                ));
                return None;
            }
            let machine_parameters = program.machine_type_parameters(machine);
            let rederived = typed_trees::operator::symbolic_operator_type_application_for_operands(
                program,
                machine,
                operator,
                operand_types,
            );
            let Some(rederived) = rederived else {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "symbolic boundary application no longer reconstructs from its selected use",
                ));
                return None;
            };

            let mut checked = Vec::with_capacity(validated.len());
            for (ordinal, ((argument, parameter), expected)) in
                validated.iter().zip(parameters).zip(&rederived).enumerate()
            {
                let validation::ValidatedBoundaryOperatorApplicationArgument::TypeBinder {
                    binder_owner,
                    binder_ordinal,
                    binder_symbol,
                    machine_owner,
                    machine_binder_ordinal,
                    machine_binder_symbol,
                } = argument
                else {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "symbolic boundary application mixes closed and open arguments",
                    ));
                    return None;
                };
                let machine_parameter = usize::try_from(*machine_binder_ordinal)
                    .ok()
                    .and_then(|ordinal| machine_parameters.get(ordinal));
                if *binder_owner != operator.symbol
                    || usize::try_from(*binder_ordinal).ok() != Some(ordinal)
                    || *binder_symbol != parameter.symbol
                    || *machine_owner != machine_symbol
                    || machine_parameter.is_none_or(|candidate| {
                        candidate.symbol != *machine_binder_symbol
                            || !matches!(candidate.kind, typed_trees::data::TypeParameterKind::Type)
                    })
                    || expected.operator_binder_symbol != *binder_symbol
                    || expected.machine_binder_ordinal != *machine_binder_ordinal
                    || expected.machine_binder_symbol != *machine_binder_symbol
                {
                    diagnostics.push(diagnostics::Diagnostic::error(
                        "symbolic boundary application does not rejoin its operator and machine binders",
                    ));
                    return None;
                }
                checked.push(
                    CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder {
                        binder_owner: *binder_owner,
                        binder_ordinal: *binder_ordinal,
                        binder_symbol: *binder_symbol,
                        machine_binder_ordinal: *machine_binder_ordinal,
                        machine_binder_symbol: *machine_binder_symbol,
                    },
                );
            }
            Some((machine_symbol, checked))
        }

        fn named_expression_operand_types(
            program: &dyn ::validation::CheckingInput,
            call: &typed_trees::expression::TableCallExpression,
            origin: CheckedValueOrigin,
        ) -> Vec<Option<TypeReferenceHandle>> {
            let mut operand_types = Vec::new();
            if call.receiver.is_valid() {
                let receiver = expression_type_reference_for_origin(program, call.receiver, origin);
                if receiver.is_some() {
                    operand_types.push(receiver);
                }
            }
            operand_types.extend(
                expressions(program)
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| {
                        expression_type_reference_for_origin(program, *argument, origin).or_else(
                            || {
                                validation::landed_integer_literal_type_reference(
                                    program, *argument,
                                )
                            },
                        )
                    }),
            );
            operand_types
        }

        fn spelled_operand_types(
            program: &dyn ::validation::CheckingInput,
            operator_use: &CheckedOperatorUseFact,
        ) -> Vec<Option<TypeReferenceHandle>> {
            if operator_use.occurrence != checked_trees::CheckedOperatorOccurrence::Expression {
                return operator_use
                    .operands(program)
                    .unwrap_or_default()
                    .iter()
                    .map(|operand| {
                        expression_type_reference_for_origin(program, *operand, operator_use.origin)
                    })
                    .collect();
            }
            match program.expression_table.expression(operator_use.expression) {
                ExpressionNode::Binary(binary) => vec![
                    expression_type_reference_for_origin(program, binary.left, operator_use.origin),
                    expression_type_reference_for_origin(
                        program,
                        binary.right,
                        operator_use.origin,
                    ),
                ],
                ExpressionNode::Indexed(indexed) => {
                    indexed_operand_types(program, indexed, operator_use.origin)
                }
                _ => Vec::new(),
            }
        }
    }
    mod body_candidates {
        /*
            Complete boundary-body candidates while checking owns declarations and
            specialization judgments. Lowering publishes these facts without selecting
            providers or re-running conformance discovery.
        */
        use arena::{Arena, HandleSpan};
        use checked_trees::{
            CheckedBoundaryBodyCandidate as BodyCandidate,
            CheckedBoundaryOperatorApplicationDemand, CheckedBoundaryOperatorDeclaration,
            CheckedTrees,
        };
        use language_semantics::BoundaryBodySelection;
        use language_semantics::MachineSupplyMode;
        use typed_trees::operator::boundary_operator_requirement_identity;
        use typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance;

        pub(crate) fn retain_boundary_body_candidates(
            checked: &mut CheckedTrees,
        ) -> Result<(), &'static str> {
            checked.facts.operators.boundary_declarations = declarations(checked)?;
            let demands = &checked.facts.operators.boundary_applications;
            let mut bodies = Arena::new();
            let mut spans: Vec<HandleSpan<BodyCandidate>> = Vec::with_capacity(demands.len());
            let mut distinct = Vec::<usize>::new();
            for (ordinal, demand) in demands.iter().enumerate() {
                if let Some(previous) = distinct.iter().copied().find(|previous| {
                    demands[*previous].requirement_symbol == demand.requirement_symbol
                        && demands[*previous].arguments == demand.arguments
                }) {
                    spans.push(spans[previous]);
                    continue;
                }
                let candidates = candidates_for_application(checked, demand)?;
                spans.push(bodies.insert_many(candidates));
                distinct.push(ordinal);
            }
            for (demand, span) in checked
                .facts
                .operators
                .boundary_applications
                .iter_mut()
                .zip(spans)
            {
                demand.body_candidates = span;
            }
            checked.facts.operators.boundary_body_candidates = bodies;
            Ok(())
        }

        fn declarations(
            checked: &CheckedTrees,
        ) -> Result<Arena<CheckedBoundaryOperatorDeclaration>, &'static str> {
            let typed = &checked.typed;
            let mut declarations = Arena::new();
            let in_product = |symbol| {
                typed
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .and_then(|span| typed.symbols.source_file(span))
                    .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
            };
            for machine in typed.machines().iter().filter(|machine| {
                machine.supply_mode.is_checked_body()
                    && !machine.target_family_interface
                    && !typed
                        .machine_specializations
                        .iter()
                        .any(|row| row.instance == machine.symbol)
            }) {
                let conformances = typed.machine_trait_conformances(machine);
                let is_product = in_product(machine.symbol)
                    && (!machine.attached_data_symbol.is_valid()
                        || in_product(machine.attached_data_symbol))
                    && conformances
                        .iter()
                        .all(|edge| in_product(edge.symbol) && in_product(edge.requirement_symbol));
                for conformance in conformances
                    .iter()
                    .filter(|edge| edge.external_binding.is_none())
                {
                    let Some(operator) = resolve_satisfied_boundary_operator_for_conformance(
                        typed,
                        machine,
                        conformance,
                    ) else {
                        continue;
                    };
                    if typed.machine(operator.symbol).is_some() {
                        continue;
                    }
                    let callable = typed
                        .machine_callable_identity_text(machine)
                        .ok_or("checked operator declaration lost its callable identity")?;
                    let provider = typed.attached_data_path(machine).unwrap_or_default();
                    let provider_package = typed
                        .data_definition_by_symbol(machine.attached_data_symbol)
                        .filter(|owner| machine.attached_data.as_ref() == Some(&owner.name))
                        .and_then(|owner| typed.symbols.symbol_package_identity(owner.symbol));
                    declarations.append(CheckedBoundaryOperatorDeclaration {
                        requirement: operator.symbol,
                        machine: machine.symbol,
                        selection: BoundaryBodySelection {
                            requirement: boundary_operator_requirement_identity(typed, operator),
                            requirement_package: typed
                                .symbols
                                .symbol_package_identity(operator.symbol),
                            callable,
                            package: typed.symbols.symbol_package_identity(machine.symbol),
                            provider,
                            provider_package,
                            provider_application: None,
                            requirement_lifetime_partition: Vec::new(),
                            target: typed.source_target_context(machine).map(str::to_owned),
                            is_product,
                        },
                    });
                }
            }
            Ok(declarations)
        }

        fn candidates_for_application(
            checked: &CheckedTrees,
            demand: &CheckedBoundaryOperatorApplicationDemand,
        ) -> Result<Vec<BodyCandidate>, &'static str> {
            let typed = &checked.typed;
            let mut candidates = Vec::new();
            for machine in typed.machines().iter().filter(|machine| {
                machine.supply_mode == MachineSupplyMode::CheckedBody
                    && typed.machine_type_parameters(machine).is_empty()
                    && machine.lifetime_parameters.is_empty()
            }) {
                let specialization = typed
                    .machine_specializations
                    .iter()
                    .find(|row| row.instance == machine.symbol);
                let selection = if let Some(specialization) = specialization {
                    if !specialization.operator_realizations.iter().any(|row| {
                        row.requirement_symbol == demand.requirement_symbol
                            && checked_trees::operators::checked_operator_application_matches_realization(
                                typed, demand, row,
                            )
                    }) {
                        continue;
                    }
                    typed
                        .machine_by_symbol(specialization.template)
                        .ok_or("boundary candidate specialization lost its template")?
                } else {
                    if !demand.arguments.is_empty() {
                        continue;
                    }
                    machine
                };
                let mut declarations = checked.facts.operators.boundary_declarations.iter().filter(
                    |(_, declaration)| {
                        declaration.machine == selection.symbol
                            && declaration.requirement == demand.requirement_symbol
                    },
                );
                let Some((declaration, _)) = declarations.next() else {
                    continue;
                };
                if declarations.next().is_some() {
                    return Err("checked operator body has competing declaration relationships");
                }
                let entry = typed
                    .machine_states(machine)
                    .first()
                    .ok_or("boundary candidate has no entry")?;
                let contract = checked
                    .facts
                    .contract_plans
                    .machines()
                    .iter()
                    .find(|row| row.machine == machine.symbol)
                    .ok_or("boundary candidate has no checked contract")?;
                if contract.commitment.is_zero() {
                    return Err("boundary candidate has an empty checked contract");
                }
                let specialization = specialization
                    .map(|row| {
                        let identity = checked
                            .facts
                            .specialization_identities
                            .for_instance(row.instance)
                            .ok_or("boundary candidate has no checked specialization")?;
                        Ok::<_, &'static str>((row.template, identity.commitment.as_bytes()))
                    })
                    .transpose()?;
                candidates.push(BodyCandidate {
                    declaration,
                    is_product: typed
                        .symbols
                        .symbol_provenance_source_span(machine.symbol)
                        .and_then(|span| typed.symbols.source_file(span))
                        .is_none_or(|file| {
                            file.dependency_scope == source::DependencyScope::Product
                        })
                        && typed.symbols.symbol_package_identity(machine.symbol)
                            == typed.symbols.symbol_package_identity(selection.symbol),
                    selection: selection.symbol,
                    selection_identity: typed.machine_callable_identity(selection),
                    target: typed.source_target_context(machine).map(str::to_owned),
                    machine: machine.symbol,
                    entry: entry.symbol,
                    contract: contract.commitment.as_bytes(),
                    specialization,
                });
            }
            Ok(candidates)
        }
    }
    mod provider_requirements {
        /*
            Publish requirement and occurrence classification while Psi owns source.
            This inventory describes no selected provider and requires no implementation.
        */
        use checked_trees::{
            CheckedProviderRequirement, CheckedProviderRequirementKind, CheckedTrees,
        };
        use typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement;

        pub(crate) fn retain_provider_requirements(checked: &mut CheckedTrees) {
            let typed = &checked.typed;
            let in_product = |symbol| {
                typed
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .and_then(|span| typed.symbols.source_file(span))
                    .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
            };
            let mut requirements = arena::Arena::new();
            for operator in typed.operators() {
                requirements.append(CheckedProviderRequirement {
                    symbol: operator.symbol,
                    kind: CheckedProviderRequirementKind::Operator {
                        is_product_boundary: operator.is_boundary && in_product(operator.symbol),
                    },
                    schema_identity: typed_trees::operator::boundary_operator_requirement_identity(
                        typed, operator,
                    ),
                    package_identity: typed.symbols.symbol_package_identity(operator.symbol),
                });
            }
            for machine in typed.machines() {
                let Some(requirement) = IntrinsicRequirement::from_requirement(typed, machine)
                else {
                    continue;
                };
                let path = typed.symbols.display_path(machine.symbol, "::");
                if !machine.symbol.is_valid()
                    || !in_product(machine.symbol)
                    || machine.termination_plan.interface.published().is_none()
                    || !path
                        .rsplit_once("::")
                        .is_some_and(|(owner, name)| !owner.is_empty() && !name.is_empty())
                {
                    continue;
                }
                requirements.append(CheckedProviderRequirement {
                    symbol: machine.symbol,
                    kind: CheckedProviderRequirementKind::DirectCall {
                        diagnostic_name: requirement.display(),
                    },
                    schema_identity: if machine.spelling.is_some() {
                        requirement.requirement_identity
                    } else {
                        path
                    },
                    package_identity: requirement.package_identity,
                });
            }
            checked.facts.operators.provider_requirements = requirements;
        }
    }

    pub(crate) use applications::bind_boundary_operator_application_demands;
    pub(crate) use body_candidates::retain_boundary_body_candidates;
    pub(crate) use provider_requirements::retain_provider_requirements;
}
