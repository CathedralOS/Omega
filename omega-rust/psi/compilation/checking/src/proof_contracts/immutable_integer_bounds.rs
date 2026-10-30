use symbols::SymbolHandle;
use symbols::SymbolKeyMap as HashMap;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::statement::{StatementHandle, StatementNode, TableLocalData};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// One whole-program resolution index behind the bound-leaf lookups in this
/// module. Bound evidence re-resolves the same immutable locals and state
/// parameters once per leaf expression; a caller decomposing many bound
/// expressions builds the index once and reuses it instead of rescanning
/// every machine's statements and parameter lists per leaf. `None` entries
/// preserve the scans' ambiguity verdicts: a name or symbol owned by more
/// than one declaration has no bound identity.
#[derive(Clone)]
pub struct ImmutableBoundLookup<'program> {
    program: &'program dyn crate::CheckingInput,
    /// Shared through clones: a cloned lookup reads the same maps, built at
    /// most once across the whole clone family, and once per frozen program
    /// across every lookup its checks construct.
    maps: std::sync::OnceLock<std::sync::Arc<BoundMaps>>,
}

/// The index owns statement handles and parameter positions rather than
/// borrows, so a frozen program's memo can hold one copy for all lookups.
pub(crate) struct BoundMaps {
    locals_by_symbol: HashMap<SymbolHandle, Option<StatementHandle>>,
    locals_by_name: HashMap<Box<str>, Option<StatementHandle>>,
    parameters_by_symbol: HashMap<SymbolHandle, Option<ParameterPosition>>,
    machine_self_parameters: HashMap<SymbolHandle, ParameterPosition>,
    /// Positions behind `projected_integer_bound_field`: a projected
    /// receiver's declared type resolves to the data definition, machine, or
    /// machine-attached data naming it. First matches win, as the scans did.
    data_by_symbol: HashMap<SymbolHandle, usize>,
    machines_by_symbol: HashMap<SymbolHandle, usize>,
    data_by_name: HashMap<Box<str>, usize>,
}

/// A state parameter by its machine's, its state's and its own position.
#[derive(Clone, Copy)]
struct ParameterPosition {
    machine: u32,
    state: u32,
    parameter: u32,
}

impl BoundMaps {
    fn build(program: &dyn crate::CheckingInput) -> Self {
        let mut maps = BoundMaps {
            locals_by_symbol: HashMap::default(),
            locals_by_name: HashMap::default(),
            parameters_by_symbol: HashMap::default(),
            machine_self_parameters: HashMap::default(),
            data_by_symbol: HashMap::default(),
            machines_by_symbol: HashMap::default(),
            data_by_name: HashMap::default(),
        };
        for (position, definition) in program.data_definitions().iter().enumerate() {
            maps.data_by_symbol
                .entry(definition.symbol)
                .or_insert(position);
            if !maps.data_by_name.contains_key(definition.name.as_str()) {
                maps.data_by_name
                    .insert(definition.name.as_str().into(), position);
            }
        }
        for (position, machine) in program.machines().iter().enumerate() {
            maps.machines_by_symbol
                .entry(machine.symbol)
                .or_insert(position);
        }
        for (machine_position, machine) in program.machines().iter().enumerate() {
            for (state_position, state) in program.machine_states(machine).iter().enumerate() {
                for (parameter_position, parameter) in
                    program.state_parameters(state).iter().enumerate()
                {
                    let position = ParameterPosition {
                        machine: machine_position as u32,
                        state: state_position as u32,
                        parameter: parameter_position as u32,
                    };
                    maps.parameters_by_symbol
                        .entry(parameter.symbol)
                        .and_modify(|entry| *entry = None)
                        .or_insert(Some(position));
                }
                for (handle, statement) in program
                    .statement_table
                    .iter_statements(state.statement_nodes)
                {
                    let Some(local) = statement.as_local_data() else {
                        continue;
                    };
                    maps.locals_by_symbol
                        .entry(local.symbol)
                        .and_modify(|entry| *entry = None)
                        .or_insert(Some(handle));
                    match maps.locals_by_name.get_mut(local.name.as_str()) {
                        Some(entry) => *entry = None,
                        None => {
                            maps.locals_by_name
                                .insert(local.name.as_str().into(), Some(handle));
                        }
                    }
                }
            }
        }
        maps
    }
}

impl<'program> ImmutableBoundLookup<'program> {
    /// Cheap: the whole-program index builds on first query, so a caller that
    /// exits before needing bound evidence never pays for it.
    pub fn new(program: &'program dyn crate::CheckingInput) -> Self {
        ImmutableBoundLookup {
            program,
            maps: std::sync::OnceLock::new(),
        }
    }

    fn maps(&self) -> &BoundMaps {
        self.maps.get_or_init(|| {
            crate::frozen_program::frozen_memo(
                self.program,
                |memos| &memos.bound_maps,
                || BoundMaps::build(self.program),
            )
        })
    }

    /// A frozen binding may supply an owned record observation. Reference
    /// access and every crossed projection remain separate consumer checks.
    pub fn immutable_binding_type(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
        match self.local_entry(symbol) {
            Some(Some(local)) if !local.is_mutable => Some(local.type_reference),
            Some(_) => None,
            None => match self.parameter_entry(symbol) {
                Some(Some(parameter)) if !parameter.is_mutable => Some(parameter.type_reference),
                Some(_) => None,
                None if self.program.symbols.get(symbol).kind == symbols::SymbolKind::Const => {
                    crate::declaration_type_reference(self.program, symbol)
                }
                None => None,
            },
        }
    }

    fn local_at(&self, handle: StatementHandle) -> &'program TableLocalData {
        let program: &'program dyn crate::CheckingInput = self.program;
        match program.statement_table.statement(handle) {
            StatementNode::LocalData(local) => local,
            _ => unreachable!("the bound index records local-data statements only"),
        }
    }

    fn parameter_at(&self, position: ParameterPosition) -> &'program StateParameter {
        let program: &'program dyn crate::CheckingInput = self.program;
        let machine = &program.machines()[position.machine as usize];
        let state = &program.machine_states(machine)[position.state as usize];
        &program.state_parameters(state)[position.parameter as usize]
    }

    fn local_entry(&self, symbol: SymbolHandle) -> Option<Option<&'program TableLocalData>> {
        let entry = *self.maps().locals_by_symbol.get(&symbol)?;
        Some(entry.map(|handle| self.local_at(handle)))
    }

    fn local_named(&self, name: &str) -> Option<Option<&'program TableLocalData>> {
        let entry = *self.maps().locals_by_name.get(name)?;
        Some(entry.map(|handle| self.local_at(handle)))
    }

    fn parameter_entry(&self, symbol: SymbolHandle) -> Option<Option<&'program StateParameter>> {
        let entry = *self.maps().parameters_by_symbol.get(&symbol)?;
        Some(entry.map(|position| self.parameter_at(position)))
    }

    fn machine_self_parameter(&self, symbol: SymbolHandle) -> Option<&'program StateParameter> {
        let position = *self.maps().machine_self_parameters.get(&symbol)?;
        Some(self.parameter_at(position))
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match normalize_bound(program, lookup, expression, &mut Vec::new())? {
        NormalizedBound::LocalValue(symbol) => Some(symbol),
        NormalizedBound::Expression(_) | NormalizedBound::MutableValue => None,
    }
}

/// Normalize the same additive terms as a two-symbol bound, retaining exactly
/// one immutable value identity and its constant shift. Reading a computed
/// local must not change the available bound vocabulary: its checked exact
/// value and finite immutable copies contribute the same terms as the direct
/// expression. Mutable leaves and unsupported coefficients remain unknown.
pub fn immutable_integer_bound_symbol_offset(
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<ImmutableIntegerBoundOffset> {
    let mut terms = BoundSumTerms::default();
    collect_bound_terms(program, lookup, expression, &mut terms, 0)?;
    let (Some(symbol), None) = (terms.first, terms.second) else {
        return None;
    };
    Some(ImmutableIntegerBoundOffset {
        symbol,
        offset: terms.offset,
    })
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
    program: &dyn crate::CheckingInput,
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

#[derive(Default, Clone)]
struct BoundSumTerms {
    first: Option<SymbolHandle>,
    second: Option<SymbolHandle>,
    offset: i64,
}

/// The type reference's primitive when it is an exact-domain integer
/// primitive, else `None`.
fn exact_integer_type_primitive(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    crate::value_custody::type_shapes::exact_integer_primitive(program, type_reference, false)
}

fn collect_bound_terms(
    program: &dyn crate::CheckingInput,
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
            if let Some(initializer) = bound_leaf_initializer(program, lookup, symbol) {
                // Algebraic expansion is optional knowledge about an existing
                // immutable value, not permission to replay its initializer.
                // If it reads mutable storage or exceeds this vocabulary,
                // retain the captured value as an opaque atom. Failed
                // reduction must not partially alter the enclosing terms.
                let mut expanded = terms.clone();
                if collect_bound_terms(program, lookup, initializer, &mut expanded, depth + 1)
                    .is_some()
                {
                    *terms = expanded;
                    return Some(());
                }
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

/// Resolve the computed initializer at the immutable value identity, after
/// finite copies have been followed. An alias's declaration may contain only
/// a name while the captured value came from an additive computation. Mutable
/// storage is never followed here: it is not an immutable bound leaf, and a
/// snapshot whose initializer reads it keeps its own occurrence identity.
fn bound_leaf_initializer(
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    symbol: SymbolHandle,
) -> Option<ExpressionHandle> {
    let LocalLookup::Found(local) = local_by_symbol(lookup, symbol) else {
        return None;
    };
    if local.is_mutable || local.ordinary_value_initializer().is_none() {
        return None;
    }
    let binary = program.expression_table.binary_node(local.initial_value)?;
    // Expanding a stored computation must retain the declaration that chose
    // its operator meaning. The local's structural owner supplies that exact
    // callable context; a `+` requirement selecting an authored operation is
    // not builtin addition, even when both operands are Exact integers.
    let state_symbol = program.symbols.get(local.symbol).parent;
    let (machine, state) = crate::find_state(program, state_symbol)?;
    if !crate::has_builtin_bound_expression_meaning(
        program,
        machine,
        Some(state),
        local.initial_value,
    ) {
        return None;
    }
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
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    base: ExpressionHandle,
) -> Option<SymbolHandle> {
    let path = program.expression_table.name_path_node(base)?;
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
    exact_integer_type_primitive(program, type_reference)?;

    match normalize_bound(program, lookup, base, &mut Vec::new())? {
        NormalizedBound::LocalValue(symbol) => Some(symbol),
        NormalizedBound::Expression(expression) => {
            let path = program.expression_table.name_path_node(expression)?;
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
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let path = program.expression_table.name_path_node(expression)?;
    let members = program.expression_table.name_path_members(path.members);
    let (symbol, is_mutable, type_reference) = single_segment_receiver(lookup, path, members)?;
    if !is_mutable || !symbol.is_valid() {
        return None;
    }
    exact_integer_type_primitive(program, type_reference)?;
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
    program: &'program dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, &'program typed_trees::data::DataField, bool)> {
    let member = program.expression_table.member_node(expression)?;
    if member.case_variant.is_some() {
        return None;
    }
    let path = program.expression_table.name_path_node(member.receiver)?;
    let members = program.expression_table.name_path_members(path.members);
    let (symbol, is_mutable, type_reference) = single_segment_receiver(lookup, path, members)?;
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
            lookup
                .maps()
                .data_by_symbol
                .get(type_symbol)
                .map(|position| &program.data_definitions()[*position])?,
            true,
        ),
        symbols::SymbolKind::Machine => {
            let machine = lookup
                .maps()
                .machines_by_symbol
                .get(type_symbol)
                .map(|position| &program.machines()[*position])?;
            let attached = machine.attached_data.as_deref()?;
            // Attached members resolve by name: the authored member symbol
            // is interned against the machine's spelling, not the field's.
            (
                lookup
                    .maps()
                    .data_by_name
                    .get(attached)
                    .map(|position| &program.data_definitions()[*position])?,
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
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, SymbolHandle, bool)> {
    let (symbol, field, is_mutable) = projected_integer_bound_field(program, lookup, expression)?;
    let primitive = program
        .type_reference_table
        .primitive_type(field.type_reference)?;
    if !primitive.is_fixed_integer()
        || program.arithmetic_domain_for_type_reference(field.type_reference)
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
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>, bool)> {
    let indexed = program.expression_table.indexed_node(expression)?;
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
    if !primitive.is_fixed_integer()
        || program.arithmetic_domain_for_type_reference(*element_type)
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
    program: &dyn crate::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    projected_integer_bound_field(program, lookup, expression)
        .map(|(_, field, _)| field.type_reference)
}

/// Normalize an integer literal or finite immutable local-copy chain to one
/// exact host index. Symbolic parameter leaves and every unsupported alias
/// shape remain unknown.
pub fn normalize_immutable_integer_bound_to_usize(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
                    LocalLookup::Found(local) => {
                        normalize_local(program, lookup, local, seen_aliases)
                    }
                    LocalLookup::Invalid => None,
                }
            } else {
                match local_by_name(lookup, members[0].as_str()) {
                    LocalLookup::Found(local) => {
                        normalize_local(program, lookup, local, seen_aliases)
                    }
                    LocalLookup::Missing | LocalLookup::Invalid => None,
                }
            }
        }
        _ => None,
    }
}

fn normalize_local(
    program: &dyn crate::CheckingInput,
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
    if !local.role.has_ordinary_initializer() {
        return Some(NormalizedBound::LocalValue(local.symbol));
    }
    seen_aliases.push(local.symbol);
    let normalized = match program.expression_table.expression(local.initial_value) {
        ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
            normalize_bound(program, lookup, local.initial_value, seen_aliases).map(|bound| {
                match bound {
                    // The local stores a value, not a retargetable alias to its source.
                    NormalizedBound::MutableValue => NormalizedBound::LocalValue(local.symbol),
                    bound => bound,
                }
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
/// A single-segment path's storage coordinate — the resolved symbol, its
/// mutability, and its declared type — joined through bound provenance or
/// the local lookup.
fn single_segment_receiver(
    lookup: &ImmutableBoundLookup<'_>,
    path: &typed_trees::expression::TableNamePath,
    members: &[typed_trees::name::Identifier],
) -> Option<(SymbolHandle, bool, typed_trees::types::TypeReferenceHandle)> {
    if members.len() != 1 {
        return None;
    }
    if path.symbol.is_valid() && path.head_symbol == path.symbol {
        bound_name_receiver(lookup, path)
    } else {
        match local_by_name(lookup, members[0].as_str()) {
            LocalLookup::Found(local) => {
                Some((local.symbol, local.is_mutable, local.type_reference))
            }
            LocalLookup::Missing | LocalLookup::Invalid => None,
        }
    }
}

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
                let parameter = lookup.machine_self_parameter(path.symbol)?;
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
    match lookup.local_entry(symbol) {
        Some(Some(local)) => LocalLookup::Found(local),
        Some(None) => LocalLookup::Invalid,
        None => LocalLookup::Missing,
    }
}

fn local_by_name<'program>(
    lookup: &ImmutableBoundLookup<'program>,
    name: &str,
) -> LocalLookup<'program> {
    match lookup.local_named(name) {
        Some(Some(local)) => LocalLookup::Found(local),
        Some(None) => LocalLookup::Invalid,
        None => LocalLookup::Missing,
    }
}

fn parameter_mutability(lookup: &ImmutableBoundLookup<'_>, symbol: SymbolHandle) -> Option<bool> {
    match lookup.parameter_entry(symbol) {
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
    lookup.parameter_entry(symbol).flatten()
}
