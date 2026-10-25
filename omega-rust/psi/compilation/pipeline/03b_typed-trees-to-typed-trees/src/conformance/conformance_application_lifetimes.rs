//! Contextual lifetime resolution for generic conformance applications.
//!
//! This module consumes ordinary parameter-position borrow constraints. It is
//! deliberately separate from application closure: absent, ambiguous, and
//! conflicting regions remain errors rather than declaration-based guesses.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::name::Identifier;
use typed_trees::statement::{StatementHandle, StatementNode};
use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod argument_scopes;

#[derive(Clone, Copy)]
enum ElisionCallSite {
    Expression(ExpressionHandle),
    Statement(StatementHandle),
}

struct ElisionCall {
    site: ElisionCallSite,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    target: SymbolHandle,
    arguments: Vec<ExpressionHandle>,
    machine_arguments: Box<[StaticMachineArgument]>,
}

/// Resolve erased conformance regions from the same parameter-position borrow
/// facts used by ordinary calls. This runs before strict application closure:
/// it only fills a complete, uniquely constrained lifetime lane and otherwise
/// leaves the authored application untouched for the normal diagnostic.
pub(crate) fn resolve_elided_conformance_lifetimes(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let calls = collect_elision_calls(program);
    let mut diagnostics = Vec::new();
    for mut call in calls {
        let Some((callee_machine, callee_state)) = validation::find_state(program, call.target)
        else {
            continue;
        };
        let Some(caller_machine) = program.machine_by_symbol(call.caller_machine) else {
            continue;
        };
        // The caller state must resolve inside the caller machine itself; a
        // whole-program find could admit a same-named state stored elsewhere.
        let caller_state =
            validation::find_state_in_machine(program, call.caller_machine, call.caller_state);

        let mut call_lifetimes = Vec::<(String, String)>::new();
        let mut conflicting_call_lifetime = false;
        let parameters = program.state_parameters(callee_state);
        let skip = parameters.len().saturating_sub(call.arguments.len());
        for (argument, parameter) in call.arguments.iter().zip(parameters.iter().skip(skip)) {
            let Some((actual, applications)) = caller_state
                .and_then(|state| argument_scopes::source_application(program, state, *argument))
            else {
                continue;
            };
            // A formed borrow presents the declared place as its referent.
            // Its new outer loan has no authored region operand, but nested
            // reference and phantom operands still constrain the invocation.
            // An existing reference value keeps its original shell below.
            let required = if program
                .expression_table
                .borrow_expression(*argument)
                .is_some()
            {
                match program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                {
                    TypeReferenceNode::Reference { referee, .. } => *referee,
                    _ => parameter.type_reference,
                }
            } else {
                parameter.type_reference
            };
            let inferred = program.with_enclosing_data_identity_scopes(&applications, |frames| {
                visit_lifetime_bindings_in_scope(
                    program,
                    required,
                    actual,
                    &callee_machine.lifetime_parameters,
                    argument_scopes::Scope {
                        owner: caller_machine.symbol,
                        frames,
                    },
                    caller_machine.symbol,
                    RequiredLifetimeScope::Operand,
                    &mut |required, actual| {
                        if let Some(actual) = actual {
                            bind_lifetime(
                                &mut call_lifetimes,
                                required.as_str(),
                                actual.as_str(),
                                &mut conflicting_call_lifetime,
                            );
                        }
                    },
                );
            });
            // A missing supplying telescope leaves this argument unconstrained;
            // ordinary closure diagnoses it. It is never a caller-only scope.
            if inferred.is_none() {
                continue;
            }
        }
        if conflicting_call_lifetime {
            diagnostics.push(Diagnostic::error(format!(
                "call to `{}` has conflicting ordinary borrow constraints for an erased lifetime",
                callee_state.name
            )));
            continue;
        }

        let application = match call.site {
            ElisionCallSite::Expression(handle) => program
                .expression_table
                .call_node(handle)
                .map(|call| call.authored_machine_application),
            ElisionCallSite::Statement(handle) => program
                .statement_table
                .statement(handle)
                .as_call()
                .map(|call| call.authored_machine_application),
        };

        let evidence_bounds = callee_machine.conformance_bounds.iter().collect::<Vec<_>>();
        let mut evidence_index = 0usize;
        for argument_ordinal in 0..call.machine_arguments.len() {
            let mut selected = call.machine_arguments[argument_ordinal].clone();
            if !selected.symbol.is_valid()
                || !matches!(
                    program.symbols.get(selected.symbol).kind,
                    SymbolKind::Conformance | SymbolKind::ConformanceParameter
                )
            {
                continue;
            }
            let Some(bound) = evidence_bounds.get(evidence_index) else {
                break;
            };
            evidence_index += 1;
            if !matches!(
                program.symbols.get(selected.symbol).kind,
                SymbolKind::Conformance
            ) {
                continue;
            }
            let Some(conformance) = program.conformance_by_symbol(selected.symbol) else {
                continue;
            };
            if conformance.lifetime_parameters.is_empty() {
                continue;
            }
            let mut inferred = Vec::<(String, String)>::new();
            let mut conflict = false;
            for (declared, expected) in program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .iter()
                .zip(&bound.arguments)
            {
                collect_conformance_lifetime_bindings(
                    program,
                    *declared,
                    *expected,
                    &conformance.lifetime_parameters,
                    &call_lifetimes,
                    &mut inferred,
                    &mut conflict,
                );
            }
            let explicit = selected.application.as_ref().and_then(|application| {
                (application.lifetime_arguments.len() == conformance.lifetime_parameters.len())
                    .then(|| {
                        application
                            .lifetime_arguments
                            .iter()
                            .map(|argument| argument.as_str().to_owned())
                            .collect::<Vec<_>>()
                    })
            });
            let resolved = explicit.or_else(|| {
                conformance
                    .lifetime_parameters
                    .iter()
                    .map(|parameter| {
                        inferred.iter().find_map(|(name, value)| {
                            (name == parameter.as_str()).then(|| value.clone())
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|_| !conflict)
            });
            let Some(resolved) = resolved else {
                continue;
            };

            let machine_substitutions = static_substitutions(
                program.machine_type_parameters(callee_machine),
                &call.machine_arguments,
            );
            let conformance_substitutions = static_substitutions(
                program.conformance_type_parameters(conformance),
                selected
                    .application
                    .as_deref()
                    .map_or(&[], |app| &app.arguments),
            );
            let inferred_substitutions = conformance
                .lifetime_parameters
                .iter()
                .zip(&resolved)
                .map(|(parameter, argument)| (parameter.as_str().to_owned(), argument.clone()))
                .collect::<Vec<_>>();
            let declared_arguments = program
                .type_reference_table
                .type_reference_handles(conformance.arguments);
            let shapes_match = declared_arguments.len() == bound.arguments.len()
                && declared_arguments
                    .iter()
                    .zip(&bound.arguments)
                    .all(|(declared, expected)| {
                        typed_trees::type_identity::applied_type_identity_in_scope(
                            program,
                            *declared,
                            conformance.symbol,
                            call.caller_machine,
                            &[AppliedTypeIdentityScope {
                                declaration: conformance.symbol,
                                arguments: &conformance_substitutions,
                                lifetimes: &inferred_substitutions,
                            }],
                        )
                        .zip(typed_trees::type_identity::applied_type_identity_in_scope(
                            program,
                            *expected,
                            callee_machine.symbol,
                            call.caller_machine,
                            &[AppliedTypeIdentityScope {
                                declaration: callee_machine.symbol,
                                arguments: &machine_substitutions,
                                lifetimes: &call_lifetimes,
                            }],
                        ))
                        .is_some_and(|(declared, expected)| declared == expected)
                    });
            if !shapes_match {
                if selected
                    .application
                    .as_ref()
                    .is_some_and(|application| !application.lifetime_arguments.is_empty())
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "conformance `{}` supplies lifetime arguments that disagree with the call's ordinary borrow constraints",
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<unnamed>", |name| name.as_str())
                    )));
                }
                continue;
            }
            if let Some(application) = &mut selected.application
                && application.lifetime_arguments.is_empty()
            {
                application.lifetime_arguments = resolved
                    .into_iter()
                    .map(Identifier::generated)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
            }
            call.machine_arguments[argument_ordinal] = selected;
        }

        if let Some(application) = application.filter(|application| application.is_valid()) {
            let binding_declaration = program.lifetime_declaration_owner(callee_machine.symbol);
            let lifetimes = call_lifetimes
                .iter()
                .map(|(parameter, argument)| {
                    typed_trees::static_applications::StaticMachineLifetimeBinding {
                        parameter: Identifier::generated(parameter),
                        argument: Identifier::generated(argument),
                    }
                })
                .collect::<Vec<_>>();
            // Region-only ordinary calls have no type-specialization step to
            // bind their source telescope later. Retain its exact declaration
            // with the operands; an existing static selection keeps its owner.
            let source = program
                .expression_table
                .static_machine_application_mut(application);
            if !source.binding_declaration.is_valid() {
                source.binding_declaration = binding_declaration;
            }
            program
                .expression_table
                .set_static_machine_application_lifetimes(application, &lifetimes);
        }
        match call.site {
            ElisionCallSite::Expression(handle) => {
                let ExpressionNode::Call(current) = program.expression_table.expression_mut(handle)
                else {
                    continue;
                };
                current.machine_arguments = call.machine_arguments;
            }
            ElisionCallSite::Statement(handle) => {
                let StatementNode::Call(current) = program.statement_table.statement_mut(handle)
                else {
                    continue;
                };
                current.machine_arguments = call.machine_arguments;
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn collect_elision_calls(program: &dyn ::validation::CheckingInput) -> Vec<ElisionCall> {
    let mut calls = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            // One walk coordinate per statement scope: every expression visit
            // inside it collects under the same caller machine and state.
            let walk = ElisionWalk {
                program,
                caller_machine: machine.symbol,
                caller_state: state.symbol,
            };
            for handle in state.statement_nodes.handles() {
                match program.statement_table.statement(handle) {
                    StatementNode::Call(call) => {
                        calls.push(ElisionCall {
                            site: ElisionCallSite::Statement(handle),
                            caller_machine: machine.symbol,
                            caller_state: state.symbol,
                            target: call.target_symbol,
                            arguments: program
                                .statement_table
                                .expression_handles(call.arguments)
                                .to_vec(),
                            machine_arguments: call.machine_arguments.clone(),
                        });
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            walk.visit(*argument, &mut calls);
                        }
                    }
                    StatementNode::Expression(expression) => walk.visit(*expression, &mut calls),
                    StatementNode::LocalData(local) => walk.visit(local.initial_value, &mut calls),
                    StatementNode::Assignment(assignment) => {
                        walk.visit(assignment.target, &mut calls);
                        walk.visit(assignment.value, &mut calls);
                    }
                    StatementNode::AssemblyFact(fact) => {
                        for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                            walk.visit(expression, &mut calls);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    calls
}

/// The caller coordinate every elision call in one statement scope shares.
/// The walk's program, machine and state are bound once here; each visit
/// borrows them instead of re-threading the same five arguments per recursion.
struct ElisionWalk<'a> {
    program: &'a dyn ::validation::CheckingInput,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
}

impl ElisionWalk<'_> {
    /// Collect the call rows `expression` reaches, in pre-order, one row per
    /// expression site. `SavedSubject` and literal expressions carry none.
    fn visit(&self, expression: ExpressionHandle, calls: &mut Vec<ElisionCall>) {
        if !expression.is_valid()
            || calls.iter().any(
                |call| matches!(call.site, ElisionCallSite::Expression(handle) if handle == expression),
            )
        {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Call(call) => {
                calls.push(ElisionCall {
                    site: ElisionCallSite::Expression(expression),
                    caller_machine: self.caller_machine,
                    caller_state: self.caller_state,
                    target: call.target_symbol,
                    arguments: self
                        .program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                    machine_arguments: call.machine_arguments.clone(),
                });
                self.visit(call.receiver, calls);
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.visit(*argument, calls);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.visit(*value, calls);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                self.visit(atomic.value, calls);
                self.visit(atomic.result, calls);
            }
            ExpressionNode::Match(dispatch) => {
                self.program
                    .expression_table
                    .match_deep_all(dispatch, |handle| {
                        self.visit(handle, calls);
                        true
                    });
            }
            ExpressionNode::Binary(binary) => {
                self.visit(binary.left, calls);
                self.visit(binary.right, calls);
            }
            ExpressionNode::Cast(cast) => self.visit(cast.value, calls),
            ExpressionNode::Indexed(indexed) => {
                self.visit(indexed.collection, calls);
                self.visit(indexed.index, calls);
            }
            ExpressionNode::Member(member) => self.visit(member.receiver, calls),
            ExpressionNode::Borrow(inner) => self.visit(inner.target, calls),
            ExpressionNode::Range(range) => {
                self.visit(range.start, calls);
                self.visit(range.end, calls);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.visit(field.value, calls);
                }
            }
            ExpressionNode::Unary(unary) => self.visit(unary.operand, calls),
            typed_trees::literal_expression_pattern!() => {}
        }
    }
}

fn bind_lifetime(
    bindings: &mut Vec<(String, String)>,
    parameter: &str,
    argument: &str,
    conflict: &mut bool,
) {
    if let Some((_, existing)) = bindings.iter().find(|(name, _)| name == parameter) {
        if existing != argument {
            *conflict = true;
        }
    } else {
        bindings.push((parameter.to_owned(), argument.to_owned()));
    }
}

fn resolved_lifetime<'a>(bindings: &'a [(String, String)], name: &str) -> Option<&'a str> {
    bindings
        .iter()
        .rev()
        .find_map(|(parameter, argument)| (parameter == name).then_some(argument.as_str()))
}

/// An unapplied field schema still declares its own region parameters after
/// retaining its lexical owner. A supplied Type operand instead carries fixed
/// caller regions, even when it came from this same declaration recursively.
#[derive(Clone, Copy)]
enum RequiredLifetimeScope {
    Operand,
    DeclaredField(SymbolHandle),
}

/// Infer only from the exact declaration field selected by the constructor.
/// Keep the complete actual value type while resolving any enclosing source
/// applications; a newly formed borrow's outer shell is part of that value.
pub(crate) fn collect_declared_field_lifetime_bindings(
    program: &TypedTrees,
    declaration: &typed_trees::data::DataDefinition,
    field: &typed_trees::data::DataField,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
    actual: TypeReferenceHandle,
    bindings: &mut Vec<(String, String)>,
    conflict: &mut bool,
) -> Option<()> {
    let (owner, selected) = program.any_field_owner(field.symbol)?;
    if owner.symbol != declaration.symbol || selected.type_reference != field.type_reference {
        return None;
    }
    let required_scope = if declaration.generic_instance.is_none() {
        RequiredLifetimeScope::DeclaredField(declaration.symbol)
    } else {
        // An applied field is already an operand, not fresh schema vocabulary.
        RequiredLifetimeScope::Operand
    };
    let applications = argument_scopes::source_application(program, state, source)
        .map(|(_, applications)| applications)
        .unwrap_or_default();
    program.with_enclosing_data_identity_scopes(&applications, |frames| {
        visit_lifetime_bindings_in_scope(
            program,
            field.type_reference,
            actual,
            &declaration.lifetime_parameters,
            argument_scopes::Scope {
                owner: machine.symbol,
                frames,
            },
            machine.symbol,
            required_scope,
            &mut |required, actual| {
                if let Some(actual) = actual {
                    bind_lifetime(bindings, required.as_str(), actual.as_str(), conflict);
                }
            },
        );
    })
}

/// Read simultaneous parameter-position region operands, including reference
/// elision. The consumer owns conflict detection and whether elision is legal.
pub(crate) fn visit_lifetime_bindings(
    program: &dyn ::validation::CheckingInput,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[Identifier],
    visit: &mut impl FnMut(&Identifier, Option<&Identifier>),
) {
    visit_lifetime_bindings_in_scope(
        program,
        required,
        actual,
        parameters,
        argument_scopes::Scope {
            owner: SymbolHandle::invalid(),
            frames: &[],
        },
        SymbolHandle::invalid(),
        RequiredLifetimeScope::Operand,
        visit,
    );
}

fn visit_lifetime_bindings_in_scope(
    program: &dyn validation::CheckingInput,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[Identifier],
    actual_scope: argument_scopes::Scope<'_>,
    caller: SymbolHandle,
    required_scope: RequiredLifetimeScope,
    visit: &mut impl FnMut(&Identifier, Option<&Identifier>),
) {
    // A supplied Type operand is already in its caller's lexical region
    // scope. Its coincident spellings are not occurrences of this callee's
    // fresh lifetime parameters, even in recursive or same-owner selections.
    let owner = program
        .type_reference_table
        .operand_lifetime_owner(required);
    if owner.is_valid()
        && !matches!(required_scope, RequiredLifetimeScope::DeclaredField(declaration) if owner == declaration)
    {
        return;
    }
    if matches!(required_scope, RequiredLifetimeScope::DeclaredField(_))
        && !matches!(
            program.type_reference_table.operand_origin(required),
            typed_trees::types::TypeOperandOrigin::Lexical
        )
    {
        return;
    }
    let Some((actual, actual_scope)) = actual_scope.resolve(program, actual) else {
        return;
    };
    // A bare nominal supplies the same application with anonymous regions.
    // An omitted Generic region lane is equally anonymous. Report absence as
    // an operand so result application can retain its validated supplier scope;
    // never invent a caller lifetime or infer missing Type/Const arguments.
    let actual_nominal = match program.type_reference_table.type_reference(actual) {
        TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } => Some((
            *base_symbol,
            lifetime_arguments.as_slice(),
            program
                .type_reference_table
                .type_reference_handles(*arguments),
        )),
        TypeReferenceNode::Named { symbol, .. } => Some((*symbol, &[][..], &[][..])),
        _ => None,
    };
    if let TypeReferenceNode::Generic {
        base_symbol: required_base,
        lifetime_arguments: required_lifetimes,
        arguments: required_arguments,
        ..
    } = program.type_reference_table.type_reference(required)
        && let Some((actual_base, actual_lifetimes, actual_arguments)) = actual_nominal
        && required_base.is_valid()
        && *required_base == actual_base
    {
        for (position, required) in required_lifetimes.iter().enumerate() {
            if parameters
                .iter()
                .any(|parameter| parameter.as_str() == required.as_str())
            {
                let actual = actual_lifetimes
                    .get(position)
                    .and_then(|actual| actual_scope.argument(program, caller, actual));
                visit(required, actual.as_ref());
            }
        }
        for (required, actual) in program
            .type_reference_table
            .type_reference_handles(*required_arguments)
            .iter()
            .zip(actual_arguments)
        {
            visit_lifetime_bindings_in_scope(
                program,
                *required,
                *actual,
                parameters,
                actual_scope,
                caller,
                required_scope,
                visit,
            );
        }
        return;
    }
    match (
        program.type_reference_table.type_reference(required),
        program.type_reference_table.type_reference(actual),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: required_referee,
                lifetime: required_lifetime,
                ..
            },
            TypeReferenceNode::Reference {
                referee: actual_referee,
                lifetime: actual_lifetime,
                ..
            },
        ) => {
            if let Some(required) = required_lifetime
                && parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == required.as_str())
            {
                let actual = actual_lifetime
                    .as_ref()
                    .and_then(|actual| actual_scope.argument(program, caller, actual));
                visit(required, actual.as_ref());
            }
            visit_lifetime_bindings_in_scope(
                program,
                *required_referee,
                *actual_referee,
                parameters,
                actual_scope,
                caller,
                required_scope,
                visit,
            );
        }
        typed_trees::constrained_type_reference_pair!(required, actual) => {
            visit_lifetime_bindings_in_scope(
                program,
                *required,
                *actual,
                parameters,
                actual_scope,
                caller,
                required_scope,
                visit,
            )
        }
        (TypeReferenceNode::Constrained { base_type, .. }, _) => visit_lifetime_bindings_in_scope(
            program,
            *base_type,
            actual,
            parameters,
            actual_scope,
            caller,
            required_scope,
            visit,
        ),
        (
            TypeReferenceNode::Slice {
                element_type: required,
            },
            TypeReferenceNode::Slice {
                element_type: actual,
            },
        )
        | (
            TypeReferenceNode::FixedArray {
                element_type: required,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: actual,
                ..
            },
        ) => visit_lifetime_bindings_in_scope(
            program,
            *required,
            *actual,
            parameters,
            actual_scope,
            caller,
            required_scope,
            visit,
        ),
        _ => {}
    }
}

fn collect_conformance_lifetime_bindings(
    program: &dyn ::validation::CheckingInput,
    declared: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    parameters: &[Identifier],
    expected_lifetimes: &[(String, String)],
    bindings: &mut Vec<(String, String)>,
    conflict: &mut bool,
) {
    match (
        program.type_reference_table.type_reference(declared),
        program.type_reference_table.type_reference(expected),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: declared_referee,
                lifetime: declared_lifetime,
                ..
            },
            TypeReferenceNode::Reference {
                referee: expected_referee,
                lifetime: expected_lifetime,
                ..
            },
        ) => {
            if let (Some(declared), Some(expected)) = (declared_lifetime, expected_lifetime)
                && parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == declared.as_str())
                && let Some(expected) = resolved_lifetime(expected_lifetimes, expected.as_str())
            {
                bind_lifetime(bindings, declared.as_str(), expected, conflict);
            }
            collect_conformance_lifetime_bindings(
                program,
                *declared_referee,
                *expected_referee,
                parameters,
                expected_lifetimes,
                bindings,
                conflict,
            );
        }
        (
            TypeReferenceNode::Constrained {
                base_type: declared,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: expected,
                ..
            },
        ) => collect_conformance_lifetime_bindings(
            program,
            *declared,
            *expected,
            parameters,
            expected_lifetimes,
            bindings,
            conflict,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: declared_base,
                lifetime_arguments: declared_lifetimes,
                arguments: declared_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: expected_base,
                lifetime_arguments: expected_arguments,
                arguments,
                ..
            },
        ) if declared_base.is_valid() && declared_base == expected_base => {
            for (declared, expected) in declared_lifetimes.iter().zip(expected_arguments) {
                if parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == declared.as_str())
                    && let Some(expected) = resolved_lifetime(expected_lifetimes, expected.as_str())
                {
                    bind_lifetime(bindings, declared.as_str(), expected, conflict);
                }
            }
            for (declared, expected) in program
                .type_reference_table
                .type_reference_handles(*declared_arguments)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                )
            {
                collect_conformance_lifetime_bindings(
                    program,
                    *declared,
                    *expected,
                    parameters,
                    expected_lifetimes,
                    bindings,
                    conflict,
                );
            }
        }
        (
            TypeReferenceNode::Slice {
                element_type: declared,
            },
            TypeReferenceNode::Slice {
                element_type: expected,
            },
        )
        | (
            TypeReferenceNode::FixedArray {
                element_type: declared,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: expected,
                ..
            },
        ) => collect_conformance_lifetime_bindings(
            program,
            *declared,
            *expected,
            parameters,
            expected_lifetimes,
            bindings,
            conflict,
        ),
        _ => {}
    }
}

fn static_substitutions<'argument>(
    parameters: &[typed_trees::data::TypeParameter],
    arguments: &'argument [StaticMachineArgument],
) -> Vec<(SymbolHandle, IndexIdentityArgument<'argument>)> {
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            (
                parameter.symbol,
                IndexIdentityArgument::StaticArgument(argument),
            )
        })
        .collect()
}
