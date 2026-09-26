//! Borrowed `{instance, table}` descriptor parameters lower to ordinary
//! two-word ABI rows and graph-level indirect-call operations. Independent
//! validation replays the descriptor signature, the parameter pass-through
//! custody, and the requirement slot identity from the source declarations.

use super::structural_borrows::source_plan;
use super::{AbstractOperation, AbstractOperationPlan, AbstractResult, NativeTarget, OperationId};
use calling_conventions::{CallPlan, ValueClass, ValueShape};
use semantic_vocabulary::{MachineId, ValueId};
use target_operations::{
    TargetControlSuccessor, TargetControlTerminator, TargetDynamicDescriptorArgument,
    TargetDynamicDescriptorInstanceSource, TargetDynamicDescriptorParameterAbi, TargetFunction,
    TargetOperationPlan, TargetUnitOperation, TargetUnitScalarHomeRequirement,
};

/// The customer shape: an entry that selects one concrete erased descriptor, a
/// helper that forwards its own borrowed descriptor parameter once, and a
/// helper that invokes the descriptor requirement and returns its result.
fn forwarded_parameter_source() -> AbstractOperationPlan {
    source_plan(
        r#"
            trait Shape { machine code(&self) -> i32; }
            data Item { value: i32; }
            Primary: Item satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> self.value } }
            }
            data Main { item: Item; }
            machine Main::run(&self) {
                let erased: &dyn Shape = &self.item as &dyn Item::Primary;
                let result: i32 = dispatch(erased);
            }
            machine dispatch(erased: &dyn Shape) -> i32 {
                let result: i32 = finish(erased);
                transition { _ -> result }
            }
            machine finish(erased: &dyn Shape) -> i32 {
                let result: i32 = erased.code();
                transition { _ -> result }
            }
        "#,
    )
}

/// The unit form: a borrowed descriptor parameter forwarded through two
/// helpers to a unit requirement invocation.
fn forwarded_unit_parameter_source() -> AbstractOperationPlan {
    source_plan(
        r#"
            trait Action { machine act(&self); }
            data Item { value: i32; }
            machine Item::act(&self) { }
            Primary: Item satisfies Action {
                Action::act = Item::act;
            }
            data Main { item: Item; }
            machine Main::run(&self) {
                let erased: &dyn Action = &self.item as &dyn Item::Primary;
                dispatch(erased);
            }
            machine dispatch(erased: &dyn Action) {
                finish(erased);
            }
            machine finish(erased: &dyn Action) {
                erased.act();
            }
        "#,
    )
}

fn lowered(native: NativeTarget, source: &AbstractOperationPlan) -> TargetOperationPlan {
    crate::lower_to_target_operations(source, crate::TargetLoweringRequest::new(native))
        .expect("descriptor parameter source lowers")
}

/// The helper that invokes `erased.code()` — the only function retaining a
/// `DynamicParameterScalarCall` row.
fn dispatching_function(plan: &TargetOperationPlan) -> &TargetFunction {
    plan.functions
        .iter()
        .find(|function| {
            function
                .graph
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::DynamicParameterScalarCall { .. }
                    )
                })
        })
        .expect("parameter dispatch function")
}

fn dispatching_function_mut(plan: &mut TargetOperationPlan) -> &mut TargetFunction {
    plan.functions
        .iter_mut()
        .find(|function| {
            function
                .graph
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::DynamicParameterScalarCall { .. }
                    )
                })
        })
        .expect("parameter dispatch function")
}

/// The helper that forwards its own descriptor parameter — the only function
/// retaining a `Parameter`-sourced dynamic argument.
fn forwarding_function(plan: &TargetOperationPlan) -> &TargetFunction {
    plan.functions
        .iter()
        .find(|function| {
            function
                .graph
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| match operation {
                    TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                        dynamic_arguments,
                        ..
                    } => dynamic_arguments.iter().any(|argument| {
                        matches!(
                            argument.instance,
                            TargetDynamicDescriptorInstanceSource::Parameter { .. }
                        )
                    }),
                    _ => false,
                })
        })
        .expect("parameter forwarding function")
}

fn only_dynamic_parameter(function: &TargetFunction) -> &TargetDynamicDescriptorParameterAbi {
    let [row] = function.graph.dynamic_parameters.as_slice() else {
        panic!(
            "expected exactly one descriptor parameter row on {:?}",
            function.machine
        )
    };
    row
}

#[allow(clippy::type_complexity)]
fn parameter_dispatch_row(
    function: &TargetFunction,
) -> (
    &AbstractResult,
    &abstract_operations::AbstractParameterDynamicDispatch,
    &TargetDynamicDescriptorParameterAbi,
    &terminal_psi::TerminalDynamicRequirement,
    &CallPlan,
    u32,
    &TargetUnitScalarHomeRequirement,
) {
    function
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation {
            TargetUnitOperation::DynamicParameterScalarCall {
                result,
                dynamic_dispatch,
                parameter_abi,
                requirement,
                dispatch_call_plan,
                table_slot_byte_offset,
                result_home,
                ..
            } => Some((
                result,
                dynamic_dispatch,
                parameter_abi,
                requirement,
                dispatch_call_plan,
                *table_slot_byte_offset,
                result_home,
            )),
            _ => None,
        })
        .expect("parameter dispatch call row")
}

fn forwarded_argument(function: &TargetFunction) -> (&CallPlan, &TargetDynamicDescriptorArgument) {
    function
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation {
            TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                call_plan,
                dynamic_arguments,
                ..
            } => Some((call_plan, dynamic_arguments.first()?)),
            _ => None,
        })
        .expect("forwarding call row")
}

#[test]
fn descriptor_parameter_signature_binds_two_trailing_pointer_words() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = forwarded_parameter_source();
        let target = lowered(native, &source);
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();

        let finish = dispatching_function(&target);
        let abi = only_dynamic_parameter(finish);
        let pointer = u16::try_from(native.pointer_size).unwrap();
        for placement in [&abi.instance, &abi.table] {
            assert_eq!(placement.shape.class, ValueClass::Integer);
            assert_eq!(placement.shape.byte_size, pointer);
            assert_eq!(placement.shape.alignment, pointer);
        }
        assert_ne!(
            abi.instance.locations, abi.table.locations,
            "descriptor words occupy distinct placements"
        );
        // The descriptor words are the complete trailing parameter lane.
        assert_eq!(
            finish.graph.call_plan.parameters.as_slice(),
            &[abi.instance.clone(), abi.table.clone()]
        );
        assert_eq!(
            abi.parameter.trait_identity, "Shape",
            "semantic declaration retained on the ABI row"
        );
        assert_eq!(abi.parameter.ordinal, 0);
        assert_eq!(
            abi.parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
        );
    }
}

#[test]
fn parameter_dispatch_row_replays_requirement_slot_and_result_home() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = forwarded_parameter_source();
        let target = lowered(native, &source);
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();

        let finish = dispatching_function(&target);
        let abi = only_dynamic_parameter(finish).clone();
        let (result, dispatch, parameter_abi, requirement, plan, slot_offset, home) =
            parameter_dispatch_row(finish);
        assert_eq!(*parameter_abi, abi, "operation keeps the exact ABI row");
        assert_eq!(dispatch.parameter, abi.parameter);
        assert_eq!(dispatch.dispatch.owner, finish.machine);
        assert_eq!(dispatch.dispatch.parameter_ordinal, 0);
        assert_eq!(dispatch.dispatch.requirement_slot, 0);
        assert_eq!(requirement.slot, 0);
        assert_eq!(
            requirement.result,
            terminal_psi::ClosedConformanceCallableResult::I32
        );
        assert_eq!(slot_offset, 0, "first requirement occupies table slot zero");
        // The indirect call ABI is exactly `code(&self) -> i32`: one pointer
        // receiver word in, one i32 result out.
        let pointer = u16::try_from(native.pointer_size).unwrap();
        assert_eq!(plan.parameters.len(), 1);
        assert_eq!(plan.parameters[0].shape.byte_size, pointer);
        assert_eq!(
            plan.result.as_ref().map(|placement| placement.shape),
            Some(ValueShape {
                class: ValueClass::Integer,
                byte_size: 4,
                alignment: 4
            })
        );
        assert_eq!(home.source_value, result.value);
        assert_eq!(home.scalar_type, result.scalar_type);
        // The result home is a live scalar definition the block terminator
        // consumes as the function's returned value. Return values cross block
        // boundaries through explicit edge bindings, so the return's source is
        // a block parameter whose argument chain reaches the result home.
        let mut bound_source = std::collections::BTreeMap::new();
        for block in &finish.graph.blocks {
            let successors: Vec<&TargetControlSuccessor> = match &block.terminator {
                TargetControlTerminator::Jump { successor } => vec![successor],
                TargetControlTerminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true, when_false],
                _ => Vec::new(),
            };
            for successor in successors {
                for binding in &successor.bindings {
                    bound_source.insert(binding.parameter, binding.argument);
                }
            }
        }
        let returns_result = finish.graph.blocks.iter().any(|block| {
            let TargetControlTerminator::ReturnScalar { source_value, .. } = block.terminator
            else {
                return false;
            };
            let mut returned = source_value;
            loop {
                if returned == home.source_value {
                    return true;
                }
                match bound_source.get(&returned) {
                    Some(argument) => returned = *argument,
                    None => return false,
                }
            }
        });
        assert!(returns_result, "dispatch result home feeds the return edge");
    }
}

#[test]
fn parameter_forwarding_reuses_caller_words_without_projection() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = forwarded_parameter_source();
        let target = lowered(native, &source);
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();

        let finish = dispatching_function(&target);
        let dispatch = forwarding_function(&target);
        let caller_abi = only_dynamic_parameter(dispatch).clone();
        let (call_plan, argument) = forwarded_argument(dispatch);
        let TargetUnitOperation::StructuralScalarCallWithDynamicArguments { callee, .. } = dispatch
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| {
                matches!(
                    operation,
                    TargetUnitOperation::StructuralScalarCallWithDynamicArguments { .. }
                )
            })
            .expect("forwarding call row")
        else {
            unreachable!()
        };
        assert_eq!(*callee, finish.machine);
        assert_eq!(
            argument.custody.argument.source,
            terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 }
        );
        let TargetDynamicDescriptorInstanceSource::Parameter {
            parameter,
            destination,
        } = &argument.instance
        else {
            panic!("forwarded descriptor instance must pass the caller word through")
        };
        // Both caller words pass through unchanged into the callee's lane.
        assert_eq!(*parameter, caller_abi);
        assert_eq!(*destination, call_plan.parameters[0]);
        assert_eq!(argument.table_destination, call_plan.parameters[1]);
        assert_ne!(
            call_plan.parameters[0], call_plan.parameters[1],
            "instance and table occupy distinct callee placements"
        );

        // The entry still supplies a concrete projected selection.
        let entry = target
            .functions
            .iter()
            .find(|function| function.machine == target.entry)
            .expect("entry function");
        assert!(entry.graph.dynamic_parameters.is_empty());
        let (_, projected) = forwarded_argument(entry);
        assert!(matches!(
            projected.instance,
            TargetDynamicDescriptorInstanceSource::Projection(_)
        ));
    }
}

#[allow(clippy::type_complexity)]
fn mutate_dispatch_row(
    target: &mut TargetOperationPlan,
    f: impl FnOnce(
        &mut abstract_operations::AbstractParameterDynamicDispatch,
        &mut TargetDynamicDescriptorParameterAbi,
        &mut terminal_psi::TerminalDynamicRequirement,
        &mut CallPlan,
        &mut u32,
        &mut TargetUnitScalarHomeRequirement,
    ),
) {
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let TargetUnitOperation::DynamicParameterScalarCall {
                    dynamic_dispatch,
                    parameter_abi,
                    requirement,
                    dispatch_call_plan,
                    table_slot_byte_offset,
                    result_home,
                    ..
                } = operation
                else {
                    continue;
                };
                f(
                    dynamic_dispatch,
                    parameter_abi,
                    requirement,
                    dispatch_call_plan,
                    table_slot_byte_offset,
                    result_home,
                );
                return;
            }
        }
    }
    panic!("no parameter dispatch row to mutate");
}

fn mutate_forwarded_argument(
    target: &mut TargetOperationPlan,
    f: impl FnOnce(&mut TargetDynamicDescriptorArgument),
) {
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            for operation in &mut block.operations {
                let TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                    dynamic_arguments,
                    ..
                } = operation
                else {
                    continue;
                };
                if let Some(argument) = dynamic_arguments.iter_mut().find(|argument| {
                    matches!(
                        argument.instance,
                        TargetDynamicDescriptorInstanceSource::Parameter { .. }
                    )
                }) {
                    f(argument);
                    return;
                }
            }
        }
    }
    panic!("no forwarded descriptor argument to mutate");
}

#[test]
fn parameter_dispatch_rejects_substituted_slot_plan_and_result() {
    let source = forwarded_parameter_source();
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target = lowered(native, &source);
        let mutations: Vec<Box<dyn Fn(&mut TargetOperationPlan)>> = vec![
            // Requirement slot substitution.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, _, requirement, _, _, _| requirement.slot = 1);
            }),
            // Wrong table slot byte offset for the same slot.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, _, _, _, offset, _| *offset = 8);
            }),
            // Caller parameter ordinal substitution in the dispatch row.
            Box::new(|target| {
                mutate_dispatch_row(target, |dispatch, _, _, _, _, _| {
                    dispatch.dispatch.parameter_ordinal = 1
                });
            }),
            // Dispatch owner substitution.
            Box::new(|target| {
                mutate_dispatch_row(target, |dispatch, _, _, _, _, _| {
                    dispatch.dispatch.owner = MachineId::new(1).unwrap()
                });
            }),
            // Access substitution on the bound descriptor declaration.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, abi, _, _, _, _| {
                    abi.parameter.access = terminal_psi::StructuralAccess::Owned
                });
            }),
            // Instance word substitution inside the operation's ABI row.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, abi, _, _, _, _| abi.instance.locations.clear());
            }),
            // A different requirement result type under the same slot.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, _, requirement, _, _, _| {
                    requirement.result = terminal_psi::ClosedConformanceCallableResult::Unit
                });
            }),
            // Dispatch call plan with the receiver word dropped.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, _, _, plan, _, _| plan.parameters.clear());
            }),
            // Scalar result shape substitution.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, _, _, _, _, home| {
                    home.shape.byte_size = 8;
                    home.shape.alignment = 8;
                });
            }),
            // Result home rebound to an unrelated source value.
            Box::new(|target| {
                mutate_dispatch_row(target, |_, _, _, _, _, home| {
                    home.source_value = ValueId::new(90).unwrap()
                });
            }),
        ];
        for (index, mutation) in mutations.iter().enumerate() {
            let mut forged = target.clone();
            mutation(&mut forged);
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &forged).is_err(),
                "substitution {index} must fail validation"
            );
        }
    }
}

#[test]
fn parameter_forwarding_rejects_substituted_words_and_custody() {
    let source = forwarded_parameter_source();
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target = lowered(native, &source);
        let mutations: Vec<Box<dyn Fn(&mut TargetOperationPlan)>> = vec![
            // The instance destination drifts off the callee's first word.
            Box::new(|target| {
                mutate_forwarded_argument(target, |argument| {
                    let TargetDynamicDescriptorInstanceSource::Parameter { destination, .. } =
                        &mut argument.instance
                    else {
                        panic!("parameter source")
                    };
                    destination.locations.clear();
                });
            }),
            // The caller parameter row is replaced by the callee's own row.
            Box::new(|target| {
                let finish_abi = only_dynamic_parameter(dispatching_function(target)).clone();
                mutate_forwarded_argument(target, |argument| {
                    let TargetDynamicDescriptorInstanceSource::Parameter { parameter, .. } =
                        &mut argument.instance
                    else {
                        panic!("parameter source")
                    };
                    *parameter = finish_abi;
                });
            }),
            // Semantic custody claims a projected selection instead of the
            // parameter pass-through.
            Box::new(|target| {
                mutate_forwarded_argument(target, |argument| {
                    argument.custody.argument.source =
                        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 };
                });
            }),
            // Access substitution on the callee-side custody row.
            Box::new(|target| {
                mutate_forwarded_argument(target, |argument| {
                    argument.custody.target.access = terminal_psi::StructuralAccess::Owned;
                });
            }),
            // Trait identity substitution on the callee-side custody row.
            Box::new(|target| {
                mutate_forwarded_argument(target, |argument| {
                    argument.custody.target.trait_identity = "Other".into();
                });
            }),
        ];
        for (index, mutation) in mutations.iter().enumerate() {
            let mut forged = target.clone();
            mutation(&mut forged);
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &forged).is_err(),
                "substitution {index} must fail validation"
            );
        }
    }
}

#[test]
fn signature_replay_rejects_mutated_descriptor_parameter_roster() {
    let source = forwarded_parameter_source();
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target = lowered(native, &source);
        // Ordinal substitution inside the signature roster.
        let mut forged = target.clone();
        dispatching_function_mut(&mut forged)
            .graph
            .dynamic_parameters[0]
            .parameter
            .ordinal = 1;
        assert!(matches!(
            crate::validate_abstract_to_target_translation(&source, native, &forged),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralSignatureMismatch { .. }
            )
        ));
        // Instance/table word swap inside the signature roster.
        let mut forged = target.clone();
        let row = &mut dispatching_function_mut(&mut forged)
            .graph
            .dynamic_parameters[0];
        std::mem::swap(&mut row.instance, &mut row.table);
        assert!(matches!(
            crate::validate_abstract_to_target_translation(&source, native, &forged),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralSignatureMismatch { .. }
            )
        ));
        // Dropping the roster row removes the descriptor lane entirely.
        let mut forged = target.clone();
        dispatching_function_mut(&mut forged)
            .graph
            .dynamic_parameters
            .clear();
        assert!(matches!(
            crate::validate_abstract_to_target_translation(&source, native, &forged),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralSignatureMismatch { .. }
            )
        ));
    }
}

#[test]
fn forged_dispatch_row_rejects_unrelated_operation_identity() {
    let source = forwarded_parameter_source();
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let mut forged = lowered(native, &source);
        // Retarget the dispatch row at a psi operation that is not the source
        // `erased.code()` operation.
        let mut rebound = false;
        for function in &mut forged.functions {
            for block in &mut function.graph.blocks {
                for operation in &mut block.operations {
                    if let TargetUnitOperation::DynamicParameterScalarCall {
                        psi_operation, ..
                    } = operation
                    {
                        *psi_operation = OperationId::new(77).unwrap();
                        rebound = true;
                    }
                }
            }
        }
        assert!(rebound, "dispatch row found for rebinding");
        assert!(crate::validate_abstract_to_target_translation(&source, native, &forged).is_err());
    }
}

#[test]
fn unit_parameter_dispatch_lowers_indirect_call_and_validates() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = forwarded_unit_parameter_source();
        let target = lowered(native, &source);
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();

        // `finish` invokes the parameter requirement through the unit row.
        let finish = target
            .functions
            .iter()
            .find(|function| {
                function
                    .graph
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        matches!(
                            operation,
                            TargetUnitOperation::DynamicParameterUnitCall { .. }
                        )
                    })
            })
            .expect("unit parameter dispatch function");
        let abi = only_dynamic_parameter(finish);
        let (
            dynamic_dispatch,
            parameter_abi,
            requirement,
            dispatch_call_plan,
            table_slot_byte_offset,
        ) = finish
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::DynamicParameterUnitCall {
                    dynamic_dispatch,
                    parameter_abi,
                    requirement,
                    dispatch_call_plan,
                    table_slot_byte_offset,
                    ..
                } => Some((
                    dynamic_dispatch,
                    parameter_abi,
                    requirement,
                    dispatch_call_plan,
                    *table_slot_byte_offset,
                )),
                _ => None,
            })
            .expect("unit parameter dispatch row");
        assert_eq!(*parameter_abi, *abi);
        assert_eq!(dynamic_dispatch.parameter, abi.parameter);
        assert_eq!(dynamic_dispatch.dispatch.owner, finish.machine);
        assert_eq!(dynamic_dispatch.dispatch.requirement_slot, 0);
        assert_eq!(requirement.slot, 0);
        assert_eq!(
            requirement.result,
            terminal_psi::ClosedConformanceCallableResult::Unit
        );
        assert_eq!(table_slot_byte_offset, 0);
        let pointer = u16::try_from(native.pointer_size).unwrap();
        assert_eq!(dispatch_call_plan.parameters.len(), 1);
        assert_eq!(dispatch_call_plan.parameters[0].shape.byte_size, pointer);
        assert!(
            dispatch_call_plan.result.is_none(),
            "unit call has no result"
        );

        // `dispatch` forwards its own parameter words to `finish`.
        let dispatch = target
            .functions
            .iter()
            .find(|function| {
                function
                    .graph
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| match operation {
                        TargetUnitOperation::StructuralUnitCallWithDynamicArguments {
                            dynamic_arguments,
                            ..
                        } => dynamic_arguments.iter().any(|argument| {
                            matches!(
                                argument.instance,
                                TargetDynamicDescriptorInstanceSource::Parameter { .. }
                            )
                        }),
                        _ => false,
                    })
            })
            .expect("unit forwarding function");
        let caller_abi = only_dynamic_parameter(dispatch);
        let (call_plan, argument) = dispatch
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::StructuralUnitCallWithDynamicArguments {
                    call_plan,
                    dynamic_arguments,
                    ..
                } => Some((call_plan, dynamic_arguments.first()?)),
                _ => None,
            })
            .expect("unit forwarding call row");
        let TargetDynamicDescriptorInstanceSource::Parameter {
            parameter,
            destination,
        } = &argument.instance
        else {
            panic!("unit forwarder must pass its own instance word through")
        };
        assert_eq!(parameter, caller_abi);
        assert_eq!(*destination, call_plan.parameters[0]);
        assert_eq!(argument.table_destination, call_plan.parameters[1]);
    }
}

#[test]
fn malformed_source_descriptor_declarations_fail_lowering() {
    let source = forwarded_parameter_source();
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        // A descriptor parameter ordinal that no longer matches the dense
        // zero-based lane.
        let mut drifted = source.clone();
        for function in &mut drifted.functions {
            for operation in &mut function.operations {
                if let AbstractOperation::DynamicDescriptorParameter { parameter } = operation {
                    parameter.ordinal = 3;
                }
            }
        }
        assert!(
            crate::lower_to_target_operations(&drifted, crate::TargetLoweringRequest::new(native))
                .is_err()
        );

        // Ownership substitution on the declared descriptor parameter.
        let mut owned = source.clone();
        for function in &mut owned.functions {
            for operation in &mut function.operations {
                if let AbstractOperation::DynamicDescriptorParameter { parameter } = operation {
                    parameter.access = terminal_psi::StructuralAccess::Owned;
                }
            }
        }
        assert!(
            crate::lower_to_target_operations(&owned, crate::TargetLoweringRequest::new(native))
                .is_err()
        );

        // A requirement slot the descriptor declaration does not carry.
        let mut slot = source.clone();
        for function in &mut slot.functions {
            for operation in &mut function.operations {
                if let AbstractOperation::CallDynamicParameterScalar {
                    dynamic_dispatch, ..
                } = operation
                {
                    dynamic_dispatch.dispatch.requirement_slot = 9;
                }
            }
        }
        assert!(
            crate::lower_to_target_operations(&slot, crate::TargetLoweringRequest::new(native))
                .is_err()
        );

        // Dropping the parameter declaration strands the dispatch custody.
        let mut stranded = source.clone();
        for function in &mut stranded.functions {
            function.operations.retain(|operation| {
                !matches!(
                    operation,
                    AbstractOperation::DynamicDescriptorParameter { .. }
                )
            });
        }
        assert!(
            crate::lower_to_target_operations(&stranded, crate::TargetLoweringRequest::new(native))
                .is_err()
        );
    }
}
