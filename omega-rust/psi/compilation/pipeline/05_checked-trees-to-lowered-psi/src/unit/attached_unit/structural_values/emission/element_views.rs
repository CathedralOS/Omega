//! Borrowed element-view establishment: a direct view publishes its fresh
//! place straight as the completed operation result, while a joined view
//! first resolves the region across its contributing sources. Exclusive
//! loans carry the binding's declared multiplicity; shared views stay
//! unrestricted.

use super::super::super::super::StructuralArgument;
use super::super::super::{
    CheckedScalarExpressionRole, LoweringError, PlaceId, StructuralMultiplicity, allocate_dense,
    place_id, unsupported,
};
use super::{Emission, ValueContinuation};
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, emit_direct_expression,
};
use crate::expression_preparation::bindings::LoanAccess;

impl Emission<'_, '_, '_> {
    /// Establish one borrowed element view without a selection join: the op
    /// result is itself the checked value, so the fresh place publishes
    /// directly as this binding's completed operation result. A shared view
    /// result is `Unrestricted`; an exclusive loan is always affine custody,
    /// whatever multiplicity the binding's carrier type declares.
    pub(super) fn direct_element_view(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
    ) -> Result<PlaceId, LoweringError> {
        let multiplicity = match loan {
            LoanAccess::Shared => StructuralMultiplicity::Unrestricted,
            LoanAccess::Mutable => StructuralMultiplicity::Affine,
        };
        self.establish_element_view(identities, source, region, expression, loan, multiplicity)
    }

    /// Establish one element view inside a selection. A shared view joins the
    /// selection parameter under its loan. An exclusive view transfers the
    /// selected descriptor owned onto that parameter, which is the carrier
    /// the successor reads.
    pub(super) fn joined_element_view(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        // A nested position — a record/case field value — names no selection
        // block parameter to join. The view establishes directly as a fresh
        // operation result, exactly as a top-level view binding does; the
        // caller already repointed `structural_type` and `multiplicity` at the
        // field's declared custody.
        let Some(continuation) = continuation else {
            return self.direct_element_view(identities, source, region, expression, loan);
        };
        let place = self.establish_element_view(
            identities,
            source,
            region,
            expression,
            loan,
            StructuralMultiplicity::Unrestricted,
        )?;
        let mut argument = StructuralArgument {
            place,
            path: Vec::new(),
            access: loan.structural_access(),
        };
        // The join parameter is the selected descriptor. Forward that same
        // owned carrier; a loan of the arm-local view would die on this edge
        // and leave the parameter without a disposal record.
        if continuation
            .structural_parameters
            .first()
            .is_some_and(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
        {
            argument.access = terminal_psi::StructuralAccess::Owned;
        }
        self.complete_loan(argument, continuation, loan)?;
        Ok(continuation.place)
    }

    /// Establish one element view over the collection `source` lends, as a
    /// fresh operation result of `multiplicity`. A shared loan lowers to
    /// `EstablishElementView` and binds only a `SharedBorrow` argument; an
    /// exclusive loan lowers to `EstablishMutableElementView` and binds the
    /// argument through the readable-loan binder, which admits either access.
    pub(super) fn establish_element_view(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
        multiplicity: StructuralMultiplicity,
    ) -> Result<PlaceId, LoweringError> {
        // A range source contains an operation, not just an address. Use the
        // same storage binding and endpoint replay as calls and state edges;
        // publishing the enclosing collection would silently widen the view.
        if let Some(range) = super::super::super::view_ranges::range(source) {
            use super::super::super::view_ranges;
            if !matches!(
                region,
                checked_trees::CheckedElementViewRegion::WholeCollection
            ) {
                return unsupported("element range cannot replace a representation-byte region");
            }
            let storage = view_ranges::storage(
                identities,
                self.checked,
                self.state,
                self.statement,
                source,
                self.structural_types,
            )?;
            let bindings = self.structural_bindings();
            let whole = storage
                .requires_establishment()
                .then(|| allocate_dense(self.environment.next_place).map(place_id))
                .transpose()?;
            let (bound, establishment) = storage.bind(
                identities,
                self.checked,
                self.state,
                &bindings,
                whole,
                self.structural_types,
            )?;
            if let Some(establishment) = establishment {
                let declaration = view_ranges::establish(
                    &establishment,
                    self.values,
                    self.next_value,
                    self.operations,
                )?;
                self.operations.register_structural_materialization(
                    declaration.id,
                    self.state,
                    self.statement,
                )?;
                self.environment.private_places.push(declaration);
            }
            let place = place_id(allocate_dense(self.environment.next_place)?);
            let declaration = view_ranges::emit(
                identities,
                self.checked,
                view_ranges::ViewRangeSite {
                    state: self.state,
                    statement: self.statement,
                    range,
                },
                bound,
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
            self.operations.register_structural_materialization(
                place,
                self.state,
                self.statement,
            )?;
            self.environment.private_places.push(declaration);
            return Ok(place);
        }
        let element = super::super::super::view_ranges::element_type(
            identities,
            self.checked,
            self.structural_type,
            self.structural_types,
        )?;
        let bindings = self.structural_bindings();
        let (_, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
        let parameters = self.checked.state_parameters(authored);
        let argument = match loan {
            LoanAccess::Shared => {
                bindings.shared_structural_argument(identities, self.checked, source, parameters)?
            }
            LoanAccess::Mutable => bindings.borrowed_structural_argument(
                identities,
                self.checked,
                source,
                parameters,
            )?,
        };
        // The descriptor captures the byte endpoints now. Later field writes
        // and state transfers keep this formed region; they never reread the
        // authored selector to reconstruct a different borrowed window.
        let endpoints = match region {
            checked_trees::CheckedElementViewRegion::WholeCollection => None,
            checked_trees::CheckedElementViewRegion::RepresentationBytes { start, end } => {
                let start = self.scalar(
                    identities,
                    CheckedScalarExpressionRole::RepresentationViewOffset { expression },
                    *start,
                    self.sources.len(),
                )?;
                let index = crate::expression_preparation::prepare_expression::exact_u64_index(
                    LoweredDirectExpression::Parameter {
                        position: 0,
                        scalar_type: start.scalar_type,
                    },
                )?;
                let start = emit_direct_expression(
                    &index,
                    std::slice::from_ref(&start),
                    self.next_value,
                    self.operations,
                );
                let end = emit_direct_expression(
                    &LoweredDirectExpression::IntegerLiteral {
                        value: semantic_vocabulary::IntegerValue::Unsigned((*end).into()),
                        scalar_type: crate::emission::scalar_types::terminal_scalar_type(
                            checked_trees::types::PrimitiveType::U64,
                        )?,
                    },
                    &[],
                    self.next_value,
                    self.operations,
                );
                Some((start, end))
            }
        };
        let id = self.operations.allocate();
        let region = match endpoints {
            None => terminal_psi::ElementViewRegion::WholeCollection,
            Some((start, end)) => terminal_psi::ElementViewRegion::RepresentationBytes {
                start,
                end,
                obligation: crate::terminal_identities::obligation_id(
                    id.get().checked_add(1).ok_or(LoweringError::Unsupported(
                        "representation view obligation identity overflows",
                    ))?,
                ),
            },
        };
        let place = place_id(allocate_dense(self.environment.next_place)?);
        self.environment
            .private_places
            .push(super::super::super::view_ranges::emit_view(
                place,
                self.structural_type,
                argument,
                element,
                region,
                multiplicity,
                id,
                self.operations,
            ));
        self.operations
            .register_structural_materialization(place, self.state, self.statement)?;
        self.evaluation.current_claims.publish(place, Vec::new())?;
        Ok(place)
    }
}
