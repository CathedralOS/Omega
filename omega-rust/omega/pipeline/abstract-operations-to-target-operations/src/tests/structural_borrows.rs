//! Authored borrows retain reference ABI placement after canonical Psi checking.

use calling_conventions::{ValueClass, ValueLocation};
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
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
