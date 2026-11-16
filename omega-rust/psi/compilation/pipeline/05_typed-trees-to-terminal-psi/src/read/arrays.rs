//! Resolve an array's actual element type and closed extent once. Array
//! elements keep the canonical indexed namespace; they never acquire invented
//! field declarations or a nominal schema.

use crate::body::Qualification;
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::ArrayExtent;
use terminal_semantics::CollectionExtent;
use typed_trees::expression::{ExpressionHandle, TableIndexedExpression};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle};

use super::authority::Access;
use super::{Program, Slot, places::RecordSource};
use crate::{
    Expression, ExpressionKind, RecordType, SourceOrigin, StorageShape, Unserved, ValueType, refuse,
};

// Retain the same scoped type/region identity used by byte views. A bare
// sequence is affine; an explicit shared loan is unrestricted. Neither kind
// supplies elements or backing merely by appearing in a declaration.
pub(super) fn source_sequence_view(
    typed: &typed_trees::TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    machine: MachineId,
) -> Checked<
    Option<(
        TypeReferenceHandle,
        Option<terminal_psi::StructuralAccess>,
        String,
    )>,
> {
    use typed_trees::types::TypeReferenceNode;
    let fail = |reason| refuse(machine, Construct::Machine, Unserved::Type(reason));
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err(fail("sequence lost its retained type reference"));
    }
    let (slice, access) = match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Slice { .. } => (reference, None),
        TypeReferenceNode::Reference {
            referee, access, ..
        } => {
            if !typed.type_reference_table.contains_type_reference(*referee) {
                return Err(fail("sequence loan lost its retained referent"));
            }
            if !matches!(
                typed.type_reference_table.type_reference(*referee),
                TypeReferenceNode::Slice { .. }
            ) {
                return Ok(None);
            }
            if super::bytes::source_byte_view(typed, reference, lexical_owner, machine)?.is_some() {
                return Ok(None);
            }
            if *access != language_core::ReferenceAccess::Shared {
                return Err(fail(
                    "mutable typed sequence requires its actual exclusive backing and loan",
                ));
            }
            (*referee, Some(terminal_psi::StructuralAccess::SharedBorrow))
        }
        _ => return Ok(None),
    };
    let TypeReferenceNode::Slice { element_type } =
        typed.type_reference_table.type_reference(slice)
    else {
        return Err(fail("sequence lost its exact slice declaration"));
    };
    if !typed
        .type_reference_table
        .contains_type_reference(*element_type)
    {
        return Err(fail("sequence lost its retained element type"));
    }
    let identity = if access.is_some() {
        let retained = typed.type_reference_table.operand_lifetime_owner(reference);
        let owner = if retained.is_valid() {
            retained
        } else {
            lexical_owner
        };
        super::places::reference_lifetime_declared(typed, machine, reference, owner)?;
        typed_trees::type_identity::applied_type_identity_in_region_scope(
            typed,
            reference,
            owner,
            owner,
            &[],
            &[],
        )
        .and_then(|identity| typed.type_identity_text(identity))
    } else {
        typed.type_identity_text(typed.normalized_type_identity(reference))
    }
    .filter(|identity| !identity.is_empty())
    .ok_or_else(|| fail("sequence has no complete owned type/region identity"))?;
    Ok(Some((*element_type, access, identity)))
}

impl Program<'_> {
    /// The element of a plain closed byte array. This recognizes its storage,
    /// not a conversion to a view; only an explicit range forms that loan.
    pub(super) fn byte_array_element(&self, actual: Option<ValueType>) -> Option<StructuralTypeId> {
        let Some(ValueType::Record(actual, Qualification::NONE, _)) = actual else {
            return None;
        };
        let actual = self.records.get(actual)?;
        let StorageShape::FixedArray {
            element,
            qualification: Qualification::NONE,
            length: ArrayExtent::Literal(_),
        } = actual.shape
        else {
            return None;
        };
        (actual.requires.is_empty() && self.records.get(element).is_some_and(|element|
            element.requires.is_empty() && matches!(element.shape,
                StorageShape::PrimitiveScalar { scalar_type: ScalarType::Integer(integer), policy: None }
                    if integer.bits() == Some(8) && integer.sign() == IntegerSign::Unsigned && !integer.is_address())))
            .then_some(element)
    }

    /// The element of a closed fixed array whose element storage and
    /// qualification are exactly the typed view's. `WholeCollection`
    /// establishment preserves identical element types directly
    /// (byte_views.md, "Collection view establishment"); this reader admits
    /// no layout-equal substitute, so a different element row is refused.
    fn typed_array_element(
        &self,
        actual: Option<ValueType>,
        element: StructuralTypeId,
        qualification: Qualification,
    ) -> Option<StructuralTypeId> {
        let Some(ValueType::Record(actual, Qualification::NONE, _)) = actual else {
            return None;
        };
        let actual = self.records.get(actual)?;
        let StorageShape::FixedArray {
            element: stored,
            qualification: stored_qualification,
            length: ArrayExtent::Literal(_),
        } = actual.shape
        else {
            return None;
        };
        (actual.requires.is_empty() && stored == element && stored_qualification == qualification)
            .then_some(element)
    }

    /// Borrow a range of a closed array using the backing's shape. A `u8`
    /// destination forms a byte descriptor; an `&[T]` destination forms the
    /// typed element view of the same backing. Both are the one
    /// `ArrayView` formation, which Terminal Psi establishes as a
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

        let unavailable =
            |detail| refuse(machine, Construct::Machine, Unserved::Expression(detail));
        let Some(borrow) = self.typed.expression_table.borrow_expression(argument) else {
            return Ok(None);
        };
        let Some(indexed) = self.typed.expression_table.indexed_node(borrow.target) else {
            return Ok(None);
        };
        let Some(range) = self.typed.expression_table.range_node(indexed.index) else {
            return Ok(None);
        };
        let ValueType::Record(mut view, Qualification::NONE, _) = expected else {
            return Ok(None);
        };
        // Formation owns a stored descriptor with the written loan. Only the
        // callee's direct formal carries its access separately from its shape.
        if formal_access == Some(ReferenceAccess::Mutable)
            && self.data_type(machine, view)?.shape
                == StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access: None })
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
            if self.data_type(machine, actual)?.shape
                != StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                })
            {
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
        let requested = match borrow.access {
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
        let mut base = self.expression(machine, slots, indexed.collection, None)?;
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
        let byte_view = typed_element.is_none() && self.is_readable_byte_view(machine, backing)?;
        // A shared descriptor can snapshot its unrestricted value. A child
        // of an exclusive descriptor instead keeps a persistent payload loan,
        // including when that child only observes the bytes.
        if byte_view
            && requested == StructuralAccess::SharedBorrow
            && self.observes_only(machine, base.result)?
            && self.data_type(machine, backing)?.copyable
        {
            self.authorized(machine, backing_type, Access::Read)?;
            let value = self.byte_subslice(machine, slots, base, range, borrow.target, source)?;
            self.complete_formed_operator(machine, borrow.target, &value)?;
            return Ok(Some(value));
        }
        // A partial range of fixed storage is the window of the whole-array
        // loan: the loan keeps the backing's exact extent `N`, and the
        // window's `start <= end <= N` legs are checked against that loan's
        // own length like any other subslice (byte_views.md, "Source
        // correspondence"). A writable window also needs to keep its
        // exclusive descriptor live through the subslice.
        let partial = range.start.is_valid() || range.end.is_valid() || range.end_inclusive;
        let element = match typed_element {
            None if byte_view => {
                if backing_type.qualification() != Qualification::NONE {
                    return Err(unavailable("a borrowed range retains qualified backing"));
                }
                let retained = self
                    .record_sources
                    .get(backing)
                    .and_then(|source| source.application.as_ref())
                    .map(|source| source.reference)
                    .ok_or_else(|| unavailable("byte range without its retained element type"))?;
                let declared = super::bytes::source_byte_view(
                    self.typed,
                    retained,
                    self.lifetime_owner,
                    machine,
                )?
                .ok_or_else(|| unavailable("byte range without its retained element type"))?;
                let value = self.value_type(machine, declared.element_type)?;
                self.storage_type(machine, declared.element_type, value)?
            }
            None => self
                .byte_array_element(base.result)
                .ok_or_else(|| unavailable("array view without a plain closed u8 array"))?,
            Some((element, qualification)) => self
                .typed_array_element(base.result, element, qualification)
                .ok_or_else(|| {
                    unavailable("typed array view without a closed array of its exact element")
                })?,
        };
        self.collection_range_selection(machine, borrow.target, backing)?;
        let crossed = self.reference_place_access(machine, slots, &base, borrow.access)?;
        let slot = super::storage_path::root_slot(machine, &base, &self.captures)?
            .ok_or_else(|| unavailable("array view without an existing backing place"))?;
        if !byte_view
            && !crossed
            && borrow.access == ReferenceAccess::Mutable
            && (slots[slot].borrowed == Some(ReferenceAccess::Shared)
                || slots[slot].borrowed.is_none() && !slots[slot].assignable)
        {
            return Err(unavailable(
                "mutable array view without exclusive backing access",
            ));
        }
        self.authorized(
            machine,
            backing_type,
            if borrow.access == ReferenceAccess::Mutable {
                Access::Write
            } else {
                Access::Read
            },
        )?;
        if borrow.access == ReferenceAccess::Mutable {
            self.written_through(machine, &base)?;
        }
        let mut regions = self.storage_loan_regions(machine, slots, &base)?;
        if typed_element.is_some() {
            // A typed view prefixes its backing loan's region to the regions
            // its elements hold, which the array value carries (`sequence_type`).
            let mut tuple = self.region_tuple(regions).to_vec();
            tuple.extend_from_slice(self.region_tuple(backing_type.regions()));
            regions = self.intern_regions(tuple);
        }
        // Lend the actual collection before evaluating endpoints. A mutable
        // descriptor must reborrow its payload, not move out of its caller's
        // storage and leave a restoration debt there.
        let whole = Expression {
            call_context: None,
            kind: ExpressionKind::ArrayView {
                base: Box::new(base),
                access: borrow.access,
                element,
            },
            result: Some(ValueType::Record(view, Qualification::NONE, regions)),
            source,
        };
        // A partial range adds a derived window, but the source operator
        // still belongs to the original array backing.
        self.complete_formed_operator(machine, borrow.target, &whole)?;
        if partial {
            return self
                .collection_window(machine, slots, whole, range, source)
                .map(Some);
        }
        Ok(Some(whole))
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
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("array literal without a closed contextual type"),
            ));
        };
        let (element, qualification, extent) =
            self.array_element(machine, super::authority::Opened::constructing(array))?;
        // An array value holds its elements' regions (`array_type`).
        let element_regions = self.collection_element_regions(machine, array, regions)?;
        let declared = super::places::with_regions(
            self.storage_value_type(machine, element, qualification)?,
            element_regions,
        );
        let supplied = self.typed.expression_table.expression_handles(supplied);
        if let CollectionExtent::Fixed(extent) = extent
            && u64::try_from(supplied.len()).ok() != Some(extent)
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("array literal extent"),
            ));
        }
        let mut elements = Vec::with_capacity(supplied.len());
        for handle in supplied {
            let element = self.expression(machine, slots, *handle, Some(declared))?;
            if !self.agrees(element.result, declared) {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Typing("array literal element"),
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
    ) -> Checked<Expression> {
        let source = SourceOrigin::of(self.typed.expression_table.source_span(handle));
        // A range preserves the backing region of its operand. Context can
        // instantiate that region (including immutable literal backing), but
        // never selects an operator or grants mutable access.
        let hint = expected.filter(|value| matches!(value, ValueType::Record(row, _, _)
            if matches!(self.records.get(*row).map(|row| &row.shape), Some(StorageShape::ByteSequence(carrier)) if carrier.is_shared_readable_view())));
        let hint = matches!(
            self.node(machine, indexed.index)?,
            typed_trees::expression::ExpressionNode::Range(_)
        )
        .then_some(hint)
        .flatten();
        let base = self.place(machine, slots, indexed.collection, hint, access)?;
        let Some(array) = self.opened_place(machine, &base, access)? else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("index into non-array storage"),
            ));
        };
        if self.is_readable_byte_view(machine, array.record())? {
            if let typed_trees::expression::ExpressionNode::Range(range) =
                self.node(machine, indexed.index)?
            {
                return self.byte_subslice(machine, slots, base, range, handle, source);
            }
            return self.byte_element(machine, slots, base, indexed.index, source);
        }
        let (element, qualification, extent) = self.array_element(machine, array)?;
        let stored_length = matches!(extent, CollectionExtent::StoredLength);
        if stored_length && access != Access::Read {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("sequence index needs its current length and bounds judgment"),
            ));
        }
        if stored_length && !self.data_type(machine, element)?.copyable {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("copying a slice element requires a copyable element type"),
            ));
        }
        let regions = base
            .result
            .map_or(crate::body::Regions::NONE, ValueType::regions);
        let element_regions = self.collection_element_regions(machine, array.record(), regions)?;
        let result = super::places::with_regions(
            self.storage_value_type(machine, element, qualification)?,
            element_regions,
        );
        let index_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("u64 index carrier"),
            )
        })?;
        if stored_length {
            let operands = self.collection_index_operand_types(machine, array.record());
            if !typed_trees::operator::builtin_index_read_is_spelled(
                self.typed,
                self.lifetime_owner,
                handle,
                &operands,
            ) {
                return Err(refuse(
                    machine,
                    Construct::Machine,
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
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("array index is not a fixed integer"),
            ));
        }
        if stored_length
            && index.result.and_then(ValueType::scalar) != Some(ScalarType::Integer(index_type))
        {
            return Err(refuse(
                machine,
                Construct::Machine,
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
            StorageShape::ElementView {
                element,
                qualification,
                ..
            } => Ok((*element, *qualification, CollectionExtent::StoredLength)),
            _ => Err(refuse(
                machine,
                Construct::Machine,
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
                policy: None,
            } => ValueType::Scalar(scalar_type, Qualification::NONE),
            StorageShape::PrimitiveScalar {
                scalar_type,
                policy: Some(policy),
            } => ValueType::Scalar(scalar_type, Qualification::of_policy(policy)),
            _ => ValueType::Record(storage, qualification, crate::body::Regions::NONE),
        })
    }

    /// A source sequence keeps its exact element and type identity. This row
    /// supplies no elements, lifetime or backing: those belong to construction
    /// and transport through the existing canonical ElementView operations.
    pub(super) fn sequence_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        element: TypeReferenceHandle,
    ) -> Checked<ValueType> {
        let Some((declared, access, identity)) =
            source_sequence_view(self.typed, reference, self.lifetime_owner, machine)?
        else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("sequence without its source declaration"),
            ));
        };
        if declared != element {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("sequence selects another element occurrence"),
            ));
        }
        let mut tuple = Vec::new();
        if access.is_some() {
            tuple.push(self.borrowed_view_region(
                machine,
                reference,
                self.lifetime_owner,
                &identity,
            )?);
        }
        let value = self.value_type(machine, element)?;
        tuple.extend_from_slice(self.region_tuple(value.regions()));
        let regions = self.intern_regions(tuple);
        let (element, qualification) = self.collection_element_storage(machine, element, value)?;
        let shape = StorageShape::ElementView {
            element,
            qualification,
            access,
        };
        if let Some((id, _)) = self.records.iter().find(|(_, row)| row.shape == shape) {
            return Ok(ValueType::Record(id, Qualification::NONE, regions));
        }
        let id = self.records.push(RecordType {
            owns_linear_claim: false,
            copyable: self.typed.stored_value_multiplicity(reference)
                == language_semantics::Multiplicity::Unrestricted,
            identity,
            shape,
            requires: Vec::new(),
            authored_layout: false,
        });
        let mut source = physical_source(reference);
        source.region_owner = self.lifetime_owner;
        self.record_sources.push(source);
        Ok(ValueType::Record(id, Qualification::NONE, regions))
    }

    pub(super) fn array_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        element: TypeReferenceHandle,
        _extent: &FixedArrayLength,
    ) -> Checked<ValueType> {
        use typed_trees::constant_recipes::DeclaredArrayExtent;

        let unserved = |reason| refuse(machine, Construct::Machine, Unserved::Type(reason));
        let length = match self.typed.declared_array_extent(reference) {
            Some(DeclaredArrayExtent::Literal(length)) => ArrayExtent::Literal(length),
            Some(DeclaredArrayExtent::Recipe(recipe)) => {
                let recipe = self.typed.constant_recipes.get(recipe);
                let contexts = self
                    .typed
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts);
                let [context] = contexts else {
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
            None => return Err(unserved("unresolved array extent")),
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
        let copyable = self.data_type(machine, element)?.copyable;
        let owns_linear_claim = self.data_type(machine, element)?.owns_linear_claim;
        let identity = self.storage_identity(machine, reference)?;
        let id = self.records.push(RecordType {
            owns_linear_claim,
            copyable,
            identity,
            shape,
            requires: Vec::new(),
            authored_layout: false,
        });
        self.record_sources.push(physical_source(reference));
        // The row is the element's carrier; the regions its elements hold
        // belong to the array value's type, as a record's do (`regions`).
        Ok(ValueType::Record(id, Qualification::NONE, value.regions()))
    }

    fn collection_element_storage(
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
        let (scalar_type, policy) = match value {
            ValueType::Scalar(scalar, qualification)
                if qualification.domains.is_empty()
                    || self.type_purpose
                        == super::storage_metadata::TypePurpose::StorageDescription =>
            {
                (scalar, qualification.policy)
            }
            ValueType::Scalar(..) => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("array of elements that carry a declared domain"),
                ));
            }
            ValueType::Record(id, Qualification::NONE, _) => return Ok(id),
            ValueType::Record(id, _, _)
                if self.type_purpose
                    == super::storage_metadata::TypePurpose::StorageDescription =>
            {
                return Ok(id);
            }
            ValueType::Record(..) => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("array of elements that carry a declared domain"),
                ));
            }
            ValueType::Rigid(_) => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("rigid array element"),
                ));
            }
        };
        if let Some((id, _)) = self.records.iter().find(|(_, row)| {
            matches!(row.shape, StorageShape::PrimitiveScalar { scalar_type: known, policy: prior }
                if known == scalar_type && prior == policy)
        }) {
            return Ok(id);
        }
        let identity = self.storage_identity(machine, reference)?;
        let id = self.records.push(RecordType {
            owns_linear_claim: false,
            copyable: true,
            identity,
            shape: StorageShape::PrimitiveScalar {
                scalar_type,
                policy,
            },
            requires: Vec::new(),
            authored_layout: false,
        });
        self.record_sources.push(physical_source(reference));
        Ok(id)
    }

    fn storage_identity(
        &self,
        machine: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<String> {
        self.typed
            .type_identity_text(self.typed.normalized_type_identity(reference))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Spelling("array storage type identity"),
                )
            })
    }
}

pub(super) fn physical_source(reference: TypeReferenceHandle) -> RecordSource {
    RecordSource {
        declaration: SymbolHandle::invalid(),
        application: Some(typed_trees::SourceTypeApplication {
            reference,
            substitutions: Vec::new(),
        }),
        region_owner: SymbolHandle::invalid(),
        fields: Vec::new(),
        cases: Vec::new(),
    }
}

#[cfg(test)]
mod sequence_source_tests {
    use super::*;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{TypedTrees, name::Identifier, types::TypeReferenceNode};

    #[test]
    fn a_sequence_cannot_rebind_its_source_or_element_to_equal_width_storage() {
        // This is a typed-metadata falsifier, not source or authority admission.
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(root, symbols::builtin_type_symbols());
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
        let ValueType::Record(first_row, _, _) =
            program.sequence_type(machine, first, unsigned).unwrap()
        else {
            unreachable!()
        };
        let ValueType::Record(second_row, _, _) =
            program.sequence_type(machine, second, signed).unwrap()
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
