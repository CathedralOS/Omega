use super::*;
use numerics::integer_carrier::FixedInteger;
use symbols::BuiltinTypeAtom;

fn declaration(atom: BuiltinTypeAtom) -> DeclarationDescription {
    let identity = application_identity::type_identity::compiler_type_name(&atom.identity());
    DeclarationDescription {
        owner_identity: identity.clone(),
        identity_is_hermetic: true,
        name: atom.symbol_name().into_owned(),
        shape: SchemaShape::Primitive(atom),
        supply: String::new(),
        multiplicity: "Copy".into(),
        carry: None,
        type_parameters: Vec::new(),
        lifetime_parameters: Vec::new(),
        application_identity: Some(application_identity::type_identity::compiler_type_identity(
            &atom.identity(),
        )),
        members: Vec::new(),
        retired_identities: Vec::new(),
    }
}

fn graph(declaration: DeclarationDescription) -> Result<SemanticSchemaGraph, String> {
    SemanticSchemaGraph::from_nodes(
        vec![SchemaNode::Invalid, SchemaNode::Declaration(declaration)],
        SchemaNodeHandle::from_index(1),
        SchemaQueryAuthority::ForeignScope {
            requester_identity: "schema-test".into(),
        },
    )
}

#[test]
fn primitive_schemas_preserve_exact_carriers_and_reject_old_images() {
    assert_eq!(
        application_identity::type_identity::compiler_type_identity(
            &BuiltinTypeAtom::U64.identity()
        ),
        r"named(name(compiler-type\(atom\(u64\)\)))",
    );
    let mut images = Vec::new();
    for atom in [
        BuiltinTypeAtom::Integer(FixedInteger::unsigned(13)),
        BuiltinTypeAtom::Integer(FixedInteger::signed(13)),
        BuiltinTypeAtom::Address,
        BuiltinTypeAtom::F32,
        BuiltinTypeAtom::F64,
    ] {
        let graph = graph(declaration(atom)).unwrap();
        assert_eq!(graph.declaration().shape, SchemaShape::Primitive(atom));
        assert_eq!(graph.fields().count(), 0);
        let image = graph.runtime_image();
        assert_eq!(
            SemanticSchemaGraph::from_runtime_image(&image).unwrap(),
            graph
        );
        assert!(!images.contains(&image));
        let mut old = image.clone();
        old[..8].copy_from_slice(&1u64.to_le_bytes());
        assert!(
            SemanticSchemaGraph::from_runtime_image(&old)
                .unwrap_err()
                .contains("version")
        );
        images.push(image);
    }
}

#[test]
fn primitive_schemas_reject_forged_identity_and_structural_members() {
    let original = declaration(BuiltinTypeAtom::U64);
    let mut changed = original.clone();
    changed.shape = SchemaShape::Primitive(BuiltinTypeAtom::Address);
    assert!(
        graph(changed)
            .unwrap_err()
            .contains("exact scalar identity")
    );
    let mut changed = original.clone();
    changed.members.push(SchemaNodeHandle::from_index(1));
    assert!(graph(changed).unwrap_err().contains("adds data members"));
    let mut changed = original;
    changed.shape = SchemaShape::Primitive(BuiltinTypeAtom::AtomicU64);
    assert!(graph(changed).unwrap_err().contains("non-scalar carrier"));
}
