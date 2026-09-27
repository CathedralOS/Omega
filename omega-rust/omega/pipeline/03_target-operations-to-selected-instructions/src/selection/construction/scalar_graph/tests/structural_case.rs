//! Lawfully legalized cases select through ordinary control and edge bridges.
use super::{
    CallSignature, CallingPolicy, LegalizedScalarReturnValue, LegalizedScalarTerminator,
    SelectedTerminator, StructuralTypeId, evaluate_call_plan,
};

#[test]
fn joined_structural_return_rejects_owner_declaration_and_abi_substitution() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let (abstracted, targeted, unit) =
            crate::tests::legalization::structural_case::fixture(native);
        let legal = crate::legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let mut source = legal.plan().scalar_functions[0].clone();
        let LegalizedScalarTerminator::StructuralCase {
            source: subject,
            layout,
            ..
        } = &source.blocks[0].terminator
        else {
            panic!("case source");
        };
        let shape = layout.shape;
        let declaration = terminal_psi::StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(99).unwrap(),
            position: 0,
            is_self: false,
            structural_type: subject.structural_type(),
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let block = source.blocks[1].id;
        source.blocks[1]
            .structural_parameters
            .push(declaration.clone());
        source.structural.as_mut().unwrap().result =
            Some(terminal_psi::StructuralResultDeclaration {
                reference_sources: Vec::new(),
                place: semantic_vocabulary::PlaceId::new(100).unwrap(),
                structural_type: declaration.structural_type,
                multiplicity: declaration.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        source.call_plan.result = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: Vec::new(),
                result: Some(shape),
            },
        )
        .unwrap()
        .result;
        let returned = LegalizedScalarReturnValue::Structural {
            source: legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration,
            },
        };
        let accepted =
            crate::selection::aggregate_result_input::returned(&source, &returned).unwrap();
        assert_eq!(
            accepted.0,
            selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                block,
                place: semantic_vocabulary::PlaceId::new(99).unwrap(),
            }
        );
        for mutation in [
            "owner",
            "place",
            "position",
            "access",
            "multiplicity",
            "type",
            "abi",
        ] {
            let mut changed_source = source.clone();
            let mut changed = returned.clone();
            let LegalizedScalarReturnValue::Structural {
                source:
                    legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                        block,
                        declaration,
                    },
            } = &mut changed
            else {
                panic!("block owner");
            };
            match mutation {
                "owner" => *block = changed_source.blocks[0].id,
                "place" => declaration.place = semantic_vocabulary::PlaceId::new(98).unwrap(),
                "position" => declaration.position += 1,
                "access" => declaration.access = terminal_psi::StructuralAccess::SharedBorrow,
                "multiplicity" => {
                    declaration.multiplicity = terminal_psi::StructuralMultiplicity::Linear
                }
                "type" => declaration.structural_type = StructuralTypeId::new(98).unwrap(),
                "abi" => {
                    changed_source
                        .call_plan
                        .result
                        .as_mut()
                        .unwrap()
                        .shape
                        .byte_size += 1
                }
                _ => unreachable!(),
            }
            assert!(
                crate::selection::aggregate_result_input::returned(&changed_source, &changed)
                    .is_none(),
                "accepted {mutation}"
            );
        }
    }
}

/// A case over the function's own owned sum parameter legalizes to the
/// parameter owner and selects from the entry-retained parameter slot. The
/// receiving replays reject re-rooting it at a different owner.
#[test]
fn parameter_rooted_case_dispatches_from_the_entry_retained_parameter_slot() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, targeted, unit) =
            crate::tests::legalization::structural_case::parameter_fixture(native);
        let place = semantic_vocabulary::PlaceId::new(1).unwrap();
        let legal = crate::legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let LegalizedScalarTerminator::StructuralCase {
            source: subject, ..
        } = &legal.plan().scalar_functions[0].blocks[0].terminator
        else {
            panic!("case source");
        };
        assert_eq!(
            subject,
            &legalized_operations::LegalizedStructuralCaseSource::Parameter {
                declaration: abstracted.functions[0].structural_parameters[0].clone(),
            }
        );
        // The receiving replay derives the owner itself: a block-arrival or
        // borrowed claim over the same place is not this parameter.
        for mutation in ["block owner", "borrowed"] {
            let mut forged = legal.plan().clone();
            let entry_block = forged.scalar_functions[0].blocks[0].id;
            let LegalizedScalarTerminator::StructuralCase { source, .. } =
                &mut forged.scalar_functions[0].blocks[0].terminator
            else {
                panic!("case source");
            };
            let legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration } =
                source.clone()
            else {
                panic!("parameter source");
            };
            *source = match mutation {
                "block owner" => {
                    legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                        block: entry_block,
                        declaration,
                    }
                }
                _ => legalized_operations::LegalizedStructuralCaseSource::Parameter {
                    declaration: terminal_psi::StructuralParameterDeclaration {
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        ..declaration
                    },
                },
            };
            assert!(
                crate::validate_legalized_operations(&targeted, &abstracted, &unit, forged)
                    .is_err(),
                "accepted {mutation}"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let function = &selected.plan().functions[0];
        let slot = selected_instructions::LocalStorageSlotId::StructuralParameter { place };
        assert_eq!(
            function
                .local_storage_slots
                .iter()
                .filter(|candidate| candidate.id == slot)
                .count(),
            1
        );
        let entry = function
            .blocks
            .iter()
            .find(|block| block.id == function.entry_block)
            .unwrap();
        let SelectedTerminator::ConditionalBranch {
            when_zero,
            when_nonzero,
            ..
        } = &entry.terminator
        else {
            panic!("case branch")
        };
        for successor in [when_zero, when_nonzero] {
            assert_eq!(
                successor.structural_case.as_ref().unwrap().source,
                selected_instructions::SelectedCaseDispatchSource::Local { slot }
            );
        }
    }
}

/// A case over the function's own owned mixed parameter legalizes to the
/// parameter owner and selects from the entry-retained parameter slot, whose
/// layout spans the common fields alongside the dispatched alternatives.
#[test]
fn mixed_parameter_rooted_case_dispatches_from_the_entry_retained_parameter_slot() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let (abstracted, targeted, unit) =
            crate::tests::legalization::structural_case::mixed_parameter_fixture(native);
        let place = semantic_vocabulary::PlaceId::new(1).unwrap();
        let legal = crate::legalize_target_operations(&targeted, &abstracted, &unit).unwrap();
        let LegalizedScalarTerminator::StructuralCase {
            source: subject, ..
        } = &legal.plan().scalar_functions[0].blocks[0].terminator
        else {
            panic!("case source");
        };
        assert_eq!(
            subject,
            &legalized_operations::LegalizedStructuralCaseSource::Parameter {
                declaration: abstracted.functions[0].structural_parameters[0].clone(),
            }
        );
        for mutation in ["block owner", "borrowed"] {
            let mut forged = legal.plan().clone();
            let entry_block = forged.scalar_functions[0].blocks[0].id;
            let LegalizedScalarTerminator::StructuralCase { source, .. } =
                &mut forged.scalar_functions[0].blocks[0].terminator
            else {
                panic!("case source");
            };
            let legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration } =
                source.clone()
            else {
                panic!("parameter source");
            };
            *source = match mutation {
                "block owner" => {
                    legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                        block: entry_block,
                        declaration,
                    }
                }
                _ => legalized_operations::LegalizedStructuralCaseSource::Parameter {
                    declaration: terminal_psi::StructuralParameterDeclaration {
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        ..declaration
                    },
                },
            };
            assert!(
                crate::validate_legalized_operations(&targeted, &abstracted, &unit, forged)
                    .is_err(),
                "accepted {mutation}"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let function = &selected.plan().functions[0];
        let slot = selected_instructions::LocalStorageSlotId::StructuralParameter { place };
        assert_eq!(
            function
                .local_storage_slots
                .iter()
                .filter(|candidate| candidate.id == slot)
                .count(),
            1
        );
        let entry = function
            .blocks
            .iter()
            .find(|block| block.id == function.entry_block)
            .unwrap();
        let SelectedTerminator::ConditionalBranch {
            when_zero,
            when_nonzero,
            ..
        } = &entry.terminator
        else {
            panic!("case branch")
        };
        for successor in [when_zero, when_nonzero] {
            assert_eq!(
                successor.structural_case.as_ref().unwrap().source,
                selected_instructions::SelectedCaseDispatchSource::Local { slot }
            );
        }
    }
}
