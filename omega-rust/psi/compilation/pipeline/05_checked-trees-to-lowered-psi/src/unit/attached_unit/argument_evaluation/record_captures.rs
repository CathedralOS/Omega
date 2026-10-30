//! Prepare observed storage and captured coordinates at declaration boundaries.
//! Primitive loans share the original addressable home. Coordinate snapshots
//! stay private across joins; later observations cannot overwrite them.
use super::{Evaluation, LoweringError, StructuralTypeDeclaration, ValueDeclaration};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::emit_direct_expression;
use crate::expression_preparation::prepare_expression::exact_u64_index;
use terminal_psi::{StructuralPlaceDeclaration, StructuralTypeShape};

impl Evaluation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_storage_before(
        &mut self,

        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        state: symbols::SymbolHandle,
        statement: u32,
        types: &[StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
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
        let borrowed =
            crate::expression_preparation::computation_graph::borrowed_arguments_from_roots(
                checked, &roots,
            )?;
        for argument in borrowed {
            self.prepare_borrowed_argument(
                identities,
                checked,
                state,
                argument,
                values,
                types,
                domains,
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
            // A silent loan names its checked backing, even when its scalar
            // observation is unsupported. Locating that home grants no store.
            if let checked_trees::CheckedPlaceLoanRoot::Local { symbol } = loan.loan.root {
                self.prepare_immutable_local_borrow(
                    checked,
                    state,
                    Some(facts::PlaceRoot::Symbol(symbol)),
                    values,
                    types,
                    domains,
                    next_place,
                    operations,
                    private_places,
                )?;
            }
            let loan = &self.place_observations[position];
            let selectors = loan
                .loan
                .path
                .iter()
                .filter_map(|segment| match segment {
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
                        Some(selector)
                    }
                    _ => None,
                })
                .cloned()
                .collect::<Vec<_>>();
            if selectors.is_empty() || loan.selectors.len() == selectors.len() {
                continue;
            }
            if !loan.selectors.is_empty() {
                return super::unsupported("place loan has an incomplete selector capture");
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
            let mut coordinates = Vec::with_capacity(selectors.len());
            for selector in &selectors {
                let selector = match selector {
                    checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                        bindings.runtime_index_storage(*symbol)?
                    }
                    checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                        bindings.runtime_index_parameter(*position)?
                    }
                    checked_trees::CheckedRuntimeIndex::Local { position } => {
                        bindings.runtime_index_local(*position)?
                    }
                    checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                        bindings.runtime_index_field(identities, field)?
                    }
                    _ => {
                        return super::unsupported(
                            "loan coordinate has no declaration-time scalar source",
                        );
                    }
                };
                let selector = exact_u64_index(selector)?;
                let scalar_type = selector.scalar_type();
                if scalar_type
                    != super::terminal_scalar_type(typed_trees::types::PrimitiveType::U64)?
                {
                    return super::unsupported("loan coordinate lost its exact index carrier");
                }
                let mut carriers = types.iter().filter(|declaration| {
                    declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar_type)
                        && declaration.scalar_restriction.is_none()
                });
                let structural_type = carriers
                    .next()
                    .ok_or(LoweringError::Unsupported(
                        "loan coordinate has no declared primitive snapshot carrier",
                    ))?
                    .id;
                if carriers.next().is_some() {
                    return super::unsupported(
                        "loan coordinate primitive snapshot carrier is ambiguous",
                    );
                }
                let value = emit_direct_expression(&selector, values, next_value, operations);
                let value =
                    super::super::primitive_locals::completed_scalar(value, values, operations)?;
                let (place, declaration) = super::super::primitive_locals::push_established_local(
                    next_place,
                    operations,
                    value,
                    structural_type,
                    super::super::primitive_locals::PrimitiveStorageCatalog {
                        types,
                        domains,
                        scalars: self.shared_qualifications.catalog(),
                    },
                    None,
                )?;
                private_places.push(declaration);
                coordinates.push(place);
            }
            let loan = &mut std::sync::Arc::make_mut(&mut self.place_observations)[position];
            if let Ok(capture) = &mut loan.capture
                && capture.runtime_index.is_some()
            {
                capture.coordinate = coordinates.first().copied();
            }
            loan.selectors = coordinates;
        }
        self.scalar_bindings = self
            .scalar_bindings
            .clone()
            .with_place_observations(&self.place_observations);
        Ok(())
    }
}
