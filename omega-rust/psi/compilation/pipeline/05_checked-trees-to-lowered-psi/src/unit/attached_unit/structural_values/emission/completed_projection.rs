//! Completed structural projections use the same place frontiers as call actuals.
//! Keep a descriptor's endpoint before its selector can replace that descriptor;
//! selector values stay in the active scalar roster across private continuations.

use super::{Emission, LoweringError, StructuralAccess, StructuralArgument};
use crate::emission::completed_place::Frontier;
use crate::expression_preparation::bindings::structural_paths::{
    PreparedStructuralPathSegment, emit_captured_argument_path,
};
use semantic_vocabulary::PlaceId;

impl Emission<'_, '_, '_> {
    pub(super) fn complete_projection_source(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: checked_trees::expression::ExpressionHandle,
        source: &super::ProjectionSource,
        evaluations: arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
        endpoint: typed_trees::type_identity::TypeIdentityId,
        access: StructuralAccess,
    ) -> Result<StructuralArgument, LoweringError> {
        let root = super::lookup_type_id(self.type_ids, &source.root_type_identity)?;
        // The checked producer owns the root coordinate. A stored reference
        // can present logical backing without granting ownership of it; a
        // whole descriptor or saved physical projection stays Published.
        // Leaf copy/move and endpoint completion remain separate judgments.
        let mut suffix =
            crate::expression_preparation::bindings::structural_paths::prepare_projection_path(
                identities,
                self.checked,
                &source.path,
                if source.logical_root_type_identity.is_valid() {
                    crate::expression_preparation::bindings::structural_paths::ProjectionRoot::Borrowed {
                        published: root,
                        logical: source.logical_root_type_identity,
                        // projected_argument_path deliberately omits root-shell
                        // steps for symbol-rooted logical storage projections.
                        spelled_reference_hops: 0,
                    }
                } else {
                    crate::expression_preparation::bindings::structural_paths::ProjectionRoot::Published(root)
                },
                endpoint,
                access,
                self.structural_types,
            )?;
        crate::expression_preparation::bindings::structural_paths::retain_selector_evaluations(
            &source.path,
            evaluations,
            &self.checked.facts.values.scalar_computations.operands,
            &mut suffix,
        )?;
        let mut prepared = source
            .root_path
            .iter()
            .cloned()
            .map(PreparedStructuralPathSegment::Static)
            .collect::<Vec<_>>();
        prepared.extend(suffix);
        self.complete_projection(identities, expression, source.root, prepared, access)
    }

    pub(super) fn complete_projection(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: checked_trees::expression::ExpressionHandle,
        mut place: PlaceId,
        mut prepared: Vec<PreparedStructuralPathSegment>,
        access: StructuralAccess,
    ) -> Result<StructuralArgument, LoweringError> {
        let root = self.projection_root_type(place)?;
        let frontiers = crate::emission::completed_place::plan(
            self.structural_types,
            root,
            &prepared,
            false,
            access,
            crate::emission::completed_place::EndpointUse::Storage,
        )?;
        // This producer consumes its final endpoint immediately: the copy or
        // reference establishment follows before another actual can execute.
        // Only addresses crossing a later selector need private transport.
        let last_selector = frontiers
            .iter()
            .rposition(|frontier| matches!(frontier, Frontier::Selector { .. }));
        let original = prepared.clone();
        let source_count = self.values.len();
        let mut removed = 0;
        let mut consumed = 0;
        for (frontier_position, frontier) in frontiers.into_iter().enumerate() {
            match frontier {
                Frontier::Selector { position } => {
                    self.projection_selector(
                        identities,
                        expression,
                        &original[position],
                        source_count,
                    )?;
                }
                Frontier::Capture { prefix, capture } => {
                    if last_selector.is_none_or(|last| frontier_position > last) {
                        continue;
                    }
                    let length = prefix
                        .checked_sub(removed)
                        .ok_or(LoweringError::Unsupported(
                            "projection frontier precedes its completed address",
                        ))?;
                    let selected = prepared.get(..length).ok_or(LoweringError::Unsupported(
                        "projection frontier exceeds its retained path",
                    ))?;
                    let selector_count = selected
                        .iter()
                        .filter(|segment| evaluated_selector(segment))
                        .count();
                    let mut selectors = self.values[source_count + consumed..]
                        .iter()
                        .map(|value| value.id);
                    let path = emit_captured_argument_path(
                        selected,
                        place,
                        Some(self.values.as_slice()),
                        &mut selectors,
                        Some(&mut *self.calls),
                        Some((&mut *self.next_value, &mut *self.operations)),
                    )?;
                    let source = StructuralArgument {
                        place,
                        path,
                        access,
                    };
                    let (qualifications, projected) =
                        crate::unit::attached_unit::operation_frame::completed_arguments::selected_memberships(
                            &source, &[], self.evaluation, self.operations,
                        )?;
                    let (structural_type, qualifications, projected) =
                        crate::emission::completed_place::present_capture(
                            &capture,
                            access,
                            self.structural_types,
                            self.domain_declarations,
                            qualifications,
                            projected,
                        )?;
                    place = crate::emission::completed_address::capture(
                        source.place,
                        source.path,
                        structural_type,
                        access,
                        qualifications,
                        projected,
                        self.evaluation,
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.environment.next_place,
                        self.environment.private_places,
                        self.operations,
                    )?;
                    prepared.drain(..length);
                    removed = prefix;
                    consumed += selector_count;
                }
                Frontier::Value { .. } => {
                    return super::unsupported(
                        "projection address evaluation cannot manufacture a value copy",
                    );
                }
            }
        }
        let mut selectors = self.values[source_count + consumed..]
            .iter()
            .map(|value| value.id);
        let path = emit_captured_argument_path(
            &prepared,
            place,
            Some(self.values.as_slice()),
            &mut selectors,
            Some(&mut *self.calls),
            Some((&mut *self.next_value, &mut *self.operations)),
        )?;
        self.values.truncate(source_count);
        Ok(StructuralArgument {
            place,
            path,
            access,
        })
    }

    /// Geometry comes from established source homes and producer declarations,
    /// including private places that have no operation-result declaration.
    /// This lookup grants neither storage liveness nor projection permission.
    pub(super) fn projection_root_type(
        &self,
        place: PlaceId,
    ) -> Result<super::StructuralTypeId, LoweringError> {
        self.evaluation
            .structural_root_parameters()
            .find(|row| row.place == place)
            .map(|row| row.structural_type)
            .or_else(|| {
                self.evaluation
                    .completed_bindings(
                        self.operations,
                        self.environment.private_places.iter().cloned(),
                    )
                    .structural_place_type(place)
            })
            .ok_or(LoweringError::Unsupported(
                "projection lost its physical root declaration",
            ))
    }

    fn projection_selector(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: checked_trees::expression::ExpressionHandle,
        segment: &PreparedStructuralPathSegment,
        source_count: usize,
    ) -> Result<(), LoweringError> {
        match segment {
            PreparedStructuralPathSegment::ComputedIndex(computation)
            | PreparedStructuralPathSegment::EvaluatedFixedIndex { computation, .. } => {
                let mut roles =
                    self.checked
                        .facts
                        .values
                        .scalar_computations
                        .roots_for_statement(self.state, self.statement)
                        .into_iter()
                        .filter_map(|root| {
                            matches!(root.role,
                        checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                            expression: owner, ..
                        } if owner == expression && root.root == *computation
                    ).then_some(root.role)
                        });
                let role = roles.next().ok_or(LoweringError::Unsupported(
                    "projection selector lost its exact authored computation root",
                ))?;
                if roles.next().is_some() {
                    return super::unsupported("projection selector has multiple authored roots");
                }
                let value = self.scalar(identities, role, *computation, source_count)?;
                self.values.push(value);
            }
            PreparedStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::StructuralField(_)
                | checked_trees::CheckedRuntimeIndex::StorageRead { .. },
            ) => {
                let bindings = self.evaluation.contextual_bindings();
                let observation = match segment {
                    PreparedStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::StorageRead { symbol },
                    ) => bindings.runtime_index_storage(*symbol)?,
                    PreparedStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::StructuralField(field),
                    ) => bindings.runtime_index_field(identities, field)?,
                    _ => unreachable!("matched runtime selector"),
                };
                if !matches!(observation.scalar_type(), semantic_vocabulary::ScalarType::Integer(integer)
                    if !integer.is_address())
                {
                    return super::unsupported("projection selector is not an integer read");
                }
                let id = crate::emission::operation_emission::emit_direct_expression(
                    &observation,
                    self.values.as_slice(),
                    self.next_value,
                    self.operations,
                );
                let value = self
                    .operations
                    .iter()
                    .rev()
                    .filter_map(|operation| operation.result.scalar())
                    .find(|value| value.id == id)
                    .or_else(|| self.values.iter().find(|value| value.id == id).copied())
                    .ok_or(LoweringError::Unsupported(
                        "projection selector lost its evaluated value",
                    ))?;
                self.values.push(value);
            }
            _ => return super::unsupported("projection frontier has no executable selector"),
        }
        Ok(())
    }
}

fn evaluated_selector(segment: &PreparedStructuralPathSegment) -> bool {
    matches!(
        segment,
        PreparedStructuralPathSegment::ComputedIndex(_)
            | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
            | PreparedStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::StructuralField(_)
                    | checked_trees::CheckedRuntimeIndex::StorageRead { .. }
            )
    )
}
