//! Prepare exact rank input values before selecting a ranking witness.
//! Typed coordinates, entry selector bounds and value snapshots belong here;
//! authored/inferred witness policies and invariant regimes remain consumers.

mod coordinates;
mod entry;
use crate::checks::current_scalar_values::CurrentScalarValues;
pub(super) use coordinates::Coordinate;
pub(super) use entry::{
    prove as prove_integer_entry, prove_with_formation as prove_integer_entry_with_formation,
};
use typed_trees::{machine::Machine, state::State};

pub(super) struct EntryInputs<'program> {
    pub values: CurrentScalarValues<'program>,
    pub coordinates: Vec<Coordinate>,
}

pub(super) fn at_entry<'program>(
    program: &'program dyn validation::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    frames: Option<&validation::CallFrameResolver<'_>>,
    paths: &[Vec<facts::PlaceSegment>],
) -> Option<EntryInputs<'program>> {
    let execution = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    // A declaration has an entry observation even when its body is empty.
    let first = execution.indices().first().copied().unwrap_or(0);
    let values = CurrentScalarValues::at_observing_projections(
        program,
        machine,
        state,
        first,
        Default::default(),
        frames,
        paths,
    )?;
    let coordinates = coordinates::coordinates(program, state, &values);
    Some(EntryInputs {
        values,
        coordinates,
    })
}
