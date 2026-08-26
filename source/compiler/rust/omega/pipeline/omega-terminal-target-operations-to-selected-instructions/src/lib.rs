#![forbid(unsafe_code)]

//! Instruction selection for the first production clean-Terminal virtual
//! register CFG slice.
//!
//! Selection and validation are separate steps. The public producer returns
//! only the opaque validated carrier and makes no liveness or allocation claim.

use std::collections::BTreeSet;

use omega_optimization_unit::{
    FuelSettlement, PsiOptimizationUnit, PsiProvenance, ValueDefinitionSite,
};
use omega_register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
};
use omega_terminal_abstract_operations::TerminalAbstractOperationPlan;
use omega_terminal_selected_instructions::{
    TerminalSelectedBlock, TerminalSelectedBlockId, TerminalSelectedConstraintKeys,
    TerminalSelectedFixedInputConstraint, TerminalSelectedFunction, TerminalSelectedInstruction,
    TerminalSelectedInstructionId, TerminalSelectedInstructionKind,
    TerminalSelectedInstructionPlan, TerminalSelectedInstructionPlanIdentity,
    TerminalSelectedInstructionProvenance, TerminalSelectedOperand,
    TerminalSelectedSelectionConstraints, TerminalSelectedSuccessor, TerminalSelectedTerminator,
    TerminalVirtualRegister, TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use omega_terminal_target_operations::TerminalTargetOperationPlan;
use psi_core::{IntegerSign, ScalarType};

mod source;
use source::{SourceFunction, SourceLeaf, SourceLeafValue, derive_source_functions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalSelectedInstructions {
    plan: TerminalSelectedInstructionPlan,
    receipt: TerminalSelectedInstructionValidationReceipt,
}

impl ValidatedTerminalSelectedInstructions {
    pub const fn plan(&self) -> &TerminalSelectedInstructionPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> TerminalSelectedInstructionValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSelectedInstructionValidationReceipt {
    identity: TerminalSelectedInstructionPlanIdentity,
    optimization_unit: omega_optimization_core::OptimizationUnitIdentity,
    fuel_schedule: psi_core::FuelScheduleIdentity,
    function_count: usize,
    block_count: usize,
    virtual_register_count: usize,
    instruction_count: usize,
}

impl TerminalSelectedInstructionValidationReceipt {
    pub const fn identity(self) -> TerminalSelectedInstructionPlanIdentity {
        self.identity
    }

    pub const fn optimization_unit(self) -> omega_optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> psi_core::FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn block_count(self) -> usize {
        self.block_count
    }

    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }

    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstructionError {
    SourceCustodyMismatch,
    TargetRegisterArchitectureMismatch,
    UnsupportedSourceShape {
        function: usize,
    },
    UnsupportedIntegerShape {
        function: usize,
    },
    UnsupportedCondition {
        function: usize,
    },
    MissingConstantDefinition {
        function: usize,
        arm_edge: psi_core::EdgeId,
    },
    MissingFuelProvenance {
        function: usize,
    },
    MissingConstraint(RegisterConstraintKey),
    MissingInputRegisterView {
        function: usize,
    },
    NonCanonicalVirtualRegisters {
        function: usize,
    },
    NonCanonicalBlocks {
        function: usize,
    },
    NonCanonicalInstructions {
        function: usize,
    },
    FunctionProjectionMismatch {
        function: usize,
    },
    VirtualRegisterProjectionMismatch {
        function: usize,
        register: u32,
    },
    BlockProjectionMismatch {
        function: usize,
        block: u32,
    },
    InstructionProjectionMismatch {
        function: usize,
        instruction: u32,
    },
    ConstraintOperandMismatch {
        function: usize,
        instruction: u32,
    },
    ConstraintEffectMismatch {
        function: usize,
        instruction: u32,
    },
    SuccessorProjectionMismatch {
        function: usize,
        block: u32,
    },
    UseBeforeDefinition {
        function: usize,
        instruction: u32,
        register: u32,
    },
    MultipleDefinitions {
        function: usize,
        register: u32,
    },
    ProvenancePartitionMismatch {
        function: usize,
    },
}

impl std::fmt::Display for SelectedInstructionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal instruction selection failed: {self:?}")
    }
}

impl std::error::Error for SelectedInstructionError {}

/// Select and then independently validate the bounded production VReg CFG.
#[allow(clippy::too_many_arguments)]
pub fn select_terminal_instructions(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedTerminalSelectedInstructions, SelectedInstructionError> {
    let source = derive_source_functions(target, abstract_plan, unit)?;
    let plan = build_plan(target, unit, constraints, physical, catalog, &source)?;
    validate_terminal_selected_instructions(
        target,
        abstract_plan,
        unit,
        constraints,
        physical,
        catalog,
        plan,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_selected_instructions(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    plan: TerminalSelectedInstructionPlan,
) -> Result<ValidatedTerminalSelectedInstructions, SelectedInstructionError> {
    if target.terminal_psi != plan.terminal_psi
        || target.target != plan.target
        || target.entry != plan.entry
        || unit.fuel_schedule != plan.fuel_schedule
        || physical.model().architecture != target.target.architecture
        || catalog.architecture() != target.target.architecture
    {
        return Err(SelectedInstructionError::TargetRegisterArchitectureMismatch);
    }
    let source = derive_source_functions(target, abstract_plan, unit)?;
    if source.len() != plan.functions.len() || target.functions.len() != plan.functions.len() {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let expected_fixed_inputs = source
        .iter()
        .map(|source| {
            1 + usize::from(matches!(
                source.when_true.value,
                SourceLeafValue::EntryParameter { .. }
            ))
        })
        .sum::<usize>();
    if constraints.fixed_inputs.len() != expected_fixed_inputs {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    require_key_rows(constraints.keys, catalog)?;
    for (function_index, ((target_function, source), selected)) in target
        .functions
        .iter()
        .zip(&source)
        .zip(&plan.functions)
        .enumerate()
    {
        validate_function(
            function_index,
            target_function,
            source,
            selected,
            constraints,
            physical,
            catalog,
        )?;
    }
    let receipt = receipt(&plan, unit);
    Ok(ValidatedTerminalSelectedInstructions { plan, receipt })
}

fn build_plan(
    target: &TerminalTargetOperationPlan,
    unit: &PsiOptimizationUnit,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    source: &[SourceFunction],
) -> Result<TerminalSelectedInstructionPlan, SelectedInstructionError> {
    require_key_rows(constraints.keys, catalog)?;
    Ok(TerminalSelectedInstructionPlan {
        terminal_psi: target.terminal_psi,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions: target
            .functions
            .iter()
            .zip(source)
            .enumerate()
            .map(|(index, (target_function, source))| {
                build_function(
                    index,
                    target_function,
                    source,
                    constraints,
                    physical,
                    catalog,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn build_function(
    function: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    source: &SourceFunction,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedFunction, SelectedInstructionError> {
    let input = fixed_input_constraint(
        target.machine,
        source.condition_source,
        source.condition_parameter_index,
        source.condition_register,
        &constraints.fixed_inputs,
    )
    .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
    let input_view = input.fixed_view;
    let Some(input_view) = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == input_view)
    else {
        return Err(SelectedInstructionError::MissingInputRegisterView { function });
    };
    let input_class = input_view.class;
    let keys = constraints.keys;
    let result_class = match &source.when_true.value {
        SourceLeafValue::Immediate { .. } => row(catalog, keys.materialize_i64)?.operands[0].class,
        SourceLeafValue::EntryParameter {
            parameter_index,
            register,
            ..
        } => {
            let result_input = fixed_input_constraint(
                target.machine,
                source.when_true.source_value,
                *parameter_index,
                *register,
                &constraints.fixed_inputs,
            )
            .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
            physical
                .model()
                .views
                .iter()
                .find(|view| view.id == result_input.fixed_view)
                .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?
                .class
        }
    };
    let u64_type =
        ScalarType::Integer(psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    Ok(TerminalSelectedFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        entry_block: TerminalSelectedBlockId(0),
        virtual_registers: {
            let mut registers = vec![TerminalVirtualRegister {
                id: TerminalVirtualRegisterId(0),
                scalar_type: ScalarType::Boolean,
                class: input_class,
                origin: TerminalVirtualRegisterOrigin::EntryParameter {
                    source_value: source.condition_source,
                    parameter_index: source.condition_parameter_index,
                },
                definition_site: source.condition_definition_site,
                entry_fixed_view: Some(input_view.id),
            }];
            match (&source.when_true.value, &source.when_false.value) {
                (
                    SourceLeafValue::Immediate {
                        definition_site: true_site,
                        ..
                    },
                    SourceLeafValue::Immediate {
                        definition_site: false_site,
                        ..
                    },
                ) => {
                    registers.push(TerminalVirtualRegister {
                        id: TerminalVirtualRegisterId(1),
                        scalar_type: u64_type,
                        class: result_class,
                        origin: TerminalVirtualRegisterOrigin::InstructionResult {
                            instruction: TerminalSelectedInstructionId(2),
                            source_value: source.when_true.source_value,
                        },
                        definition_site: *true_site,
                        entry_fixed_view: None,
                    });
                    registers.push(TerminalVirtualRegister {
                        id: TerminalVirtualRegisterId(2),
                        scalar_type: u64_type,
                        class: result_class,
                        origin: TerminalVirtualRegisterOrigin::InstructionResult {
                            instruction: TerminalSelectedInstructionId(4),
                            source_value: source.when_false.source_value,
                        },
                        definition_site: *false_site,
                        entry_fixed_view: None,
                    });
                }
                (
                    SourceLeafValue::EntryParameter {
                        parameter_index,
                        register,
                        definition_site,
                    },
                    SourceLeafValue::EntryParameter { .. },
                ) => {
                    let fixed = fixed_input_constraint(
                        target.machine,
                        source.when_true.source_value,
                        *parameter_index,
                        *register,
                        &constraints.fixed_inputs,
                    )
                    .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
                    registers.push(TerminalVirtualRegister {
                        id: TerminalVirtualRegisterId(1),
                        scalar_type: u64_type,
                        class: result_class,
                        origin: TerminalVirtualRegisterOrigin::EntryParameter {
                            source_value: source.when_true.source_value,
                            parameter_index: *parameter_index,
                        },
                        definition_site: *definition_site,
                        entry_fixed_view: Some(fixed.fixed_view),
                    });
                }
                _ => return Err(SelectedInstructionError::UnsupportedSourceShape { function }),
            }
            registers
        },
        blocks: match (&source.when_true.value, &source.when_false.value) {
            (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => vec![
                build_entry_block(source, keys, catalog)?,
                build_constant_return_block(
                    function,
                    TerminalSelectedBlockId(1),
                    source.true_block,
                    2,
                    3,
                    TerminalVirtualRegisterId(1),
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_constant_return_block(
                    function,
                    TerminalSelectedBlockId(2),
                    source.false_block,
                    4,
                    5,
                    TerminalVirtualRegisterId(2),
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
                vec![
                    build_entry_block(source, keys, catalog)?,
                    build_parameter_return_block(
                        function,
                        TerminalSelectedBlockId(1),
                        source.true_block,
                        2,
                        TerminalVirtualRegisterId(1),
                        &source.when_true,
                        keys,
                        catalog,
                    )?,
                    build_parameter_return_block(
                        function,
                        TerminalSelectedBlockId(2),
                        source.false_block,
                        3,
                        TerminalVirtualRegisterId(1),
                        &source.when_false,
                        keys,
                        catalog,
                    )?,
                ]
            }
            _ => return Err(SelectedInstructionError::UnsupportedSourceShape { function }),
        },
    })
}

fn build_entry_block(
    source: &SourceFunction,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedBlock, SelectedInstructionError> {
    Ok(TerminalSelectedBlock {
        id: TerminalSelectedBlockId(0),
        source_block: source.entry_block,
        instructions: vec![instruction(
            TerminalSelectedInstructionId(0),
            TerminalSelectedInstructionKind::CompareI64Zero,
            keys.compare_i64_zero,
            &[TerminalVirtualRegisterId(0)],
            TerminalSelectedInstructionProvenance {
                values: vec![source.condition_source],
                ..Default::default()
            },
            catalog,
        )?],
        terminator: TerminalSelectedTerminator::ConditionalBranch {
            instruction: instruction(
                TerminalSelectedInstructionId(1),
                TerminalSelectedInstructionKind::ConditionalBranchNonZero,
                keys.conditional_branch,
                &[],
                TerminalSelectedInstructionProvenance {
                    values: vec![source.condition_source],
                    ..Default::default()
                },
                catalog,
            )?,
            when_nonzero: TerminalSelectedSuccessor {
                psi_edge: source.branch_true_edge,
                block: TerminalSelectedBlockId(1),
                source_target: source.true_block,
                bindings: source.branch_true_bindings.clone(),
                fuel: source.branch_true_fuel.clone(),
            },
            when_zero: TerminalSelectedSuccessor {
                psi_edge: source.branch_false_edge,
                block: TerminalSelectedBlockId(2),
                source_target: source.false_block,
                bindings: source.branch_false_bindings.clone(),
                fuel: source.branch_false_fuel.clone(),
            },
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn build_constant_return_block(
    function: usize,
    id: TerminalSelectedBlockId,
    source_block: psi_core::BlockId,
    materialize_id: u32,
    return_id: u32,
    register: TerminalVirtualRegisterId,
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedBlock, SelectedInstructionError> {
    let SourceLeafValue::Immediate {
        value,
        constant_operation,
        constant_fuel,
        ..
    } = &source.value
    else {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    };
    Ok(TerminalSelectedBlock {
        id,
        source_block,
        instructions: vec![instruction(
            TerminalSelectedInstructionId(materialize_id),
            TerminalSelectedInstructionKind::MaterializeI64 { value: *value },
            keys.materialize_i64,
            &[register],
            TerminalSelectedInstructionProvenance {
                operations: vec![*constant_operation],
                values: vec![source.source_value],
                fuel: constant_fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?],
        terminator: TerminalSelectedTerminator::Return {
            instruction: instruction(
                TerminalSelectedInstructionId(return_id),
                TerminalSelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[register],
                TerminalSelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn build_parameter_return_block(
    function: usize,
    id: TerminalSelectedBlockId,
    source_block: psi_core::BlockId,
    return_id: u32,
    register: TerminalVirtualRegisterId,
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedBlock, SelectedInstructionError> {
    if !matches!(source.value, SourceLeafValue::EntryParameter { .. }) {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    Ok(TerminalSelectedBlock {
        id,
        source_block,
        instructions: Vec::new(),
        terminator: TerminalSelectedTerminator::Return {
            instruction: instruction(
                TerminalSelectedInstructionId(return_id),
                TerminalSelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[register],
                TerminalSelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}

fn instruction(
    id: TerminalSelectedInstructionId,
    kind: TerminalSelectedInstructionKind,
    key: RegisterConstraintKey,
    registers: &[TerminalVirtualRegisterId],
    provenance: TerminalSelectedInstructionProvenance,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedInstruction, SelectedInstructionError> {
    let row = row(catalog, key)?;
    if row.operands.len() != registers.len() {
        return Err(SelectedInstructionError::MissingConstraint(key));
    }
    Ok(TerminalSelectedInstruction {
        id,
        kind,
        constraint: key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(constraint, register)| TerminalSelectedOperand {
                operand: constraint.operand,
                virtual_register: *register,
                access: constraint.access,
                class: constraint.class,
                fixed_view: constraint.fixed_view,
                tied_to: constraint.tied_to,
                early_clobber: constraint.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance,
    })
}

fn require_key_rows(
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for key in [
        keys.materialize_i64,
        keys.compare_i64_zero,
        keys.conditional_branch,
        keys.return_i64,
    ] {
        row(catalog, key)?;
    }
    Ok(())
}

fn row(
    catalog: &ValidatedRegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> Result<&RegisterInstructionConstraint, SelectedInstructionError> {
    catalog
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == key)
        .ok_or(SelectedInstructionError::MissingConstraint(key))
}

fn fixed_input_constraint(
    machine: psi_core::MachineId,
    source_value: psi_core::ValueId,
    parameter_index: usize,
    register: omega_terminal_target_operations::MachineRegister,
    inputs: &[TerminalSelectedFixedInputConstraint],
) -> Option<&TerminalSelectedFixedInputConstraint> {
    let mut matches = inputs.iter().filter(|input| {
        input.machine == machine
            && input.source_value == source_value
            && input.parameter_index == parameter_index
            && input.register == register
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn validate_function(
    function_index: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    source: &SourceFunction,
    function: &TerminalSelectedFunction,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if function.machine != target.machine
        || function.attachment != target.attachment
        || function.provenance != target.provenance
        || function.entry_block != TerminalSelectedBlockId(0)
    {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }
    validate_dense(function_index, source, function)?;
    validate_virtual_registers(
        function_index,
        target,
        source,
        function,
        constraints,
        physical,
        catalog,
    )?;
    validate_selected_blocks(function_index, source, function, constraints.keys, catalog)?;
    for block in &function.blocks {
        validate_block_constraints(function_index, block, function, catalog)?;
    }
    validate_def_use(function_index, function, catalog)?;
    validate_provenance_partition(function_index, source, function)?;
    Ok(())
}

fn validate_virtual_registers(
    function_index: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    source: &SourceFunction,
    function: &TerminalSelectedFunction,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let input = fixed_input_constraint(
        target.machine,
        source.condition_source,
        source.condition_parameter_index,
        source.condition_register,
        &constraints.fixed_inputs,
    )
    .ok_or(SelectedInstructionError::MissingInputRegisterView {
        function: function_index,
    })?;
    let Some(input_view) = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == input.fixed_view)
    else {
        return Err(SelectedInstructionError::MissingInputRegisterView {
            function: function_index,
        });
    };
    let u64_type =
        ScalarType::Integer(psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    let mut expected = vec![(
        ScalarType::Boolean,
        input_view.class,
        TerminalVirtualRegisterOrigin::EntryParameter {
            source_value: source.condition_source,
            parameter_index: source.condition_parameter_index,
        },
        source.condition_definition_site,
        Some(input.fixed_view),
    )];
    match (&source.when_true.value, &source.when_false.value) {
        (
            SourceLeafValue::Immediate {
                definition_site: true_site,
                ..
            },
            SourceLeafValue::Immediate {
                definition_site: false_site,
                ..
            },
        ) => {
            let result_class = row(catalog, constraints.keys.materialize_i64)?.operands[0].class;
            expected.push((
                u64_type,
                result_class,
                TerminalVirtualRegisterOrigin::InstructionResult {
                    instruction: TerminalSelectedInstructionId(2),
                    source_value: source.when_true.source_value,
                },
                *true_site,
                None,
            ));
            expected.push((
                u64_type,
                result_class,
                TerminalVirtualRegisterOrigin::InstructionResult {
                    instruction: TerminalSelectedInstructionId(4),
                    source_value: source.when_false.source_value,
                },
                *false_site,
                None,
            ));
        }
        (
            SourceLeafValue::EntryParameter {
                parameter_index,
                register,
                definition_site,
            },
            SourceLeafValue::EntryParameter { .. },
        ) => {
            let result_input = fixed_input_constraint(
                target.machine,
                source.when_true.source_value,
                *parameter_index,
                *register,
                &constraints.fixed_inputs,
            )
            .ok_or(SelectedInstructionError::MissingInputRegisterView {
                function: function_index,
            })?;
            let Some(result_view) = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == result_input.fixed_view)
            else {
                return Err(SelectedInstructionError::MissingInputRegisterView {
                    function: function_index,
                });
            };
            expected.push((
                u64_type,
                result_view.class,
                TerminalVirtualRegisterOrigin::EntryParameter {
                    source_value: source.when_true.source_value,
                    parameter_index: *parameter_index,
                },
                *definition_site,
                Some(result_input.fixed_view),
            ));
        }
        _ => {
            return Err(SelectedInstructionError::UnsupportedSourceShape {
                function: function_index,
            });
        }
    }
    for (index, (register, expected)) in function.virtual_registers.iter().zip(expected).enumerate()
    {
        if register.scalar_type != expected.0
            || register.class != expected.1
            || register.origin != expected.2
            || register.definition_site != expected.3
            || register.entry_fixed_view != expected.4
        {
            return Err(
                SelectedInstructionError::VirtualRegisterProjectionMismatch {
                    function: function_index,
                    register: index as u32,
                },
            );
        }
    }
    Ok(())
}

fn validate_selected_blocks(
    function_index: usize,
    source: &SourceFunction,
    function: &TerminalSelectedFunction,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if function.blocks[0].source_block != source.entry_block
        || function.blocks[1].source_block != source.true_block
        || function.blocks[2].source_block != source.false_block
    {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: function
                .blocks
                .iter()
                .enumerate()
                .find(|(index, block)| {
                    block.source_block
                        != [source.entry_block, source.true_block, source.false_block][*index]
                })
                .map_or(0, |(index, _)| index as u32),
        });
    }
    let entry = &function.blocks[0];
    if entry.instructions.len() != 1 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    validate_instruction_projection(
        function_index,
        &entry.instructions[0],
        TerminalSelectedInstructionId(0),
        TerminalSelectedInstructionKind::CompareI64Zero,
        keys.compare_i64_zero,
        &[TerminalVirtualRegisterId(0)],
        &TerminalSelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    let TerminalSelectedTerminator::ConditionalBranch {
        instruction,
        when_nonzero,
        when_zero,
    } = &entry.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    };
    validate_instruction_projection(
        function_index,
        instruction,
        TerminalSelectedInstructionId(1),
        TerminalSelectedInstructionKind::ConditionalBranchNonZero,
        keys.conditional_branch,
        &[],
        &TerminalSelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    let expected_true = TerminalSelectedSuccessor {
        psi_edge: source.branch_true_edge,
        block: TerminalSelectedBlockId(1),
        source_target: source.true_block,
        bindings: source.branch_true_bindings.clone(),
        fuel: source.branch_true_fuel.clone(),
    };
    let expected_false = TerminalSelectedSuccessor {
        psi_edge: source.branch_false_edge,
        block: TerminalSelectedBlockId(2),
        source_target: source.false_block,
        bindings: source.branch_false_bindings.clone(),
        fuel: source.branch_false_fuel.clone(),
    };
    if when_nonzero != &expected_true || when_zero != &expected_false {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    match (&source.when_true.value, &source.when_false.value) {
        (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => {
            validate_constant_return_block_projection(
                function_index,
                &function.blocks[1],
                2,
                3,
                TerminalVirtualRegisterId(1),
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_constant_return_block_projection(
                function_index,
                &function.blocks[2],
                4,
                5,
                TerminalVirtualRegisterId(2),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
            validate_parameter_return_block_projection(
                function_index,
                &function.blocks[1],
                2,
                TerminalVirtualRegisterId(1),
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_parameter_return_block_projection(
                function_index,
                &function.blocks[2],
                3,
                TerminalVirtualRegisterId(1),
                &source.when_false,
                keys,
                catalog,
            )
        }
        _ => Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_constant_return_block_projection(
    function_index: usize,
    block: &TerminalSelectedBlock,
    materialize_id: u32,
    return_id: u32,
    register: TerminalVirtualRegisterId,
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let SourceLeafValue::Immediate {
        value,
        constant_operation,
        constant_fuel,
        ..
    } = &source.value
    else {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        });
    };
    if block.instructions.len() != 1 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function_index,
        &block.instructions[0],
        TerminalSelectedInstructionId(materialize_id),
        TerminalSelectedInstructionKind::MaterializeI64 { value: *value },
        keys.materialize_i64,
        &[register],
        &TerminalSelectedInstructionProvenance {
            operations: vec![*constant_operation],
            values: vec![source.source_value],
            fuel: constant_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    let TerminalSelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function_index,
        instruction,
        TerminalSelectedInstructionId(return_id),
        TerminalSelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[register],
        &TerminalSelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_parameter_return_block_projection(
    function_index: usize,
    block: &TerminalSelectedBlock,
    return_id: u32,
    register: TerminalVirtualRegisterId,
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if !matches!(source.value, SourceLeafValue::EntryParameter { .. })
        || !block.instructions.is_empty()
    {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    let TerminalSelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function_index,
        instruction,
        TerminalSelectedInstructionId(return_id),
        TerminalSelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[register],
        &TerminalSelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_instruction_projection(
    function: usize,
    instruction: &TerminalSelectedInstruction,
    id: TerminalSelectedInstructionId,
    kind: TerminalSelectedInstructionKind,
    key: RegisterConstraintKey,
    registers: &[TerminalVirtualRegisterId],
    provenance: &TerminalSelectedInstructionProvenance,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    if instruction.id != id
        || instruction.kind != kind
        || instruction.constraint != key
        || instruction.provenance != *provenance
        || instruction.operands.len() != registers.len()
        || instruction
            .operands
            .iter()
            .zip(registers)
            .zip(&constraint.operands)
            .any(|((operand, register), expected)| {
                operand.virtual_register != *register
                    || operand.operand != expected.operand
                    || operand.access != expected.access
                    || operand.class != expected.class
                    || operand.fixed_view != expected.fixed_view
                    || operand.tied_to != expected.tied_to
                    || operand.early_clobber != expected.early_clobber
            })
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function,
            instruction: id.0,
        });
    }
    if instruction.implicit_uses != constraint.implicit_uses
        || instruction.implicit_defs != constraint.implicit_defs
        || instruction.clobbers != constraint.clobbers
    {
        return Err(SelectedInstructionError::ConstraintEffectMismatch {
            function,
            instruction: id.0,
        });
    }
    Ok(())
}

fn validate_dense(
    function_index: usize,
    source: &SourceFunction,
    function: &TerminalSelectedFunction,
) -> Result<(), SelectedInstructionError> {
    let (expected_register_count, expected_instruction_count) =
        match (&source.when_true.value, &source.when_false.value) {
            (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => (3, 6),
            (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
                (2, 4)
            }
            _ => {
                return Err(SelectedInstructionError::UnsupportedSourceShape {
                    function: function_index,
                });
            }
        };
    if function.virtual_registers.len() != expected_register_count
        || function
            .virtual_registers
            .iter()
            .enumerate()
            .any(|(index, register)| register.id.0 as usize != index)
    {
        return Err(SelectedInstructionError::NonCanonicalVirtualRegisters {
            function: function_index,
        });
    }
    if function.blocks.len() != 3
        || function
            .blocks
            .iter()
            .enumerate()
            .any(|(index, block)| block.id.0 as usize != index)
    {
        return Err(SelectedInstructionError::NonCanonicalBlocks {
            function: function_index,
        });
    }
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .chain(std::iter::once(
                    terminator_instruction(&block.terminator).id,
                ))
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    if ids
        != (0..expected_instruction_count)
            .map(TerminalSelectedInstructionId)
            .collect::<Vec<_>>()
    {
        return Err(SelectedInstructionError::NonCanonicalInstructions {
            function: function_index,
        });
    }
    Ok(())
}

fn validate_block_constraints(
    function_index: usize,
    block: &TerminalSelectedBlock,
    function: &TerminalSelectedFunction,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for instruction in block
        .instructions
        .iter()
        .chain(std::iter::once(terminator_instruction(&block.terminator)))
    {
        let row = row(catalog, instruction.constraint)?;
        if instruction.operands.len() != row.operands.len() {
            return Err(SelectedInstructionError::ConstraintOperandMismatch {
                function: function_index,
                instruction: instruction.id.0,
            });
        }
        for (operand, constraint) in instruction.operands.iter().zip(&row.operands) {
            let Some(register) = function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)
            else {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: instruction.id.0,
                });
            };
            if operand.operand != constraint.operand
                || operand.access != constraint.access
                || operand.class != constraint.class
                || operand.fixed_view != constraint.fixed_view
                || operand.tied_to != constraint.tied_to
                || operand.early_clobber != constraint.early_clobber
                || register.class != constraint.class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: instruction.id.0,
                });
            }
        }
        if instruction.implicit_uses != row.implicit_uses
            || instruction.implicit_defs != row.implicit_defs
            || instruction.clobbers != row.clobbers
        {
            return Err(SelectedInstructionError::ConstraintEffectMismatch {
                function: function_index,
                instruction: instruction.id.0,
            });
        }
    }
    Ok(())
}

fn validate_def_use(
    function_index: usize,
    function: &TerminalSelectedFunction,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let mut definitions = vec![0_u8; function.virtual_registers.len()];
    let entry_registers = function
        .virtual_registers
        .iter()
        .filter_map(|register| {
            matches!(
                register.origin,
                TerminalVirtualRegisterOrigin::EntryParameter { .. }
            )
            .then_some(register.id)
        })
        .collect::<BTreeSet<_>>();
    for register in &entry_registers {
        definitions[register.0 as usize] = 1;
    }
    for block in &function.blocks {
        let mut available = entry_registers.clone();
        for instruction in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
        {
            let row = row(catalog, instruction.constraint)?;
            for (operand, constraint) in instruction.operands.iter().zip(&row.operands) {
                let index = operand.virtual_register.0 as usize;
                if matches!(
                    constraint.access,
                    RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                ) && !available.contains(&operand.virtual_register)
                {
                    return Err(SelectedInstructionError::UseBeforeDefinition {
                        function: function_index,
                        instruction: instruction.id.0,
                        register: operand.virtual_register.0,
                    });
                }
                if matches!(
                    constraint.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                ) {
                    definitions[index] += 1;
                    if definitions[index] != 1 {
                        return Err(SelectedInstructionError::MultipleDefinitions {
                            function: function_index,
                            register: operand.virtual_register.0,
                        });
                    }
                    available.insert(operand.virtual_register);
                }
            }
        }
    }
    if definitions.iter().any(|count| *count != 1) {
        return Err(SelectedInstructionError::MultipleDefinitions {
            function: function_index,
            register: definitions.iter().position(|count| *count != 1).unwrap() as u32,
        });
    }
    Ok(())
}

fn validate_provenance_partition(
    function_index: usize,
    source: &SourceFunction,
    function: &TerminalSelectedFunction,
) -> Result<(), SelectedInstructionError> {
    let entry = &function.blocks[0];
    let TerminalSelectedTerminator::ConditionalBranch {
        instruction: branch,
        when_nonzero,
        when_zero,
    } = &entry.terminator
    else {
        return Err(SelectedInstructionError::ProvenancePartitionMismatch {
            function: function_index,
        });
    };
    if !entry.instructions[0].provenance.fuel.is_empty()
        || !branch.provenance.fuel.is_empty()
        || when_nonzero.fuel != source.branch_true_fuel
        || when_zero.fuel != source.branch_false_fuel
    {
        return Err(SelectedInstructionError::ProvenancePartitionMismatch {
            function: function_index,
        });
    }
    for (block, leaf) in function.blocks[1..]
        .iter()
        .zip([&source.when_true, &source.when_false])
    {
        let TerminalSelectedTerminator::Return { instruction, .. } = &block.terminator else {
            return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                function: function_index,
            });
        };
        match &leaf.value {
            SourceLeafValue::Immediate { constant_fuel, .. } => {
                if block.instructions.len() != 1
                    || block.instructions[0].provenance.fuel != *constant_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::EntryParameter { .. } => {
                if !block.instructions.is_empty() || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
        }
    }
    Ok(())
}

fn terminator_instruction(terminator: &TerminalSelectedTerminator) -> &TerminalSelectedInstruction {
    match terminator {
        TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
        | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
    }
}

fn receipt(
    plan: &TerminalSelectedInstructionPlan,
    unit: &PsiOptimizationUnit,
) -> TerminalSelectedInstructionValidationReceipt {
    let function_count = plan.functions.len();
    let block_count = plan
        .functions
        .iter()
        .map(|function| function.blocks.len())
        .sum();
    let virtual_register_count = plan
        .functions
        .iter()
        .map(|function| function.virtual_registers.len())
        .sum();
    let instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len() + 1)
        .sum();
    TerminalSelectedInstructionValidationReceipt {
        identity: terminal_selected_instruction_plan_identity(plan),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        function_count,
        block_count,
        virtual_register_count,
        instruction_count,
    }
}

pub fn terminal_selected_instruction_plan_identity(
    plan: &TerminalSelectedInstructionPlan,
) -> TerminalSelectedInstructionPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-selected-instructions.v1\0");
    bytes.extend_from_slice(plan.terminal_psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.terminal_psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
        encode_len(&mut bytes, function.virtual_registers.len());
        for register in &function.virtual_registers {
            bytes.extend_from_slice(&register.id.0.to_le_bytes());
            encode_scalar_type(&mut bytes, register.scalar_type);
            bytes.extend_from_slice(&register.class.0.to_le_bytes());
            match register.origin {
                TerminalVirtualRegisterOrigin::EntryParameter {
                    source_value,
                    parameter_index,
                } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&source_value.get().to_le_bytes());
                    bytes.extend_from_slice(&(parameter_index as u64).to_le_bytes());
                }
                TerminalVirtualRegisterOrigin::InstructionResult {
                    instruction,
                    source_value,
                } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&source_value.get().to_le_bytes());
                }
            }
            encode_definition_site(&mut bytes, register.definition_site);
            encode_option_u16(&mut bytes, register.entry_fixed_view.map(|view| view.0));
        }
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.id.0.to_le_bytes());
            bytes.extend_from_slice(&block.source_block.get().to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                encode_instruction(&mut bytes, instruction);
            }
            match &block.terminator {
                TerminalSelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero,
                    when_zero,
                } => {
                    bytes.push(0);
                    encode_instruction(&mut bytes, instruction);
                    encode_successor(&mut bytes, when_nonzero);
                    encode_successor(&mut bytes, when_zero);
                }
                TerminalSelectedTerminator::Return {
                    instruction,
                    psi_return_edge,
                } => {
                    bytes.push(1);
                    encode_instruction(&mut bytes, instruction);
                    bytes.extend_from_slice(&psi_return_edge.get().to_le_bytes());
                }
            }
        }
    }
    TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(&bytes)
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &TerminalSelectedInstruction) {
    bytes.extend_from_slice(&instruction.id.0.to_le_bytes());
    bytes.push(match instruction.kind {
        TerminalSelectedInstructionKind::CompareI64Zero => 0,
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => 1,
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => 2,
        TerminalSelectedInstructionKind::ReturnI64 => 3,
    });
    if let TerminalSelectedInstructionKind::MaterializeI64 { value } = instruction.kind {
        match value {
            psi_core::IntegerValue::Signed(value) => {
                bytes.push(0);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            psi_core::IntegerValue::Unsigned(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    encode_constraint_key(bytes, instruction.constraint);
    encode_len(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        bytes.extend_from_slice(&operand.operand.to_le_bytes());
        bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
        bytes.push(match operand.access {
            RegisterOperandAccess::Use => 0,
            RegisterOperandAccess::Def => 1,
            RegisterOperandAccess::UseDef => 2,
        });
        bytes.extend_from_slice(&operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_ids(
        bytes,
        instruction
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .values
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        instruction.provenance.edges.iter().map(|edge| edge.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    encode_fuel(bytes, &instruction.provenance.fuel);
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &TerminalSelectedSuccessor) {
    bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.block.0.to_le_bytes());
    bytes.extend_from_slice(&successor.source_target.get().to_le_bytes());
    encode_len(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.scalar_type);
    }
    encode_fuel(bytes, &successor.fuel);
}

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    bytes.extend_from_slice(&(fuel.len() as u64).to_le_bytes());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                bytes.extend_from_slice(&edge.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: omega_target::NativeTarget) {
    bytes.push(match target.architecture {
        omega_target::Architecture::X86_64 => 0,
        omega_target::Architecture::Aarch64 => 1,
    });
    bytes.push(match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        omega_register_model::RegisterConstraintFamily::Call => 0,
        omega_register_model::RegisterConstraintFamily::Return => 1,
        omega_register_model::RegisterConstraintFamily::SystemCall => 2,
        omega_register_model::RegisterConstraintFamily::InlineAssembly => 3,
        omega_register_model::RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}

fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                psi_core::IntegerCarrier::Fixed => 0,
                psi_core::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
    }
}

fn encode_option_id(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}

fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
