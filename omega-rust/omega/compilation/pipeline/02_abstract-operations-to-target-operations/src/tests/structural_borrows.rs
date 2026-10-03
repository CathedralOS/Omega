//! Authored borrows retain reference ABI placement after canonical Psi checking.

mod byte_owner_views;
mod direct_call_operands;
mod mutable_view_arrivals;
mod nominal_cleanup;

use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, BlockId, CrashCause, EdgeId, IntegerSign,
    IntegerType, IntegerValue, ScalarType, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape, ValueId,
};
use abstract_operations::AbstractStructuralCallResult;
use calling_conventions::{ValueClass, ValueLocation};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use target::NativeTarget;
use target_operations::{
    TargetDynamicCallResult, TargetDynamicDispatch, TargetOperationPlan, TargetStructuralArgument,
    TargetUnitOperation,
};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi_to_abstract_operations::lower_artifact;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

pub(super) fn source_plan(source: &str) -> abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::run"),
    )
    .expect("lower source");
    lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &encode_module(&terminal.semantic_module).expect("encode semantics"),
            proof_bytes: &encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
                .expect("encode proof"),
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
    .expect("verify and lower canonical artifact")
}

#[test]
fn local_slice_record_copy_retains_view_identity_and_bound() {
    let source = source_plan(
        r#"
            data Tag [copy] { case First; case Second; }
            data Cell [copy] { tag: Tag; value: u64; }
            data Bag { cells: [Cell; 2]; }
            data Main {}
            machine Main::run(source: &Bag, destination: &mut Bag) {
                let input: &[Cell] = source.cells.as_slice();
                let output: &mut [Cell] = destination.cells.as_mut_slice();
                output[0] = input[1];
            }
        "#,
    );
    let (copy, copied_view, copied_path, copied_type) = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .find_map(|operation| match operation {
            AbstractOperation::StructuralLeafCopy {
                psi_operation,
                source,
                path,
                result,
            } => Some((
                *psi_operation,
                *source,
                path.clone(),
                result.structural_type,
            )),
            _ => None,
        })
        .expect("source-produced record snapshot");
    assert!(
        source
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .any(|operation| matches!(operation,
            AbstractOperation::EstablishElementView { result, source, .. }
                if result.place == copied_view && source.place != copied_view
                    && result.structural_type != copied_type))
    );
    let abstract_operations::StructuralCopyPath::Ordinary(path) = &copied_path else {
        panic!("ordinary copied element path");
    };
    assert!(matches!(
        path.as_slice(),
        [terminal_psi::StructuralPathSegment::RuntimeIndex { .. }]
    ));
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("copy through the established local view");
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("target retains structural source custody");
        let copied = target.functions.iter().flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation,
                TargetUnitOperation::StructuralLeafCopy { psi_operation, .. } if *psi_operation == copy))
            .expect("target copied element");
        let TargetUnitOperation::StructuralLeafCopy {
            source: actual_source,
            path,
            result_home,
            indices,
            ..
        } = copied
        else {
            unreachable!()
        };
        assert_eq!(*actual_source, copied_view);
        assert_eq!(*path, copied_path);
        assert_eq!(result_home.structural_type(), copied_type);
        assert_eq!(indices.len(), 1);
    }
}

#[test]
fn owned_state_argument_retains_destination_storage_and_rejects_substitution() {
    let source = source_plan(
        r#"
            data Amount [copy] { value: i32; }
            machine consume(amount: Amount) -> i32 { amount.value }
            data Main {}
            machine Main::run() -> i32 {
                let amount: Amount = Amount { value: 70 };
                transition { _ -> arrived(amount) }
                state arrived(amount: Amount) -> i32 { consume(amount) }
            }
        "#,
    );
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("owned state argument lowers from its destination home");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        // A value argument is snapshotted where it is evaluated: the call
        // passes a fresh copy, and that copy reads the state's destination
        // home, the structural parameter its arrival edge bound.
        let (function, argument) = target
            .functions
            .iter()
            .find_map(|function| {
                function
                    .graph
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find_map(|operation| match operation {
                        TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                        _ => None,
                    })
                    .map(|argument| (function, argument))
            })
            .expect("owned call argument");
        let target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: copy,
        } = argument.source
        else {
            panic!("owned argument passes its snapshot: {:?}", argument.source);
        };
        let copied_from = function
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::StructuralLeafCopy {
                    psi_operation,
                    source,
                    path,
                    ..
                } if *psi_operation == copy => {
                    assert_eq!(
                        *path,
                        abstract_operations::StructuralCopyPath::Ordinary(Vec::new())
                    );
                    Some(*source)
                }
                _ => None,
            })
            .expect("the snapshot is established in the calling function");
        let arrival = function
            .graph
            .blocks
            .iter()
            .find(|block| {
                block
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == copied_from)
            })
            .expect("the snapshot reads the state's destination home")
            .block;
        // Stage 02 settles which home the argument names. The copy row and
        // the copied operand's byte geometry are body replay: legalization
        // rebuilds them from the copy producer (`leaf_copy` legalization
        // tests reject a substituted copy source or shifted operand offset).
        for mutation in 0..3 {
            let changed = mutate_call_arguments(&target, |argument| match mutation {
                0 => {
                    argument.source =
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: OperationId::new(999_999).unwrap(),
                        }
                }
                // Passing the destination home itself would skip the snapshot.
                1 => {
                    argument.source =
                        target_operations::TargetStructuralArgumentSource::BlockParameter {
                            block: arrival,
                            place: copied_from,
                        }
                }
                _ => argument.access = StructuralAccess::SharedBorrow,
            });
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "{native:?}: substituted state argument {mutation}"
            );
        }
    }
}

#[test]
fn borrowed_scalar_call_preserves_verified_requirement_obligations() {
    borrowed_call_requirement_custody(true);
}

#[test]
fn moved_field_receiver_rejoins_its_exact_establishing_operation() {
    let source = source_plan(
        r#"
            data Item { value: i32; }
            machine Item::bump(self) -> Item {
                Item { value: ((self.value as i32 in Wrapping) + 1) as i32 }
            }
            data Main { item: Item; }
            machine Main::run(&mut self) {
                let taken: Item = self.item;
                self.item = move taken.bump();
            }
        "#,
    );
    let extraction = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .find_map(|operation| match operation {
            AbstractOperation::MoveStructuralField { psi_operation, .. } => Some(*psi_operation),
            _ => None,
        })
        .expect("field extraction produces the consuming receiver");
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        let mut target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("owned receiver uses its moved field home");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let argument = target.functions.iter_mut()
            .flat_map(|function| &mut function.graph.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.iter_mut().find(|argument| {
                    matches!(argument.source,
                        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation }
                        if psi_operation == extraction)
                }),
                _ => None,
            }).expect("call retains the extraction's exact home");
        argument.source = target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: OperationId::new(999_999).unwrap(),
        };
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &target).is_err(),
            "an equal-shaped but unrelated producer cannot supply receiver custody"
        );
    }
}

#[test]
fn copied_call_argument_rejoins_its_exact_establishing_operation() {
    let source = source_plan(
        r#"
            data Item [copy] { value: u32; }
            data Target { seen: u32; }
            machine Target::accept(&mut self, item: Item) { self.seen = item.value; }
            data Main { item: Item; target: Target; }
            machine Main::run(&mut self) { self.target.accept(self.item); }
        "#,
    );
    let copy = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .find_map(|operation| match operation {
            AbstractOperation::StructuralLeafCopy { psi_operation, .. } => Some(*psi_operation),
            _ => None,
        })
        .expect("the borrowed field is copied before the call");
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("a copied call argument has its own native storage");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let mut substituted = target.clone();
        let argument = substituted.functions.iter_mut()
            .flat_map(|function| &mut function.graph.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.iter_mut().find(|argument| {
                    matches!(argument.source,
                        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation }
                        if psi_operation == copy)
                }),
                _ => None,
            }).expect("the call names its copied argument home");
        argument.source = target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: OperationId::new(999_999).unwrap(),
        };
        assert!(matches!(
            crate::validate_abstract_to_target_translation(&source, native, &substituted),
            Err(
                crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch { .. }
            )
        ));
    }
}

/// A record-array literal establishes a local home, so its element loan takes
/// the live-graph route: the argument is a bare leaf pointer at the element's
/// offset. The parameter-roster route's array transport metadata does not
/// apply, and forging it, or another element's offset, still rejects.
#[test]
fn local_home_element_loan_lends_its_bare_leaf() {
    let source = source_plan(
        r#"
            data Room { exit_count: u32; }
            machine bump(room: &mut Room) { room.exit_count = 1; }
            data Main {}
            machine Main::run(&mut self) {
                let rooms: [Room; 2] = [Room { exit_count: 0 }, Room { exit_count: 5 }];
                bump(&mut rooms[1]);
            }
        "#,
    );
    for native in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("a local element loan lowers");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let argument = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments
                    .iter()
                    .find(|argument| argument.access == StructuralAccess::MutableBorrow),
                _ => None,
            })
            .expect("the call lends one element");
        assert_eq!(
            argument.path,
            [terminal_psi::StructuralPathSegment::FixedIndex(1)]
        );
        assert_eq!(argument.source_byte_offset, 4);
        assert_eq!(
            (argument.fixed_array_length, argument.element_stride),
            (None, None)
        );
        for mutation in [
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.fixed_array_length = Some(2)
            }) as Box<dyn Fn(&mut TargetStructuralArgument)>,
            Box::new(|argument: &mut TargetStructuralArgument| argument.element_stride = Some(4)),
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 0),
        ] {
            assert!(matches!(
                crate::validate_abstract_to_target_translation(
                    &source,
                    native,
                    &mutate_call_arguments(&target, mutation)
                ),
                Err(
                    crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch { .. }
                )
            ));
        }
    }
}

#[test]
fn ordinary_calls_reject_missing_substituted_and_forged_scalar_homes() {
    let source = source_plan(
        r#"
            data Main { value: u64; }
            machine Main::put(&mut self, value: u64) { self.value = value; }
            machine Main::get(&self) -> u64 { self.value }
            machine Main::run(&mut self) {
                self.put(3);
                let observed: u64 = self.get();
            }
        "#,
    );
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("borrowed Unit and scalar calls lower");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let scalar_home = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Scalar(home),
                    ..
                } => Some(*home),
                _ => None,
            })
            .expect("scalar result retained even when unused");
        for mutation in 0..5 {
            let changed = mutate_call_row(
                &target,
                |operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::Call {
                            result: target_operations::TargetCallResult::Scalar(_),
                            ..
                        }
                    )
                },
                |operation| {
                    let TargetUnitOperation::Call { result, .. } = operation else {
                        unreachable!()
                    };
                    if mutation == 0 {
                        *result = target_operations::TargetCallResult::Unit;
                        return;
                    }
                    let target_operations::TargetCallResult::Scalar(home) = result else {
                        unreachable!()
                    };
                    match mutation {
                        1 => home.defining_operation = OperationId::new(99_001).unwrap(),
                        2 => home.source_value = ValueId::new(99_002).unwrap(),
                        3 => home.scalar_type = ScalarType::Boolean,
                        _ => home.shape = calling_conventions::ValueShape::integer(1, 1),
                    }
                },
            );
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "{native:?}, scalar home mutation {mutation}"
            );
        }
        let changed = mutate_call_row(
            &target,
            |operation| {
                matches!(
                    operation,
                    TargetUnitOperation::Call {
                        result: target_operations::TargetCallResult::Unit,
                        ..
                    }
                )
            },
            |operation| {
                let TargetUnitOperation::Call { result, .. } = operation else {
                    unreachable!()
                };
                *result = target_operations::TargetCallResult::Scalar(scalar_home);
            },
        );
        assert!(
            crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
            "{native:?}, forged scalar result on Unit call"
        );
    }
}

fn borrowed_call_requirement_custody(scalar_result: bool) {
    let source = source_plan(
        r#"
            data Main { value: u64; }
            machine Main::put(&mut self, value: u64)
            requires
                1 <= value;
                value <= 7;
            { self.value = value; }
            machine Main::get(&self, value: u64) -> u64
            requires
                1 <= value;
                value <= 7;
            { self.value }
            machine Main::run(&mut self) {
                self.put(3);
                let observed: u64 = self.get(3);
            }
        "#,
    );
    let (operation, obligations) = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .find_map(|operation| match operation {
            AbstractOperation::CallStructural {
                psi_operation,
                requirement_obligations,
                result: AbstractStructuralCallResult::Unit,
                ..
            } if !scalar_result => Some((*psi_operation, requirement_obligations)),
            AbstractOperation::CallStructural {
                psi_operation,
                requirement_obligations,
                result: AbstractStructuralCallResult::Scalar(_),
                ..
            } if scalar_result => Some((*psi_operation, requirement_obligations)),
            _ => None,
        })
        .expect("authored borrowed call");
    // The obligation roster is the callee's Terminal `requires` roster. A
    // Unit callee publishes one row per authored clause; a scalar-graph
    // callee publishes its clauses as one canonical conjunction
    // (`PreparedScalarContract::requirement_count`), so its call owes one.
    if scalar_result {
        assert_eq!(obligations.len(), 1, "one canonical conjunction survives");
    } else {
        assert!(obligations.len() >= 2, "both authored requirements survive");
        assert_ne!(obligations[0], obligations[1]);
    }
    let select = |candidate: &TargetUnitOperation| match candidate {
        TargetUnitOperation::Call { psi_operation, .. } => *psi_operation == operation,
        _ => false,
    };
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("verified receiver requirements lower");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        for mutation in 0..4 {
            if mutation == 2 && obligations.len() < 2 {
                continue;
            }
            let changed = mutate_call_row(&target, select, |call| {
                let TargetUnitOperation::Call {
                    requirement_obligations,
                    ..
                } = call
                else {
                    unreachable!("selected borrowed call")
                };
                assert_eq!(requirement_obligations, obligations);
                match mutation {
                    0 => requirement_obligations.clear(),
                    1 => {
                        requirement_obligations[0] = semantic_vocabulary::ObligationId::new(
                            obligations
                                .iter()
                                .map(|obligation| obligation.get())
                                .max()
                                .unwrap()
                                + 1,
                        )
                        .unwrap()
                    }
                    2 => requirement_obligations.swap(0, 1),
                    _ => requirement_obligations.push(obligations[0]),
                }
            });
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "{native:?}, scalar result {scalar_result}, obligation mutation {mutation}"
            );
        }
    }
}

#[test]
fn structural_calls_preserve_verified_crash_continuations() {
    let source = source_plan(
        r#"
            data Packet { should_abort: bool; }
            data Helper {}
            machine Helper::inspect(packet: Packet)
            crashes Abort
                packet.should_abort
            {}
            data Main { value: u64; }
            machine Main::put(&mut self, value: u64)
            crashes Abort
            {
                self.value = value;
            }
            machine Main::get(&self) -> u64
            crashes Abort
            {
                self.value
            }
            machine Main::run(&mut self, packet: Packet)
            crashes Abort
            {
                Helper::inspect(packet);
                self.put(3);
                let observed: u64 = self.get();
            }
        "#,
    );
    // The authored call publishes the callee's surviving guarded route.
    let mut calls = Vec::new();
    for function in &source.functions {
        for operation in &function.operations {
            let (psi_operation, crash_continuations) = match operation {
                AbstractOperation::CallStructural {
                    psi_operation,
                    crash_continuations,
                    result:
                        AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
                    ..
                } => (*psi_operation, crash_continuations),
                _ => continue,
            };
            assert_eq!(crash_continuations.len(), 1, "{operation:?}");
            assert_eq!(crash_continuations[0].cause, CrashCause::Abort);
            calls.push((psi_operation, crash_continuations.clone()));
        }
    }
    // The owned-parameter Unit call and the `&self` scalar-result call ride
    // the borrowed lane; the `&mut self` Unit call rides the structural lane.
    assert_eq!(calls.len(), 3, "three crash-bearing calls");
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("verified call crash continuations lower");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        for (operation, expected) in &calls {
            for mutation in 0..3 {
                let changed = mutate_call_row(
                    &target,
                    |candidate| {
                        matches!(
                            candidate,
                            TargetUnitOperation::Call { psi_operation, .. }
                            if *psi_operation == *operation
                        )
                    },
                    |call| {
                        let TargetUnitOperation::Call {
                            crash_continuations,
                            ..
                        } = call
                        else {
                            unreachable!("selected crash-bearing call")
                        };
                        assert_eq!(crash_continuations, expected);
                        match mutation {
                            0 => crash_continuations.clear(),
                            1 => crash_continuations[0].cause = CrashCause::Trap,
                            _ => crash_continuations[0].alternatives.clear(),
                        }
                    },
                );
                assert!(
                    crate::validate_abstract_to_target_translation(&source, native, &changed)
                        .is_err(),
                    "{native:?}, call {operation:?}, crash roster mutation {mutation}"
                );
            }
        }
    }
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
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(1).unwrap(),
            callee: callee.machine,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: caller.structural_parameters[0].place,
                access: StructuralAccess::MutableBorrow,
                path: vec![terminal_psi::StructuralPathSegment::Field("tally".into())],
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
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
        if let AbstractOperation::CallStructural {
            arguments,
            result: AbstractStructuralCallResult::Unit,
            ..
        } = operation
        {
            arguments.push(caller_value);
        }
    }
    plan
}

/// The same projected borrow rooted at a caller-established record home
/// rather than a machine parameter. The referent's declaration comes from
/// the `EstablishRecord` results, so the retained argument must replay
/// against that home's own root type and byte offset.
fn established_home_borrow_plan() -> abstract_operations::AbstractOperationPlan {
    let mut plan = projected_field_borrow_plan();
    let tally = StructuralTypeId::new(1).unwrap();
    let main = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let tally_place = PlaceId::new(11).unwrap();
    let main_place = PlaceId::new(12).unwrap();
    let callee = plan.functions[1].machine;
    let caller = &mut plan.functions[0];
    caller.structural_parameters.clear();
    caller.operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(10).unwrap(),
            result: ValueId::new(10).unwrap(),
            scalar_type: unsigned,
            value: IntegerValue::Unsigned(7),
        },
        AbstractOperation::EstablishRecord {
            psi_operation: OperationId::new(11).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: tally_place,
                structural_type: tally,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            fields: vec![terminal_psi::RecordFieldInitializer {
                field: StructuralFieldId::new(1).unwrap(),
                value: terminal_psi::RecordFieldValue::Scalar {
                    value: ValueId::new(10).unwrap(),
                    range_obligation: None,
                },
            }],
        },
        AbstractOperation::EstablishRecord {
            psi_operation: OperationId::new(12).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: main_place,
                structural_type: main,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            fields: vec![terminal_psi::RecordFieldInitializer {
                field: StructuralFieldId::new(1).unwrap(),
                value: terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                    place: tally_place,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                }),
            }],
        },
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(13).unwrap(),
            callee,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: main_place,
                access: StructuralAccess::MutableBorrow,
                path: vec![terminal_psi::StructuralPathSegment::Field("tally".into())],
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    plan
}

/// A shared borrow of one element inside an owned caller array. The indexed
/// projection retains the root array's extent and element stride beside the
/// projected byte offset — metadata the validator must reconstruct from the
/// referent's declaration rather than accept from the retained row.
fn indexed_element_borrow_plan() -> abstract_operations::AbstractOperationPlan {
    let element = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let function = |raw, structural_type, access, multiplicity| AbstractFunction {
        machine: MachineId::new(raw).unwrap(),
        attachment: None,
        entry: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: PlaceId::new(raw).unwrap(),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity,
            access,
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
    let mut caller = function(
        1,
        array,
        StructuralAccess::Owned,
        StructuralMultiplicity::Affine,
    );
    let callee = function(
        2,
        element,
        StructuralAccess::SharedBorrow,
        StructuralMultiplicity::Unrestricted,
    );
    caller.operations.insert(
        0,
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(1).unwrap(),
            callee: callee.machine,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: caller.structural_parameters[0].place,
                access: StructuralAccess::SharedBorrow,
                path: vec![terminal_psi::StructuralPathSegment::FixedIndex(2)],
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
        },
    );
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller.machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: element,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "[u64; 4]".into(),
                shape: StructuralTypeShape::FixedArray { element, length: 4 },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![caller, callee],
    }
}

/// A caller that owns an aggregate actual, invokes a callee returning the
/// same structural type, and returns that result itself. The retained
/// structural-result `Call` must carry the declared result identity plus the
/// independently derived durable home.
fn structural_result_call_plan() -> abstract_operations::AbstractOperationPlan {
    let primitive = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let result = |identity| terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: PlaceId::new(identity).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let declaration = terminal_psi::StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: PlaceId::new(99).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let return_structural = |source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(1).unwrap(),
        source: PlaceId::new(source).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let call = |identity, source| AbstractOperation::CallStructural {
        psi_operation: OperationId::new(identity).unwrap(),
        result: AbstractStructuralCallResult::Structural {
            result: result(identity),
            returned_claim_transfers: Vec::new(),
            selected_evidence: Vec::new(),
        },
        callee: callee_machine,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(source).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "[u64; 2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: primitive,
                    length: 2,
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Structural(declaration.clone()),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(10).unwrap(),
                        result: ValueId::new(10).unwrap(),
                        scalar_type: unsigned,
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::EstablishScalarArray {
                        psi_operation: OperationId::new(11).unwrap(),
                        result: result(11),
                        elements: vec![ValueId::new(10).unwrap(), ValueId::new(10).unwrap()],
                    },
                    call(12, 11),
                    return_structural(12),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(20).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: array,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: AbstractFunctionResult::Structural(declaration),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(20)],
            },
        ],
    }
}

/// A caller that owns a record carrier with one reference leaf, moves it into
/// a callee that returns the same carrier, and returns the result itself. The
/// callee's declared result maps its `reference` leaf back to the owned
/// ingress parameter, so the retained structural-result `Call` must carry one
/// `reference_results` row resolved through caller custody: path
/// `[reference]`, root the caller's parameter place.
fn reference_result_call_plan() -> abstract_operations::AbstractOperationPlan {
    let primitive = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let carrier = StructuralTypeId::new(3).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let caller_place = PlaceId::new(1).unwrap();
    let callee_place = PlaceId::new(20).unwrap();
    let result_place = PlaceId::new(12).unwrap();
    let leaf_path = || {
        vec![terminal_psi::StructuralPathSegment::Field(
            "reference".into(),
        )]
    };
    let ingress = |place| StructuralArgument {
        place,
        path: vec![
            terminal_psi::StructuralPathSegment::Field("reference".into()),
            terminal_psi::StructuralPathSegment::Referent,
        ],
        access: StructuralAccess::MutableBorrow,
    };
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let declaration = |place, source| terminal_psi::StructuralResultDeclaration {
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![terminal_psi::StructuralReferenceResultSource {
            path: leaf_path(),
            sources: vec![source],
        }],
    };
    let result = |place| terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let return_structural = |raw, source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(raw).unwrap(),
        source,
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    AbstractOperationPlan {
        psi: super::support::identity(),
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: reference,
                identity: "&mut u64".into(),
                shape: StructuralTypeShape::Reference {
                    referent: primitive,
                    access: StructuralAccess::MutableBorrow,
                },
            },
            StructuralTypeDeclaration {
                id: carrier,
                identity: "Carrier".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "payload".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(unsigned),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "reference".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(reference),
                        },
                    ],
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(97).unwrap(),
                    ingress(caller_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::CallStructural {
                        psi_operation: OperationId::new(12).unwrap(),
                        result: AbstractStructuralCallResult::Structural {
                            result: result(result_place),
                            returned_claim_transfers: Vec::new(),
                            selected_evidence: Vec::new(),
                        },
                        callee: callee_machine,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: caller_place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        }],
                        dynamic_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    return_structural(1, result_place),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(98).unwrap(),
                    ingress(callee_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(2, callee_place)],
            },
        ],
    }
}

/// A descriptor stored into an aggregate field and later called through it.
/// The retained rows are one `StoreDynamicDescriptor` plus one stored-dispatch
/// `CallDynamic` whose plan and source argument replay the selected
/// realization's single-parameter signature.
fn stored_descriptor_source() -> abstract_operations::AbstractOperationPlan {
    source_plan(
        r#"
            trait Measure { machine measure(&self) -> bool; }
            data Item [copy] { value: bool; }
            Primary: Item satisfies Measure {
                machine measure(&self) -> bool { transition { _ -> self.value } }
            }
            data Holder<'item> { handler: &'item dyn Measure; }
            data Main [copy] { item: Item; }
            machine Main::run<'item>(&self) {
                let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
                let holder: Holder<'item> = Holder { handler: erased };
                let result: bool = holder.handler.measure();
            }
        "#,
    )
}

/// A rebound descriptor call: the retained rebound-dispatch `CallDynamic`
/// carries both the initializer and the latest source against the same
/// realization plan.
fn rebound_descriptor_source() -> abstract_operations::AbstractOperationPlan {
    source_plan(
        r#"
            trait Measure {
                machine measure(&self) -> i32;
                machine alternate(&self) -> i32;
            }
            data Item { value: i32; }
            Primary: Item satisfies Measure {
                machine measure(&self) -> i32 { transition { _ -> self.value } }
                machine alternate(&self) -> i32 { transition { _ -> self.value } }
            }
            data Main { decoy: Item; selected: Item; }
            machine Main::run(&mut self) {
                let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
                erased = &self.selected as &dyn Item::Primary;
                let result: i32 = erased.measure();
            }
        "#,
    )
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
                    TargetUnitOperation::Call { call_plan, .. } => call_plan,
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
                    TargetUnitOperation::Call { arguments, .. } => arguments,
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

/// Mutate the first retained operation matching `select`. Call rows that the
/// generic argument/plan helpers do not destructure are edited in place.
fn mutate_call_row(
    target: &TargetOperationPlan,
    select: impl Fn(&TargetUnitOperation) -> bool,
    f: impl FnOnce(&mut TargetUnitOperation),
) -> TargetOperationPlan {
    let mut target = target.clone();
    for function in &mut target.functions {
        for block in &mut function.graph.blocks {
            if let Some(operation) = block
                .operations
                .iter_mut()
                .find(|operation| select(operation))
            {
                f(operation);
                return target;
            }
        }
    }
    panic!("no matching retained call row");
}

#[test]
fn scalar_call_fixed_byte_window_retains_backing_and_extent() {
    // A scalar-result call lends the same caller-backed window a Unit call
    // does: the borrowed view is a call-scoped presentation, and the scalar
    // result carries no custody of the backing.
    let source = source_plan(
        r#"
            data Main { bytes: [u8; 41]; }
            data Helper {}
            machine Helper::measure(source: &[u8]) -> u64 {
                source.len
            }
            machine Main::run(&mut self) {
                let observed: u64 = Helper::measure(&self.bytes[1..41]);
            }
        "#,
    );
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("scalar call over a fixed byte window lowers");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let argument = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                _ => None,
            })
            .expect("scalar call structural argument");
        assert_eq!(argument.fixed_array_length, Some(40));
        assert_eq!(argument.element_stride, Some(1));
        assert_eq!(argument.source_byte_offset, 1);
        for mutation in 0..3 {
            let changed = mutate_call_arguments(&target, |argument| match mutation {
                0 => argument.source_byte_offset += 1,
                1 => argument.fixed_array_length = Some(41),
                _ => argument.element_stride = Some(2),
            });
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "{native:?}, window mutation {mutation}"
            );
        }
    }
}

#[test]
fn established_home_borrow_rejects_substituted_root_projection_and_home() {
    let source = established_home_borrow_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(13).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            // Referent root type substitution.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.root_structural_type = StructuralTypeId::new(1).unwrap()
            }) as Box<dyn Fn(&mut TargetStructuralArgument)>,
            // A different byte offset inside the same established home.
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 8),
            // Forged array-transport metadata on a plain pointer carrier.
            Box::new(|argument: &mut TargetStructuralArgument| argument.element_stride = Some(8)),
            // A sibling establishment is not this referent's producer.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(11).unwrap(),
                    }
            }),
            // An established home is not a block arrival.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: semantic_vocabulary::BlockId::new(1).unwrap(),
                        place: argument.place,
                    }
            }),
        ] {
            let mutated = mutate_call_arguments(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted argument identity"
            );
        }
    }
}

#[test]
fn indexed_element_borrow_rejects_substituted_projection_and_transport() {
    let source = indexed_element_borrow_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            // A different element inside the same owned array home.
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 0)
                as Box<dyn Fn(&mut TargetStructuralArgument)>,
            Box::new(|argument: &mut TargetStructuralArgument| argument.source_byte_offset = 24),
            // The referent root is the array, not its element.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.root_structural_type = StructuralTypeId::new(1).unwrap()
            }),
            // The projected carrier is the element, not the root array.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.structural_type = StructuralTypeId::new(2).unwrap()
            }),
            // Dropped or fabricated array transport metadata.
            Box::new(|argument: &mut TargetStructuralArgument| argument.fixed_array_length = None),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.fixed_array_length = Some(8)
            }),
            Box::new(|argument: &mut TargetStructuralArgument| argument.element_stride = None),
            Box::new(|argument: &mut TargetStructuralArgument| argument.element_stride = Some(16)),
            // Access, ABI class, and destination substitutions still reject.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.access = terminal_psi::StructuralAccess::MutableBorrow
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.shape.class = ValueClass::Integer
            }),
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.destination.locations.clear()
            }),
            // An owned array parameter is not an operation-established home.
            Box::new(|argument: &mut TargetStructuralArgument| {
                argument.source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(1).unwrap(),
                    }
            }),
        ] {
            let mutated = mutate_call_arguments(&target, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted indexed projection"
            );
        }
    }
}

#[test]
fn call_scalar_arguments_reject_substituted_identity_and_placement() {
    let source = projected_field_borrow_scalar_argument_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("honest scalar argument transport");
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

#[test]
fn structural_result_call_rejects_substituted_result_home_and_plan() {
    let source = structural_result_call_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(12).unwrap(),
        };
    let is_result_call = |operation: &TargetUnitOperation| {
        matches!(
            operation,
            TargetUnitOperation::Call {
                result: target_operations::TargetCallResult::Structural { .. },
                ..
            }
        )
    };
    let forged_declaration = || StructuralParameterDeclaration {
        place: PlaceId::new(9).unwrap(),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(2).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    callee,
                    result: target_operations::TargetCallResult::Structural { .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                *callee = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { result, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                result.place = PlaceId::new(42).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { result, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                result.structural_type = StructuralTypeId::new(1).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { result, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { callee_result, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                callee_result.structural_type = StructuralTypeId::new(1).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { callee_result, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                callee_result.multiplicity = StructuralMultiplicity::Affine;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { result_home, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                *result_home = None;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { result_home, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                let home = result_home.as_mut().unwrap();
                home.origin = target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block: BlockId::new(9).unwrap(),
                    declaration: forged_declaration(),
                };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Structural { result_home, .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                let home = result_home.as_mut().unwrap();
                home.layout = target_operations::TargetStructuralHomeLayout::Aggregate(
                    calling_conventions::ValueShape::integer(4, 4),
                );
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    arguments,
                    result: target_operations::TargetCallResult::Structural { .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                arguments[0].access = StructuralAccess::SharedBorrow;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    scalar_arguments,
                    call_plan,
                    result: target_operations::TargetCallResult::Structural { .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                scalar_arguments.push(target_operations::TargetUnitScalarCallArgument {
                    parameter_index: 0,
                    source: target_operations::TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 0,
                        source_value: ValueId::new(50).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    },
                    placement: call_plan.parameters[0].clone(),
                });
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    requirement_obligations,
                    result: target_operations::TargetCallResult::Structural { .. },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
        ] {
            let mutated = mutate_call_row(&target, is_result_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted structural result row"
            );
        }
        for plan_mutation in [
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.stack_alignment = plan.stack_alignment.wrapping_add(8);
            }) as Box<dyn Fn(&mut calling_conventions::CallPlan)>,
            // A structural-result callee always has a result placement;
            // dropping it is not an equivalent plan on any target.
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.result = None;
            }),
            Box::new(|plan: &mut calling_conventions::CallPlan| {
                plan.policy = calling_conventions::CallingPolicy::MicrosoftX64;
            }),
        ] {
            let mutated = mutate_call_plan(&target, plan_mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "embedded callee plan detail"
            );
        }
    }
}

#[test]
fn selected_reference_result_retains_every_root_path() {
    let source = source_plan(
        r#"
            domain [u8; 8]::Utf8(bytes) requires valid_utf8(bytes);
            data Holder { value: u64; label: [u8; 8] in Utf8; }
            data Bag { holders: [Holder; 3]; }
            data Main { bag: Bag; }
            machine pick(bag: &mut Bag, second: bool) -> &mut Holder {
                let holders: &mut [Holder] = bag.holders.as_mut_slice();
                transition second {
                    true -> &mut holders[1]
                    false -> &mut holders[0]
                }
            }
            machine Main::run(&mut self) {
                self.bag.holders[2].label = "ok";
                let selected: &mut Holder = pick(&mut self.bag, true);
                let sibling_length: u64 = self.bag.holders[2].label.len;
                selected.value = sibling_length;
            }
        "#,
    );
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("selected reference custody lowers without choosing an address");
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("receiver reconstructs both possible roots");
        let is_selected = |operation: &TargetUnitOperation| {
            matches!(operation,
            TargetUnitOperation::Call {
                result: target_operations::TargetCallResult::Structural { reference_results, .. },
                ..
            } if reference_results.iter().any(|reference| reference.root_paths.len() == 2))
        };
        assert!(
            target
                .functions
                .iter()
                .flat_map(|function| &function.graph.blocks)
                .flat_map(|block| &block.operations)
                .any(is_selected)
        );
        let sibling = terminal_psi::StructuralPathSegment::FixedIndex(2);
        assert!(
            target
                .functions
                .iter()
                .flat_map(|function| &function.graph.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation,
                TargetUnitOperation::StructuralByteSequenceFieldStore { destination, .. }
                    if destination.path.contains(&sibling)))
        );
        assert!(
            target
                .functions
                .iter()
                .flat_map(|function| &function.graph.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation,
                TargetUnitOperation::StructuralByteSequenceFieldLength { source, .. }
                    if source.path.contains(&sibling)))
        );
        for mutation in 0..6 {
            let mut changed = target.clone();
            let operation = changed
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .find(|operation| is_selected(operation))
                .expect("selected result call");
            let TargetUnitOperation::Call {
                result:
                    target_operations::TargetCallResult::Structural {
                        reference_results, ..
                    },
                ..
            } = operation
            else {
                panic!("selected call");
            };
            let reference = &mut reference_results[0];
            assert!(reference.sole_root_path().is_none());
            match mutation {
                0 => {
                    reference.root_paths.pop();
                }
                1 => reference.root_paths.clear(),
                2 => reference.root_paths.swap(0, 1),
                3 => reference.root_paths[1] = reference.root_paths[0].clone(),
                4 => {
                    *reference.root_paths[1].last_mut().unwrap() =
                        terminal_psi::StructuralPathSegment::FixedIndex(2);
                }
                _ => reference.root = PlaceId::new(777).unwrap(),
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "candidate mutation {mutation} accepted"
            );
        }
    }
}

#[test]
fn structural_result_call_rejects_forged_reference_result_rows() {
    let source = reference_result_call_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(12).unwrap(),
        };
    let is_result_call = |operation: &TargetUnitOperation| {
        matches!(
            operation,
            TargetUnitOperation::Call {
                result: target_operations::TargetCallResult::Structural { .. },
                ..
            }
        )
    };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        for mutation in [
            // A different suspended root: the caller parameter place is the
            // only honest referent identity.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result:
                        target_operations::TargetCallResult::Structural {
                            reference_results, ..
                        },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results[0].root = PlaceId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            // A different leaf path inside the same result carrier.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result:
                        target_operations::TargetCallResult::Structural {
                            reference_results, ..
                        },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results[0].path =
                    vec![terminal_psi::StructuralPathSegment::Field("payload".into())];
            }),
            // A deeper projection that no declared result leaf occupies.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result:
                        target_operations::TargetCallResult::Structural {
                            reference_results, ..
                        },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results[0]
                    .path
                    .push(terminal_psi::StructuralPathSegment::FixedIndex(0));
            }),
            // A second forged row beside the honest one.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result:
                        target_operations::TargetCallResult::Structural {
                            reference_results, ..
                        },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                let forged = reference_results[0].clone();
                reference_results.push(forged);
            }),
            // Dropping the roster entirely is not an equivalent row.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result:
                        target_operations::TargetCallResult::Structural {
                            reference_results, ..
                        },
                    ..
                } = operation
                else {
                    unreachable!()
                };
                reference_results.clear();
            }),
        ] {
            let mutated = mutate_call_row(&target, is_result_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "forged reference result row"
            );
        }
    }
}

#[test]
fn returned_record_projected_call_rejects_forged_physical_origins() {
    returned_record_call_rejects_forged_physical_origins(
        r#"
            data Child { value: u64; }
            data Holder { child: Child; other: Child; sibling: u64; }
            data Bag { holders: [Holder; 2]; }
            data Main { bag: Bag; }
            machine pick(bag: &mut Bag) -> &mut Holder {
                let holders: &mut [Holder] = bag.holders.as_mut_slice();
                transition { _ -> &mut holders[1] }
            }
            machine replace(child: &mut Child) { child.value = 41; }
            machine Main::run(&mut self) {
                let selected: &mut Holder = pick(&mut self.bag);
                replace(&mut selected.child);
            }
        "#,
        true,
    );
}

#[test]
fn returned_record_method_receiver_rejects_forged_physical_origins() {
    returned_record_call_rejects_forged_physical_origins(
        r#"
            data Child { value: u64; }
            data Holder { child: Child; other: Child; sibling: u64; }
            data Bag { holders: [Holder; 2]; }
            data Main { bag: Bag; }
            machine pick(bag: &mut Bag) -> &mut Holder {
                let holders: &mut [Holder] = bag.holders.as_mut_slice();
                transition { _ -> &mut holders[1] }
            }
            machine Holder::replace(&mut self) { self.child.value = 41; }
            machine Main::run(&mut self) {
                let selected: &mut Holder = pick(&mut self.bag);
                selected.replace();
            }
        "#,
        false,
    );
}

fn returned_record_call_rejects_forged_physical_origins(program: &str, projected: bool) {
    let source = source_plan(program);
    let (caller, projected_call) = source
        .functions
        .iter()
        .find_map(|function| {
            function
                .operations
                .iter()
                .find_map(|operation| match operation {
                    AbstractOperation::CallStructural {
                        psi_operation,
                        structural_arguments,
                        ..
                    } if structural_arguments.iter().any(|argument| {
                        matches!(
                            argument.path.first(),
                            Some(terminal_psi::StructuralPathSegment::Referent)
                        )
                    }) =>
                    {
                        Some((function.machine, *psi_operation))
                    }
                    _ => None,
                })
        })
        .expect("projected call operation");
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: caller,
            operation: projected_call,
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("projected returned-record call lowers");
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("honest projected origin validates");
        let selected_field = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call {
                    psi_operation,
                    arguments,
                    ..
                } if *psi_operation == projected_call => arguments.first()?.path.last(),
                _ => None,
            })
            .and_then(|segment| match segment {
                terminal_psi::StructuralPathSegment::Field(identity) => Some(identity.clone()),
                _ => None,
            });
        let sibling_field = source.structural_types.iter().find_map(|declaration| {
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return None;
            };
            let selected_type = fields.iter().find_map(|field| {
                (Some(&field.identity) == selected_field.as_ref()).then_some(&field.field_type)
            })?;
            fields
                .iter()
                .find(|field| {
                    Some(&field.identity) != selected_field.as_ref()
                        && &field.field_type == selected_type
                })
                .map(|field| field.identity.clone())
        });
        assert_eq!(sibling_field.is_some(), projected);
        for corruption in 0..if projected { 6 } else { 5 } {
            let changed = mutate_call_row(
                &target,
                |operation| {
                    matches!(operation,
                        TargetUnitOperation::Call { psi_operation, .. }
                            if *psi_operation == projected_call)
                },
                |operation| {
                    let TargetUnitOperation::Call { arguments, .. } = operation else {
                        unreachable!()
                    };
                    let argument = arguments.first_mut().expect("projected argument");
                    match corruption {
                        0 => argument.place = PlaceId::new(999).unwrap(),
                        1 => {
                            assert_eq!(
                                argument.path[0],
                                terminal_psi::StructuralPathSegment::Referent
                            );
                            argument.path[0] = terminal_psi::StructuralPathSegment::FixedIndex(0);
                        }
                        2 => argument.source_byte_offset += 8,
                        3 => {
                            argument.source =
                                target_operations::TargetStructuralArgumentSource::StructuralHome {
                                    psi_operation: OperationId::new(999).unwrap(),
                                };
                        }
                        4 => argument.root_structural_type = argument.structural_type,
                        5 => {
                            let field = argument.path.last_mut().expect("selected child field");
                            *field = terminal_psi::StructuralPathSegment::Field(
                                sibling_field.clone().expect("same-typed sibling identity"),
                            );
                        }
                        _ => unreachable!(),
                    }
                },
            );
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &changed),
                Err(expected.clone()),
                "forged returned-record physical origin {corruption}"
            );
        }
    }
}

#[test]
fn embedded_scalar_calls_replay_declared_signature_rows() {
    let source = super::unit_scalar_calls::attached_unit_scalar_call_plan();
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(11).unwrap(),
        };
    let is_scalar_call =
        |operation: &TargetUnitOperation| matches!(operation, TargetUnitOperation::Call { .. });
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let calls = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation, TargetUnitOperation::Call { .. }))
            .count();
        assert_eq!(calls, 2);
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call { callee, .. } = operation else {
                    unreachable!()
                };
                *callee = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call { call_plan, .. } = operation else {
                    unreachable!()
                };
                call_plan.stack_alignment = call_plan.stack_alignment.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    scalar_arguments: arguments,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                arguments[0].parameter_index = 9;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    scalar_arguments: arguments,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                arguments[0].placement.locations.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    scalar_arguments: arguments,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                let scalar_type = arguments[0].source.scalar_type();
                arguments[0].source =
                    target_operations::TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 0,
                        source_value: ValueId::new(99).unwrap(),
                        scalar_type,
                    };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Scalar(result_home),
                    ..
                } = operation
                else {
                    unreachable!()
                };
                result_home.source_value = ValueId::new(99).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Scalar(result_home),
                    ..
                } = operation
                else {
                    unreachable!()
                };
                result_home.defining_operation = OperationId::new(10).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Scalar(result_home),
                    ..
                } = operation
                else {
                    unreachable!()
                };
                result_home.scalar_type = ScalarType::Boolean;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    result: target_operations::TargetCallResult::Scalar(result_home),
                    ..
                } = operation
                else {
                    unreachable!()
                };
                result_home.shape = calling_conventions::ValueShape::integer(8, 8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    requirement_obligations,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
            // Removing the scalar result cannot turn the source call into Unit.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call { result, .. } = operation else {
                    unreachable!()
                };
                *result = target_operations::TargetCallResult::Unit;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::Call {
                    claim_transfers, ..
                } = operation
                else {
                    unreachable!()
                };
                claim_transfers.push(terminal_psi::ClaimTransfer {
                    claim: semantic_vocabulary::ClaimId::new(99).unwrap(),
                    argument_index: 0,
                });
            }),
        ] {
            let mutated = mutate_call_row(&target, is_scalar_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted scalar call row"
            );
        }
        // The embedded call replays against the callee's published entrance: a
        // callee that never published one cannot satisfy a retained call row.
        let mut unpublished = target.clone();
        unpublished
            .functions
            .iter_mut()
            .find(|function| function.machine == MachineId::new(2).unwrap())
            .unwrap()
            .scalar_abi = None;
        assert_eq!(
            crate::validate_abstract_to_target_translation(&source, native, &unpublished),
            Err(expected.clone())
        );
    }
}

#[test]
fn direct_calls_reject_cross_kind_result_custody() {
    let sources = [
        projected_field_borrow_plan(),
        super::unit_scalar_calls::attached_unit_scalar_call_plan(),
        structural_result_call_plan(),
    ];
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let targets = sources
            .iter()
            .map(|source| {
                let target = crate::lower_to_target_operations(
                    source,
                    crate::TargetLoweringRequest::new(native),
                )
                .unwrap();
                crate::validate_abstract_to_target_translation(source, native, &target).unwrap();
                target
            })
            .collect::<Vec<_>>();
        let results = targets
            .iter()
            .map(|target| {
                target
                    .functions
                    .iter()
                    .flat_map(|function| &function.graph.blocks)
                    .flat_map(|block| &block.operations)
                    .find_map(|operation| match operation {
                        TargetUnitOperation::Call { result, .. } => Some(result.clone()),
                        _ => None,
                    })
                    .expect("fixture retains direct call")
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            results[0],
            target_operations::TargetCallResult::Unit
        ));
        assert!(matches!(
            results[1],
            target_operations::TargetCallResult::Scalar(_)
        ));
        assert!(matches!(
            results[2],
            target_operations::TargetCallResult::Structural { .. }
        ));
        for (source_position, (source, target)) in sources.iter().zip(&targets).enumerate() {
            for (replacement_position, replacement) in results.iter().enumerate() {
                if source_position == replacement_position {
                    continue;
                }
                let changed = mutate_call_row(
                    target,
                    |operation| matches!(operation, TargetUnitOperation::Call { .. }),
                    |operation| {
                        let TargetUnitOperation::Call { result, .. } = operation else {
                            unreachable!()
                        };
                        *result = replacement.clone();
                    },
                );
                assert!(
                    crate::validate_abstract_to_target_translation(source, native, &changed)
                        .is_err(),
                    "{native:?}, result kind {source_position} replaced by {replacement_position}"
                );
            }
        }
    }
}

/// The retained dispatch of one `CallDynamic` row a substitution rewrites.
fn dynamic_dispatch(operation: &mut TargetUnitOperation) -> &mut TargetDynamicDispatch {
    let TargetUnitOperation::CallDynamic { dispatch, .. } = operation else {
        unreachable!("dynamic call row")
    };
    dispatch
}

/// The scalar result home of one `CallDynamic` row a substitution rewrites.
fn dynamic_result_home(
    operation: &mut TargetUnitOperation,
) -> &mut target_operations::TargetUnitScalarHomeRequirement {
    let TargetUnitOperation::CallDynamic {
        result: TargetDynamicCallResult::Scalar(home),
        ..
    } = operation
    else {
        unreachable!("scalar-result dynamic call row")
    };
    home
}

fn stored_source_argument(operation: &mut TargetUnitOperation) -> &mut TargetStructuralArgument {
    let TargetDynamicDispatch::Stored {
        source_argument, ..
    } = dynamic_dispatch(operation)
    else {
        unreachable!("stored dispatch")
    };
    source_argument
}

fn rebound_initial_argument(operation: &mut TargetUnitOperation) -> &mut TargetStructuralArgument {
    let TargetDynamicDispatch::Rebound {
        initial_argument, ..
    } = dynamic_dispatch(operation)
    else {
        unreachable!("rebound dispatch")
    };
    initial_argument
}

fn rebound_latest_argument(operation: &mut TargetUnitOperation) -> &mut TargetStructuralArgument {
    let TargetDynamicDispatch::Rebound {
        rebound_argument, ..
    } = dynamic_dispatch(operation)
    else {
        unreachable!("rebound dispatch")
    };
    rebound_argument
}

#[test]
fn stored_descriptor_calls_replay_unique_custody_plan_and_result() {
    let source = stored_descriptor_source();
    let entry = source
        .functions
        .iter()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let store_operation = entry
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::StoreDynamicDescriptor { psi_operation, .. } => Some(*psi_operation),
            _ => None,
        })
        .expect("stored descriptor operation");
    let call_operation = entry
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamic {
                psi_operation,
                dispatch: abstract_operations::AbstractDynamicDispatch::Stored(_),
                result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                ..
            } => Some(*psi_operation),
            _ => None,
        })
        .expect("stored dynamic call operation");
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let operations = target
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap()
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations);
        let mut store = None;
        let mut call = None;
        for operation in operations {
            match operation {
                TargetUnitOperation::StoreDynamicDescriptor { .. } => store = Some(operation),
                TargetUnitOperation::CallDynamic {
                    dispatch: TargetDynamicDispatch::Stored { .. },
                    ..
                } => call = Some(operation),
                _ => {}
            }
        }
        let TargetUnitOperation::StoreDynamicDescriptor {
            source_argument, ..
        } = store.expect("stored descriptor row")
        else {
            unreachable!()
        };
        assert_eq!(source_argument.access, StructuralAccess::SharedBorrow);
        assert_eq!(source_argument.shape.class, ValueClass::BorrowedReference);
        let TargetUnitOperation::CallDynamic {
            dispatch:
                TargetDynamicDispatch::Stored {
                    source_argument, ..
                },
            call_plan,
            result: TargetDynamicCallResult::Scalar(result_home),
            ..
        } = call.expect("stored dynamic call row")
        else {
            unreachable!()
        };
        assert_eq!(source_argument.access, StructuralAccess::SharedBorrow);
        assert_eq!(source_argument.shape.class, ValueClass::BorrowedReference);
        assert_eq!(call_plan.parameters.len(), 1);
        assert_eq!(result_home.defining_operation, call_operation);
    }
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        let call_expected =
            crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                machine: source.entry,
                operation: call_operation,
            };
        let is_stored_call = |operation: &TargetUnitOperation| {
            matches!(
                operation,
                TargetUnitOperation::CallDynamic {
                    dispatch: TargetDynamicDispatch::Stored { .. },
                    ..
                }
            )
        };
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetDynamicDispatch::Stored { custody, .. } = dynamic_dispatch(operation)
                else {
                    unreachable!()
                };
                custody.dispatch.realization = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            // A stored descriptor is never invoked for a Unit result.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::CallDynamic { result, .. } = operation else {
                    unreachable!()
                };
                *result = TargetDynamicCallResult::Unit;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::CallDynamic { call_plan, .. } = operation else {
                    unreachable!()
                };
                call_plan.stack_alignment = call_plan.stack_alignment.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let source_argument = stored_source_argument(operation);
                source_argument.access = if source_argument.access == StructuralAccess::Owned {
                    StructuralAccess::MutableBorrow
                } else {
                    StructuralAccess::Owned
                };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let source_argument = stored_source_argument(operation);
                source_argument.source_byte_offset =
                    source_argument.source_byte_offset.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                stored_source_argument(operation).path.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                stored_source_argument(operation)
                    .destination
                    .locations
                    .clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                stored_source_argument(operation).source =
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: OperationId::new(1).unwrap(),
                    };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                dynamic_result_home(operation).source_value = ValueId::new(99).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                dynamic_result_home(operation).shape =
                    calling_conventions::ValueShape::integer(8, 8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::CallDynamic {
                    requirement_obligations,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
        ] {
            let mutated = mutate_call_row(&target, is_stored_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(call_expected.clone()),
                "substituted stored-call row"
            );
        }
        let store_expected =
            crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                machine: source.entry,
                operation: store_operation,
            };
        let is_store = |operation: &TargetUnitOperation| {
            matches!(
                operation,
                TargetUnitOperation::StoreDynamicDescriptor { .. }
            )
        };
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoreDynamicDescriptor { stored, .. } = operation else {
                    unreachable!()
                };
                stored.descriptor.field_identity = "forged".into();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoreDynamicDescriptor {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.source_byte_offset =
                    source_argument.source_byte_offset.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::StoreDynamicDescriptor {
                    source_argument, ..
                } = operation
                else {
                    unreachable!()
                };
                source_argument.destination.locations.clear();
            }),
        ] {
            let mutated = mutate_call_row(&target, is_store, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(store_expected.clone()),
                "substituted descriptor-store row"
            );
        }
    }
}

#[test]
fn rebound_dynamic_calls_replay_both_instance_projections() {
    let source = rebound_descriptor_source();
    let entry = source
        .functions
        .iter()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let call_operation = entry
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamic {
                psi_operation,
                dispatch: abstract_operations::AbstractDynamicDispatch::Rebound(_),
                result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                ..
            } => Some(*psi_operation),
            _ => None,
        })
        .expect("rebound dynamic call operation");
    let is_rebound_call = |operation: &TargetUnitOperation| {
        matches!(
            operation,
            TargetUnitOperation::CallDynamic {
                dispatch: TargetDynamicDispatch::Rebound { .. },
                ..
            }
        )
    };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let call = target
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap()
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| is_rebound_call(operation))
            .expect("retained rebound call");
        let TargetUnitOperation::CallDynamic {
            dispatch:
                TargetDynamicDispatch::Rebound {
                    initial_argument,
                    rebound_argument,
                    ..
                },
            call_plan,
            result: TargetDynamicCallResult::Scalar(result_home),
            ..
        } = call
        else {
            unreachable!()
        };
        for argument in [initial_argument, rebound_argument] {
            assert_eq!(argument.access, StructuralAccess::SharedBorrow);
            assert_eq!(argument.shape.class, ValueClass::BorrowedReference);
        }
        // Both projections land on the realization's single parameter slot.
        assert_eq!(call_plan.parameters.len(), 1);
        assert_eq!(initial_argument.destination, rebound_argument.destination);
        assert_eq!(result_home.defining_operation, call_operation);
    }
    let expected =
        crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
            machine: source.entry,
            operation: call_operation,
        };
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .unwrap();
        for mutation in [
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetDynamicDispatch::Rebound { custody, .. } = dynamic_dispatch(operation)
                else {
                    unreachable!()
                };
                custody.dispatch.realization = MachineId::new(77).unwrap();
            }) as Box<dyn FnOnce(&mut TargetUnitOperation)>,
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::CallDynamic { call_plan, .. } = operation else {
                    unreachable!()
                };
                call_plan.stack_alignment = call_plan.stack_alignment.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let initial_argument = rebound_initial_argument(operation);
                initial_argument.source_byte_offset =
                    initial_argument.source_byte_offset.wrapping_add(8);
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let initial_argument = rebound_initial_argument(operation);
                initial_argument.access = if initial_argument.access == StructuralAccess::Owned {
                    StructuralAccess::MutableBorrow
                } else {
                    StructuralAccess::Owned
                };
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                rebound_latest_argument(operation).path.clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                rebound_latest_argument(operation)
                    .destination
                    .locations
                    .clear();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                dynamic_result_home(operation).source_value = ValueId::new(99).unwrap();
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                dynamic_result_home(operation).scalar_type = ScalarType::Boolean;
            }),
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::CallDynamic {
                    requirement_obligations,
                    ..
                } = operation
                else {
                    unreachable!()
                };
                requirement_obligations.push(semantic_vocabulary::ObligationId::new(7).unwrap());
            }),
            // Dropping the scalar result row is a different call role, not a
            // cheaper presentation of the same call.
            Box::new(|operation: &mut TargetUnitOperation| {
                let TargetUnitOperation::CallDynamic { result, .. } = operation else {
                    unreachable!()
                };
                *result = TargetDynamicCallResult::Unit;
            }),
        ] {
            let mutated = mutate_call_row(&target, is_rebound_call, mutation);
            assert_eq!(
                crate::validate_abstract_to_target_translation(&source, native, &mutated),
                Err(expected.clone()),
                "substituted rebound-call row"
            );
        }
    }
}

#[test]
fn scalar_reference_call_replays_original_backing_and_viewed_type() {
    let source = source_plan(
        r#"
        data Main {}
        machine read(value: &u32) -> u32 { value }
        machine Main::run(value: f32) -> u32 {
            let view: &u32 = &value as &u32;
            read(view)
        }
    "#,
    );
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("shared scalar representation call lowers");
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("original backing and viewed argument independently validate");
        let operation = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call {
                    psi_operation,
                    arguments,
                    ..
                } if arguments.iter().any(|argument| {
                    argument.root_structural_type != argument.structural_type
                        && argument.path == [terminal_psi::StructuralPathSegment::Referent]
                }) =>
                {
                    Some(*psi_operation)
                }
                _ => None,
            })
            .expect("a reference call preserves distinct backing and viewed declarations");
        for corruption in 0..5 {
            let changed = mutate_call_row(
                &target,
                |row| matches!(row, TargetUnitOperation::Call { psi_operation, .. } if *psi_operation == operation),
                |row| {
                    let TargetUnitOperation::Call { arguments, .. } = row else {
                        unreachable!()
                    };
                    let argument = &mut arguments[0];
                    match corruption {
                        0 => argument.root_structural_type = argument.structural_type,
                        1 => argument.structural_type = argument.root_structural_type,
                        2 => argument.access = StructuralAccess::MutableBorrow,
                        3 => argument.path.clear(),
                        _ => argument.source = target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                            psi_operation: OperationId::new(999).unwrap(),
                        },
                    }
                },
            );
            assert!(matches!(crate::validate_abstract_to_target_translation(&source, native, &changed),
                Err(crate::AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch { .. })),
                "forged shared representation argument {corruption}");
        }
    }
}
