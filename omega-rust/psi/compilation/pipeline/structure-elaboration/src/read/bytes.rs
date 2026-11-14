//! Byte literals and views use Terminal's byte carrier and real immutable
//! backing. They have no nominal data declaration or layout authority.

use crate::body::Qualification;
use language_core::ReferenceAccess;
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
) -> Checked<Option<(ByteSequenceCarrier, String)>> {
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
    let TypeReferenceNode::Slice { element_type } =
        typed.type_reference_table.type_reference(*referee)
    else {
        return Ok(None);
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
    let owner = match typed.type_reference_table.operand_lifetime_owner(reference) {
        retained if retained.is_valid() => retained,
        _ => lexical_owner,
    };
    let identity = typed_trees::type_identity::applied_type_identity_in_region_scope(
        typed,
        reference,
        owner,
        owner,
        &[],
        &[],
    )
    .and_then(|identity| typed.type_identity_text(identity))
    .filter(|identity| !identity.is_empty())
    .ok_or_else(|| fail("byte view has no complete owned region identity"))?;
    Ok(Some((
        ByteSequenceCarrier::BorrowedView {
            access: Some(access),
        },
        identity,
    )))
}

impl Program<'_> {
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
        let Some((carrier, identity)) =
            source_byte_view(self.typed, reference, lexical_owner, machine)?
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
        let region = self.borrowed_view_region(machine, reference, lexical_owner, &identity)?;
        let regions = self.intern_regions(vec![region]);
        let typed_view = |row| Some(ValueType::Record(row, Qualification::NONE, regions));
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
