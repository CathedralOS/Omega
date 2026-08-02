//! End-to-end v2 Boolean terminal-Psi canary, including host execution.

use std::path::PathBuf;

#[cfg(unix)]
use std::{process::Command, time::SystemTime};

use omega_interpreter::{TerminalScalarValue, interpret_terminal_measured};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::TerminalAbstractOperation;
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_verified_module;
use psi_core::{BlockId, ContractId, EdgeId, MachineId, OperationId, ScalarType, ValueId};
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
fn v2_boolean_round_trips_verifies_meters_lowers_and_executes_natively() {
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
    let entry = machine_code
        .functions
        .iter()
        .find(|function| function.machine == machine_code.entry)
        .expect("entry function");
    assert_eq!(entry.provenance.operations, [operation]);
    assert_eq!(entry.provenance.edges, [edge]);
    assert_eq!(run_host_machine_code(&entry.bytes), 1);
}

#[cfg(unix)]
fn run_host_machine_code(bytes: &[u8]) -> i32 {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-boolean-native-{}-{nonce}",
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
        "host linker rejected Boolean machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute Boolean native canary")
        .code()
        .expect("Boolean native canary exited normally")
}

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
