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

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::CheckedTrees;
    use numerics::arithmetic::ArithmeticDomain;

    fn typed_fixture(source: &str) -> typed_trees::TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize layout-query fixture");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse layout-query fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .expect("resolve layout-query fixture");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type layout-query fixture")
    }

    fn checked_fixture(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
        typed_trees_to_checked_trees::lower_typed_trees(
            typed_fixture(source),
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
    }

    /// Handles of every retained layout query, in expression-table order.
    fn query_handles(program: &CheckedTrees) -> Vec<typed_trees::expression::ExpressionHandle> {
        program
            .typed
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, node)| match node {
                ExpressionNode::Call(call) if call.layout_query.is_some() => Some(handle),
                _ => None,
            })
            .collect()
    }

    fn folded_value(
        program: &CheckedTrees,
        handle: typed_trees::expression::ExpressionHandle,
    ) -> (i64, numerics::literals::IntegerLanding) {
        let ExpressionNode::Integer(literal) = program.typed.expression_table.expression(handle)
        else {
            panic!("layout query did not fold to an integer literal")
        };
        (
            literal.value_i64().expect("folded literal fits i64"),
            literal.landing().expect("folded literal landed at u64"),
        )
    }

    #[test]
    fn queries_fold_to_the_selected_targets_layout() {
        let mut checked = checked_fixture(
            r#"
            data Row { a: u64; b: u32; }
            data Empty {}
            const ROW_BYTES: u64 = size_of<Row>();
            const ROW_ALIGN: u64 = align_of<Row>();
            const EMPTY_BYTES: u64 = size_of<Empty>();
            data Main {}
            machine Main::main(&mut self) {
                let n: u64 = size_of<Row>();
                let m: u64 = align_of<Row>();
            }
            "#,
        )
        .expect("layout-query fixture checks");
        // Each `const` retains an authored evidence copy beside the
        // materialized initializer, so the three query leaves plus two
        // runtime uses are eight retained occurrences.
        let handles = query_handles(&checked);
        assert_eq!(handles.len(), 8);
        fold_data_layout_queries(&mut checked.typed, target::NativeTarget::host(), &[])
            .expect("host fold");
        for handle in &handles {
            let (_, landing) = folded_value(&checked, *handle);
            assert_eq!(
                landing,
                IntegerLanding {
                    landed_type: LandedIntegerType::U64,
                    domain: ArithmeticDomain::Exact,
                }
            );
        }
        // Each declaration's materialized root is the literal the selected
        // target measured: padding counts for `Row`, and a zero-sized data
        // type is a real answer, not an unresolved layout.
        let declared = |name: &str| {
            checked
                .typed
                .tables
                .const_declarations
                .iter()
                .map(|(_, declaration)| declaration)
                .find(|declaration| checked.typed.symbols.name(declaration.symbol) == name)
                .unwrap_or_else(|| panic!("const `{name}`"))
        };
        let materialized_value =
            |name: &str| folded_value(&checked, declared(name).materialized_initializer).0;
        assert_eq!(materialized_value("ROW_BYTES"), 16);
        assert_eq!(materialized_value("ROW_ALIGN"), 8);
        assert_eq!(materialized_value("EMPTY_BYTES"), 0);
        // The materialized `const` carries the same canonical `u64` encoding
        // the scalar evaluator produces for an authored literal.
        assert_eq!(
            declared("ROW_BYTES")
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding()),
            Some(
                language_semantics::const_value::CanonicalConstIdentity::integer("u64", 16)
                    .encoding
                    .as_str()
            ),
        );
    }

    #[test]
    fn reference_operand_reports_the_reference_not_the_referent() {
        let mut checked = checked_fixture(
            r#"
            data Row { a: u64; b: u32; }
            const REF_BYTES: u64 = size_of<&Row>();
            "#,
        )
        .expect("reference query fixture checks");
        let handles = query_handles(&checked);
        assert_eq!(handles.len(), 2, "authored plus materialized copy");
        fold_data_layout_queries(&mut checked.typed, target::NativeTarget::host(), &[])
            .expect("host fold");
        for handle in handles {
            // A host reference is one pointer slot, not `Row`'s 16 bytes.
            assert_eq!(folded_value(&checked, handle).0, 8);
        }
    }

    /// Reads the materialized `const` initializer's folded value by name.
    fn materialized_const_value(program: &CheckedTrees, name: &str) -> i64 {
        let declaration = program
            .typed
            .tables
            .const_declarations
            .iter()
            .map(|(_, declaration)| declaration)
            .find(|declaration| program.typed.symbols.name(declaration.symbol) == name)
            .unwrap_or_else(|| panic!("const `{name}`"));
        folded_value(program, declaration.materialized_initializer).0
    }

    #[test]
    fn fixed_arrays_measure_literal_extent_and_zero_length() {
        let mut checked = checked_fixture(
            r#"
            const ARR_BYTES: u64 = size_of<[u64; 3]>();
            const ARR_ALIGN: u64 = align_of<[u64; 3]>();
            const ZERO_BYTES: u64 = size_of<[u8; 0]>();
            "#,
        )
        .expect("array operand fixture checks");
        fold_data_layout_queries(&mut checked.typed, target::NativeTarget::host(), &[])
            .expect("host fold");
        assert_eq!(materialized_const_value(&checked, "ARR_BYTES"), 24);
        assert_eq!(materialized_const_value(&checked, "ARR_ALIGN"), 8);
        // A zero-length array is a validated zero-sized layout answering a
        // real 0; the unresolved-length fallback shares this tuple only on
        // the reject path exercised below.
        assert_eq!(materialized_const_value(&checked, "ZERO_BYTES"), 0);
    }

    #[test]
    fn slice_reference_reports_the_fat_descriptor() {
        let mut checked = checked_fixture(
            r#"
            const VIEW_BYTES: u64 = size_of<&[u8]>();
            const VIEW_ALIGN: u64 = align_of<&[u8]>();
            "#,
        )
        .expect("slice-view operand fixture checks");
        fold_data_layout_queries(&mut checked.typed, target::NativeTarget::host(), &[])
            .expect("host fold");
        // `&[u8]` retains its `{pointer, length}` descriptor: two pointer
        // slots at pointer alignment, not the element's 1/1 and not a thin
        // pointer.
        let host = target::NativeTarget::host();
        assert_eq!(
            materialized_const_value(&checked, "VIEW_BYTES"),
            i64::try_from(2 * host.pointer_size).unwrap(),
        );
        assert_eq!(
            materialized_const_value(&checked, "VIEW_ALIGN"),
            i64::try_from(host.pointer_alignment).unwrap(),
        );
    }

    #[test]
    fn unresolved_array_length_rejects_instead_of_zero_sizing() {
        let mut typed = typed_fixture("data Main {}");
        // Synthesize the shape a surviving unresolved length produces: the
        // layout builder's (0, 1) fallback for it is bit-identical to a real
        // zero-sized layout, so the operand walk must reject before that
        // fallback is read as an answer.
        let element = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: typed_trees::name::Identifier::from("u8"),
        });
        let unresolved = typed
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: element,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbols::SymbolHandle::invalid(),
                    name: typed_trees::name::Identifier::from("N"),
                },
            });
        let reason = reject_unresolved_layout_operands(&typed, unresolved)
            .expect_err("an unbound const length is not a validated zero");
        assert!(reason.contains("not resolved to a constant"), "{reason}");
    }

    #[test]
    fn proof_only_carrier_is_rejected_not_zero_sized() {
        let mut checked = checked_fixture(
            r#"
            data Nat { next: Nat; }
            const NAT_BYTES: u64 = size_of<Nat>();
            "#,
        )
        .expect("proof-only fixture checks");
        let diagnostics =
            fold_data_layout_queries(&mut checked.typed, target::NativeTarget::host(), &[])
                .expect_err("proof-only carrier must not fold");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("proof-only")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn machine_declarations_are_not_data_carriers() {
        // The operand rejection lives at lowering: a machine or state symbol
        // is never a layout operand, so the query must not type.
        let tokens = source_files_to_tokens::Lexer::new(
            r#"
            data Motor {}
            machine Motor::run(&mut self) {}
            const BAD: u64 = size_of<Motor::run>();
            "#,
        )
        .tokenize()
        .expect("tokenize machine-operand fixture");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse machine-operand fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .expect("resolve machine-operand fixture");
        let diagnostic =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect_err("a machine operand must not type as a layout query");
        assert!(
            diagnostic
                .message
                .contains("a machine declaration is not a data carrier"),
            "{diagnostic:?}"
        );
    }
}
