//! Bind numeric casts involving floats to their selected conversion requirements.
//!
//! Exact widening casts use a destination conversion only when every source
//! value is exactly representable. Named conversions may round, but `as`
//! cannot silently do so. Same-format qualification remains a cast; it is
//! not an invitation to call a similarly named operator. All three conversion
//! directions share selection and source custody with explicit named calls.
//!
//! `value as i32 in Saturating` with a float `value` has exactly the meaning
//! of the destination-owned conversion `I32::from_f64` overload whose result
//! is `i32 in Saturating`: toward-zero truncation that clamps to the carrier
//! and maps NaN to zero ([destination-owned conversion](../../../../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion)).
//! Checking and the checked interpreter already give the cast that meaning.
//! Terminal Psi has no float-to-integer primitive, though: every float
//! realization is a requirement-level `BoundaryCall` that Omega installs from
//! the selected provider. A cast that stays a cast therefore reaches no
//! executable route.
//!
//! This pass turns the cast into the ordinary call the author could have
//! written, like `bind_selected_operator_calls` does for a spelled `a * b`.
//! The cast names the requirement path by the conversion-naming law
//! (destination `I32`, source `f64`) and selects its overload by the cast's
//! complete result qualification under the
//! [result-domain overload rule](../../../../../../../wiki/spec/language/domains.md#result-domain-overloads):
//! the requested dispatch set is the cast's `in Saturating`/`in Trapping`
//! policy and must equal exactly one visible overload's result projection.
//! No overload with another policy substitutes for it. When the conversion
//! family is not visible (no `core::float_operations` import) the cast is
//! left as it is: it still checks and interprets, and native production keeps
//! rejecting it for want of a route.
//!
//! Exact float-to-integer casts stay casts. Their obligation is the cast's own
//! finite-and-in-range proof (declared ranges and dominating guards), which
//! the unqualified conversion's `requires` entailment does not yet discharge;
//! binding them would reject valid programs.
//!
//! The pass runs after `resolve_named_result_overloads`. That pass re-selects
//! a named operator call from the result type its context expects, and a
//! cast's context (`(value as i32 in Saturating) as i32`) expects none, so a
//! call bound earlier would be rebound to the unqualified overload.
//!
//! The call keeps the cast's source span and receives a resolved `Call`
//! selection occurrence at that span. Review custody and provider
//! realization then see the same exact selection an explicitly spelled
//! `I32::from_f64(value)` in that position would carry.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

#[cfg(test)]
mod tests;

/// One cast to rewrite, selected before the expression table is mutated.
struct ConversionCastBinding {
    cast: ExpressionHandle,
    operand: ExpressionHandle,
    requirement: SymbolHandle,
    requirement_member: Identifier,
}

/// Rewrite exact float widenings and float-to-integer policy casts whose
/// destination-owned conversion overload is visible into a call of that
/// overload. Returns the number of casts bound.
pub fn bind_float_conversion_casts(program: &mut TypedTrees) -> Result<usize, Vec<Diagnostic>> {
    let bindings = collect_conversion_cast_bindings(program);
    let mut diagnostics = Vec::new();
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
        *program.expression_table.expression_mut(binding.cast) =
            ExpressionNode::Call(TableCallExpression {
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
                operational_acknowledgement: language_semantics::CallOperationalAcknowledgement {
                    origin: language_semantics::CallOperationalAcknowledgementOrigin::Source,
                    ..Default::default()
                },
            });
        program
            .expression_table
            .attach_authored_selection_occurrences(binding.cast, [occurrence]);
    }
    if diagnostics.is_empty() {
        Ok(bindings.len())
    } else {
        Err(diagnostics)
    }
}

fn collect_conversion_cast_bindings(program: &TypedTrees) -> Vec<ConversionCastBinding> {
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
    program: &TypedTrees,
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    cast: &typed_trees::expression::TableCastExpression,
) -> Option<(SymbolHandle, Identifier)> {
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() || !cast.result_type.is_valid() {
        return None;
    }
    let destination = program.primitive_type_reference(cast.target_type)?;
    // Replacing a constrained cast with a bare call would delete its target
    // predicate obligations: result-domain dispatch does not include them.
    if !matches!(
        program
            .type_reference_table
            .type_reference(cast.target_type),
        typed_trees::types::TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    let destination_owner = destination_conversion_owner(destination)?;
    let source = crate::value_custody::expression_types::expression_result_type_reference(
        program, machine, state, cast.value,
    )
    .and_then(|reference| crate::unwrapped_type_reference(program, reference))
    .and_then(|reference| program.primitive_type_reference(reference))?;
    let source_is_float = matches!(source, PrimitiveType::F32 | PrimitiveType::F64);
    let destination_is_float = matches!(destination, PrimitiveType::F32 | PrimitiveType::F64);
    let supported = if destination_is_float {
        cast.domain == ArithmeticDomain::Exact && exact_float_widening(source, destination)
    } else {
        source_is_float
            && matches!(
                cast.domain,
                ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
            )
    };
    if !supported {
        return None;
    }
    let source_span = program.expression_table.source_span(expression);
    if source_span.span.start >= source_span.span.end {
        return None;
    }
    let requested = program.normalized_result_dispatch_set(cast.result_type);
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
            && !parameter.is_self
            && !parameter.is_mutable
            && !parameter.is_const
            && program.primitive_type_reference(parameter.type_reference) == Some(source)
            && crate::unwrapped_type_reference(program, operator.return_type)
                .and_then(|reference| program.primitive_type_reference(reference))
                == Some(destination)
            && program
                .normalized_operator_overload_identity(operator)
                .result_dispatch()
                == &requested
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
    Some((operator.symbol, Identifier::generated(requirement_member)))
}

/// Whole-carrier representability is enough for these coercions. Other
/// conversions need a value-specific proof or an explicitly named rounding
/// operation; a total library conversion alone does not authorize an `as`.
fn exact_float_widening(source: PrimitiveType, destination: PrimitiveType) -> bool {
    matches!(
        (source, destination),
        (
            PrimitiveType::I8 | PrimitiveType::I16 | PrimitiveType::U8 | PrimitiveType::U16,
            PrimitiveType::F32 | PrimitiveType::F64
        ) | (
            PrimitiveType::I32 | PrimitiveType::U32 | PrimitiveType::F32,
            PrimitiveType::F64
        )
    )
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
