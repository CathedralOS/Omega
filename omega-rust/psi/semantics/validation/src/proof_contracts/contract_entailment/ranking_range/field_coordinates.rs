//! Exact field projections used by one ranking judgment. Root-template
//! projections and current-state projections share an atom only through a
//! unique, nominally checked arrival role. Record ancestry alone never proves
//! copy equality.
use super::{
    BTreeMap, Comparison, Engine, ExpressionHandle, ExpressionNode, Polynomial, RankingRangeState,
    State, TypedTrees, fields,
};
use fields::FieldCoordinate;
use symbols::SymbolHandle;

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
    /// one field's actual through another field's new value.
    pub(super) fn substitute(
        &self,
        program: &'program TypedTrees,
        state: &State,
        entry_parameters: Option<&[SymbolHandle]>,
        destination: Option<RankingRangeState<'_>>,
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
            if let Some(destination) = destination {
                coordinate.at_arrival(program, destination, entry_symbol)?;
            }
            let actual = coordinate.actual(program, state, engine, argument)?;
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
