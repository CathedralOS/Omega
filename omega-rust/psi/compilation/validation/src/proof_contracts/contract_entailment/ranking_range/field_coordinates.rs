//! Exact field projections used by one ranking judgment. Root-template
//! projections and current-state projections share an atom only through a
//! unique, nominally checked arrival role. Record ancestry alone never proves
//! copy equality. Each authored member projection names one field coordinate;
//! the shared [`CoordinateSet`] walk installs it and the arrival substitution
//! reads the scalar actual this family computes.
use super::coordinate::{Coordinate, CoordinateFamily, CoordinateSet};
use super::{
    Comparison, Engine, ExpressionHandle, Machine, Polynomial, RankSubjectCoordinate, State,
};

/// The field-rank coordinate family: coordinates resolved from the authored
/// member projection itself and read as ordinary scalar actuals.
pub(super) struct FieldFamily;

pub(super) type FieldCoordinates<'program> = CoordinateSet<'program, FieldFamily>;

impl<'program> CoordinateFamily<'program> for FieldFamily {
    fn resolve(
        program: &'program dyn crate::CheckingInput,
        _machine: &Machine,
        scope: &State,
        expression: ExpressionHandle,
    ) -> Option<Coordinate<'program>> {
        Coordinate::resolve_field_projection(program, scope, expression)
    }

    fn demanded(demanded: &[RankSubjectCoordinate], entry: &RankSubjectCoordinate) -> bool {
        demanded.contains(entry)
    }

    fn arrived_actual(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        destination_state: &State,
        destination: &RankSubjectCoordinate,
        engine: &mut Engine<'_>,
        argument: ExpressionHandle,
        _bindings: &[(symbols::SymbolHandle, String)],
    ) -> Option<Polynomial> {
        super::arrival_scalar_actual(
            program,
            machine,
            state,
            destination_state,
            destination,
            engine,
            argument,
        )
    }

    fn comparisons(
        program: &dyn crate::CheckingInput,
        coordinate: &Coordinate<'program>,
    ) -> Vec<Comparison> {
        coordinate.field_comparisons(program)
    }
}

/// The natural field coordinates `subjects` name: each authored member chain
/// resolves against `authored`'s formals, then re-resolves onto `site`'s
/// unique carrier of that role. A bare subject can still name a coordinate:
/// its entry role may arrive packed inside a record carrier's unique natural
/// leaf. `site` equals `authored` at an entry judgment; a chain that cannot
/// arrive fails the judgment rather than naming a guessed carrier.
pub(super) fn include_subject_coordinates<'program>(
    program: &'program dyn crate::CheckingInput,
    authored: &'program State,
    site: &'program State,
    entry_parameters: Option<&super::RankStateBindings>,
    subjects: &[ExpressionHandle],
    coordinates: &mut FieldCoordinates<'program>,
) -> Option<()> {
    for subject in subjects {
        if let Some(coordinate) = Coordinate::resolve_field_projection(program, authored, *subject)
        {
            let coordinate = coordinate.at_entry_arrival(program, site, entry_parameters)?;
            coordinates.include(coordinate);
            continue;
        }
        if let Some(entries) = entry_parameters
            && let Some(coordinate) =
                super::carried_subject_coordinate(program, authored, site, entries, *subject)
        {
            coordinates.include(coordinate);
        }
    }
    Some(())
}
