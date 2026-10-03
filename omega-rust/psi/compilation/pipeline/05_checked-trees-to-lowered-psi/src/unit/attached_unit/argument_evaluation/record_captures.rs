//! Materialize captured coordinates at their declaration boundary. The private
//! primitive home keeps its initialized value across selective joins; source
//! locals and later observations never acquire permission to overwrite it.
use super::{Evaluation, LoweringError, StructuralTypeDeclaration, ValueDeclaration};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::emit_direct_expression;
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::prepare_expression::exact_u64_index;
use crate::unit::{allocate_dense, place_id};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeShape,
};

impl Evaluation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn capture_record_coordinates_before(
        &mut self,
        statement: u32,
        types: &[StructuralTypeDeclaration],
        values: &[ValueDeclaration],
        next_place: &mut u64,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
        private_places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        for position in 0..self.byte_record_loans.len() {
            let loan = &self.byte_record_loans[position];
            if loan.statement_index >= statement {
                continue;
            }
            // Unsupported unused loans retain their ordinary use-site refusal.
            let Ok(capture) = &loan.capture else { continue };
            let Some(selector) = &capture.runtime_index else {
                continue;
            };
            if capture.coordinate.is_some() {
                continue;
            }
            let bindings = self
                .scalar_bindings
                .clone()
                .unwrap_or_else(|| {
                    ScalarBindings::new(values.iter().map(|value| value.scalar_type))
                })
                .with_structural_parameters(&self.structural_parameters)
                .with_resolved_structural_observations(
                    &self.structural_fields,
                    &self.structural_cases,
                )
                .with_primitive_storage(&self.primitive_storage)
                .with_structural_locals(&self.structural_locals);
            let selector = match selector {
                checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                    bindings.runtime_index_parameter(*position)?
                }
                checked_trees::CheckedRuntimeIndex::Local { position } => {
                    bindings.runtime_index_local(*position)?
                }
                checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                    bindings.runtime_index_field(field)?
                }
                _ => {
                    return super::unsupported(
                        "record coordinate has no declaration-time scalar source",
                    );
                }
            };
            if capture
                .selector_primitive
                .map(super::terminal_scalar_type)
                .transpose()?
                != Some(selector.scalar_type())
            {
                return super::unsupported("record coordinate changed its captured scalar carrier");
            }
            let selector = exact_u64_index(selector)?;
            let scalar_type = selector.scalar_type();
            if scalar_type != super::terminal_scalar_type(typed_trees::types::PrimitiveType::U64)? {
                return super::unsupported("record coordinate lost its exact index carrier");
            }
            let mut carriers = types.iter().filter(|declaration| {
                declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar_type)
            });
            let structural_type = carriers
                .next()
                .ok_or(LoweringError::Unsupported(
                    "record coordinate has no declared primitive snapshot carrier",
                ))?
                .id;
            if carriers.next().is_some() {
                return super::unsupported(
                    "record coordinate primitive snapshot carrier is ambiguous",
                );
            }
            let value = emit_direct_expression(&selector, values, next_value, operations);
            let producer = operations.allocate();
            let place = place_id(allocate_dense(next_place)?);
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: producer,
                result: OperationResult::Structural(StructuralOperationResult {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                    qualification_establishments: Vec::new(),
                }),
                kind: OperationKind::EstablishPrimitiveLocal { value },
            });
            private_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                },
            });
            std::sync::Arc::make_mut(&mut self.byte_record_loans)[position]
                .capture
                .as_mut()
                .map_err(|error| error.clone())?
                .coordinate = Some(place);
        }
        Ok(())
    }
}
