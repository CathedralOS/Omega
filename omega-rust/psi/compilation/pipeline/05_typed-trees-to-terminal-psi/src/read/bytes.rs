//! Byte literals and views use Terminal's byte carrier and real immutable
//! backing. They have no nominal data declaration or layout authority.

use crate::body::Qualification;
use language_core::ReferenceAccess;
use semantic_vocabulary::ByteSequencePredicateSet;
use semantic_vocabulary::{MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{ByteSequenceCarrier, StructuralAccess};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Program, places::RecordSource};
use crate::{
    Expression, ExpressionKind, RecordType, SourceOrigin, StorageShape, Unserved, ValueType, refuse,
};

// The reader and source export use the same reference/region owner. A shape
// tag or an arbitrary valid type reference cannot establish byte provenance.
//
// The owner of the lifetime a view names is the typed trees' answer for that
// occurrence when they retained one. They retain none for a type written in
// a declaration, whose lexical owner is that declaration (lifetimes.md: a
// field's lifetimes are "binders declared in the lexical owner"), so the
// reader passes the data declaration whose field it is reading as
// `lexical_owner`, the rule `value_type` applies to `Inner<'a>` in a field.
pub(super) fn source_byte_view(
    typed: &typed_trees::TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    machine: MachineId,
) -> Checked<Option<SourceByteView>> {
    let fail = |reason| refuse(machine, Construct::Machine, Unserved::Type(reason));
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err(fail("byte view lost its retained type reference"));
    }
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = typed.type_reference_table.type_reference(reference)
    else {
        return Ok(None);
    };
    if !typed.type_reference_table.contains_type_reference(*referee) {
        return Err(fail("byte view lost its slice reference"));
    }
    // Only slice references belong to the byte-view operation. A domain
    // on an ordinary referent is judged by that referent's type owner.
    if !matches!(
        typed.type_reference_table.type_reference(
            typed
                .type_reference_table
                .unconstrained_type_reference(*referee)
        ),
        TypeReferenceNode::Slice { .. }
    ) {
        return Ok(None);
    }
    // `&[u8] in Utf8`: the domains are displayed on the referent, and a
    // reference's domains are its referent's (domains.md, "Mutable views").
    // Read them before the slice beneath.
    let (slice, domains) = byte_view_domains(typed, *referee, machine)?;
    let TypeReferenceNode::Slice { element_type } =
        typed.type_reference_table.type_reference(slice)
    else {
        if domains.is_empty() {
            return Ok(None);
        }
        return Err(fail(
            "domain on a reference to something other than a byte slice",
        ));
    };
    if !typed
        .type_reference_table
        .contains_type_reference(*element_type)
    {
        return Err(fail("byte view lost its element type"));
    }
    let TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*element_type)
    else {
        return Ok(None);
    };
    let Some(atom) = typed.symbols.builtin_type_atom(*symbol) else {
        return Ok(None);
    };
    let element = super::types::atom_type(atom, machine)?;
    if !matches!(element, semantic_vocabulary::ScalarType::Integer(integer)
        if integer.bits() == Some(8) && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && !integer.is_address())
    {
        return Ok(None);
    }
    let access = match access {
        ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
        ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
        ReferenceAccess::WriteOnly => return Err(fail("write-only byte view")),
    };
    // structural_access.md: "Qualified mutable, write-only, and
    // access-unspecified view fields remain unsupported". A write through a
    // mutable view would have to preserve the predicates of every place
    // that lent it, which nothing here follows.
    if access != StructuralAccess::SharedBorrow && !domains.is_empty() {
        return Err(fail("mutable byte view that carries a domain"));
    }
    let owner = match typed.type_reference_table.operand_lifetime_owner(reference) {
        retained if retained.is_valid() => retained,
        _ => lexical_owner,
    };
    super::places::reference_lifetime_declared(typed, machine, reference, owner)?;
    // The carrier's identity: a view with domains stores the same
    // descriptor as the view without them, so both are one row and the
    // domains are the value's qualification (`view_type`).
    let identity = typed_trees::type_identity::applied_carrier_identity_in_region_scope(
        typed, reference, owner, owner,
    )
    .and_then(|identity| typed.type_identity_text(identity))
    .filter(|identity| !identity.is_empty())
    .ok_or_else(|| fail("byte view has no complete owned region identity"))?;
    Ok(Some(SourceByteView {
        carrier: ByteSequenceCarrier::BorrowedView {
            access: Some(access),
        },
        identity,
        domains,
        element_type: *element_type,
    }))
}

/// A byte view type as the reader keeps it: the descriptor's carrier, the
/// identity of its row, and the domains displayed on its referent.
pub(super) struct SourceByteView {
    pub(super) carrier: ByteSequenceCarrier,
    pub(super) identity: String,
    pub(super) domains: Vec<crate::body::DomainInstance>,
    pub(super) element_type: TypeReferenceHandle,
}

// Both the formal and its source record use the declared slice's identity.
// The parameter owns the access and region of this loan.
pub(super) fn mutable_byte_formal_identity(
    typed: &typed_trees::TypedTrees,
    machine: MachineId,
    reference: TypeReferenceHandle,
) -> Checked<String> {
    let types = &typed.type_reference_table;
    let malformed = || {
        refuse(
            machine,
            Construct::Machine,
            Unserved::Type("byte view formal without its declared slice"),
        )
    };
    let TypeReferenceNode::Reference {
        referee,
        access: ReferenceAccess::Mutable,
        ..
    } = types.type_reference(reference)
    else {
        return Err(malformed());
    };
    if !types.contains_type_reference(*referee)
        || !matches!(
            types.type_reference(*referee),
            TypeReferenceNode::Slice { .. }
        )
    {
        return Err(malformed());
    }
    let identity = typed.normalized_type_identity_text(*referee);
    (!identity.is_empty())
        .then_some(identity)
        .ok_or_else(malformed)
}

/// The type beneath the domains displayed on a view's referent, and those
/// domains in written order.
///
/// Served: a declared domain whose one `requires` selects a genuine Core
/// byte observer of an explicit shared whole view of `self`, and that holds
/// of the empty view (`valid_utf8`, `no_nul`, `ascii_only`). Such a domain is decided by the bytes a view
/// shows, which the lowered form carries as the field's predicates
/// (`StructuralFieldType::PredicateByteSequence`) and the checker judges
/// from the bytes each store supplies (byte_views.md: "Primitive byte
/// predicates are reconstructed from the exact source"; structural_access.md:
/// "Literal qualification rosters must agree with their actual octets").
///
/// Refused by name: any other domain on a view, whose meaning is a
/// proposition no view's bytes decide; and `non_empty`, which zeroed
/// storage (the empty view) does not satisfy, so a record holding one would
/// be gated, which this slice does not judge.
fn byte_view_domains(
    typed: &typed_trees::TypedTrees,
    mut referee: TypeReferenceHandle,
    machine: MachineId,
) -> Checked<(TypeReferenceHandle, Vec<crate::body::DomainInstance>)> {
    use typed_trees::byte_predicates::{ByteSequencePredicate, byte_slice_domain_predicate};
    use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode};
    let fail = |reason| refuse(machine, Construct::Machine, Unserved::Type(reason));
    let table = &typed.type_reference_table;
    let mut domains = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = table.type_reference(referee)
    {
        for constraint in table.constraints(*constraints) {
            let TypeConstraintNode::Domain(domain) = constraint else {
                return Err(fail(
                    "byte view constrained by other than a declared domain",
                ));
            };
            if domain.subject != DomainConstraintSubject::Declared
                || !domain.arguments.is_empty()
                || !domain.symbol.is_valid()
            {
                return Err(fail(
                    "byte view constrained by other than a declared domain",
                ));
            }
            let definition = typed
                .domain_definition_for_instance(domain.symbol, domain.semantic_id)
                .ok_or_else(|| fail("byte view lost its exact domain application"))?;
            match byte_slice_domain_predicate(typed, definition).map(|(_, predicate)| predicate) {
                Some(ByteSequencePredicate::NonEmpty) => {
                    return Err(fail("byte view in a domain the empty view is not in"));
                }
                Some(_) => {}
                None => {
                    return Err(fail(
                        "byte view in a domain that is not a primitive byte predicate",
                    ));
                }
            }
            let instance = crate::body::DomainInstance {
                declaration: domain.symbol,
                identity: domain.semantic_id,
            };
            if domains.contains(&instance) {
                return Err(fail("byte view constrained by one domain twice"));
            }
            domains.push(instance);
        }
        referee = *base_type;
        if !table.contains_type_reference(referee) {
            return Err(fail("byte view lost its slice reference"));
        }
    }
    Ok((referee, domains))
}

impl Program<'_> {
    /// Whether `record` is a borrowed byte view whose bytes can be read:
    /// shared, or exclusive (byte_views.md: "An available shared or exact
    /// mutable view").
    pub(super) fn is_readable_byte_view(
        &self,
        machine: MachineId,
        record: semantic_vocabulary::StructuralTypeId,
    ) -> Checked<bool> {
        Ok(matches!(
            self.data_type(machine, record)?.shape,
            StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None
                    | Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow),
            })
        ))
    }

    /// Whether a value of this type is a readable borrowed byte view with no
    /// domain: domains.md, "Operands": "Builtin operators and comparisons
    /// ... accept unqualified operands only."
    pub(super) fn is_plain_readable_bytes(
        &self,
        machine: MachineId,
        value: Option<ValueType>,
    ) -> Checked<bool> {
        match value {
            Some(ValueType::Record(row, Qualification::NONE, _)) => {
                self.is_readable_byte_view(machine, row)
            }
            _ => Ok(false),
        }
    }

    /// `view[index]`: one byte of a borrowed byte view. The index is a
    /// `u64` (byte_views.md, `ByteSequenceRead`: "`u64` index and length"),
    /// and whether it lies inside the view is the checker's judgment, not
    /// this reading's.
    pub(super) fn byte_element(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        base: Expression,
        index: typed_trees::expression::ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
        let scalar = |bits| {
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).expect("valid width"))
        };
        let index = self.expression(machine, slots, index, Some(ValueType::plain(scalar(64))))?;
        if index.result.and_then(ValueType::scalar) != Some(scalar(64)) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("byte view index that is not a u64"),
            ));
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::ByteElement {
                base: Box::new(base),
                index: Box::new(index),
                element: self.plain_scalar_storage(machine, scalar(8))?,
            },
            result: Some(ValueType::plain(scalar(8))),
            source,
        })
    }

    pub(super) fn collection_index_operand_types(
        &self,
        machine: MachineId,
        row: semantic_vocabulary::StructuralTypeId,
    ) -> [Option<TypeReferenceHandle>; 2] {
        let reference = self
            .record_sources
            .get(row)
            .and_then(|source| source.application.as_ref())
            .map(|application| application.reference);
        [reference, self.collection_count_operand_type(machine)]
    }

    pub(super) fn collection_count_operand_type(
        &self,
        machine: MachineId,
    ) -> Option<TypeReferenceHandle> {
        let count = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid"),
        );
        self.typed
            .type_reference_table
            .named_references()
            .find_map(|(reference, symbol, _)| {
                self.typed
                    .symbols
                    .builtin_type_atom(symbol)
                    .is_some_and(|atom| super::types::atom_type(atom, machine) == Ok(count))
                    .then_some(reference)
            })
    }

    pub(super) fn collection_range_selection(
        &self,
        machine: MachineId,
        handle: typed_trees::expression::ExpressionHandle,
        row: semantic_vocabulary::StructuralTypeId,
    ) -> Checked<()> {
        let [collection, count] = self.collection_index_operand_types(machine, row);
        let operands = [collection, count, count];
        if !typed_trees::operator::resolve_indexed_spelling_for_operands(
            self.typed,
            language_core::OperatorSpelling::Range,
            &operands,
            self.typed.expression_table.source_span(handle),
        )
        .is_empty()
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.typed,
                self.lifetime_owner,
                handle,
                language_core::OperatorSpelling::Range,
                &operands,
            )
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Expression("range with a selected user operator"),
            ));
        }
        Ok(())
    }

    pub(super) fn byte_subslice(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        base: Expression,
        range: &typed_trees::expression::TableRangeExpression,
        handle: typed_trees::expression::ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unavailable =
            |detail| refuse(machine, Construct::Machine, Unserved::Expression(detail));
        let Some(ValueType::Record(row, _, _)) = base.result else {
            return Err(unavailable("range without its byte descriptor"));
        };
        if !matches!(self.data_type(machine, row)?.shape,
            StorageShape::ByteSequence(carrier) if carrier.is_shared_readable_view())
        {
            return Err(unavailable(
                "range of mutable backing without its persistent shared loan",
            ));
        }
        let selected = self
            .typed
            .expression_table
            .constructed_value_type_reference(handle);
        if self
            .typed
            .type_reference_table
            .contains_type_reference(selected)
        {
            // Typing retains the shared view only after selecting the builtin
            // range with the collection's exact source type. Its shape does
            // not carry the operand's domain onto the new window.
            let Some(ValueType::Record(view, Qualification::NONE, _)) =
                self.view_type(machine, selected)?
            else {
                return Err(unavailable(
                    "byte range with a different selected result type",
                ));
            };
            if !self.is_readable_byte_view(machine, view)? {
                return Err(unavailable(
                    "byte range with a different selected result type",
                ));
            }
        } else {
            if base
                .result
                .is_some_and(|value| !value.qualification().domains.is_empty())
            {
                return Err(unavailable(
                    "range of qualified bytes without its selected operand type",
                ));
            }
            self.collection_range_selection(machine, handle, row)?;
        }
        self.collection_window(machine, slots, base, range, source)
    }

    /// The `[start, end)` window of a shared descriptor whose builtin range
    /// selection is already checked. Both endpoint legs are relative to the
    /// captured descriptor's own length.
    pub(super) fn collection_window(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        base: Expression,
        range: &typed_trees::expression::TableRangeExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unavailable =
            |detail| refuse(machine, Construct::Machine, Unserved::Expression(detail));
        let Some(ValueType::Record(row, _, regions)) = base.result else {
            return Err(unavailable("range without its collection descriptor"));
        };
        let count = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid"),
        );
        let mut endpoint = |handle: typed_trees::expression::ExpressionHandle| -> Checked<Option<Box<Expression>>> {
            if !handle.is_valid() { return Ok(None); }
            let value = self.expression(machine, slots, handle, Some(ValueType::plain(count)))?;
            if value.result != Some(ValueType::plain(count)) {
                return Err(unavailable("range endpoint that is not a plain u64"));
            }
            Ok(Some(Box::new(value)))
        };
        let start = endpoint(range.start)?;
        let mut end = endpoint(range.end)?;
        if range.end_inclusive {
            let last = end
                .take()
                .ok_or_else(|| unavailable("inclusive range without an end"))?;
            end = Some(Box::new(Expression {
                call_context: None,
                kind: ExpressionKind::Strict {
                    operation: crate::body::Strict::Add,
                    policy: None,
                    left: last,
                    right: Box::new(Expression {
                        call_context: None,
                        kind: ExpressionKind::Integer(semantic_vocabulary::IntegerValue::Unsigned(
                            1,
                        )),
                        result: Some(ValueType::plain(count)),
                        source,
                    }),
                },
                result: Some(ValueType::plain(count)),
                source,
            }));
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::CollectionSubslice {
                base: Box::new(base),
                start,
                end,
            },
            result: Some(ValueType::Record(row, Qualification::NONE, regions)),
            source,
        })
    }

    pub(super) fn collection_measure(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        receiver: typed_trees::expression::ExpressionHandle,
        measure: typed_trees::expression::CollectionMeasure,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let base = self.expression(machine, slots, receiver, None)?;
        self.measured_collection(machine, base, measure, source)
    }

    /// Every length consumer uses the same readable collection formation.
    /// A ranking clause does not make a scalar or write-only view readable.
    pub(super) fn measured_collection(
        &mut self,
        machine: MachineId,
        base: Expression,
        measure: typed_trees::expression::CollectionMeasure,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use typed_trees::expression::CollectionMeasure;
        let Some(record) = self
            .opened(machine, base.result, super::authority::Access::Read)?
            .map(super::authority::Opened::record)
        else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("collection measurement of non-collection storage"),
            ));
        };
        let integer =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid");
        // `array.len` of a fixed array is the extent its type states, a
        // number known here: generics.md, "Runtime index identity and
        // storage", "arrays keep static extents", and domains.md, "`[u8; N]
        // in D` is the same exact `N`-byte fixed-array carrier as `[u8; N]`;
        // its `.len` is `N`". Nothing is measured, so no operation is
        // lowered for it and the expression is that number, a `u64`
        // (counts_and_addresses.md: "General lengths ... use `u64`").
        //
        // The array must be a place named by fields from a local or a
        // formal: such a path evaluates nothing, so leaving it out loses
        // nothing. A call result, an element chosen by an index or a case
        // payload would have to be evaluated or established first, and is
        // refused here.
        if measure == CollectionMeasure::Length
            && let Some(length) = self.data_type(machine, record)?.shape.fixed_array_length()
        {
            let mut place = &base;
            while let ExpressionKind::Field { base, .. } | ExpressionKind::Referent(base) =
                &place.kind
            {
                place = base;
            }
            if !matches!(place.kind, ExpressionKind::Slot(_)) {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Expression("length of a fixed array that is not a field path"),
                ));
            }
            return Ok(Expression {
                call_context: None,
                kind: ExpressionKind::Integer(semantic_vocabulary::IntegerValue::Unsigned(
                    length.into(),
                )),
                result: Some(ValueType::plain(semantic_vocabulary::ScalarType::Integer(
                    integer,
                ))),
                source,
            });
        }
        if measure != CollectionMeasure::Length
            || self
                .data_type(machine, record)?
                .shape
                .length_operation()
                .is_none()
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Expression("collection measurement without its descriptor operation"),
            ));
        }
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::CollectionMeasure {
                base: Box::new(base),
                measure,
            },
            result: Some(ValueType::plain(semantic_vocabulary::ScalarType::Integer(
                integer,
            ))),
            source,
        })
    }

    /// Whether the type is a borrowed byte or typed view. Its complete
    /// region tuple belongs to the value type, never to its storage row.
    pub(super) fn is_borrowed_view(&self, value_type: ValueType) -> bool {
        let ValueType::Record(row, _, _) = value_type else {
            return false;
        };
        self.records.get(row).is_some_and(|row| {
            matches!(
                row.shape,
                StorageShape::ByteSequence(_)
                    | StorageShape::ElementView {
                        access: Some(
                            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                        ),
                        ..
                    }
            )
        })
    }

    /// An incoming mutable view lends backing for the call. Keeping it in an
    /// aggregate requires an owned child descriptor, not a move out of the
    /// formal's storage. Establish that ordinary reborrow at any value use.
    pub(super) fn retain_view_input(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        value: Expression,
        expected: Option<ValueType>,
    ) -> Checked<Expression> {
        let (
            ExpressionKind::Slot(slot),
            Some(ValueType::Record(actual, qualification, regions)),
            Some(ValueType::Record(wanted, wanted_qualification, _)),
        ) = (&value.kind, value.result, expected)
        else {
            return Ok(value);
        };
        if slots.get(*slot).and_then(|slot| slot.borrowed)
            != Some(language_core::ReferenceAccess::Mutable)
            || qualification != wanted_qualification
        {
            return Ok(value);
        }
        let element = match (
            &self.data_type(machine, actual)?.shape,
            &self.data_type(machine, wanted)?.shape,
        ) {
            (
                StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None | Some(StructuralAccess::MutableBorrow),
                }),
                StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                }),
            ) => {
                let element = self.byte_view_element_type(machine, wanted)?;
                let value_type = self.value_type(machine, element)?;
                self.collection_element_storage(machine, element, value_type)?
                    .0
            }
            (
                StorageShape::ElementView {
                    element: actual,
                    qualification: actual_q,
                    access: Some(StructuralAccess::MutableBorrow),
                },
                StorageShape::ElementView {
                    element: wanted,
                    qualification: wanted_q,
                    access: Some(StructuralAccess::MutableBorrow),
                },
            ) if actual == wanted && actual_q == wanted_q => *actual,
            _ => return Ok(value),
        };
        let source = value.source;
        Ok(Expression {
            call_context: None,
            result: Some(ValueType::Record(wanted, qualification, regions)),
            kind: ExpressionKind::CollectionView {
                base: Box::new(value),
                access: language_core::ReferenceAccess::Mutable,
                element,
            },
            source,
        })
    }

    /// Storage rows erase regions and retain their first source occurrence.
    /// Re-read that occurrence in its own declaration, never in a later user's
    /// scope: a field's named lifetime belongs to its data declaration.
    pub(super) fn byte_view_element_type(
        &self,
        machine: MachineId,
        row: StructuralTypeId,
    ) -> Checked<TypeReferenceHandle> {
        let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Type(detail));
        let source = self
            .record_sources
            .get(row)
            .ok_or_else(|| unavailable("byte view without its source owner"))?;
        let physical = source
            .application
            .as_ref()
            .ok_or_else(|| unavailable("byte view without its source element"))?;
        source_byte_view(self.typed, physical.reference, source.region_owner, machine)?
            .map(|source| source.element_type)
            .ok_or_else(|| unavailable("byte view without its retained source type"))
    }

    pub(super) fn view_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<Option<ValueType>> {
        self.view_type_with_formal_access(machine, reference, false)
    }

    pub(super) fn view_type_with_formal_access(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        formal_access: bool,
    ) -> Checked<Option<ValueType>> {
        let lexical_owner = self.lifetime_owner;
        let Some(SourceByteView {
            carrier,
            identity,
            domains,
            ..
        }) = source_byte_view(self.typed, reference, lexical_owner, machine)?
        else {
            return match super::arrays::source_sequence_view(
                self.typed,
                reference,
                lexical_owner,
                machine,
            )? {
                Some((element, _, _)) => self.sequence_type(machine, reference, element).map(Some),
                None => Ok(None),
            };
        };
        // Keep the domains on the actual view type in every placement.
        // Signature clauses and checked establishments owe their predicates;
        // reading this type supplies no proof of those predicates.
        let mut qualification = Qualification::NONE;
        for domain in domains {
            qualification.domains = qualification.domains.with(domain).ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("byte view in more than four domains"),
                )
            })?;
        }

        let region = self.borrowed_view_region(machine, reference, lexical_owner, &identity)?;
        let regions = self.intern_regions(vec![region]);
        let typed_view = |row| Some(ValueType::Record(row, qualification, regions));
        let (carrier, identity) = if formal_access
            && matches!(
                carrier,
                ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                }
            ) {
            (
                ByteSequenceCarrier::BorrowedView { access: None },
                mutable_byte_formal_identity(self.typed, machine, reference)?,
            )
        } else {
            (carrier, identity)
        };
        // One row per carrier, whatever region: the lowered form erases
        // regions. The row keeps the first occurrence it was read from as
        // its source, whose identity text it carries
        // (`source_record_correspondence`).
        let row = self.records.iter().find_map(|(id, row)| {
            let view = self.record_sources.get(id).is_some_and(|source| {
                !source.declaration.is_valid() && source.application.is_some()
            });
            (view && row.shape == StorageShape::ByteSequence(carrier)).then_some(id)
        });
        if let Some(row) = row {
            self.retain_byte_domains(machine, row, qualification)?;
            return Ok(typed_view(row));
        }
        let row = self.records.push(RecordType {
            multiplicity: if carrier.is_shared_readable_view() {
                terminal_psi::StructuralMultiplicity::Unrestricted
            } else {
                terminal_psi::StructuralMultiplicity::Affine
            },
            cleanup: None,
            identity,
            shape: StorageShape::ByteSequence(carrier),
            requires: Vec::new(),
            authored_layout: false,
        });
        self.record_sources.push(RecordSource {
            declaration: SymbolHandle::invalid(),
            application: Some(typed_trees::SourceTypeApplication {
                reference,
                substitutions: Vec::new(),
            }),
            region_owner: lexical_owner,
            fields: Vec::new(),
            cases: Vec::new(),
        });
        self.retain_byte_domains(machine, row, qualification)?;
        Ok(typed_view(row))
    }

    pub(super) fn retain_byte_domains(
        &mut self,
        machine: MachineId,
        carrier: StructuralTypeId,
        qualification: Qualification,
    ) -> Checked<()> {
        for source in qualification.domains.members() {
            let (_, predicate) = typed_trees::byte_predicates::byte_slice_domain_predicate(
                self.typed,
                self.domain_theory(machine, source)?,
            )
            .ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("byte qualification lost its complete primitive declaration"),
                )
            })?;
            let row = crate::body::SourceByteDomain {
                source,
                carrier,
                predicate,
            };
            if let Some(known) = self
                .byte_domains
                .iter()
                .find(|known| known.source == source && known.carrier == carrier)
            {
                if *known != row {
                    return Err(refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Type("one byte domain has conflicting primitive meanings"),
                    ));
                }
            } else {
                self.byte_domains.push(row);
            }
            self.admitted_declarations.include(source.declaration);
        }
        Ok(())
    }

    pub(super) fn byte_literal(
        &mut self,
        machine: MachineId,
        bytes: &[u8],
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let carrier = ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        };
        // A literal's bytes are immutable program data that outlive every
        // region, so it is a view of whichever region the place it stands
        // for is valid for.
        let (row, regions) = match expected {
            Some(ValueType::Record(row, _, regions))
                if self.data_type(machine, row)?.shape == StorageShape::ByteSequence(carrier) =>
            {
                (row, regions)
            }
            // data_and_literals.md, "Quoted bytes": "A quoted literal
            // contains bytes, has ordinary shared byte-view type `&[u8]`".
            // A scalar is never that type, so the literal is wrong for a
            // scalar place in every program. A record destination (another
            // view, a view under a domain) stays a refusal.
            Some(ValueType::Scalar(scalar, _)) => {
                return Err(crate::reject(
                    machine,
                    source,
                    format!(
                        "this quoted literal is a shared view of bytes, `&[u8]`, and the place \
                         it is supplied to holds a `{}`: a quoted literal has the byte-view \
                         type and no other",
                        super::rejections::scalar_named(scalar)
                    ),
                ));
            }
            Some(_) => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("literal requires an immutable byte view"),
                ));
            }
            None => (self.literal_byte_type(carrier), crate::body::Regions::NONE),
        };
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::ByteLiteral(bytes.to_vec()),
            result: Some(ValueType::Record(row, Qualification::NONE, regions)),
            source,
        })
    }

    fn literal_byte_type(&mut self, carrier: ByteSequenceCarrier) -> StructuralTypeId {
        if let Some(row) = self.records.iter().find_map(|(id, row)| {
            let literal = self.record_sources.get(id).is_some_and(|source| {
                !source.declaration.is_valid() && source.application.is_none()
            });
            (literal && row.shape == StorageShape::ByteSequence(carrier)).then_some(id)
        }) {
            return row;
        }
        let row = self.records.push(RecordType {
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            cleanup: None,
            // This text labels the structural type; it names no declaration.
            // Exact literal contents and origin belong to its producer below.
            identity: "immutable literal bytes".into(),
            shape: StorageShape::ByteSequence(carrier),
            requires: Vec::new(),
            authored_layout: false,
        });
        self.record_sources.push(RecordSource {
            declaration: SymbolHandle::invalid(),
            application: None,
            region_owner: SymbolHandle::invalid(),
            fields: Vec::new(),
            cases: Vec::new(),
        });
        row
    }

    /// The primitive byte predicates of a value's type: one for each domain
    /// a byte view in a domain carries (`byte_view_domains` admitted only
    /// those), none for every other type.
    pub(super) fn view_predicates(
        &self,
        machine: MachineId,
        value_type: ValueType,
    ) -> Checked<ByteSequencePredicateSet> {
        let predicates = ByteSequencePredicateSet::EMPTY;
        let ValueType::Record(row, qualification, _) = value_type else {
            return Ok(predicates);
        };
        if !matches!(
            self.data_type(machine, row)?.shape,
            StorageShape::ByteSequence(_)
        ) {
            return Ok(predicates);
        }
        self.byte_qualification_predicates(machine, qualification)
    }

    /// Pure byte predicates named by exact domain applications. The caller
    /// owns the byte carrier's shape; this query supplies no value proof.
    pub(super) fn byte_qualification_predicates(
        &self,
        machine: MachineId,
        qualification: Qualification,
    ) -> Checked<ByteSequencePredicateSet> {
        if qualification.policy.is_some() {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("arithmetic policy on byte storage"),
            ));
        }
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        for domain in qualification.domains.members() {
            let theory = self.domain_theory(machine, domain)?;
            if theory.access_policy.is_some() {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("byte predicate conversion needs governed domain evidence"),
                ));
            }
            let predicate =
                typed_trees::byte_predicates::domain_byte_predicate(self.typed, domain.declaration)
                    .ok_or_else(|| {
                        refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Type(
                                "byte view in a domain that is not a primitive byte predicate",
                            ),
                        )
                    })?;
            predicates.insert(predicate);
        }
        Ok(predicates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{TypedTrees, machine::Machine, name::Identifier};

    #[test]
    fn byte_views_keep_region_owners_and_export_the_exact_reference() {
        // This is a typed-reference control, not source or authority admission.
        // Equal carrier geometry cannot substitute for the declared region.
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::Machine, SymbolNameRef::Static("first")),
                (SymbolKind::Machine, SymbolNameRef::Static("second")),
            ]),
        );
        let byte = children
            .handles()
            .nth(symbols::BuiltinTypeAtom::U8.ordinal())
            .unwrap();
        let first = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
        let second = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 1)
            .unwrap();
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        assert_eq!(
            typed.symbols.builtin_type_atom(byte),
            Some(symbols::BuiltinTypeAtom::U8)
        );
        for (symbol, name) in [(first, "first"), (second, "second")] {
            typed.push_machine(Machine {
                symbol,
                name: Identifier::generated(name),
                lifetime_parameters: vec![
                    Identifier::generated("region"),
                    Identifier::generated("other"),
                ],
                ..Default::default()
            });
        }
        let byte = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: byte,
            name: Identifier::generated("u8"),
        });
        let slice = typed
            .type_reference_table
            .insert(TypeReferenceNode::Slice { element_type: byte });
        let mut view = |owner, region: &str| {
            let reference = typed
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: slice,
                    access: ReferenceAccess::Shared,
                    lifetime: Some(Identifier::generated(region)),
                });
            typed
                .type_reference_table
                .retain_operand_lifetime_owner(reference, owner)
                .unwrap();
            reference
        };
        let a = view(first, "region");
        let same = view(first, "region");
        let b = view(second, "region");
        let c = view(first, "other");
        let missing = view(first, "missing");
        let mut program = Program::new(&typed, false);
        let machine = MachineId::new(1).unwrap();
        let a = program.view_type(machine, a).unwrap().unwrap();
        assert_eq!(program.view_type(machine, same).unwrap(), Some(a));
        assert_ne!(program.view_type(machine, b).unwrap(), Some(a));
        assert_ne!(program.view_type(machine, c).unwrap(), Some(a));
        assert!(program.view_type(machine, missing).is_err());
        assert!(
            program
                .source_record_correspondence(machine)
                .unwrap()
                .is_empty()
        );
        let ValueType::Record(a, _, _) = a else {
            panic!("a byte descriptor is structural")
        };
        program
            .record_sources
            .get_mut(a)
            .unwrap()
            .application
            .as_mut()
            .unwrap()
            .reference = byte;
        assert!(program.source_record_correspondence(machine).is_err());
    }
}
