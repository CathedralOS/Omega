//! Admit authored type and const operands at the immutable checking boundary.
//!
//! Executable specialization may erase operands and constant folding may erase
//! a type root. Retained applications keep source declarations, payloads and
//! lexical owners; this pass judges those inputs, not clone names or canonical
//! strings. Tuple derivation shares carrier readers so it cannot erase a named
//! const's carrier or a literal's earlier landing.

use diagnostics::Diagnostic;
use numerics::literals::LandedIntegerType;
use symbols::SymbolKind;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::references::StaticArgumentCall;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod invocation_parameters;
mod requirement_predicates;
pub(crate) use invocation_parameters::invocation_parameters;

/// Formation of authored const recipes precedes substitution/canonical erasure.
/// Incomplete selections remain incomplete; this pass supplies no tuple or
/// executable authority and never uses the called machine's ensures.
pub fn validate_source_const_recipes(
    program: &dyn crate::CheckingInput,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let scopes = requirement_predicates::CallScopes::build(program);
    let expression_states = expression_call_states(program);
    let mut validate = |call, context| {
        let Some(state) = call_state(call, context, &expression_states) else {
            return;
        };
        let Some(caller) = program.machine_holding_state(state.symbol) else {
            return;
        };
        let parameters = match source_telescope(program, call) {
            Ok(Some((_, parameters))) => parameters,
            Ok(None) => return,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(reason));
                return;
            }
        };
        let arguments = ordinary_arguments(program, call);
        crate::value_custody::const_expressions::validate_argument_recipes(
            program,
            caller,
            state,
            parameters,
            &arguments,
            &mut diagnostics,
            formation_obligations,
        );
    };
    for (handle, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            validate(
                StaticArgumentCall::Expression(handle, call),
                scopes.context(handle),
            );
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call() {
                    validate(StaticArgumentCall::Statement(call), Some((machine, state)));
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn validate_authored_static_arguments(
    program: &dyn crate::CheckingInput,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = crate::TopLevelSymbols::build(program, &mut diagnostics);
    let scopes = requirement_predicates::CallScopes::build(program);
    let expression_states = expression_call_states(program);
    for (handle, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            validate_call(
                program,
                StaticArgumentCall::Expression(handle, call),
                scopes.context(handle),
                &expression_states,
                &symbols,
                &mut diagnostics,
                formation_obligations,
            );
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call() {
                    validate_call(
                        program,
                        StaticArgumentCall::Statement(call),
                        Some((machine, state)),
                        &expression_states,
                        &symbols,
                        &mut diagnostics,
                        formation_obligations,
                    );
                }
            }
        }
    }
    // The declaration owns a complete family, including tuples no source call
    // uses. Preparation can derive these rows but cannot admit their operands
    // or substituted predicates on the checker's behalf.
    for application in program.closed_requirement_applications() {
        let source = program
            .expression_table
            .static_machine_application(application.application);
        let arguments = program
            .expression_table
            .static_machine_application_arguments(application.application);
        let Some((owner, requirement)) = program.trait_machine_signature(application.template)
        else {
            diagnostics.push(Diagnostic::error(
                "closed requirement lost its authored declaration",
            ));
            continue;
        };
        // The application receipt owns its source route. Rebuilding only the
        // materialized argument tuple here loses enclosing and inherited
        // simultaneous frames, including captured regions in phantom operands.
        let scope = application
            .trait_scope
            .scope(program)
            .filter(|scope| scope.declaration.symbol == owner.symbol);
        let Some(signature) = scope
            .as_ref()
            .and_then(|scope| program.applied_callable_signature(scope, application.template))
        else {
            diagnostics.push(Diagnostic::error(
                "closed requirement lost its complete applied source telescope",
            ));
            continue;
        };
        if let Err(error) = validate_arguments(
            program,
            requirement.name.as_str(),
            program.state_signature_type_parameters(signature),
            arguments,
            None,
        ) {
            diagnostics.push(error);
        }
        requirement_predicates::validate_where_facts(
            program,
            owner,
            signature,
            arguments,
            source.lexical_owner,
            None,
            &mut diagnostics,
        );
    }
    if diagnostics.iter().any(Diagnostic::is_error) {
        Err(diagnostics)
    } else {
        Ok(())
    }
}

/// A static application names the authored callable telescope, whether its
/// declaration owns a body or is a trait requirement signature. Executable
/// instance identity cannot substitute for that source admission boundary.
fn source_telescope<'program>(
    program: &'program dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> Result<Option<(&'program str, &'program [typed_trees::data::TypeParameter])>, String> {
    let target = call.registration_operation(program);
    if let Some(machine) = program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
    {
        return Ok(Some((
            machine.name.as_str(),
            program.machine_type_parameters(machine),
        )));
    }
    let Some((_, source, scope)) = source_requirement(program, call)? else {
        return Ok(None);
    };
    let requirement = match scope {
        Some(scope) => program
            .applied_callable_signature(&scope, source.symbol)
            .or_else(|| {
                (scope.substitutions.is_empty() && scope.declaration.lifetime_parameters.is_empty())
                    .then_some(source)
            })
            .ok_or_else(|| {
                format!(
                    "source application of requirement `{}` lost its exact outer trait telescope",
                    source.name,
                )
            })?,
        None => source,
    };
    Ok(Some((
        requirement.name.as_str(),
        program.state_signature_type_parameters(requirement),
    )))
}

/// Preserve the occurrence's registration identity while rejoining its exact
/// declaring bound. Admission reads the outer trait substitution only: the
/// invocation's supplied operands must not substitute away their own checks.
fn source_requirement<'program>(
    program: &'program dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> Result<
    Option<(
        &'program typed_trees::trait_definition::TraitDefinition,
        &'program typed_trees::signature::StateSignature,
        Option<typed_trees::trait_definition::AppliedTraitScope<'program>>,
    )>,
    String,
> {
    let target = call.registration_operation(program);
    if let Some((owner, signature)) = program.trait_machine_signature(target) {
        return Ok(Some((owner, signature, None)));
    }
    let Some(machine) = program.machine_by_symbol(call.lexical_owner(program)) else {
        return Ok(None);
    };
    let Some(requirement) =
        super::calls::named_conformance_target_requirement(program, machine, target)?
    else {
        return Ok(None);
    };
    Ok(Some((
        requirement.application.declaration,
        requirement.signature,
        Some(requirement.application),
    )))
}

/// Machine conformance binders occupy a separate telescope. Remove only
/// operands joined to those exact binders by the retained application; the
/// remaining Type/Const/Machine operands keep their authored order. A kind
/// cursor here would admit permutations of the ordinary telescope.
fn ordinary_arguments<'program>(
    program: &'program dyn crate::CheckingInput,
    call: StaticArgumentCall<'program>,
) -> std::borrow::Cow<'program, [StaticMachineArgument]> {
    let arguments = call.arguments(program);
    let handle = call.application();
    if !handle.is_valid() {
        return std::borrow::Cow::Borrowed(arguments);
    }
    let application = program.expression_table.static_machine_application(handle);
    let Some(machine) = program.machine_by_symbol(application.binding_declaration) else {
        return std::borrow::Cow::Borrowed(arguments);
    };
    if !program
        .machine_by_symbol(call.registration_operation(program))
        .or_else(|| program.machine_holding_state(call.registration_operation(program)))
        .is_some_and(|source| source.symbol == machine.symbol)
    {
        return std::borrow::Cow::Borrowed(arguments);
    }
    if machine.conformance_bounds.is_empty() {
        return std::borrow::Cow::Borrowed(arguments);
    }
    let bindings = program
        .expression_table
        .static_machine_application_bindings(handle);
    let mut consumed = vec![false; bindings.len()];
    std::borrow::Cow::Owned(
        arguments
            .iter()
            .filter(|argument| {
                let evidence = bindings.iter().enumerate().position(|(index, binding)| {
                    !consumed[index]
                        && machine.conformance_bounds.iter().any(|bound| {
                            bound.binder == binding.parameter
                                && bindings.iter().filter(|other| other.parameter == bound.binder).count() == 1
                        })
                        && matches!(&binding.value,
                            typed_trees::static_applications::StaticMachineBindingValue::Conformance(actual)
                            if actual == *argument)
                });
                if let Some(index) = evidence {
                    consumed[index] = true;
                    false
                } else {
                    true
                }
            })
            .cloned()
            .collect(),
    )
}

fn expression_call_states<'program>(
    program: &'program dyn crate::CheckingInput,
) -> Vec<(typed_trees::expression::ExpressionHandle, &'program State)> {
    let mut states = Vec::new();
    let mut expressions = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                expressions.clear();
                for (_, root) in program.tables.expression_roots(statement) {
                    crate::expression_traversal::collect_expression_tree(
                        program,
                        root,
                        &mut expressions,
                    );
                }
                states.extend(expressions.iter().filter_map(|expression| {
                    program.expression_table.call_node(*expression)?;
                    Some((*expression, state))
                }));
            }
        }
    }
    states.sort_by_key(|(expression, _)| (expression.arena_index(), expression.generation()));
    states
}

fn expression_state<'program>(
    states: &'program [(typed_trees::expression::ExpressionHandle, &'program State)],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<&'program State> {
    let identity = (expression.arena_index(), expression.generation());
    let first = states.partition_point(|(candidate, _)| {
        (candidate.arena_index(), candidate.generation()) < identity
    });
    states.get(first).and_then(|(candidate, state)| {
        ((candidate.arena_index(), candidate.generation()) == identity).then_some(*state)
    })
}

fn call_state<'program>(
    call: StaticArgumentCall<'_>,
    context: Option<(&'program typed_trees::machine::Machine, &'program State)>,
    expression_states: &'program [(typed_trees::expression::ExpressionHandle, &'program State)],
) -> Option<&'program State> {
    if let Some((_, state)) = context {
        return Some(state);
    }
    let StaticArgumentCall::Expression(handle, _) = call else {
        return None;
    };
    expression_state(expression_states, handle)
}

fn validate_call(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
    context: Option<(&typed_trees::machine::Machine, &typed_trees::state::State)>,
    expression_states: &[(typed_trees::expression::ExpressionHandle, &State)],
    symbols: &crate::TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    if let Err(diagnostic) =
        crate::compiler_operations::validate_declaration_operands(program, call)
    {
        diagnostics.push(diagnostic);
        return;
    }
    let arguments = call.arguments(program);
    requirement_predicates::validate(program, call, context, arguments, diagnostics);
    // `Requirement<Arguments>::operation(...)` forms a trait application at
    // the call site. It owes the same argument kinds, const ranges and
    // property bounds as any authored type application of that trait.
    if let StaticArgumentCall::Expression(_, applied) = call
        && applied.requirement_application.is_valid()
    {
        match context {
            Some((machine, _)) => crate::validate_static_type_argument(
                program,
                crate::StaticTypeArgumentOwner::Machine(machine),
                applied.requirement_application,
                symbols,
                diagnostics,
                formation_obligations,
            ),
            None => diagnostics.push(Diagnostic::error(
                "applied requirement call has no retained caller machine",
            )),
        }
    }
    if arguments.is_empty() {
        return;
    }
    let telescope = match source_telescope(program, call) {
        Ok(telescope) => telescope,
        Err(reason) => {
            diagnostics.push(Diagnostic::error(reason));
            return;
        }
    };
    if let Some((name, parameters)) = telescope
        && !parameters.is_empty()
    {
        let state = call_state(call, context, expression_states);
        let ordinary = ordinary_arguments(program, call);
        if let Err(error) = validate_arguments(program, name, parameters, &ordinary, state) {
            diagnostics.push(error);
        }
        if let Some(state) = state
            && let Some(machine) = program.machine_holding_state(state.symbol)
        {
            crate::value_custody::const_expressions::validate_argument_recipes(
                program,
                machine,
                state,
                parameters,
                &ordinary,
                diagnostics,
                formation_obligations,
            );
        }
    }
    let owner = call.lexical_owner(program);
    let caller = crate::StaticTypeArgumentOwner::from_lexical_owner(program, owner);
    validate_types(
        program,
        caller,
        call,
        arguments,
        symbols,
        diagnostics,
        formation_obligations,
    );
}

fn validate_types(
    program: &dyn crate::CheckingInput,
    caller: Option<crate::StaticTypeArgumentOwner<'_>>,
    source_call: StaticArgumentCall<'_>,
    arguments: &[StaticMachineArgument],
    symbols: &crate::TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    for argument in arguments {
        // ConstExpression is a retained value recipe, not a type application.
        // Its exact formal kind/carrier was judged by validate_arguments;
        // retained tuple validation owns its formation obligations.
        if argument.type_reference.is_valid()
            && const_expression_argument(program, argument).is_none()
            && !is_runtime_value_expression(program, argument)
        {
            if let Some(caller) = caller {
                crate::validate_static_type_argument(
                    program,
                    caller,
                    argument.type_reference,
                    symbols,
                    diagnostics,
                    formation_obligations,
                );
            } else {
                let owner = source_call.lexical_owner(program);
                let owner_label = if owner.is_valid() {
                    format!(
                        "{:?} `{}`",
                        program.symbols.get(owner).kind,
                        program.symbols.name(owner)
                    )
                } else {
                    "<missing>".to_owned()
                };
                let target = source_call.registration_operation(program);
                let target = if target.is_valid() {
                    program.symbols.name(target)
                } else {
                    "<unresolved>"
                };
                let mut diagnostic = Diagnostic::error(format!(
                    "structural static type argument has no retained caller type/lifetime context for `{target}`; lexical owner {owner_label}"
                ));
                if let StaticArgumentCall::Expression(handle, _) = source_call {
                    diagnostic =
                        diagnostic.with_source_span(program.expression_table.source_span(handle));
                }
                diagnostics.push(diagnostic);
            }
        }
        if let Some(application) = &argument.application {
            validate_types(
                program,
                caller,
                source_call,
                &application.arguments,
                symbols,
                diagnostics,
                formation_obligations,
            );
        }
    }
}

pub fn spelling(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> Option<String> {
    typed_trees::typed_trees::static_const_argument_spelling(program.typed_program(), argument)
}

pub fn forwarded_type(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> TypeReferenceHandle {
    if !argument.symbol.is_valid() {
        return TypeReferenceHandle::invalid();
    }
    // A resolved forwarding operand may belong to a requirement signature
    // or an enclosing implementation telescope, not only an executable
    // machine. Read the exact declaration's kind through the shared index;
    // its carrier supplies neither a closed value nor substitution authority.
    match program
        .data_type_parameter_by_symbol(argument.symbol)
        .map(|parameter| &parameter.kind)
    {
        Some(
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference },
        ) => *type_reference,
        _ => TypeReferenceHandle::invalid(),
    }
}

/// An ordinary expression retained in the parser's const-expression payload.
/// Its selected binder determines whether it is runtime-capable or must close
/// statically. Names and evidence-shaped projections have separate payloads.
pub fn is_runtime_value_expression(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> bool {
    let TypeReferenceNode::ConstExpression(expression) = program
        .type_reference_table
        .type_reference(argument.type_reference)
    else {
        return false;
    };
    program.expression_table.expression_is_valid(*expression)
}

/// The shape of one ordinary runtime subject admissible into a `Value`
/// binder: a single-segment name carrying no static payload. A spelled name
/// that resolved to no static declaration still qualifies; the caller's value
/// scope decides whether it denotes a subject (`resolve_runtime_subject`).
/// A field projection and a call expression are the same kind of subject; they
/// are recognized by [`is_runtime_value_expression`] and `runtime_field_subject`
/// because the parser cannot see the binder kind.
pub fn is_runtime_value_subject(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> bool {
    !argument.type_reference.is_valid()
        && argument.application.is_none()
        && argument.evidence_projection.is_none()
        && argument.const_literal.is_none()
        && argument.path.len() == 1
        && (!argument.symbol.is_valid()
            || matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::Local | SymbolKind::Parameter
            ))
}

/// A structural Type operand has exactly one custody lane. Sharing this
/// judgment keeps conformance and machine telescopes from interpreting an
/// already-lowered applied Type as an absent named symbol.
pub(crate) fn validate_structural_type_operand(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> Result<(), &'static str> {
    if !argument.type_reference.is_valid()
        || !program
            .type_reference_table
            .contains_type_reference(argument.type_reference)
        || !argument.path.is_empty()
        || argument.symbol.is_valid()
        || argument.const_literal.is_some()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
        return Err("structural static type argument has invalid or mixed argument custody");
    }
    Ok(())
}

fn validate_arguments(
    program: &dyn crate::CheckingInput,
    name: &str,
    parameters: &[typed_trees::data::TypeParameter],
    arguments: &[StaticMachineArgument],
    state: Option<&State>,
) -> Result<(), Diagnostic> {
    let has_types = parameters
        .iter()
        .any(|parameter| matches!(parameter.kind, TypeParameterKind::Type));
    let has_values = parameters
        .iter()
        .any(|parameter| parameter.kind.is_const_or_value());
    for (ordinal, argument) in arguments.iter().enumerate() {
        // Specialization may group operands by kind only after admission.
        // Judge every operand against its authored slot, including forwarded
        // binders and retained expressions: a kind-specific cursor silently
        // accepts permutations when their carriers happen to match.
        let parameter = parameters.get(ordinal);
        if let Some(parameter) = parameter
            && matches!(parameter.kind, TypeParameterKind::Const { .. })
            && program
                .data_type_parameter_by_symbol(argument.symbol)
                .is_some_and(|source| matches!(source.kind, TypeParameterKind::Value { .. }))
        {
            return Err(Diagnostic::error(format!(
                "const parameter `{}` of `{name}` cannot accept runtime-capable Value binder `{}`",
                parameter.name,
                argument.display_name(),
            )));
        }
        let named_type = argument.symbol.is_valid()
            && (matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::BuiltinType | SymbolKind::Data
            ) || program
                .data_type_parameter_by_symbol(argument.symbol)
                .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Type)));
        // `power<Degree - 1>` is a const value expression. Its type-reference
        // payload is only the retained tree; it does not select a type binder.
        if const_expression_argument(program, argument).is_some()
            && !parameter.is_some_and(|parameter| parameter.kind.is_const_or_value())
        {
            return Err(Diagnostic::error(format!(
                "machine `{}` static argument {} requires its declared binder kind, not a const value",
                name,
                ordinal + 1
            )));
        }
        if parameter.is_some_and(|parameter| {
            parameter.kind.is_const_or_value()
                && const_expression_argument(program, argument).is_some_and(|expression| {
                    crate::value_custody::const_expressions::has_carrier(
                        program,
                        expression,
                        &parameter.kind,
                        program
                            .type_reference_table
                            .operand_lifetime_owner(argument.type_reference),
                    )
                })
        }) {
            continue;
        }
        let value_parameter = parameter
            .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }));
        let runtime_expression = is_runtime_value_expression(program, argument);
        let runtime_field = state.is_some_and(|state| {
            argument
                .evidence_projection
                .as_ref()
                .is_some_and(|projection| {
                    crate::runtime_field_subject(
                        program,
                        state,
                        &projection.term,
                        &projection.member,
                    )
                    .is_some()
                })
        });
        let admitted_runtime = value_parameter && (runtime_expression || runtime_field);
        if (argument.type_reference.is_valid() && !admitted_runtime) || (named_type && has_types) {
            if !parameters
                .get(ordinal)
                .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` static argument {} requires its declared binder kind, not a type argument",
                    name,
                    ordinal + 1
                )));
            }
        }
        if admitted_runtime {
            continue;
        }
        if value_parameter && let Some(projection) = &argument.evidence_projection {
            return Err(Diagnostic::error(format!(
                "value parameter of machine `{name}` received `{}.{}`, which is not a field of a local or parameter in this state",
                projection.term, projection.member
            )));
        }
        if argument.type_reference.is_valid() {
            validate_structural_type_operand(program, argument).map_err(Diagnostic::error)?;
            continue;
        }
        // An unused type binder is still a type, not an unsupplied data
        // constructor. Check the selected declaration before specialization
        // can erase this argument from the executable call.
        if named_type
            && argument.application.is_none()
            && program.symbols.get(argument.symbol).kind == SymbolKind::Data
            && let Some(definition) = program.data_definition_by_symbol(argument.symbol)
            && (!definition.type_parameters.is_empty()
                || !definition.lifetime_parameters.is_empty())
        {
            return Err(Diagnostic::error(format!(
                "static type argument `{}` selects generic data without its complete type/lifetime arguments",
                definition.name,
            )));
        }
        let declaration = program.const_declarations().iter().find(|declaration| {
            argument.symbol.is_valid() && declaration.symbol == argument.symbol
        });
        let forwarded = forwarded_type(program, argument);
        let forwarded_type = forwarded.is_valid().then_some(forwarded);
        if declaration.is_none()
            && forwarded_type.is_none()
            && spelling(program, argument).is_none()
        {
            let type_shaped = argument.type_reference.is_valid()
                || argument.symbol.is_valid()
                    && matches!(
                        program.symbols.get(argument.symbol).kind,
                        SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                    );
            if has_values && !has_types && type_shaped {
                return Err(Diagnostic::error(format!(
                    "machine `{}` requires a const value, not type `{}`",
                    name,
                    argument.display_name()
                )));
            }
            // A `Value` binder admits static arguments through the ordinary
            // const specialization path and ordinary runtime subjects through
            // the dynamic realization path. Anything else spelled here is
            // neither.
            if let Some(parameter) = parameter
                && matches!(parameter.kind, TypeParameterKind::Value { .. })
                && argument.application.is_none()
                && argument.evidence_projection.is_none()
                && !type_shaped
            {
                if is_runtime_value_subject(program, argument) {
                    continue;
                }
                let parameter_name = parameter.name.as_str();
                return Err(Diagnostic::error(format!(
                    "value parameter `{parameter_name}` of machine `{}` received `{}`, which \
                     is neither a static const value nor an ordinary runtime value subject",
                    name,
                    argument.display_name()
                )));
            }
            continue;
        }
        let Some(parameter) = parameter else {
            return Err(Diagnostic::error(format!(
                "machine `{}` has no const parameter for extra argument `{}`",
                name,
                argument.display_name()
            )));
        };
        let parameter_name = &parameter.name;
        let required = match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => type_reference,
            _ => {
                return Err(Diagnostic::error(format!(
                    "machine `{}` static argument {} requires its declared binder kind, not a const value",
                    name,
                    ordinal + 1
                )));
            }
        };
        if argument.type_reference.is_valid()
            || argument.application.is_some()
            || argument.evidence_projection.is_some()
        {
            return Err(Diagnostic::error(format!(
                "const argument `{}` must select a closed value or a const binder",
                argument.display_name()
            )));
        }
        if let Some(actual) = declaration
            .map(|declaration| declaration.declared_type)
            .or(forwarded_type)
            && program.normalized_type_identity(actual)
                != program.normalized_type_identity(*required)
        {
            return Err(Diagnostic::error(format!(
                "const parameter `{parameter_name}` of `{}` requires `{}`, but `{}` declares `{}`",
                name,
                program.display_type_reference(*required),
                argument.display_name(),
                program.display_type_reference(actual)
            )));
        }
        // Decimal specialization keys cannot erase a literal's earlier typed
        // landing. Only anonymous literals may land in the binder's carrier.
        if let Some(landing) = argument
            .const_literal
            .as_ref()
            .and_then(|literal| literal.landing())
        {
            let actual = match landing.landed_type {
                LandedIntegerType::I8 => PrimitiveType::I8,
                LandedIntegerType::I16 => PrimitiveType::I16,
                LandedIntegerType::I32 => PrimitiveType::I32,
                LandedIntegerType::I64 => PrimitiveType::I64,
                LandedIntegerType::U8 => PrimitiveType::U8,
                LandedIntegerType::U16 => PrimitiveType::U16,
                LandedIntegerType::U32 => PrimitiveType::U32,
                LandedIntegerType::U64 => PrimitiveType::U64,
                LandedIntegerType::Addr => PrimitiveType::Addr,
            };
            if program.type_reference_table.primitive_type(*required) != Some(actual) {
                return Err(Diagnostic::error(format!(
                    "const parameter `{parameter_name}` of `{}` requires `{}`, but its literal already landed as `{}`",
                    name,
                    program.display_type_reference(*required),
                    landing.landed_type.name(),
                )));
            }
        }
        // Public declaration identity is broader than proof-static atom identity.
        // Decode through the ordinary specialization reader even for unused
        // binders, whose Unit calls may otherwise need no substitution.
        if declaration.is_some() && spelling(program, argument).is_none() {
            return Err(Diagnostic::error(format!(
                "const argument `{}` has no eligible canonical value",
                argument.display_name()
            )));
        }
    }
    Ok(())
}

fn const_expression_argument(
    program: &dyn crate::CheckingInput,
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
