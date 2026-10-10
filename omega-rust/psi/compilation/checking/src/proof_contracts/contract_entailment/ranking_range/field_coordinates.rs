//! Exact field projections used by one ranking judgment. Root-template
//! projections and current-state projections share an atom only through a
//! unique, nominally checked arrival role. Record ancestry alone never proves
//! copy equality.
use super::{
    BTreeMap, Comparison, Engine, ExpressionHandle, Machine, Polynomial, RankingRangeState, State,
    fields, projections,
};
use fields::FieldCoordinate;
use symbols::SymbolHandle;
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

    pub(super) fn comparisons(&self, program: &dyn crate::CheckingInput) -> Vec<Comparison> {
        self.coordinates()
            .flat_map(|coordinate| coordinate.field_comparisons(program))
            .collect()
    }

    /// Bind only authored projections, never all fields in a record. Two
    /// same-named fields remain independent by parameter and resolved chain.
    pub(super) fn install(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        state: &State,
        root: &State,
        entry_parameters: Option<&super::RankStateBindings>,
        engine: &mut Engine<'_>,
        expressions: &[ExpressionHandle],
    ) -> Option<()> {
        projections::for_each_projection_expression(
            program,
            expressions,
            |member, expression, visit| {
                if let Some(coordinate) = FieldCoordinate::resolve_field_projection(
                    program, state, expression,
                )
                .or_else(|| {
                    let coordinate =
                        FieldCoordinate::resolve_field_projection(program, root, expression)?;
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
                Some(())
            },
        )
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
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        entry_parameters: Option<&super::RankStateBindings>,
        destination: Option<RankingRangeState<'_>>,
        carrier: &'program StateParameter,
        _required: &[SymbolHandle],
        engine: &mut Engine<'_>,
        _formal: SymbolHandle,
        argument: ExpressionHandle,
        substitutions: &mut BTreeMap<String, Polynomial>,
    ) -> Option<bool> {
        let mut matched = false;
        for coordinate in self.coordinates() {
            let entry = match entry_parameters {
                None => coordinate.location(program),
                Some(entries) => {
                    match super::coordinate::arrived_entry_coordinate(program, entries, coordinate)
                    {
                        Some(entry) => entry,
                        None => continue,
                    }
                }
            };
            let (arrival_state, target) = match destination {
                Some(destination) => (
                    destination.state,
                    destination.entry_parameters.destination(&entry)?,
                ),
                None => (state, entry),
            };
            if target.parameter != carrier.symbol {
                continue;
            }
            let actual = super::arrival_scalar_actual(
                program,
                machine,
                state,
                arrival_state,
                &target,
                engine,
                argument,
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
