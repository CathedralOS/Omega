//! Authored borrows retain reference ABI placement after canonical Psi checking.

use calling_conventions::{ValueClass, ValueLocation};
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations::TargetOperation;
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

#[test]
fn shared_receivers_use_references_in_unit_and_scalar_signatures() {
    let plan = shared_source();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = crate::lower_to_target_operations(&plan, target).expect("lower ABI");
        crate::validate_abstract_to_target_translation(&plan, target, &lowered)
            .expect("independent structural header replay");
        for function in &lowered.functions {
            let parameters = match &function.operation {
                TargetOperation::UnitBody(body) => &body.parameters,
                _ => {
                    &function
                        .mixed_structural_scalar_abi
                        .as_ref()
                        .expect("scalar receiver ABI")
                        .structural_parameters
                }
            };
            let [parameter] = parameters.as_slice() else {
                panic!("one receiver")
            };
            assert_eq!(
                parameter.shape.class,
                ValueClass::BorrowedReference,
                "{target:?}"
            );
            assert_eq!(parameter.placement.shape, parameter.shape);
            assert!(matches!(
                parameter.placement.locations.as_slice(),
                [ValueLocation::Indirect {
                    copy_stack_byte_offset: None,
                    ..
                }]
            ));
        }
    }
}

#[test]
fn structural_header_replay_rejects_copied_borrows_and_access_substitution() {
    let source = shared_source();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let original = crate::lower_to_target_operations(&source, target).unwrap();
        for function_position in 0..original.functions.len() {
            for mutation in 0..4 {
                let mut candidate = original.clone();
                let function = &mut candidate.functions[function_position];
                let (actual_plan, parameter) = match &mut function.operation {
                    TargetOperation::UnitBody(body) => {
                        (&mut body.call_plan, &mut body.parameters[0])
                    }
                    _ => {
                        let abi = function.mixed_structural_scalar_abi.as_mut().unwrap();
                        (&mut abi.call_plan, &mut abi.structural_parameters[0])
                    }
                };
                match mutation {
                    0 | 2 => {
                        // A self-consistent value ABI still cannot represent a source borrow.
                        let shape = if mutation == 0 {
                            calling_conventions::ValueShape::integer(
                                parameter.shape.byte_size,
                                parameter.shape.alignment,
                            )
                        } else {
                            calling_conventions::ValueShape::borrowed_reference(
                                parameter.shape.byte_size + 1,
                                parameter.shape.alignment,
                            )
                        };
                        *actual_plan = calling_conventions::evaluate_call_plan(
                            calling_conventions::CallingPolicy::native_for_target(target),
                            &calling_conventions::CallSignature {
                                parameters: vec![shape],
                                result: actual_plan
                                    .result
                                    .as_ref()
                                    .map(|placement| placement.shape),
                            },
                        )
                        .unwrap();
                        parameter.shape = shape;
                        parameter.placement = actual_plan.parameters[0].clone();
                    }
                    1 => parameter.access = terminal_psi::StructuralAccess::Owned,
                    3 => parameter.place = semantic_vocabulary::PlaceId::new(999).unwrap(),
                    _ => unreachable!(),
                }
                assert!(
                    crate::validate_abstract_to_target_translation(&source, target, &candidate)
                        .is_err(),
                    "{target:?}: mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn matching_reference_shapes_do_not_authorize_argument_access_substitution() {
    let mut source = shared_source();
    let argument = source
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.operations)
        .find_map(|operation| match operation {
            abstract_operations::AbstractOperation::CallStructuralScalar {
                structural_arguments,
                ..
            } => structural_arguments.first_mut(),
            _ => None,
        })
        .expect("projected shared call");
    argument.access = terminal_psi::StructuralAccess::MutableBorrow;
    assert!(crate::lower_to_target_operations(&source, NativeTarget::linux_x64()).is_err());
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
