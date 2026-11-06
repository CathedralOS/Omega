//! Retain exact construction destinations before result-type inference.
//!
//! A constructor's selected declaration and receiving application determine
//! otherwise unobservable generic arguments. Retain that source type without
//! granting its fields or qualifiers. The same destination walk lands quoted
//! bytes into owned fixed arrays: exact width, no truncation or zero padding.

use crate::tables::{expression_table, expression_table_mut};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::selected_construction_field_type;

struct ConstructionCall {
    target: SymbolHandle,
    source_owner: SymbolHandle,
    static_arguments: Vec<StaticMachineArgument>,
    arguments: Vec<ExpressionHandle>,
}

/// Prepare constructors and exact-width byte literals at destinations produced
/// by this run: those of machines appended after
/// `machine_frontier`, plus any retained machine named in `completed_machines`
/// whose body this run completed in place. Base literals have already been
/// contextualized, so expression discovery also starts past
/// `expression_frontier`; a whole program passes zeroes.
pub(crate) fn prepare(
    program: &mut TypedTrees,
    expression_frontier: usize,
    machine_frontier: usize,
    completed_machines: &[SymbolHandle],
) -> Result<(), Diagnostic> {
    let mut destinations = Vec::<(ExpressionHandle, TypeReferenceHandle)>::new();
    let mut calls = Vec::new();
    let mut field_destinations = Vec::new();

    // Aggregate fields and value-call arguments carry their destination types
    // in the typed expression graph itself.
    for (handle, expression) in program.expression_table.expression_entries() {
        if usize::try_from(handle.arena_index()).is_ok_and(|index| index <= expression_frontier) {
            continue;
        }
        match expression {
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    if let Some(field_type) = selected_construction_field_type(
                        program,
                        handle,
                        literal,
                        field.field_symbol,
                    ) {
                        field_destinations.push((field.value, field_type, literal.type_symbol));
                    }
                }
            }
            ExpressionNode::Call(call) => collect_call_destinations(
                program,
                call.target_symbol,
                call_source_owner(
                    program,
                    call.authored_machine_application,
                    SymbolHandle::invalid(),
                ),
                &call.machine_arguments,
                program.expression_table.expression_handles(call.arguments),
                &mut calls,
            ),
            _ => {}
        }
    }

    // Machine/state declarations own the remaining result positions: the
    // machines this run appended, plus every retained machine whose body it
    // completed in place.
    for (ordinal, machine) in program.machines().iter().enumerate() {
        if ordinal < machine_frontier && !completed_machines.contains(&machine.symbol) {
            continue;
        }
        for owned in program.machine_owned_data(machine) {
            if owned.initial_value.is_valid() && owned.type_reference.is_valid() {
                destinations.push((owned.initial_value, owned.type_reference));
            }
        }
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                match statement {
                    StatementNode::Assembly(_) => {}
                    StatementNode::LocalData(local)
                        if local.initial_value.is_valid() && local.type_reference.is_valid() =>
                    {
                        destinations.push((local.initial_value, local.type_reference));
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(destination) = validation::assignment_stored_value_type(
                            program, machine, state, assignment,
                        ) {
                            destinations.push((assignment.value, destination));
                        }
                    }
                    StatementNode::Call(call) => collect_call_destinations(
                        program,
                        call.target_symbol,
                        call_source_owner(
                            program,
                            call.authored_machine_application,
                            machine.symbol,
                        ),
                        &call.machine_arguments,
                        program.statement_table.expression_handles(call.arguments),
                        &mut calls,
                    ),
                    StatementNode::Expression(value)
                        if state.return_type.is_valid()
                            && statement_index + 1 == statements.len() =>
                    {
                        destinations.push((*value, state.return_type));
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(value)
                                    if state.return_type.is_valid() =>
                                {
                                    destinations.push((*value, state.return_type));
                                }
                                TransitionTargetNode::Named {
                                    path, arguments, ..
                                } => {
                                    collect_call_destinations(
                                        program,
                                        path.symbol,
                                        machine.symbol,
                                        &[],
                                        program.statement_table.expression_handles(*arguments),
                                        &mut calls,
                                    );
                                }
                                TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {
                                }
                            }
                        }
                    }
                    StatementNode::AssemblyFact(_)
                    | StatementNode::Expression(_)
                    | StatementNode::LocalData(_) => {}
                }
            }
        }
    }

    // A receiving schema can supply lifetime names even without type arguments.
    // Keep its declaration on the copied type; checking the constructor in its
    // caller must not reinterpret those names as caller-authored binders.
    for (value, reference, declaration) in field_destinations {
        let reference = program
            .apply_callable_type(reference, &[], &[], declaration)
            .map_err(Diagnostic::error)?;
        destinations.push((value, reference));
    }
    for call in calls {
        let binders = program.declaration_type_parameters(call.target).to_vec();
        // An unapplied generic signature belongs to the callee, not this
        // constructor's lexical scope. Inferred calls wait for specialization;
        // explicit operands use the same simultaneous callable substitution.
        if binders.len() != call.static_arguments.len() {
            continue;
        }
        if !call.static_arguments.is_empty() && !call.source_owner.is_valid() {
            // Detached arena expressions have no caller-owned application to
            // infer from. Decline the hint; ordinary source checks still own
            // admission of every live call and its explicit static arguments.
            continue;
        }
        let mut substitutions = Vec::with_capacity(binders.len());
        for (binder, argument) in binders.iter().zip(&call.static_arguments) {
            let actual = program
                .static_operand_type_reference_in_scope(argument, call.source_owner)
                .map_err(Diagnostic::error)?;
            substitutions.push((binder.symbol, actual));
        }
        let Some(parameters) = validation::call_target_parameters(program, call.target) else {
            continue;
        };
        let required: Vec<_> = parameters
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect();
        for (argument, reference) in call.arguments.into_iter().zip(required) {
            if !needs_destination(program, argument) {
                continue;
            }
            let reference = program
                .apply_callable_type(reference, &substitutions, &[], call.target)
                .map_err(Diagnostic::error)?;
            destinations.push((argument, reference));
        }
    }
    for (value, destination) in destinations {
        land_one(program, value, destination)?;
    }
    Ok(())
}

fn collect_call_destinations(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    source_owner: SymbolHandle,
    static_arguments: &[StaticMachineArgument],
    arguments: &[ExpressionHandle],
    calls: &mut Vec<ConstructionCall>,
) {
    if !target_symbol.is_valid()
        || !arguments
            .iter()
            .any(|argument| needs_destination(program, *argument))
    {
        return;
    }
    calls.push(ConstructionCall {
        target: target_symbol,
        source_owner,
        static_arguments: static_arguments.to_vec(),
        arguments: arguments.to_vec(),
    });
}

fn call_source_owner(
    program: &TypedTrees,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    lexical_owner: SymbolHandle,
) -> SymbolHandle {
    if application.is_valid() {
        let owner = program
            .expression_table
            .static_machine_application(application)
            .lexical_owner;
        if owner.is_valid() {
            return owner;
        }
    }
    lexical_owner
}

fn needs_destination(program: &TypedTrees, value: ExpressionHandle) -> bool {
    value.is_valid()
        && matches!(
            program.expression_table.expression(value),
            ExpressionNode::StructLiteral(_)
                | ExpressionNode::String(_)
                | ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Match(_)
        )
}

fn land_one(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    if !value.is_valid() || !destination.is_valid() {
        return Ok(());
    }
    if let Some(dispatch) = program.expression_table.match_node(value) {
        let arms = program.expression_table.match_arms(dispatch.arms).to_vec();
        for arm in arms {
            land_one(program, arm.value, destination)?;
        }
        return Ok(());
    }
    if let ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(value).clone()
    {
        // Destination is a receiving obligation, never the constructor's
        // produced type. The shared join checks its exact authored owner and
        // selected case before exposing an applied field telescope.
        // An open application can still determine the constructor's argument
        // tuple (including phantom parameters). Retain it before inference;
        // only prepared field views may supply child destinations below.
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(destination);
        let exact_template = matches!(program.type_reference_table.type_reference(carrier),
            TypeReferenceNode::Generic { base_symbol, .. } if *base_symbol == literal.type_symbol);
        if !exact_template
            && program
                .closed_data_constructor_definition(destination, literal.type_symbol)
                .is_none()
        {
            return Ok(());
        }
        let prior = expression_table(program).construction_required_type_reference(value);
        if !prior.is_valid() {
            expression_table_mut(program).set_construction_required_type_reference(value, carrier);
        }
        // An inferred constructor application is source type information and
        // must travel with the literal when its generic body is specialized.
        // Receiving obligations are separate, and are recomputed for each use.
        if !literal.application_type.is_valid()
            && let ExpressionNode::StructLiteral(retained) =
                expression_table_mut(program).expression_mut(value)
        {
            retained.application_type = carrier;
        }
        let fields = expression_table(program)
            .struct_fields(literal.fields)
            .to_vec();
        for field in fields {
            if let Some(field_type) =
                selected_construction_field_type(program, value, &literal, field.field_symbol)
            {
                let field_type = program
                    .apply_callable_type(field_type, &[], &[], literal.type_symbol)
                    .map_err(Diagnostic::error)?;
                land_one(program, field.value, field_type)?;
            }
        }
        return Ok(());
    }
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(destination);
    if let (
        ExpressionNode::ArrayLiteral(elements),
        TypeReferenceNode::FixedArray { element_type, .. },
    ) = (
        program.expression_table.expression(value),
        program.type_reference_table.type_reference(carrier),
    ) {
        let elements = expression_table(program)
            .expression_handles(*elements)
            .to_vec();
        let element_type = *element_type;
        for element in elements {
            land_one(program, element, element_type)?;
        }
        return Ok(());
    }
    validation::land_owned_fixed_byte_literal(program, value, destination)
}
