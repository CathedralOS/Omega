//! Declaration inputs have one binding route in whole expressions and integer
//! operands. Prefer a completed capture: its backing may already have changed.
//! Otherwise observe the exact admitted destination under the source formal's
//! carrier; arithmetic policy comes from that same formal type, never a hint.

use super::Builder;
use checked_trees::CheckedScalarComputationHandle;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    pub(super) fn declaration_input(
        &mut self,
        expression: ExpressionHandle,
    ) -> Result<Option<(CheckedScalarComputationHandle, TypeReferenceHandle)>, &'static str> {
        if let Some(length) = self.declaration_input_length(expression)? {
            return Ok(Some(length));
        }
        let Some(place) = validation::canonical_place_from_expression(self.program, expression)
        else {
            return Ok(None);
        };
        let Some(symbol) = place.root.as_symbol() else {
            return Ok(None);
        };
        let mut captures = self
            .declaration_values
            .iter()
            .filter(|(formal, _)| *formal == place);
        if let Some((_, value)) = captures.next() {
            if captures.next().is_some() {
                return Err("a declaration input has multiple completed captures");
            }
            let formal = self
                .declaration_parameters
                .iter()
                .find(|formal| formal.symbol == symbol)
                .ok_or("a captured declaration input has no source formal")?;
            let geometry = validation::structural_scalar_coordinate_in_type(
                self.program,
                place.root,
                formal.type_reference,
                &place.segments,
            )
            .ok_or("a captured declaration read has no exact formal geometry")?;
            if Some(geometry.primitive_type) != Some(self.plans.nodes.get(*value).primitive_type) {
                return Err("a completed declaration capture has the wrong formal carrier");
            }
            return Ok(Some((*value, geometry.result_type_reference)));
        }
        let mut bindings = self
            .declaration_bindings
            .iter()
            .filter(|(formal, _)| *formal == place);
        let Some((_, coordinate)) = bindings.next() else {
            return Ok(None);
        };
        if bindings.next().is_some() {
            return Err("a declaration input has multiple destination coordinates");
        }
        let coordinate = coordinate.clone();
        let formal = self
            .declaration_parameters
            .iter()
            .find(|formal| formal.symbol == symbol)
            .ok_or("a declaration coordinate has no source formal")?;
        let reference = validation::structural_scalar_coordinate_in_type(
            self.program,
            place.root,
            formal.type_reference,
            &place.segments,
        )
        .ok_or("a declaration coordinate has no exact scalar geometry")?
        .result_type_reference;
        let primitive = self
            .program
            .primitive_type_reference(reference)
            .ok_or("a declaration coordinate has no fixed scalar carrier")?;
        // Geometry alone supplies no dynamic-index receipts. An indexed or
        // otherwise unavailable destination still refuses through this owner.
        let value = self
            .scalar_coordinate(expression, &coordinate, primitive, &[])
            .ok_or("a declaration coordinate lacks a supported entry observation")?;
        Ok(Some((value, reference)))
    }

    /// A rebound sequence formal's length reads the destination's descriptor.
    /// The formal's own geometry has no scalar coordinate for `len`, so the
    /// ordinary binding route above cannot reach it.
    fn declaration_input_length(
        &mut self,
        expression: ExpressionHandle,
    ) -> Result<Option<(CheckedScalarComputationHandle, TypeReferenceHandle)>, &'static str> {
        // Only a declaring application (a peer's endpoint or a declared
        // measure) rebinds a sequence formal. Without one, for example a
        // state's own `terminates by (i, path.len)`, the caller's ordinary
        // expression lowering reads the length, as it did before this route.
        let Some(scope) = self.declaration_scope.as_ref() else {
            return Ok(None);
        };
        if self.declaration_bindings.is_empty() {
            return Ok(None);
        }
        let Some(member) = self.program.expression_table.member_node(expression) else {
            return Ok(None);
        };
        let Some(receiver) =
            validation::canonical_place_from_expression(self.program, member.receiver)
        else {
            return Ok(None);
        };
        let mut bindings = self
            .declaration_bindings
            .iter()
            .filter(|(formal, _)| *formal == receiver);
        let Some((_, destination)) = bindings.next() else {
            return Ok(None);
        };
        if bindings.next().is_some() {
            return Err("a declaration input has multiple destination coordinates");
        }
        let Some(symbol) = receiver.root.as_symbol() else {
            return Ok(None);
        };
        let formal = self
            .declaration_parameters
            .iter()
            .find(|formal| formal.symbol == symbol)
            .ok_or("a declaration coordinate has no source formal")?;
        let formal_type = validation::project_type_reference_from_segments(
            self.program,
            formal.type_reference,
            &receiver.segments,
        )
        .ok_or("a rebound sequence formal has no exact geometry")?;
        if validation::collection_operations::collection_measure_member(
            self.program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(formal_type),
        ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
        {
            return Ok(None);
        }
        let Some(parameter) = destination.root.as_symbol() else {
            return Err("a rebound sequence length has no parameter destination");
        };
        let (_, state) = validation::find_state(self.program, self.state)
            .ok_or("a rebound sequence length has no evaluating state")?;
        let (_, _, value) = validation::ranking_slice_length_observation(
            self.program,
            state,
            &validation::RankSubjectCoordinate {
                parameter,
                path: destination.segments.clone(),
            },
        )
        .ok_or("a rebound sequence length has no descriptor observation")?;
        // The expression belongs to the declaring state, not the evaluator.
        let owner = scope
            .plan
            .application_formations
            .get(scope.application)
            .application
            .owner;
        let (owner_machine, owner_state) = validation::find_state(self.program, owner)
            .ok_or("a rebound sequence length has no declaring state")?;
        let reference = validation::expression_result_type_reference(
            self.program,
            owner_machine,
            owner_state,
            expression,
        )
        .ok_or("a rebound sequence length has no result type")?;
        let value = self.insert(
            typed_trees::types::PrimitiveType::U64,
            super::CheckedScalarComputationKind::Value(value),
        );
        Ok(Some((value, reference)))
    }

    /// Pure lowering owns a scalar telescope, so a subtree that refers to
    /// an explicitly rebound declaration input must first pass through the
    /// computation graph. The shared typed-reference walk owns all expression
    /// forms; independent literals and ordinary formals retain their fast path.
    /// A completed capture also blocks the shortcut without a coordinate row:
    /// its saved value must not be replaced by the ordinary formal parameter.
    pub(super) fn has_declaration_binding(&self, expression: ExpressionHandle) -> bool {
        if self.declaration_bindings.is_empty() && self.declaration_values.is_empty() {
            return false;
        }
        struct Bindings<'bindings> {
            bindings: &'bindings [(validation::CanonicalPlace, validation::CanonicalPlace)],
            captures: &'bindings [(validation::CanonicalPlace, CheckedScalarComputationHandle)],
            found: bool,
            visited: Vec<ExpressionHandle>,
        }
        impl Extend<SymbolHandle> for Bindings<'_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
                self.found |= symbols.into_iter().any(|symbol| {
                    self.bindings
                        .iter()
                        .any(|(formal, _)| formal.root == facts::PlaceRoot::Symbol(symbol))
                        || self
                            .captures
                            .iter()
                            .any(|(formal, _)| formal.root == facts::PlaceRoot::Symbol(symbol))
                });
            }
        }
        impl typed_trees::references::TypedReferences for Bindings<'_> {
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                if self.visited.contains(&expression) {
                    return false;
                }
                self.visited.push(expression);
                true
            }
            fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
                false
            }
        }
        let mut bindings = Bindings {
            bindings: self.declaration_bindings,
            captures: &self.declaration_values,
            found: false,
            visited: Vec::new(),
        };
        typed_trees::references::visit_expression(self.program, &mut bindings, expression);
        bindings.found
    }
}
