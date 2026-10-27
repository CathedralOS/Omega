//! Retain executable Value indices in the descriptor fields created by syntax
//! lowering. A destination supplies the application; construction reads each
//! argument once at that point. This derives ordinary field expressions only:
//! immutable checking still proves their equality to the captured index.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteralField};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

type Substitutions = Vec<(SymbolHandle, ExpressionHandle)>;

/// Immutable preparation-time caller judgments. Generated descriptor reads add
/// no writes, so mutation of constructor field lists cannot enlarge this set.
struct CallerEffects {
    machine: SymbolHandle,
    track_effects: bool,
    no_write_calls: Vec<ExpressionHandle>,
}

impl CallerEffects {
    fn inert(&self, program: &TypedTrees, expression: ExpressionHandle) -> bool {
        if !self.track_effects {
            return true;
        }
        validation::expression_traversal::inert_value_tree_with_operations(
            program,
            expression,
            |operation| match program.expression_table.expression(operation) {
                ExpressionNode::Call(_) => self
                    .no_write_calls
                    .binary_search_by_key(
                        &(operation.arena_index(), operation.generation()),
                        |candidate| (candidate.arena_index(), candidate.generation()),
                    )
                    .is_ok(),
                ExpressionNode::Binary(_) => {
                    program
                        .machine_by_symbol(self.machine)
                        .is_some_and(|machine| {
                            validation::has_builtin_binary_expression_meaning(
                                program, machine, None, operation,
                            )
                        })
                }
                _ => false,
            },
        )
    }
}

fn contains_reachable_struct_literal(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(_) => true,
        ExpressionNode::Borrow(borrow) => {
            contains_reachable_struct_literal(program, borrow.target)
        }
        ExpressionNode::Atomic(atomic) => {
            contains_reachable_struct_literal(program, atomic.value)
        }
        ExpressionNode::Cast(cast) => contains_reachable_struct_literal(program, cast.value),
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .any(|arm| contains_reachable_struct_literal(program, arm.value)),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| contains_reachable_struct_literal(program, *element)),
        _ => false,
    }
}

pub(super) fn materialize(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let has_value_parameter_data = program
        .data_definitions()
        .iter()
        .any(|definition| program.data_value_parameter_fields(definition).next().is_some());
    let mut destinations = Vec::new();
    let mut calls = Vec::new();
    let mut caller_effects = Vec::new();
    let mut caller_effects_by_machine: symbols::SymbolKeyMap<SymbolHandle, usize> =
        symbols::SymbolKeyMap::default();
    let checking_context = has_value_parameter_data.then(|| validation::CheckingContext::new(program));
    let frames = checking_context
        .as_ref()
        .and_then(|context| validation::CallFrameResolver::new(context));
    let sidecars = has_value_parameter_data
        .then(|| typed_trees::references::MachineSidecarIndex::build(program));
    for machine in program.machines() {
        let mut no_write_calls = Vec::new();
        let mut statement_calls = Vec::new();
        let mut machine_has_struct_destination_or_call = false;
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::LocalData(local) => {
                        if contains_reachable_struct_literal(program, local.initial_value) {
                            machine_has_struct_destination_or_call = true;
                            destinations.push((
                                machine.symbol,
                                local.initial_value,
                                local.type_reference,
                                Vec::new(),
                            ));
                        }
                    }
                    StatementNode::Expression(value) if state.return_type.is_valid() => {
                        if contains_reachable_struct_literal(program, *value) {
                            machine_has_struct_destination_or_call = true;
                            destinations.push((
                                machine.symbol,
                                *value,
                                state.return_type,
                                Vec::new(),
                            ));
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        if contains_reachable_struct_literal(program, assignment.value)
                            && let Some(reference) = validation::expression_result_type_reference(
                                program,
                                machine,
                                state,
                                assignment.target,
                            )
                        {
                            machine_has_struct_destination_or_call = true;
                            destinations.push((
                                machine.symbol,
                                assignment.value,
                                reference,
                                Vec::new(),
                            ));
                        }
                    }
                    StatementNode::Call(call) => {
                        let arguments = program
                            .statement_table
                            .expression_handles(call.arguments);
                        if arguments
                            .iter()
                            .any(|arg| contains_reachable_struct_literal(program, *arg))
                        {
                            machine_has_struct_destination_or_call = true;
                            calls.push((
                                machine.symbol,
                                call.target_symbol,
                                arguments.to_vec(),
                            ));
                        }
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(value) => {
                                    if contains_reachable_struct_literal(program, *value) {
                                        machine_has_struct_destination_or_call = true;
                                        destinations.push((
                                            machine.symbol,
                                            *value,
                                            state.return_type,
                                            Vec::new(),
                                        ));
                                    }
                                }
                                TransitionTargetNode::Named {
                                    path, arguments, ..
                                } => {
                                    let arguments = program
                                        .statement_table
                                        .expression_handles(*arguments);
                                    if arguments.iter().any(|arg| {
                                        contains_reachable_struct_literal(program, *arg)
                                    }) {
                                        machine_has_struct_destination_or_call = true;
                                        calls.push((
                                            machine.symbol,
                                            path.symbol,
                                            arguments.to_vec(),
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
                let mut expressions = Vec::new();
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    if let Some(call) = program.expression_table.call_node(expression) {
                        if has_value_parameter_data {
                            statement_calls.push(expression);
                        }
                        let arguments = program
                            .expression_table
                            .expression_handles(call.arguments);
                        if arguments
                            .iter()
                            .any(|arg| contains_reachable_struct_literal(program, *arg))
                        {
                            machine_has_struct_destination_or_call = true;
                            calls.push((
                                machine.symbol,
                                call.target_symbol,
                                arguments.to_vec(),
                            ));
                        }
                    }
                }
            }
        }
        if has_value_parameter_data && machine_has_struct_destination_or_call {
            for expression in statement_calls {
                if frames.as_ref().is_some_and(|frames| {
                    frames
                        .expression_write_frame(machine, expression)
                        .complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                }) {
                    no_write_calls.push(expression);
                }
            }
            let references = typed_trees::references::collect_machine_references(
                program,
                machine,
                sidecars.as_ref().expect("built sidecars"),
            );
            for expression in references.expressions {
                if program.expression_table.call_node(expression).is_some()
                    && frames.as_ref().is_some_and(|frames| {
                        frames.declaration_expression_has_empty_write_frame(
                            expression,
                            &|operation| {
                                validation::has_builtin_binary_expression_meaning(
                                    program, machine, None, operation,
                                )
                            },
                        )
                    })
                {
                    no_write_calls.push(expression);
                }
            }
            no_write_calls.sort_by_key(|call| (call.arena_index(), call.generation()));
            no_write_calls.dedup();
        } else {
            no_write_calls.clear();
        }
        let effect_index = caller_effects.len();
        caller_effects.push(CallerEffects {
            machine: machine.symbol,
            track_effects: has_value_parameter_data,
            no_write_calls,
        });
        caller_effects_by_machine.insert(machine.symbol, effect_index);
    }
    drop(frames);
    drop(checking_context);
    for (caller, target, arguments) in calls {
        let effects = &caller_effects[*caller_effects_by_machine
            .get(&caller)
            .expect("collected caller effects")];
        let Some(parameters) = validation::call_target_parameters(program, target) else {
            continue;
        };
        let substitutions: Substitutions = if has_value_parameter_data {
            let can_reobserve = arguments
                .iter()
                .all(|argument| effects.inert(program, *argument));
            parameters
                .iter()
                .zip(&arguments)
                .map(|(parameter, argument)| {
                    let stable_literal = matches!(
                        program.expression_table.expression(*argument),
                        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                    );
                    // A Name is an observation, not a snapshot of an earlier call
                    // operand. Until ordinary argument preparation retains that
                    // snapshot, any possible intervening effect makes reuse unsafe.
                    let retained = if can_reobserve || stable_literal {
                        *argument
                    } else {
                        ExpressionHandle::invalid()
                    };
                    (parameter.symbol, retained)
                })
                .collect()
        } else {
            Vec::new()
        };
        destinations.extend(
            parameters
                .iter()
                .zip(arguments)
                .filter(|(_, argument)| contains_reachable_struct_literal(program, *argument))
                .map(|(parameter, argument)| {
                    (
                        caller,
                        argument,
                        parameter.type_reference,
                        substitutions.clone(),
                    )
                }),
        );
    }
    let mut diagnostics = Vec::new();
    for (caller, expression, reference, substitutions) in destinations {
        let effects = &caller_effects[*caller_effects_by_machine
            .get(&caller)
            .expect("collected caller effects")];
        fill(
            program,
            expression,
            reference,
            &substitutions,
            effects,
            false,
            &mut diagnostics,
        );
    }
    let const_declarations: Vec<_> = program
        .const_declarations()
        .iter()
        .flat_map(|declaration| {
            [
                (
                    declaration.materialized_initializer,
                    declaration.declared_type,
                ),
                (declaration.authored_initializer, declaration.declared_type),
            ]
        })
        .filter(|(value, _)| contains_reachable_struct_literal(program, *value))
        .collect();
    let const_effects = CallerEffects {
        machine: SymbolHandle::invalid(),
        track_effects: has_value_parameter_data,
        no_write_calls: Vec::new(),
    };
    for (value, declared_type) in const_declarations {
        if value.is_valid() {
            fill(
                program,
                value,
                declared_type,
                &Vec::new(),
                &const_effects,
                false,
                &mut diagnostics,
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn fill(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    substitutions: &Substitutions,
    effects: &CallerEffects,
    earlier_effects: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() || !reference.is_valid() {
        return;
    }
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    match program.expression_table.expression(expression).clone() {
        ExpressionNode::Borrow(borrow) => {
            if let TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(reference)
            {
                let referee = *referee;
                fill(
                    program,
                    borrow.target,
                    referee,
                    substitutions,
                    effects,
                    earlier_effects,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => fill(
            program,
            atomic.value,
            reference,
            substitutions,
            effects,
            true,
            diagnostics,
        ),
        ExpressionNode::Match(dispatch) => {
            let arms = program.expression_table.match_arms(dispatch.arms).to_vec();
            let mut prior = earlier_effects || !effects.inert(program, dispatch.subject);
            for arm in arms {
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    prior |= !effects.inert(program, pattern);
                }
                fill(
                    program,
                    arm.value,
                    reference,
                    substitutions,
                    effects,
                    prior,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Cast(cast) => fill(
            program,
            cast.value,
            cast.target_type,
            substitutions,
            effects,
            earlier_effects,
            diagnostics,
        ),
        ExpressionNode::ArrayLiteral(elements) => {
            if let TypeReferenceNode::FixedArray { element_type, .. } =
                program.type_reference_table.type_reference(reference)
            {
                let element_type = *element_type;
                let elements = program
                    .expression_table
                    .expression_handles(elements)
                    .to_vec();
                let mut prior = earlier_effects;
                for element in elements {
                    fill(
                        program,
                        element,
                        element_type,
                        substitutions,
                        effects,
                        prior,
                        diagnostics,
                    );
                    prior |= !effects.inert(program, element);
                }
            }
        }
        ExpressionNode::StructLiteral(mut literal) => {
            // This is a required constructor application, not the enclosing
            // statement's result (which can be Unit when a value is discarded).
            if program
                .data_constructor_definition(reference, literal.type_symbol)
                .is_some()
            {
                let prior = program
                    .expression_table
                    .construction_required_type_reference(expression);
                if prior.is_valid() {
                    if program.normalized_type_identity(prior)
                        != program.normalized_type_identity(reference)
                    {
                        diagnostics.push(Diagnostic::error("constructor occurrence has conflicting receiving applications; preserve separate evaluation contexts"));
                        return;
                    }
                    // Equivalent repeated preparation must not replace the
                    // issuing occurrence's exact retained application row.
                } else {
                    program
                        .expression_table
                        .set_construction_required_type_reference(expression, reference);
                }
            }
            let mut fields = program
                .expression_table
                .struct_fields(literal.fields)
                .to_vec();
            let descriptors: Vec<_> = program
                .data_application_value_fields(reference)
                .into_iter()
                .map(|(_, field, argument)| (field.clone(), argument))
                .collect();
            let mut implicit_fields = Vec::new();
            for (field, argument) in descriptors {
                if let Some(ordinal) = fields
                    .iter()
                    .position(|authored| authored.field_symbol == field.symbol)
                {
                    let unavailable_argument =
                        match program.type_reference_table.type_reference(argument) {
                            TypeReferenceNode::Named { symbol, .. } => substitutions
                                .iter()
                                .any(|(parameter, value)| parameter == symbol && !value.is_valid()),
                            _ => false,
                        };
                    // An explicit descriptor is evaluated at its authored
                    // position. Earlier effects cannot be justified from
                    // statement-entry scalar facts. A literal is stable;
                    // otherwise require an inert prefix through this field.
                    let value = fields[ordinal].value;
                    let stable_literal = matches!(
                        program.expression_table.expression(value),
                        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                    );
                    let stable_position = stable_literal
                        || !earlier_effects
                            && fields[..=ordinal]
                                .iter()
                                .all(|field| effects.inert(program, field.value));
                    if unavailable_argument || !stable_position {
                        diagnostics.push(Diagnostic::error(format!("cannot validate runtime index descriptor `{}` across argument or field effects without an evaluated scalar snapshot", field.name.as_str())));
                    }
                    continue;
                }
                match index_expression(program, argument, field.type_reference, substitutions, effects, earlier_effects) {
                            Some(value) => implicit_fields.push(TableStructLiteralField { name: field.name, field_symbol: field.symbol, value }),
                            None => diagnostics.push(Diagnostic::error(format!("cannot retain runtime index descriptor `{}` from its application argument without an evaluated scalar subject", field.name.as_str()))),
                        }
            }
            // The application captures its indices before evaluating
            // authored payloads. Keep authored field order unchanged.
            implicit_fields.extend(fields);
            fields = implicit_fields;
            let mut prior = earlier_effects;
            let children: Vec<_> = fields
                .iter()
                .filter_map(|field| {
                    let field_prior = prior;
                    prior |= !effects.inert(program, field.value);
                    if !contains_reachable_struct_literal(program, field.value) {
                        return None;
                    }
                    let selected = program.data_constructor_field(
                        reference, literal.type_symbol, field.field_symbol,
                    )?;
                    Some((field.value, selected.type_reference, field_prior))
                })
                .collect();
            literal.fields = program.expression_table.insert_struct_fields(fields);
            *program.expression_table.expression_mut(expression) =
                ExpressionNode::StructLiteral(literal);
            for (value, reference, prior) in children {
                fill(
                    program,
                    value,
                    reference,
                    substitutions,
                    effects,
                    prior,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn index_expression(
    program: &mut TypedTrees,
    argument: TypeReferenceHandle,
    carrier: TypeReferenceHandle,
    substitutions: &Substitutions,
    effects: &CallerEffects,
    earlier_effects: bool,
) -> Option<ExpressionHandle> {
    if let TypeReferenceNode::ConstExpression(expression) =
        *program.type_reference_table.type_reference(argument)
    {
        return capture_recipe(program, expression, substitutions, effects, earlier_effects);
    }
    let TypeReferenceNode::Named { symbol, name } = program
        .type_reference_table
        .type_reference(argument)
        .clone()
    else {
        return None;
    };
    if program.const_declaration_by_symbol(symbol).is_some() {
        let value = crate::monomorphization::materialize_static_value(program, argument, carrier)?;
        return Some(program.expression_table.insert(value));
    }
    if symbol.is_valid() {
        if let Some((_, value)) = substitutions
            .iter()
            .find(|(parameter, _)| *parameter == symbol)
        {
            if !value.is_valid() {
                return None;
            }
            // The supplying telescope already established whether this
            // argument can be reobserved. Preserve the same complete scalar
            // recipe used for an application argument, including member reads;
            // a bare-name whitelist loses descriptor metadata at ordinary calls.
            return capture_recipe(program, *value, &Vec::new(), effects, earlier_effects);
        }
        if earlier_effects {
            return None;
        }
        return Some(crate::monomorphization::insert_subject_name(
            &mut program.expression_table,
            name,
            symbol,
        ));
    }
    let value = crate::monomorphization::materialize_static_value(program, argument, carrier)?;
    Some(program.expression_table.insert(value))
}

/// Materialize the application recipe under its exact supplying telescope.
/// Only inert reads can be replayed at construction. A missing operand capture
/// remains missing; a same-spelled caller name cannot replace it.
fn capture_recipe(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    substitutions: &Substitutions,
    effects: &CallerEffects,
    earlier_effects: bool,
) -> Option<ExpressionHandle> {
    let stable_literal = matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
    );
    if !stable_literal && (earlier_effects || !effects.inert(program, expression)) {
        return None;
    }
    let mut tree = Vec::new();
    validation::expression_traversal::collect_expression_tree(program, expression, &mut tree);
    for member in &tree {
        if let ExpressionNode::Name(path) = program.expression_table.expression(*member)
            && let Some((_, actual)) = substitutions
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)
            && (!actual.is_valid() || !effects.inert(program, *actual))
        {
            return None;
        }
    }
    // A type application owns a recipe, not an executable occurrence. Keep
    // its selected meaning and graph independent from the descriptor read;
    // later execution/checking must not rewrite the signature's proof schema.
    let copied = program.expression_table.insert_copy(expression);
    tree.clear();
    validation::expression_traversal::collect_expression_tree(program, copied, &mut tree);
    for member in tree {
        let replacement = match program.expression_table.expression(member) {
            ExpressionNode::Name(path) => substitutions
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)
                .map(|(_, actual)| *actual),
            _ => None,
        };
        if let Some(actual) = replacement {
            let actual = program.expression_table.insert_copy(actual);
            let value = program.expression_table.expression(actual).clone();
            *program.expression_table.expression_mut(member) = value;
        }
    }
    Some(copied)
}
