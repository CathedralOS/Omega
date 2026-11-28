//! Byte literals and views use Terminal's byte carrier and real immutable
//! backing. They have no nominal data declaration or layout authority.
use crate::read::refuse_at;

use crate::body::Qualification;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use language_core::ReferenceAccess;
use semantic_vocabulary::ByteSequencePredicateSet;
use semantic_vocabulary::{MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::{ByteSequenceCarrier, StructuralAccess};

use super::{Program, places::RecordSource};
use crate::{
    Expression, ExpressionKind, RecordType, SourceOrigin, StorageShape, Unserved, ValueType,
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
    typed: &crate::source_state::TypedTrees,
    view: &crate::source_state::SourceSliceApplication,
    lexical_owner: SymbolHandle,
    machine: MachineId,
) -> Checked<Option<SourceByteView>> {
    let fail = |reason| refuse_at(machine, Unserved::Type(reason));
    let Some(access) = view.access else {
        return Ok(None);
    };
    let domains = byte_view_domains(typed, &view.referent, machine)?;
    let element = view
        .element
        .scope
        .supplied_type_reference(typed, view.element.reference)
        .map_err(fail)?;
    let TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(element.reference)
    else {
        return Ok(None);
    };
    let Some(atom) = typed.symbols.builtin_type_atom(*symbol) else {
        return Ok(None);
    };
    let element_type = element.reference;
    let scalar = super::types::atom_type(atom, machine)?;
    if !matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer)
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
    let owner = match typed
        .type_reference_table
        .operand_lifetime_owner(view.reference.reference)
    {
        retained if retained.is_valid() => retained,
        _ => lexical_owner,
    };
    super::places::reference_lifetime_declared(typed, machine, view.reference.reference, owner)?;
    // The carrier's identity: a view with domains stores the same
    // descriptor as the view without them, so both are one row and the
    // domains are the value's qualification (`view_type`).
    let identity = view
        .reference
        .scope
        .carrier_identity_in_region_scope(typed, view.reference.reference, owner)
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| fail("byte view has no complete owned region identity"))?;
    Ok(Some(SourceByteView {
        carrier: ByteSequenceCarrier::BorrowedView {
            access: Some(access),
        },
        identity,
        domains,
        element_type,
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

/// Authenticate the source type against the byte carrier the reader emitted.
/// A mutable formal keeps its loan access beside the slice carrier; an owned
/// descriptor keeps that access on the carrier itself.
pub(super) fn byte_view_matches_carrier(
    typed: &crate::source_state::TypedTrees,
    machine: MachineId,
    application: &crate::source_state::SourceTypeApplication,
    owner: SymbolHandle,
    carrier: ByteSequenceCarrier,
    identity: &str,
) -> Checked<bool> {
    let Some(view) = super::source_views::source_view(typed, application.clone(), owner, machine)?
    else {
        return Ok(false);
    };
    let super::source_views::SourceViewKind::Bytes(bytes) = &view.kind else {
        return Ok(false);
    };
    Ok(
        if matches!(carrier, ByteSequenceCarrier::BorrowedView { access: None }) {
            matches!(
                bytes.carrier,
                ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow)
                }
            ) && mutable_byte_formal_identity(typed, machine, &view.slice)? == identity
        } else {
            bytes.carrier == carrier && bytes.identity == identity
        },
    )
}

/// Reuse the exact source row that produced a byte record, including its role.
/// Equal descriptor geometry alone cannot establish source correspondence.
pub(super) fn byte_record_source_matches(
    typed: &crate::source_state::TypedTrees,
    machine: MachineId,
    declaration: &RecordType,
    source: &RecordSource,
) -> Checked<bool> {
    let StorageShape::ByteSequence(carrier) = declaration.shape else {
        return Ok(false);
    };
    if source.declaration.is_valid() || !source.fields.is_empty() || !source.cases.is_empty() {
        return Ok(false);
    }
    match &source.application {
        Some(application) if application.substitutions.is_empty() => byte_view_matches_carrier(
            typed,
            machine,
            application,
            source.region_owner,
            carrier,
            &declaration.identity,
        ),
        Some(_) => Ok(false),
        None => Ok(carrier.is_shared_readable_view()),
    }
}

// Both the formal and its source record use the declared slice's identity.
// The parameter owns the access and region of this loan.
pub(super) fn mutable_byte_formal_identity(
    typed: &crate::source_state::TypedTrees,
    machine: MachineId,
    view: &crate::source_state::SourceSliceApplication,
) -> Checked<String> {
    let malformed = || {
        refuse_at(
            machine,
            Unserved::Type("byte view formal without its declared slice"),
        )
    };
    if view.access != Some(ReferenceAccess::Mutable) {
        return Err(malformed());
    }
    if view.referent.reference != view.slice.reference {
        return Err(malformed());
    }
    let identity = typed
        .type_identity_text(view.slice.type_identity(typed))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(malformed)?;
    Ok(identity)
}

/// The type beneath the domains displayed on a view's referent, and those
/// domains in written order.
///
/// Domain names remain attached to the exact view type. Primitive byte
/// predicates use their compact byte judgment; other declarations contribute
/// ordinary structural predicates at casts, calls and storage uses.
fn byte_view_domains(
    typed: &crate::source_state::TypedTrees,
    referee: &crate::source_state::SourceTypeApplication,
    machine: MachineId,
) -> Checked<Vec<crate::body::DomainInstance>> {
    use crate::source_state::byte_predicates::{
        ByteSequencePredicate, byte_slice_domain_predicate,
    };
    use crate::source_state::types::{DomainConstraintSubject, TypeConstraintNode};
    let fail = |reason| refuse_at(machine, Unserved::Type(reason));
    let table = &typed.type_reference_table;
    let mut referee = referee.clone();
    let mut domains = Vec::new();
    let mut remaining = table.type_reference_count();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = table.type_reference(referee.reference)
    {
        if remaining == 0 {
            return Err(fail("byte view has a cyclic qualification tree"));
        }
        remaining -= 1;
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
                None => {}
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
        let supplied = referee
            .scope
            .supplied_type_reference(typed, *base_type)
            .map_err(fail)?;
        if !std::sync::Arc::ptr_eq(&referee.scope, &supplied.scope) {
            remaining = table.type_reference_count();
        }
        referee = supplied;
    }
    Ok(domains)
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
        index: crate::source_state::expression::ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
        let scalar = |bits| {
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).expect("valid width"))
        };
        let index = self.expression(machine, slots, index, Some(ValueType::plain(scalar(64))))?;
        if index.result.and_then(ValueType::scalar) != Some(scalar(64)) {
            return Err(refuse_at(
                machine,
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
        handle: crate::source_state::expression::ExpressionHandle,
        row: semantic_vocabulary::StructuralTypeId,
    ) -> Checked<()> {
        let [collection, count] = self.collection_index_operand_types(machine, row);
        let operands = [collection, count, count];
        if !crate::source_state::operator::resolve_indexed_spelling_for_operands(
            self.typed,
            language_core::OperatorSpelling::Range,
            &operands,
            self.typed.expression_table.source_span(handle),
        )
        .is_empty()
            || !crate::source_state::operator::has_builtin_spelled_expression_meaning(
                self.typed,
                self.lifetime_owner,
                handle,
                language_core::OperatorSpelling::Range,
                &operands,
            )
        {
            return Err(refuse_at(
                machine,
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
        range: &crate::source_state::expression::TableRangeExpression,
        handle: crate::source_state::expression::ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unavailable = |detail| refuse_at(machine, Unserved::Expression(detail));
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
        range: &crate::source_state::expression::TableRangeExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unavailable = |detail| refuse_at(machine, Unserved::Expression(detail));
        let Some(ValueType::Record(row, _, regions)) = base.result else {
            return Err(unavailable("range without its collection descriptor"));
        };
        let count = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid"),
        );
        let mut endpoint = |handle: crate::source_state::expression::ExpressionHandle| -> Checked<Option<Box<Expression>>> {
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
        receiver: crate::source_state::expression::ExpressionHandle,
        measure: crate::source_state::expression::CollectionMeasure,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let base = self.expression(machine, slots, receiver, None)?;
        self.measured_collection(machine, slots, base, measure, source)
    }

    /// Every length consumer uses the same readable collection formation.
    /// A ranking clause does not make a scalar or write-only view readable.
    pub(super) fn measured_collection(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        base: Expression,
        measure: crate::source_state::expression::CollectionMeasure,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use crate::source_state::expression::CollectionMeasure;
        let Some(record) = self
            .opened(machine, base.result, super::authority::Access::Read)?
            .map(super::authority::Opened::record)
        else {
            return Err(refuse_at(
                machine,
                Unserved::Typing("collection measurement of non-collection storage"),
            ));
        };
        let integer =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid");
        // A fixed array's length is its declared extent. A generic check
        // reads the exact constant input; a closed array supplies its literal.
        // Keep the existing pure field-path restriction: erasing an effectful
        // base or an unestablished case projection would lose its obligations.
        if measure == CollectionMeasure::Length
            && let Some(length) = self.data_type(machine, record)?.shape.fixed_array_extent()
        {
            let mut place = &base;
            while let ExpressionKind::Field { base, .. } | ExpressionKind::Referent(base) =
                &place.kind
            {
                place = base;
            }
            if !matches!(place.kind, ExpressionKind::Slot(_)) {
                return Err(refuse_at(
                    machine,
                    Unserved::Expression("length of a fixed array that is not a field path"),
                ));
            }
            let result = ValueType::plain(semantic_vocabulary::ScalarType::Integer(integer));
            let kind = match length {
                terminal_psi::ArrayExtent::Literal(length) => ExpressionKind::Integer(
                    semantic_vocabulary::IntegerValue::Unsigned(length.into()),
                ),
                terminal_psi::ArrayExtent::Parameter { owner, value } => {
                    let binding = self.extent_input(machine, owner, value)?;
                    let position = slots
                        .iter()
                        .rposition(|slot| slot.symbol == binding.symbol)
                        .ok_or_else(|| {
                            refuse_at(
                                machine,
                                Unserved::Expression("fixed-array length has no constant binding"),
                            )
                        })?;
                    let slot = &slots[position];
                    let input_type = ValueType::plain(binding.scalar_type);
                    if slot.value_type != input_type
                        || slot.assignable
                        || slot.homed
                        || slot.erased
                        || slot.borrowed.is_some()
                    {
                        return Err(refuse_at(
                            machine,
                            Unserved::Expression(
                                "fixed-array length lost its immutable constant input",
                            ),
                        ));
                    }
                    let operand = Expression {
                        call_context: None,
                        kind: ExpressionKind::Slot(position),
                        result: Some(input_type),
                        source,
                    };
                    if input_type == result {
                        operand.kind
                    } else {
                        ExpressionKind::Cast(Box::new(operand))
                    }
                }
                terminal_psi::ArrayExtent::Application(application) => {
                    let input_type = self
                        .scalar_applications
                        .scalar_type(application)
                        .ok_or_else(|| {
                            refuse_at(
                                machine,
                                Unserved::Expression(
                                    "fixed-array length has no declared scalar application",
                                ),
                            )
                        })?;
                    let input_value = ValueType::plain(input_type);
                    let operand = Expression {
                        call_context: None,
                        kind: ExpressionKind::ScalarApplication { application },
                        result: Some(input_value),
                        source,
                    };
                    if input_value == result {
                        operand.kind
                    } else {
                        ExpressionKind::Cast(Box::new(operand))
                    }
                }
            };
            return Ok(Expression {
                call_context: None,
                kind,
                result: Some(result),
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
            return Err(refuse_at(
                machine,
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
                    access: None | Some(StructuralAccess::MutableBorrow),
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
        let unavailable = |detail| refuse_at(machine, Unserved::Type(detail));
        let source = self
            .record_sources
            .get(row)
            .ok_or_else(|| unavailable("byte view without its source owner"))?;
        let physical = source
            .application
            .as_ref()
            .ok_or_else(|| unavailable("byte view without its source element"))?;
        let view = super::source_views::source_view(
            self.typed,
            physical.clone(),
            source.region_owner,
            machine,
        )?
        .ok_or_else(|| unavailable("byte view without its retained source type"))?;
        match view.kind {
            super::source_views::SourceViewKind::Bytes(bytes) => Ok(bytes.element_type),
            _ => Err(unavailable("byte view changes its retained source carrier")),
        }
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
        let Some(view) = super::source_views::source_view(
            self.typed,
            self.storage_source_application(reference),
            lexical_owner,
            machine,
        )?
        else {
            return Ok(None);
        };
        let super::source_views::SourceViewKind::Bytes(SourceByteView {
            carrier,
            identity,
            domains,
            ..
        }) = &view.kind
        else {
            return self.sequence_type(machine, view, formal_access).map(Some);
        };
        let (carrier, identity) = (*carrier, identity.clone());
        // Keep the domains on the actual view type in every placement.
        // Signature clauses and checked establishments owe their predicates;
        // reading this type supplies no proof of those predicates.
        let mut qualification = Qualification::NONE;
        for domain in domains {
            qualification.domains = qualification.domains.with(*domain).ok_or_else(|| {
                refuse_at(
                    machine,
                    Unserved::Type("byte view in more than four domains"),
                )
            })?;
        }

        let region = self.borrowed_view_region(
            machine,
            view.slice.reference.reference,
            lexical_owner,
            &identity,
        )?;
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
                mutable_byte_formal_identity(self.typed, machine, &view.slice)?,
            )
        } else {
            (carrier, identity)
        };
        let row = self.byte_view_record(carrier, Some((identity, reference, lexical_owner)));
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
            let definition = self.domain_theory(machine, source)?;
            let Some((_, predicate)) =
                crate::source_state::byte_predicates::byte_slice_domain_predicate(
                    self.typed, definition,
                )
            else {
                self.declared_domain(
                    machine,
                    source,
                    ValueType::record(carrier),
                    SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
                )?;
                continue;
            };
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
                    return Err(refuse_at(
                        machine,
                        Unserved::Type("one byte domain has conflicting primitive meanings"),
                    ));
                }
            } else {
                // A field can introduce this domain into a generic check form
                // before any direct declaration read. Check its source range
                // operator when its byte predicate enters the form.
                self.declared_predicates(machine, definition)?;
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
            // contains bytes, has ordinary shared byte-view type `@[u8]`".
            // A scalar is never that type, so the literal is wrong for a
            // scalar place in every program. A record destination (another
            // view, a view under a domain) stays a refusal.
            Some(ValueType::Scalar(scalar, _)) => {
                return Err(crate::reject(
                    machine,
                    source,
                    format!(
                        "this quoted literal is a shared view of bytes, `@[u8]`, and the place \
                         it is supplied to holds a `{}`: a quoted literal has the byte-view \
                         type and no other",
                        super::rejections::scalar_named(scalar)
                    ),
                ));
            }
            Some(_) => {
                return Err(refuse_at(
                    machine,
                    Unserved::Type("literal requires an immutable byte view"),
                ));
            }
            None => (
                self.byte_view_record(carrier, None),
                crate::body::Regions::NONE,
            ),
        };
        let regions = if regions == crate::body::Regions::NONE {
            // A projection can be the first use of a literal. Its immutable
            // image backing satisfies the lifetime that projection infers.
            let lifetime = self.fresh_region();
            self.intern_regions(vec![lifetime])
        } else {
            regions
        };
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::ByteLiteral(bytes.to_vec()),
            result: Some(ValueType::Record(row, Qualification::NONE, regions)),
            source,
        })
    }

    /// A quoted value and an authored byte view have the same structural type.
    /// Its region and domains stay on the value. If a literal first introduced
    /// the row, retain the first authored reference when one becomes available.
    fn byte_view_record(
        &mut self,
        carrier: ByteSequenceCarrier,
        authored: Option<(String, TypeReferenceHandle, SymbolHandle)>,
    ) -> StructuralTypeId {
        let known = self.records.iter().find_map(|(row, record)| {
            let structural = self
                .record_sources
                .get(row)
                .is_some_and(|source| !source.declaration.is_valid());
            (structural && record.shape == StorageShape::ByteSequence(carrier)).then_some(row)
        });
        if let Some(row) = known
            && (authored.is_none()
                || self
                    .record_sources
                    .get(row)
                    .is_some_and(|source| source.application.is_some()))
        {
            return row;
        }
        let (identity, application, region_owner) = match authored {
            Some((identity, reference, owner)) => (
                identity,
                Some(self.storage_source_application(reference)),
                owner,
            ),
            None => (
                "immutable literal bytes".into(),
                None,
                SymbolHandle::invalid(),
            ),
        };
        if let Some(row) = known {
            let source = self
                .record_sources
                .get_mut(row)
                .expect("byte view has a source row");
            source.application = application;
            source.region_owner = region_owner;
            self.records
                .get_mut(row)
                .expect("byte view record exists")
                .identity = identity;
            return row;
        }
        let record_type = RecordType {
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
            erased: Vec::new(),
        };
        let source = RecordSource {
            constant_input_owner: None,
            declaration: SymbolHandle::invalid(),
            cleanup: None,
            application,
            region_owner,
            fields: Vec::new(),
            cases: Vec::new(),
            erased: Vec::new(),
        };
        self.push_record_row(record_type, source)
    }

    /// A slice domain has an unsized logical subject. Its length is observed
    /// through a shared descriptor, without constructing owned slice storage.
    /// A supplied descriptor must still display that exact byte carrier.
    pub(super) fn logical_byte_domain_carrier(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        presented: Option<ValueType>,
    ) -> Checked<Option<ValueType>> {
        let fail = |reason| refuse_at(machine, Unserved::Type(reason));
        let Some(slice) = self
            .source_scope
            .slice_application(self.typed, reference)
            .map_err(fail)?
        else {
            return Ok(None);
        };
        if slice.access.is_some() {
            return Ok(None);
        }
        let element = slice
            .element
            .scope
            .supplied_type_reference(self.typed, slice.element.reference)
            .map_err(fail)?;
        let TypeReferenceNode::Named { symbol, .. } = self
            .typed
            .type_reference_table
            .type_reference(element.reference)
        else {
            return Ok(None);
        };
        let Some(atom) = self.typed.symbols.builtin_type_atom(*symbol) else {
            return Ok(None);
        };
        if !matches!(super::types::atom_type(atom, machine)?, semantic_vocabulary::ScalarType::Integer(integer)
            if integer.bits() == Some(8) && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && !integer.is_address())
        {
            return Ok(None);
        }
        if let Some(value_type) = presented {
            let ValueType::Record(row, _, _) = value_type else {
                return Err(fail(
                    "a byte-slice domain requires a shared byte descriptor",
                ));
            };
            if !matches!(self.data_type(machine, row)?.shape,
                StorageShape::ByteSequence(bytes) if bytes.is_shared_readable_view())
            {
                return Err(fail(
                    "a byte-slice domain requires a shared byte descriptor",
                ));
            }
            return Ok(Some(value_type));
        }
        Ok(Some(ValueType::record(self.byte_view_record(
            ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            },
            None,
        ))))
    }

    /// The primitive byte predicates of a value's type: one for each domain
    /// a byte view carries. Ordinary domain clauses retain their separate
    /// structural qualification and are not summarized by this bit set.
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
            return Err(refuse_at(
                machine,
                Unserved::Type("arithmetic policy on byte storage"),
            ));
        }
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        for domain in qualification.domains.members() {
            let theory = self.domain_theory(machine, domain)?;
            if theory.access_policy.is_some() {
                return Err(refuse_at(
                    machine,
                    Unserved::Type("byte predicate conversion needs governed domain evidence"),
                ));
            }
            if let Some((_, predicate)) =
                crate::source_state::byte_predicates::byte_domain_predicate(self.typed, theory)
            {
                predicates.insert(predicate);
            }
            // An ordinary domain is proved by its structural clauses, never
            // by a primitive predicate inferred from its spelling.
        }
        Ok(predicates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::{TypedTrees, machine::Machine, name::Identifier};
    use crate::source_symbols::SymbolTableBuilder;
    use symbols::{SymbolKind, SymbolNameRef};

    #[test]
    fn byte_views_keep_region_owners_and_export_the_exact_reference() {
        // This is a typed-reference control, not source or authority admission.
        // Equal carrier geometry cannot substitute for the declared region.
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            crate::source_symbols::builtin_type_symbols()
                .into_iter()
                .chain([
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
        let arguments = typed
            .type_reference_table
            .insert(TypeReferenceNode::Slice { element_type: b });
        let lists = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: arguments,
        });
        let mut program = Program::new(&typed, false);
        let machine = MachineId::new(1).unwrap();
        let origin = SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent);
        let quote = |program: &mut Program<'_>| {
            let literal = program.byte_literal(machine, b"A", None, origin).unwrap();
            let Some(ValueType::Record(row, Qualification::NONE, regions)) = literal.result else {
                panic!("an unqualified quoted value has a structural byte view")
            };
            assert_eq!(program.region_tuple(regions).len(), 1);
            row
        };
        let from_literal = quote(&mut program);
        assert!(
            program
                .record_sources
                .get(from_literal)
                .unwrap()
                .application
                .is_none()
        );
        let a = program.view_type(machine, a).unwrap().unwrap();
        assert_eq!(quote(&mut program), from_literal);
        assert_eq!(program.view_type(machine, same).unwrap(), Some(a));
        assert_ne!(program.view_type(machine, b).unwrap(), Some(a));
        assert_ne!(program.view_type(machine, c).unwrap(), Some(a));
        assert!(program.view_type(machine, missing).is_err());
        // The sequence uses `b`; its shared physical byte row was first read
        // for `a`. The two values retain distinct regions above.
        let ValueType::Record(arguments_row, _, _) =
            program.view_type(machine, arguments).unwrap().unwrap()
        else {
            panic!("a sequence must have a structural carrier");
        };
        program.view_type(machine, lists).unwrap().unwrap();
        assert!(
            program
                .source_record_correspondence(machine)
                .unwrap()
                .is_empty()
        );
        let ValueType::Record(a, _, _) = a else {
            panic!("a byte descriptor is structural")
        };
        assert_eq!(a, from_literal);
        // A sequence cannot substitute its outer descriptor for its byte element.
        let declared = program.records.get(arguments_row).unwrap().shape.clone();
        let crate::StorageShape::ElementView { element, .. } =
            &mut program.records.get_mut(arguments_row).unwrap().shape
        else {
            panic!("the sequence lost its physical element");
        };
        *element = arguments_row;
        assert!(program.source_record_correspondence(machine).is_err());
        program.records.get_mut(arguments_row).unwrap().shape = declared;
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

    #[test]
    fn formed_collection_operands_keep_the_source_of_mutable_formal_carriers() {
        use crate::source_state::expression::{ExpressionNode, TableNamePath};
        use crate::source_state::signature::StateParameter;

        // Keep the source type metadata that specialization copies. Without it
        // the operator reader would use its separate no-metadata path.
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            crate::source_symbols::builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Machine, SymbolNameRef::Static("store"))]),
        );
        let byte = children
            .handles()
            .nth(symbols::BuiltinTypeAtom::U8.ordinal())
            .unwrap();
        let owner = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
        let parameter = symbols
            .insert_children(
                owner,
                [(SymbolKind::Parameter, SymbolNameRef::Static("bytes"))],
            )
            .handle_at(0);
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let byte = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: byte,
            name: "u8".into(),
        });
        let slice = typed
            .type_reference_table
            .insert(TypeReferenceNode::Slice { element_type: byte });
        let references = [
            ReferenceAccess::Mutable,
            ReferenceAccess::Mutable,
            ReferenceAccess::Shared,
        ]
        .map(|access| {
            let reference = typed
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: slice,
                    access,
                    lifetime: Some("region".into()),
                });
            typed
                .type_reference_table
                .retain_operand_lifetime_owner(reference, owner)
                .unwrap();
            reference
        });
        let parameters = typed.state_parameters.insert_many([StateParameter {
            symbol: parameter,
            name: "bytes".into(),
            type_reference: references[0],
            is_mutable: true,
            ..Default::default()
        }]);
        let states = typed
            .machine_states
            .insert_many([crate::source_state::state::State {
                symbol: owner,
                name: "store".into(),
                parameters,
                ..Default::default()
            }]);
        typed.push_machine(Machine {
            symbol: owner,
            name: "store".into(),
            states,
            lifetime_parameters: vec!["region".into()],
            ..Default::default()
        });
        let expressions = references.map(|reference| {
            let mut members = Default::default();
            typed
                .expression_table
                .push_name_path_member(&mut members, "bytes".into());
            let expression = typed
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: parameter,
                    symbol: parameter,
                    ..Default::default()
                }));
            typed
                .expression_table
                .set_constructed_value_type_reference(expression, reference);
            expression
        });
        let mut program = Program::new(&typed, false);
        program.lifetime_owner = owner;
        let machine = MachineId::new(1).unwrap();
        let formals = typed.state_parameters.span(parameters).unwrap();
        let formal = &formals[0];
        let (operand, erased, borrowed) = program.formal(machine, formal).unwrap();
        assert!(!erased);
        assert_eq!(borrowed.unwrap().access, ReferenceAccess::Mutable);
        let owned_descriptor = program.value_type(machine, references[0]).unwrap();
        assert_ne!(
            owned_descriptor.erased(),
            operand.erased(),
            "a formal binding and an owned mutable descriptor have different carriers"
        );
        for index in [0, 1] {
            assert_eq!(
                typed
                    .expression_table
                    .constructed_value_type_reference(expressions[index]),
                references[index]
            );
            assert_eq!(
                program
                    .structural_operator_operand_reference(machine, expressions[index], operand,)
                    .unwrap(),
                Some(references[index]),
                "lookup retains the full original reference, including a copied occurrence"
            );
        }
        let shared = program
            .structural_operator_operand_reference(machine, expressions[2], operand)
            .unwrap_err();
        assert_eq!(
            shared.reason,
            terminal_check_tables::RefusalReason::ForeignIdentity
        );
        let ValueType::Record(record, qualification, regions) = operand else {
            panic!("the mutable formal carries a structural byte view")
        };
        let qualified = ValueType::Record(
            record,
            Qualification {
                policy: Some(crate::body::Policy::Wrapping),
                ..qualification
            },
            regions,
        );
        assert_eq!(
            program
                .structural_operator_operand_reference(machine, expressions[0], qualified,)
                .unwrap_err()
                .reason,
            terminal_check_tables::RefusalReason::ForeignIdentity
        );
        assert!(
            program
                .source_record_correspondence(machine)
                .unwrap()
                .is_empty()
        );
        program
            .record_sources
            .get_mut(record)
            .unwrap()
            .application
            .as_mut()
            .unwrap()
            .reference = references[2];
        assert_eq!(
            program
                .structural_operator_operand_reference(machine, expressions[0], operand,)
                .unwrap_err()
                .reason,
            terminal_check_tables::RefusalReason::ForeignIdentity
        );
        assert!(
            program.source_record_correspondence(machine).is_err(),
            "the same shared source-owner validator rejects the forged record source"
        );
    }
}
