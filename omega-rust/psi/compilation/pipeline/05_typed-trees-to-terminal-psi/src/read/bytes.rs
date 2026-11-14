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
        if integer.bits() == 8 && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && !integer.is_address())
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
    }))
}

/// Whether `definition` is a domain of the byte slice `[u8]` whose one
/// `requires` is a primitive byte predicate of `self` (`valid_utf8(self)`).
pub(super) fn primitive_byte_domain(
    typed: &typed_trees::TypedTrees,
    definition: &typed_trees::domain::DomainDefinition,
    machine: MachineId,
) -> bool {
    let table = &typed.type_reference_table;
    typed_trees::byte_predicates::domain_byte_predicate(typed, definition.symbol).is_some()
        && table.contains_type_reference(definition.target_type)
        && matches!(table.type_reference(definition.target_type),
        TypeReferenceNode::Slice { element_type }
            if table.contains_type_reference(*element_type)
                && matches!(table.type_reference(*element_type),
                    TypeReferenceNode::Named { symbol, .. }
                        if typed.symbols.builtin_type_atom(*symbol).is_some_and(|atom| {
                            matches!(super::types::atom_type(atom, machine),
                                Ok(semantic_vocabulary::ScalarType::Integer(integer))
                                    if integer.bits() == 8
                                        && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                                        && !integer.is_address())
                        })))
}

/// A byte view type as the reader keeps it: the descriptor's carrier, the
/// identity of its row, and the domains displayed on its referent.
pub(super) struct SourceByteView {
    pub(super) carrier: ByteSequenceCarrier,
    pub(super) identity: String,
    pub(super) domains: Vec<SymbolHandle>,
}

/// The type beneath the domains displayed on a view's referent, and those
/// domains in written order.
///
/// Served: a declared domain whose one `requires` is a primitive byte
/// predicate of `self` and that holds of the empty view (`valid_utf8`,
/// `no_nul`, `ascii_only`). Such a domain is decided by the bytes a view
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
) -> Checked<(TypeReferenceHandle, Vec<SymbolHandle>)> {
    use typed_trees::byte_predicates::{ByteSequencePredicate, domain_byte_predicate};
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
            let primitive = typed
                .domain_definition(domain.symbol)
                .is_some_and(|definition| primitive_byte_domain(typed, definition, machine));
            match domain_byte_predicate(typed, domain.symbol).filter(|_| primitive) {
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
            if domains.contains(&domain.symbol) {
                return Err(fail("byte view constrained by one domain twice"));
            }
            domains.push(domain.symbol);
        }
        referee = *base_type;
        if !table.contains_type_reference(referee) {
            return Err(fail("byte view lost its slice reference"));
        }
    }
    Ok((referee, domains))
}

impl Program<'_> {
    pub(super) fn collection_measure(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        receiver: typed_trees::expression::ExpressionHandle,
        measure: typed_trees::expression::CollectionMeasure,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use typed_trees::expression::CollectionMeasure;
        let base = self.expression(machine, slots, receiver, None)?;
        let Some(record) = self.opened(machine, base.result, super::authority::Access::Read)?
        else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("collection measurement of non-collection storage"),
            ));
        };
        if measure != CollectionMeasure::Length
            || !matches!(
                self.data_type(machine, record)?.shape,
                StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow),
                })
            )
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Expression("collection measurement without its descriptor operation"),
            ));
        }
        let integer =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid");
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
                        access: Some(StructuralAccess::SharedBorrow),
                        ..
                    }
            )
        })
    }

    pub(super) fn view_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<Option<ValueType>> {
        let lexical_owner = self.lifetime_owner;
        let Some(SourceByteView {
            carrier,
            identity,
            domains,
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
        // A view in a domain is served where the checker judges what it
        // holds: a data field, whose every store it judges from the bytes
        // supplied, and the cast that makes a value for one
        // (`QualifiedViews`). A local, formal or result of such a type would
        // owe its predicates at moves and calls that nothing judges yet.
        let mut qualification = Qualification::NONE;
        if !domains.is_empty() {
            if !self.qualified_views {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("byte view in a domain outside a data field"),
                ));
            }
            for domain in domains {
                qualification.domains = qualification.domains.with(domain).ok_or_else(|| {
                    refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Type("byte view in more than four domains"),
                    )
                })?;
            }
        }

        let region = self.borrowed_view_region(machine, reference, lexical_owner, &identity)?;
        let regions = self.intern_regions(vec![region]);
        let typed_view = |row| Some(ValueType::Record(row, qualification, regions));
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
            return Ok(typed_view(row));
        }
        let row = self.records.push(RecordType {
            owns_linear_claim: false,
            copyable: carrier.is_shared_readable_view(),
            identity,
            shape: StorageShape::ByteSequence(carrier),
            requires: Vec::new(),
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
        Ok(typed_view(row))
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
            owns_linear_claim: false,
            copyable: true,
            // This text labels the structural type; it names no declaration.
            // Exact literal contents and origin belong to its producer below.
            identity: "immutable literal bytes".into(),
            shape: StorageShape::ByteSequence(carrier),
            requires: Vec::new(),
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

    /// The primitive byte predicates of a field's type: one for each domain
    /// a byte view in a domain carries (`byte_view_domains` admitted only
    /// those), none for every other type.
    pub(super) fn view_predicates(
        &self,
        machine: MachineId,
        value_type: ValueType,
    ) -> Checked<ByteSequencePredicateSet> {
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        let ValueType::Record(row, qualification, _) = value_type else {
            return Ok(predicates);
        };
        if !matches!(
            self.data_type(machine, row)?.shape,
            StorageShape::ByteSequence(_)
        ) {
            return Ok(predicates);
        }
        for domain in qualification.domains.members() {
            let predicate = typed_trees::byte_predicates::domain_byte_predicate(self.typed, domain)
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
