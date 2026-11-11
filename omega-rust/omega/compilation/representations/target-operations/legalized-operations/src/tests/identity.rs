// Plan identity drift tests for scalar graphs, structural contracts, ABI layouts, ownership
// frontier facts, byte-sequence literals, and primitive/field stores.
//
// Every field a forged placement, ownership frontier fact, literal byte sequence, or store
// destination could carry reaches `LegalizedOperationPlanIdentity`. In particular:
// - The ABI layout is reconstructible from the declaration and selected calling policy, and any
//   substituted placement field drifts the digest.
// - Retained `ownership_frontier_facts` bind by both presence, ordering, and canonical fact
//   identity.
// - Runtime element index selectors, strides, and bounds certificates participate in store
//   identities, and owned block arrivals, loan parameters, and owned operation results produce
//   distinct store encodings.

use super::super::{
    LegalizedOperationPlanIdentity, LegalizedScalarFunction, LegalizedScalarParameter,
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarTerminator,
    legalized_operation_plan_identity,
};
use super::{
    CallSignature, CallingPolicy, ClaimId, IntegerSign, IntegerType, IntegerValue,
    LegalizedOperationPlan, LegalizedScalarCall, LegalizedScalarInstructionKind, OwnershipEvent,
    ScalarType, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeShape, ValueShape, call_aware_plan,
    evaluate_call_plan, id, scalar_argument_mut, scalar_call_unit_plan, structural_argument_mut,
    structural_call_mut,
};

pub(super) fn assert_identity_drift(
    original: LegalizedOperationPlanIdentity,
    corrupted: &LegalizedOperationPlan,
) {
    assert_ne!(legalized_operation_plan_identity(corrupted), original);
}

fn scalar_return_mut(function: &mut LegalizedScalarFunction) -> &mut LegalizedScalarReturn {
    let LegalizedScalarTerminator::Return(returned) = &mut function.blocks[0].terminator else {
        panic!("return fixture")
    };
    returned
}

#[test]
fn scalar_graph_identity_binds_ordered_source_and_abi_custody() {
    let plan = scalar_call_unit_plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_eq!(identity, legalized_operation_plan_identity(&plan));
    let mut empty = plan.clone();
    empty.scalar_functions.clear();
    assert_ne!(legalized_operation_plan_identity(&empty), identity);
    for mutation in 0..22 {
        let mut changed = plan.clone();
        let function = &mut changed.scalar_functions[0];
        match mutation {
            0 => function.attachment = Some(id(999)),
            1 => {
                function.blocks[0].instructions[0].kind =
                    LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(8))
            }
            2 => function.blocks[0].instructions.swap(0, 1),
            3 => {
                call_mut(function, 0).target = target_operations::NativeCallTarget::Machine(id(999))
            }
            4 => call_mut(function, 0).arguments.swap(0, 1),
            5 => {
                function.blocks[0].instructions[4]
                    .result
                    .as_mut()
                    .unwrap()
                    .value = id(999)
            }
            6 => function.blocks[0].instructions[3].work[0].units += 1,
            7 => function.blocks[0].instructions[3].effect.output += 1,
            8 => scalar_return_mut(function).edge = id(999),
            9 => scalar_return_mut(function).ownership.clear(),
            10 => function.call_plan.shadow_bytes += 8,
            11 => function.entry_block = id(999),
            12 => function.blocks[0].id = id(999),
            13 => {
                scalar_return_mut(function).value = LegalizedScalarReturnValue::Value {
                    value: id(114),
                    scalar_type: semantic_vocabulary::ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                }
            }
            14 => {
                function.blocks[0].instructions[0]
                    .result
                    .as_mut()
                    .unwrap()
                    .definition_site = optimization_unit::ValueDefinitionSite::FunctionParameter(0)
            }
            15 => call_mut(function, 0)
                .result_placement
                .as_mut()
                .unwrap()
                .locations
                .clear(),
            16 => call_mut(function, 0).requirement_obligations.push(id(999)),
            17 => *scalar_argument_mut(&mut call_mut(function, 0).arguments[0]).0 = id(999),
            18 => scalar_return_mut(function).work[0].units += 1,
            19 => scalar_return_mut(function).effect.input += 1,
            20 => function.blocks[0].instructions.pop().map(|_| ()).unwrap(),
            _ => function.provenance.operations.reverse(),
        }
        assert_identity_drift(identity, &changed);
    }
    let mut parameterized = plan.clone();
    parameterized.scalar_functions[0]
        .parameters
        .push(LegalizedScalarParameter {
            value: id(700),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            definition_site: optimization_unit::ValueDefinitionSite::FunctionParameter(0),
            placement: call_mut(&mut plan.clone().scalar_functions[0], 0).arguments[0]
                .placement()
                .clone(),
        });
    let identity = legalized_operation_plan_identity(&parameterized);
    for mutation in 0..4 {
        let mut changed = parameterized.clone();
        let parameter = &mut changed.scalar_functions[0].parameters[0];
        match mutation {
            0 => parameter.value = id(701),
            1 => {
                parameter.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
            }
            2 => {
                parameter.definition_site =
                    optimization_unit::ValueDefinitionSite::FunctionParameter(1)
            }
            _ => parameter.placement.locations.clear(),
        }
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn call_aware_unit_identity_binds_semantic_and_target_custody() {
    let plan = call_aware_plan();
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..23 {
        let mut changed = plan.clone();
        match mutation {
            0 => changed.scalar_functions[0].structural = None,
            1 => {
                changed.scalar_functions[0]
                    .structural
                    .as_mut()
                    .unwrap()
                    .structural_types
                    .make_mut()[0]
                    .identity = terminal_psi::TerminalIdentityIndex(999)
            }
            2 => changed.scalar_functions[0].call_plan.shadow_bytes += 8,
            3 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .parameters
                .swap(0, 1),
            4 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .parameters[0]
                .semantic
                .qualifications
                .clear(),
            5 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .parameters[0]
                .target
                .placement
                .locations
                .clear(),
            6 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .structural_places
                .swap(0, 1),
            7 => {
                changed.scalar_functions[0]
                    .structural
                    .as_mut()
                    .unwrap()
                    .entry_claims[0]
                    .claim = id::<ClaimId>(3)
            }
            8 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .published_service_ceiling
                .push(id(1)),
            9 => {
                structural_call_mut(&mut changed).target =
                    target_operations::NativeCallTarget::Machine(id(3))
            }
            10 => structural_call_mut(&mut changed).arguments.swap(0, 1),
            11 => structural_argument_mut(&mut structural_call_mut(&mut changed).arguments[0])
                .0
                .path
                .push(StructuralPathSegment::Field(
                    terminal_psi::TerminalIdentityIndex(2),
                )),
            12 => {
                structural_argument_mut(&mut structural_call_mut(&mut changed).arguments[0])
                    .1
                    .source_byte_offset = 8
            }
            13 => structural_call_mut(&mut changed).claim_transfers.swap(0, 1),
            14 => changed.scalar_functions[0].blocks[0].instructions[0].work[0].units += 1,
            15 => {
                changed.scalar_functions[0].blocks[0].instructions[0]
                    .effect
                    .output += 1
            }
            16 => structural_call_mut(&mut changed).requirement_obligations[0] = id(2),
            17 => {
                structural_call_mut(&mut changed).crash_continuations[0].cause =
                    terminal_psi::CrashCause::Abort
            }
            18 => {
                let OwnershipEvent::ClaimTransfer(claims) =
                    &mut changed.scalar_functions[0].blocks[0].instructions[0].ownership[0]
                else {
                    panic!("transfer fixture");
                };
                claims.swap(0, 1);
            }
            19 => changed.scalar_functions[0].blocks[0].instructions.clear(),
            20 => scalar_return_mut(&mut changed.scalar_functions[0]).work[0].units += 1,
            21 => {
                scalar_return_mut(&mut changed.scalar_functions[0])
                    .effect
                    .input += 1
            }
            _ => {
                let OwnershipEvent::Cleanup(actions) =
                    &mut scalar_return_mut(&mut changed.scalar_functions[0]).ownership[0]
                else {
                    panic!("cleanup fixture");
                };
                actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(id(
                    1,
                )));
            }
        }
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn abi_layout_substitutions_drift_the_plan_identity() {
    let plan = scalar_call_unit_plan();
    let identity = legalized_operation_plan_identity(&plan);
    let shape = ValueShape::integer(8, 8);
    let signature = CallSignature {
        parameters: vec![shape, shape],
        result: Some(shape),
    };
    let carried = call_mut(&mut plan.clone().scalar_functions[0], 0)
        .call_plan
        .clone();
    assert_eq!(
        carried,
        evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).unwrap()
    );
    assert_ne!(
        carried,
        evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature).unwrap()
    );
    for mutation in 0..10 {
        let mut changed = plan.clone();
        let call = call_mut(&mut changed.scalar_functions[0], 0);
        match mutation {
            0 => call.call_plan.policy = CallingPolicy::MicrosoftX64,
            1 => {
                let calling_conventions::ValueLocation::Register { register, .. } =
                    &mut call.call_plan.parameters[0].locations[0]
                else {
                    panic!("register fixture");
                };
                *register = calling_conventions::MachineRegister::X86Rdx;
            }
            2 => {
                let calling_conventions::ValueLocation::Register { byte_size, .. } =
                    &mut call.call_plan.parameters[1].locations[0]
                else {
                    panic!("register fixture");
                };
                *byte_size += 1;
            }
            3 => {
                call.call_plan.parameters[0].locations[0] =
                    calling_conventions::ValueLocation::Stack {
                        stack_byte_offset: 0,
                        value_byte_offset: 0,
                        byte_size: 8,
                        alignment: 8,
                    };
            }
            4 => call.call_plan.parameters[0].shape = ValueShape::integer(8, 4),
            5 => call.call_plan.result = Some(call.call_plan.parameters[0].clone()),
            6 => call.call_plan.stack_alignment += 2,
            7 => {
                call.call_plan.entry_control = calling_conventions::EntryControl::SupervisorCall {
                    number_register: calling_conventions::MachineRegister::X86Rax,
                    immediate: 0x80,
                };
            }
            8 => {
                call.call_plan.ordinary_clobbers = calling_conventions::RegisterSet::new([
                    calling_conventions::MachineRegister::X86Rbx,
                ]);
            }
            _ => {
                let calling_conventions::ValueLocation::Register { register, .. } =
                    &mut call.result_placement.as_mut().unwrap().locations[0]
                else {
                    panic!("register fixture");
                };
                *register = calling_conventions::MachineRegister::X86Rcx;
            }
        }
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn ownership_frontier_fact_identities_bind_the_plan_identity() {
    let plan = scalar_call_unit_plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert!(plan.scalar_functions[0].ownership_frontier_facts.is_empty());
    let fact = |site| {
        optimization_unit::OwnershipFrontierFact::new(
            plan.psi,
            id(1),
            site,
            optimization_unit::OwnershipFrontierSnapshot {
                claims: Vec::new(),
                owned_places: Vec::new(),
                partial_custody: Vec::new(),
            },
        )
    };
    let mut retained = plan.clone();
    retained.scalar_functions[0].ownership_frontier_facts = vec![
        fact(optimization_unit::OwnershipFrontierSite::BlockEntry(id(1))),
        fact(optimization_unit::OwnershipFrontierSite::EdgeEntry(id(1))),
    ];
    let retained_identity = legalized_operation_plan_identity(&retained);
    assert_identity_drift(identity, &retained);
    let mut reordered = retained.clone();
    reordered.scalar_functions[0]
        .ownership_frontier_facts
        .swap(0, 1);
    assert_identity_drift(retained_identity, &reordered);
    let mut substituted = retained.clone();
    substituted.scalar_functions[0].ownership_frontier_facts[1] =
        fact(optimization_unit::OwnershipFrontierSite::EdgeEntry(id(2)));
    assert_identity_drift(retained_identity, &substituted);
    let mut dropped = retained.clone();
    dropped.scalar_functions[0].ownership_frontier_facts.pop();
    assert_identity_drift(retained_identity, &dropped);
}

#[test]
fn structural_signature_and_calls_cannot_alias_value_less_unit_graph() {
    let original = call_aware_plan();
    let mut erased = original.clone();
    let function = &mut erased.scalar_functions[0];
    function.structural = None;
    function.blocks[0].instructions.clear();
    function.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(erased.target),
        &CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .unwrap();
    assert_ne!(
        legalized_operation_plan_identity(&original),
        legalized_operation_plan_identity(&erased)
    );
}

fn call_mut(function: &mut LegalizedScalarFunction, index: usize) -> &mut LegalizedScalarCall {
    function.blocks[0]
        .instructions
        .iter_mut()
        .filter_map(|instruction| match &mut instruction.kind {
            LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .nth(index)
        .expect("call fixture")
}

#[test]
fn literal_identity_binds_payload_declaration_and_producer_reference() {
    let mut plan = call_aware_plan();
    let call = structural_call_mut(&mut plan);
    let (_, argument) = structural_argument_mut(&mut call.arguments[0]);
    argument.source = target_operations::TargetStructuralArgumentSource::EstablishedByteView {
        psi_operation: id(900),
    };
    let destination = StructuralPlaceDeclaration {
        id: argument.place,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: argument.structural_type,
        },
    };
    let structural_type = StructuralTypeDeclaration {
        requires: Vec::new(),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        copyable: false,
        descriptor_cleanup: false,
        scalar_restriction: None,
        id: argument.structural_type,
        identity: terminal_psi::TerminalIdentityIndex(1),
        shape: StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        }),
    };
    let mut establishment = plan.scalar_functions[0].blocks[0].instructions[0].clone();
    establishment.operation = id(900);
    establishment.result = None;
    establishment.kind = LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
        destination,
        structural_type,
        bytes: vec![0, 128, 255],
    };
    plan.scalar_functions[0].blocks[0]
        .instructions
        .push(establishment);
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..8 {
        let mut changed = plan.clone();
        if mutation < 2 {
            let call = structural_call_mut(&mut changed);
            let (_, argument) = structural_argument_mut(&mut call.arguments[0]);
            argument.source = if mutation == 0 {
                target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: id(901),
                }
            } else {
                argument.destination.clone().into()
            };
        } else {
            let row = changed.scalar_functions[0].blocks[0]
                .instructions
                .last_mut()
                .unwrap();
            let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                bytes,
            } = &mut row.kind
            else {
                panic!("literal fixture");
            };
            match mutation {
                2 => row.operation = id(901),
                3 => destination.id = id(901),
                4 => {
                    destination.kind = StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal: 1,
                        structural_type: structural_type.id,
                    }
                }
                5 => structural_type.id = id(901),
                6 => bytes[1] = 129,
                _ => bytes.push(0),
            }
        }
        assert_identity_drift(identity, &changed);
    }
}

fn primitive_store_plan() -> LegalizedOperationPlan {
    let mut plan = scalar_call_unit_plan();
    let row = &mut plan.scalar_functions[0].blocks[0].instructions[0];
    row.result = None;
    row.kind = LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
        path: Vec::new(),
        byte_offset: 0,
        destination: StructuralParameterDeclaration {
            place: id(501),
            position: 0,
            structural_type: id(502),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::WriteOnlyBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        }
        .into(),
        value: abstract_operations::AbstractResult {
            value: id(503),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        },
        byte_size: 4,
        indices: Vec::new(),
    };
    plan
}

#[test]
fn primitive_store_identity_binds_destination_source_type_and_width() {
    let plan = primitive_store_plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_eq!(identity, legalized_operation_plan_identity(&plan.clone()));
    for mutation in 0..15 {
        let mut changed = plan.clone();
        let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination:
                abstract_operations::StructuralScalarStoreDestination::Parameter(destination),
            path,
            value,
            byte_offset,
            byte_size,
            indices,
        } = &mut changed.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            panic!("primitive store fixture")
        };
        match mutation {
            0 => destination.place = id(999),
            1 => destination.position = 1,
            2 => destination.structural_type = id(999),
            3 => destination.multiplicity = StructuralMultiplicity::Linear,
            4 => destination.access = StructuralAccess::MutableBorrow,
            5 => destination.qualifications.push(id(999)),
            6 => value.value = id(999),
            7 => {
                value.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
            }
            8 => destination.projected_qualifications.push(
                terminal_psi::StructuralPathQualification {
                    path: vec![StructuralPathSegment::FixedIndex(1)],
                    domain: id(998),
                },
            ),
            9 => *byte_size = 8,
            10 => path.push(StructuralPathSegment::FixedIndex(1)),
            11 => *byte_offset = 4,
            12 => indices.push(runtime_index(id(700), 4)),
            13 => indices.push(runtime_index(id(701), 4)),
            _ => indices.push(runtime_index(id(700), 8)),
        }
        assert_ne!(
            identity,
            legalized_operation_plan_identity(&changed),
            "primitive store mutation {mutation}"
        );
    }
    let mut ieee = plan.clone();
    let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { value, .. } =
        &mut ieee.scalar_functions[0].blocks[0].instructions[0].kind
    else {
        panic!("primitive store fixture")
    };
    value.scalar_type = ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32);
    assert_identity_drift(identity, &ieee);
}

#[test]
fn primitive_store_references_its_input_and_is_not_a_field_store() {
    let plan = primitive_store_plan();
    assert!(plan.scalar_functions[0].references_value(id(503)));
    assert!(!plan.scalar_functions[0].references_value(id(999)));
    let mut field = plan.clone();
    let row = &mut field.scalar_functions[0].blocks[0].instructions[0];
    let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
        destination,
        value,
        byte_size,
        ..
    } = row.kind.clone()
    else {
        panic!("primitive store fixture")
    };
    row.kind = LegalizedScalarInstructionKind::StructuralScalarFieldStore {
        destination,
        value,
        byte_size: byte_size.into(),
        path: Vec::new(),
        field: id(504),
        byte_offset: 0,
        indices: Vec::new(),
    };
    assert_identity_drift(legalized_operation_plan_identity(&plan), &field);
    let field_identity = legalized_operation_plan_identity(&field);
    let LegalizedScalarInstructionKind::StructuralScalarFieldStore {
        destination: abstract_operations::StructuralScalarStoreDestination::Parameter(destination),
        ..
    } = &mut field.scalar_functions[0].blocks[0].instructions[0].kind
    else {
        panic!("field store fixture")
    };
    destination
        .projected_qualifications
        .push(terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::FixedIndex(1)],
            domain: id(998),
        });
    assert_identity_drift(field_identity, &field);
}

#[test]
fn primitive_store_identity_binds_owned_producer_and_home() {
    let mut plan = primitive_store_plan();
    let borrowed_identity = legalized_operation_plan_identity(&plan);
    let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } =
        &mut plan.scalar_functions[0].blocks[0].instructions[0].kind
    else {
        panic!("primitive store fixture");
    };
    *destination = abstract_operations::StructuralScalarStoreDestination::OwnedResult {
        producer: id(505),
        result: terminal_psi::StructuralOperationResult {
            place: id(501),
            structural_type: id(502),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        },
    };
    assert_identity_drift(borrowed_identity, &plan);
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..3 {
        let mut changed = plan.clone();
        let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination:
                abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result },
            ..
        } = &mut changed.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            panic!("owned primitive store");
        };
        match mutation {
            0 => *producer = id(999),
            1 => result.place = id(999),
            _ => result.structural_type = id(999),
        }
        assert_identity_drift(identity, &changed);
    }
}

fn runtime_index(
    operand: semantic_vocabulary::ValueId,
    stride: u32,
) -> crate::LegalizedRuntimeIndexOperand {
    crate::LegalizedRuntimeIndexOperand {
        operand: abstract_operations::AbstractResult {
            value: operand,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        },
        stride,
        extent: Some(3),
        obligation: id(702),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
            b"runtime-index",
        ),
    }
}

#[test]
fn owned_block_field_store_identity_binds_block_declaration_and_variant() {
    use abstract_operations::StructuralScalarStoreDestination as Destination;

    let plan = primitive_store_plan();
    let field_store = |destination: Destination| {
        let mut changed = plan.clone();
        let row = &mut changed.scalar_functions[0].blocks[0].instructions[0];
        let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            value, byte_size, ..
        } = row.kind.clone()
        else {
            panic!("primitive store fixture")
        };
        row.kind = LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination,
            value,
            byte_size: byte_size.into(),
            path: Vec::new(),
            field: id(504),
            byte_offset: 0,
            indices: Vec::new(),
        };
        legalized_operation_plan_identity(&changed)
    };
    let declaration = |place: u64| StructuralParameterDeclaration {
        place: id(place),
        position: 0,
        structural_type: id(502),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    };
    let arrival = |block: u64, place: u64| Destination::OwnedBlockParameter {
        block: id(block),
        parameter: declaration(place),
    };
    let identity = field_store(arrival(3, 501));
    assert_eq!(identity, field_store(arrival(3, 501)));
    assert_ne!(identity, field_store(arrival(4, 501)));
    assert_ne!(identity, field_store(arrival(3, 999)));
    assert_ne!(
        identity,
        field_store(Destination::Parameter(declaration(501)))
    );
    let mut shared = declaration(501);
    shared.access = StructuralAccess::SharedBorrow;
    assert_ne!(
        identity,
        field_store(Destination::OwnedBlockParameter {
            block: id(3),
            parameter: shared,
        })
    );
}
