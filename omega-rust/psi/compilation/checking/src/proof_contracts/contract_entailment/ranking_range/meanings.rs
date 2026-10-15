//! Exact declaration/custody meaning precedes arithmetic projection.
//! Admitting an evaluation supplies its carrier, not an integer coordinate:
//! sealed IEEE comparisons may guard an integer rank, while the strict
//! arithmetic engine independently declines their floating-point operands.
use super::{BinaryOperator, ExpressionHandle, ExpressionNode, Machine, PrimitiveType, State};
use typed_trees::expression::UnaryOperator;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn admitted_carrier(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Option<TypeReferenceHandle>> {
    let node = program
        .expression_table
        .expression_at_depth(expression, depth)?;
    if super::super::proof_integer::anonymous_integer_value(program, expression).is_some() {
        // This uses the existing anonymous-operator classifier and folds only
        // a wholly anonymous integral subtree, not typed integer division.
        return Some(None);
    }
    match node {
        ExpressionNode::Integer(_) => Some(
            program
                .closed_integer_value_in(expression, machine.symbol)?
                .type_reference,
        ),
        ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => Some(None),
        ExpressionNode::Name(path) if path.symbol.is_valid() => {
            if path.head_symbol == path.symbol
                && let Some(parameter) = program.state_parameter_by_symbol(state, path.symbol)
            {
                // Meaning is independent of binding stability. State and call
                // edge owners separately establish exact prefix preservation,
                // including for mutable parameters.
                return (!parameter.is_const).then_some(Some(parameter.type_reference));
            }
            // Declaration identity admits the place's carrier independently
            // of its current value. Saved/captured observation owners still
            // establish stability and supply values before normalization.
            crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            )
            .map(Some)
        }
        ExpressionNode::Atomic(atomic) => {
            admitted_carrier(program, machine, state, atomic.value, depth + 1)
        }
        ExpressionNode::Member(_) => {
            // The same exact declaration owner handles formal and local
            // projections. This gives a carrier, not permission to reread a
            // mutable field after its completed observation.
            if let Some(reference) = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return Some(Some(reference));
            }
            crate::value_custody::places::collection_length_receiver(
                program,
                machine,
                Some(state),
                expression,
            )?;
            // Structural collection metadata has builtin numeric meaning
            // regardless of where its descriptor lives. This admits no value:
            // the coordinate owner must independently supply its exact extent
            // or captured observation. Formal-only admission lost local array
            // measures even when their declared extent was already known.
            Some(None)
        }
        ExpressionNode::StructLiteral(literal) => {
            if !literal.type_symbol.is_valid() {
                return None;
            }
            // Construction owns the complete typed operand roster. Its rank
            // projection admits and forms the exact selected leaf separately;
            // an unrelated payload need not belong to integer arithmetic.
            // Whole-actual effects and formation stay with the source owners.
            crate::declarations::data_queries::data_definition_by_symbol(
                program,
                literal.type_symbol,
            )?;
            // An inline constructor need not retain a standalone named
            // reference (notably a lifetime application). Admission invents
            // no applied type; coordinate resolution still authenticates and
            // forms each demanded declared leaf independently.
            Some(crate::expression_result_type_reference(
                program, machine, state, expression,
            ))
        }
        ExpressionNode::ArrayLiteral(_) => {
            Some(Some(crate::parameter_expression_result_type_reference(
                program,
                machine.symbol,
                program.state_parameters(state),
                expression,
            )?))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection_type =
                admitted_carrier(program, machine, state, indexed.collection, depth + 1)?;
            if !program.expression_table.indexed_on_range(indexed) {
                // An element read can be another arrival's payload without
                // becoming a ranking coordinate. The shared place owner
                // authenticates its selected builtin operation; the raw
                // element type retains its arithmetic policy and constraints.
                // Bounds, access and value preservation remain independent
                // source obligations, not consequences of this carrier query.
                if !crate::value_custody::places::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    expression,
                ) {
                    return None;
                }
                admitted_carrier(program, machine, state, indexed.index, depth + 1)?;
                return crate::value_custody::places::declared_indexed_projection_type_raw(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                .map(Some);
            }
            // Range formation composes with its collection's meaning. The
            // length owner separately proves its geometry against that exact
            // collection; being a formal rather than an array or another
            // range is not part of the operation's meaning.
            if !crate::value_custody::places::has_builtin_subslice_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return None;
            }
            let range = program.expression_table.range_node(indexed.index)?;
            for endpoint in [range.start, range.end] {
                if endpoint.is_valid() {
                    admitted_carrier(program, machine, state, endpoint, depth + 1)?;
                }
            }
            Some(collection_type)
        }
        ExpressionNode::Borrow(borrow) => {
            // A borrow performs no operation of its own; the referent's
            // meaning is what the edge substitutes into a borrowed formal.
            admitted_carrier(program, machine, state, borrow.target, depth + 1)?;
            Some(None)
        }
        ExpressionNode::Call(call) => {
            // Result carriers belong to the selected public declaration,
            // including dynamic slots. A concrete body is not the authority
            // for that type or for the result's eventual guarantee.
            // Mutating an argument does not erase the result carrier. The
            // edge's complete write frame preserves its live premises;
            // normalization separately supplies an occurrence-bound result
            // from the public contract when the witness actually reads it.
            let owner = super::super::specification_calls::selected_call_contract(program, call)?;
            Some(Some(super::super::specification_calls::owner_return_type(
                &owner,
            )))
        }
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
            // A value cast between fixed integer primitives denotes the same
            // arithmetic operand the strict judgment normalizes through; its
            // carrier is the authored target, not the source. Recasts,
            // non-integer targets and non-integer sources keep no builtin
            // meaning: their formation obligations stay with the cast owner.
            admitted_carrier(program, machine, state, cast.value, depth + 1)?;
            let integer_target = program
                .primitive_type_reference(cast.target_type)
                .is_some_and(|target| target.is_fixed_integer());
            let integer_source =
                crate::proof_contracts::proof_embeddings::integer_embedding(program, cast.value)
                    .is_some_and(|source| source.primitive.is_fixed_integer());
            (integer_target && integer_source).then(|| Some(cast.target_type))
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            admitted_carrier(program, machine, state, unary.operand, depth + 1)?;
            Some(None)
        }
        ExpressionNode::Binary(binary) => {
            let left = admitted_carrier(program, machine, state, binary.left, depth + 1)?;
            let right = admitted_carrier(program, machine, state, binary.right, depth + 1)?;
            // A sealed IEEE comparison is a total Boolean evaluation with
            // exact selected meaning. Its operands still pass the same
            // declaration walk above; no float polynomial or bound is minted.
            if crate::proof_contracts::bound_expression_meaning::sealed_float_comparison(
                program,
                machine,
                Some(state),
                expression,
                binary,
            ) {
                return Some(None);
            }
            // `<<`/`>>` name no overloadable spelling: a typed occurrence is
            // the builtin operator. Its carrier is the shifted operand's own
            // type; the count is an independent integer operand whose
            // primitive need not agree with it. Carrier-less counts such as
            // a slice length remain admissible operands, as for division.
            if matches!(
                binary.operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            ) {
                let integer_carrier = |carrier| {
                    program
                        .primitive_type_reference(carrier)
                        .is_some_and(PrimitiveType::accepts_integer_literal)
                };
                return match left {
                    Some(left) if integer_carrier(left) && right.is_none_or(integer_carrier) => {
                        Some(Some(left))
                    }
                    _ => None,
                };
            }
            // `&` also names no overloadable spelling: a typed occurrence is
            // the builtin operator. Unlike a shift both operands share one
            // carrier -- an anonymous literal or natural coordinate is the
            // same wildcard input division's operands are -- and the
            // operation is total inside it, so meaning keeps that agreed
            // integer primitive as the result carrier.
            if binary.operator == BinaryOperator::BitwiseAnd {
                if left.zip(right).is_some_and(|(left, right)| {
                    program.primitive_type_reference(left)
                        != program.primitive_type_reference(right)
                }) {
                    return None;
                }
                return match left.or(right) {
                    Some(carrier)
                        if program
                            .primitive_type_reference(carrier)
                            .is_some_and(PrimitiveType::accepts_integer_literal) =>
                    {
                        Some(Some(carrier))
                    }
                    _ => None,
                };
            }
            let spelling = match binary.operator {
                BinaryOperator::And | BinaryOperator::Or => return Some(None),
                operator => match typed_trees::operator::binary_operator_spelling(operator) {
                    Some(spelling) => spelling,
                    None => return None,
                },
            };
            // Proof coordinates can be carrier-less (notably slice length),
            // but operator selection needs their actual runtime types. Keep
            // the coordinate carriers below independent of this dispatch query.
            let operand_types = [binary.left, binary.right].map(|operand| {
                crate::proof_contracts::bound_expression_meaning::operand_type(
                    program,
                    machine,
                    Some(state),
                    crate::expression_traversal::unwrapped(program, operand),
                )
            });
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                &operand_types,
            ) {
                return None;
            }
            // Only already-admitted builtin arithmetic inherits its operand
            // carrier. This does not turn a natural coordinate into stored data.
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) {
                if left.zip(right).is_some_and(|(left, right)| {
                    program.primitive_type_reference(left)
                        != program.primitive_type_reference(right)
                }) {
                    return None;
                }
                Some(left.or(right))
            } else {
                Some(None)
            }
        }
        _ => None,
    }
}

/// Bind each Exact non-polynomial term admitted by this ranking query.
/// The general strict arithmetic engine does not infer executable division
/// or shifting from a token. Anonymous rational subtrees still fold as
/// rationals; each landed quotient, remainder, or shift retains both
/// operands for state transport. This is meaning, not formation: the range
/// owner still checks every operation before using the endpoint, including
/// an overflowing intermediate quotient or an out-of-width shift count.
pub(super) fn install_nonpolynomial_terms(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    engine: &mut super::Engine<'_>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<()> {
    if depth >= 128 {
        return None;
    }
    if super::super::proof_integer::anonymous_integer_value(program, expression).is_some() {
        return Some(());
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            install_nonpolynomial_terms(program, machine, state, engine, atomic.value, depth + 1)?;
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                install_nonpolynomial_terms(
                    program,
                    machine,
                    state,
                    engine,
                    call.receiver,
                    depth + 1,
                )?;
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                install_nonpolynomial_terms(program, machine, state, engine, *argument, depth + 1)?;
            }
        }
        ExpressionNode::Binary(binary) => {
            for operand in [binary.left, binary.right] {
                install_nonpolynomial_terms(program, machine, state, engine, operand, depth + 1)?;
            }
            if matches!(
                binary.operator,
                BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
            ) {
                if let Some(carrier) = admitted_carrier(program, machine, state, expression, 0)? {
                    let primitive = super::exact_integer_parameter(program, carrier)?;
                    match binary.operator {
                        BinaryOperator::Divide | BinaryOperator::Modulo => {
                            engine.bind_strict_integer_division(expression)?
                        }
                        // The count's defined range is the shifted
                        // carrier's width (the F8 ruling), so the term mints
                        // with it rather than re-deriving a bound.
                        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => engine
                            .bind_strict_integer_shift(
                                expression,
                                crate::proof_contracts::arithmetic_domains::integer_bit_width(
                                    primitive,
                                )? as u32,
                            )?,
                        // `&` is total inside the shared carrier, so the
                        // term keeps the carrier's shape -- width and
                        // signedness -- rather than a defined-range bound.
                        BinaryOperator::BitwiseAnd => engine.bind_strict_integer_bitwise_and(
                            expression,
                            crate::proof_contracts::arithmetic_domains::integer_bit_width(
                                primitive,
                            )? as u32,
                            primitive.is_signed_integer(),
                        )?,
                        _ => return None,
                    }
                } else if binary.operator != BinaryOperator::Modulo {
                    return None;
                }
                // Natural coordinates such as a slice length have no machine
                // carrier here. Their constant-modulus normalization remains
                // with the existing arithmetic engine, not a fabricated width.
            }
        }
        _ => {}
    }
    Some(())
}
