//! Owned arrival ABI and source-custody controls over the shared scalar graph fixture.
use super::*;

fn owned_fixture(
    native: ::target::NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    use terminal_psi::{
        BindingRelevance, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
        StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    };
    let (mut plan, _, _) = fixture(native);
    let identity = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
    let place = |ordinal| semantic_vocabulary::PlaceId::new(ordinal).unwrap();
    plan.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: identity,
            identity: "test::Payload".into(),
            shape: StructuralTypeShape::Record {
                fields: (1..=2)
                    .map(|ordinal| StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(ordinal).unwrap(),
                        identity: format!("test::Payload::field{ordinal}"),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(u64_type())),
                    })
                    .collect(),
            },
        });
    let declaration = |ordinal| StructuralParameterDeclaration {
        place: place(ordinal),
        position: 0,
        is_self: false,
        structural_type: identity,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let function = &mut plan.functions[0];
    // This raw fixture tests ownership/ABI correspondence, not arithmetic proof
    // admission. Preserve scalar dependencies with total identity calls; the
    // source-produced countdown exercises verified decrement evidence.
    for (position, ordinal, result, argument) in [(5, 4, 6, 2), (7, 6, 8, 7)] {
        function.operations[position] = AbstractOperation::Call {
            psi_operation: operation(ordinal),
            result: value(result),
            scalar_type: ScalarType::Integer(u64_type()),
            callee: MachineId::new(2).unwrap(),
            arguments: vec![value(argument)],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
    }
    function.structural_parameters.push(declaration(1));
    function.block_entries[1]
        .structural_parameters
        .push(declaration(2));
    for (position, argument) in [(0, 1), (8, 2)] {
        let AbstractOperation::Jump {
            structural_bindings,
            ..
        } = &mut function.operations[position]
        else {
            panic!("jump");
        };
        structural_bindings.push(abstract_operations::AbstractStructuralBinding {
            parameter: place(2),
            argument: StructuralArgument {
                place: place(argument),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        });
    }
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut function.operations[9]
    else {
        panic!("return");
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place(2)));
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (plan, target, unit)
}

#[test]
fn unobserved_owned_arrivals_keep_value_abi_and_scalar_return_cleanup() {
    for native in [
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::linux_arm64(),
        ::target::NativeTarget::macos_arm64(),
        ::target::NativeTarget::windows_x64(),
    ] {
        let (plan, target, unit) = owned_fixture(native);
        check(&plan, &target, &unit).unwrap();
        crate::legalization::scalar_graph_input::match_input(
            &target.functions[0],
            &plan.functions[0],
            &unit.functions[0],
            &target,
            &plan,
            &unit,
        )
        .unwrap();
        let abi = target.functions[0]
            .mixed_structural_scalar_abi
            .as_ref()
            .unwrap();
        assert_eq!(
            abi.structural_parameters[0].shape,
            ValueShape::integer(16, 8)
        );
        assert_eq!(
            abi.structural_parameters[0].access,
            terminal_psi::StructuralAccess::Owned
        );
        let graph = &target.functions[0].graph;
        assert_eq!(
            graph.blocks[1].structural_parameters,
            plan.functions[0].block_entries[1].structural_parameters
        );
        assert!(
            matches!(&graph.blocks[3].terminator, TargetControlTerminator::ReturnScalar { cleanup_actions, .. } if cleanup_actions.len() == 1)
        );
    }
}

#[test]
fn unobserved_owned_arrivals_reject_substituted_bindings_cleanup_and_abi() {
    let (plan, target, unit) = owned_fixture(::target::NativeTarget::macos_arm64());
    for mutation in [
        "borrow",
        "missing binding",
        "missing cleanup",
        "foreign cleanup",
        "shape",
    ] {
        let mut changed = target.clone();
        let graph = &mut changed.functions[0].graph;
        match mutation {
            "borrow" | "missing binding" => {
                let TargetControlTerminator::Jump { successor } = &mut graph.blocks[0].terminator
                else {
                    panic!("jump");
                };
                if mutation == "borrow" {
                    successor.structural_bindings[0].argument.access =
                        terminal_psi::StructuralAccess::SharedBorrow;
                } else {
                    successor.structural_bindings.clear();
                }
            }
            "missing cleanup" | "foreign cleanup" => {
                let TargetControlTerminator::ReturnScalar {
                    cleanup_actions, ..
                } = &mut graph.blocks[3].terminator
                else {
                    panic!("return");
                };
                if mutation == "missing cleanup" {
                    cleanup_actions.clear();
                } else {
                    cleanup_actions[0] = terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                        semantic_vocabulary::PlaceId::new(1).unwrap(),
                    );
                }
            }
            "shape" => graph.parameters[0].shape = ValueShape::borrowed_reference(16, 8),
            _ => unreachable!(),
        }
        assert!(check(&plan, &changed, &unit).is_err(), "{mutation}");
    }
}

#[test]
fn owned_arrival_field_read_requires_the_matching_native_graph() {
    let native = ::target::NativeTarget::macos_arm64();
    let (mut plan, target, _) = owned_fixture(native);
    let observation = AbstractOperation::IntegerStructuralField {
        psi_operation: operation(1),
        result: AbstractResult {
            value: value(3),
            scalar_type: ScalarType::Integer(u64_type()),
        },
        source: plan.functions[0].block_entries[1].structural_parameters[0].place,
        field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
    };
    plan.functions[0].operations[1] = observation;
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let observed_target =
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .expect("an established owned arrival supports an exact field read");
    crate::legalization::scalar_graph_input::match_input(
        &observed_target.functions[0],
        &plan.functions[0],
        &unit.functions[0],
        &observed_target,
        &plan,
        &unit,
    )
    .expect("the current graph retains the field observation");
    // The formerly unobserved graph cannot stand in for the changed program.
    assert!(
        crate::legalization::scalar_graph_input::match_input(
            &target.functions[0],
            &plan.functions[0],
            &unit.functions[0],
            &target,
            &plan,
            &unit
        )
        .is_err()
    );
}

#[test]
fn unobserved_owned_unrestricted_arrivals_keep_abi_without_affine_disposal() {
    let native = ::target::NativeTarget::macos_arm64();
    let (mut plan, _, _) = owned_fixture(native);
    let function = &mut plan.functions[0];
    for parameter in function.structural_parameters.iter_mut().chain(
        function
            .block_entries
            .iter_mut()
            .flat_map(|block| &mut block.structural_parameters),
    ) {
        parameter.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
    }
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut function.operations[9]
    else {
        panic!("return");
    };
    cleanup_actions.clear();
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    check(&plan, &target, &unit).unwrap();
    crate::legalization::scalar_graph_input::match_input(
        &target.functions[0],
        &plan.functions[0],
        &unit.functions[0],
        &target,
        &plan,
        &unit,
    )
    .unwrap();
    let abi = target.functions[0]
        .mixed_structural_scalar_abi
        .as_ref()
        .unwrap();
    assert_eq!(
        abi.structural_parameters[0].multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
    );
    assert_eq!(
        abi.structural_parameters[0].shape,
        ValueShape::integer(16, 8)
    );
    for parameter in plan.functions[0].structural_parameters.iter_mut() {
        parameter.multiplicity = terminal_psi::StructuralMultiplicity::Linear;
    }
    assert!(
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .is_err()
    );
}

#[test]
fn unobserved_owned_arrivals_do_not_supply_missing_arithmetic_authority() {
    let native = ::target::NativeTarget::macos_arm64();
    let (mut plan, _, _) = owned_fixture(native);
    plan.functions[0].operations[5] = AbstractOperation::ExactIntegerSubtract {
        psi_operation: operation(4),
        result: value(6),
        scalar_type: u64_type(),
        left: value(2),
        right: value(4),
        obligation: ObligationId::new(1).unwrap(),
    };
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    assert!(unit.accepted_obligation_facts.is_empty());
    check(&plan, &target, &unit).unwrap();
    assert!(
        crate::legalization::scalar_graph_input::match_input(
            &target.functions[0],
            &plan.functions[0],
            &unit.functions[0],
            &target,
            &plan,
            &unit
        )
        .is_err()
    );
}
