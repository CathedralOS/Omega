//! Element-view establishment under the scalar operand owner.
//!
//! A `let view: &[T] = collection.as_slice()` or `let slots: &mut [T] =
//! collection.as_mut_slice()` local is spelled as a call but lends storage
//! that already exists, so the value rejoins the loan checked borrow
//! admission recorded instead of producing a call result. The loan's access
//! is the view's access: a shared read makes a `BorrowedSliceView`, an
//! exclusive `Mutable` loan a `MutableSliceView`. The declared view's element
//! equals the lent collection's element for an ordinary collection opener.
//! A representation view instead retains its authored byte start and source
//! extent: the existing recast judgment admits the relation, and Terminal
//! independently proves the emitted region's bounds and exact target tiling.
//! Reinterpreting the entire buffer before slicing is not equivalent: the
//! suffix can tile even when the whole source cannot.
use super::{
    Builder, ExpressionHandle, ExpressionNode, StatementNode, TypeReferenceNode, TypedTrees,
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
        // Loan provenance describes the result's backing, not how the value
        // is evaluated. An ordinary retained call must execute even when its
        // result borrows the same collection; the callee may narrow its extent
        // or perform effects before returning. Only erased collection openers
        // have no runtime occurrence to preserve here.
        if let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
            && self
                .call_ordinal(
                    expression,
                    crate::flow::call_target_symbol(self.program, call),
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
        let state = crate::semantic::calls::find_state(self.program, self.state)?;
        let (mut place, access) = self.view_formation(expression, expected, view.access)?;
        crate::flow::normalize_attached_place_root(
            self.program,
            self.machine,
            self.state,
            &mut place,
        );
        let machine = crate::lookup::machine_by_symbol(self.program, self.machine)?;
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
                let mut collection =
                    crate::flow::canonical_place_from_expression(self.program, authored.source)?;
                crate::flow::normalize_attached_place_root(
                    self.program,
                    self.machine,
                    self.state,
                    &mut collection,
                );
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
            let mut collection =
                crate::flow::canonical_place_from_expression(self.program, region.collection)?;
            crate::flow::normalize_attached_place_root(
                self.program,
                self.machine,
                self.state,
                &mut collection,
            );
            // Literal suffixes have an exact range loan. A runtime suffix
            // borrows the whole byte backing conservatively; its captured
            // endpoints, rather than an element-sized loan, narrow the view.
            let mut authored_loan = collection.clone();
            if let ExpressionNode::Integer(offset) =
                self.program.expression_table.expression(region.start)
            {
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
        // A view's lent window is a `FixedRange` segment — the range is the
        // view's own extent, so the borrowable walk admits it beside the
        // static leaf segments for primitive-scalar elements. Wider elements
        // keep element ordinals; only the u8 window spells byte bounds.
        let scalar_element = self
            .program
            .primitive_type_reference(view.element)
            .is_some();
        let byte_element = self.program.primitive_type_reference(view.element)
            == Some(typed_trees::types::PrimitiveType::U8);
        let borrowable = canonical_place_is_borrowable(&place)
            || (scalar_element
                && matches!(place.root, facts::PlaceRoot::Symbol(_))
                && place.segments.iter().all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Field { .. }
                            | facts::PlaceSegment::FixedIndex { .. }
                            | facts::PlaceSegment::FixedRange { .. }
                    )
                }));
        if !borrowable {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // The root's dense structural position is its signature ordinal when
        // the name resolves to a carried parameter; anything else is an
        // operation-sequence local keyed by symbol. Both views settle the dense
        // index only when no borrowed `self` precedes the root — that receiver
        // joins the namespace only when the signature retains it, a decision
        // this emission cannot see — so `dense_structural_parameter_index` may
        // decline and the root then keys itself by symbol, which every roster
        // family matches directly.
        let parameter_index = self
            .authored_parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)
            .and_then(|authored_position| self.dense_structural_parameter_index(authored_position));
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
        // A view's window (`self.source[0..2]`) ends its loan place in a
        // `FixedRange`; the window's own type mints no row, so the path mints
        // against the container and the range reattaches as `FixedByteRange`
        // for u8 or as an `ElementViewSubslice` source for wider scalar
        // elements — its endpoints stay element ordinals.
        let (window, container_place) = match place.segments.last() {
            Some(facts::PlaceSegment::FixedRange { start, end }) if scalar_element => (
                Some((*start, *end)),
                crate::flow::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..place.segments.len() - 1].to_vec(),
                },
            ),
            _ => (None, place.clone()),
        };
        let Some((projected, mut path)) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &container_place,
            )
        else {
            return None;
        };
        // Wider scalar elements carry the window on the source plan, not on
        // the path: `ElementViewSubslice` names the same field-projection path
        // a `&[u8]` byte window would, then attaches literal element-ordinal
        // endpoints against the fixed array's declared extent. Exclusive
        // loans keep declining: no `MutableSliceView` consumer reads an
        // element subslice source yet.
        if window.is_some() && !byte_element && access != CheckedStructuralAccess::SharedBorrow {
            return None;
        }
        let (source, type_identity) = if let Some((start, end)) = window {
            if byte_element {
                let Some(()) =
                    crate::execution::terminal_unit::calls::fixed_byte_range_within_extent(
                        self.program,
                        projected,
                        start,
                        end,
                    )
                else {
                    return None;
                };
                path.push(
                    checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange {
                        start: u64::try_from(start).ok()?,
                        end: u64::try_from(end).ok()?,
                    },
                );
                (
                    source,
                    self.program
                        .normalized_type_identity(projected)
                        .into_string(),
                )
            } else {
                let mut container = projected;
                while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. }
                | typed_trees::types::TypeReferenceNode::Reference {
                    referee: base_type,
                    ..
                } = self.program.type_reference_table.type_reference(container)
                {
                    container = *base_type;
                }
                let typed_trees::types::TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                    ..
                } = self.program.type_reference_table.type_reference(container)
                else {
                    return None;
                };
                if !(start <= end && end <= usize::try_from(*length).ok()?) {
                    return None;
                }
                let root = match source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    } => checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_index,
                    },
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } => checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
                    _ => return None,
                };
                let endpoint = |value: usize| {
                    Some(checked_trees::CheckedScalarExpression::IntegerLiteral {
                        literal: numerics::literals::IntegerLiteral::from_value(
                            i64::try_from(value).ok()?,
                        ),
                    })
                };
                (
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                        root,
                        expression,
                        start: Some(endpoint(start)?),
                        end: Some(endpoint(end)?),
                    },
                    crate::execution::terminal_unit::types::borrowed_slice_view_type_identity(
                        self.program,
                        expected,
                        &[],
                        &[],
                    ),
                )
            }
        } else if validation::slice_view_shell(self.program, projected, &[]).is_some() {
            // A whole view over a place that is itself a view (`let view:
            // &[i32] = value.readings` where the member is already `&[i32]`)
            // names the view's own identity with the reference shell peeled —
            // the loan's access binds on the member, not on the shape.
            (
                source,
                crate::execution::terminal_unit::types::borrowed_slice_view_type_identity(
                    self.program,
                    expected,
                    &[],
                    &[],
                ),
            )
        } else {
            (
                source,
                self.program
                    .normalized_type_identity(projected)
                    .into_string(),
            )
        };
        // Ordinary collection openers retain their exact element identity.
        // An authored recast retains both declared identities: source
        // checking admits the relation and Terminal independently replays
        // geometry and validity under the authenticated access.
        let Some(lent) = lent_collection_element(self.program, projected) else {
            return None;
        };
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
            self.plans
                .roots
                .append(checked_trees::CheckedScalarComputationRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: u32::try_from(self.statement_index).ok()?,
                    role: checked_trees::CheckedScalarExpressionRole::RepresentationViewOffset {
                        expression,
                    },
                    root: start,
                });
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

    /// One `collection[start..end]` range at a bare expression occurrence —
    /// a transition target's record field, a computation operand — narrows
    /// an established view the way a call argument's range does. Its source
    /// is the collection's own view place projected through the same path
    /// operation, and its present endpoints are the rows checked scalar
    /// planning bound under the occurrence's `Expression` site. Sources are
    /// established views only: an owned collection's window needs the loan
    /// custody the `let` and call lanes separately adjudicate, so a
    /// fixed-array collection keeps declining here.
    pub(super) fn subslice_view_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let operation = validation::subslice_expression(self.program, expression)?;
        // Only the builtin exclusive range narrows a view; an authored
        // operator overload on the same spelling owns its own semantics.
        if self
            .operators
            .expression_use(operation)
            .is_some_and(|selected| {
                selected.spelling != language_core::OperatorSpelling::Range
                    || selected.selected_operator_symbol.is_valid()
                    || selected.candidate_count != 0
                    || !matches!(
                        selected.status,
                        checked_trees::CheckedOperatorResolutionStatus::Missing
                            | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                    )
            })
        {
            return None;
        }
        let ExpressionNode::Indexed(indexed) = self.program.expression_table.expression(operation)
        else {
            return None;
        };
        let ExpressionNode::Range(range) = self.program.expression_table.expression(indexed.index)
        else {
            return None;
        };
        let shell = validation::reference_shell(self.program, expected)?;
        let access: CheckedStructuralAccess = shell.access.into();
        if !matches!(
            access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        ) {
            return None;
        }
        let bytes = matches!(
            crate::execution::terminal_unit::types::byte_sequence_carrier(
                self.program,
                expected,
                &[],
            ),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
        );
        let type_identity = if bytes {
            crate::execution::terminal_unit::types::byte_sequence_type_identity(
                self.program,
                expected,
                &[],
                &[],
            )?
        } else {
            if validation::slice_view_shell(self.program, expected, &[]).is_none() {
                return None;
            }
            crate::execution::terminal_unit::types::borrowed_slice_view_type_identity(
                self.program,
                expected,
                &[],
                &[],
            )
        };
        let machine = crate::lookup::machine_by_symbol(self.program, self.machine)?;
        let state = crate::semantic::calls::find_state(self.program, self.state)?;
        if !validation::has_builtin_subslice_meaning(self.program, machine, Some(state), operation)
        {
            return None;
        }
        let mut place =
            crate::flow::canonical_place_from_expression(self.program, indexed.collection)?;
        crate::flow::normalize_attached_place_root(
            self.program,
            self.machine,
            self.state,
            &mut place,
        );
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        let Some((projected, path)) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &place,
            )
        else {
            return None;
        };
        // The narrowed view re-slices an established view: the collection's
        // projected type spells the same view family and identity, and a
        // reference root carries exactly this range's access. Owned storage
        // names no established view and keeps declining.
        if bytes {
            if !matches!(
                crate::execution::terminal_unit::types::byte_sequence_carrier(
                    self.program,
                    projected,
                    &[],
                ),
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
            ) {
                return None;
            }
            if crate::execution::terminal_unit::types::byte_sequence_type_identity(
                self.program,
                projected,
                &[],
                &[],
            )? != type_identity
            {
                return None;
            }
        } else {
            if validation::slice_view_shell(self.program, projected, &[]).is_none() {
                return None;
            }
            if crate::execution::terminal_unit::types::borrowed_slice_view_type_identity(
                self.program,
                projected,
                &[],
                &[],
            ) != type_identity
            {
                return None;
            }
        }
        // A parameter root numbers into the dense structural parameter list;
        // local collections have no established-view row at this planning
        // point and keep declining beside owned storage.
        let index = self
            .authored_parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)
            .and_then(|position| self.dense_structural_parameter_index(position))?;
        let root = checked_trees::CheckedStorageRoot::Parameter { index };
        // The scalar planner bound each present endpoint under the
        // occurrence's `Expression` site; replay the exact row only when it
        // names this range's authored endpoint as a `u64` value.
        let statement_ordinal = u32::try_from(self.statement_index).ok()?;
        let site = checked_trees::CheckedSubsliceSite::Expression { expression };
        let endpoint = |endpoint: ExpressionHandle, role| {
            if !endpoint.is_valid() {
                return Some(None);
            }
            let (binding, value) =
                self.pure
                    .bound_expression_at(self.state, statement_ordinal, role)?;
            (binding.expression == endpoint
                && !binding.destination.is_valid()
                && value.primitive_type() == Some(typed_trees::types::PrimitiveType::U64))
            .then(|| Some(value.clone()))
        };
        let start = endpoint(
            range.start,
            checked_trees::CheckedScalarExpressionRole::SubsliceStart { site },
        )?;
        let end = endpoint(
            range.end,
            checked_trees::CheckedScalarExpressionRole::SubsliceEnd { site },
        )?;
        let source = if bytes {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root,
                expression,
                start,
                end,
            }
        } else {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root,
                expression,
                start,
                end,
            }
        };
        let source = checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity,
            access,
        };
        Some(if access == CheckedStructuralAccess::SharedBorrow {
            CheckedStructuralValueKind::BorrowedSliceView {
                source,
                region: checked_trees::CheckedElementViewRegion::WholeCollection,
            }
        } else {
            CheckedStructuralValueKind::MutableSliceView {
                source,
                region: checked_trees::CheckedElementViewRegion::WholeCollection,
            }
        })
    }
}

/// The element type of an owned or borrowed contiguous collection place.
/// A fixed array declares its extent and a slice carries a stored one; both
/// lend the same elements, so both can back a view.
fn lent_collection_element(
    program: &TypedTrees,
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
