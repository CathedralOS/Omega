//! Result metadata follows the complete body-derived descriptor association.
//! A stored slice descriptor's length is not a physical field named `len`,
//! and its backing address does not determine that length. Only exact copies
//! from immutable owned slots can share an invocation observation here. Windows,
//! presentations, changed slots and divergent source identities stay unavailable.
//! The finite return equations establish correspondence without importing the
//! recursive guarantee being checked; ranking and loan authority remain separate.
use super::super::super::super::prover::call_guarantees::arithmetic;
use super::super::ExitScalars;
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::ExpressionHandle;
use validation::{CanonicalPlace, ScopedArithmeticValue};

impl ExitScalars<'_, '_> {
    pub(super) fn returned_descriptor_length(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        let entry = self.program.machine_states(self.machine).first()?;
        let member = self.program.expression_table.member_node(expression)?;
        let receiver_type = validation::expression_result_type_reference(
            self.program,
            self.machine,
            entry,
            member.receiver,
        )?;
        if validation::collection_operations::collection_measure_member(
            self.program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(receiver_type),
        ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
        {
            return None;
        }
        let result_path = self.result_receiver_segments(member.receiver)?;
        let output = validation::reference_shell(self.program, receiver_type)?;
        if !matches!(
            self.program.type_reference_table.type_reference(
                self.program
                    .type_reference_table
                    .unconstrained_type_reference(output.referee)
            ),
            typed_trees::types::TypeReferenceNode::Slice { .. }
        ) {
            return None;
        }
        let sources = self
            .call_frames?
            .reference_result_leaf_origins(entry.symbol)?;
        let mut selected = None;
        for source in sources
            .into_iter()
            .filter(|source| source.result_path == result_path)
        {
            if source.source.is_image_storage
                || !source.source.recasts.is_empty()
                || source.source.representation.is_some()
                || source.source.segments.iter().any(|segment| {
                    !matches!(
                        segment,
                        PlaceSegment::Field { .. }
                            | PlaceSegment::Case { .. }
                            | PlaceSegment::FixedIndex { .. }
                    )
                })
            {
                return None;
            }
            let parameter = self
                .program
                .state_parameter_by_symbol(entry, source.source.root)?;
            if parameter.is_mutable
                || parameter.is_const
                || validation::reference_shell(self.program, parameter.type_reference).is_some()
            {
                return None;
            }
            let path = crate::borrow::owner_path_from_place_segments(
                self.program,
                &source.source.segments,
            );
            if !validation::loan_carrier_has_owned_storage(
                self.program,
                entry.symbol,
                0,
                parameter.symbol,
                &path,
            ) {
                return None;
            }
            let source_type = validation::project_type_reference_from_segments(
                self.program,
                parameter.type_reference,
                &source.source.segments,
            )?;
            let input = validation::reference_shell(self.program, source_type)?;
            if !validation::storage_origin_types_match(self.program, input.referee, output.referee)
            {
                return None;
            }
            let place = CanonicalPlace {
                root: PlaceRoot::Symbol(parameter.symbol),
                segments: source.source.segments,
            };
            if selected.as_ref().is_some_and(|prior| *prior != place) {
                return None;
            }
            selected = Some(place);
        }
        Some(arithmetic::length_atom(selected?))
    }
}
