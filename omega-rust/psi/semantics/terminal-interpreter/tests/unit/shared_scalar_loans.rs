//! A verified primitive-scalar shared borrow executes end-to-end: the entry
//! block loans a record's scalar leaf into a join whose `&u64` block
//! parameter binds the exact leaf view, the view forwards whole to a callee's
//! shared formal, and the callee's `PrimitiveScalarRead` observes the record
//! field's contents from `structural_scalar_fields` — never primitive root
//! storage, which only backs whole primitive referents and locals.

use super::{
    AcceptTerminalEffects, AdmissionProfile, BindingRelevance, Block, IntegerSign, IntegerType,
    IntegerValue, Operation, OperationKind, OperationResult, ProofBundle, ScalarType,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalInterpretError,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalScalarValue,
    TerminalStructuralValue, Terminator, ValueDeclaration, block_id, contract_id, decode_module,
    edge_id, empty_contract, encode_module, encode_proof_section, machine_id, operation_id,
    place_id, structural_field_id, structural_type_id, unit_module, value_id, verify_module,
};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_interpreter::{TerminalStructuralInputs, TerminalStructuralScalarFieldValue};

const PAYLOAD_TYPE: u64 = 1;
const SCALAR_TYPE: u64 = 2;
const ROOT: u64 = 1;
const VIEW: u64 = 2;
const CALL_VALUE: u64 = 3;
const READ_VALUE: u64 = 4;
const RESULT_DECL: u64 = 5;
const CALLEE_RESULT_DECL: u64 = 6;
const CALLEE_ROOT: u64 = 10;
const JOIN_BLOCK: u64 = 901;
const CALLEE: u64 = 902;
const CALLEE_BLOCK: u64 = 910;
const OPAQUE_IDENTITY: u64 = 7;
const LOANED: u64 = 42;

fn u64_scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn scalar_declaration(ordinal: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(ordinal),
        scalar_type: u64_scalar(),
    }
}

fn field(raw: u64, identity: &str) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: structural_field_id(raw),
        identity: identity.to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(u64_scalar()),
    }
}

fn shared_parameter(
    raw: u64,
    position: u32,
    structural_type: u64,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place_id(raw),
        position,
        is_self: false,
        structural_type: structural_type_id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn loan_argument() -> StructuralArgument {
    StructuralArgument {
        place: place_id(ROOT),
        path: vec![StructuralPathSegment::Field("value".to_owned())],
        access: StructuralAccess::SharedBorrow,
    }
}

/// The verified module from the shared-scalar-loan admission tests: an owned
/// `Payload` entry parameter loans its `value` scalar leaf into `JOIN_BLOCK`,
/// whose `&u64` parameter forwards whole to `read`'s shared formal. `read`
/// observes the leaf through an empty-path `PrimitiveScalarRead`.
fn shared_scalar_call_module() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(PAYLOAD_TYPE),
            identity: "test::Payload".to_owned(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "value"),
                    StructuralFieldDeclaration {
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        ..field(2, "flag")
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(SCALAR_TYPE),
            identity: "test::Scalar".to_owned(),
            shape: StructuralTypeShape::PrimitiveScalar(u64_scalar()),
        },
    ];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(ROOT),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(PAYLOAD_TYPE),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(VIEW),
            kind: StructuralPlaceKind::BlockParameter {
                block: block_id(JOIN_BLOCK),
                position: 0,
            },
        },
    ];
    machine.result = TerminalMachineResult::Scalar(scalar_declaration(RESULT_DECL));
    machine.blocks[0].terminator = Terminator::Jump {
        structural_arguments: vec![loan_argument()],
        edge: edge_id(901),
        target: block_id(JOIN_BLOCK),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: vec![shared_parameter(VIEW, 0, SCALAR_TYPE)],
        id: block_id(JOIN_BLOCK),
        parameters: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            id: operation_id(902),
            result: OperationResult::Scalar(scalar_declaration(CALL_VALUE)),
            kind: OperationKind::CallStructuralScalar {
                callee: machine_id(CALLEE),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(VIEW),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }],
        terminator: Terminator::Return {
            edge: edge_id(903),
            value: value_id(CALL_VALUE),
            cleanup_actions: Vec::new(),
        },
    });
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(CALLEE),
        attachment: None,
        structural_parameters: vec![shared_parameter(CALLEE_ROOT, 0, SCALAR_TYPE)],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(scalar_declaration(CALLEE_RESULT_DECL)),
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(CALLEE_ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(CALLEE_BLOCK),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(CALLEE_BLOCK),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation_id(910),
                result: OperationResult::Scalar(scalar_declaration(READ_VALUE)),
                kind: OperationKind::PrimitiveScalarRead {
                    source: place_id(CALLEE_ROOT),
                    path: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(911),
                value: value_id(READ_VALUE),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(911)),
    });
    module
}

fn start(
    bytes: &[u8],
    proof: &[u8],
    scalar_fields: &[TerminalStructuralScalarFieldValue],
) -> TerminalExecution {
    TerminalExecution::start_artifact(
        bytes,
        proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: OPAQUE_IDENTITY,
                structural_type: structural_type_id(PAYLOAD_TYPE),
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            scalar_fields,
            ..Default::default()
        },
    )
    .expect("the shared scalar loan starts")
}

#[test]
fn primitive_shared_loan_executes_the_leaf_read() {
    let module = shared_scalar_call_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the joined scalar view reaches the callee's shared formal");
    let bytes = encode_module(&module).expect("shared-loan module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let scalar_fields = [TerminalStructuralScalarFieldValue {
        argument_index: 0,
        path: Vec::new(),
        field: structural_field_id(1),
        value: TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(u128::from(LOANED)),
        },
    }];
    let expected = TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(u128::from(LOANED)),
        },
    ));
    let mut execution = start(&bytes, &proof, &scalar_fields);
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        expected,
        "the callee's read observes the record leaf through the shared loan"
    );
    let total = meter.usage().total_units();
    // Every suspension point replays the same verified view binding and the
    // same leaf read; the scalar is observed exactly once per completion.
    for split in 0..total {
        let mut execution = start(&bytes, &proof, &scalar_fields);
        let mut meter = TerminalFuelMeter::with_allowance(split);
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - split).unwrap();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            expected,
            "split {split}"
        );
    }
}

fn jump_argument(module: &mut TerminalModule) -> &mut StructuralArgument {
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    &mut structural_arguments[0]
}

#[test]
fn primitive_shared_loan_rejects_mutated_access_custody_or_leaf() {
    for mutation in 0..18 {
        let mut module = shared_scalar_call_module();
        match mutation {
            // No access mode other than SharedBorrow satisfies the loan.
            0 => jump_argument(&mut module).access = StructuralAccess::Owned,
            1 => jump_argument(&mut module).access = StructuralAccess::MutableBorrow,
            2 => jump_argument(&mut module).access = StructuralAccess::WriteOnlyBorrow,
            // The leaf projection must resolve exactly: the whole record is
            // the wrong type, an unknown identity has no field, and a leaf of
            // another scalar type resolves to a different canonical shape.
            3 => jump_argument(&mut module).path = Vec::new(),
            4 => {
                jump_argument(&mut module).path =
                    vec![StructuralPathSegment::Field("missing".to_owned())]
            }
            5 => {
                jump_argument(&mut module).path =
                    vec![StructuralPathSegment::Field("flag".to_owned())]
            }
            // An unknown place or the not-yet-bound parameter cannot supply.
            6 => jump_argument(&mut module).place = place_id(99),
            7 => jump_argument(&mut module).place = place_id(VIEW),
            // The block parameter admits no other custody or access class.
            8 => {
                module.machines[0].blocks[1].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Affine
            }
            9 => {
                module.machines[0].blocks[1].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Linear
            }
            10 => {
                module.machines[0].blocks[1].structural_parameters[0].access =
                    StructuralAccess::Owned
            }
            11 => {
                module.machines[0].blocks[1].structural_parameters[0].access =
                    StructuralAccess::MutableBorrow
            }
            12 => {
                module.machines[0].blocks[1].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }
            // The declared referent type is the canonical leaf, not the root.
            13 => {
                module.machines[0].blocks[1].structural_parameters[0].structural_type =
                    structural_type_id(PAYLOAD_TYPE)
            }
            // The callee's formal is itself a shared borrow.
            14 => module.machines[1].structural_parameters[0].access = StructuralAccess::Owned,
            15 => {
                module.machines[1].structural_parameters[0].access = StructuralAccess::MutableBorrow
            }
            // The read cannot project past the primitive carrier, and it
            // cannot name a place the callee never bound.
            16 => {
                module.machines[1].blocks[0].operations[0].kind =
                    OperationKind::PrimitiveScalarRead {
                        source: place_id(CALLEE_ROOT),
                        path: vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                            structural_field_id(1),
                        )],
                    }
            }
            17 => {
                module.machines[1].blocks[0].operations[0].kind =
                    OperationKind::PrimitiveScalarRead {
                        source: place_id(VIEW),
                        path: Vec::new(),
                    }
            }
            _ => unreachable!(),
        }
        assert!(
            encode_module(&module).is_err(),
            "mutation {mutation} must stay rejected"
        );
    }
}

#[test]
fn primitive_shared_loan_read_requires_the_leaf_contents() {
    let module = shared_scalar_call_module();
    let bytes = encode_module(&module).expect("shared-loan module encodes");
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    // The scalar leaf has no host backing: the callee's read observes a
    // missing `structural_scalar_fields` entry, not absent primitive storage.
    let mut execution = start(&bytes, &proof, &[]);
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution.resume(&mut meter, &mut AcceptTerminalEffects),
        Err(TerminalInterpretError::StructuralScalarFieldMissing {
            source: place_id(CALLEE_ROOT),
            field: structural_field_id(1),
        }),
        "a shared leaf view never falls back to primitive root storage"
    );
}
