//! Capture every declared read from one authenticated nominal subject. The
//! source subject remains an identity; no integer atom stands for its record.

use super::EntryInputs;
use typed_trees::{
    expression::ExpressionHandle, machine::Machine, state::State, types::TypeReferenceHandle,
};
use validation::{
    CapturedRankApplicationInput, DeclaredInputShape, RankSubjectCoordinate,
    ScopedArithmeticBinding, ScopedArithmeticExpression,
};

pub(super) struct RecordInput {
    pub subject: ExpressionHandle,
    pub owner: symbols::SymbolHandle,
    pub body: ExpressionHandle,
    pub input_type: TypeReferenceHandle,
    pub root: RankSubjectCoordinate,
    pub shape: DeclaredInputShape,
}

impl RecordInput {
    pub(super) fn prepare(
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        state: &State,
        subject: ExpressionHandle,
        parameter: symbols::SymbolHandle,
        body: ExpressionHandle,
    ) -> Option<Self> {
        let declaration = program.measures().iter().find(|declaration| {
            declaration
                .parameter
                .as_ref()
                .is_some_and(|formal| formal.symbol == parameter)
        })?;
        if program
            .expression_table
            .expression_handles(declaration.body)
            != [body]
            || !validation::place_has_builtin_coordinates(program, machine, Some(state), subject)
        {
            return None;
        }
        let input_type =
            validation::declared_place_type_raw(program, machine, Some(state), subject)?;
        let shape = validation::declared_input_shape(program, declaration)?;
        if !validation::declared_input_type_matches(program, input_type, shape.type_reference) {
            return None;
        }
        let root = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            subject,
        )?;
        Some(Self {
            subject,
            owner: declaration.symbol,
            body,
            input_type,
            root: RankSubjectCoordinate {
                parameter: root.root.as_symbol()?,
                path: root.segments,
            },
            shape,
        })
    }

    pub(super) fn paths(&self) -> Vec<Vec<facts::PlaceSegment>> {
        self.shape
            .reads
            .iter()
            .map(|read| {
                self.root
                    .path
                    .iter()
                    .chain(&read.coordinate.path)
                    .cloned()
                    .collect()
            })
            .collect()
    }

    pub(super) fn capture(
        &self,
        prepared: &EntryInputs<'_>,
    ) -> Option<CapturedRankApplicationInput> {
        let mut bindings = Vec::new();
        if let Some(snapshot) = validation::captured_entry_owned_snapshot(
            prepared.values.program(),
            prepared.values.machine(),
            prepared.values.state(),
            &self.root,
        ) {
            bindings.push(ScopedArithmeticBinding {
                binder: validation::ScopedArithmeticBinder::Symbol(self.shape.parameter),
                value: validation::ScopedArithmeticValue::OwnedSnapshot(snapshot),
            });
        }
        for (read, path) in self.shape.reads.iter().zip(self.paths()) {
            let coordinate = prepared.coordinates.iter().find(|coordinate| {
                coordinate.parameter == self.root.parameter
                    && coordinate.primitive == read.primitive_type
                    && coordinate.observation == validation::RankPremiseObservation::Value
                    && prepared
                        .values
                        .same_scalar_projection(&coordinate.path, &path)
            })?;
            bindings.push(ScopedArithmeticBinding {
                binder: validation::ScopedArithmeticBinder::Coordinate(read.coordinate.clone()),
                value: coordinate.entry_value.clone(),
            });
        }
        Some(CapturedRankApplicationInput {
            input_type: self.input_type,
            expression: ScopedArithmeticExpression {
                owner: self.owner,
                expression: self.body,
                bindings,
            },
        })
    }
}
