use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_numerics::literals::{IntegerLanding, IntegerLiteral};

use super::super::super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use super::super::super::storage_places::{enum_variant_value, enum_variant_value_in_table};
use omega_platform_interface::PlaceKey;

const INLINE_RUNTIME_STATIC_VALUE_COUNT: usize = 8;

/// Exact bits plus the phase-B landing that gives those bits width,
/// signedness, and arithmetic policy. The former bare-i64 table silently
/// erased this interpretation whenever a folded constant crossed a place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeStaticInteger {
    bits: i64,
    landing: Option<IntegerLanding>,
}

impl RuntimeStaticInteger {
    pub(super) fn anonymous(bits: i64) -> Self {
        Self {
            bits,
            landing: None,
        }
    }

    fn from_literal(literal: &IntegerLiteral) -> Option<Self> {
        Some(Self {
            bits: literal.bits_u64()? as i64,
            landing: literal.landing(),
        })
    }

    pub(super) fn bits(self) -> i64 {
        self.bits
    }

    pub(super) fn landing(self) -> Option<IntegerLanding> {
        self.landing
    }

    pub(super) fn with_bits(self, bits: i64) -> Self {
        Self { bits, ..self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStaticValues {
    inline: [Option<(PlaceKey, RuntimeStaticInteger)>; INLINE_RUNTIME_STATIC_VALUE_COUNT],
    len: usize,
    overflow: Vec<(PlaceKey, RuntimeStaticInteger)>,
}

impl RuntimeStaticValues {
    pub(crate) fn new() -> Self {
        Self::with_capacity(0)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            overflow: Vec::with_capacity(
                capacity.saturating_sub(INLINE_RUNTIME_STATIC_VALUE_COUNT),
            ),
        }
    }

    pub(crate) fn clear(&mut self) {
        let inline_len = self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT);
        for slot in self.inline.iter_mut().take(inline_len) {
            *slot = None;
        }

        self.len = 0;
        self.overflow.clear();
    }

    fn get(&self, target: &PlaceKey) -> Option<RuntimeStaticInteger> {
        self.iter()
            .find(|(existing_target, _)| existing_target == target)
            .map(|(_, value)| *value)
    }

    /// Forget every recorded constant for a whole place subtree -- every key
    /// that `starts_with(prefix)`. Used after a RUNTIME-indexed write `arr[i] =
    /// ..` (non-constant index), which can land on ANY element of `arr`: each
    /// sibling `arr[k]` constant is now potentially stale, so the entire
    /// collection must fall back to live storage.
    fn invalidate_prefix(&mut self, prefix: &PlaceKey) {
        for slot in self
            .inline
            .iter_mut()
            .take(self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT))
        {
            if matches!(slot, Some((existing, _)) if existing.starts_with(prefix)) {
                *slot = None;
            }
        }
        self.overflow
            .retain(|(existing, _)| !existing.starts_with(prefix));
    }

    fn set(&mut self, target: PlaceKey, value: RuntimeStaticInteger) {
        if let Some((_, existing_value)) = self
            .iter_mut()
            .find(|(existing_target, _)| existing_target == &target)
        {
            *existing_value = value;
            return;
        }

        if self.len < INLINE_RUNTIME_STATIC_VALUE_COUNT {
            self.inline[self.len] = Some((target, value));
        } else {
            self.overflow.push((target, value));
        }

        self.len += 1;
    }

    fn iter(&self) -> impl Iterator<Item = &(PlaceKey, RuntimeStaticInteger)> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut (PlaceKey, RuntimeStaticInteger)> {
        self.inline
            .iter_mut()
            .take(self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT))
            .filter_map(Option::as_mut)
            .chain(self.overflow.iter_mut())
    }
}

impl Default for RuntimeStaticValues {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn resolve_runtime_static_integer_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
) -> Option<i64> {
    resolve_runtime_static_integer(
        input,
        source_key,
        expression,
        aliases,
        alias_expressions,
        static_values,
    )
    .map(RuntimeStaticInteger::bits)
}

pub(super) fn resolve_runtime_static_integer(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
) -> Option<RuntimeStaticInteger> {
    match expression {
        Expression::Atomic(_) => None,
        // Full 8-byte pattern: the literal-width gate guarantees an oversize
        // literal only reaches u64-classed (8-byte) targets, where these bits
        // ARE the value.
        Expression::Integer(value) => RuntimeStaticInteger::from_literal(value),
        Expression::Name(_) => enum_variant_value(&input.layouts, expression)
            .map(RuntimeStaticInteger::anonymous)
            .or_else(|| {
                resolve_runtime_resolved_static_integer(
                    input,
                    resolve_runtime_alias_expression(
                        expression,
                        source_key,
                        aliases,
                        alias_expressions,
                    ),
                    static_values,
                )
            }),
        Expression::Indexed(_) | Expression::Member(_) | Expression::Borrow(_) => {
            resolve_runtime_resolved_static_integer(
                input,
                resolve_runtime_alias_expression(
                    expression,
                    source_key,
                    aliases,
                    alias_expressions,
                ),
                static_values,
            )
        }
        Expression::Range(_) => None,
        Expression::Unary(unary) => {
            let value = resolve_runtime_static_integer(
                input,
                source_key,
                &unary.operand,
                aliases,
                alias_expressions,
                static_values,
            )?;
            Some(RuntimeStaticInteger::anonymous(i64::from(
                value.bits() == 0,
            )))
        }
        Expression::Boolean(value) => Some(RuntimeStaticInteger::anonymous(i64::from(*value))),
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Call(_)
        | Expression::Cast(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_)
        | Expression::ZeroValue(_) => None,
    }
}

pub(super) fn resolve_runtime_static_integer_value_in_table(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
) -> Option<i64> {
    resolve_runtime_static_integer_in_table(input, expressions, expression, static_values)
        .map(RuntimeStaticInteger::bits)
}

pub(super) fn resolve_runtime_static_integer_in_table(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
) -> Option<RuntimeStaticInteger> {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(_) => None,
        ExpressionNode::Integer(value) => RuntimeStaticInteger::from_literal(value),
        ExpressionNode::Boolean(value) => Some(RuntimeStaticInteger::anonymous(i64::from(*value))),
        ExpressionNode::Name(_) => {
            enum_variant_value_in_table(&input.layouts, expressions, expression)
                .map(RuntimeStaticInteger::anonymous)
                .or_else(|| {
                    let key = PlaceKey::from_expression_handle(expressions, expression)?;
                    static_values.get(&key)
                })
        }
        ExpressionNode::Member(member) => {
            // `<string-literal>.len`: a string literal flowing into a `&[u8] in
            // Utf8` parameter (the encoding-domain text model, #66) is inlined to
            // its literal at the value-call splice, so `text.len` reaches here as
            // `"hello".len`. The literal's `&[u8]` view has exactly its UTF-8
            // BYTE length, a compile-time constant -- fold it. Without this the
            // member has no storage place (a literal has no descriptor slot) and
            // the result-slot write silently drops (the call returns a stale 0).
            if member.member.as_str() == "len"
                && let Some(literal) = expressions.string_literal_value(member.receiver)
            {
                return i64::try_from(literal.len())
                    .ok()
                    .map(RuntimeStaticInteger::anonymous);
            }
            let key = PlaceKey::from_expression_handle(expressions, expression)?;
            static_values.get(&key)
        }
        ExpressionNode::Indexed(_) | ExpressionNode::Borrow(_) => {
            let key = PlaceKey::from_expression_handle(expressions, expression)?;
            static_values.get(&key)
        }
        ExpressionNode::Unary(unary) => {
            let value = resolve_runtime_static_integer_in_table(
                input,
                expressions,
                unary.operand,
                static_values,
            )?;
            Some(RuntimeStaticInteger::anonymous(i64::from(
                value.bits() == 0,
            )))
        }
        ExpressionNode::Range(_) => None,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

pub(super) fn resolve_runtime_static_integer_landing_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
) -> Option<IntegerLanding> {
    match expressions.expression(expression) {
        ExpressionNode::Integer(literal) => literal.landing(),
        ExpressionNode::Binary(binary) => {
            resolve_runtime_static_integer_landing_in_table(expressions, binary.left, static_values)
                .or_else(|| {
                    resolve_runtime_static_integer_landing_in_table(
                        expressions,
                        binary.right,
                        static_values,
                    )
                })
        }
        ExpressionNode::Borrow(inner) => resolve_runtime_static_integer_landing_in_table(
            expressions,
            inner.target,
            static_values,
        ),
        _ => {
            let key = PlaceKey::from_expression_handle(expressions, expression)?;
            static_values.get(&key)?.landing()
        }
    }
}

/// Resolve a compile-time-constant floating-point value (a float literal).
/// Returned as an `f64`; the caller narrows to the target width (`f32`/`f64`)
/// when computing the stored bit pattern.
pub(super) fn resolve_runtime_static_float_value_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<f64> {
    match expressions.expression(expression) {
        ExpressionNode::Float(literal) => Some(literal.landed_f64()),
        _ => None,
    }
}

fn resolve_runtime_resolved_static_integer(
    input: &InstructionSelectionInput<'_>,
    expression: Expression,
    static_values: &RuntimeStaticValues,
) -> Option<RuntimeStaticInteger> {
    let expression = strip_mutable_expression(expression);
    match expression {
        Expression::Atomic(_) => None,
        // Full 8-byte pattern: the literal-width gate guarantees an oversize
        // literal only reaches u64-classed (8-byte) targets, where these bits
        // ARE the value.
        Expression::Integer(value) => RuntimeStaticInteger::from_literal(&value),
        Expression::Boolean(value) => Some(RuntimeStaticInteger::anonymous(i64::from(value))),
        Expression::Name(_) | Expression::Indexed(_) | Expression::Member(_) => {
            enum_variant_value(&input.layouts, &expression)
                .map(RuntimeStaticInteger::anonymous)
                .or_else(|| {
                    let key = PlaceKey::from_expression(&expression)?;
                    static_values.get(&key)
                })
        }
        Expression::Unary(unary) => {
            let value =
                resolve_runtime_resolved_static_integer(input, unary.operand, static_values)?;
            Some(RuntimeStaticInteger::anonymous(i64::from(
                value.bits() == 0,
            )))
        }
        Expression::Range(_) => None,
        Expression::Borrow(_)
        | Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Call(_)
        | Expression::Cast(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_)
        | Expression::ZeroValue(_) => None,
    }
}

/// If writing `target` touches an element of an array through a NON-constant
/// index -- `arr[i]` OR `arr[i].field` (a field of an indexed element) OR a
/// deeper path over it -- return that array's `PlaceKey`. A runtime-indexed write
/// can land on any element, so every recorded constant for the whole array is
/// stale and must be voided. A CONSTANT-index write (`arr[2]`, `arr[2].field`) is
/// keyable precisely and handled by the normal single-place set/invalidate
/// (returns `None`).
fn runtime_indexed_write_collection(target: &Expression) -> Option<PlaceKey> {
    match target {
        Expression::Borrow(inner) => runtime_indexed_write_collection(&inner.target),
        Expression::Indexed(indexed) => {
            if matches!(indexed.index, Expression::Integer(_)) {
                return None;
            }
            // The collection may not be a plain place (`grid[i][j]` -- its
            // collection is `grid[i]`; `grid[1][j]` -- `grid[1]`): descend to
            // the DEEPEST resolvable place prefix, which is what the write
            // invalidates. Returning None here left the stale fold LIVE -- a
            // later const read of any element folded to its pre-write value.
            nested_place_key(&indexed.collection)
        }
        // A field (or deeper) of an indexed element keeps the index in the
        // RECEIVER: `arr[i].field` is `Member(Indexed(arr[i]), field)`. Walk in.
        Expression::Member(member) => runtime_indexed_write_collection(&member.receiver),
        _ => None,
    }
}

/// The deepest INVALIDATION-SAFE prefix of a possibly-indexed place chain: the
/// longest leading run with NO runtime-indexed component. `PlaceKey`
/// stringifies a runtime index into a synthetic member (`grid[i]` ->
/// `["self","grid","[self.i]"]`), which never prefixes the CONST keys the
/// tracker records (`["self","grid","[1]","[2]"]`) -- so a prefix taken at or
/// above a runtime level silently voids NOTHING. Stop BELOW the outermost
/// runtime-indexed node instead: `grid[i]` -> `grid` (voids every element);
/// `grid[1]` -> `grid[1]` (precise); `rows[i].data` -> `rows`.
fn nested_place_key(expression: &Expression) -> Option<PlaceKey> {
    match expression {
        Expression::Borrow(inner) => nested_place_key(&inner.target),
        Expression::Indexed(indexed) => {
            if !matches!(indexed.index, Expression::Integer(_))
                || place_chain_has_runtime_index(&indexed.collection)
            {
                return nested_place_key(&indexed.collection);
            }
            PlaceKey::from_expression(expression).or_else(|| nested_place_key(&indexed.collection))
        }
        Expression::Member(member) => {
            if place_chain_has_runtime_index(&member.receiver) {
                return nested_place_key(&member.receiver);
            }
            PlaceKey::from_expression(expression).or_else(|| nested_place_key(&member.receiver))
        }
        _ => PlaceKey::from_expression(expression),
    }
}

fn place_chain_has_runtime_index(expression: &Expression) -> bool {
    match expression {
        Expression::Borrow(inner) => place_chain_has_runtime_index(&inner.target),
        Expression::Indexed(indexed) => {
            !matches!(indexed.index, Expression::Integer(_))
                || place_chain_has_runtime_index(&indexed.collection)
        }
        Expression::Member(member) => place_chain_has_runtime_index(&member.receiver),
        _ => false,
    }
}

/// Handle-table variant of [`runtime_indexed_write_collection`].
fn runtime_indexed_write_collection_in_table(
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> Option<PlaceKey> {
    match expressions.expression(target) {
        ExpressionNode::Borrow(inner) => {
            runtime_indexed_write_collection_in_table(expressions, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            if matches!(
                expressions.expression(indexed.index),
                ExpressionNode::Integer(_)
            ) {
                return None;
            }
            // See `nested_place_key` -- descend to the deepest resolvable
            // place prefix so nested collections still void their folds.
            nested_place_key_in_table(expressions, indexed.collection)
        }
        ExpressionNode::Member(member) => {
            runtime_indexed_write_collection_in_table(expressions, member.receiver)
        }
        _ => None,
    }
}

/// Handle-table variant of [`nested_place_key`].
fn nested_place_key_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<PlaceKey> {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => nested_place_key_in_table(expressions, inner.target),
        ExpressionNode::Indexed(indexed) => {
            if !matches!(
                expressions.expression(indexed.index),
                ExpressionNode::Integer(_)
            ) || place_chain_has_runtime_index_in_table(expressions, indexed.collection)
            {
                return nested_place_key_in_table(expressions, indexed.collection);
            }
            PlaceKey::from_expression_handle(expressions, expression)
                .or_else(|| nested_place_key_in_table(expressions, indexed.collection))
        }
        ExpressionNode::Member(member) => {
            if place_chain_has_runtime_index_in_table(expressions, member.receiver) {
                return nested_place_key_in_table(expressions, member.receiver);
            }
            PlaceKey::from_expression_handle(expressions, expression)
                .or_else(|| nested_place_key_in_table(expressions, member.receiver))
        }
        _ => PlaceKey::from_expression_handle(expressions, expression),
    }
}

fn place_chain_has_runtime_index_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            place_chain_has_runtime_index_in_table(expressions, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            !matches!(
                expressions.expression(indexed.index),
                ExpressionNode::Integer(_)
            ) || place_chain_has_runtime_index_in_table(expressions, indexed.collection)
        }
        ExpressionNode::Member(member) => {
            place_chain_has_runtime_index_in_table(expressions, member.receiver)
        }
        _ => false,
    }
}

pub(super) fn set_runtime_static_value(
    static_values: &mut RuntimeStaticValues,
    target: Expression,
    value: RuntimeStaticInteger,
) {
    // A runtime-indexed write records no precise constant; it instead voids the
    // whole collection (it changed an unknown element).
    if let Some(collection) = runtime_indexed_write_collection(&target) {
        static_values.invalidate_prefix(&collection);
        return;
    }

    let Some(target) = PlaceKey::from_expression(&strip_mutable_expression(target)) else {
        return;
    };

    static_values.set(target, value);
}

pub(super) fn set_runtime_static_value_in_table(
    static_values: &mut RuntimeStaticValues,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: RuntimeStaticInteger,
) {
    if let Some(collection) = runtime_indexed_write_collection_in_table(expressions, target) {
        static_values.invalidate_prefix(&collection);
        return;
    }

    let Some(target) = PlaceKey::from_expression_handle(expressions, target) else {
        return;
    };

    static_values.set(target, value);
}

/// Drop any recorded constant for the place written by `target`. Call this after
/// emitting a write whose value is not a tracked compile-time constant so later
/// reads of the same place resolve against live storage instead of a stale fold.
pub(super) fn invalidate_runtime_static_value_in_table(
    static_values: &mut RuntimeStaticValues,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) {
    if let Some(collection) = runtime_indexed_write_collection_in_table(expressions, target) {
        static_values.invalidate_prefix(&collection);
        return;
    }

    let Some(target) = PlaceKey::from_expression_handle(expressions, target) else {
        return;
    };

    // A whole-place write invalidates constants for every descendant field;
    // for scalar/member targets this is equivalent to exact invalidation.
    // Aggregate reconstruction relies on the stronger form so omitted fields
    // cannot retain stale compile-time facts after the target is zero-filled.
    static_values.invalidate_prefix(&target);
}

/// If `target` is a runtime-indexed write `arr[i]` (non-constant index), void
/// every folded constant for the whole collection `arr` -- the write can land on
/// any element, so each sibling `arr[k]` constant is now stale. No-op for
/// non-indexed and const-indexed targets (those keep precise per-place tracking
/// via set/invalidate). Safe to call up front, before the value is resolved: at
/// runtime the RHS read precedes the write, so a same-array read still sees the
/// pre-write value from live storage. Calling it once at the mutation-write
/// entry covers every indexed sub-path (frame/machine copy, indexed integer).
pub(super) fn invalidate_runtime_static_collection_for_indexed_write(
    static_values: &mut RuntimeStaticValues,
    target: &Expression,
) {
    if let Some(collection) = runtime_indexed_write_collection(target) {
        static_values.invalidate_prefix(&collection);
    }
}

#[cfg(test)]
mod tests {
    use psi_numerics::arithmetic::ArithmeticDomain;
    use psi_numerics::literals::{IntegerLanding, LandedIntegerType};

    use super::{RuntimeStaticInteger, RuntimeStaticValues};
    use omega_platform_interface::PlaceKey;

    #[test]
    fn static_value_round_trip_preserves_integer_landing() {
        let landing = IntegerLanding {
            landed_type: LandedIntegerType::U32,
            domain: ArithmeticDomain::Wrapping,
        };
        let value = RuntimeStaticInteger {
            bits: u32::MAX.into(),
            landing: Some(landing),
        };
        let target = PlaceKey::default();
        let mut static_values = RuntimeStaticValues::new();

        static_values.set(target.clone(), value);

        assert_eq!(static_values.get(&target), Some(value));
        assert_eq!(
            static_values.get(&target).and_then(|value| value.landing()),
            Some(landing)
        );
    }
}
