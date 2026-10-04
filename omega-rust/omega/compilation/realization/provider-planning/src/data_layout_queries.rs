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
//! are the `LayoutPlanReport::{size, align}` that
//! `build-time-evaluation::compute_native_layout_plan` produced and the layout
//! installer validated, so policy, schema, and field custody stay exactly
//! where they were judged. Machine declarations and proof-only carriers were
//! already rejected at lowering, where the operand was admitted as a data
//! type.

use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Fold every retained `size_of`/`align_of` query in the typed program to
/// its `u64` literal under `target`. Also stamps the canonical `u64` encoding
/// on a `const` whose materialized initializer root was the query node, so
/// the folded declaration joins ordinary constant identity.
pub fn fold_data_layout_queries(
    program: &mut TypedTrees,
    target: target::NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<(), Vec<Diagnostic>> {
    // The proof-only classification is computed over the typed program;
    // a `size_of<Nat>` over recursive data must name the
    // classification instead of reporting the layout builder's zero default.
    let proof_only = validation::proof_only_classification(program);
    let queries: Vec<(
        typed_trees::expression::ExpressionHandle,
        typed_trees::expression::DataLayoutQueryRequest,
        source::SourceSpan,
    )> = program
        .expression_table
        .iter_expressions()
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
    for (handle, query, reference) in queries {
        let operand = query.operand.type_reference;
        let spelling = query.kind.target_name();
        if let Some(held) = proof_only.proof_only_mention(program, operand) {
            return Err(vec![
                Diagnostic::error(format!(
                    "`{spelling}` cannot measure a proof-only carrier: `{held}` has no runtime layout"
                ))
                .with_source_span(reference),
            ]);
        }
        reject_unresolved_layout_operands(program, operand)
            .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(reference)])?;
        let layout = layout::layout_type_reference(
            program,
            target,
            opaque_representation_selections,
            operand,
        )
        .map_err(|error| vec![error])?;
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
        // A `const N: u64 = size_of<T>()` declaration carries the query node
        // as its materialized root; folding the node in place materializes the
        // constant, so stamp the same canonical `u64` encoding the scalar
        // evaluator would have produced.
        program
            .tables
            .const_declarations
            .for_each_mut(|_, declaration| {
                if declaration.materialized_initializer == handle
                    && declaration.evaluated_value.is_none()
                {
                    declaration.evaluated_value = Some(
                        language_semantics::const_value::CanonicalConstIdentity::integer(
                            "u64",
                            i128::from(value),
                        )
                        .encoding
                        .into(),
                    );
                }
            });
    }
    Ok(())
}

/// A `layout_type_reference` of an unresolved fixed array returns the
/// builder's `(0, 1)` default — bit-identical to a valid zero-sized layout.
/// Walk the operand's type-reference tree first so `size_of<[u8; N]>()` with
/// an unbound `N` rejects as "no fixed size" instead of silently answering
/// `size_of` on a phantom zero-extent carrier.
fn reject_unresolved_layout_operands(
    typed: &typed_trees::TypedTrees,
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
                    // `ConstCall` lengths must already have folded to
                    // `Literal` before settlement; a surviving non-literal
                    // length means the layout was never resolved.
                    FixedArrayLength::ConstParameter { name, .. } => {
                        return Err(format!(
                            "`size_of`/`align_of` cannot measure a fixed array: \
                             the length `{name}` is not resolved to a constant"
                        ));
                    }
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
