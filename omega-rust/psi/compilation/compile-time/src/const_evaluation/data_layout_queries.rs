//! Source formation, deferral and closure of `size_of`/`align_of` queries.
//!
//! A retained layout query names its subject from the target closure and is
//! answered only once settlement selects the native target and the opaque
//! representation selections; the frontend evaluation domain carries no
//! target facts. Any program containing one therefore keeps its value-
//! dependent const work pending until `evaluate_selected_operators` resumes
//! under exact selected execution — the same deferral family range endpoints
//! and const applications already use. Psi closes retained queries using
//! supplied layout measurements
//! (constant and runtime positions alike) under that target custody before
//! the target closure consumes the typed program. Only the continuation's
//! owned declaration roots participate; Product descriptions retained during
//! Build admission never become host-target evaluation work.

use diagnostics::Diagnostic;
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::{DataLayoutQueryRequest, ExpressionNode};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Whether any typed call still carries a retained data-layout query. The
/// query itself is already the pending marker: lowering admits the request
/// only as an explicit `DataLayoutQueryRequest`, so the field's presence is
/// the complete deferral condition — no admission or selection closure is
/// needed to know the answer is target-dependent. Ownership traversal excludes
/// unselected declarations and stale arena nodes.
pub(crate) fn pending_layout_queries_need_operator_selection(
    typed: &TypedTrees,
    selection: &typed_trees::references::DeclarationSelection,
) -> bool {
    typed_trees::references::collect_selected_program_references(typed, selection)
        .expressions
        .into_iter()
        .any(
            |expression| match typed.expression_table.expression(expression) {
                ExpressionNode::Call(call) => call.layout_query.is_some(),
                _ => false,
            },
        )
}

/// Supplies a fresh measurement view for one query pass. Evaluation can install
/// new authored plans between passes; contexts must not leak across that change.
/// The provider measures storage only: Psi owns formation and source mutation.
pub trait LayoutQueryProvider {
    type Measurements: LayoutQueryMeasurements;

    fn prepare(&self, program: &TypedTrees) -> Self::Measurements;
}

/// Read-only geometry for the exact supplied source operand. None means an
/// unresolved layout, not a zero-sized carrier. Errors retain the layout reason.
pub trait LayoutQueryMeasurements {
    fn measure(
        &self,
        program: &TypedTrees,
        query: &DataLayoutQueryRequest,
    ) -> Result<Option<usize>, Diagnostic>;
}

/// Fold selected declarations' retained `size_of`/`align_of` queries to `u64`
/// literals under supplied measurements. Initializers then complete through
/// ordinary semantic evaluation, including derived expressions.
///
/// A plan-laid operand's layout exists only once the deferred evaluation
/// installs the validated plan (`compute_plan_laid_layouts_in`). With
/// `defer_uninstalled_plans`, a query whose operand names a `Policy<Schema>`
/// application without an installed plan is left in place for the post-
/// evaluation pass to fold — the plan installer's own ordering, not a guess
/// at which operands will become measurable. With `false` every query
/// resolves or reports the layout authority's diagnostic. `selection` owns
/// evaluation work, so canonical Product descriptions remain untouched while
/// Build is evaluated on the same arenas.
pub fn fold_data_layout_queries(
    program: &mut TypedTrees,
    defer_uninstalled_plans: bool,
    selection: &typed_trees::references::DeclarationSelection,
    provider: &impl LayoutQueryProvider,
) -> Result<(), Vec<Diagnostic>> {
    // The proof-only classification is computed over the typed program;
    // a `size_of<Nat>` over recursive data must name the
    // classification instead of reporting the layout builder's zero default.
    let proof_only = checked_trees::typed_trees::proof_only::classify(program);
    let owned_expressions =
        checked_trees::typed_trees::references::collect_selected_program_references(
            program, selection,
        )
        .expressions;
    let queries: Vec<(
        checked_trees::typed_trees::expression::ExpressionHandle,
        checked_trees::typed_trees::expression::DataLayoutQueryRequest,
        source::SourceSpan,
    )> = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| owned_expressions.contains(handle))
        .filter_map(|(handle, node)| match node {
            ExpressionNode::Call(call) => call.layout_query.as_ref().map(|query| {
                (
                    handle,
                    query.clone(),
                    program.expression_table.source_span(handle),
                )
            }),
            _ => None,
        })
        .collect();
    let measurements = provider.prepare(program);
    for (handle, query, reference) in queries {
        let operand = query.operand.type_reference;
        if defer_uninstalled_plans && plan_laid_operand_pending_installation(program, operand) {
            continue;
        }
        let spelling = query.kind.target_name();
        if let Some(held) = proof_only.proof_only_mention(program, operand) {
            return Err(vec![
                Diagnostic::error(format!(
                    "`{spelling}` cannot measure a proof-only carrier: `{held}` has no runtime layout"
                ))
                .with_source_span(reference),
            ]);
        }
        if !defer_uninstalled_plans {
            reject_unresolved_layout_operands(program, operand)
                .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(reference)])?;
        }
        let extent = measurements
            .measure(program, &query)
            .map_err(|error| vec![error])?;
        let Some(extent) = extent else {
            if defer_uninstalled_plans {
                continue;
            }
            return Err(vec![
                Diagnostic::error(format!(
                    "`{spelling}` cannot measure a carrier with an unresolved array extent"
                ))
                .with_source_span(reference),
            ]);
        };
        let value = u64::try_from(extent).map_err(|_| {
            vec![
                Diagnostic::error(format!(
                    "`{spelling}` layout extent exceeds the `u64` result carrier"
                ))
                .with_source_span(reference),
            ]
        })?;
        let literal = IntegerLiteral::from_parts(false, IntegerRadix::Decimal, &value.to_string())
            .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(reference)])?
            .with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
        *program.expression_table.expression_mut(handle) = ExpressionNode::Integer(literal);
    }
    Ok(())
}

/// Whether the operand is a `Policy<Schema>` application whose validated
/// plan instance the deferred evaluation has not installed yet. Mirrors the
/// layout builder's own route: an installed `plan_laid_layouts` entry for
/// `{policy_symbol, schema_symbol}` answers immediately, otherwise the
/// attached `{policy}::plan` machine marks the operand plan-laid and the
/// query must wait for installation rather than error early.
fn plan_laid_operand_pending_installation(
    program: &TypedTrees,
    operand: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program.type_reference_table.type_reference(operand)
    else {
        return false;
    };
    if arguments.is_empty() || !base_symbol.is_valid() {
        return false;
    }
    let argument_handles = program
        .type_reference_table
        .type_reference_handles(*arguments);
    let installed = program.plan_laid_layouts.iter().any(|plan| {
        plan.policy_symbol == *base_symbol
            && argument_handles
                .iter()
                .any(|argument| program.type_reference_symbol(*argument) == plan.schema_symbol)
    });
    if installed {
        return false;
    }
    let Some(definition) = program.data_definition_by_symbol(*base_symbol) else {
        return false;
    };
    program.machines().iter().any(|machine| {
        machine.attached_data_symbol == *base_symbol
            && machine.name.as_str() == format!("{}::plan", definition.name)
    })
}

/// A `layout_type_reference` of an unresolved fixed array returns the
/// builder's `(0, 1)` default — bit-identical to a valid zero-sized layout.
/// Walk the operand's type-reference tree first so `size_of<[u8; N]>()` with
/// an unbound `N` rejects as "no fixed size" instead of silently answering
/// `size_of` on a phantom zero-extent carrier.
fn reject_unresolved_layout_operands(
    typed: &checked_trees::typed_trees::TypedTrees,
    root: TypeReferenceHandle,
) -> Result<(), String> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(reference) = pending.pop() {
        if !reference.is_valid() || visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                match length {
                    FixedArrayLength::Literal(_) => {}
                    FixedArrayLength::Expression(_) => return Err("`size_of`/`align_of` cannot measure an array with an unresolved extent expression".into()),
                    // `ConstCall` lengths must already have folded to
                    // `Literal` before settlement; a surviving non-literal
                    // length means the layout was never resolved.
                    FixedArrayLength::ConstParameter { name, .. } => {
                        return Err(format!(
                            "`size_of`/`align_of` cannot measure a fixed array: \
                             the length `{name}` is not resolved to a constant"
                        ));
                    }
                    FixedArrayLength::ConstCall { .. }
                        if typed.closed_fixed_array_length(reference).is_some() => {}
                    FixedArrayLength::ConstCall { name, .. } => {
                        return Err(format!(
                            "`size_of`/`align_of` cannot measure a fixed array whose \
                             length `{name}` never evaluated to a constant"
                        ));
                    }
                }
                pending.push(*element_type);
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
            TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Generic { arguments, .. } => {
                pending.extend(
                    typed
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .copied(),
                );
            }
            TypeReferenceNode::ConstExpression(_) => {
                return Err(
                    "`size_of`/`align_of` takes a data carrier, not a proof-static \
                     const expression"
                        .to_owned(),
                );
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => {}
        }
    }
    Ok(())
}
