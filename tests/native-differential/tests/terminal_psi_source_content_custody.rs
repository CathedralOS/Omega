//! Source-to-install canary for one whole content-bearing custody exit.

use std::path::{Path, PathBuf};

use omega_abstract_operations::AbstractOperation;
use omega_abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, lower_to_target_operations_with_provider_executions,
};
use omega_calling_conventions::{CallSignature, ValueShape};
use omega_image_emission::{
    build_installation_record_with_provider_executions, build_object_artifact,
    decode_installation_record, emit_executable_image, encode_installation_record,
    validate_installation_record,
};
use omega_machine_emission::emit_machine_code;
use omega_native_differential_test::admit_native_provider;
use omega_optimization_validation::validate_verified_psi_optimization_unit;
use omega_psi_to_abstract_operations::{
    build_verified_psi_optimization_unit, lower_artifact_sections,
    lower_artifact_sections_for_optimization,
};
use omega_target_operations::{BoundaryRealization, DirectPortReadU8Realization};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::{ContentAlgebraKind, ProfileDecisionId};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_fuel::TerminalFuelSchedule;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn source_canary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .join("tests/omega/pass/terminal_psi/content_custody_exit/main.omg")
}

#[test]
fn source_whole_content_custody_exit_reaches_canonical_installation() {
    let source = std::fs::read_to_string(source_canary()).expect("read content-custody canary");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let lowered = lower_machine(&checked, "Root::enter").expect("lower source to terminal Psi");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    let profile = AdmissionProfile::default();
    let optimizer_input =
        lower_artifact_sections_for_optimization(&semantic_bytes, &proof_bytes, &profile)
            .expect("verify canonical optimizer input");
    let verified_unit = build_verified_psi_optimization_unit(
        optimizer_input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("retain content-bearing optimizer unit");
    validate_verified_psi_optimization_unit(&verified_unit)
        .expect("content-bearing source custody validates at optimizer admission");
    assert_eq!(
        verified_unit.unit().structural_domains.as_ref(),
        lowered.semantic_module.structural_domains.as_slice(),
        "optimizer admission retains the exact source-derived projection owners"
    );
    assert!(
        verified_unit
            .unit()
            .structural_domains
            .iter()
            .any(|domain| domain.content_projection.is_some()),
        "the source canary must exercise structural-domain projection replay"
    );
    assert_eq!(
        verified_unit.unit().functions[0].content_entry_claims,
        lowered.semantic_module.machines[0].content_entry_claims
    );
    let abstract_plan = lower_artifact_sections(&semantic_bytes, &proof_bytes, &profile)
        .expect("verify and lower canonical terminal artifact");

    let [function] = abstract_plan.functions.as_slice() else {
        panic!("source canary must lower one function")
    };
    let Some((boundary, completion_sources, receipts)) =
        function.operations.iter().find_map(|operation| {
            let AbstractOperation::BoundaryCall {
                boundary,
                completion_claim_sources,
                completion_receipts,
                ..
            } = operation
            else {
                return None;
            };
            Some((*boundary, completion_claim_sources, completion_receipts))
        })
    else {
        panic!("source canary must retain its bodyless custody exit")
    };
    let [source] = completion_sources.as_slice() else {
        panic!("source canary must retain one completion source")
    };
    let content = source
        .content
        .as_ref()
        .expect("completion source must retain content identity");
    assert_eq!(source.claim, content.claim);
    assert!(content.input.segments.is_empty());
    let [projection] = content.projections.as_slice() else {
        panic!("content claim must retain its owner-unique projection")
    };
    assert_eq!(projection.algebra.kind, ContentAlgebraKind::CountedQuantity);
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].claim, source.claim);

    let boundary_declaration = abstract_plan
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == boundary)
        .expect("bodyless requirement declaration");
    let [service] = boundary_declaration.published_service_ceiling.as_slice() else {
        panic!("bodyless requirement must retain its exact PortIo service")
    };
    let execution = admit_native_provider(
        omega_target::NativeTarget::linux_x64(),
        &boundary_declaration.identity,
        0xc017_0000,
        CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(1, 1)),
        },
    );
    let settlement = AdmittedBoundarySettlement {
        boundary,
        provider_execution: &execution,
        realization: BoundaryRealization::DirectPortReadU8(DirectPortReadU8Realization {
            service: *service,
            port: 0x60,
        })
        .into(),
    };
    let mut unbound_linear_input = abstract_plan.clone();
    let AbstractOperation::BoundaryCall {
        completion_receipts,
        ..
    } = unbound_linear_input.functions[0]
        .operations
        .iter_mut()
        .find(|operation| matches!(operation, AbstractOperation::BoundaryCall { .. }))
        .expect("source boundary call")
    else {
        unreachable!()
    };
    completion_receipts.clear();
    assert!(matches!(
        lower_to_target_operations_with_provider_executions(
            &unbound_linear_input,
            omega_target::NativeTarget::linux_x64(),
            &[settlement.clone()],
        ),
        Err(
            omega_abstract_operations_to_target_operations::LoweringError::UnsupportedOperationInScalarFunction(_)
        )
    ));
    let target = lower_to_target_operations_with_provider_executions(
        &abstract_plan,
        omega_target::NativeTarget::linux_x64(),
        &[settlement],
    )
    .expect("bind exact provider and lower source-derived custody exit");
    let assigned = assign_registers(&target).expect("assign target locations");
    let machine = emit_machine_code(&assigned).expect("emit machine code");
    let [settlement] = machine.functions[0].boundary_settlements.as_slice() else {
        panic!("machine code must retain one boundary settlement")
    };
    assert_eq!(
        settlement.completion_claim_sources,
        completion_sources.as_slice()
    );
    let [provider_custody] = settlement.completion_provider_custody.as_slice() else {
        panic!("successful settlement must transfer one claim into provider custody")
    };
    assert_eq!(provider_custody.source, *source);
    assert_eq!(provider_custody.receipt, receipts[0]);
    assert_eq!(
        provider_custody
            .provider_execution
            .provider_plan_report_identity,
        execution.provider_plan().normalized_identity()
    );

    let mut missing_content_source = machine.clone();
    missing_content_source.functions[0].boundary_settlements[0].completion_claim_sources[0]
        .content = None;
    let missing_content_error = build_object_artifact(&missing_content_source)
        .expect_err("missing content source must reject");
    assert!(matches!(
        missing_content_error,
        omega_image_emission::ObjectError::InvalidCompletionProviderCustody { .. }
    ));

    let mut substituted_receipt = machine.clone();
    substituted_receipt.functions[0].boundary_settlements[0].completion_receipts[0].claim =
        psi_core::ClaimId::new(source.claim.get() + 1).expect("substituted claim");
    assert!(matches!(
        build_object_artifact(&substituted_receipt),
        Err(omega_image_emission::ObjectError::InvalidCompletionReceiptCustody { .. })
    ));

    let mut substituted_provider = machine.clone();
    substituted_provider.functions[0].boundary_settlements[0].completion_provider_custody[0]
        .provider_execution
        .provider_plan_report_identity ^= 1;
    assert!(matches!(
        build_object_artifact(&substituted_provider),
        Err(omega_image_emission::ObjectError::InvalidCompletionProviderCustody { .. })
    ));

    let object = build_object_artifact(&machine).expect("build terminal object");
    let image = emit_executable_image(&object, 3).expect("emit executable image");
    let installation = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(1).expect("profile decision"),
        [&execution],
    )
    .expect("bind provider custody into installation");
    validate_installation_record(&installation, &image)
        .expect("installation replays image custody");
    let encoded = encode_installation_record(&installation).expect("encode canonical installation");
    let decoded = decode_installation_record(&encoded).expect("decode canonical installation");
    assert_eq!(decoded, installation);
    assert_eq!(
        decoded.boundary_settlements()[0]
            .settlement
            .completion_claim_sources,
        completion_sources.as_slice()
    );
    assert_eq!(
        decoded.boundary_settlements()[0]
            .settlement
            .completion_provider_custody,
        settlement.completion_provider_custody
    );
}
