//! Bind numeric casts involving floats to their selected conversion requirements.
//!
//! All casts retain their authored target on the derived call, so ordinary
//! occurrence checking discharges unchanged denotation using live value facts.
//! Selection alone proves nothing. Integer-to-float coercions use the same
//! live occurrence facts as other conversions. Named conversions may round, but `as`
//! cannot silently do so. Same-format qualification remains a cast; it is
//! not an invitation to call a similarly named operator. All three conversion
//! directions share selection and source custody with explicit named calls.
//!
//! Terminal Psi has no float-to-integer primitive: every float
//! realization is a requirement-level `BoundaryCall` that Omega installs from
//! the selected provider. An unbound carrier-changing float cast therefore
//! reaches no executable conversion route.
//!
//! This pass puts the ordinary conversion call beneath the authored cast.
//! The call owns carrier realization and the cast keeps qualification,
//! including target predicates, under the ordinary membership checker. A
//! constrained destination is not a reason to omit an executable operation,
//! and its annotation is never a premise about the conversion's result.
//! The cast names the requirement path by the conversion-naming law
//! (destination `I32`, source `f64`) and selects its overload by the cast's
//! unqualified carrier result. An `in Saturating`, `in Trapping` or `in Wrapping`
//! suffix qualifies future operations; it cannot select a conversion that
//! clamps, traps or truncates an unrepresentable source. The unqualified named
//! operation is executable only after proving unchanged denotation. Explicit
//! named calls still select their result-domain overload normally. See
//! [exact coercion](../../../../../../../wiki/spec/language/domains.md#exact-coercion-and-erasure).
//! When the conversion family is not visible (no `core::float_operations` import) the cast is
//! left as it is: it still checks and interprets, and native production keeps
//! rejecting it for want of a route.
//!
//! Float-to-integer casts select the unqualified conversion through the
//! same route. Selection grants no conversion proof: ordinary call validation
//! still checks the selected requirement's finite/truncated-range premises,
//! and Terminal publication retains those obligations. Leaving the carrier
//! conversion unbound would give valid checked expressions no executable operation.
//!
//! The pass runs after `resolve_named_result_overloads`. That pass re-selects
//! a named operator call from the result type its context expects. A derived
//! carrier call must not inherit the authored cast's future arithmetic policy.
//!
//! The call keeps the cast's source span and receives a resolved `Call`
//! selection occurrence at that span. Review custody and provider
//! realization then see the same exact selection an explicitly spelled
//! `I32::from_f64(value)` in that position would carry.

use diagnostics::Diagnostic;
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

/// One cast to rewrite, selected before the expression table is mutated.
struct ConversionCastBinding {
    exact_coercion_target: typed_trees::types::TypeReferenceHandle,
    cast: ExpressionHandle,
    operand: ExpressionHandle,
    requirement: SymbolHandle,
    requirement_member: Identifier,
}

/// Bind the visible destination-owned conversion beneath each supported
/// numeric cast, retaining its target predicates. Returns the number bound.
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
                    layout_query: None,
                    arguments,
                    evidence_arguments: Box::default(),
                    // The authored cast selects this conversion; its span carries
                    // the Call selection recorded above, exactly one per call.
                    operational_acknowledgement:
                        language_semantics::CallOperationalAcknowledgement {
                            origin:
                                language_semantics::CallOperationalAcknowledgementOrigin::Source,
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

fn collect_conversion_cast_bindings(
    program: &dyn crate::CheckingInput,
) -> Vec<ConversionCastBinding> {
    let mut bindings = Vec::new();
    let mut visited = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    visited: &mut Vec<bool>,
    bindings: &mut Vec<ConversionCastBinding>,
) {
    if !expression.is_valid() {
        return;
    }
    // A shared subtree (the duplicated `abs` operand, say) is one authored
    // occurrence: visit it once. Indexed by arena position, not searched.
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
    crate::expression_traversal::children(program, node, |child| {
        collect_expression_bindings(program, machine, state, child, visited, bindings)
    });
}

/// The one visible conversion overload a supported numeric cast
/// selects, with the requirement's final path member for the call spelling.
fn selected_conversion(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    cast: &typed_trees::expression::TableCastExpression,
) -> Option<(SymbolHandle, Identifier)> {
    // Qualification is the outer cast's obligation, not an alternative
    // carrier operation. Select the same unqualified conversion beneath it;
    // the authored cast retains its normalized domain and exact use identity.
    if cast.form.is_recast() || !cast.result_type.is_valid() {
        return None;
    }
    let destination = program.primitive_type_reference(cast.target_type)?;
    let destination_owner = destination_conversion_owner(destination)?;
    let source = crate::value_custody::expression_types::expression_result_type_reference(
        program, machine, state, cast.value,
    )
    .and_then(|reference| crate::unwrapped_type_reference(program, reference))
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
            && crate::unwrapped_type_reference(program, operator.return_type)
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
    // Declaration validation owns duplicate overload diagnostics; an
    // ambiguous family binds nothing here.
    if selected.next().is_some() {
        return None;
    }
    // The destination/source names select a family, not its semantics. A
    // format coercion can use only the selected requirement whose sealed
    // guarantee actually states the IEEE format conversion just proved exact.
    if destination_is_float
        && crate::numeric_float_conversion_contract(program, operator)
            != Some((source, destination))
    {
        return None;
    }
    if source_is_float
        && !destination_is_float
        && crate::float_integer_conversion_contract(program, operator).is_none()
    {
        return None;
    }
    Some((operator.symbol, Identifier::generated(requirement_member)))
}

/// Destination namespaces for fixed integers and IEEE floats. Address,
/// Boolean and other carriers cannot borrow a numeric conversion by layout.
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
