//! Apply source requirement predicates in the exact call's declaring scope.
//!
//! Retained static applications preserve the caller machine, not a state-local
//! selector's carrier. Recover that context from exact expression membership;
//! never choose a state by spelling or borrow an application from a provider.
//! The global source admission pass still checks original generic templates
//! and discarded arena calls. Missing or ambiguous context fails closed.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
use typed_trees::references::StaticArgumentCall;
use typed_trees::{machine::Machine, state::State, trait_definition::AppliedTraitScope};

/// Invocation-local membership, not another retained program representation.
pub(super) struct CallScopes<'program> {
    entries: Vec<(ExpressionHandle, &'program Machine, &'program State)>,
}

impl<'program> CallScopes<'program> {
    pub(super) fn build(program: &'program dyn crate::CheckingInput) -> Self {
        // Both applied trait telescopes and runtime value-family membership
        // need the call's exact state. A nongeneric trait can still declare a
        // generic method whose where clause relies on state Requires facts.
        if !program
            .machines()
            .iter()
            .any(|machine| !machine.where_facts.is_empty())
            && !program.traits().iter().any(|owner| {
                program
                    .trait_machine_signatures(owner)
                    .iter()
                    .any(|signature| !signature.where_facts.is_empty())
            })
        {
            return Self {
                entries: Vec::new(),
            };
        }
        let mut entries = Vec::new();
        let mut expressions = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                expressions.clear();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    for (_, root) in program.statement_table.expression_roots(statement) {
                        crate::expression_traversal::collect_expression_tree(
                            program,
                            root,
                            &mut expressions,
                        );
                    }
                }
                entries.extend(expressions.iter().filter_map(|expression| {
                    program.expression_table.call_node(*expression)?;
                    Some((*expression, machine, state))
                }));
            }
        }
        entries
            .sort_by_key(|(expression, _, _)| (expression.arena_index(), expression.generation()));
        Self { entries }
    }

    pub(super) fn context(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(&'program Machine, &'program State)> {
        let identity = (expression.arena_index(), expression.generation());
        let first = self.entries.partition_point(|(candidate, _, _)| {
            (candidate.arena_index(), candidate.generation()) < identity
        });
        let (_, machine, state) = self
            .entries
            .get(first)
            .filter(|entry| entry.0 == expression)?;
        if self
            .entries
            .get(first + 1)
            .is_some_and(|entry| entry.0 == expression)
        {
            return None;
        }
        Some((*machine, *state))
    }
}

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
    context: Option<(&Machine, &State)>,
    arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = call.registration_operation(program);
    let Some((owner, source)) = program.trait_machine_signature(target) else {
        validate_machine_where_facts(program, call, context, target, arguments, diagnostics);
        return;
    };
    if source.where_facts.is_empty() {
        return;
    }
    // This reader receives the semantic source call. Closed requirement
    // headers instead belong to the checked executable at the same occurrence;
    // neither target can donate the other's tuple or declaration authority.
    let executable = match call {
        StaticArgumentCall::Expression(expression, _) => {
            let Some(executable) = program.expression_table.call_node(expression) else {
                diagnostics.push(Diagnostic::error(
                    "static predicate admission lost its executable call occurrence",
                ));
                return;
            };
            executable.target_symbol
        }
        StaticArgumentCall::Statement(call) => call.target_symbol,
    };
    let scope = if let Some(application) = program.closed_requirement_application(executable) {
        AppliedTraitScope::new(
            program,
            owner,
            typed_trees::trait_definition::TraitApplicationArguments {
                lexical_owner: call.lexical_owner(program),
                enclosing_data_applications: &[],
                arguments: application.trait_arguments,
                lifetimes: &[],
            },
        )
        .map(|scope| (scope, false))
    } else {
        context
            .and_then(|(machine, state)| {
                let (receiver, operands) = match call {
                    StaticArgumentCall::Expression(_, call) => (
                        crate::machine_calls::calls::dynamic_requirement::expression_trait_receiver(
                            program,
                            call.receiver,
                        ),
                        program.expression_table.expression_handles(call.arguments),
                    ),
                    StaticArgumentCall::Statement(call) => (
                        call.receiver_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    ),
                };
                crate::machine_calls::calls::dynamic_requirement::dynamic_selector_call(
                    program,
                    machine,
                    Some(state),
                    receiver,
                    executable,
                    operands,
                )
                .ok()
                .flatten()
                .filter(|dynamic| dynamic.signature.symbol == target)
                .map(|dynamic| (dynamic.application, true))
            })
            .or_else(|| {
                program
                    .trait_type_parameters(owner)
                    .is_empty()
                    .then(|| (AppliedTraitScope::abstract_scope(owner), false))
            })
    };
    let (scope, resolved_dynamically) = match scope {
        Some((scope, dynamic)) => (Some(scope), dynamic),
        None => (None, false),
    };
    // A selector call through `dyn` whose applied tuple is complete, closed,
    // and outside the declared roster belongs to the dynamic-family roster
    // validator, which rejects it with the membership diagnostic; runtime
    // subjects stay here so this judgment can discharge their membership
    // through the caller's requires (or reject it unproved).
    if resolved_dynamically
        && let typed_trees::finite_family::FamilyProbe::Finite { arity, tuples } =
            program.finite_signature_family(source)
    {
        let applied = arguments
            .iter()
            .map(|argument| program.static_const_argument_identity(argument))
            .collect::<Option<Vec<_>>>();
        let closed_non_member = applied.is_some_and(|identities| {
            identities.len() == arity
                && !tuples
                    .iter()
                    .any(|tuple| tuple.identities.as_ref() == identities)
        });
        if closed_non_member {
            return;
        }
    }
    let signature = scope
        .as_ref()
        .and_then(|scope| program.applied_callable_signature(scope, target));
    let Some(signature) = signature else {
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove declared where facts for static application of requirement `{}::{}` under its complete applied telescope", owner.name, source.name,
        )));
        return;
    };
    validate_where_facts(
        program,
        owner,
        signature,
        arguments,
        call.lexical_owner(program),
        context.map(|(_, state)| state),
        diagnostics,
    );
}

pub(super) fn validate_where_facts(
    program: &dyn crate::CheckingInput,
    owner: &typed_trees::trait_definition::TraitDefinition,
    source: &typed_trees::signature::StateSignature,
    arguments: &[StaticMachineArgument],
    lexical_owner: SymbolHandle,
    caller_state: Option<&State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use crate::proof_contracts::contract_entailment::{
        StrictArithmeticImplicationJudgment, static_application_where_judgment,
    };
    let judgment = static_application_where_judgment(
        program,
        source,
        arguments,
        lexical_owner,
        caller_state,
        &[],
    );
    match judgment {
        StrictArithmeticImplicationJudgment::Proven => {}
        StrictArithmeticImplicationJudgment::Refuted => diagnostics.push(Diagnostic::error(format!(
            "static application of requirement `{}::{}` violates its declared where facts: the complete tuple is structurally false", owner.name, source.name,
        ))),
        StrictArithmeticImplicationJudgment::Unknown => diagnostics.push(Diagnostic::error(format!(
            "cannot prove declared where facts for static application of requirement `{}::{}` under its complete applied telescope", owner.name, source.name,
        ))),
    }
}

/// Ordinary calls use the retained original application, including inferred
/// Type/Const operands. Erased executable operands and receiving-data defaults
/// cannot donate the original predicate's tuple or binder correspondence.
fn validate_machine_where_facts(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
    context: Option<(&Machine, &State)>,
    target: SymbolHandle,
    authored_arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(selected) = program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
    else {
        return;
    };
    if selected.where_facts.is_empty() {
        return;
    }
    let application = call.application();
    let declaration_symbol = if application.is_valid() {
        program
            .expression_table
            .static_machine_application(application)
            .binding_declaration
    } else {
        selected.symbol
    };
    validate_machine_predicate_application(
        program,
        application,
        declaration_symbol,
        call.lexical_owner(program),
        context.map(|(_, state)| state),
        authored_arguments,
        diagnostics,
    );
}

pub(super) fn validate_machine_predicate_application(
    program: &dyn crate::CheckingInput,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    declaration_symbol: SymbolHandle,
    lexical_owner: SymbolHandle,
    caller_state: Option<&State>,
    authored_arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use crate::proof_contracts::contract_entailment::{
        StrictArithmeticImplicationJudgment, callable_where_judgment,
    };
    use typed_trees::static_applications::{
        StaticApplicationDeclaration, StaticApplicationView, StaticMachineBindingValue,
    };
    let Some(declaration) = program.machine_by_symbol(declaration_symbol) else {
        diagnostics.push(Diagnostic::error(
            "ordinary where application lost its original declaration",
        ));
        return;
    };
    if declaration.where_facts.is_empty() {
        return;
    }
    let projected = if application.is_valid() {
        StaticApplicationView::new(program.typed_program(), application)
            .map(|view| {
                let parameters = view.declaration().parameters().to_vec();
                let mut arguments = Vec::with_capacity(parameters.len());
                for binding in view.bindings().iter().take(parameters.len()) {
                    let argument = match &binding.value {
                        StaticMachineBindingValue::Type(reference) => StaticMachineArgument {
                            type_reference: *reference,
                            ..Default::default()
                        },
                        StaticMachineBindingValue::Const(reference) => program
                            .typed_program()
                            .static_operand_from_type_reference(*reference)
                            .map_err(
                                |_| "ordinary where const operand has no retained source meaning",
                            )?,
                        StaticMachineBindingValue::RuntimeValue { subject, .. } => {
                            StaticMachineArgument {
                                symbol: *subject,
                                ..Default::default()
                            }
                        }
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                    };
                    arguments.push(argument);
                }
                Ok((parameters, arguments))
            })
            .and_then(|projection| projection)
    } else {
        StaticApplicationDeclaration::for_symbol(program.typed_program(), declaration.symbol).map(
            |declaration| {
                (
                    declaration.parameters().to_vec(),
                    authored_arguments.to_vec(),
                )
            },
        )
    };
    let (parameters, arguments) = match projected {
        Ok(projected) => projected,
        Err(reason) => {
            diagnostics.push(Diagnostic::error(reason));
            return;
        }
    };
    // Operand categories/carriers/bounds precede predicate membership. The
    // projection is the full original roster, not the authored explicit slice.
    if let Err(error) = super::validate_argument_telescope(
        program,
        declaration.name.as_str(),
        &parameters,
        &arguments,
    ) {
        diagnostics.push(error);
        return;
    }
    let runtime_parameters = program
        .machine_states(declaration)
        .first()
        .map(|entry| program.state_parameters(entry))
        .unwrap_or_default();
    let judgment = callable_where_judgment(
        program,
        declaration.symbol,
        &parameters,
        runtime_parameters,
        declaration.where_facts,
        &arguments,
        lexical_owner,
        caller_state,
        &[],
    );
    match judgment {
        StrictArithmeticImplicationJudgment::Proven => {}
        StrictArithmeticImplicationJudgment::Refuted => diagnostics.push(Diagnostic::error(format!(
            "static application of machine `{}` violates its declared where facts: the complete tuple is structurally false", declaration.name,
        ))),
        StrictArithmeticImplicationJudgment::Unknown => diagnostics.push(Diagnostic::error(format!(
            "cannot prove declared where facts for static application of machine `{}` under its complete applied telescope", declaration.name,
        ))),
    }
}
