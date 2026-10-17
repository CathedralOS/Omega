//! Element-view establishment under the scalar operand owner.
//!
//! A `let view: &[T]` or `let slots: &mut [T]` local initialized by a
//! collection opener lends storage that already exists, so the value rejoins
//! the loan checked borrow admission recorded instead of producing a call
//! result. The loan's access
//! is the view's access: a shared read makes a `BorrowedSliceView`, an
//! exclusive `Mutable` loan a `MutableSliceView`. The declared view's element
//! equals the lent collection's element for an ordinary collection opener.
//! A representation view instead retains its authored byte start and source
//! extent: the existing recast judgment admits the relation, and Terminal
//! independently proves the emitted region's bounds and exact target tiling.
//! Reinterpreting the entire buffer before slicing is not equivalent: the
//! suffix can tile even when the whole source cannot.
use super::{
    Builder, ExpressionHandle, ExpressionNode, StatementNode, TypeReferenceNode,
    structural_values::canonical_place_is_borrowable,
};
use checked_trees::{CheckedStructuralAccess, CheckedStructuralValueKind};
use typed_trees::types::TypeReferenceHandle;

mod formation;

impl Builder<'_, '_> {
    /// One whole `[T]` view of the collection place this statement lends.
    /// Fixed arrays and vectors own contiguous storage and a slice borrows
    /// it, so a view local denotes the lent place under its loan rather than
    /// a fresh value with copied elements, and its stored length is that
    /// collection's own extent. Checked borrow admission already recorded the
    /// loan or transition argument roster names the exact lent place and
    /// access. Formation rejoins that lexical owner before the common
    /// collection/region walk; a local and an inline operand publish the same
    /// descriptor operation at their respective evaluation points.
    pub(super) fn slice_view_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        // Range formation has one owner, including literal endpoints. Do not
        // reinterpret a declined range as a whole-collection loan here.
        if validation::subslice_expression(self.program, expression).is_some() {
            return None;
        }
        // Loan provenance describes the result's backing, not how the value
        // is evaluated. An ordinary retained call must execute even when its
        // result borrows the same collection; the callee may narrow its extent
        // or perform effects before returning. Only erased collection openers
        // have no runtime occurrence to preserve here.
        if let Some(call) = self.program.expression_table.call_node(expression)
            && self
                .call_ordinal(
                    expression,
                    validation::call_target_symbol(self.program, call),
                )
                .is_some()
        {
            return None;
        }
        let view =
            crate::execution::terminal_unit::types::element_view_shell(self.program, expected, &[])
                .or_else(|| {
                    matches!(
                        crate::execution::terminal_unit::types::byte_sequence_carrier(
                            self.program,
                            expected,
                            &[],
                        ),
                        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
                    )
                    .then(|| validation::slice_view_shell(self.program, expected, &[]))
                    .flatten()
                })?;
        let state = self.program.state_by_symbol(self.state)?;
        let (mut place, access) = self.view_formation(expression, expected, view.access)?;

        let machine = self.program.machine_by_symbol(self.machine)?;
        let authored_recast = validation::slice_recast_source(self.program, expression);
        let recast = validation::slice_recast_byte_region(self.program, machine, state, expression);
        if let Some(authored) = authored_recast {
            if authored.access != view.access
                || self.program.normalized_type_identity(authored.element)
                    != self.program.normalized_type_identity(view.element)
            {
                return None;
            }
            if recast.is_none() {
                let collection =
                    validation::canonical_place_from_expression(self.program, authored.source)?;

                if collection != place {
                    return None;
                }
            }
        }
        if let Some(region) = recast {
            if region.access != view.access
                || self.program.normalized_type_identity(region.element)
                    != self.program.normalized_type_identity(view.element)
            {
                return None;
            }
            let collection =
                validation::canonical_place_from_expression(self.program, region.collection)?;

            // Literal suffixes have an exact range loan. A runtime suffix
            // borrows the whole byte backing conservatively; its captured
            // endpoints, rather than an element-sized loan, narrow the view.
            let mut authored_loan = collection.clone();
            if let Some(offset) = self.program.expression_table.integer_node(region.start) {
                authored_loan
                    .segments
                    .push(facts::PlaceSegment::FixedRange {
                        start: usize::try_from(offset.value_i64()?).ok()?,
                        end: usize::try_from(region.extent).ok()?,
                    });
            }
            if authored_loan != place {
                return None;
            }
            place = collection;
        }
        // A captured region is a loan footprint, not a declared value: its
        // suffix segment has no element type. Rejoin the checked formation to
        // its backing before asking whether storage already holds a view.
        // Otherwise a literal suffix declines while an equivalent runtime
        // suffix, whose conservative loan names the whole backing, succeeds.
        let declared = validation::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if validation::slice_view_shell(self.program, declared, &[]).is_some() {
            // Transport the current descriptor through the ordinary reference
            // join. Collection establishment would replace its captured extent
            // with backing geometry. Stored-leaf selection also retains every
            // crossing through a local reference before selecting this view.
            let (mut source, _, selectors) = self.borrowed_leaf_argument(expression, declared)?;
            source.access = access;
            let selectors = self.retain_reference_selectors(expression, selectors)?;
            return Some(CheckedStructuralValueKind::Reference {
                source,
                selectors,
                storage: checked_trees::CheckedReferenceStorage::Declared,
                backing_type: declared,
                source_value: checked_trees::CheckedStructuralValueHandle::invalid(),
            });
        }
        if !canonical_place_is_borrowable(&place) {
            return None;
        }
        let symbol = place.root.as_symbol()?;
        // The root's dense structural position is its signature ordinal when
        // the name resolves to a carried parameter; anything else is an
        // operation-sequence local keyed by symbol. Both views settle the dense
        // index only when no borrowed `self` precedes the root — that receiver
        // joins the namespace only when the signature retains it, a decision
        // this emission cannot see — so `dense_structural_parameter_index` may
        // decline and the root then keys itself by symbol, which every roster
        // family matches directly.
        let parameter_index =
            typed_trees::signature::state_parameter_position(self.authored_parameters, symbol)
                .and_then(|authored_position| {
                    self.dense_structural_parameter_index(authored_position)
                });
        let source = match parameter_index {
            Some(parameter_index) => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                }
            }
            None => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            }
        };
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
            None,
            None,
        )?;
        // A slice descriptor already carries its backing. An array endpoint
        // instead lends its referee's storage, including a borrowed array
        // parameter or an array behind a stored reference. Reuse the common
        // storage projection so only stored shells gain explicit crossings.
        let (projected, path) =
            if validation::slice_view_shell(self.program, projected, &[]).is_some() {
                (projected, path)
            } else {
                crate::execution::terminal_unit::calls::projected_observed_path(
                    self.program,
                    self.machine,
                    self.state,
                    self.statement_index,
                    &place,
                    &|_| None,
                )?
            };
        // A view borrows the registered storage carrier. Predicate-only
        // qualifications remain in the loan evidence, not a second array type.
        let type_identity =
            validation::stored_structural_type_identity(self.program, projected, &[], &[]);
        let lent = lent_collection_element(self.program, projected)?;
        // Ordinary collection openers retain their exact element identity.
        // An authored recast retains both declared identities: source
        // checking admits the relation and Terminal independently replays
        // geometry and validity under the authenticated access.
        if recast.is_none()
            && authored_recast.is_none()
            && self.program.normalized_type_identity(lent)
                != self.program.normalized_type_identity(view.element)
        {
            return None;
        }
        let region = if let Some(region) = recast {
            let primitive =
                super::semantic_casts::result_type(self.program, self.state, region.start)
                    .and_then(|reference| self.program.primitive_type_reference(reference))
                    .or_else(|| {
                        matches!(
                            self.program.expression_table.expression(region.start),
                            ExpressionNode::Integer(_)
                        )
                        .then_some(typed_trees::types::PrimitiveType::U64)
                    })?;
            let start = self.expression(region.start, primitive)?;
            self.plans.nodes.get_mut(start).authored_root = region.start;
            self.push_scalar_root(
                checked_trees::CheckedScalarExpressionRole::RepresentationViewOffset { expression },
                start,
            )?;
            checked_trees::CheckedElementViewRegion::RepresentationBytes {
                start,
                end: region.extent,
            }
        } else {
            checked_trees::CheckedElementViewRegion::WholeCollection
        };
        let source = checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity,
            access,
        };
        Some(if access == CheckedStructuralAccess::SharedBorrow {
            CheckedStructuralValueKind::BorrowedSliceView { source, region }
        } else {
            CheckedStructuralValueKind::MutableSliceView { source, region }
        })
    }

    /// Structural values use the same range admission as call operands and
    /// local bindings. The source carrier decides whether storage first lends
    /// a descriptor; endpoint computations do not select another producer.
    pub(super) fn subslice_view_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        use crate::execution::terminal_unit::{calls, types::ShapeCollector};
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_by_symbol(self.state)?;
        let parameters = if machine.attached_data.is_some() {
            calls::ambient_self_scalar_signature(self.program, machine, state)?.0
        } else {
            calls::free_structural_scalar_signature(
                self.program,
                &mut ShapeCollector::new(self.program),
                state,
                &[],
            )?
            .0
        };
        let site = match self
            .program
            .statement_table
            .statement_at(state, self.statement_index)
        {
            Some(StatementNode::LocalData(local)) if local.initial_value == expression => {
                checked_trees::CheckedSubsliceSite::LocalBinding
            }
            _ => checked_trees::CheckedSubsliceSite::Expression { expression },
        };
        let source = calls::view_subslice::admit_resolved(
            self.program,
            self.operators,
            self.pure,
            self.plans,
            self.borrow,
            machine,
            state,
            &parameters,
            expected,
            expression,
            self.statement_index,
            site,
        )?
        .argument();
        Some(match source.access {
            CheckedStructuralAccess::SharedBorrow => {
                CheckedStructuralValueKind::BorrowedSliceView {
                    source,
                    region: checked_trees::CheckedElementViewRegion::WholeCollection,
                }
            }
            CheckedStructuralAccess::MutableBorrow => {
                CheckedStructuralValueKind::MutableSliceView {
                    source,
                    region: checked_trees::CheckedElementViewRegion::WholeCollection,
                }
            }
            _ => return None,
        })
    }
}

/// The element type of an owned or borrowed contiguous collection place.
/// A fixed array declares its extent and a slice carries a stored one; both
/// lend the same elements, so both can back a view.
fn lent_collection_element(
    program: &dyn ::validation::CheckingInput,
    mut type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
            _ => return None,
        }
    }
}
