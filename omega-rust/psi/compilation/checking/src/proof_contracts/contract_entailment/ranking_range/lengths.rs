//! Exact slice metadata coordinates, kept separate from scalar parameter values.
//! Collection lengths compose through borrows and range formation regardless
//! of whether the source extent is declared or observed. Saved initializers
//! use the same evaluator without caller facts: a later observation cannot
//! retroactively describe the descriptor captured by an earlier binding.
use super::coordinate::{Coordinate, CoordinateKind};
use super::fields::{
    collect_leaf_paths, declared_spelling_verified, member_chain, record_referent, rooted_carrier,
    unique_literal_field,
};
use super::{
    BTreeMap, BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode,
    Machine, Polynomial, RankingRangeState, State, TypeReferenceNode, TypedTrees, projections,
    unwrap_constraint_shells,
};
use language_semantics::declaration_selection::CollectionViewOperation;
use symbols::SymbolHandle;
use typed_trees::expression::TableCallExpression;
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn is_slice(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    crate::program_scope::type_shape::has_slice_base(program, reference)
}

pub(super) fn parameter<'program>(
    program: &'program TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<&'program StateParameter> {
    let Some(symbol) = crate::expression_traversal::single_segment_name_symbol(program, expression)
    else {
        return None;
    };
    // Mutability is a storage capability, not a value change: the edge owners
    // supply a parameter-preserving evaluated prefix, so a mutable slice still
    // denotes its arrival sequence -- and its arrival length -- at the edge.
    program.state_parameters(state).iter().find(|parameter| {
        parameter.symbol == symbol
            && !parameter.is_const
            && is_slice(program, parameter.type_reference)
    })
}

pub(super) fn bindings(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    entry_parameters: Option<&[SymbolHandle]>,
) -> Vec<(SymbolHandle, String)> {
    let mut bindings = Vec::new();
    for (position, parameter) in program.state_parameters(state).iter().enumerate() {
        // Mutable slices are admitted on the same contract as `parameter`:
        // callers evaluate the produced length facts over a prefix that is
        // proven to preserve the parameter's path, so the arrival binding is
        // still the value the edge observes.
        if !parameter.symbol.is_valid()
            || parameter.is_const
            || !is_slice(program, parameter.type_reference)
        {
            continue;
        }
        let identity = format!("\0ranking:length:{:?}", parameter.symbol);
        bindings.push((parameter.symbol, identity.clone()));
        if let Some(entries) = entry_parameters {
            let entry = entries[position];
            if entry.is_valid()
                && entry != parameter.symbol
                && entries
                    .iter()
                    .filter(|candidate| **candidate == entry)
                    .count()
                    == 1
            {
                bindings.push((entry, identity));
            }
        }
    }
    // A bare slice formal may arrive packed inside a record literal: the
    // formal claiming its entry role then holds the collection at the
    // record's unique slice leaf, so the role's produced length reads the
    // projected coordinate there rather than a guessed path.
    let Some(root) = program.machine_states(machine).first() else {
        return bindings;
    };
    if let Some(entries) = entry_parameters {
        for (parameter, entry) in program.state_parameters(state).iter().zip(entries) {
            if !entry.is_valid()
                || entries
                    .iter()
                    .filter(|candidate| **candidate == *entry)
                    .count()
                    != 1
                || bindings.iter().any(|(symbol, _)| *symbol == *entry)
            {
                continue;
            }
            let Some(root_formal) = program
                .state_parameters(root)
                .iter()
                .find(|formal| formal.symbol == *entry)
            else {
                continue;
            };
            if !is_slice(program, root_formal.type_reference) {
                continue;
            }
            if let Some(coordinate) = slice_leaf_coordinate(program, parameter) {
                bindings.push((*entry, coordinate.identity.clone()));
            }
        }
    }
    bindings
}

pub(super) fn install(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: &State,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
    expressions: &[ExpressionHandle],
) -> Option<()> {
    if bindings.is_empty() {
        return Some(());
    }
    for expression in expressions {
        install_expression(
            program,
            machine,
            state,
            root,
            bindings,
            engine,
            *expression,
            0,
        )?;
    }
    Some(())
}

fn install_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: &State,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<()> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    // These handles are metadata projections only. A slice Name stays outside
    // the scalar language, including in expressions that algebraically cancel.
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_)
    ) {
        for scope in [state, root] {
            let Some(receiver) = crate::value_custody::places::collection_length_receiver(
                program,
                machine,
                Some(scope),
                expression,
            ) else {
                continue;
            };
            let Some(parameter) = parameter(program, scope, receiver) else {
                continue;
            };
            let Some((_, identity)) = bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
            else {
                continue;
            };
            if !engine.bind_strict_projection(expression, Polynomial::atom(identity.clone())) {
                return None;
            }
        }
    }
    let mut visit = |child| {
        install_expression(
            program,
            machine,
            state,
            root,
            bindings,
            engine,
            child,
            depth + 1,
        )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            visit(binary.left)?;
            visit(binary.right)?;
        }
        ExpressionNode::Unary(unary) => visit(unary.operand)?,
        ExpressionNode::Atomic(atomic) => visit(atomic.value)?,
        ExpressionNode::Member(member) => visit(member.receiver)?,
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection)?;
            visit(indexed.index)?;
        }
        ExpressionNode::Range(range) => {
            for endpoint in [range.start, range.end] {
                if endpoint.is_valid() {
                    visit(endpoint)?;
                }
            }
        }
        _ => {}
    }
    Some(())
}

pub(super) fn actual(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
) -> Option<Polynomial> {
    evaluate_length(program, machine, state, expression, bindings, engine, 0)
}

fn evaluate_length(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    bindings: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
    depth: usize,
) -> Option<Polynomial> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        if !matches!(
            borrow.access,
            language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
        ) {
            return None;
        }
        return evaluate_length(
            program,
            machine,
            state,
            borrow.target,
            bindings,
            engine,
            depth + 1,
        );
    }
    let coordinate = |expression| {
        if let Some(parameter) = parameter(program, state, expression) {
            let (_, identity) = bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)?;
            return Some(Polynomial::atom(identity.clone()));
        }
        // A member chain ending at a slice leaf names the same canonical
        // coordinate the projected-storage owner installs for `.len`
        // spellings; its atom is fabricated here only as a substitution
        // result, so a chain the state never established stays an unknown
        // value and fails the membership proofs it would have needed.
        SliceCoordinate::resolve_length(program, state, expression)
            .map(|coordinate| coordinate.value())
    };
    if let Some(length) = coordinate(expression) {
        return Some(length);
    }
    if crate::value_custody::places::has_builtin_subslice_meaning(
        program,
        machine,
        Some(state),
        expression,
    ) {
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        else {
            return None;
        };
        let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
        else {
            return None;
        };
        // Constant-extent queries also enter here without the call owner's
        // meaning pass. Never interpret an authored operator as arithmetic
        // merely because its operands happen to normalize to constants.
        for endpoint in [range.start, range.end] {
            if endpoint.is_valid() {
                super::meanings::builtin(program, machine, state, endpoint, depth + 1)?;
            }
        }
        let source = evaluate_length(
            program,
            machine,
            state,
            indexed.collection,
            bindings,
            engine,
            depth + 1,
        )?;
        let start = if range.start.is_valid() {
            engine.normalize(range.start)?
        } else {
            Polynomial::default()
        };
        let mut end = if range.end.is_valid() {
            engine.normalize(range.end)?
        } else {
            source.clone()
        };
        let authored_end = end.clone();
        if range.end_inclusive {
            if !range.end.is_valid() {
                return None;
            }
            end = end.add(&Polynomial::constant(BigInt::from_u64(1)));
        }
        let prove = |difference: Polynomial| {
            engine.requires_unsatisfiable
                || engine.prove_at_least(&engine.substituted(&difference), &BigInt::zero())
        };
        // Normalizing -1 to zero must not make an invalid inclusive index a
        // valid empty window. Check the authored index before its successor.
        return (prove(start.clone())
            && (!range.end_inclusive || prove(authored_end))
            && prove(end.sub(&start))
            && prove(source.sub(&end)))
        .then(|| end.sub(&start));
    }
    // Array extent belongs to the declared place, not to its current contents.
    // Replacing elements or the whole array cannot resize it.
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(_) | ExpressionNode::Member(_)
    ) && let Some(length) = crate::value_custody::places::declared_collection_extent(
        program,
        machine,
        Some(state),
        expression,
    ) {
        return Some(Polynomial::constant(BigInt::from_u64(length as u64)));
    }
    let length = match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            let Some(symbol) =
                crate::expression_traversal::single_segment_name_symbol(program, expression)
            else {
                return None;
            };
            // The name must resolve to exactly one local declaration of this
            // state: zero means the name is a formal or unknown, two means the
            // statement list is ambiguous, and a mutable or written local's
            // incoming extent is not the value the call site observes.
            let (_, local) = program.state_local_declaration(state, symbol)?;
            if local.is_mutable
                || !local.initial_value.is_valid()
                || binding_is_exclusively_exposed(program, state, symbol)
                || binding_is_written(program, state, symbol)
            {
                return None;
            }
            produced_length(program, machine, state, local.initial_value, depth + 1)
        }
        ExpressionNode::ArrayLiteral(values) => Some(values.len() as u64),
        ExpressionNode::Call(call) => slice_view_call_length(program, machine, state, call),
        _ => None,
    }?;
    Some(Polynomial::constant(BigInt::from_u64(length)))
}

/// A constant produced extent, independent of current caller facts. Reuse
/// ordinary length composition with an empty arithmetic environment so saved
/// initializers never borrow facts established after their evaluation.
pub(super) fn produced_length(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<u64> {
    let mut engine = Engine::strict_with_symbol_bindings(program, machine, &[]);
    evaluate_length(program, machine, state, expression, &[], &mut engine, depth)?
        .constant_value()?
        .to_u64()
}

/// An immutable local's initializer describes this occurrence only while no
/// exclusive borrow roots at its binding: `expose(&mut view)` hands the
/// descriptor to a callee that can install a different slice, so the saved
/// initializer no longer names the referent the call sees. The scan is
/// deliberately state-wide -- the caller's obligation order is not the
/// statement order -- and covers every carrier that can hold an expression.
pub(super) fn binding_is_exclusively_exposed(
    program: &TypedTrees,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
    for root in statement_expression_roots(program, state) {
        crate::expression_traversal::collect_expression_nodes(program, root, &mut nodes, &mut seen);
    }
    nodes
        .iter()
        .any(|node| match program.expression_table.expression(*node) {
            ExpressionNode::Borrow(borrow) => {
                borrow.access.is_exclusive()
                    && borrow_root_symbol(program, borrow.target) == Some(symbol)
            }
            _ => false,
        })
}

/// An entry observation names the value the call edge still sees only while
/// nothing in `state` writes the carrier. `binding_is_exclusively_exposed`
/// covers `&mut`/`&out` borrow expressions; the remaining write channels are
/// an assignment whose target path descends to `symbol`, a receiver call
/// whose callee claims a mutable `self`, and a sealed atomic operation on
/// the place. Reads and moves leave the observed value alone -- a moved
/// name cannot be an actual afterwards. The scan is state-wide for the same
/// reason as the borrow scan: obligation order is not statement order.
pub(super) fn binding_is_written(
    program: &TypedTrees,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
    {
        match statement {
            StatementNode::Assignment(assignment)
                if borrow_root_symbol(program, assignment.target) == Some(symbol) =>
            {
                return true;
            }
            StatementNode::Call(call)
                if call.receiver_root_symbol == symbol
                    && call_receiver_is_mutable(program, call.target_symbol) =>
            {
                return true;
            }
            _ => {}
        }
    }
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
    for root in statement_expression_roots(program, state) {
        crate::expression_traversal::collect_expression_nodes(program, root, &mut nodes, &mut seen);
    }
    nodes
        .iter()
        .any(|node| match program.expression_table.expression(*node) {
            ExpressionNode::Call(call) => {
                call.receiver.is_valid()
                    && borrow_root_symbol(program, call.receiver) == Some(symbol)
                    && call_receiver_is_mutable(program, call.target_symbol)
            }
            ExpressionNode::Atomic(atomic) => {
                borrow_root_symbol(program, atomic.value) == Some(symbol)
                    || borrow_root_symbol(program, atomic.result) == Some(symbol)
            }
            _ => false,
        })
}

/// Unknown callees conservatively invalidate a receiver observation. Resolved
/// static calls carry every writable place through their explicit arguments.
fn call_receiver_is_mutable(program: &TypedTrees, target_symbol: SymbolHandle) -> bool {
    crate::machine_calls::calls::machine_state_by_symbol(program, target_symbol).is_none()
}

/// Every expression root a statement of `state` can carry: the channels a
/// nested borrow or receiver call could reach an observed carrier through.
fn statement_expression_roots(program: &TypedTrees, state: &State) -> Vec<ExpressionHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .flat_map(|statement| program.statement_table.expression_roots(statement))
        .map(|(_, root)| root)
        .collect()
}

/// The root symbol a borrowable place path descends from: `view`, `view.field`,
/// or `view[i]` all root at `view`'s local symbol.
fn borrow_root_symbol(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let mut visited = 0;
    while program.expression_table.expression_is_valid(expression) && visited < 128 {
        visited += 1;
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Name(path) => return Some(path.head_symbol),
            _ => return None,
        }
    }
    None
}

/// `x.as_slice()` / `x.as_mut_slice()` over a declared fixed array is a pure
/// view of the receiver's declared extent. A resolved machine target, any
/// authored arguments or evidence, or a dispatch/layout request names a
/// different operation that keeps no produced length here.
fn slice_view_call_length(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
) -> Option<u64> {
    if !matches!(
        CollectionViewOperation::from_authored_spelling(call.target.as_str()),
        Some(CollectionViewOperation::SharedSlice | CollectionViewOperation::MutableSlice)
    ) || call.target_symbol.is_valid()
        || !call.arguments.is_empty()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    crate::value_custody::places::declared_collection_extent(
        program,
        machine,
        Some(state),
        call.receiver,
    )
    .map(|length| length as u64)
}

/// A produced length coordinate over projected storage: the slice leaf of an
/// exact member chain rooted at a state formal, `parameter.steps...field`.
/// The atom names the collection's produced length, never its contents. Like
/// the field coordinate of an integer leaf, the chain is resolved against the
/// formal's own declaration and re-resolved onto the arrival's carrier through
/// the discovered telescope — a telescope names a role, so duplicated record
/// roles resolve to no coordinate rather than a guessed copy.
/// The produced-length coordinate: a [`coordinate::Coordinate`] whose step
/// owners are exact named records and whose leaf is a slice field.
pub(super) type SliceCoordinate<'program> = Coordinate<'program>;

impl<'program> Coordinate<'program> {
    /// The coordinate an authored member chain names: every step an exact
    /// declared field, by resolved symbol and spelling, of the record before
    /// it, and the leaf a slice. Bare formals keep the `bindings` coordinate;
    /// a slice leaf reached through a reference boundary inside the chain is
    /// not a projected coordinate (the field owner has the same rule).
    pub(super) fn resolve_length(
        program: &'program TypedTrees,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        let (parameter, chain) = member_chain(program, state, expression)?;
        let coordinate = Self::for_parameter(
            program,
            parameter,
            &chain.iter().map(|(symbol, _)| *symbol).collect::<Vec<_>>(),
            CoordinateKind::Length,
            false,
        )?;
        declared_spelling_verified(&chain, &coordinate.steps, coordinate.field)
            .then_some(coordinate)
    }

    /// The length this coordinate holds after `expression` arrives in its
    /// formal: the formal itself forwarded, a record carrier behind the
    /// destination's `&` or a member-target `p.f` whose projection prefix
    /// lands on this coordinate's root, or the literal chain rebuilt
    /// declaration by declaration down to the slice leaf, whose own length
    /// `actual` then computes. `arrival_borrowed` is the destination formal's
    /// own reference boundary, so a borrowed arrival unwraps one `&` no matter
    /// how the source slot stored its record. A literal of another
    /// declaration, a foreign record, or a missing step is not this
    /// coordinate.
    pub(super) fn arrived_length(
        &self,
        program: &'program TypedTrees,
        machine: &Machine,
        state: &State,
        engine: &mut Engine<'_>,
        expression: ExpressionHandle,
        arrival_borrowed: bool,
        bindings: &[(SymbolHandle, String)],
    ) -> Option<Polynomial> {
        let mut current = expression;
        if arrival_borrowed {
            // An owned actual into a borrowed formal has no reference boundary
            // to read through; only a `&` spelling reaches the record the
            // destination denotes.
            let ExpressionNode::Borrow(borrow) = program.expression_table.expression(current)
            else {
                return None;
            };
            current = borrow.target;
        }
        if let Some((carrier, prefix)) = rooted_carrier(program, state, current) {
            return Some(self.through_carrier(program, carrier, &prefix)?.value());
        }
        let mut owner = self.root;
        let mut leaf = current;
        for (depth, field) in self.steps.iter().chain([&self.field]).enumerate() {
            if depth > 0
                && let Some((carrier, prefix)) = rooted_carrier(program, state, leaf)
            {
                // `leaf` names the record this step reads through a carrier
                // instead of a literal: a formal forward or a member
                // projection inside the rebuilt record. Its prefix must land
                // on the record the chain expects at this step; the remaining
                // chain then resolves under the carrier's own declaration, so
                // a forward of this coordinate's own prefix keeps this
                // coordinate itself.
                return self
                    .through_carrier_at(program, carrier, &prefix, owner, depth)
                    .map(|coordinate| coordinate.value());
            }
            leaf = unique_literal_field(program, leaf, owner, field)?;
            if let TypeReferenceNode::Named { symbol, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            {
                owner = *symbol;
            }
        }
        actual(program, machine, state, leaf, bindings, engine)
    }
}

/// The produced-length coordinate a bare slice entry role takes inside
/// `parameter`'s record: the declaration must hold exactly one slice leaf
/// reachable through owned exact record steps. The claim only locates the
/// leaf -- the arrival's field value, membership, and strict descent still
/// prove independently -- and a second slice leaf keeps no coordinate rather
/// than guessing which collection holds the role.
pub(super) fn slice_leaf_coordinate<'program>(
    program: &'program TypedTrees,
    parameter: &'program StateParameter,
) -> Option<SliceCoordinate<'program>> {
    let (root, _) = record_referent(program, parameter.type_reference)?;
    let mut paths = Vec::new();
    collect_leaf_paths(
        program,
        root,
        |program, field| is_slice(program, field.type_reference),
        &mut paths,
    );
    let [path] = paths.as_slice() else {
        return None;
    };
    SliceCoordinate::for_parameter(program, parameter, path, CoordinateKind::Length, false)
}

/// The projected slice coordinates one range judgment reads: the produced
/// rank coordinate plus every auxiliary `.len` coordinate discovered while
/// installing authored expressions. Binding spellings, declared facts, and
/// arrival substitution all iterate the same set, so a coordinate named in a
/// guard and the measure's own subject resolve to the same canonical atom.
pub(super) struct SliceCoordinates<'program> {
    rank: Option<SliceCoordinate<'program>>,
    auxiliary: Vec<SliceCoordinate<'program>>,
}

impl<'program> SliceCoordinates<'program> {
    pub(super) fn new(rank: SliceCoordinate<'program>) -> Self {
        Self {
            rank: Some(rank),
            auxiliary: Vec::new(),
        }
    }

    /// No produced rank coordinate: the measure is not `Slice::Length`, but
    /// authored `.len` spellings inside endpoints, requires facts, guards, or
    /// actuals still name auxiliary coordinates the same install discovers.
    pub(super) fn empty() -> Self {
        Self {
            rank: None,
            auxiliary: Vec::new(),
        }
    }

    pub(super) fn value(&self) -> Option<Polynomial> {
        self.rank.as_ref().map(SliceCoordinate::value)
    }

    pub(super) fn coordinates(&self) -> impl Iterator<Item = &SliceCoordinate<'program>> {
        self.rank.iter().chain(&self.auxiliary)
    }

    fn include(&mut self, coordinate: SliceCoordinate<'program>) {
        // The identity spells the formal and the whole resolved chain, so two
        // same-named leaves under different steps stay distinct coordinates.
        if !self
            .coordinates()
            .any(|existing| existing.identity == coordinate.identity)
        {
            self.auxiliary.push(coordinate);
        }
    }

    /// A produced length is a natural coordinate; every installed coordinate
    /// contributes its `>= 0` fact exactly as the bare-formal bindings do.
    pub(super) fn comparisons(&self) -> Vec<Comparison> {
        self.coordinates()
            .map(|coordinate| {
                (
                    BinaryOperator::GreaterOrEqual,
                    coordinate.value(),
                    Polynomial::default(),
                )
            })
            .collect()
    }

    /// Bind only authored `x.chain.len` projections that resolve to a
    /// projected coordinate, never all slice fields in a record. Two
    /// same-named leaves remain independent by formal and resolved chain.
    pub(super) fn install(
        &mut self,
        program: &'program TypedTrees,
        machine: &Machine,
        state: &State,
        root: &State,
        entry_parameters: Option<&[SymbolHandle]>,
        engine: &mut Engine<'_>,
        expressions: &[ExpressionHandle],
    ) -> Option<()> {
        projections::for_each_projection_expression(
            program,
            expressions,
            |member, expression, visit| {
                let resolved = crate::value_custody::places::collection_length_receiver(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                .and_then(|receiver| SliceCoordinate::resolve_length(program, state, receiver))
                .or_else(|| {
                    let receiver = crate::value_custody::places::collection_length_receiver(
                        program,
                        machine,
                        Some(root),
                        expression,
                    )?;
                    let coordinate = SliceCoordinate::resolve_length(program, root, receiver)?;
                    coordinate.at_arrival(
                        program,
                        RankingRangeState {
                            state,
                            entry_parameters: entry_parameters?,
                        },
                        coordinate.parameter.symbol,
                    )
                });
                if let Some(coordinate) = resolved {
                    if !engine.bind_strict_projection(expression, coordinate.value()) {
                        return None;
                    }
                    self.include(coordinate);
                }
                visit(member.receiver);
                Some(())
            },
            |node, visit| match node {
                ExpressionNode::Indexed(indexed) => {
                    visit(indexed.collection);
                    visit(indexed.index);
                }
                ExpressionNode::Range(range) => {
                    for endpoint in [range.start, range.end] {
                        if endpoint.is_valid() {
                            visit(endpoint);
                        }
                    }
                }
                _ => {}
            },
        )
    }

    /// Every demanded projected slice of this formal receives the same
    /// evaluated actual. The caller applies this map simultaneously, never
    /// sequentially rewriting one coordinate's actual through another's new
    /// value. A duplicated or role-less destination claim produces no
    /// substitution rather than a first/last-wins guess.
    pub(super) fn substitute(
        &self,
        program: &'program TypedTrees,
        machine: &Machine,
        state: &State,
        entry_parameters: Option<&[SymbolHandle]>,
        destination: Option<RankingRangeState<'_>>,
        engine: &mut Engine<'_>,
        formal: SymbolHandle,
        argument: ExpressionHandle,
        bindings: &[(SymbolHandle, String)],
        substitutions: &mut BTreeMap<String, Polynomial>,
    ) -> Option<bool> {
        let mut matched = false;
        for coordinate in self.coordinates() {
            let entry_symbol = match entry_parameters {
                None => coordinate.parameter.symbol,
                Some(entries) => {
                    let Some(entry) = super::coordinate_entry_symbol(
                        program,
                        state,
                        entries,
                        coordinate.parameter.symbol,
                    )?
                    else {
                        // A role-less record serves its own guards only; its
                        // projected slices never rewrite a template
                        // coordinate.
                        continue;
                    };
                    coordinate.at_arrival(
                        program,
                        RankingRangeState {
                            state,
                            entry_parameters: entries,
                        },
                        entry,
                    )?;
                    entry
                }
            };
            if entry_symbol != formal {
                continue;
            }
            // The destination formal's own coordinate walks the actual: a
            // role arriving nested inside the carrier's record -- `pair.bag`
            // holding the collection's record -- or as a record this chain
            // reaches partway reads the literal through the chain the
            // destination's invariant names, not the source slot's own
            // spelling. The destination formal's own type likewise decides
            // whether the actual arrives under one borrow: an owned source
            // slot can feed a `&R` formal through `&x`, while the source
            // coordinate's boundary says nothing about the arrival. A root
            // edge re-fills the coordinate's own formal, so its boundary is
            // the arrival's.
            let actual = match destination {
                Some(destination) => {
                    let arrived = coordinate.at_arrival(program, destination, entry_symbol)?;
                    arrived.arrived_length(
                        program,
                        machine,
                        state,
                        engine,
                        argument,
                        arrived.borrowed,
                        bindings,
                    )?
                }
                None => coordinate.arrived_length(
                    program,
                    machine,
                    state,
                    engine,
                    argument,
                    coordinate.borrowed,
                    bindings,
                )?,
            };
            if substitutions
                .insert(coordinate.identity.clone(), actual)
                .is_some()
            {
                return None;
            }
            matched = true;
        }
        Some(matched)
    }
}
