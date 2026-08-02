//! End-to-end versioned scalar terminal-Psi canaries, including host execution.

use std::path::PathBuf;

#[cfg(unix)]
use std::{process::Command, time::SystemTime};

use omega_interpreter::{TerminalScalarValue, interpret_terminal_measured};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::TerminalAbstractOperation;
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_image_emission::{
    build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, emit_terminal_executable_image,
    emit_terminal_object_container, encode_terminal_installation_record,
    validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_verified_module;
use psi_core::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ProfileDecisionId, ScalarType, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, SemanticVersion, TerminalMachine,
    TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_codec::{decode_module, encode_module, terminal_psi_identity};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_fuel::{FuelChargeSite, TerminalFuelSchedule};
use psi_terminal_verifier::{ProofBundle, verify_module};

#[cfg(unix)]
#[test]
fn v2_boolean_reaches_owned_object_image_and_native_execution() {
    let machine = MachineId::new(20).expect("machine");
    let operation = OperationId::new(20).expect("operation");
    let edge = EdgeId::new(20).expect("edge");
    let constant = ValueId::new(20).expect("constant");
    let result = ValueId::new(21).expect("result");
    let module = TerminalModule {
        semantic_version: SemanticVersion::V2,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Boolean,
            },
            entry: BlockId::new(20).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(20).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation,
                    result: ValueDeclaration {
                        id: constant,
                        scalar_type: ScalarType::Boolean,
                    },
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Return {
                    edge,
                    value: constant,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(20).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v2 identity");
    let canonical_bytes = encode_module(&module).expect("v2 canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v2 after producer drop");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free Boolean module verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed Boolean fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    assert_eq!(fixed.terminal_psi(), original_identity);
    assert_eq!(fixed.schedule(), TerminalFuelSchedule::CURRENT.identity());
    assert_eq!(fixed.ceiling_units(), 2);

    let measured = interpret_terminal_measured(&verified, &[]).expect("interpret Boolean");
    assert_eq!(measured.value(), TerminalScalarValue::Boolean(true));
    assert_eq!(measured.usage().total_units(), 2);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Operation(operation))
            .unwrap()
            .units(),
        1
    );

    let abstract_plan = lower_verified_module(&verified).expect("lower Boolean requirements");
    assert!(matches!(
        abstract_plan.functions[0].operations[0],
        TerminalAbstractOperation::BooleanConstant { value: true, .. }
    ));
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select Boolean target operation");
    let machine_code = emit_machine_code(&target_plan).expect("emit Boolean machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build owned terminal object artifact");
    assert_eq!(artifact.entry_function().provenance.operations, [operation]);
    assert_eq!(artifact.entry_function().provenance.edges, [edge]);
    let entry_bytes = artifact.entry_function().bytes(&artifact).to_vec();

    drop(machine_code);
    drop(target_plan);
    drop(abstract_plan);
    drop(verified);
    drop(module);

    let object = emit_terminal_object_container(&artifact);
    assert_eq!(object.terminal_psi, original_identity);
    assert_eq!(&object.output.bytes[..8], b"OMGOBJ\0\0");
    assert_eq!(object.output.text_bytes, artifact.text_bytes().len());
    assert_eq!(object.output.relocations, 0);
    let image = emit_terminal_executable_image(&artifact, 3)
        .expect("emit exact standalone host image after semantic state is dropped");
    assert_eq!(image.terminal_psi(), original_identity);
    assert_eq!(image.output().final_text_bytes, artifact.text_bytes());
    assert!(
        image
            .output()
            .executable_regions
            .unclassified_gaps
            .is_empty()
    );
    assert_eq!(image.output().executable_regions.regions.len(), 1);
    assert!(image.output().compiler_text_validation.is_some());
    let installation = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(2).expect("Boolean installation profile decision"),
        [],
    )
    .expect("Boolean image should produce an installation record");
    validate_terminal_installation_record(&installation, &image)
        .expect("Boolean installation record should bind its exact image");
    let installation_bytes =
        encode_terminal_installation_record(&installation).expect("installation bytes");
    assert_eq!(
        decode_terminal_installation_record(&installation_bytes),
        Ok(installation)
    );

    assert_eq!(run_host_machine_code(&entry_bytes), 1);
    #[cfg(target_os = "macos")]
    assert_eq!(run_host_executable_image(&image.output().bytes), 1);
}

#[cfg(unix)]
#[test]
fn v3_wrapping_add_reaches_owned_object_image_and_native_execution() {
    let machine = MachineId::new(30).expect("machine");
    let left_operation = OperationId::new(30).expect("left operation");
    let right_operation = OperationId::new(31).expect("right operation");
    let add_operation = OperationId::new(32).expect("add operation");
    let edge = EdgeId::new(30).expect("edge");
    let left = ValueId::new(30).expect("left");
    let right = ValueId::new(31).expect("right");
    let sum = ValueId::new(32).expect("sum");
    let result = ValueId::new(33).expect("result");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V3,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(30).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(30).expect("block"),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: left_operation,
                        result: ValueDeclaration {
                            id: left,
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(200),
                        },
                    },
                    Operation {
                        id: right_operation,
                        result: ValueDeclaration {
                            id: right,
                            scalar_type,
                        },
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(100),
                        },
                    },
                    Operation {
                        id: add_operation,
                        result: ValueDeclaration {
                            id: sum,
                            scalar_type,
                        },
                        kind: OperationKind::WrappingIntegerAdd { left, right },
                    },
                ],
                terminator: Terminator::Return { edge, value: sum },
            }],
            contract: MachineContract {
                id: ContractId::new(30).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v3 identity");
    let canonical_bytes = encode_module(&module).expect("v3 canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v3 after producer drop");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free wrapping-add module verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed wrapping-add fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    assert_eq!(fixed.terminal_psi(), original_identity);
    assert_eq!(fixed.schedule(), TerminalFuelSchedule::CURRENT.identity());
    assert_eq!(fixed.ceiling_units(), 4);

    let measured = interpret_terminal_measured(&verified, &[]).expect("interpret wrapping add");
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(44),
        }
    );
    assert_eq!(measured.usage().total_units(), 4);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Operation(add_operation))
            .unwrap()
            .units(),
        1
    );

    let abstract_plan = lower_verified_module(&verified).expect("lower wrapping-add requirements");
    assert!(matches!(
        abstract_plan.functions[0].operations[2],
        TerminalAbstractOperation::WrappingIntegerAdd {
            psi_operation,
            scalar_type: operation_type,
            left: operation_left,
            right: operation_right,
            ..
        } if psi_operation == add_operation
            && operation_type == integer
            && operation_left == left
            && operation_right == right
    ));
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select wrapping-add target operation");
    let machine_code = emit_machine_code(&target_plan).expect("emit wrapping-add machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build owned terminal object artifact");
    assert_eq!(
        artifact.entry_function().provenance.operations,
        [left_operation, right_operation, add_operation]
    );
    assert_eq!(artifact.entry_function().provenance.edges, [edge]);
    let entry_bytes = artifact.entry_function().bytes(&artifact).to_vec();

    drop(machine_code);
    drop(target_plan);
    drop(abstract_plan);
    drop(verified);
    drop(module);

    let object = emit_terminal_object_container(&artifact);
    assert_eq!(object.terminal_psi, original_identity);
    assert_eq!(&object.output.bytes[..8], b"OMGOBJ\0\0");
    let image = emit_terminal_executable_image(&artifact, 3)
        .expect("emit standalone host image after semantic state is dropped");
    assert_eq!(image.terminal_psi(), original_identity);
    assert_eq!(image.output().final_text_bytes, artifact.text_bytes());
    assert!(image.output().compiler_text_validation.is_some());
    let installation = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(3).expect("wrapping-add installation profile decision"),
        [],
    )
    .expect("wrapping-add image should produce an installation record");
    validate_terminal_installation_record(&installation, &image)
        .expect("wrapping-add installation record should bind its exact image");
    let installation_bytes =
        encode_terminal_installation_record(&installation).expect("installation bytes");
    assert_eq!(
        decode_terminal_installation_record(&installation_bytes),
        Ok(installation)
    );

    assert_eq!(run_host_machine_code(&entry_bytes), 44);
    #[cfg(target_os = "macos")]
    assert_eq!(run_host_executable_image(&image.output().bytes), 44);
}

#[cfg(target_os = "macos")]
fn run_host_executable_image(bytes: &[u8]) -> i32 {
    use std::os::unix::fs::PermissionsExt;

    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-scalar-image-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create image test directory");
    let _cleanup = ScratchDirectory(directory.clone());
    let executable_path = directory.join("omega-program");
    std::fs::write(&executable_path, bytes).expect("write direct terminal image");
    let mut permissions = std::fs::metadata(&executable_path)
        .expect("terminal image metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable_path, permissions)
        .expect("mark terminal image executable");
    Command::new(&executable_path)
        .status()
        .expect("execute direct terminal image")
        .code()
        .expect("direct terminal image exited normally")
}

#[cfg(unix)]
fn run_host_machine_code(bytes: &[u8]) -> i32 {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-scalar-native-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create native test directory");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _main\n.p2align 2\n_main:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl main\n.type main,@function\nmain:\n.byte {bytes}\n.size main, .-main\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    std::fs::write(&assembly_path, assembly).expect("write native linker harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal native canary")
        .code()
        .expect("terminal native canary exited normally")
}

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
