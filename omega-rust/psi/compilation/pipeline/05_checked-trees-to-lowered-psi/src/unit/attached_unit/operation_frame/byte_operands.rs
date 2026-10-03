//! Complete a descriptor operand after its destination selectors.
//! Existing storage binds through the ordinary structural namespace; literals
//! and ranges use the same producers as other byte operands. A projected leaf
//! is copied as a descriptor, never as bytes or fabricated owned capacity.
use super::*;
use crate::emission::structural_byte_sequence_store as byte_store;
use checked_trees::{CheckedByteSequenceValue, CheckedUnitStructuralArgumentSourcePlan as Source};

impl OperationFrame<'_, '_> {
    pub(super) fn byte_descriptor_operand(
        &mut self,
        store: &checked_trees::CheckedByteSequenceStorePlan,
    ) -> Result<PlaceId, LoweringError> {
        let CheckedByteSequenceValue::Descriptor { expression, source } =
            store.value.nodes.get(store.value.root)
        else {
            return unsupported("view field store has no descriptor operand");
        };
        if source.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
            return unsupported("stored descriptor requires shared source custody");
        }
        if let Source::ByteSequenceLiteral { bytes } = &source.source {
            return byte_store::emit_literal_view(
                bytes,
                self.structural_types.declarations(),
                self.private_places.literals(),
                self.next_place,
                self.operations,
            );
        }
        if let Source::StructuralResult { binding_ordinal } = source.source {
            if !source.path.is_empty() {
                return unsupported("stored call descriptor changes its completed projection");
            }
            let produced = byte_store::completed_call_result(
                self.operations,
                self.state,
                store.statement_index,
                binding_ordinal,
            )?;
            if produced.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !produced.claims.is_empty()
                || !produced.projected_qualifications.is_empty()
                || !self
                    .structural_types
                    .declarations()
                    .iter()
                    .any(|declaration| {
                        declaration.id == produced.structural_type
                            && declaration.identity == source.type_identity
                            && matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                                        access: None
                                            | Some(terminal_psi::StructuralAccess::SharedBorrow),
                                    }
                                )
                            )
                    })
            {
                return unsupported("stored call descriptor lost shared completed custody");
            }
            return Ok(produced.place);
        }
        let bindings = self
            .evaluation
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| {
                self.dense_assignment_namespace()
                    .with_structural_observations(self.structural_types.declarations())
            })
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_view_locals(&self.evaluation.view_locals)
            .with_byte_record_loans(&self.evaluation.byte_record_loans);
        let structural_type =
            byte_store::existing_descriptor_value_type(self.structural_types.declarations())?;
        if let Source::ByteSequenceSubslice {
            expression: range_expression,
            start,
            end,
            ..
        } = &source.source
        {
            if range_expression != expression {
                return unsupported("stored view range changed its source expression");
            }
            let storage = super::super::view_ranges::storage(
                self.checked,
                self.state,
                store.statement_index,
                source,
                self.structural_types.declarations(),
            )?;
            let whole = storage
                .requires_establishment()
                .then(|| allocate_dense(self.next_place).map(place_id))
                .transpose()?;
            let (range_source, establishment) =
                storage.bind(self.checked, self.state, &bindings, whole)?;
            if let Some(establishment) = establishment {
                let declaration =
                    super::super::view_ranges::establish(&establishment, self.operations);
                self.private_places.temporaries().push(declaration);
            }
            let destination = place_id(allocate_dense(self.next_place)?);
            let result_type = range_source.structural_type;
            let declaration = super::super::view_ranges::emit(
                self.checked,
                ViewRangeSite {
                    state: self.state,
                    statement: store.statement_index,
                    site: checked_trees::CheckedSubsliceSite::Expression {
                        expression: *expression,
                    },
                    expression: *expression,
                    retained: Some((start, end)),
                },
                range_source,
                &source.path,
                result_type,
                destination,
                &bindings,
                self.values,
                self.next_value,
                self.operations,
            )?;
            self.private_places.temporaries().push(declaration);
            return Ok(destination);
        }
        let (machine, state) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let place = bindings.shared_structural_place(
            source,
            machine,
            self.checked.state_parameters(state),
        )?;
        let place = self.evaluation.current_structural_place(place);
        if source.path.is_empty() {
            return Ok(place);
        }
        let fields = crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
            &self.evaluation.structural_parameters,
            self.structural_types.declarations(),
        );
        let mut selectors = parameters::mint_runtime_index_selectors(
            std::slice::from_ref(source),
            &fields,
            self.values.as_slice(),
            self.next_value,
            self.operations,
        )?
        .into_iter();
        let path = crate::expression_preparation::bindings::structural_paths::lower_argument_path(
            &source.path,
            Some(self.values.as_slice()),
            &mut selectors,
            Some(&mut *self.calls),
        )?;
        let destination = place_id(allocate_dense(self.next_place)?);
        let producer = self.operations.allocate();
        self.operations.push(Operation {
            id: producer,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: destination,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::StructuralLeafCopy {
                source: place,
                path,
            },
        });
        self.private_places
            .temporaries()
            .push(StructuralPlaceDeclaration {
                id: destination,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                },
            });
        Ok(destination)
    }
}
