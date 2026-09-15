//! Authored borrows retain reference ABI placement after canonical Psi checking.

use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, BlockId, EdgeId, IntegerSign, IntegerType,
    ScalarType, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldId, StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, ValueId,
};
use calling_conventions::{ValueClass, ValueLocation};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use target::NativeTarget;
use target_operations::{TargetOperationPlan, TargetStructuralArgument, TargetUnitOperation};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi_to_abstract_operations::lower_artifact;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn source_plan(source: &str) -> abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal =
        checked_trees_to_lowered_psi::lower_machine(&checked, "Main::run").expect("lower source");
    lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &encode_module(&terminal.semantic_module).expect("encode semantics"),
            proof_bytes: &encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
                .expect("encode proof"),
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_plan())
    .expect("verify and lower canonical artifact")
}

#[test]
fn relevant_erased_record_fields_have_no_runtime_layout_or_scalar_access() {
    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape,
    };
    for scalar_storage in [true, false] {
        let mut source = source_plan(
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 65; }",
        );
        // This stage test retains an explicit receiver occurrence. Source-level
        // removal of an unused empty self is a separate correspondence contract.
        if !scalar_storage {
            let entry = source
                .functions
                .iter_mut()
                .find(|function| function.machine == source.entry)
                .unwrap();
            entry.operations.retain(|operation| {
                matches!(
                    operation,
                    abstract_operations::AbstractOperation::ReturnUnit { .. }
                )
            });
        }
        let entry = source
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap();
        let receiver = entry.structural_parameters[0].structural_type;
        let erased_field = StructuralFieldId::new(99).unwrap();
        let declaration = source
            .structural_types
            .make_mut()
            .iter_mut()
            .find(|declaration| declaration.id == receiver)
            .unwrap();
        let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
            panic!("receiver record");
        };
        if !scalar_storage {
            fields.clear();
        }
        fields.insert(
            0,
            StructuralFieldDeclaration {
                id: erased_field,
                identity: "service".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Erased {
                    type_identity: "FusedService".into(),
                },
            },
        );
        for target in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
            let lowered = crate::lower_to_target_operations(
                &source,
                crate::TargetLoweringRequest::new(target),
            )
            .expect("erased carrier occupies no bytes");
            crate::validate_abstract_to_target_translation(&source, target, &lowered)
                .expect("independent receiver shape replay");
            let entry = lowered
                .functions
                .iter()
                .find(|function| function.machine == source.entry)
                .unwrap();
            assert_eq!(
                entry.graph.parameters[0].shape.byte_size,
                if scalar_storage { 4 } else { 0 }
            );
            assert_eq!(
                entry.graph.parameters[0].shape.class,
                ValueClass::BorrowedReference
            );
            assert!(!entry.graph.parameters[0].placement.locations.is_empty());
            let mut changed = lowered.clone();
            let entry = changed
                .functions
                .iter_mut()
                .find(|function| function.machine == source.entry)
                .unwrap();
            entry.graph.parameters[0].shape.byte_size += 1;
            assert!(
                crate::validate_abstract_to_target_translation(&source, target, &changed).is_err()
            );
        }
        if scalar_storage {
            let entry = source
                .functions
                .iter_mut()
                .find(|function| function.machine == source.entry)
                .unwrap();
            let store = entry
                .operations
                .iter_mut()
                .find_map(|operation| match operation {
                    abstract_operations::AbstractOperation::StructuralScalarFieldStore {
                        field,
                        ..
                    } => Some(field),
                    _ => None,
                })
                .expect("scalar field store");
            *store = erased_field;
            assert!(
                crate::lower_to_target_operations(
                    &source,
                    crate::TargetLoweringRequest::new(NativeTarget::macos_arm64())
                )
                .is_err(),
                "runtime erasure grants no scalar access"
            );
        }
    }
}

fn shared_source() -> abstract_operations::AbstractOperationPlan {
    source_plan(
        r#"
            trait Read { machine read(&self) -> i32; }
            data Item { value: i32; }
            Reader: Item satisfies Read {
                machine read(&self) -> i32 { transition { _ -> self.value } }
            }
            data Main { item: Item; }
            machine Main::run(&self) {
                let reader: &dyn Read = &self.item as &dyn Item::Reader;
                let value: i32 = reader.read();
            }
        "#,
    )
}

fn projected_field_borrow_plan() -> abstract_operations::AbstractOperationPlan {
    let tally = StructuralTypeId::new(1).unwrap();
    let main = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let function = |raw, structural_type| AbstractFunction {
        machine: MachineId::new(raw).unwrap(),
        attachment: None,
        entry: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: PlaceId::new(raw).unwrap(),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(raw).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations: vec![AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(raw).unwrap(),
            cleanup_actions: Vec::new(),
        }],
    };
    let mut caller = function(1, main);
    let callee = function(2, tally);
    caller.operations.insert(
        0,
        AbstractOperation::CallUnit {
            psi_operation: OperationId::new(1).unwrap(),
            callee: callee.machine,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: caller.structural_parameters[0].place,
                access: StructuralAccess::MutableBorrow,
                path: vec![terminal_psi::StructuralPathSegment::Field("tally".into())],
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller.machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: tally,
                identity: "Tally".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "calls".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(unsigned),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: main,
                identity: "Main".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "tally".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(tally),
                    }],
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![caller, callee],
    }
}

/// The same projected-field borrow with one runtime scalar argument, so the
/// retained call exercises the scalar prefix of the callee's plan too.
fn projected_field_borrow_scalar_argument_plan() -> abstract_operations::AbstractOperationPlan {
    let mut plan = projected_field_borrow_plan();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_value = ValueId::new(40).unwrap();
    plan.functions[0].parameters.push(AbstractParameter {
        value: caller_value,
        scalar_type: unsigned,
    });
    plan.functions[1].parameters.push(AbstractParameter {
        value: ValueId::new(41).unwrap(),
        scalar_type: unsigned,
    });
    for operation in &mut plan.functions[0].operations {
        if let AbstractOperation::CallUnit { arguments, .. } = operation {
            arguments.push(caller_value);
        }
    }
    plan
}

fn mutate_call_plan(
    target: &TargetOperationPlan,
    f: impl Fn(&mut calling_conventions::CallPlan),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let call_plan = match operation {
                    TargetUnitOperation::Call { call_plan, .. }
                    | TargetUnitOperation::StructuralScalarCall { call_plan, .. }
                    | TargetUnitOperation::StructuralResultCall { call_plan, .. } => call_plan,
                    _ => continue,
                };
                f(call_plan);
            }
        }
    }
    target
}

fn mutate_scalar_arguments(
    target: &TargetOperationPlan,
    f: impl Fn(&mut Vec<target_operations::TargetUnitScalarCallArgument>),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let arguments = match operation {
                    TargetUnitOperation::Call {
                        scalar_arguments, ..
                    }
                    | TargetUnitOperation::StructuralScalarCall {
                        scalar_arguments, ..
                    }
                    | TargetUnitOperation::StructuralResultCall {
                        scalar_arguments, ..
                    } => scalar_arguments,
                    _ => continue,
                };
                f(arguments);
            }
        }
    }
    target
}

fn mutate_call_arguments(
    target: &TargetOperationPlan,
    f: impl Fn(&mut TargetStructuralArgument),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let arguments = match operation {
                    TargetUnitOperation::Call { arguments, .. }
                    | TargetUnitOperation::StructuralScalarCall { arguments, .. }
                    | TargetUnitOperation::StructuralResultCall { arguments, .. } => arguments,
                    _ => continue,
                };
                for argument in arguments {
                    f(argument);
                }
            }
        }
    }
    target
}

#[test]
fn projected_field_borrow_retains_borrowed_reference_and_validates() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = projected_field_borrow_plan();
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let argument = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            argument.access,
            terminal_psi::StructuralAccess::MutableBorrow
        );
        assert_eq!(argument.shape.class, ValueClass::BorrowedReference);
        assert_eq!(argument.structural_type, StructuralTypeId::new(1).unwrap());
        assert_eq!(
            argument.root_structural_type,
            StructuralTypeId::new(2).unwrap()
        );
        assert_eq!(
            argument.path,
            vec![terminal_psi::StructuralPathSegment::Field("tally".into())]
        );
        assert_eq!(argument.source_byte_offset, 0);
        assert!(!argument.destination.locations.is_empty());
        assert!(
            argument
                .destination
                .locations
                .iter()
                .all(|location| matches!(location, ValueLocation::Indirect { .. }))
        );
    }
}

#[test]
fn projected_field_borrow_rejects_substituted_identity_access_shape_and_placement() {
    let source = projected_field_borrow_plan();
    let native = NativeTarget::linux_x64();
    let target =
        crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
            .unwrap();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for mutation in [
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.access = terminal_psi::StructuralAccess::SharedBorrow
        }) as Box<dyn Fn(&mut TargetStructuralArgument)>,
        Box::new(|argument: &mut TargetStructuralArgument| argument.path.clear()),
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.structural_type = argument.root_structural_type
        }),
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.shape.class = ValueClass::Integer
        }),
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.place = PlaceId::new(7).unwrap()
        }),
        Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 8),
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.root_structural_type = StructuralTypeId::new(1).unwrap()
        }),
        Box::new(|argument: &mut TargetStructuralArgument| argument.destination.locations.clear()),
    ] {
        let mutated = mutate_call_arguments(&target, mutation);
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &mutated),
            Err(expected.clone())
        );
    }
}

#[test]
fn projected_field_borrow_rejects_substituted_home_identity() {
    let source = projected_field_borrow_plan();
    let native = NativeTarget::linux_x64();
    let target =
        crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
            .unwrap();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for mutation in [
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.source = target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: OperationId::new(1).unwrap(),
            }
        }) as Box<dyn Fn(&mut TargetStructuralArgument)>,
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.source =
                target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                    psi_operation: OperationId::new(1).unwrap(),
                }
        }),
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.source =
                target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: OperationId::new(77).unwrap(),
                }
        }),
        Box::new(|argument: &mut TargetStructuralArgument| {
            argument.source = target_operations::TargetStructuralArgumentSource::BlockParameter {
                block: semantic_vocabulary::BlockId::new(1).unwrap(),
                place: argument.place,
            }
        }),
        Box::new(|argument: &mut TargetStructuralArgument| {
            let target_operations::TargetStructuralArgumentSource::Placement(placement) =
                &argument.source
            else {
                panic!("caller parameter transport");
            };
            let mut forged = placement.clone();
            forged.locations.clear();
            argument.source = forged.into();
        }),
    ] {
        let mutated = mutate_call_arguments(&target, mutation);
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &mutated),
            Err(expected.clone()),
            "substituted argument source"
        );
    }
}

#[test]
fn projected_field_borrow_rejects_substituted_callee_plan() {
    let source = projected_field_borrow_plan();
    let native = NativeTarget::linux_x64();
    let target =
        crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
            .unwrap();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for mutation in [
        Box::new(|plan: &mut calling_conventions::CallPlan| {
            plan.stack_alignment = plan.stack_alignment.wrapping_add(8);
        }) as Box<dyn Fn(&mut calling_conventions::CallPlan)>,
        Box::new(|plan: &mut calling_conventions::CallPlan| {
            plan.result = Some(plan.parameters[0].clone());
        }),
        Box::new(|plan: &mut calling_conventions::CallPlan| {
            plan.parameters.push(plan.parameters[0].clone());
        }),
        Box::new(|plan: &mut calling_conventions::CallPlan| {
            plan.policy = calling_conventions::CallingPolicy::MicrosoftX64;
        }),
        Box::new(|plan: &mut calling_conventions::CallPlan| {
            plan.shadow_bytes = plan.shadow_bytes.wrapping_add(4);
        }),
    ] {
        let mutated = mutate_call_plan(&target, mutation);
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &mutated),
            Err(expected.clone()),
            "embedded callee plan detail"
        );
    }
}

#[test]
fn call_scalar_arguments_reject_substituted_identity_and_placement() {
    let source = projected_field_borrow_scalar_argument_plan();
    let native = NativeTarget::linux_x64();
    let target =
        crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
            .unwrap();
    crate::validate_abstract_to_target_translation(&source, native, &target)
        .expect("honest scalar argument transport");
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for mutation in [
        Box::new(
            |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                arguments.clear();
            },
        ) as Box<dyn Fn(&mut Vec<target_operations::TargetUnitScalarCallArgument>)>,
        Box::new(
            |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                arguments[0].parameter_index = 9;
            },
        ),
        Box::new(
            |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                arguments[0].placement.locations.clear();
            },
        ),
        Box::new(
            |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                let target_operations::TargetUnitScalarArgumentSource::Parameter {
                    scalar_type,
                    ..
                } = arguments[0].source
                else {
                    panic!("caller parameter transport");
                };
                arguments[0].source =
                    target_operations::TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 0,
                        source_value: semantic_vocabulary::ValueId::new(99).unwrap(),
                        scalar_type,
                    };
            },
        ),
        Box::new(
            |arguments: &mut Vec<target_operations::TargetUnitScalarCallArgument>| {
                arguments.push(arguments[0].clone());
            },
        ),
    ] {
        let mutated = mutate_scalar_arguments(&target, mutation);
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &mutated),
            Err(expected.clone()),
            "scalar argument identity"
        );
    }
}

#[test]
fn every_borrow_mode_uses_register_and_stack_pointers_without_value_copies() {
    use crate::lowering::structural_signature::StructuralCallSignature;
    use calling_conventions::IndirectPointerLocation;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralAccess;

    let source = shared_source();
    let receiver = source
        .functions
        .iter()
        .flat_map(|function| &function.structural_parameters)
        .next()
        .unwrap();
    let declarations = source
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let parameters = (0..12)
            .map(|position| {
                let mut parameter = receiver.clone();
                parameter.position = position;
                parameter.place =
                    semantic_vocabulary::PlaceId::new(u64::from(position) + 1).unwrap();
                parameter.is_self = false;
                parameter.access = access;
                parameter
            })
            .collect::<Vec<_>>();
        let signature = StructuralCallSignature::derive(
            &[],
            &parameters,
            None,
            &declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .unwrap();
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::windows_x64(),
            NativeTarget::macos_arm64(),
        ] {
            let plan = signature.plan(target).unwrap();
            let mut register_pointers = 0;
            let mut stack_pointers = 0;
            for placement in &plan.parameters {
                if access == StructuralAccess::Owned {
                    assert_eq!(placement.shape.class, ValueClass::Integer);
                    continue;
                }
                assert_eq!(placement.shape.class, ValueClass::BorrowedReference);
                match placement.locations.as_slice() {
                    [
                        ValueLocation::Indirect {
                            pointer,
                            copy_stack_byte_offset: None,
                            ..
                        },
                    ] => match pointer {
                        IndirectPointerLocation::Register(_) => register_pointers += 1,
                        IndirectPointerLocation::Stack { .. } => stack_pointers += 1,
                    },
                    locations => panic!("borrow must remain a pointer: {locations:?}"),
                }
            }
            if access != StructuralAccess::Owned {
                assert!(register_pointers > 0 && stack_pointers > 0, "{target:?}");
            }
        }
    }
}
