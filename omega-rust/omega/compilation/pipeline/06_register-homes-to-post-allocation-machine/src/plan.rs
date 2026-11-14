/*
We construct the baseline `PostAllocationMachinePlan` from validated register
homes and pre-allocation machine effects, and we seal the resulting plan with
its structural receipt (`:PostAllocationCustody`, `:UniqueApplicableChoice`).

Before inspecting functions or instructions, `validate_roots` checks that the
selected program, machine effects, live ranges, legality evidence, register
homes, post-allocation manifest, and target register environment all agree on
their upstream digests and target architecture. For each function, we index
virtual-register home assignments with a stable sort (`PreparedHomes`) so
first-match lookup is logarithmic without altering duplicate-home behavior.

Each instruction's virtual operands are resolved to their assigned physical
register views, checked against register-class and fixed-view constraints, and
expanded into concrete storage, read, and write register units. We then select
the unique baseline machine alternative (`choose_alternative`) whose operand
aliasing predicate holds (`:UniqueApplicableChoice`). Alternatives tagged
`OptimizationSelected` are never chosen here; they are reserved for the
post-allocation optimization phase in `optimization.rs`.
*/

use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::PostAllocationOptimizationManifestIdentity;
use physical_instructions::{
    MachineAlternativeChoiceRule, PhysicalAddressOperation as Address, PhysicalOperandFootprint,
    PostAllocationMachineBlock, PostAllocationMachineFunction, PostAllocationMachineIdentity,
    PostAllocationMachineInstruction, PostAllocationMachinePlan, post_allocation_machine_identity,
};
use register_homes::{
    AllocatedProgramFacts, FunctionRegisterHomes, RegisterHomeIdentity, RegisterHomePlan,
    VirtualRegisterHome,
};
use register_model::{
    PreparedRegisterConstraintCatalog, RegisterOperandAccess, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel,
};
use selected_instructions::{
    BlockMachineEffects, InstructionMachineEffects, MachineAlternative,
    MachineAlternativeApplicability, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, SelectedBlock, SelectedInstruction,
    SelectedInstructionKind as Kind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    VirtualRegisterId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostAllocationMachineReceipt {
    identity: PostAllocationMachineIdentity,
    selected: SelectedInstructionPlanIdentity,
    effects: PreAllocationMachineEffectIdentity,
    homes: RegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    function_count: usize,
    block_count: usize,
    instruction_count: usize,
    operand_count: usize,
    unit_action_count: usize,
}

impl PostAllocationMachineReceipt {
    copied_accessors! {
        identity: PostAllocationMachineIdentity,
        selected: SelectedInstructionPlanIdentity,
        effects: PreAllocationMachineEffectIdentity,
        homes: RegisterHomeIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        register_environment: TargetRegisterEnvironmentIdentity,
        function_count: usize,
        block_count: usize,
        instruction_count: usize,
        operand_count: usize,
        unit_action_count: usize,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPostAllocationMachinePlan {
    plan: Arc<PostAllocationMachinePlan>,
    receipt: PostAllocationMachineReceipt,
}

impl ValidatedPostAllocationMachinePlan {
    pub fn plan(&self) -> &PostAllocationMachinePlan {
        &self.plan
    }

    pub fn shared_plan(&self) -> Arc<PostAllocationMachinePlan> {
        Arc::clone(&self.plan)
    }

    pub const fn receipt(&self) -> PostAllocationMachineReceipt {
        self.receipt
    }

    pub(crate) fn new(
        plan: PostAllocationMachinePlan,
        receipt: PostAllocationMachineReceipt,
    ) -> Self {
        Self {
            plan: Arc::new(plan),
            receipt,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostAllocationMachineError {
    TargetMismatch,
    SelectedRootMismatch,
    EffectRootMismatch,
    RangeRootMismatch,
    LegalityRootMismatch,
    HomeRootMismatch,
    RegisterEnvironmentMismatch,
    PhysicalRegisterModelMismatch,
    RegisterConstraintCatalogMismatch,
    PostAllocationManifestMismatch,
    OptimizationUnitMismatch,
    WorkScheduleMismatch,
    FunctionMismatch {
        function: usize,
    },
    BlockMismatch {
        function: usize,
        block: usize,
    },
    InstructionMismatch {
        function: usize,
        instruction: u32,
    },
    MissingHome {
        function: usize,
        register: u32,
    },
    UnknownView {
        function: usize,
        register: u32,
        view: u16,
    },
    HomeClassMismatch {
        function: usize,
        register: u32,
    },
    MissingApplicabilityOperand {
        instruction: u32,
        operand: u16,
    },
    NoApplicableAlternative {
        instruction: u32,
    },
    AmbiguousApplicableAlternatives {
        instruction: u32,
    },
    IdentityMismatch,
    CountOverflow,
}

stage_error_display!(
    PostAllocationMachineError,
    "invalid post-allocation machine sidecar"
);

pub(crate) struct PlanSources<'a> {
    pub(crate) allocation: AllocatedProgramFacts<'a>,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: &'a ValidatedPhysicalRegisterModel,
    pub(crate) constraints: &'a PreparedRegisterConstraintCatalog,
}

pub(crate) fn seal_post_allocation_machine_plan(
    plan: PostAllocationMachinePlan,
) -> Result<ValidatedPostAllocationMachinePlan, PostAllocationMachineError> {
    let receipt = post_allocation_receipt(&plan)?;
    Ok(ValidatedPostAllocationMachinePlan::new(plan, receipt))
}

pub(crate) fn post_allocation_receipt(
    plan: &PostAllocationMachinePlan,
) -> Result<PostAllocationMachineReceipt, PostAllocationMachineError> {
    let block_count = plan.functions.iter().try_fold(0_usize, |count, function| {
        count
            .checked_add(function.blocks.len())
            .ok_or(PostAllocationMachineError::CountOverflow)
    })?;
    let (ordinary_instruction_count, operand_count, ordinary_unit_action_count) = plan
        .instructions()
        .try_fold((0_usize, 0_usize, 0_usize), |counts, instruction| {
            Ok::<_, PostAllocationMachineError>((
                counts
                    .0
                    .checked_add(1)
                    .ok_or(PostAllocationMachineError::CountOverflow)?,
                counts
                    .1
                    .checked_add(instruction.operands.len())
                    .ok_or(PostAllocationMachineError::CountOverflow)?,
                counts
                    .2
                    .checked_add(instruction.unit_uses.len())
                    .and_then(|count| count.checked_add(instruction.unit_defs.len()))
                    .and_then(|count| count.checked_add(instruction.unit_clobbers.len()))
                    .ok_or(PostAllocationMachineError::CountOverflow)?,
            ))
        })?;
    Ok(PostAllocationMachineReceipt {
        identity: plan.identity,
        selected: plan.selected,
        effects: plan.effects,
        homes: plan.homes,
        post_allocation_manifest: plan.post_allocation_manifest,
        register_environment: plan.register_environment,
        function_count: plan.functions.len(),
        block_count,
        instruction_count: ordinary_instruction_count,
        operand_count,
        unit_action_count: ordinary_unit_action_count,
    })
}

pub(crate) fn compute_post_allocation_machine_plan(
    sources: &PlanSources<'_>,
) -> Result<PostAllocationMachinePlan, PostAllocationMachineError> {
    validate_roots(sources)?;
    let allocation = sources.allocation;
    let selected_plan = allocation.selected.plan;
    let functions = build_functions(
        selected_plan,
        allocation.effects,
        allocation.homes,
        sources.physical,
    )?;
    let mut plan = PostAllocationMachinePlan {
        identity: PostAllocationMachineIdentity::from_bytes([0; 32]),
        selected: allocation.selected.identity,
        effects: allocation.effects.identity,
        ranges: allocation.ranges_identity,
        legality: allocation.legality_identity,
        homes: allocation.homes_identity,
        post_allocation_manifest: allocation.manifest.identity,
        target: selected_plan.target,
        register_environment: sources.register_environment,
        physical_register_model: sources.physical.identity(),
        register_constraints: allocation.effects.register_constraints,
        machine_effect_catalog: allocation.effects.machine_effect_catalog,
        choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        functions,
    };
    plan.identity = post_allocation_machine_identity(&plan);
    Ok(plan)
}

fn validate_roots(sources: &PlanSources<'_>) -> Result<(), PostAllocationMachineError> {
    let PlanSources {
        allocation,
        register_environment,
        physical,
        constraints,
    } = *sources;
    let AllocatedProgramFacts {
        selected,
        effects,
        ranges,
        ranges_identity,
        legality,
        legality_identity,
        homes,
        homes_identity,
        manifest,
    } = allocation;
    if effects.selected != selected.identity || ranges.selected != selected.identity {
        return Err(PostAllocationMachineError::SelectedRootMismatch);
    }
    if effects.optimization_unit != selected.optimization_unit
        || ranges.optimization_unit != selected.optimization_unit
    {
        return Err(PostAllocationMachineError::OptimizationUnitMismatch);
    }
    if effects.work_schedule != selected.work_schedule
        || ranges.work_schedule != selected.work_schedule
    {
        return Err(PostAllocationMachineError::WorkScheduleMismatch);
    }
    if legality.ranges != ranges_identity {
        return Err(PostAllocationMachineError::RangeRootMismatch);
    }
    if homes.ranges != ranges_identity || homes.legality != legality_identity {
        return Err(PostAllocationMachineError::HomeRootMismatch);
    }
    if effects.register_environment != register_environment
        || legality.register_environment != register_environment
        || homes.register_environment != register_environment
    {
        return Err(PostAllocationMachineError::RegisterEnvironmentMismatch);
    }
    if manifest.target != selected.plan.target
        || manifest.selected != selected.identity
        || manifest.ranges != ranges_identity
        || manifest.legality != legality_identity
        || manifest.homes != homes_identity
        || manifest.register_environment != register_environment
    {
        return Err(PostAllocationMachineError::PostAllocationManifestMismatch);
    }
    if effects.target != selected.plan.target {
        return Err(PostAllocationMachineError::TargetMismatch);
    }
    if physical.model().architecture != selected.plan.target.architecture {
        return Err(PostAllocationMachineError::PhysicalRegisterModelMismatch);
    }
    if constraints.physical_identity() != physical.identity()
        || constraints.identity() != effects.register_constraints
    {
        return Err(PostAllocationMachineError::RegisterConstraintCatalogMismatch);
    }
    Ok(())
}

struct PreparedHomes<'homes> {
    assignments: Vec<&'homes VirtualRegisterHome>,
}

impl<'homes> PreparedHomes<'homes> {
    fn new(homes: &'homes FunctionRegisterHomes) -> Self {
        let mut assignments = homes.assignments.iter().collect::<Vec<_>>();
        assignments.sort_by_key(|home| home.virtual_register);
        Self { assignments }
    }

    fn get(&self, register: VirtualRegisterId) -> Option<&'homes VirtualRegisterHome> {
        let index = self
            .assignments
            .partition_point(|home| home.virtual_register < register);
        self.assignments
            .get(index)
            .copied()
            .filter(|home| home.virtual_register == register)
    }
}

fn build_functions(
    selected: &SelectedInstructionPlan,
    effects: &PreAllocationMachineEffectPlan,
    homes: &RegisterHomePlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<PostAllocationMachineFunction>, PostAllocationMachineError> {
    selected
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            let effect_function = effects
                .functions
                .get(function_index)
                .filter(|effects| effects.machine == function.machine)
                .ok_or(PostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                })?;
            let home_function = homes
                .functions
                .get(function_index)
                .filter(|home| home.machine == function.machine)
                .or_else(|| homes.function(function.machine))
                .ok_or(PostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                })?;
            if effect_function.blocks.len() != function.blocks.len() {
                return Err(PostAllocationMachineError::FunctionMismatch {
                    function: function_index,
                });
            }
            let prepared_homes = PreparedHomes::new(home_function);
            let blocks = function
                .blocks
                .iter()
                .enumerate()
                .map(|(block_index, block)| {
                    build_block(
                        function_index,
                        block_index,
                        block,
                        &effect_function.blocks[block_index],
                        &prepared_homes,
                        physical,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PostAllocationMachineFunction {
                machine: function.machine,
                outgoing_arguments: function.outgoing_arguments.clone(),
                local_storage_slots: function.local_storage_slots.clone(),
                blocks,
            })
        })
        .collect()
}

fn build_block(
    function_index: usize,
    block_index: usize,
    selected: &SelectedBlock,
    effects: &BlockMachineEffects,
    homes: &PreparedHomes<'_>,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PostAllocationMachineBlock, PostAllocationMachineError> {
    if effects.block != selected.id {
        return Err(PostAllocationMachineError::BlockMismatch {
            function: function_index,
            block: block_index,
        });
    }
    let selected_instructions = selected_instructions(selected).collect::<Vec<_>>();
    if effects.instructions.len() != selected_instructions.len() {
        return Err(PostAllocationMachineError::BlockMismatch {
            function: function_index,
            block: block_index,
        });
    }
    let instructions = selected_instructions
        .into_iter()
        .zip(&effects.instructions)
        .map(|(selected, effects)| {
            build_instruction(function_index, selected, effects, homes, physical)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PostAllocationMachineBlock {
        block: selected.id,
        instructions,
    })
}

fn selected_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    let terminator = selected_instructions::terminator_instruction(&block.terminator);
    block.instructions.iter().chain(std::iter::once(terminator))
}

fn build_instruction(
    function_index: usize,
    selected: &SelectedInstruction,
    effects: &InstructionMachineEffects,
    homes: &PreparedHomes<'_>,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PostAllocationMachineInstruction, PostAllocationMachineError> {
    if effects.instruction != selected.id || effects.kind != selected.kind {
        return Err(PostAllocationMachineError::InstructionMismatch {
            function: function_index,
            instruction: selected.id.0,
        });
    }
    let operands = selected
        .operands
        .iter()
        .map(|operand| {
            let home = homes.get(operand.virtual_register).ok_or(
                PostAllocationMachineError::MissingHome {
                    function: function_index,
                    register: operand.virtual_register.0,
                },
            )?;
            let view = physical.model().view(home.view).ok_or(
                PostAllocationMachineError::UnknownView {
                    function: function_index,
                    register: operand.virtual_register.0,
                    view: home.view.0,
                },
            )?;
            if home.class != operand.class || view.class != operand.class {
                return Err(PostAllocationMachineError::HomeClassMismatch {
                    function: function_index,
                    register: operand.virtual_register.0,
                });
            }
            if operand.fixed_view.is_some_and(|fixed| fixed != home.view) {
                return Err(PostAllocationMachineError::HomeClassMismatch {
                    function: function_index,
                    register: operand.virtual_register.0,
                });
            }
            let reads = reads(operand.access);
            let writes = writes(operand.access);
            Ok(PhysicalOperandFootprint {
                operand: operand.operand,
                virtual_register: operand.virtual_register,
                class: operand.class,
                view: home.view,
                access: operand.access,
                storage_units: view.units.clone(),
                read_units: if reads {
                    view.units.clone()
                } else {
                    Vec::new()
                },
                write_units: if writes {
                    view.write_units.clone()
                } else {
                    Vec::new()
                },
                write_semantics: writes.then_some(view.write_semantics),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let alternative =
        choose_alternative(selected.id.0, &operands, &effects.alternatives, physical)?;
    let mut unit_uses = effects.unit_uses.iter().copied().collect::<BTreeSet<_>>();
    let mut unit_defs = effects.unit_defs.iter().copied().collect::<BTreeSet<_>>();
    for operand in &operands {
        unit_uses.extend(&operand.read_units);
        unit_defs.extend(&operand.write_units);
    }
    Ok(PostAllocationMachineInstruction {
        instruction: selected.id,
        alternative,
        operands,
        address: match selected.kind {
            Kind::Load64 { byte_offset } => Some(Address::Load64 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Load8 { byte_offset } => Some(Address::Load8 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Load16 { byte_offset } => Some(Address::Load16 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Load32 { byte_offset } => Some(Address::Load32 {
                base_operand: 0,
                byte_offset,
            }),
            Kind::LoadPacked { byte_offset, width } => Some(Address::LoadPacked {
                base_operand: 0,
                byte_offset,
                width,
            }),
            Kind::StorePacked { byte_offset, width } => Some(Address::StorePacked {
                base_operand: 0,
                byte_offset,
                width,
            }),
            Kind::Store {
                byte_offset,
                byte_size,
            } => Some(Address::Store {
                base_operand: 0,
                byte_offset,
                byte_size,
            }),
            Kind::AddressOffset { byte_offset } => Some(Address::AddressOffset {
                base_operand: 0,
                byte_offset,
            }),
            Kind::Store64 { slot, byte_offset } => Some(Address::Store64 { slot, byte_offset }),
            Kind::FrameAddress { slot, byte_offset } => {
                Some(Address::FrameAddress { slot, byte_offset })
            }
            Kind::Load8Indexed => Some(Address::Load8Indexed {
                base_operand: 0,
                index_operand: 1,
            }),
            Kind::AccessorScalarAccess {
                form,
                contract,
                byte_offset,
            } => Some(Address::AccessorScalarAccess {
                base_operand: 0,
                byte_offset,
                form,
                contract,
            }),
            Kind::HostedReadByte { slot } => Some(Address::HostedReadByte { slot }),
            Kind::HostedWriteByteI32 { slot } => Some(Address::HostedWriteByteI32 { slot }),
            Kind::SaveFloatingControl { slot } => Some(Address::SaveFloatingControl { slot }),
            Kind::RestoreFloatingControl { slot } => Some(Address::RestoreFloatingControl { slot }),
            _ => None,
        },
        implicit_unit_uses: effects.unit_uses.clone(),
        implicit_unit_defs: effects.unit_defs.clone(),
        implicit_unit_clobbers: effects.unit_clobbers.clone(),
        unit_uses: unit_uses.into_iter().collect(),
        unit_defs: unit_defs.into_iter().collect(),
        unit_clobbers: effects.unit_clobbers.clone(),
    })
}

const fn reads(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

const fn writes(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}

fn choose_alternative(
    instruction: u32,
    operands: &[PhysicalOperandFootprint],
    alternatives: &[MachineAlternative],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<MachineAlternative, PostAllocationMachineError> {
    let mut applicable = Vec::new();
    for alternative in alternatives {
        if is_applicable(instruction, operands, alternative.applicability, physical)? {
            applicable.push(alternative.clone());
        }
    }
    match applicable.as_slice() {
        [alternative] => Ok(alternative.clone()),
        [] => Err(PostAllocationMachineError::NoApplicableAlternative { instruction }),
        _ => Err(PostAllocationMachineError::AmbiguousApplicableAlternatives { instruction }),
    }
}

fn is_applicable(
    instruction: u32,
    operands: &[PhysicalOperandFootprint],
    applicability: MachineAlternativeApplicability,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<bool, PostAllocationMachineError> {
    let view = |operand| {
        operands
            .iter()
            .find(|candidate| candidate.operand == operand)
            .map(|operand| operand.view)
            .ok_or(PostAllocationMachineError::MissingApplicabilityOperand {
                instruction,
                operand,
            })
    };
    let aliases = |left, right| physical.model().aliases(left, right);
    Ok(match applicability {
        MachineAlternativeApplicability::Always => true,
        MachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
            aliases(view(result)?, view(operand)?)
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let result = view(result)?;
            aliases(result, view(aliased_operand)?) && !aliases(result, view(distinct_operand)?)
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            aliases(result, view(left)?) && aliases(result, view(right)?)
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            !aliases(result, view(left)?) && !aliases(result, view(right)?)
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left,
            right,
            excluded_view,
        } => !aliases(view(left)?, excluded_view) || !aliases(view(right)?, excluded_view),
        MachineAlternativeApplicability::OptimizationSelected { .. } => false,
    })
}

#[cfg(test)]
mod tests {
    use physical_instructions::PhysicalOperandFootprint;
    use register_model::{
        RegisterClassId, RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel,
        validate_physical_register_model,
    };
    use selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineLatencyKnowledge, MachineSizeKnowledge, VirtualRegisterId,
    };

    use super::choose_alternative;

    fn physical() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(isa_x86_64::x86_64_physical_register_model()).unwrap()
    }

    fn view(physical: &ValidatedPhysicalRegisterModel, name: &str) -> RegisterViewId {
        physical.model().view_named(name).unwrap().id
    }

    fn operand(number: u16, view: RegisterViewId) -> PhysicalOperandFootprint {
        PhysicalOperandFootprint {
            operand: number,
            virtual_register: VirtualRegisterId(u32::from(number)),
            class: RegisterClassId(0),
            view,
            access: if number == 2 {
                RegisterOperandAccess::Def
            } else {
                RegisterOperandAccess::Use
            },
            storage_units: Vec::new(),
            read_units: Vec::new(),
            write_units: Vec::new(),
            write_semantics: None,
        }
    }

    fn alternative(
        variant: u32,
        applicability: MachineAlternativeApplicability,
    ) -> MachineAlternative {
        MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::ExactSubtractI64,
                variant,
            },
            applicability,
            size: MachineSizeKnowledge::ExactBytes(3),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: selected_instructions::MachineEncodedEffects::fallthrough_v1(
                vec![0, 1],
                vec![2],
            ),
        }
    }

    fn subtract_alternatives() -> Vec<MachineAlternative> {
        vec![
            alternative(
                0,
                MachineAlternativeApplicability::ResultAliasesOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
            ),
            alternative(
                1,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 0,
                    distinct_operand: 1,
                },
            ),
            alternative(
                2,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 1,
                    distinct_operand: 0,
                },
            ),
            alternative(
                3,
                MachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
            ),
        ]
    }

    #[test]
    fn x86_subtract_home_partition_selects_each_unique_variant() {
        let physical = physical();
        let rax = view(&physical, "rax");
        let rbx = view(&physical, "rbx");
        let rcx = view(&physical, "rcx");
        let alternatives = subtract_alternatives();
        for (views, expected) in [
            ([rax, rax, rax], 0),
            ([rax, rbx, rax], 1),
            ([rbx, rax, rax], 2),
            ([rax, rbx, rcx], 3),
        ] {
            let operands = views
                .into_iter()
                .enumerate()
                .map(|(number, view)| operand(u16::try_from(number).unwrap(), view))
                .collect::<Vec<_>>();
            assert_eq!(
                choose_alternative(7, &operands, &alternatives, &physical)
                    .unwrap()
                    .key
                    .variant,
                expected
            );
        }
    }

    #[test]
    fn x86_lea_add_accepts_r12_as_a_rex_extended_sib_index() {
        let physical = physical();
        let rax = view(&physical, "rax");
        let r12 = view(&physical, "r12");
        let add = MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::ExactAddI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(5),
            },
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: selected_instructions::MachineEncodedEffects::fallthrough_v1(
                vec![0, 1],
                vec![2],
            ),
        };
        let operands = [operand(0, r12), operand(1, r12), operand(2, rax)];
        assert_eq!(
            choose_alternative(8, &operands, std::slice::from_ref(&add), &physical)
                .unwrap()
                .key,
            add.key
        );
    }

    #[test]
    fn the_legality_rule_never_chooses_an_optimizer_selected_form() {
        let physical = physical();
        let rax = view(&physical, "rax");
        let operands = [operand(2, rax)];
        let alternatives = [
            alternative(0, MachineAlternativeApplicability::Always),
            alternative(
                1,
                MachineAlternativeApplicability::OptimizationSelected {
                    optimization:
                        optimization_core::Optimization::X86SelectXorZeroI64MaterializationV1,
                },
            ),
        ];
        assert_eq!(
            choose_alternative(9, &operands, &alternatives, &physical)
                .unwrap()
                .key
                .variant,
            0
        );
        assert_eq!(
            choose_alternative(9, &operands, &alternatives[1..], &physical),
            Err(crate::PostAllocationMachineError::NoApplicableAlternative { instruction: 9 })
        );
    }
}
