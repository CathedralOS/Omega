//! Current-only admission preserves exact canonical bytes and refuses migration.
use super::{CodecError, FORMAT_MARKER, decode_module, encode_module};
use terminal_psi::VocabularyMarker;

// The same Unit semantics with the current declaration rosters and markers.
// Empty erased-scalar-formal rosters occupy four bytes at block offset 252
// and contract offset 289, and empty erased-proof rosters occupy four bytes
// at block offset 264 and module offset 309 (end); these are part of the
// current body, not markers.
const CURRENT_UNIT: &[u8] = &[
    80, 83, 73, 84, 69, 82, 77, 0, 115, 0, 116, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
fn current_unit_golden_bytes_remain_identical() {
    let module = decode_module(CURRENT_UNIT).expect("current Unit fixture");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    assert_eq!(module.machines.len(), 1);
    assert_eq!(
        module.machines[0].result,
        terminal_psi::TerminalMachineResult::Unit
    );
    assert_eq!(encode_module(&module).unwrap(), CURRENT_UNIT);
}

fn declared_service_module() -> terminal_psi::TerminalModule {
    let mut module = decode_module(CURRENT_UNIT).expect("current Unit fixture");
    let service = semantic_vocabulary::ServiceId::new(1).unwrap();
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service,
        identity: "Console".into(),
        parents: Vec::new(),
    });
    module.machines[0].declared_service_reach = vec![service];
    module.machines[0].published_service_ceiling = vec![service];
    module.root_service_reach.concrete = vec![service];
    module
}

fn closed_reach_module() -> terminal_psi::TerminalModule {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, OperationId};
    use terminal_psi::{
        ClosedReachApplication, ClosedReachCall, ClosedReachMachineBinding, ClosedReachParameter,
        Operation, OperationKind, OperationResult, Terminator,
    };
    let mut module = declared_service_module();
    let service = module.services[0].id;
    for ordinal in [2, 3] {
        let mut selected = module.machines[0].clone();
        selected.id = MachineId::new(ordinal).unwrap();
        selected.entry = BlockId::new(ordinal).unwrap();
        selected.contract.id = ContractId::new(ordinal).unwrap();
        selected.blocks[0].id = selected.entry;
        selected.blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(ordinal).unwrap(),
            trivial_affine_discards: Vec::new(),
        };
        module.machines.push(selected);
    }
    let binding = |ordinal| {
        ClosedReachParameter::Machine(ClosedReachMachineBinding {
            nominal_requirement: Some("Callback::call".into()),
            upper_bound: vec![service],
            selected_identity: format!("selected-{ordinal}"),
            selected_contract_commitment: [3; 32],
            selected_reach: vec![service],
            callee: Some(MachineId::new(ordinal).unwrap()),
            schema: None,
        })
    };
    let owner = &mut module.machines[0];
    owner.declared_service_reach.clear();
    owner.closed_reach_application = Some(ClosedReachApplication {
        template_identity: "forward".into(),
        template_commitment: [1; 32],
        specialization_commitment: [2; 32],
        telescope: vec![
            ClosedReachParameter::Type {
                argument: "u64".into(),
            },
            binding(2),
            ClosedReachParameter::Const {
                argument: "u64:2".into(),
            },
            binding(3),
        ],
        fixed: Vec::new(),
        dependencies: vec![1, 3],
        calls: vec![
            ClosedReachCall {
                operation: OperationId::new(1).unwrap(),
                binder: 1,
                application: None,
            },
            ClosedReachCall {
                operation: OperationId::new(2).unwrap(),
                binder: 3,
                application: None,
            },
        ],
    });
    for (ordinal, binder, callee) in [(1, 1, 2), (2, 3, 3)] {
        owner.blocks[0].operations.push(Operation {
            id: OperationId::new(ordinal).unwrap(),
            static_reach_binding: Some(binder),
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: MachineId::new(callee).unwrap(),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
    }
    module
}

#[test]
fn closed_reach_decoder_rejects_tampered_substitutions_and_call_joins() {
    use terminal_psi::{ClosedReachParameter, OperationKind};
    let original = closed_reach_module();
    let mutations: &[fn(&mut terminal_psi::TerminalModule)] = &[
        |module| module.machines[0].closed_reach_application = None,
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies
                .remove(0);
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls
                .clear()
        },
        |module| module.machines[0].blocks[0].operations[0].static_reach_binding = None,
        |module| module.machines[0].blocks[0].operations[0].static_reach_binding = Some(3),
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies = vec![0]
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies = vec![1, 1]
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies
                .clear()
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope
                .remove(0);
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_reach.clear();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.upper_bound.clear();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.nominal_requirement = None;
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.callee = None;
        },
        |module| {
            let OperationKind::CallUnit { callee, .. } =
                &mut module.machines[0].blocks[0].operations[0].kind
            else {
                panic!("fixture");
            };
            *callee = semantic_vocabulary::MachineId::new(3).unwrap();
        },
        |module| {
            module.machines[1].blocks[0].operations =
                std::mem::take(&mut module.machines[0].blocks[0].operations);
        },
    ];
    for (ordinal, mutate) in mutations.iter().enumerate() {
        let mut changed = original.clone();
        mutate(&mut changed);
        let bytes = super::encode_raw(&changed).expect("raw tampered product");
        assert!(
            decode_module(&bytes).is_err(),
            "mutation {ordinal} accepted"
        );
    }
}

#[test]
fn structural_callback_requirement_remains_fixed_beside_nominal_reach() {
    use terminal_psi::ClosedReachParameter;
    let mut module = closed_reach_module();
    let application = module.machines[0]
        .closed_reach_application
        .as_mut()
        .unwrap();
    application.dependencies = vec![3];
    application.fixed = vec![module.services[0].id];
    let ClosedReachParameter::Machine(binding) = &mut application.telescope[1] else {
        panic!("fixture");
    };
    binding.nominal_requirement = None;
    binding.selected_reach.clear();
    module.machines[1].declared_service_reach.clear();
    module.machines[1].published_service_ceiling.clear();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    module.machines[0]
        .closed_reach_application
        .as_mut()
        .unwrap()
        .fixed
        .clear();
    // The nominal selection still supplies Console. Matching the output union
    // cannot excuse omitting the structural callback's fixed requirement row.
    assert!(decode_module(&super::encode_raw(&module).unwrap()).is_err());
}

#[test]
fn declared_service_reach_decoder_rejects_corrupt_rows_and_missing_root() {
    let module = declared_service_module();
    let mut duplicate = module.clone();
    duplicate.machines[0]
        .declared_service_reach
        .push(semantic_vocabulary::ServiceId::new(1).unwrap());
    let bytes = super::encode_raw(&duplicate).expect("raw corrupt fixture");
    assert!(matches!(
        decode_module(&bytes),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::DuplicatePublishedService { .. }
        ))
    ));
    let mut missing_root = module;
    missing_root.root_service_reach.concrete.clear();
    let bytes = super::encode_raw(&missing_root).expect("raw stale root fixture");
    assert!(matches!(
        decode_module(&bytes),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::RootConcreteServiceReachMismatch { .. }
        ))
    ));
}

fn fixed_boundary_service_module() -> terminal_psi::TerminalModule {
    let mut module = decode_module(CURRENT_UNIT).expect("current Unit fixture");
    let fixed = semantic_vocabulary::ServiceId::new(1).unwrap();
    let bound = semantic_vocabulary::ServiceId::new(2).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
    module.services = vec![
        terminal_psi::ServiceDeclaration {
            id: fixed,
            identity: "Installer".into(),
            parents: Vec::new(),
        },
        terminal_psi::ServiceDeclaration {
            id: bound,
            identity: "Console".into(),
            parents: Vec::new(),
        },
    ];
    module
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            id: boundary,
            identity: "Installer::step".into(),
            attachment: None,
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: vec![fixed],
            published_service_ceiling: vec![fixed, bound],
        });
    module.machines[0].published_service_ceiling = vec![fixed, bound];
    module.machines[0].blocks[0]
        .operations
        .push(terminal_psi::Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: semantic_vocabulary::OperationId::new(1).unwrap(),
            result: terminal_psi::OperationResult::Unit,
            kind: terminal_psi::OperationKind::BoundaryCall {
                requirement_obligations: Vec::new(),
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        });
    module.root_service_reach.concrete = vec![fixed];
    module.root_service_reach.installation_dependencies =
        vec![terminal_psi::InstallationReachDependency {
            requirement_identity: "Installer::step".into(),
            upper_bound: vec![bound],
        }];
    module
}

#[test]
fn fixed_boundary_service_reach_decoder_rejects_corrupt_partition_and_rosters() {
    let module = fixed_boundary_service_module();
    for mutation in 0..6 {
        let mut corrupt = module.clone();
        match mutation {
            0 => corrupt.boundary_machines[0].fixed_service_reach.clear(),
            1 => corrupt.root_service_reach.concrete.clear(),
            2 => corrupt.root_service_reach.installation_dependencies[0]
                .upper_bound
                .clear(),
            3 => {
                corrupt.boundary_machines[0].fixed_service_reach =
                    vec![semantic_vocabulary::ServiceId::new(3).unwrap()];
            }
            4 => {
                let fixed = corrupt.boundary_machines[0].fixed_service_reach[0];
                corrupt.boundary_machines[0].fixed_service_reach.push(fixed);
            }
            5 => corrupt.boundary_machines[0]
                .published_service_ceiling
                .clear(),
            _ => unreachable!(),
        }
        let bytes = super::encode_raw(&corrupt).expect("raw corruption fixture");
        assert!(
            decode_module(&bytes).is_err(),
            "mutation {mutation} must reject"
        );
    }
}

#[test]
fn every_noncurrent_format_and_vocabulary_marker_rejects() {
    let mut header = CURRENT_UNIT[..12].to_vec();
    for marker in 0..=u16::MAX {
        if marker != FORMAT_MARKER {
            header[8..10].copy_from_slice(&marker.to_le_bytes());
            assert_eq!(
                decode_module(&header),
                Err(CodecError::UnsupportedFormatMarker(marker))
            );
        }
    }
    header[8..10].copy_from_slice(&FORMAT_MARKER.to_le_bytes());
    for marker in 0..=u16::MAX {
        if marker != VocabularyMarker::CURRENT.get() {
            header[10..12].copy_from_slice(&marker.to_le_bytes());
            assert_eq!(
                decode_module(&header),
                Err(CodecError::UnsupportedVocabularyMarker(marker))
            );
        }
    }
}

fn schema_reach_module() -> terminal_psi::TerminalModule {
    use terminal_psi::{
        ClosedReachApplication, ClosedReachArgument, ClosedReachCallApplication,
        ClosedReachParameter, ClosedReachSchema,
    };
    let mut module = closed_reach_module();
    let service = module.services[0].id;
    let application = module.machines[0]
        .closed_reach_application
        .as_mut()
        .unwrap();
    application.telescope.pop();
    application.dependencies = vec![1];
    let ClosedReachParameter::Machine(binding) = &mut application.telescope[1] else {
        panic!("schema fixture machine binder");
    };
    binding.selected_identity = "schema|selected=schema::entry".into();
    binding.callee = None;
    binding.schema = Some(ClosedReachSchema {
        template_identity: "schema".into(),
        template_commitment: [4; 32],
    });
    for (call, ordinal) in application.calls.iter_mut().zip([2_u8, 3]) {
        call.binder = 1;
        call.application = Some(ClosedReachCallApplication {
            callee: semantic_vocabulary::MachineId::new(u64::from(ordinal)).unwrap(),
            specialization_commitment: [ordinal; 32],
            arguments: vec![ClosedReachArgument::Const(format!("u64:{ordinal}"))],
        });
    }
    for operation in &mut module.machines[0].blocks[0].operations {
        operation.static_reach_binding = Some(1);
    }
    for (machine, ordinal) in module.machines[1..].iter_mut().zip([2_u8, 3]) {
        machine.closed_reach_application = Some(ClosedReachApplication {
            template_identity: "schema".into(),
            template_commitment: [4; 32],
            specialization_commitment: [ordinal; 32],
            telescope: vec![ClosedReachParameter::Const {
                argument: format!("u64:{ordinal}"),
            }],
            fixed: vec![service],
            dependencies: Vec::new(),
            calls: Vec::new(),
        });
    }
    module
}

#[test]
fn schema_reach_calls_roundtrip_distinct_tuples_and_reject_every_truncation() {
    let module = schema_reach_module();
    let bytes = encode_module(&module).expect("two applications of one schema encode");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    for length in 0..bytes.len() {
        assert!(
            decode_module(&bytes[..length]).is_err(),
            "schema truncation at {length}"
        );
    }
}

#[test]
fn schema_reach_decoder_rejects_changed_template_tuple_and_callee_joins() {
    use terminal_psi::{ClosedReachArgument, ClosedReachParameter, OperationKind};
    let original = schema_reach_module();
    encode_module(&original).expect("unmodified schema fixture must validate");
    let mutations: &[fn(&mut terminal_psi::TerminalModule)] = &[
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application = None
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.schema = None;
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.callee = Some(semantic_vocabulary::MachineId::new(2).unwrap());
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_identity = "other|selected=schema::entry".into();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_identity = "schema|selected=".into();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.schema.as_mut().unwrap().template_commitment = [0; 32];
        },
        |module| {
            // Keep both header copies in agreement: the selected owner must
            // still prevent a coherent substitution of an unrelated template.
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.schema.as_mut().unwrap().template_identity = "other".into();
            for machine in &mut module.machines[1..] {
                machine
                    .closed_reach_application
                    .as_mut()
                    .unwrap()
                    .template_identity = "other".into();
            }
        },
        |module| {
            module.machines[1]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .template_identity = "other".into()
        },
        |module| {
            module.machines[1]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .template_commitment = [5; 32]
        },
        |module| module.machines[1].closed_reach_application = None,
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .specialization_commitment = [3; 32]
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .arguments
                .clear()
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .arguments[0] = ClosedReachArgument::Const("u64:3".into())
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .arguments[0] = ClosedReachArgument::Type("u64:2".into())
        },
        |module| {
            module.machines[1]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[0] = ClosedReachParameter::Const {
                argument: "u64:3".into(),
            }
        },
        |module| {
            let OperationKind::CallUnit { callee, .. } =
                &mut module.machines[0].blocks[0].operations[0].kind
            else {
                panic!("fixture");
            };
            *callee = semantic_vocabulary::MachineId::new(3).unwrap();
        },
        |module| {
            // Redirect both executable and roster callees, but retain the
            // expected original tuple and commitment.
            let redirected = semantic_vocabulary::MachineId::new(3).unwrap();
            let OperationKind::CallUnit { callee, .. } =
                &mut module.machines[0].blocks[0].operations[0].kind
            else {
                panic!("fixture");
            };
            *callee = redirected;
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .callee = redirected;
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_reach.clear();
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .fixed = vec![module.services[0].id];
        },
    ];
    for (ordinal, mutate) in mutations.iter().enumerate() {
        let mut changed = original.clone();
        mutate(&mut changed);
        let bytes = super::encode_raw(&changed).expect("raw schema corruption encodes");
        assert!(
            decode_module(&bytes).is_err(),
            "schema mutation {ordinal} accepted"
        );
    }
}

#[test]
fn nominal_schema_dependency_follows_helpers_but_not_disconnected_applications() {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, OperationId};
    use terminal_psi::{Operation, OperationKind, OperationResult, Terminator};
    let mut module = schema_reach_module();
    let mut helper = module.machines[0].clone();
    helper.id = MachineId::new(4).unwrap();
    helper.entry = BlockId::new(4).unwrap();
    helper.contract.id = ContractId::new(4).unwrap();
    helper.blocks[0].id = helper.entry;
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(4).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    helper
        .closed_reach_application
        .as_mut()
        .unwrap()
        .template_identity = "helper".into();
    helper
        .closed_reach_application
        .as_mut()
        .unwrap()
        .template_commitment = [6; 32];
    helper
        .closed_reach_application
        .as_mut()
        .unwrap()
        .specialization_commitment = [7; 32];
    let owner = &mut module.machines[0];
    owner
        .closed_reach_application
        .as_mut()
        .unwrap()
        .calls
        .clear();
    owner.blocks[0].operations = vec![Operation {
        id: OperationId::new(3).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: helper.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    module.machines.push(helper);
    let bytes = encode_module(&module).expect("nominal schema dependency forwards through helper");
    assert_eq!(decode_module(&bytes).unwrap(), module);

    module.machines[0].blocks[0].operations.clear();
    // Preserve ordinary concrete reach using a declaration, isolating the
    // missing schema application coverage from ordinary service-closure errors.
    module.machines[0].declared_service_reach = vec![module.services[0].id];
    assert!(matches!(
        terminal_verifier::validate_module_representation(&module),
        Err(terminal_verifier::ModuleError::InvalidClosedReachApplication { .. })
    ));
    let disconnected = super::encode_raw(&module).expect("disconnected schema fixture encodes raw");
    assert!(decode_module(&disconnected).is_err());
}

#[test]
fn element_view_write_requires_a_valid_module_to_encode() {
    use semantic_vocabulary::{ObligationId, OperationId, PlaceId, ValueId};
    use terminal_psi::{Operation, OperationKind, OperationResult};

    // Validation runs on both wire directions, so an invalid write never
    // reaches the format; the op-level wire pair lives in block_wire tests.
    let mut module = declared_service_module();
    module.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(9).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::ElementViewWrite {
            destination: PlaceId::new(1).unwrap(),
            index: ValueId::new(1).unwrap(),
            value: ValueId::new(2).unwrap(),
            length: ValueId::new(3).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        },
    });
    assert!(encode_module(&module).is_err());
    let raw = super::encode_raw(&module).expect("raw encoding skips validation");
    assert!(decode_module(&raw).is_err());
}

#[test]
fn boundary_scalar_guarantees_preserve_bytes_and_exact_result_namespace() {
    use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, ValueId};
    let mut module = fixed_boundary_service_module();
    let value = ValueId::new(1).unwrap();
    module.boundary_machines[0].result =
        terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Boolean);
    module.boundary_machines[0].scalar_ensures = vec![Proposition::Equal(
        ScalarTerm::value(value, ScalarType::Boolean),
        ScalarTerm::boolean(true),
    )];
    module.machines[0].blocks[0].operations[0].result =
        terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
            id: value,
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        });
    let bytes = encode_module(&module).expect("result guarantee in its own telescope");
    let reloaded = decode_module(&bytes).expect("canonical result guarantee");
    assert_eq!(
        reloaded.boundary_machines[0].scalar_ensures,
        module.boundary_machines[0].scalar_ensures
    );
    assert_eq!(encode_module(&reloaded).unwrap(), bytes);
    module.boundary_machines[0].scalar_ensures[0] = Proposition::Equal(
        ScalarTerm::value(value, ScalarType::Boolean),
        ScalarTerm::boolean(false),
    );
    assert_ne!(
        encode_module(&module).unwrap(),
        bytes,
        "contract changes change artifact identity"
    );
    module.boundary_machines[0].scalar_ensures[0] = Proposition::Equal(
        ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean),
        ScalarTerm::boolean(true),
    );
    let forged = super::encode_raw(&module).unwrap();
    assert!(
        matches!(
            decode_module(&forged),
            Err(CodecError::InvalidModule(
                terminal_verifier::ModuleError::MalformedProposition(_)
            ))
        ),
        "a guarantee cannot name a caller value or another result coordinate"
    );
}
