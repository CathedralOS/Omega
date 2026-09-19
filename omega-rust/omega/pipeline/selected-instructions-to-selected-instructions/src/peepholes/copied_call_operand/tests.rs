use crate::CopiedCallOperandError;
use crate::CopiedCallOperandReceipt;
use crate::ValidatedCopiedCallOperand;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_copied_call_operand;
use crate::validate_copied_call_operand_fold;
use crate::validated_machine_effect_catalog;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::{RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedTerminator, ValidatedMachineEffectCatalog, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
}

fn instruction(
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    row: &RegisterInstructionConstraint,
    registers: &[VirtualRegisterId],
) -> SelectedInstruction {
    SelectedInstruction {
        id,
        kind,
        constraint: row.key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: *register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: Default::default(),
    }
}

const COPY: SelectedInstructionId = SelectedInstructionId(2);
const CALL: SelectedInstructionId = SelectedInstructionId(4);
const EXTRA: SelectedInstructionId = SelectedInstructionId(7);
const SOURCE: VirtualRegisterId = VirtualRegisterId(0);
const DESTINATION: VirtualRegisterId = VirtualRegisterId(1);
const RESULT: VirtualRegisterId = VirtualRegisterId(2);
const ARGUMENT: VirtualRegisterId = VirtualRegisterId(3);
const SPARE: VirtualRegisterId = VirtualRegisterId(4);

/// Which direct internal call form the fixture's consumer carries.
#[derive(Clone, Copy)]
enum Call {
    Unit,
    Scalar,
    Aggregate,
}

impl Call {
    /// The first selected row of the call family carrying at least one
    /// `Use` argument — the fixture needs a copied operand to reroute.
    fn row(
        self,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Option<&RegisterInstructionConstraint> {
        let keys = environment.selected_keys();
        let roster: &[RegisterConstraintKey] = match self {
            Self::Unit => &keys.call_unit,
            Self::Scalar => &keys.call_scalar,
            Self::Aggregate => &keys.call_aggregate,
        };
        roster
            .iter()
            .filter_map(|key| environment.constraint(*key))
            .find(|row| {
                row.operands
                    .iter()
                    .any(|operand| operand.access == RegisterOperandAccess::Use)
            })
    }

    fn kind(self) -> SelectedInstructionKind {
        let callee = MachineId::new(9).unwrap();
        match self {
            Self::Unit => SelectedInstructionKind::CallUnit { callee },
            Self::Scalar => SelectedInstructionKind::CallScalar { callee },
            Self::Aggregate => SelectedInstructionKind::CallAggregate { callee },
        }
    }
}

fn register(
    id: VirtualRegisterId,
    scalar_type: ScalarType,
    class: register_model::RegisterClassId,
    origin: VirtualRegisterOrigin,
) -> VirtualRegister {
    VirtualRegister {
        id,
        scalar_type,
        class,
        origin,
        definition_site: None,
        entry_fixed_view: None,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission
/// claim: `destination = copy_i64 source; call …, destination, …` in the
/// entry block, returning Unit. The copy's destination may be read by
/// other consumers; the reroute rebinds only the call's `Use` operands
/// that name it. Variants edit the single fixture function through
/// `mutated`.
fn fixture(target: NativeTarget, call: Call) -> ValidatedCopiedCallOperand {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy_row = environment.constraint(keys.copy_i64).unwrap();
    let call_row = call.row(&environment).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = copy_row.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    // One register per operand position: argument positions read the
    // copy's destination except a trailing argument, when the row carries
    // more than one, which reads an entry parameter so the reroute's
    // operand selection is observable.
    let call_registers: Vec<VirtualRegisterId> = call_row
        .operands
        .iter()
        .enumerate()
        .map(|(position, operand)| match operand.access {
            RegisterOperandAccess::Use => {
                if position == 0 {
                    DESTINATION
                } else {
                    ARGUMENT
                }
            }
            _ => RESULT,
        })
        .collect();
    let mut registers = vec![
        VirtualRegister {
            id: SOURCE,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(2).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        register(
            DESTINATION,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: COPY,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        register(
            RESULT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: CALL,
                source_value: ValueId::new(4).unwrap(),
            },
        ),
        VirtualRegister {
            id: ARGUMENT,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(5).unwrap(),
                parameter_index: 1,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(1)),
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: SPARE,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(6).unwrap(),
                parameter_index: 2,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(2)),
            entry_fixed_view: None,
        },
    ];
    // Every operand class must equal its roster row's class: where the row
    // declares a different class for a position, the roster entries above
    // are restated at that class.
    for (operand, register_id) in call_row.operands.iter().zip(&call_registers) {
        let entry = registers
            .iter_mut()
            .find(|entry| entry.id == *register_id)
            .unwrap();
        entry.class = operand.class;
    }
    let mut consumer = instruction(CALL, call.kind(), call_row, &call_registers);
    consumer.provenance.operations = vec![OperationId::new(9).unwrap()];
    consumer.provenance.values = vec![ValueId::new(7).unwrap()];
    let machine = MachineId::new(1).unwrap();
    let plan = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target,
        entry: machine,
        functions: vec![SelectedFunction {
            machine,
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    instruction(
                        COPY,
                        SelectedInstructionKind::CopyI64,
                        copy_row,
                        &[SOURCE, DESTINATION],
                    ),
                    consumer,
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(9),
                        SelectedInstructionKind::ReturnUnit,
                        terminal_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(4).unwrap(),
                },
            }],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedCopiedCallOperand {
        receipt: CopiedCallOperandReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

/// The machine-effect catalog bound to `environment` for the plan's
/// target — the catalog the pair declarations resolve their effect
/// surfaces in.
fn catalog(
    source: &ValidatedCopiedCallOperand,
    environment: &ValidatedTargetRegisterEnvironment,
) -> ValidatedMachineEffectCatalog {
    validated_machine_effect_catalog(source.transformed().target, environment.constraints())
        .unwrap()
}

/// Edit the single fixture function, then refresh the receipt identities
/// so the mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
) -> ValidatedCopiedCallOperand {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target, Call::Scalar);
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn fold(
    source: &ValidatedCopiedCallOperand,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedCopiedCallOperand, CopiedCallOperandError> {
    let effect_catalog = catalog(source, environment);
    fold_selected_copied_call_operand(
        source,
        0,
        COPY,
        CALL,
        environment,
        &effect_catalog,
        budget(),
    )
}

fn replay(
    source: &ValidatedCopiedCallOperand,
    environment: &ValidatedTargetRegisterEnvironment,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCopiedCallOperand, CopiedCallOperandError> {
    let effect_catalog = catalog(source, environment);
    validate_copied_call_operand_fold(
        source,
        0,
        COPY,
        CALL,
        environment,
        &effect_catalog,
        budget(),
        proposed,
    )
}

/// The record a successful reroute publishes: `call` at the same position
/// with every `Use` operand that named the copy's destination rebound to
/// the source register, everything else verbatim.
fn rerouted_to(result: &ValidatedCopiedCallOperand) {
    let function = &result.transformed().functions[0];
    let instruction = &function.blocks[0].instructions[1];
    assert_eq!(instruction.id, CALL);
    for operand in &instruction.operands {
        if operand.access == RegisterOperandAccess::Use {
            assert_ne!(
                operand.virtual_register, DESTINATION,
                "a copied operand still names the destination"
            );
        }
    }
    assert_eq!(
        instruction.operands[0].virtual_register, SOURCE,
        "operand 0 must read the copy's source"
    );
}

/// The call forms the family declares. `CallScalar` exists on every
/// target; `CallUnit` and `CallAggregate` fire only where the target's
/// selected roster carries an argument-taking row.
#[test]
fn scalar_call_pairs_fold_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target, Call::Scalar);
        let result = fold(&source, &environment).unwrap();
        rerouted_to(&result);
        let consumer = &result.transformed().functions[0].blocks[0].instructions[1];
        let source_consumer = &source.transformed().functions[0].blocks[0].instructions[1];
        // The folded record keeps the consumer's kind — callee included —
        // constraint row, operand order, ABI views, result operands,
        // implicit uses and definitions, the complete caller-saved clobber
        // roster, and provenance verbatim; only the copied `Use` operand
        // registers change.
        assert_eq!(consumer.kind, source_consumer.kind);
        assert_eq!(consumer.constraint, source_consumer.constraint);
        assert_eq!(consumer.operands.len(), source_consumer.operands.len());
        for (folded, original) in consumer.operands.iter().zip(&source_consumer.operands) {
            assert_eq!(folded.operand, original.operand);
            assert_eq!(folded.access, original.access);
            assert_eq!(folded.class, original.class);
            assert_eq!(folded.fixed_view, original.fixed_view);
            assert_eq!(folded.tied_to, original.tied_to);
            assert_eq!(folded.early_clobber, original.early_clobber);
            if folded.access != RegisterOperandAccess::Use {
                assert_eq!(folded.virtual_register, original.virtual_register);
            }
        }
        assert_eq!(consumer.implicit_uses, source_consumer.implicit_uses);
        assert_eq!(
            consumer.implicit_uses.len() + consumer.implicit_defs.len() + consumer.clobbers.len(),
            source_consumer.implicit_uses.len()
                + source_consumer.implicit_defs.len()
                + source_consumer.clobbers.len()
        );
        assert!(!consumer.clobbers.is_empty() || !consumer.implicit_uses.is_empty());
        assert_eq!(consumer.provenance, source_consumer.provenance);
        // The producer stays: the copy keeps publishing its register for
        // every other reader.
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0],
            source.transformed().functions[0].blocks[0].instructions[0]
        );
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        replay(&source, &environment, result.transformed().clone()).unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        replay(&source, &environment, detached).unwrap();
    }
}

/// The `CallUnit` roster is x86-64-only today — AArch64 admits no Unit
/// call form — so the pair fires where the target selects an
/// argument-carrying Unit row and the family's declaration spans it.
#[test]
fn unit_call_pairs_fold_where_the_target_selects_them() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        if Call::Unit.row(&environment).is_none() {
            continue;
        }
        let source = fixture(target, Call::Unit);
        let result = fold(&source, &environment).unwrap();
        rerouted_to(&result);
        replay(&source, &environment, result.transformed().clone()).unwrap();
    }
    assert!(
        baseline_target_register_environment(NativeTarget::linux_x64())
            .unwrap()
            .selected_keys()
            .call_unit
            .iter()
            .any(|key| {
                baseline_target_register_environment(NativeTarget::linux_x64())
                    .unwrap()
                    .constraint(*key)
                    .is_some_and(|row| {
                        row.operands
                            .iter()
                            .any(|operand| operand.access == RegisterOperandAccess::Use)
                    })
            }),
        "the x86-64 target must select an argument-carrying Unit call row"
    );
}

/// The direct aggregate call reroutes a copied argument operand wherever
/// the target selects an argument-carrying aggregate row.
#[test]
fn aggregate_call_pairs_fold_where_the_target_selects_them() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        if Call::Aggregate.row(&environment).is_none() {
            continue;
        }
        let source = fixture(target, Call::Aggregate);
        let result = fold(&source, &environment).unwrap();
        rerouted_to(&result);
        replay(&source, &environment, result.transformed().clone()).unwrap();
    }
}

/// A call reading the copy's destination at several `Use` positions
/// rebinds every one of them: the pair is (producer, consumer), not
/// (producer, operand).
#[test]
fn every_copied_operand_rebinds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let arity_two = environment
        .selected_keys()
        .call_scalar
        .iter()
        .filter_map(|key| environment.constraint(*key))
        .find(|row| {
            row.operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use)
                .count()
                >= 2
        })
        .map(|row| row.key);
    let Some(arity_two) = arity_two else {
        return;
    };
    let source = mutated(target, |function, environment| {
        let row = environment.constraint(arity_two).unwrap();
        let call = &mut function.blocks[0].instructions[1];
        call.constraint = row.key;
        call.operands = row
            .operands
            .iter()
            .map(|operand| SelectedOperand {
                operand: operand.operand,
                virtual_register: match operand.access {
                    RegisterOperandAccess::Use => DESTINATION,
                    _ => RESULT,
                },
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect();
        call.implicit_uses = row.implicit_uses.clone();
        call.implicit_defs = row.implicit_defs.clone();
        call.clobbers = row.clobbers.clone();
        // Every operand class must equal its roster entry's: restate the
        // destination and result registers at the new row's classes.
        for operand in &call.operands {
            let entry = function
                .virtual_registers
                .iter_mut()
                .find(|entry| entry.id == operand.virtual_register)
                .unwrap();
            entry.class = operand.class;
        }
    });
    let result = fold(&source, &environment).unwrap();
    rerouted_to(&result);
    let instruction = &result.transformed().functions[0].blocks[0].instructions[1];
    let uses = instruction
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Use)
        .count();
    assert!(uses >= 2, "fixture must exercise several copied operands");
    replay(&source, &environment, result.transformed().clone()).unwrap();
}

/// A call that does not read the copy's destination has nothing to
/// reroute — the pair refuses rather than publishing an identity fold.
#[test]
fn uncopied_arguments_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        for operand in &mut function.blocks[0].instructions[1].operands {
            if operand.access == RegisterOperandAccess::Use {
                operand.virtual_register = ARGUMENT;
            }
        }
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedUse)
    );
}

/// A call consumer of an undeclared kind — the normalized foreign call's
/// boundary custody is not the direct internal contract — refuses at the
/// consumer gate.
#[test]
fn undeclared_consumer_kinds_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for kind in [
        SelectedInstructionKind::NormalizedForeignCall {
            boundary: semantic_vocabulary::BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        },
        SelectedInstructionKind::CopyI64,
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        SelectedInstructionKind::AddressOffset { byte_offset: 0 },
    ] {
        let source = mutated(target, |function, _| {
            function.blocks[0].instructions[1].kind = kind;
        });
        assert_eq!(
            fold(&source, &environment),
            Err(CopiedCallOperandError::UnsupportedConsumer),
            "{kind:?}"
        );
    }
}

/// A malformed call roster refuses at the record gate: operand numbers
/// that skip their position, a `Use` trailing the `Def` result, a
/// `tied_to` tie, an `early_clobber` mark, or a scalar-result record
/// carrying a second result all break the declared operand shape.
#[test]
fn malformed_rosters_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].operand = 7;
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedConsumer)
    );
    let source = mutated(target, |function, _| {
        let last = function.blocks[0].instructions[1].operands.len() - 1;
        function.blocks[0].instructions[1].operands[last].access = RegisterOperandAccess::Use;
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedConsumer)
    );
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].tied_to = Some(0);
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedConsumer)
    );
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].early_clobber = true;
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedConsumer)
    );
    let source = mutated(target, |function, _| {
        let call = &mut function.blocks[0].instructions[1];
        let last = *call.operands.last().unwrap();
        call.operands.push(SelectedOperand {
            operand: last.operand + 1,
            ..last
        });
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedConsumer)
    );
}

/// A record that diverges from its declared row — a stripped ABI view, a
/// dropped implicit use, a lost caller-saved clobber — refuses at the
/// row-contract gate: the rewritten record would silently republish a
/// surface the row does not own.
#[test]
fn row_divergence_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].fixed_view = None;
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::ConstraintMismatch)
    );
    let source = mutated(target, |function, _| {
        let consumer = &mut function.blocks[0].instructions[1];
        assert!(
            !consumer.implicit_uses.is_empty(),
            "the call row must implicitly use the stack pointer"
        );
        consumer.implicit_uses.pop();
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::ConstraintMismatch)
    );
    let source = mutated(target, |function, _| {
        let consumer = &mut function.blocks[0].instructions[1];
        assert!(
            !consumer.clobbers.is_empty(),
            "the call row must carry a caller-saved clobber roster"
        );
        consumer.clobbers.pop();
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::ConstraintMismatch)
    );
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].class = register_model::RegisterClassId(999);
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::ConstraintMismatch)
    );
}

/// Producers that are not the copy the operand reads refuse: a different
/// kind, a record carrying unit traffic, a source equal to the
/// destination, or a copy that is not the destination's last definition
/// before the call.
#[test]
fn wrong_producer_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A producer of another kind.
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
            value: semantic_vocabulary::IntegerValue::Unsigned(3),
        };
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedProducer)
    );
    // A decorated producer record — unit traffic a `CopyI64` cannot carry.
    let source = mutated(target, |function, environment| {
        let call_row = environment
            .constraint(function.blocks[0].instructions[1].constraint)
            .unwrap();
        let unit = call_row.implicit_uses[0];
        function.blocks[0].instructions[0].implicit_uses.push(unit);
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedProducer)
    );
    // A self-copy names no source the operand could rebind to.
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[0].virtual_register = DESTINATION;
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedProducer)
    );
    // A second copy defines the destination again between producer and
    // call: the named producer is not the last definition.
    let source = mutated(target, |function, environment| {
        let copy_row = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            1,
            instruction(
                EXTRA,
                SelectedInstructionKind::CopyI64,
                copy_row,
                &[SPARE, DESTINATION],
            ),
        );
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedProducer)
    );
    // The named producer is a body instruction but trails the call: no
    // in-block definition before the call carries the destination.
    let source = mutated(target, |function, _| {
        let copy = function.blocks[0].instructions.remove(0);
        function.blocks[0].instructions.push(copy);
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedProducer)
    );
}

/// The source register must survive to the call unchanged: an intervening
/// definition of it between the copy and the call refuses — a rebound
/// operand would observe the new value, not the copied one.
#[test]
fn source_redefinition_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy_row = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            1,
            instruction(
                EXTRA,
                SelectedInstructionKind::CopyI64,
                copy_row,
                &[SPARE, SOURCE],
            ),
        );
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedUse)
    );
}

/// A copy whose source register's class cannot carry the operand's
/// declared class refuses — the rebound operand would name a register of
/// the wrong class.
#[test]
fn class_mismatch_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        let class = function.blocks[0].instructions[1].operands[0].class;
        let other = register_model::RegisterClassId(class.0 + 1000);
        let entry = function
            .virtual_registers
            .iter_mut()
            .find(|entry| entry.id == SOURCE)
            .unwrap();
        entry.class = other;
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::ConstraintMismatch)
    );
}

/// An operand register missing from the function's roster refuses.
#[test]
fn missing_roster_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != DESTINATION);
    });
    assert_eq!(
        fold(&source, &environment),
        Err(CopiedCallOperandError::UnsupportedUse)
    );
}

/// A foreign environment or foreign catalog cannot attest this plan: the
/// wrong target refuses at the source gate, and a catalog bound to another
/// target's constraint catalog refuses at the surface gate.
#[test]
fn foreign_sources_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Call::Scalar);
    let foreign_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    let foreign_catalog = validated_machine_effect_catalog(
        NativeTarget::linux_arm64(),
        foreign_environment.constraints(),
    )
    .unwrap();
    assert_eq!(
        fold_selected_copied_call_operand(
            &source,
            0,
            COPY,
            CALL,
            &foreign_environment,
            &foreign_catalog,
            budget(),
        ),
        Err(CopiedCallOperandError::SourceMismatch)
    );
    assert_eq!(
        fold_selected_copied_call_operand(
            &source,
            0,
            COPY,
            CALL,
            &environment,
            &foreign_catalog,
            budget(),
        ),
        Err(CopiedCallOperandError::EffectSurfaceMismatch)
    );
    // A function index outside the plan, a consumer id that names no body
    // instruction, and a producer id that names none in the consumer's
    // block all refuse at their own gates.
    assert_eq!(
        fold_selected_copied_call_operand(
            &source,
            7,
            COPY,
            CALL,
            &environment,
            &catalog(&source, &environment),
            budget(),
        ),
        Err(CopiedCallOperandError::SourceMismatch)
    );
    assert_eq!(
        fold_selected_copied_call_operand(
            &source,
            0,
            COPY,
            SelectedInstructionId(77),
            &environment,
            &catalog(&source, &environment),
            budget(),
        ),
        Err(CopiedCallOperandError::UnsupportedConsumer)
    );
    assert_eq!(
        fold_selected_copied_call_operand(
            &source,
            0,
            SelectedInstructionId(77),
            CALL,
            &environment,
            &catalog(&source, &environment),
            budget(),
        ),
        Err(CopiedCallOperandError::UnsupportedProducer)
    );
    // The terminator's carried instruction is not a call form the family
    // declares.
    let terminator_id = match &source.transformed().functions[0].blocks[0].terminator {
        SelectedTerminator::Return { instruction, .. } => instruction.id,
        _ => panic!("the fixture terminator is a return"),
    };
    assert_eq!(
        fold_selected_copied_call_operand(
            &source,
            0,
            COPY,
            terminator_id,
            &environment,
            &catalog(&source, &environment),
            budget(),
        ),
        Err(CopiedCallOperandError::UnsupportedConsumer)
    );
}

/// An exhausted work budget refuses — the whole-function scan, the
/// producer scan, the last-definition walk, and the interval audit all
/// charge against `validation_steps`.
#[test]
fn exhausted_budget_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Call::Scalar);
    for steps in [1u64, 4] {
        let starved = OptimizationWorkBudget::new(100, 100, steps, 100, 100).unwrap();
        assert_eq!(
            fold_selected_copied_call_operand(
                &source,
                0,
                COPY,
                CALL,
                &environment,
                &catalog(&source, &environment),
                starved,
            ),
            Err(CopiedCallOperandError::WorkBudgetExceeded),
            "validation_steps={steps}"
        );
    }
    // The replay charges the same budget: a proposal produced on a full
    // budget cannot be validated on a starved one.
    let result = fold(&source, &environment).unwrap();
    let starved = OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap();
    assert_eq!(
        validate_copied_call_operand_fold(
            &source,
            0,
            COPY,
            CALL,
            &environment,
            &catalog(&source, &environment),
            starved,
            result.transformed().clone(),
        ),
        Err(CopiedCallOperandError::WorkBudgetExceeded)
    );
}

/// A proposal that is not exactly the rerouted record refuses replay: an
/// unmodified plan, a wrong register on the rebound operand, a partially
/// rebound operand list, a substituted kind, or a plan that diverges
/// anywhere outside the consumer record.
#[test]
fn mismatched_proposals_reject_replay() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Call::Scalar);
    // The unchanged plan is not a fold.
    assert_eq!(
        replay(&source, &environment, source.transformed().clone()),
        Err(CopiedCallOperandError::ReplayMismatch)
    );
    let mut wrong_register = source.transformed().clone();
    wrong_register.functions[0].blocks[0].instructions[1].operands[0].virtual_register = SPARE;
    assert_eq!(
        replay(&source, &environment, wrong_register),
        Err(CopiedCallOperandError::ReplayMismatch)
    );
    // Rebinding a `Def` operand on top of the real fold — the result
    // register is not an argument and may not move.
    let folded = fold(&source, &environment).unwrap();
    let mut wrong_access = folded.transformed().clone();
    let last = wrong_access.functions[0].blocks[0].instructions[1]
        .operands
        .len()
        - 1;
    wrong_access.functions[0].blocks[0].instructions[1].operands[last].virtual_register = SPARE;
    assert_eq!(
        replay(&source, &environment, wrong_access),
        Err(CopiedCallOperandError::ReplayMismatch)
    );
    // A kind substitution is not this fold.
    let mut wrong_kind = source.transformed().clone();
    wrong_kind.functions[0].blocks[0].instructions[1].kind = SelectedInstructionKind::CallScalar {
        callee: MachineId::new(41).unwrap(),
    };
    assert_eq!(
        replay(&source, &environment, wrong_kind),
        Err(CopiedCallOperandError::ReplayMismatch)
    );
    // Divergence outside the consumer record — the producer edited — is a
    // replay failure even when the consumer is correctly rewritten.
    let result = fold(&source, &environment).unwrap();
    let mut corrupted = result.transformed().clone();
    corrupted.functions[0].blocks[0].instructions[0].operands[0].virtual_register = SPARE;
    assert_eq!(
        replay(&source, &environment, corrupted),
        Err(CopiedCallOperandError::ReplayMismatch)
    );
}

/// A stale candidate refuses: once the fold lands, the producer's
/// destination no longer feeds the call, so a second fold of the same pair
/// has no operand to reroute.
#[test]
fn stale_candidate_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Call::Scalar);
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        fold_selected_copied_call_operand(
            &result,
            0,
            COPY,
            CALL,
            &environment,
            &catalog(&result, &environment),
            budget(),
        ),
        Err(CopiedCallOperandError::UnsupportedUse)
    );
}

/// The fold is deterministic and terminal: two folds of the same source
/// publish identical plans, and the folded plan carries no second fold for
/// this pair.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Call::Scalar);
    let first = fold(&source, &environment).unwrap();
    let second = fold(&source, &environment).unwrap();
    assert_eq!(first.transformed(), second.transformed());
    assert_eq!(first.receipt(), second.receipt());
}
