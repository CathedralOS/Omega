//! Authored borrows retain reference ABI placement after canonical Psi checking.

use calling_conventions::{ValueClass, ValueLocation};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations::{TargetOperationPlan, TargetStructuralArgument, TargetUnitOperation};
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn source_plan(source: &str) -> abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal =
        checked_trees_to_lowered_psi::lower_machine(&checked, "Main::run").expect("lower source");
    lower_artifact_sections(
        &encode_module(&terminal.semantic_module).expect("encode semantics"),
        &encode_proof_bundle(&terminal.proof_bundle).expect("encode proof"),
        &AdmissionProfile::default(),
    )
    .expect("verify and lower canonical artifact")
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
    use super::*;
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
        let target = crate::lower_to_target_operations(&source, native).unwrap();
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
    let target = crate::lower_to_target_operations(&source, native).unwrap();
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
