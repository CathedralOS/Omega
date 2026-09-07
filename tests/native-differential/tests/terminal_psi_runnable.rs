use abstract_operations::AbstractOperation;
use abstract_operations_to_abstract_operations::validation::validate_verified_psi_optimization_unit;
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, LoweringError,
    lower_to_target_operations_with_provider_executions,
};
use calling_conventions::{CallSignature, ValueShape};
use omega_native_differential_test::admit_native_provider;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{BoundaryMachineId, StructuralPlaceKind};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations::{LinuxExitGroupI32Realization, LinuxWriteLineRealization};
use terminal_codec::{decode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi::TerminalModule;
use terminal_psi_to_abstract_operations::{
    build_verified_psi_optimization_unit, lower_artifact_sections,
    lower_artifact_sections_for_optimization,
};
use terminal_verifier::ProofBundle;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn straight_line_console_source(write_count: usize, exit_status: i32) -> String {
    let writes = (0..write_count)
        .map(|index| format!("        self.console.write_line(\"line-{index:02}\");\n"))
        .collect::<String>();
    format!(
        r#"
    boundary trait Console {{
        machine write_line(text: &[u8])
        reaches Console;
        machine exit_process(return_code: i32)
        reaches Console;
    }}

    data Main {{ console: Console; }}
    machine Main::main(&mut self)
    reaches Console
    {{
{writes}        self.console.exit_process({exit_status});
    }}
"#
    )
}

fn canonical_console_source() -> &'static str {
    r#"
    boundary trait Console {
        machine write_line(text: &[u8])
        reaches Console;
        machine exit_process(return_code: i32)
        reaches Console;
    }

    data Main { console: Console; }
    machine Main::main(&mut self)
    reaches Console
    {
        self.console.write_line("Hello, Omega.");
        self.console.exit_process(0);
    }
"#
}

const CONCRETE_ROOT_SERVICE_REACH_SOURCE: &str = r#"
    boundary trait PortIo {}
    pub data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Root {}
    machine Root::enter(receipt: Receipt)
    reaches PortIo
    {
        Receipt::settle(receipt);
    }
"#;

const BOUNDED_ROOT_SERVICE_REACH_SOURCE: &str = r#"
    boundary trait MachineControl {}
    boundary trait PortIo {}

    boundary trait InterruptCompletion {
        machine complete() -> bool
        reaches <= MachineControl + PortIo;
    }

    data Root {}
    machine Root::enter<machine Completion>() -> bool
    where machine Completion satisfies InterruptCompletion::complete;
    {
        let accepted: bool = Completion();
        accepted
    }
"#;

fn project_source(source: &str) -> (Vec<u8>, Vec<u8>) {
    project_source_entry(source, "Main::main")
}

fn project_source_entry(source: &str, entry: &str) -> (Vec<u8>, Vec<u8>) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize O1 source");
    let syntax = parse_syntax_trees(&tokens).expect("parse O1 source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve O1 source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type O1 source");
    let checked = lower_typed_trees(typed).expect("check O1 source");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .expect("lower O1 source to terminal Psi");
    (
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode O1 terminal Psi"),
        encode_proof_bundle(&lowered.proof_bundle).expect("encode O1 proof bundle"),
    )
}

#[test]
fn source_byte_sequence_literal_reaches_verified_optimizer_admission() {
    let (semantic, proof) = project_source(canonical_console_source());
    let input =
        lower_artifact_sections_for_optimization(&semantic, &proof, &AdmissionProfile::default())
            .expect("source byte literal verifies for optimizer admission");
    let verified =
        build_verified_psi_optimization_unit(input, TerminalFuelSchedule::CURRENT.identity())
            .expect("source byte literal retains its optimizer unit");
    validate_verified_psi_optimization_unit(&verified)
        .expect("source byte-literal producer dominates its boundary use");

    let function = verified
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == verified.unit().entry)
        .expect("optimizer unit retains the source entry");
    let [block] = function.blocks.as_slice() else {
        panic!("current source byte-literal lowering remains one block")
    };
    let (producer_index, literal) = block
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| match &node.operation {
            AbstractOperation::EstablishByteSequenceLiteral { place, bytes, .. }
                if bytes == b"Hello, Omega." =>
            {
                Some((index, place.id))
            }
            _ => None,
        })
        .expect("source lowering retains the exact byte literal");
    let use_index = block
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| match &node.operation {
            AbstractOperation::BoundaryCall {
                structural_arguments,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| argument.place == literal) =>
            {
                Some(index)
            }
            _ => None,
        })
        .expect("source lowering retains the literal boundary use");
    assert!(producer_index < use_index);
}

#[test]
fn source_provider_attachment_specialization_reaches_verified_optimizer_admission() {
    let (semantic, proof) = project_source(&straight_line_console_source(2, 0));
    let input =
        lower_artifact_sections_for_optimization(&semantic, &proof, &AdmissionProfile::default())
            .expect("source provider attachment verifies for optimizer admission");
    let verified =
        build_verified_psi_optimization_unit(input, TerminalFuelSchedule::CURRENT.identity())
            .expect("source provider attachment retains its optimizer unit");
    validate_verified_psi_optimization_unit(&verified)
        .expect("source provider attachment satisfies exact specialization replay");

    let source_services = &verified.input().context().module().services;
    assert_eq!(
        verified.unit().services.as_ref(),
        source_services.as_slice()
    );
    assert_eq!(
        verified.unit().root_service_reach,
        verified.input().context().module().root_service_reach
    );
    let service_catalog = verified
        .unit()
        .services
        .iter()
        .map(|service| service.id)
        .collect::<std::collections::BTreeSet<_>>();
    let console_service = verified
        .unit()
        .services
        .iter()
        .find(|service| service.identity.ends_with("Console"))
        .expect("source Console declaration remains in optimizer custody");
    assert!(console_service.parents.is_empty());
    assert_eq!(
        verified.unit().root_service_reach.concrete,
        [console_service.id]
    );
    assert!(
        verified
            .unit()
            .root_service_reach
            .installation_dependencies
            .is_empty()
    );
    assert!(verified.unit().functions.iter().all(|function| {
        function
            .published_service_ceiling
            .iter()
            .all(|service| service_catalog.contains(service))
    }));
    assert!(verified.unit().boundary_machines.iter().all(|boundary| {
        boundary
            .published_service_ceiling
            .iter()
            .all(|service| service_catalog.contains(service))
    }));
    for function in &verified.unit().functions {
        for node in function.blocks.iter().flat_map(|block| &block.nodes) {
            if let AbstractOperation::PortWrite { service, .. } = node.operation {
                assert!(service_catalog.contains(&service));
                assert!(function.published_service_ceiling.contains(&service));
            }
        }
    }
    for provider in &verified.unit().provider_candidates {
        let candidate = verified
            .unit()
            .functions
            .iter()
            .find(|function| function.machine == provider.candidate)
            .expect("source provider candidate remains a function");
        let boundary = verified
            .unit()
            .boundary_machines
            .iter()
            .find(|boundary| boundary.id == provider.boundary)
            .expect("source provider boundary remains declared");
        assert_eq!(
            provider.refinement.realized_service_ceiling,
            candidate.published_service_ceiling
        );
        assert!(
            provider
                .refinement
                .realized_service_ceiling
                .iter()
                .all(|service| boundary.published_service_ceiling.contains(service))
        );
    }

    let function = verified
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == verified.unit().entry)
        .expect("optimizer unit retains the provider-backed source entry");
    assert!(
        function
            .published_service_ceiling
            .contains(&console_service.id)
    );
    let provider_roots = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment { boundary, .. } => Some((place.id, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(provider_roots.len(), 2);
    assert!(provider_roots.windows(2).all(|pair| pair[0].1 < pair[1].1));
    let provider_places = provider_roots
        .iter()
        .map(|(place, _)| *place)
        .collect::<std::collections::BTreeSet<_>>();
    let rooted_boundaries = provider_roots
        .iter()
        .map(|(_, boundary)| *boundary)
        .collect::<std::collections::BTreeSet<_>>();
    let boundary_calls = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            AbstractOperation::BoundaryCall {
                boundary,
                structural_arguments,
                ..
            } => {
                let declaration = verified
                    .unit()
                    .boundary_machines
                    .iter()
                    .find(|declaration| declaration.id == *boundary)
                    .expect("source boundary call retains its declaration");
                assert!(
                    declaration
                        .published_service_ceiling
                        .iter()
                        .all(|service| function.published_service_ceiling.contains(service))
                );
                assert!(
                    structural_arguments
                        .iter()
                        .all(|argument| !provider_places.contains(&argument.place))
                );
                Some(*boundary)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(boundary_calls.len(), 3);
    assert_eq!(
        boundary_calls
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        rooted_boundaries
    );
}

#[test]
fn source_concrete_root_service_reach_reaches_verified_optimizer_admission() {
    let (semantic, proof) = project_source_entry(CONCRETE_ROOT_SERVICE_REACH_SOURCE, "Root::enter");
    let input =
        lower_artifact_sections_for_optimization(&semantic, &proof, &AdmissionProfile::default())
            .expect("source concrete service reach verifies for optimizer admission");
    let verified =
        build_verified_psi_optimization_unit(input, TerminalFuelSchedule::CURRENT.identity())
            .expect("source concrete service reach retains its optimizer unit");
    validate_verified_psi_optimization_unit(&verified)
        .expect("source concrete service reach passes independent replay");

    assert_eq!(
        verified.unit().root_service_reach,
        verified.input().context().module().root_service_reach
    );
    assert!(
        verified
            .unit()
            .root_service_reach
            .installation_dependencies
            .is_empty()
    );
    let concrete = verified
        .unit()
        .root_service_reach
        .concrete
        .iter()
        .map(|service| {
            verified
                .unit()
                .services
                .iter()
                .find(|declaration| declaration.id == *service)
                .expect("root-reachable service remains declared")
                .identity
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(concrete, ["PortIo"]);
}

#[test]
fn source_bounded_root_service_reach_reaches_verified_optimizer_admission() {
    let (semantic, proof) = project_source_entry(BOUNDED_ROOT_SERVICE_REACH_SOURCE, "Root::enter");
    let input =
        lower_artifact_sections_for_optimization(&semantic, &proof, &AdmissionProfile::default())
            .expect("source bounded service reach verifies for optimizer admission");
    let verified =
        build_verified_psi_optimization_unit(input, TerminalFuelSchedule::CURRENT.identity())
            .expect("source bounded service reach retains its optimizer unit");
    validate_verified_psi_optimization_unit(&verified)
        .expect("source bounded service reach passes independent replay");

    assert_eq!(
        verified.unit().root_service_reach,
        verified.input().context().module().root_service_reach
    );
    assert!(verified.unit().root_service_reach.concrete.is_empty());
    let [dependency] = verified
        .unit()
        .root_service_reach
        .installation_dependencies
        .as_slice()
    else {
        panic!("source bounded reach retains one installation dependency")
    };
    let boundary = verified
        .unit()
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity == dependency.requirement_identity)
        .expect("installation dependency names the retained requirement boundary");
    assert_eq!(boundary.published_service_ceiling, dependency.upper_bound);
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|service| {
            verified
                .unit()
                .services
                .iter()
                .find(|declaration| declaration.id == *service)
                .expect("bounded service remains declared")
                .identity
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);
}

#[test]
fn native_o0_lowering_rejects_a_provider_admitted_for_another_requirement() {
    let semantic = canonical_o0_bytes();
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("canonical empty proof");
    let decoded = decode_module(&semantic).expect("decode fixture");
    let (write_boundary, exit_boundary) = o0_boundaries(&decoded);
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified O0 plan");
    let target = NativeTarget::linux_x64();
    let wrong_write_provider = admit_native_provider(
        target,
        boundary_identity(&decoded, exit_boundary),
        30_000,
        CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
            result: None,
        },
    );
    let exit_provider = admit_native_provider(
        target,
        boundary_identity(&decoded, exit_boundary),
        31_000,
        CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: None,
        },
    );
    assert!(matches!(
        lower_to_target_operations_with_provider_executions(
            &plan,
            target,
            &[
                AdmittedBoundarySettlement {
                    boundary: write_boundary,
                    execution: AdmittedBoundaryExecution::Provider(&wrong_write_provider),
                    realization: LinuxWriteLineRealization.into(),
                },
                AdmittedBoundarySettlement {
                    boundary: exit_boundary,
                    execution: AdmittedBoundaryExecution::Provider(&exit_provider),
                    realization: LinuxExitGroupI32Realization.into(),
                },
            ],
        ),
        Err(LoweringError::ProviderExecutionRequirementMismatch { boundary, .. })
            if boundary == write_boundary
    ));
}

fn canonical_o0_bytes() -> Vec<u8> {
    project_source(canonical_console_source()).0
}

fn o0_boundaries(module: &TerminalModule) -> (BoundaryMachineId, BoundaryMachineId) {
    (
        find_boundary(module, "Console::write_line"),
        find_boundary(module, "Console::exit_process"),
    )
}

fn find_boundary(module: &TerminalModule, name: &str) -> BoundaryMachineId {
    module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains(name))
        .unwrap_or_else(|| panic!("canonical console boundary {name}"))
        .id
}

fn boundary_identity(module: &TerminalModule, id: BoundaryMachineId) -> &str {
    &module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.id == id)
        .expect("O0 boundary declaration")
        .identity
}
