//! Selected-operand/ABI replay coverage for machine-effect validation.
//!
//! The effect sidecar carries no explicit operands, so this boundary is the
//! last stage that can reject an operand row that drifted from the target
//! constraint row after selection and rewrites. These witnesses corrupt one
//! operand-contract field at a time against real target constraint rows.

use optimization_unit::{FuelSettlement, PsiProvenance};
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::{
    RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess, RegisterViewId,
    ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, SelectedBlock,
    SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand,
    SelectedTerminator, ValidatedMachineEffectCatalog, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
};
use target::NativeTarget;

use super::{MachineEffectError, validate_function, validate_instruction};

fn environment() -> ValidatedTargetRegisterEnvironment {
    baseline_target_register_environment(NativeTarget::linux_x64())
        .expect("baseline x86-64 environment")
}

fn catalog(environment: &ValidatedTargetRegisterEnvironment) -> ValidatedMachineEffectCatalog {
    isa_x86_64::validate_x86_64_machine_effect_catalog(
        environment.target(),
        environment.constraints(),
        isa_x86_64::x86_64_machine_effect_catalog(environment.target(), environment.constraints())
            .expect("x86-64 machine-effect catalog"),
    )
    .expect("validated x86-64 machine-effect catalog")
}

fn compare_row(environment: &ValidatedTargetRegisterEnvironment) -> RegisterInstructionConstraint {
    environment
        .constraint(environment.allocation_constraint_keys().compare_i64)
        .expect("compare_i64 constraint row")
        .clone()
}

fn scalar_call_row(
    environment: &ValidatedTargetRegisterEnvironment,
) -> RegisterInstructionConstraint {
    environment
        .constraint(environment.allocation_constraint_keys().call_scalar[2])
        .expect("two-argument scalar-call constraint row")
        .clone()
}

/// A function whose virtual registers and one instruction are rebuilt from the
/// target constraint row, plus the effect row the catalog declares for it.
fn fixture(
    kind: SelectedInstructionKind,
    row: &RegisterInstructionConstraint,
    catalog: &ValidatedMachineEffectCatalog,
) -> (
    SelectedFunction,
    SelectedInstruction,
    InstructionMachineEffects,
) {
    let instruction = SelectedInstruction {
        id: SelectedInstructionId(0),
        kind,
        constraint: row.key,
        operands: row
            .operands
            .iter()
            .enumerate()
            .map(|(index, operand)| SelectedOperand {
                operand: operand.operand,
                virtual_register: VirtualRegisterId(index as u32),
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
        provenance: SelectedInstructionProvenance::default(),
    };
    let virtual_registers = row
        .operands
        .iter()
        .enumerate()
        .map(|(index, operand)| VirtualRegister {
            id: VirtualRegisterId(index as u32),
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 scalar type"),
            ),
            class: operand.class,
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction: instruction.id,
                operand: operand.operand,
            },
            definition_site: None,
            entry_fixed_view: None,
        })
        .collect();
    let function = SelectedFunction {
        machine: MachineId::new(1).expect("machine id"),
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
        virtual_registers,
        blocks: Vec::new(),
    };
    let declaration = catalog
        .catalog()
        .declarations
        .iter()
        .find(|declaration| declaration.constraint == row.key)
        .expect("effect declaration for the selected constraint");
    let expected = InstructionMachineEffects {
        instruction: instruction.id,
        kind,
        constraint: row.key,
        unit_uses: row.implicit_uses.clone(),
        unit_defs: row.implicit_defs.clone(),
        unit_clobbers: row.clobbers.clone(),
        memory: declaration.memory,
        trap: declaration.trap,
        barrier: declaration.barrier,
        call: declaration.call,
        cleanup: declaration.cleanup,
        provenance: instruction.provenance.clone(),
        alternatives: declaration.alternatives.clone(),
    };
    (function, instruction, expected)
}

fn validate(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    actual: &InstructionMachineEffects,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), MachineEffectError> {
    validate_instruction(function, instruction, actual, constraints, catalog)
}

#[test]
fn constraint_operands_pass_replay_for_arithmetic_and_call_rows() {
    let environment = environment();
    let catalog = catalog(&environment);
    for (kind, row) in [
        (
            SelectedInstructionKind::CompareI64,
            compare_row(&environment),
        ),
        (
            SelectedInstructionKind::CallScalar {
                callee: MachineId::new(2).expect("callee id"),
            },
            scalar_call_row(&environment),
        ),
    ] {
        let (function, instruction, expected) = fixture(kind, &row, &catalog);
        assert_eq!(
            validate(
                &function,
                &instruction,
                &expected,
                environment.constraints(),
                &catalog,
            ),
            Ok(())
        );
    }
}

#[test]
fn drifted_operand_fields_are_rejected() {
    let environment = environment();
    let catalog = catalog(&environment);
    let row = compare_row(&environment);
    let (function, instruction, expected) =
        fixture(SelectedInstructionKind::CompareI64, &row, &catalog);

    let mut renumbered = instruction.clone();
    renumbered.operands[0].operand = 7;
    assert_operand_mismatch(&function, &renumbered, &expected, &environment, &catalog);

    let mut reaccessed = instruction.clone();
    reaccessed.operands[0].access = RegisterOperandAccess::Def;
    assert_operand_mismatch(&function, &reaccessed, &expected, &environment, &catalog);

    let mut reclassed = instruction.clone();
    reclassed.operands[0].class = RegisterClassId(row.operands[0].class.0 + 1);
    assert_operand_mismatch(&function, &reclassed, &expected, &environment, &catalog);

    let mut reordered = instruction.clone();
    reordered.operands[0].fixed_view = Some(RegisterViewId(0));
    assert_operand_mismatch(&function, &reordered, &expected, &environment, &catalog);

    let mut tied = instruction.clone();
    tied.operands[1].tied_to = Some(0);
    assert_operand_mismatch(&function, &tied, &expected, &environment, &catalog);

    let mut early = instruction.clone();
    early.operands[1].early_clobber = true;
    assert_operand_mismatch(&function, &early, &expected, &environment, &catalog);

    let mut truncated = instruction.clone();
    truncated.operands.pop();
    assert_operand_mismatch(&function, &truncated, &expected, &environment, &catalog);
}

#[test]
fn drifted_call_operand_views_and_clobber_shape_are_rejected() {
    let environment = environment();
    let catalog = catalog(&environment);
    let row = scalar_call_row(&environment);
    assert!(
        row.operands
            .iter()
            .any(|operand| operand.fixed_view.is_some()),
        "scalar-call constraint row must pin ABI register views"
    );
    let (function, instruction, expected) = fixture(
        SelectedInstructionKind::CallScalar {
            callee: MachineId::new(2).expect("callee id"),
        },
        &row,
        &catalog,
    );

    let mut unpinned = instruction.clone();
    let argument = unpinned
        .operands
        .iter_mut()
        .find(|operand| operand.fixed_view.is_some())
        .expect("pinned call operand");
    argument.fixed_view = None;
    assert_operand_mismatch(&function, &unpinned, &expected, &environment, &catalog);

    let mut rebound = instruction.clone();
    let argument = rebound
        .operands
        .iter_mut()
        .find(|operand| operand.fixed_view.is_some())
        .expect("pinned call operand");
    argument.fixed_view = Some(RegisterViewId(argument.fixed_view.unwrap().0 + 1));
    assert_operand_mismatch(&function, &rebound, &expected, &environment, &catalog);

    let mut clobbered = instruction.clone();
    clobbered.clobbers.pop();
    assert!(matches!(
        validate(
            &function,
            &clobbered,
            &expected,
            environment.constraints(),
            &catalog,
        ),
        Err(MachineEffectError::ConstraintEffectMismatch { .. })
    ));
}

#[test]
fn operand_registers_must_resolve_in_the_function_register_table() {
    let environment = environment();
    let catalog = catalog(&environment);
    let row = compare_row(&environment);
    let (function, instruction, expected) =
        fixture(SelectedInstructionKind::CompareI64, &row, &catalog);

    let mut dangling = instruction.clone();
    dangling.operands[0].virtual_register = VirtualRegisterId(4096);
    assert_operand_mismatch(&function, &dangling, &expected, &environment, &catalog);

    let mut misclassified = function.clone();
    misclassified.virtual_registers[0].class = RegisterClassId(row.operands[0].class.0 + 1);
    assert_operand_mismatch(
        &misclassified,
        &instruction,
        &expected,
        &environment,
        &catalog,
    );

    let mut shuffled = function.clone();
    shuffled.virtual_registers[0].id = VirtualRegisterId(1);
    assert_operand_mismatch(&shuffled, &instruction, &expected, &environment, &catalog);
}

#[test]
fn fuel_settlements_replay_against_claimed_provenance_sites() {
    let environment = environment();
    let catalog = catalog(&environment);
    let row = compare_row(&environment);
    let (function, mut instruction, mut expected) =
        fixture(SelectedInstructionKind::CompareI64, &row, &catalog);
    let operation = OperationId::new(2).expect("operation id");
    let edge = EdgeId::new(3).expect("edge id");
    instruction.provenance = SelectedInstructionProvenance {
        operations: vec![operation],
        edges: vec![edge],
        fuel: vec![
            FuelSettlement {
                site: PsiProvenance::Operation(operation),
                units: 7,
            },
            FuelSettlement {
                site: PsiProvenance::Edge(edge),
                units: 11,
            },
        ],
        ..Default::default()
    };
    expected.provenance = instruction.provenance.clone();
    assert_eq!(
        validate(
            &function,
            &instruction,
            &expected,
            environment.constraints(),
            &catalog,
        ),
        Ok(())
    );

    for corruption in 0..4 {
        let mut forged = instruction.clone();
        match corruption {
            // Fuel settled against an operation the instruction never claims.
            0 => forged.provenance.fuel.push(FuelSettlement {
                site: PsiProvenance::Operation(OperationId::new(41).expect("operation id")),
                units: 1,
            }),
            // Fuel settled against an edge the instruction never claims.
            1 => forged.provenance.fuel.push(FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(43).expect("edge id")),
                units: 1,
            }),
            // A claimed site may not settle twice.
            2 => forged.provenance.fuel.push(forged.provenance.fuel[0]),
            // A claimed site still owes a nonzero charge.
            _ => forged.provenance.fuel[1].units = 0,
        }
        assert!(
            matches!(
                validate(
                    &function,
                    &forged,
                    &expected,
                    environment.constraints(),
                    &catalog,
                ),
                Err(MachineEffectError::FuelProvenanceMismatch { .. })
            ),
            "corruption {corruption} must be rejected"
        );
    }
}

#[test]
fn function_replay_reaches_terminator_operand_rows() {
    let environment = environment();
    let catalog = catalog(&environment);
    let compare = compare_row(&environment);
    let return_row = environment
        .constraint(environment.allocation_constraint_keys().return_i64)
        .expect("return_i64 constraint row")
        .clone();
    let (mut function, instruction, compare_effects) =
        fixture(SelectedInstructionKind::CompareI64, &compare, &catalog);
    let base_registers = function.virtual_registers.len() as u32;
    function
        .virtual_registers
        .extend(
            return_row
                .operands
                .iter()
                .enumerate()
                .map(|(index, operand)| VirtualRegister {
                    id: VirtualRegisterId(base_registers + index as u32),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 scalar type"),
                    ),
                    class: operand.class,
                    origin: VirtualRegisterOrigin::InstructionScratch {
                        instruction: SelectedInstructionId(1),
                        operand: operand.operand,
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                }),
        );
    let terminator = SelectedInstruction {
        id: SelectedInstructionId(1),
        kind: SelectedInstructionKind::ReturnScalar,
        constraint: return_row.key,
        operands: return_row
            .operands
            .iter()
            .enumerate()
            .map(|(index, operand)| SelectedOperand {
                operand: operand.operand,
                virtual_register: VirtualRegisterId(base_registers + index as u32),
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: return_row.implicit_uses.clone(),
        implicit_defs: return_row.implicit_defs.clone(),
        clobbers: return_row.clobbers.clone(),
        provenance: SelectedInstructionProvenance::default(),
    };
    let declaration = catalog
        .catalog()
        .declarations
        .iter()
        .find(|declaration| declaration.constraint == return_row.key)
        .expect("effect declaration for the return constraint");
    let return_effects = InstructionMachineEffects {
        instruction: terminator.id,
        kind: terminator.kind,
        constraint: terminator.constraint,
        unit_uses: terminator.implicit_uses.clone(),
        unit_defs: terminator.implicit_defs.clone(),
        unit_clobbers: terminator.clobbers.clone(),
        memory: declaration.memory,
        trap: declaration.trap,
        barrier: declaration.barrier,
        call: declaration.call,
        cleanup: declaration.cleanup,
        provenance: terminator.provenance.clone(),
        alternatives: declaration.alternatives.clone(),
    };
    function.blocks = vec![SelectedBlock {
        id: SelectedBlockId(0),
        origin: SelectedBlockOrigin::Source(BlockId::new(1).expect("block id")),
        instructions: vec![instruction],
        terminator: SelectedTerminator::Return {
            instruction: terminator,
            psi_return_edge: EdgeId::new(1).expect("edge id"),
        },
    }];
    let actual = FunctionMachineEffects {
        machine: function.machine,
        blocks: vec![BlockMachineEffects {
            block: SelectedBlockId(0),
            instructions: vec![compare_effects, return_effects],
        }],
    };
    assert_eq!(
        validate_function(&function, &actual, environment.constraints(), &catalog),
        Ok(())
    );

    let SelectedTerminator::Return { instruction, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    instruction.operands[0].access = RegisterOperandAccess::Def;
    assert!(matches!(
        validate_function(&function, &actual, environment.constraints(), &catalog),
        Err(MachineEffectError::ConstraintOperandMismatch { .. })
    ));
}

fn assert_operand_mismatch(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    actual: &InstructionMachineEffects,
    environment: &ValidatedTargetRegisterEnvironment,
    catalog: &ValidatedMachineEffectCatalog,
) {
    assert!(
        matches!(
            validate(
                function,
                instruction,
                actual,
                environment.constraints(),
                catalog,
            ),
            Err(MachineEffectError::ConstraintOperandMismatch { .. })
        ),
        "corrupted operand contract must be rejected"
    );
}
