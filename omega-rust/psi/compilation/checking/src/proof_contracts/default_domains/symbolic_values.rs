//! Conservative symbolic evaluation for default-domain invariant checks.

use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

/// The transported value is its landed representation, not host arithmetic
/// and not an integer encoding of a float interval. Bitwise equality gives
/// the state meet a stable rule even for NaNs and signed zeros.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LiteralValue {
    Integer(i128),
    Float32(u32),
    Float64(u64),
}

impl LiteralValue {
    pub(super) fn integer(self) -> Option<i128> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }

    fn float(
        self,
        primitive: typed_trees::types::PrimitiveType,
    ) -> Option<numerics::float_semantics::FloatMeaning> {
        use numerics::float_semantics::FloatMeaning;
        match (self, primitive) {
            (Self::Float32(bits), typed_trees::types::PrimitiveType::F32) => {
                Some(FloatMeaning::from_f32(f32::from_bits(bits)))
            }
            (Self::Float64(bits), typed_trees::types::PrimitiveType::F64) => {
                Some(FloatMeaning::from_f64(f64::from_bits(bits)))
            }
            _ => None,
        }
    }
}

pub(super) fn literal_value(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<LiteralValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.text().parse().ok().map(LiteralValue::Integer),
        ExpressionNode::Float(value) => {
            use numerics::float_semantics::{FloatFormat, FloatSemantics};
            Some(match value.landing()? {
                numerics::literals::FloatFormat::F32 => LiteralValue::Float32(
                    FloatSemantics::from_decimal(FloatFormat::BINARY32, value.text())?
                        .to_f32()
                        .to_bits(),
                ),
                numerics::literals::FloatFormat::F64 => LiteralValue::Float64(
                    FloatSemantics::from_decimal(FloatFormat::BINARY64, value.text())?
                        .to_f64()
                        .to_bits(),
                ),
            })
        }
        ExpressionNode::Borrow(inner) => literal_value(program, inner.target),
        _ => None,
    }
}

/// Distinguish IEEE predicates before the integer symbolic shortcut sees
/// them. In particular `value == value` is not true for an unknown float.
pub(super) fn mentions_float(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Float(_) | ExpressionNode::UndeterminedNaN(_) => true,
        ExpressionNode::Name(path) => {
            let is_float_field = |field: &typed_trees::data::DataField| {
                field.symbol.is_valid()
                    && field.symbol == path.symbol
                    && matches!(
                        program.primitive_type_reference(field.type_reference),
                        Some(
                            typed_trees::types::PrimitiveType::F32
                                | typed_trees::types::PrimitiveType::F64
                        )
                    )
            };
            program
                .data_members(definition)
                .iter()
                .any(|member| match member {
                    typed_trees::data::DataMember::Field(field) => is_float_field(field),
                    typed_trees::data::DataMember::Variant(variant) => program
                        .data_payload_fields(variant)
                        .iter()
                        .any(is_float_field),
                })
        }
        ExpressionNode::Binary(binary) => {
            mentions_float(program, definition, binary.left)
                || mentions_float(program, definition, binary.right)
        }
        ExpressionNode::Unary(unary) => mentions_float(program, definition, unary.operand),
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SymbolicValue {
    Atom(symbols::SymbolHandle),
    Integer(i128),
    Add(Vec<SymbolicValue>),
    Multiply(Vec<SymbolicValue>),
    Subtract(Box<SymbolicValue>, Box<SymbolicValue>),
}

pub(super) fn integer_literal_value(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<i128> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Borrow(inner) => integer_literal_value(program, inner.target),
        _ => None,
    }
}

/// Fold a where fact over the tracked valuation: tracked fields read their
/// value (a non-literal write poisons), untracked fields read the ZII zero
/// (machine-owned data is born zeroed).
pub(super) fn fold(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    valuation: &[(&str, Option<LiteralValue>)],
    symbols: &[(String, SymbolicValue)],
    measures: &[(String, Option<i128>, Option<i128>)],
    born_zero: bool,
    expression: ExpressionHandle,
) -> Option<i128> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            // Application views retain authored predicate symbols. A static
            // binder reads its exact application argument, never a field's
            // born-zero valuation (and never a same-spelled method binder).
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind == symbols::SymbolKind::TypeParameter
            {
                return applied_const_integer(program, definition, path.symbol)
                    .map(|(_, value)| value);
            }
            let last = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            match valuation.iter().find(|(name, _)| *name == last) {
                Some((_, value)) => value.and_then(LiteralValue::integer),
                // SOUNDNESS (slice 4): the born zero is real only in the
                // never-re-entered boot state; elsewhere an untracked field
                // may hold any prior value -- poison the fold.
                None if born_zero => Some(0),
                None => None,
            }
        }
        ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
        ExpressionNode::Member(member)
            if CollectionMeasure::from_authored_spelling(member.member.as_str()).is_some() =>
        {
            let Some(path) = program.expression_table.name_path_node(member.receiver) else {
                return None;
            };
            let field = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            match measures.iter().find(|(name, _, _)| name == field) {
                Some((_, length, capacity)) => {
                    match CollectionMeasure::from_authored_spelling(member.member.as_str()) {
                        Some(CollectionMeasure::Length) => *length,
                        Some(CollectionMeasure::Capacity) => *capacity,
                        None => None,
                    }
                }
                None if born_zero => Some(0),
                None => None,
            }
        }
        ExpressionNode::Binary(binary) => {
            if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                && mentions_float(program, definition, expression)
            {
                return super::float_predicates::data_float_predicate(
                    program,
                    definition,
                    expression,
                    &|symbol, primitive| {
                        let field = program.data_field(definition, symbol)?;
                        match valuation
                            .iter()
                            .find(|(name, _)| *name == field.name.as_str())
                        {
                            Some((_, value)) => value.and_then(|value| value.float(primitive)),
                            None if born_zero => {
                                Some(numerics::float_semantics::FloatMeaning::Zero {
                                    negative: false,
                                })
                            }
                            None => None,
                        }
                    },
                )
                .map(i128::from);
            }
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) && let (Some(left), Some(right)) = (
                symbolic_operand(program, symbols, binary.left),
                symbolic_operand(program, symbols, binary.right),
            ) && left == right
            {
                return Some(i128::from(matches!(binary.operator, BinaryOperator::Equal)));
            }
            let left = fold(
                program,
                definition,
                valuation,
                symbols,
                measures,
                born_zero,
                binary.left,
            )?;
            let right = fold(
                program,
                definition,
                valuation,
                symbols,
                measures,
                born_zero,
                binary.right,
            )?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                BinaryOperator::Less => Some(i128::from(left < right)),
                BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                BinaryOperator::Greater => Some(i128::from(left > right)),
                BinaryOperator::Equal => Some(i128::from(left == right)),
                BinaryOperator::NotEqual => Some(i128::from(left != right)),
                BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Read a scalar static argument through the producer's retained application.
/// Missing or symbolic arguments remain unknown; this query creates no types,
/// expressions or evidence and cannot certify an open declaration at zero.
pub(crate) fn applied_const_integer(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    binder: symbols::SymbolHandle,
) -> Option<(typed_trees::types::PrimitiveType, i128)> {
    let (primitive, actual) = applied_const_argument(program, definition, binder)?;
    let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(actual)
    else {
        return None;
    };
    if symbol.is_valid() {
        return None;
    }
    if let Some(value) =
        language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
    {
        return match value.decode_encoding()? {
            language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                value, ..
            } => Some((primitive, value)),
            _ => None,
        };
    }
    name.as_str().parse().ok().map(|value| (primitive, value))
}

/// The retained declaration-to-argument join is meaningful before runtime
/// closure too. A symbolic actual retains its exact lexical binder; callers
/// either bind its admitted value or leave it unknown, never zero it.
pub(super) fn applied_const_argument(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    binder: symbols::SymbolHandle,
) -> Option<(
    typed_trees::types::PrimitiveType,
    typed_trees::types::TypeReferenceHandle,
)> {
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program
        .type_reference_table
        .type_reference(definition.generic_instance?)
    else {
        return None;
    };
    let template = program.unique_data_definition_by_symbol(*base_symbol)?;
    let parameters = program.data_type_parameters(template);
    let position = parameters
        .iter()
        .position(|parameter| parameter.symbol == binder && parameter.kind.is_const_or_value())?;
    let (typed_trees::data::TypeParameterKind::Const { type_reference }
    | typed_trees::data::TypeParameterKind::Value { type_reference }) = parameters[position].kind
    else {
        return None;
    };
    let primitive = program.primitive_type_reference(type_reference)?;
    if !primitive.is_fixed_integer() {
        return None;
    }
    let actual = *program
        .type_reference_table
        .type_reference_handles(*arguments)
        .get(position)?;
    Some((primitive, actual))
}

pub(super) fn expression_symbolic_value(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolicValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if path.symbol.is_valid() => {
            Some(SymbolicValue::Atom(path.symbol))
        }
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i128>()
            .ok()
            .map(SymbolicValue::Integer),
        ExpressionNode::Binary(binary) => symbolic_binary_shape(binary, |expression| {
            expression_symbolic_value(program, expression)
        }),
        _ => None,
    }
}

fn commutative_symbolic_value(
    add: bool,
    left: SymbolicValue,
    right: SymbolicValue,
) -> SymbolicValue {
    // Canonicalize commutation only. Do not flatten/reassociate: signed
    // saturating addition is commutative but not associative, and symbolic
    // provenance must not silently strengthen the selected arithmetic theory.
    let mut operands = vec![left, right];
    operands.sort_by_key(symbolic_sort_key);
    if add {
        SymbolicValue::Add(operands)
    } else {
        SymbolicValue::Multiply(operands)
    }
}

fn symbolic_sort_key(value: &SymbolicValue) -> String {
    match value {
        SymbolicValue::Atom(symbol) => {
            format!("a:{}:{}", symbol.arena_index(), symbol.generation())
        }
        SymbolicValue::Integer(value) => format!("i:{value}"),
        SymbolicValue::Add(values) => format!(
            "+({})",
            values
                .iter()
                .map(symbolic_sort_key)
                .collect::<Vec<_>>()
                .join(",")
        ),
        SymbolicValue::Multiply(values) => format!(
            "*({})",
            values
                .iter()
                .map(symbolic_sort_key)
                .collect::<Vec<_>>()
                .join(",")
        ),
        SymbolicValue::Subtract(left, right) => {
            format!(
                "-({},{})",
                symbolic_sort_key(left),
                symbolic_sort_key(right)
            )
        }
    }
}

fn symbolic_operand(
    program: &dyn crate::CheckingInput,
    symbols: &[(String, SymbolicValue)],
    expression: ExpressionHandle,
) -> Option<SymbolicValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let field = program
                .expression_table
                .name_path_members(path.members)
                .last()?
                .as_str();
            symbols
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, value)| value.clone())
                .or_else(|| {
                    path.symbol
                        .is_valid()
                        .then_some(SymbolicValue::Atom(path.symbol))
                })
        }
        ExpressionNode::Integer(value) => value
            .text()
            .parse::<i128>()
            .ok()
            .map(SymbolicValue::Integer),
        ExpressionNode::Binary(binary) => symbolic_binary_shape(binary, |expression| {
            symbolic_operand(program, symbols, expression)
        }),
        _ => None,
    }
}

pub(super) fn expression_sequence_measures(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(Option<i128>, Option<i128>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::String(literal) => {
            let measure = i128::try_from(literal.len()).ok();
            Some((measure, measure))
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let measure =
                i128::try_from(program.expression_table.expression_handles(*elements).len()).ok();
            Some((measure, measure))
        }
        _ => None,
    }
}

/// Whether ANY reachable nested expression is a call. A call is a hard
/// consumption point for open invariant windows, so the traversal must cover
/// every expression shape that can carry one -- casts, unary operands,
/// indices, literals' elements and fields, match subjects and arms, ranges,
/// and atomic value/result -- not merely the binary/member/breadth spine.
pub(super) fn expression_contains_call(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call(program, binary.left)
                || expression_contains_call(program, binary.right)
        }
        ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
        ExpressionNode::Borrow(inner) => expression_contains_call(program, inner.target),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call(program, indexed.collection)
                || expression_contains_call(program, indexed.index)
        }
        ExpressionNode::Cast(cast) => expression_contains_call(program, cast.value),
        ExpressionNode::Unary(unary) => expression_contains_call(program, unary.operand),
        ExpressionNode::Atomic(atomic) => {
            expression_contains_call(program, atomic.value)
                || expression_contains_call(program, atomic.result)
        }
        ExpressionNode::Range(range) => {
            expression_contains_call(program, range.start)
                || expression_contains_call(program, range.end)
        }
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| expression_contains_call(program, *element)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains_call(program, field.value)),
        ExpressionNode::Match(matched) => {
            expression_contains_call(program, matched.subject)
                || program
                    .expression_table
                    .match_arms(matched.arms)
                    .iter()
                    .any(|arm| {
                        expression_contains_call(program, arm.value)
                            || match arm.pattern {
                                typed_trees::expression::MatchPattern::Value(pattern) => {
                                    expression_contains_call(program, pattern)
                                }
                                typed_trees::expression::MatchPattern::Wildcard => false,
                            }
                    })
        }
        _ => false,
    }
}

/// The arithmetic binary shape shared by operand sources: commutative
/// Add/Multiply canonicalization and ordered Subtract.
fn symbolic_binary_shape(
    binary: &typed_trees::expression::TableBinaryExpression,
    operand: impl Fn(ExpressionHandle) -> Option<SymbolicValue>,
) -> Option<SymbolicValue> {
    let left = operand(binary.left)?;
    let right = operand(binary.right)?;
    match binary.operator {
        BinaryOperator::Add => Some(commutative_symbolic_value(true, left, right)),
        BinaryOperator::Multiply => Some(commutative_symbolic_value(false, left, right)),
        BinaryOperator::Subtract => Some(SymbolicValue::Subtract(Box::new(left), Box::new(right))),
        _ => None,
    }
}
