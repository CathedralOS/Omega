use std::collections::HashMap;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::PrimitiveType;

/// One whole-program resolution index behind the bound-leaf lookups in this
/// module. Bound evidence re-resolves the same immutable locals and state
/// parameters once per leaf expression; a caller decomposing many bound
/// expressions builds the index once and reuses it instead of rescanning
/// every machine's statements and parameter lists per leaf. `None` entries
/// preserve the scans' ambiguity verdicts: a name or symbol owned by more
/// than one declaration has no bound identity.
pub struct ImmutableBoundLookup<'program> {
    locals_by_symbol: HashMap<SymbolHandle, Option<&'program TableLocalData>>,
    locals_by_name: HashMap<&'program str, Option<&'program TableLocalData>>,
    parameters_by_symbol: HashMap<SymbolHandle, Option<&'program StateParameter>>,
    machine_self_parameters: HashMap<SymbolHandle, &'program StateParameter>,
}

impl<'program> ImmutableBoundLookup<'program> {
    pub fn new(program: &'program TypedTrees) -> Self {
        let mut lookup = ImmutableBoundLookup {
            locals_by_symbol: HashMap::new(),
            locals_by_name: HashMap::new(),
            parameters_by_symbol: HashMap::new(),
            machine_self_parameters: HashMap::new(),
        };
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for parameter in program.state_parameters(state) {
                    lookup
                        .parameters_by_symbol
                        .entry(parameter.symbol)
                        .and_modify(|entry| *entry = None)
                        .or_insert(Some(parameter));
                    if parameter.is_self {
                        lookup
                            .machine_self_parameters
                            .entry(machine.symbol)
                            .or_insert(parameter);
                    }
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let StatementNode::LocalData(local) = statement else {
                        continue;
                    };
                    lookup
                        .locals_by_symbol
                        .entry(local.symbol)
                        .and_modify(|entry| *entry = None)
                        .or_insert(Some(local));
                    lookup
                        .locals_by_name
                        .entry(local.name.as_str())
                        .and_modify(|entry| *entry = None)
                        .or_insert(Some(local));
                }
            }
        }
        lookup
    }
}

/// One immutable integer boundary shifted by a compile-time constant: the
/// mathematical value `symbol + offset`, valid only under Exact arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImmutableIntegerBoundOffset {
    pub symbol: SymbolHandle,
    pub offset: i64,
}

/// One immutable integer boundary formed from two distinct immutable
/// bindings and an exact constant: the mathematical value
/// `first + second + offset`, valid only under Exact arithmetic on every
/// term.
///
/// `first` canonically precedes `second` in arena order so `a + b` and
/// `b + a` share one stored spelling. Every term carries coefficient one:
/// `x + x`, a signed term such as `a - b`, and a third distinct symbol all
/// have no spelling here and stay unknown rather than approximating a
/// coefficient or term the vocabulary cannot express.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImmutableIntegerBoundSum {
    pub first: SymbolHandle,
    pub second: SymbolHandle,
    pub offset: i64,
}

/// Immutable integer boundary normalization preserves either an exact literal,
/// one immutable binding's identity, or one resolved immutable symbol leaf.
///
/// A constant offset requires Exact arithmetic on the immediate operand because
/// exact-domain `+`/`-` is a proof obligation discharged before checking
/// (decision 17, `arithmetic_domains.rs`); with the selector already passing
/// slice-range validation, its runtime value is the mathematical
/// `symbol + offset`. Wrapping, Saturating, and Trapping operands may wrap or
/// clamp and therefore remain unknown.
#[cfg(test)]
mod tests;

enum LocalLookup<'program> {
    Missing,
    Found(&'program TableLocalData),
    Invalid,
}

enum NormalizedBound {
    Expression(ExpressionHandle),
    LocalValue(SymbolHandle),
    MutableValue,
}

/// Normalize an integer-bound expression through a finite chain of immutable
/// local copies.
///
/// The terminal leaf is either the original integer literal or one exact
/// symbol-backed bare name, such as a machine/state parameter. Mutable,
/// ambiguous, cyclic, qualified, and computed aliases remain unknown. A bare
/// local name without a retained symbol is accepted only when it names one
/// unique immutable local in the complete typed program.
pub fn normalize_immutable_integer_bound_expression(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match normalize_bound(program, lookup, expression, &mut Vec::new())? {
        NormalizedBound::Expression(expression) => Some(expression),
        NormalizedBound::LocalValue(_) | NormalizedBound::MutableValue => None,
    }
}

/// Retain the identity of one immutable integer binding whose initializer is
/// computed or reads a mutable value, including through immutable copies.
/// This establishes shared value identity, not the initializer's value or
/// equivalence to a separately evaluated computation.
/// Static-index callers continue to use the expression/usize normalizers.
pub fn immutable_integer_bound_value_symbol(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match normalize_bound(program, lookup, expression, &mut Vec::new())? {
        NormalizedBound::LocalValue(symbol) => Some(symbol),
        NormalizedBound::Expression(_) | NormalizedBound::MutableValue => None,
    }
}

pub fn immutable_integer_bound_symbol_offset(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<ImmutableIntegerBoundOffset> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let (base, offset) = match binary.operator {
        typed_trees::expression::BinaryOperator::Add => {
            let left_value = program.expression_table.constant_integer_value(binary.left);
            let right_value = program
                .expression_table
                .constant_integer_value(binary.right);
            match (left_value, right_value) {
                (Some(k), None) => (binary.right, k),
                (None, Some(k)) => (binary.left, k),
                _ => return None,
            }
        }
        typed_trees::expression::BinaryOperator::Subtract => {
            let k = program
                .expression_table
                .constant_integer_value(binary.right)?;
            if program
                .expression_table
                .constant_integer_value(binary.left)
                .is_some()
            {
                return None;
            }
            (binary.left, k.checked_neg()?)
        }
        _ => return None,
    };
    let symbol = bound_leaf_symbol(program, lookup, base)?;
    Some(ImmutableIntegerBoundOffset { symbol, offset })
}

/// Normalize an `Add`/`Subtract` tree of immutable integer bindings into the
/// two-symbol bound `first + second + offset`.
///
/// Constant subtrees fold into `offset`; each remaining leaf must clear the
/// same immutable, single-segment, exact-domain gate a bare `symbol + const`
/// base does. Subtraction of a symbolic term, a third distinct symbol, and a
/// repeated symbol all stay unknown rather than approximating a coefficient
/// or term the vocabulary cannot express.
pub fn immutable_integer_bound_sum(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<ImmutableIntegerBoundSum> {
    let mut terms = BoundSumTerms::default();
    collect_bound_terms(program, lookup, expression, &mut terms, 0)?;
    let (Some(mut first), Some(mut second)) = (terms.first, terms.second) else {
        return None;
    };
    if (first.arena_index(), first.generation()) > (second.arena_index(), second.generation()) {
        std::mem::swap(&mut first, &mut second);
    }
    Some(ImmutableIntegerBoundSum {
        first,
        second,
        offset: terms.offset,
    })
}

#[derive(Default)]
struct BoundSumTerms {
    first: Option<SymbolHandle>,
    second: Option<SymbolHandle>,
    offset: i64,
}

fn collect_bound_terms(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
    terms: &mut BoundSumTerms,
    depth: usize,
) -> Option<()> {
    if !expression.is_valid() || depth >= 128 {
        return None;
    }
    // A pure constant subtree folds into the offset before any symbolic
    // reading; `i - (1 + 2)` is `i - 3` even though its right child is a
    // `Subtract` node.
    if let Some(value) = program.expression_table.constant_integer_value(expression) {
        terms.offset = terms.offset.checked_add(value)?;
        return Some(());
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => match binary.operator {
            typed_trees::expression::BinaryOperator::Add => {
                collect_bound_terms(program, lookup, binary.left, terms, depth + 1)?;
                collect_bound_terms(program, lookup, binary.right, terms, depth + 1)
            }
            typed_trees::expression::BinaryOperator::Subtract => {
                collect_bound_terms(program, lookup, binary.left, terms, depth + 1)?;
                // Only a constant right side folds; `a - b` would carry a
                // negative coefficient on `b`, which has no spelling.
                let value = program
                    .expression_table
                    .constant_integer_value(binary.right)?;
                terms.offset = terms.offset.checked_sub(value)?;
                Some(())
            }
            _ => None,
        },
        _ => {
            let symbol = bound_leaf_symbol(program, lookup, expression)?;
            if let Some(initializer) = bound_leaf_initializer(program, lookup, expression) {
                // An immutable local bound to a bound-shaped initializer
                // contributes that initializer's terms: the local stores the
                // expression's exact value, never a retargetable alias.
                return collect_bound_terms(program, lookup, initializer, terms, depth + 1);
            }
            if terms.first == Some(symbol) || terms.second == Some(symbol) {
                // `x + x` is `2x`; coefficient-one terms cannot express it.
                return None;
            }
            if terms.first.is_none() {
                terms.first = Some(symbol);
            } else if terms.second.is_none() {
                terms.second = Some(symbol);
            } else {
                return None;
            }
            Some(())
        }
    }
}

/// When one bound leaf is an immutable local whose initializer is itself a
/// bound-shaped `Add`/`Subtract` tree, the leaf's terms come from that
/// initializer -- generated hoist locals are exactly this shape. A mutable
/// local, a non-bound initializer, or a bare parameter name contributes the
/// leaf's own symbol instead.
fn bound_leaf_initializer(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid() || path.head_symbol != path.symbol {
        return None;
    }
    let LocalLookup::Found(local) = local_by_symbol(lookup, path.symbol) else {
        return None;
    };
    if local.is_mutable || !local.initial_value.is_valid() {
        return None;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    matches!(
        binary.operator,
        typed_trees::expression::BinaryOperator::Add
            | typed_trees::expression::BinaryOperator::Subtract
    )
    .then_some(local.initial_value)
}

/// Resolve one bound leaf to its immutable symbol identity: a bare
/// single-segment name whose declared type is an exact-domain integer
/// primitive, followed through finite immutable local copies. Mutable,
/// ambiguous, qualified, and non-integer leaves have no bound identity.
fn bound_leaf_symbol(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    base: ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(base) else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    if members.len() != 1 {
        return None;
    }

    let type_reference = if path.symbol.is_valid() && path.head_symbol == path.symbol {
        match local_by_symbol(lookup, path.symbol) {
            LocalLookup::Found(local) => local.type_reference,
            LocalLookup::Missing => parameter_by_symbol(lookup, path.symbol)?.type_reference,
            LocalLookup::Invalid => return None,
        }
    } else {
        match local_by_name(lookup, members[0].as_str()) {
            LocalLookup::Found(local) => local.type_reference,
            LocalLookup::Missing | LocalLookup::Invalid => return None,
        }
    };
    let primitive = program
        .type_reference_table
        .primitive_type(type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || program.arithmetic_domain_for_type_reference(type_reference)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }

    match normalize_bound(program, lookup, base, &mut Vec::new())? {
        NormalizedBound::LocalValue(symbol) => Some(symbol),
        NormalizedBound::Expression(expression) => {
            let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
                return None;
            };
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 1 && path.symbol.is_valid() && path.head_symbol == path.symbol)
                .then_some(path.symbol)
        }
        NormalizedBound::MutableValue => None,
    }
}

/// Resolve one bare single-segment name to the symbol of the mutable storage
/// it names: one unique mutable local or mutable state parameter whose
/// declared type is an exact-domain integer primitive.
///
/// The symbol names live storage, not an immutable identity: the bound it
/// contributes is "the value currently stored under this symbol". Consumers
/// may use it as a query coordinate, but stated evidence can only claim it
/// while version evidence pins the storage to the same occurrence the claim
/// established. Immutable bindings continue through the immutable readers.
pub fn mutable_integer_bound_storage_symbol(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    if members.len() != 1 {
        return None;
    }
    let (symbol, is_mutable, type_reference) =
        if path.symbol.is_valid() && path.head_symbol == path.symbol {
            bound_name_receiver(lookup, path)?
        } else {
            match local_by_name(lookup, members[0].as_str()) {
                LocalLookup::Found(local) => (local.symbol, local.is_mutable, local.type_reference),
                LocalLookup::Missing | LocalLookup::Invalid => return None,
            }
        };
    if !is_mutable || !symbol.is_valid() {
        return None;
    }
    let primitive = program
        .type_reference_table
        .primitive_type(type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || program.arithmetic_domain_for_type_reference(type_reference)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    Some(symbol)
}

/// Resolve one single-segment receiver-rooted field projection —
/// `pair.first` or `param.first` — to the receiver's storage symbol, the
/// exact declared field symbol, and the receiver's mutability. The receiver
/// must be one bare local or state/machine parameter name and the field's
/// declared type an exact-domain integer primitive; projections through
/// receivers that are not bare names, through case variants, into
/// multi-level member paths, or into non-integer fields stay unboundable.
///
/// The receiver symbol's meaning still belongs to the consumer: an
/// immutable receiver contributes a frozen projection identity, a mutable
/// receiver the value currently stored under the projected coordinate,
/// which stated evidence can only claim under the version-pin evidence the
/// storage vocabulary already requires.
fn projected_integer_bound_field<'program>(
    program: &'program TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, &'program typed_trees::data::DataField, bool)> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.case_variant.is_some() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver) else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    if members.len() != 1 {
        return None;
    }
    let (symbol, is_mutable, type_reference) =
        if path.symbol.is_valid() && path.head_symbol == path.symbol {
            bound_name_receiver(lookup, path)?
        } else {
            match local_by_name(lookup, members[0].as_str()) {
                LocalLookup::Found(local) => (local.symbol, local.is_mutable, local.type_reference),
                LocalLookup::Missing | LocalLookup::Invalid => return None,
            }
        };
    if !symbol.is_valid() {
        return None;
    }
    let receiver = crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
    let typed_trees::types::TypeReferenceNode::Named {
        symbol: type_symbol,
        ..
    } = program.type_reference_table.type_reference(receiver)
    else {
        return None;
    };
    if !type_symbol.is_valid() {
        return None;
    }
    // A `self` receiver names the machine; its members live on the attached
    // data definition, the same resolution `effective_member_symbol` uses.
    let (data, authored_matches_field) = match program.symbols.get(*type_symbol).kind {
        symbols::SymbolKind::Data => (
            program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *type_symbol)?,
            true,
        ),
        symbols::SymbolKind::Machine => {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == *type_symbol)?;
            let attached = machine.attached_data.as_deref()?;
            // Attached members resolve by name: the authored member symbol
            // is interned against the machine's spelling, not the field's.
            (
                program
                    .data_definitions()
                    .iter()
                    .find(|data| data.name.as_str() == attached)?,
                false,
            )
        }
        _ => return None,
    };
    let field = crate::value_custody::places::exact_data_member_field(
        program,
        data,
        if authored_matches_field {
            member.member_symbol
        } else {
            SymbolHandle::invalid()
        },
        member.member.as_str(),
        None,
    )?;
    Some((symbol, field, is_mutable))
}

pub fn projected_integer_bound_root(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, SymbolHandle, bool)> {
    let (symbol, field, is_mutable) = projected_integer_bound_field(program, lookup, expression)?;
    let primitive = program
        .type_reference_table
        .primitive_type(field.type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || program.arithmetic_domain_for_type_reference(field.type_reference)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    Some((symbol, field.symbol, is_mutable))
}

/// The root, resolved place path, and mutability of one indexed
/// integer-bound expression (`items[2]`, `self.pivot[2]`): the root's
/// resolved symbol, the ordered segments reaching the fixed element, and
/// whether the root storage is mutable. Runtime index expressions,
/// case projections, and deeper-than-one member collections stay outside
/// the bound vocabulary.
pub fn indexed_integer_bound_root(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>, bool)> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let (symbol, mut segments, is_mutable, type_reference) =
        match program.expression_table.expression(indexed.collection) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                if members.len() != 1 {
                    return None;
                }
                let (symbol, is_mutable, type_reference) =
                    if path.symbol.is_valid() && path.head_symbol == path.symbol {
                        bound_name_receiver(lookup, path)?
                    } else {
                        match local_by_name(lookup, members[0].as_str()) {
                            LocalLookup::Found(local) => {
                                (local.symbol, local.is_mutable, local.type_reference)
                            }
                            LocalLookup::Missing | LocalLookup::Invalid => return None,
                        }
                    };
                (symbol, Vec::new(), is_mutable, type_reference)
            }
            // A member collection (`self.pivot`, `pair.items`) roots the bound
            // at the receiver's symbol; the member field is the path's first
            // resolved segment.
            ExpressionNode::Member(_) => {
                let (symbol, field, is_mutable) =
                    projected_integer_bound_field(program, lookup, indexed.collection)?;
                (
                    symbol,
                    vec![facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    }],
                    is_mutable,
                    field.type_reference,
                )
            }
            _ => return None,
        };
    if !symbol.is_valid() {
        return None;
    }
    let index = program
        .expression_table
        .constant_integer_value(indexed.index)
        .and_then(|value| usize::try_from(value).ok())?;
    segments.push(facts::PlaceSegment::FixedIndex { index });
    let receiver = crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
    let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
        program.type_reference_table.type_reference(receiver)
    else {
        return None;
    };
    let primitive = program.type_reference_table.primitive_type(*element_type)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || program.arithmetic_domain_for_type_reference(*element_type)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    Some((symbol, segments, is_mutable))
}

/// The declared type of a projected integer-bound subject: the member field's
/// own reference, for consumers comparing a member subject's carrier against a
/// declared domain or contract type.
pub fn projected_integer_bound_subject_type(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    projected_integer_bound_field(program, lookup, expression).map(|(_, field, _)| field.type_reference)
}

/// Normalize an integer literal or finite immutable local-copy chain to one
/// exact host index. Symbolic parameter leaves and every unsupported alias
/// shape remain unknown.
pub fn normalize_immutable_integer_bound_to_usize(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    let expression = normalize_immutable_integer_bound_expression(program, lookup, expression)?;
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
}

fn normalize_bound(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
    seen_aliases: &mut Vec<SymbolHandle>,
) -> Option<NormalizedBound> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(NormalizedBound::Expression(expression)),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.len() != 1 {
                return None;
            }
            if path.symbol.is_valid() && path.head_symbol == path.symbol {
                match local_by_symbol(lookup, path.symbol) {
                    LocalLookup::Missing => {
                        if parameter_mutability(lookup, path.symbol)? {
                            Some(NormalizedBound::MutableValue)
                        } else {
                            Some(NormalizedBound::Expression(expression))
                        }
                    }
                    LocalLookup::Found(local) => normalize_local(program, lookup, local, seen_aliases),
                    LocalLookup::Invalid => None,
                }
            } else {
                match local_by_name(lookup, members[0].as_str()) {
                    LocalLookup::Found(local) => normalize_local(program, lookup, local, seen_aliases),
                    LocalLookup::Missing | LocalLookup::Invalid => None,
                }
            }
        }
        _ => None,
    }
}

fn normalize_local(
    program: &TypedTrees,
    lookup: &ImmutableBoundLookup<'_>,
    local: &TableLocalData,
    seen_aliases: &mut Vec<SymbolHandle>,
) -> Option<NormalizedBound> {
    if !local.symbol.is_valid() || seen_aliases.contains(&local.symbol) {
        return None;
    }
    if local.is_mutable {
        return Some(NormalizedBound::MutableValue);
    }
    seen_aliases.push(local.symbol);
    let normalized = match program.expression_table.expression(local.initial_value) {
        ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
            normalize_bound(program, lookup, local.initial_value, seen_aliases).map(|bound| match bound {
                // The local stores a value, not a retargetable alias to its source.
                NormalizedBound::MutableValue => NormalizedBound::LocalValue(local.symbol),
                bound => bound,
            })
        }
        _ if local.initial_value.is_valid() => Some(NormalizedBound::LocalValue(local.symbol)),
        _ => None,
    };
    seen_aliases.pop();
    normalized
}

/// The mutability and declared type of one resolved bound name: a unique
/// local, a state parameter, or — when the name resolves to the machine
/// itself — the machine's `self` receiver parameter.
fn bound_name_receiver(
    lookup: &ImmutableBoundLookup<'_>,
    path: &typed_trees::expression::TableNamePath,
) -> Option<(SymbolHandle, bool, typed_trees::types::TypeReferenceHandle)> {
    match local_by_symbol(lookup, path.symbol) {
        LocalLookup::Found(local) => Some((local.symbol, local.is_mutable, local.type_reference)),
        LocalLookup::Missing => {
            if let Some(parameter) = parameter_by_symbol(lookup, path.symbol) {
                Some((path.symbol, parameter.is_mutable, parameter.type_reference))
            } else {
                let parameter = lookup.machine_self_parameters.get(&path.symbol)?;
                Some((path.symbol, parameter.is_mutable, parameter.type_reference))
            }
        }
        LocalLookup::Invalid => None,
    }
}

fn local_by_symbol<'program>(
    lookup: &ImmutableBoundLookup<'program>,
    symbol: SymbolHandle,
) -> LocalLookup<'program> {
    match lookup.locals_by_symbol.get(&symbol) {
        Some(Some(local)) => LocalLookup::Found(local),
        Some(None) => LocalLookup::Invalid,
        None => LocalLookup::Missing,
    }
}

fn local_by_name<'program>(
    lookup: &ImmutableBoundLookup<'program>,
    name: &str,
) -> LocalLookup<'program> {
    match lookup.locals_by_name.get(name) {
        Some(Some(local)) => LocalLookup::Found(local),
        Some(None) => LocalLookup::Invalid,
        None => LocalLookup::Missing,
    }
}

fn parameter_mutability(lookup: &ImmutableBoundLookup<'_>, symbol: SymbolHandle) -> Option<bool> {
    match lookup.parameters_by_symbol.get(&symbol) {
        Some(Some(parameter)) => Some(parameter.is_mutable),
        // An ambiguous parameter symbol has no bound identity; a symbol that
        // never declared a parameter cannot be mutable.
        Some(None) => None,
        None => Some(false),
    }
}

fn parameter_by_symbol<'program>(
    lookup: &ImmutableBoundLookup<'program>,
    symbol: SymbolHandle,
) -> Option<&'program StateParameter> {
    lookup.parameters_by_symbol.get(&symbol).copied().flatten()
}
