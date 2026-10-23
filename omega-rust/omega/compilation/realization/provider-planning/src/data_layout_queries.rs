//! `size_of<T>()`/`align_of<T>()` evaluation against the selected target.
//!
//! Psi recognizes the compiler-owned spellings and retains each occurrence as
//! an explicit `DataLayoutQueryRequest` on the call (see
//! `language_core::data_layout_queries`); it cannot answer them because the
//! answer depends on the target's layout. Once execution settlement has
//! selected the native target, this module folds every retained query into an
//! ordinary `u64` integer literal in place: every parent expression and every
//! `const` materialized initializer that shared the node sees the literal.
//!
//! The queried layout is never recomputed here. Ordinary type references go
//! through `layout::layout_type_reference` — the backend's layout authority —
//! which reports the carrier's own storage (padding included, erased fields
//! omitted, references measured as the reference, not the referent). A type
//! whose `data` definition carries a validated layout plan consults the
//! installed `PlanLaidLayout` inside that same path: its size and alignment
//! are the `LayoutPlanReport::{size, align}` that the plan-laid policy
//! evaluation in `build-time-evaluation` produced and the layout
//! installer validated, so policy, schema, and field custody stay exactly
//! where they were judged. Machine declarations and proof-only carriers were
//! already rejected at lowering, where the operand was admitted as a data
//! type.

use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::expression::ExpressionNode;
use checked_trees::typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};

/// Fold every selected declaration's retained `size_of`/`align_of` query in the typed program to
/// its `u64` literal under `target`. Psi then completes each retained scalar
/// initializer through ordinary semantic evaluation, including derived expressions.
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
    target: target::TargetProfile,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    defer_uninstalled_plans: bool,
    selection: &checked_trees::typed_trees::references::DeclarationSelection,
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
    // The physical backend consumes invariant geometry. A family needs one
    // private layout view closed under this exact profile; source alternatives
    // remain intact for Terminal production. All queries borrow that same view.
    let layout_program = program
        .plan_laid_layouts
        .iter()
        .any(|plan| {
            matches!(
                plan.realizations,
                checked_trees::typed_trees::PlanLaidRealizations::TargetFamily(_)
            )
        })
        .then(|| {
            let mut closed = program.clone();
            for (plan, authored) in closed
                .plan_laid_layouts
                .iter_mut()
                .zip(&program.plan_laid_layouts)
            {
                if let Some(geometry) =
                    authored.geometry_for_target(program, Some(target.target_name()))
                {
                    plan.realizations = checked_trees::typed_trees::PlanLaidRealizations::Invariant(
                        geometry.clone(),
                    );
                }
            }
            closed
        });
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
        let layout = layout::try_layout_type_reference(
            layout_program.as_ref().unwrap_or(program),
            target.native_target(),
            opaque_representation_selections,
            operand,
        )
        .map_err(|error| vec![error])?;
        let Some(layout) = layout else {
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
        let extent = match query.kind {
            DataLayoutQueryKind::Size => layout.size,
            DataLayoutQueryKind::Alignment => layout.alignment,
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
