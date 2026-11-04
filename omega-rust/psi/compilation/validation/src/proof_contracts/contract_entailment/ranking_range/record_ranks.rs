//! A declared measure over a record ranks the subject's complete value, so
//! there is no integer atom to substitute at a recursive arrival. The source
//! rank applies the declaration to the formal's declared reads and its entry
//! snapshot. The arriving rank applies it again to the record this call
//! actually builds: each read is that actual's completed scalar, and the
//! whole-value snapshot is captured at this exact call's argument cut. A
//! source snapshot never stands for the arrival, and an equal constructor
//! elsewhere supplies nothing: both reads and snapshot follow this occurrence.

use super::super::arithmetic_judgment::IntegerApplicationInput;
use super::coordinate::CoordinateKind;
use super::field_coordinates::FieldCoordinates;
use super::fields::FieldCoordinate;
use super::{
    BTreeMap, Engine, ExpressionHandle, Machine, Polynomial, RankStateBindings,
    RankSubjectCoordinate, RankValue, RankingRangeState, State,
};
use symbols::SymbolHandle;

pub(super) struct RecordRank {
    measure: SymbolHandle,
    parameter: SymbolHandle,
    body: ExpressionHandle,
    /// The authored subject at the entry declaration: its formal and path.
    subject: RankSubjectCoordinate,
    /// Every declared read with the field coordinate identity it names at the
    /// judged source state. Arrival substitution maps that identity to the
    /// arriving actual's completed scalar.
    reads: Vec<(crate::DeclaredInputRead, String)>,
}

impl RecordRank {
    /// `Some(None)` for a scalar formal, which keeps the scalar polynomial
    /// route. A record formal must resolve every declared read to an exact
    /// field coordinate of the subject, or the judgment fails closed. The
    /// resolved coordinates join `coordinates`, so they receive the field's
    /// declared facts and its arrival substitution like any authored read.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve<'program>(
        program: &'program dyn crate::CheckingInput,
        root: &'program State,
        state: &'program State,
        entry_parameters: Option<&RankStateBindings>,
        subject: ExpressionHandle,
        parameter: SymbolHandle,
        body: ExpressionHandle,
        coordinates: &mut FieldCoordinates<'program>,
    ) -> Option<Option<Self>> {
        let declaration = program.measures().iter().find(|declaration| {
            declaration
                .parameter
                .as_ref()
                .is_some_and(|formal| formal.symbol == parameter)
        })?;
        let formal = declaration.parameter.as_ref()?;
        if program
            .primitive_type_reference(formal.type_reference)
            .is_some()
        {
            return Some(None);
        }
        let shape = super::declared_input_shape(program, declaration)?;
        let place =
            crate::canonical_place_from_expression_in_state(program, root.symbol, 0, subject)?;
        let subject = RankSubjectCoordinate {
            parameter: place.root.as_symbol()?,
            path: place.segments,
        };
        let mut reads = Vec::with_capacity(shape.reads.len());
        for read in shape.reads {
            // A read inside a nested call is supplied through that call's
            // own actual telescope, which this edge does not capture.
            if !read.calls.is_empty() {
                return None;
            }
            let location = RankSubjectCoordinate {
                parameter: subject.parameter,
                path: subject
                    .path
                    .iter()
                    .chain(&read.coordinate.path)
                    .cloned()
                    .collect(),
            };
            let coordinate =
                FieldCoordinate::at_coordinate(program, root, &location, CoordinateKind::Integer)?
                    .at_entry_arrival(program, state, entry_parameters)?;
            reads.push((read, coordinate.identity.clone()));
            coordinates.include(coordinate);
        }
        Some(Some(Self {
            measure: declaration.symbol,
            parameter,
            body,
            subject,
            reads,
        }))
    }

    /// The source rank: the declaration applied to this state's subject
    /// value, read through the same field atoms its guards and premises use.
    pub(super) fn source(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        entry_parameters: Option<&RankStateBindings>,
        engine: &mut Engine<'_>,
    ) -> Option<RankValue> {
        let current = match entry_parameters {
            Some(entries) => entries.destination(&self.subject)?,
            None => self.subject.clone(),
        };
        let input = IntegerApplicationInput::Record {
            type_reference: super::ranking_coordinate_type(program, state, &current)?,
            snapshot: crate::captured_entry_owned_snapshot(program, machine, state, &current),
            reads: self
                .reads
                .iter()
                .map(|(read, identity)| (read.clone(), Polynomial::atom(identity.clone())))
                .collect(),
        };
        self.applied(program, engine, input)
    }

    /// The arriving rank: the declaration applied to the record this call
    /// passes. `substitutions` holds each read coordinate's completed actual
    /// scalar; a read without one has no arrival value and fails the edge.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn arrival(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        destination: Option<RankingRangeState<'_>>,
        engine: &mut Engine<'_>,
        arguments: &[ExpressionHandle],
        substitutions: &BTreeMap<String, Polynomial>,
    ) -> Option<RankValue> {
        let (arrival_state, target) = match destination {
            Some(destination) => (
                destination.state,
                destination.entry_parameters.destination(&self.subject)?,
            ),
            None => (state, self.subject.clone()),
        };
        let position = program
            .state_parameters(arrival_state)
            .iter()
            .position(|parameter| parameter.symbol == target.parameter)?;
        let input = IntegerApplicationInput::Record {
            type_reference: super::ranking_coordinate_type(program, arrival_state, &target)?,
            snapshot: call_snapshot(program, machine, state, arguments, position, &target.path),
            reads: self
                .reads
                .iter()
                .map(|(read, identity)| Some((read.clone(), substitutions.get(identity)?.clone())))
                .collect::<Option<Vec<_>>>()?,
        };
        self.applied(program, engine, input)
    }

    fn applied(
        &self,
        program: &dyn crate::CheckingInput,
        engine: &mut Engine<'_>,
        input: IntegerApplicationInput,
    ) -> Option<RankValue> {
        (program.symbols.get(self.parameter).parent == self.measure).then_some(())?;
        RankValue::with_input(program, engine, self.parameter, self.body, input)
    }
}

/// The whole-value snapshot of `arguments[position]` at this call's argument
/// cut. Expression handles name occurrences, so the call is the one whose
/// argument list is exactly this edge's. An uncapturable snapshot leaves the
/// application without whole-value identity: only its declared reads form.
fn call_snapshot(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    arguments: &[ExpressionHandle],
    position: usize,
    path: &[facts::PlaceSegment],
) -> Option<crate::CapturedOwnedSnapshot> {
    let statement = super::super::completed_values::expression_statement_index(
        program,
        state,
        *arguments.get(position)?,
    )?;
    let site = crate::collect_call_sites_in_statement(program, machine, state, statement)
        .into_iter()
        .find(|site| crate::call_site_argument_expressions(program, site) == arguments)?;
    crate::captured_call_owned_snapshot(program, machine, state, statement, site, position, path)
}
