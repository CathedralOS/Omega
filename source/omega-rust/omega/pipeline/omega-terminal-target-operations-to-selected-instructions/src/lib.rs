#![forbid(unsafe_code)]

//! Target legalization and instruction selection for the first production
//! clean-Terminal virtual-register CFG slice.
//!
//! The mandatory checked legalizer produces an opaque target-legal carrier;
//! selection accepts only that carrier rather than freely recombined raw
//! target, abstract, and optimization-unit inputs. Both public producers return
//! opaque validated carriers and make no liveness or allocation claim.

use std::collections::BTreeSet;

use omega_calling_conventions::{
    CallingPolicy, EntryControl, IndirectPointerLocation, MachineRegister, ValueClass,
    ValueLocation,
};
use omega_optimization_core::OptimizationValidatorIdentity;
use omega_optimization_unit::{
    FuelSettlement, PsiOptimizationUnit, PsiProvenance, ValueDefinitionSite,
};
use omega_register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
};
use omega_terminal_abstract_operations::TerminalAbstractOperationPlan;
use omega_terminal_legalized_operations::{
    TerminalLegalizedCallUnit, TerminalLegalizedCallUnitArgument,
    TerminalLegalizedCallUnitParameter, TerminalLegalizedFunction as SourceFunction,
    TerminalLegalizedImmediate as SourceImmediate, TerminalLegalizedLeaf as SourceLeaf,
    TerminalLegalizedLeafValue as SourceLeafValue, TerminalLegalizedOperationPlan,
    TerminalLegalizedOperationPlanIdentity,
    TerminalLegalizedStructuralUnitFunction as SourceStructuralUnitFunction,
    TerminalLegalizedUnitFunction as SourceUnitFunction,
    terminal_legalized_operation_plan_identity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlock, TerminalSelectedBlockId, TerminalSelectedConstraintKeys,
    TerminalSelectedFixedInputConstraint, TerminalSelectedFunction, TerminalSelectedInstruction,
    TerminalSelectedInstructionId, TerminalSelectedInstructionKind,
    TerminalSelectedInstructionPlan, TerminalSelectedInstructionPlanIdentity,
    TerminalSelectedInstructionProvenance, TerminalSelectedMicrosoftX64OwnedIndirectPairLayout,
    TerminalSelectedOperand, TerminalSelectedSelectionConstraints,
    TerminalSelectedStructuralUnitAbi, TerminalSelectedStructuralUnitAbiRecipe,
    TerminalSelectedStructuralUnitCallArgument, TerminalSelectedStructuralUnitCallInstruction,
    TerminalSelectedStructuralUnitFunction, TerminalSelectedStructuralUnitIndirectBinding,
    TerminalSelectedStructuralUnitParameter, TerminalSelectedStructuralUnitReturn,
    TerminalSelectedSuccessor, TerminalSelectedTerminator, TerminalVirtualRegister,
    TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use omega_terminal_target_operations::TerminalTargetOperationPlan;
use psi_core::{IntegerCarrier, IntegerSign, ScalarType};
use psi_terminal::{BindingRelevance, StructuralAccess, StructuralFieldType, StructuralTypeShape};

mod legalization_replay;
mod source;
use legalization_replay::replay_terminal_legalized_plan;
use source::derive_source_structural_unit_functions;
use source::{derive_source_functions, derive_source_unit_functions};

pub fn terminal_legalization_validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.terminal-target-legalization-independent-replay.v6",
    )
}

/// Opaque custody of the canonical V6 target-legal projection.
///
/// This carrier grants no instruction-selection, liveness, allocation,
/// emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalLegalizedOperations {
    plan: TerminalLegalizedOperationPlan,
    receipt: TerminalLegalizationValidationReceipt,
}

impl ValidatedTerminalLegalizedOperations {
    pub const fn plan(&self) -> &TerminalLegalizedOperationPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> TerminalLegalizationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLegalizationValidationReceipt {
    identity: TerminalLegalizedOperationPlanIdentity,
    validator: OptimizationValidatorIdentity,
    optimization_unit: omega_optimization_core::OptimizationUnitIdentity,
    fuel_schedule: psi_core::FuelScheduleIdentity,
    target: omega_target::NativeTarget,
    function_count: usize,
    decomposition_count: usize,
}

impl TerminalLegalizationValidationReceipt {
    pub const fn identity(self) -> TerminalLegalizedOperationPlanIdentity {
        self.identity
    }

    pub const fn validator(self) -> OptimizationValidatorIdentity {
        self.validator
    }

    pub const fn optimization_unit(self) -> omega_optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn fuel_schedule(self) -> psi_core::FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn target(self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    /// Independently replayed non-identity legalization occurrence groups.
    pub const fn decomposition_count(self) -> usize {
        self.decomposition_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLegalizationError {
    SourceCustodyMismatch,
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
    NonCanonicalLegalizedPlan,
}

impl std::fmt::Display for TerminalLegalizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Terminal target legalization failed: {self:?}")
    }
}

impl std::error::Error for TerminalLegalizationError {}

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
    legalized: TerminalLegalizedOperationPlanIdentity,
    legalization_validator: OptimizationValidatorIdentity,
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

    pub const fn legalized(self) -> TerminalLegalizedOperationPlanIdentity {
        self.legalized
    }

    pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
        self.legalization_validator
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

/// Canonicalize the bounded target-operation input into the mandatory V6
/// legal-operation carrier, then replay its complete source projection.
pub fn legalize_terminal_target_operations(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<ValidatedTerminalLegalizedOperations, TerminalLegalizationError> {
    let plan = TerminalLegalizedOperationPlan {
        terminal_psi: target.terminal_psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions: derive_source_functions(target, abstract_plan, unit)?,
        unit_functions: derive_source_unit_functions(target, abstract_plan, unit)?,
        structural_unit_functions: derive_source_structural_unit_functions(
            target,
            abstract_plan,
            unit,
        )?,
    };
    validate_terminal_legalized_operations(target, abstract_plan, unit, plan)
}

/// Independently replay the exact admitted V6 projection from the raw target,
/// abstract, and verified optimization-unit custody against every proposed
/// field.
pub fn validate_terminal_legalized_operations(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    plan: TerminalLegalizedOperationPlan,
) -> Result<ValidatedTerminalLegalizedOperations, TerminalLegalizationError> {
    let decomposition_count = replay_terminal_legalized_plan(target, abstract_plan, unit, &plan)?;
    let receipt = TerminalLegalizationValidationReceipt {
        identity: terminal_legalized_operation_plan_identity(&plan),
        validator: terminal_legalization_validator_identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        function_count: plan.functions.len()
            + plan.unit_functions.len()
            + plan.structural_unit_functions.len(),
        decomposition_count,
    };
    Ok(ValidatedTerminalLegalizedOperations { plan, receipt })
}

/// Select and then independently validate the bounded production VReg CFG.
pub fn select_terminal_instructions(
    legalized: &ValidatedTerminalLegalizedOperations,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedTerminalSelectedInstructions, SelectedInstructionError> {
    let plan = build_plan(legalized, constraints, physical, catalog)?;
    validate_terminal_selected_instructions(legalized, constraints, physical, catalog, plan)
}

pub fn validate_terminal_selected_instructions(
    legalized: &ValidatedTerminalLegalizedOperations,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    plan: TerminalSelectedInstructionPlan,
) -> Result<ValidatedTerminalSelectedInstructions, SelectedInstructionError> {
    let target = legalized.plan();
    if target.terminal_psi != plan.terminal_psi
        || target.target != plan.target
        || target.entry != plan.entry
        || target.fuel_schedule != plan.fuel_schedule
        || physical.model().architecture != target.target.architecture
        || catalog.architecture() != target.target.architecture
    {
        return Err(SelectedInstructionError::TargetRegisterArchitectureMismatch);
    }
    if target.functions.len() + target.unit_functions.len() != plan.functions.len()
        || target.structural_unit_functions.len() != plan.structural_unit_functions.len()
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let mut expected_machines = target
        .functions
        .iter()
        .map(|function| function.machine)
        .chain(
            target
                .unit_functions
                .iter()
                .map(|function| function.machine),
        )
        .collect::<Vec<_>>();
    expected_machines.sort_unstable();
    if plan
        .functions
        .iter()
        .map(|function| function.machine)
        .ne(expected_machines)
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let expected_fixed_inputs = target
        .functions
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
    for (function_index, selected) in plan.functions.iter().enumerate() {
        let scalar = target
            .functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        let unit = target
            .unit_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        match (scalar.as_slice(), unit.as_slice()) {
            ([source], []) => validate_function(
                function_index,
                source,
                selected,
                constraints,
                physical,
                catalog,
            )?,
            ([], [source]) => {
                validate_unit_function(function_index, source, selected, constraints.keys, catalog)?
            }
            _ => return Err(SelectedInstructionError::SourceCustodyMismatch),
        }
    }
    let mut expected_structural_machines = target
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .collect::<Vec<_>>();
    expected_structural_machines.sort_unstable();
    if plan
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .ne(expected_structural_machines)
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    for (function_index, selected) in plan.structural_unit_functions.iter().enumerate() {
        let Some(source) = target
            .structural_unit_functions
            .iter()
            .find(|source| source.machine == selected.machine)
        else {
            return Err(SelectedInstructionError::SourceCustodyMismatch);
        };
        validate_structural_unit_function(
            function_index + plan.functions.len(),
            source,
            selected,
            target,
            constraints.keys,
            catalog,
        )?;
    }
    let receipt = receipt(&plan, legalized);
    Ok(ValidatedTerminalSelectedInstructions { plan, receipt })
}

fn build_plan(
    legalized: &ValidatedTerminalLegalizedOperations,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedInstructionPlan, SelectedInstructionError> {
    let target = legalized.plan();
    require_key_rows(constraints.keys, catalog)?;
    let mut functions = target
        .functions
        .iter()
        .enumerate()
        .map(|(index, source)| build_function(index, source, constraints, physical, catalog))
        .collect::<Result<Vec<_>, _>>()?;
    functions.extend(
        target
            .unit_functions
            .iter()
            .map(|source| build_unit_function(source, constraints.keys, catalog))
            .collect::<Result<Vec<_>, _>>()?,
    );
    functions.sort_by_key(|function| function.machine);
    let mut structural_unit_functions = target
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(index, source)| {
            build_structural_unit_function(index, source, target, constraints.keys, catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;
    structural_unit_functions.sort_by_key(|function| function.machine);
    Ok(TerminalSelectedInstructionPlan {
        terminal_psi: target.terminal_psi,
        fuel_schedule: target.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions,
        structural_unit_functions,
    })
}

fn build_unit_function(
    source: &SourceUnitFunction,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedFunction, SelectedInstructionError> {
    Ok(TerminalSelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: TerminalSelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: vec![TerminalSelectedBlock {
            id: TerminalSelectedBlockId(0),
            source_block: source.entry_block,
            instructions: Vec::new(),
            terminator: TerminalSelectedTerminator::Return {
                instruction: instruction(
                    TerminalSelectedInstructionId(0),
                    TerminalSelectedInstructionKind::ReturnUnit,
                    keys.return_unit,
                    &[],
                    TerminalSelectedInstructionProvenance {
                        edges: vec![source.return_edge],
                        fuel: source.return_fuel.clone(),
                        ..Default::default()
                    },
                    catalog,
                )?,
                psi_return_edge: source.return_edge,
            },
        }],
    })
}

fn structural_unit_layout(
    function: usize,
    source: &SourceStructuralUnitFunction,
) -> Result<TerminalSelectedMicrosoftX64OwnedIndirectPairLayout, SelectedInstructionError> {
    if source.call_plan.policy != CallingPolicy::MicrosoftX64
        || source.call_plan.result.is_some()
        || !source.call_plan.callback_materializations.is_empty()
        || source.call_plan.stack_alignment != 16
        || source.call_plan.shadow_bytes != 32
        || source.call_plan.entry_control != EntryControl::CallReturn
        || source.parameters.len() != 2
        || source.call_plan.parameters.len() != 2
    {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    let pointers = [MachineRegister::X86Rcx, MachineRegister::X86Rdx];
    let offsets = [32, 48];
    let mut bindings = [TerminalSelectedStructuralUnitIndirectBinding {
        parameter_index: 0,
        pointer: pointers[0],
        copy_stack_byte_offset: offsets[0],
        byte_count: 16,
        alignment: 8,
    }; 2];
    for (index, parameter) in source.parameters.iter().enumerate() {
        if parameter.semantic.position != index as u32
            || parameter.semantic.is_self
            || parameter.semantic.access != StructuralAccess::Owned
            || parameter.target.place != parameter.semantic.place
            || parameter.target.structural_type != parameter.semantic.structural_type
            || parameter.target.multiplicity != parameter.semantic.multiplicity
            || parameter.target.access != StructuralAccess::Owned
            || parameter.target.shape.class != ValueClass::Integer
            || parameter.target.shape.byte_size != 16
            || parameter.target.shape.alignment != 8
            || parameter.target.placement != source.call_plan.parameters[index]
            || parameter.target.placement.locations.len() != 1
        {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        }
        let ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(pointer),
            copy_stack_byte_offset: Some(copy_stack_byte_offset),
            byte_size,
            alignment,
        } = parameter.target.placement.locations[0]
        else {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        };
        if pointer != pointers[index]
            || copy_stack_byte_offset != offsets[index]
            || byte_size != 16
            || alignment != 8
        {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        }
        bindings[index] = TerminalSelectedStructuralUnitIndirectBinding {
            parameter_index: index,
            pointer,
            copy_stack_byte_offset,
            byte_count: byte_size,
            alignment,
        };
    }
    if source.parameters[0].semantic.structural_type
        != source.parameters[1].semantic.structural_type
        || source.parameters[0].semantic.multiplicity != source.parameters[1].semantic.multiplicity
        || source.parameters[0].semantic.qualifications
            != source.parameters[1].semantic.qualifications
        || source.parameters[0].semantic.place == source.parameters[1].semantic.place
        || !is_extent_structural_type(source)
    {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    Ok(TerminalSelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count: 32,
        outgoing_frame_byte_count: 72,
        pre_call_stack_alignment: 16,
        bindings,
    })
}

fn is_extent_structural_type(source: &SourceStructuralUnitFunction) -> bool {
    let structural_type = source.parameters[0].semantic.structural_type;
    let Some(declaration) = source
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
    else {
        return false;
    };
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return false;
    };
    if fields.len() != 2
        || fields
            .iter()
            .any(|field| field.relevance != BindingRelevance::Relevant)
    {
        return false;
    }
    matches!(
        fields[0].field_type,
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
            if integer.carrier() == IntegerCarrier::Address
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    ) && matches!(
        fields[1].field_type,
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
            if integer.carrier() == IntegerCarrier::Fixed
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    )
}

fn structural_call_row(
    function: usize,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<&RegisterInstructionConstraint, SelectedInstructionError> {
    let key = keys
        .structural_unit_call
        .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
    let row = row(catalog, key)?;
    if !row.operands.is_empty() {
        return Err(SelectedInstructionError::MissingConstraint(key));
    }
    Ok(row)
}

fn build_structural_unit_function(
    function: usize,
    source: &SourceStructuralUnitFunction,
    plan: &TerminalLegalizedOperationPlan,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedStructuralUnitFunction, SelectedInstructionError> {
    if plan.target != omega_target::NativeTarget::uefi_x64() {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    let layout = structural_unit_layout(function, source)?;
    let call = source
        .call
        .as_ref()
        .map(|call| {
            let callee = plan
                .structural_unit_functions
                .iter()
                .find(|candidate| candidate.machine == call.callee)
                .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
            let callee_layout = structural_unit_layout(function, callee)?;
            if callee.call_plan != source.call_plan
                || callee_layout != layout
                || call.arguments.len() != 2
                || call.arguments.iter().enumerate().any(|(index, argument)| {
                    argument.semantic.access != StructuralAccess::Owned
                        || !argument.semantic.path.is_empty()
                        || argument.target.place != argument.semantic.place
                        || argument.target.access != argument.semantic.access
                        || argument.target.path != argument.semantic.path
                        || argument.target.root_structural_type
                            != source.parameters[index].semantic.structural_type
                        || argument.target.structural_type
                            != callee.parameters[index].semantic.structural_type
                        || argument.target.source_byte_offset != 0
                        || argument.target.fixed_array_length.is_some()
                        || argument.target.element_stride.is_some()
                        || argument.target.shape != source.parameters[index].target.shape
                        || argument.target.source != source.parameters[index].target.placement
                        || argument.target.destination != callee.parameters[index].target.placement
                })
            {
                return Err(SelectedInstructionError::UnsupportedSourceShape { function });
            }
            let row = structural_call_row(function, keys, catalog)?;
            Ok(TerminalSelectedStructuralUnitCallInstruction {
                id: TerminalSelectedInstructionId(0),
                operation: call.operation,
                callee: call.callee,
                caller_call_plan: source.call_plan.clone(),
                callee_call_plan: callee.call_plan.clone(),
                arguments: call
                    .arguments
                    .iter()
                    .map(|argument| TerminalSelectedStructuralUnitCallArgument {
                        semantic: argument.semantic.clone(),
                        target: argument.target.clone(),
                    })
                    .collect(),
                claim_transfers: call.claim_transfers.clone(),
                layout,
                constraint: row.key,
                implicit_uses: row.implicit_uses.clone(),
                implicit_defs: row.implicit_defs.clone(),
                clobbers: row.clobbers.clone(),
                provenance: TerminalSelectedInstructionProvenance {
                    operations: vec![call.operation],
                    fuel: call.fuel.clone(),
                    ..Default::default()
                },
                effect: call.effect,
                ownership: call.ownership.clone(),
            })
        })
        .transpose()?;
    let return_id = TerminalSelectedInstructionId(u32::from(call.is_some()));
    let return_instruction = instruction(
        return_id,
        TerminalSelectedInstructionKind::ReturnUnit,
        keys.return_unit,
        &[],
        TerminalSelectedInstructionProvenance {
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    Ok(TerminalSelectedStructuralUnitFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        structural_types: source.structural_types.clone(),
        abi: TerminalSelectedStructuralUnitAbi {
            recipe: TerminalSelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1,
            call_plan: source.call_plan.clone(),
            parameters: source
                .parameters
                .iter()
                .map(|parameter| TerminalSelectedStructuralUnitParameter {
                    semantic: parameter.semantic.clone(),
                    target: parameter.target.clone(),
                })
                .collect(),
            layout,
        },
        structural_places: source.structural_places.clone(),
        entry_claims: source.entry_claims.clone(),
        published_service_ceiling: source.published_service_ceiling.clone(),
        entry_block: TerminalSelectedBlockId(0),
        source_entry_block: source.entry_block,
        call,
        terminator: TerminalSelectedStructuralUnitReturn {
            instruction: return_instruction,
            psi_return_edge: source.return_edge,
            effect: source.return_effect,
            ownership: source.return_ownership.clone(),
        },
    })
}

fn build_function(
    function: usize,
    source: &SourceFunction,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedFunction, SelectedInstructionError> {
    let input = fixed_input_constraint(
        source.machine,
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
                source.machine,
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
        SourceLeafValue::ExactAdd { .. } | SourceLeafValue::WidenedExactAdd { .. } => {
            row(catalog, keys.add_i64)?.operands[2].class
        }
        SourceLeafValue::ActiveResidentExactAddChain(..) => {
            row(catalog, keys.add_i64)?.operands[2].class
        }
        SourceLeafValue::ExactSubtract { .. } | SourceLeafValue::WidenedExactSubtract { .. } => {
            row(catalog, keys.subtract_i64)?.operands[2].class
        }
    };
    let u64_type =
        ScalarType::Integer(psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    Ok(TerminalSelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
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
                    SourceLeafValue::ActiveResidentExactAddChain(chain),
                    SourceLeafValue::Immediate {
                        definition_site: false_site,
                        ..
                    },
                ) => {
                    for (id, instruction, source_value, definition_site) in [
                        (
                            1,
                            2,
                            chain.resident.source_value,
                            chain.resident.definition_site,
                        ),
                        (2, 3, chain.left.source_value, chain.left.definition_site),
                        (3, 4, chain.right.source_value, chain.right.definition_site),
                        (4, 5, chain.inner.source_value, chain.inner.definition_site),
                        (
                            5,
                            6,
                            chain.middle.source_value,
                            chain.middle.definition_site,
                        ),
                        (
                            6,
                            7,
                            chain.result.source_value,
                            chain.result.definition_site,
                        ),
                        (7, 9, source.when_false.source_value, *false_site),
                    ] {
                        registers.push(TerminalVirtualRegister {
                            id: TerminalVirtualRegisterId(id),
                            scalar_type: u64_type,
                            class: result_class,
                            origin: TerminalVirtualRegisterOrigin::InstructionResult {
                                instruction: TerminalSelectedInstructionId(instruction),
                                source_value,
                            },
                            definition_site,
                            entry_fixed_view: None,
                        });
                    }
                }
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
                        source.machine,
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
                (
                    SourceLeafValue::WidenedExactAdd {
                        widen_definition_site: true_site,
                        left_temporary: true_left_temporary,
                        right_temporary: true_right_temporary,
                        left: true_left,
                        right: true_right,
                        ..
                    }
                    | SourceLeafValue::WidenedExactSubtract {
                        widen_definition_site: true_site,
                        left_temporary: true_left_temporary,
                        right_temporary: true_right_temporary,
                        left: true_left,
                        right: true_right,
                        ..
                    },
                    SourceLeafValue::WidenedExactAdd {
                        widen_definition_site: false_site,
                        left_temporary: false_left_temporary,
                        right_temporary: false_right_temporary,
                        left: false_left,
                        right: false_right,
                        ..
                    }
                    | SourceLeafValue::WidenedExactSubtract {
                        widen_definition_site: false_site,
                        left_temporary: false_left_temporary,
                        right_temporary: false_right_temporary,
                        left: false_left,
                        right: false_right,
                        ..
                    },
                ) => {
                    for (id, instruction, source_value, definition_site, legalized_temporary) in [
                        (
                            1,
                            2,
                            true_left.source_value,
                            true_left.definition_site,
                            Some(*true_left_temporary),
                        ),
                        (
                            2,
                            3,
                            true_right.source_value,
                            true_right.definition_site,
                            Some(*true_right_temporary),
                        ),
                        (3, 4, source.when_true.source_value, *true_site, None),
                        (
                            4,
                            6,
                            false_left.source_value,
                            false_left.definition_site,
                            Some(*false_left_temporary),
                        ),
                        (
                            5,
                            7,
                            false_right.source_value,
                            false_right.definition_site,
                            Some(*false_right_temporary),
                        ),
                        (6, 8, source.when_false.source_value, *false_site, None),
                    ] {
                        registers.push(TerminalVirtualRegister {
                            id: TerminalVirtualRegisterId(id),
                            scalar_type: u64_type,
                            class: result_class,
                            origin: match legalized_temporary {
                                Some(temporary) => {
                                    TerminalVirtualRegisterOrigin::LegalizationTemporary {
                                        instruction: TerminalSelectedInstructionId(instruction),
                                        temporary,
                                        source_value,
                                    }
                                }
                                None => TerminalVirtualRegisterOrigin::InstructionResult {
                                    instruction: TerminalSelectedInstructionId(instruction),
                                    source_value,
                                },
                            },
                            definition_site,
                            entry_fixed_view: None,
                        });
                    }
                }
                (
                    SourceLeafValue::ExactAdd {
                        definition_site: true_site,
                        left: true_left,
                        right: true_right,
                        ..
                    }
                    | SourceLeafValue::ExactSubtract {
                        definition_site: true_site,
                        left: true_left,
                        right: true_right,
                        ..
                    },
                    SourceLeafValue::ExactAdd {
                        definition_site: false_site,
                        left: false_left,
                        right: false_right,
                        ..
                    }
                    | SourceLeafValue::ExactSubtract {
                        definition_site: false_site,
                        left: false_left,
                        right: false_right,
                        ..
                    },
                ) => {
                    for (id, instruction, source_value, definition_site) in [
                        (1, 2, true_left.source_value, true_left.definition_site),
                        (2, 3, true_right.source_value, true_right.definition_site),
                        (3, 4, source.when_true.source_value, *true_site),
                        (4, 6, false_left.source_value, false_left.definition_site),
                        (5, 7, false_right.source_value, false_right.definition_site),
                        (6, 8, source.when_false.source_value, *false_site),
                    ] {
                        registers.push(TerminalVirtualRegister {
                            id: TerminalVirtualRegisterId(id),
                            scalar_type: u64_type,
                            class: result_class,
                            origin: TerminalVirtualRegisterOrigin::InstructionResult {
                                instruction: TerminalSelectedInstructionId(instruction),
                                source_value,
                            },
                            definition_site,
                            entry_fixed_view: None,
                        });
                    }
                }
                _ => return Err(SelectedInstructionError::UnsupportedSourceShape { function }),
            }
            registers
        },
        blocks: match (&source.when_true.value, &source.when_false.value) {
            (
                SourceLeafValue::ActiveResidentExactAddChain(..),
                SourceLeafValue::Immediate { .. },
            ) => vec![
                build_entry_block(source, keys, catalog)?,
                build_active_resident_exact_add_chain_block(
                    function,
                    TerminalSelectedBlockId(1),
                    source.true_block,
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_constant_return_block(
                    function,
                    TerminalSelectedBlockId(2),
                    source.false_block,
                    9,
                    10,
                    TerminalVirtualRegisterId(7),
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
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
            (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => vec![
                build_entry_block(source, keys, catalog)?,
                build_exact_binary_return_block(
                    function,
                    TerminalSelectedBlockId(1),
                    source.true_block,
                    [2, 3, 4, 5],
                    [
                        TerminalVirtualRegisterId(1),
                        TerminalVirtualRegisterId(2),
                        TerminalVirtualRegisterId(3),
                    ],
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_exact_binary_return_block(
                    function,
                    TerminalSelectedBlockId(2),
                    source.false_block,
                    [6, 7, 8, 9],
                    [
                        TerminalVirtualRegisterId(4),
                        TerminalVirtualRegisterId(5),
                        TerminalVirtualRegisterId(6),
                    ],
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
                vec![
                    build_entry_block(source, keys, catalog)?,
                    build_exact_binary_return_block(
                        function,
                        TerminalSelectedBlockId(1),
                        source.true_block,
                        [2, 3, 4, 5],
                        [
                            TerminalVirtualRegisterId(1),
                            TerminalVirtualRegisterId(2),
                            TerminalVirtualRegisterId(3),
                        ],
                        &source.when_true,
                        keys,
                        catalog,
                    )?,
                    build_exact_binary_return_block(
                        function,
                        TerminalSelectedBlockId(2),
                        source.false_block,
                        [6, 7, 8, 9],
                        [
                            TerminalVirtualRegisterId(4),
                            TerminalVirtualRegisterId(5),
                            TerminalVirtualRegisterId(6),
                        ],
                        &source.when_false,
                        keys,
                        catalog,
                    )?,
                ]
            }
            (
                SourceLeafValue::WidenedExactSubtract { .. },
                SourceLeafValue::WidenedExactSubtract { .. },
            ) => vec![
                build_entry_block(source, keys, catalog)?,
                build_exact_binary_return_block(
                    function,
                    TerminalSelectedBlockId(1),
                    source.true_block,
                    [2, 3, 4, 5],
                    [
                        TerminalVirtualRegisterId(1),
                        TerminalVirtualRegisterId(2),
                        TerminalVirtualRegisterId(3),
                    ],
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_exact_binary_return_block(
                    function,
                    TerminalSelectedBlockId(2),
                    source.false_block,
                    [6, 7, 8, 9],
                    [
                        TerminalVirtualRegisterId(4),
                        TerminalVirtualRegisterId(5),
                        TerminalVirtualRegisterId(6),
                    ],
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
            (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => vec![
                build_entry_block(source, keys, catalog)?,
                build_exact_binary_return_block(
                    function,
                    TerminalSelectedBlockId(1),
                    source.true_block,
                    [2, 3, 4, 5],
                    [
                        TerminalVirtualRegisterId(1),
                        TerminalVirtualRegisterId(2),
                        TerminalVirtualRegisterId(3),
                    ],
                    &source.when_true,
                    keys,
                    catalog,
                )?,
                build_exact_binary_return_block(
                    function,
                    TerminalSelectedBlockId(2),
                    source.false_block,
                    [6, 7, 8, 9],
                    [
                        TerminalVirtualRegisterId(4),
                        TerminalVirtualRegisterId(5),
                        TerminalVirtualRegisterId(6),
                    ],
                    &source.when_false,
                    keys,
                    catalog,
                )?,
            ],
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

#[allow(clippy::too_many_arguments)]
fn build_exact_binary_return_block(
    function: usize,
    id: TerminalSelectedBlockId,
    source_block: psi_core::BlockId,
    instruction_ids: [u32; 4],
    registers: [TerminalVirtualRegisterId; 3],
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedBlock, SelectedInstructionError> {
    let (obligation, operations, values, operation_fuel, left, right, kind, key) =
        match &source.value {
            SourceLeafValue::ExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                add_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation],
                vec![left.source_value, right.source_value, source.source_value],
                add_fuel.clone(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::WidenedExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                narrow_result,
                add_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                add_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::ExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                subtract_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation],
                vec![left.source_value, right.source_value, source.source_value],
                subtract_fuel.clone(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            SourceLeafValue::WidenedExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                narrow_result,
                subtract_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                subtract_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            _ => return Err(SelectedInstructionError::UnsupportedSourceShape { function }),
        };
    let materialize = |id, register, immediate: &SourceImmediate| {
        instruction(
            TerminalSelectedInstructionId(id),
            TerminalSelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            TerminalSelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )
    };
    Ok(TerminalSelectedBlock {
        id,
        source_block,
        instructions: vec![
            materialize(instruction_ids[0], registers[0], left)?,
            materialize(instruction_ids[1], registers[1], right)?,
            instruction(
                TerminalSelectedInstructionId(instruction_ids[2]),
                kind,
                key,
                &registers,
                TerminalSelectedInstructionProvenance {
                    operations,
                    values,
                    obligations: vec![*obligation],
                    fuel: operation_fuel,
                    ..Default::default()
                },
                catalog,
            )?,
        ],
        terminator: TerminalSelectedTerminator::Return {
            instruction: instruction(
                TerminalSelectedInstructionId(instruction_ids[3]),
                TerminalSelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[registers[2]],
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

fn build_active_resident_exact_add_chain_block(
    function: usize,
    id: TerminalSelectedBlockId,
    source_block: psi_core::BlockId,
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<TerminalSelectedBlock, SelectedInstructionError> {
    let SourceLeafValue::ActiveResidentExactAddChain(chain) = &source.value else {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    };
    let materialize = |id, register, immediate: &SourceImmediate| {
        instruction(
            TerminalSelectedInstructionId(id),
            TerminalSelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            TerminalSelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )
    };
    let exact_add = |id,
                     operands: [TerminalVirtualRegisterId; 3],
                     add: &omega_terminal_legalized_operations::TerminalLegalizedExactAdd,
                     values: Vec<psi_core::ValueId>| {
        instruction(
            TerminalSelectedInstructionId(id),
            TerminalSelectedInstructionKind::ExactAddI64 {
                obligation: add.obligation,
                accepted_fact: add.accepted_fact,
            },
            keys.add_i64,
            &operands,
            TerminalSelectedInstructionProvenance {
                operations: vec![add.operation],
                values,
                obligations: vec![add.obligation],
                fuel: add.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )
    };
    Ok(TerminalSelectedBlock {
        id,
        source_block,
        instructions: vec![
            materialize(2, TerminalVirtualRegisterId(1), &chain.resident)?,
            materialize(3, TerminalVirtualRegisterId(2), &chain.left)?,
            materialize(4, TerminalVirtualRegisterId(3), &chain.right)?,
            exact_add(
                5,
                [
                    TerminalVirtualRegisterId(2),
                    TerminalVirtualRegisterId(3),
                    TerminalVirtualRegisterId(4),
                ],
                &chain.inner,
                vec![
                    chain.left.source_value,
                    chain.right.source_value,
                    chain.inner.source_value,
                ],
            )?,
            exact_add(
                6,
                [
                    TerminalVirtualRegisterId(1),
                    TerminalVirtualRegisterId(4),
                    TerminalVirtualRegisterId(5),
                ],
                &chain.middle,
                vec![
                    chain.resident.source_value,
                    chain.inner.source_value,
                    chain.middle.source_value,
                ],
            )?,
            exact_add(
                7,
                [
                    TerminalVirtualRegisterId(1),
                    TerminalVirtualRegisterId(5),
                    TerminalVirtualRegisterId(6),
                ],
                &chain.result,
                vec![
                    chain.resident.source_value,
                    chain.middle.source_value,
                    chain.result.source_value,
                ],
            )?,
        ],
        terminator: TerminalSelectedTerminator::Return {
            instruction: instruction(
                TerminalSelectedInstructionId(8),
                TerminalSelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[TerminalVirtualRegisterId(6)],
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
        keys.copy_i64,
        keys.add_i64,
        keys.add_i64_immediate,
        keys.compare_i64_zero,
        keys.conditional_branch,
        keys.return_i64,
        keys.return_unit,
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
    source: &SourceFunction,
    function: &TerminalSelectedFunction,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if function.machine != source.machine
        || function.attachment != source.attachment
        || function.provenance != source.provenance
        || function.entry_block != TerminalSelectedBlockId(0)
    {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }
    validate_dense(function_index, source, function)?;
    validate_virtual_registers(
        function_index,
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

fn validate_unit_function(
    function_index: usize,
    source: &SourceUnitFunction,
    function: &TerminalSelectedFunction,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let expected_provenance = TerminalSelectedInstructionProvenance {
        edges: vec![source.return_edge],
        fuel: source.return_fuel.clone(),
        ..Default::default()
    };
    let valid_shape = function.machine == source.machine
        && function.attachment == source.attachment
        && function.provenance == source.provenance
        && function.entry_block == TerminalSelectedBlockId(0)
        && function.virtual_registers.is_empty()
        && function.blocks.len() == 1
        && function.blocks[0].id == TerminalSelectedBlockId(0)
        && function.blocks[0].source_block == source.entry_block
        && function.blocks[0].instructions.is_empty();
    if !valid_shape {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }
    let block = &function.blocks[0];
    let TerminalSelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    };
    if instruction.id != TerminalSelectedInstructionId(0)
        || instruction.kind != TerminalSelectedInstructionKind::ReturnUnit
        || instruction.constraint != keys.return_unit
        || !instruction.operands.is_empty()
        || instruction.provenance != expected_provenance
        || *psi_return_edge != source.return_edge
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function: function_index,
            instruction: instruction.id.0,
        });
    }
    validate_block_constraints(function_index, block, function, catalog)?;
    validate_def_use(function_index, function, catalog)
}

fn validate_structural_unit_function(
    function_index: usize,
    source: &SourceStructuralUnitFunction,
    selected: &TerminalSelectedStructuralUnitFunction,
    plan: &TerminalLegalizedOperationPlan,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if plan.target != omega_target::NativeTarget::uefi_x64() {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        });
    }
    let layout = structural_unit_layout(function_index, source)?;
    let parameters_match = selected.abi.parameters.len() == source.parameters.len()
        && selected
            .abi
            .parameters
            .iter()
            .zip(&source.parameters)
            .all(|(selected, source)| {
                selected.semantic == source.semantic && selected.target == source.target
            });
    if selected.machine != source.machine
        || selected.attachment != source.attachment
        || selected.provenance != source.provenance
        || selected.structural_types != source.structural_types
        || selected.abi.recipe
            != TerminalSelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1
        || selected.abi.call_plan != source.call_plan
        || !parameters_match
        || selected.abi.layout != layout
        || selected.structural_places != source.structural_places
        || selected.entry_claims != source.entry_claims
        || selected.published_service_ceiling != source.published_service_ceiling
        || selected.entry_block != TerminalSelectedBlockId(0)
        || selected.source_entry_block != source.entry_block
    {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }

    match (&source.call, &selected.call) {
        (None, None) => {}
        (Some(source_call), Some(selected_call)) => {
            let Some(callee) = plan
                .structural_unit_functions
                .iter()
                .find(|candidate| candidate.machine == source_call.callee)
            else {
                return Err(SelectedInstructionError::SourceCustodyMismatch);
            };
            let callee_layout = structural_unit_layout(function_index, callee)?;
            let row = structural_call_row(function_index, keys, catalog)?;
            let arguments_match = selected_call.arguments.len() == source_call.arguments.len()
                && selected_call
                    .arguments
                    .iter()
                    .zip(&source_call.arguments)
                    .all(|(selected, source)| {
                        selected.semantic == source.semantic && selected.target == source.target
                    });
            let call_shape_valid = source_call.arguments.len() == 2
                && source_call
                    .arguments
                    .iter()
                    .enumerate()
                    .all(|(index, argument)| {
                        argument.semantic.access == StructuralAccess::Owned
                            && argument.semantic.path.is_empty()
                            && argument.target.place == argument.semantic.place
                            && argument.target.access == argument.semantic.access
                            && argument.target.path == argument.semantic.path
                            && argument.target.root_structural_type
                                == source.parameters[index].semantic.structural_type
                            && argument.target.structural_type
                                == callee.parameters[index].semantic.structural_type
                            && argument.target.source_byte_offset == 0
                            && argument.target.fixed_array_length.is_none()
                            && argument.target.element_stride.is_none()
                            && argument.target.shape == source.parameters[index].target.shape
                            && argument.target.source == source.parameters[index].target.placement
                            && argument.target.destination
                                == callee.parameters[index].target.placement
                    });
            if callee.call_plan != source.call_plan
                || callee_layout != layout
                || !call_shape_valid
                || selected_call.id != TerminalSelectedInstructionId(0)
                || selected_call.operation != source_call.operation
                || selected_call.callee != source_call.callee
                || selected_call.caller_call_plan != source.call_plan
                || selected_call.callee_call_plan != callee.call_plan
                || !arguments_match
                || selected_call.claim_transfers != source_call.claim_transfers
                || selected_call.layout != layout
                || selected_call.constraint != row.key
                || selected_call.implicit_uses != row.implicit_uses
                || selected_call.implicit_defs != row.implicit_defs
                || selected_call.clobbers != row.clobbers
                || selected_call.provenance
                    != (TerminalSelectedInstructionProvenance {
                        operations: vec![source_call.operation],
                        fuel: source_call.fuel.clone(),
                        ..Default::default()
                    })
                || selected_call.effect != source_call.effect
                || selected_call.ownership != source_call.ownership
            {
                return Err(SelectedInstructionError::InstructionProjectionMismatch {
                    function: function_index,
                    instruction: selected_call.id.0,
                });
            }
        }
        _ => {
            return Err(SelectedInstructionError::FunctionProjectionMismatch {
                function: function_index,
            });
        }
    }

    let return_id = TerminalSelectedInstructionId(u32::from(source.call.is_some()));
    let instruction = &selected.terminator.instruction;
    let return_row = row(catalog, keys.return_unit)?;
    if instruction.id != return_id
        || instruction.kind != TerminalSelectedInstructionKind::ReturnUnit
        || instruction.constraint != keys.return_unit
        || !instruction.operands.is_empty()
        || instruction.implicit_uses != return_row.implicit_uses
        || instruction.implicit_defs != return_row.implicit_defs
        || instruction.clobbers != return_row.clobbers
        || instruction.provenance
            != (TerminalSelectedInstructionProvenance {
                edges: vec![source.return_edge],
                fuel: source.return_fuel.clone(),
                ..Default::default()
            })
        || selected.terminator.psi_return_edge != source.return_edge
        || selected.terminator.effect != source.return_effect
        || selected.terminator.ownership != source.return_ownership
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function: function_index,
            instruction: instruction.id.0,
        });
    }
    Ok(())
}

fn validate_virtual_registers(
    function_index: usize,
    source: &SourceFunction,
    function: &TerminalSelectedFunction,
    constraints: &TerminalSelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let input = fixed_input_constraint(
        source.machine,
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
            SourceLeafValue::ActiveResidentExactAddChain(chain),
            SourceLeafValue::Immediate {
                definition_site: false_site,
                ..
            },
        ) => {
            let binary = row(catalog, constraints.keys.add_i64)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 5,
                });
            }
            for (instruction, source_value, definition_site) in [
                (
                    2,
                    chain.resident.source_value,
                    chain.resident.definition_site,
                ),
                (3, chain.left.source_value, chain.left.definition_site),
                (4, chain.right.source_value, chain.right.definition_site),
                (5, chain.inner.source_value, chain.inner.definition_site),
                (6, chain.middle.source_value, chain.middle.definition_site),
                (7, chain.result.source_value, chain.result.definition_site),
                (9, source.when_false.source_value, *false_site),
            ] {
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    TerminalVirtualRegisterOrigin::InstructionResult {
                        instruction: TerminalSelectedInstructionId(instruction),
                        source_value,
                    },
                    definition_site,
                    None,
                ));
            }
        }
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
                source.machine,
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
        (
            SourceLeafValue::WidenedExactAdd {
                widen_definition_site: true_site,
                left_temporary: true_left_temporary,
                right_temporary: true_right_temporary,
                left: true_left,
                right: true_right,
                ..
            }
            | SourceLeafValue::WidenedExactSubtract {
                widen_definition_site: true_site,
                left_temporary: true_left_temporary,
                right_temporary: true_right_temporary,
                left: true_left,
                right: true_right,
                ..
            },
            SourceLeafValue::WidenedExactAdd {
                widen_definition_site: false_site,
                left_temporary: false_left_temporary,
                right_temporary: false_right_temporary,
                left: false_left,
                right: false_right,
                ..
            }
            | SourceLeafValue::WidenedExactSubtract {
                widen_definition_site: false_site,
                left_temporary: false_left_temporary,
                right_temporary: false_right_temporary,
                left: false_left,
                right: false_right,
                ..
            },
        ) => {
            let binary_key = match &source.when_true.value {
                SourceLeafValue::WidenedExactAdd { .. } => constraints.keys.add_i64,
                SourceLeafValue::WidenedExactSubtract { .. } => constraints.keys.subtract_i64,
                _ => unreachable!("matched widened exact binary leaves"),
            };
            let binary = row(catalog, binary_key)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 4,
                });
            }
            for (instruction, source_value, definition_site, temporary) in [
                (
                    2,
                    true_left.source_value,
                    true_left.definition_site,
                    Some(*true_left_temporary),
                ),
                (
                    3,
                    true_right.source_value,
                    true_right.definition_site,
                    Some(*true_right_temporary),
                ),
                (4, source.when_true.source_value, *true_site, None),
                (
                    6,
                    false_left.source_value,
                    false_left.definition_site,
                    Some(*false_left_temporary),
                ),
                (
                    7,
                    false_right.source_value,
                    false_right.definition_site,
                    Some(*false_right_temporary),
                ),
                (8, source.when_false.source_value, *false_site, None),
            ] {
                let instruction = TerminalSelectedInstructionId(instruction);
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    match temporary {
                        Some(temporary) => TerminalVirtualRegisterOrigin::LegalizationTemporary {
                            instruction,
                            temporary,
                            source_value,
                        },
                        None => TerminalVirtualRegisterOrigin::InstructionResult {
                            instruction,
                            source_value,
                        },
                    },
                    definition_site,
                    None,
                ));
            }
        }
        (
            SourceLeafValue::ExactAdd {
                definition_site: true_site,
                left: true_left,
                right: true_right,
                ..
            }
            | SourceLeafValue::ExactSubtract {
                definition_site: true_site,
                left: true_left,
                right: true_right,
                ..
            },
            SourceLeafValue::ExactAdd {
                definition_site: false_site,
                left: false_left,
                right: false_right,
                ..
            }
            | SourceLeafValue::ExactSubtract {
                definition_site: false_site,
                left: false_left,
                right: false_right,
                ..
            },
        ) => {
            let binary_key = match &source.when_true.value {
                SourceLeafValue::ExactAdd { .. } => constraints.keys.add_i64,
                SourceLeafValue::ExactSubtract { .. } => constraints.keys.subtract_i64,
                _ => unreachable!("matched exact binary leaves"),
            };
            let binary = row(catalog, binary_key)?;
            let materialize = row(catalog, constraints.keys.materialize_i64)?;
            if binary.operands.len() != 3
                || materialize.operands.len() != 1
                || binary
                    .operands
                    .iter()
                    .any(|operand| operand.class != binary.operands[2].class)
                || materialize.operands[0].class != binary.operands[2].class
            {
                return Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: function_index,
                    instruction: 4,
                });
            }
            for (instruction, source_value, definition_site) in [
                (2, true_left.source_value, true_left.definition_site),
                (3, true_right.source_value, true_right.definition_site),
                (4, source.when_true.source_value, *true_site),
                (6, false_left.source_value, false_left.definition_site),
                (7, false_right.source_value, false_right.definition_site),
                (8, source.when_false.source_value, *false_site),
            ] {
                expected.push((
                    u64_type,
                    binary.operands[2].class,
                    TerminalVirtualRegisterOrigin::InstructionResult {
                        instruction: TerminalSelectedInstructionId(instruction),
                        source_value,
                    },
                    definition_site,
                    None,
                ));
            }
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
        (SourceLeafValue::ActiveResidentExactAddChain(..), SourceLeafValue::Immediate { .. }) => {
            validate_active_resident_exact_add_chain_block_projection(
                function_index,
                &function.blocks[1],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_constant_return_block_projection(
                function_index,
                &function.blocks[2],
                9,
                10,
                TerminalVirtualRegisterId(7),
                &source.when_false,
                keys,
                catalog,
            )
        }
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
        (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    TerminalVirtualRegisterId(1),
                    TerminalVirtualRegisterId(2),
                    TerminalVirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    TerminalVirtualRegisterId(4),
                    TerminalVirtualRegisterId(5),
                    TerminalVirtualRegisterId(6),
                ],
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    TerminalVirtualRegisterId(1),
                    TerminalVirtualRegisterId(2),
                    TerminalVirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    TerminalVirtualRegisterId(4),
                    TerminalVirtualRegisterId(5),
                    TerminalVirtualRegisterId(6),
                ],
                &source.when_false,
                keys,
                catalog,
            )
        }
        (
            SourceLeafValue::WidenedExactSubtract { .. },
            SourceLeafValue::WidenedExactSubtract { .. },
        ) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    TerminalVirtualRegisterId(1),
                    TerminalVirtualRegisterId(2),
                    TerminalVirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    TerminalVirtualRegisterId(4),
                    TerminalVirtualRegisterId(5),
                    TerminalVirtualRegisterId(6),
                ],
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => {
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[1],
                [2, 3, 4, 5],
                [
                    TerminalVirtualRegisterId(1),
                    TerminalVirtualRegisterId(2),
                    TerminalVirtualRegisterId(3),
                ],
                &source.when_true,
                keys,
                catalog,
            )?;
            validate_exact_binary_return_block_projection(
                function_index,
                &function.blocks[2],
                [6, 7, 8, 9],
                [
                    TerminalVirtualRegisterId(4),
                    TerminalVirtualRegisterId(5),
                    TerminalVirtualRegisterId(6),
                ],
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
fn validate_exact_binary_return_block_projection(
    function_index: usize,
    block: &TerminalSelectedBlock,
    instruction_ids: [u32; 4],
    registers: [TerminalVirtualRegisterId; 3],
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let (obligation, operations, values, operation_fuel, left, right, kind, key) =
        match &source.value {
            SourceLeafValue::ExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                add_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation],
                vec![left.source_value, right.source_value, source.source_value],
                add_fuel.clone(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::WidenedExactAdd {
                obligation,
                accepted_fact,
                add_operation,
                narrow_result,
                add_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*add_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                add_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.add_i64,
            ),
            SourceLeafValue::ExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                subtract_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation],
                vec![left.source_value, right.source_value, source.source_value],
                subtract_fuel.clone(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            SourceLeafValue::WidenedExactSubtract {
                obligation,
                accepted_fact,
                subtract_operation,
                narrow_result,
                subtract_fuel,
                widen_operation,
                widen_fuel,
                left,
                right,
                ..
            } => (
                obligation,
                vec![*subtract_operation, *widen_operation],
                vec![
                    left.source_value,
                    right.source_value,
                    *narrow_result,
                    source.source_value,
                ],
                subtract_fuel.iter().chain(widen_fuel).copied().collect(),
                left,
                right,
                TerminalSelectedInstructionKind::ExactSubtractI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                keys.subtract_i64,
            ),
            _ => {
                return Err(SelectedInstructionError::UnsupportedSourceShape {
                    function: function_index,
                });
            }
        };
    if block.instructions.len() != 3 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    for (position, immediate) in [left, right].into_iter().enumerate() {
        validate_instruction_projection(
            function_index,
            &block.instructions[position],
            TerminalSelectedInstructionId(instruction_ids[position]),
            TerminalSelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[registers[position]],
            &TerminalSelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    validate_instruction_projection(
        function_index,
        &block.instructions[2],
        TerminalSelectedInstructionId(instruction_ids[2]),
        kind,
        key,
        &registers,
        &TerminalSelectedInstructionProvenance {
            operations,
            values,
            obligations: vec![*obligation],
            fuel: operation_fuel,
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
        TerminalSelectedInstructionId(instruction_ids[3]),
        TerminalSelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[registers[2]],
        &TerminalSelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}

fn validate_active_resident_exact_add_chain_block_projection(
    function: usize,
    block: &TerminalSelectedBlock,
    source: &SourceLeaf,
    keys: TerminalSelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let SourceLeafValue::ActiveResidentExactAddChain(chain) = &source.value else {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    };
    if block.instructions.len() != 6 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function,
            block: block.id.0,
        });
    }
    for (position, (id, register, immediate)) in [
        (2, TerminalVirtualRegisterId(1), &chain.resident),
        (3, TerminalVirtualRegisterId(2), &chain.left),
        (4, TerminalVirtualRegisterId(3), &chain.right),
    ]
    .into_iter()
    .enumerate()
    {
        validate_instruction_projection(
            function,
            &block.instructions[position],
            TerminalSelectedInstructionId(id),
            TerminalSelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            &TerminalSelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    for (position, (id, registers, add, values)) in [
        (
            5,
            [
                TerminalVirtualRegisterId(2),
                TerminalVirtualRegisterId(3),
                TerminalVirtualRegisterId(4),
            ],
            &chain.inner,
            vec![
                chain.left.source_value,
                chain.right.source_value,
                chain.inner.source_value,
            ],
        ),
        (
            6,
            [
                TerminalVirtualRegisterId(1),
                TerminalVirtualRegisterId(4),
                TerminalVirtualRegisterId(5),
            ],
            &chain.middle,
            vec![
                chain.resident.source_value,
                chain.inner.source_value,
                chain.middle.source_value,
            ],
        ),
        (
            7,
            [
                TerminalVirtualRegisterId(1),
                TerminalVirtualRegisterId(5),
                TerminalVirtualRegisterId(6),
            ],
            &chain.result,
            vec![
                chain.resident.source_value,
                chain.middle.source_value,
                chain.result.source_value,
            ],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        validate_instruction_projection(
            function,
            &block.instructions[position + 3],
            TerminalSelectedInstructionId(id),
            TerminalSelectedInstructionKind::ExactAddI64 {
                obligation: add.obligation,
                accepted_fact: add.accepted_fact,
            },
            keys.add_i64,
            &registers,
            &TerminalSelectedInstructionProvenance {
                operations: vec![add.operation],
                values,
                obligations: vec![add.obligation],
                fuel: add.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    let TerminalSelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function,
            block: block.id.0,
        });
    }
    validate_instruction_projection(
        function,
        instruction,
        TerminalSelectedInstructionId(8),
        TerminalSelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[TerminalVirtualRegisterId(6)],
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
            (
                SourceLeafValue::ActiveResidentExactAddChain(..),
                SourceLeafValue::Immediate { .. },
            ) => (8, 11),
            (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
                (2, 4)
            }
            (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => (7, 10),
            (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
                (7, 10)
            }
            (
                SourceLeafValue::WidenedExactSubtract { .. },
                SourceLeafValue::WidenedExactSubtract { .. },
            ) => (7, 10),
            (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => {
                (7, 10)
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
            SourceLeafValue::ExactAdd {
                add_fuel,
                left,
                right,
                ..
            } => {
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != *add_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::WidenedExactAdd {
                add_fuel,
                widen_fuel,
                left,
                right,
                ..
            } => {
                let legal_fuel = add_fuel
                    .iter()
                    .chain(widen_fuel)
                    .copied()
                    .collect::<Vec<_>>();
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != legal_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::WidenedExactSubtract {
                subtract_fuel,
                widen_fuel,
                left,
                right,
                ..
            } => {
                let legal_fuel = subtract_fuel
                    .iter()
                    .chain(widen_fuel)
                    .copied()
                    .collect::<Vec<_>>();
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != legal_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::ExactSubtract {
                subtract_fuel,
                left,
                right,
                ..
            } => {
                if block.instructions.len() != 3
                    || block.instructions[0].provenance.fuel != left.fuel
                    || block.instructions[1].provenance.fuel != right.fuel
                    || block.instructions[2].provenance.fuel != *subtract_fuel
                    || instruction.provenance.fuel != leaf.return_fuel
                {
                    return Err(SelectedInstructionError::ProvenancePartitionMismatch {
                        function: function_index,
                    });
                }
            }
            SourceLeafValue::ActiveResidentExactAddChain(chain) => {
                if block.instructions.len() != 6
                    || block.instructions[0].provenance.fuel != chain.resident.fuel
                    || block.instructions[1].provenance.fuel != chain.left.fuel
                    || block.instructions[2].provenance.fuel != chain.right.fuel
                    || block.instructions[3].provenance.fuel != chain.inner.fuel
                    || block.instructions[4].provenance.fuel != chain.middle.fuel
                    || block.instructions[5].provenance.fuel != chain.result.fuel
                    || instruction.provenance.fuel != leaf.return_fuel
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
    legalized: &ValidatedTerminalLegalizedOperations,
) -> TerminalSelectedInstructionValidationReceipt {
    let function_count = plan.functions.len() + plan.structural_unit_functions.len();
    let block_count = plan
        .functions
        .iter()
        .map(|function| function.blocks.len())
        .sum::<usize>()
        + plan.structural_unit_functions.len();
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
        .sum::<usize>()
        + plan
            .structural_unit_functions
            .iter()
            .map(|function| 1 + usize::from(function.call.is_some()))
            .sum::<usize>();
    TerminalSelectedInstructionValidationReceipt {
        identity: terminal_selected_instruction_plan_identity(plan),
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        optimization_unit: legalized.receipt().optimization_unit(),
        fuel_schedule: legalized.receipt().fuel_schedule(),
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
    bytes.extend_from_slice(b"omega.terminal-selected-instructions.v9\0");
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
                TerminalVirtualRegisterOrigin::LegalizationTemporary {
                    instruction,
                    temporary,
                    source_value,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&temporary.0.to_le_bytes());
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
    bytes.extend_from_slice(&selected_structural_legalized_identity(plan).bytes());
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        encode_selected_structural_unit_function(&mut bytes, function);
    }
    TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(&bytes)
}

fn selected_structural_legalized_identity(
    plan: &TerminalSelectedInstructionPlan,
) -> TerminalLegalizedOperationPlanIdentity {
    terminal_legalized_operation_plan_identity(&TerminalLegalizedOperationPlan {
        terminal_psi: plan.terminal_psi,
        optimization_unit: omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
            b"omega.selected-structural-legalized-fingerprint.v1",
        ),
        fuel_schedule: plan.fuel_schedule,
        target: plan.target,
        entry: plan.entry,
        functions: Vec::new(),
        unit_functions: Vec::new(),
        structural_unit_functions: plan
            .structural_unit_functions
            .iter()
            .map(|function| SourceStructuralUnitFunction {
                machine: function.machine,
                attachment: function.attachment,
                provenance: function.provenance.clone(),
                structural_types: function.structural_types.clone(),
                call_plan: function.abi.call_plan.clone(),
                parameters: function
                    .abi
                    .parameters
                    .iter()
                    .map(|parameter| TerminalLegalizedCallUnitParameter {
                        semantic: parameter.semantic.clone(),
                        target: parameter.target.clone(),
                    })
                    .collect(),
                structural_places: function.structural_places.clone(),
                entry_claims: function.entry_claims.clone(),
                published_service_ceiling: function.published_service_ceiling.clone(),
                entry_block: function.source_entry_block,
                call: function
                    .call
                    .as_ref()
                    .map(|call| TerminalLegalizedCallUnit {
                        operation: call.operation,
                        callee: call.callee,
                        arguments: call
                            .arguments
                            .iter()
                            .map(|argument| TerminalLegalizedCallUnitArgument {
                                semantic: argument.semantic.clone(),
                                target: argument.target.clone(),
                            })
                            .collect(),
                        claim_transfers: call.claim_transfers.clone(),
                        fuel: call.provenance.fuel.clone(),
                        effect: call.effect,
                        ownership: call.ownership.clone(),
                    }),
                return_edge: function.terminator.psi_return_edge,
                return_fuel: function.terminator.instruction.provenance.fuel.clone(),
                return_effect: function.terminator.effect,
                return_ownership: function.terminator.ownership.clone(),
            })
            .collect(),
    })
}

fn encode_selected_structural_unit_function(
    bytes: &mut Vec<u8>,
    function: &TerminalSelectedStructuralUnitFunction,
) {
    bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
    bytes.push(match function.abi.recipe {
        TerminalSelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1 => 1,
    });
    encode_structural_layout(bytes, function.abi.layout);
    match &function.call {
        None => bytes.push(0),
        Some(call) => {
            bytes.push(1);
            bytes.extend_from_slice(&call.id.0.to_le_bytes());
            encode_structural_layout(bytes, call.layout);
            encode_constraint_key(bytes, call.constraint);
            encode_u16s(bytes, call.implicit_uses.iter().map(|unit| unit.0));
            encode_u16s(bytes, call.implicit_defs.iter().map(|unit| unit.0));
            encode_u16s(bytes, call.clobbers.iter().map(|unit| unit.0));
            encode_selected_provenance(bytes, &call.provenance);
        }
    }
    encode_instruction(bytes, &function.terminator.instruction);
}

fn encode_structural_layout(
    bytes: &mut Vec<u8>,
    layout: TerminalSelectedMicrosoftX64OwnedIndirectPairLayout,
) {
    bytes.extend_from_slice(&layout.shadow_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.outgoing_frame_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.pre_call_stack_alignment.to_le_bytes());
    for binding in layout.bindings {
        bytes.extend_from_slice(&(binding.parameter_index as u64).to_le_bytes());
        encode_machine_register(bytes, binding.pointer);
        bytes.extend_from_slice(&binding.copy_stack_byte_offset.to_le_bytes());
        bytes.extend_from_slice(&binding.byte_count.to_le_bytes());
        bytes.extend_from_slice(&binding.alignment.to_le_bytes());
    }
}

fn encode_selected_provenance(
    bytes: &mut Vec<u8>,
    provenance: &TerminalSelectedInstructionProvenance,
) {
    encode_ids(
        bytes,
        provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(bytes, provenance.values.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.edges.iter().map(|edge| edge.get()));
    encode_ids(
        bytes,
        provenance
            .obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    encode_fuel(bytes, &provenance.fuel);
}

fn encode_machine_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    let (tag, payload) = match register {
        MachineRegister::X86Rax => (0, 0),
        MachineRegister::X86Rcx => (1, 0),
        MachineRegister::X86Rdx => (2, 0),
        MachineRegister::X86Rbx => (3, 0),
        MachineRegister::X86Rsp => (4, 0),
        MachineRegister::X86Rbp => (5, 0),
        MachineRegister::X86Rsi => (6, 0),
        MachineRegister::X86Rdi => (7, 0),
        MachineRegister::X86R8 => (8, 0),
        MachineRegister::X86R9 => (9, 0),
        MachineRegister::X86R10 => (10, 0),
        MachineRegister::X86R11 => (11, 0),
        MachineRegister::X86R12 => (12, 0),
        MachineRegister::X86R13 => (13, 0),
        MachineRegister::X86R14 => (14, 0),
        MachineRegister::X86R15 => (15, 0),
        MachineRegister::X86Xmm(index) => (16, index),
        MachineRegister::Aarch64X(index) => (17, index),
        MachineRegister::Aarch64V(index) => (18, index),
    };
    bytes.push(tag);
    bytes.push(payload);
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
        TerminalSelectedInstructionKind::CopyI64 => 4,
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => 5,
        TerminalSelectedInstructionKind::ExactAddI64Immediate { .. } => 6,
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => 7,
        TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        TerminalSelectedInstructionKind::ReturnUnit => 9,
    });
    match instruction.kind {
        TerminalSelectedInstructionKind::MaterializeI64 { value } => match value {
            psi_core::IntegerValue::Signed(value) => {
                bytes.push(0);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            psi_core::IntegerValue::Unsigned(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        },
        TerminalSelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        }
        | TerminalSelectedInstructionKind::ExactSubtractI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        } => {
            match immediate {
                psi_core::IntegerValue::Signed(value) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                psi_core::IntegerValue::Unsigned(value) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        TerminalSelectedInstructionKind::CompareI64Zero
        | TerminalSelectedInstructionKind::CopyI64
        | TerminalSelectedInstructionKind::ConditionalBranchNonZero
        | TerminalSelectedInstructionKind::ReturnI64
        | TerminalSelectedInstructionKind::ReturnUnit => {}
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

#[cfg(test)]
mod structural_unit_tests {
    use super::*;
    use omega_register_model::validate_physical_register_model;
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
        TerminalAbstractOperation,
    };
    use omega_terminal_isa_x86_64::{
        X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO,
        X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_MATERIALIZE_I64,
        X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR, X86_64_MICROSOFT_RETURN,
        X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE,
        validate_x86_64_register_constraint_catalog, x86_64_physical_register_model,
        x86_64_register_constraint_catalog,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId, ScalarType,
        StructuralFieldId, StructuralTypeId,
    };
    use psi_terminal::{
        BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralArgument,
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalPsiIdentity, VocabularyMarker,
    };

    fn structural_call_fixture() -> (
        TerminalAbstractOperationPlan,
        TerminalTargetOperationPlan,
        PsiOptimizationUnit,
    ) {
        let caller = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let caller_block = BlockId::new(1).unwrap();
        let callee_block = BlockId::new(2).unwrap();
        let caller_places = [PlaceId::new(1).unwrap(), PlaceId::new(2).unwrap()];
        let callee_places = [PlaceId::new(3).unwrap(), PlaceId::new(4).unwrap()];
        let structural_type = StructuralTypeId::new(1).unwrap();
        let call = OperationId::new(1).unwrap();
        let caller_return = EdgeId::new(1).unwrap();
        let callee_return = EdgeId::new(2).unwrap();
        let parameter = |place, position| StructuralParameterDeclaration {
            place,
            position,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: vec![psi_core::StructuralDomainId::new(1).unwrap()],
        };
        let abstract_plan = TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x51; 32]),
            },
            entry: caller,
            structural_types: vec![StructuralTypeDeclaration {
                id: structural_type,
                identity: "Extent".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "base".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                psi_core::IntegerType::address(64).unwrap(),
                            )),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "length".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                psi_core::IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            )),
                        },
                    ],
                },
            }],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                TerminalAbstractFunction {
                    machine: caller,
                    attachment: None,
                    entry: caller_block,
                    parameters: Vec::new(),
                    structural_parameters: caller_places
                        .into_iter()
                        .enumerate()
                        .map(|(position, place)| parameter(place, position as u32))
                        .collect(),
                    result: TerminalAbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block: caller_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::CallUnit {
                            psi_operation: call,
                            callee,
                            structural_arguments: caller_places
                                .into_iter()
                                .map(|place| StructuralArgument {
                                    place,
                                    access: StructuralAccess::Owned,
                                    path: Vec::new(),
                                })
                                .collect(),
                            claim_transfers: Vec::new(),
                        },
                        TerminalAbstractOperation::ReturnUnit {
                            psi_edge: caller_return,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                },
                TerminalAbstractFunction {
                    machine: callee,
                    attachment: None,
                    entry: callee_block,
                    parameters: Vec::new(),
                    structural_parameters: callee_places
                        .into_iter()
                        .enumerate()
                        .map(|(position, place)| parameter(place, position as u32))
                        .collect(),
                    result: TerminalAbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block: callee_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![TerminalAbstractOperation::ReturnUnit {
                        psi_edge: callee_return,
                        cleanup_actions: Vec::new(),
                    }],
                },
            ],
        };
        let target =
            omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                omega_target::NativeTarget::uefi_x64(),
            )
            .unwrap();
        let unit = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        (abstract_plan, target, unit)
    }

    fn microsoft_selection_environment() -> (
        ValidatedPhysicalRegisterModel,
        ValidatedRegisterConstraintCatalog,
        TerminalSelectedSelectionConstraints,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        let constraints = TerminalSelectedSelectionConstraints {
            keys: TerminalSelectedConstraintKeys {
                structural_unit_call: Some(X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR),
                materialize_i64: X86_64_MATERIALIZE_I64,
                copy_i64: X86_64_COPY_I64,
                add_i64: X86_64_ADD_I64,
                subtract_i64: X86_64_SUBTRACT_I64,
                add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
                subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
                compare_i64_zero: X86_64_COMPARE_I64_ZERO,
                conditional_branch: X86_64_CONDITIONAL_BRANCH,
                return_i64: X86_64_MICROSOFT_RETURN,
                return_unit: X86_64_MICROSOFT_RETURN_UNIT,
            },
            fixed_inputs: Vec::new(),
        };
        (physical, catalog, constraints)
    }

    #[test]
    fn structural_call_and_terminal_callee_are_produced_and_replayed() {
        let (abstract_plan, target, unit) = structural_call_fixture();
        let legalized = legalize_terminal_target_operations(&target, &abstract_plan, &unit)
            .expect("one whole-root call and its structural callee legalize");
        assert!(legalized.plan().unit_functions.is_empty());
        assert_eq!(legalized.plan().structural_unit_functions.len(), 2);
        assert!(legalized.plan().structural_unit_functions[0].call.is_some());
        assert!(legalized.plan().structural_unit_functions[1].call.is_none());
        assert_eq!(legalized.receipt().function_count(), 2);

        let (physical, catalog, constraints) = microsoft_selection_environment();
        let selected = select_terminal_instructions(&legalized, &constraints, &physical, &catalog)
            .expect("bounded Microsoft structural Unit calls select atomically");
        assert!(selected.plan().functions.is_empty());
        assert_eq!(selected.plan().structural_unit_functions.len(), 2);
        let caller = &selected.plan().structural_unit_functions[0];
        let call = caller.call.as_ref().unwrap();
        assert_eq!(call.id, TerminalSelectedInstructionId(0));
        assert_eq!(
            call.constraint,
            X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR
        );
        assert!(call.arguments.len() == 2 && !call.implicit_uses.is_empty());
        assert_eq!(
            caller.terminator.instruction.id,
            TerminalSelectedInstructionId(1)
        );
        assert!(caller.terminator.instruction.operands.is_empty());
        assert!(selected.plan().structural_unit_functions[1].call.is_none());
        assert_eq!(selected.receipt().function_count(), 2);
        assert_eq!(selected.receipt().block_count(), 2);
        assert_eq!(selected.receipt().virtual_register_count(), 0);
        assert_eq!(selected.receipt().instruction_count(), 3);
    }

    #[test]
    fn selected_structural_replay_rejects_abi_constraint_and_semantic_custody_mutations() {
        let (abstract_plan, target, unit) = structural_call_fixture();
        let legalized =
            legalize_terminal_target_operations(&target, &abstract_plan, &unit).unwrap();
        let (physical, catalog, constraints) = microsoft_selection_environment();
        let selected =
            select_terminal_instructions(&legalized, &constraints, &physical, &catalog).unwrap();
        let selected_identity = selected.receipt().identity();

        let mut corrupted = selected.plan().clone();
        corrupted.structural_unit_functions[0]
            .abi
            .layout
            .outgoing_frame_byte_count -= 8;
        assert_ne!(
            terminal_selected_instruction_plan_identity(&corrupted),
            selected_identity
        );
        assert!(
            validate_terminal_selected_instructions(
                &legalized,
                &constraints,
                &physical,
                &catalog,
                corrupted
            )
            .is_err()
        );

        let mut corrupted = selected.plan().clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .implicit_uses
            .pop();
        assert_ne!(
            terminal_selected_instruction_plan_identity(&corrupted),
            selected_identity
        );
        assert!(
            validate_terminal_selected_instructions(
                &legalized,
                &constraints,
                &physical,
                &catalog,
                corrupted
            )
            .is_err()
        );

        let mut corrupted = selected.plan().clone();
        corrupted.structural_unit_functions[0].abi.parameters[0]
            .semantic
            .qualifications[0] = psi_core::StructuralDomainId::new(2).unwrap();
        assert_ne!(
            terminal_selected_instruction_plan_identity(&corrupted),
            selected_identity
        );
        assert!(
            validate_terminal_selected_instructions(
                &legalized,
                &constraints,
                &physical,
                &catalog,
                corrupted
            )
            .is_err()
        );

        let mut corrupted = selected.plan().clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .effect
            .output += 1;
        assert_ne!(
            terminal_selected_instruction_plan_identity(&corrupted),
            selected_identity
        );
        assert!(
            validate_terminal_selected_instructions(
                &legalized,
                &constraints,
                &physical,
                &catalog,
                corrupted
            )
            .is_err()
        );

        let mut missing_key = constraints.clone();
        missing_key.keys.structural_unit_call = None;
        assert!(
            select_terminal_instructions(&legalized, &missing_key, &physical, &catalog).is_err()
        );

        let linux_target =
            omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                omega_target::NativeTarget::linux_x64(),
            )
            .unwrap();
        let linux_legalized =
            legalize_terminal_target_operations(&linux_target, &abstract_plan, &unit).unwrap();
        assert!(
            select_terminal_instructions(&linux_legalized, &constraints, &physical, &catalog)
                .is_err()
        );

        let mut wrong_shape = abstract_plan.clone();
        let StructuralTypeShape::Record { fields } = &mut wrong_shape.structural_types[0].shape
        else {
            unreachable!()
        };
        fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Integer(
            psi_core::IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        ));
        let wrong_target =
            omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations(
                &wrong_shape,
                omega_target::NativeTarget::uefi_x64(),
            )
            .unwrap();
        let wrong_unit = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &wrong_shape,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let wrong_legalized =
            legalize_terminal_target_operations(&wrong_target, &wrong_shape, &wrong_unit).unwrap();
        assert!(
            select_terminal_instructions(&wrong_legalized, &constraints, &physical, &catalog)
                .is_err()
        );
    }

    #[test]
    fn independent_replay_rejects_placement_effect_and_roster_erasure() {
        let (abstract_plan, target, unit) = structural_call_fixture();
        let legalized =
            legalize_terminal_target_operations(&target, &abstract_plan, &unit).unwrap();

        let mut corrupted = legalized.plan().clone();
        corrupted.structural_unit_functions[0]
            .call_plan
            .shadow_bytes += 8;
        assert!(
            validate_terminal_legalized_operations(&target, &abstract_plan, &unit, corrupted,)
                .is_err()
        );

        let mut corrupted_target = target.clone();
        let omega_terminal_target_operations::TerminalTargetOperation::UnitBody(callee) =
            &mut corrupted_target.functions[1].operation
        else {
            panic!("fixture callee is Unit")
        };
        callee.call_plan.shadow_bytes += 8;
        assert!(
            legalize_terminal_target_operations(&corrupted_target, &abstract_plan, &unit).is_err()
        );

        let mut corrupted = legalized.plan().clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .arguments[0]
            .target
            .source_byte_offset = 1;
        assert!(
            validate_terminal_legalized_operations(&target, &abstract_plan, &unit, corrupted,)
                .is_err()
        );

        let mut corrupted = legalized.plan().clone();
        corrupted.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .effect
            .output += 1;
        assert!(
            validate_terminal_legalized_operations(&target, &abstract_plan, &unit, corrupted,)
                .is_err()
        );

        let mut erased = legalized.plan().clone();
        erased.structural_unit_functions.clear();
        assert!(
            validate_terminal_legalized_operations(&target, &abstract_plan, &unit, erased,)
                .is_err()
        );
    }
}
