//! Bind authored member and call identities before specialization consumes
//! their generic parameter lists. This writes only a still-owned typed program;
//! final checking resolutions publish into CheckFacts instead.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use validation::selection_resolution::{
    AuthoredSelectionResolution, AuthoredSelectionResolutionTarget, authored_data_member,
    collect_checked_transition_target_selections, declaration_target, exact_named_operator_call,
    finalization_diagnostic, push_consistent_resolution,
};

/// Retain lexical custody before type folding or specialization consumes a
/// source call. Erased lifetime operands need this carrier even without a
/// spelled static list; runtime arity cannot decide whether a telescope exists.
/// This derives ownership only; checking still judges all operands.
pub(crate) fn retain_static_argument_contexts(program: &mut TypedTrees) {
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::references::{TypedReferences, visit_declarations, visit_machine};
    use typed_trees::statement::StatementNode;

    #[derive(Default)]
    struct SourceCalls {
        owner: SymbolHandle,
        data_owners: Vec<SymbolHandle>,
        visited: Vec<ExpressionHandle>,
        expressions: Vec<(ExpressionHandle, SymbolHandle)>,
    }

    impl Extend<SymbolHandle> for SourceCalls {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }

    impl TypedReferences for SourceCalls {
        fn declaration(&mut self, owner: SymbolHandle) -> bool {
            if !self.data_owners.contains(&owner) {
                return false;
            }
            self.owner = owner;
            self.visited.clear();
            true
        }

        fn expression(&mut self, expression: ExpressionHandle) -> bool {
            if self.visited.contains(&expression) {
                return false;
            }
            self.visited.push(expression);
            self.expressions.push((expression, self.owner));
            true
        }
    }

    let mut roots = SourceCalls {
        data_owners: program
            .data_definitions()
            .iter()
            .map(|data| data.symbol)
            .collect(),
        ..SourceCalls::default()
    };
    visit_declarations(program, &mut roots);
    // Only data and machines supply the type/lifetime scopes this admission
    // uses. Trait and mathematical descriptions keep their own checking law.
    for machine in program.machines() {
        roots.owner = machine.symbol;
        roots.visited.clear();
        visit_machine(program, &mut roots, machine);
    }
    for (expression, owner) in roots.expressions {
        let Some(call) = program.expression_table.source_call_node(expression) else {
            continue;
        };
        if !call.authored_machine_application.is_valid()
            && call.machine_arguments.is_empty()
            && !has_region_telescope(program, call.target_symbol)
        {
            continue;
        }
        if !call.authored_machine_application.is_valid() {
            let mut arguments = call.machine_arguments.to_vec();
            typed_trees::expression::StaticMachineArgument::retain_authored_positions(
                &mut arguments,
            );
            program
                .expression_table
                .source_call_node_mut(expression)
                .expect("retaining static positions preserves the original occurrence")
                .machine_arguments = arguments.into_boxed_slice();
        }
        // Inventory and specialization share one source issuer. In particular,
        // a local trait requirement has no boundary header or ordinary machine
        // specialization pass that could fill an absent declaration later.
        crate::monomorphization::applications::retain_authored_call(
            program,
            crate::monomorphization::CallSite::Expression(expression),
            owner,
        );
    }
    let statement_calls = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program.machine_states(machine).iter().flat_map(|state| {
                program
                    .statement_table
                    .iter_statements(state.statement_nodes)
                    .filter_map(|(handle, statement)| {
                        let call = statement.as_call()?;
                        (!call.machine_arguments.is_empty()
                            || has_region_telescope(program, call.target_symbol)
                            || call.authored_machine_application.is_valid())
                        .then(|| (handle, machine.symbol, call.clone()))
                    })
            })
        })
        .collect::<Vec<_>>();
    for (handle, owner, call) in statement_calls {
        if !call.authored_machine_application.is_valid() {
            let mut arguments = call.machine_arguments.to_vec();
            typed_trees::expression::StaticMachineArgument::retain_authored_positions(
                &mut arguments,
            );
            let StatementNode::Call(original) = program.statement_table.statement_mut(handle)
            else {
                unreachable!("retaining static positions preserves the original call");
            };
            original.machine_arguments = arguments.into_boxed_slice();
        }
        crate::monomorphization::applications::retain_authored_call(
            program,
            crate::monomorphization::CallSite::Statement(handle),
            owner,
        );
    }
    retain_receiver_contexts(program);
}

/// Explicit receiving data operands belong to the source occurrence,
/// independently of namespace qualifiers and static member operands. Run at
/// inventory and after result elaboration so each unique declaration parameter
/// rejoins its genuine stored application before executable retargeting.
/// Carrier synthesis retains independent member binders; the conformance
/// producer joins their exact original IDs with the actual receiving graph.
pub(crate) fn retain_receiver_contexts(program: &mut TypedTrees) {
    use crate::monomorphization::CallSite;
    use typed_trees::statement::StatementNode;

    let mut receivers = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut visited = Vec::new();
            for (before, handle) in state.statement_nodes.handles().enumerate() {
                let statement = program.statement_table.statement(handle);
                if let StatementNode::Call(call) = statement
                    && let Some(receiver) = validation::call_data_application_in_state(
                        program,
                        state,
                        before,
                        call.target_symbol,
                        call.authored_machine_application,
                        program.statement_table.expression_handles(call.arguments),
                    )
                {
                    receivers.push((
                        CallSite::Statement(handle),
                        machine.symbol,
                        call.target_symbol,
                        receiver,
                    ));
                }
                let mut expressions = Vec::new();
                validation::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    if visited.contains(&expression) {
                        continue;
                    }
                    visited.push(expression);
                    let Some(executed) = program.expression_table.call_node(expression) else {
                        continue;
                    };
                    let Some(call) = program.expression_table.source_call(executed) else {
                        continue;
                    };
                    let Some(receiver) = validation::call_data_application_in_state(
                        program,
                        state,
                        before,
                        executed.target_symbol,
                        call.authored_machine_application,
                        program.expression_table.expression_handles(call.arguments),
                    ) else {
                        continue;
                    };
                    receivers.push((
                        CallSite::Expression(expression),
                        machine.symbol,
                        executed.target_symbol,
                        receiver,
                    ));
                }
            }
        }
    }
    for (site, owner, target, receiver) in receivers {
        let application =
            crate::monomorphization::applications::retain_authored_call(program, site, owner);
        if !application.is_valid() {
            continue;
        }
        program
            .expression_table
            .set_static_machine_application_receiving_data_parameter(
                application,
                receiver.parameter,
            );
        let receiver = receiver.application;
        let previous = program
            .expression_table
            .static_machine_application(application)
            .enclosing_data_applications;
        if program
            .type_reference_table
            .type_reference_handles(previous)
            != receiver.enclosing_data_applications
        {
            let references = program.type_reference_table.insert_type_reference_handles(
                receiver.enclosing_data_applications.iter().copied(),
            );
            program
                .expression_table
                .set_static_machine_application_enclosing_data_applications(
                    application,
                    references,
                );
        }
        let Some(instance) = program
            .machine_by_symbol(target)
            .or_else(|| program.machine_holding_state(target))
        else {
            continue;
        };
        let instance_symbol = instance.symbol;
        // A namespace qualifier or an equal-layout carrier cannot issue
        // another machine's attachment. The selected place must reach the
        // exact nominal receiving declaration, whether its tuple is open or closed.
        let receiving = program
            .closed_data_definition(receiver.reference)
            .map(|data| data.symbol)
            .unwrap_or_else(|| {
                validation::machine_symbol_from_type_reference_handle(program, receiver.reference)
            });
        if receiving != instance.attached_data_symbol {
            continue;
        }
        let Ok(family) = program.authored_conformance_machine(instance) else {
            continue;
        };
        let family_symbol = family.symbol;
        if !program
            .expression_table
            .static_machine_application(application)
            .binding_declaration
            .is_valid()
        {
            // Independent member admission belongs to the actual derived
            // header. The conformance producer separately projects the source
            // family without replacing this call's authored owner.
            program
                .expression_table
                .static_machine_application_mut(application)
                .binding_declaration = if instance.generic_data_template.is_valid() {
                instance_symbol
            } else {
                family_symbol
            };
        }
        program.retain_machine_conformance_application(instance_symbol, application);
    }
}

fn has_region_telescope(
    program: &dyn validation::CheckingInput,
    target: symbols::SymbolHandle,
) -> bool {
    validation::call_target_state(program, target)
        .and_then(|state| validation::find_state(program, state.symbol))
        .is_some_and(|(machine, _)| !machine.lifetime_parameters.is_empty())
}

pub(crate) fn bind_pre_specialization_authored_selections(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let contract_facts = arena::Arena::default();
    let mut resolutions = Vec::new();
    // A generic bound's operation is known before any concrete implementation
    // exists. Preserve that declaration selection on the authored occurrence;
    // an unused template's contracts must not acquire primitive token meaning.
    for (expression, target) in validation::collect_pre_specialization_operator_selections(program)?
    {
        for occurrence in program
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                continue;
            };
            let binding = AuthoredDeclarationSelectionLateBinding::CheckedOperator;
            if selection.target() == AuthoredDeclarationSelectionTarget::LateBound(binding) {
                push_consistent_resolution(
                    &mut resolutions,
                    AuthoredSelectionResolution {
                        occurrence,
                        binding,
                        target,
                    },
                )?;
            }
        }
    }
    // One exact-owner index answers every late-bound member and call's
    // query in a single pass instead of a whole-program rescan each.
    let owner_index =
        validation::expression_owners::OwnerEnvironmentIndex::new(program, &contract_facts);
    for (expression, node) in program.expression_table.iter_expressions() {
        for occurrence in program
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return Err(Diagnostic::error(format!(
                    "expression retains unknown authored declaration selection occurrence {}",
                    occurrence.ordinal(),
                )));
            };
            let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
                continue;
            };
            let target = match (binding, node) {
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => validation::expression_owners::checked_member_target_from_exact_owner(
                    program,
                    &contract_facts,
                    expression,
                    member,
                    Some(&owner_index),
                )
                .map(|target| match target {
                    validation::expression_owners::OwnerMemberTarget::Declaration(symbol) => {
                        AuthoredSelectionResolutionTarget::Declaration(authored_data_member(
                            program, symbol,
                        ))
                    }
                    validation::expression_owners::OwnerMemberTarget::CollectionMeasure(measure) => {
                        AuthoredSelectionResolutionTarget::Intrinsic(measure.intrinsic())
                    }
                }),
                // Shorthand still naming a builtin has not acquired its
                // checked operand-directed float meaning. Keep its occurrence
                // open until call binding; integer uses finalize normally at
                // the end of checking. Named authored overrides are unaffected.
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) if program
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    .is_some_and(symbols::BuiltinFunction::is_float_shorthand) =>
                {
                    None
                }
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => validation::compiler_operations::classify_expression_call_intrinsic(
                    program, call,
                )
                .map(AuthoredSelectionResolutionTarget::Intrinsic)
                .or_else(|| {
                    exact_named_operator_call(program, call)
                        .and_then(|operator| declaration_target(operator.symbol))
                        .or_else(|| {
                            validation::expression_owners::checked_machine_call_target_from_exact_owner(
                                program,
                                &contract_facts,
                                expression,
                                call,
                                Some(&owner_index),
                            )
                            .and_then(declaration_target)
                        })
                }),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };
            push_consistent_resolution(
                &mut resolutions,
                AuthoredSelectionResolution {
                    occurrence,
                    binding,
                    target,
                },
            )?;
        }
    }
    collect_checked_transition_target_selections(program, &mut resolutions)?;

    let mut selections = program.authored_declaration_selections().clone();
    for resolution in resolutions {
        match resolution.target {
            AuthoredSelectionResolutionTarget::Declaration(symbol) => {
                selections.finalize_late_bound(resolution.occurrence, resolution.binding, symbol)
            }
            AuthoredSelectionResolutionTarget::Intrinsic(intrinsic) => {
                selections.finalize_intrinsic(resolution.occurrence, resolution.binding, intrinsic)
            }
        }
        .map_err(|error| finalization_diagnostic(resolution, error))?;
    }
    program.retain_authored_declaration_selections(selections);
    Ok(())
}
