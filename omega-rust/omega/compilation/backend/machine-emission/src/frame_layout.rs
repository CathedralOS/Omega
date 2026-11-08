/*
We stage and replay non-authoritative callee-save storage and target-owned ordinary stack-frame geometry for post-allocation machine plans.

First, we map each function's modified ABI-preserved register units through the target's preservation storage catalog into canonical abstract area-relative slots (`stage_non_authoritative_callee_save_storage`) and replay that mapping by keyed group reconstruction. Second, we join the post-allocation machine plan to that validated storage to assign exact stack-frame coordinates (`stage_target_frame_layout`), including outgoing ABI argument and shadow areas, over-aligned local and value-copy reservations inside fixed ABI frames, System V leaf red-zone residency for allocator spill storage, stack-commit probe granules, and target-owned continuation unwind custody. Every produced frame layout is verified by independent replay (`validate_target_frame_layout`) before sealing its receipt.
*/

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::count_where;
use crate::digest_encoder::DomainDigestEncoder;
pub use machine_code::{
    CalleeSaveFrameSlot, FrameUnwindPlan, FrameUnwindRestore,
    FunctionNonAuthoritativeCalleeSaveStorage, FunctionTargetFrameLayout,
    NonAuthoritativeCalleeSaveSlot, NonAuthoritativeCalleeSaveSlotId,
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy, ReturnAddressFrameCustody, StackProbePlan,
    TargetFrameLayoutIdentity, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
    target_frame_layout_identity,
};
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use physical_instructions::PostAllocationMachineIdentity;
pub use register_environment::FrameAbiPreservationConvention;
use register_environment::{
    ValidatedTargetRegisterEnvironment, selected_abi_preservation,
    selected_preservation_storage_catalog,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use register_model::{
    PhysicalRegisterModelIdentity, PreservationStorageCatalogIdentity, PreservationStorageGroupId,
    RegisterUnitId, TargetRegisterEnvironmentIdentity, ValidatedPreservationStorageCatalog,
};
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, MachineAlternativeFamily};
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedUnitRequirement, FunctionAllocatedCalleeSavedRequirements,
    ValidatedAllocatedCalleeSavedRequirements, encode_callee_saved_modification_witness_identity,
};
use target::{Architecture, NativeTarget, ObjectFormat};

pub(crate) const MAX_FRAME_BYTES: u64 = i32::MAX as u64;

pub(crate) mod stack_commit {
    pub(crate) use super::stack_commit_granule_bytes;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFrameLayoutError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTarget,
    FunctionRosterMismatch,
    MissingStackPointerView,
    MissingLinkRegisterView,
    GeometryOverflow,
    NonCanonicalLayout,
    FrameExceedsAddressableExtent {
        frame_size_bytes: u64, // Frame exceeds MAX_FRAME_BYTES addressable by nonnegative i32 displacements.
    },
}

diagnostics::failed_display!(TargetFrameLayoutError, "target frame layout failed");

impl std::error::Error for TargetFrameLayoutError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonAuthoritativeCalleeSaveStorageError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTargetCatalog,
    UnknownPreservedUnit(RegisterUnitId),
    NonCanonicalStorage,
    UsageMismatch,
    StorageGeometryOverflow,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

diagnostics::failed_display!(
    NonAuthoritativeCalleeSaveStorageError,
    "non-authoritative callee-save storage planning failed"
);

impl std::error::Error for NonAuthoritativeCalleeSaveStorageError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonAuthoritativeCalleeSaveStorageReceipt {
    identity: NonAuthoritativeCalleeSaveStorageIdentity,
    callee_saved_requirements: AllocatedCalleeSavedRequirementIdentity,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical_register_model: PhysicalRegisterModelIdentity,
    preservation_storage_catalog: PreservationStorageCatalogIdentity,
    target: NativeTarget,
    abi: FrameAbiPreservationConvention,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    usage: OptimizationWorkUsage,
    function_count: usize,
    modified_function_count: usize,
    slot_count: usize,
    modified_unit_count: usize,
    witness_count: usize,
    max_abstract_area_bytes: u64,
    max_abstract_area_alignment: u64,
}

impl NonAuthoritativeCalleeSaveStorageReceipt {
    pub const fn identity(self) -> NonAuthoritativeCalleeSaveStorageIdentity {
        self.identity
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNonAuthoritativeCalleeSaveStorage {
    plan: NonAuthoritativeCalleeSaveStoragePlan,
    receipt: NonAuthoritativeCalleeSaveStorageReceipt,
}

impl ValidatedNonAuthoritativeCalleeSaveStorage {
    pub const fn plan(&self) -> &NonAuthoritativeCalleeSaveStoragePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> NonAuthoritativeCalleeSaveStorageReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFrameLayoutReceipt {
    identity: TargetFrameLayoutIdentity,
    post_allocation_machine: PostAllocationMachineIdentity,
    callee_saved_requirements: AllocatedCalleeSavedRequirementIdentity,
    callee_save_storage: NonAuthoritativeCalleeSaveStorageIdentity,
    target: NativeTarget,
    abi: FrameAbiPreservationConvention,
    policy: TargetFrameLayoutPolicy,
    function_count: usize,
    framed_function_count: usize,
    calling_function_count: usize,
    callee_save_slot_count: usize,
    saved_link_count: usize,
    probed_function_count: usize,
    max_frame_size_bytes: u64,
}

impl TargetFrameLayoutReceipt {
    pub const fn identity(self) -> TargetFrameLayoutIdentity {
        self.identity
    }
    pub const fn post_allocation_machine(self) -> PostAllocationMachineIdentity {
        self.post_allocation_machine
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTargetFrameLayout {
    plan: Arc<TargetFrameLayoutPlan>,
    receipt: TargetFrameLayoutReceipt,
}

impl ValidatedTargetFrameLayout {
    pub fn plan(&self) -> &TargetFrameLayoutPlan {
        &self.plan
    }
    pub fn shared_plan(&self) -> Arc<TargetFrameLayoutPlan> {
        Arc::clone(&self.plan)
    }
    pub const fn receipt(&self) -> TargetFrameLayoutReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameContinuationCustody {
    CallerActivationStack {
        return_address_size_bytes: u16,
    },
    LinkRegister {
        view: &'static str,
        saved_size_bytes: u16,
    },
}

impl FrameContinuationCustody {
    pub(crate) fn admits(self, custody: ReturnAddressFrameCustody) -> bool {
        matches!(
            (self, custody),
            (
                FrameContinuationCustody::CallerActivationStack { .. },
                ReturnAddressFrameCustody::CallerActivationStack { .. },
            ) | (
                FrameContinuationCustody::LinkRegister { .. },
                ReturnAddressFrameCustody::LiveLinkRegister { .. }
                    | ReturnAddressFrameCustody::SavedLinkRegister { .. },
            )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameUnwindPolicy {
    pub abi: FrameAbiPreservationConvention,
    pub stack_pointer_view: &'static str,
    pub continuation: FrameContinuationCustody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallSiteStackContract {
    stack_alignment_bytes: u16,
    entry_residue_bytes: u64,
}

pub(crate) const fn stack_commit_granule_bytes(target: NativeTarget) -> Option<u64> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf)
        | (Architecture::X86_64, ObjectFormat::Coff)
        | (Architecture::X86_64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Elf) => Some(4_096),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(16_384),
        (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

pub(crate) const fn frame_unwind_policy(target: NativeTarget) -> Option<FrameUnwindPolicy> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) | (Architecture::X86_64, ObjectFormat::MachO) => {
            Some(FrameUnwindPolicy {
                abi: FrameAbiPreservationConvention::SystemVAMD64,
                stack_pointer_view: "rsp",
                continuation: FrameContinuationCustody::CallerActivationStack {
                    return_address_size_bytes: 8,
                },
            })
        }
        (Architecture::X86_64, ObjectFormat::Coff) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::MicrosoftX64,
            stack_pointer_view: "rsp",
            continuation: FrameContinuationCustody::CallerActivationStack {
                return_address_size_bytes: 8,
            },
        }),
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::Aapcs64,
            stack_pointer_view: "sp",
            continuation: FrameContinuationCustody::LinkRegister {
                view: "x30",
                saved_size_bytes: 8,
            },
        }),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::DarwinAapcs64,
            stack_pointer_view: "sp",
            continuation: FrameContinuationCustody::LinkRegister {
                view: "x30",
                saved_size_bytes: 8,
            },
        }),
        (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

fn call_site_stack_contract(
    environment: &ValidatedTargetRegisterEnvironment,
    abi: FrameAbiPreservationConvention,
) -> Option<CallSiteStackContract> {
    let stack_alignment_bytes = selected_abi_preservation(environment)
        .ok()?
        .convention
        .stack_alignment;
    let entry_residue_bytes = match (environment.target().architecture, abi) {
        (
            Architecture::X86_64,
            FrameAbiPreservationConvention::SystemVAMD64
            | FrameAbiPreservationConvention::MicrosoftX64,
        ) => 8,
        (
            Architecture::Aarch64,
            FrameAbiPreservationConvention::Aapcs64 | FrameAbiPreservationConvention::DarwinAapcs64,
        ) => 0,
        _ => return None,
    };
    Some(CallSiteStackContract {
        stack_alignment_bytes,
        entry_residue_bytes,
    })
}

fn body_writes_link_register(
    environment: &ValidatedTargetRegisterEnvironment,
    function: &physical_instructions::PostAllocationMachineFunction,
) -> Result<bool, TargetFrameLayoutError> {
    let policy = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameLayoutError::UnsupportedTarget)?;
    let FrameContinuationCustody::LinkRegister { view, .. } = policy.continuation else {
        return Ok(false);
    };
    let link = environment
        .physical()
        .model()
        .view_named(view)
        .ok_or(TargetFrameLayoutError::MissingLinkRegisterView)?;
    Ok(function
        .instructions()
        .flat_map(|instruction| {
            instruction
                .unit_defs
                .iter()
                .chain(&instruction.unit_clobbers)
        })
        .any(|unit| link.units.contains(unit)))
}

fn contains_call(function: &physical_instructions::PostAllocationMachineFunction) -> bool {
    function.instructions().any(|instruction| {
        matches!(
            instruction.alternative.key.family,
            MachineAlternativeFamily::CallScalar
                | MachineAlternativeFamily::InvokeNoNormal
                | MachineAlternativeFamily::CallUnit
                | MachineAlternativeFamily::CallAggregate
                | MachineAlternativeFamily::NormalizedForeignCall
        )
    })
}

fn stable_address_loans(
    function: &physical_instructions::PostAllocationMachineFunction,
) -> Vec<LocalStorageSlotId> {
    function
        .instructions()
        .filter_map(|instruction| match instruction.address {
            Some(physical_instructions::PhysicalAddressOperation::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                ..
            }) if !matches!(slot, LocalStorageSlotId::Spill { .. }) => Some(slot),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn unwind_restores(
    return_address: ReturnAddressFrameCustody,
    callee_save_slots: &[CalleeSaveFrameSlot],
) -> Vec<FrameUnwindRestore> {
    let mut restores = Vec::with_capacity(callee_save_slots.len() + 1);
    if let ReturnAddressFrameCustody::SavedLinkRegister {
        view,
        frame_offset_bytes,
        size_bytes,
    } = return_address
    {
        restores.push(FrameUnwindRestore {
            view,
            frame_offset_bytes,
            size_bytes: u64::from(size_bytes),
        });
    }
    restores.extend(
        callee_save_slots
            .iter()
            .rev()
            .map(|slot| FrameUnwindRestore {
                view: slot.storage_view,
                frame_offset_bytes: slot.frame_offset_bytes,
                size_bytes: slot.size_bytes,
            }),
    );
    restores
}

pub(crate) fn committed_extent(row: &FunctionTargetFrameLayout) -> Option<u64> {
    let resident = row.red_zone_resident_bytes;
    let committed = row.frame_size_bytes.checked_sub(resident)?;
    if resident == 0 {
        return Some(committed);
    }
    (resident == row.frame_size_bytes
        && !row.contains_call
        && row.callee_save_slots.is_empty()
        && row
            .local_storage_slots
            .iter()
            .all(|slot| matches!(slot.id, LocalStorageSlotId::Spill { .. })))
    .then_some(committed)
}

pub(crate) fn stage_non_authoritative_callee_save_storage(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let plan = admit_storage_plan(
        source,
        environment,
        policy,
        budget,
        derive_storage_functions,
    )?;
    validate_non_authoritative_callee_save_storage(source, environment, plan)
}

pub(crate) fn validate_non_authoritative_callee_save_storage(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: NonAuthoritativeCalleeSaveStoragePlan,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let source_plan = source.plan();
    let catalog = selected_preservation_storage_catalog(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    if candidate.callee_saved_requirements != source.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.preservation_storage_catalog != catalog.identity()
        || candidate.target != environment.target()
        || candidate.abi != source_plan.abi
        || candidate.callee_saved_units != source_plan.callee_saved_units
        || source_plan.register_environment != environment.identity()
        || source_plan.physical_register_model != environment.physical().identity()
        || source_plan.target != environment.target()
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let replayed = admit_storage_plan(
        source,
        environment,
        candidate.policy,
        candidate.budget,
        reconstruct_storage_functions,
    )?;
    if candidate.usage != replayed.usage {
        return Err(NonAuthoritativeCalleeSaveStorageError::UsageMismatch);
    }
    if candidate.functions != replayed.functions {
        return Err(NonAuthoritativeCalleeSaveStorageError::NonCanonicalStorage);
    }
    let receipt = seal_storage(&candidate);
    Ok(ValidatedNonAuthoritativeCalleeSaveStorage {
        plan: candidate,
        receipt,
    })
}

fn admit_storage_plan(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
    functions: impl FnOnce(
        &AllocatedCalleeSavedRequirementPlan,
        &ValidatedPreservationStorageCatalog,
    ) -> Result<
        Vec<FunctionNonAuthoritativeCalleeSaveStorage>,
        NonAuthoritativeCalleeSaveStorageError,
    >,
) -> Result<NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStorageError> {
    if policy != NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 {
        return Err(NonAuthoritativeCalleeSaveStorageError::UnsupportedPolicy);
    }
    let source_plan = source.plan();
    if source_plan.register_environment != environment.identity()
        || source_plan.physical_register_model != environment.physical().identity()
        || source_plan.target != environment.target()
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let preservation = selected_abi_preservation(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    let catalog = selected_preservation_storage_catalog(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    if source_plan.abi != preservation.kind
        || source_plan.callee_saved_units != preservation.convention.callee_saved
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let functions = functions(source_plan, &catalog)?;
    let usage = storage_usage(source_plan, &catalog, &functions)?;
    let ceiling = budget.for_extent(storage_extent(source_plan)?);
    if !usage.within(ceiling) {
        return Err(NonAuthoritativeCalleeSaveStorageError::BudgetExceeded {
            required: usage,
            budget: ceiling,
        });
    }
    Ok(NonAuthoritativeCalleeSaveStoragePlan {
        callee_saved_requirements: source.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        preservation_storage_catalog: catalog.identity(),
        target: environment.target(),
        abi: source_plan.abi,
        callee_saved_units: source_plan.callee_saved_units.clone(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn derive_storage_functions(
    source: &AllocatedCalleeSavedRequirementPlan,
    catalog: &ValidatedPreservationStorageCatalog,
) -> Result<Vec<FunctionNonAuthoritativeCalleeSaveStorage>, NonAuthoritativeCalleeSaveStorageError>
{
    source
        .functions
        .iter()
        .map(|function| derive_storage_function(function, catalog))
        .collect()
}

fn derive_storage_function(
    source: &FunctionAllocatedCalleeSavedRequirements,
    catalog: &ValidatedPreservationStorageCatalog,
) -> Result<FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let mut slots = Vec::new();
    let mut extent = 0_u64;
    let mut max_alignment = 1_u64;
    for group in &catalog.catalog().groups {
        let modified_units = source
            .modified_units
            .iter()
            .filter(|requirement| group.preserved_units.contains(&requirement.unit))
            .cloned()
            .collect::<Vec<_>>();
        if modified_units.is_empty() {
            continue;
        }
        let alignment = group.alignment_bytes;
        let offset = align_up_storage(extent, alignment)?;
        extent = offset
            .checked_add(group.size_bytes)
            .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
        max_alignment = max_alignment.max(group.alignment_bytes);
        slots.push(NonAuthoritativeCalleeSaveSlot {
            id: NonAuthoritativeCalleeSaveSlotId(
                u16::try_from(slots.len())
                    .map_err(|_| NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?,
            ),
            storage_group: group.id,
            storage_view: group.storage_view,
            preserved_units: group.preserved_units.clone(),
            modified_units,
            abstract_offset_bytes: offset,
            size_bytes: group.size_bytes,
            alignment_bytes: group.alignment_bytes,
        });
    }
    if slots
        .iter()
        .map(|slot| slot.modified_units.len())
        .sum::<usize>()
        != source.modified_units.len()
    {
        let missing = source
            .modified_units
            .iter()
            .find(|requirement| {
                !slots
                    .iter()
                    .any(|slot| slot.modified_units.contains(requirement))
            })
            .map(|requirement| requirement.unit)
            .unwrap_or(RegisterUnitId(u16::MAX));
        return Err(NonAuthoritativeCalleeSaveStorageError::UnknownPreservedUnit(missing));
    }
    Ok(FunctionNonAuthoritativeCalleeSaveStorage {
        machine: source.machine,
        abstract_area_bytes: extent,
        abstract_area_alignment: max_alignment,
        slots,
    })
}

fn align_up_storage(
    value: u64,
    alignment: u64,
) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    let mask = alignment
        .checked_sub(1)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
    value
        .checked_add(mask)
        .map(|rounded| rounded & !mask)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)
}

fn reconstruct_storage_functions(
    source: &AllocatedCalleeSavedRequirementPlan,
    catalog: &ValidatedPreservationStorageCatalog,
) -> Result<Vec<FunctionNonAuthoritativeCalleeSaveStorage>, NonAuthoritativeCalleeSaveStorageError>
{
    let mut unit_groups = BTreeMap::<RegisterUnitId, PreservationStorageGroupId>::new();
    for group in &catalog.catalog().groups {
        for unit in &group.preserved_units {
            if unit_groups.insert(*unit, group.id).is_some() {
                return Err(NonAuthoritativeCalleeSaveStorageError::NonCanonicalStorage);
            }
        }
    }
    source
        .functions
        .iter()
        .map(|function| {
            let mut modified = BTreeMap::<
                PreservationStorageGroupId,
                Vec<AllocatedCalleeSavedUnitRequirement>,
            >::new();
            for requirement in &function.modified_units {
                let group = unit_groups.get(&requirement.unit).copied().ok_or(
                    NonAuthoritativeCalleeSaveStorageError::UnknownPreservedUnit(requirement.unit),
                )?;
                modified.entry(group).or_default().push(requirement.clone());
            }
            let mut slots = Vec::new();
            let mut extent = 0_u64;
            let mut max_alignment = 1_u64;
            for group in &catalog.catalog().groups {
                let Some(modified_units) = modified.remove(&group.id) else {
                    continue;
                };
                let alignment = group.alignment_bytes;
                let mask = alignment
                    .checked_sub(1)
                    .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
                let offset = extent
                    .checked_add(mask)
                    .map(|rounded| rounded & !mask)
                    .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
                extent = offset
                    .checked_add(group.size_bytes)
                    .ok_or(NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow)?;
                max_alignment = max_alignment.max(group.alignment_bytes);
                slots.push(NonAuthoritativeCalleeSaveSlot {
                    id: NonAuthoritativeCalleeSaveSlotId(u16::try_from(slots.len()).map_err(
                        |_| NonAuthoritativeCalleeSaveStorageError::StorageGeometryOverflow,
                    )?),
                    storage_group: group.id,
                    storage_view: group.storage_view,
                    preserved_units: group.preserved_units.clone(),
                    modified_units,
                    abstract_offset_bytes: offset,
                    size_bytes: group.size_bytes,
                    alignment_bytes: group.alignment_bytes,
                });
            }
            if !modified.is_empty() {
                return Err(NonAuthoritativeCalleeSaveStorageError::NonCanonicalStorage);
            }
            Ok(FunctionNonAuthoritativeCalleeSaveStorage {
                machine: function.machine,
                abstract_area_bytes: extent,
                abstract_area_alignment: max_alignment,
                slots,
            })
        })
        .collect()
}

fn storage_usage(
    source: &AllocatedCalleeSavedRequirementPlan,
    catalog: &ValidatedPreservationStorageCatalog,
    functions: &[FunctionNonAuthoritativeCalleeSaveStorage],
) -> Result<OptimizationWorkUsage, NonAuthoritativeCalleeSaveStorageError> {
    let f = count_storage(source.functions.len())?;
    let g = count_storage(catalog.catalog().groups.len())?;
    let u = catalog
        .catalog()
        .groups
        .iter()
        .try_fold(0_u64, |total, group| {
            add_storage(total, count_storage(group.preserved_units.len())?)
        })?;
    let m = source.functions.iter().try_fold(0_u64, |total, function| {
        add_storage(total, count_storage(function.modified_units.len())?)
    })?;
    let w = source
        .modified_units()
        .try_fold(0_u64, |total, requirement| {
            add_storage(total, count_storage(requirement.witnesses.len())?)
        })?;
    let s = functions.iter().try_fold(0_u64, |total, function| {
        add_storage(total, count_storage(function.slots.len())?)
    })?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: [1, f, m].into_iter().try_fold(0, add_storage)?,
        candidates: [g, m].into_iter().try_fold(0, add_storage)?,
        validation_steps: [1, g, u, f, m, w, s].into_iter().try_fold(0, add_storage)?,
        commits: [1, f, s, m, w].into_iter().try_fold(0, add_storage)?,
        iterations: [1, g, u, f, m, w].into_iter().try_fold(0, add_storage)?,
    })
}

fn count_storage(value: usize) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    u64::try_from(value).map_err(|_| NonAuthoritativeCalleeSaveStorageError::WorkOverflow)
}

fn add_storage(left: u64, right: u64) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    left.checked_add(right)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::WorkOverflow)
}

fn storage_extent(
    source: &AllocatedCalleeSavedRequirementPlan,
) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    source
        .functions
        .iter()
        .try_fold(count_storage(source.functions.len())?, |total, function| {
            function.modified_units.iter().try_fold(
                add_storage(total, count_storage(function.modified_units.len())?)?,
                |total, requirement| {
                    add_storage(total, count_storage(requirement.witnesses.len())?)
                },
            )
        })
}

pub(crate) fn non_authoritative_callee_save_storage_identity(
    plan: &NonAuthoritativeCalleeSaveStoragePlan,
) -> NonAuthoritativeCalleeSaveStorageIdentity {
    let mut hasher = DomainDigestEncoder::new(b"omega.non-authoritative-callee-save-storage.v2");
    hasher.raw(&plan.callee_saved_requirements.bytes());
    hasher.raw(&plan.register_environment.bytes());
    hasher.raw(&plan.physical_register_model.bytes());
    hasher.raw(&plan.preservation_storage_catalog.bytes());
    hasher.raw(&plan.target.identity_bytes());
    hasher.byte(plan.abi.abi_tag());
    register_model::sha256_register_units(&mut hasher, &plan.callee_saved_units);
    hasher.byte(match plan.policy {
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 => 0,
    });
    hasher.raw(&plan.budget.encode());
    hasher.raw(&plan.usage.encode());
    register_model::sha256_length(&mut hasher, plan.functions.len());
    for function in &plan.functions {
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.raw(&function.abstract_area_bytes.to_le_bytes());
        hasher.raw(&function.abstract_area_alignment.to_le_bytes());
        register_model::sha256_length(&mut hasher, function.slots.len());
        for slot in &function.slots {
            hasher.raw(&slot.id.0.to_le_bytes());
            hasher.raw(&slot.storage_group.0.to_le_bytes());
            hasher.raw(&slot.storage_view.0.to_le_bytes());
            register_model::sha256_register_units(&mut hasher, &slot.preserved_units);
            register_model::sha256_length(&mut hasher, slot.modified_units.len());
            for requirement in &slot.modified_units {
                hasher.raw(&requirement.unit.0.to_le_bytes());
                register_model::sha256_length(&mut hasher, requirement.witnesses.len());
                for witness in &requirement.witnesses {
                    encode_callee_saved_modification_witness_identity(&mut hasher, *witness);
                }
            }
            hasher.raw(&slot.abstract_offset_bytes.to_le_bytes());
            hasher.raw(&slot.size_bytes.to_le_bytes());
            hasher.raw(&slot.alignment_bytes.to_le_bytes());
        }
    }
    NonAuthoritativeCalleeSaveStorageIdentity::from_bytes(hasher.finish())
}

fn seal_storage(
    plan: &NonAuthoritativeCalleeSaveStoragePlan,
) -> NonAuthoritativeCalleeSaveStorageReceipt {
    NonAuthoritativeCalleeSaveStorageReceipt {
        identity: non_authoritative_callee_save_storage_identity(plan),
        callee_saved_requirements: plan.callee_saved_requirements,
        register_environment: plan.register_environment,
        physical_register_model: plan.physical_register_model,
        preservation_storage_catalog: plan.preservation_storage_catalog,
        target: plan.target,
        abi: plan.abi,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        modified_function_count: count_where(&plan.functions, |function| {
            !function.slots.is_empty()
        }),
        slot_count: plan
            .functions
            .iter()
            .map(|function| function.slots.len())
            .sum(),
        modified_unit_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.slots)
            .map(|slot| slot.modified_units.len())
            .sum(),
        witness_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.slots)
            .flat_map(|slot| &slot.modified_units)
            .map(|requirement| requirement.witnesses.len())
            .sum(),
        max_abstract_area_bytes: plan
            .functions
            .iter()
            .map(|function| function.abstract_area_bytes)
            .max()
            .unwrap_or(0),
        max_abstract_area_alignment: plan
            .functions
            .iter()
            .map(|function| function.abstract_area_alignment)
            .max()
            .unwrap_or(1),
    }
}

pub(crate) fn stage_target_frame_layout(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameLayoutPolicy,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    let plan = derive_frame_layout(machine, requirements, storage, environment, policy)?;
    validate_target_frame_layout(machine, requirements, storage, environment, plan)
}

pub(crate) fn validate_target_frame_layout(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: TargetFrameLayoutPlan,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    if candidate.post_allocation_machine != machine.machine().receipt().identity()
        || candidate.callee_saved_requirements != requirements.receipt().identity()
        || candidate.callee_save_storage != storage.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
        || candidate.abi != requirements.plan().abi
    {
        return Err(TargetFrameLayoutError::RootMismatch);
    }
    validate_frame_layout_plan(machine, requirements, storage, environment, &candidate)?;
    let receipt = seal_frame_layout(&candidate);
    Ok(ValidatedTargetFrameLayout {
        plan: Arc::new(candidate),
        receipt,
    })
}

fn seal_frame_layout(plan: &TargetFrameLayoutPlan) -> TargetFrameLayoutReceipt {
    TargetFrameLayoutReceipt {
        identity: target_frame_layout_identity(plan),
        post_allocation_machine: plan.post_allocation_machine,
        callee_saved_requirements: plan.callee_saved_requirements,
        callee_save_storage: plan.callee_save_storage,
        target: plan.target,
        abi: plan.abi,
        policy: plan.policy,
        function_count: plan.functions.len(),
        framed_function_count: count_where(&plan.functions, |row| row.frame_size_bytes != 0),
        calling_function_count: count_where(&plan.functions, |row| row.contains_call),
        callee_save_slot_count: plan
            .functions
            .iter()
            .map(|row| row.callee_save_slots.len())
            .sum(),
        saved_link_count: plan
            .functions
            .iter()
            .filter(|row| {
                matches!(
                    row.return_address,
                    ReturnAddressFrameCustody::SavedLinkRegister { .. }
                )
            })
            .count(),
        probed_function_count: count_where(&plan.functions, |row| row.stack_probe.touches != 0),
        max_frame_size_bytes: plan
            .functions
            .iter()
            .map(|row| row.frame_size_bytes)
            .max()
            .unwrap_or(0),
    }
}

fn derive_frame_layout(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameLayoutPolicy,
) -> Result<TargetFrameLayoutPlan, TargetFrameLayoutError> {
    let machine_plan = machine.machine().plan();
    let requirement_plan = requirements.plan();
    let storage_plan = storage.plan();
    if machine_plan.target != environment.target()
        || machine_plan.register_environment != environment.identity()
        || machine_plan.physical_register_model != environment.physical().identity()
        || requirement_plan.selected != machine_plan.selected
        || requirement_plan.homes != machine_plan.homes
        || requirement_plan.post_allocation_manifest != machine_plan.post_allocation_manifest
        || requirement_plan.register_environment != environment.identity()
        || requirement_plan.physical_register_model != environment.physical().identity()
        || requirement_plan.target != environment.target()
        || storage_plan.callee_saved_requirements != requirements.receipt().identity()
        || storage_plan.register_environment != environment.identity()
        || storage_plan.physical_register_model != environment.physical().identity()
        || storage_plan.target != environment.target()
        || storage_plan.abi != requirement_plan.abi
    {
        return Err(TargetFrameLayoutError::RootMismatch);
    }

    if machine_plan.functions.len() != requirement_plan.functions.len()
        || machine_plan.functions.len() != storage_plan.functions.len()
    {
        return Err(TargetFrameLayoutError::FunctionRosterMismatch);
    }

    let functions = machine_plan
        .functions
        .iter()
        .zip(&requirement_plan.functions)
        .zip(&storage_plan.functions)
        .map(
            |((machine_function, requirement_function), storage_function)| {
                if machine_function.machine != requirement_function.machine
                    || machine_function.machine != storage_function.machine
                {
                    return Err(TargetFrameLayoutError::FunctionRosterMismatch);
                }
                let calls = contains_call(machine_function);
                let loans = stable_address_loans(machine_function);
                let callee_save_slots = storage_function
                    .slots
                    .iter()
                    .map(|slot| CalleeSaveFrameSlot {
                        abstract_slot: slot.id,
                        storage_view: slot.storage_view,
                        frame_offset_bytes: slot.abstract_offset_bytes,
                        size_bytes: slot.size_bytes,
                        alignment_bytes: slot.alignment_bytes,
                    })
                    .collect::<Vec<_>>();
                function_layout(
                    environment,
                    requirement_plan.abi,
                    policy,
                    machine_function.machine,
                    calls,
                    body_writes_link_register(environment, machine_function)?,
                    &machine_function.outgoing_arguments,
                    &machine_function.local_storage_slots,
                    loans,
                    storage_function.abstract_area_bytes,
                    callee_save_slots,
                )
            },
        )
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TargetFrameLayoutPlan {
        post_allocation_machine: machine.machine().receipt().identity(),
        callee_saved_requirements: requirements.receipt().identity(),
        callee_save_storage: storage.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        target: environment.target(),
        abi: requirement_plan.abi,
        policy,
        functions,
    })
}

#[allow(clippy::too_many_arguments)]
fn function_layout(
    environment: &ValidatedTargetRegisterEnvironment,
    abi: FrameAbiPreservationConvention,
    policy: TargetFrameLayoutPolicy,
    machine: semantic_vocabulary::MachineId,
    contains_call: bool,
    writes_link_register: bool,
    outgoing_arguments: &[selected_instructions::SelectedOutgoingArgumentSlot],
    local_storage: &[selected_instructions::SelectedLocalStorageSlot],
    stable_address_loans: Vec<selected_instructions::LocalStorageSlotId>,
    callee_save_area_bytes: u64,
    mut callee_save_slots: Vec<CalleeSaveFrameSlot>,
) -> Result<FunctionTargetFrameLayout, TargetFrameLayoutError> {
    let call_site = call_site_stack_contract(environment, abi)
        .ok_or(TargetFrameLayoutError::UnsupportedTarget)?;
    let shadow_bytes = if contains_call && abi == FrameAbiPreservationConvention::MicrosoftX64 {
        32
    } else {
        0
    };
    let mut outgoing_extent = u64::from(shadow_bytes);
    for slot in outgoing_arguments {
        if !contains_call
            || slot.byte_size == 0
            || !slot.alignment.is_power_of_two()
            || slot.abi_stack_byte_offset < u32::from(shadow_bytes)
            || !slot.abi_stack_byte_offset.is_multiple_of(u32::from(
                slot.reservation_alignment(call_site.stack_alignment_bytes),
            ))
            || (slot.alignment > call_site.stack_alignment_bytes
                && slot.id.role != selected_instructions::OutgoingArgumentSlotRole::ValueCopy)
        {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        let end = slot
            .abi_stack_byte_offset
            .checked_add(
                slot.reserved_byte_size(call_site.stack_alignment_bytes)
                    .ok_or(TargetFrameLayoutError::GeometryOverflow)?,
            )
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
        outgoing_extent = outgoing_extent.max(u64::from(end));
    }
    let outgoing_abi_area = machine_code::OutgoingAbiFrameArea {
        byte_size: outgoing_extent,
        shadow_bytes,
    };
    let mut local_extent = outgoing_extent;
    let mut local_storage_slots = Vec::with_capacity(local_storage.len());
    for (index, slot) in local_storage.iter().enumerate() {
        if slot.byte_size == 0
            || !slot.alignment.is_power_of_two()
            || local_storage[..index]
                .iter()
                .any(|earlier| earlier.id == slot.id)
        {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        let frame_offset_bytes = align_up_frame(
            local_extent,
            u64::from(slot.alignment.min(call_site.stack_alignment_bytes)),
        )?;
        let placed = machine_code::LocalStorageFrameSlot {
            id: slot.id,
            frame_offset_bytes,
            size_bytes: slot.byte_size,
            alignment_bytes: slot.alignment,
        };
        local_extent = frame_offset_bytes
            .checked_add(placed.reserved_size_bytes(call_site.stack_alignment_bytes))
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
        local_storage_slots.push(placed);
    }
    if stable_address_loans
        .iter()
        .any(|loan| !local_storage_slots.iter().any(|slot| slot.id == *loan))
    {
        return Err(TargetFrameLayoutError::NonCanonicalLayout);
    }
    let preservation_offset = if local_storage_slots.is_empty() && callee_save_slots.is_empty() {
        outgoing_extent
    } else {
        let preservation_alignment = callee_save_slots
            .iter()
            .map(|slot| slot.alignment_bytes)
            .max()
            .unwrap_or(8)
            .max(8);
        if !preservation_alignment.is_power_of_two() {
            return Err(TargetFrameLayoutError::NonCanonicalLayout);
        }
        align_up_frame(local_extent, preservation_alignment)?
    };
    for slot in &mut callee_save_slots {
        slot.frame_offset_bytes = slot
            .frame_offset_bytes
            .checked_add(preservation_offset)
            .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
    }
    let used_area_bytes = callee_save_area_bytes
        .checked_add(preservation_offset)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
    let locals_need_aligned_activation = local_storage.iter().any(|slot| {
        !call_site
            .entry_residue_bytes
            .is_multiple_of(u64::from(slot.alignment))
    });
    let unwind = frame_unwind_policy(environment.target())
        .ok_or(TargetFrameLayoutError::UnsupportedTarget)?;
    if unwind.abi != abi {
        return Err(TargetFrameLayoutError::UnsupportedTarget);
    }
    let stack_pointer = environment
        .physical()
        .model()
        .view_named(unwind.stack_pointer_view)
        .ok_or(TargetFrameLayoutError::MissingStackPointerView)?
        .id;
    let (frame_size_bytes, red_zone_resident_bytes, return_address) = match unwind.continuation {
        FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes,
        } => {
            let frame_size = if contains_call
                || locals_need_aligned_activation
                || (unwind.abi == FrameAbiPreservationConvention::MicrosoftX64
                    && used_area_bytes != 0)
            {
                align_to_residue(
                    used_area_bytes,
                    u64::from(call_site.stack_alignment_bytes),
                    call_site.entry_residue_bytes,
                )?
            } else {
                align_up_frame(used_area_bytes, 8)?
            };
            let red_zone_capacity = u64::from(
                selected_abi_preservation(environment)
                    .map_err(|_| TargetFrameLayoutError::UnsupportedTarget)?
                    .convention
                    .red_zone_bytes,
            );
            let red_zone_resident = if unwind.abi == FrameAbiPreservationConvention::SystemVAMD64
                && !contains_call
                && callee_save_slots.is_empty()
                && frame_size != 0
                && frame_size <= red_zone_capacity
                && local_storage.iter().all(|slot| {
                    matches!(
                        slot.id,
                        selected_instructions::LocalStorageSlotId::Spill { .. }
                    )
                }) {
                frame_size
            } else {
                0
            };
            (
                frame_size,
                red_zone_resident,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: frame_size - red_zone_resident,
                    size_bytes: return_address_size_bytes,
                },
            )
        }
        FrameContinuationCustody::LinkRegister {
            view,
            saved_size_bytes,
        } => {
            let link = environment
                .physical()
                .model()
                .view_named(view)
                .ok_or(TargetFrameLayoutError::MissingLinkRegisterView)?
                .id;
            let saved_size = u64::from(saved_size_bytes);
            if contains_call
                || writes_link_register
                || policy == TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1
            {
                let link_offset = align_up_frame(used_area_bytes, saved_size)?;
                let used = link_offset
                    .checked_add(saved_size)
                    .ok_or(TargetFrameLayoutError::GeometryOverflow)?;
                (
                    align_up_frame(used, u64::from(call_site.stack_alignment_bytes))?,
                    0,
                    ReturnAddressFrameCustody::SavedLinkRegister {
                        view: link,
                        frame_offset_bytes: link_offset,
                        size_bytes: saved_size_bytes,
                    },
                )
            } else {
                (
                    align_up_frame(used_area_bytes, u64::from(call_site.stack_alignment_bytes))?,
                    0,
                    ReturnAddressFrameCustody::LiveLinkRegister { view: link },
                )
            }
        }
    };

    if frame_size_bytes > MAX_FRAME_BYTES {
        return Err(TargetFrameLayoutError::FrameExceedsAddressableExtent { frame_size_bytes });
    }

    let committed_bytes = frame_size_bytes - red_zone_resident_bytes;
    let stack_probe = probe_plan(environment.target(), committed_bytes)?;
    let restores = unwind_restores(return_address, &callee_save_slots);

    Ok(FunctionTargetFrameLayout {
        machine,
        contains_call,
        stack_pointer,
        pre_call_stack_alignment: call_site.stack_alignment_bytes,
        frame_size_bytes,
        red_zone_resident_bytes,
        abi_stack_alignment_bytes: call_site.stack_alignment_bytes,
        outgoing_abi_area,
        local_storage_slots,
        stable_address_loans,
        callee_save_slots,
        return_address,
        stack_probe,
        unwind: FrameUnwindPlan {
            restores,
            released_bytes: committed_bytes,
            return_address,
        },
    })
}

fn probe_plan(
    target: NativeTarget,
    frame_size_bytes: u64,
) -> Result<StackProbePlan, TargetFrameLayoutError> {
    let interval_bytes =
        stack_commit_granule_bytes(target).ok_or(TargetFrameLayoutError::UnsupportedTarget)?;
    let touches = if frame_size_bytes > interval_bytes {
        u32::try_from(frame_size_bytes.div_ceil(interval_bytes))
            .map_err(|_| TargetFrameLayoutError::GeometryOverflow)?
    } else {
        0
    };
    Ok(StackProbePlan {
        interval_bytes,
        touches,
    })
}

fn align_up_frame(value: u64, alignment: u64) -> Result<u64, TargetFrameLayoutError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)
}

fn align_to_residue(
    value: u64,
    alignment: u64,
    residue: u64,
) -> Result<u64, TargetFrameLayoutError> {
    let current = value % alignment;
    let padding = (residue + alignment - current) % alignment;
    value
        .checked_add(padding)
        .ok_or(TargetFrameLayoutError::GeometryOverflow)
}

fn validate_frame_layout_plan(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameLayoutPlan,
) -> Result<(), TargetFrameLayoutError> {
    use TargetFrameLayoutError as Error;
    let current = machine.machine().plan();
    let required = requirements.plan();
    let saved = storage.plan();
    if current.target != environment.target()
        || current.register_environment != environment.identity()
        || current.physical_register_model != environment.physical().identity()
        || required.selected != current.selected
        || required.homes != current.homes
        || required.post_allocation_manifest != current.post_allocation_manifest
        || required.register_environment != environment.identity()
        || required.physical_register_model != environment.physical().identity()
        || required.target != environment.target()
        || saved.callee_saved_requirements != requirements.receipt().identity()
        || saved.register_environment != environment.identity()
        || saved.physical_register_model != environment.physical().identity()
        || saved.target != environment.target()
        || saved.abi != required.abi
    {
        return Err(Error::RootMismatch);
    }

    if current.functions.len() != required.functions.len()
        || current.functions.len() != saved.functions.len()
    {
        return Err(Error::FunctionRosterMismatch);
    }
    if candidate.functions.len() != current.functions.len() {
        return Err(Error::NonCanonicalLayout);
    }
    let call_site =
        call_site_stack_contract(environment, required.abi).ok_or(Error::UnsupportedTarget)?;
    for (((source, requirement), storage), row) in current
        .functions
        .iter()
        .zip(&required.functions)
        .zip(&saved.functions)
        .zip(&candidate.functions)
    {
        if source.machine != requirement.machine || source.machine != storage.machine {
            return Err(Error::FunctionRosterMismatch);
        }
        let calls = contains_call(source);
        let shadow = if calls && required.abi == FrameAbiPreservationConvention::MicrosoftX64 {
            32_u64
        } else {
            0
        };
        let mut outgoing = shadow;
        for (index, slot) in source.outgoing_arguments.iter().enumerate() {
            let end = u64::from(slot.abi_stack_byte_offset)
                + u64::from(
                    slot.reserved_byte_size(call_site.stack_alignment_bytes)
                        .ok_or(Error::GeometryOverflow)?,
                );
            if !calls
                || slot.byte_size == 0
                || !slot.alignment.is_power_of_two()
                || u64::from(slot.abi_stack_byte_offset) < shadow
                || !slot.abi_stack_byte_offset.is_multiple_of(u32::from(
                    slot.reservation_alignment(call_site.stack_alignment_bytes),
                ))
                || (slot.alignment > call_site.stack_alignment_bytes
                    && slot.id.role != selected_instructions::OutgoingArgumentSlotRole::ValueCopy)
                || end > u64::from(u32::MAX)
                || source.outgoing_arguments[..index].iter().any(|earlier| {
                    earlier.id == slot.id
                        || (earlier.id.invocation_site == slot.id.invocation_site
                            && u64::from(earlier.abi_stack_byte_offset) < end
                            && u64::from(slot.abi_stack_byte_offset)
                                < u64::from(earlier.abi_stack_byte_offset)
                                    + u64::from(
                                        earlier
                                            .reserved_byte_size(call_site.stack_alignment_bytes)
                                            .unwrap_or(u32::MAX),
                                    ))
                })
            {
                return Err(Error::NonCanonicalLayout);
            }
            outgoing = outgoing.max(end);
        }
        if row.local_storage_slots.len() != source.local_storage_slots.len() {
            return Err(Error::NonCanonicalLayout);
        }
        let mut local_extent = outgoing;
        for (index, (local, placed)) in source
            .local_storage_slots
            .iter()
            .zip(&row.local_storage_slots)
            .enumerate()
        {
            if local.byte_size == 0
                || !local.alignment.is_power_of_two()
                || source.local_storage_slots[..index]
                    .iter()
                    .any(|earlier| earlier.id == local.id)
                || placed.id != local.id
                || placed.size_bytes != local.byte_size
                || placed.alignment_bytes != local.alignment
                || !minimal_aligned_extent(
                    local_extent,
                    placed.frame_offset_bytes,
                    u64::from(local.alignment.min(call_site.stack_alignment_bytes)),
                    0,
                )
            {
                return Err(Error::NonCanonicalLayout);
            }
            local_extent = placed
                .frame_offset_bytes
                .checked_add(placed.reserved_size_bytes(call_site.stack_alignment_bytes))
                .ok_or(Error::GeometryOverflow)?;
        }
        let loans = stable_address_loans(source);
        if row.stable_address_loans != loans
            || loans
                .iter()
                .any(|loan| !row.local_storage_slots.iter().any(|slot| slot.id == *loan))
        {
            return Err(Error::NonCanonicalLayout);
        }
        let preservation_offset =
            if source.local_storage_slots.is_empty() && storage.slots.is_empty() {
                outgoing
            } else {
                let alignment = storage
                    .slots
                    .iter()
                    .map(|slot| slot.alignment_bytes)
                    .max()
                    .unwrap_or(8)
                    .max(8);
                if !alignment.is_power_of_two() {
                    return Err(Error::NonCanonicalLayout);
                }
                local_extent
                    .checked_add((alignment - local_extent % alignment) % alignment)
                    .ok_or(Error::GeometryOverflow)?
            };
        let area = storage
            .abstract_area_bytes
            .checked_add(preservation_offset)
            .ok_or(Error::GeometryOverflow)?;
        if row.frame_size_bytes > MAX_FRAME_BYTES {
            return Err(Error::FrameExceedsAddressableExtent {
                frame_size_bytes: row.frame_size_bytes,
            });
        }
        let resident = row.red_zone_resident_bytes;
        let committed = committed_extent(row).ok_or(Error::NonCanonicalLayout)?;
        if resident != 0 {
            let capacity = u64::from(
                selected_abi_preservation(environment)
                    .map_err(|_| Error::UnsupportedTarget)?
                    .convention
                    .red_zone_bytes,
            );
            if required.abi != FrameAbiPreservationConvention::SystemVAMD64
                || calls
                || !storage.slots.is_empty()
                || row.frame_size_bytes > capacity
            {
                return Err(Error::NonCanonicalLayout);
            }
        }
        let probe_interval =
            stack_commit_granule_bytes(environment.target()).ok_or(Error::UnsupportedTarget)?;
        let touches = u64::from(row.stack_probe.touches);
        if row.stack_probe.interval_bytes != probe_interval
            || (touches == 0) != (committed <= probe_interval)
            || (touches != 0
                && !(touches
                    .checked_mul(probe_interval)
                    .is_some_and(|covered| covered >= committed)
                    && touches
                        .checked_sub(1)
                        .and_then(|earlier| earlier.checked_mul(probe_interval))
                        .is_some_and(|uncovered| uncovered < committed)))
        {
            return Err(Error::NonCanonicalLayout);
        }
        if row.machine != source.machine
            || row.contains_call != calls
            || row.pre_call_stack_alignment != call_site.stack_alignment_bytes
            || row.abi_stack_alignment_bytes != call_site.stack_alignment_bytes
            || row.outgoing_abi_area.byte_size != outgoing
            || u64::from(row.outgoing_abi_area.shadow_bytes) != shadow
            || row.callee_save_slots.len() != storage.slots.len()
            || row
                .callee_save_slots
                .iter()
                .zip(&storage.slots)
                .any(|(placed, abstract_slot)| {
                    placed.abstract_slot != abstract_slot.id
                        || placed.storage_view != abstract_slot.storage_view
                        || placed.frame_offset_bytes.checked_sub(preservation_offset)
                            != Some(abstract_slot.abstract_offset_bytes)
                        || placed.size_bytes != abstract_slot.size_bytes
                        || placed.alignment_bytes != abstract_slot.alignment_bytes
                        || !save_region_is_disjoint(
                            placed.frame_offset_bytes,
                            placed.size_bytes,
                            placed.alignment_bytes,
                            preservation_offset,
                            area,
                        )
                })
        {
            return Err(Error::NonCanonicalLayout);
        }
        let physical = environment.physical().model();
        let unwind = frame_unwind_policy(environment.target()).ok_or(Error::UnsupportedTarget)?;
        if unwind.abi != required.abi {
            return Err(Error::UnsupportedTarget);
        }
        let stack = physical
            .view_named(unwind.stack_pointer_view)
            .ok_or(Error::MissingStackPointerView)?
            .id;
        if row.stack_pointer != stack || !unwind.continuation.admits(row.return_address) {
            return Err(Error::NonCanonicalLayout);
        }
        match unwind.continuation {
            FrameContinuationCustody::CallerActivationStack {
                return_address_size_bytes,
            } => {
                let locals_need_aligned_activation =
                    source.local_storage_slots.iter().any(|slot| {
                        !call_site
                            .entry_residue_bytes
                            .is_multiple_of(u64::from(slot.alignment))
                    });
                let (alignment, residue) = if calls
                    || locals_need_aligned_activation
                    || (required.abi == FrameAbiPreservationConvention::MicrosoftX64 && area != 0)
                {
                    (
                        u64::from(call_site.stack_alignment_bytes),
                        call_site.entry_residue_bytes,
                    )
                } else {
                    (8, 0)
                };
                if !minimal_aligned_extent(area, row.frame_size_bytes, alignment, residue)
                    || row.return_address
                        != (ReturnAddressFrameCustody::CallerActivationStack {
                            post_prologue_offset_bytes: committed,
                            size_bytes: return_address_size_bytes,
                        })
                {
                    return Err(Error::NonCanonicalLayout);
                }
            }
            FrameContinuationCustody::LinkRegister {
                view,
                saved_size_bytes,
            } => {
                let link = physical
                    .view_named(view)
                    .ok_or(Error::MissingLinkRegisterView)?
                    .id;
                let saved_size = u64::from(saved_size_bytes);
                if calls
                    || body_writes_link_register(environment, source)?
                    || candidate.policy
                        == TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1
                {
                    let ReturnAddressFrameCustody::SavedLinkRegister {
                        view,
                        frame_offset_bytes,
                        size_bytes,
                    } = row.return_address
                    else {
                        return Err(Error::NonCanonicalLayout);
                    };
                    let used = frame_offset_bytes
                        .checked_add(saved_size)
                        .ok_or(Error::GeometryOverflow)?;
                    if view != link
                        || size_bytes != saved_size_bytes
                        || !minimal_aligned_extent(area, frame_offset_bytes, saved_size, 0)
                        || !minimal_aligned_extent(
                            used,
                            row.frame_size_bytes,
                            u64::from(call_site.stack_alignment_bytes),
                            0,
                        )
                    {
                        return Err(Error::NonCanonicalLayout);
                    }
                } else if row.return_address
                    != (ReturnAddressFrameCustody::LiveLinkRegister { view: link })
                    || !minimal_aligned_extent(
                        area,
                        row.frame_size_bytes,
                        u64::from(call_site.stack_alignment_bytes),
                        0,
                    )
                {
                    return Err(Error::NonCanonicalLayout);
                }
            }
        }
        let restores = unwind_restores(row.return_address, &row.callee_save_slots);
        if row.unwind.restores != restores
            || row.unwind.released_bytes != committed
            || row.unwind.return_address != row.return_address
        {
            return Err(Error::NonCanonicalLayout);
        }
    }
    Ok(())
}

fn minimal_aligned_extent(used: u64, extent: u64, alignment: u64, residue: u64) -> bool {
    extent
        .checked_sub(used)
        .is_some_and(|padding| padding < alignment)
        && extent % alignment == residue
}

fn save_region_is_disjoint(
    offset: u64,
    size: u64,
    alignment: u64,
    outgoing: u64,
    end: u64,
) -> bool {
    size != 0
        && alignment != 0
        && offset.is_multiple_of(alignment)
        && offset >= outgoing
        && offset.checked_add(size).is_some_and(|limit| limit <= end)
}

#[cfg(test)]
fn spill(register: u32) -> selected_instructions::SelectedLocalStorageSlot {
    selected_instructions::SelectedLocalStorageSlot {
        id: selected_instructions::LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(register),
        },
        byte_size: 8,
        alignment: 8,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CalleeSaveFrameSlot, FrameAbiPreservationConvention, FrameContinuationCustody,
        FrameUnwindPolicy, MAX_FRAME_BYTES, ReturnAddressFrameCustody, TargetFrameLayoutError,
        TargetFrameLayoutPolicy, call_site_stack_contract, frame_unwind_policy, function_layout,
        minimal_aligned_extent, save_region_is_disjoint, spill, stack_commit_granule_bytes,
    };
    use target::{NativeTarget, ObjectFormat};

    macro_rules! row {
        ($a:expr => $b:expr) => {
            ($a, $b)
        };
    }

    macro_rules! t {
        ($target:expr => $abi:ident => $c:expr => $d:expr) => {
            ($target, FrameAbiPreservationConvention::$abi, $c, $d)
        };
        ($target:expr => $abi:ident => $c:expr) => {
            ($target, FrameAbiPreservationConvention::$abi, $c)
        };
    }

    #[derive(Clone, Copy)]
    enum CaseShape {
        Bare,
        Call,
        Link,
    }

    #[allow(clippy::too_many_arguments)]
    fn frame_with_saves(
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        abi: FrameAbiPreservationConvention,
        contains_call: bool,
        writes_link_register: bool,
        outgoing_arguments: &[selected_instructions::SelectedOutgoingArgumentSlot],
        local_storage: &[selected_instructions::SelectedLocalStorageSlot],
        callee_save_area_bytes: u64,
        callee_save_slots: Vec<CalleeSaveFrameSlot>,
    ) -> Result<machine_code::FunctionTargetFrameLayout, TargetFrameLayoutError> {
        function_layout(
            environment,
            abi,
            TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            semantic_vocabulary::MachineId::new(1).unwrap(),
            contains_call,
            writes_link_register,
            outgoing_arguments,
            local_storage,
            Vec::new(),
            callee_save_area_bytes,
            callee_save_slots,
        )
    }

    fn frame(
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        abi: FrameAbiPreservationConvention,
        shape: CaseShape,
        storage: (
            &[selected_instructions::SelectedOutgoingArgumentSlot],
            &[selected_instructions::SelectedLocalStorageSlot],
        ),
    ) -> Result<machine_code::FunctionTargetFrameLayout, TargetFrameLayoutError> {
        let (outgoing_arguments, local_storage) = storage;
        frame_with_saves(
            environment,
            abi,
            matches!(shape, CaseShape::Call),
            matches!(shape, CaseShape::Link),
            outgoing_arguments,
            local_storage,
            0,
            Vec::new(),
        )
    }

    #[test]
    fn call_site_contract_binds_the_declared_convention_alignment() {
        for (target, abi, residue) in [
            (
                NativeTarget::linux_x64(),
                FrameAbiPreservationConvention::SystemVAMD64,
                8,
            ),
            (
                NativeTarget::windows_x64(),
                FrameAbiPreservationConvention::MicrosoftX64,
                8,
            ),
            (
                NativeTarget::uefi_x64(),
                FrameAbiPreservationConvention::MicrosoftX64,
                8,
            ),
            (
                NativeTarget::linux_arm64(),
                FrameAbiPreservationConvention::Aapcs64,
                0,
            ),
            (
                NativeTarget::macos_arm64(),
                FrameAbiPreservationConvention::DarwinAapcs64,
                0,
            ),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let contract = call_site_stack_contract(&environment, abi).unwrap();
            assert_eq!(
                contract.stack_alignment_bytes,
                register_environment::selected_abi_preservation(&environment)
                    .unwrap()
                    .convention
                    .stack_alignment,
                "{target:?}"
            );
            assert_eq!(contract.entry_residue_bytes, residue, "{target:?}");
        }
    }

    #[test]
    fn call_site_contract_fails_closed_on_undeclared_pairs() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
                .unwrap();
        for abi in [
            FrameAbiPreservationConvention::Aapcs64,
            FrameAbiPreservationConvention::DarwinAapcs64,
        ] {
            assert_eq!(call_site_stack_contract(&environment, abi), None, "{abi:?}");
        }
    }

    #[test]
    fn stack_commit_matrix_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (NativeTarget::linux_x64(), 4_096),
            (NativeTarget::windows_x64(), 4_096),
            (NativeTarget::uefi_x64(), 4_096),
            (NativeTarget::linux_arm64(), 4_096),
            (NativeTarget::macos_arm64(), 16_384),
            (NativeTarget::macos_x64(), 4_096),
        ] {
            assert_eq!(
                stack_commit_granule_bytes(target),
                Some(expected),
                "{target:?}"
            );
        }
    }

    #[test]
    fn stack_commit_matrix_fails_closed_on_undeclared_pairs() {
        let target = NativeTarget {
            object_format: ObjectFormat::Coff,
            ..NativeTarget::linux_arm64()
        };
        assert_eq!(stack_commit_granule_bytes(target), None, "{target:?}");
    }

    #[test]
    fn unwind_matrix_declares_one_row_per_supported_pair() {
        let caller_stack = FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes: 8,
        };
        let link = FrameContinuationCustody::LinkRegister {
            view: "x30",
            saved_size_bytes: 8,
        };
        for (target, expected) in [
            (
                NativeTarget::linux_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::SystemVAMD64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                },
            ),
            (
                NativeTarget::macos_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::SystemVAMD64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                },
            ),
            (
                NativeTarget::windows_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::MicrosoftX64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                },
            ),
            (
                NativeTarget::uefi_x64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::MicrosoftX64,
                    stack_pointer_view: "rsp",
                    continuation: caller_stack,
                },
            ),
            (
                NativeTarget::linux_arm64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::Aapcs64,
                    stack_pointer_view: "sp",
                    continuation: link,
                },
            ),
            (
                NativeTarget::macos_arm64(),
                FrameUnwindPolicy {
                    abi: FrameAbiPreservationConvention::DarwinAapcs64,
                    stack_pointer_view: "sp",
                    continuation: link,
                },
            ),
        ] {
            assert_eq!(frame_unwind_policy(target), Some(expected), "{target:?}");
        }
    }

    #[test]
    fn unwind_matrix_fails_closed_on_undeclared_pairs() {
        let target = NativeTarget {
            object_format: ObjectFormat::Coff,
            ..NativeTarget::linux_arm64()
        };
        assert_eq!(frame_unwind_policy(target), None, "{target:?}");
    }

    #[test]
    fn continuation_admits_only_its_own_custody_family() {
        let caller_stack = FrameContinuationCustody::CallerActivationStack {
            return_address_size_bytes: 8,
        };
        let link = FrameContinuationCustody::LinkRegister {
            view: "x30",
            saved_size_bytes: 8,
        };
        let view = register_model::RegisterViewId(7);
        assert!(
            caller_stack.admits(ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            })
        );
        assert!(!caller_stack.admits(ReturnAddressFrameCustody::LiveLinkRegister { view }));
        assert!(
            !caller_stack.admits(ReturnAddressFrameCustody::SavedLinkRegister {
                view,
                frame_offset_bytes: 0,
                size_bytes: 8,
            })
        );
        assert!(link.admits(ReturnAddressFrameCustody::LiveLinkRegister { view }));
        assert!(link.admits(ReturnAddressFrameCustody::SavedLinkRegister {
            view,
            frame_offset_bytes: 0,
            size_bytes: 8,
        }));
        assert!(
            !link.admits(ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            })
        );
    }

    #[test]
    fn saves_cannot_overlap_the_outgoing_abi_area_or_escape_storage() {
        assert!(save_region_is_disjoint(8, 8, 8, 4, 16));
        assert!(!save_region_is_disjoint(4, 8, 8, 4, 12));
        assert!(save_region_is_disjoint(32, 8, 8, 32, 40));
        assert!(save_region_is_disjoint(48, 16, 16, 32, 64));
        for (offset, size, alignment, end) in [
            (24, 8, 8, 40),
            (32, 16, 8, 40),
            (33, 8, 8, 48),
            (32, 8, 0, 40),
            (32, 0, 8, 40),
            (u64::MAX - 7, 16, 8, u64::MAX),
        ] {
            assert!(!save_region_is_disjoint(offset, size, alignment, 32, end));
        }
    }

    #[test]
    fn submitted_padding_must_be_minimal_and_have_the_abi_residue() {
        assert!(minimal_aligned_extent(9, 16, 8, 0));
        assert!(minimal_aligned_extent(16, 24, 16, 8));
        assert!(!minimal_aligned_extent(9, 8, 8, 0));
        assert!(!minimal_aligned_extent(9, 24, 8, 0));
        assert!(!minimal_aligned_extent(16, 16, 16, 8));
        assert!(!minimal_aligned_extent(u64::MAX, 0, 16, 0));
        assert!(minimal_aligned_extent(u64::MAX - 7, u64::MAX - 7, 16, 8));
    }

    #[test]
    fn compiler_spill_slots_expand_final_frame_without_aliasing_outgoing_or_other_locals() {
        for target in [
            NativeTarget::windows_x64(),
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let slots = [0, 1].map(spill);
            let layout = function_layout(
                &environment,
                abi,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                semantic_vocabulary::MachineId::new(1).unwrap(),
                true,
                false,
                &[],
                &slots,
                Vec::new(),
                0,
                Vec::new(),
            )
            .unwrap();
            let first = &layout.local_storage_slots[0];
            let second = &layout.local_storage_slots[1];
            assert!(first.frame_offset_bytes >= layout.outgoing_abi_area.byte_size);
            assert_eq!(second.frame_offset_bytes, first.frame_offset_bytes + 8);
            assert!(second.frame_offset_bytes + 8 <= layout.frame_size_bytes);
            assert_eq!(first.id, slots[0].id);
            assert!(
                function_layout(
                    &environment,
                    abi,
                    TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                    semantic_vocabulary::MachineId::new(1).unwrap(),
                    true,
                    false,
                    &[],
                    &[slots[0].clone(), slots[0].clone()],
                    Vec::new(),
                    0,
                    Vec::new()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn owned_overaligned_homes_keep_fixed_x64_stack_geometry() {
        use selected_instructions::{
            LocalStorageSlotId, OutgoingArgumentSlotId, OutgoingArgumentSlotRole,
            SelectedLocalStorageSlot, SelectedOutgoingArgumentSlot,
        };
        use semantic_vocabulary::{OperationId, PlaceId};
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let outgoing = SelectedOutgoingArgumentSlot {
                id: OutgoingArgumentSlotId {
                    role: OutgoingArgumentSlotRole::ValueCopy,
                    invocation_site: OperationId::new(11).unwrap().into(),
                    argument_index: 0,
                },
                byte_size: 32,
                alignment: 32,
                abi_stack_byte_offset: if target == NativeTarget::windows_x64() {
                    32
                } else {
                    0
                },
            };
            let local = SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Structural {
                    operation: OperationId::new(12).unwrap(),
                    place: PlaceId::new(1).unwrap(),
                },
                byte_size: 64,
                alignment: 64,
            };
            let layout = frame(
                &environment,
                abi,
                CaseShape::Call,
                (
                    std::slice::from_ref(&outgoing),
                    std::slice::from_ref(&local),
                ),
            )
            .unwrap();
            assert_eq!(layout.abi_stack_alignment_bytes, 16);
            assert_eq!(layout.frame_size_bytes % 16, 8);
            assert_eq!(layout.red_zone_resident_bytes, 0);
            let placed = &layout.local_storage_slots[0];
            let outgoing_end = u64::from(outgoing.abi_stack_byte_offset)
                + u64::from(outgoing.reserved_byte_size(16).unwrap());
            assert!(placed.frame_offset_bytes >= outgoing_end);
            assert!(
                placed.frame_offset_bytes + placed.reserved_size_bytes(16)
                    <= layout.frame_size_bytes
            );
            for (start, size, alignment, reserved) in [
                (
                    u64::from(outgoing.abi_stack_byte_offset),
                    32_u64,
                    32_u64,
                    u64::from(outgoing.reserved_byte_size(16).unwrap()),
                ),
                (
                    placed.frame_offset_bytes,
                    64,
                    64,
                    placed.reserved_size_bytes(16),
                ),
            ] {
                for stack_residue in (0..alignment).step_by(16) {
                    let base = stack_residue + start;
                    let address = (base + alignment - 1) & !(alignment - 1);
                    assert_eq!(address % alignment, 0);
                    assert!(address >= base && address + size <= base + reserved);
                }
            }
            let mut invalid = outgoing;
            invalid.id.role = OutgoingArgumentSlotRole::Argument;
            assert_eq!(
                frame(&environment, abi, CaseShape::Call, (&[invalid], &[])),
                Err(TargetFrameLayoutError::NonCanonicalLayout),
            );
        }
    }

    #[test]
    fn narrow_outgoing_payload_aligns_preservation_without_activation_locals() {
        for (target, abi, stack_offset, save_name) in [
            t!(NativeTarget::linux_x64() => SystemVAMD64 => 0 => "rbx"),
            t!(NativeTarget::windows_x64() => MicrosoftX64 => 32 => "rbx"),
            t!(NativeTarget::linux_arm64() => Aapcs64 => 0 => "x19"),
            t!(NativeTarget::macos_arm64() => DarwinAapcs64 => 0 => "x19"),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let outgoing = selected_instructions::SelectedOutgoingArgumentSlot {
                id: selected_instructions::OutgoingArgumentSlotId {
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    invocation_site: semantic_vocabulary::OperationId::new(11).unwrap().into(),
                    argument_index: 8,
                },
                byte_size: 4,
                alignment: 4,
                abi_stack_byte_offset: stack_offset,
            };
            let save = CalleeSaveFrameSlot {
                abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                storage_view: environment
                    .physical()
                    .model()
                    .view_named(save_name)
                    .unwrap()
                    .id,
                frame_offset_bytes: 0,
                size_bytes: 8,
                alignment_bytes: 8,
            };
            let layout = frame_with_saves(
                &environment,
                abi,
                true,
                false,
                std::slice::from_ref(&outgoing),
                &[],
                8,
                vec![save],
            )
            .unwrap();
            let outgoing_end = u64::from(stack_offset) + 4;
            assert_eq!(layout.outgoing_abi_area.byte_size, outgoing_end);
            assert!(layout.local_storage_slots.is_empty());
            let saved = &layout.callee_save_slots[0];
            assert_eq!(saved.frame_offset_bytes, outgoing_end + 4);
            assert!(
                saved
                    .frame_offset_bytes
                    .is_multiple_of(saved.alignment_bytes)
            );
            assert!(saved.frame_offset_bytes + saved.size_bytes <= layout.frame_size_bytes);

            let without_save =
                frame(&environment, abi, CaseShape::Call, (&[outgoing], &[])).unwrap();
            assert_eq!(without_save.outgoing_abi_area.byte_size, outgoing_end);
            assert!(without_save.callee_save_slots.is_empty());
        }
    }

    #[test]
    fn activation_locals_are_not_outgoing_abi_storage_on_any_host() {
        let id = selected_instructions::LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
        };
        let local = selected_instructions::SelectedLocalStorageSlot {
            id,
            byte_size: 24,
            alignment: 8,
        };
        for target in [
            NativeTarget::windows_x64(),
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let layout = frame(
                &environment,
                abi,
                CaseShape::Call,
                (&[], std::slice::from_ref(&local)),
            )
            .unwrap();
            let outgoing = if abi == FrameAbiPreservationConvention::MicrosoftX64 {
                32
            } else {
                0
            };
            assert_eq!(layout.outgoing_abi_area.byte_size, outgoing);
            assert_eq!(layout.local_storage_slots[0].id, id);
            assert_eq!(layout.local_storage_slots[0].frame_offset_bytes, outgoing);
            assert_eq!(layout.local_storage_slots[0].size_bytes, 24);
            assert!(layout.frame_size_bytes >= outgoing + 24);
            assert!(
                frame(
                    &environment,
                    abi,
                    CaseShape::Call,
                    (&[], &[local.clone(), local.clone()])
                )
                .is_err()
            );
        }
    }

    #[test]
    fn outgoing_pointer_slots_reserve_storage_on_every_host_abi() {
        for (target, abi, offset) in [
            t!(NativeTarget::windows_x64() => MicrosoftX64 => 32),
            t!(NativeTarget::linux_x64() => SystemVAMD64 => 0),
            t!(NativeTarget::linux_arm64() => Aapcs64 => 0),
            t!(NativeTarget::macos_arm64() => DarwinAapcs64 => 0),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let slot = selected_instructions::SelectedOutgoingArgumentSlot {
                id: selected_instructions::OutgoingArgumentSlotId {
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    invocation_site: semantic_vocabulary::OperationId::new(11).unwrap().into(),
                    argument_index: 8,
                },
                byte_size: 8,
                alignment: 8,
                abi_stack_byte_offset: offset,
            };
            let layout = frame(
                &environment,
                abi,
                CaseShape::Call,
                (std::slice::from_ref(&slot), &[]),
            )
            .unwrap();
            assert_eq!(layout.outgoing_abi_area.byte_size, u64::from(offset) + 8);
            assert!(layout.frame_size_bytes >= u64::from(offset) + 8);
            assert!(frame(&environment, abi, CaseShape::Bare, (&[slot], &[])).is_err());
        }
    }

    #[test]
    fn large_frames_commit_through_an_exact_probe_roster() {
        let local = selected_instructions::SelectedLocalStorageSlot {
            id: selected_instructions::LocalStorageSlotId::Structural {
                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                place: semantic_vocabulary::PlaceId::new(11).unwrap(),
            },
            byte_size: 5_000,
            alignment: 8,
        };
        for (target, abi, interval, touches) in [
            t!(NativeTarget::linux_x64() => SystemVAMD64 => 4_096_u64 => 2_u32),
            t!(NativeTarget::windows_x64() => MicrosoftX64 => 4_096 => 2),
            t!(NativeTarget::uefi_x64() => MicrosoftX64 => 4_096 => 2),
            t!(NativeTarget::linux_arm64() => Aapcs64 => 4_096 => 2),
            t!(NativeTarget::macos_arm64() => DarwinAapcs64 => 16_384 => 0),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let layout = frame(
                &environment,
                abi,
                CaseShape::Bare,
                (&[], std::slice::from_ref(&local)),
            )
            .unwrap();
            assert_eq!(layout.stack_probe.interval_bytes, interval, "{target:?}");
            assert_eq!(layout.stack_probe.touches, touches, "{target:?}");
            let small = selected_instructions::SelectedLocalStorageSlot {
                byte_size: 24,
                ..local.clone()
            };
            let small_layout = frame(
                &environment,
                abi,
                CaseShape::Bare,
                (&[], std::slice::from_ref(&small)),
            )
            .unwrap();
            assert_eq!(
                small_layout.stack_probe.interval_bytes, interval,
                "{target:?}"
            );
            assert_eq!(small_layout.stack_probe.touches, 0, "{target:?}");
        }
    }

    #[test]
    fn frames_past_the_shared_displacement_reach_are_refused_by_name() {
        for (target, abi) in [
            row!(NativeTarget::linux_x64() => FrameAbiPreservationConvention::SystemVAMD64),
            row!(NativeTarget::windows_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::linux_arm64() => FrameAbiPreservationConvention::Aapcs64),
            row!(NativeTarget::macos_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let layout = |byte_size: u32| {
                frame(
                    &environment,
                    abi,
                    CaseShape::Bare,
                    (
                        &[],
                        &[selected_instructions::SelectedLocalStorageSlot {
                            id: selected_instructions::LocalStorageSlotId::Structural {
                                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                                place: semantic_vocabulary::PlaceId::new(11).unwrap(),
                            },
                            byte_size,
                            alignment: 8,
                        }],
                    ),
                )
            };
            assert!(layout(33_554_440).unwrap().frame_size_bytes >= 33_554_440);
            assert!(layout(0x7fff_f000).unwrap().frame_size_bytes <= MAX_FRAME_BYTES);
            assert!(matches!(
                layout(0x8000_0000),
                Err(TargetFrameLayoutError::FrameExceedsAddressableExtent {
                    frame_size_bytes
                }) if frame_size_bytes > MAX_FRAME_BYTES
            ));
        }
    }

    #[test]
    fn system_v_leaf_spill_storage_inside_the_red_zone_commits_nothing() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
                .unwrap();
        for slots in [
            vec![spill(0)],
            vec![spill(0), spill(1), spill(2)],
            (0..16).map(spill).collect::<Vec<_>>(),
        ] {
            let extent = u64::try_from(slots.len()).unwrap() * 8;
            let layout = frame(
                &environment,
                FrameAbiPreservationConvention::SystemVAMD64,
                CaseShape::Bare,
                (&[], &slots),
            )
            .unwrap();
            assert_eq!(layout.frame_size_bytes, extent);
            assert_eq!(layout.red_zone_resident_bytes, extent);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: 0,
                    size_bytes: 8,
                }
            );
            assert_eq!(layout.stack_probe.touches, 0);
        }
        let over: Vec<_> = (0..17).map(spill).collect();
        let layout = frame(
            &environment,
            FrameAbiPreservationConvention::SystemVAMD64,
            CaseShape::Bare,
            (&[], &over),
        )
        .unwrap();
        assert_eq!(layout.red_zone_resident_bytes, 0);
        assert_eq!(layout.frame_size_bytes, 136);
        assert_eq!(
            layout.return_address,
            ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 136,
                size_bytes: 8,
            }
        );
    }

    #[test]
    fn red_zone_residency_requires_leaf_spill_only_sysv_storage() {
        let structural = selected_instructions::SelectedLocalStorageSlot {
            id: selected_instructions::LocalStorageSlotId::Structural {
                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                place: semantic_vocabulary::PlaceId::new(11).unwrap(),
            },
            byte_size: 8,
            alignment: 8,
        };
        let sysv =
            register_environment::baseline_target_register_environment(NativeTarget::linux_x64())
                .unwrap();
        for (contains_call, locals, save_area, saves) in [
            (true, vec![spill(0)], 0, Vec::new()),
            (false, vec![structural.clone()], 0, Vec::new()),
            (false, vec![spill(0), structural.clone()], 0, Vec::new()),
            (
                false,
                vec![spill(0)],
                8,
                vec![CalleeSaveFrameSlot {
                    abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                    storage_view: sysv.physical().model().view_named("rbx").unwrap().id,
                    frame_offset_bytes: 0,
                    size_bytes: 8,
                    alignment_bytes: 8,
                }],
            ),
        ] {
            let layout = frame_with_saves(
                &sysv,
                FrameAbiPreservationConvention::SystemVAMD64,
                contains_call,
                false,
                &[],
                &locals,
                save_area,
                saves,
            )
            .unwrap();
            assert_eq!(layout.red_zone_resident_bytes, 0, "{locals:?}");
            assert!(layout.frame_size_bytes != 0);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: layout.frame_size_bytes,
                    size_bytes: 8,
                }
            );
        }
        for (target, abi) in [
            row!(NativeTarget::windows_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::uefi_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::linux_arm64() => FrameAbiPreservationConvention::Aapcs64),
            row!(NativeTarget::macos_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let layout = frame(&environment, abi, CaseShape::Bare, (&[], &[spill(0)])).unwrap();
            assert_eq!(layout.red_zone_resident_bytes, 0, "{target:?}");
            assert!(layout.frame_size_bytes != 0, "{target:?}");
        }
        let empty = frame(
            &sysv,
            FrameAbiPreservationConvention::SystemVAMD64,
            CaseShape::Bare,
            (&[], &[]),
        )
        .unwrap();
        assert_eq!(empty.frame_size_bytes, 0);
        assert_eq!(empty.red_zone_resident_bytes, 0);
    }

    #[test]
    fn unwind_matrix_fails_closed_on_conventions_the_pair_does_not_declare() {
        for (target, abi) in [
            row!(NativeTarget::linux_x64() => FrameAbiPreservationConvention::MicrosoftX64),
            row!(NativeTarget::windows_x64() => FrameAbiPreservationConvention::SystemVAMD64),
            row!(NativeTarget::uefi_x64() => FrameAbiPreservationConvention::SystemVAMD64),
            row!(NativeTarget::linux_arm64() => FrameAbiPreservationConvention::DarwinAapcs64),
            row!(NativeTarget::macos_arm64() => FrameAbiPreservationConvention::Aapcs64),
        ] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            assert_eq!(
                frame(&environment, abi, CaseShape::Bare, (&[], &[])),
                Err(TargetFrameLayoutError::UnsupportedTarget),
                "{target:?} must refuse the foreign convention {abi:?}"
            );
        }
    }

    #[test]
    fn windows_frames_separate_shadow_space_from_preservation_storage() {
        let environment =
            register_environment::baseline_target_register_environment(NativeTarget::windows_x64())
                .unwrap();
        for (area, extent) in [(0, 0), (8, 8), (16, 24)] {
            let layout = frame_with_saves(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                false,
                false,
                &[],
                &[],
                area,
                Vec::new(),
            )
            .unwrap();
            assert_eq!(layout.frame_size_bytes, extent);
            assert_eq!(layout.outgoing_abi_area.byte_size, 0);
            assert_eq!(layout.outgoing_abi_area.shadow_bytes, 0);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: extent,
                    size_bytes: 8,
                }
            );
        }
        for (area, expected_extent) in [(0, 40), (8, 40), (16, 56)] {
            let slots = if area == 0 {
                Vec::new()
            } else {
                vec![CalleeSaveFrameSlot {
                    abstract_slot: machine_code::NonAuthoritativeCalleeSaveSlotId(0),
                    storage_view: environment.physical().model().view_named("rbx").unwrap().id,
                    frame_offset_bytes: 0,
                    size_bytes: area,
                    alignment_bytes: 8,
                }]
            };
            let layout = frame_with_saves(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                true,
                false,
                &[],
                &[],
                area,
                slots,
            )
            .unwrap();
            assert_eq!(layout.outgoing_abi_area.byte_size, 32);
            assert_eq!(layout.outgoing_abi_area.shadow_bytes, 32);
            assert_eq!(layout.frame_size_bytes, expected_extent);
            assert!(
                layout
                    .callee_save_slots
                    .iter()
                    .all(|slot| slot.frame_offset_bytes == 32)
            );
        }
        assert_eq!(
            frame_with_saves(
                &environment,
                FrameAbiPreservationConvention::MicrosoftX64,
                true,
                false,
                &[],
                &[],
                u64::MAX,
                Vec::new()
            ),
            Err(TargetFrameLayoutError::GeometryOverflow)
        );
    }

    #[test]
    fn allocated_link_register_write_gets_a_leaf_frame_slot() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_arm64()] {
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let link = environment.physical().model().view_named("x30").unwrap().id;
            let abi = register_environment::selected_abi_preservation(&environment)
                .unwrap()
                .kind;
            let layout = frame(&environment, abi, CaseShape::Link, (&[], &[])).unwrap();
            assert!(!layout.contains_call);
            assert_eq!(layout.frame_size_bytes, 16);
            assert_eq!(
                layout.return_address,
                ReturnAddressFrameCustody::SavedLinkRegister {
                    view: link,
                    frame_offset_bytes: 0,
                    size_bytes: 8,
                }
            );
        }
    }
}
