//! Check callee-save storage by keyed reconstruction and frame geometry by
//! equations over explicit demands. Checked records keep private seals; the
//! physical planner owns slot ordering and frame construction.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::count_where;
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
use post_allocation_machine::PostAllocationMachineIdentity;
use post_allocation_machine::PostAllocationMachinePlan;
pub use register_environment::FrameAbiPreservationConvention;
use register_environment::{
    ValidatedTargetRegisterEnvironment, selected_abi_preservation,
    selected_preservation_storage_catalog,
};
use register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirements, AllocatedCalleeSavedUnitRequirement,
    encode_callee_saved_modification_witness_identity,
};
use register_model::{
    PhysicalRegisterModelIdentity, PreservationStorageCatalogIdentity, PreservationStorageGroupId,
    RegisterUnitId, TargetRegisterEnvironmentIdentity, ValidatedPreservationStorageCatalog,
};
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, MachineAlternativeFamily};
use target::{Architecture, NativeTarget, ObjectFormat};
use wire::digest_encoder::DomainDigestEncoder;

pub const MAX_FRAME_BYTES: u64 = i32::MAX as u64;

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
    pub fn into_plan(self) -> NonAuthoritativeCalleeSaveStoragePlan {
        self.plan
    }
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
    demands: Arc<machine_code::frame_demands::MachineFrameDemands>,
    plan: Arc<TargetFrameLayoutPlan>,
    receipt: TargetFrameLayoutReceipt,
}

impl ValidatedTargetFrameLayout {
    pub fn shared_demands(&self) -> Arc<machine_code::frame_demands::MachineFrameDemands> {
        Arc::clone(&self.demands)
    }

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
pub enum FrameContinuationCustody {
    CallerActivationStack {
        return_address_size_bytes: u16,
    },
    LinkRegister {
        view: &'static str,
        saved_size_bytes: u16,
    },
}

impl FrameContinuationCustody {
    pub fn admits(self, custody: ReturnAddressFrameCustody) -> bool {
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
pub struct FrameUnwindPolicy {
    pub abi: FrameAbiPreservationConvention,
    pub stack_pointer_view: &'static str,
    pub continuation: FrameContinuationCustody,
    pub preservation: FramePreservationPlacement,
}

/// Where a frame keeps its preserved registers and saved link register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePreservationPlacement {
    /// Above the local storage, at the top of the frame.
    AboveLocals,
    /// Directly above the outgoing-argument area, below the local storage.
    /// Windows ARM64 unwind codes describe a register saved at most 504
    /// bytes above the stack pointer, so its frames keep the save area low
    /// whatever their locals need.
    BelowLocals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSiteStackContract {
    pub stack_alignment_bytes: u16,
    pub entry_residue_bytes: u64,
}

pub const fn stack_commit_granule_bytes(target: NativeTarget) -> Option<u64> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf)
        | (Architecture::X86_64, ObjectFormat::Coff)
        | (Architecture::X86_64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Elf)
        | (Architecture::Aarch64, ObjectFormat::Coff) => Some(4_096),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(16_384),
    }
}

pub const fn frame_unwind_policy(target: NativeTarget) -> Option<FrameUnwindPolicy> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) | (Architecture::X86_64, ObjectFormat::MachO) => {
            Some(FrameUnwindPolicy {
                abi: FrameAbiPreservationConvention::SystemVAMD64,
                stack_pointer_view: "rsp",
                continuation: FrameContinuationCustody::CallerActivationStack {
                    return_address_size_bytes: 8,
                },
                preservation: FramePreservationPlacement::AboveLocals,
            })
        }
        (Architecture::X86_64, ObjectFormat::Coff) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::MicrosoftX64,
            stack_pointer_view: "rsp",
            continuation: FrameContinuationCustody::CallerActivationStack {
                return_address_size_bytes: 8,
            },
            preservation: FramePreservationPlacement::AboveLocals,
        }),
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(FrameUnwindPolicy {
            abi: FrameAbiPreservationConvention::Aapcs64,
            stack_pointer_view: "sp",
            continuation: FrameContinuationCustody::LinkRegister {
                view: "x30",
                saved_size_bytes: 8,
            },
            preservation: FramePreservationPlacement::AboveLocals,
        }),
        (Architecture::Aarch64, ObjectFormat::MachO | ObjectFormat::Coff) => {
            Some(FrameUnwindPolicy {
                abi: FrameAbiPreservationConvention::DarwinAapcs64,
                stack_pointer_view: "sp",
                continuation: FrameContinuationCustody::LinkRegister {
                    view: "x30",
                    saved_size_bytes: 8,
                },
                preservation: if matches!(target.object_format, ObjectFormat::Coff) {
                    FramePreservationPlacement::BelowLocals
                } else {
                    FramePreservationPlacement::AboveLocals
                },
            })
        }
    }
}

pub fn call_site_stack_contract(
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

pub fn body_writes_link_register(
    environment: &ValidatedTargetRegisterEnvironment,
    function: &post_allocation_machine::PostAllocationMachineFunction,
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

pub fn contains_call(function: &post_allocation_machine::PostAllocationMachineFunction) -> bool {
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

pub fn stable_address_loans(
    function: &post_allocation_machine::PostAllocationMachineFunction,
) -> Vec<LocalStorageSlotId> {
    function
        .instructions()
        .filter_map(|instruction| match instruction.address {
            Some(post_allocation_machine::PhysicalAddressOperation::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                ..
            }) if !matches!(slot, LocalStorageSlotId::Spill { .. }) => Some(slot),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn unwind_restores(
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

pub fn committed_extent(row: &FunctionTargetFrameLayout) -> Option<u64> {
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

pub fn validate_non_authoritative_callee_save_storage(
    source: &AllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: NonAuthoritativeCalleeSaveStoragePlan,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let source_plan = source.plan();
    let catalog = selected_preservation_storage_catalog(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    if candidate.callee_saved_requirements != source.identity()
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
    Ok(seal_admitted_callee_save_storage(candidate))
}

/// Seal storage that `admit_storage_plan` just built from its roots.
///
/// `admit_storage_plan` fixes the roots and enforces the work budget, and the
/// derivation it ran rejects a requirement no storage group preserves. Replaying
/// the derivation would only compare it with itself; a body that retains its
/// stage inputs calls `validate_non_authoritative_callee_save_storage` instead.
pub fn seal_admitted_callee_save_storage(
    plan: NonAuthoritativeCalleeSaveStoragePlan,
) -> ValidatedNonAuthoritativeCalleeSaveStorage {
    let receipt = seal_storage(&plan);
    ValidatedNonAuthoritativeCalleeSaveStorage { plan, receipt }
}

pub fn admit_storage_plan(
    source: &AllocatedCalleeSavedRequirements,
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
        callee_saved_requirements: source.identity(),
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

pub fn prepare_frame_demands(
    machine: &PostAllocationMachinePlan,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<machine_code::frame_demands::MachineFrameDemands, TargetFrameLayoutError> {
    use machine_code::frame_demands::*;
    Ok(MachineFrameDemands {
        identity: machine.identity,
        selected: machine.selected,
        homes: machine.homes,
        post_allocation_manifest: machine.post_allocation_manifest,
        target: machine.target,
        register_environment: machine.register_environment,
        physical_register_model: machine.physical_register_model,
        functions: machine
            .functions
            .iter()
            .map(|function| {
                Ok(FunctionFrameDemands {
                    machine: function.machine,
                    contains_call: contains_call(function),
                    writes_link_register: body_writes_link_register(environment, function)?,
                    stable_address_loans: stable_address_loans(function),
                    local_storage_slots: function.local_storage_slots.clone(),
                    outgoing_arguments: function.outgoing_arguments.clone(),
                })
            })
            .collect::<Result<Vec<_>, TargetFrameLayoutError>>()?,
    })
}

pub fn validate_target_frame_demands(
    machine: Arc<machine_code::frame_demands::MachineFrameDemands>,
    requirements: &AllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: TargetFrameLayoutPlan,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    validate_frame_roots(&machine, requirements, storage, environment, &candidate)?;
    validate_frame_layout_plan(&machine, requirements, storage, environment, &candidate)?;
    Ok(seal_validated_frame_layout(machine, candidate))
}

/// Seal a layout that frame planning just derived from these roots.
///
/// Frame planning already enforces the layout rules while it derives the plan,
/// so replaying them here would only compare the derivation with itself. A
/// body that retains its stage inputs calls `validate_target_frame_demands`
/// instead, which replays every rule.
pub fn seal_derived_target_frame_layout(
    machine: Arc<machine_code::frame_demands::MachineFrameDemands>,
    requirements: &AllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: TargetFrameLayoutPlan,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    validate_frame_roots(&machine, requirements, storage, environment, &candidate)?;
    if candidate.functions.len() != machine.functions.len()
        || candidate
            .functions
            .iter()
            .zip(&machine.functions)
            .any(|(row, source)| row.machine != source.machine)
    {
        return Err(TargetFrameLayoutError::FunctionRosterMismatch);
    }
    Ok(seal_validated_frame_layout(machine, candidate))
}

fn validate_frame_roots(
    machine: &machine_code::frame_demands::MachineFrameDemands,
    requirements: &AllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameLayoutPlan,
) -> Result<(), TargetFrameLayoutError> {
    if candidate.post_allocation_machine != machine.identity
        || candidate.callee_saved_requirements != requirements.identity()
        || candidate.callee_save_storage != storage.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
        || candidate.abi != requirements.plan().abi
    {
        return Err(TargetFrameLayoutError::RootMismatch);
    }
    Ok(())
}

fn seal_validated_frame_layout(
    machine: Arc<machine_code::frame_demands::MachineFrameDemands>,
    candidate: TargetFrameLayoutPlan,
) -> ValidatedTargetFrameLayout {
    let receipt = seal_frame_layout(&candidate);
    ValidatedTargetFrameLayout {
        demands: machine,
        plan: Arc::new(candidate),
        receipt,
    }
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

fn validate_frame_layout_plan(
    machine: &machine_code::frame_demands::MachineFrameDemands,
    requirements: &AllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: &TargetFrameLayoutPlan,
) -> Result<(), TargetFrameLayoutError> {
    use TargetFrameLayoutError as Error;
    let current = machine;
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
        || saved.callee_saved_requirements != requirements.identity()
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
        let calls = source.contains_call;
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
        let unwind = frame_unwind_policy(environment.target()).ok_or(Error::UnsupportedTarget)?;
        if unwind.abi != required.abi {
            return Err(Error::UnsupportedTarget);
        }
        let preservation_alignment = storage
            .slots
            .iter()
            .map(|slot| slot.alignment_bytes)
            .max()
            .unwrap_or(8)
            .max(8);
        if !preservation_alignment.is_power_of_two() {
            return Err(Error::NonCanonicalLayout);
        }
        let saves_link = matches!(
            unwind.continuation,
            FrameContinuationCustody::LinkRegister { .. }
        ) && (calls
            || source.writes_link_register
            || candidate.policy == TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1);
        // Windows ARM64 frames keep preserved registers and the saved link
        // register directly above the outgoing area, then the locals.
        let low_preservation = match unwind.preservation {
            FramePreservationPlacement::AboveLocals => None,
            FramePreservationPlacement::BelowLocals => {
                let offset = if storage.slots.is_empty() {
                    outgoing
                } else {
                    outgoing
                        .checked_add(
                            (preservation_alignment - outgoing % preservation_alignment)
                                % preservation_alignment,
                        )
                        .ok_or(Error::GeometryOverflow)?
                };
                let end = storage
                    .abstract_area_bytes
                    .checked_add(offset)
                    .ok_or(Error::GeometryOverflow)?;
                let locals = match (saves_link, unwind.continuation) {
                    (
                        true,
                        FrameContinuationCustody::LinkRegister {
                            saved_size_bytes, ..
                        },
                    ) => {
                        let size = u64::from(saved_size_bytes);
                        end.checked_add((size - end % size) % size)
                            .and_then(|link| link.checked_add(size))
                            .ok_or(Error::GeometryOverflow)?
                    }
                    _ => end,
                };
                Some((offset, locals))
            }
        };
        let mut local_extent = low_preservation.map_or(outgoing, |(_, locals)| locals);
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
        let loans = &source.stable_address_loans;
        if &row.stable_address_loans != loans
            || loans
                .iter()
                .any(|loan| !row.local_storage_slots.iter().any(|slot| slot.id == *loan))
        {
            return Err(Error::NonCanonicalLayout);
        }
        let preservation_offset = if let Some((offset, _)) = low_preservation {
            offset
        } else if source.local_storage_slots.is_empty() && storage.slots.is_empty() {
            outgoing
        } else {
            let alignment = preservation_alignment;
            local_extent
                .checked_add((alignment - local_extent % alignment) % alignment)
                .ok_or(Error::GeometryOverflow)?
        };
        // The frame must cover everything below its top: the save area, and
        // for low preservation the locals above it.
        let frame_extent = if low_preservation.is_some() {
            local_extent
        } else {
            storage
                .abstract_area_bytes
                .checked_add(preservation_offset)
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
                if saves_link {
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
                        .ok_or(Error::GeometryOverflow)?
                        .max(frame_extent);
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
                        frame_extent,
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

pub fn minimal_aligned_extent(used: u64, extent: u64, alignment: u64, residue: u64) -> bool {
    extent
        .checked_sub(used)
        .is_some_and(|padding| padding < alignment)
        && extent % alignment == residue
}

pub fn save_region_is_disjoint(
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
