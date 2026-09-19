//! Exact field projections used by one ranking judgment. Root-template
//! projections and current-state projections share an atom only through a
//! unique, nominally checked arrival role. Record ancestry alone never proves
//! copy equality.
use super::{
    BTreeMap, Comparison, Engine, ExpressionHandle, ExpressionNode, Polynomial, RankingRangeState,
    State, TypedTrees, comparison_proven, fields,
};
use fields::FieldCoordinate;
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::signature::StateParameter;

pub(super) struct FieldCoordinates<'program> {
    rank: Option<FieldCoordinate<'program>>,
    auxiliary: Vec<FieldCoordinate<'program>>,
}

impl<'program> FieldCoordinates<'program> {
    pub(super) fn new(rank: FieldCoordinate<'program>) -> Self {
        Self {
            rank: Some(rank),
            auxiliary: Vec::new(),
        }
    }

    pub(super) fn empty() -> Self {
        Self {
            rank: None,
            auxiliary: Vec::new(),
        }
    }

    pub(super) fn value(&self) -> Option<Polynomial> {
        self.rank.as_ref().map(FieldCoordinate::value)
    }

    pub(super) fn coordinates(&self) -> impl Iterator<Item = &FieldCoordinate<'program>> {
        self.rank.iter().chain(&self.auxiliary)
    }

    pub(super) fn include(&mut self, coordinate: FieldCoordinate<'program>) {
        // The identity spells the formal and the whole resolved chain, so two
        // same-named leaves under different steps stay distinct coordinates.
        if !self
            .coordinates()
            .any(|existing| existing.identity == coordinate.identity)
        {
            self.auxiliary.push(coordinate);
        }
    }

    pub(super) fn comparisons(&self, program: &TypedTrees) -> Vec<Comparison> {
        self.coordinates()
            .flat_map(|coordinate| coordinate.comparisons(program))
            .collect()
    }

    /// Bind only authored projections, never all fields in a record. Two
    /// same-named fields remain independent by parameter and resolved chain.
    pub(super) fn install(
        &mut self,
        program: &'program TypedTrees,
        state: &State,
        root: &State,
        entry_parameters: Option<&[SymbolHandle]>,
        engine: &mut Engine<'_>,
        expressions: &[ExpressionHandle],
    ) -> Option<()> {
        let mut pending = expressions
            .iter()
            .map(|expression| (*expression, 0))
            .collect::<Vec<_>>();
        while let Some((expression, depth)) = pending.pop() {
            if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            let mut visit = |child| pending.push((child, depth + 1));
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) => {
                    if let Some(coordinate) = FieldCoordinate::resolve_projection(
                        program, state, expression,
                    )
                    .or_else(|| {
                        let coordinate =
                            FieldCoordinate::resolve_projection(program, root, expression)?;
                        coordinate.at_arrival(
                            program,
                            RankingRangeState {
                                state,
                                entry_parameters: entry_parameters?,
                            },
                            coordinate.parameter.symbol,
                        )
                    }) {
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
                ExpressionNode::Range(range) => {
                    visit(range.start);
                    visit(range.end);
                }
                _ => {}
            }
        }
        Some(())
    }

    /// Every demanded field of this formal receives the same evaluated actual.
    /// The caller applies this map simultaneously, never sequentially rewriting
    /// one field's actual through another field's new value. `carrier` is the
    /// destination formal whose slot `argument` fills (unused on a root edge),
    /// and `required` names the entry symbols the judgment holds equal at
    /// every arrival. A role claimed by several slots can still be read:
    /// contested required copies stay equal under the edge's own alias
    /// hypotheses, so each claimant's own record is the carrier its actual
    /// must fill and every transport must agree with the first reading.
    /// A contested unrequired claim still produces no substitution rather
    /// than a first/last-wins guess.
    pub(super) fn substitute(
        &self,
        program: &'program TypedTrees,
        state: &State,
        entry_parameters: Option<&[SymbolHandle]>,
        destination: Option<RankingRangeState<'_>>,
        carrier: &'program StateParameter,
        required: &[SymbolHandle],
        engine: &mut Engine<'_>,
        formal: SymbolHandle,
        argument: ExpressionHandle,
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
                        // fields never rewrite a template coordinate.
                        continue;
                    }
                    if coordinate
                        .at_arrival(
                            program,
                            RankingRangeState {
                                state,
                                entry_parameters: entries,
                            },
                            *entry,
                        )
                        .is_none()
                        && !required.contains(entry)
                    {
                        // Contested copies of an unrequired role can never
                        // name which value this coordinate reads. A contested
                        // required role keeps every copy equal under the
                        // edge's alias hypotheses, so the coordinate on this
                        // formal still reads the role's value.
                        return None;
                    }
                    *entry
                }
            };
            if entry_symbol != formal {
                continue;
            }
            // The destination formal's own coordinate walks the actual: a
            // role arriving nested inside the carrier's record -- `pair.left`
            // holding `countdown` -- or as a record this chain reaches
            // partway reads the literal through the chain the destination's
            // invariant names, not the source slot's own spelling. The
            // destination's type likewise decides whether the actual arrives
            // under one borrow: an owned source slot can feed a `&R` formal
            // through `&x`, while the source coordinate's boundary says
            // nothing about the arrival. A root edge re-fills the
            // coordinate's own formal, so its boundary is the arrival's.
            let actual = match destination {
                Some(destination) => {
                    let arrived = match coordinate.at_arrival(program, destination, entry_symbol) {
                        Some(arrived) => arrived,
                        None => {
                            // Several destination slots claim the role: this
                            // slot's own declaration is the carrier the
                            // actual must fill. A claimant that cannot carry
                            // the chain leaves the scalar substitution path
                            // to read its actual.
                            if !required.contains(&entry_symbol) {
                                return None;
                            }
                            match coordinate.for_carrier(program, carrier) {
                                Some(arrived) => arrived,
                                None => continue,
                            }
                        }
                    };
                    arrived.actual(program, state, engine, argument, arrived.borrowed)?
                }
                None => coordinate.actual(program, state, engine, argument, coordinate.borrowed)?,
            };
            if let Some(existing) = substitutions.get(&coordinate.identity) {
                // A contested required role reaches this map once per
                // claimant: every actual must agree with the first reading,
                // the same re-proof the scalar destination arm runs for
                // duplicated required copies.
                if !engine.requires_unsatisfiable
                    && !comparison_proven(engine, BinaryOperator::Equal, existing, &actual)
                {
                    return None;
                }
            } else {
                substitutions.insert(coordinate.identity.clone(), actual);
            }
            matched = true;
        }
        Some(matched)
    }
}
