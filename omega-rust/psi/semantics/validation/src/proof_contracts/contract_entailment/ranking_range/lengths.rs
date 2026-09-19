//! Exact slice metadata coordinates, kept separate from scalar parameter values.
use super::fields::{
    collect_leaf_paths, declared_field, member_chain, nested_arrival_chain, record_referent,
    rooted_carrier, unique_literal_field,
};
use super::{
    BTreeMap, BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode,
    Machine, Polynomial, RankingRangeState, State, TypeReferenceNode, TypedTrees,
    unwrap_constraint_shells,
};
use symbols::SymbolHandle;
use typed_trees::data::DataField;
use typed_trees::expression::TableCallExpression;
use typed_trees::signature::StateParameter;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle};

pub(super) fn is_slice(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Slice { .. } => return true,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
    false
}

pub(super) fn parameter<'program>(
    program: &'program TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<&'program StateParameter> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    // Mutability is a storage capability, not a value change: the edge owners
    // supply a parameter-preserving evaluated prefix, so a mutable slice still
    // denotes its arrival sequence -- and its arrival length -- at the edge.
    program.state_parameters(state).iter().find(|parameter| {
        parameter.symbol == path.symbol
            && !parameter.is_self
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
    for (position, parameter) in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
    {
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
        for (parameter, entry) in program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(entries)
        {
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
                .find(|formal| !formal.is_self && formal.symbol == *entry)
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
        SliceCoordinate::resolve(program, state, expression).map(|coordinate| coordinate.value())
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
        let source = coordinate(indexed.collection)?;
        let start = if range.start.is_valid() {
            engine.normalize(range.start)?
        } else {
            Polynomial::default()
        };
        let end = if range.end.is_valid() {
            engine.normalize(range.end)?
        } else {
            source.clone()
        };
        let prove = |difference: Polynomial| {
            engine.requires_unsatisfiable
                || engine.prove_at_least(&engine.substituted(&difference), &BigInt::zero())
        };
        return (prove(start.clone()) && prove(end.sub(&start)) && prove(source.sub(&end)))
            .then(|| end.sub(&start));
    }
    // The actual names no formal coordinate, but its produced length can still
    // be exact caller evidence: a state-local slice binding's own extent, or a
    // declared fixed array's `.as_slice()` view.
    produced_length(program, machine, state, expression, 0)
        .map(|length| Polynomial::constant(BigInt::from_u64(length)))
}

/// The produced length a non-formal actual still carries into the call
/// boundary. A slice formal's `.len` binds a symbolic coordinate the caller
/// fills from `bindings`; a state-local `let` binding or a declared fixed
/// array's slice view instead carries a constant the callee's requires row
/// substitutes exactly -- the saved caller observation the boundary keeps.
/// Only shapes whose extent is fixed by declaration qualify: a `let mut`
/// local's initializer is its incoming value, not the value later writes may
/// leave, so mutable locals produce nothing here; an arbitrary call or borrow
/// produces nothing either.
pub(super) fn produced_length(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<u64> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid()
                || path.head_symbol != path.symbol
                || program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
            {
                return None;
            }
            // The name must resolve to exactly one immutable local declaration
            // of this state: zero means the name is a formal or unknown, two
            // means the statement list is ambiguous, and a mutable local's
            // incoming extent is not the value the call site observes.
            let locals = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == path.symbol => Some(local),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [local] = locals.as_slice() else {
                return None;
            };
            if local.is_mutable
                || !local.initial_value.is_valid()
                || binding_is_exclusively_exposed(program, state, path.symbol)
            {
                return None;
            }
            produced_length(program, machine, state, local.initial_value, depth + 1)
        }
        ExpressionNode::Borrow(borrow) => {
            produced_length(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::ArrayLiteral(values) => Some(values.len() as u64),
        ExpressionNode::Member(_) => declared_fixed_array_extent(
            program,
            crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            )?,
        ),
        ExpressionNode::Call(call) => slice_view_call_length(program, machine, state, call),
        _ => None,
    }
}

/// An immutable local's initializer describes this occurrence only while no
/// exclusive borrow roots at its binding: `expose(&mut view)` hands the
/// descriptor to a callee that can install a different slice, so the saved
/// initializer no longer names the referent the call sees. The scan is
/// deliberately state-wide -- the caller's obligation order is not the
/// statement order -- and covers every carrier that can hold an expression.
fn binding_is_exclusively_exposed(
    program: &TypedTrees,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    let mut roots = Vec::new();
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
    {
        match statement {
            StatementNode::RootBinding(binding) => {
                roots.extend([binding.receiver, binding.implementation_operand]);
            }
            StatementNode::AssemblyFact(fact) => roots.push(fact.expression),
            StatementNode::Assignment(assignment) => {
                roots.extend([assignment.target, assignment.value]);
            }
            StatementNode::Call(call) => roots.extend(
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            ),
            StatementNode::Expression(expression) => roots.push(*expression),
            StatementNode::LocalData(local) => roots.push(local.initial_value),
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    roots.push(guard);
                }
                for target in [transition.target, transition.continuation] {
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named { arguments, .. } => roots.extend(
                            program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .copied(),
                        ),
                        TransitionTargetNode::Value(expression) => roots.push(*expression),
                        _ => {}
                    }
                }
            }
        }
    }
    let mut nodes = Vec::new();
    for root in roots {
        crate::value_custody::expression_types::collect_expression_nodes(program, root, &mut nodes);
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
    if !matches!(call.target.as_str(), "as_slice" | "as_mut_slice")
        || call.target_symbol.is_valid()
        || !call.arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    declared_fixed_array_extent(
        program,
        crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            call.receiver,
        )?,
    )
}

/// The literal extent a declared type spells, unwrapping reference and
/// domain-constraint shells. Any other extent form -- a const binder, an
/// unevaluated const call, a slice -- keeps the length unknown.
fn declared_fixed_array_extent(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<u64> {
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(length),
                ..
            } => return Some(*length as u64),
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return None,
        }
    }
    None
}

/// A produced length coordinate over projected storage: the slice leaf of an
/// exact member chain rooted at a state formal, `parameter.steps...field`.
/// The atom names the collection's produced length, never its contents. Like
/// the field coordinate of an integer leaf, the chain is resolved against the
/// formal's own declaration and re-resolved onto the arrival's carrier through
/// the discovered telescope — a telescope names a role, so duplicated record
/// roles resolve to no coordinate rather than a guessed copy.
pub(super) struct SliceCoordinate<'program> {
    /// The formal whose record carries the collection.
    pub parameter: &'program StateParameter,
    /// The formal names its record through a reference; a borrowed arrival
    /// unwraps one `&` before the literal walk.
    pub borrowed: bool,
    /// The record the formal's (referent) type declares.
    pub root: SymbolHandle,
    /// The record-typed fields from `root` down to `field`'s owner, in order.
    pub steps: Vec<&'program DataField>,
    /// The slice-typed leaf field the length is produced from.
    pub field: &'program DataField,
    pub identity: String,
}

impl<'program> SliceCoordinate<'program> {
    /// The coordinate an authored member chain names: every step an exact
    /// declared field, by resolved symbol and spelling, of the record before
    /// it, and the leaf a slice. Bare formals keep the `bindings` coordinate;
    /// a slice leaf reached through a reference boundary inside the chain is
    /// not a projected coordinate (the field owner has the same rule).
    pub(super) fn resolve(
        program: &'program TypedTrees,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        let (parameter, chain) = member_chain(program, state, expression)?;
        let coordinate = Self::for_parameter(
            program,
            parameter,
            &chain.iter().map(|(symbol, _)| *symbol).collect::<Vec<_>>(),
        )?;
        let spelled_as_declared = chain
            .iter()
            .zip(coordinate.steps.iter().chain([&coordinate.field]))
            .all(|((_, spelled), field)| **spelled == field.name);
        spelled_as_declared.then_some(coordinate)
    }

    fn for_parameter(
        program: &'program TypedTrees,
        parameter: &'program StateParameter,
        chain: &[SymbolHandle],
    ) -> Option<Self> {
        // A mutable record still denotes its arrival value at an edge whose
        // evaluated prefix is proven to preserve its path; mutability is a
        // storage capability, not evidence the collection moved.
        if parameter.is_self || parameter.is_const {
            return None;
        }
        let (root, borrowed) = record_referent(program, parameter.type_reference)?;
        let (last, step_symbols) = chain.split_last()?;
        let mut owner = root;
        let mut steps = Vec::with_capacity(step_symbols.len());
        for symbol in step_symbols {
            let (_, field) = declared_field(program, owner, *symbol)?;
            // An intermediate step is an owned exact record; a reference
            // boundary inside the chain needs its own load evidence.
            let TypeReferenceNode::Named { symbol: next, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            else {
                return None;
            };
            steps.push(field);
            owner = *next;
        }
        let (_, field) = declared_field(program, owner, *last)?;
        // The produced coordinate is the collection's length; an integer or
        // record leaf is a different produced value, not a slice length.
        if !is_slice(program, field.type_reference) {
            return None;
        }
        let mut identity = format!("\0ranking:length:{:?}", parameter.symbol);
        for symbol in chain {
            identity.push_str(&format!(":{symbol:?}"));
        }
        Some(Self {
            parameter,
            borrowed,
            root,
            steps,
            field,
            identity,
        })
    }

    /// The resolved field symbols from the root record down to `field`.
    pub(super) fn chain(&self) -> Vec<SymbolHandle> {
        self.steps
            .iter()
            .chain([&self.field])
            .map(|field| field.symbol)
            .collect()
    }

    /// A telescope names a role, not record compatibility or equality between
    /// copies. Only a unique formal with the exact declaration can carry this
    /// coordinate; duplicated record roles need independent length facts.
    pub(super) fn at_arrival(
        &self,
        program: &'program TypedTrees,
        arrival: RankingRangeState<'_>,
        entry_symbol: SymbolHandle,
    ) -> Option<Self> {
        if !entry_symbol.is_valid() {
            return None;
        }
        let mut parameters = program
            .state_parameters(arrival.state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(arrival.entry_parameters)
            .filter_map(|(parameter, entry)| (*entry == entry_symbol).then_some(parameter));
        let parameter = parameters.next()?;
        if parameters.next().is_some() {
            return None;
        }
        self.for_carrier(program, parameter)
    }

    /// The coordinate this chain names on `carrier`'s own record: the chain
    /// resolved directly against the carrier's declaration, else the unique
    /// nested path that declaration admits for the chain's boundary records.
    /// These are the same two readings `at_arrival` keeps once a telescope has
    /// located the role's carrier. A caller that already holds the formal asks
    /// this directly; two admissible readings keep no coordinate rather than
    /// guess which record the carriage meant.
    pub(super) fn for_carrier(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
    ) -> Option<Self> {
        let chain = self.chain();
        if let Some(coordinate) = Self::for_parameter(program, carrier, &chain)
            && coordinate.root == self.root
        {
            return Some(coordinate);
        }
        // The role may instead arrive nested inside the carrier formal's own
        // record -- `pair.bag` carries the collection's record -- or as a
        // record this chain descends partway. The formal's declaration must
        // admit exactly one such reading; two readings leave the carriage
        // ambiguous and keep no coordinate, exactly as the field owner does.
        let chain = nested_arrival_chain(program, carrier, self.root, &self.steps, &chain)?;
        Self::for_parameter(program, carrier, &chain)
    }

    pub(super) fn value(&self) -> Polynomial {
        Polynomial::atom(self.identity.clone())
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
    pub(super) fn arrived(
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

    /// The coordinate this chain reads through `carrier`'s member `prefix`:
    /// `p.f` (or a bare formal) denotes a record, and this coordinate's chain
    /// resumes there once the prefix lands on this coordinate's own root
    /// record. A prefix step through anything but an exact named record -- a
    /// reference, a slice, or a primitive -- has no carrier coordinate.
    fn through_carrier(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
        prefix: &[(SymbolHandle, &typed_trees::name::Identifier)],
    ) -> Option<Self> {
        self.through_carrier_at(program, carrier, prefix, self.root, 0)
    }

    /// The coordinate this chain reads when `carrier.prefix` supplies the
    /// record `expected` at chain position `depth`: the prefix walks the
    /// carrier's own declaration, must land on `expected`, and the remaining
    /// chain steps resolve there. A prefix landing on any other record names
    /// a different carriage, not this coordinate.
    fn through_carrier_at(
        &self,
        program: &'program TypedTrees,
        carrier: &'program StateParameter,
        prefix: &[(SymbolHandle, &typed_trees::name::Identifier)],
        expected: SymbolHandle,
        depth: usize,
    ) -> Option<Self> {
        let (carrier_root, _) = record_referent(program, carrier.type_reference)?;
        let mut chain = Vec::with_capacity(prefix.len() + self.steps.len() + 1 - depth);
        let mut owner = carrier_root;
        for (symbol, _) in prefix {
            let (_, field) = declared_field(program, owner, *symbol)?;
            // A prefix step must land on an exact declared record so this
            // coordinate's chain resumes at one nominal root.
            let TypeReferenceNode::Named { symbol: next, .. } = program
                .type_reference_table
                .type_reference(unwrap_constraint_shells(program, field.type_reference))
            else {
                return None;
            };
            chain.push(*symbol);
            owner = *next;
        }
        if owner != expected {
            return None;
        }
        chain.extend(self.chain().into_iter().skip(depth));
        Self::for_parameter(program, carrier, &chain)
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
    SliceCoordinate::for_parameter(program, parameter, path)
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
        let mut pending = expressions
            .iter()
            .map(|expression| (*expression, 0usize))
            .collect::<Vec<_>>();
        while let Some((expression, depth)) = pending.pop() {
            if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            let mut visit = |child| pending.push((child, depth + 1));
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) => {
                    let resolved = crate::value_custody::places::collection_length_receiver(
                        program,
                        machine,
                        Some(state),
                        expression,
                    )
                    .and_then(|receiver| SliceCoordinate::resolve(program, state, receiver))
                    .or_else(|| {
                        let receiver = crate::value_custody::places::collection_length_receiver(
                            program,
                            machine,
                            Some(root),
                            expression,
                        )?;
                        let coordinate = SliceCoordinate::resolve(program, root, receiver)?;
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
                }
                ExpressionNode::Borrow(borrow) => visit(borrow.target),
                ExpressionNode::Binary(binary) => {
                    visit(binary.left);
                    visit(binary.right);
                }
                ExpressionNode::Unary(unary) => visit(unary.operand),
                ExpressionNode::Atomic(atomic) => visit(atomic.value),
                ExpressionNode::StructLiteral(literal) => {
                    for field in program.expression_table.struct_fields(literal.fields) {
                        visit(field.value);
                    }
                }
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
            }
        }
        Some(())
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
                    let (_, entry) = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .zip(entries)
                        .find(|(parameter, _)| parameter.symbol == coordinate.parameter.symbol)?;
                    if !entry.is_valid() {
                        // A role-less record serves its own guards only; its
                        // projected slices never rewrite a template
                        // coordinate.
                        continue;
                    }
                    coordinate.at_arrival(
                        program,
                        RankingRangeState {
                            state,
                            entry_parameters: entries,
                        },
                        *entry,
                    )?;
                    *entry
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
                    arrived.arrived(
                        program,
                        machine,
                        state,
                        engine,
                        argument,
                        arrived.borrowed,
                        bindings,
                    )?
                }
                None => coordinate.arrived(
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
