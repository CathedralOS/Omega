//! Prepare observed storage and captured coordinates at declaration boundaries.
//! Primitive loans share the original addressable home. Coordinate snapshots
//! stay private across joins; later observations cannot overwrite them.
use super::{Evaluation, LoweringError, StructuralTypeDeclaration, ValueDeclaration};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::emit_direct_expression;
use crate::expression_preparation::bindings::place_observations::LoanedSource;
use crate::expression_preparation::prepare_expression::exact_u64_index;
use terminal_psi::{StructuralPlaceDeclaration, StructuralTypeShape};

impl Evaluation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_storage_before(
&mut self,

        checked: &checked_trees::CheckedTrees,
        state: symbols::SymbolHandle,
        statement: u32,
        types: &[StructuralTypeDeclaration],
        values: &[ValueDeclaration],
        next_place: &mut u64,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
        private_places: &mut Vec<StructuralPlaceDeclaration>,
    ) -> Result<(), LoweringError> {
        if !self
            .computation_address_roots
            .iter()
            .any(|(owner, _)| *owner == state)
        {
            let roots = checked
                .facts
                .values
                .scalar_computations
                .roots
                .iter()
                .filter_map(|(_, root)| {
                    (root.state == state).then_some((root.statement_ordinal, root.root))
                })
                .collect();
            self.computation_address_roots.push((state, roots));
        }
        let roots = self
            .computation_address_roots
            .iter()
            .find(|(owner, _)| *owner == state)
            .ok_or(LoweringError::Unsupported(
                "computed address demand lost its retained state",
            ))?
            .1
            .iter()
            .filter_map(|(ordinal, root)| (*ordinal == statement).then_some(*root))
            .collect::<Vec<_>>();
        for argument in
            crate::expression_preparation::computation_graph::borrowed_arguments_from_roots(
                checked, &roots,
            )?
        {
            self.prepare_borrowed_argument(
                checked,
                state,
                argument,
                values,
                types,
                next_place,
                operations,
                private_places,
            )?;
        }
        for position in 0..self.place_observations.len() {
            let loan = &self.place_observations[position];
            if loan.statement_index >= statement {
                continue;
            }
            // Unsupported unused loans retain their ordinary use-site refusal.
            let Ok(capture) = &loan.capture else { continue };
            // An observation is address demand just like passing a loan. Keep
            // the same original primitive home for both; a local recast does
            // not need to be passed to a machine before it can be observed.
            if let LoanedSource::Local(symbol) = capture.source {
                self.prepare_immutable_local_borrow(
                    checked,
                    state,
                    Some(facts::PlaceRoot::Symbol(symbol)),
                    values,
                    types,
                    next_place,
                    operations,
                    private_places,
                )?;
            }
            let loan = &self.place_observations[position];
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
            let selector = exact_u64_index(selector)?;
            let scalar_type = selector.scalar_type();
            if scalar_type != super::terminal_scalar_type(typed_trees::types::PrimitiveType::U64)? {
                return super::unsupported("record coordinate lost its exact index carrier");
            }
            let mut carriers = types.iter().filter(|declaration| {
                declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar_type)
                    && declaration.scalar_restriction.is_none()
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
            let (place, declaration) = super::super::primitive_locals::push_established_local(
                next_place,
                operations,
                value,
                structural_type,
                types,
                None,
            )?;
            private_places.push(declaration);
            std::sync::Arc::make_mut(&mut self.place_observations)[position]
                .capture
                .as_mut()
                .map_err(|error| error.clone())?
                .coordinate = Some(place);
        }
        Ok(())
    }
}
