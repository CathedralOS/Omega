//! Borrowed element-view establishment: a direct view publishes its fresh
//! place straight as the completed operation result, while a joined view
//! first resolves the region across its contributing sources. Exclusive
//! loans carry the binding's declared multiplicity; shared views stay
//! unrestricted.

use super::super::super::super::StructuralArgument;
use super::super::super::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, OperationResult, PlaceId,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeShape,
    allocate_dense, place_id, unsupported,
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
    /// result is `Unrestricted`; an exclusive loan is affine custody, so a
    /// mutable view result carries this binding's declared multiplicity.
    pub(super) fn direct_element_view(
        &mut self,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
    ) -> Result<PlaceId, LoweringError> {
        let multiplicity = match loan {
            LoanAccess::Shared => StructuralMultiplicity::Unrestricted,
            LoanAccess::Mutable => self.multiplicity,
        };
        self.establish_element_view(source, region, expression, loan, multiplicity)
    }

    /// Establish one element view inside a selection: the fresh `Unrestricted`
    /// view result joins the selection's block parameter under the view's
    /// loan, and that parameter carries the binding's custody.
    pub(super) fn joined_element_view(
        &mut self,
        source: &checked_trees::CheckedUnitStructuralArgumentPlan,
        region: &checked_trees::CheckedElementViewRegion,
        expression: checked_trees::expression::ExpressionHandle,
        loan: LoanAccess,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        // A nested position — a record/case field value — names no selection
        // block parameter to join. The view establishes directly as a fresh
        // operation result, exactly as a top-level view binding does; the
        // caller already repointed `structural_type` and `multiplicity` at the
        // field's declared custody.
        let Some(continuation) = continuation else {
            return self.direct_element_view(source, region, expression, loan);
        };
        let place = self.establish_element_view(
            source,
            region,
            expression,
            loan,
            StructuralMultiplicity::Unrestricted,
        )?;
        self.complete_loan(
            StructuralArgument {
                place,
                path: Vec::new(),
                access: loan.structural_access(),
            },
            continuation,
            loan,
        )?;
        Ok(continuation.place)
    }

    /// Establish one element view over the collection `source` lends, as a
    /// fresh operation result of `multiplicity`. A shared loan lowers to
    /// `EstablishElementView` and binds only a `SharedBorrow` argument; an
    /// exclusive loan lowers to `EstablishMutableElementView` and binds the
    /// argument through the readable-loan binder, which admits either access.
    pub(super) fn establish_element_view(
        &mut self,
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
                self.checked,
                self.state,
                self.statement,
                source,
                self.structural_types,
            )?;
            let bindings = self.structural_bindings();
            let whole = storage
                .requires_establishment()
                .then(|| allocate_dense(self.next_place).map(place_id))
                .transpose()?;
            let (bound, establishment) = storage.bind(
                self.checked,
                self.state,
                &bindings,
                whole,
                self.structural_types,
            )?;
            if let Some(establishment) = establishment {
                self.temporary_places.push(view_ranges::establish(
                    &establishment,
                    self.values,
                    self.next_value,
                    self.operations,
                ));
            }
            let (_, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
            let site = match self
                .checked
                .statement_table
                .statements(authored.statement_nodes)
                .get(self.statement as usize)
            {
                Some(checked_trees::statement::StatementNode::LocalData(local))
                    if local.initial_value == expression =>
                {
                    checked_trees::CheckedSubsliceSite::LocalBinding
                }
                _ => checked_trees::CheckedSubsliceSite::Expression { expression },
            };
            let place = place_id(allocate_dense(self.next_place)?);
            self.temporary_places.push(view_ranges::emit(
                self.checked,
                view_ranges::ViewRangeSite {
                    state: self.state,
                    statement: self.statement,
                    site,
                    expression: range.expression,
                    retained: Some((range.start, range.end)),
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
            )?);
            return Ok(place);
        }
        let element = self
            .structural_types
            .iter()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::ElementView { element }
                    if declaration.id == self.structural_type =>
                {
                    Some(element)
                }
                // A byte-sequence view's element is always the `u8` scalar:
                // the carrier keeps no element field, so the view op's stride
                // resolves through the primitive row directly.
                StructuralTypeShape::ByteSequence(_) if declaration.id == self.structural_type => {
                    crate::terminal_identities::lookup_type_id(
                        self.type_ids,
                        crate::terminal_identities::BYTE_VIEW_ELEMENT_TYPE_IDENTITY,
                    )
                    .ok()
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(match loan {
                LoanAccess::Shared => "borrowed element view lost its element carrier",
                LoanAccess::Mutable => "mutable element view lost its element carrier",
            }))?;
        let bindings = self.structural_bindings();
        let (machine, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
        let parameters = self.checked.state_parameters(authored);
        let argument = match loan {
            LoanAccess::Shared => {
                bindings.shared_structural_argument(source, machine, parameters)?
            }
            LoanAccess::Mutable => {
                bindings.borrowed_structural_argument(source, machine, parameters)?
            }
        };
        // The descriptor captures the byte endpoints now. Later field writes
        // and state transfers keep this formed region; they never reread the
        // authored selector to reconstruct a different borrowed window.
        let endpoints = match region {
            checked_trees::CheckedElementViewRegion::WholeCollection => None,
            checked_trees::CheckedElementViewRegion::RepresentationBytes { start, end } => {
                let start = self.scalar(
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
        let place = place_id(allocate_dense(self.next_place)?);
        let kind = match loan {
            LoanAccess::Shared => OperationKind::EstablishElementView {
                destination: place,
                source: argument,
                element,
                region,
            },
            LoanAccess::Mutable => OperationKind::EstablishMutableElementView {
                destination: place,
                source: argument,
                element,
                region,
            },
        };
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: self.structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind,
        });
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type: self.structural_type,
            },
        });
        Ok(place)
    }
}
