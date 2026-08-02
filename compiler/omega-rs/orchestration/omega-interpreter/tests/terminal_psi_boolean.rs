//! End-to-end versioned scalar terminal-Psi canaries, including host execution.

use std::path::PathBuf;

#[cfg(unix)]
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

#[cfg(unix)]
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

#[cfg(unix)]
#[test]
fn v4_saturating_add_reaches_owned_object_image_and_native_execution() {
    let machine = MachineId::new(40).expect("machine");
    let left_operation = OperationId::new(40).expect("left operation");
    let right_operation = OperationId::new(41).expect("right operation");
    let add_operation = OperationId::new(42).expect("add operation");
    let edge = EdgeId::new(40).expect("edge");
    let left = ValueId::new(40).expect("left");
    let right = ValueId::new(41).expect("right");
    let sum = ValueId::new(42).expect("sum");
    let result = ValueId::new(43).expect("result");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V4,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(40).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(40).expect("block"),
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
                        kind: OperationKind::SaturatingIntegerAdd { left, right },
                    },
                ],
                terminator: Terminator::Return { edge, value: sum },
            }],
            contract: MachineContract {
                id: ContractId::new(40).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v4 identity");
    let canonical_bytes = encode_module(&module).expect("v4 canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v4 after producer drop");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free saturating-add module verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed saturating-add fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    assert_eq!(fixed.terminal_psi(), original_identity);
    assert_eq!(fixed.schedule(), TerminalFuelSchedule::CURRENT.identity());
    assert_eq!(fixed.ceiling_units(), 4);

    let measured = interpret_terminal_measured(&verified, &[]).expect("interpret saturating add");
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(255),
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

    let abstract_plan =
        lower_verified_module(&verified).expect("lower saturating-add requirements");
    assert!(matches!(
        abstract_plan.functions[0].operations[2],
        TerminalAbstractOperation::SaturatingIntegerAdd {
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
        .expect("select saturating-add target operation");
    let machine_code = emit_machine_code(&target_plan).expect("emit saturating-add machine code");
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
        .expect("emit standalone saturating-add image after semantic state is dropped");
    assert_eq!(image.terminal_psi(), original_identity);
    assert_eq!(image.output().final_text_bytes, artifact.text_bytes());
    assert!(image.output().compiler_text_validation.is_some());
    let installation = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(4).expect("saturating-add installation profile decision"),
        [],
    )
    .expect("saturating-add image should produce an installation record");
    validate_terminal_installation_record(&installation, &image)
        .expect("saturating-add installation record should bind its exact image");
    let installation_bytes =
        encode_terminal_installation_record(&installation).expect("installation bytes");
    assert_eq!(
        decode_terminal_installation_record(&installation_bytes),
        Ok(installation)
    );

    assert_eq!(run_host_machine_code(&entry_bytes), 255);
    #[cfg(target_os = "macos")]
    assert_eq!(run_host_executable_image(&image.output().bytes), 255);
}

#[cfg(unix)]
#[test]
fn v6_signed_i64_saturating_subtract_matches_both_bounds_natively() {
    let machine = MachineId::new(120).expect("machine");
    let operation = OperationId::new(120).expect("operation");
    let edge = EdgeId::new(120).expect("edge");
    let left = ValueId::new(120).expect("left parameter");
    let right = ValueId::new(121).expect("right parameter");
    let difference = ValueId::new(122).expect("difference");
    let result = ValueId::new(123).expect("result");
    let integer = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let scalar_type = ScalarType::Integer(integer);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V6,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: vec![
                ValueDeclaration {
                    id: left,
                    scalar_type,
                },
                ValueDeclaration {
                    id: right,
                    scalar_type,
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(120).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(120).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation,
                    result: ValueDeclaration {
                        id: difference,
                        scalar_type,
                    },
                    kind: OperationKind::SaturatingIntegerSubtract { left, right },
                }],
                terminator: Terminator::Return {
                    edge,
                    value: difference,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(120).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v6 subtraction identity");
    let canonical_bytes = encode_module(&module).expect("v6 subtraction canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v6 subtraction module");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free saturating subtraction verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed subtraction fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("subtraction fuel recomputes");
    assert_eq!(fixed.ceiling_units(), 2);

    for (left_value, right_value, expected) in [(i64::MAX, -1, i64::MAX), (i64::MIN, 1, i64::MIN)] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Signed(left_value as i128),
                },
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Signed(right_value as i128),
                },
            ],
        )
        .expect("interpret signed saturating subtraction");
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Signed(expected as i128),
            }
        );
        assert_eq!(measured.usage().total_units(), 2);
    }

    let abstract_plan =
        lower_verified_module(&verified).expect("lower saturating-subtract requirements");
    assert!(matches!(
        abstract_plan.functions[0].operations[0],
        TerminalAbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            scalar_type: operation_type,
            left: operation_left,
            right: operation_right,
            ..
        } if psi_operation == operation
            && operation_type == integer
            && operation_left == left
            && operation_right == right
    ));
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select saturating-subtract target operation");
    let machine_code =
        emit_machine_code(&target_plan).expect("emit saturating-subtract machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build saturating-subtract object artifact");
    assert_eq!(artifact.entry_function().provenance.operations, [operation]);
    assert_eq!(artifact.entry_function().provenance.edges, [edge]);
    let entry_bytes = artifact.entry_function().bytes(&artifact).to_vec();

    drop(machine_code);
    drop(target_plan);
    drop(abstract_plan);
    drop(verified);
    drop(module);

    assert_eq!(run_host_i64_saturating_subtract_canary(&entry_bytes), 0);
}

#[cfg(unix)]
#[test]
fn v5_wrapping_subtract_matches_interpretation_and_native_execution() {
    let machine = MachineId::new(100).expect("machine");
    let operation = OperationId::new(100).expect("operation");
    let edge = EdgeId::new(100).expect("edge");
    let left = ValueId::new(100).expect("left parameter");
    let right = ValueId::new(101).expect("right parameter");
    let difference = ValueId::new(102).expect("difference");
    let result = ValueId::new(103).expect("result");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V5,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: vec![
                ValueDeclaration {
                    id: left,
                    scalar_type,
                },
                ValueDeclaration {
                    id: right,
                    scalar_type,
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(100).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(100).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation,
                    result: ValueDeclaration {
                        id: difference,
                        scalar_type,
                    },
                    kind: OperationKind::WrappingIntegerSubtract { left, right },
                }],
                terminator: Terminator::Return {
                    edge,
                    value: difference,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(100).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v5 subtraction identity");
    let canonical_bytes = encode_module(&module).expect("v5 subtraction canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v5 subtraction module");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free wrapping subtraction verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed subtraction fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("subtraction fuel recomputes");
    assert_eq!(fixed.ceiling_units(), 2);

    let arguments = [
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(5),
        },
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(10),
        },
    ];
    let measured =
        interpret_terminal_measured(&verified, &arguments).expect("interpret wrapping subtract");
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(251),
        }
    );
    assert_eq!(measured.usage().total_units(), 2);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Operation(operation))
            .unwrap()
            .units(),
        1
    );

    let abstract_plan =
        lower_verified_module(&verified).expect("lower wrapping-subtract requirements");
    assert!(matches!(
        abstract_plan.functions[0].operations[0],
        TerminalAbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            scalar_type: operation_type,
            left: operation_left,
            right: operation_right,
            ..
        } if psi_operation == operation
            && operation_type == integer
            && operation_left == left
            && operation_right == right
    ));
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select wrapping-subtract target operation");
    let machine_code =
        emit_machine_code(&target_plan).expect("emit wrapping-subtract machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build wrapping-subtract object artifact");
    assert_eq!(artifact.entry_function().provenance.operations, [operation]);
    assert_eq!(artifact.entry_function().provenance.edges, [edge]);
    let entry_bytes = artifact.entry_function().bytes(&artifact).to_vec();

    drop(machine_code);
    drop(target_plan);
    drop(abstract_plan);
    drop(verified);
    drop(module);

    assert_eq!(
        run_host_machine_code_with_u8_arguments(&entry_bytes, &[5, 10]),
        251
    );
}

#[cfg(unix)]
#[test]
fn v7_wrapping_multiply_matches_interpretation_and_native_execution() {
    let machine = MachineId::new(110).expect("machine");
    let operation = OperationId::new(110).expect("operation");
    let edge = EdgeId::new(110).expect("edge");
    let left = ValueId::new(110).expect("left parameter");
    let right = ValueId::new(111).expect("right parameter");
    let product = ValueId::new(112).expect("product");
    let result = ValueId::new(113).expect("result");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V7,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: vec![
                ValueDeclaration {
                    id: left,
                    scalar_type,
                },
                ValueDeclaration {
                    id: right,
                    scalar_type,
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(110).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(110).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation,
                    result: ValueDeclaration {
                        id: product,
                        scalar_type,
                    },
                    kind: OperationKind::WrappingIntegerMultiply { left, right },
                }],
                terminator: Terminator::Return {
                    edge,
                    value: product,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(110).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v7 multiplication identity");
    let canonical_bytes = encode_module(&module).expect("v7 multiplication canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v7 multiplication module");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free wrapping multiplication verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed multiplication fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("multiplication fuel recomputes");
    assert_eq!(fixed.ceiling_units(), 2);

    let arguments = [
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(20),
        },
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(13),
        },
    ];
    let measured =
        interpret_terminal_measured(&verified, &arguments).expect("interpret wrapping multiply");
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(4),
        }
    );
    assert_eq!(measured.usage().total_units(), 2);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Operation(operation))
            .unwrap()
            .units(),
        1
    );

    let abstract_plan =
        lower_verified_module(&verified).expect("lower wrapping-multiply requirements");
    assert!(matches!(
        abstract_plan.functions[0].operations[0],
        TerminalAbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            scalar_type: operation_type,
            left: operation_left,
            right: operation_right,
            ..
        } if psi_operation == operation
            && operation_type == integer
            && operation_left == left
            && operation_right == right
    ));
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select wrapping-multiply target operation");
    let machine_code =
        emit_machine_code(&target_plan).expect("emit wrapping-multiply machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build wrapping-multiply object artifact");
    assert_eq!(artifact.entry_function().provenance.operations, [operation]);
    assert_eq!(artifact.entry_function().provenance.edges, [edge]);
    let entry_bytes = artifact.entry_function().bytes(&artifact).to_vec();

    drop(machine_code);
    drop(target_plan);
    drop(abstract_plan);
    drop(verified);
    drop(module);

    assert_eq!(
        run_host_machine_code_with_u8_arguments(&entry_bytes, &[20, 13]),
        4
    );
}

#[cfg(unix)]
#[test]
fn v4_nested_runtime_arithmetic_uses_register_and_stack_parameters_natively() {
    let machine = MachineId::new(60).expect("machine");
    let wrapping_operation = OperationId::new(60).expect("wrapping operation");
    let saturating_operation = OperationId::new(61).expect("saturating operation");
    let edge = EdgeId::new(60).expect("edge");
    let wrapped = ValueId::new(70).expect("wrapped value");
    let saturated = ValueId::new(71).expect("saturated value");
    let result = ValueId::new(72).expect("result");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let parameters = (0..9)
        .map(|index| ValueDeclaration {
            id: ValueId::new(60 + index).expect("parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let module = TerminalModule {
        semantic_version: SemanticVersion::V4,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: parameters.clone(),
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(60).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(60).expect("block"),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: wrapping_operation,
                        result: ValueDeclaration {
                            id: wrapped,
                            scalar_type,
                        },
                        kind: OperationKind::WrappingIntegerAdd {
                            left: parameters[0].id,
                            right: parameters[8].id,
                        },
                    },
                    Operation {
                        id: saturating_operation,
                        result: ValueDeclaration {
                            id: saturated,
                            scalar_type,
                        },
                        kind: OperationKind::SaturatingIntegerAdd {
                            left: wrapped,
                            right: parameters[1].id,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    edge,
                    value: saturated,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(60).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v4 runtime identity");
    let canonical_bytes = encode_module(&module).expect("v4 runtime canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v4 runtime module");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free runtime arithmetic module verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed runtime arithmetic fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("runtime arithmetic fuel recomputes");
    assert_eq!(fixed.ceiling_units(), 3);

    let argument_values: [u8; 9] = [250, 253, 0, 0, 0, 0, 0, 0, 10];
    let arguments = argument_values
        .into_iter()
        .map(|value| TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(u128::from(value)),
        })
        .collect::<Vec<_>>();
    let measured =
        interpret_terminal_measured(&verified, &arguments).expect("interpret runtime arithmetic");
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(255),
        }
    );
    assert_eq!(measured.usage().total_units(), 3);

    let abstract_plan =
        lower_verified_module(&verified).expect("lower runtime arithmetic requirements");
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select runtime arithmetic target operations");
    let machine_code =
        emit_machine_code(&target_plan).expect("emit nested runtime arithmetic machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build nested runtime arithmetic object artifact");
    assert_eq!(
        artifact.entry_function().provenance.operations,
        [wrapping_operation, saturating_operation]
    );
    assert_eq!(artifact.entry_function().provenance.edges, [edge]);
    let entry_bytes = artifact.entry_function().bytes(&artifact).to_vec();

    drop(machine_code);
    drop(target_plan);
    drop(abstract_plan);
    drop(verified);
    drop(module);

    assert_eq!(
        run_host_machine_code_with_u8_arguments(&entry_bytes, &argument_values),
        255
    );
}

#[cfg(unix)]
#[test]
fn v4_signed_i64_runtime_saturation_matches_both_bounds_natively() {
    let machine = MachineId::new(80).expect("machine");
    let operation = OperationId::new(80).expect("operation");
    let edge = EdgeId::new(80).expect("edge");
    let left = ValueId::new(80).expect("left parameter");
    let right = ValueId::new(81).expect("right parameter");
    let sum = ValueId::new(82).expect("sum");
    let result = ValueId::new(83).expect("result");
    let integer = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let scalar_type = ScalarType::Integer(integer);
    let module = TerminalModule {
        semantic_version: SemanticVersion::V4,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters: vec![
                ValueDeclaration {
                    id: left,
                    scalar_type,
                },
                ValueDeclaration {
                    id: right,
                    scalar_type,
                },
            ],
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(80).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(80).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation,
                    result: ValueDeclaration {
                        id: sum,
                        scalar_type,
                    },
                    kind: OperationKind::SaturatingIntegerAdd { left, right },
                }],
                terminator: Terminator::Return { edge, value: sum },
            }],
            contract: MachineContract {
                id: ContractId::new(80).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free signed runtime saturation verifies");
    for (left_value, right_value, expected) in [(i64::MAX, 1, i64::MAX), (i64::MIN, -1, i64::MIN)] {
        let measured = interpret_terminal_measured(
            &verified,
            &[
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Signed(left_value as i128),
                },
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Signed(right_value as i128),
                },
            ],
        )
        .expect("interpret signed runtime saturation");
        assert_eq!(
            measured.value(),
            TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Signed(expected as i128),
            }
        );
        assert_eq!(measured.usage().total_units(), 2);
    }

    let abstract_plan =
        lower_verified_module(&verified).expect("lower signed runtime saturation requirements");
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select signed runtime saturation target operations");
    let machine_code =
        emit_machine_code(&target_plan).expect("emit signed runtime saturation machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build signed runtime saturation object artifact");
    assert_eq!(artifact.entry_function().provenance.operations, [operation]);
    assert_eq!(artifact.entry_function().provenance.edges, [edge]);
    let entry_bytes = artifact.entry_function().bytes(&artifact).to_vec();

    drop(machine_code);
    drop(target_plan);
    drop(abstract_plan);
    drop(verified);
    drop(module);

    assert_eq!(run_host_i64_saturation_canary(&entry_bytes), 0);
}

#[cfg(unix)]
#[test]
fn v1_runtime_stack_parameter_matches_interpretation_and_native_execution() {
    let machine = MachineId::new(50).expect("machine");
    let edge = EdgeId::new(50).expect("edge");
    let result = ValueId::new(59).expect("result");
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let parameters = (0..9)
        .map(|index| ValueDeclaration {
            id: ValueId::new(50 + index).expect("parameter"),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let returned = parameters[8].id;
    let module = TerminalModule {
        semantic_version: SemanticVersion::V1,
        entry: machine,
        machines: vec![TerminalMachine {
            id: machine,
            parameters,
            result: ValueDeclaration {
                id: result,
                scalar_type,
            },
            entry: BlockId::new(50).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(50).expect("block"),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge,
                    value: returned,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(50).expect("contract"),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let original_identity = terminal_psi_identity(&module).expect("v1 identity");
    let canonical_bytes = encode_module(&module).expect("v1 canonical bytes");
    drop(module);
    let module = decode_module(&canonical_bytes).expect("decode v1 after producer drop");
    assert_eq!(terminal_psi_identity(&module).unwrap(), original_identity);

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("proof-free parameter module verifies");
    let fixed = derive_fixed_entry_fuel(&verified, machine).expect("fixed parameter fuel");
    validate_fixed_entry_fuel(&verified, &fixed).expect("fixed fuel recomputes");
    assert_eq!(fixed.terminal_psi(), original_identity);
    assert_eq!(fixed.ceiling_units(), 1);

    let mut arguments = (1..=8)
        .map(|value| TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(value),
        })
        .collect::<Vec<_>>();
    arguments.push(TerminalScalarValue::Integer {
        scalar_type: integer,
        value: IntegerValue::Unsigned(77),
    });
    let measured =
        interpret_terminal_measured(&verified, &arguments).expect("interpret parameter return");
    assert_eq!(
        measured.value(),
        TerminalScalarValue::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(77),
        }
    );
    assert_eq!(measured.usage().total_units(), 1);
    assert_eq!(
        measured
            .usage()
            .at(FuelChargeSite::Edge(edge))
            .unwrap()
            .units(),
        1
    );

    let abstract_plan = lower_verified_module(&verified).expect("lower parameter requirements");
    assert_eq!(abstract_plan.functions[0].parameters.len(), 9);
    assert_eq!(abstract_plan.functions[0].parameters[8].value, returned);
    assert!(matches!(
        abstract_plan.functions[0].operations[0],
        TerminalAbstractOperation::Return { value, .. } if value == returned
    ));
    let target_plan = lower_to_target_operations(&abstract_plan, NativeTarget::host())
        .expect("select host parameter ABI location");
    let machine_code = emit_machine_code(&target_plan).expect("emit parameter-return machine code");
    let artifact = build_terminal_object_artifact(&machine_code)
        .expect("build owned parameter-return object artifact");
    assert!(artifact.entry_function().provenance.operations.is_empty());
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
    assert_eq!(
        run_host_machine_code_with_u8_arguments(&entry_bytes, &[1, 2, 3, 4, 5, 6, 7, 8, 77]),
        77
    );
}

#[cfg(target_os = "macos")]
fn run_host_executable_image(bytes: &[u8]) -> i32 {
    use std::os::unix::fs::PermissionsExt;

    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-scalar-image-{}-{nonce}-{sequence}",
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
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-scalar-native-{}-{nonce}-{sequence}",
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
fn run_host_machine_code_with_u8_arguments(bytes: &[u8], arguments: &[u8]) -> i32 {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-parameter-native-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create parameter native test directory");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let harness_path = directory.join("harness.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let symbol = if cfg!(target_os = "macos") {
        "_omega_entry"
    } else {
        "omega_entry"
    };
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl {symbol}\n.p2align 2\n{symbol}:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl {symbol}\n.type {symbol},@function\n{symbol}:\n.byte {bytes}\n.size {symbol}, .-{symbol}\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let parameter_types = std::iter::repeat_n("unsigned char", arguments.len())
        .collect::<Vec<_>>()
        .join(", ");
    let argument_values = arguments
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let harness = format!(
        "extern unsigned char omega_entry({parameter_types});\nint main(void) {{ return omega_entry({argument_values}); }}\n"
    );
    std::fs::write(&assembly_path, assembly).expect("write parameter native assembly");
    std::fs::write(&harness_path, harness).expect("write parameter native harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&harness_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected parameter terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute parameter terminal native canary")
        .code()
        .expect("parameter terminal native canary exited normally")
}

#[cfg(unix)]
fn run_host_i64_saturation_canary(bytes: &[u8]) -> i32 {
    let harness = "#include <limits.h>\n\
extern long long omega_entry(long long, long long);\n\
int main(void) {\n\
  if (omega_entry(LLONG_MAX, 1) != LLONG_MAX) return 1;\n\
  if (omega_entry(LLONG_MIN, -1) != LLONG_MIN) return 2;\n\
  return 0;\n\
}\n";
    run_host_i64_binary_canary(bytes, "saturation", harness)
}

#[cfg(unix)]
fn run_host_i64_saturating_subtract_canary(bytes: &[u8]) -> i32 {
    let harness = "#include <limits.h>\n\
extern long long omega_entry(long long, long long);\n\
int main(void) {\n\
  if (omega_entry(LLONG_MAX, -1) != LLONG_MAX) return 1;\n\
  if (omega_entry(LLONG_MIN, 1) != LLONG_MIN) return 2;\n\
  return 0;\n\
}\n";
    run_host_i64_binary_canary(bytes, "saturating-subtract", harness)
}

#[cfg(unix)]
fn run_host_i64_binary_canary(bytes: &[u8], label: &str, harness: &str) -> i32 {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "omega-terminal-i64-{label}-native-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create i64 saturation native test directory");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let harness_path = directory.join("harness.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let symbol = if cfg!(target_os = "macos") {
        "_omega_entry"
    } else {
        "omega_entry"
    };
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl {symbol}\n.p2align 2\n{symbol}:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl {symbol}\n.type {symbol},@function\n{symbol}:\n.byte {bytes}\n.size {symbol}, .-{symbol}\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    std::fs::write(&assembly_path, assembly).expect("write i64 saturation native assembly");
    std::fs::write(&harness_path, harness).expect("write i64 saturation native harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&harness_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected i64 saturation terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute i64 saturation terminal native canary")
        .code()
        .expect("i64 saturation terminal native canary exited normally")
}

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
