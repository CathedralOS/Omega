/*
    Compile-time machine admission, build activation verification, hermetic
    build-machine execution, selected operator validation, and target-family
    body preparation.

    We enforce the reachable-closure admission floor (discharged `requires`
    contracts, non-linear carriers, termination, and `ConstEvaluable` results),
    validate toolchain `Build` activations, and rebind selected provider and
    binary operator bodies on private execution copies.
*/

pub(crate) mod admission {
    use crate::unique::push_unique;
    use checked_interpreter::{BuildMachineEvaluationRequest, BuildTimeOperationEvaluation};
    use checked_trees::CheckedTrees;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;

    use crate::BuildTimeValue;
    use crate::const_evaluation::data_layout_queries::has_pending_target_observations;
    use crate::evaluation_accounting::EvaluationAccounting;
    use crate::syntax_probes::PreparedEvaluationInputs;

    mod closure_validation {
        use super::{
            BuildTimeAdmissionPlan, BuildTimeCallEdge, BuildTimeProgram, RecursiveCheckingInput,
        };
        use crate::build_time_evaluation::{expression_table, type_reference_table};
        use language_semantics::{MachineSupplyMode, Multiplicity, TerminationGuarantee};
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::TypedTrees;
        use typed_trees::machine::Machine;
        use typed_trees::signature::{SignatureContract, StateSignature};
        use typed_trees_to_checked_trees::{
            infer_checked_recursive_machine_terminations, infer_machine_termination_summary,
        };

        pub(super) fn checked_closure_violation<'request>(
            admission: &'request BuildTimeAdmissionPlan,
            input: BuildTimeProgram<'request>,
            call_edges: &[BuildTimeCallEdge],
            execution: &dyn validation::CheckingInput,
            root: &Machine,
            discharge_authored_requires: bool,
        ) -> Option<String> {
            machine_termination_violation(
                call_edges,
                execution,
                root.symbol,
                &mut ClosureWalk::new(admission, input, root.symbol),
                discharge_authored_requires,
            )
        }

        pub(super) fn closure_has_authored_requires(
            call_edges: &[BuildTimeCallEdge],
            program: &TypedTrees,
            root: SymbolHandle,
        ) -> bool {
            each_closure_machine(
                call_edges,
                program,
                root,
                |machine| {
                    (has_authored_requires(program.machine_contracts(machine))
                        || program
                            .machine_states(machine)
                            .iter()
                            .any(|state| has_authored_requires(program.state_contracts(state))))
                    .then_some(())
                },
                |callable| callable_has_authored_requires(program, callable).then_some(()),
            )
            .is_some()
        }

        pub(super) fn closure_requires_are_root_parameter_domains(
            call_edges: &[BuildTimeCallEdge],
            program: &TypedTrees,
            root: SymbolHandle,
        ) -> bool {
            each_closure_machine(
                call_edges,
                program,
                root,
                |machine| {
                    if has_authored_requires(program.machine_contracts(machine)) {
                        return Some(());
                    }
                    for state in program.machine_states(machine) {
                        for contract in program.state_contracts(state) {
                            if contract.kind
                                != typed_trees::signature::SignatureContractKind::Requires
                            {
                                continue;
                            }
                            if machine.symbol != root
                                || !is_parameter_domain_premise(program, state, contract)
                            {
                                return Some(());
                            }
                        }
                    }
                    None
                },
                |callable| callable_has_authored_requires(program, callable).then_some(()),
            )
            .is_none()
        }

        fn each_closure_machine(
            call_edges: &[BuildTimeCallEdge],
            program: &TypedTrees,
            root: SymbolHandle,
            mut machine: impl FnMut(&Machine) -> Option<()>,
            mut callable: impl FnMut(SymbolHandle) -> Option<()>,
        ) -> Option<()> {
            super::walk_call_closure(call_edges, program, root, |found| {
                if machine(found).is_some() {
                    return Some(());
                }
                call_edges
                    .iter()
                    .filter(|call| call.source_machine_symbol == found.symbol)
                    .filter(|call| call.target_machine(program).is_none())
                    .any(|call| callable(call.target_state_symbol).is_some())
                    .then_some(())
            })
        }

        fn is_parameter_domain_premise(
            program: &TypedTrees,
            state: &typed_trees::state::State,
            contract: &SignatureContract,
        ) -> bool {
            use typed_trees::types::{
                DomainConstraintSubject, TypeConstraintNode, TypeReferenceNode,
            };

            if contract.keyword_source_span.is_some() || contract.binding.is_some() {
                return false;
            }
            let facts = program.proof_facts.span_or_empty(contract.facts);
            !facts.is_empty()
                && facts.iter().all(|fact| {
                    let Some(membership) = fact.as_membership() else {
                        return false;
                    };
                    if !membership.domain_symbol.is_valid()
                        || !membership.domain_arguments.is_empty()
                        || !expression_table(program).expression_is_valid(membership.value)
                    {
                        return false;
                    }
                    let Some(path) = program.expression_table.name_path_node(membership.value)
                    else {
                        return false;
                    };
                    let Some(parameter) =
                        program.state_parameters(state).iter().find(|parameter| {
                            parameter.symbol.is_valid() && parameter.symbol == path.symbol
                        })
                    else {
                        return false;
                    };
                    let mut reference = parameter.type_reference;
                    let mut visited = Vec::new();
                    while let TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } = program.type_reference_table.type_reference(reference)
                    {
                        if visited.contains(&reference) {
                            return false;
                        }
                        visited.push(reference);
                        if type_reference_table(program)
                            .constraints(*constraints)
                            .iter()
                            .any(|constraint| {
                                matches!(
                                    constraint,
                                    TypeConstraintNode::Domain(domain)
                                        if domain.subject == DomainConstraintSubject::Declared
                                            && domain.symbol == membership.domain_symbol
                                            && domain.arguments.is_empty()
                                )
                            })
                        {
                            return true;
                        }
                        reference = *base_type;
                    }
                    false
                })
        }

        struct ClosureWalk<'request> {
            admission: &'request BuildTimeAdmissionPlan,
            input: BuildTimeProgram<'request>,
            root: SymbolHandle,
            recursion_input: Option<RecursiveCheckingInput<'request>>,
            completed: Vec<SymbolHandle>,
            active: Vec<SymbolHandle>,
            path: Vec<String>,
            recursive_terminations: Vec<checked_trees::CheckedRuntimeRankMember>,
        }

        impl<'request> ClosureWalk<'request> {
            fn new(
                admission: &'request BuildTimeAdmissionPlan,
                input: BuildTimeProgram<'request>,
                root: SymbolHandle,
            ) -> Self {
                Self {
                    admission,
                    input,
                    root,
                    recursion_input: None,
                    completed: Vec::new(),
                    active: Vec::new(),
                    path: Vec::new(),
                    recursive_terminations: Vec::new(),
                }
            }

            fn recursive_terminations(
                &mut self,
                machine: SymbolHandle,
                body: arena::HandleSpan<typed_trees::state::State>,
            ) -> Result<Vec<checked_trees::CheckedRuntimeRankMember>, String> {
                if self.recursion_input.is_none() {
                    self.recursion_input = Some(
                        self.admission
                            .prepare_recursive_checking_input(self.input, self.root)?,
                    );
                }
                let checked = self
                    .recursion_input
                    .as_ref()
                    .expect("this request prepared its recursive checking input")
                    .checked();
                infer_checked_recursive_machine_terminations(checked, machine, body)
                    .map_err(|diagnostics| diagnostics::join_messages(&diagnostics))
            }
        }

        fn machine_termination_violation(
            call_edges: &[BuildTimeCallEdge],
            input: &dyn validation::CheckingInput,
            machine_symbol: SymbolHandle,
            walk: &mut ClosureWalk<'_>,
            discharge_authored_requires: bool,
        ) -> Option<String> {
            let program = input.typed_program();
            if walk.completed.contains(&machine_symbol) {
                return None;
            }
            let machine = program.machine(machine_symbol)?;
            walk.path.push(machine.name.as_str().to_owned());

            if !discharge_authored_requires
                && let Some(violation) =
                    machine_precondition_violation(program, machine, &walk.path)
            {
                walk.path.pop();
                return Some(violation);
            }
            if let Some(violation) = machine_linear_carrier_violation(program, machine, &walk.path)
            {
                walk.path.pop();
                return Some(violation);
            }

            if walk.active.contains(&machine_symbol) {
                let matches_body = |member: &checked_trees::CheckedRuntimeRankMember| {
                    member.machine == machine_symbol && member.body == machine.states
                };
                if !walk.recursive_terminations.iter().any(matches_body) {
                    let result = walk.recursive_terminations(machine_symbol, machine.states);
                    match result {
                        Ok(members) => walk.recursive_terminations.extend(members),
                        Err(reason) => {
                            let violation = format!(
                                "recursive machine-call component along `{}` failed ordinary checking: {reason}",
                                walk.path.join(" -> ")
                            );
                            walk.path.pop();
                            return Some(violation);
                        }
                    }
                }
                let measured = walk.recursive_terminations.iter().any(matches_body);
                if !measured {
                    let violation = format!(
                        "recursive machine-call cycle has no ordinary termination proof along `{}`",
                        walk.path.join(" -> ")
                    );
                    walk.path.pop();
                    return Some(violation);
                }
                walk.path.pop();
                return None;
            }
            walk.active.push(machine_symbol);

            let target_snapshot = match program.layout_target_facts_result(machine_symbol) {
                Ok(snapshot) => snapshot.is_some(),
                Err(reason) => {
                    walk.active.pop();
                    walk.path.pop();
                    return Some(reason.to_owned());
                }
            };
            let locally_terminates = target_snapshot
                || (machine.supply_mode == MachineSupplyMode::CheckedBody
                    && matches!(
                        infer_machine_termination_summary(input, machine_symbol),
                        Some(TerminationGuarantee::Terminates { .. })
                    ));
            if !locally_terminates {
                let violation = if matches!(
                    machine.supply_mode,
                    MachineSupplyMode::Requirement
                        | MachineSupplyMode::TopLevelRequirement
                        | MachineSupplyMode::Boundary
                ) {
                    format!(
                        "machine call path `{}` reaches a requirement that requires exact authored selection before evaluation; the evaluator cannot execute a bodyless requirement",
                        walk.path.join(" -> ")
                    )
                } else {
                    format!(
                        "machine call path `{}` has no ordinary checked `Terminates` guarantee",
                        walk.path.join(" -> ")
                    )
                };
                walk.active
                    .retain(|active_symbol| *active_symbol != machine_symbol);
                walk.path.pop();
                return Some(violation);
            }

            for call in call_edges
                .iter()
                .filter(|call| call.source_machine_symbol == machine_symbol)
            {
                if let Some(target_machine_symbol) = call.target_machine(program) {
                    if let Some(violation) = machine_termination_violation(
                        call_edges,
                        input,
                        target_machine_symbol,
                        walk,
                        discharge_authored_requires,
                    ) {
                        walk.active
                            .retain(|active_symbol| *active_symbol != machine_symbol);
                        walk.path.pop();
                        return Some(violation);
                    }
                } else if let Some(violation) = callable_contract_violation(
                    program,
                    call.target_state_symbol,
                    &walk.path,
                    discharge_authored_requires,
                ) {
                    walk.active
                        .retain(|active_symbol| *active_symbol != machine_symbol);
                    walk.path.pop();
                    return Some(violation);
                }
            }

            walk.active
                .retain(|active_symbol| *active_symbol != machine_symbol);
            walk.completed.push(machine_symbol);
            walk.path.pop();
            None
        }

        fn machine_precondition_violation(
            program: &TypedTrees,
            machine: &Machine,
            path: &[String],
        ) -> Option<String> {
            if has_authored_requires(program.machine_contracts(machine)) {
                return Some(format!(
                    "machine `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
                    machine.name,
                    path.join(" -> ")
                ));
            }
            program.machine_states(machine).iter().find_map(|state| {
        has_authored_requires(program.state_contracts(state)).then(|| {
            format!(
                "state `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
                state.name,
                path.join(" -> ")
            )
        })
    })
        }

        pub(super) fn has_authored_requires(contracts: &[SignatureContract]) -> bool {
            contracts.iter().any(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
        }

        fn machine_linear_carrier_violation(
            program: &TypedTrees,
            machine: &Machine,
            _path: &[String],
        ) -> Option<String> {
            validation::compiler_evaluation::linear_carrier_violation(program, machine)
        }

        fn callable_signature(
            program: &TypedTrees,
            symbol: SymbolHandle,
        ) -> Option<&StateSignature> {
            if !symbol.is_valid()
                || matches!(
                    program.symbols.get(symbol).kind,
                    SymbolKind::BuiltinFunction | SymbolKind::Operator
                )
            {
                return None;
            }
            program
                .machine_parameter_signature(symbol)
                .map(|(_, signature)| signature)
                .or_else(|| {
                    program
                        .trait_machine_signature(symbol)
                        .map(|(_, signature)| signature)
                })
        }

        fn callable_has_authored_requires(program: &TypedTrees, symbol: SymbolHandle) -> bool {
            callable_signature(program, symbol).is_some_and(|signature| {
                has_authored_requires(program.state_signature_contracts(signature))
            })
        }

        fn callable_contract_violation(
            program: &TypedTrees,
            symbol: SymbolHandle,
            path: &[String],
            discharge_authored_requires: bool,
        ) -> Option<String> {
            let signature = callable_signature(program, symbol)?;

            if !discharge_authored_requires
                && has_authored_requires(program.state_signature_contracts(signature))
            {
                return Some(format!(
                    "callable contract `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
                    signature.name,
                    path.join(" -> ")
                ));
            }

            for parameter in program.state_signature_parameters(signature) {
                if program.type_multiplicity(parameter.type_reference) == Multiplicity::Linear {
                    return Some(format!(
                        "callable contract `{}` parameter `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                        signature.name,
                        parameter.name,
                        program.display_type_reference(parameter.type_reference),
                        path.join(" -> ")
                    ));
                }
            }
            if program.type_multiplicity(signature.return_type) == Multiplicity::Linear {
                return Some(format!(
                    "callable contract `{}` result has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                    signature.name,
                    program.display_type_reference(signature.return_type),
                    path.join(" -> ")
                ));
            }

            if signature.termination_guarantee.promises_termination() {
                return None;
            }

            let name = signature.name.as_str();
            Some(format!(
                "callable contract `{name}` reached from `{}` publishes no `Terminates` guarantee",
                path.join(" -> ")
            ))
        }
    }
    mod concrete_invocation {
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::TypedTrees;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};

        use super::BuildTimeProgram;
        use super::closure_validation::has_authored_requires;
        use crate::syntax_probes::PreparedEvaluationInputs;
        use typed_trees_to_checked_trees::lower_typed_trees;
        use typed_trees_to_typed_trees::prepare_typed_trees;

        pub(super) fn zero_argument_invocation_discharges(
            program: &TypedTrees,
            machine: &Machine,
            source_span: Option<source::SourceSpan>,
        ) -> bool {
            value_invocation_discharges(program, machine, &[], source_span).is_ok()
        }

        pub(super) fn value_invocation_discharges(
            program: &TypedTrees,
            machine: &Machine,
            arguments: &[crate::BuildTimeValue],
            source_span: Option<source::SourceSpan>,
        ) -> Result<(), String> {
            let entry = entry_state(program, machine)
                .ok_or("constant invocation has no exact entry state")?;
            let parameters = program.state_parameters(entry);
            if parameters.len() != arguments.len() {
                return Err(
                    "constant invocation arguments differ from its checked entry arity".into(),
                );
            }
            let mut probe = program.clone();
            probe.pending_const_range_endpoints.clear();
            let (probe_machine, mut state) = invocation_probe_declarations(
                &mut probe,
                machine.symbol,
                "@const-invocation",
                entry.return_type,
            );
            let mut actuals = Vec::with_capacity(arguments.len());
            for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
                if matches!(argument, crate::BuildTimeValue::LayoutTarget(_)) {
                    let name = format!("@context-{position}");
                    let symbol = probe
                        .symbols
                        .insert_generated_children(
                            state.symbol,
                            [(SymbolKind::Parameter, name.as_str())],
                        )
                        .start();
                    let mut retained = parameter.clone();
                    retained.symbol = symbol;
                    retained.name = typed_trees::name::Identifier::generated(&name);
                    probe.push_state_parameter(&mut state, retained.clone());
                    let path = probe_name_path(&mut probe, symbol, retained.name);
                    actuals.push(probe.expression_table.insert(ExpressionNode::Name(path)));
                    continue;
                }
                let mut carrier = parameter.type_reference;
                while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(carrier)
                {
                    carrier = *base_type;
                }
                let reference = match program.type_reference_table.type_reference(carrier) {
                    typed_trees::types::TypeReferenceNode::Reference {
                        referee, access, ..
                    } => Some((*referee, *access)),
                    _ => None,
                };
                let mut snapshot = argument.clone();
                let value = super::supplied_values::literal(
                    &mut probe,
                    reference.map_or(parameter.type_reference, |(referee, _)| referee),
                    &mut snapshot,
                )
                .map_err(|reason| format!("constant argument `{}`: {reason}", parameter.name))?;
                let actual = if let Some((referee, access)) = reference {
                    let name = format!("@argument-{position}");
                    let symbol = probe
                        .symbols
                        .insert_generated_children(
                            state.symbol,
                            [(SymbolKind::Local, name.as_str())],
                        )
                        .start();
                    let name = typed_trees::name::Identifier::generated(&name);
                    probe.statement_table.push_statement(
                        &mut state.statement_nodes,
                        StatementNode::LocalData(typed_trees::statement::TableLocalData {
                            symbol,
                            name: name.clone(),
                            type_reference: referee,
                            initial_value: value,
                            is_mutable: access.is_exclusive(),
                            ..Default::default()
                        }),
                    );
                    let path = probe_name_path(&mut probe, symbol, name);
                    let target = probe.expression_table.insert(ExpressionNode::Name(path));
                    probe.expression_table.insert(ExpressionNode::Borrow(
                        typed_trees::expression::TableBorrowExpression { target, access },
                    ))
                } else {
                    value
                };
                actuals.push(actual);
            }
            let call = TableCallExpression {
                authored_call_selection: None,
                authored_call_binding: None,
                authored_machine_application: Default::default(),
                requirement_application: Default::default(),
                exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
                receiver: ExpressionHandle::invalid(),
                target_symbol: entry.symbol,
                static_machine_parameter: SymbolHandle::invalid(),
                target: entry.name.clone(),
                static_requirement_dispatch: None,
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                installed_interrupt_operation: None,
                layout_query: None,
                native_code_layout: None,
                build_selection: Default::default(),
                arguments: probe.expression_table.insert_expression_handles(actuals),
                evidence_arguments: Box::default(),
                operational_acknowledgement:
                    language_semantics::CallOperationalAcknowledgement::default(),
            };
            let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
            let call_span = source_span
                .or_else(|| probe.symbols.symbol_source_span(machine.symbol))
                .unwrap_or_default();
            probe.expression_table.set_source_span(concrete, call_span);
            let owner = finish_invocation_probe(&mut probe, probe_machine, state, concrete);
            let preparation_request = typed_trees_to_typed_trees::PreparationRequest::preliminary();
            let checking = typed_trees_to_checked_trees::CheckingRequest::new();
            let checked = prepare_typed_trees(probe, &preparation_request)
                .and_then(|(probe, preparation)| lower_typed_trees(probe, preparation, &checking))
                .map_err(|diagnostics| {
                    format!(
                        "constant invocation failed ordinary checking: {}",
                        diagnostics::join_messages(&diagnostics),
                    )
                })?;
            require_complete_invocation_conditions(&checked, owner)?;
            require_empty_failure_summary(&checked, owner)
        }

        fn probe_name_path(
            probe: &mut TypedTrees,
            symbol: SymbolHandle,
            name: typed_trees::name::Identifier,
        ) -> typed_trees::expression::TableNamePath {
            let mut members = arena::HandleSpan::empty();
            probe
                .expression_table
                .push_name_path_member(&mut members, name);
            let mut member_symbols = arena::HandleSpan::empty();
            probe
                .expression_table
                .push_name_path_member_symbol(&mut member_symbols, symbol);
            typed_trees::expression::TableNamePath::new(members, member_symbols, symbol, symbol)
        }

        pub(super) fn require_empty_failure_summary(
            checked: &checked_trees::CheckedTrees,
            owner: SymbolHandle,
        ) -> Result<(), String> {
            let summary = checked
                .body_crash_summary(owner)
                .ok_or("constant invocation has no complete checked failure summary")?;
            if summary.buckets().is_empty() {
                Ok(())
            } else {
                let mut causes = summary
                    .buckets()
                    .iter()
                    .map(|bucket| bucket.cause())
                    .collect::<Vec<_>>();
                causes.sort();
                causes.dedup();
                Err(format!(
                    "constant invocation retains unhandled {causes:?} routes at its concrete arguments"
                ))
            }
        }

        pub(super) fn argument_independent_closure_discharges(
            admission: &super::BuildTimeAdmissionPlan,
            input: BuildTimeProgram<'_>,
            machine: &Machine,
        ) -> Result<bool, String> {
            let program = input.typed();
            let entry_is_unconditional = !has_authored_requires(program.machine_contracts(machine))
                && entry_state(program, machine)
                    .is_some_and(|entry| !has_authored_requires(program.state_contracts(entry)));
            if !entry_is_unconditional {
                return Ok(false);
            }
            if let BuildTimeProgram::Checked(checked) = input {
                require_complete_invocation_conditions(checked, machine.symbol)?;
                return Ok(true);
            }
            admission.checked_closure_admission(program, machine.symbol)?;
            Ok(true)
        }

        pub(super) fn checked_demanded_closure(
            domain: &crate::build_time_evaluation::EvaluationDomain,
            program: &TypedTrees,
        ) -> Result<checked_trees::CheckedTrees, String> {
            let mut probe = domain.execution_program(program);
            probe.pending_const_range_endpoints.clear();
            let preparation_request = typed_trees_to_typed_trees::PreparationRequest::preliminary();
            let checking = typed_trees_to_checked_trees::CheckingRequest::new();
            let (probe, preparation) =
                prepare_typed_trees(probe, &preparation_request).map_err(|error| {
                    format!(
                        "demanded premise closure preparation failed: {}",
                        diagnostics::join_messages(&error)
                    )
                })?;
            let checked = lower_typed_trees(probe, preparation, &checking).map_err(|error| {
                format!(
                    "demanded premise closure checking failed: {}",
                    diagnostics::join_messages(&error)
                )
            })?;
            Ok(checked)
        }

        pub(super) fn entry_state<'a>(
            program: &'a TypedTrees,
            machine: &Machine,
        ) -> Option<&'a State> {
            let leaf = machine
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or_default();
            let states = program.machine_states(machine);
            states
                .iter()
                .find(|state| state.name.as_str() == leaf)
                .or_else(|| states.first())
        }

        pub(crate) struct SnapshotArgument {
            pub original: ExpressionHandle,
            pub literal: ExpressionNode,
        }

        pub(crate) fn snapshot_arguments_discharge(
            admission: &super::BuildTimeAdmissionPlan,
            program: &TypedTrees,
            retained: &[SymbolHandle],
            owner: SymbolHandle,
            probe_name: &str,
            preparation_request: &typed_trees_to_typed_trees::PreparationRequest,
            checking: &typed_trees_to_checked_trees::CheckingRequest,
            expression: ExpressionHandle,
            callee: &str,
            return_type: typed_trees::types::TypeReferenceHandle,
            snapshots: Vec<SnapshotArgument>,
        ) -> Result<(), String> {
            let mut probe = program.clone();
            if probe.expression_table.call_node(expression).is_none() {
                return Err("constant invocation has no selected call".into());
            }
            let copied = probe.expression_table.copy_graph(None, expression);
            typed_trees_to_typed_trees::restore_copied_call_sources(&mut probe, &[copied.root])
                .map_err(|reason| format!("constant call source restoration failed: {reason}"))?;
            probe.pending_const_range_endpoints.clear();
            let machines = probe
                .machines()
                .iter()
                .filter(|candidate| retained.contains(&candidate.symbol))
                .cloned()
                .collect::<Vec<_>>();
            probe.roots.machines = arena::HandleSpan::empty();
            for candidate in machines {
                probe.push_machine(candidate);
            }
            for snapshot in snapshots {
                let copied_argument = copied
                    .correspondence
                    .iter()
                    .find(|(original, _)| *original == snapshot.original)
                    .map(|(_, copied)| *copied)
                    .ok_or("constant snapshot has no exact copied operand")?;
                *probe.expression_table.expression_mut(copied_argument) = snapshot.literal;
            }
            let concrete = copied.root;
            let owner =
                append_invocation_probe(&mut probe, owner, probe_name, concrete, return_type);
            let checked = prepare_typed_trees(probe, preparation_request)
                .and_then(|(probe, preparation)| lower_typed_trees(probe, preparation, checking))
                .map_err(|diagnostics| {
                    format!(
                        "constant invocation of `{callee}` failed checking: {}",
                        diagnostics::join_messages(&diagnostics)
                    )
                })?;
            require_complete_invocation_conditions(&checked, owner)?;
            require_empty_failure_summary(&checked, owner)?;
            require_public_call_floor(&checked, owner, concrete, callee)?;
            let plan = admission.checked_program_admission(&checked)?;
            let machine = checked
                .typed
                .machine_by_symbol(owner)
                .ok_or("constant snapshot invocation lost its checked machine")?;
            plan.require_floor(
                BuildTimeProgram::Checked(&checked),
                machine,
                Some(crate::BuildTimeInvocationCustody::Source(
                    program.expression_table.source_span(expression),
                )),
                true,
            )
            .map_err(String::from)
        }

        fn require_public_call_floor(
            checked: &checked_trees::CheckedTrees,
            owner: SymbolHandle,
            expression: ExpressionHandle,
            callee: &str,
        ) -> Result<(), String> {
            let program = checked.typed.as_ref();
            let call = program
                .expression_table
                .call_node(expression)
                .ok_or("checked constant invocation has no call expression")?;
            let site = validation::CallSite::Expression { expression, call };
            if call.static_requirement_dispatch.is_none()
                && validation::selected_requirement_signature(program, call.target_symbol).is_none()
            {
                return Ok(());
            }
            let (_, signature) = validation::resolve_requirement_contract(program, owner, &site)?;
            let services = validation::trait_signature_service_reach(checked, signature.symbol)
                .ok_or("constant call has no exact applied public service envelope")?;
            let services = services
                .iter()
                .map(|service| {
                    program
                        .service_reaches
                        .definition(*service)
                        .map(|definition| definition.name.to_string())
                        .ok_or("constant call's public envelope names an unavailable service")
                })
                .collect::<Result<Vec<_>, _>>()?;
            semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_floor(
                callee,
                &semantic_vocabulary::compiler_evaluation::CompilerEvaluationEffects {
                    services,
                    may_suspend: signature.suspends,
                    may_block: signature.blocks,
                },
                None,
                None,
            )
        }

        pub(super) fn append_invocation_probe(
            probe: &mut TypedTrees,
            owner: SymbolHandle,
            name: &str,
            expression: ExpressionHandle,
            destination: typed_trees::types::TypeReferenceHandle,
        ) -> SymbolHandle {
            let (machine, state) = invocation_probe_declarations(probe, owner, name, destination);
            finish_invocation_probe(probe, machine, state, expression)
        }

        fn invocation_probe_declarations(
            probe: &mut TypedTrees,
            owner: SymbolHandle,
            name: &str,
            destination: typed_trees::types::TypeReferenceHandle,
        ) -> (Machine, State) {
            let symbol = probe
                .symbols
                .insert_generated_root_from(owner, SymbolKind::Machine, name);
            let children = probe
                .symbols
                .insert_generated_children(symbol, [(SymbolKind::State, name)]);
            (
                Machine {
                    symbol,
                    name: typed_trees::name::Identifier::generated(name),
                    ..Default::default()
                },
                State {
                    symbol: children.start(),
                    name: typed_trees::name::Identifier::generated(name),
                    return_type: destination,
                    ..Default::default()
                },
            )
        }

        fn finish_invocation_probe(
            probe: &mut TypedTrees,
            mut machine: Machine,
            mut state: State,
            expression: ExpressionHandle,
        ) -> SymbolHandle {
            let target = probe
                .statement_table
                .insert_transition_target(TransitionTargetNode::Value(expression));
            let source_span = probe.expression_table.source_span(expression);
            probe.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::Transition(TableTransition {
                    target,
                    source_span,
                    ..Default::default()
                }),
            );
            let owner = machine.symbol;
            probe.push_machine_state(&mut machine, state);
            probe.push_machine(machine);
            owner
        }

        pub(crate) fn require_complete_invocation_conditions(
            checked: &checked_trees::CheckedTrees,
            root: SymbolHandle,
        ) -> Result<(), String> {
            let has_pending_boundary_scalars = checked
                .facts
                .flow
                .terminal_unit_effects
                .boundary_machines
                .iter()
                .any(|plan| {
                    !plan.pending_scalar_requires.is_empty()
                        || !plan.pending_scalar_recipes.is_empty()
                });
            let has_pending_recipes = checked
                .facts
                .constant_recipe_obligations
                .iter()
                .any(|obligation| obligation.is_pending());
            let has_pending_predicates_or_ranges = checked
                .facts
                .constant_predicate_obligations
                .iter()
                .any(|obligation| obligation.is_pending())
                || checked
                    .facts
                    .integer_range_obligations
                    .iter()
                    .any(|obligation| obligation.is_pending());
            if !has_pending_boundary_scalars
                && !has_pending_recipes
                && !has_pending_predicates_or_ranges
            {
                return Ok(());
            }

            let domain = PreparedEvaluationInputs::domain_for_program(&checked.typed, &[root], &[]);
            let demanded = |owner: SymbolHandle| !owner.is_valid() || domain.contains(owner);
            if has_pending_boundary_scalars
                && checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .boundary_machines
                    .iter()
                    .any(|plan| {
                        (!plan.pending_scalar_requires.is_empty()
                            || !plan.pending_scalar_recipes.is_empty())
                            && (demanded(plan.machine)
                                || demanded(plan.state)
                                || checked.typed.target_machine_families.iter().any(|family| {
                                    family.canonical == plan.machine
                                        && family
                                            .members
                                            .iter()
                                            .any(|member| demanded(member.machine))
                                }))
                    })
            {
                return Err(
            "constant invocation retains pending scalar contract clauses in its demanded closure"
                .to_owned(),
        );
            }

            if has_pending_recipes
                && let Some(obligation) = checked
                    .facts
                    .constant_recipe_obligations
                    .iter()
                    .find(|obligation| obligation.is_pending() && demanded(obligation.owner()))
            {
                return Err(format!(
                    "constant invocation cannot establish its own pending target condition: {}",
                    obligation.description()
                ));
            }
            if has_pending_predicates_or_ranges
                && (checked
                    .facts
                    .constant_predicate_obligations
                    .iter()
                    .any(|obligation| obligation.is_pending() && demanded(obligation.caller()))
                    || checked
                        .facts
                        .integer_range_obligations
                        .iter()
                        .any(|obligation| obligation.is_pending() && demanded(obligation.owner())))
            {
                return Err(
            "constant invocation retains a pending target proof condition in its demanded closure"
                .to_owned(),
        );
            }
            Ok(())
        }
    }
    mod failures {
        use super::{
            BuildTimeAdmissionPlan, BuildTimeAdmissionRejection, BuildTimeInvocationCustody,
            BuildTimeProgram, closure_validation::has_authored_requires, concrete_invocation,
        };
        use crate::BuildTimeValue;
        use typed_trees::machine::Machine;

        impl BuildTimeAdmissionPlan {
            pub(super) fn require_supplied_invocation(
                &self,
                input: BuildTimeProgram<'_>,
                machine: &Machine,
                arguments: &mut [BuildTimeValue],
                custody: Option<BuildTimeInvocationCustody>,
            ) -> Result<(), BuildTimeAdmissionRejection> {
                self.require_invocation_failures(input, machine, arguments, custody)
                    .map_err(BuildTimeAdmissionRejection::from)?;
                self.require_floor(input, machine, custody, true)
            }

            pub(crate) fn require_invocation_failures(
                &self,
                input: BuildTimeProgram<'_>,
                machine: &Machine,
                arguments: &mut [BuildTimeValue],
                custody: Option<BuildTimeInvocationCustody>,
            ) -> Result<(), String> {
                let judgment = (|| {
                    super::supplied_values::require_established_arguments(
                        input.typed(),
                        machine,
                        arguments,
                    )?;
                    let prepared = self.prepare_recursive_checking_input(input, machine.symbol)?;
                    let checked = prepared.checked();
                    let selected = exact_checked_machine(checked, machine.symbol)?;
                    require_checked_invocation_failures(
                        checked, selected, arguments, input, custody,
                    )
                })();
                require_failure_judgment(machine, judgment)
            }
        }

        fn exact_checked_machine(
            checked: &checked_trees::CheckedTrees,
            symbol: symbols::SymbolHandle,
        ) -> Result<&Machine, String> {
            let mut matching = checked
                .typed
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == symbol);
            let selected = matching
                .next()
                .ok_or("constant invocation lost its exact checked machine")?;
            if matching.next().is_some() {
                return Err("constant invocation has ambiguous checked machine identity".into());
            }
            Ok(selected)
        }

        fn invocation_source_span(
            input: BuildTimeProgram<'_>,
            custody: Option<BuildTimeInvocationCustody>,
        ) -> Option<source::SourceSpan> {
            custody.and_then(|custody| match custody {
                BuildTimeInvocationCustody::Source(span) => Some(span),
                BuildTimeInvocationCustody::Symbol(symbol) => {
                    input.typed().symbols.symbol_provenance_source_span(symbol)
                }
            })
        }

        fn require_checked_invocation_failures(
            checked: &checked_trees::CheckedTrees,
            machine: &Machine,
            arguments: &[BuildTimeValue],
            input: BuildTimeProgram<'_>,
            custody: Option<BuildTimeInvocationCustody>,
        ) -> Result<(), String> {
            let entry = concrete_invocation::entry_state(&checked.typed, machine)
                .ok_or("constant invocation has no exact entry state")?;
            let entry_has_requirements =
                has_authored_requires(checked.typed.machine_contracts(machine))
                    || has_authored_requires(checked.typed.state_contracts(entry));
            if !entry_has_requirements
                && concrete_invocation::require_empty_failure_summary(checked, machine.symbol)
                    .is_ok()
            {
                return Ok(());
            }
            concrete_invocation::value_invocation_discharges(
                &checked.typed,
                machine,
                arguments,
                invocation_source_span(input, custody),
            )
        }

        fn require_failure_judgment(
            machine: &Machine,
            judgment: Result<(), String>,
        ) -> Result<(), String> {
            semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_failures(
                machine.name.as_str(),
                judgment.as_ref().map(|_| ()).map_err(String::as_str),
            )
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            fn typed(source: &str) -> typed_trees::TypedTrees {
                let tokens = source_files_to_tokens::Lexer::new(source)
                    .tokenize()
                    .expect("lex");
                let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
                let mut sources = source::SourceMap::default();
                sources.add_with_metadata(
                    "constant.omg".into(),
                    source.into(),
                    ".".into(),
                    semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
                    source::SourceOrigin::User,
                );
                let mut request =
                    syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
                request.sources = Some(std::sync::Arc::new(sources));
                let resolved =
                    syntax_trees_to_symbol_resolved_trees::resolve(request).expect("resolve");
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .expect("type")
            }

            fn on_compilation_stack(test: impl FnOnce() + Send + 'static) {
                std::thread::Builder::new()
                    .stack_size(16 * 1024 * 1024)
                    .spawn(test)
                    .expect("start compilation test")
                    .join()
                    .expect("compilation test");
            }

            #[test]
            fn source_admission_proves_guarded_actuals_before_execution() {
                on_compilation_stack(|| {
                    let program = typed(
                        r#"
            machine divide(numerator: u64, denominator: u64) -> u64
            crashes Trap denominator == 0u64 {
                transition denominator == 0u64 {
                    true -> bad()
                    false -> (numerator / denominator)
                }
                machine bad() -> u64 crashes Trap { crash Trap; }
            }
        "#,
                    );
                    let admission = BuildTimeAdmissionPlan::infer(&program, None);
                    let value = admission
                        .evaluate_machine(
                            BuildTimeProgram::Typed(&program),
                            "divide",
                            vec![BuildTimeValue::Int(14), BuildTimeValue::Int(2)],
                        )
                        .expect("a nonzero concrete denominator removes the route");
                    assert_eq!(value, BuildTimeValue::Int(7));
                    let error = admission
                        .evaluate_machine(
                            BuildTimeProgram::Typed(&program),
                            "divide",
                            vec![BuildTimeValue::Int(14), BuildTimeValue::Int(0)],
                        )
                        .expect_err("a surviving route must refuse before interpretation");
                    assert!(
                        error.reason.contains("not build-time admissible"),
                        "{error:?}"
                    );
                    assert!(error.reason.contains("unhandled [Trap]"), "{error:?}");
                    assert!(error.reason.contains("before execution"), "{error:?}");
                });
            }

            #[test]
            fn source_failure_summary_distinguishes_unknown_from_proved_empty() {
                on_compilation_stack(|| {
                    let (program, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
                        typed("machine answer() -> u64 { 7 }"),
                        &typed_trees_to_typed_trees::PreparationRequest::settled(),
                    )
                    .expect("prepare");
                    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(
                        program,
                        preparation,
                        &typed_trees_to_checked_trees::CheckingRequest::new(),
                    )
                    .expect("check");
                    let owner = checked
                        .typed
                        .realized_machine_named("answer")
                        .expect("answer")
                        .symbol;
                    assert!(checked.body_crash_summary(owner).is_some());
                    concrete_invocation::require_empty_failure_summary(&checked, owner)
                        .expect("ordinary checking proves the complete empty result");
                    let plan = checked
                        .facts
                        .contract_plans
                        .machines_mut()
                        .iter_mut()
                        .find(|plan| plan.machine == owner)
                        .expect("exact plan");
                    plan.crash = plan.crash.clone().with_checked_body_summary(None);
                    assert!(checked.body_crash_summary(owner).is_none());
                    let error = concrete_invocation::require_empty_failure_summary(&checked, owner)
                        .expect_err("absence cannot grant no-failure evidence");
                    assert!(
                        error.contains("no complete checked failure summary"),
                        "{error}"
                    );
                });
            }

            fn typed_with_layout_target(source: &str) -> typed_trees::TypedTrees {
                let core =
                    std::str::from_utf8(language_core::layout_storage::SOURCE).expect("core UTF-8");
                let mut sources = source::SourceMap::default();
                let core_id = sources
                    .add_with_metadata(
                        "/compiler/core/layout.omg".into(),
                        core.into(),
                        "/compiler/core".into(),
                        None,
                        source::SourceOrigin::Toolchain,
                    )
                    .source_id;
                let user_id = sources
                    .add_with_metadata(
                        "policy.omg".into(),
                        source.into(),
                        ".".into(),
                        semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
                        source::SourceOrigin::User,
                    )
                    .source_id;
                let tokens = source_files_to_tokens::Lexer::new(core)
                    .tokenize()
                    .expect("core lex");
                let mut syntax =
                    tokens_to_syntax_trees::parse_syntax_trees_with_id(core_id, &tokens)
                        .expect("core parse");
                let roots = syntax
                    .root_item_handles()
                    .iter()
                    .copied()
                    .filter(|root| {
                        matches!(syntax.root_item(*root), syntax_trees::item::Item::Data(data)
            if data.name.as_str(&syntax) == "LayoutTarget")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(roots.len(), 1, "the actual sealed carrier is unique");
                syntax.roots.items = roots;
                syntax.roots.mathematical_definitions.clear();
                let tokens = source_files_to_tokens::Lexer::new(source)
                    .tokenize()
                    .expect("policy lex");
                tokens_to_syntax_trees::parse(&mut syntax, user_id, &tokens).expect("policy parse");
                let mut request =
                    syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
                request.sources = Some(std::sync::Arc::new(sources));
                let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request)
                    .expect("policy resolve");
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .expect("policy type")
            }

            #[test]
            fn source_admission_checks_guarded_policy_with_real_target_argument() {
                on_compilation_stack(|| {
                    let program = typed_with_layout_target(
                        r#"
            machine plan(target: LayoutTarget, denominator: u64) -> u64
            crashes Trap denominator == 0u64 {
                transition denominator == 0u64 {
                    true -> bad()
                    false -> (7u64 / denominator)
                }
                machine bad() -> u64 crashes Trap { crash Trap; }
            }
        "#,
                    );
                    let target = layout_plans::LayoutTargetFacts {
                        profile: "macos_arm64".into(),
                        pointer_size: 8,
                        pointer_alignment: 8,
                        case_tag_size: 4,
                        case_tag_alignment: 4,
                        little_endian: true,
                    };
                    let admission = BuildTimeAdmissionPlan::infer(&program, None);
                    let value = admission
                        .evaluate_machine(
                            BuildTimeProgram::Typed(&program),
                            "plan",
                            vec![
                                BuildTimeValue::LayoutTarget(target.clone()),
                                BuildTimeValue::Int(1),
                            ],
                        )
                        .expect("a real opaque target does not prevent concrete guard discharge");
                    assert_eq!(value, BuildTimeValue::Int(7));
                    let error = admission
                        .evaluate_machine(
                            BuildTimeProgram::Typed(&program),
                            "plan",
                            vec![BuildTimeValue::LayoutTarget(target), BuildTimeValue::Int(0)],
                        )
                        .expect_err(
                            "the same target with a zero actual must refuse before execution",
                        );
                    assert!(error.reason.contains("unhandled [Trap]"), "{error:?}");
                    assert!(error.reason.contains("before execution"), "{error:?}");
                });
            }

            #[test]
            fn source_admission_establishes_host_parameter_domains_before_execution() {
                on_compilation_stack(|| {
                    let (program, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
                        typed(
                            r#"
            domain u64::NonZero requires self != 0;
            machine divide(denominator: u64 in NonZero) -> u64 { 7u64 / denominator }
        "#,
                        ),
                        &typed_trees_to_typed_trees::PreparationRequest::settled(),
                    )
                    .expect("prepare");
                    let checked = typed_trees_to_checked_trees::lower_typed_trees(
                        program,
                        preparation,
                        &typed_trees_to_checked_trees::CheckingRequest::new(),
                    )
                    .expect("check");
                    let program = checked.typed.as_ref();
                    let symbol = program
                        .realized_machine_named("divide")
                        .expect("divide")
                        .symbol;
                    concrete_invocation::require_empty_failure_summary(&checked, symbol)
                        .expect("the body is crash-free under its declared parameter domain");
                    let admission = BuildTimeAdmissionPlan::infer(program, None);
                    let value = admission
                        .evaluate_machine(
                            BuildTimeProgram::Checked(&checked),
                            "divide",
                            vec![BuildTimeValue::Int(1)],
                        )
                        .expect("a host actual in the declared domain is established");
                    assert_eq!(value, BuildTimeValue::Int(7));
                    let value = admission
                        .evaluate_machine_symbol_for_invocation_measured(
                            BuildTimeProgram::Checked(&checked),
                            symbol,
                            vec![BuildTimeValue::Int(1)],
                            BuildTimeInvocationCustody::Symbol(symbol),
                        )
                        .expect("the symbol entry establishes the same valid host domain")
                        .into_parts()
                        .0;
                    assert_eq!(value, BuildTimeValue::Int(7));
                    let error = admission
                        .evaluate_machine(
                            BuildTimeProgram::Checked(&checked),
                            "divide",
                            vec![BuildTimeValue::Int(0)],
                        )
                        .expect_err("a body summary cannot establish its host argument domain");
                    assert!(
                        error
                            .reason
                            .contains("constant invocation failed ordinary checking"),
                        "{error:?}"
                    );
                    assert!(error.reason.contains("requires"), "{error:?}");
                    assert!(
                        error.reason.contains("not build-time admissible"),
                        "{error:?}"
                    );
                    assert!(error.reason.contains("before execution"), "{error:?}");
                    let error = admission
                        .evaluate_machine_symbol_for_invocation_measured(
                            BuildTimeProgram::Checked(&checked),
                            symbol,
                            vec![BuildTimeValue::Int(0)],
                            BuildTimeInvocationCustody::Symbol(symbol),
                        )
                        .expect_err("the symbol entry must establish the same host domain");
                    assert!(
                        error.contains("constant invocation failed ordinary checking"),
                        "{error}"
                    );
                    assert!(error.contains("requires"), "{error}");
                    assert!(error.contains("before execution"), "{error}");
                });
            }

            #[test]
            fn source_empty_failure_summary_does_not_discharge_authored_requires() {
                on_compilation_stack(|| {
                    let (program, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
                        typed("machine answer(value: u64) -> u64 requires value != 0 { 7 }"),
                        &typed_trees_to_typed_trees::PreparationRequest::settled(),
                    )
                    .expect("prepare");
                    let checked = typed_trees_to_checked_trees::lower_typed_trees(
                        program,
                        preparation,
                        &typed_trees_to_checked_trees::CheckingRequest::new(),
                    )
                    .expect("check");
                    let program = checked.typed.as_ref();
                    let symbol = program
                        .realized_machine_named("answer")
                        .expect("answer")
                        .symbol;
                    concrete_invocation::require_empty_failure_summary(&checked, symbol)
                        .expect("the constant body has an empty failure summary");
                    let admission = BuildTimeAdmissionPlan::infer(program, None);
                    let value = admission
                        .evaluate_machine(
                            BuildTimeProgram::Checked(&checked),
                            "answer",
                            vec![BuildTimeValue::Int(1)],
                        )
                        .expect("the exact actual establishes the authored entry requirement");
                    assert_eq!(value, BuildTimeValue::Int(7));
                    let error = admission
                        .evaluate_machine(
                            BuildTimeProgram::Checked(&checked),
                            "answer",
                            vec![BuildTimeValue::Int(0)],
                        )
                        .expect_err("a crash-free body cannot grant its invocation precondition");
                    assert!(
                        error
                            .reason
                            .contains("constant invocation failed ordinary checking"),
                        "{error:?}"
                    );
                    assert!(error.reason.contains("requires"), "{error:?}");
                    assert!(error.reason.contains("before execution"), "{error:?}");
                });
            }
        }
    }
    mod supplied_values {
        use crate::build_time_evaluation::type_reference_table;
        use build_time_values::BuildTimeValue;
        use numerics::literals::{FloatLiteral, IntegerLiteral, IntegerRadix};
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataDefinition, DataField, DataMember};
        use typed_trees::expression::{
            ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
        };
        use typed_trees::machine::Machine;
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        pub(crate) fn require_established_arguments(
            program: &TypedTrees,
            machine: &Machine,
            arguments: &mut [BuildTimeValue],
        ) -> Result<(), String> {
            let entry = program
                .machine_states(machine)
                .first()
                .ok_or("policy has no entry state")?;
            let parameters = program.state_parameters(entry);
            if parameters.len() != arguments.len() {
                return Err("supplied policy arguments do not match the entry arity".into());
            }
            let mut probe = program.clone();
            let mut constructions = Vec::with_capacity(arguments.len());
            for (parameter, value) in parameters.iter().zip(arguments) {
                let destination =
                    match type_reference_table(program).type_reference(parameter.type_reference) {
                        TypeReferenceNode::Reference { referee, .. } => *referee,
                        _ => parameter.type_reference,
                    };
                if let BuildTimeValue::LayoutTarget(facts) = value {
                    facts.validate()?;
                    let symbol = program.type_reference_table.type_symbol(destination);
                    if !program.core_layout_declaration(
                        symbol,
                        language_core::sealed_intrinsics::SealedIntrinsic::LayoutTargetData,
                    ) {
                        return Err(
                    "compiler target context must arrive at the exact core LayoutTarget carrier"
                        .into(),
                );
                    }
                    continue;
                }
                let value = literal(&mut probe, destination, value)
                    .map_err(|reason| format!("policy argument `{}`: {reason}", parameter.name))?;
                let container = probe.symbols.insert_generated_root_from(
                    machine.symbol,
                    symbols::SymbolKind::Data,
                    "@supplied-carrier",
                );
                let field = probe
                    .symbols
                    .insert_generated_children(container, [(symbols::SymbolKind::Field, "value")])
                    .start();
                let mut definition = DataDefinition {
                    symbol: container,
                    name: typed_trees::name::Identifier::generated("@supplied-carrier"),
                    ..Default::default()
                };
                probe.push_data_member(
                    &mut definition,
                    DataMember::Field(DataField {
                        symbol: field,
                        name: typed_trees::name::Identifier::generated("value"),
                        type_reference: destination,
                        ..Default::default()
                    }),
                );
                let fields =
                    probe
                        .expression_table
                        .insert_struct_fields([TableStructLiteralField {
                            name: typed_trees::name::Identifier::generated("value"),
                            field_symbol: field,
                            value,
                        }]);
                let expression = probe.expression_table.insert(ExpressionNode::StructLiteral(
                    TableStructLiteral {
                        type_name: definition.name.clone(),
                        type_symbol: container,
                        fields,
                        ..Default::default()
                    },
                ));
                let destination = probe.type_reference_table.insert(TypeReferenceNode::Named {
                    symbol: container,
                    name: definition.name.clone(),
                });
                probe.push_data_definition(definition);
                probe.expression_table.set_source_span(
                    expression,
                    program
                        .symbols
                        .symbol_source_span(parameter.symbol)
                        .unwrap_or_default(),
                );
                let owner = super::concrete_invocation::append_invocation_probe(
                    &mut probe,
                    machine.symbol,
                    "@supplied-value",
                    expression,
                    destination,
                );
                constructions.push((owner, expression));
            }
            let checking = validation::CheckingContext::new(&probe);
            for (owner, expression) in constructions {
                let machine = probe
                    .machine(owner)
                    .ok_or("supplied-value construction lost its owner")?;
                let state = probe
                    .machine_states(machine)
                    .first()
                    .ok_or("supplied-value construction lost its state")?;
                let mut formation_obligations = Vec::new();
                validation::validate_value_construction(
                    &checking,
                    machine,
                    state,
                    expression,
                    &mut formation_obligations,
                )
                .map_err(|diagnostics| {
                    format!(
                        "supplied policy arguments failed value establishment: {}",
                        diagnostics::join_messages(&diagnostics),
                    )
                })?;
                if formation_obligations.iter().any(|row| row.is_pending()) {
                    return Err("supplied policy arguments require a closed selected address model before value establishment".into());
                }
            }
            Ok(())
        }

        pub(super) fn literal(
            program: &mut TypedTrees,
            destination: TypeReferenceHandle,
            value: &mut BuildTimeValue,
        ) -> Result<ExpressionHandle, String> {
            let carrier = type_reference_table(program)
                .type_reference(destination)
                .clone();
            let node = match carrier {
                TypeReferenceNode::Constrained { base_type, .. } => {
                    return literal(program, base_type, value);
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } => {
                    let BuildTimeValue::Array(elements) = value else {
                        return Err("expected an array snapshot".into());
                    };
                    if elements.len() != length {
                        return Err(format!(
                            "expected {length} elements, found {}",
                            elements.len()
                        ));
                    }
                    let mut expressions = Vec::with_capacity(length);
                    for element in elements.iter_mut() {
                        expressions.push(literal(program, element_type, element)?);
                    }
                    ExpressionNode::ArrayLiteral(
                        program
                            .expression_table
                            .insert_expression_handles(expressions),
                    )
                }
                TypeReferenceNode::Named { symbol, name } => {
                    if let Some(primitive) = PrimitiveType::from_name(name.as_str()) {
                        match (primitive, value) {
                            (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => {
                                ExpressionNode::Boolean(*value)
                            }
                            (primitive, BuildTimeValue::Int(value))
                                if primitive.accepts_integer_literal() =>
                            {
                                let digits = if primitive.is_signed_integer() {
                                    value.unsigned_abs().to_string()
                                } else {
                                    (*value as u64).to_string()
                                };
                                ExpressionNode::Integer(
                                    IntegerLiteral::from_parts(
                                        primitive.is_signed_integer() && *value < 0,
                                        IntegerRadix::Decimal,
                                        &digits,
                                    )
                                    .map_err(str::to_owned)?,
                                )
                            }
                            (
                                PrimitiveType::F32 | PrimitiveType::F64,
                                BuildTimeValue::Float(value),
                            ) => ExpressionNode::Float(FloatLiteral::from_f64(*value)),
                            (
                                PrimitiveType::F32 | PrimitiveType::F64,
                                BuildTimeValue::UndeterminedNaN(format),
                            ) => ExpressionNode::UndeterminedNaN(*format),
                            _ => return Err(format!("snapshot does not match primitive `{name}`")),
                        }
                    } else {
                        let definition = program
                            .data_definition_by_symbol(symbol)
                            .ok_or("snapshot has no exact data declaration")?
                            .clone();
                        let (case_name, case_symbol, declared, supplied) = match value {
                            BuildTimeValue::Struct { type_name, fields } => {
                                if type_name != definition.name.as_str() {
                                    return Err(format!(
                                        "expected `{}`, found `{type_name}`",
                                        definition.name
                                    ));
                                }
                                let declared = program
                                    .data_members(&definition)
                                    .iter()
                                    .filter_map(|member| member.as_field().cloned())
                                    .collect::<Vec<_>>();
                                (None, None, declared, fields)
                            }
                            BuildTimeValue::Case { variant, payload } => {
                                let selected = program
                                    .data_members(&definition)
                                    .iter()
                                    .find_map(|member| match member {
                                        DataMember::Variant(candidate)
                                            if candidate.name.as_str() == variant =>
                                        {
                                            Some(candidate.clone())
                                        }
                                        _ => None,
                                    })
                                    .ok_or_else(|| {
                                        format!("unknown case `{variant}` of `{}`", definition.name)
                                    })?;
                                (
                                    Some(selected.name.clone()),
                                    Some(selected.symbol),
                                    program.data_payload_fields(&selected).to_vec(),
                                    payload,
                                )
                            }
                            _ => {
                                return Err(format!(
                                    "expected a declared value of `{}`",
                                    definition.name
                                ));
                            }
                        };
                        supplied.retain(|(name, _)| {
                            declared.iter().any(|field| name == field.name.as_str())
                        });
                        if declared.len() != supplied.len() {
                            return Err(format!(
                                "expected {} fields, found {}",
                                declared.len(),
                                supplied.len()
                            ));
                        }
                        let mut fields = Vec::with_capacity(declared.len());
                        for field in declared {
                            let mut matches = supplied
                                .iter_mut()
                                .filter(|(name, _)| name == field.name.as_str());
                            let (_, value) = matches
                                .next()
                                .ok_or_else(|| format!("missing field `{}`", field.name))?;
                            if matches.next().is_some() {
                                return Err(format!("repeated field `{}`", field.name));
                            }
                            fields.push(TableStructLiteralField {
                                name: field.name,
                                field_symbol: field.symbol,
                                value: literal(program, field.type_reference, value)?,
                            });
                        }
                        ExpressionNode::StructLiteral(TableStructLiteral {
                            application_type: Default::default(),
                            type_name: definition.name,
                            type_symbol: symbol,
                            case_name,
                            case_symbol,
                            fields: program.expression_table.insert_struct_fields(fields),
                        })
                    }
                }
                _ => return Err("snapshot has no closed literal carrier".into()),
            };
            Ok(program.expression_table.insert(node))
        }
    }

    pub(crate) use concrete_invocation::{
        SnapshotArgument,
        require_complete_invocation_conditions as require_complete_checked_conditions,
        snapshot_arguments_discharge,
    };
    pub(crate) mod const_evaluable {
        use language_semantics::{DataSupplyMode, Multiplicity};
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataDefinition, DataMember, DataShapeKind, TypeParameterKind};
        use typed_trees::machine::Machine;
        use typed_trees::type_identity::TypeIdentityId;
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        use crate::BuildTimeValue;
        use crate::build_time_evaluation::type_reference_table;
        use numerics::literals::FloatFormat;

        pub(super) fn require_const_evaluable_result(
            program: &TypedTrees,
            machine: &Machine,
            value: &BuildTimeValue,
        ) -> Result<(), String> {
            let state = entry_state(program, machine).ok_or_else(|| {
                format!(
                    "machine `{}` has no state whose result can be checked for ConstEvaluable",
                    machine.name
                )
            })?;
            let mut active_data = Vec::new();
            check_value(
                program,
                state.return_type,
                value,
                "result",
                &mut active_data,
            )
            .map_err(|violation| {
                format!(
                    "build-time result of machine `{}` is not ConstEvaluable: {violation}",
                    machine.name
                )
            })
        }

        /// Give a Terminal-read result the source names `check_value` and every
        /// build-time consumer compare against. The Terminal artifact labels a
        /// record with its canonical type identity and may qualify a case with
        /// its type; the machine's declared return type names the data
        /// definition, its fields and its cases. A shape that does not follow the
        /// declaration is left unchanged so `check_value` reports it.
        pub(crate) fn relabel_result(
            program: &TypedTrees,
            machine: &Machine,
            value: BuildTimeValue,
        ) -> BuildTimeValue {
            match entry_state(program, machine) {
                Some(state) => relabel_value(program, state.return_type, value),
                None => value,
            }
        }

        fn relabel_value(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
            value: BuildTimeValue,
        ) -> BuildTimeValue {
            if !type_reference.is_valid() {
                return value;
            }
            match (
                program.type_reference_table.type_reference(type_reference),
                value,
            ) {
                (TypeReferenceNode::Constrained { base_type, .. }, value) => {
                    relabel_value(program, *base_type, value)
                }
                (
                    TypeReferenceNode::FixedArray { element_type, .. },
                    BuildTimeValue::Array(elements),
                ) => BuildTimeValue::Array(
                    elements
                        .into_iter()
                        .map(|element| relabel_value(program, *element_type, element))
                        .collect(),
                ),
                (
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
                    value @ (BuildTimeValue::Struct { .. } | BuildTimeValue::Case { .. }),
                ) => {
                    let Some(definition) = program.closed_data_definition(type_reference) else {
                        return value;
                    };
                    match value {
                        BuildTimeValue::Struct { type_name, fields } => {
                            let declared = program
                                .data_members(definition)
                                .iter()
                                .filter_map(|member| match member {
                                    DataMember::Field(field) => Some(field),
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            let _ = type_name;
                            BuildTimeValue::Struct {
                                type_name: definition.name.as_str().to_owned(),
                                fields: fields
                                    .into_iter()
                                    .map(|(name, field_value)| {
                                        let field_type = declared
                                            .iter()
                                            .find(|field| field.name.as_str() == name)
                                            .map(|field| field.type_reference);
                                        let field_value = match field_type {
                                            Some(field_type) => {
                                                relabel_value(program, field_type, field_value)
                                            }
                                            None => field_value,
                                        };
                                        (name, field_value)
                                    })
                                    .collect(),
                            }
                        }
                        BuildTimeValue::Case { variant, payload } => {
                            let leaf = variant.rsplit("::").next().unwrap_or(&variant).to_owned();
                            let selected =
                                program.data_members(definition).iter().find_map(|member| {
                                    match member {
                                        DataMember::Variant(candidate)
                                            if candidate.name.as_str() == leaf =>
                                        {
                                            Some(candidate)
                                        }
                                        _ => None,
                                    }
                                });
                            let Some(selected) = selected else {
                                return BuildTimeValue::Case { variant, payload };
                            };
                            let declared = program.data_payload_fields(selected);
                            BuildTimeValue::Case {
                                variant: leaf,
                                payload: payload
                                    .into_iter()
                                    .map(|(name, field_value)| {
                                        let field_type = declared
                                            .iter()
                                            .find(|field| field.name.as_str() == name)
                                            .map(|field| field.type_reference);
                                        let field_value = match field_type {
                                            Some(field_type) => {
                                                relabel_value(program, field_type, field_value)
                                            }
                                            None => field_value,
                                        };
                                        (name, field_value)
                                    })
                                    .collect(),
                            }
                        }
                        value => value,
                    }
                }
                (_, value) => value,
            }
        }

        fn entry_state<'program>(
            program: &'program TypedTrees,
            machine: &Machine,
        ) -> Option<&'program typed_trees::state::State> {
            let states = program.machine_states(machine);
            let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
            states
                .iter()
                .find(|state| state.name.as_str() == leaf)
                .or_else(|| states.first())
        }

        fn check_value(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
            value: &BuildTimeValue,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            if !type_reference.is_valid() {
                return Err(format!("{path} has an invalid declared type"));
            }
            if matches!(value, BuildTimeValue::Text(_)) {
                return Err(format!(
                    "{path} contains Text; borrowed or dynamically sized text cannot cross the semantic const boundary"
                ));
            }

            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Unit => expect_value(path, value, "Unit", |value| {
                    matches!(value, BuildTimeValue::Unit)
                }),
                TypeReferenceNode::Named { name, .. } => {
                    if name.as_str().starts_with("Atomic") {
                        return Err(format!(
                            "{path} has interior-mutable type `{name}`, which is not const-copy eligible"
                        ));
                    }
                    if let Some(primitive) = PrimitiveType::from_name(name.as_str()) {
                        return check_primitive(path, primitive, value);
                    }
                    check_data(
                        program,
                        type_reference,
                        name.as_str(),
                        value,
                        path,
                        active_data,
                    )
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    let FixedArrayLength::Literal(expected_length) = length else {
                        return Err(format!(
                            "{path} has a non-literal array length and is not a closed const type"
                        ));
                    };
                    let BuildTimeValue::Array(elements) = value else {
                        return Err(format!(
                            "{path} expected an array value, found {}",
                            kind(value)
                        ));
                    };
                    if elements.len() != *expected_length {
                        return Err(format!(
                            "{path} expected {expected_length} array element(s), found {}",
                            elements.len()
                        ));
                    }
                    for (index, element) in elements.iter().enumerate() {
                        check_value(
                            program,
                            *element_type,
                            element,
                            &format!("{path}[{index}]"),
                            active_data,
                        )?;
                    }
                    Ok(())
                }
                TypeReferenceNode::Reference { .. } => Err(format!(
                    "{path} has reference type; references cannot escape semantic const evaluation"
                )),
                TypeReferenceNode::Slice { .. } => Err(format!(
                    "{path} has slice type; dynamically sized values cannot cross the semantic const boundary"
                )),
                TypeReferenceNode::Constrained { base_type, .. } => {
                    check_value(program, *base_type, value, path, active_data)
                }
                TypeReferenceNode::Generic { base_name, .. } => check_data(
                    program,
                    type_reference,
                    base_name.as_str(),
                    value,
                    path,
                    active_data,
                ),
                TypeReferenceNode::ConstExpression(_) => Err(format!(
                    "{path} has a proof-static expression type, not a runtime const value type"
                )),
                TypeReferenceNode::DynamicTrait { .. } => Err(format!(
                    "{path} has a dynamic trait type and is not a closed const type"
                )),
            }
        }

        fn check_primitive(
            path: &str,
            primitive: PrimitiveType,
            value: &BuildTimeValue,
        ) -> Result<(), String> {
            match (primitive, value) {
                (PrimitiveType::Bool, BuildTimeValue::Bool(_))
                | (PrimitiveType::F32 | PrimitiveType::F64, BuildTimeValue::Float(_)) => Ok(()),
                (PrimitiveType::F32, BuildTimeValue::UndeterminedNaN(FloatFormat::F32))
                | (PrimitiveType::F64, BuildTimeValue::UndeterminedNaN(FloatFormat::F64)) => Ok(()),
                (primitive, BuildTimeValue::Int(_)) if primitive.accepts_integer_literal() => {
                    Ok(())
                }
                _ => Err(format!(
                    "{path} expected `{}`, found {}",
                    primitive.name(),
                    kind(value)
                )),
            }
        }

        fn consistent_nominal_spelling(recorded: &str, declared: &str) -> bool {
            recorded == declared
                || recorded
                    .strip_suffix(declared)
                    .is_some_and(|prefix| prefix.ends_with("::"))
        }

        fn closed_data<'program>(
            program: &'program TypedTrees,
            reference: TypeReferenceHandle,
            name: &str,
            path: &str,
        ) -> Result<&'program DataDefinition, String> {
            let definition = program.closed_data_definition(reference).ok_or_else(|| {
                format!(
                    "{path} has an open or generic aggregate type and is not a closed const type"
                )
            })?;
            if matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { .. }
            ) && !consistent_nominal_spelling(name, definition.name.as_str())
            {
                return Err(format!(
                    "{path} has inconsistent nominal type spelling `{name}` for `{}`",
                    definition.name
                ));
            }
            Ok(definition)
        }

        fn check_data(
            program: &TypedTrees,
            reference: TypeReferenceHandle,
            name: &str,
            value: &BuildTimeValue,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            let definition = closed_data(program, reference, name, path)?;
            let identity = program.normalized_type_identity(reference);
            if !identity.is_valid() || active_data.contains(&identity) {
                return Err(format!(
                    "{path} reaches recursive or unresolved data `{name}`, which is outside the closed aggregate const boundary"
                ));
            }
            active_data.push(identity);
            let result = (|| {
                require_closed_data_shape(program, definition, path, active_data)?;
                match DataDefinition::shape_kind_from_members(program.data_members(definition)) {
                    DataShapeKind::Empty | DataShapeKind::Record => {
                        if definition.properties.multiplicity == Multiplicity::Unrestricted {
                            check_record(program, definition, value, path, active_data)
                        } else {
                            Err(format!(
                                "{path} has affine or linear type `{}`; ConstEvaluable record results must be freely copyable",
                                definition.name
                            ))
                        }
                    }
                    DataShapeKind::Enum => check_sum(program, definition, value, path, active_data),
                    DataShapeKind::Mixed => Err(format!(
                        "{path} has mixed record/sum shape `{name}`, which is outside this ConstEvaluable stage"
                    )),
                }
            })();
            active_data.pop();
            result
        }

        fn require_closed_data_shape(
            program: &TypedTrees,
            definition: &DataDefinition,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            if definition.supply_mode != DataSupplyMode::CheckedShape {
                return Err(format!(
                    "{path} has boundary-opaque type `{}`, not a checked const shape",
                    definition.name
                ));
            }
            if !definition.lifetime_parameters.is_empty()
                || !program.data_type_parameters(definition).is_empty()
            {
                return Err(format!(
                    "{path} has open or generic aggregate type `{}`",
                    definition.name
                ));
            }
            if let Some(application) = definition.generic_instance {
                require_closed_generic_application(
                    program,
                    definition,
                    application,
                    path,
                    active_data,
                )?;
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "{path} has quotient type `{}`; quotient representatives require their separate checked admission",
                    definition.name
                ));
            }
            Ok(())
        }

        fn require_closed_generic_application(
            program: &TypedTrees,
            definition: &DataDefinition,
            application: TypeReferenceHandle,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            let TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } = program.type_reference_table.type_reference(application)
            else {
                return Err(format!(
                    "{path} has generic aggregate type `{}` whose origin is not a resolved generic application",
                    definition.name
                ));
            };
            if !lifetime_arguments.is_empty() {
                return Err(format!(
                    "{path} has generic aggregate type `{}` with non-erased lifetime arguments",
                    definition.name
                ));
            }
            if !base_symbol.is_valid() {
                return Err(format!(
                    "{path} has generic aggregate type `{base_name}` without an exact nominal base identity"
                ));
            }
            let mut bases = program
                .data_definitions()
                .iter()
                .filter(|candidate| candidate.symbol == *base_symbol);
            let base = bases.next().ok_or_else(|| {
                format!(
                    "{path} has generic aggregate type `{base_name}` with an unknown base data type"
                )
            })?;
            if bases.next().is_some() {
                return Err(format!(
                    "{path} has ambiguous nominal base identity for `{base_name}`"
                ));
            }
            if !consistent_nominal_spelling(base_name.as_str(), base.name.as_str())
                || base.generic_instance.is_some()
            {
                return Err(format!(
                    "{path} has generic aggregate type `{}` whose base `{base_name}` is not the open generic template",
                    definition.name
                ));
            }
            let parameters = program.data_type_parameters(base);
            let arguments = type_reference_table(program).type_reference_handles(*arguments);
            if arguments.len() != parameters.len() {
                return Err(format!(
                    "{path} has generic aggregate type `{}` supplying {} argument(s) for {} base parameter(s)",
                    definition.name,
                    arguments.len(),
                    parameters.len()
                ));
            }
            for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
                require_closed_argument(
                    program,
                    &parameter.kind,
                    *argument,
                    &format!("{path} parameter `{}`", parameter.name),
                    active_data,
                )?;
            }
            Ok(())
        }

        fn require_closed_argument(
            program: &TypedTrees,
            kind: &TypeParameterKind,
            argument: TypeReferenceHandle,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            match kind {
                TypeParameterKind::Type => {
                    require_closed_type(program, argument, path, active_data)
                }
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    if type_reference_table(program)
                        .anonymous_numeric_literal_value(argument)
                        .is_some()
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "{path} has a non-literal const argument and is not a closed const application"
                        ))
                    }
                }
                TypeParameterKind::Implementation => {
                    let operand = program
                        .static_operand_from_type_reference(argument)
                        .map_err(|reason| {
                            format!("{path} lost its erased implementation operand: {reason}")
                        })?;
                    validation::conformance_applications::close_conformance_application(
                        program, &operand,
                    )
                    .map(|_| ())
                    .map_err(|reason| {
                        format!(
                            "{path} is not a closed named implementation: {}",
                            reason.message
                        )
                    })
                }
                TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => {
                    Err(format!(
                        "{path} is not a data parameter; only closed const atoms and closed types can cross the const boundary"
                    ))
                }
            }
        }

        fn require_closed_type(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            if !type_reference.is_valid() {
                return Err(format!("{path} has an invalid argument type"));
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Unit => Ok(()),
                TypeReferenceNode::Named { name, .. } => {
                    if name.as_str().starts_with("Atomic") {
                        return Err(format!(
                            "{path} has interior-mutable type `{name}`, which is not const-copy eligible"
                        ));
                    }
                    if PrimitiveType::from_name(name.as_str()).is_some() {
                        return Ok(());
                    }
                    require_closed_data_type(
                        program,
                        type_reference,
                        name.as_str(),
                        path,
                        active_data,
                    )
                }
                TypeReferenceNode::Generic { base_name, .. } => require_closed_data_type(
                    program,
                    type_reference,
                    base_name.as_str(),
                    path,
                    active_data,
                ),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    let FixedArrayLength::Literal(_) = length else {
                        return Err(format!(
                            "{path} has a non-literal array length and is not a closed const type"
                        ));
                    };
                    require_closed_type(program, *element_type, &format!("{path}[]"), active_data)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    require_closed_type(program, *base_type, path, active_data)
                }
                TypeReferenceNode::Reference { .. } => Err(format!("{path} has reference type")),
                TypeReferenceNode::Slice { .. } => Err(format!("{path} has slice type")),

                TypeReferenceNode::ConstExpression(_) => Err(format!(
                    "{path} has a proof-static expression type, not a runtime const value type"
                )),
                TypeReferenceNode::DynamicTrait { .. } => {
                    Err(format!("{path} has a dynamic trait type"))
                }
            }
        }

        fn require_closed_data_type(
            program: &TypedTrees,
            reference: TypeReferenceHandle,
            name: &str,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            let definition = closed_data(program, reference, name, path)?;
            let identity = program.normalized_type_identity(reference);
            if !identity.is_valid() || active_data.contains(&identity) {
                return Err(format!(
                    "{path} reaches recursive or unresolved data `{name}` through a generic argument"
                ));
            }
            active_data.push(identity);
            let result = (|| {
                require_closed_data_shape(program, definition, path, active_data)?;
                for member in program.data_members(definition) {
                    match member {
                        DataMember::Field(field) => require_closed_type(
                            program,
                            field.type_reference,
                            &format!("{path}.{}", field.name),
                            active_data,
                        )?,
                        DataMember::Variant(variant) => {
                            for field in program.data_payload_fields(variant) {
                                require_closed_type(
                                    program,
                                    field.type_reference,
                                    &format!("{path}::{}.{}", variant.name, field.name),
                                    active_data,
                                )?;
                            }
                        }
                    }
                }
                Ok(())
            })();
            active_data.pop();
            result
        }

        fn check_record(
            program: &TypedTrees,
            definition: &DataDefinition,
            value: &BuildTimeValue,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            let BuildTimeValue::Struct { type_name, fields } = value else {
                return Err(format!(
                    "{path} expected record `{}`, found {}",
                    definition.name,
                    kind(value)
                ));
            };
            if type_name != definition.name.as_str() {
                return Err(format!(
                    "{path} expected record `{}`, found record `{type_name}`",
                    definition.name
                ));
            }
            let declared_fields = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    DataMember::Variant(_) => None,
                })
                .collect::<Vec<_>>();
            if fields.len() != declared_fields.len() {
                return Err(format!(
                    "{path} expected {} field(s), found {}",
                    declared_fields.len(),
                    fields.len()
                ));
            }
            for declared in declared_fields {
                let mut matches = fields
                    .iter()
                    .filter(|(name, _)| name == declared.name.as_str());
                let (_, field_value) = matches.next().ok_or_else(|| {
                    format!(
                        "{path} is missing declared field `{}` of `{}`",
                        declared.name, definition.name
                    )
                })?;
                if matches.next().is_some() {
                    return Err(format!(
                        "{path} repeats field `{}` of `{}`",
                        declared.name, definition.name
                    ));
                }
                check_value(
                    program,
                    declared.type_reference,
                    field_value,
                    &format!("{path}.{}", declared.name),
                    active_data,
                )?;
            }
            Ok(())
        }

        fn check_sum(
            program: &TypedTrees,
            definition: &DataDefinition,
            value: &BuildTimeValue,
            path: &str,
            active_data: &mut Vec<TypeIdentityId>,
        ) -> Result<(), String> {
            let BuildTimeValue::Case { variant, payload } = value else {
                return Err(format!(
                    "{path} expected a case of `{}`, found {}",
                    definition.name,
                    kind(value)
                ));
            };
            let mut variants = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(candidate) if candidate.name.as_str() == variant => {
                        Some(candidate)
                    }
                    DataMember::Field(_) | DataMember::Variant(_) => None,
                });
            let selected = variants.next().ok_or_else(|| {
                format!(
                    "{path} names unknown case `{variant}` of `{}`",
                    definition.name
                )
            })?;
            if variants.next().is_some() {
                return Err(format!(
                    "{path} names ambiguous case `{variant}` of `{}`",
                    definition.name
                ));
            }
            let declared_payload = program.data_payload_fields(selected);
            if payload.len() != declared_payload.len() {
                return Err(format!(
                    "{path} case `{variant}` expected {} payload field(s), found {}",
                    declared_payload.len(),
                    payload.len()
                ));
            }
            for declared in declared_payload {
                let mut matches = payload
                    .iter()
                    .filter(|(name, _)| name == declared.name.as_str());
                let (_, payload_value) = matches.next().ok_or_else(|| {
                    format!(
                        "{path} case `{variant}` is missing payload field `{}`",
                        declared.name
                    )
                })?;
                if matches.next().is_some() {
                    return Err(format!(
                        "{path} case `{variant}` repeats payload field `{}`",
                        declared.name
                    ));
                }
                check_value(
                    program,
                    declared.type_reference,
                    payload_value,
                    &format!("{path}::{variant}.{}", declared.name),
                    active_data,
                )?;
            }
            Ok(())
        }

        fn expect_value(
            path: &str,
            value: &BuildTimeValue,
            expected: &str,
            predicate: impl FnOnce(&BuildTimeValue) -> bool,
        ) -> Result<(), String> {
            if predicate(value) {
                Ok(())
            } else {
                Err(format!("{path} expected {expected}, found {}", kind(value)))
            }
        }

        fn kind(value: &BuildTimeValue) -> &'static str {
            match value {
                BuildTimeValue::Unit => "Unit",
                BuildTimeValue::LayoutTarget(_) => "compiler-issued target context",
                BuildTimeValue::Int(_) => "integer",
                BuildTimeValue::Bool(_) => "Bool",
                BuildTimeValue::Float(_) | BuildTimeValue::UndeterminedNaN(_) => "float",
                BuildTimeValue::Text(_) => "Text",
                BuildTimeValue::Struct { .. } => "record",
                BuildTimeValue::Case { .. } => "sum case",
                BuildTimeValue::Array(_) => "array",
            }
        }
    }
    mod selection_authority {
        use crate::build_time_evaluation::expression_table;
        use language_semantics::declaration_selection::BuildOperation;
        use semantic_vocabulary::PackageKeyIdentity;
        use source::{SourceOrigin, SourceSpan};
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::{
            TypedTrees,
            expression::{ExpressionHandle, ExpressionNode},
            machine::Machine,
            state::State,
            statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
        };

        use super::{BuildTimeCallEdge, BuildTimeInvocationCustody, BuildTimeSelectionAuthority};
        use typed_trees::AuthoredDeclarationSelectionTarget;
        use typed_trees_to_checked_trees::typed_operator_authored_selection_candidates;
        use validation::compiler_operations::build_product_request_intrinsic;
        use validation::has_builtin_binary_expression_meaning;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct SelectionViolation {
            pub(crate) message: String,
            pub(crate) source_span: Option<SourceSpan>,
        }

        impl SelectionViolation {
            fn unlocated(message: String) -> Self {
                Self {
                    message,
                    source_span: None,
                }
            }
        }

        impl From<SelectionViolation> for String {
            fn from(violation: SelectionViolation) -> Self {
                violation.message
            }
        }

        pub(super) fn selection_authority_violation(
            call_edges: &[BuildTimeCallEdge],
            input: &dyn validation::CheckingInput,
            root: &Machine,
            custody: Option<BuildTimeInvocationCustody>,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
            selected: &[crate::SelectedBuildTimeBinaryOperator],
            selected_providers: &[crate::SelectedBuildTimeProviderBody],
            checked_operators: Option<&checked_trees::CheckedOperatorFacts>,
        ) -> Option<SelectionViolation> {
            let program = input.typed_program();
            if let Some(authority) = authority {
                let Some(custody) = custody else {
                    return Some(SelectionViolation::unlocated(
                        "package-aware build-time evaluation has no authored invocation custody"
                            .to_owned(),
                    ));
                };
                let requester = match custody {
                    BuildTimeInvocationCustody::Source(source) => {
                        package_for_source(program, source)
                    }
                    BuildTimeInvocationCustody::Symbol(symbol) => {
                        package_for_symbol(program, symbol)
                    }
                };
                if let Some(violation) = require_selection(
                    program,
                    requester,
                    package_for_symbol(program, root.symbol),
                    authority,
                    &format!("build-time invocation of `{}`", root.name),
                ) {
                    return Some(SelectionViolation::unlocated(violation));
                }
            }

            super::walk_call_closure(call_edges, program, root.symbol, |source_machine| {
                if let Some(violation) = machine_selection_violation(
                    input,
                    source_machine.symbol,
                    authority,
                    selected,
                    checked_operators,
                ) {
                    return Some(violation);
                }
                for call in call_edges
                    .iter()
                    .filter(|call| call.source_machine_symbol == source_machine.symbol)
                {
                    let Some(target_machine) = call.target_machine(program) else {
                        let Some(authority) = authority else {
                            continue;
                        };
                        let Some(operator) = target_operator_symbol(program, call) else {
                            return Some(SelectionViolation::unlocated(format!(
                                "build-time call from `{}` has no exact target-machine identity",
                                program.symbols.display_path(source_machine.symbol, "::")
                            )));
                        };
                        let context = format!(
                            "build-time named operator call `{}` -> `{}`",
                            program.symbols.display_path(source_machine.symbol, "::"),
                            program.symbols.display_path(operator, "::")
                        );
                        if let Some(violation) = require_selection(
                            program,
                            package_for_symbol(program, source_machine.symbol),
                            package_for_symbol(program, operator),
                            authority,
                            &context,
                        ) {
                            return Some(SelectionViolation::unlocated(violation));
                        }
                        continue;
                    };
                    let context = format!(
                        "build-time call `{}` -> `{}`",
                        program.symbols.display_path(source_machine.symbol, "::"),
                        program.symbols.display_path(target_machine, "::")
                    );
                    let delegated = match program.static_machine_binding_selects_target(
                        source_machine.symbol,
                        call.static_machine_parameter,
                        target_machine,
                    ) {
                        Ok(delegated) => delegated,
                        Err(reason) => {
                            return Some(SelectionViolation::unlocated(reason.to_owned()));
                        }
                    };
                    let provider_delegated = selected_providers.iter().any(|row| {
                        row.origin.machine_symbol() == Some(source_machine.symbol)
                            && row.expression == call.expression
                            && row.provider_machine == target_machine
                            && row.provider_state == call.target_state_symbol
                    });
                    if !delegated
                        && !provider_delegated
                        && let Some(authority) = authority
                        && let Some(violation) = require_selection(
                            program,
                            package_for_symbol(program, source_machine.symbol),
                            package_for_symbol(program, target_machine),
                            authority,
                            &context,
                        )
                    {
                        return Some(SelectionViolation::unlocated(violation));
                    }
                }
                None
            })
        }

        fn machine_selection_violation(
            input: &dyn validation::CheckingInput,
            machine_symbol: SymbolHandle,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
            selected: &[crate::SelectedBuildTimeBinaryOperator],
            checked_operators: Option<&checked_trees::CheckedOperatorFacts>,
        ) -> Option<SelectionViolation> {
            let program = input.typed_program();
            let machine = program.machine(machine_symbol)?;
            for state in program.machine_states(machine) {
                let mut expressions = Vec::new();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    collect_statement_expressions(program, statement, &mut expressions);
                }

                let mut visited = Vec::new();
                for expression in expressions {
                    if let Some(violation) = expression_selection_violation(
                        input,
                        machine,
                        state,
                        expression,
                        authority,
                        selected,
                        checked_operators,
                        &mut visited,
                    ) {
                        return Some(violation);
                    }
                }
            }
            None
        }

        fn collect_statement_expressions(
            program: &TypedTrees,
            statement: &StatementNode,
            expressions: &mut Vec<ExpressionHandle>,
        ) {
            match statement {
                StatementNode::Assembly(_) => {}
                StatementNode::AssemblyFact(_) => {}
                StatementNode::Assignment(assignment) => {
                    expressions.push(assignment.target);
                    expressions.push(assignment.value);
                }
                StatementNode::Call(call) => expressions.extend(
                    program
                        .statement_table
                        .expression_handles(call.arguments)
                        .iter()
                        .copied(),
                ),
                StatementNode::Expression(expression) => expressions.push(*expression),
                StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                    expressions.push(local.initial_value)
                }
                StatementNode::LocalData(_) => {}
                StatementNode::Transition(transition) => {
                    if let TransitionGuardNode::When(guard) = transition.guard {
                        expressions.push(guard);
                    }
                    collect_transition_expressions(program, transition.target, expressions);
                    if transition.continuation.is_valid() {
                        collect_transition_expressions(
                            program,
                            transition.continuation,
                            expressions,
                        );
                    }
                }
            }
        }

        fn collect_transition_expressions(
            program: &TypedTrees,
            target: typed_trees::statement::TransitionTargetHandle,
            expressions: &mut Vec<ExpressionHandle>,
        ) {
            if !target.is_valid() {
                return;
            }
            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => expressions.extend(
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .iter()
                        .copied(),
                ),
                TransitionTargetNode::Value(expression) => expressions.push(*expression),
                TransitionTargetNode::Terminal => {}
            }
        }

        fn expression_selection_violation(
            input: &dyn validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
            selected: &[crate::SelectedBuildTimeBinaryOperator],
            checked_operators: Option<&checked_trees::CheckedOperatorFacts>,
            visited: &mut Vec<ExpressionHandle>,
        ) -> Option<SelectionViolation> {
            let program = input.typed_program();
            if !expression.is_valid() || visited.contains(&expression) {
                return None;
            }
            visited.push(expression);

            if matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Binary(_)
            ) && !has_builtin_binary_expression_meaning(input, machine, Some(state), expression)
                && !selected.iter().any(|row| {
                    row.expression == expression
                        && row.origin.machine_symbol() == Some(machine.symbol)
                })
                && !checked_operators.is_some_and(|facts| {
                    facts
                        .selected_trait_candidate_in_machine(expression, machine.symbol)
                        .is_some()
                })
            {
                return Some(SelectionViolation {
                    message: format!(
                        "build-time binary operator requires exact authored selection before evaluation; the evaluator cannot execute it as a builtin operator (machine `{}`, state `{}`, expression {expression:?})",
                        machine.name, state.name
                    ),
                    source_span: Some(program.expression_table.source_span(expression)),
                });
            }

            if let Some(violation) = expression_occurrence_violation(input, expression, authority) {
                return Some(violation);
            }

            let children = expression_children(program, expression);
            for child in children {
                if let Some(violation) = expression_selection_violation(
                    input,
                    machine,
                    state,
                    child,
                    authority,
                    selected,
                    checked_operators,
                    visited,
                ) {
                    return Some(violation);
                }
            }
            None
        }

        pub(crate) fn expression_children(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Vec<ExpressionHandle> {
            let mut children = Vec::new();
            ::validation::expression_traversal::children(
                program,
                program.expression_table.expression(expression),
                |child| {
                    if child.is_valid() {
                        children.push(child);
                    }
                },
            );
            children
        }
        pub(crate) fn require_call_expression_selection(
            program: &TypedTrees,
            expression: ExpressionHandle,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
        ) -> Result<(), String> {
            let Some(call) = program.expression_table.call_node(expression) else {
                return Err("range endpoint lost its original call selection".to_owned());
            };
            if let Some(violation) = expression_occurrence_violation(program, expression, authority)
            {
                return Err(violation.message);
            }
            if call.receiver.is_valid()
                && let Some(violation) =
                    expression_occurrence_violation(program, call.receiver, authority)
            {
                return Err(violation.message);
            }
            Ok(())
        }

        pub(crate) fn require_closed_expression_custody(
            program: &TypedTrees,
            expression: ExpressionHandle,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
        ) -> Result<(), String> {
            let mut pending = vec![expression];
            let mut visited = Vec::new();
            while let Some(expression) = pending.pop() {
                if !program.expression_table.expression_is_valid(expression) {
                    return Err("invalid constant selection expression".into());
                }
                if visited.contains(&expression) {
                    continue;
                }
                visited.push(expression);
                if let Some(violation) =
                    expression_occurrence_violation(program, expression, authority)
                {
                    return Err(violation.message);
                }
                pending.extend(expression_children(program, expression).into_iter().rev());
            }
            Ok(())
        }

        fn expression_occurrence_violation(
            input: &dyn validation::CheckingInput,
            expression: ExpressionHandle,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
        ) -> Option<SelectionViolation> {
            let program = input.typed_program();
            if let Some(authority) = authority {
                let occurrences = expression_table(program)
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>();
                for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
                    let Some(selection) = input.authored_declaration_selections().get(occurrence)
                    else {
                        return Some(SelectionViolation {
                            message: format!(
                                "build-time expression retains unknown authored declaration selection occurrence {}",
                                occurrence.ordinal()
                            ),
                            source_span: Some(program.expression_table.source_span(expression)),
                        });
                    };
                    let located = |message: String| SelectionViolation {
                        message,
                        source_span: Some(selection.source_span()),
                    };
                    match build_product_request_intrinsic(input, expression, selection) {
                        Ok(Some(expected)) => {
                            if selection.target()
                                != AuthoredDeclarationSelectionTarget::Intrinsic(expected)
                            {
                                return Some(located(
                            "Build request occurrence differs from its source-issued inert meaning"
                                .to_owned(),
                        ));
                            }
                            continue;
                        }
                        Ok(None) => {}
                        Err(reason) => return Some(located(reason)),
                    }
                    if validation::compiler_operations::is_build_declaration_selection(
                        input, expression,
                    ) {
                        use typed_trees::AuthoredDeclarationSelectionKind as Kind;
                        if selection.kind() == Kind::Call {
                            continue;
                        }
                        if matches!(selection.kind(), Kind::StaticArgument | Kind::Conformance)
                            && let Some(argument_index) =
                                selection.top_level_static_argument_position()
                        {
                            let expected =
                                validation::compiler_operations::resolve_build_static_operand(
                                    input,
                                    expression,
                                    argument_index,
                                    selection.source_span(),
                                );
                            let expected = match expected {
                                Ok(expected) => expected,
                                Err(message) => return Some(located(message)),
                            };
                            use validation::selection_resolution::AuthoredSelectionResolutionTarget as Resolution;
                            let retained = match selection.target() {
                                AuthoredDeclarationSelectionTarget::Resolved(target) => {
                                    Some(Resolution::Declaration(target.selected_symbol()))
                                }
                                AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => {
                                    Some(Resolution::Intrinsic(intrinsic))
                                }
                                AuthoredDeclarationSelectionTarget::LateBound(_) => None,
                            };
                            if retained != Some(expected) {
                                return Some(located("Build static operand occurrence does not retain its exact checked meaning".to_owned()));
                            }
                            continue;
                        }
                    }
                    let requester = package_for_source(program, selection.source_span());
                    let owner = match selection.target() {
                AuthoredDeclarationSelectionTarget::Intrinsic(_) => continue,
                AuthoredDeclarationSelectionTarget::LateBound(binding) => {
                    if binding
                        == typed_trees::AuthoredDeclarationSelectionLateBinding::CheckedOperator
                        && (typed_trees_to_checked_trees::typed_operator_has_no_authored_selection(
                            program, expression,
                        ) || unresolved_operator_candidates_are_confined(
                            program, expression, requester, authority,
                        ))
                    {
                        continue;
                    } else {
                        match late_bound_selection_symbol(
                            input,
                            expression,
                            &occurrences[..occurrence_offset],
                            binding,
                        ) {
                            Some(selected) => package_for_symbol(program, selected),
                            None => {
                                match unresolved_spelling_confinement(
                                    program,
                                    program.symbols.source_text(selection.source_span()),
                                    requester,
                                    authority,
                                    binding,
                                ) {
                                    Ok(()) => continue,
                                    Err(unconfined) => {
                                        return Some(located(format!(
                                            "build-time expression has unresolved authored {:?} selection `{}` ({binding:?}) with no derivable exact owner, so its package authority is confined by spelling across the program and {unconfined}; package authority must be known before compiler execution",
                                            selection.kind(),
                                            program.symbols.source_text(selection.source_span()),
                                        )));
                                    }
                                }
                            }
                        }
                    }
                }
                AuthoredDeclarationSelectionTarget::Resolved(selected) => {
                    if program
                        .symbols
                        .builtin_type_atom(selected.selected_symbol())
                        .is_some()
                    {
                        continue;
                    }
                    package_for_symbol(program, selected.selected_symbol())
                }
            };
                    let context = format!("build-time authored {:?} selection", selection.kind());
                    if let Some(violation) =
                        require_selection(program, requester, owner, authority, &context)
                    {
                        return Some(located(violation));
                    }
                }
            }

            None
        }

        fn unresolved_operator_candidates_are_confined(
            program: &TypedTrees,
            expression: ExpressionHandle,
            requester: PackageCustody,
            authority: &dyn BuildTimeSelectionAuthority,
        ) -> bool {
            let candidates = typed_operator_authored_selection_candidates(program, expression);
            candidates.into_iter().all(|candidate| {
                require_selection(
                    program,
                    requester,
                    package_for_symbol(program, candidate),
                    authority,
                    "candidate for an unresolved build-time operator selection",
                )
                .is_none()
            })
        }

        fn unresolved_spelling_confinement(
            program: &TypedTrees,
            spelling: &str,
            requester: PackageCustody,
            authority: &dyn BuildTimeSelectionAuthority,
            binding: typed_trees::AuthoredDeclarationSelectionLateBinding,
        ) -> Result<(), String> {
            use typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

            if !matches!(
                binding,
                Binding::CheckedCall
                    | Binding::CheckedMember
                    | Binding::CheckedStaticPathSegment
                    | Binding::CheckedStructLiteralType
                    | Binding::CheckedStructLiteralCase
                    | Binding::CheckedStructLiteralField
            ) {
                return Err(format!(
                    "a {binding:?} occurrence has no spelling-confinement route"
                ));
            }
            let candidates = program
                .symbols
                .symbols()
                .nodes()
                .iter()
                .filter_map(|(symbol, data)| {
                    (program.symbols.name(symbol) == spelling
                        && candidate_kind_matches_binding(data.kind, binding))
                    .then_some(symbol)
                })
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                return Err(format!(
                    "no declaration spelled `{spelling}` exists in the program"
                ));
            }
            for candidate in candidates {
                if let Some(violation) = require_selection(
                    program,
                    requester,
                    package_for_symbol(program, candidate),
                    authority,
                    "candidate for an unresolved build-time declaration selection",
                ) {
                    return Err(format!(
                        "`{}` is not: {violation}",
                        program.symbols.display_path(candidate, "::")
                    ));
                }
            }
            Ok(())
        }

        fn candidate_kind_matches_binding(
            kind: SymbolKind,
            binding: typed_trees::AuthoredDeclarationSelectionLateBinding,
        ) -> bool {
            use typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

            match binding {
                Binding::CheckedCall => matches!(
                    kind,
                    SymbolKind::BuiltinFunction
                        | SymbolKind::Function
                        | SymbolKind::Machine
                        | SymbolKind::MachineParameter
                        | SymbolKind::State
                        | SymbolKind::Trait
                        | SymbolKind::ConformanceParameter
                ),
                Binding::CheckedMember | Binding::CheckedStructLiteralField => {
                    matches!(kind, SymbolKind::Field | SymbolKind::State)
                }
                Binding::CheckedStaticPathSegment => !matches!(
                    kind,
                    SymbolKind::Root
                        | SymbolKind::Local
                        | SymbolKind::Parameter
                        | SymbolKind::TypeParameter
                        | SymbolKind::ConformanceParameter
                        | SymbolKind::MachineParameter
                        | SymbolKind::PropositionParameter
                        | SymbolKind::PropositionMachineParameter
                ),
                Binding::CheckedStructLiteralType => {
                    matches!(kind, SymbolKind::BuiltinType | SymbolKind::Data)
                }
                Binding::CheckedStructLiteralCase => matches!(kind, SymbolKind::Variant),
                Binding::CheckedCaseMembership
                | Binding::CheckedDomainMembership
                | Binding::CheckedStaticArgument
                | Binding::CheckedOperator
                | Binding::CheckedConformance => false,
            }
        }

        fn late_bound_selection_symbol(
            input: &dyn validation::CheckingInput,
            expression: ExpressionHandle,
            prior_occurrences: &[typed_trees::AuthoredDeclarationSelectionOccurrenceId],
            binding: typed_trees::AuthoredDeclarationSelectionLateBinding,
        ) -> Option<SymbolHandle> {
            let program = input.typed_program();
            use typed_trees::AuthoredDeclarationSelectionLateBinding as Binding;

            let ordinal = prior_occurrences
                .iter()
                .filter(|occurrence| {
                    input
                        .authored_declaration_selections()
                        .get(**occurrence)
                        .is_some_and(|selection| {
                            selection.target()
                                == AuthoredDeclarationSelectionTarget::LateBound(binding)
                        })
                })
                .count();
            let table = &program.expression_table;
            let selected = match (binding, table.expression(expression)) {
                (Binding::CheckedStaticArgument, ExpressionNode::Call(call))
                    if BuildOperation::from_call_target(call.target.as_str())
                        == Some(BuildOperation::ProviderSelection) =>
                {
                    call.machine_arguments
                        .get(ordinal)
                        .map(|argument| argument.symbol)
                        .unwrap_or_default()
                }
                (Binding::CheckedCall, ExpressionNode::Call(call)) => {
                    if call.target_symbol.is_valid() {
                        call.target_symbol
                    } else {
                        typed_trees::operator::resolve_named_expression_call(program, call)
                            .map(|operator| operator.symbol)
                            .unwrap_or_else(SymbolHandle::invalid)
                    }
                }
                (Binding::CheckedMember, ExpressionNode::Member(member)) => {
                    if member.member_symbol.is_valid() {
                        member.member_symbol
                    } else {
                        typed_trees_to_checked_trees::late_bound_member_declaration_from_exact_owner(
                    program, expression,
                )
                .unwrap_or_else(SymbolHandle::invalid)
                    }
                }
                (Binding::CheckedStaticPathSegment, ExpressionNode::Name(path)) => table
                    .name_path_member_symbols(path.member_symbols)
                    .get(ordinal)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid),
                (Binding::CheckedStructLiteralType, ExpressionNode::StructLiteral(literal)) => {
                    literal.type_symbol
                }
                (Binding::CheckedStructLiteralCase, ExpressionNode::StructLiteral(literal)) => {
                    literal.case_symbol.unwrap_or_else(SymbolHandle::invalid)
                }
                (Binding::CheckedStructLiteralField, ExpressionNode::StructLiteral(literal)) => {
                    table
                        .struct_fields(literal.fields)
                        .get(ordinal)
                        .map(|field| field.field_symbol)
                        .unwrap_or_else(SymbolHandle::invalid)
                }
                (
                    Binding::CheckedOperator,
                    ExpressionNode::Binary(_) | ExpressionNode::Unary(_),
                ) => SymbolHandle::invalid(),
                _ => SymbolHandle::invalid(),
            };
            selected.is_valid().then_some(selected)
        }

        fn target_operator_symbol(
            program: &TypedTrees,
            call: &BuildTimeCallEdge,
        ) -> Option<SymbolHandle> {
            (call.target_operator_symbol.is_valid()
                && typed_trees::operator::declaration_by_symbol(
                    program,
                    call.target_operator_symbol,
                )
                .is_some())
            .then_some(call.target_operator_symbol)
        }

        #[derive(Debug, Clone, Copy)]
        enum PackageCustody {
            Toolchain,
            Package(PackageKeyIdentity),
            UnownedUser,
            Missing,
        }

        fn package_for_source(program: &TypedTrees, source: SourceSpan) -> PackageCustody {
            match program.symbols.source_file(source) {
                Some(file) if file.origin == SourceOrigin::Toolchain => PackageCustody::Toolchain,
                Some(file) => file
                    .package_identity
                    .map(PackageCustody::Package)
                    .unwrap_or(PackageCustody::UnownedUser),
                None => PackageCustody::Missing,
            }
        }

        fn package_for_symbol(program: &TypedTrees, symbol: SymbolHandle) -> PackageCustody {
            if let Some(identity) = program.symbols.symbol_package_identity(symbol) {
                return PackageCustody::Package(identity);
            }
            match program.symbols.symbol_source_origin(symbol) {
                Some(SourceOrigin::Toolchain) => PackageCustody::Toolchain,
                Some(SourceOrigin::User) => PackageCustody::UnownedUser,
                None => PackageCustody::Missing,
            }
        }

        fn require_selection(
            _program: &TypedTrees,
            requester: PackageCustody,
            owner: PackageCustody,
            authority: &dyn BuildTimeSelectionAuthority,
            context: &str,
        ) -> Option<String> {
            match (requester, owner) {
                (PackageCustody::Toolchain, _) | (_, PackageCustody::Toolchain) => None,
                (PackageCustody::Package(requester), PackageCustody::Package(owner))
                    if authority.allows_declaration_selection(requester, owner) =>
                {
                    None
                }
                (PackageCustody::Package(requester), PackageCustody::Package(owner)) => {
                    Some(format!(
                        "{context} selects package {} from package {} without direct dependency authority",
                        authority.package_label(owner),
                        authority.package_label(requester),
                    ))
                }
                (PackageCustody::UnownedUser, _) | (_, PackageCustody::UnownedUser) => Some(
                    format!("{context} has user source without reconciled package custody"),
                ),
                (PackageCustody::Missing, _) | (_, PackageCustody::Missing) => Some(format!(
                    "{context} lacks compiler-owned source/package provenance"
                )),
            }
        }
    }

    use closure_validation::checked_closure_violation;
    use const_evaluable::{relabel_result, require_const_evaluable_result};
    use selection_authority::selection_authority_violation;
    pub(crate) use selection_authority::{
        expression_children, require_call_expression_selection, require_closed_expression_custody,
    };

    pub use validation::DeclarationSelectionAuthority as BuildTimeSelectionAuthority;

    pub(crate) fn require_const_expression_selection(
        program: &TypedTrees,
        machine: &Machine,
        source: source::SourceSpan,
        authority: Option<&dyn BuildTimeSelectionAuthority>,
    ) -> Result<(), String> {
        match selection_authority_violation(
            &[],
            program,
            machine,
            Some(BuildTimeInvocationCustody::Source(source)),
            authority,
            &[],
            &[],
            None,
        ) {
            Some(violation) => Err(violation.message),
            None => Ok(()),
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BuildTimeAdmissionRejection {
        pub reason: String,
        pub source_span: Option<source::SourceSpan>,
    }

    impl From<String> for BuildTimeAdmissionRejection {
        fn from(reason: String) -> Self {
            Self {
                reason,
                source_span: None,
            }
        }
    }

    impl From<BuildTimeAdmissionRejection> for String {
        fn from(rejection: BuildTimeAdmissionRejection) -> Self {
            rejection.reason
        }
    }

    impl std::fmt::Display for BuildTimeAdmissionRejection {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(&self.reason)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BuildTimeInvocationCustody {
        Source(source::SourceSpan),
        Symbol(SymbolHandle),
    }

    #[derive(Clone, Copy)]
    pub enum BuildTimeProgram<'program> {
        Typed(&'program TypedTrees),
        Checked(&'program CheckedTrees),
    }

    impl<'program> BuildTimeProgram<'program> {
        fn checking_input(self) -> &'program dyn validation::CheckingInput {
            match self {
                Self::Typed(program) => program,
                Self::Checked(program) => program,
            }
        }

        fn operator_facts(self) -> Option<&'program checked_trees::CheckedOperatorFacts> {
            match self {
                Self::Typed(_) => None,
                Self::Checked(program) => Some(&program.facts.operators),
            }
        }

        fn typed(self) -> &'program TypedTrees {
            self.checking_input().typed_program()
        }
    }

    pub struct BuildTimeAdmissionPlan {
        service_reaches: flow_effects::ServiceReachInferencePlan,
        suspension: Vec<BuildTimeSuspensionRow>,
        blocking: Vec<BuildTimeBlockingRow>,
        call_edges: Vec<BuildTimeCallEdge>,
        pub(crate) operators: checked_trees::CheckedOperatorFacts,
        selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        selected_operators: Vec<crate::SelectedBuildTimeBinaryOperator>,
        selected_provider_bodies: Vec<crate::SelectedBuildTimeProviderBody>,
        selected_execution: Option<TypedTrees>,
        pub(crate) accounting: EvaluationAccounting,
        checked_closures: std::cell::RefCell<
            Vec<(
                (usize, usize, usize),
                crate::build_time_evaluation::EvaluationDomain,
                Result<std::rc::Rc<(CheckedTrees, Self)>, String>,
            )>,
        >,
    }

    enum RecursiveCheckingInput<'input> {
        Borrowed(&'input CheckedTrees),
        Demanded(std::rc::Rc<(CheckedTrees, BuildTimeAdmissionPlan)>),
        Selected(Box<CheckedTrees>),
    }

    impl RecursiveCheckingInput<'_> {
        fn checked(&self) -> &CheckedTrees {
            match self {
                Self::Borrowed(checked) => checked,
                Self::Demanded(demanded) => &demanded.0,
                Self::Selected(checked) => checked,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct BuildTimeSuspensionRow {
        machine_symbol: SymbolHandle,
        transitive_may_suspend: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct BuildTimeBlockingRow {
        machine_symbol: SymbolHandle,
        transitive_may_block: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct BuildTimeCallEdge {
        source_machine_symbol: SymbolHandle,
        static_machine_parameter: SymbolHandle,
        target_machine_symbol: SymbolHandle,
        target_state_symbol: SymbolHandle,
        target_operator_symbol: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
    }

    impl BuildTimeCallEdge {
        fn target_machine(&self, program: &TypedTrees) -> Option<SymbolHandle> {
            if self.target_machine_symbol.is_valid() {
                return Some(self.target_machine_symbol);
            }
            (self.target_state_symbol.is_valid()
                && program.symbols.get(self.target_state_symbol).kind
                    == symbols::SymbolKind::Machine)
                .then_some(self.target_state_symbol)
        }
    }

    fn walk_call_closure<Outcome>(
        call_edges: &[BuildTimeCallEdge],
        program: &TypedTrees,
        root: SymbolHandle,
        mut visit: impl FnMut(&Machine) -> Option<Outcome>,
    ) -> Option<Outcome> {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(symbol) = crate::worklist::unvisited(&mut pending, &mut visited) {
            let Some(machine) = program.machine(symbol) else {
                continue;
            };
            if let Some(outcome) = visit(machine) {
                return Some(outcome);
            }
            pending.extend(
                call_edges
                    .iter()
                    .filter(|call| call.source_machine_symbol == symbol)
                    .filter_map(|call| call.target_machine(program)),
            );
        }
        None
    }

    impl BuildTimeAdmissionPlan {
        pub(crate) fn with_accounting(mut self, accounting: EvaluationAccounting) -> Self {
            self.accounting = accounting;
            self
        }
        pub fn infer(
            program: &TypedTrees,
            selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
        ) -> Self {
            let operational = validation::infer_operational_may(program);
            let service_reaches = validation::infer_service_reaches(program, &operational);
            let operators = validation::derive_pre_flow_operator_selections(program);
            let (suspension, blocking, call_edges) =
                Self::project_axes_and_operator_edges(program, &operational, &operators);
            Self {
                service_reaches,
                suspension,
                blocking,
                call_edges,
                operators,
                selection_authority,
                selected_operators: Vec::new(),
                selected_provider_bodies: Vec::new(),
                selected_execution: None,
                accounting: Default::default(),
                checked_closures: Default::default(),
            }
        }

        pub(crate) fn infer_execution_dependencies(
            program: &TypedTrees,
            operators: &checked_trees::CheckedOperatorFacts,
            execution: &mut Vec<SymbolHandle>,
        ) {
            let operational = validation::infer_operational_may(program);
            let (_, _, call_edges) =
                Self::project_axes_and_operator_edges(program, &operational, operators);
            Self::append_edges_to_execution_dependencies(&call_edges, program, execution);
        }

        fn project_axes_and_operator_edges(
            program: &TypedTrees,
            operational: &flow_effects::OperationalPlan,
            operators: &checked_trees::CheckedOperatorFacts,
        ) -> (
            Vec<BuildTimeSuspensionRow>,
            Vec<BuildTimeBlockingRow>,
            Vec<BuildTimeCallEdge>,
        ) {
            let (suspension, blocking, mut call_edges) =
                project_operational_axes(program, operational);
            for (expression, origin, _, _) in operators.resolved_expression_selections() {
                let Some(owner) = origin.machine_symbol() else {
                    continue;
                };
                let Some(candidate) =
                    operators.selected_trait_candidate_in_machine(expression, owner)
                else {
                    continue;
                };
                if !candidate.realization_machine_symbol.is_valid()
                    || !candidate.realization_state_symbol.is_valid()
                {
                    continue;
                };
                call_edges.push(BuildTimeCallEdge {
                    source_machine_symbol: owner,
                    static_machine_parameter: SymbolHandle::invalid(),
                    target_machine_symbol: candidate.realization_machine_symbol,
                    target_state_symbol: candidate.realization_state_symbol,
                    target_operator_symbol: candidate.trait_requirement_symbol,
                    expression,
                });
            }
            (suspension, blocking, call_edges)
        }

        pub(crate) fn require_closed_static_applications(
            &self,
            program: &TypedTrees,
            root: SymbolHandle,
        ) -> Result<(), String> {
            walk_call_closure(&self.call_edges, program, root, |machine| {
                if machine.structural_type_equations_pending {
                    return Some(format!(
                        "machine `{}` retains an undischarged structural type equation",
                        machine.name
                    ));
                }
                (!machine.type_parameters.is_empty()).then(|| {
                    format!(
                        "machine `{}` retains an unspecialized static application",
                        machine.name
                    )
                })
            })
            .map_or(Ok(()), Err)
        }

        fn closure_machine_symbols(
            &self,
            program: &TypedTrees,
            root: SymbolHandle,
        ) -> Vec<SymbolHandle> {
            let mut closure = Vec::new();
            let _ = walk_call_closure(&self.call_edges, program, root, |machine| {
                closure.push(machine.symbol);
                None::<()>
            });
            closure
        }

        pub(crate) fn append_execution_dependencies(
            &self,
            program: &TypedTrees,
            execution: &mut Vec<SymbolHandle>,
        ) {
            Self::append_edges_to_execution_dependencies(&self.call_edges, program, execution);
        }

        pub fn execution_dependencies(
            &self,
            program: &TypedTrees,
            roots: &[SymbolHandle],
        ) -> Vec<SymbolHandle> {
            let mut execution = roots.to_vec();
            self.append_execution_dependencies(program, &mut execution);
            execution
        }

        fn with_selected_provider_bodies(
            mut self,
            program: &TypedTrees,
            bodies: &[crate::SelectedBuildTimeProviderBody],
        ) -> Result<Self, String> {
            if !self.selected_provider_bodies.is_empty() {
                return Err("build-time execution bodies are already settled".to_owned());
            }
            if let Some(execution) = super::selected_operators::prepare_selected_execution(
                program,
                bodies,
                &self.selected_operators,
            )? {
                let operational = validation::infer_operational_may(&execution);
                let operators = validation::derive_pre_flow_operator_selections(&execution);
                self.call_edges =
                    Self::project_axes_and_operator_edges(&execution, &operational, &operators).2;
                self.selected_execution = Some(execution);
            }
            self.selected_provider_bodies = bodies.to_vec();
            self.checked_closures.borrow_mut().clear();
            Ok(self)
        }

        fn append_edges_to_execution_dependencies(
            call_edges: &[BuildTimeCallEdge],
            program: &TypedTrees,
            execution: &mut Vec<SymbolHandle>,
        ) {
            let mut position = 0;
            while let Some(&machine) = execution.get(position) {
                position += 1;
                for edge in call_edges
                    .iter()
                    .filter(|edge| edge.source_machine_symbol == machine)
                {
                    if let Some(target) = edge.target_machine(program) {
                        push_unique(execution, target);
                    }
                }
            }
        }

        pub fn with_selected_operators(
            mut self,
            program: &TypedTrees,
            operators: &[crate::SelectedBuildTimeBinaryOperator],
        ) -> Result<Self, String> {
            self.selected_operators =
                super::selected_operators::retain_specialized_operators(program, operators)?;
            self.checked_closures.borrow_mut().clear();
            Ok(self)
        }

        pub(crate) fn closure_needs_execution_preparation(
            &self,
            program: &TypedTrees,
            root: SymbolHandle,
            facts: &checked_trees::CheckedOperatorFacts,
        ) -> bool {
            let mut pending = vec![root];
            let mut visited = Vec::new();
            while let Some(machine) = crate::worklist::unvisited(&mut pending, &mut visited) {
                if program
                    .machine_by_symbol(machine)
                    .is_some_and(|declaration| {
                        declaration.supplies_checked_body()
                            && !declaration.conformance_bounds.is_empty()
                    })
                {
                    return true;
                }
                if program
                    .target_machine_families
                    .iter()
                    .any(|family| family.canonical == machine && !family.members.is_empty())
                {
                    return true;
                }
                if program.machines().iter().any(|declaration| {
                    declaration.symbol == machine
                        && declaration.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                }) {
                    return true;
                }
                let boundary_use = |selected_operator_symbol: SymbolHandle| {
                    if !selected_operator_symbol.is_valid() {
                        return false;
                    }
                    program
                        .trait_machine_signature(selected_operator_symbol)
                        .is_some_and(|(owner, _)| owner.is_boundary)
                        || program
                            .operators()
                            .iter()
                            .chain(program.machine_token_bindings())
                            .any(|operator| {
                                operator.symbol == selected_operator_symbol && operator.is_boundary
                            })
                };
                if facts
                    .resolved_expression_selections()
                    .any(|(_, origin, requirement, _)| {
                        origin.machine_symbol() == Some(machine) && boundary_use(requirement)
                    })
                {
                    return true;
                }
                if self.call_edges.iter().any(|edge| {
                    edge.source_machine_symbol == machine
                        && (boundary_use(edge.target_operator_symbol)
                            || boundary_use(edge.target_state_symbol))
                }) {
                    return true;
                }
                pending.extend(
                    self.call_edges
                        .iter()
                        .filter(|edge| edge.source_machine_symbol == machine)
                        .filter_map(|edge| edge.target_machine(program)),
                );
            }
            let domain =
                PreparedEvaluationInputs::domain_for_admission(program, self, &[root], &[]);
            has_pending_target_observations(program, domain.work())
        }

        pub fn require_common_floor(
            &self,
            program: &TypedTrees,
            machine: &Machine,
        ) -> Result<(), BuildTimeAdmissionRejection> {
            self.require_floor(BuildTimeProgram::Typed(program), machine, None, false)
        }

        pub fn require_common_floor_for_invocation(
            &self,
            program: &TypedTrees,
            machine: &Machine,
            custody: BuildTimeInvocationCustody,
        ) -> Result<(), BuildTimeAdmissionRejection> {
            self.require_checked_invocation_floor(program, machine, custody, false)
        }

        pub(crate) fn require_common_floor_for_concrete_premise_invocation(
            &self,
            program: &TypedTrees,
            machine: &Machine,
            custody: BuildTimeInvocationCustody,
        ) -> Result<(), BuildTimeAdmissionRejection> {
            self.require_checked_invocation_floor(program, machine, custody, true)
        }

        pub(super) fn checked_closure_admission(
            &self,
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
        ) -> Result<std::rc::Rc<(CheckedTrees, Self)>, String> {
            crate::validate_selected_operators(program, &self.selected_operators)?;
            crate::validate_selected_provider_bodies(program, &self.selected_provider_bodies)?;
            let program_key = (
                std::ptr::from_ref(program) as usize,
                program.machines().len(),
                program.data_definitions().len(),
            );
            let mut roots = vec![machine_symbol];
            for family in &program.target_machine_families {
                if family.canonical == machine_symbol
                    || family
                        .members
                        .iter()
                        .any(|member| member.machine == machine_symbol)
                {
                    push_unique(&mut roots, family.canonical);
                    for member in &family.members {
                        push_unique(&mut roots, member.machine);
                    }
                }
            }
            let domain = PreparedEvaluationInputs::domain_for_admission(program, self, &roots, &[]);
            if let Some(cached) =
                self.checked_closures
                    .borrow()
                    .iter()
                    .find_map(|(key, cached_domain, result)| {
                        (*key == program_key && *cached_domain == domain).then(|| result.clone())
                    })
            {
                let pair = cached?;
                concrete_invocation::require_complete_invocation_conditions(
                    &pair.0,
                    machine_symbol,
                )?;
                return Ok(pair);
            }
            let computed = (|| {
                let checked = concrete_invocation::checked_demanded_closure(&domain, program)?;
                let admission = self.checked_program_admission(&checked)?;
                Ok(std::rc::Rc::new((checked, admission)))
            })();
            self.checked_closures
                .borrow_mut()
                .push((program_key, domain, computed.clone()));
            let pair = computed?;
            concrete_invocation::require_complete_invocation_conditions(&pair.0, machine_symbol)?;
            Ok(pair)
        }

        fn checked_program_admission(&self, checked: &CheckedTrees) -> Result<Self, String> {
            let execution = checked.typed.as_ref();
            let references = typed_trees::references::collect_program_references(execution);
            let operators = self
                .selected_operators
                .iter()
                .filter(|row| {
                    row.origin
                        .machine_symbol()
                        .is_some_and(|owner| execution.machine(owner).is_some())
                        && references.expressions.contains(&row.expression)
                })
                .cloned()
                .collect::<Vec<_>>();
            let provider_bodies = self
                .selected_provider_bodies
                .iter()
                .filter(|row| {
                    row.origin
                        .machine_symbol()
                        .is_some_and(|owner| execution.machine(owner).is_some())
                        && references.expressions.contains(&row.expression)
                })
                .cloned()
                .collect::<Vec<_>>();
            Self::infer(execution, self.selection_authority.clone())
                .with_selected_operators(execution, &operators)?
                .with_selected_provider_bodies(execution, &provider_bodies)
        }

        fn prepare_recursive_checking_input<'input>(
            &self,
            input: BuildTimeProgram<'input>,
            root: SymbolHandle,
        ) -> Result<RecursiveCheckingInput<'input>, String> {
            let prepared = match &self.selected_execution {
                Some(execution) => {
                    let admission = Self::infer(execution, self.selection_authority.clone());
                    let domain = PreparedEvaluationInputs::domain_for_admission(
                        execution,
                        &admission,
                        &[root],
                        &[],
                    );
                    let checked =
                        concrete_invocation::checked_demanded_closure(&domain, execution)?;
                    RecursiveCheckingInput::Selected(Box::new(checked))
                }
                None => match input {
                    BuildTimeProgram::Checked(checked) => RecursiveCheckingInput::Borrowed(checked),
                    BuildTimeProgram::Typed(program) => RecursiveCheckingInput::Demanded(
                        self.checked_closure_admission(program, root)?,
                    ),
                },
            };
            concrete_invocation::require_complete_invocation_conditions(prepared.checked(), root)?;
            Ok(prepared)
        }

        fn require_checked_invocation_floor(
            &self,
            program: &TypedTrees,
            machine: &Machine,
            custody: BuildTimeInvocationCustody,
            discharge_authored_requires: bool,
        ) -> Result<(), BuildTimeAdmissionRejection> {
            let pair = self.checked_closure_admission(program, machine.symbol)?;
            let (checked, admission) = pair.as_ref();
            let execution = checked.typed.as_ref();
            let root = execution
                .machine_by_symbol(machine.symbol)
                .ok_or_else(|| "checked invocation lost its exact declaration".to_owned())?;
            admission.require_floor(
                BuildTimeProgram::Checked(checked),
                root,
                Some(custody),
                discharge_authored_requires,
            )
        }

        pub(crate) fn closure_includes_authored_requires(
            &self,
            program: &TypedTrees,
            machine: &Machine,
        ) -> bool {
            closure_validation::closure_has_authored_requires(
                &self.call_edges,
                program,
                machine.symbol,
            )
        }

        fn zero_argument_invocation_discharge(
            &self,
            program: &TypedTrees,
            machine: &Machine,
            arguments: &[BuildTimeValue],
            custody: Option<BuildTimeInvocationCustody>,
        ) -> bool {
            arguments.is_empty()
                && self.closure_includes_authored_requires(program, machine)
                && concrete_invocation::zero_argument_invocation_discharges(
                    program,
                    machine,
                    custody.and_then(|custody| match custody {
                        BuildTimeInvocationCustody::Source(span) => Some(span),
                        BuildTimeInvocationCustody::Symbol(symbol) => {
                            program.symbols.symbol_source_span(symbol)
                        }
                    }),
                )
        }

        pub(crate) fn closure_requires_are_entry_parameter_domains(
            &self,
            program: &TypedTrees,
            machine: &Machine,
        ) -> bool {
            closure_validation::closure_requires_are_root_parameter_domains(
                &self.call_edges,
                program,
                machine.symbol,
            )
        }

        fn require_floor(
            &self,
            input: BuildTimeProgram<'_>,
            machine: &Machine,
            custody: Option<BuildTimeInvocationCustody>,
            discharge_authored_requires: bool,
        ) -> Result<(), BuildTimeAdmissionRejection> {
            self.require_floor_with_authority(
                input,
                machine,
                custody,
                discharge_authored_requires,
                self.selection_authority.as_deref(),
            )
        }

        pub(crate) fn require_invocation_with_authority(
            &self,
            program: &TypedTrees,
            machine: &Machine,
            custody: BuildTimeInvocationCustody,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
        ) -> Result<(), BuildTimeAdmissionRejection> {
            self.require_floor_with_authority(
                BuildTimeProgram::Typed(program),
                machine,
                Some(custody),
                false,
                authority,
            )
        }

        fn require_floor_with_authority(
            &self,
            input: BuildTimeProgram<'_>,
            machine: &Machine,
            custody: Option<BuildTimeInvocationCustody>,
            discharge_authored_requires: bool,
            authority: Option<&dyn BuildTimeSelectionAuthority>,
        ) -> Result<(), BuildTimeAdmissionRejection> {
            let program = input.typed();
            crate::validate_selected_operators(program, &self.selected_operators)?;
            crate::validate_selected_provider_bodies(program, &self.selected_provider_bodies)?;
            let mut services = Vec::new();
            let mut transitive_may_suspend = false;
            let mut transitive_may_block = false;
            for symbol in self.closure_machine_symbols(program, machine.symbol) {
                let summary = self.service_reaches.for_machine(symbol).ok_or_else(|| {
                format!(
                    "build-time closure machine {symbol:?} has no inferred service-reach summary"
                )
            })?;
                for service in self.service_reaches.services(summary.effective) {
                    let definition = program.service_reaches.definition(*service).ok_or_else(|| {
                    format!("build-time closure machine {symbol:?} reaches an unknown canonical service identity")
                })?;
                    if !services.contains(&definition.name.as_str()) {
                        services.push(definition.name.as_str());
                    }
                }
                transitive_may_suspend |= self.machine_suspension(symbol).ok_or_else(|| {
                    format!("build-time closure machine {symbol:?} has no suspension summary")
                })?;
                transitive_may_block |= self.machine_blocking(symbol).ok_or_else(|| {
                    format!("build-time closure machine {symbol:?} has no blocking summary")
                })?;
            }
            self.require_closed_static_applications(program, machine.symbol)?;
            let discharge_authored_requires = discharge_authored_requires
                || (self.closure_includes_authored_requires(program, machine)
                    && concrete_invocation::argument_independent_closure_discharges(
                        self, input, machine,
                    )?);
            let execution = self.selected_execution.as_ref().unwrap_or(program);
            let execution_machine =
                execution.machine_by_symbol(machine.symbol).ok_or_else(|| {
                    "selected build-time execution lost its exact source machine".to_owned()
                })?;
            let execution_input: &dyn validation::CheckingInput =
                match self.selected_execution.as_ref() {
                    Some(execution) => execution,
                    None => input.checking_input(),
                };
            let closure_violation = checked_closure_violation(
                self,
                input,
                &self.call_edges,
                execution_input,
                execution_machine,
                discharge_authored_requires,
            );
            let selection_violation = selection_authority_violation(
                &self.call_edges,
                input.checking_input(),
                machine,
                custody,
                authority,
                &self.selected_operators,
                &self.selected_provider_bodies,
                input.operator_facts(),
            );
            let effects = semantic_vocabulary::compiler_evaluation::CompilerEvaluationEffects {
                services: services.into_iter().map(str::to_owned).collect(),
                may_suspend: transitive_may_suspend,
                may_block: transitive_may_block,
            };
            semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_floor(
                machine.name.as_str(),
                &effects,
                closure_violation.as_deref(),
                selection_violation
                    .as_ref()
                    .map(|violation| violation.message.as_str()),
            )
            .map_err(|reason| BuildTimeAdmissionRejection {
                reason,
                source_span: selection_violation.and_then(|violation| violation.source_span),
            })
        }

        pub fn evaluate_machine(
            &self,
            input: BuildTimeProgram<'_>,
            machine_name: &str,
            arguments: Vec<BuildTimeValue>,
        ) -> Result<BuildTimeValue, BuildTimeAdmissionRejection> {
            let program = input.typed();
            let machine = program
                .realized_machine_named(machine_name)
                .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
            let mut arguments = arguments;
            self.require_supplied_invocation(input, machine, &mut arguments, None)?;
            self.accounting
                .evaluate(
                    checked_interpreter::BuildMachineProgram::Typed(program),
                    BuildMachineEvaluationRequest {
                        ..BuildMachineEvaluationRequest::named(machine_name, arguments)
                    },
                )
                .map(BuildTimeOperationEvaluation::into_value)
                .map_err(|error| BuildTimeAdmissionRejection::from(error.to_string()))
        }

        pub fn evaluate_machine_symbol_for_invocation_measured(
            &self,
            input: BuildTimeProgram<'_>,
            machine_symbol: SymbolHandle,
            arguments: Vec<BuildTimeValue>,
            custody: BuildTimeInvocationCustody,
        ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
            let program = input.typed();
            let machine = program.machine(machine_symbol).ok_or_else(|| {
            format!(
                "no machine with exact symbol {machine_symbol:?} exists in the evaluated program"
            )
        })?;
            let mut arguments = arguments;
            self.require_supplied_invocation(input, machine, &mut arguments, Some(custody))?;
            self.accounting
                .evaluate(
                    checked_interpreter::BuildMachineProgram::Typed(program),
                    BuildMachineEvaluationRequest {
                        ..BuildMachineEvaluationRequest::symbol(machine_symbol, arguments)
                    },
                )
                .map_err(String::from)
                .and_then(|evaluation| {
                    crate::MeasuredEvaluation::from_checked(evaluation.into_measured())
                })
        }

        pub fn admitted_program_machine_closure_symbols(
            &self,
            input: BuildTimeProgram<'_>,
            machine_symbol: SymbolHandle,
            custody: BuildTimeInvocationCustody,
        ) -> Result<Vec<SymbolHandle>, String> {
            let program = input.typed();
            let machine = program.machine(machine_symbol).ok_or_else(|| {
            format!(
                "no machine with exact symbol {machine_symbol:?} exists in the evaluated program"
            )
        })?;
            match input {
                BuildTimeProgram::Typed(program) => {
                    self.require_common_floor_for_invocation(program, machine, custody)?;
                }
                BuildTimeProgram::Checked(checked) => {
                    concrete_invocation::require_complete_invocation_conditions(
                        checked,
                        machine.symbol,
                    )?;
                    self.require_floor(input, machine, Some(custody), false)?;
                }
            }

            let machine_symbols = program
                .machines()
                .iter()
                .map(|machine| machine.symbol)
                .collect::<Vec<_>>();
            let mut pending = VecDeque::from([machine_symbol]);
            let mut closure = Vec::new();
            while let Some(source) = pending.pop_front() {
                if closure.contains(&source) {
                    continue;
                }
                closure.push(source);
                for target in self
                    .call_edges
                    .iter()
                    .filter(|edge| edge.source_machine_symbol == source)
                    .map(|edge| edge.target_machine_symbol)
                {
                    if !target.is_valid() || !machine_symbols.contains(&target) {
                        return Err(format!(
                            "build-time machine closure rooted at `{}` contains an unresolved machine target",
                            machine.name,
                        ));
                    }
                    if !closure.contains(&target) {
                        pending.push_back(target);
                    }
                }
            }
            Ok(closure)
        }

        pub fn evaluate_const_evaluable_machine(
            &self,
            program: &TypedTrees,
            machine_name: &str,
            arguments: Vec<BuildTimeValue>,
        ) -> Result<BuildTimeValue, String> {
            self.evaluate_const_evaluable_machine_for_invocation_opt(
                program,
                machine_name,
                arguments,
                None,
            )
        }

        fn evaluate_const_evaluable_machine_for_invocation_opt(
            &self,
            program: &TypedTrees,
            machine_name: &str,
            arguments: Vec<BuildTimeValue>,
            custody: Option<BuildTimeInvocationCustody>,
        ) -> Result<BuildTimeValue, String> {
            let machine = program
                .realized_machine_named(machine_name)
                .ok_or_else(|| format!("no machine named `{machine_name}` exists"))?;
            let mut arguments = arguments;
            let discharge =
                self.zero_argument_invocation_discharge(program, machine, &arguments, custody);
            self.require_floor(
                BuildTimeProgram::Typed(program),
                machine,
                custody,
                discharge,
            )?;
            self.require_invocation_failures(
                BuildTimeProgram::Typed(program),
                machine,
                &mut arguments,
                custody,
            )?;
            let value = self
                .accounting
                .evaluate(
                    checked_interpreter::BuildMachineProgram::Typed(program),
                    BuildMachineEvaluationRequest {
                        operators: &self.selected_operators,
                        ..BuildMachineEvaluationRequest::symbol(machine.symbol, arguments)
                    },
                )
                .map(BuildTimeOperationEvaluation::into_measured)?
                .into_parts()
                .0;
            require_const_evaluable_result(program, machine, &value)?;
            Ok(value)
        }

        pub fn evaluate_const_evaluable_machine_for_invocation(
            &self,
            program: &TypedTrees,
            machine_name: &str,
            arguments: Vec<BuildTimeValue>,
            custody: BuildTimeInvocationCustody,
        ) -> Result<BuildTimeValue, String> {
            self.evaluate_const_evaluable_machine_for_invocation_opt(
                program,
                machine_name,
                arguments,
                Some(custody),
            )
        }

        pub fn evaluate_const_evaluable_machine_symbol_for_invocation(
            &self,
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
            arguments: Vec<BuildTimeValue>,
            custody: BuildTimeInvocationCustody,
        ) -> Result<BuildTimeValue, String> {
            self.evaluate_const_evaluable_machine_symbol_for_invocation_measured(
                program,
                machine_symbol,
                arguments,
                custody,
            )
            .map(crate::MeasuredEvaluation::into_value)
        }

        pub(crate) fn evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
            &self,
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
            arguments: Vec<BuildTimeValue>,
            custody: BuildTimeInvocationCustody,
        ) -> Result<BuildTimeValue, String> {
            self.evaluate_const_evaluable_invocation_measured(
                program,
                machine_symbol,
                arguments,
                custody,
                true,
                None,
            )
            .map(crate::MeasuredEvaluation::into_value)
        }

        pub fn evaluate_const_evaluable_machine_symbol_for_invocation_measured(
            &self,
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
            arguments: Vec<BuildTimeValue>,
            custody: BuildTimeInvocationCustody,
        ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
            self.evaluate_const_evaluable_invocation_measured(
                program,
                machine_symbol,
                arguments,
                custody,
                false,
                None,
            )
        }

        /// `target` names the deployment target whose family members the
        /// invocation may run and whose facts discharge its retained scalar
        /// conditions; `None` refuses target-selected entries and any entry
        /// that retains such a condition.
        pub fn evaluate_const_evaluable_program_machine_symbol_for_invocation_measured(
            &self,
            input: BuildTimeProgram<'_>,
            machine_symbol: SymbolHandle,
            arguments: Vec<BuildTimeValue>,
            custody: BuildTimeInvocationCustody,
            target: Option<crate::InvocationTarget<'_>>,
        ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
            match input {
                BuildTimeProgram::Typed(program) => self
                    .evaluate_const_evaluable_invocation_measured(
                        program,
                        machine_symbol,
                        arguments,
                        custody,
                        false,
                        target,
                    ),
                BuildTimeProgram::Checked(checked) => {
                    let program = checked.typed.as_ref();
                    let matching: Vec<_> = program
                        .machines()
                        .iter()
                        .filter(|machine| machine.symbol == machine_symbol)
                        .collect();
                    let [machine] = matching.as_slice() else {
                        return Err("build-time invocation has no unique exact machine".into());
                    };
                    let mut arguments = arguments;
                    let discharge_authored_requires = self.zero_argument_invocation_discharge(
                        program,
                        machine,
                        &arguments,
                        Some(custody),
                    );
                    concrete_invocation::require_complete_invocation_conditions(
                        checked,
                        machine_symbol,
                    )?;
                    self.require_floor(
                        BuildTimeProgram::Checked(checked),
                        machine,
                        Some(custody),
                        discharge_authored_requires,
                    )?;
                    self.require_invocation_failures(
                        input,
                        machine,
                        &mut arguments,
                        Some(custody),
                    )?;
                    let measured =
                        crate::machine_execution::build_machines::evaluate_checked_invocation(
                            checked_interpreter::BuildMachineProgram::Checked(checked),
                            checked,
                            machine_symbol,
                            arguments,
                            &self.selected_operators,
                            target,
                            &self.accounting,
                        )?;
                    let measured =
                        measured.map_value(|value| relabel_result(program, machine, value));
                    require_const_evaluable_result(program, machine, measured.value())?;
                    Ok(measured)
                }
            }
        }

        fn evaluate_const_evaluable_invocation_measured(
            &self,
            program: &TypedTrees,
            machine_symbol: SymbolHandle,
            arguments: Vec<BuildTimeValue>,
            custody: BuildTimeInvocationCustody,
            discharge_authored_requires: bool,
            target: Option<crate::InvocationTarget<'_>>,
        ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
            let matching: Vec<_> = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == machine_symbol)
                .collect();
            let [machine] = matching.as_slice() else {
                return Err("build-time invocation has no unique exact machine".into());
            };
            let mut arguments = arguments;
            let discharge_authored_requires = discharge_authored_requires
                || self.zero_argument_invocation_discharge(
                    program,
                    machine,
                    &arguments,
                    Some(custody),
                );
            let pair = self.checked_closure_admission(program, machine_symbol)?;
            let (checked, admission) = pair.as_ref();
            let execution = checked.typed.as_ref();
            let machine = execution
                .machine_by_symbol(machine_symbol)
                .ok_or("checked constant invocation lost its exact machine")?;
            admission.require_floor(
                BuildTimeProgram::Checked(checked),
                machine,
                Some(custody),
                discharge_authored_requires,
            )?;
            admission.require_invocation_failures(
                BuildTimeProgram::Checked(checked),
                machine,
                &mut arguments,
                Some(custody),
            )?;
            let measured = crate::machine_execution::build_machines::evaluate_checked_invocation(
                checked_interpreter::BuildMachineProgram::Checked(checked),
                checked,
                machine_symbol,
                arguments,
                &admission.selected_operators,
                target,
                &self.accounting,
            )?;
            let measured = measured.map_value(|value| relabel_result(execution, machine, value));
            require_const_evaluable_result(execution, machine, measured.value())?;
            Ok(measured)
        }

        pub(crate) fn require_const_evaluable_invocation(
            &self,
            program: &TypedTrees,
            machine: &Machine,
            arguments: &[BuildTimeValue],
            custody: BuildTimeInvocationCustody,
            discharge_authored_requires: bool,
        ) -> Result<(), String> {
            let discharge_authored_requires = discharge_authored_requires
                || self.zero_argument_invocation_discharge(
                    program,
                    machine,
                    arguments,
                    Some(custody),
                );
            if discharge_authored_requires {
                self.require_common_floor_for_concrete_premise_invocation(
                    program, machine, custody,
                )?;
            } else {
                self.require_common_floor_for_invocation(program, machine, custody)?;
            }
            let mut snapshots = arguments.to_vec();
            self.require_invocation_failures(
                BuildTimeProgram::Typed(program),
                machine,
                &mut snapshots,
                Some(custody),
            )?;
            Ok(())
        }

        fn machine_suspension(&self, machine_symbol: SymbolHandle) -> Option<bool> {
            self.suspension
                .iter()
                .find(|row| row.machine_symbol == machine_symbol)
                .map(|row| row.transitive_may_suspend)
        }

        fn machine_blocking(&self, machine_symbol: SymbolHandle) -> Option<bool> {
            self.blocking
                .iter()
                .find(|row| row.machine_symbol == machine_symbol)
                .map(|row| row.transitive_may_block)
        }
    }

    fn project_operational_axes(
        program: &TypedTrees,
        operational: &flow_effects::OperationalPlan,
    ) -> (
        Vec<BuildTimeSuspensionRow>,
        Vec<BuildTimeBlockingRow>,
        Vec<BuildTimeCallEdge>,
    ) {
        let mut suspension = Vec::new();
        let mut blocking = Vec::new();
        let mut call_edges = Vec::new();
        for machine in operational.machines() {
            suspension.push(BuildTimeSuspensionRow {
                machine_symbol: machine.symbol,
                transitive_may_suspend: machine.transitive_may_suspend,
            });
            blocking.push(BuildTimeBlockingRow {
                machine_symbol: machine.symbol,
                transitive_may_block: machine.transitive_may_block,
            });
            for state in operational.states.span_or_empty(machine.states) {
                for call in operational.calls.span_or_empty(state.calls) {
                    call_edges.push(BuildTimeCallEdge {
                        source_machine_symbol: machine.symbol,
                        static_machine_parameter: call.static_machine_parameter,
                        target_machine_symbol: call.target_machine_symbol,
                        target_state_symbol: call.target_state_symbol,
                        target_operator_symbol: call.target_operator_symbol,
                        expression: match validation::find_call_site(
                            program,
                            machine.symbol,
                            state.symbol,
                            call.statement_index,
                            call.call_ordinal,
                        ) {
                            Some(validation::CallSite::Expression { expression, .. }) => expression,
                            _ => typed_trees::expression::ExpressionHandle::invalid(),
                        },
                    });
                }
            }
        }
        (suspension, blocking, call_edges)
    }
}
pub(crate) mod build_activation {
    use crate::build_time_evaluation::type_reference_table;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::data::DataMember;

    pub fn admit_toolchain_build_activation(typed: &TypedTrees) -> Result<bool, Vec<Diagnostic>> {
        let Some(build_symbol) = build_activation_symbol(typed)? else {
            return Ok(false);
        };
        validate_build_activation_integrity(typed, build_symbol)?;
        Ok(true)
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum BuildStorage {
        Owned,
        Borrowed,
    }

    fn build_storage(
        typed: &TypedTrees,
        type_reference: typed_trees::types::TypeReferenceHandle,
        expected: SymbolHandle,
    ) -> Option<BuildStorage> {
        match typed.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                build_storage(typed, *referee, expected).map(|_| BuildStorage::Borrowed)
            }
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                build_storage(typed, *base_type, expected)
            }
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == expected => {
                Some(BuildStorage::Owned)
            }
            _ => None,
        }
    }

    fn build_activation_symbol(
        typed: &TypedTrees,
    ) -> Result<Option<SymbolHandle>, Vec<Diagnostic>> {
        let exact_builds = typed
            .data_definitions()
            .iter()
            .filter(|definition| {
                is_exact_toolchain_build_prelude_data(typed, definition.symbol, "Build")
            })
            .collect::<Vec<_>>();
        let build = match exact_builds.as_slice() {
            [] => return Ok(None),
            [build] => *build,
            _ => {
                return Err(vec![Diagnostic::error(
                    "the program declares more than one toolchain Build activation",
                )]);
            }
        };
        let x86_feature_fields = typed
            .data_members(build)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "x86_deployment_features" => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [x86_feature_field] = x86_feature_fields.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "toolchain Build declares {} `x86_deployment_features` fields; the activation requires exactly one",
                x86_feature_fields.len()
            ))]);
        };
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            type_reference_table(typed).type_reference(x86_feature_field.type_reference)
        else {
            return Err(vec![Diagnostic::error(
                "toolchain Build.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type",
            )]);
        };
        if !is_exact_toolchain_build_prelude_data(typed, *symbol, "X86DeploymentFeatures") {
            return Err(vec![Diagnostic::error(
                "toolchain Build.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type",
            )]);
        }
        Ok(Some(build.symbol))
    }

    fn validate_build_activation_integrity(
        typed: &TypedTrees,
        build_symbol: SymbolHandle,
    ) -> Result<(), Vec<Diagnostic>> {
        use typed_trees::{expression::ExpressionNode, statement::StatementNode};

        let mut diagnostics = Vec::new();
        for definition in typed.data_definitions() {
            for member in typed.data_members(definition) {
                let Some(field) = member.as_field() else {
                    continue;
                };
                if build_storage(typed, field.type_reference, build_symbol)
                    == Some(BuildStorage::Owned)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "the compiler-owned Build activation cannot be stored in field `{}`",
                        field.name.as_str()
                    )));
                }
            }
        }
        for machine in typed.machines() {
            if !machine.body_is_present
                && typed.machine_states(machine).iter().any(|state| {
                    typed.state_parameters(state).iter().any(|parameter| {
                        build_storage(typed, parameter.type_reference, build_symbol).is_some()
                    })
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "bodyless machine `{}` cannot receive the compiler-owned Build activation",
                    machine.name.as_str()
                )));
            }
            for owned in typed.machine_owned_data(machine) {
                if build_storage(typed, owned.type_reference, build_symbol)
                    == Some(BuildStorage::Owned)
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "the compiler-owned Build activation cannot be stored in machine field `{}`",
                    owned.name.as_str()
                )));
                }
            }
            for state in typed.machine_states(machine) {
                for statement in typed.statement_table.statements(state.statement_nodes) {
                    match statement {
                        StatementNode::Assignment(assignment) => {
                            if validation::assignment_stored_value_type(
                                typed, machine, state, assignment,
                            )
                            .is_some_and(|reference| {
                                build_storage(typed, reference, build_symbol)
                                    == Some(BuildStorage::Owned)
                            }) {
                                diagnostics.push(Diagnostic::error(
                                "the compiler-owned Build activation cannot be replaced as a whole value",
                            ));
                            }
                        }
                        StatementNode::LocalData(local)
                            if build_storage(typed, local.type_reference, build_symbol)
                                == Some(BuildStorage::Owned) =>
                        {
                            diagnostics.push(Diagnostic::error(format!(
                            "the compiler-owned Build activation cannot be copied into local `{}`",
                            local.name.as_str()
                        )));
                        }
                        _ => {}
                    }
                }
            }
        }
        for (_, expression) in typed.expression_table.iter_expressions() {
            match expression {
                ExpressionNode::StructLiteral(literal) if literal.type_symbol == build_symbol => {
                    diagnostics.push(Diagnostic::error(
                        "source cannot construct the compiler-owned Build activation",
                    ));
                }
                ExpressionNode::ZeroValue(type_reference)
                    if build_storage(typed, *type_reference, build_symbol).is_some() =>
                {
                    diagnostics.push(Diagnostic::error(
                    "source cannot construct a zero value of the compiler-owned Build activation",
                ));
                }
                _ => {}
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    pub fn is_exact_toolchain_build_prelude_data(
        typed: &TypedTrees,
        symbol: SymbolHandle,
        expected_name: &str,
    ) -> bool {
        typed.data_definitions().iter().any(|definition| {
            definition.symbol == symbol
                && definition.name.as_str() == expected_name
                && typed.symbols.is_build_prelude_symbol(symbol)
        })
    }
}
pub(crate) mod build_machines {
    use crate::build_time_evaluation::EvaluationDomain;
    use crate::const_evaluation::const_applications::defer_pending_const_applications;
    use checked_interpreter::BuildMachineProgram;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;

    mod numeric_imports {
        use numerics::float_semantics::{FloatFormat, FloatSemantics};
        use semantic_vocabulary::{IeeeFloatValue, ScalarType};
        use terminal_interpreter::{
            TerminalEffectRejection, TerminalInstance, TerminalProgram, TerminalScalarImport,
            TerminalScalarValue,
        };
        use typed_trees::expression::BinaryOperator;

        pub(super) fn install(
            program: &TerminalProgram<'_>,
            instance: &mut TerminalInstance<'_, '_>,
            typed: &typed_trees::TypedTrees,
            operators: &[crate::SelectedBuildTimeBinaryOperator],
        ) -> Result<(), String> {
            let identities = operators
                .iter()
                .map(|row| {
                    typed
                        .normalized_hermetic_symbol_identity(row.requirement)
                        .map(|identity| (identity, row))
                        .map_err(|error| {
                            format!("selected numeric requirement identity: {error:?}")
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            for import in program.imports() {
                let Some(declaration) = &import.declaration else {
                    continue;
                };
                let identity = program
                    .module()
                    .identities
                    .text(declaration.source.semantic_declaration);
                let mut selected = identities
                    .iter()
                    .filter(|(candidate, _)| Some(candidate.as_str()) == identity);
                let Some((_, first)) = selected.next() else {
                    continue;
                };
                if selected.any(|(_, other)| {
                    other.operation != first.operation || other.format != first.format
                }) {
                    return Err(
                        "one numeric requirement has competing selected executable meanings".into(),
                    );
                }
                let format = match first.format {
                    numerics::literals::FloatFormat::F32 => {
                        semantic_vocabulary::IeeeFloatFormat::Binary32
                    }
                    numerics::literals::FloatFormat::F64 => {
                        semantic_vocabulary::IeeeFloatFormat::Binary64
                    }
                };
                if import.scalar_parameters != [ScalarType::IeeeFloat(format); 2] {
                    return Err("selected numeric import lost its exact operand formats".into());
                }
                let operation = first.operation;
                let adapter = TerminalScalarImport::for_import(
                    import,
                    move |arguments: &[TerminalScalarValue]| evaluate(operation, arguments),
                )
                .map_err(|error| error.to_string())?;
                instance.bind_import(import.id, adapter).map_err(|error| {
                    format!("selected numeric import binding failed: {error:?}")
                })?;
            }
            Ok(())
        }

        fn evaluate(
            operation: BinaryOperator,
            arguments: &[TerminalScalarValue],
        ) -> Result<TerminalScalarValue, TerminalEffectRejection> {
            let [
                TerminalScalarValue::IeeeFloat(left),
                TerminalScalarValue::IeeeFloat(right),
            ] = arguments
            else {
                return Err(TerminalEffectRejection::new(
                    "numeric import requires two IEEE operands",
                ));
            };
            if left.format() != right.format() {
                return Err(TerminalEffectRejection::new(
                    "numeric import operand formats differ",
                ));
            }
            let format = match left {
                IeeeFloatValue::Binary32(_) => FloatFormat::BINARY32,
                IeeeFloatValue::Binary64(_) => FloatFormat::BINARY64,
            };
            let left_meaning = terminal_interpreter::float_result_policy::meaning(*left);
            let right_meaning = terminal_interpreter::float_result_policy::meaning(*right);
            let boolean = match operation {
                BinaryOperator::Equal => Some(FloatSemantics::equal(&left_meaning, &right_meaning)),
                BinaryOperator::NotEqual => {
                    Some(FloatSemantics::not_equal(&left_meaning, &right_meaning))
                }
                BinaryOperator::Less => Some(FloatSemantics::less(&left_meaning, &right_meaning)),
                BinaryOperator::LessOrEqual => {
                    Some(FloatSemantics::less_or_equal(&left_meaning, &right_meaning))
                }
                BinaryOperator::Greater => {
                    Some(FloatSemantics::greater(&left_meaning, &right_meaning))
                }
                BinaryOperator::GreaterOrEqual => Some(FloatSemantics::greater_or_equal(
                    &left_meaning,
                    &right_meaning,
                )),
                _ => None,
            };
            if let Some(value) = boolean {
                return Ok(TerminalScalarValue::Boolean(value));
            }
            let result = match operation {
                BinaryOperator::Add => FloatSemantics::add(format, &left_meaning, &right_meaning),
                BinaryOperator::Subtract => {
                    FloatSemantics::subtract(format, &left_meaning, &right_meaning)
                }
                BinaryOperator::Multiply => {
                    FloatSemantics::multiply(format, &left_meaning, &right_meaning)
                }
                BinaryOperator::Divide => {
                    FloatSemantics::divide(format, &left_meaning, &right_meaning)
                }
                _ => {
                    return Err(TerminalEffectRejection::new(
                        "selected operation has no primitive IEEE meaning",
                    ));
                }
            };
            Ok(TerminalScalarValue::IeeeFloat(match left {
                IeeeFloatValue::Binary32(_) => IeeeFloatValue::Binary32(result.to_f32().to_bits()),
                IeeeFloatValue::Binary64(_) => IeeeFloatValue::Binary64(result.to_f64().to_bits()),
            }))
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn primitive_supply_returns_raw_ieee_result_for_call_local_policy() {
                let maximum =
                    TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(f32::MAX.to_bits()));
                let result =
                    evaluate(BinaryOperator::Add, &[maximum, maximum]).expect("IEEE addition");
                assert!(matches!(
                    result,
                    TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits))
                        if bits == f32::INFINITY.to_bits()
                ));
            }

            #[test]
            fn primitive_comparison_keeps_unordered_nan_meaning() {
                let nan =
                    TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(f64::NAN.to_bits()));
                assert!(matches!(
                    evaluate(BinaryOperator::Equal, &[nan, nan]).expect("equality"),
                    TerminalScalarValue::Boolean(false)
                ));
                assert!(matches!(
                    evaluate(BinaryOperator::NotEqual, &[nan, nan]).expect("inequality"),
                    TerminalScalarValue::Boolean(true)
                ));
            }
        }
    }

    pub use checked_interpreter::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits,
        CapturedFilesystemRoot as BuildMachineCapturedFilesystemRoot,
        FilesystemAccess as BuildMachineFilesystemAccess,
        FilesystemGrantRoot as BuildMachineFilesystemGrantRoot,
        FilesystemGrantRootIdentity as BuildMachineFilesystemGrantRootIdentity,
        FilesystemMetadataLayout as BuildMachineFilesystemMetadataLayout,
        FilesystemSponsor as BuildMachineFilesystemSponsor,
        FilesystemSponsorLimits as BuildMachineFilesystemSponsorLimits,
        FsGrants as BuildMachineFilesystemGrants,
    };

    /// Evaluate one admitted invocation of a checked machine on Terminal Psi.
    ///
    /// We produce the invocation artifact for exactly `symbol`, bind each
    /// compiler-materialized argument to the entry's scalar or structural
    /// parameter list in source order (the artifact splits one source list into
    /// those two ordered lists), run the instance under the pure work budget,
    /// and read the returned value back through the shared value bridge. A
    /// value the bridge cannot represent refuses the invocation; no other
    /// engine is tried.
    pub(crate) fn evaluate_checked_invocation(
        program: BuildMachineProgram<'_>,
        checked: &checked_trees::CheckedTrees,
        symbol: SymbolHandle,
        arguments: Vec<crate::BuildTimeValue>,
        operators: &[crate::SelectedBuildTimeBinaryOperator],
        target: Option<crate::InvocationTarget<'_>>,
        accounting: &crate::evaluation_accounting::EvaluationAccounting,
    ) -> Result<crate::MeasuredEvaluation<crate::BuildTimeValue>, String> {
        use crate::terminal_values::{
            input_contents, result_value, scalar_argument, scalar_result,
        };
        use terminal_interpreter::{
            TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralInputs,
            TerminalStructuralValue,
        };
        super::selected_operators::validate_selected_operators(&checked.typed, operators)?;
        accounting.evaluate_terminal_invocation(
            program,
            symbol,
            &arguments,
            operators,
            |meter, retained_artifact| {
                let toolchain_sources = source::source_identity::toolchain_source_identities(
                    checked.symbols.source_files(),
                )?;
                let mut production = psi_terminal_production::TerminalProductionRequest::new(
                    checked,
                    psi_terminal_production::TerminalMachineSelection::Symbol(symbol),
                    &toolchain_sources,
                );
                production.publication = terminal_codec::TerminalPublication::Invocation;
                let produced = production
                    .produce(
                        Default::default(),
                        &mut psi_terminal_production::UnmeasuredProduction,
                    )
                    .map_err(|error| error.to_string())?;
                *retained_artifact = Some(Arc::new(produced));
                let artifact = retained_artifact
                    .as_ref()
                    .ok_or("invocation Terminal production lost its exact artifact")?;
                let program = terminal_interpreter::TerminalProgram::borrow(artifact.view());
                // A target-family member runs only under its target's selection.
                // Without a named target the entry stays unselected and refuses.
                let selected = target.map(|target| {
                    terminal_installation::admit_target_machine_selections(
                        artifact.view(),
                        target.name,
                    )
                });
                // An artifact with target layout applications runs only under
                // the selected target's layouts, so we admit them from the
                // target's facts whenever the caller supplied them.
                let layouts = match (target, &selected) {
                    (Some(target), Some(selected)) => target
                        .layout_target
                        .map(|facts| {
                            terminal_installation::admit_target_layouts(
                                artifact.view(),
                                selected,
                                facts,
                            )
                        })
                        .transpose()
                        .map_err(|error| format!("invocation Terminal target layouts: {error}"))?,
                    _ => None,
                };
                let installation = selected
                    .as_ref()
                    .map(|selected| {
                        program.admit_runtime_supplies(
                            terminal_interpreter::TerminalRuntimeSupplyRequest {
                                native_code_layouts: None,
                                x86_flags: None,
                                memory_ordering: None,
                                installation: None,
                                host_imports: &[],
                                entry: None,
                                target_machines: Some(selected),
                                target_layouts: layouts.as_ref(),
                            },
                        )
                    })
                    .transpose()
                    .map_err(|error| format!("invocation Terminal target selection: {error:?}"))?;
                let entry = match &installation {
                    Some(installation) => program.entry_installed(installation),
                    None => program.entry(),
                }
                .map_err(|error| format!("invocation Terminal entry: {error:?}"))?;
                // Checking can defer a scalar condition into the artifact
                // when its value depends on the target. Terminal execution
                // refuses to start while one is unadmitted, so we discharge the
                // entry's conditions for this target first and run under that
                // exact admission. Nothing here skips a condition: without a
                // target, or when one fails, the invocation refuses.
                let completed = if artifact
                    .view()
                    .module()
                    .scalar_applications
                    .conditions
                    .is_empty()
                {
                    None
                } else {
                    let (Some(target), Some(selected), Some(unconditioned)) =
                        (target, &selected, &installation)
                    else {
                        return Err(
                            "invocation retains scalar conditions that only a named target can discharge"
                                .into(),
                        );
                    };
                    let requirements = program
                        .execution_requirements(Some(unconditioned))
                        .map_err(|error| format!("invocation Terminal demands: {error:?}"))?;
                    let mut demanded = requirements.scalar_applications;
                    for application in requirements.scalar_entry_conditions {
                        if !demanded.contains(&application) {
                            demanded.push(application);
                        }
                    }
                    let declaration = artifact
                        .view()
                        .module()
                        .scalar_applications
                        .proof_scope
                        .callable_declaration(terminal_psi::TargetMachineCallable::Machine(
                            entry.machine,
                        ))
                        .ok_or("invocation entry has no exact checked proof declaration")?;
                    let providers = terminal_installation::admit_provider_execution_installation(
                        artifact.view(),
                        &[],
                        &[],
                    )
                    .map_err(|error| format!("invocation Terminal providers: {error:?}"))?;
                    let scalars = crate::scalar_applications::complete_invocation_scalar_applications(
                        artifact.view(),
                        target,
                        selected,
                        &providers,
                        declaration,
                        &demanded,
                        meter,
                    )
                    .map_err(|error| format!("invocation scalar conditions: {error}"))?;
                    Some((providers, scalars))
                };
                let installation = match (&completed, &selected) {
                    (Some((providers, scalars)), Some(selected)) => Some(
                        program
                            .admit_runtime_supplies(
                                terminal_interpreter::TerminalRuntimeSupplyRequest {
                                    native_code_layouts: None,
                                    x86_flags: None,
                                    memory_ordering: None,
                                    installation: Some(providers),
                                    host_imports: &[],
                                    entry: None,
                                    target_machines: Some(selected),
                                    target_layouts: Some(scalars.primary().layouts()),
                                },
                            )
                            .map_err(|error| {
                                format!("invocation Terminal scalar installation: {error:?}")
                            })?,
                    ),
                    _ => installation,
                };
                if entry.scalar_parameters.len() + entry.structural_parameters.len()
                    != arguments.len()
                {
                    return Err(format!(
                        "invocation supplies {} argument(s) to a Terminal entry with {} parameter(s)",
                        arguments.len(),
                        entry.scalar_parameters.len() + entry.structural_parameters.len(),
                    ));
                }
                let mut scalars = Vec::with_capacity(entry.scalar_parameters.len());
                let mut contents = Vec::with_capacity(entry.structural_parameters.len());
                let mut structural = Vec::with_capacity(entry.structural_parameters.len());
                for argument in &arguments {
                    let scalar_shaped = matches!(
                        argument,
                        crate::BuildTimeValue::Int(_) | crate::BuildTimeValue::Bool(_)
                    );
                    if scalar_shaped && scalars.len() < entry.scalar_parameters.len() {
                        let parameter = entry.scalar_parameters[scalars.len()];
                        scalars.push(scalar_argument(parameter.scalar_type, argument)?);
                        continue;
                    }
                    let index = structural.len();
                    let parameter = entry.structural_parameters.get(index).ok_or(
                        "invocation argument has no matching Terminal parameter",
                    )?;
                    let ordinal =
                        u32::try_from(index).map_err(|_| "too many structural arguments")?;
                    contents.push(input_contents(
                        &program,
                        ordinal,
                        parameter.structural_type,
                        argument,
                    )?);
                    structural.push(TerminalStructuralValue::minted(
                        u64::from(ordinal) + 1,
                        parameter.structural_type,
                        Vec::new(),
                    ));
                }
                if scalars.len() != entry.scalar_parameters.len() {
                    return Err("invocation arguments differ from the Terminal entry's scalar parameters".into());
                }
                let result_type = entry
                    .result
                    .structural()
                    .map(|result| result.structural_type);
                let account = meter.invocation_account().cloned();
                let mut instance = program
                    .instantiate(terminal_interpreter::TerminalInstantiation {
                        scalar_arguments: &scalars,
                        structural_inputs: TerminalStructuralInputs {
                            arguments: &structural,
                            contents: &contents,
                            ..Default::default()
                        },
                        installation: installation.as_ref(),
                        account: account.as_ref(),
                        scalar_admission: completed.as_ref().map(|(_, scalars)| scalars),
                        ..Default::default()
                    })
                    .map_err(|error| format!("invocation Terminal arguments: {error:?}"))?;
                numeric_imports::install(&program, &mut instance, &checked.typed, operators)?;
                let status = instance
                    .invoke(meter)
                    .map_err(|error| format!("invocation Terminal execution failed: {error:?}"))?;
                let value = match status {
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                        crate::BuildTimeValue::Unit
                    }
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value)) => {
                        scalar_result(value)?
                    }
                    TerminalExecutionStatus::Complete(
                        TerminalExecutionResult::Structural(_)
                        | TerminalExecutionResult::ScalarCase(_)
                        | TerminalExecutionResult::ScalarArray(_),
                    ) => result_value(
                        &instance,
                        result_type.ok_or("invocation result has no structural declaration")?,
                    )?,
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        return Err(format!(
                            "invocation Terminal execution exhausted its work budget after {} units",
                            meter.usage().total_units(),
                        ));
                    }
                    TerminalExecutionStatus::Crashed(crash) => {
                        return Err(format!("invocation Terminal execution crashed: {crash:?}"));
                    }
                    other => {
                        return Err(format!(
                            "invocation Terminal execution did not complete as a pure value: {other:?}"
                        ));
                    }
                };
                let cells = value
                    .retained_cell_count()
                    .ok_or("invocation result cell count overflowed")?;
                let bytes = value
                    .retained_text_byte_count()
                    .ok_or("invocation result payload count overflowed")?;
                if let Some(account) = &account {
                    account
                        .charge_result_custody(cells, bytes)
                        .map_err(|error| format!("invocation result custody: {error}"))?;
                }
                instance
                    .close()
                    .map_err(|error| format!("invocation Terminal close: {error:?}"))?;
                Ok(value)
            },
        )
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum BuildMachineExecutionMode {
        Pure,
        Granted {
            filesystem: BuildMachineFilesystemAccess,
            filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout,
        },
    }

    pub struct PreparedBuildMachineProgram {
        program: PreparedBuildMachineBody,
        identity: Arc<PreparedBuildMachineProgramIdentity>,
    }

    enum PreparedBuildMachineBody {
        Typed(Box<TypedTrees>),
        Checked(CheckedBuildProgram),
    }

    #[derive(Clone, PartialEq, Eq)]
    pub struct CheckedBuildProgram {
        checked: Arc<checked_trees::CheckedTrees>,
        product_declarations: Arc<TypedTrees>,
    }

    #[derive(Clone)]
    pub struct PreparedBuildMachineEntry {
        program_identity: Arc<PreparedBuildMachineProgramIdentity>,
        symbol: SymbolHandle,
    }

    struct PreparedBuildMachineProgramIdentity {
        _private: u8,
    }

    impl PreparedBuildMachineEntry {
        pub const fn symbol(&self) -> SymbolHandle {
            self.symbol
        }
    }

    impl PreparedBuildMachineProgram {
        pub fn from_checked(program: checked_trees::CheckedTrees) -> Self {
            let product_declarations = program
                .typed
                .product_declaration_frontier
                .clone()
                .unwrap_or_else(|| Arc::new(program.typed.as_ref().clone()));
            Self {
                program: PreparedBuildMachineBody::Checked(CheckedBuildProgram {
                    checked: Arc::new(program),
                    product_declarations,
                }),
                identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
            }
        }

        pub fn prepare(program: &TypedTrees) -> Result<Self, Vec<diagnostics::Diagnostic>> {
            Self::prepare_in(program, &EvaluationDomain::all())
        }

        pub(crate) fn prepare_in(
            program: &TypedTrees,
            domain: &EvaluationDomain,
        ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
            let mut typed = domain.execution_program(program);
            crate::const_evaluation::range_endpoints::defer_pending_endpoint_calls(&mut typed)?;
            defer_pending_const_applications(&mut typed, &EvaluationDomain::all())?;
            let typed = typed_trees_to_typed_trees::prepare_constant_execution(typed)?;
            Ok(Self {
                program: PreparedBuildMachineBody::Typed(Box::new(typed)),
                identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
            })
        }

        pub fn typed(&self) -> &TypedTrees {
            match &self.program {
                PreparedBuildMachineBody::Typed(typed) => typed,
                PreparedBuildMachineBody::Checked(program) => &program.checked.typed,
            }
        }

        pub fn checked_program(&self) -> Option<CheckedBuildProgram> {
            match &self.program {
                PreparedBuildMachineBody::Checked(program) => Some(program.clone()),
                PreparedBuildMachineBody::Typed(_) => None,
            }
        }

        pub fn checked_trees(&self) -> Option<&checked_trees::CheckedTrees> {
            match &self.program {
                PreparedBuildMachineBody::Checked(program) => Some(&program.checked),
                PreparedBuildMachineBody::Typed(_) => None,
            }
        }

        pub(crate) fn evaluate_checked_constant(
            &self,
            entry: &PreparedBuildMachineEntry,
            operators: &[crate::SelectedBuildTimeBinaryOperator],
            accounting: &crate::evaluation_accounting::EvaluationAccounting,
        ) -> Result<terminal_interpreter::TerminalExecutionResult, String> {
            let PreparedBuildMachineBody::Checked(program) = &self.program else {
                return Err("constant invocation requires a checked preparation".into());
            };
            let symbol = self.validate_entry(entry)?;
            super::selected_operators::validate_selected_operators(self.typed(), operators)?;
            accounting.evaluate_terminal(
            self.interpreter_program(),
            symbol,
            operators,
            |meter, retained_artifact| {
                let toolchain_sources = source::source_identity::toolchain_source_identities(
                    program.checked.symbols.source_files(),
                )?;
                let mut production = psi_terminal_production::TerminalProductionRequest::new(
                    &program.checked,
                    psi_terminal_production::TerminalMachineSelection::Symbol(symbol),
                    &toolchain_sources,
                );
                production.publication = terminal_codec::TerminalPublication::Invocation;
                let produced = production.produce(
                    Default::default(),
                    &mut psi_terminal_production::UnmeasuredProduction,
                )
                .map_err(|error| error.to_string())?;
                *retained_artifact = Some(Arc::new(produced));
                let artifact = retained_artifact.as_ref().ok_or_else(|| {
                    "constant Terminal production lost its exact artifact".to_owned()
                })?;
                let program = terminal_interpreter::TerminalProgram::borrow(artifact.view());
                let mut instance = program
                    .instantiate(terminal_interpreter::TerminalInstantiation {
                        scalar_arguments: &[],
                        structural_inputs: Default::default(),
                        ..Default::default()
                    })
                    .map_err(|error| error.to_string())?;
                numeric_imports::install(&program, &mut instance, self.typed(), operators)?;
                match instance
                    .invoke(meter)
                    .map_err(|error| format!("constant Terminal invocation failed: {error:?}"))?
                {
                    terminal_interpreter::TerminalExecutionStatus::Complete(value) => Ok(value),
                    terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => Err(format!(
                        "constant Terminal execution exhausted its work budget after {} units",
                        meter.usage().total_units(),
                    )),
                    terminal_interpreter::TerminalExecutionStatus::Crashed(crash) => {
                        Err(format!("constant Terminal execution crashed: {crash:?}"))
                    }
                    terminal_interpreter::TerminalExecutionStatus::AwaitingEffect(site) => Err(
                        format!("constant Terminal execution cannot await a host effect: {site:?}"),
                    ),
                    terminal_interpreter::TerminalExecutionStatus::HostComputationPending(site) => Err(
                        format!("constant Terminal execution has no host computation scheduler: {site:?}"),
                    ),
                    terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(completion) => Err(
                        format!("constant Terminal execution completed externally: {completion:?}"),
                    ),
                }
            },
        )
        }

        fn interpreter_program(&self) -> BuildMachineProgram<'_> {
            match &self.program {
                PreparedBuildMachineBody::Typed(typed) => BuildMachineProgram::Typed(typed),
                PreparedBuildMachineBody::Checked(program) => BuildMachineProgram::CheckedClosure {
                    execution: &program.checked,
                    product_declarations: &program.product_declarations,
                },
            }
        }

        pub fn entry(&self, symbol: SymbolHandle) -> Result<PreparedBuildMachineEntry, String> {
            if !self.typed().machine(symbol).is_some() {
                return Err(format!(
                    "prepared build program contains no machine with exact symbol {symbol:?}"
                ));
            }
            if self.typed().machines().iter().any(|machine| {
                machine.structural_type_equations_pending || !machine.type_parameters.is_empty()
            }) {
                super::admission::BuildTimeAdmissionPlan::infer(self.typed(), None)
                    .require_closed_static_applications(self.typed(), symbol)?;
            }
            Ok(PreparedBuildMachineEntry {
                program_identity: Arc::clone(&self.identity),
                symbol,
            })
        }

        fn validate_entry(
            &self,
            entry: &PreparedBuildMachineEntry,
        ) -> Result<SymbolHandle, String> {
            if !Arc::ptr_eq(&self.identity, &entry.program_identity) {
                return Err(
                    "prepared build-machine entry belongs to a different prepared program"
                        .to_owned(),
                );
            }
            Ok(entry.symbol)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::PreparedBuildMachineProgram;
        use crate::evaluation_accounting::EvaluationAccounting;
        use semantic_vocabulary::IntegerValue;
        use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};

        fn typed(source: &str) -> typed_trees::TypedTrees {
            let tokens = source_files_to_tokens::Lexer::new(source)
                .tokenize()
                .expect("lex");
            let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
            let mut sources = source::SourceMap::default();
            sources.add_with_metadata(
                "constant.omg".into(),
                source.into(),
                ".".into(),
                semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
                source::SourceOrigin::User,
            );
            let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
            request.sources = Some(std::sync::Arc::new(sources));
            let resolved =
                syntax_trees_to_symbol_resolved_trees::resolve(request).expect("resolve");
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type")
        }

        fn checked(source: &str) -> PreparedBuildMachineProgram {
            let (typed, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
                typed(source),
                &typed_trees_to_typed_trees::PreparationRequest::settled(),
            )
            .expect("prepare");
            let checked = typed_trees_to_checked_trees::lower_typed_trees(
                typed,
                preparation,
                &typed_trees_to_checked_trees::CheckingRequest::new(),
            )
            .expect("check");
            PreparedBuildMachineProgram::from_checked(checked)
        }

        fn on_compilation_stack(test: impl FnOnce() + Send + 'static) {
            std::thread::Builder::new()
                .stack_size(16 * 1024 * 1024)
                .spawn(test)
                .expect("start compilation test")
                .join()
                .expect("compilation test");
        }

        #[test]
        fn checked_constant_runs_transitive_terminal_calls() {
            on_compilation_stack(|| {
                let program = checked(
                    "machine leaf() -> u64 { 16 }
             machine middle() -> u64 { leaf() }
             machine answer() -> u64 { middle() }",
                );
                let symbol = program
                    .typed()
                    .realized_machine_named("answer")
                    .expect("answer")
                    .symbol;
                let entry = program.entry(symbol).expect("entry");
                let accounting = EvaluationAccounting::recording();
                let result = program
                    .evaluate_checked_constant(&entry, &[], &accounting)
                    .expect("Terminal execution");
                let completed = accounting.finish().expect("complete Terminal accounting");
                assert!(completed.has_complete_usage());
                let [crate::PureEvaluationRecord::Terminal(receipt)] = completed.receipts() else {
                    panic!("one root Terminal execution must retain one typed receipt");
                };
                assert_eq!(receipt.machine(), symbol);
                let input = program.checked_program().expect("exact checked input");
                assert!(matches!(
                    receipt.source_program(),
                    crate::PureEvaluationProgram::CheckedClosure {
                        execution,
                        product_declarations,
                    } if execution.as_ref() == input.checked.as_ref()
                        && product_declarations.as_ref() == input.product_declarations.as_ref()
                ));
                assert!(receipt.operators().is_empty());
                assert!(receipt.program().is_some());
                assert!(receipt.usage().total_units() > 0);
                assert!(matches!(
                    receipt.outcome(),
                    crate::PureTerminalEvaluationOutcome::Returned { value } if value == &result
                ));
                completed
                    .validate_charges()
                    .expect("retained Terminal usage");
                assert!(
                    matches!(
                        result,
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(16),
                            ..
                        })
                    ),
                    "{result:?}"
                );
            });
        }

        #[test]
        fn checked_constant_rejects_another_preparations_entry() {
            on_compilation_stack(|| {
                let first = checked("machine answer() -> u64 { 16 }");
                let second = checked("machine answer() -> u64 { 17 }");
                let symbol = first
                    .typed()
                    .realized_machine_named("answer")
                    .expect("answer")
                    .symbol;
                let entry = first.entry(symbol).expect("entry");
                let error = second
                    .evaluate_checked_constant(&entry, &[], &EvaluationAccounting::default())
                    .expect_err("foreign entry");
                assert!(error.contains("different prepared program"), "{error}");
            });
        }

        #[test]
        fn checked_constant_retains_unsigned_result_meaning() {
            on_compilation_stack(|| {
                let program = checked("machine answer() -> u64 { 18446744073709551615u64 }");
                let symbol = program
                    .typed()
                    .realized_machine_named("answer")
                    .expect("answer")
                    .symbol;
                let entry = program.entry(symbol).expect("entry");
                let result = program
                    .evaluate_checked_constant(&entry, &[], &EvaluationAccounting::default())
                    .expect("Terminal execution");
                assert!(
                    matches!(
                        result,
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(18446744073709551615),
                            ..
                        })
                    ),
                    "{result:?}"
                );
            });
        }

        #[test]
        fn checked_constant_admits_the_retained_recursive_component() {
            on_compilation_stack(|| {
                let program = checked(
                    "data Node { step: u64; count: u64 }
             machine descend(node: Node, index: u64, work: u64) -> u64
             reaches terminates by work
             requires work >= 1 && index <= 8 && node.count <= 8
             {
                 transition index < node.count && work > 1 {
                     true -> through(node, index, work - 1)
                     _ -> work
                 }
                 machine through(node: Node, index: u64, work: u64) -> u64
                 requires work >= 1 && index <= 7
                 {
                     descend(node, index + 1, work)
                 }
             }
             machine answer() -> u64 {
                 descend(Node { step: 1u64, count: 8u64 }, 0u64, 3u64)
             }",
                );
                let symbol = program
                    .typed()
                    .realized_machine_named("answer")
                    .expect("answer")
                    .symbol;
                let entry = program.entry(symbol).expect("entry");
                let result = program
                    .evaluate_checked_constant(&entry, &[], &EvaluationAccounting::default())
                    .expect("Terminal execution");
                assert!(
                    matches!(
                        result,
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(1),
                            ..
                        })
                    ),
                    "{result:?}"
                );
            });
        }

        #[test]
        fn checked_constant_admits_mutating_recursive_component() {
            on_compilation_stack(|| {
                let program = checked(
                    "data Node { step: u64; count: u64 }
             machine descend(node: Node, index: u64, work: u64) -> u64
             reaches terminates by work
             requires work >= 1 && index <= 8 && node.count <= 8
             {
                 transition index < node.count && work > 1 {
                     true -> through(node, index, work - 1)
                     _ -> work
                 }
                 machine through(mut node: Node, index: u64, work: u64) -> u64
                 requires work >= 1 && index <= 7 && node.count <= 8
                 {
                     node.step = node.step;
                     descend(node, index + 1, work)
                 }
             }
             machine answer() -> u64 {
                 descend(Node { step: 1u64, count: 8u64 }, 0u64, 3u64)
             }",
                );
                let symbol = program
                    .typed()
                    .realized_machine_named("answer")
                    .expect("answer")
                    .symbol;
                let entry = program.entry(symbol).expect("entry");
                let result = program
                    .evaluate_checked_constant(&entry, &[], &EvaluationAccounting::default())
                    .expect("Terminal execution");
                assert!(
                    matches!(
                        result,
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(1),
                            ..
                        })
                    ),
                    "{result:?}"
                );
            });
        }

        #[test]
        fn checked_constant_refuses_a_cycle_without_descent() {
            on_compilation_stack(|| {
                let (typed, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
                    typed(
                        "machine descend(work: u64) -> u64
                 reaches terminates by work
                 requires work >= 1
                 {
                     transition work > 1 {
                         true -> through(work)
                         _ -> work
                     }
                     machine through(work: u64) -> u64
                     requires work >= 1
                     {
                         descend(work)
                     }
                 }",
                    ),
                    &typed_trees_to_typed_trees::PreparationRequest::settled(),
                )
                .expect("prepare");
                let error = typed_trees_to_checked_trees::lower_typed_trees(
                    typed,
                    preparation,
                    &typed_trees_to_checked_trees::CheckingRequest::new(),
                )
                .expect_err("an undescended recursion cannot carry a measure promise");
                assert!(
                    error.iter().any(|diagnostic| {
                        diagnostic.message.contains("termination")
                            || diagnostic.message.contains("descent")
                            || diagnostic.message.contains("measure")
                    }),
                    "{error:?}"
                );
            });
        }

        #[test]
        fn checked_constant_does_not_install_boundary_imports() {
            on_compilation_stack(|| {
                let program = checked(
                    "boundary trait Counter { machine read() -> u64 reaches; }
             machine answer() -> u64 reaches Counter { Counter::read() }",
                );
                let symbol = program
                    .typed()
                    .realized_machine_named("answer")
                    .expect("answer")
                    .symbol;
                let entry = program.entry(symbol).expect("entry");
                let error = program
                    .evaluate_checked_constant(&entry, &[], &EvaluationAccounting::default())
                    .expect_err("no host authority");
                assert!(error.contains("UnboundImports"), "{error}");
            });
        }
    }
}
pub(crate) mod selected_operators {
    use crate::SelectedBuildTimeBinaryOperator;
    use crate::build_time_evaluation::expression_table;
    use checked_trees::CheckedOperatorOccurrence;
    use typed_trees::machine::{SatisfiedDeclaration, resolve_satisfied_declaration};
    use typed_trees::{
        TypedTrees,
        expression::{ExpressionHandle, ExpressionNode},
    };
    use validation::expression_traversal::collect_expression_tree;

    macro_rules! miss {
        ($message:literal) => {
            return Err($message.into())
        };
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SelectedBuildTimeProviderBody {
        pub expression: ExpressionHandle,
        pub origin: checked_trees::CheckedValueOrigin,
        pub requirement: symbols::SymbolHandle,
        pub operands: Vec<ExpressionHandle>,
        pub provider_machine: symbols::SymbolHandle,
        pub provider_state: symbols::SymbolHandle,
        pub provider_type: String,
        pub provider_declaration: symbols::SymbolHandle,
        pub target_scope: String,
        pub provider: checked_trees::CheckedProviderPlanCommitment,
    }

    impl SelectedBuildTimeProviderBody {
        pub fn prepare(
            program: &TypedTrees,
            expression: ExpressionHandle,
            origin: checked_trees::CheckedValueOrigin,
            requirement: symbols::SymbolHandle,
            provider: &typed_trees::machine::Machine,
            provider_type: String,
            provider_declaration: symbols::SymbolHandle,
            provider_package: Option<semantic_vocabulary::PackageKeyIdentity>,
            target_scope: &str,
            commitment: checked_trees::CheckedProviderPlanCommitment,
        ) -> Result<Self, String> {
            let operands = match program.expression_table.expression(expression) {
                ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
                ExpressionNode::Indexed(indexed) => vec![indexed.collection, indexed.index],
                ExpressionNode::Call(call) => expression_table(program)
                    .expression_handles(call.arguments)
                    .to_vec(),
                _ => {
                    miss!("selected build-time provider expression has no callable operands");
                }
            };
            let Some(entry) = program.machine_states(provider).first() else {
                return Err("selected build-time provider has no executable entry state".into());
            };
            if !provider_declaration.is_valid()
                || program.symbols.display_path(provider_declaration, "::") != provider_type
                || program
                    .symbols
                    .symbol_package_identity(provider_declaration)
                    != provider_package
                || (program
                    .conformance_by_symbol(provider_declaration)
                    .is_none()
                    && !program
                        .data_definitions()
                        .iter()
                        .any(|owner| owner.symbol == provider_declaration))
                || target_scope.is_empty()
            {
                return Err(
                "selected build-time provider has ambiguous declaration or absent target custody"
                    .into(),
            );
            }
            Ok(Self {
                expression,
                origin,
                requirement,
                operands,
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type,
                provider_declaration,
                target_scope: target_scope.to_owned(),
                provider: commitment,
            })
        }
    }

    pub(crate) fn prepare_selected_execution(
        program: &TypedTrees,
        selected: &[SelectedBuildTimeProviderBody],
        operators: &[SelectedBuildTimeBinaryOperator],
    ) -> Result<Option<TypedTrees>, String> {
        validate_selected_provider_bodies(program, selected)?;
        validate_selected_operators(program, operators)?;
        if operators.iter().any(|operator| {
            selected.iter().any(|body| {
                body.expression == operator.expression
                    && body.origin.machine_symbol() == operator.origin.machine_symbol()
            })
        }) {
            return Err("selected build-time expression has competing executable supplies".into());
        }
        if selected.is_empty() && operators.is_empty() {
            return Ok(None);
        }
        let mut program = program.clone();
        let mut selections = program.authored_declaration_selections().clone();
        for (expression, requirement) in selected
            .iter()
            .map(|row| (row.expression, row.requirement))
            .chain(
                operators
                    .iter()
                    .map(|row| (row.expression, row.requirement)),
            )
        {
            if let Some(occurrence) = direct_operator_occurrence(&program, expression, requirement)?
            {
                use language_semantics::declaration_selection::{
                    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
                };
                let selection = selections.get(occurrence).ok_or_else(|| {
                    "selected build-time expression lost its authored selection".to_owned()
                })?;
                if let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target()
                    && !selects_requirement(&program, target.selected_symbol(), requirement)
                {
                    return Err(
                        "selected build-time operation has competing occurrence targets".into(),
                    );
                }
                if let AuthoredDeclarationSelectionTarget::LateBound(
                    binding @ (AuthoredDeclarationSelectionLateBinding::CheckedOperator
                    | AuthoredDeclarationSelectionLateBinding::CheckedCall),
                ) = selection.target()
                {
                    let target = match program.expression_table.expression(expression) {
                        ExpressionNode::Call(call)
                            if binding == AuthoredDeclarationSelectionLateBinding::CheckedCall
                                && call.target_symbol.is_valid() =>
                        {
                            if !selects_requirement(&program, call.target_symbol, requirement) {
                                return Err(
                                    "selected build-time call changed its requirement target"
                                        .into(),
                                );
                            }
                            call.target_symbol
                        }
                        _ => requirement,
                    };
                    selections
                        .finalize_late_bound(occurrence, binding, target)
                        .map_err(|error| {
                            format!("cannot retain selected build-time requirement: {error:?}")
                        })?;
                }
            }
        }
        program.retain_authored_declaration_selections(selections);
        for row in selected {
            let provider_name = program
                .machine(row.provider_machine)
                .map(|machine| machine.name.as_str().to_owned())
                .ok_or_else(|| {
                    "selected build-time provider body lost its exact machine".to_owned()
                })?;
            let arguments = program
                .expression_table
                .insert_expression_handles(row.operands.iter().copied());
            *program.expression_table.expression_mut(row.expression) =
            ExpressionNode::Call(typed_trees::expression::TableCallExpression {
 authored_call_selection: None,
 authored_call_binding: None,
                authored_machine_application: Default::default(),
                requirement_application: Default::default(),
                exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
                receiver: ExpressionHandle::invalid(),
                target_symbol: row.provider_state,
                static_machine_parameter: symbols::SymbolHandle::invalid(),
                target: typed_trees::name::Identifier::generated(provider_name),
                static_requirement_dispatch: None,
                machine_arguments: Box::new([]),
                quotient_operation: None,
                private_layout_operation: None,
            installed_interrupt_operation: None,
                layout_query: None,
                    native_code_layout: None,
                    build_selection: Default::default(),
                arguments,
                evidence_arguments: Box::new([]),
                operational_acknowledgement:
                    language_semantics::CallOperationalAcknowledgement {
                        origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                        acknowledges_suspend: false,
                        acknowledges_block: false,
                    },
            });
        }
        for row in operators {
            *program.expression_table.expression_mut(row.expression) =
                ExpressionNode::Binary(typed_trees::expression::TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left: row.operands[0],
                    operator: row.operation,
                    right: row.operands[1],
                });
        }
        Ok(Some(program))
    }

    fn require_unique_operator_custody<T>(
        program: &TypedTrees,
        selected: &[T],
        object: &str,
        custody: impl Fn(&T) -> Option<(ExpressionHandle, Option<symbols::SymbolHandle>)>,
    ) -> Result<Option<checked_trees::CheckedOperatorFacts>, String> {
        if selected.is_empty() {
            return Ok(None);
        }
        let facts = validation::derive_pre_flow_operator_selections(program);
        for (index, row) in selected.iter().enumerate() {
            let key = custody(row);
            if key.is_none() || selected[..index].iter().any(|prior| custody(prior) == key) {
                return Err(format!("{object} lacks unique exact provider custody"));
            }
        }
        Ok(Some(facts))
    }

    pub fn validate_selected_provider_bodies(
        program: &TypedTrees,
        selected: &[SelectedBuildTimeProviderBody],
    ) -> Result<(), String> {
        let Some(facts) = require_unique_operator_custody(
            program,
            selected,
            "selected build-time provider body",
            |row| {
                (!row.provider.is_zero()).then_some((row.expression, row.origin.machine_symbol()))
            },
        )?
        else {
            return Ok(());
        };
        for row in selected.iter() {
            let spelled_occurrences = facts
                .uses
                .iter()
                .filter(|(_, fact)| {
                    fact.expression == row.expression
                        && fact.occurrence == CheckedOperatorOccurrence::Expression
                        && fact.origin.machine_symbol() == row.origin.machine_symbol()
                })
                .count();
            let named_occurrences = facts
                .named_uses
                .iter()
                .filter(|(_, fact)| {
                    fact.expression == row.expression
                        && fact.origin.machine_symbol() == row.origin.machine_symbol()
                })
                .count();
            let requirement_occurrences = facts
                .named_requirement_uses()
                .filter(|fact| {
                    fact.expression == row.expression
                        && fact.origin.machine_symbol() == row.origin.machine_symbol()
                })
                .count();
            if spelled_occurrences + named_occurrences + requirement_occurrences != 1 {
                miss!("selected build-time expression is shared by distinct current origins");
            }
            let resolved_spelled: Vec<_> = facts
                .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
                .filter(|fact| {
                    fact.expression == row.expression
                        && fact.origin == row.origin
                        && fact.occurrence == CheckedOperatorOccurrence::Expression
                })
                .collect();
            let resolved_named: Vec<_> = facts
                .named_uses()
                .map(|fact| (fact.expression, fact.origin, fact.selected_operator_symbol))
                .chain(
                    facts
                        .named_requirement_uses()
                        .map(|fact| (fact.expression, fact.origin, fact.requirement_symbol)),
                )
                .filter(|(expression, origin, _)| {
                    *expression == row.expression && *origin == row.origin
                })
                .map(|(_, _, requirement)| requirement)
                .collect();
            let (selected_symbol, spelled_spelling) =
                match (resolved_spelled.as_slice(), resolved_named.as_slice()) {
                    ([fact], []) => (fact.selected_operator_symbol, Some(fact.spelling)),
                    ([], [requirement]) => (*requirement, None),
                    _ => {
                        miss!("selected build-time provider body has no unique current use");
                    }
                };
            if selected_symbol != row.requirement {
                miss!("selected build-time provider body differs from current requirement");
            }
            let operands = match program.expression_table.expression(row.expression) {
                ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
                ExpressionNode::Indexed(indexed) => {
                    match program.expression_table.expression(indexed.index) {
                        ExpressionNode::Range(_) => {
                            return Err(
                            "selected build-time provider use retains a range operand shape with no fixed-token checked-adapter dispatch"
                                .into(),
                        );
                        }
                        _ => vec![indexed.collection, indexed.index],
                    }
                }
                ExpressionNode::Call(call) => expression_table(program)
                    .expression_handles(call.arguments)
                    .to_vec(),
                _ => return Err("selected build-time provider expression disappeared".into()),
            };
            if operands != row.operands {
                return Err(
                    "selected build-time provider operands differ from current authored selection"
                        .into(),
                );
            }
            let providers: Vec<_> = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == row.provider_machine)
                .collect();
            let [provider] = providers.as_slice() else {
                return Err("selected build-time provider machine is not unique".into());
            };
            let named_provider = program.conformance_by_symbol(row.provider_declaration);
            if program.symbols.display_path(row.provider_declaration, "::") != row.provider_type
                || (named_provider.is_none()
                    && (provider.attached_data_symbol != row.provider_declaration
                        || program.attached_data_path(provider).as_deref()
                            != Some(row.provider_type.as_str())))
            {
                return Err(
                    "selected build-time provider does not belong to its nominal provider type"
                        .into(),
                );
            }
            if provider.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
                return Err("selected build-time provider is not a checked body".into());
            }
            if !program.machine_type_parameters(provider).is_empty() {
                return Err(
                "selected build-time provider is generic; closed substitution must settle the realization first"
                    .into(),
            );
            }
            let Some(entry) = program.machine_states(provider).first() else {
                return Err("selected build-time provider has no executable entry state".into());
            };
            if entry.symbol != row.provider_state {
                return Err(
                    "selected build-time provider state is not the provider's executable entry"
                        .into(),
                );
            }
            if row.target_scope.is_empty()
                || program
                    .source_target_context(provider)
                    .is_some_and(|target| target != row.target_scope)
            {
                return Err(
                    "selected build-time provider belongs to another target capsule".into(),
                );
            }
            if program
                .machine_contracts(provider)
                .iter()
                .chain(program.state_contracts(entry).iter())
                .any(|contract| contract.is_crashes())
            {
                return Err(
                "selected build-time provider body crash contracts have no build-time execution support"
                    .into(),
            );
            }
            if let Some((owner, requirement)) = program.trait_machine_signature(row.requirement) {
                if !owner.is_boundary
                    || spelled_spelling
                        .is_some_and(|spelling| requirement.spelling != Some(spelling))
                {
                    return Err(
                        "selected build-time trait requirement lost its exact boundary identity"
                            .into(),
                    );
                }
                if !program.trait_type_parameters(owner).is_empty()
                    || !requirement.type_parameters.is_empty()
                {
                    miss!("selected build-time requirement needs its closed trait application");
                }
                if program
                    .signature_contracts
                    .span_or_empty(requirement.contracts)
                    .iter()
                    .any(|contract| contract.is_crashes())
                {
                    return Err(
                        "selected operator crash invocations have no build-time execution support"
                            .into(),
                    );
                }
                let parameters = program.state_parameters(entry);
                let required_parameters = program.state_signature_parameters(requirement);
                if parameters.len() != row.operands.len()
                    || required_parameters.len() != row.operands.len()
                    || program.normalized_type_identity(entry.return_type)
                        != program.normalized_type_identity(requirement.return_type)
                    || !parameters.iter().zip(required_parameters.iter()).all(
                        |(actual, required)| {
                            program.normalized_type_identity(actual.type_reference)
                                == program.normalized_type_identity(required.type_reference)
                        },
                    )
                {
                    return Err("selected build-time provider entry signature differs from its trait requirement".into());
                }
                let bound = if let Some(declaration) = named_provider {
                    validation::revalidate_named_implementation(program, declaration)
                    .map_err(|diagnostics| format!("selected build-time implementation does not satisfy its contracts: {diagnostics:?}"))?;
                    declaration
                        .rows
                        .iter()
                        .filter(|mapping| {
                            mapping.declaring_trait == owner.symbol
                                && mapping.requirement == row.requirement
                                && (mapping.realization_machine == provider.symbol
                                    && mapping.realization_state == entry.symbol
                                    || program.target_machine_families.iter().any(|family| {
                                        family.canonical == mapping.realization_machine
                                            && family.members.iter().any(|member| {
                                                member.machine == provider.symbol
                                                    && member.target.as_str() == row.target_scope
                                            })
                                    }))
                        })
                        .count()
                } else {
                    validation::revalidate_machine_trait_conformances(program, provider)
                    .map_err(|diagnostics| format!("selected build-time provider does not satisfy its contracts: {diagnostics:?}"))?;
                    program.machine_trait_conformances(provider).iter().filter(|conformance| {
                    conformance.external_binding.is_none()
                        && matches!(resolve_satisfied_declaration(program, provider, conformance),
                            Some(SatisfiedDeclaration::Trait { definition, requirement })
                                if definition.symbol == owner.symbol && requirement.symbol == row.requirement)
                }).count()
                };
                if bound != 1 {
                    miss!("selected build-time provider has no unique exact trait realization");
                }
                continue;
            }
            let mut requirements = program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == row.requirement);
            let machine_requirement = requirements.next();
            if let Some(requirement) = machine_requirement {
                if requirements.next().is_some() {
                    return Err("selected build-time requirement is not unique".into());
                }
                if spelled_spelling.is_some_and(|spelling| requirement.spelling != Some(spelling)) {
                    miss!("selected build-time provider requirement lost its boundary spelling");
                }
                if !program.machine_type_parameters(requirement).is_empty() {
                    return Err("selected build-time provider requirement is generic; symbolic application coverage is not part of this execution".into());
                }
                let [required_entry] = program.machine_states(requirement) else {
                    return Err(
                        "selected build-time requirement has no unique entry signature".into(),
                    );
                };
                let parameters = program.state_parameters(required_entry);
                if parameters.len() != row.operands.len() {
                    return Err(
                    "selected build-time provider requirement arity differs from retained operands"
                        .into(),
                );
                }
                if program
                    .machine_contracts(requirement)
                    .iter()
                    .chain(program.state_contracts(required_entry))
                    .any(|contract| contract.is_crashes())
                {
                    return Err(
                        "selected operator crash invocations have no build-time execution support"
                            .into(),
                    );
                }
                let conformances = program
                    .machine_trait_conformances(provider)
                    .iter()
                    .filter(|conformance| {
                        if conformance.external_binding.is_some() {
                            return false;
                        }
                        let Some(SatisfiedDeclaration::TopLevelRequirement(required)) =
                            resolve_satisfied_declaration(program, provider, conformance)
                        else {
                            return false;
                        };
                        required.symbol == requirement.symbol
                            && validation::revalidate_top_level_requirement_realization(
                                program,
                                provider,
                                required,
                                conformance,
                            )
                            .is_ok()
                    })
                    .count();
                if conformances != 1 {
                    return Err("selected build-time provider does not bind its requirement through exactly one checked conformance".into());
                }
                let mut bindings = program
                    .machine_token_bindings()
                    .iter()
                    .filter(|binding| binding.symbol == requirement.symbol);
                match (bindings.next(), bindings.next()) {
                    (None, None) if requirement.spelling.is_none() => continue,
                    (Some(binding), None) if binding.spelling == requirement.spelling => {}
                    _ => {
                        return Err(
                            "selected build-time token view differs from its actual requirement"
                                .into(),
                        );
                    }
                }
            }
            let matching: Vec<_> = program
                .operators()
                .iter()
                .chain(program.machine_token_bindings())
                .filter(|operator| operator.symbol == row.requirement)
                .collect();
            let [operator] = matching.as_slice() else {
                return Err("selected build-time requirement is not unique".into());
            };
            if !operator.is_boundary
                || spelled_spelling.is_some_and(|spelling| operator.spelling != Some(spelling))
            {
                miss!("selected build-time provider requirement lost its boundary spelling");
            }
            if !program.operator_type_parameters(operator).is_empty() {
                return Err(
                "selected build-time provider requirement is generic; symbolic application coverage is not part of this execution"
                    .into(),
            );
            }
            if program
                .signature_contracts
                .span_or_empty(operator.contracts)
                .iter()
                .any(|contract| contract.is_crashes())
            {
                return Err(
                    "selected operator crash invocations have no build-time execution support"
                        .into(),
                );
            }
            if program.operator_parameters(operator).len() != row.operands.len() {
                return Err(
                    "selected build-time provider requirement arity differs from retained operands"
                        .into(),
                );
            }
            let entry_parameters: Vec<_> = program.state_parameters(entry).iter().collect();
            if entry_parameters.len() != row.operands.len()
                || program.normalized_type_identity(entry.return_type)
                    != program.normalized_type_identity(operator.return_type)
                || !program
                    .operator_parameters(operator)
                    .iter()
                    .zip(entry_parameters.iter())
                    .all(|(requirement, parameter)| {
                        program.normalized_type_identity(requirement.type_reference)
                            == program.normalized_type_identity(parameter.type_reference)
                    })
            {
                return Err(
                    "selected build-time provider entry signature differs from its requirement"
                        .into(),
                );
            }
            if machine_requirement.is_some() {
                continue;
            }
            let [namespace, requirement] = program.operator_path_members(operator.name) else {
                miss!("selected build-time provider requirement has no exact namespace path");
            };
            let conformances = program
                .machine_trait_conformances(provider)
                .iter()
                .filter(|conformance| {
                    conformance.external_binding.is_none()
                    && conformance.name.as_str() == namespace.as_str()
                    && conformance.requirement.as_ref().map(|name| name.as_str())
                        == Some(requirement.as_str())
                    && (typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                        program, provider, conformance,
                    ).is_some_and(|resolved| resolved.symbol == operator.symbol)
                        || typed_trees::operator::resolve_specialized_checked_operator_application(
                            program,
                            provider,
                            namespace.as_str(),
                            requirement.as_str(),
                        ).is_some_and(|(resolved, _)| resolved.symbol == operator.symbol))
                })
                .count();
            if conformances != 1 {
                return Err(
                "selected build-time provider does not bind its requirement through exactly one checked conformance"
                    .into(),
            );
            }
        }
        Ok(())
    }

    pub fn validate_selected_operators(
        program: &TypedTrees,
        selected: &[SelectedBuildTimeBinaryOperator],
    ) -> Result<(), String> {
        if selected
            .iter()
            .any(|operator| operator.has_crash_contract(program))
        {
            miss!("selected operator crash invocations have no build-time execution support");
        }
        let Some(facts) = require_unique_operator_custody(
            program,
            selected,
            "selected build-time operator",
            |row| {
                (!row.provider.is_zero()).then_some((row.expression, row.origin.machine_symbol()))
            },
        )?
        else {
            return Ok(());
        };
        for row in selected.iter() {
            let occurrences = facts
                .uses
                .iter()
                .filter(|(_, fact)| fact.occurrence == CheckedOperatorOccurrence::Expression)
                .map(|(_, fact)| (fact.expression, fact.origin))
                .chain(
                    facts
                        .named_uses()
                        .map(|fact| (fact.expression, fact.origin)),
                )
                .chain(
                    facts
                        .named_requirement_uses()
                        .map(|fact| (fact.expression, fact.origin)),
                )
                .filter(|(expression, origin)| {
                    *expression == row.expression
                        && origin.machine_symbol() == row.origin.machine_symbol()
                })
                .count();
            if occurrences != 1 {
                miss!("selected build-time expression is shared by distinct current origins");
            }
            let mut first_selection = None;
            let mut multiple_selections = false;
            facts.for_each_resolved_expression_selection(
                row.expression,
                |origin, requirement, policy, _| {
                    if origin == row.origin {
                        if first_selection.is_none() {
                            first_selection = Some((requirement, policy));
                        } else {
                            multiple_selections = true;
                        }
                    }
                },
            );
            let Some((requirement, policy)) = first_selection.filter(|_| !multiple_selections)
            else {
                return Err("selected build-time operator has no unique current use".into());
            };
            if requirement != row.requirement || policy != row.policy {
                return Err("selected build-time operator differs from current requirement or arithmetic policy".into());
            }
            let operands = match program.expression_table.expression(row.expression) {
                ExpressionNode::Binary(binary) if binary.operator == row.operation => {
                    [binary.left, binary.right]
                }
                ExpressionNode::Call(call) => {
                    let [left, right] = program.expression_table.expression_handles(call.arguments)
                    else {
                        return Err("selected build-time binary requirement has wrong arity".into());
                    };
                    [*left, *right]
                }
                _ => return Err("selected build-time binary expression disappeared".into()),
            };
            if operands != row.operands {
                miss!("selected build-time operation differs from current authored selection");
            }
            let matching: Vec<_> = program
                .operators()
                .iter()
                .chain(program.machine_token_bindings())
                .filter(|operator| operator.symbol == row.requirement)
                .collect();
            let [operator] = matching.as_slice() else {
                return Err("selected build-time requirement is not unique".into());
            };
            if let Some(requirement) = program.machine(row.requirement) {
                let [entry] = program.machine_states(requirement) else {
                    return Err("selected build-time Float requirement has no unique entry".into());
                };
                if requirement.supply_mode
                    != language_semantics::MachineSupplyMode::TopLevelRequirement
                    || requirement.body_is_present
                    || requirement.spelling != operator.spelling
                    || entry.parameters != operator.parameters
                    || entry.return_type != operator.return_type
                    || requirement.type_parameters != operator.type_parameters
                    || requirement.contracts != operator.contracts
                {
                    miss!("selected build-time token view differs from its actual requirement");
                }
            }
            if typed_trees::operator::primitive_float_binary_semantics(program, operator)
                != Some((row.operation, row.format))
            {
                miss!("selected build-time operation has no matching sealed Float meaning");
            }
            let [left, right] = program.operator_parameters(operator) else {
                return Err("selected build-time binary requirement has wrong arity".into());
            };
            let primitive = match row.format {
                numerics::literals::FloatFormat::F32 => typed_trees::types::PrimitiveType::F32,
                numerics::literals::FloatFormat::F64 => typed_trees::types::PrimitiveType::F64,
            };
            if !operator.is_boundary
                || program.primitive_type_reference(left.type_reference) != Some(primitive)
                || program.primitive_type_reference(right.type_reference) != Some(primitive)
            {
                return Err(
                    "selected build-time requirement has incompatible boundary or operand carriers"
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn retain_specialized_operators(
        program: &TypedTrees,
        selected: &[SelectedBuildTimeBinaryOperator],
    ) -> Result<Vec<SelectedBuildTimeBinaryOperator>, String> {
        validate_selected_operators(program, selected)?;
        if selected.is_empty() {
            return Ok(Vec::new());
        }
        let facts = validation::derive_pre_flow_operator_selections(program);
        let mut retained = selected.to_vec();
        for (expression, origin, requirement, policy) in facts.resolved_expression_selections() {
            let Some(owner) = origin.machine_symbol() else {
                continue;
            };
            if selected
                .iter()
                .any(|row| row.expression == expression && row.origin == origin)
            {
                continue;
            }
            for source in selected {
                if source.requirement != requirement || source.policy != policy {
                    continue;
                }
                let Some(source_owner) = source.origin.machine_symbol() else {
                    continue;
                };
                if !specialization_copies_template(program, source_owner, owner)? {
                    continue;
                }
                let Some(occurrence) =
                    direct_operator_occurrence(program, source.expression, requirement)?
                else {
                    continue;
                };
                if direct_operator_occurrence(program, expression, requirement)? != Some(occurrence)
                {
                    continue;
                }
                let row = SelectedBuildTimeBinaryOperator::prepare(
                    program,
                    expression,
                    origin,
                    requirement,
                    policy,
                    source.provider,
                )?;
                if row.operation != source.operation || row.format != source.format {
                    return Err(
                        "specialized build-time operator changed its selected meaning".into(),
                    );
                }
                if let Some(previous) = retained
                    .iter()
                    .find(|previous| previous.expression == expression && previous.origin == origin)
                {
                    if previous != &row {
                        return Err(
                            "specialized build-time operator has competing supplied executions"
                                .into(),
                        );
                    }
                } else {
                    retained.push(row);
                }
            }
        }
        validate_selected_operators(program, &retained)?;
        Ok(retained)
    }

    fn direct_operator_occurrence(
        program: &TypedTrees,
        expression: ExpressionHandle,
        requirement: symbols::SymbolHandle,
    ) -> Result<Option<typed_trees::AuthoredDeclarationSelectionOccurrenceId>, String> {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionKind as Kind,
            AuthoredDeclarationSelectionLateBinding as LateBinding,
            AuthoredDeclarationSelectionTarget as Target,
        };
        let mut descendants = Vec::new();
        collect_expression_tree(program, expression, &mut descendants);
        let authored_call = match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => call.authored_call_selection,
            _ => None,
        };
        let mut selected = None;
        for occurrence in expression_table(program).authored_selection_occurrences(expression) {
            if authored_call.is_some_and(|own| own != occurrence) {
                continue;
            }
            if descendants.iter().any(|child| {
                *child != expression
                    && expression_table(program)
                        .authored_selection_occurrences(*child)
                        .any(|inherited| inherited == occurrence)
            }) {
                continue;
            }
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("selected build-time operation lost its own selection")?;
            if !matches!(selection.kind(), Kind::Operator | Kind::Call) {
                continue;
            }
            if authored_call.is_some() && selection.kind() != Kind::Call {
                return Err("selected build-time call lost its own call occurrence".into());
            }
            match (selection.kind(), selection.target()) {
                (Kind::Operator, Target::LateBound(LateBinding::CheckedOperator))
                | (Kind::Call, Target::LateBound(LateBinding::CheckedCall)) => {}
                (_, Target::Resolved(target))
                    if selects_requirement(program, target.selected_symbol(), requirement) => {}
                _ => {
                    return Err(
                        "selected build-time operation has conflicting own selection".into(),
                    );
                }
            }
            if selected.is_some_and(|previous| previous != occurrence) {
                return Err("selected build-time operation has ambiguous own selection".into());
            }
            selected = Some(occurrence);
        }
        if authored_call.is_some() && selected != authored_call {
            return Err("selected build-time call lost its attached own occurrence".into());
        }
        Ok(selected)
    }

    fn selects_requirement(
        program: &TypedTrees,
        target: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
    ) -> bool {
        target == requirement
            || program
                .machine_holding_state(target)
                .is_some_and(|machine| machine.symbol == requirement)
    }

    fn specialization_copies_template(
        program: &TypedTrees,
        source: symbols::SymbolHandle,
        instance: symbols::SymbolHandle,
    ) -> Result<bool, String> {
        if source == instance || program.machine_specialization(source).is_some() {
            return Ok(false);
        }
        let mut instances = program
            .machine_specializations
            .iter()
            .filter(|row| row.instance == instance);
        let Some(application) = instances.next() else {
            return Ok(false);
        };
        if instances.next().is_some() {
            return Err("selected operator copy has ambiguous specialization provenance".into());
        }
        if application.template != source {
            return Ok(false);
        }
        let (Some(template), Some(copy)) = (program.machine(source), program.machine(instance))
        else {
            return Ok(false);
        };
        let binding = expression_table(program)
            .static_machine_applications()
            .find(|(handle, _)| *handle == application.binding_application)
            .map(|(_, binding)| binding)
            .ok_or("selected operator copy lost its producer-owned static application")?;
        if application.template_parameters != template.type_parameters
            || binding.binding_declaration != source
            || binding.registration_operation != source
            || !binding.lexical_owner.is_valid()
        {
            return Err(
                "selected operator copy does not retain its original template application".into(),
            );
        }
        Ok(program.source_target_context(template) == program.source_target_context(copy))
    }
}
pub(crate) mod selected_target_bodies {
    use crate::build_time_evaluation::expression_table;
    use symbols::SymbolHandle;
    use typed_trees::{TypedTrees, expression::ExpressionNode, name::Identifier};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedBuildTimeTargetBody {
        head: SymbolHandle,
        member: SymbolHandle,
        target_scope: String,
    }

    impl SelectedBuildTimeTargetBody {
        pub fn prepare(
            program: &TypedTrees,
            head: SymbolHandle,
            member: SymbolHandle,
            target_scope: &str,
        ) -> Result<Self, String> {
            let row = Self {
                head,
                member,
                target_scope: target_scope.to_owned(),
            };
            row.validate(program)?;
            Ok(row)
        }

        fn validate(&self, program: &TypedTrees) -> Result<(), String> {
            let mut families = program
                .target_machine_families
                .iter()
                .filter(|family| family.canonical == self.head);
            let family = families
                .next()
                .ok_or("selected constant target body lost its exact family")?;
            if families.next().is_some()
                || self.target_scope.is_empty()
                || family
                    .members
                    .iter()
                    .filter(|member| {
                        member.machine == self.member && member.target.as_str() == self.target_scope
                    })
                    .count()
                    != 1
                || family
                    .members
                    .iter()
                    .filter(|member| member.target.as_str() == self.target_scope)
                    .count()
                    != 1
            {
                return Err(
                    "selected constant target body has ambiguous member or target custody"
                        .to_owned(),
                );
            }
            let head = program
                .machine_by_symbol(self.head)
                .ok_or("selected constant family lost its portable head")?;
            let member = program
                .machine_by_symbol(self.member)
                .ok_or("selected constant family lost its body")?;
            if program.symbols.symbol_package_identity(head.symbol)
                != program.symbols.symbol_package_identity(member.symbol)
            {
                return Err(
                    "selected constant family body crosses declaration package custody".to_owned(),
                );
            }
            if !head.target_family_interface
                || member.target_family_interface
                || !member.supplies_checked_body()
                || program.machine_authored_target(member) != Some(self.target_scope.as_str())
            {
                return Err(
                    "selected constant target body does not realize its retained declaration"
                        .to_owned(),
                );
            }
            let head_entry = program
                .machine_states(head)
                .first()
                .ok_or("selected constant family lost its head signature")?;
            let member_entry = program
                .machine_states(member)
                .first()
                .ok_or("selected constant family lost its body signature")?;
            let head_parameters = program.state_parameters(head_entry);
            let member_parameters = program.state_parameters(member_entry);
            if head_parameters.len() != member_parameters.len()
                || program.normalized_type_identity(head_entry.return_type)
                    != program.normalized_type_identity(member_entry.return_type)
                || !head_parameters
                    .iter()
                    .zip(member_parameters)
                    .all(|(head, member)| {
                        head.is_const == member.is_const
                            && head.is_mutable == member.is_mutable
                            && program.normalized_type_identity(head.type_reference)
                                == program.normalized_type_identity(member.type_reference)
                    })
            {
                return Err(
                    "selected constant target body differs from its portable signature".to_owned(),
                );
            }
            Ok(())
        }

        pub fn receipt(&self) -> typed_trees::constant_recipes::ConstantTargetBodySelection {
            typed_trees::constant_recipes::ConstantTargetBodySelection {
                head: self.head,
                member: self.member,
                target_scope: self.target_scope.clone(),
            }
        }
    }

    pub fn prepare_selected_target_family_calls(
        program: &mut TypedTrees,
        rows: &[SelectedBuildTimeTargetBody],
    ) -> Result<(), String> {
        for (index, row) in rows.iter().enumerate() {
            row.validate(program)?;
            if rows[..index].iter().any(|prior| prior.head == row.head) {
                return Err("constant family has competing selected bodies".to_owned());
            }
        }
        let mut bindings = std::collections::BTreeMap::new();
        for row in rows {
            let head = program
                .machine_by_symbol(row.head)
                .ok_or("constant family head disappeared")?;
            let head_entry = program
                .machine_states(head)
                .first()
                .ok_or("constant family signature disappeared")?;
            let member = program
                .machine_by_symbol(row.member)
                .ok_or("constant family body disappeared")?;
            let member_entry = program
                .machine_states(member)
                .first()
                .ok_or("constant family body entry disappeared")?;
            for symbol in [row.head, head_entry.symbol] {
                let key = (symbol.arena_index(), symbol.generation());
                let binding = (member_entry.symbol, member.name.as_str().to_owned());
                if bindings.insert(key, binding).is_some() {
                    return Err("constant family call has competing exact bindings".to_owned());
                }
            }
        }
        let expressions = typed_trees::references::collect_program_references(program).expressions;
        let mut selections = program.authored_declaration_selections().clone();
        for expression in expressions {
            let Some(call) = program.expression_table.call_node(expression) else {
                continue;
            };
            let Some((symbol, name)) = bindings.get(&(
                call.target_symbol.arena_index(),
                call.target_symbol.generation(),
            )) else {
                continue;
            };
            let (symbol, name) = (*symbol, name.clone());
            for occurrence in expression_table(program).authored_selection_occurrences(expression) {
                use language_semantics::declaration_selection::{
                    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
                };
                let selection = selections
                    .get(occurrence)
                    .ok_or("constant family call lost its source selection")?;
                if selection.target()
                    == AuthoredDeclarationSelectionTarget::LateBound(
                        AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    )
                {
                    selections
                        .finalize_late_bound(
                            occurrence,
                            AuthoredDeclarationSelectionLateBinding::CheckedCall,
                            call.target_symbol,
                        )
                        .map_err(|reason| {
                            format!("constant family source selection could not settle: {reason:?}")
                        })?;
                }
            }

            let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            call.target_symbol = symbol;
            call.target = Identifier::generated(name);
        }
        program.retain_authored_declaration_selections(selections);
        Ok(())
    }
}
