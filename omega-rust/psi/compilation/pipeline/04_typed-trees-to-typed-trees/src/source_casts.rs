//! Bind authored numeric casts to their exact declared conversion requirement.

use diagnostics::Diagnostic;
use language_semantics::CallOperationalAcknowledgement;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;
use validation::CheckingInput;

pub(crate) struct ConversionCastBinding {
    exact_coercion_target: typed_trees::types::TypeReferenceHandle,
    cast: ExpressionHandle,
    operand: ExpressionHandle,
    requirement: SymbolHandle,
    requirement_member: Identifier,
}

pub fn bind_float_conversion_casts(program: &mut TypedTrees) -> Result<usize, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let bindings = collect_conversion_cast_bindings(program);
    for binding in &bindings {
        let source_span = program.expression_table.source_span(binding.cast);
        let occurrence = match program.record_resolved_authored_declaration_selection_once(
            source_span,
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            binding.requirement,
        ) {
            Ok(occurrence) => occurrence,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "numeric cast cannot record its selected float conversion: {error:?}"
                    ))
                    .with_source_span(source_span),
                );
                continue;
            }
        };
        let arguments = program
            .expression_table
            .insert_expression_handles([binding.operand]);
        let conversion =
            program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    runtime_call: None,
                    authored_call_selection: None,
                    authored_call_binding: None,
                    authored_machine_application: Default::default(),
                    requirement_application: Default::default(),
                    exact_coercion_target: binding.exact_coercion_target,
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: binding.requirement,
                    static_machine_parameter: SymbolHandle::invalid(),
                    target: binding.requirement_member.clone(),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::default(),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    build_selection: Default::default(),
                    arguments,
                    evidence_arguments: Box::default(),
                    operational_acknowledgement: CallOperationalAcknowledgement {
                        origin: language_semantics::CallOperationalAcknowledgementOrigin::Source,
                        ..Default::default()
                    },
                }));
        program
            .expression_table
            .set_source_span(conversion, source_span);
        program
            .expression_table
            .attach_authored_selection_occurrences(conversion, [occurrence]);
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(binding.cast)
        else {
            diagnostics.push(
                Diagnostic::error("numeric conversion binding lost its authored cast")
                    .with_source_span(source_span),
            );
            continue;
        };
        cast.value = conversion;
    }
    if diagnostics.is_empty() {
        Ok(bindings.len())
    } else {
        Err(diagnostics)
    }
}

fn collect_conversion_cast_bindings(program: &dyn CheckingInput) -> Vec<ConversionCastBinding> {
    let mut bindings = Vec::new();
    let mut visited = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                for (_, root) in program.tables.expression_roots(statement) {
                    collect_expression_bindings(
                        program,
                        machine,
                        state,
                        root,
                        &mut visited,
                        &mut bindings,
                    );
                }
            }
        }
    }
    bindings
}

fn collect_expression_bindings(
    program: &dyn CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    visited: &mut Vec<bool>,
    bindings: &mut Vec<ConversionCastBinding>,
) {
    if !expression.is_valid() {
        return;
    }
    let index = expression.arena_index() as usize;
    if visited.len() <= index {
        visited.resize(index + 1, false);
    }
    if std::mem::replace(&mut visited[index], true) {
        return;
    }
    let node = program.expression_table.expression(expression);
    if let ExpressionNode::Cast(cast) = node
        && let Some((requirement, requirement_member)) =
            selected_conversion(program, machine, state, expression, cast)
    {
        bindings.push(ConversionCastBinding {
            exact_coercion_target: cast.target_type,
            cast: expression,
            operand: cast.value,
            requirement,
            requirement_member,
        });
    }
    program.typed_program().expression_children(node, |child| {
        collect_expression_bindings(program, machine, state, child, visited, bindings)
    });
}

fn selected_conversion(
    program: &dyn CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    cast: &typed_trees::expression::TableCastExpression,
) -> Option<(SymbolHandle, Identifier)> {
    if cast.form.is_recast() || !cast.result_type.is_valid() {
        return None;
    }
    let destination = program.primitive_type_reference(cast.target_type)?;
    let destination_owner = destination_conversion_owner(destination)?;
    let source = crate::source_type_queries::expression_result_type_reference(
        program, machine, state, cast.value,
    )
    .and_then(|reference| validation::unwrapped_type_reference(program, reference))
    .and_then(|reference| program.primitive_type_reference(reference))?;
    let source_is_float = matches!(source, PrimitiveType::F32 | PrimitiveType::F64);
    let destination_is_float = matches!(destination, PrimitiveType::F32 | PrimitiveType::F64);
    let supported = if destination_is_float {
        (source_is_float && source != destination)
            || numerics::float_semantics_catalog::IntegerCarrier::from_spelling(source.name())
                .is_some()
    } else {
        source_is_float
    };
    if !supported {
        return None;
    }
    let source_span = program.expression_table.source_span(expression);
    if source_span.span.start >= source_span.span.end {
        return None;
    }
    let requested = program.callable_result_dispatch_identity(&Default::default());
    let requirement_member = format!("from_{}", source.name());
    let mut selected = program.operators().iter().filter(|operator| {
        let path = program.operator_path_members(operator.name);
        let [owner, member] = path else {
            return false;
        };
        let [parameter] = program.operator_parameters(operator) else {
            return false;
        };
        operator.is_boundary
            && operator.spelling.is_none()
            && program.operator_type_parameters(operator).is_empty()
            && owner.as_str() == destination_owner
            && member.as_str() == requirement_member
            && !parameter.is_mutable
            && !parameter.is_const
            && program.primitive_type_reference(parameter.type_reference) == Some(source)
            && validation::unwrapped_type_reference(program, operator.return_type)
                .and_then(|reference| program.primitive_type_reference(reference))
                == Some(destination)
            && program
                .callable_identity_result_dispatch(program.operator_callable_identity(operator))
                == requested
            && program
                .symbols
                .source_reference_may_select_symbol(source_span, operator.symbol)
    });
    let operator = selected.next()?;
    if selected.next().is_some() {
        return None;
    }
    if destination_is_float
        && validation::numeric_float_conversion_contract(program, operator)
            != Some((source, destination))
    {
        return None;
    }
    if source_is_float
        && !destination_is_float
        && validation::float_integer_conversion_contract(program, operator).is_none()
    {
        return None;
    }
    Some((operator.symbol, Identifier::generated(requirement_member)))
}

fn destination_conversion_owner(destination: PrimitiveType) -> Option<&'static str> {
    Some(match destination {
        PrimitiveType::I8 => "I8",
        PrimitiveType::I16 => "I16",
        PrimitiveType::I32 => "I32",
        PrimitiveType::I64 => "I64",
        PrimitiveType::U8 => "U8",
        PrimitiveType::U16 => "U16",
        PrimitiveType::U32 => "U32",
        PrimitiveType::U64 => "U64",
        PrimitiveType::F32 => "F32",
        PrimitiveType::F64 => "F64",
        _ => return None,
    })
}
