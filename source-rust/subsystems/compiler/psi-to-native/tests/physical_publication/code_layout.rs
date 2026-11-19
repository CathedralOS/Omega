// A canonical query tests native publication independently of source syntax.
// The queried machine is reached only by the query and never executes.
use super::*;
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralPlaceKind};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
};

fn query_artifact(selected: MachineId) -> Arc<CanonicalTerminalArtifact> {
    let mut module = constant_artifact(false).semantic_module().clone();
    let mut identities = module.identities.clone().into_builder();
    let region = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let layout = StructuralTypeId::new(3).unwrap();
    let address =
        StructuralFieldType::Scalar(ScalarType::Integer(IntegerType::address(64).unwrap()));
    let unsigned = StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    ));
    let mut field = |number, name, field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(number).unwrap(),
        identity: identities.intern(name),
        declaration_identity: Default::default(),
        field_type,
        inline_carrier: None,
        storage: None,
        relevance: BindingRelevance::Relevant,
    };
    let region_fields = vec![
        field(1, "base", address.clone()),
        field(2, "length", unsigned),
    ];
    let layout_fields = vec![
        field(3, "entry", address),
        field(4, "regions", StructuralFieldType::Structural(view)),
    ];
    let mut declare = |id, name, multiplicity, shape| {
        module.types.structural_types.push(StructuralTypeDeclaration {
            id,
            identity: identities.intern(name),
            declaration_identity: Default::default(),
            quotient_carrier: None,
            multiplicity,
            owns_linear_claim: false,
            no_code_affine_cleanup: multiplicity == StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            requires: Vec::new(),
            shape,
        });
    };
    declare(
        region,
        "CodeRegion",
        StructuralMultiplicity::Unrestricted,
        StructuralTypeShape::Record {
            fields: region_fields,
            storage: StructuralRecordStorage::Packed,
        },
    );
    declare(
        view,
        "RegionView",
        StructuralMultiplicity::Affine,
        StructuralTypeShape::ElementView {
            element: region,
            access: Some(StructuralAccess::SharedBorrow),
        },
    );
    declare(
        layout,
        "CodeLayout",
        StructuralMultiplicity::Affine,
        StructuralTypeShape::Record {
            fields: layout_fields,
            storage: StructuralRecordStorage::Packed,
        },
    );
    module.identities = identities.build();

    let mut selected_body = module.machines[0].clone();
    selected_body.id = selected;
    selected_body.contract.id = ContractId::new(2).unwrap();
    selected_body.blocks[0].operations[0].id = OperationId::new(3).unwrap();
    module.machines.push(selected_body);

    let root = &mut module.machines[0];
    let producer = OperationId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    root.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type: layout,
        },
    });
    let block = &mut root.blocks[0];
    block.operations[0].id = OperationId::new(2).unwrap();
    block.operations.insert(
        0,
        Operation::unsuspended(
            producer,
            OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type: layout,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishNativeCodeLayout { machine: selected },
        ),
    );
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut block.terminator
    else {
        unreachable!()
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::RetireHome(place));

    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    Arc::new(CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap())
}

#[test]
fn published_image_reads_the_selected_machines_linked_layout() {
    let selected = MachineId::new(2).unwrap();
    check_canonical_publication(
        query_artifact(selected),
        false,
        false,
        |_, _| {},
        |image| {
            let layouts = image.inspect_root_code_layouts().unwrap();
            let [only] = layouts.as_slice() else {
                panic!(
                    "only the machine retained by the layout query has a descriptor: {layouts:?}"
                );
            };
            assert_eq!(only.machine, selected);
            let output = image.image().output();
            let text = output.final_image_layout.text_address
                ..output.final_image_layout.text_address + output.final_text_bytes.len() as u64;
            let placed = image.object().function_on_machine(selected).unwrap();
            let start = text.start + u64::try_from(placed.text_offset).unwrap();
            let end = start + u64::try_from(placed.byte_count).unwrap();
            assert_eq!(only.entry, start);
            assert!(text.contains(&only.entry));
            assert!(only.regions.iter().all(|&(base, length)| {
                base >= start && base.checked_add(length).is_some_and(|limit| limit <= end)
            }));
            assert!(
                only.regions
                    .iter()
                    .any(|&(base, length)| (base..base + length).contains(&only.entry))
            );
        },
    );
}
