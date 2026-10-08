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
/// source call. This derives ownership only; checking still judges all operands.
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
        let Some(call) = program.expression_table.call_node(expression) else {
            continue;
        };
        if call.machine_arguments.is_empty() || call.authored_machine_application.is_valid() {
            continue;
        }
        let call = call.clone();
        let application = program.expression_table.retain_static_machine_application(
            call.target_symbol,
            call.target,
            &call.machine_arguments,
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = owner;
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            unreachable!("retaining a source application does not replace its call");
        };
        call.authored_machine_application = application;
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
                            && !call.authored_machine_application.is_valid())
                        .then(|| (handle, machine.symbol, call.clone()))
                    })
            })
        })
        .collect::<Vec<_>>();
    for (handle, owner, call) in statement_calls {
        let application = program.expression_table.retain_static_machine_application(
            call.target_symbol,
            call.target,
            &call.machine_arguments,
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = owner;
        let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
            unreachable!("retaining a source application does not replace its call");
        };
        call.authored_machine_application = application;
    }
}

pub(crate) fn bind_pre_specialization_authored_selections(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let contract_facts = arena::Arena::default();
    let mut resolutions = Vec::new();
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
