//! Narrow an evaluated descriptor with the ordinary range operation. The
//! source value completes before endpoints; no source symbol or fake storage
//! loan replaces the descriptor's exact contents and backing.
use super::{
    Emission, LoweringError, PlaceId, StructuralTypeShape, ValueContinuation, allocate_dense,
    lookup_type_id, place_id, unsupported,
};
use crate::unit::attached_unit::view_ranges;
use checked_trees::{CheckedCallScalarArgument, CheckedStructuralValueHandle, CheckedSubsliceSite};

impl Emission<'_, '_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn subslice_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: CheckedStructuralValueHandle,
        start: &Option<CheckedCallScalarArgument>,
        end: &Option<CheckedCallScalarArgument>,
        end_inclusive: bool,
        site: CheckedSubsliceSite,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let node = self.structural_value_node(source);
        let source_type = lookup_type_id(
            self.type_ids,
            &validation::stored_structural_type_identity(
                &self.checked.typed,
                node.type_reference,
                &[],
                &[],
            ),
        )?;
        let family =
            match terminal_psi::find_structural_type(self.structural_types, self.structural_type)
                .map(|declaration| &declaration.shape)
            {
                Some(StructuralTypeShape::ByteSequence(_)) => {
                    crate::emission::operation_emission::view_subslice::ViewFamily::Bytes
                }
                Some(StructuralTypeShape::ElementView { .. }) => {
                    crate::emission::operation_emission::view_subslice::ViewFamily::Elements
                }
                _ => return unsupported("subslice value has no view descriptor carrier"),
            };
        let completed =
            self.establish_computed_operand(identities, source, source_type, node.type_reference)?;
        crate::expression_preparation::bindings::retain_completed_structural_homes(
            &mut self.evaluation.structural_locals,
            &[
                crate::expression_preparation::bindings::StructuralLocalBinding {
                    result_binding_ordinal: None,
                    root: facts::PlaceRoot::Expression(node.expression),
                    argument: completed.argument.clone(),
                    structural_type: source_type,
                },
            ],
        )?;
        if !completed.claims.is_empty() {
            return unsupported("view descriptor unexpectedly carries an owned content claim");
        }
        let access =
            match validation::slice_view_shell(&self.checked.typed, self.type_reference, &[])
                .map(|view| view.access)
            {
                Some(language_core::ReferenceAccess::Shared) => {
                    terminal_psi::StructuralAccess::SharedBorrow
                }
                Some(language_core::ReferenceAccess::Mutable) => {
                    terminal_psi::StructuralAccess::MutableBorrow
                }
                _ => return unsupported("subslice value requires readable view access"),
            };
        if completed.argument.access == terminal_psi::StructuralAccess::SharedBorrow
            && access == terminal_psi::StructuralAccess::MutableBorrow
        {
            return unsupported("subslice cannot amplify its completed source access");
        }
        let place = place_id(allocate_dense(self.environment.next_place)?);
        let declaration = view_ranges::emit(
            identities,
            self.checked,
            view_ranges::ViewRangeSite {
                state: self.state,
                statement: self.statement,
                range: view_ranges::RangeEndpoints {
                    start,
                    end,
                    end_inclusive,
                    site,
                },
            },
            view_ranges::ViewRangeSource {
                place: completed.argument.place,
                path: completed.argument.path.into_iter().map(crate::emission::operation_emission::expressions::LoweredElementPathSegment::Static).collect(),
                structural_type: source_type,
                family,
                access,
            },
            self.structural_type,
            place,
            self.evaluation,
            self.values.len(),
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
            self.environment,
        )?;
        self.environment.private_places.push(declaration);
        self.completed(place, continuation)
    }
}
