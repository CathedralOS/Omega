//! Retain executable Value indices in the descriptor fields created by syntax
//! lowering. A destination supplies the application; construction reads each
//! argument once at that point. This derives ordinary field expressions only:
//! immutable checking still proves their equality to the captured index.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteralField};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

type Substitutions = Vec<(SymbolHandle, ExpressionHandle)>;

/// Immutable preparation-time caller judgments. Generated descriptor reads add
/// no writes, so mutation of constructor field lists cannot enlarge this set.
struct CallerEffects {
    machine: SymbolHandle,
    no_write_calls: Vec<ExpressionHandle>,
}

impl CallerEffects {
    fn inert(&self, program: &TypedTrees, expression: ExpressionHandle) -> bool {
        validation::expression_traversal::inert_value_tree_with_calls(program, expression, |call| {
            self.no_write_calls
                .binary_search_by_key(&(call.arena_index(), call.generation()), |candidate| {
                    (candidate.arena_index(), candidate.generation())
                })
                .is_ok()
        })
    }
}

pub(super) fn materialize(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut destinations = Vec::new();
    let mut calls = Vec::new();
    let mut caller_effects = Vec::new();
    let frames = validation::CallFrameResolver::new(program);
    for machine in program.machines() {
        let mut no_write_calls = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::LocalData(local) => destinations.push((
                        machine.symbol,
                        local.initial_value,
                        local.type_reference,
                        Vec::new(),
                    )),
                    StatementNode::Expression(value) if state.return_type.is_valid() => {
                        destinations.push((machine.symbol, *value, state.return_type, Vec::new()));
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(reference) = validation::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            assignment.target,
                        ) {
                            destinations.push((
                                machine.symbol,
                                assignment.value,
                                reference,
                                Vec::new(),
                            ));
                        }
                    }
                    StatementNode::Call(call) => calls.push((
                        machine.symbol,
                        call.target_symbol,
                        program
                            .statement_table
                            .expression_handles(call.arguments)
                            .to_vec(),
                    )),
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(value) => destinations.push((
                                    machine.symbol,
                                    *value,
                                    state.return_type,
                                    Vec::new(),
                                )),
                                TransitionTargetNode::Named {
                                    path, arguments, ..
                                } => calls.push((
                                    machine.symbol,
                                    path.symbol,
                                    program
                                        .statement_table
                                        .expression_handles(*arguments)
                                        .to_vec(),
                                )),
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
                        if frames.as_ref().is_some_and(|frames| {
                            frames
                                .expression_write_frame(machine, expression)
                                .complete_paths()
                                .is_some_and(|paths| paths.is_empty())
                        }) {
                            no_write_calls.push(expression);
                        }
                        calls.push((
                            machine.symbol,
                            call.target_symbol,
                            program
                                .expression_table
                                .expression_handles(call.arguments)
                                .to_vec(),
                        ));
                    }
                }
            }
        }
        no_write_calls.sort_by_key(|call| (call.arena_index(), call.generation()));
        no_write_calls.dedup();
        caller_effects.push(CallerEffects {
            machine: machine.symbol,
            no_write_calls,
        });
    }
    drop(frames);
    for (caller, target, arguments) in calls {
        let effects = caller_effects
            .iter()
            .find(|effects| effects.machine == caller)
            .expect("collected caller effects");
        let Some(parameters) = validation::call_target_parameters(program, target) else {
            continue;
        };
        let can_reobserve = arguments
            .iter()
            .all(|argument| effects.inert(program, *argument));
        let substitutions: Substitutions = parameters
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
            .collect();
        destinations.extend(
            parameters
                .iter()
                .zip(arguments)
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
        let effects = caller_effects
            .iter()
            .find(|effects| effects.machine == caller)
            .expect("collected caller effects");
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
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
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
            let mut fields = program
                .expression_table
                .struct_fields(literal.fields)
                .to_vec();
            let application_reference = match program.type_reference_table.type_reference(reference)
            {
                TypeReferenceNode::Named { symbol, .. } => program
                    .unique_data_definition_by_symbol(*symbol)
                    .and_then(|definition| definition.generic_instance)
                    .unwrap_or(reference),
                _ => reference,
            };
            if let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program
                .type_reference_table
                .type_reference(application_reference)
            {
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if let Some(definition) = program.unique_data_definition_by_symbol(*base_symbol) {
                    let parameters = program.data_type_parameters(definition);
                    let descriptors: Vec<_> = program
                        .data_value_parameter_fields(definition)
                        .enumerate()
                        .filter_map(|(descriptor_ordinal, (parameter, field))| {
                            let ordinal = parameters
                                .iter()
                                .position(|candidate| candidate.symbol == parameter.symbol)?;
                            let field = if application_reference != reference {
                                // A syntax-specialized instance has its own field
                                // symbols but preserves the leading descriptor order.
                                let concrete = program
                                    .unique_data_definition_by_symbol(literal.type_symbol)?;
                                program
                                    .data_members(concrete)
                                    .get(descriptor_ordinal)?
                                    .as_field()?
                            } else {
                                field
                            };
                            Some((field.clone(), *arguments.get(ordinal)?))
                        })
                        .collect();
                    let mut implicit_fields = Vec::new();
                    for (field, argument) in descriptors {
                        if let Some(ordinal) = fields
                            .iter()
                            .position(|authored| authored.field_symbol == field.symbol)
                        {
                            let unavailable_argument =
                                match program.type_reference_table.type_reference(argument) {
                                    TypeReferenceNode::Named { symbol, .. } => {
                                        substitutions.iter().any(|(parameter, value)| {
                                            parameter == symbol && !value.is_valid()
                                        })
                                    }
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
                        match index_expression(program, argument, field.type_reference, substitutions, earlier_effects) {
                            Some(value) => implicit_fields.push(TableStructLiteralField { name: field.name, field_symbol: field.symbol, value }),
                            None => diagnostics.push(Diagnostic::error(format!("cannot retain runtime index descriptor `{}` from its application argument without an evaluated scalar subject", field.name.as_str()))),
                        }
                    }
                    // The application captures its indices before evaluating
                    // authored payloads. Keep authored field order unchanged.
                    implicit_fields.extend(fields);
                    fields = implicit_fields;
                }
            }
            let definition = program.data_application_definition(reference).or_else(|| {
                program
                    .type_reference_table
                    .type_head_symbol(reference)
                    .and_then(|symbol| program.data_definition_by_symbol(symbol))
            });
            let mut prior = earlier_effects;
            let children: Vec<_> = definition
                .map(|definition| {
                    fields
                        .iter()
                        .filter_map(|field| {
                            let field_prior = prior;
                            prior |= !effects.inert(program, field.value);
                            let reference =
                                program.data_members(definition).iter().find_map(|member| {
                                    match member {
                                        DataMember::Field(declared)
                                            if declared.symbol == field.field_symbol =>
                                        {
                                            Some(declared.type_reference)
                                        }
                                        DataMember::Variant(case)
                                            if Some(case.symbol) == literal.case_symbol =>
                                        {
                                            program
                                                .data_payload_fields(case)
                                                .iter()
                                                .find(|declared| {
                                                    declared.symbol == field.field_symbol
                                                })
                                                .map(|declared| declared.type_reference)
                                        }
                                        _ => None,
                                    }
                                })?;
                            Some((field.value, reference, field_prior))
                        })
                        .collect()
                })
                .unwrap_or_default();
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
    earlier_effects: bool,
) -> Option<ExpressionHandle> {
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
            // A Name can be observed again only before any possible effect.
            // Literal operands are stable without another storage observation.
            return matches!(
                program.expression_table.expression(*value),
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
            )
            .then_some(*value)
            .or_else(|| {
                (!earlier_effects
                    && matches!(
                        program.expression_table.expression(*value),
                        ExpressionNode::Name(_)
                    ))
                .then_some(*value)
            });
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
