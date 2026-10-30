//! Bind an explicit provider allocation to an already admitted entry claim.
//! Entry values supply their host-established extent premises; this
//! operation only joins their exact descriptor with actual storage. It cannot
//! add a qualification or establish resident content.

use super::{ProviderByteAllocation, ProviderStorageBinding, ProviderStorageSlot};
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::{StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue};
use access_plans::AdmittedResourceProfile;
use extents::Extent;
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment};

/// Owned storage supplied by the embedding provider for an exact entry
/// occurrence. Scalar geometry remains in the ordinary structural inputs.
/// The embedding host owes the entry's declared premises independently of bytes.
/// A Granted-only input does not become Vacant or Resident by being bound.
#[derive(Debug)]
pub struct TerminalProviderStorageInput {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
    pub extent: Extent,
    pub profile: AdmittedResourceProfile,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct TerminalProviderStorageRejection {
    pub input: TerminalProviderStorageInput,
    pub reason: String,
}

#[derive(Debug)]
pub struct TerminalReturnedProviderStorage {
    pub extent: Extent,
    pub profile: AdmittedResourceProfile,
    pub bytes: Vec<u8>,
}

impl TerminalExecution<'_> {
    /// Recover a complete provider allocation carried by the normal return.
    /// Resident contents, unfinished executions, borrowed results and partial
    /// windows cannot use this vacancy-preserving handback.
    pub fn take_returned_provider_storage(
        &mut self,
        path: &[StructuralPathSegment],
    ) -> Result<TerminalReturnedProviderStorage, String> {
        let Some(crate::results::TerminalExecutionResult::Structural(result)) = &self.result else {
            return Err("provider storage handback requires a normal structural return".into());
        };
        if !self.call_stack.is_empty() || result.claims.is_empty() {
            return Err("provider storage handback requires returned ownership custody".into());
        }
        if !crate::structural_inputs::case_membership::case_projection_in_value_is_active(
            &self.program.structural_types(),
            &result.value,
            self.structural_cases.as_map(),
            path,
        )
        .map_err(|reason| format!("provider storage result case: {reason:?}"))?
        {
            return Err("provider storage result path is inactive".into());
        }
        let value = crate::custody::project_structural_value(
            &self.program.structural_types(),
            &result.value,
            path,
        )
        .map_err(|reason| format!("provider storage result path: {reason:?}"))?;
        let storage = self
            .program
            .catalogs
            .placement_lifecycle_plans
            .iter()
            .find(|plan| {
                plan.storage.carrier == value.structural_type
                    && value.qualifications.contains(&plan.storage.granted)
                    && plan
                        .storage
                        .vacant
                        .is_some_and(|vacant| value.qualifications.contains(&vacant))
            })
            .map(|plan| &plan.storage)
            .ok_or("provider storage handback requires the returned Granted/Vacant extent")?;
        if crate::custody::has_live_loan(self.reference_referents.as_map(), &value) {
            return Err("provider storage handback retains a live loan".into());
        }
        let home = StructuralRuntimePlace::from(&value);
        // The frame's live claims were consumed before its cleanup ran.
        // Only the paid return's exact active roster can hand storage back;
        // some unrelated claim in the same result is not ownership of this home.
        let returned_claim = returned_storage_claim(
            &self.program.structural_types(),
            self.structural_cases.as_map(),
            result,
            &self.returned_claims,
            &home,
        )
        .ok_or("provider storage handback has no exact returned ownership claim")?;
        let binding = self
            .provider_storage
            .descriptors
            .get(&home)
            .ok_or("provider storage result has no original allocation")?;
        let ProviderStorageSlot::Live { allocation, .. } =
            self.provider_storage.allocations.get(binding.allocation)
        else {
            return Err("provider storage result allocation has already retired".into());
        };
        if binding.resident != arena::Handle::default() {
            return Err("provider storage handback cannot discard resident custody".into());
        }
        if binding.offset != 0 || binding.length != allocation.extent().length() {
            return Err("provider storage handback does not own its complete allocation".into());
        }
        if !self
            .provider_storage_geometry_matches(
                &home,
                storage,
                allocation.extent().base(),
                allocation.extent().length(),
            )
            .map_err(|error| error.to_string())?
        {
            return Err("provider storage handback changed the bound extent geometry".into());
        }
        let handle = binding.allocation;
        let ProviderStorageSlot::Live {
            allocation,
            profile,
            ..
        } = std::mem::take(self.provider_storage.allocations.get_mut(handle))
        else {
            return Err("provider storage result allocation retired during handback".into());
        };
        self.provider_storage.allocations.free(handle);
        self.provider_storage
            .descriptors
            .retain(|_, binding| binding.allocation != handle);
        self.returned_claims.remove(returned_claim);
        let (extent, bytes) = allocation.into_parts();
        Ok(TerminalReturnedProviderStorage {
            extent,
            profile,
            bytes,
        })
    }

    pub(crate) fn is_live_provider_descriptor(
        &self,
        home: &StructuralRuntimePlace,
        binding: &ProviderStorageBinding,
    ) -> bool {
        if !self.provider_storage.binding_is_live(*binding) {
            return false;
        }
        // Borrowed helpers retain the owner in a suspended caller. Its claim
        // remains authoritative there; a descriptor association alone is not.
        self.call_stack
            .iter()
            .map(|frame| (&frame.live_claims, &frame.structural_values))
            .chain(std::iter::once((
                &self.live_claims,
                &self.structural_values,
            )))
            .any(|(claims, values)| {
                claims.values().any(|claim| {
                    let Some(value) = claim.place.and_then(|place| values.get(&place)) else {
                        return false;
                    };
                    crate::structural_inputs::case_membership::case_projection_in_value_is_active(
                        &self.program.structural_types(),
                        value,
                        self.structural_cases.as_map(),
                        &claim.path,
                    )
                    .is_ok_and(|active| active)
                        && crate::custody::project_structural_value(
                            &self.program.structural_types(),
                            value,
                            &claim.path,
                        )
                        .is_ok_and(|selected| StructuralRuntimePlace::from(&selected) == *home)
                })
            })
    }

    pub(crate) fn provider_storage_geometry_matches(
        &self,
        home: &StructuralRuntimePlace,
        storage: &terminal_psi::TerminalPlacementStorage,
        base: u64,
        length: u64,
    ) -> Result<bool, TerminalInterpretError> {
        let integer = |field| -> Result<Option<u64>, TerminalInterpretError> {
            let value = self
                .structural_scalar_fields
                .get(&StructuralScalarRuntimeField::member(home.clone(), field))
                .map(crate::scalar_storage::ScalarCell::read)
                .transpose()?;
            Ok(match value {
                Some(TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                }) => u64::try_from(value).ok(),
                _ => None,
            })
        };
        Ok(integer(storage.base)? == Some(base) && integer(storage.length)? == Some(length))
    }

    /// Supply actual backing after ordinary entry binding and before execution.
    /// A refusal leaves the execution unchanged and returns every owned input.
    /// This low-level embedding operation does not select a lifecycle provider.
    pub fn bind_provider_storage(
        &mut self,
        input: TerminalProviderStorageInput,
    ) -> Result<(), TerminalProviderStorageRejection> {
        let home = match self.provider_storage_entry_home(&input) {
            Ok(home) => home,
            Err(reason) => return Err(TerminalProviderStorageRejection { input, reason }),
        };
        let TerminalProviderStorageInput {
            argument_index,
            path,
            extent,
            profile,
            bytes,
        } = input;
        let allocation = match ProviderByteAllocation::from_admitted_provider(extent, bytes) {
            Ok(allocation) => allocation,
            Err(rejected) => {
                return Err(TerminalProviderStorageRejection {
                    input: TerminalProviderStorageInput {
                        argument_index,
                        path,
                        extent: rejected.extent,
                        profile,
                        bytes: rejected.bytes,
                    },
                    reason: rejected.reason.into(),
                });
            }
        };
        let length = allocation.extent().length();
        // No allocation handle is published until both owned records are paid.
        // Returning the original inputs on refusal leaves entry binding retryable.
        let lease = self
            .account
            .as_ref()
            .map(|account| account.reserve_storage(1, 0))
            .transpose()
            .map_err(crate::errors::TerminalInterpretError::Storage);
        let receiving = lease.and_then(|lease| {
            self.provider_storage
                .descriptors
                .insert_vacant(
                    home,
                    ProviderStorageBinding {
                        allocation: arena::Handle::default(),
                        offset: 0,
                        length,
                        resident: arena::Handle::default(),
                    },
                )
                .map(|descriptor| (lease, descriptor))
        });
        let (lease, descriptor) = match receiving {
            Ok(receiving) => receiving,
            Err(reason) => {
                let (extent, bytes) = allocation.into_parts();
                return Err(TerminalProviderStorageRejection {
                    input: TerminalProviderStorageInput {
                        argument_index,
                        path,
                        extent,
                        profile,
                        bytes,
                    },
                    reason: format!("provider storage admission: {reason:?}"),
                });
            }
        };
        descriptor.allocation =
            self.provider_storage
                .allocations
                .insert(ProviderStorageSlot::Live {
                    allocation,
                    profile,
                    _lease: lease,
                });
        Ok(())
    }

    fn provider_storage_entry_home(
        &self,
        input: &TerminalProviderStorageInput,
    ) -> Result<StructuralRuntimePlace, String> {
        if !self.entry_storage_open {
            return Err("provider entry storage must be bound before the first resume".into());
        }
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or("provider storage has no selected entry")?;
        let parameter = machine
            .structural_parameters
            .get(input.argument_index as usize)
            .ok_or("provider storage entry argument is absent")?;
        if parameter.access != StructuralAccess::Owned {
            return Err("provider storage requires an owned entry argument".into());
        }
        let argument = StructuralArgument {
            place: parameter.place,
            path: input.path.clone(),
            access: StructuralAccess::Owned,
        };
        let root = self
            .structural_values
            .get(&parameter.place)
            .ok_or("provider storage entry has no actual argument")?;
        if !crate::structural_inputs::case_membership::case_projection_in_value_is_active(
            &self.program.structural_types(),
            root,
            self.structural_cases.as_map(),
            &input.path,
        )
        .map_err(|reason| format!("provider storage entry case: {reason:?}"))?
        {
            return Err("provider storage entry path is inactive".into());
        }
        let value = crate::custody::resolve_single_structural_argument(
            &self.program.structural_types(),
            &self.structural_values,
            &argument,
        )
        .map_err(|reason| format!("provider storage lost its entry projection: {reason:?}"))?;
        let home = StructuralRuntimePlace::from(&value);
        if self.provider_storage.descriptors.contains_key(&home) {
            return Err("provider storage entry occurrence is already bound".into());
        }
        let mut roles = self
            .program
            .catalogs
            .placement_lifecycle_plans
            .iter()
            .map(|plan| &plan.storage)
            .filter(|storage| storage.carrier == value.structural_type);
        let storage = roles
            .next()
            .ok_or("provider storage has no retained extent roles")?;
        if roles.any(|other| {
            other.base != storage.base
                || other.length != storage.length
                || other.granted != storage.granted
                || matches!((other.vacant, storage.vacant), (Some(left), Some(right)) if left != right)
        }) {
            return Err("provider storage has conflicting retained extent roles".into());
        }
        let (declared, _) = terminal_psi::StructuralPathQualification::select(
            &parameter.qualifications,
            &parameter.projected_qualifications,
            &input.path,
        );
        // Backing admission retains this input's already checked contract.
        // A different input's lifecycle demand cannot require vacancy here;
        // initialization checks that qualification at its own use.
        if !declared.contains(&storage.granted)
            || !declared
                .iter()
                .all(|domain| value.qualifications.contains(domain))
        {
            return Err("provider storage entry lacks its declared granted extent premises".into());
        }
        if !self
            .live_claims
            .values()
            .any(|claim| claim.place == Some(parameter.place) && claim.path == input.path)
        {
            return Err("provider storage entry has no exact live ownership claim".into());
        }
        if !self
            .provider_storage_geometry_matches(
                &home,
                storage,
                input.extent.base(),
                input.extent.length(),
            )
            .map_err(|error| error.to_string())?
        {
            return Err("provider storage differs from the actual entry geometry".into());
        }
        let loan = input
            .extent
            .loan(0, input.extent.length())
            .map_err(|reason| format!("provider storage range: {reason}"))?;
        input
            .profile
            .restrict_to_loan(&loan)
            .map_err(|reason| format!("provider storage profile: {reason}"))?;
        for (_, existing) in self.provider_storage.allocations.iter() {
            if let ProviderStorageSlot::Live { allocation, .. } = existing {
                let existing = allocation.extent();
                if existing.origin() == input.extent.origin()
                    && existing.address_space() == input.extent.address_space()
                    && existing.era() == input.extent.era()
                    && existing.base() < input.extent.end()
                    && input.extent.base() < existing.end()
                {
                    return Err("provider storage overlaps an already bound issuance".into());
                }
            }
        }
        Ok(home)
    }
}

/// Match the already settled return receipt, never a surviving frame binding.
/// Logical case paths are resolved before comparing physical homes.
fn returned_storage_claim(
    types: &crate::program::ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    cases: &std::collections::BTreeMap<
        StructuralRuntimePlace,
        crate::structural_inputs::case_membership::StructuralCaseContents,
    >,
    result: &crate::results::TerminalStructuralResult,
    returned_claims: &[terminal_psi::StructuralResultClaimBinding],
    home: &StructuralRuntimePlace,
) -> Option<usize> {
    returned_claims.iter().position(|binding| {
        result.claims.contains(&binding.claim)
            && crate::structural_inputs::case_membership::case_projection_in_value_is_active(
                types,
                &result.value,
                cases,
                &binding.path,
            )
            .is_ok_and(|active| active)
            && crate::custody::project_structural_value(types, &result.value, &binding.path)
                .is_ok_and(|selected| StructuralRuntimePlace::from(&selected) == *home)
    })
}
