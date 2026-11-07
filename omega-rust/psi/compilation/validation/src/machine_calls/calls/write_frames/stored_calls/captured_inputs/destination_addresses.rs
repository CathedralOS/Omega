//! An assignment captures storage, not the value later held in its selectors.
//! Reference receivers use the same completed suppliers as call arguments.
//! Each projection appends only its checked owned span; an interior pointer
//! must have its own captured descriptor before the next span can be selected.

use super::*;
use crate::machine_calls::calls::write_frames::{
    FramePathPrecision, FramePlaceOrigin, FrameSourcePlace,
};

pub(in crate::machine_calls::calls::write_frames::stored_calls) fn assignment_addresses(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    target: &SlotArgumentCapture,
    receiving: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
) -> Result<Vec<FramePlaceOrigin>, &'static str> {
    let query = Inputs {
        program,
        machine,
        captured: target,
        symbols,
        entry: &target.context,
    };
    query.address(target.expression, receiving, &mut target.inference.clone())
}

impl Inputs<'_, '_> {
    fn address(
        &self,
        expression: ExpressionHandle,
        receiving: TypeReferenceHandle,
        inference: &mut FrameInference,
    ) -> Result<Vec<FramePlaceOrigin>, &'static str> {
        self.captured
            .observation(expression)
            .ok_or("destination has no unique completed operand capture")?;
        let (state, _, _) = caller_aliases::caller_statement_owner(
            self.program,
            self.machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )
        .ok_or("destination operand has no exact statement owner")?;
        let actual =
            crate::expression_result_type_reference(self.program, self.machine, state, expression)
                .ok_or("destination operand has no checked result type")?;
        if self.program.normalized_type_identity(actual)
            != self.program.normalized_type_identity(receiving)
        {
            let shell = crate::reference_shell(self.program, actual)
                .ok_or("destination result is neither its receiving cell nor a reference to it")?;
            if self.program.normalized_type_identity(shell.referee)
                != self.program.normalized_type_identity(receiving)
            {
                return Err("destination reference does not select the receiving storage type");
            }
            let (sources, _) = partition_sources(
                self.reference(expression, actual, false, inference)
                    .ok_or("destination descriptor has no complete captured suppliers")?,
            );
            let origins = sources
                .into_iter()
                .map(|source| match source {
                    ReferenceSource::Captured(observed)
                        if observed.source.lifetime_access.is_none() =>
                    {
                        Some(observed.source.origin)
                    }
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
                .ok_or("destination descriptor retains an uncaptured spatial supplier")?;
            return if origins.is_empty() {
                Err("destination descriptor has no spatial backing")
            } else {
                Ok(origins)
            };
        }
        if let Some((receiver, relative)) = self.projection(expression) {
            crate::place_has_builtin_coordinates(
                self.program,
                self.machine,
                Some(state),
                expression,
            )
            .then_some(())
            .ok_or("destination projection has no selected builtin coordinates")?;
            let receiver_type = crate::expression_result_type_reference(
                self.program,
                self.machine,
                state,
                receiver,
            )
            .ok_or("destination receiver has no checked result type")?;
            let storage = super::super::super::reference_subjects::carrier_storage_type(
                self.program,
                receiver_type,
            )
            .ok_or("destination receiver has no storage carrier")?;
            let mut cursor = stored_origins::StorageProjectionCursor::new(self.program, storage);
            for segment in &relative {
                cursor.advance(segment)
                    .ok_or("destination projection crosses an uncaptured descriptor or unsupported coordinate")?;
            }
            if self.program.normalized_type_identity(
                cursor
                    .finish()
                    .ok_or("destination projection has no complete owned endpoint")?,
            ) != self.program.normalized_type_identity(receiving)
            {
                return Err("destination owned projection changes its receiving storage type");
            }
            let mut origins = self.address(receiver, storage, inference)?;
            for origin in &mut origins {
                if !origin.source.root.is_valid() || !origin.source.builtin_coordinates {
                    return Err("destination projection has no declaration-backed coordinates");
                }
                origin.source = origin.source.append_segments(&relative);
                let (path, precision) = super::super::super::place_paths::coordinate_storage_path(
                    self.program,
                    self.program.symbols.name(origin.source.root),
                    &origin.source.segments,
                );
                origin.path = path;
                if precision == FramePathPrecision::CollectionCoarse {
                    origin.precision = precision;
                }
            }
            return Ok(origins);
        }
        // A final reference-valued cell is its descriptor home here. Loading
        // its value would redirect a descriptor replacement to its referent.
        let place = crate::canonical_place_from_expression(self.program, expression)
            .ok_or("destination has no canonical storage place")?;
        let root = place
            .root
            .as_symbol()
            .ok_or("destination has no declared storage root")?;
        (place.segments.is_empty() && root.is_valid())
            .then_some(())
            .ok_or("destination retains an unresolved storage projection")?;
        let reference = crate::declaration_type_reference(self.program, root)
            .ok_or("destination root has no declared storage type")?;
        if crate::reference_shell(self.program, reference).is_some() {
            return Err("destination reference binding has no owned storage capture");
        }
        (self.program.normalized_type_identity(reference)
            == self.program.normalized_type_identity(receiving))
        .then_some(())
        .ok_or("destination root changes its receiving storage type")?;
        Ok(vec![FramePlaceOrigin {
            path: self.program.symbols.name(root).to_owned(),
            precision: FramePathPrecision::Exact,
            source: FrameSourcePlace {
                root,
                builtin_coordinates: true,
                ..Default::default()
            },
        }])
    }
}
