//! Exact slice metadata coordinates, kept separate from scalar parameter values.
use super::fields::{
    declared_field, member_chain, record_referent, rooted_carrier, unique_literal_field,
};
use super::{
    BTreeMap, BigInt, BinaryOperator, Comparison, Engine, ExpressionHandle, ExpressionNode,
    Machine, Polynomial, RankingRangeState, State, TypeReferenceNode, TypedTrees,
    unwrap_constraint_shells,
};
use symbols::SymbolHandle;
use typed_trees::data::DataField;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

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
    if !crate::value_custody::places::has_builtin_subslice_meaning(
        program,
        machine,
        Some(state),
        expression,
    ) {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
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
    (prove(start.clone()) && prove(end.sub(&start)) && prove(source.sub(&end)))
        .then(|| end.sub(&start))
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
        let coordinate = Self::for_parameter(program, parameter, &self.chain())?;
        (coordinate.root == self.root).then_some(coordinate)
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
            if depth > 0 && self.is_prefix_projection(program, state, leaf, depth) {
                return Some(self.value());
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
        let (carrier_root, _) = record_referent(program, carrier.type_reference)?;
        let mut chain = Vec::with_capacity(prefix.len() + self.steps.len() + 1);
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
        if owner != self.root {
            return None;
        }
        chain.extend(self.chain());
        Self::for_parameter(program, carrier, &chain)
    }

    /// `expression` is exactly `parameter.steps[..depth]`: the same formal
    /// projected through the first `depth` steps of this chain.
    fn is_prefix_projection(
        &self,
        program: &'program TypedTrees,
        state: &State,
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        member_chain(program, state, expression).is_some_and(|(parameter, chain)| {
            parameter.symbol == self.parameter.symbol
                && chain.len() == depth
                && chain
                    .iter()
                    .zip(&self.steps)
                    .all(|((symbol, spelled), step)| {
                        *symbol == step.symbol && *spelled == &step.name
                    })
        })
    }
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
            // The destination formal's own type decides whether the actual
            // arrives under one borrow: an owned source slot can feed a `&R`
            // formal through `&x`, while the source coordinate's boundary
            // says nothing about the arrival. A root edge re-fills the
            // coordinate's own formal, so its boundary is the arrival's.
            let arrival_borrowed = match destination {
                Some(destination) => {
                    coordinate
                        .at_arrival(program, destination, entry_symbol)?
                        .borrowed
                }
                None => coordinate.borrowed,
            };
            let actual = coordinate.arrived(
                program,
                machine,
                state,
                engine,
                argument,
                arrival_borrowed,
                bindings,
            )?;
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
