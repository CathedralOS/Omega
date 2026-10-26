//! Preserve an actual selected profile call, not an assertion from equal data.
//!
//! The ordinary call owns execution and loan cleanup. Its continuation captures
//! the original borrowed extent and only a normal return attaches that origin
//! to the returned profile's storage. Writes invalidate the complete profile;
//! independent value copies do not inherit this association. Initialization
//! borrows the original result, then separately admits its complete normalized
//! contents against the captured live allocation. These records alone grant
//! neither backing nor resident content.

use super::ProviderStorageBinding;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralArgument, TerminalPlacementLifecycleExecution};

pub(crate) struct PendingResourceProfile {
    boundary: BoundaryMachineId,
    provider: MachineId,
    backing: Option<ProviderStorageBinding>,
    result_type: StructuralTypeId,
}

#[derive(Clone)]
pub(crate) struct CompletedResourceProfile {
    pub(crate) boundary: BoundaryMachineId,
    pub(crate) provider: MachineId,
    pub(crate) backing: Option<ProviderStorageBinding>,
    pub(crate) result_type: StructuralTypeId,
}

impl TerminalExecution<'_> {
    /// Read a profile only from the original completed selected call. Ordinary
    /// profile data, even byte-for-byte equal data, supplies no such origin.
    /// This joins the current linear descriptor to the captured allocation
    /// window; moving that descriptor does not change the window's identity.
    /// Normalization produces compatibility data, not permission to write.
    pub(crate) fn lifecycle_profile(
        &self,
        plan: &terminal_psi::TerminalPlacementLifecyclePlan,
        storage: &TerminalStructuralValue,
        profile: &TerminalStructuralValue,
    ) -> Result<
        (
            ProviderStorageBinding,
            access_plans::ValidatedResourceProfile,
        ),
        TerminalInterpretError,
    > {
        let TerminalPlacementLifecycleExecution::CompilerPrimitive {
            profile_type: Some(profile_type),
            profile_requirement: Some(requirement),
        } = &plan.execution
        else {
            return Err(malformed());
        };
        let completion = self
            .provider_storage
            .completed_profiles
            .get(&StructuralRuntimePlace::from(profile))
            .ok_or(TerminalInterpretError::ContentTransformationMismatch(
                "placement profile is not an unchanged selected-provider result",
            ))?;
        let content = match plan.operation {
            terminal_psi::TerminalPlacementLifecycleOperation::Initialize => Some(
                plan.storage
                    .owned_content()
                    .map_err(TerminalInterpretError::ContentTransformationMismatch)?
                    .0,
            ),
            terminal_psi::TerminalPlacementLifecycleOperation::MoveOut => Some(
                plan.storage
                    .owned_content()
                    .map_err(TerminalInterpretError::ContentTransformationMismatch)?
                    .1,
            ),
            terminal_psi::TerminalPlacementLifecycleOperation::View => None,
        };
        if profile.structural_type != *profile_type
            || completion.result_type != *profile_type
            || completion.boundary != requirement.boundary
            || self.boundary_supplies.get(&requirement.boundary).copied()
                != Some(crate::effects::AdmittedBoundarySupply::CheckedProvider(
                    completion.provider,
                ))
            || storage.structural_type != plan.storage.carrier
            || !storage.qualifications.contains(&plan.storage.granted)
            || content.is_some_and(|content| !storage.qualifications.contains(&content))
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "placement lost its exact profile provider, result, or required grant",
            ));
        }
        let home = StructuralRuntimePlace::from(storage);
        let binding = self
            .provider_storage
            .descriptors
            .get(&home)
            .copied()
            .ok_or(TerminalInterpretError::ContentTransformationMismatch(
                "placement extent has no actual provider backing",
            ))?;
        if completion.backing != Some(binding)
            || match plan.operation {
                terminal_psi::TerminalPlacementLifecycleOperation::Initialize => {
                    binding.resident != arena::Handle::default()
                }
                terminal_psi::TerminalPlacementLifecycleOperation::MoveOut => {
                    binding.resident == arena::Handle::default()
                }
                terminal_psi::TerminalPlacementLifecycleOperation::View => false,
            }
            || !self.is_live_provider_descriptor(&home, &binding)
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "placement profile belongs to a different or retired allocation window",
            ));
        }
        let (_, _, base) = self.provider_storage.live_provider_allocation(&binding)?;
        if !self.provider_storage_geometry_matches(&home, &plan.storage, base, binding.length) {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "placement descriptor changed its captured allocation geometry",
            ));
        }
        let supplied = super::profile_values::read(self, profile)?;
        let normalized = access_plans::validate_resource_profile(supplied, binding.length)
            .map_err(|diagnostic| TerminalInterpretError::ResourceProfile {
                boundary: requirement.boundary,
                diagnostic,
            })?;
        Ok((binding, normalized))
    }

    /// Called only after ordinary selected-provider admission and argument
    /// binding, before its body runs. A direct implementation call never enters
    /// this route, even when it returns identical ResourceProfile data.
    pub(crate) fn capture_resource_profile_call(
        &self,
        boundary: BoundaryMachineId,
        provider: MachineId,
        arguments: &[StructuralArgument],
    ) -> Result<Option<PendingResourceProfile>, TerminalInterpretError> {
        let Some((plan, result_type)) = self
            .program
            .catalogs
            .placement_lifecycle_plans
            .iter()
            .find_map(|plan| match &plan.execution {
                TerminalPlacementLifecycleExecution::CompilerPrimitive {
                    profile_type: Some(result_type),
                    profile_requirement: Some(requirement),
                } if requirement.boundary == boundary => Some((plan, *result_type)),
                _ => None,
            })
        else {
            return Ok(None);
        };
        let [argument] = arguments else {
            return Err(malformed());
        };
        if argument.access != StructuralAccess::SharedBorrow {
            return Err(malformed());
        }
        let value =
            self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
        if value.structural_type != plan.storage.carrier
            || !value.qualifications.contains(&plan.storage.granted)
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "profile supply did not borrow its exact granted extent",
            ));
        }
        let storage = StructuralRuntimePlace::from(&value);
        let backing = self.provider_storage.descriptors.get(&storage).copied();
        Ok(Some(PendingResourceProfile {
            boundary,
            provider,
            backing,
            result_type,
        }))
    }

    /// Completion follows the selected body's paid return and loan cleanup.
    /// Preserve its complete result occurrence without asserting that the
    /// provider supplied valid geometry, rights, or any actual storage.
    pub(crate) fn complete_resource_profile_call(
        &mut self,
        pending: &PendingResourceProfile,
        result: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        if result.structural_type != pending.result_type {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "profile supply returned a different result carrier",
            ));
        }
        let home = StructuralRuntimePlace::from(result);
        if self.provider_storage.completed_profiles.contains_key(&home) {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "profile completion would replace an existing result occurrence",
            ));
        }
        self.provider_storage.completed_profiles.insert(
            home,
            CompletedResourceProfile {
                boundary: pending.boundary,
                provider: pending.provider,
                backing: pending.backing,
                result_type: pending.result_type,
            },
        )?;
        Ok(())
    }
}
