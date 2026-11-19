use super::*;
use crate::source_inputs::SourceMap;
use crate::source_state::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment, TypedTrees,
    data::{DataDefinition, DataField, DataMember, DataProperties},
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use crate::source_symbols::SymbolTableBuilder;
use application_identity::opaque_representations::{
    OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
    OpaqueRepresentationLifecycleDisposition,
};
use arena::IdTable;
use semantic_vocabulary::PackageKeyIdentity;
use source_admission::SourceOrigin;
use source_text::{SourceSpan, Span};
use std::{path::PathBuf, sync::Arc};
use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef};

// Supply the input contract of this stage, including the same package identity
// for every symbol. Build selection authority is checked by the source compiler.
fn selection() -> (TypedTrees, TypeReferenceHandle, TypeReferenceHandle) {
    let mut sources = SourceMap::default();
    let text = "Receipt Word Representation bits Permit";
    let source = sources
        .add_with_metadata(
            PathBuf::from("receipt.omg"),
            text.into(),
            PathBuf::from("package"),
            PackageKeyIdentity::from_digest([73; 32]),
            SourceOrigin::User,
        )
        .source_id;
    let token = |name: &str| {
        let start = text.find(name).unwrap();
        SymbolNameRef::Source(SourceSpan::new(
            source,
            Span::new(start, start + name.len()),
        ))
    };
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        crate::source_symbols::builtin_type_symbols()
            .into_iter()
            .chain([
                (SymbolKind::Data, token("Receipt")),
                (SymbolKind::Data, token("Word")),
                (SymbolKind::Conformance, token("Representation")),
                (SymbolKind::Data, token("Permit")),
            ]),
    );
    let opaque = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
    let word = children
        .handles()
        .nth(symbols::BUILTIN_TYPE_COUNT + 1)
        .unwrap();
    let conformance = children
        .handles()
        .nth(symbols::BUILTIN_TYPE_COUNT + 2)
        .unwrap();
    let field = builder
        .insert_children(word, [(SymbolKind::Field, token("bits"))])
        .handle_at(0);
    let mut typed = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let mut named = |name: &str, symbol| {
        typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol,
            name: name.into(),
        })
    };
    let receipt_type = named("Receipt", opaque);
    let permit = children
        .handles()
        .nth(symbols::BUILTIN_TYPE_COUNT + 3)
        .unwrap();
    let permit_type = named("Permit", permit);
    let word_type = named("Word", word);
    let bits = named(
        "u64",
        children
            .handles()
            .nth(BuiltinTypeAtom::U64.ordinal())
            .unwrap(),
    );
    let members = typed
        .data_members
        .insert_many([DataMember::Field(DataField {
            symbol: field,
            name: "bits".into(),
            type_reference: bits,
            ..Default::default()
        })]);
    typed.push_data_definition(DataDefinition {
        name: "Receipt".into(),
        symbol: opaque,
        supply_mode: language_core::DataSupplyMode::BoundaryOpaque,
        properties: DataProperties {
            multiplicity: language_core::Multiplicity::Linear,
            carry: None,
        },
        ..Default::default()
    });
    typed.push_data_definition(DataDefinition {
        name: "Permit".into(),
        symbol: permit,
        supply_mode: language_core::DataSupplyMode::BoundaryOpaque,
        ..Default::default()
    });
    typed.push_data_definition(DataDefinition {
        name: "Word".into(),
        symbol: word,
        members,
        ..Default::default()
    });
    let encoding = b"receipt-word-application".to_vec();
    let (report, commitment) =
        application_identity::conformance_identity::source_identity_commitments(&encoding);
    typed.opaque_representation_selections.push(
        OpaqueRepresentationSelection::from_validated_application(
            opaque,
            word,
            word_type,
            ClosedConformanceApplication {
                declaration: conformance,
                report_fingerprint: report,
                commitment: ClosedConformanceApplicationCommitment::from_digest(commitment),
                identity_encoding: encoding.into(),
                ..Default::default()
            },
            OpaqueRepresentationLifecycleDisposition::Inert,
            OpaqueRepresentationCopyDisposition::PlacementOnly,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
            symbols::SymbolHandle::invalid(),
            SourceSpan::default(),
        ),
    );
    (typed, receipt_type, permit_type)
}

#[test]
fn by_value_abi_publishes_carrier_bytes_and_preserves_the_opaque_claim() {
    let (typed, receipt, permit) = selection();
    let machine = MachineId::new(1).unwrap();
    let mut program = Program::new(&typed, false);
    program.begin_declaration_storage(machine, 0).unwrap();
    let opaque = program.calling_abi_storage(machine, receipt).unwrap();
    let permit = program.calling_abi_storage(machine, permit).unwrap();
    let carriers = program.prepare_opaque_carriers(machine).unwrap();
    assert_eq!(carriers.len(), 1);
    let decl = crate::lowering::lower::places::declare(
        machine,
        &program.records,
        &IdTable::default(),
        std::iter::empty::<&crate::lowering::Signature>(),
        &program.byte_domains,
        Some(&typed),
        Default::default(),
    )
    .unwrap();
    let mut module = TerminalModule::declarations();
    module.identities = decl.identities;
    module.types.structural_types = decl.structural_types;
    module.layouts.native_layouts = decl.native_layouts;
    let correspondence = program.source_record_correspondence(machine).unwrap();
    program
        .publish_source_declarations(machine, &correspondence, &mut module)
        .unwrap();
    let mut abi = CallingAbiStorage {
        parameters: vec![terminal_psi::CallingAbiParameter {
            structural_type: opaque,
            callback_backing: Some(opaque),
            access: StructuralAccess::SharedBorrow,
        }],
        result: None,
        telescope: terminal_psi::CallingSignatureTelescope {
            parameters: vec![],
            binders: vec![],
            direct_callbacks: vec![],
        },
    };
    program
        .publish_opaque_carriers(machine, carriers, std::iter::once(&abi), &mut module)
        .unwrap();
    assert!(module.types.opaque_representations.is_empty());
    assert_eq!(
        module.structural_type(opaque).unwrap().shape,
        StructuralTypeShape::Opaque
    );
    abi.parameters[0].access = StructuralAccess::Owned;
    let carriers = program.prepare_opaque_carriers(machine).unwrap();
    program
        .publish_opaque_carriers(machine, carriers, std::iter::once(&abi), &mut module)
        .unwrap();
    let [relation] = module.types.opaque_representations.as_slice() else {
        panic!("one by-value selection")
    };
    assert_eq!(relation.opaque, opaque);
    relation.application.validate_identity_encoding().unwrap();
    let nominal = module.structural_type(opaque).unwrap();
    let carrier = module.structural_type(relation.carrier).unwrap();
    assert!(nominal.owns_linear_claim);
    assert!(!carrier.owns_linear_claim);
    let left = nominal.shape.fields().next().unwrap();
    let right = carrier.shape.fields().next().unwrap();
    assert_ne!(left.id, right.id);
    assert_eq!(left.field_type, right.field_type);
    assert_eq!(module.identities.text(left.identity), Some("bits"));
    let carrier_id = relation.carrier;
    let carrier_field = right.id;
    let physical = module.take_opaque_storage_shapes().unwrap();
    let nominal = module.structural_type(opaque).unwrap();
    assert!(nominal.owns_linear_claim);
    assert_eq!(nominal.shape, StructuralTypeShape::Opaque);
    assert!(!terminal_semantics::zeroed_structural_shape(
        module.types.structural_types.iter(),
        opaque
    ));
    assert_eq!(
        module
            .structural_type(carrier_id)
            .unwrap()
            .shape
            .fields()
            .next()
            .unwrap()
            .id,
        carrier_field
    );
    physical.restore(&mut module).unwrap();
    let bytes = terminal_codec::encode_module(&module).unwrap();
    assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), module);
    let signature = semantic_vocabulary::CallingAbiIdentity {
        requirement_identity: "receive".into(),
        declaring_application: "receiver".into(),
        operands: vec![],
    }
    .id();
    abi.parameters.push(terminal_psi::CallingAbiParameter {
        structural_type: permit,
        callback_backing: Some(permit),
        access: StructuralAccess::SharedBorrow,
    });
    // The policy evaluator issues this storage query before invoking a policy.
    // A by-reference permit needs no bytes. Changing that same argument to
    // owned must fail even though the receipt has a selected representation.
    module
        .interfaces.calling_abi_signatures
        .push(terminal_psi::CallingAbiDeclaration {
            id: signature,
            requirement_identity: Default::default(),
            declaring_application: Default::default(),
            operands: vec![],
            formals: vec![],
            default_eligible: true,
            physical_owner: Ok(semantic_vocabulary::CallingDeclarationOwner::TopLevelRequirement),
            storage: Ok(abi),
        });
    let dependencies = |module: &TerminalModule| {
        terminal_semantics::calling_abi_extent_dependencies(module, signature, "linux_x86_64")
    };
    assert_eq!(dependencies(&module).unwrap(), []);
    module.interfaces.calling_abi_signatures[0]
        .storage
        .as_mut()
        .unwrap()
        .parameters[1]
        .access = StructuralAccess::Owned;
    assert_eq!(
        dependencies(&module).unwrap_err(),
        "opaque boundary value has no selected storage representation"
    );
    module.interfaces.calling_abi_signatures.clear();
    module.types.opaque_representations[0].carrier = opaque;
    let redirected = terminal_codec::encode_module(&module).unwrap();
    assert!(
        terminal_codec::decode_module(&redirected)
            .unwrap_err()
            .to_string()
            .contains("opaque representation substituted its exact storage identity")
    );
}
