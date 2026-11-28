//! Resolve an array's actual element type and closed extent once. Array
//! elements keep the canonical indexed namespace; they never acquire invented
//! field declarations or a nominal schema.
use crate::read::refuse_at;

mod argument_types;

use crate::body::Qualification;
use crate::source_state::SourceTypeApplication;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, TableIndexedExpression};
use crate::source_state::types::{FixedArrayLength, TypeReferenceHandle};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::ArrayExtent;
use terminal_semantics::CollectionExtent;

use super::authority::Access;
use super::{Program, Slot, places::RecordSource};
use crate::{
    Expression, ExpressionKind, RecordType, SourceOrigin, StorageShape, Unserved, ValueType,
};

// Retain the same scoped type/region identity used by byte views. A bare
// sequence is affine; an explicit shared loan is unrestricted. Neither kind
// supplies elements or backing merely by appearing in a declaration.
pub(super) struct SourceSequenceView {
    pub access: Option<terminal_psi::StructuralAccess>,
    pub identity: String,
}

pub(super) fn source_sequence_view(
    typed: &crate::source_state::TypedTrees,
    application: &SourceTypeApplication,
    view: &crate::source_state::SourceSliceApplication,
    lexical_owner: SymbolHandle,
    machine: MachineId,
) -> Checked<Option<SourceSequenceView>> {
    let fail = |reason| refuse_at(machine, Unserved::Type(reason));
    // Non-byte qualification needs its own predicate owner. Keep the existing
    // refusal instead of dropping a domain while selecting an element view.
    if view.referent.reference != view.slice.reference {
        return Ok(None);
    }
    let access = match view.access {
        None => None,
        Some(language_core::ReferenceAccess::Shared) => {
            Some(terminal_psi::StructuralAccess::SharedBorrow)
        }
        Some(language_core::ReferenceAccess::Mutable) => {
            Some(terminal_psi::StructuralAccess::MutableBorrow)
        }
        Some(language_core::ReferenceAccess::WriteOnly) => {
            return Err(fail("write-only typed sequence"));
        }
    };
    let identity = if access.is_some() {
        let reference = view.reference.reference;
        let retained = typed.type_reference_table.operand_lifetime_owner(reference);
        let owner = if retained.is_valid() {
            retained
        } else {
            lexical_owner
        };
        super::places::reference_lifetime_declared(typed, machine, reference, owner)?;
        application
            .scope
            .type_identity_in_region_scope(typed, application.reference, owner)
            .and_then(|identity| typed.type_identity_text(identity))
    } else {
        typed.type_identity_text(application.type_identity(typed))
    }
    .filter(|identity| !identity.is_empty())
    .ok_or_else(|| fail("sequence has no complete owned type/region identity"))?;
    Ok(Some(SourceSequenceView { access, identity }))
}

impl Program<'_> {
    /// A symbolic extent names a constant input of this checking body. Its
    /// scalar value is not a layout or an input owned by another machine.
    pub(super) fn extent_input(
        &self,
        machine: MachineId,
        owner: MachineId,
        value: semantic_vocabulary::ValueId,
    ) -> Checked<super::ConstBinding> {
        let unavailable = |detail| refuse_at(machine, Unserved::Type(detail));
        if owner != machine {
            return Err(unavailable(
                "fixed-array length names another machine's input",
            ));
        }
        let binding = self
            .const_scope
            .iter()
            .find(|binding| binding.owner == owner && binding.value == value)
            .copied()
            .ok_or_else(|| unavailable("fixed-array length has no owned constant input"))?;
        if !matches!(binding.scalar_type, ScalarType::Integer(integer)
            if !integer.is_address() && integer.sign() == IntegerSign::Unsigned
                && integer.bits().is_some_and(|bits| bits <= 64))
        {
            return Err(unavailable(
                "fixed-array length needs an unsigned input of at most 64 bits",
            ));
        }
        Ok(binding)
    }

    /// The exact element storage of a closed byte array. Whole-value domains
    /// stay on its source expression and do not change this storage geometry;
    /// only an explicit range forms a descriptor loan.
    pub(super) fn byte_array_element(&self, actual: Option<ValueType>) -> Option<StructuralTypeId> {
        let Some(ValueType::Record(actual, _, _)) = actual else {
            return None;
        };
        let actual = self.records.get(actual)?;
        let StorageShape::FixedArray {
            element,
            qualification: Qualification::NONE,
            length:
                ArrayExtent::Literal(_) | ArrayExtent::Parameter { .. } | ArrayExtent::Application(_),
        } = actual.shape
        else {
            return None;
        };
        (actual.requires.is_empty() && self.records.get(element).is_some_and(|element|
            element.requires.is_empty() && matches!(element.shape,
                StorageShape::PrimitiveScalar { scalar_type: ScalarType::Integer(integer), qualification: Qualification::NONE }
                    if integer.bits() == Some(8) && integer.sign() == IntegerSign::Unsigned && !integer.is_address())))
            .then_some(element)
    }

    /// The element of fixed storage or an existing readable view with the
    /// identical element and qualification. Reborrowing a view preserves its
    /// own extent and backing; `WholeCollection` keeps fixed-array types
    /// (byte_views.md, "Collection view establishment"); this reader admits
    /// no layout-equal substitute, so a different element row is refused.
    fn typed_array_element(
        &self,
        slots: &[Slot],
        base: &Expression,
        element: StructuralTypeId,
        qualification: Qualification,
    ) -> Option<StructuralTypeId> {
        let Some(ValueType::Record(actual, Qualification::NONE, _)) = base.result else {
            return None;
        };
        let actual = self.records.get(actual)?;
        let (stored, stored_qualification) = match actual.shape {
            StorageShape::FixedArray {
                element,
                qualification,
                length:
                    ArrayExtent::Literal(_)
                    | ArrayExtent::Parameter { .. }
                    | ArrayExtent::Application(_),
            }
            | StorageShape::ElementView {
                element,
                qualification,
                access:
                    Some(
                        terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow,
                    ),
            } => (element, qualification),
            StorageShape::ElementView {
                element,
                qualification,
                access: None,
            } if self.direct_element_view_formal(slots, base) => (element, qualification),
            _ => return None,
        };
        (actual.requires.is_empty() && stored == element && stored_qualification == qualification)
            .then_some(element)
    }

    /// Borrow a range of a closed array using the backing's shape. A `u8`
    /// destination forms a byte descriptor; an `@[T]` destination forms the
    /// typed element view of the same backing. Both are the one
    /// `CollectionView` formation, which Terminal Psi establishes as a
    /// `WholeCollection` element view whose length is the array's exact
    /// extent. A partial range retains that backing through the descriptor's
    /// checked subslice operation.
    pub(super) fn borrowed_array_range(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        argument: ExpressionHandle,
        expected: ValueType,
        source: SourceOrigin,
        formal_access: Option<language_core::ReferenceAccess>,
    ) -> Checked<Option<Expression>> {
        use language_core::ReferenceAccess;
        use terminal_psi::{ByteSequenceCarrier, StructuralAccess};

        let unavailable = |detail| refuse_at(machine, Unserved::Expression(detail));
        // expressions.md, "Borrowed range access": "a bare range
        // `items[start..end]` forms a shared view when received as a
        // reference". So a bare range lends exactly what `@items[start..end]`
        // lends. Read the actual backing once below: a fixed array forms a
        // descriptor, while a byte view keeps its checked subslice operation.
        let (target, written_access) = match self.typed.expression_table.borrow_expression(argument)
        {
            Some(borrow) => (borrow.target, borrow.access),
            None => (argument, ReferenceAccess::Shared),
        };
        let Some(indexed) = self.typed.expression_table.indexed_node(target) else {
            return Ok(None);
        };
        let Some(range) = self.typed.expression_table.range_node(indexed.index) else {
            return Ok(None);
        };
        let ValueType::Record(mut view, expected_qualification, _) = expected else {
            return Ok(None);
        };
        // Formation owns a stored descriptor with the written loan. Only the
        // callee's direct formal carries its access separately from its shape.
        if formal_access == Some(ReferenceAccess::Mutable)
            && matches!(
                self.data_type(machine, view)?.shape,
                StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access: None })
                    | StorageShape::ElementView { access: None, .. }
            )
        {
            let reference = self
                .typed
                .expression_table
                .constructed_value_type_reference(argument);
            let Some(ValueType::Record(actual, Qualification::NONE, _)) =
                self.view_type(machine, reference)?
            else {
                return Err(unavailable("array view without its retained source type"));
            };
            if !matches!(
                self.data_type(machine, actual)?.shape,
                StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                }) | StorageShape::ElementView {
                    access: Some(StructuralAccess::MutableBorrow),
                    ..
                }
            ) {
                return Err(crate::reject(
                    machine,
                    source,
                    "the explicit array view lends only immutable access; a mutable parameter cannot grant writable access to that backing",
                ));
            }
            view = actual;
        }
        // A typed view names its element row; a byte view's element is the
        // backing's own `u8` row, checked below.
        let (access, typed_element) = match self.data_type(machine, view)?.shape {
            StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(access),
            }) => (access, None),
            StorageShape::ElementView {
                element,
                qualification,
                access: Some(access),
            } => (access, Some((element, qualification))),
            _ => return Ok(None),
        };
        let requested = match written_access {
            ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
            ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
            ReferenceAccess::WriteOnly => {
                return Err(unavailable("write-only array view formation"));
            }
        };
        if access != requested {
            if requested == StructuralAccess::SharedBorrow
                && access == StructuralAccess::MutableBorrow
            {
                return Err(crate::reject(
                    machine,
                    source,
                    "the explicit array view lends only immutable access; a mutable parameter cannot grant writable access to that backing",
                ));
            }
            // The formation keeps the written access and its actual source
            // type. A call may lend this descriptor's backing for observation;
            // the destination never changes the loan being formed here.
            let reference = self
                .typed
                .expression_table
                .constructed_value_type_reference(argument);
            let Some(ValueType::Record(actual, Qualification::NONE, _)) =
                self.view_type(machine, reference)?
            else {
                return Err(unavailable("array view without its retained source type"));
            };
            if self.data_type(machine, actual)?.shape
                != StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(requested),
                })
            {
                return Err(unavailable(
                    "array view differs from its written source type",
                ));
            }
            if typed_element.is_some() {
                return Err(unavailable(
                    "typed array view whose written access differs from its destination",
                ));
            }
            view = actual;
        }
        let backing_access = if written_access == ReferenceAccess::Mutable {
            Access::Write
        } else {
            Access::Borrow
        };
        let mut base = self.place(machine, slots, indexed.collection, None, backing_access)?;
        if let Some((referent, _)) = self.reference_referent(
            machine,
            base.result
                .ok_or_else(|| unavailable("array view without backing storage"))?,
        )? {
            base = Expression {
                call_context: None,
                source: base.source,
                kind: ExpressionKind::Referent(Box::new(base)),
                result: Some(referent),
            };
        }
        let Some(backing_type @ ValueType::Record(backing, _, _)) = base.result else {
            return Err(unavailable("array view without a plain closed u8 array"));
        };
        self.complete_executable_range(machine, target, indexed.collection, &base)?;
        let byte_view = typed_element.is_none() && self.is_readable_byte_view(machine, backing)?;
        let partial = range.start.is_valid() || range.end.is_valid() || range.end_inclusive;
        // A plain destination cannot erase a governed or non-byte source's
        // requirements. Validate actual backing qualifications independently
        // of which predicates the destination requests.
        let source_predicates = if backing_type.qualification() != Qualification::NONE {
            if typed_element.is_some()
                || !(byte_view || self.byte_array_element(base.result).is_some())
            {
                return Err(unavailable(
                    "qualified range backing is not served byte storage",
                ));
            }
            self.byte_qualification_predicates(machine, backing_type.qualification())?
        } else {
            semantic_vocabulary::ByteSequencePredicateSet::EMPTY
        };
        if expected_qualification != Qualification::NONE {
            if typed_element.is_some() || requested != StructuralAccess::SharedBorrow {
                return Err(unavailable(
                    "qualified range without a shared byte descriptor",
                ));
            }
            let retained = if partial {
                source_predicates.subslice()
            } else {
                source_predicates
            };
            let required = self.byte_qualification_predicates(machine, expected_qualification)?;
            if !retained.implies(required) {
                return Err(unavailable(
                    "range domains not preserved by its actual byte backing",
                ));
            }
        }
        // Establish the plain descriptor first, then prove its requested
        // presentation through the ordinary qualification edge. A result
        // annotation alone cannot establish predicates of the selected bytes.
        let present = |reader: &mut Self, expression: Expression| -> Checked<Expression> {
            let Some(ValueType::Record(row, Qualification::NONE, regions)) = expression.result
            else {
                return Err(unavailable("range lost its actual descriptor"));
            };
            if expected_qualification == Qualification::NONE {
                return Ok(expression);
            }
            let result = ValueType::Record(row, expected_qualification, regions);
            let (clauses, stated) =
                reader.record_cast_predicates(machine, expected_qualification, result)?;
            let kind = if clauses.is_empty() {
                ExpressionKind::Requalify(Box::new(expression))
            } else {
                ExpressionKind::Establish {
                    operand: Box::new(expression),
                    clauses,
                    stated: terminal_check_tables::program::EstablishedCondition::DomainMembership(
                        stated,
                    ),
                }
            };
            Ok(Expression {
                call_context: None,
                kind,
                result: Some(result),
                source,
            })
        };
        // A shared descriptor can snapshot its unrestricted value. A child
        // of an exclusive descriptor instead keeps a persistent payload loan,
        // including when that child only observes the bytes.
        if byte_view
            && requested == StructuralAccess::SharedBorrow
            && self.observes_only(machine, base.result)?
            && self.data_type(machine, backing)?.is_copyable()
        {
            self.authorized(machine, backing_type, Access::Read)?;
            let value = self.byte_subslice(machine, slots, base, range, target, source)?;
            return present(self, value).map(Some);
        }
        // A partial range of fixed storage is the window of the whole-array
        // loan: the loan keeps the backing's exact extent `N`, and the
        // window's `start <= end <= N` legs are checked against that loan's
        // own length like any other subslice (byte_views.md, "Source
        // correspondence"). A writable window also needs to keep its
        // exclusive descriptor live through the subslice.
        let element = match typed_element {
            None if byte_view => {
                let element = self.byte_view_element_type(machine, backing)?;
                let value = self.value_type(machine, element)?;
                self.storage_type(machine, element, value)?
            }
            None => self
                .byte_array_element(base.result)
                .ok_or_else(|| unavailable("array view without a plain closed u8 array"))?,
            Some((element, qualification)) => self
                .typed_array_element(slots, &base, element, qualification)
                .ok_or_else(|| {
                    unavailable("typed array view without a closed array of its exact element")
                })?,
        };
        self.collection_range_selection(machine, target, backing)?;
        let crossed = self.reference_place_access(machine, slots, &base, written_access)?;
        if !byte_view && !crossed && written_access == ReferenceAccess::Mutable {
            // A crossed reference already supplied its access ceiling. Only
            // directly held array storage needs its local's mutability here.
            let slot = super::storage_path::root_slot(machine, &base, &self.captures)?
                .ok_or_else(|| unavailable("array view without an existing backing place"))?;
            if slots[slot].borrowed == Some(ReferenceAccess::Shared)
                || slots[slot].borrowed.is_none() && !slots[slot].assignable
            {
                return Err(unavailable(
                    "mutable array view without exclusive backing access",
                ));
            }
        }
        self.authorized(
            machine,
            backing_type,
            if written_access == ReferenceAccess::Mutable {
                Access::Write
            } else {
                Access::Read
            },
        )?;
        if written_access == ReferenceAccess::Mutable {
            self.written_through(machine, &base)?;
        }
        let mut regions = self.storage_loan_regions(machine, slots, &base)?;
        if typed_element.is_some() {
            // Reborrowing replaces the view's outer loan. Only the element
            // regions survive from its existing descriptor.
            let elements = self.collection_element_regions(
                machine,
                backing,
                backing_type.regions(),
                self.direct_element_view_formal(slots, &base),
            )?;
            let mut tuple = self.region_tuple(regions).to_vec();
            tuple.extend_from_slice(self.region_tuple(elements));
            regions = self.intern_regions(tuple);
        }
        // Lend the actual collection before evaluating endpoints. A mutable
        // descriptor must reborrow its payload, not move out of its caller's
        // storage and leave a restoration debt there.
        let whole = Expression {
            call_context: None,
            kind: ExpressionKind::CollectionView {
                base: Box::new(base),
                access: written_access,
                element,
            },
            result: Some(ValueType::Record(view, Qualification::NONE, regions)),
            source,
        };
        let formed = if partial {
            self.collection_window(machine, slots, whole, range, source)?
        } else {
            whole
        };
        present(self, formed).map(Some)
    }

    /// The fixed array storage `base` lends, when it is a shared borrow of an
    /// array place, a borrowed array formal, the referent of a held array
    /// reference, or a reference to an array. Anything else comes back
    /// unchanged.
    fn fixed_array_backing(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        base: Expression,
    ) -> Checked<Result<Expression, Expression>> {
        let is_array = |reader: &mut Self, value: Option<ValueType>| -> Checked<bool> {
            Ok(match value {
                Some(ValueType::Record(row, _, _)) => matches!(
                    reader.data_type(machine, row)?.shape,
                    StorageShape::FixedArray { .. }
                ),
                _ => false,
            })
        };
        if let ExpressionKind::Borrow {
            access: language_core::ReferenceAccess::Shared,
            place,
            ..
        } = &base.kind
            && is_array(self, place.result)?
        {
            let ExpressionKind::Borrow { place, .. } = base.kind else {
                unreachable!("matched above");
            };
            return Ok(Ok(*place));
        }
        let crosses_reference = match base.kind {
            ExpressionKind::Slot(slot) => {
                slots.get(slot).is_some_and(|slot| slot.borrowed.is_some())
            }
            ExpressionKind::Referent(_) => true,
            _ => false,
        };
        if crosses_reference && is_array(self, base.result)? {
            return Ok(Ok(base));
        }
        if let Some(value) = base.result
            && let Some((referent, _)) = self.reference_referent(machine, value)?
            && is_array(self, Some(referent))?
        {
            return Ok(Ok(Expression {
                call_context: None,
                source: base.source,
                kind: ExpressionKind::Referent(Box::new(base)),
                result: Some(referent),
            }));
        }
        Ok(Err(base))
    }

    /// `window as @[T; N]` and `@items[a..b] as @[T; N]`: a shared reference
    /// to the whole of a view as one fixed array, which owes the view's
    /// length equal to `N` (owner ruling on slice and fixed-array casts). A
    /// cast whose operand is not a view, or whose target is not a shared
    /// fixed-array reference, is not this form.
    pub(super) fn view_fixed_array(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        cast: &crate::source_state::expression::TableCastExpression,
        expected: ValueType,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        use terminal_psi::StructuralAccess;
        let unavailable = |detail| refuse_at(machine, Unserved::Expression(detail));
        let Some((ValueType::Record(array, _, _), StructuralAccess::SharedBorrow)) =
            self.reference_referent(machine, expected)?
        else {
            return Ok(None);
        };
        let StorageShape::FixedArray {
            element: array_element,
            length,
            ..
        } = self.data_type(machine, array)?.shape
        else {
            return Ok(None);
        };
        // The operand is a view when it names a view binding or constructs
        // one, such as a range.
        let named = match self.typed.expression_table.expression(cast.value) {
            ExpressionNode::Name(path) => self
                .slot_of(machine, slots, path)
                .ok()
                .map(|slot| slots[slot].value_type),
            _ => None,
        };
        let view = match named {
            Some(value) => value,
            None => {
                let operand = self
                    .typed
                    .expression_table
                    .constructed_value_type_reference(cast.value);
                if !self
                    .typed
                    .type_reference_table
                    .contains_type_reference(operand)
                {
                    return Ok(None);
                }
                let Some(view) = self.view_type(machine, operand)? else {
                    return Ok(None);
                };
                view
            }
        };
        let ValueType::Record(view_row, _, _) = view else {
            return Ok(None);
        };
        if !matches!(
            self.data_type(machine, view_row)?.shape,
            StorageShape::ElementView { .. }
                | StorageShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
        ) {
            return Ok(None);
        }
        let same_element = match self.data_type(machine, view_row)?.shape {
            StorageShape::ElementView {
                element,
                qualification: Qualification::NONE,
                access: Some(StructuralAccess::SharedBorrow),
            } => element == array_element,
            StorageShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }) => matches!(
                self.data_type(machine, array_element)?.shape,
                StorageShape::PrimitiveScalar {
                    scalar_type: ScalarType::Integer(integer),
                    qualification: Qualification::NONE,
                } if integer == IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid")
            ),
            _ => {
                return Err(unavailable(
                    "fixed-array reference to a view that is not a plain shared view",
                ));
            }
        };
        if !same_element {
            return Err(unavailable(
                "fixed-array reference whose element differs from its view's",
            ));
        }
        let Some(extent) = length.literal() else {
            return Err(unavailable("fixed-array reference to a symbolic extent"));
        };
        let base =
            match self.borrowed_array_range(machine, slots, cast.value, view, source, None)? {
                Some(range) => range,
                None => self.expression(machine, slots, cast.value, Some(view))?,
            };
        let Some(viewed @ ValueType::Record(backing, Qualification::NONE, _)) = base.result else {
            return Err(unavailable("fixed-array reference without a plain view"));
        };
        if !self.is_borrowed_view(viewed)
            || self.data_type(machine, backing)?.shape != self.data_type(machine, view_row)?.shape
        {
            return Err(unavailable(
                "fixed-array reference without its shared element view",
            ));
        }
        self.representation_read_authorized(machine, &base)?;
        let loan = self.loan_regions(machine, slots, &base)?;
        let ValueType::Record(reference, qualification, declared) = expected else {
            unreachable!("a reference type is a record")
        };
        let mut regions = self.region_tuple(loan).to_vec();
        regions.extend(self.region_tuple(declared).iter().skip(1).copied());
        let regions = self.intern_regions(regions);
        Ok(Some(Expression {
            kind: ExpressionKind::CollectionFixedArray {
                base: Box::new(base),
                extent,
            },
            result: Some(ValueType::Record(reference, qualification, regions)),
            call_context: None,
            source,
        }))
    }

    /// A collection recast borrows the same captured window. Its destination
    /// supplies only presentation; geometry, total validity and count fit are
    /// judged on the resulting representation-view operation.
    pub(super) fn collection_recast(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        cast: &crate::source_state::expression::TableCastExpression,
        reference: TypeReferenceHandle,
        target: ValueType,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use language_core::ReferenceAccess;
        use terminal_psi::{ByteSequenceCarrier, StructuralAccess};
        let unavailable = |detail| refuse_at(machine, Unserved::Expression(detail));
        if cast.form.recast_access() != Some(ReferenceAccess::Shared) {
            return Err(unavailable(
                "exclusive storage view without establishment and handback evidence",
            ));
        }
        if cast.semantic_domain_symbol.is_valid()
            || !cast.semantic_domain.is_empty()
            || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
        {
            return Err(unavailable("storage view that also changes a domain"));
        }
        let ValueType::Record(view, Qualification::NONE, _) = target else {
            return Err(unavailable(
                "collection recast without a plain view destination",
            ));
        };
        let element = match self.data_type(machine, view)?.shape {
            StorageShape::ElementView {
                element,
                qualification: Qualification::NONE,
                access: Some(StructuralAccess::SharedBorrow),
            } => element,
            StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }) => {
                let view = super::source_views::source_view(
                    self.typed,
                    self.storage_source_application(reference),
                    self.lifetime_owner,
                    machine,
                )?
                .ok_or_else(|| unavailable("collection recast lost its byte element"))?;
                if !matches!(view.kind, super::source_views::SourceViewKind::Bytes(_)) {
                    return Err(unavailable("collection recast changed its byte carrier"));
                }
                let element = view.slice.element.reference;
                let value = self.value_type(machine, element)?;
                self.storage_type(machine, element, value)?
            }
            _ => {
                return Err(unavailable(
                    "collection recast without a shared destination",
                ));
            }
        };
        let base = self.expression(machine, slots, cast.value, None)?;
        // A borrowed fixed array presents its whole extent: the view covers
        // all `N` elements of the same backing, and its length comes from the
        // array type (owner ruling on slice and fixed-array casts).
        let base = match self.fixed_array_backing(machine, slots, base)? {
            Ok(fixed) => {
                let backing = fixed
                    .result
                    .ok_or_else(|| unavailable("fixed array recast without backing storage"))?;
                self.authorized(machine, backing, Access::Read)?;
                self.representation_read_authorized(machine, &fixed)?;
                let regions = self.storage_loan_regions(machine, slots, &fixed)?;
                return Ok(Expression {
                    kind: ExpressionKind::CollectionView {
                        base: Box::new(fixed),
                        access: ReferenceAccess::Shared,
                        element,
                    },
                    result: Some(ValueType::Record(view, Qualification::NONE, regions)),
                    call_context: None,
                    source,
                });
            }
            Err(base) => base,
        };
        let Some(viewed @ ValueType::Record(backing, Qualification::NONE, _)) = base.result else {
            return Err(unavailable(
                "collection recast without plain collection backing",
            ));
        };
        if !self.is_borrowed_view(viewed)
            || !matches!(
                self.data_type(machine, backing)?.shape,
                StorageShape::ElementView {
                    qualification: Qualification::NONE,
                    access: Some(StructuralAccess::SharedBorrow),
                    ..
                }
            )
        {
            return Err(unavailable(
                "collection recast without a shared collection descriptor",
            ));
        }
        self.representation_read_authorized(machine, &base)?;
        let regions = self.loan_regions(machine, slots, &base)?;
        Ok(Expression {
            kind: ExpressionKind::CollectionView {
                base: Box::new(base),
                access: ReferenceAccess::Shared,
                element,
            },
            result: Some(ValueType::Record(view, Qualification::NONE, regions)),
            call_context: None,
            source,
        })
    }

    pub(super) fn array_literal(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        supplied: arena::HandleSpan<ExpressionHandle>,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        // A literal constructs the bare carrier, whatever its context
        // expects: a qualified destination then refuses the plain value.
        let Some(ValueType::Record(array, _, regions)) = expected else {
            return Err(refuse_at(
                machine,
                Unserved::Type("array literal without a closed contextual type"),
            ));
        };
        let (element, qualification, extent) =
            self.array_element(machine, super::authority::Opened::constructing(array))?;
        // An array value holds its elements' regions (`array_type`).
        let element_regions = self.collection_element_regions(machine, array, regions, false)?;
        let declared = super::places::with_regions(
            self.storage_value_type(machine, element, qualification)?,
            element_regions,
        );
        let supplied = self.typed.expression_table.expression_handles(supplied);
        if let CollectionExtent::Fixed(extent) = extent
            && u64::try_from(supplied.len()).ok() != Some(extent)
        {
            return Err(refuse_at(machine, Unserved::Typing("array literal extent")));
        }
        let mut elements = Vec::with_capacity(supplied.len());
        for handle in supplied {
            let element = self.expression(machine, slots, *handle, Some(declared))?;
            if !self.agrees(element.result, Some(declared)) {
                return Err(self.mistyped(
                    machine,
                    element.source,
                    super::rejections::Destination::ArrayElement,
                    Some(declared),
                    element.result,
                    "array literal element",
                ));
            }
            elements.push(element);
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Array { elements },
            result: Some(ValueType::Record(array, Qualification::NONE, regions)),
            source,
        })
    }

    pub(super) fn indexed(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        indexed: &TableIndexedExpression,
        handle: ExpressionHandle,
        expected: Option<ValueType>,
        access: Access,
        element_read: ElementRead,
    ) -> Checked<Expression> {
        let source = SourceOrigin::of(self.typed.expression_table.source_span(handle));
        // A range preserves the backing region of its operand. Context can
        // instantiate that region (including immutable literal backing), but
        // never selects an operator or grants mutable access.
        let hint = expected.filter(|value| matches!(value, ValueType::Record(row, _, _)
            if matches!(self.records.get(*row).map(|row| &row.shape), Some(StorageShape::ByteSequence(carrier)) if carrier.is_shared_readable_view())));
        let range = self.typed.expression_table.range_node(indexed.index);
        let hint = range.and(hint);
        let base = self.place(machine, slots, indexed.collection, hint, access)?;
        // A domain cast preserves the reference descriptor. Indexing projects
        // into its referent, including when the cast removed a domain.
        let base = if matches!(
            self.node(machine, indexed.collection)?,
            ExpressionNode::Cast(_)
        ) {
            self.observed(machine, base, hint)?
        } else {
            base
        };
        let Some(array) = self.opened_place(machine, &base, access)? else {
            return Err(refuse_at(
                machine,
                Unserved::Typing("index into non-array storage"),
            ));
        };
        let byte_view = self.is_readable_byte_view(machine, array.record())?;
        if let Some(range) = range {
            self.complete_executable_range(machine, handle, indexed.collection, &base)?;
            if byte_view {
                return self.byte_subslice(machine, slots, base, range, handle, source);
            }
            // A shared typed view keeps its backing region. An exclusive
            // descriptor requires a live reborrow through the whole window.
            if matches!(
                self.data_type(machine, array.record())?.shape,
                StorageShape::ElementView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    ..
                }
            ) {
                self.collection_range_selection(machine, handle, array.record())?;
                return self.collection_window(machine, slots, base, range, source);
            }
            return Err(refuse_at(
                machine,
                Unserved::Expression("range of a collection that is not a shared view"),
            ));
        }
        if byte_view {
            return self.byte_element(machine, slots, base, indexed.index, source);
        }
        let (element, qualification, extent) = self.array_element(machine, array)?;
        let stored_length = matches!(extent, CollectionExtent::StoredLength);
        // Reading a whole slice element copies it out of the view, so the
        // element type must be copyable. A field projection reads only the
        // selected field; the lowering copies that field from the element's
        // place and still refuses an owned extraction from borrowed storage.
        if stored_length
            && access == Access::Read
            && element_read == ElementRead::Whole
            && !self.data_type(machine, element)?.is_copyable()
        {
            return Err(refuse_at(
                machine,
                Unserved::Typing("copying a slice element requires a copyable element type"),
            ));
        }
        let regions = base
            .result
            .map_or(crate::body::Regions::NONE, ValueType::regions);
        let element_regions = self.collection_element_regions(
            machine,
            array.record(),
            regions,
            self.direct_element_view_formal(slots, &base),
        )?;
        let result = super::places::with_regions(
            self.storage_value_type(machine, element, qualification)?,
            element_regions,
        );
        let index_type = IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| refuse_at(machine, Unserved::Type("u64 index carrier")))?;
        if stored_length {
            let operands = self.collection_index_operand_types(machine, array.record());
            if !crate::source_state::operator::builtin_index_read_is_spelled(
                self.typed,
                self.lifetime_owner,
                handle,
                &operands,
            ) {
                return Err(refuse_at(
                    machine,
                    Unserved::Typing("slice index without its selected core operation"),
                ));
            }
        }
        let index = self.expression(
            machine,
            slots,
            indexed.index,
            Some(ValueType::Scalar(
                ScalarType::Integer(index_type),
                Qualification::NONE,
            )),
        )?;
        if !matches!(index.result.and_then(ValueType::scalar), Some(ScalarType::Integer(integer)) if !integer.is_address())
        {
            return Err(refuse_at(
                machine,
                Unserved::Typing("array index is not a fixed integer"),
            ));
        }
        if stored_length
            && index.result.and_then(ValueType::scalar) != Some(ScalarType::Integer(index_type))
        {
            return Err(refuse_at(
                machine,
                Unserved::Typing("slice index that is not a u64"),
            ));
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Index {
                base: Box::new(base),
                index: Box::new(index),
            },
            result: Some(result),
            source,
        })
    }

    pub(super) fn array_element(
        &self,
        machine: MachineId,
        array: super::authority::Opened,
    ) -> Checked<(StructuralTypeId, Qualification, CollectionExtent)> {
        match &self.data_type(machine, array.record())?.shape {
            StorageShape::FixedArray {
                element,
                qualification,
                length: ArrayExtent::Literal(length),
            } => Ok((*element, *qualification, CollectionExtent::Fixed(*length))),
            StorageShape::FixedArray {
                element,
                qualification,
                length: ArrayExtent::Parameter { owner, value },
            } => Ok((
                *element,
                *qualification,
                CollectionExtent::Parameter {
                    owner: *owner,
                    value: *value,
                },
            )),
            StorageShape::FixedArray {
                element,
                qualification,
                length: ArrayExtent::Application(application),
            } => Ok((
                *element,
                *qualification,
                CollectionExtent::Application(*application),
            )),
            StorageShape::ElementView {
                element,
                qualification,
                ..
            } => Ok((*element, *qualification, CollectionExtent::StoredLength)),
            _ => Err(refuse_at(
                machine,
                Unserved::Typing("index or literal of a non-array type"),
            )),
        }
    }

    pub(super) fn storage_value_type(
        &self,
        machine: MachineId,
        storage: StructuralTypeId,
        qualification: Qualification,
    ) -> Checked<ValueType> {
        Ok(match self.data_type(machine, storage)?.shape {
            StorageShape::PrimitiveScalar {
                scalar_type,
                qualification,
            } => ValueType::Scalar(scalar_type, qualification),
            _ => ValueType::Record(storage, qualification, crate::body::Regions::NONE),
        })
    }

    /// A source sequence keeps its exact element and type identity. This row
    /// supplies no elements, lifetime or backing: those belong to construction
    /// and transport through the existing canonical ElementView operations.
    pub(super) fn sequence_type(
        &mut self,
        machine: MachineId,
        view: super::source_views::SourceView,
        formal_access: bool,
    ) -> Checked<ValueType> {
        let super::source_views::SourceViewKind::Elements(SourceSequenceView { access, identity }) =
            view.kind
        else {
            return Err(refuse_at(
                machine,
                Unserved::Type("element view changed its selected carrier"),
            ));
        };
        let application = view.source;
        let reference = application.reference;
        let mut tuple = Vec::new();
        if access.is_some() {
            tuple.push(self.borrowed_view_region(
                machine,
                view.slice.reference.reference,
                self.lifetime_owner,
                &identity,
            )?);
        }
        // A supplied descriptor already retains its exact element carrier.
        // Borrow that type, rather than rereading a caller's binder as a new
        // binder of the receiving machine.
        let (element, qualification, element_regions) = if access.is_some() {
            let crate::source_state::types::TypeReferenceNode::Reference { referee, .. } = self
                .typed
                .type_reference_table
                .type_reference(view.slice.reference.reference)
            else {
                return Err(refuse_at(
                    machine,
                    Unserved::Type("sequence loan lost its source reference"),
                ));
            };
            let value = self.value_type(machine, *referee)?;
            let ValueType::Record(record, _, _) = value else {
                return Err(refuse_at(
                    machine,
                    Unserved::Type("sequence loan lost its element descriptor"),
                ));
            };
            let StorageShape::ElementView {
                element,
                qualification,
                access: None,
            } = self.data_type(machine, record)?.shape
            else {
                return Err(refuse_at(
                    machine,
                    Unserved::Type("sequence loan requires its exact unborrowed slice type"),
                ));
            };
            if formal_access && access == Some(terminal_psi::StructuralAccess::MutableBorrow) {
                // A direct slice parameter borrows elements. Its parameter
                // supplies the access, while the carrier supplies the element.
                tuple.extend_from_slice(self.region_tuple(value.regions()));
                return Ok(ValueType::Record(
                    record,
                    Qualification::NONE,
                    self.intern_regions(tuple),
                ));
            }
            (element, qualification, value.regions())
        } else {
            let element = view.slice.element;
            let outer = std::mem::replace(&mut self.source_scope, element.scope.clone());
            let read: Checked<_> = (|| {
                let value = self.value_type(machine, element.reference)?;
                let (storage, qualification) =
                    self.collection_element_storage(machine, element.reference, value)?;
                Ok((storage, qualification, value.regions()))
            })();
            self.source_scope = outer;
            read?
        };
        tuple.extend_from_slice(self.region_tuple(element_regions));
        let regions = self.intern_regions(tuple);
        let shape = StorageShape::ElementView {
            element,
            qualification,
            access,
        };
        if let Some((id, _)) = self.records.iter().find(|(_, row)| row.shape == shape) {
            return Ok(ValueType::Record(id, Qualification::NONE, regions));
        }
        let constant_input_owner = self.storage_constant_input_owner(machine, &shape)?;
        let identity = super::places::storage_identity_for_inputs(identity, constant_input_owner);
        let record_type = RecordType {
            multiplicity: crate::body::source_multiplicity(
                self.typed.stored_value_multiplicity(reference),
            ),
            cleanup: None,
            identity,
            shape,
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        };
        let mut source = physical_source_application(application);
        source.constant_input_owner = constant_input_owner;
        source.region_owner = self.lifetime_owner;
        let id = self.push_record_row(record_type, source);
        Ok(ValueType::Record(id, Qualification::NONE, regions))
    }

    pub(super) fn array_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        element: TypeReferenceHandle,
        extent: &FixedArrayLength,
    ) -> Checked<ValueType> {
        use crate::source_state::constant_recipes::DeclaredArrayExtent;

        let unserved = |reason| refuse_at(machine, Unserved::Type(reason));
        // `[T; N]` names either a const parameter the enclosing declaration
        // binds, or a constant (`const N: u64 = measure();`). Only a constant
        // has a declaration to read a value from.
        let named_constant = matches!(extent, FixedArrayLength::ConstParameter { symbol, .. }
            if self.typed.symbols.symbols().nodes().is_valid(*symbol)
                && self.typed.symbols.get(*symbol).kind == symbols::SymbolKind::Const);
        let length = if let FixedArrayLength::ConstParameter { symbol, .. } = extent
            && !named_constant
        {
            let key = (symbol.arena_index(), symbol.generation());
            let position = self
                .const_scope
                .binary_search_by_key(&key, |binding| {
                    (binding.symbol.arena_index(), binding.symbol.generation())
                })
                .map_err(|_| unserved("array extent without an owned const input"))?;
            let binding = self.const_scope[position];
            let Some(integer) = binding.scalar_type.as_integer() else {
                return Err(unserved("array extent const input is not an integer"));
            };
            if integer.is_address()
                || integer.sign() != IntegerSign::Unsigned
                || integer.bits().is_none_or(|bits| bits > 64)
            {
                return Err(unserved(
                    "array extent const input needs an unsigned carrier of at most 64 bits",
                ));
            }
            ArrayExtent::Parameter {
                owner: binding.owner,
                value: binding.value,
            }
        } else {
            match self.typed.declared_array_extent(reference) {
                Some(DeclaredArrayExtent::Literal(length)) => ArrayExtent::Literal(length),
                Some(DeclaredArrayExtent::Recipe(recipe)) => {
                    let recipe = self.typed.constant_recipes.get(recipe);
                    let contexts = self
                        .typed
                        .constant_expression_contexts
                        .span_or_empty(recipe.contexts);
                    let Some([context]) = contexts.as_array() else {
                        return Err(unserved(
                            "array extent has no unique retained evaluation context",
                        ));
                    };
                    ArrayExtent::Application(
                        self.scalar_evaluation(
                            machine,
                            context.evaluation_entry,
                            context.evaluation_expression,
                            context.result_type,
                        )?
                        .0,
                    )
                }
                // The source driver evaluates a constant the request reaches
                // before stage 04 freezes the trees. One that observes the
                // target's layout keeps its checked initializer as an
                // application extent so each deployment closes its length.
                None if named_constant => {
                    if let FixedArrayLength::ConstParameter { symbol, .. } = extent
                        && let Some(constant) = self.typed.const_declaration_by_symbol(*symbol)
                        && constant.pending_target_observation
                        && constant.evaluated_value.is_none()
                    {
                        let (application, scalar_type) =
                            self.deferred_scalar_constant(machine, constant)?;
                        if !matches!(
                            scalar_type,
                            ScalarType::Integer(integer)
                                if !integer.is_address()
                                    && integer.sign() == IntegerSign::Unsigned
                                    && integer.bits().is_some_and(|bits| bits <= 64)
                        ) {
                            return Err(unserved(
                                "array extent constant needs an unsigned carrier of at most 64 bits",
                            ));
                        }
                        ArrayExtent::Application(application)
                    } else {
                        return Err(unserved(
                            "array extent that names a constant with no evaluated array length",
                        ));
                    }
                }
                None => return Err(unserved("unresolved array extent")),
            }
        };
        let value = self.value_type(machine, element)?;
        let (element, qualification) = self.collection_element_storage(machine, element, value)?;
        let shape = StorageShape::FixedArray {
            element,
            qualification,
            length,
        };
        if let Some((id, _)) = self.records.iter().find(|(_, row)| row.shape == shape) {
            return Ok(ValueType::Record(id, Qualification::NONE, value.regions()));
        }
        // The supplied element owns these permissions. The authored element
        // may still name a parameter whose default bound is more restrictive.
        let multiplicity = self.data_type(machine, element)?.multiplicity;
        let application = self.storage_source_application(reference);
        let identity = self.storage_identity(machine, &application)?;
        let constant_input_owner = self.storage_constant_input_owner(machine, &shape)?;
        let identity = super::places::storage_identity_for_inputs(identity, constant_input_owner);
        let record_type = RecordType {
            multiplicity,
            cleanup: None,
            identity,
            shape,
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        };
        let mut source = physical_source_application(application);
        source.constant_input_owner = constant_input_owner;
        let id = self.push_record_row(record_type, source);
        // The row is the element's carrier; the regions its elements hold
        // belong to the array value's type, as a record's do (`regions`).
        Ok(ValueType::Record(id, Qualification::NONE, value.regions()))
    }

    pub(super) fn collection_element_storage(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        value: ValueType,
    ) -> Checked<(StructuralTypeId, Qualification)> {
        match value {
            ValueType::Record(element, qualification, _) => Ok((element, qualification)),
            _ => Ok((
                self.storage_type(machine, reference, value)?,
                Qualification::NONE,
            )),
        }
    }

    pub(super) fn storage_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        value: ValueType,
    ) -> Checked<StructuralTypeId> {
        // A scalar slot keeps its complete qualification, so an element of
        // `[u8 in Mark; 4]` is stored, read and written as a `u8 in Mark`.
        // A storage description lays out bytes and needs only the carrier.
        let (scalar_type, qualification) = match value {
            ValueType::Scalar(scalar, qualification)
                if self.type_purpose
                    == super::storage_metadata::TypePurpose::StorageDescription =>
            {
                (
                    scalar,
                    Qualification {
                        domains: crate::body::DomainSet::EMPTY,
                        ..qualification
                    },
                )
            }
            ValueType::Scalar(scalar, qualification) => (scalar, qualification),
            ValueType::Record(id, Qualification::NONE, _) => return Ok(id),
            ValueType::Record(id, _, _)
                if self.type_purpose
                    == super::storage_metadata::TypePurpose::StorageDescription =>
            {
                return Ok(id);
            }
            ValueType::Record(..) => {
                return Err(refuse_at(
                    machine,
                    Unserved::Type("storage of a record that carries a declared domain"),
                ));
            }
            ValueType::Rigid(_) => {
                return Err(refuse_at(machine, Unserved::Type("rigid array element")));
            }
        };
        if let Some((id, _)) = self.records.iter().find(|(_, row)| {
            matches!(row.shape, StorageShape::PrimitiveScalar { scalar_type: known, qualification: prior }
                if known == scalar_type && prior == qualification)
        }) {
            return Ok(id);
        }
        // A scalar reached through a type parameter has the actual argument's
        // identity. Keep the same substitutions for the row and its source.
        let application = self.storage_source_application(reference);
        let identity = self.storage_identity(machine, &application)?;
        let record_type = RecordType {
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            cleanup: None,
            identity,
            shape: StorageShape::PrimitiveScalar {
                scalar_type,
                qualification,
            },
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        };
        Ok(self.push_record_row(record_type, physical_source_application(application)))
    }

    fn storage_identity(
        &self,
        machine: MachineId,
        application: &SourceTypeApplication,
    ) -> Checked<String> {
        self.typed
            .type_identity_text(application.type_identity(self.typed))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| refuse_at(machine, Unserved::Spelling("storage type identity")))
    }

    pub(super) fn storage_source_application(
        &self,
        reference: TypeReferenceHandle,
    ) -> SourceTypeApplication {
        SourceTypeApplication {
            reference,
            substitutions: Vec::new(),
            scope: self.source_scope.clone(),
        }
    }
}

pub(super) fn physical_source(reference: TypeReferenceHandle) -> RecordSource {
    physical_source_application(SourceTypeApplication {
        reference,
        ..Default::default()
    })
}

fn physical_source_application(application: SourceTypeApplication) -> RecordSource {
    RecordSource {
        constant_input_owner: None,
        declaration: SymbolHandle::invalid(),
        cleanup: None,
        application: Some(application),
        region_owner: SymbolHandle::invalid(),
        fields: Vec::new(),
        cases: Vec::new(),
        erased: Vec::new(),
    }
}

#[cfg(test)]
mod sequence_source_tests {
    use super::*;
    use crate::source_state::{TypedTrees, name::Identifier, types::TypeReferenceNode};
    use crate::source_symbols::SymbolTableBuilder;
    use symbols::{SymbolKind, SymbolNameRef};

    #[test]
    fn a_sequence_cannot_rebind_its_source_or_element_to_equal_width_storage() {
        // This is a typed-metadata falsifier, not source or authority admission.
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(root, crate::source_symbols::builtin_type_symbols());
        let unsigned = children
            .handles()
            .nth(
                symbols::BuiltinTypeAtom::Integer(
                    numerics::integer_carrier::FixedInteger::unsigned(32),
                )
                .ordinal(),
            )
            .unwrap();
        let signed = children
            .handles()
            .nth(
                symbols::BuiltinTypeAtom::Integer(numerics::integer_carrier::FixedInteger::signed(
                    32,
                ))
                .ordinal(),
            )
            .unwrap();
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unsigned = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: unsigned,
            name: Identifier::generated("u32"),
        });
        let signed = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: signed,
            name: Identifier::generated("i32"),
        });
        let first = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: unsigned,
        });
        let second = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: signed,
        });
        let machine = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        let Some(ValueType::Record(first_row, _, _)) = program.view_type(machine, first).unwrap()
        else {
            unreachable!()
        };
        let Some(ValueType::Record(second_row, _, _)) = program.view_type(machine, second).unwrap()
        else {
            unreachable!()
        };
        assert_ne!(first_row, second_row);
        assert!(
            program
                .source_record_correspondence(machine)
                .unwrap()
                .is_empty()
        );
        let original = program.records.get(first_row).unwrap().shape.clone();
        let StorageShape::ElementView { element: wrong, .. } =
            program.records.get(second_row).unwrap().shape
        else {
            unreachable!()
        };
        let StorageShape::ElementView { element, .. } =
            &mut program.records.get_mut(first_row).unwrap().shape
        else {
            unreachable!()
        };
        *element = wrong;
        assert!(
            program.source_record_correspondence(machine).is_err(),
            "same geometry is not an element declaration"
        );
        program.records.get_mut(first_row).unwrap().shape = original;
        program
            .record_sources
            .get_mut(first_row)
            .unwrap()
            .application
            .as_mut()
            .unwrap()
            .reference = second;
        assert!(
            program.source_record_correspondence(machine).is_err(),
            "an unrelated retained source cannot relabel the row"
        );
    }
}

/// How a read uses the element an index selects: as a whole value, or only
/// as the place a field projection reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ElementRead {
    Whole,
    Projected,
}
