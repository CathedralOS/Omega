//! Source-free verification must check operands following a runtime value
//! that prevents constant discharge.

use compiler::{CheckedCompileRequest, compile_to_checked};
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics_catalog::{
    FLOAT_SEMANTICS_NAMESPACE, FloatSemanticOperation, FloatSemanticValueKind,
};
use semantic_vocabulary::IeeeFloatFormat;
use terminal_psi::{
    DirectMachineFloatParameter, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatMeaningSource, FloatProjectionContractIdentity, FloatSemanticApplication,
    FloatSemanticApplicationOperand, FloatSemanticContractIdentity, ProofOnlyValueType,
    ProofValueDeclaration, ProofValueId, TerminalModule,
};
use terminal_verifier::{FloatMeaningProjectionVerificationError as Error, ModuleError};

fn projection(value: u32, source: FloatMeaningSource) -> FloatMeaningProjection {
    let operation = match source.format() {
        IeeeFloatFormat::Binary32 => FloatProjectionOperation::Meaning32,
        IeeeFloatFormat::Binary64 => FloatProjectionOperation::Meaning64,
    };
    let contract = operation.contract_identity();
    FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(value),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source,
        operation: match operation {
            FloatProjectionOperation::Meaning32 => FloatMeaningProjectionOperation::Meaning32,
            FloatProjectionOperation::Meaning64 => FloatMeaningProjectionOperation::Meaning64,
        },
        contract: FloatProjectionContractIdentity {
            format: contract.format,
            operation: contract.operation,
            declaration: contract.declaration,
            catalog_version: contract.catalog_version,
            commitment: contract.commitment,
        },
    }
}

fn minimum(left: u32, right: u32) -> FloatMeaningSource {
    use FloatSemanticValueKind::Meaning;
    let contract = FloatSemanticOperation::from_source_identity(
        FLOAT_SEMANTICS_NAMESPACE,
        "minimum",
        &[Meaning, Meaning],
        Meaning,
    )
    .unwrap()
    .contract_identity();
    FloatMeaningSource::SemanticApplication(FloatSemanticApplication {
        contract: FloatSemanticContractIdentity {
            row: contract.row,
            catalog_version: contract.catalog_version,
            commitment: contract.commitment,
        },
        format: IeeeFloatFormat::Binary32,
        operands: vec![
            FloatSemanticApplicationOperand::Meaning(ProofValueId(left)),
            FloatSemanticApplicationOperand::Meaning(ProofValueId(right)),
        ],
    })
}

fn compile_source(
    source: &str,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let directory = std::env::temp_dir().join(format!(
        "omega-float-operands-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&directory).unwrap();
    let main = directory.join("main.omg");
    std::fs::write(&main, source).unwrap();
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None));
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(!directory.exists());
    checked
}

fn source_free_module() -> TerminalModule {
    let checked = compile_source("machine read(value: f32) -> f32 { value }").unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .unwrap();
    let bytes = artifact.to_bytes();
    drop(artifact);
    drop(checked);
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let machine = &module.machines[0];
    assert!(module.float_meaning_projections.is_empty());
    // The application writer is unfinished. Construct its proposed metadata
    // explicitly; this does not claim source-level proof acceptance.
    module.float_meaning_projections = vec![
        projection(
            0,
            FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
                owner: machine.id,
                parameter: machine.parameters[0].id,
                format: IeeeFloatFormat::Binary32,
            }),
        ),
        projection(
            1,
            FloatMeaningSource::ExactBinary32Literal(2.0_f32.to_bits()),
        ),
        projection(2, minimum(0, 1)),
        projection(3, minimum(2, 1)),
    ];
    terminal_codec::decode_module(&terminal_codec::encode_module(&module).unwrap()).unwrap()
}

#[test]
fn closed_semantic_application_proves_real_core_source_contract() {
    let checked = compile_source(
        "use omega::language::core::float_operations;
         machine read() -> u64
         ensures FloatSemantics::add(FloatFormat::BINARY32,
             Float::meaning32(1.0f32), Float::meaning32(2.0f32))
             == Float::meaning32(3.0f32);
         { 7 }
         machine nested() -> u64
         ensures FloatSemantics::divide(FloatFormat::BINARY64,
             FloatSemantics::add(FloatFormat::BINARY64,
                 Float::meaning64(1.0f64), Float::meaning64(2.0f64)),
             Float::meaning64(2.0f64)) == Float::meaning64(1.5f64);
         { 9 }
         machine special_values() -> u64
         ensures (FloatSemantics::divide(FloatFormat::BINARY32,
             Float::meaning32(0.0f32), Float::meaning32(0.0f32))
             == FloatSemantics::divide(FloatFormat::BINARY32,
                 Float::meaning32(0.0f32), Float::meaning32(0.0f32)))
             && (FloatSemantics::negate(FloatFormat::BINARY32,
                 Float::meaning32(0.0f32)) != Float::meaning32(0.0f32));
         { 11 }",
    )
    .unwrap();
    let read = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .unwrap();
    let contract = &checked.machine_contracts(read)[0];
    let checked_trees::domain::ProofFact::Expression(expression) =
        checked.proof_facts.get(contract.facts.start())
    else {
        panic!("expression")
    };
    let equality = checked
        .facts
        .proof
        .float_meaning_equalities
        .iter()
        .find(|equality| equality.source_expression == *expression && equality.use_site.is_none())
        .unwrap();
    assert!(
        checked
            .facts
            .proof
            .float_semantic_applications
            .iter()
            .any(|application| application.result == equality.left
                || application.result == equality.right)
    );
}

#[test]
fn closed_semantic_application_refutes_false_real_core_source_contract() {
    let diagnostics = compile_source(
        "use omega::language::core::float_operations;
         machine read() -> u64
         ensures FloatSemantics::add(FloatFormat::BINARY32,
             Float::meaning32(1.0f32), Float::meaning32(2.0f32))
             == Float::meaning32(4.0f32);
         { 7 }",
    )
    .unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("disproved")),
        "{diagnostics:?}"
    );
    let diagnostics = compile_source(
        "use omega::language::core::float_operations;
         machine signed_zero() -> u64
         ensures FloatSemantics::negate(FloatFormat::BINARY32,
             Float::meaning32(0.0f32)) == Float::meaning32(0.0f32);
         { 9 }",
    )
    .unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("disproved")),
        "{diagnostics:?}"
    );
}

#[test]
fn closed_float_evaluation_does_not_override_authored_equality() {
    let diagnostics = compile_source(
        "use omega::language::core::float_operations;
         machine == FloatMeaning::custom_equal(left: FloatMeaning, right: FloatMeaning) -> bool { false }
         machine read() -> u64
         ensures FloatSemantics::add(FloatFormat::BINARY32,
             Float::meaning32(1.0f32), Float::meaning32(2.0f32))
             == Float::meaning32(3.0f32);
         { 7 }",
    ).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("read")
                && (diagnostic.message.contains("cannot prove")
                    || diagnostic.message.contains("no entailment tier"))),
        "{diagnostics:?}"
    );
}

#[test]
fn open_semantic_application_remains_unproved() {
    let diagnostics = compile_source(
        "use omega::language::core::float_operations;
         machine read(value: f32) -> u64
         ensures FloatSemantics::minimum(Float::meaning32(value),
             Float::meaning32(0.0f32)) == Float::meaning32(value);
         { 7 }",
    )
    .unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("machine `read`")
                && diagnostic.message.contains("no entailment tier")),
        "{diagnostics:?}"
    );
}

/// The FLOAT-PROVIDERS acceptance leg end-to-end: an authored non-reflexive
/// `FloatSemantics::*` ensures equality survives the owner's scalar-contract
/// lowering as an `Atom` clause citing its module equality row, the produced
/// artifact's own proof bundle discharges it under independent replay, and a
/// module whose cited equality row is forged keeps the clause but loses the
/// discharge.
#[test]
fn produced_artifact_verifies_authored_float_meaning_ensures() {
    let checked = compile_source(
        "use omega::language::core::float_operations;
         machine read() -> u64
         ensures FloatSemantics::add(FloatFormat::BINARY32,
             Float::meaning32(1.0f32), Float::meaning32(2.0f32))
             == Float::meaning32(3.0f32);
         { 7 }",
    )
    .unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .unwrap();
    let bytes = artifact.to_bytes();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [clause] = entry.contract.ensures.as_slice() else {
        panic!("the authored ensures equality lowers to one contract clause")
    };
    let semantic_vocabulary::Proposition::Atom(atom) = clause.proposition else {
        panic!("the authored equality cites its module row as an Atom")
    };
    let row = (0..module.float_meaning_equalities.len())
        .find(|index| {
            terminal_psi::float_meaning_equality_proposition_id(u32::try_from(*index).unwrap())
                == atom
        })
        .expect("the cited Atom names a module equality row");

    let bundle = terminal_codec::decode_proof_section_for(&module, artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the produced bundle discharges the authored equality");

    // Without its produced evidence the same module is rejected; the clause is
    // a real obligation, not metadata erased toward Truth.
    assert!(matches!(
        terminal_verifier::verify_module(
            &module,
            &terminal_psi::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(_))
    ));

    // A forged equality row — an operand retargeted to a literal whose meaning
    // differs — stays structurally valid but carries no provable axiom, so the
    // retained clause cannot discharge.
    let mut forged = module.clone();
    let original_left = forged.float_meaning_equalities[row].left;
    forged.float_meaning_equalities[row].left = module
        .float_meaning_projections
        .iter()
        .find(|projection| {
            matches!(
                projection.source,
                FloatMeaningSource::ExactBinary32Literal(_)
            ) && projection.result.id != original_left
        })
        .map(|projection| projection.result.id)
        .expect("a second literal projection exists");
    terminal_verifier::validate_module(&forged)
        .expect("forging the operand keeps carrier compatibility");
    assert!(
        terminal_verifier::verify_module(
            &forged,
            &bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a forged equality row must not satisfy the cited Atom obligation"
    );
}

#[test]
fn symbolic_float_applications_validate_all_operands_after_source_removal() {
    let module = source_free_module();
    terminal_verifier::verify_module(
        &module,
        &terminal_psi::ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let projections = &module.float_meaning_projections;
    let reconstructed =
        terminal_verifier::reconstruct_float_meaning_projection(projections, &projections[3])
            .unwrap();
    assert!(reconstructed.literal_meaning.is_none());
    assert_eq!(reconstructed.semantic_operation.unwrap().name, "minimum");

    // Locate the operand through two valid canonical encodings, without
    // hard-coding a module offset or bypassing validation in the encoder.
    let encoded = terminal_codec::encode_module(&module).unwrap();
    let mut alternate = module.clone();
    alternate.float_meaning_projections[2] = projection(2, minimum(0, 0));
    let alternate = terminal_codec::encode_module(&alternate).unwrap();
    assert_eq!(encoded.len(), alternate.len());
    let changed_bytes: Vec<_> = encoded
        .iter()
        .zip(&alternate)
        .enumerate()
        .filter_map(|(position, (original, alternate))| (original != alternate).then_some(position))
        .collect();
    assert_eq!(changed_bytes.len(), 1);
    let operand_position = changed_bytes[0];
    assert_eq!(encoded[operand_position], 1);
    assert_eq!(alternate[operand_position], 0);
    for invalid_reference in [2, 3, 255] {
        let mut tampered = encoded.clone();
        tampered[operand_position] = invalid_reference;
        assert_eq!(
            terminal_codec::decode_module(&tampered),
            Err(terminal_codec::CodecError::InvalidModule(
                ModuleError::InvalidFloatMeaningProjection {
                    index: 2,
                    error: Error::SemanticApplicationOperandRow { operand: 1 },
                },
            )),
        );
    }

    for (operand, expected) in [
        (
            FloatSemanticApplicationOperand::Meaning(ProofValueId(2)),
            Error::SemanticApplicationOperandRow { operand: 1 },
        ),
        (
            FloatSemanticApplicationOperand::Meaning(ProofValueId(3)),
            Error::SemanticApplicationOperandRow { operand: 1 },
        ),
        (
            FloatSemanticApplicationOperand::Meaning(ProofValueId(u32::MAX)),
            Error::SemanticApplicationOperandRow { operand: 1 },
        ),
        (
            FloatSemanticApplicationOperand::Format(IeeeFloatFormat::Binary32),
            Error::SemanticApplicationOperandKindMismatch { operand: 1 },
        ),
    ] {
        let mut tampered = module.clone();
        let FloatMeaningSource::SemanticApplication(application) =
            &mut tampered.float_meaning_projections[2].source
        else {
            panic!("application")
        };
        application.operands[1] = operand;
        let expected = ModuleError::InvalidFloatMeaningProjection {
            index: 2,
            error: expected,
        };
        assert_eq!(
            terminal_verifier::validate_module(&tampered).unwrap_err(),
            expected
        );
        assert_eq!(
            terminal_codec::encode_module(&tampered),
            Err(terminal_codec::CodecError::InvalidModule(expected))
        );
    }

    let mut tampered = module.clone();
    tampered.float_meaning_projections[1] = projection(
        1,
        FloatMeaningSource::ExactBinary64Literal(2.0_f64.to_bits()),
    );
    assert_eq!(
        terminal_verifier::validate_module(&tampered).unwrap_err(),
        ModuleError::InvalidFloatMeaningProjection {
            index: 2,
            error: Error::SemanticApplicationFormatMismatch
        },
    );
}
