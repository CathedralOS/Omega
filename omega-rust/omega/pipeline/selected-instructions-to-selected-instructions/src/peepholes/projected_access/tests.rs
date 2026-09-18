use crate::ProjectedAccessError;
use crate::ProjectedAccessReceipt;
use crate::ValidatedProjectedAccess;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_projected_access;
use crate::validate_projected_access_fold;
use crate::validated_machine_effect_catalog;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, ValidatedMachineEffectCatalog,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
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

const OFFSET: SelectedInstructionId = SelectedInstructionId(2);
const ACCESS: SelectedInstructionId = SelectedInstructionId(4);
const EXTRA: SelectedInstructionId = SelectedInstructionId(7);
const BASE: VirtualRegisterId = VirtualRegisterId(0);
const POINTER: VirtualRegisterId = VirtualRegisterId(1);
const RESULT: VirtualRegisterId = VirtualRegisterId(2);
const VALUE: VirtualRegisterId = VirtualRegisterId(3);
const SPARE: VirtualRegisterId = VirtualRegisterId(4);

/// Which displacement-carrying access the fixture's consumer carries.
#[derive(Clone, Copy)]
enum Access {
    Load8,
    Load16,
    Load32,
    Load64,
    Store(u8),
}

impl Access {
    fn kind(self, byte_offset: u32) -> SelectedInstructionKind {
        match self {
            Self::Load8 => SelectedInstructionKind::Load8 { byte_offset },
            Self::Load16 => SelectedInstructionKind::Load16 { byte_offset },
            Self::Load32 => SelectedInstructionKind::Load32 { byte_offset },
            Self::Load64 => SelectedInstructionKind::Load64 { byte_offset },
            Self::Store(byte_size) => SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            },
        }
    }

    fn key(
        self,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> register_model::RegisterConstraintKey {
        let keys = environment.selected_keys();
        match self {
            Self::Load8 => keys.load8,
            Self::Load16 => keys.load16,
            Self::Load32 => keys.load32,
            Self::Load64 => keys.load64,
            Self::Store(_) => keys.store,
        }
        .expect("declared access form has a constraint key on this target")
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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `pointer = address_offset base, producer_offset; access pointer,
/// consumer_offset` in the entry block, returning Unit. The producer's
/// projected register may be read by other consumers; the fold rebinds only
/// the named access's operand 0 and sums the displacements. Variants edit
/// the single fixture function through `mutated`.
fn fixture(
    target: NativeTarget,
    access: Access,
    producer_offset: u32,
    consumer_offset: u32,
) -> ValidatedProjectedAccess {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let projection = environment
        .constraint(keys.address_offset.unwrap())
        .unwrap();
    let access_row = environment.constraint(access.key(&environment)).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = projection.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let registers = vec![
        VirtualRegister {
            id: BASE,
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
            POINTER,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: OFFSET,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        register(
            RESULT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: ACCESS,
                source_value: ValueId::new(4).unwrap(),
            },
        ),
        VirtualRegister {
            id: VALUE,
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
    let tail = match access {
        Access::Store(_) => VALUE,
        _ => RESULT,
    };
    let mut consumer = instruction(
        ACCESS,
        access.kind(consumer_offset),
        access_row,
        &[POINTER, tail],
    );
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
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    instruction(
                        OFFSET,
                        SelectedInstructionKind::AddressOffset {
                            byte_offset: producer_offset,
                        },
                        projection,
                        &[BASE, POINTER],
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
    ValidatedProjectedAccess {
        receipt: ProjectedAccessReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

/// The machine-effect catalog bound to `environment` for the plan's target —
/// the catalog the pair declarations resolve their effect surfaces in.
fn catalog(
    source: &ValidatedProjectedAccess,
    environment: &ValidatedTargetRegisterEnvironment,
) -> ValidatedMachineEffectCatalog {
    validated_machine_effect_catalog(source.transformed().target, environment.constraints())
        .unwrap()
}

/// Edit the single fixture function, then refresh the receipt identities so
/// the mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
) -> ValidatedProjectedAccess {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target, Access::Load8, 8, 16);
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
    source: &ValidatedProjectedAccess,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedProjectedAccess, ProjectedAccessError> {
    let effect_catalog = catalog(source, environment);
    fold_selected_projected_access(source, 0, ACCESS, environment, &effect_catalog, budget())
}

fn replay(
    source: &ValidatedProjectedAccess,
    environment: &ValidatedTargetRegisterEnvironment,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedProjectedAccess, ProjectedAccessError> {
    let effect_catalog = catalog(source, environment);
    validate_projected_access_fold(
        source,
        0,
        ACCESS,
        environment,
        &effect_catalog,
        budget(),
        proposed,
    )
}

/// The record a successful fold publishes: `access` at the summed
/// displacement reading `base` on operand 0, everything else verbatim.
fn folded_to(result: &ValidatedProjectedAccess, expected_kind: SelectedInstructionKind) {
    let instruction = &result.transformed().functions[0].blocks[0].instructions[1];
    assert_eq!(instruction.id, ACCESS);
    assert_eq!(instruction.kind, expected_kind);
    assert_eq!(instruction.operands[0].virtual_register, BASE);
}

/// Every declared load pair folds on every target: the access keeps its
/// own kind at the summed displacement, operand 0 reads the projection's
/// base, the tail `Def` result carries verbatim, the producer stays
/// published, and the independent replay re-derives all of it without the
/// descriptor table.
#[test]
fn load_pairs_fold_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for access in [
            Access::Load8,
            Access::Load16,
            Access::Load32,
            Access::Load64,
        ] {
            let source = fixture(target, access, 8, 16);
            let result = fold(&source, &environment).unwrap();
            folded_to(&result, access.kind(24));
            let consumer = &result.transformed().functions[0].blocks[0].instructions[1];
            // The folded record keeps the consumer's row, tail operand,
            // implicit surfaces, and provenance verbatim.
            let source_consumer = &source.transformed().functions[0].blocks[0].instructions[1];
            assert_eq!(consumer.constraint, source_consumer.constraint);
            assert_eq!(consumer.operands[1], source_consumer.operands[1]);
            assert_eq!(consumer.implicit_uses, source_consumer.implicit_uses);
            assert_eq!(consumer.implicit_defs, source_consumer.implicit_defs);
            assert_eq!(consumer.clobbers, source_consumer.clobbers);
            assert_eq!(consumer.provenance, source_consumer.provenance);
            // The producer stays: the projection keeps publishing its
            // register for every other reader.
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
}

/// The declared store pair — the family's memory-writing consumer — folds
/// at every byte-addressable width on every target, retaining the
/// `WritePointerV1`/`MayArchitecturalFaultV1` surface the dereference
/// carries.
#[test]
fn store_pairs_fold_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for byte_size in [1u8, 2, 4, 8] {
            let access = Access::Store(byte_size);
            let source = fixture(target, access, 8, 16);
            let result = fold(&source, &environment).unwrap();
            folded_to(&result, access.kind(24));
            // The store's operand 1 — the stored `Use` — carries verbatim.
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[1].operands[1]
                    .virtual_register,
                VALUE
            );
            replay(&source, &environment, result.transformed().clone()).unwrap();
        }
    }
}

/// The declared bound is the access-scaled unsigned immediate every target
/// shares: the exact boundary — width times 4095 — folds on both host
/// architectures.
#[test]
fn displacement_bound_boundary_folds() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (access, producer_offset, consumer_offset) in [
            (Access::Load8, 4000u32, 95u32), // 4095 = 1 * 4095
            (Access::Load16, 4094, 4096),    // 8190 = 2 * 4095
            (Access::Load32, 4092, 12288),   // 16380 = 4 * 4095
            (Access::Load64, 32752, 8),      // 32760 = 8 * 4095
            (Access::Store(8), 0, 32760),    // store at the boundary
        ] {
            let source = fixture(target, access, producer_offset, consumer_offset);
            let result = fold(&source, &environment).unwrap();
            folded_to(&result, access.kind(producer_offset + consumer_offset));
        }
    }
}

/// A displacement past the declared bound — or not a multiple of the access
/// width — is not a form every target's encoding admits, and the pair
/// refuses rather than narrowing the access.
#[test]
fn out_of_bound_displacement_rejects() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (access, producer_offset, consumer_offset) in [
            (Access::Load8, 4095u32, 1u32),       // 4096 > 4095
            (Access::Load8, u32::MAX, 1),         // the sum overflows
            (Access::Load16, 8, 5),               // 13 is not a multiple of 2
            (Access::Load16, 8, 8190 - 8 + 4),    // over the scaled bound
            (Access::Load64, 32768, 0),           // quotient 4096
            (Access::Store(4), 8, 16384 - 8 + 4), // over the scaled bound
        ] {
            let source = fixture(target, access, producer_offset, consumer_offset);
            assert_eq!(
                fold(&source, &environment).unwrap_err(),
                ProjectedAccessError::UnsupportedDisplacement,
                "{target:?} {producer_offset} + {consumer_offset}"
            );
        }
    }
}

/// An intervening read — even of the projected register or the base —
/// does not touch either definition, so the pair still folds.
#[test]
fn intervening_reads_fold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        // Read the projected pointer into a spare register between the
        // producer and the consumer.
        function.blocks[0].instructions.insert(
            1,
            instruction(
                EXTRA,
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, SPARE],
            ),
        );
    });
    let result = fold(&source, &environment).unwrap();
    let consumer = &result.transformed().functions[0].blocks[0].instructions[2];
    assert_eq!(
        consumer.kind,
        SelectedInstructionKind::Load8 { byte_offset: 24 }
    );
    assert_eq!(consumer.operands[0].virtual_register, BASE);
}

/// The pointer's last definition before the consumer is what the pair
/// binds: when a different instruction redefines the pointer after the
/// projection, there is no `AddressOffset` last definition to fold.
#[test]
fn later_pointer_definition_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            1,
            instruction(
                EXTRA,
                SelectedInstructionKind::CopyI64,
                copy,
                &[SPARE, POINTER],
            ),
        );
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedProducer
    );
}

/// A definition of the *base* inside the producer–consumer interval leaves
/// the folded operand observing the wrong value — the boundary the
/// descriptor's operand-shape axis names — so the pair refuses.
#[test]
fn base_redefinition_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            1,
            instruction(
                EXTRA,
                SelectedInstructionKind::CopyI64,
                copy,
                &[VALUE, BASE],
            ),
        );
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedUse
    );
}

/// A projection that rewrote its own base (`base == pointer`) reads the
/// projected value, not the base — the shifted sum would be wrong.
#[test]
fn self_projecting_producer_rejects() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[0].virtual_register = POINTER;
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedProducer
    );
}

/// The pointer's definition must live in the consumer's own block: a
/// projection settling across a block boundary — the register flowing in
/// through an edge — is not the in-block last definition the grammar names.
#[test]
fn cross_block_projection_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        // The entry block keeps only the projection and jumps to a second
        // block carrying the access.
        let consumer = function.blocks[0].instructions.remove(1);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(EXTRA, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: SelectedSuccessor {
                role: SelectedSuccessorRole::Semantic,
                psi_edge: EdgeId::new(6).unwrap(),
                block: SelectedBlockId(1),
                source_target: BlockId::new(2).unwrap(),
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                structural_case: None,
                fuel: Vec::new(),
            },
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![consumer],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(9),
                    SelectedInstructionKind::ReturnUnit,
                    &terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        });
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedProducer
    );
}

/// A consumer whose kind no rule names — the indexed load folds a literal
/// into an index operand under a different relationship — is not a
/// projected access.
#[test]
fn undeclared_consumer_kinds_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The indexed load is a declared pair elsewhere but not here: its
    // operand 1 is an index `Use`, not a byte-offset fold.
    let indexed = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().load8_indexed.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            ACCESS,
            SelectedInstructionKind::Load8Indexed,
            &row,
            &[POINTER, BASE, RESULT],
        );
    });
    assert_eq!(
        fold(&indexed, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
    // The frame store's operand 0 is the stored value, not a pointer — and
    // its row carries the implicit stack-pointer use the register-only unit
    // flow cannot name.
    let frame_store = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            ACCESS,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Incoming {
                    parameter_index: 0,
                    abi_stack_byte_offset: 0,
                },
                byte_offset: 16,
            },
            &row,
            &[POINTER],
        );
    });
    assert_eq!(
        fold(&frame_store, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
    // A producer-kind record is not a consumer at all.
    assert_eq!(
        fold_selected_projected_access(
            &fixture(target, Access::Load8, 8, 16),
            0,
            OFFSET,
            &environment,
            &catalog(&fixture(target, Access::Load8, 8, 16), &environment),
            budget(),
        )
        .unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
    // The terminator-carried instruction is not a body consumer.
    assert_eq!(
        fold_selected_projected_access(
            &fixture(target, Access::Load8, 8, 16),
            0,
            SelectedInstructionId(9),
            &environment,
            &catalog(&fixture(target, Access::Load8, 8, 16), &environment),
            budget(),
        )
        .unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
    // An instruction id the function does not carry is not a consumer.
    assert_eq!(
        fold_selected_projected_access(
            &fixture(target, Access::Load8, 8, 16),
            0,
            SelectedInstructionId(99),
            &environment,
            &catalog(&fixture(target, Access::Load8, 8, 16), &environment),
            budget(),
        )
        .unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
}

/// A `Store` at a width no target encodes is not the emitted referent
/// store the pair declares.
#[test]
fn unencodable_store_width_rejects() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    for byte_size in [0u8, 3, 16] {
        let source = fixture(target, Access::Store(byte_size), 8, 16);
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            ProjectedAccessError::UnsupportedConsumer,
            "{byte_size}"
        );
    }
}

/// Operand unit bindings the rebuilt record would silently keep are not
/// the register-only unit flow the pair declares: a pinned consumer
/// operand refuses, and so does a decorated producer operand.
#[test]
fn decorated_operands_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let pinned_consumer = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        fold(&pinned_consumer, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
    let tied_producer = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[1].tied_to = Some(0);
    });
    assert_eq!(
        fold(&tied_producer, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedProducer
    );
    let early_clobber_consumer = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[1].early_clobber = true;
    });
    assert_eq!(
        fold(&early_clobber_consumer, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
}

/// Implicit unit traffic is not the register-only surface: a consumer that
/// reads or writes units implicitly, and a producer that clobbers a unit,
/// both refuse.
#[test]
fn implicit_unit_traffic_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let using_consumer = mutated(target, |function, environment| {
        let unit = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .implicit_defs
            .first()
            .copied()
            .unwrap();
        function.blocks[0].instructions[1].implicit_uses.push(unit);
    });
    assert_eq!(
        fold(&using_consumer, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
    let clobbering_producer = mutated(target, |function, environment| {
        let unit = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .implicit_defs
            .first()
            .copied()
            .unwrap();
        function.blocks[0].instructions[0].clobbers.push(unit);
    });
    assert_eq!(
        fold(&clobbering_producer, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedProducer
    );
}

/// The operand grammar is positional: a consumer whose operand 0 is not
/// the pointer `Use`, or whose tail access is not the declared one, is not
/// the emitted access form.
#[test]
fn operand_shape_mismatch_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let tail_defined = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[1].access = RegisterOperandAccess::Use;
    });
    assert_eq!(
        fold(&tail_defined, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
    let extra_operand = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        let mut consumer = function.blocks[0].instructions[1].clone();
        consumer.operands.push(SelectedOperand {
            operand: 2,
            virtual_register: SPARE,
            access: RegisterOperandAccess::Use,
            class: row.operands[0].class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        function.blocks[0].instructions[1] = consumer;
    });
    assert_eq!(
        fold(&extra_operand, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedConsumer
    );
}

/// A class the operand publishes that its constraint row does not declare
/// — or a base register whose roster class the rebound operand cannot
/// carry — is not the declared contract.
#[test]
fn class_mismatch_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let foreign_class = register_model::RegisterClassId(u16::MAX);
    // The consumer's operand-0 class no longer matches its constraint row.
    let consumer_operand_class = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].class = foreign_class;
    });
    assert_eq!(
        fold(&consumer_operand_class, &environment).unwrap_err(),
        ProjectedAccessError::ConstraintMismatch
    );
    // The producer's base operand class no longer matches its row.
    let producer_operand_class = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[0].class = foreign_class;
    });
    assert_eq!(
        fold(&producer_operand_class, &environment).unwrap_err(),
        ProjectedAccessError::ConstraintMismatch
    );
    // The base register's roster class is not the consumer operand's
    // declared class: the rebound operand would publish the wrong class.
    let base_roster_class = mutated(target, |function, _| {
        function
            .virtual_registers
            .iter_mut()
            .find(|entry| entry.id == BASE)
            .unwrap()
            .class = foreign_class;
    });
    assert_eq!(
        fold(&base_roster_class, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedUse
    );
}

/// A plan or environment from another target cannot admit this pair: the
/// bound catalog, constraints, and selected keys describe exactly one
/// target's rows.
#[test]
fn foreign_sources_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Access::Load8, 8, 16);
    // An environment for another architecture.
    let foreign_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    let foreign_catalog = validated_machine_effect_catalog(
        NativeTarget::linux_arm64(),
        foreign_environment.constraints(),
    )
    .unwrap();
    assert_eq!(
        fold_selected_projected_access(
            &source,
            0,
            ACCESS,
            &foreign_environment,
            &foreign_catalog,
            budget(),
        )
        .unwrap_err(),
        ProjectedAccessError::SourceMismatch
    );
    // A catalog bound to a different target than the plan's.
    let foreign_target_catalog =
        validated_machine_effect_catalog(NativeTarget::windows_x64(), environment.constraints())
            .unwrap();
    assert_eq!(
        fold_selected_projected_access(
            &source,
            0,
            ACCESS,
            &environment,
            &foreign_target_catalog,
            budget(),
        )
        .unwrap_err(),
        ProjectedAccessError::EffectSurfaceMismatch
    );
}

/// A starved validation budget refuses the fold — the consumer location
/// scan, the last-definition scan, and the interval audit all charge.
#[test]
fn exhausted_budget_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Access::Load8, 8, 16);
    let effect_catalog = catalog(&source, &environment);
    assert_eq!(
        fold_selected_projected_access(
            &source,
            0,
            ACCESS,
            &environment,
            &effect_catalog,
            OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
        )
        .unwrap_err(),
        ProjectedAccessError::WorkBudgetExceeded
    );
    // The replay pays the same charge even on an already-admitted shape.
    let proposed = fold(&source, &environment).unwrap();
    assert_eq!(
        validate_projected_access_fold(
            &source,
            0,
            ACCESS,
            &environment,
            &effect_catalog,
            OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
            proposed.transformed().clone(),
        )
        .unwrap_err(),
        ProjectedAccessError::WorkBudgetExceeded
    );
}

/// The replay's reconstruction is the contract: a proposal that rewrote
/// anything else — the wrong displacement, the wrong register, a missing
/// producer, an untouched consumer — is not this fold.
#[test]
fn mismatched_proposals_reject_replay() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Access::Load8, 8, 16);
    let folded = fold(&source, &environment).unwrap();
    // A proposal equal to the untouched source is not the fold.
    assert_eq!(
        replay(&source, &environment, source.transformed().clone()).unwrap_err(),
        ProjectedAccessError::ReplayMismatch
    );
    // A wrong combined displacement.
    let mut wrong_offset = (*folded.transformed()).clone();
    wrong_offset.functions[0].blocks[0].instructions[1].kind =
        SelectedInstructionKind::Load8 { byte_offset: 25 };
    assert_eq!(
        replay(&source, &environment, wrong_offset).unwrap_err(),
        ProjectedAccessError::ReplayMismatch
    );
    // A rebound operand naming the wrong register.
    let mut wrong_register = (*folded.transformed()).clone();
    wrong_register.functions[0].blocks[0].instructions[1].operands[0].virtual_register = VALUE;
    assert_eq!(
        replay(&source, &environment, wrong_register).unwrap_err(),
        ProjectedAccessError::ReplayMismatch
    );
    // A proposal that also removed the retained producer.
    let mut missing_producer = (*folded.transformed()).clone();
    missing_producer.functions[0].blocks[0]
        .instructions
        .remove(0);
    assert_eq!(
        replay(&source, &environment, missing_producer).unwrap_err(),
        ProjectedAccessError::ReplayMismatch
    );
    // A proposal that changed a different instruction too.
    let mut extra_change = (*folded.transformed()).clone();
    extra_change.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::AddressOffset { byte_offset: 40 };
    assert_eq!(
        replay(&source, &environment, extra_change).unwrap_err(),
        ProjectedAccessError::ReplayMismatch
    );
}

/// A stale candidate — the same instruction revisited after the fold —
/// no longer reads the projected register: operand 0 now names the base
/// directly, whose last in-block definition is not the projection.
#[test]
fn stale_candidate_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Access::Load8, 8, 16);
    let folded = fold(&source, &environment).unwrap();
    assert_eq!(
        fold(&folded, &environment).unwrap_err(),
        ProjectedAccessError::UnsupportedProducer
    );
}

/// The published result is itself a legal analysis input: the receipts
/// bind the source and transformed plan identities, and the fold is
/// deterministic across a detached re-publication.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, Access::Load64, 32, 64);
    let first = fold(&source, &environment).unwrap();
    let second = fold(&source, &environment).unwrap();
    assert_eq!(first.transformed(), second.transformed());
    assert_eq!(first.receipt(), second.receipt());
    // The transformed plan publishes as an independent input: replaying
    // against it re-derives the same relationship by content.
    replay(&source, &environment, first.transformed().clone()).unwrap();
}
