/*
    Provider-backed storage slots, value-image encoding and decoding, lifecycle
    transitions, relocation, and profile supply in the Terminal Psi interpreter.

    We manage host-supplied and interpreter-allocated provider storage slots,
    validate layout geometry and field access, and preserve image custody across
    relocation and extraction.
*/

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::engine::execution::TerminalExecution;
use crate::storage::record::owned_value::StagedOwnedValue;
use crate::storage::runtime_rows::RuntimeRows;
use access_plans::{AdmittedResourceProfile, InitializationResourceCompatibility};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use extents::Extent;
use terminal_psi::TerminalAccessLifecyclePlan;

pub(crate) mod backing {
    use crate::TerminalEffectRejection;
    use std::ops::{Deref, DerefMut};
    use std::sync::{Arc, Mutex, MutexGuard};

    #[derive(Debug)]
    pub(crate) enum ProviderBytes {
        Owned(Vec<u8>),
        Host(TerminalProviderBacking),
    }

    pub(crate) enum ProviderBytesGuard<'a> {
        Owned(&'a mut [u8]),
        Host(ProviderBackingGuard<'a>),
    }

    pub(crate) enum ProviderBytesRead<'a> {
        Owned(&'a [u8]),
        Host(ProviderBackingGuard<'a>),
    }

    impl ProviderBytes {
        pub(crate) fn take(&mut self) -> Result<Vec<u8>, TerminalEffectRejection> {
            match self {
                Self::Owned(bytes) => Ok(std::mem::take(bytes)),
                Self::Host(backing) => backing.retire(),
            }
        }

        pub(crate) fn read(&self) -> Result<ProviderBytesRead<'_>, TerminalEffectRejection> {
            Ok(match self {
                Self::Owned(bytes) => ProviderBytesRead::Owned(bytes),
                Self::Host(backing) => ProviderBytesRead::Host(backing.lock()?),
            })
        }

        pub(crate) fn write(&mut self) -> Result<ProviderBytesGuard<'_>, TerminalEffectRejection> {
            Ok(match self {
                Self::Owned(bytes) => ProviderBytesGuard::Owned(bytes),
                Self::Host(backing) => ProviderBytesGuard::Host(backing.lock()?),
            })
        }
    }

    impl Deref for ProviderBytesRead<'_> {
        type Target = [u8];
        fn deref(&self) -> &[u8] {
            match self {
                Self::Owned(bytes) => bytes,
                Self::Host(guard) => guard,
            }
        }
    }

    impl Deref for ProviderBytesGuard<'_> {
        type Target = [u8];
        fn deref(&self) -> &[u8] {
            match self {
                Self::Owned(bytes) => bytes,
                Self::Host(guard) => guard,
            }
        }
    }

    impl DerefMut for ProviderBytesGuard<'_> {
        fn deref_mut(&mut self) -> &mut [u8] {
            match self {
                Self::Owned(bytes) => bytes,
                Self::Host(guard) => guard,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct TerminalProviderBacking {
        bytes: Arc<Mutex<ProviderBackingState>>,
    }

    #[derive(Debug)]
    struct ProviderBackingState {
        bytes: Vec<u8>,
        retired: bool,
    }

    impl TerminalProviderBacking {
        pub fn new(bytes: Vec<u8>) -> Self {
            Self {
                bytes: Arc::new(Mutex::new(ProviderBackingState {
                    bytes,
                    retired: false,
                })),
            }
        }

        pub fn byte_count(&self) -> Result<usize, TerminalEffectRejection> {
            Ok(self.lock()?.len())
        }

        pub fn retire(&self) -> Result<Vec<u8>, TerminalEffectRejection> {
            let mut state = self
                .bytes
                .lock()
                .map_err(|_| TerminalEffectRejection::new("provider backing lock was poisoned"))?;
            if state.retired {
                return Err(TerminalEffectRejection::new(
                    "provider backing was already retired",
                ));
            }
            state.retired = true;
            Ok(std::mem::take(&mut state.bytes))
        }

        pub(crate) fn lock(&self) -> Result<ProviderBackingGuard<'_>, TerminalEffectRejection> {
            let guard = self
                .bytes
                .lock()
                .map_err(|_| TerminalEffectRejection::new("provider backing lock was poisoned"))?;
            if guard.retired {
                return Err(TerminalEffectRejection::new("provider backing was retired"));
            }
            Ok(ProviderBackingGuard { guard })
        }

        pub(crate) fn same_allocation(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.bytes, &other.bytes)
        }
    }

    pub(crate) struct ProviderBackingGuard<'a> {
        guard: MutexGuard<'a, ProviderBackingState>,
    }

    impl Deref for ProviderBackingGuard<'_> {
        type Target = [u8];
        fn deref(&self) -> &[u8] {
            &self.guard.bytes
        }
    }

    impl DerefMut for ProviderBackingGuard<'_> {
        fn deref_mut(&mut self) -> &mut [u8] {
            &mut self.guard.bytes
        }
    }

    #[cfg(test)]
    mod tests {
        use super::TerminalProviderBacking;

        #[test]
        fn handles_share_writes_and_retirement() {
            let original = TerminalProviderBacking::new(vec![7, 11, 13]);
            let retained = original.clone();
            assert!(original.same_allocation(&retained));
            retained.lock().unwrap()[1] = 23;
            assert_eq!(&*original.lock().unwrap(), &[7, 23, 13]);
            assert_eq!(original.retire().unwrap(), vec![7, 23, 13]);
            assert!(retained.lock().is_err());
            assert!(retained.retire().is_err());
        }

        #[test]
        fn equal_bytes_are_not_the_same_allocation() {
            let first = TerminalProviderBacking::new(vec![0; 8]);
            let second = TerminalProviderBacking::new(vec![0; 8]);
            assert!(!first.same_allocation(&second));
            first.retire().unwrap();
            assert_eq!(second.lock().unwrap().len(), 8);
        }
    }
}
use backing::ProviderBytes;
pub use backing::TerminalProviderBacking;
pub(crate) mod entry {
    use super::{ProviderByteAllocation, ProviderStorageBinding, ProviderStorageSlot};
    use crate::boundary::errors::TerminalInterpretError;
    use crate::boundary::structural_inputs::case_membership::{
        StructuralCaseContents, case_projection_in_value_is_active,
    };
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    };
    use crate::engine::custody::project_structural_value;
    use crate::engine::execution::TerminalExecution;
    use access_plans::AdmittedResourceProfile;
    use extents::Extent;
    use std::iter::once;
    use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment};

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
        pub fn take_returned_provider_storage(
            &mut self,
            path: &[StructuralPathSegment],
        ) -> Result<TerminalReturnedProviderStorage, String> {
            let Some(crate::boundary::results::TerminalExecutionResult::Structural(result)) =
                &self.result
            else {
                return Err("provider storage handback requires a normal structural return".into());
            };
            if !self.call_stack.is_empty() || result.claims.is_empty() {
                return Err("provider storage handback requires returned ownership custody".into());
            }
            if !case_projection_in_value_is_active(
                &self.structural_types(),
                &result.value,
                self.structural_cases.as_map(),
                path,
            )
            .map_err(|reason| format!("provider storage result case: {reason:?}"))?
            {
                return Err("provider storage result path is inactive".into());
            }
            let value = project_structural_value(&self.structural_types(), &result.value, path)
                .map_err(|reason| format!("provider storage result path: {reason:?}"))?;
            let storage = self
                .program
                .access_lifecycle_plans()
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
            if crate::engine::custody::has_live_loan(self.reference_referents.as_map(), &value) {
                return Err("provider storage handback retains a live loan".into());
            }
            let home = StructuralRuntimePlace::from(&value);
            let returned_claim = returned_storage_claim(
                &self.structural_types(),
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
                return Err(
                    "provider storage handback does not own its complete allocation".into(),
                );
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
            let ProviderStorageSlot::Live { allocation, .. } =
                self.provider_storage.allocations.get_mut(handle)
            else {
                return Err("provider storage result allocation retired during handback".into());
            };
            let bytes = allocation.bytes.take().map_err(|reason| reason.reason)?;
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
            Ok(TerminalReturnedProviderStorage {
                extent: allocation.extent,
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
            self.is_live_provider_owner(home)
        }

        pub(crate) fn is_live_provider_owner(&self, home: &StructuralRuntimePlace) -> bool {
            self.call_stack
                .iter()
                .map(|frame| (&frame.live_claims, &frame.structural_values))
                .chain(once((&self.live_claims, &self.structural_values)))
                .any(|(claims, values)| {
                    claims.values().any(|claim| {
                        let Some(value) = claim.place.and_then(|place| values.get(&place)) else {
                            return false;
                        };
                        case_projection_in_value_is_active(
                            &self.structural_types(),
                            value,
                            self.structural_cases.as_map(),
                            &claim.path,
                        )
                        .is_ok_and(|active| active)
                            && project_structural_value(
                                &self.structural_types(),
                                value,
                                &claim.path,
                            )
                            .is_ok_and(|selected| {
                                if selected.opaque_identity != home.opaque_identity {
                                    return false;
                                }
                                let Some(relative) =
                                    home.path.strip_prefix(selected.path.as_slice())
                                else {
                                    return false;
                                };
                                terminal_semantics::projection_stays_in_inline_storage(
                                    self.structural_types().values(),
                                    selected.structural_type,
                                    relative,
                                ) && case_projection_in_value_is_active(
                                    &self.structural_types(),
                                    &selected,
                                    self.structural_cases.as_map(),
                                    relative,
                                )
                                .is_ok_and(|active| active)
                                    && project_structural_value(
                                        &self.structural_types(),
                                        &selected,
                                        relative,
                                    )
                                    .is_ok_and(|backing| {
                                        StructuralRuntimePlace::from(&backing) == *home
                                    })
                            })
                    })
                })
        }

        pub(crate) fn provider_storage_geometry_matches(
            &self,
            home: &StructuralRuntimePlace,
            storage: &terminal_psi::TerminalAccessStorage,
            base: u64,
            length: u64,
        ) -> Result<bool, TerminalInterpretError> {
            Ok(self.provider_storage_geometry(home, storage)? == Some((base, length)))
        }

        pub(crate) fn provider_storage_geometry(
            &self,
            home: &StructuralRuntimePlace,
            storage: &terminal_psi::TerminalAccessStorage,
        ) -> Result<Option<(u64, u64)>, TerminalInterpretError> {
            let integer = |field| -> Result<Option<u64>, TerminalInterpretError> {
                let value = self
                    .structural_scalar_fields
                    .get(&StructuralScalarRuntimeField::member(home.clone(), field))
                    .map(crate::storage::scalar_storage::ScalarCell::read)
                    .transpose()?;
                Ok(match value {
                    Some(TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        ..
                    }) => u64::try_from(value).ok(),
                    _ => None,
                })
            };
            Ok(integer(storage.base)?.zip(integer(storage.length)?))
        }

        pub fn bind_provider_storage(
            &mut self,
            input: TerminalProviderStorageInput,
        ) -> Result<(), TerminalProviderStorageRejection> {
            let home = match self.provider_storage_entry_home(&input) {
                Ok(home) => home,
                Err(reason) => return Err(TerminalProviderStorageRejection { input, reason }),
            };
            let length = input.extent.length();
            let lease = self
                .account
                .as_ref()
                .map(|account| account.reserve_storage(1, 0))
                .transpose()
                .map_err(crate::boundary::errors::TerminalInterpretError::Storage);
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
                    return Err(TerminalProviderStorageRejection {
                        input,
                        reason: format!("provider storage admission: {reason:?}"),
                    });
                }
            };
            descriptor.allocation =
                self.provider_storage
                    .allocations
                    .insert(ProviderStorageSlot::Live {
                        allocation: ProviderByteAllocation {
                            extent: input.extent,
                            bytes: super::backing::ProviderBytes::Owned(input.bytes),
                        },
                        profile: input.profile,
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
            if input.extent.provider_issuance().is_none() {
                return Err(
                    "provider byte storage requires an admitted provider-issued extent".into(),
                );
            }
            if usize::try_from(input.extent.length()).ok() != Some(input.bytes.len()) {
                return Err("provider byte storage must cover the exact complete extent".into());
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
            if !case_projection_in_value_is_active(
                &self.structural_types(),
                root,
                self.structural_cases.as_map(),
                &input.path,
            )
            .map_err(|reason| format!("provider storage entry case: {reason:?}"))?
            {
                return Err("provider storage entry path is inactive".into());
            }
            let value = crate::engine::custody::resolve_single_structural_argument(
                &self.structural_types(),
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
                .access_lifecycle_plans()
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
            if !declared.contains(&storage.granted)
                || !declared
                    .iter()
                    .all(|domain| value.qualifications.contains(domain))
            {
                return Err(
                    "provider storage entry lacks its declared granted extent premises".into(),
                );
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

    fn returned_storage_claim(
        types: &crate::embedding::program::ProgramDeclarations<
            '_,
            terminal_psi::StructuralTypeDeclaration,
        >,
        cases: &std::collections::BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
        result: &crate::boundary::results::TerminalStructuralResult,
        returned_claims: &[terminal_psi::StructuralResultClaimBinding],
        home: &StructuralRuntimePlace,
    ) -> Option<usize> {
        returned_claims.iter().position(|binding| {
            result.claims.contains(&binding.claim)
                && case_projection_in_value_is_active(types, &result.value, cases, &binding.path)
                    .is_ok_and(|active| active)
                && project_structural_value(types, &result.value, &binding.path)
                    .is_ok_and(|selected| StructuralRuntimePlace::from(&selected) == *home)
        })
    }
}
pub(crate) mod supply {
    use super::backing::ProviderBytes;
    use super::{
        ProviderByteAllocation, ProviderStorageBinding, ProviderStorageSlot,
        TerminalProviderBacking,
    };
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::engine::execution::TerminalExecution;
    use crate::{TerminalEffectRejection, TerminalInterpretError};
    use access_plans::AdmittedResourceProfile;
    use extents::Extent;
    use semantic_vocabulary::{BoundaryMachineId, MachineId};

    pub struct TerminalProviderStorageRequest {
        pub(crate) base: u64,
        pub(crate) length: u64,
        pub(crate) boundary: BoundaryMachineId,
        pub(crate) provider: MachineId,
    }

    impl TerminalProviderStorageRequest {
        pub fn base(&self) -> u64 {
            self.base
        }
        pub fn length(&self) -> u64 {
            self.length
        }
        pub fn boundary(&self) -> BoundaryMachineId {
            self.boundary
        }
        pub fn provider(&self) -> MachineId {
            self.provider
        }
    }

    pub struct TerminalProviderStorageSupply {
        pub extent: Extent,
        pub profile: AdmittedResourceProfile,
        pub backing: TerminalProviderBacking,
    }

    pub trait TerminalProviderStorageResolver {
        fn resolve(
            &self,
            request: &TerminalProviderStorageRequest,
        ) -> Result<Option<TerminalProviderStorageSupply>, TerminalEffectRejection>;
    }

    impl TerminalExecution<'_> {
        pub(crate) fn bind_dynamic_provider_storage(
            &mut self,
            home: StructuralRuntimePlace,
            request: &TerminalProviderStorageRequest,
            supply: TerminalProviderStorageSupply,
        ) -> Result<ProviderStorageBinding, TerminalInterpretError> {
            let refusal = |reason| TerminalInterpretError::ContentTransformationMismatch(reason);
            let end = request
                .base
                .checked_add(request.length)
                .ok_or_else(|| refusal("provider storage request overflows its address space"))?;
            if supply.extent.provider_issuance().is_none()
                || request.base < supply.extent.base()
                || end > supply.extent.end()
                || usize::try_from(supply.extent.length()).ok()
                    != Some(
                        supply
                            .backing
                            .lock()
                            .map_err(TerminalInterpretError::ResidentStorage)?
                            .len(),
                    )
            {
                return Err(refusal(
                    "dynamic provider storage differs from its actual allocation",
                ));
            }
            let offset = request.base - supply.extent.base();
            let loan = supply
                .extent
                .loan(offset, request.length)
                .map_err(|error| TerminalInterpretError::AccessReference(error.to_string()))?;
            supply
                .profile
                .restrict_to_loan(&loan)
                .map_err(|error| TerminalInterpretError::AccessReference(error.to_string()))?;

            let mut existing_handle = None;
            for (handle, row) in self.provider_storage.allocations.iter() {
                let ProviderStorageSlot::Live {
                    allocation,
                    profile,
                    ..
                } = row
                else {
                    continue;
                };
                if let ProviderBytes::Host(backing) = &allocation.bytes
                    && backing.same_allocation(&supply.backing)
                {
                    if allocation.extent != supply.extent
                        || profile.receipt() != supply.profile.receipt()
                        || profile.profile() != supply.profile.profile()
                    {
                        return Err(refusal(
                            "provider changed the admission of an existing allocation",
                        ));
                    }
                    existing_handle = Some(handle);
                } else if allocation.extent.address_space() == supply.extent.address_space()
                    && allocation.extent.era() == supply.extent.era()
                    && allocation.extent.base() < supply.extent.end()
                    && supply.extent.base() < allocation.extent.end()
                {
                    return Err(refusal(
                        "different provider allocations overlap in one address space",
                    ));
                }
            }
            if let Some(allocation) = existing_handle {
                for (other, binding) in self.provider_storage.descriptors.iter() {
                    if binding.allocation == allocation
                        && *other != home
                        && binding.offset < offset + request.length
                        && offset < binding.offset + binding.length
                        && self.is_live_provider_descriptor(other, binding)
                    {
                        return Err(refusal(
                            "dynamic provider grant overlaps another live owner",
                        ));
                    }
                }
            }
            let lease = self
                .account
                .as_ref()
                .filter(|_| existing_handle.is_none())
                .map(|account| account.reserve_storage(1, 0))
                .transpose()
                .map_err(TerminalInterpretError::Storage)?;
            let binding = self.provider_storage.descriptors.insert_vacant(
                home,
                ProviderStorageBinding {
                    allocation: existing_handle.unwrap_or_default(),
                    offset,
                    length: request.length,
                    resident: arena::Handle::default(),
                },
            )?;
            if existing_handle.is_none() {
                binding.allocation =
                    self.provider_storage
                        .allocations
                        .insert(ProviderStorageSlot::Live {
                            allocation: ProviderByteAllocation {
                                extent: supply.extent,
                                bytes: ProviderBytes::Host(supply.backing),
                            },
                            profile: supply.profile,
                            _lease: lease,
                        });
            }
            Ok(*binding)
        }
    }
}
pub use supply::{
    TerminalProviderStorageRequest, TerminalProviderStorageResolver, TerminalProviderStorageSupply,
};
pub(crate) mod empty_run {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    };
    use crate::engine::custody::bind_result_claim;
    use crate::engine::execution::{TerminalExecution, operation_kind};
    use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
    use semantic_vocabulary::{IntegerValue, StructuralFieldId};
    use terminal_psi::{
        BoundaryMachineDeclaration, Operation, OperationKind, StructuralPathSegment,
    };

    impl TerminalExecution<'_> {
        pub(crate) fn release_empty_run(
            &mut self,
            operation: &Operation,
            boundary: &BoundaryMachineDeclaration,
        ) -> Result<(), TerminalInterpretError> {
            operation_kind!(operation => BoundaryCall {
                structural_arguments, completion_receipts, ..
            });
            let [source] = structural_arguments.as_slice() else {
                return Err(malformed());
            };
            let program = std::sync::Arc::clone(&self.program);
            let contract = terminal_semantics::empty_run_release_contract(
                &program.catalogs.structural_types,
                boundary,
            )
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
            let result = operation.result.structural().ok_or_else(malformed)?;
            if self.result_place_in_use(result.place)
                || !result.qualification_establishments.is_empty()
                || !self
                    .loan_completions_for_operation(operation.id)?
                    .is_empty()
                || result.structural_type != contract.result.structural_type
                || result.multiplicity != contract.result.multiplicity
                || result.qualifications != contract.result.qualifications
                || result.projected_qualifications != contract.result.projected_qualifications
            {
                return Err(malformed());
            }
            let owner = self.resolve_live_projection(source.place, &source.path, source.access)?;
            if crate::engine::custody::has_live_loan(self.reference_referents.as_map(), &owner) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let mut backing_source = source.clone();
            backing_source
                .path
                .push(StructuralPathSegment::Field(contract.storage));
            let child = ConstructorValue::resolve(self, &backing_source)?;
            let ConstructorValue::Access(backing) = &child else {
                return Err(malformed());
            };
            for requirement in &boundary.requires {
                let mut path = source.path.clone();
                path.extend_from_slice(&requirement.path);
                let subject = self.resolve_live_projection(source.place, &path, source.access)?;
                if requirement.argument_index != 0
                    || !subject.qualifications.contains(&requirement.domain)
                {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            if !self.empty_run_zero_field(&StructuralRuntimePlace::from(&owner), contract.count)?
                || !self
                    .empty_run_zero_field(&StructuralRuntimePlace::from(backing), contract.length)?
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "empty-run release requires zero cells and zero occupied bytes",
                ));
            }
            if self
                .provider_storage
                .descriptors
                .get(&StructuralRuntimePlace::from(backing))
                .is_some_and(|binding| binding.resident != arena::Handle::default())
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "empty-run release cannot discard retained resident custody",
                ));
            }
            let mut identity_cursor = self.local_structural_identities.cursor();
            let parameter = &boundary.structural_parameters[0];
            let retained = self.stage_owned_value(
                source,
                parameter.structural_type,
                parameter.multiplicity,
                &mut identity_cursor,
            )?;
            if retained.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()]
                || completion_receipts.len() != 1
                || backing
                    .qualifications
                    .iter()
                    .any(|domain| !result.qualifications.contains(domain))
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let mut claims = crate::engine::custody::complete_claims(
                &self.live_claims,
                structural_arguments,
                completion_receipts,
                &boundary.structural_parameters,
            )?;
            let [binding] = result.claims.as_slice() else {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            };
            if !binding.path.is_empty() {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
            let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
            value.qualifications = result.qualifications.clone();
            value.projected_qualifications = backing.projected_qualifications.clone();
            let mut storage = ConstructorStorage::new(self.account.as_ref());
            storage.stage(
                self,
                &backing_source,
                &child,
                &StructuralRuntimePlace::from(&value),
                true,
            )?;
            storage.validate_projected_memberships(
                self,
                &value,
                &result.projected_qualifications,
            )?;
            let mut frontier = self.live_affine_frontier.clone();
            let mut consumed = Vec::new();
            if !self.stage_constructor_operand(source, &mut frontier, &mut consumed)? {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            self.stage_result_affine_frontier(result, &mut frontier, &|region| {
                storage.case_at_projection(self, &value, &region.path)
            })?;
            let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
            storage.into_detached()?.install(self)?;
            self.structural_values.insert(result.place, value)?;
            retirement.commit(self)?;
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            Ok(())
        }

        fn empty_run_zero_field(
            &self,
            home: &StructuralRuntimePlace,
            field: StructuralFieldId,
        ) -> Result<bool, TerminalInterpretError> {
            let value = self
                .structural_scalar_fields
                .get(&StructuralScalarRuntimeField::member(home.clone(), field))
                .map(crate::storage::scalar_storage::ScalarCell::read)
                .transpose()?;
            Ok(matches!(
                value,
                Some(TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(0),
                    ..
                })
            ))
        }
    }
}
pub(crate) mod extraction {
    use super::ProviderStorageBinding;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::engine::custody::bind_result_claim;
    use crate::engine::execution::{TerminalExecution, operation_kind};
    use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
    use access_plans::{AccessPlanDiagnostic, StableCapability};
    use std::collections::BTreeSet;
    use terminal_psi::{
        BoundaryMachineDeclaration, Operation, OperationKind, StructuralAffineDiscard,
        StructuralPathQualification, StructuralPathSegment, TerminalAccessLifecyclePlan,
    };
    use terminal_semantics::{AccessResultContract, access_result_contract};

    impl TerminalExecution<'_> {
        pub(crate) fn extract_access_resident(
            &mut self,
            operation: &Operation,
            boundary: &BoundaryMachineDeclaration,
            plan: &TerminalAccessLifecyclePlan,
        ) -> Result<(), TerminalInterpretError> {
            let (vacant, resident_domain) = plan
                .storage
                .owned_content()
                .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
            operation_kind!(operation => BoundaryCall {
                structural_arguments, completion_receipts, ..
            });
            let [source, profile_argument] = structural_arguments.as_slice() else {
                return Err(malformed());
            };
            let [parameter, _] = boundary.structural_parameters.as_slice() else {
                return Err(malformed());
            };
            let result = operation.result.structural().ok_or_else(malformed)?;
            if self.result_place_in_use(result.place)
                || !result.qualification_establishments.is_empty()
                || !self
                    .loan_completions_for_operation(operation.id)?
                    .is_empty()
            {
                return Err(malformed());
            }
            let program = std::sync::Arc::clone(&self.program);
            let AccessResultContract::MoveOut {
                result: declaration,
                moved,
            } = access_result_contract(&program.catalogs, plan)
                .map_err(TerminalInterpretError::ContentTransformationMismatch)?
            else {
                return Err(malformed());
            };
            if result.structural_type != declaration.structural_type
                || result.multiplicity != declaration.multiplicity
                || result.qualifications != declaration.qualifications
                || result.projected_qualifications != declaration.projected_qualifications
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "extraction result differs from its boundary contract",
                ));
            }
            let [extent_field, value_field] = moved.fields.as_slice() else {
                return Err(malformed());
            };
            let mut identity_cursor = self.local_structural_identities.cursor();
            let owner = self.stage_owned_value(
                source,
                plan.storage.owner_carrier(),
                parameter.multiplicity,
                &mut identity_cursor,
            )?;
            if owner.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()]
                || !owner.value.qualifications.contains(&resident_domain)
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let mut backing_source = source.clone();
            backing_source.path.extend(
                plan.storage
                    .owner_storage_path(&program.catalogs.structural_types)
                    .ok_or_else(malformed)?,
            );
            let child = ConstructorValue::resolve(self, &backing_source)?;
            let ConstructorValue::Access(original_extent) = &child else {
                return Err(malformed());
            };
            if !original_extent
                .qualifications
                .contains(&plan.storage.granted)
                || original_extent.qualifications.contains(&vacant)
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let observed = self.resolve_live_projection(
                profile_argument.place,
                &profile_argument.path,
                profile_argument.access,
            )?;
            let (observed_binding, observed_profile) =
                self.lifecycle_profile(plan, 0, original_extent, &observed)?;
            let home = StructuralRuntimePlace::from(original_extent);
            let binding = self
                .provider_storage
                .descriptors
                .get(&home)
                .copied()
                .ok_or(TerminalInterpretError::ContentTransformationMismatch(
                    "extraction has no actual provider allocation",
                ))?;
            if binding.resident == arena::Handle::default()
                || binding != observed_binding
                || !self.is_live_provider_descriptor(&home, &binding)
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "extraction has no live resident custody",
                ));
            }
            let (initialized, custody, compatibility) =
                self.live_resident_custody(binding.resident)?;
            if initialized.schema != plan.schema
                || custody.value.structural_type != plan.schema
                || initialized.schema_identity != plan.schema_identity
                || initialized.policy_identity != plan.policy_identity
                || initialized.policy_plan_machine_identity != plan.policy_plan_machine_identity
                || !initialized.storage.same_custody(&plan.storage)
                || initialized.placement != plan.placement
                || initialized.image_fields != plan.image_fields
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "extraction changed the original resident policy or schema",
                ));
            }
            let (allocation, profile, base) =
                self.provider_storage.live_provider_allocation(&binding)?;
            if !self.provider_storage_geometry_matches(
                &home,
                &plan.storage,
                base,
                binding.length,
            )? {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "extraction changed the original allocation geometry",
                ));
            }
            let required = compatibility.byte_length();
            if plan.placement.layout().size != Some(required) || required > binding.length {
                return Err(malformed());
            }
            let loan = allocation
                .extent
                .loan(binding.offset, binding.length)
                .map_err(|diagnostic| TerminalInterpretError::ResourceProfile {
                    boundary: plan.boundary,
                    diagnostic: AccessPlanDiagnostic(format!(
                        "extraction extent loan: {diagnostic}"
                    )),
                })?;
            let supplied = profile.restrict_to_loan(&loan).map_err(|diagnostic| {
                TerminalInterpretError::ResourceProfile {
                    boundary: plan.boundary,
                    diagnostic,
                }
            })?;
            if supplied.identity() != observed_profile.identity() {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "extraction profile differs from the original admitted allocation supply",
                ));
            }
            let mut covered = 0;
            for region in supplied.regions() {
                if covered == required {
                    break;
                }
                if region.offset != covered
                    || !matches!(
                        region.stable,
                        StableCapability::Read | StableCapability::ReadWrite
                    )
                {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "whole resident extraction requires complete Stable read supply",
                    ));
                }
                covered = (region.offset + region.length).min(required);
            }
            if covered != required {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "resident extraction image has incomplete read supply",
                ));
            }
            let start = usize::try_from(binding.offset).map_err(|_| malformed())?;
            let end = binding
                .offset
                .checked_add(required)
                .and_then(|end| usize::try_from(end).ok())
                .ok_or_else(malformed)?;
            let backing = allocation
                .bytes
                .read()
                .map_err(TerminalInterpretError::ResidentStorage)?;
            let image = backing.get(start..end).ok_or_else(malformed)?;
            let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
            let extent_path = vec![
                StructuralPathSegment::Case(moved.id),
                StructuralPathSegment::Field(extent_field.identity),
            ];
            let value_path = vec![
                StructuralPathSegment::Case(moved.id),
                StructuralPathSegment::Field(value_field.identity),
            ];
            let extent_home = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: extent_path.clone(),
            };
            let value_home = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: value_path.clone(),
            };
            let decoded = super::value_image::restore(self, plan, image, &value_home)?;
            drop(backing);
            let mut storage = ConstructorStorage::new(self.account.as_ref());
            storage.stage_case(
                StructuralRuntimePlace::from(&value),
                result.structural_type,
                moved.id,
            )?;
            storage.stage(self, &backing_source, &child, &extent_home, true)?;
            storage.take_provider_descriptor(&extent_home, binding)?;
            storage.stage_resident_image(self, custody, &value_home, decoded)?;
            let mut qualifications = original_extent.qualifications.clone();
            qualifications.retain(|domain| *domain != resident_domain);
            qualifications.push(vacant);
            qualifications.sort();
            StructuralPathQualification::append_under(
                &qualifications,
                &original_extent.projected_qualifications,
                &extent_path,
                &mut value.projected_qualifications,
            );
            StructuralPathQualification::append_under(
                &custody.value.qualifications,
                &custody.value.projected_qualifications,
                &value_path,
                &mut value.projected_qualifications,
            );
            storage.validate_projected_memberships(
                self,
                &value,
                &result.projected_qualifications,
            )?;
            let selected = |region: &StructuralAffineDiscard| {
                storage.case_at_projection(self, &value, &region.path)
            };
            let mut expected_claims = BTreeSet::from([extent_path.clone()]);
            for path in &custody.claim_paths {
                let mut destination = value_path.clone();
                destination.extend_from_slice(path);
                if !expected_claims.insert(destination) {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            let mut claims = crate::engine::custody::complete_claims(
                &self.live_claims,
                structural_arguments,
                completion_receipts,
                &boundary.structural_parameters,
            )?;
            for binding in &result.claims {
                if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                    continue;
                }
                if !expected_claims.remove(&binding.path) {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
            }
            if !expected_claims.is_empty() {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let mut frontier = self.live_affine_frontier.clone();
            let mut consumed = Vec::new();
            if !self.stage_constructor_operand(source, &mut frontier, &mut consumed)? {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
            let actual_affine = frontier
                .iter()
                .filter(|region| region.place == result.place)
                .filter_map(|region| {
                    region
                        .path
                        .strip_prefix(value_path.as_slice())
                        .map(|path| (path.to_vec(), region.structural_type))
                })
                .collect::<BTreeSet<_>>();
            let expected_affine = custody
                .affine_regions
                .iter()
                .map(|region| (region.path.clone(), region.structural_type))
                .collect::<BTreeSet<_>>();
            if actual_affine != expected_affine {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
            let storage = storage.into_detached()?;
            storage.install(self)?;
            self.structural_values.insert(result.place, value)?;
            self.provider_storage.descriptors.insert_vacant(
                extent_home,
                ProviderStorageBinding {
                    resident: arena::Handle::default(),
                    ..binding
                },
            )?;
            let _original =
                std::mem::take(self.provider_storage.residents.get_mut(binding.resident));
            self.provider_storage.residents.free(binding.resident);
            retirement.commit(self)?;
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            Ok(())
        }
    }
}
pub(crate) mod field_access {
    use super::{ProviderStorageBinding, ProviderStorageSlot};
    use crate::boundary::effects::results::TerminalEffectResult;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalScalarValue};
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use access_plans::{AccessOperation, BorrowPolarity, EffectiveSupplyKind, ObservationModel};
    use layout_plans::{
        AggregateFieldSchema, ByteOrder, LayoutPlacementReport, LayoutPlanReport, ScalarFieldValue,
    };
    use terminal_psi::{
        StructuralAccess, StructuralPathSegment, StructuralTypeShape, TerminalAccessFieldPlan,
        TerminalAccessViewPlan, TerminalAccessorKind,
    };
    use terminal_semantics::reference_storage_byte_width;

    pub(crate) enum ProviderFieldContent<'view> {
        Resident,
        Borrowed(&'view access_plans::AccessResourceCompatibility),
    }

    impl TerminalExecution<'_> {
        pub(crate) fn execute_provider_field(
            &mut self,
            binding: ProviderStorageBinding,
            content: ProviderFieldContent<'_>,
            plan: &TerminalAccessViewPlan,
            field: &TerminalAccessFieldPlan,
            projection: &[StructuralPathSegment],
            kind: TerminalAccessorKind,
            source_access: StructuralAccess,
            arguments: &[TerminalScalarValue],
        ) -> Result<TerminalEffectResult, TerminalInterpretError> {
            if !self.provider_storage.binding_is_live(binding) {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "resident field lost its original allocation",
                ));
            }
            let access = plan.placement.access();
            let entry = access
                .plan()
                .entries()
                .get(field.access_slot as usize)
                .ok_or_else(malformed)?;
            let operation = match kind {
                TerminalAccessorKind::Read => AccessOperation::Read,
                TerminalAccessorKind::Write => AccessOperation::Write,
                TerminalAccessorKind::Take => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "resident take requires partial custody transfer",
                    ));
                }
            };
            let source_borrow = match source_access {
                StructuralAccess::SharedBorrow => BorrowPolarity::Shared,
                StructuralAccess::MutableBorrow => BorrowPolarity::Exclusive,
                _ => return Err(malformed()),
            };
            let projected = if access.field_region(entry.key()).is_some() {
                Some(
                    terminal_semantics::access_region_projection(
                        &self.program.catalogs.structural_types,
                        plan,
                        field,
                        projection,
                    )
                    .map_err(TerminalInterpretError::AccessReference)?,
                )
            } else {
                if !projection.is_empty() {
                    return Err(malformed());
                }
                None
            };
            let authorized = if let Some(projected) = projected {
                access.authorize_region_projection(
                    entry.key(),
                    projected.byte_offset,
                    u16::from(
                        reference_storage_byte_width(projected.stored_scalar)
                            .ok_or_else(malformed)?,
                    ) * 8,
                    operation.receiver_polarity(),
                    source_borrow,
                    operation,
                )
            } else {
                access.authorize(
                    entry.key(),
                    operation.receiver_polarity(),
                    source_borrow,
                    operation,
                )
            }
            .map_err(|reason| TerminalInterpretError::AccessReference(reason.to_string()))?;
            let descriptor = authorized.descriptor();
            if matches!(content, ProviderFieldContent::Resident)
                && descriptor.observation() != ObservationModel::Stable
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident scalar events require Stable observation",
                ));
            }
            let (compatibility, initialized, custody, content_bytes) = match content {
                ProviderFieldContent::Borrowed(compatibility) => {
                    (compatibility, None, None, binding.length)
                }
                ProviderFieldContent::Resident => {
                    let (initialized, custody, compatibility) =
                        self.live_resident_custody(binding.resident)?;
                    if initialized.placement != plan.placement
                        || initialized.schema != plan.schema
                        || initialized.policy_identity != plan.policy_identity
                        || initialized.policy_plan_machine_identity
                            != plan.policy_plan_machine_identity
                        || initialized.schema_identity != plan.schema_identity
                        || custody.value.structural_type != plan.schema
                    {
                        return Err(TerminalInterpretError::ContentTransformationMismatch(
                            "resident event changed its initialization contract",
                        ));
                    }
                    (
                        compatibility.placement(),
                        Some(initialized),
                        Some(custody),
                        compatibility.byte_length(),
                    )
                }
            };
            let supply = compatibility
                .fields()
                .iter()
                .find(|row| row.key() == entry.key());
            let footprint = descriptor.effect_footprint();
            let supplied = if let Some(supply) = supply {
                (descriptor.observation() != ObservationModel::Stable
                    || supply.kind() == EffectiveSupplyKind::Stable)
                    && supply.offset() == footprint.byte_offset()
                    && u64::from(supply.width_bits())
                        == footprint
                            .length_bytes()
                            .checked_mul(8)
                            .ok_or_else(malformed)?
            } else {
                compatibility.field_regions().iter().any(|region| {
                    region.key() == entry.key()
                        && footprint.byte_offset() >= region.byte_offset()
                        && footprint.end() <= region.byte_offset() + region.byte_length()
                })
            };
            if !supplied || footprint.end() > content_bytes || footprint.end() > binding.length {
                return Err(malformed());
            }
            let StructuralTypeShape::Record { fields, .. } =
                &self.structural_declaration(plan.schema)?.shape
            else {
                return Err(malformed());
            };
            let source_field = fields
                .iter()
                .find(|row| row.identity == field.schema_field)
                .ok_or_else(malformed)?;
            if initialized.is_some_and(|initialized| {
                initialized.image_fields.get(field.access_slot as usize) != Some(&source_field.id)
            }) {
                return Err(malformed());
            }
            let value_type = projected.map_or_else(
                || source_field.field_type.clone(),
                |projection| terminal_psi::StructuralFieldType::Scalar(projection.logical_scalar),
            );
            let scalar = value_type.scalar_type().ok_or(
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident accessor requires a scalar value contract",
                ),
            )?;
            let width = reference_storage_byte_width(scalar).ok_or_else(malformed)?;
            let write = match (kind, arguments) {
                (TerminalAccessorKind::Read, []) => None,
                (TerminalAccessorKind::Write, [value])
                    if value.scalar_type() == scalar && value.satisfies_field(&value_type) =>
                {
                    Some(*value)
                }
                _ => return Err(malformed()),
            };
            if write.is_some()
                && let Some(custody) = custody
            {
                let mut path = vec![StructuralPathSegment::Field(source_field.identity)];
                path.extend_from_slice(projection);
                if !custody.value.qualifications.is_empty()
                    || custody.value.projected_qualifications.iter().any(|row| {
                        terminal_semantics::structural_paths_may_overlap(&row.path, &path)
                    })
                    || custody.storage.resident_field_has_bindings(
                        &StructuralRuntimePlace::from(&custody.value),
                        source_field.identity,
                    )
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "resident field write requires preservation of retained qualifications and bindings",
                    ));
                }
            }
            let represented_bits = descriptor
                .logical_extent()
                .fragments()
                .iter()
                .try_fold(0_u64, |sum, fragment| {
                    sum.checked_add(fragment.width_bits())
                })
                .ok_or_else(malformed)?;
            let needs_read = write.is_none() || represented_bits != footprint.length_bytes() * 8;
            let (allocation, profile, _) =
                self.provider_storage.live_provider_allocation(&binding)?;
            let loan = allocation
                .extent
                .loan(binding.offset, binding.length)
                .map_err(|reason| TerminalInterpretError::AccessReference(reason.to_string()))?;
            let actual = profile
                .restrict_to_loan(&loan)
                .map_err(|reason| TerminalInterpretError::AccessReference(reason.to_string()))?;
            if descriptor.observation() == ObservationModel::Stable {
                access_plans::require_stable_region_supply(
                    &actual,
                    footprint.byte_offset(),
                    footprint.length_bytes(),
                    needs_read,
                    write.is_some(),
                    plan.placement.reach(),
                    "resident field event",
                )
                .map_err(|reason| TerminalInterpretError::AccessReference(reason.to_string()))?;
            } else if write.is_some() && needs_read {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "external field write cannot synthesize an unadmitted read-patch-write",
                ));
            }
            let start = binding
                .offset
                .checked_add(footprint.byte_offset())
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or_else(malformed)?;
            let length = usize::try_from(footprint.length_bytes()).map_err(|_| malformed())?;
            let end = start.checked_add(length).ok_or_else(malformed)?;
            let backing = allocation
                .bytes
                .read()
                .map_err(TerminalInterpretError::ResidentStorage)?;
            let bytes = backing.get(start..end).ok_or_else(malformed)?;
            let mut transfer = if needs_read {
                ByteSequenceStorage::copy(bytes, self.account.as_ref())?
            } else {
                ByteSequenceStorage::zeroed(length, self.account.as_ref())?
            };
            let layout = if projected.is_some() {
                let mut layout = field_window(
                    plan.placement.layout(),
                    entry.field(),
                    access
                        .field_region(entry.key())
                        .ok_or_else(malformed)?
                        .byte_offset(),
                    footprint.length_bytes(),
                )?;
                layout.size = Some(footprint.length_bytes());
                layout
            } else {
                field_window(
                    plan.placement.layout(),
                    entry.field(),
                    footprint.byte_offset(),
                    footprint.length_bytes(),
                )?
            };
            let member = layout
                .entries
                .first()
                .ok_or_else(malformed)?
                .member_identity;
            let result = if let Some(value) = write {
                let bits = crate::storage::primitive_storage::encoding::encode(scalar, value)?;
                let bits = u64::try_from(bits).map_err(|_| {
                    TerminalInterpretError::UnsupportedSemanticVariant(
                        "resident scalar codec exceeds its 64-bit materialization carrier",
                    )
                })?;
                let value = match member {
                    Some(member) => ScalarFieldValue::new_numbered(
                        entry.field(),
                        member,
                        u16::from(width) * 8,
                        bits,
                    ),
                    None => ScalarFieldValue::new(entry.field(), u16::from(width) * 8, bits),
                }
                .map_err(TerminalInterpretError::ValueImage)?;
                layout_plans::stage_scalar_field(
                    &layout,
                    &value,
                    ByteOrder::LittleEndian,
                    transfer.bytes_mut(),
                )
                .map_err(TerminalInterpretError::ValueImage)?;
                TerminalEffectResult::Unit
            } else {
                let schema = match member {
                    Some(member) => {
                        AggregateFieldSchema::new_numbered(entry.field(), member, u64::from(width))
                    }
                    None => AggregateFieldSchema::new(entry.field(), u64::from(width)),
                }
                .map_err(TerminalInterpretError::ValueImage)?;
                let mut encoded = [0_u8; 16];
                let decoded = encoded
                    .get_mut(..usize::from(width))
                    .ok_or_else(malformed)?;
                layout_plans::decode_aggregate_field_into(
                    &layout,
                    &schema,
                    ByteOrder::LittleEndian,
                    transfer.bytes(),
                    decoded,
                )
                .map_err(TerminalInterpretError::ValueImage)?;
                let value = crate::storage::primitive_storage::encoding::decode(
                    scalar,
                    u128::from_le_bytes(encoded),
                )?;
                if !value.satisfies_field(&value_type) {
                    return Err(malformed());
                }
                TerminalEffectResult::Scalar(value)
            };
            drop(backing);
            if write.is_some() {
                let ProviderStorageSlot::Live { allocation, .. } = self
                    .provider_storage
                    .allocations
                    .get_mut(binding.allocation)
                else {
                    return Err(malformed());
                };
                allocation
                    .bytes
                    .write()
                    .map_err(TerminalInterpretError::ResidentStorage)?
                    .get_mut(start..end)
                    .ok_or_else(malformed)?
                    .copy_from_slice(transfer.bytes());
            }
            Ok(result)
        }
    }

    fn field_window(
        layout: &LayoutPlanReport,
        field: &str,
        base: u64,
        length: u64,
    ) -> Result<LayoutPlanReport, TerminalInterpretError> {
        let mut entries = Vec::new();
        for entry in layout.entries.iter().filter(|entry| entry.field == field) {
            let mut entry = entry.clone();
            let offset = match &mut entry.placement {
                LayoutPlacementReport::At { offset }
                | LayoutPlacementReport::IntegerAt { offset, .. } => offset,
                LayoutPlacementReport::Bits { container, .. } => container,
            };
            *offset = offset.checked_sub(base).ok_or_else(malformed)?;
            entries.push(entry);
        }
        if entries.is_empty() {
            return Err(malformed());
        }
        Ok(LayoutPlanReport {
            entries,
            offsets: None,
            size: Some(length),
            align: 1,
            ..layout.clone()
        })
    }
}
pub(crate) mod initialization {
    use super::{ProviderStorageBinding, ProviderStorageSlot, RuntimeProviderStorage};
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    };
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
    use crate::storage::record::storage::ConstructorStorage;
    use access_plans::{
        InitializationResourceCompatibility, ValidatedAccessPlan, ValidatedResourceProfile,
        validate_initialization_resources,
    };
    use semantic_vocabulary::IntegerValue;
    use terminal_psi::{
        StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralPathSegment,
    };

    #[derive(Debug)]
    pub(crate) enum InitializationRejection {
        RangeTooSmall { required: u64, available: u64 },
        Misaligned { modulus: u64, residue: u64 },
        IncompatibleSupply,
    }

    pub(crate) struct PreparedProviderImage {
        pub(crate) binding: ProviderStorageBinding,
        pub(crate) image: ByteSequenceStorage,
        pub(crate) compatibility: InitializationResourceCompatibility,
    }

    impl RuntimeProviderStorage {
        pub(crate) fn prepare_initial_image(
            &mut self,
            binding: ProviderStorageBinding,
            supplied: ValidatedResourceProfile,
            placement: &ValidatedAccessPlan,
            image: ByteSequenceStorage,
        ) -> Result<Result<PreparedProviderImage, InitializationRejection>, TerminalInterpretError>
        {
            let required = placement.layout().size.ok_or_else(malformed)?;
            if u64::try_from(image.bytes().len()).ok() != Some(required) {
                return Err(malformed());
            }
            let ProviderStorageSlot::Live {
                allocation,
                profile,
                ..
            } = self.allocations.get_mut(binding.allocation)
            else {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "initialization allocation retired before staging",
                ));
            };
            let end = binding
                .offset
                .checked_add(binding.length)
                .ok_or_else(malformed)?;
            let bytes = allocation
                .bytes
                .read()
                .map_err(TerminalInterpretError::ResidentStorage)?;
            if end > allocation.extent.length()
                || usize::try_from(end)
                    .ok()
                    .is_none_or(|end| end > bytes.len())
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "initialization window exceeds its original allocation",
                ));
            }
            if required > binding.length {
                return Ok(Err(InitializationRejection::RangeTooSmall {
                    required,
                    available: binding.length,
                }));
            }
            let Ok(compatibility) = validate_initialization_resources(placement, &supplied) else {
                return Ok(Err(InitializationRejection::IncompatibleSupply));
            };
            let base = allocation
                .extent
                .base()
                .checked_add(binding.offset)
                .ok_or_else(malformed)?;
            let congruence = compatibility.placement().base_congruence();
            if !congruence.admits(base) {
                return Ok(Err(InitializationRejection::Misaligned {
                    modulus: congruence.modulus(),
                    residue: congruence.residue(),
                }));
            }
            let Ok(loan) = allocation.extent.loan_mut(binding.offset, binding.length) else {
                return Ok(Err(InitializationRejection::IncompatibleSupply));
            };
            let Ok(admitted) = profile.restrict_to_loan(&loan) else {
                return Ok(Err(InitializationRejection::IncompatibleSupply));
            };
            let Ok(compatibility) = validate_initialization_resources(placement, &admitted) else {
                return Ok(Err(InitializationRejection::IncompatibleSupply));
            };
            let congruence = compatibility.placement().base_congruence();
            if !congruence.admits(base) {
                return Ok(Err(InitializationRejection::Misaligned {
                    modulus: congruence.modulus(),
                    residue: congruence.residue(),
                }));
            }
            Ok(Ok(PreparedProviderImage {
                binding,
                image,
                compatibility,
            }))
        }
    }

    pub(crate) fn stage_rejection_failure(
        storage: &mut ConstructorStorage,
        rejection: InitializationRejection,
        failures: &[StructuralCaseDeclaration; 3],
        reason_field: &StructuralFieldDeclaration,
        opaque_identity: u64,
        result_case: semantic_vocabulary::StructuralCaseId,
    ) -> Result<(), TerminalInterpretError> {
        let (failure, coordinates) = match rejection {
            InitializationRejection::RangeTooSmall {
                required,
                available,
            } => (&failures[0], vec![required, available]),
            InitializationRejection::Misaligned { modulus, residue } => {
                (&failures[1], vec![modulus, residue])
            }
            InitializationRejection::IncompatibleSupply => (&failures[2], Vec::new()),
        };
        let reason = StructuralRuntimePlace {
            opaque_identity,
            path: vec![
                StructuralPathSegment::Case(result_case),
                StructuralPathSegment::Field(reason_field.identity),
            ],
        };
        storage.stage_case(
            reason.clone(),
            reason_field
                .field_type
                .as_structural()
                .ok_or_else(malformed)?,
            failure.id,
        )?;
        for (field, coordinate) in failure.fields.iter().zip(coordinates) {
            storage.stage_scalar_endpoint(
                &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::case_payload(
                    reason.clone(),
                    field.id,
                )),
                TerminalScalarValue::Integer {
                    scalar_type: field
                        .field_type
                        .scalar_type()
                        .and_then(|scalar| scalar.as_integer())
                        .ok_or_else(malformed)?,
                    value: IntegerValue::Unsigned(u128::from(coordinate)),
                },
            )?;
        }
        Ok(())
    }
}
pub(crate) mod lifecycle {
    use super::initialization::{InitializationRejection, PreparedProviderImage};
    use super::{ProviderStorageSlot, ResidentStorageSlot};
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    };
    use crate::engine::custody::{bind_result_claim, projected_ownership_claims};
    use crate::engine::execution::{TerminalExecution, operation_kind};
    use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
    use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
    use semantic_vocabulary::IntegerValue;
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        BoundaryMachineDeclaration, Operation, OperationKind, StructuralAccess,
        StructuralAffineDiscard, StructuralMultiplicity, StructuralOperationResult,
        StructuralPathSegment, StructuralResultClaimBinding, StructuralTypeShape,
        TerminalAccessLifecyclePlan, TerminalAccessOwner,
    };
    use terminal_semantics::{AccessResultContract, access_result_contract};

    impl TerminalExecution<'_> {
        pub(crate) fn complete_access_initialization(
            &mut self,
            operation: &Operation,
            boundary: &BoundaryMachineDeclaration,
            plan: &TerminalAccessLifecyclePlan,
            prepared: PreparedProviderImage,
        ) -> Result<(), TerminalInterpretError> {
            let (vacant, resident_domain) = plan
                .storage
                .owned_content()
                .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
            operation_kind!(operation => BoundaryCall {
                structural_arguments, completion_receipts, ..
            });
            let [extent, original, _profile] = structural_arguments.as_slice() else {
                return Err(malformed());
            };
            let result = operation.result.structural().ok_or_else(malformed)?;
            if self.result_place_in_use(result.place)
                || !result.qualification_establishments.is_empty()
                || !self
                    .loan_completions_for_operation(operation.id)?
                    .is_empty()
                || prepared.binding.resident != arena::Handle::default()
            {
                return Err(malformed());
            }
            let program = std::sync::Arc::clone(&self.program);
            let AccessResultContract::Initialize {
                result: declaration,
                ready,
                ..
            } = access_result_contract(&program.catalogs, plan)
                .map_err(TerminalInterpretError::ContentTransformationMismatch)?
            else {
                return Err(malformed());
            };
            if result.structural_type != declaration.structural_type
                || result.multiplicity != declaration.multiplicity
                || result.qualifications != declaration.qualifications
                || result.projected_qualifications != declaration.projected_qualifications
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "placement initialization result differs from its boundary contract",
                ));
            }
            let [field] = ready.fields.as_slice() else {
                return Err(malformed());
            };
            let [extent_parameter, value_parameter, _] = boundary.structural_parameters.as_slice()
            else {
                return Err(malformed());
            };
            let mut identity_cursor = self.local_structural_identities.cursor();
            let mut retained_extent = self.stage_owned_value(
                extent,
                plan.storage.carrier,
                extent_parameter.multiplicity,
                &mut identity_cursor,
            )?;
            let custody = self.stage_owned_value(
                original,
                plan.schema,
                value_parameter.multiplicity,
                &mut identity_cursor,
            )?;
            if custody.storage.retains_references() {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident custody requires live forwarding of carried references",
                ));
            }
            if retained_extent.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()]
                || !retained_extent
                    .value
                    .qualifications
                    .contains(&plan.storage.granted)
                || !retained_extent.value.qualifications.contains(&vacant)
                || retained_extent
                    .value
                    .qualifications
                    .contains(&resident_domain)
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let expected_receipts = retained_extent.claim_paths.len() + custody.claim_paths.len();
            if completion_receipts.len() != expected_receipts {
                return Err(TerminalInterpretError::CompletionReceiptMismatch);
            }
            let mut claims = crate::engine::custody::complete_claims(
                &self.live_claims,
                structural_arguments,
                completion_receipts,
                &boundary.structural_parameters,
            )?;
            let mut frontier = self.live_affine_frontier.clone();
            let mut consumed = Vec::new();
            for (source, parameter) in [extent, original]
                .into_iter()
                .zip([extent_parameter, value_parameter])
            {
                let transfers =
                    self.stage_constructor_operand(source, &mut frontier, &mut consumed)?;
                if parameter.multiplicity != StructuralMultiplicity::Unrestricted && !transfers {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
            let destination_path = vec![
                StructuralPathSegment::Case(ready.id),
                StructuralPathSegment::Field(field.identity),
            ];
            let destination = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: destination_path.clone(),
            };
            let mut backing_destination = destination.clone();
            backing_destination.path.extend(
                plan.storage
                    .owner_storage_path(&program.catalogs.structural_types)
                    .ok_or_else(malformed)?,
            );
            let mut storage = ConstructorStorage::new(self.account.as_ref());
            storage.stage_case(
                StructuralRuntimePlace::from(&value),
                result.structural_type,
                ready.id,
            )?;
            let child = ConstructorValue::resolve(self, extent)?;
            storage.stage(self, extent, &child, &backing_destination, true)?;
            storage.take_provider_descriptor(&backing_destination, prepared.binding)?;
            retained_extent
                .value
                .qualifications
                .retain(|domain| *domain != vacant);
            match plan.storage.owner {
                TerminalAccessOwner::Extent => {
                    retained_extent.value.qualifications.push(resident_domain)
                }
                TerminalAccessOwner::Run {
                    carrier,
                    count,
                    singleton,
                    ..
                } => {
                    let StructuralTypeShape::Record { fields, .. } =
                        &terminal_psi::find_structural_type(
                            &program.catalogs.structural_types,
                            carrier,
                        )
                        .ok_or_else(malformed)?
                        .shape
                    else {
                        return Err(malformed());
                    };
                    let count = fields
                        .iter()
                        .find(|field| field.id == count)
                        .ok_or_else(malformed)?;
                    storage.stage_scalar_endpoint(
                        &PrimitiveStorageEndpoint::ScalarField(
                            StructuralScalarRuntimeField::member(destination.clone(), count.id),
                        ),
                        TerminalScalarValue::Integer {
                            scalar_type: count
                                .field_type
                                .scalar_type()
                                .and_then(|scalar| scalar.as_integer())
                                .ok_or_else(malformed)?,
                            value: IntegerValue::Unsigned(1),
                        },
                    )?;
                    terminal_psi::StructuralPathQualification::append_under(
                        &[resident_domain, singleton.ok_or_else(malformed)?],
                        &[],
                        &destination_path,
                        &mut value.projected_qualifications,
                    );
                }
            }
            retained_extent.value.qualifications.sort();
            terminal_psi::StructuralPathQualification::append_under(
                &retained_extent.value.qualifications,
                &retained_extent.value.projected_qualifications,
                &backing_destination.path,
                &mut value.projected_qualifications,
            );
            storage.validate_projected_memberships(
                self,
                &value,
                &result.projected_qualifications,
            )?;
            let selected = |region: &StructuralAffineDiscard| {
                storage.case_at_projection(self, &value, &region.path)
            };
            let mut received = false;
            for binding in &result.claims {
                if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                    continue;
                }
                if received || binding.path != destination_path {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                received = true;
                bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
            }
            if !received {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
            let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
            let storage = storage.into_detached()?;
            storage.install(self)?;
            self.structural_values.insert(result.place, value)?;
            let lease = self
                .account
                .as_ref()
                .map(|account| account.reserve_storage(1, 0))
                .transpose()
                .map_err(TerminalInterpretError::Storage)?;
            let descriptor = self
                .provider_storage
                .descriptors
                .insert_vacant(backing_destination, prepared.binding)?;
            let start = usize::try_from(prepared.binding.offset).map_err(|_| malformed())?;
            let end = start
                .checked_add(prepared.image.bytes().len())
                .ok_or_else(malformed)?;
            let ProviderStorageSlot::Live { allocation, .. } = self
                .provider_storage
                .allocations
                .get_mut(prepared.binding.allocation)
            else {
                return Err(malformed());
            };
            let mut backing = allocation
                .bytes
                .write()
                .map_err(TerminalInterpretError::ResidentStorage)?;
            let bytes = backing.get_mut(start..end).ok_or_else(malformed)?;

            bytes.copy_from_slice(prepared.image.bytes());
            drop(backing);
            let resident = self
                .provider_storage
                .residents
                .insert(ResidentStorageSlot::Live {
                    window: prepared.binding,
                    initialized_by: plan.boundary,
                    custody,
                    compatibility: prepared.compatibility,
                    _lease: lease,
                });
            descriptor.resident = resident;
            retirement.commit(self)?;
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            Ok(())
        }

        pub(crate) fn reject_access_transfer(
            &mut self,
            operation: &Operation,
            boundary: &BoundaryMachineDeclaration,
            plan: &TerminalAccessLifecyclePlan,
            rejection: InitializationRejection,
        ) -> Result<(), TerminalInterpretError> {
            operation_kind!(operation => BoundaryCall {
                structural_arguments, completion_receipts, ..
            });
            let [extent, original, ..] = structural_arguments.as_slice() else {
                return Err(malformed());
            };
            let result = operation.result.structural().ok_or_else(malformed)?;
            if !self
                .loan_completions_for_operation(operation.id)?
                .is_empty()
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "owned storage transfer has no mutable borrowed input to restore",
                ));
            }
            if self.result_place_in_use(result.place)
                || !result.qualification_establishments.is_empty()
            {
                return Err(malformed());
            }
            let program = std::sync::Arc::clone(&self.program);
            let (declaration, rejected, returned, failures) =
                match access_result_contract(&program.catalogs, plan)
                    .map_err(TerminalInterpretError::ContentTransformationMismatch)?
                {
                    AccessResultContract::Initialize {
                        result,
                        rejected,
                        returned,
                        failures,
                        ..
                    }
                    | AccessResultContract::Relocate {
                        result,
                        rejected,
                        returned,
                        failures,
                        ..
                    } => (result, rejected, returned, failures),
                    _ => return Err(malformed()),
                };
            if result.structural_type != declaration.structural_type
                || result.multiplicity != declaration.multiplicity
                || result.qualifications != declaration.qualifications
                || result.projected_qualifications != declaration.projected_qualifications
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "placement rejection result differs from its boundary contract",
                ));
            }
            let [returned_field, reason_field] = rejected.fields.as_slice() else {
                return Err(malformed());
            };
            let mut identity_cursor = self.local_structural_identities.cursor();
            let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
            let root = StructuralRuntimePlace::from(&value);
            let mut storage = ConstructorStorage::new(self.account.as_ref());
            storage.stage_case(root.clone(), result.structural_type, rejected.id)?;
            let mut frontier = self.live_affine_frontier.clone();
            let mut consumed = Vec::new();
            let mut transported = BTreeMap::new();
            for (position, (source, field)) in
                [extent, original].into_iter().zip(returned).enumerate()
            {
                let parameter = boundary
                    .structural_parameters
                    .get(position)
                    .ok_or_else(malformed)?;
                if source.access != StructuralAccess::Owned
                    || parameter.access != StructuralAccess::Owned
                {
                    return Err(malformed());
                }
                let child = ConstructorValue::resolve(self, source)?;
                if child.structural_type() != parameter.structural_type {
                    return Err(malformed());
                }
                if let ConstructorValue::Access(placed) = &child
                    && crate::engine::custody::has_live_loan(
                        self.reference_referents.as_map(),
                        placed,
                    )
                {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "placement rejection cannot transfer a borrowed input",
                    ));
                }
                let destination_path = vec![
                    StructuralPathSegment::Case(rejected.id),
                    StructuralPathSegment::Field(returned_field.identity),
                    StructuralPathSegment::Field(field.identity),
                ];
                let destination = StructuralRuntimePlace {
                    opaque_identity: value.opaque_identity,
                    path: destination_path.clone(),
                };
                let transfers =
                    self.stage_constructor_operand(source, &mut frontier, &mut consumed)?;
                if parameter.multiplicity != StructuralMultiplicity::Unrestricted && !transfers {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                storage.stage(self, source, &child, &destination, transfers)?;
                child.append_memberships(&destination_path, &mut value.projected_qualifications);

                let source_claims = projected_ownership_claims(&self.live_claims, source)?;
                for (identity, claim) in &source_claims {
                    let mut path = destination_path.clone();
                    path.extend_from_slice(&claim.path);
                    if transported.insert(path, *identity).is_some() {
                        return Err(TerminalInterpretError::ClaimTransferMismatch);
                    }
                }
                let (whole, projected) = child.memberships();
                let expected = StructuralOperationResult {
                    place: source.place,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    qualifications: whole.to_vec(),
                    projected_qualifications: projected.to_vec(),
                    additional_input_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: source_claims
                        .iter()
                        .map(|(claim, held)| StructuralResultClaimBinding {
                            claim: *claim,
                            path: held.path.clone(),
                        })
                        .collect(),
                };
                let selected = |region: &StructuralAffineDiscard| {
                    let mut path = destination_path.clone();
                    path.extend_from_slice(&region.path);
                    storage.case_at_projection(self, &value, &path)
                };
                let mut expected_frontier = BTreeSet::new();
                self.stage_result_affine_frontier(&expected, &mut expected_frontier, &selected)?;
                self.validate_transferred_affine_complement(
                    source,
                    &expected,
                    &expected_frontier,
                    &selected,
                )?;
            }
            super::initialization::stage_rejection_failure(
                &mut storage,
                rejection,
                failures,
                reason_field,
                value.opaque_identity,
                rejected.id,
            )?;
            value.projected_qualifications.sort();
            value.projected_qualifications.dedup();
            storage.validate_projected_memberships(
                self,
                &value,
                &result.projected_qualifications,
            )?;
            let selected = |region: &StructuralAffineDiscard| {
                storage.case_at_projection(self, &value, &region.path)
            };
            let mut claims = crate::engine::custody::complete_claims(
                &self.live_claims,
                structural_arguments,
                completion_receipts,
                &boundary.structural_parameters,
            )?;
            let mut received = BTreeSet::new();
            for binding in &result.claims {
                if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                    continue;
                }
                let original_identity = transported
                    .remove(&binding.path)
                    .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
                if !received.insert(original_identity) {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
            }
            if !transported.is_empty() || received.len() != completion_receipts.len() {
                return Err(TerminalInterpretError::CompletionReceiptMismatch);
            }
            self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
            let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
            storage.publish(self)?;
            self.structural_values.insert(result.place, value)?;
            retirement.commit(self)?;
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            Ok(())
        }
    }
}
pub(crate) mod profile_values {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
        TerminalStructuralValue,
    };
    use crate::engine::custody::project_structural_value;
    use crate::engine::execution::TerminalExecution;
    use access_plans::{
        AtomicCapability, AtomicPermissions, AtomicTransferRule, BoundaryReach,
        BoundaryServiceReachId, ExternalCapability, ExternalReadBehavior, PeerWritability,
        ResourceProfile, ResourceRegion, StableCapability, TransferRule,
    };
    use semantic_vocabulary::{IntegerValue, StructuralCaseId};
    use terminal_psi::{StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeShape};

    pub(crate) fn read(
        execution: &TerminalExecution<'_>,
        value: &TerminalStructuralValue,
    ) -> Result<ResourceProfile, TerminalInterpretError> {
        let profile = ProfileValue {
            execution,
            value: value.clone(),
            case: None,
        };
        profile.record(2)?;
        let regions = profile.counted_entries(32)?;
        let mut result = Vec::with_capacity(regions.len());
        for region in regions {
            region.record(6)?;
            let range = region.field(0)?;
            range.record(2)?;
            let peer = match region.field(1)?.tag(2)? {
                0 => PeerWritability::Exclusive,
                1 => PeerWritability::HostileShared,
                _ => return Err(malformed()),
            };
            let stable = match region.field(2)?.tag(4)? {
                0 => StableCapability::None,
                1 => StableCapability::Read,
                2 => StableCapability::Write,
                3 => StableCapability::ReadWrite,
                _ => return Err(malformed()),
            };
            result.push(ResourceRegion {
                offset: range.unsigned(0)?,
                length: range.unsigned(1)?,
                peer,
                stable,
                external: external(region.field(3)?)?,
                atomic: atomic(region.field(4)?)?,
                reach: reach(region.field(5)?)?,
            });
        }
        Ok(ResourceProfile { regions: result })
    }

    fn external(value: ProfileValue<'_, '_>) -> Result<ExternalCapability, TerminalInterpretError> {
        let (case, value) = value.selected_case(2)?;
        if case == 0 {
            value.payload(0)?;
            return Ok(ExternalCapability::None);
        }
        value.payload(3)?;
        let read = match value.field(0)?.tag(3)? {
            0 => ExternalReadBehavior::None,
            1 => ExternalReadBehavior::Repeatable,
            2 => ExternalReadBehavior::Destructive,
            _ => return Err(malformed()),
        };
        Ok(ExternalCapability::Access {
            read,
            write: value.boolean(1)?,
            transfers: value
                .field(2)?
                .counted_entries(16)?
                .into_iter()
                .map(transfer)
                .collect::<Result<_, _>>()?,
        })
    }

    fn transfer(value: ProfileValue<'_, '_>) -> Result<TransferRule, TerminalInterpretError> {
        value.record(2)?;
        Ok(TransferRule {
            width_bits: u16::try_from(value.unsigned(0)?).map_err(|_| {
                TerminalInterpretError::ContentTransformationMismatch(
                    "profile transfer width exceeds the supported coordinate space",
                )
            })?,
            alignment_bytes: value.unsigned(1)?,
        })
    }

    fn atomic(value: ProfileValue<'_, '_>) -> Result<AtomicCapability, TerminalInterpretError> {
        let (case, value) = value.selected_case(2)?;
        if case == 0 {
            value.payload(0)?;
            return Ok(AtomicCapability::None);
        }
        value.payload(1)?;
        let mut transfers = Vec::new();
        for entry in value.field(0)?.counted_entries(16)? {
            entry.record(2)?;
            let operations = entry.field(1)?;
            operations.record(12)?;
            transfers.push(AtomicTransferRule {
                transfer: transfer(entry.field(0)?)?,
                operations: AtomicPermissions {
                    load: operations.boolean(0)?,
                    store: operations.boolean(1)?,
                    fetch_add: operations.boolean(2)?,
                    fetch_sub: operations.boolean(3)?,
                    fetch_xor: operations.boolean(4)?,
                    fetch_or: operations.boolean(5)?,
                    fetch_and: operations.boolean(6)?,
                    swap: operations.boolean(7)?,
                    compare_exchange: operations.boolean(8)?,
                    compare_exchange_once: operations.boolean(9)?,
                    try_exchange: operations.boolean(10)?,
                    try_exchange_once: operations.boolean(11)?,
                },
            });
        }
        Ok(AtomicCapability::Access { transfers })
    }

    fn reach(value: ProfileValue<'_, '_>) -> Result<BoundaryReach, TerminalInterpretError> {
        let services = value
            .counted_entries(32)?
            .into_iter()
            .map(|entry| {
                let scalar = entry
                    .execution
                    .structural_primitive_storage
                    .get(&StructuralRuntimePlace::from(&entry.value))
                    .map(crate::storage::scalar_storage::ScalarCell::read)
                    .transpose()?
                    .ok_or_else(malformed)?;
                BoundaryServiceReachId::from_normalized_identity(unsigned(scalar)?).map_err(|_| {
                    TerminalInterpretError::ContentTransformationMismatch(
                        "profile reach contains an invalid service identity",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BoundaryReach::from_services(services))
    }

    fn unsigned(value: TerminalScalarValue) -> Result<u64, TerminalInterpretError> {
        match value {
            TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            } => u64::try_from(value).map_err(|_| malformed()),
            _ => Err(malformed()),
        }
    }

    struct ProfileValue<'execution, 'program> {
        execution: &'execution TerminalExecution<'program>,
        value: TerminalStructuralValue,
        case: Option<StructuralCaseId>,
    }

    impl<'execution, 'program> ProfileValue<'execution, 'program> {
        fn fields(&self) -> Result<&[StructuralFieldDeclaration], TerminalInterpretError> {
            self.execution
                .active_field_declarations(
                    self.value.structural_type,
                    &StructuralRuntimePlace::from(&self.value),
                    self.case,
                )
                .map(|(fields, _)| fields)
        }

        fn record(&self, count: usize) -> Result<(), TerminalInterpretError> {
            if self.case.is_some() {
                return Err(malformed());
            }
            self.payload(count)
        }

        fn payload(&self, count: usize) -> Result<(), TerminalInterpretError> {
            let fields = self.fields()?;
            if fields.len() != count || fields.iter().any(|field| field.relevance.is_erased()) {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "profile lost its complete declared field roster",
                ));
            }
            Ok(())
        }

        fn scalar(&self, position: usize) -> Result<TerminalScalarValue, TerminalInterpretError> {
            let field = self.fields()?.get(position).ok_or_else(malformed)?;
            let parent = StructuralRuntimePlace::from(&self.value);
            let cell = if self.case.is_some() {
                StructuralScalarRuntimeField::case_payload(parent, field.id)
            } else {
                StructuralScalarRuntimeField::member(parent, field.id)
            };
            let value = self
                .execution
                .structural_scalar_fields
                .get(&cell)
                .map(crate::storage::scalar_storage::ScalarCell::read)
                .transpose()?
                .ok_or_else(malformed)?;
            if !value.satisfies_field(&field.field_type) {
                return Err(malformed());
            }
            Ok(value)
        }

        fn unsigned(&self, position: usize) -> Result<u64, TerminalInterpretError> {
            unsigned(self.scalar(position)?)
        }

        fn boolean(&self, position: usize) -> Result<bool, TerminalInterpretError> {
            match self.scalar(position)? {
                TerminalScalarValue::Boolean(value) => Ok(value),
                _ => Err(malformed()),
            }
        }

        fn field(&self, position: usize) -> Result<Self, TerminalInterpretError> {
            let field = self.fields()?.get(position).ok_or_else(malformed)?;
            let mut path = Vec::with_capacity(2);
            if let Some(case) = self.case {
                path.push(StructuralPathSegment::Case(case));
            }
            path.push(StructuralPathSegment::Field(field.identity));
            let value =
                project_structural_value(&self.execution.structural_types(), &self.value, &path)?;
            Ok(Self {
                execution: self.execution,
                value,
                case: None,
            })
        }

        fn selected_case(mut self, count: usize) -> Result<(usize, Self), TerminalInterpretError> {
            let types = self.execution.structural_types();
            let declaration = types.declaration(&self.value.structural_type)?;
            let StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return Err(malformed());
            };
            if cases.len() != count {
                return Err(malformed());
            }
            let stored = self
                .execution
                .structural_cases
                .get(&StructuralRuntimePlace::from(&self.value))
                .ok_or_else(malformed)?;
            if stored.structural_type != self.value.structural_type {
                return Err(malformed());
            }
            let position = cases
                .iter()
                .position(|case| case.id == stored.case)
                .ok_or_else(malformed)?;
            self.case = Some(stored.case);
            Ok((position, self))
        }

        fn tag(self, count: usize) -> Result<usize, TerminalInterpretError> {
            let (tag, selected) = self.selected_case(count)?;
            selected.payload(0)?;
            Ok(tag)
        }

        fn counted_entries(&self, capacity: u64) -> Result<Vec<Self>, TerminalInterpretError> {
            self.record(2)?;
            let count = self.unsigned(1)?;
            let array = self.field(0)?;
            let types = self.execution.structural_types();
            let StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = types.declaration(&array.value.structural_type)?.shape
            else {
                return Err(malformed());
            };
            if length != capacity || count > capacity {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "profile count exceeds its complete supplied array",
                ));
            }
            (0..count)
                .map(|index| {
                    let value = project_structural_value(
                        &types,
                        &array.value,
                        &[StructuralPathSegment::FixedIndex(index)],
                    )?;
                    Ok(Self {
                        execution: self.execution,
                        value,
                        case: None,
                    })
                })
                .collect()
        }
    }
}
pub(crate) mod profiles {
    use super::ProviderStorageBinding;
    use crate::boundary::effects::AdmittedBoundarySupply;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
    use crate::engine::execution::TerminalExecution;
    use access_plans::ValidatedResourceProfile;
    use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralArgument, TerminalAccessLifecycleExecution};

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
        pub(crate) fn lifecycle_profile(
            &self,
            plan: &terminal_psi::TerminalAccessLifecyclePlan,
            role_position: usize,
            storage: &TerminalStructuralValue,
            profile: &TerminalStructuralValue,
        ) -> Result<(ProviderStorageBinding, ValidatedResourceProfile), TerminalInterpretError>
        {
            let TerminalAccessLifecycleExecution::CompilerPrimitive {
                profile_type: Some(profile_type),
                profile_requirements,
            } = &plan.execution
            else {
                return Err(malformed());
            };
            let operands = plan.operation.profile_operands();
            if profile_requirements.len() != operands.len() {
                return Err(malformed());
            }
            let requirement = profile_requirements
                .get(role_position)
                .ok_or_else(malformed)?;
            let operand = operands.get(role_position).ok_or_else(malformed)?;
            let completion = self
                .provider_storage
                .completed_profiles
                .get(&StructuralRuntimePlace::from(profile))
                .ok_or(TerminalInterpretError::ContentTransformationMismatch(
                    "placement profile is not an unchanged selected-provider result",
                ))?;
            let content = match plan.operation {
                terminal_psi::TerminalAccessLifecycleOperation::Initialize => Some(
                    plan.storage
                        .owned_content()
                        .map_err(TerminalInterpretError::ContentTransformationMismatch)?
                        .0,
                ),
                terminal_psi::TerminalAccessLifecycleOperation::MoveOut => {
                    match plan.storage.owner {
                        terminal_psi::TerminalAccessOwner::Extent => Some(
                            plan.storage
                                .owned_content()
                                .map_err(TerminalInterpretError::ContentTransformationMismatch)?
                                .1,
                        ),
                        terminal_psi::TerminalAccessOwner::Run { .. } => None,
                    }
                }
                terminal_psi::TerminalAccessLifecycleOperation::View => None,
                terminal_psi::TerminalAccessLifecycleOperation::Relocate => {
                    if operand.occupied {
                        None
                    } else {
                        Some(
                            plan.storage
                                .owned_content()
                                .map_err(TerminalInterpretError::ContentTransformationMismatch)?
                                .0,
                        )
                    }
                }
            };
            if profile.structural_type != *profile_type
                || completion.result_type != *profile_type
                || completion.boundary != requirement.boundary
                || self.boundary_supplies.get(&requirement.boundary).copied()
                    != Some(AdmittedBoundarySupply::CheckedProvider(completion.provider))
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
                    terminal_psi::TerminalAccessLifecycleOperation::Initialize => {
                        binding.resident != arena::Handle::default()
                    }
                    terminal_psi::TerminalAccessLifecycleOperation::MoveOut => {
                        binding.resident == arena::Handle::default()
                    }
                    terminal_psi::TerminalAccessLifecycleOperation::View => false,
                    terminal_psi::TerminalAccessLifecycleOperation::Relocate => {
                        !operand.occupied && binding.resident != arena::Handle::default()
                    }
                }
                || !self.is_live_provider_descriptor(&home, &binding)
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "placement profile belongs to a different or retired allocation window",
                ));
            }
            let (_, _, base) = self.provider_storage.live_provider_allocation(&binding)?;
            if !self.provider_storage_geometry_matches(
                &home,
                &plan.storage,
                base,
                binding.length,
            )? {
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

        pub(crate) fn capture_resource_profile_call(
            &mut self,
            boundary: BoundaryMachineId,
            provider: MachineId,
            arguments: &[StructuralArgument],
            handler: &mut impl crate::TerminalEffectHandler,
        ) -> Result<Option<PendingResourceProfile>, TerminalInterpretError> {
            let Some((plan, result_type)) =
                self.program
                    .access_lifecycle_plans()
                    .iter()
                    .find_map(|plan| match &plan.execution {
                        TerminalAccessLifecycleExecution::CompilerPrimitive {
                            profile_type: Some(result_type),
                            profile_requirements,
                        } if profile_requirements
                            .iter()
                            .any(|requirement| requirement.boundary == boundary) =>
                        {
                            Some((plan, *result_type))
                        }
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
            let mut backing = self.provider_storage.descriptors.get(&storage).copied();
            if backing.is_none() {
                if !self.is_live_provider_owner(&storage) {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "dynamic profile supply has no exact live extent owner",
                    ));
                }
                let (base, length) = self
                    .provider_storage_geometry(&storage, &plan.storage)?
                    .ok_or_else(malformed)?;
                let request = super::TerminalProviderStorageRequest {
                    base,
                    length,
                    boundary,
                    provider,
                };
                if let Some(supply) = handler
                    .resolve_provider_storage(&request)
                    .map_err(TerminalInterpretError::ResidentStorage)?
                {
                    backing = Some(self.bind_dynamic_provider_storage(storage, &request, supply)?);
                }
            }
            Ok(Some(PendingResourceProfile {
                boundary,
                provider,
                backing,
                result_type,
            }))
        }

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
}
pub(crate) mod relocation {
    use super::initialization::InitializationRejection;
    use super::{ProviderStorageBinding, ProviderStorageSlot, ResidentStorageSlot};
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    };
    use crate::engine::custody::bind_result_claim;
    use crate::engine::execution::{TerminalExecution, operation_kind};
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
    use crate::storage::record::storage::{ConstructorStorage, ConstructorValue};
    use semantic_vocabulary::IntegerValue;
    use std::collections::BTreeSet;
    use terminal_psi::{
        BoundaryMachineDeclaration, Operation, OperationKind, StructuralPathQualification,
        StructuralPathSegment, TerminalAccessLifecyclePlan, TerminalAccessOwner,
    };
    use terminal_semantics::{AccessResultContract, access_result_contract};

    impl TerminalExecution<'_> {
        pub(crate) fn relocate_access_run(
            &mut self,
            operation: &Operation,
            boundary: &BoundaryMachineDeclaration,
            plan: &TerminalAccessLifecyclePlan,
        ) -> Result<(), TerminalInterpretError> {
            operation_kind!(operation => BoundaryCall {
                structural_arguments, completion_receipts, ..
            });
            let [source, destination, source_profile, destination_profile] =
                structural_arguments.as_slice()
            else {
                return Err(malformed());
            };
            let TerminalAccessOwner::Run {
                count: count_field, ..
            } = plan.storage.owner
            else {
                return Err(malformed());
            };
            let (vacant, live) = plan
                .storage
                .owned_content()
                .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
            let program = std::sync::Arc::clone(&self.program);
            let owner = self.resolve_live_projection(source.place, &source.path, source.access)?;
            if !owner.qualifications.contains(&live) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let count_value = self
                .structural_scalar_fields
                .get(&StructuralScalarRuntimeField::member(
                    StructuralRuntimePlace::from(&owner),
                    count_field,
                ))
                .ok_or_else(malformed)?
                .read()?;
            let TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(count),
                ..
            } = count_value
            else {
                return Err(malformed());
            };
            let count = u64::try_from(count).map_err(|_| malformed())?;
            let backing_path = plan
                .storage
                .owner_storage_path(&program.catalogs.structural_types)
                .ok_or_else(malformed)?;
            let mut backing_source = source.clone();
            backing_source.path.extend_from_slice(&backing_path);
            let source_child = ConstructorValue::resolve(self, &backing_source)?;
            let destination_child = ConstructorValue::resolve(self, destination)?;
            let (
                ConstructorValue::Access(source_extent),
                ConstructorValue::Access(destination_extent),
            ) = (&source_child, &destination_child)
            else {
                return Err(malformed());
            };
            let source_observation = self.resolve_live_projection(
                source_profile.place,
                &source_profile.path,
                source_profile.access,
            )?;
            let destination_observation = self.resolve_live_projection(
                destination_profile.place,
                &destination_profile.path,
                destination_profile.access,
            )?;
            let (source_binding, source_supply) =
                self.lifecycle_profile(plan, 0, source_extent, &source_observation)?;
            let (destination_binding, destination_supply) =
                self.lifecycle_profile(plan, 1, destination_extent, &destination_observation)?;
            if (count == 0) != (source_binding.resident == arena::Handle::default()) {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "run relocation count disagrees with retained cell custody",
                ));
            }
            if count != 0 {
                let (initialized, _, _) = self.live_resident_custody(source_binding.resident)?;
                if initialized.schema != plan.schema
                    || initialized.schema_identity != plan.schema_identity
                    || initialized.policy_identity != plan.policy_identity
                    || initialized.policy_plan_machine_identity != plan.policy_plan_machine_identity
                    || !initialized.storage.same_custody(&plan.storage)
                    || initialized.placement != plan.placement
                    || initialized.image_fields != plan.image_fields
                {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "relocation changed the retained cell policy or schema",
                    ));
                }
            }
            let (source_allocation, source_admission, _) = self
                .provider_storage
                .live_provider_allocation(&source_binding)?;
            let (destination_allocation, destination_admission, destination_base) = self
                .provider_storage
                .live_provider_allocation(&destination_binding)?;
            let source_end = source_binding
                .offset
                .checked_add(source_binding.length)
                .ok_or_else(malformed)?;
            let destination_end = destination_binding
                .offset
                .checked_add(destination_binding.length)
                .ok_or_else(malformed)?;
            if source_binding.allocation == destination_binding.allocation
                && source_binding.offset < destination_end
                && destination_binding.offset < source_end
            {
                return self.reject_access_transfer(
                    operation,
                    boundary,
                    plan,
                    InitializationRejection::IncompatibleSupply,
                );
            }
            let source_loan = source_allocation
                .extent
                .loan(source_binding.offset, source_binding.length)
                .map_err(|error| TerminalInterpretError::ResourceProfile {
                    boundary: plan.boundary,
                    diagnostic: access_plans::AccessPlanDiagnostic(format!(
                        "relocation source loan: {error}"
                    )),
                })?;
            let destination_loan = destination_allocation
                .extent
                .loan(destination_binding.offset, destination_binding.length)
                .map_err(|error| TerminalInterpretError::ResourceProfile {
                    boundary: plan.boundary,
                    diagnostic: access_plans::AccessPlanDiagnostic(format!(
                        "relocation destination loan: {error}"
                    )),
                })?;
            let admitted_source =
                source_admission
                    .restrict_to_loan(&source_loan)
                    .map_err(|diagnostic| TerminalInterpretError::ResourceProfile {
                        boundary: plan.boundary,
                        diagnostic,
                    })?;
            let admitted_destination = destination_admission
                .restrict_to_loan(&destination_loan)
                .map_err(|diagnostic| TerminalInterpretError::ResourceProfile {
                    boundary: plan.boundary,
                    diagnostic,
                })?;
            let selected = access_plans::validate_run_relocation_resources(
                &plan.placement,
                count,
                destination_base,
                &source_supply,
                &destination_supply,
            );
            let admitted = access_plans::validate_run_relocation_resources(
                &plan.placement,
                count,
                destination_base,
                &admitted_source,
                &admitted_destination,
            );
            let (Ok(_), Ok(compatibility)) = (selected, admitted) else {
                return self.reject_access_transfer(
                    operation,
                    boundary,
                    plan,
                    InitializationRejection::IncompatibleSupply,
                );
            };
            let source_start = usize::try_from(source_binding.offset).map_err(|_| malformed())?;
            let source_end = usize::try_from(source_end).map_err(|_| malformed())?;
            let source_bytes = source_allocation
                .bytes
                .read()
                .map_err(TerminalInterpretError::ResidentStorage)?;
            let image = ByteSequenceStorage::copy(
                source_bytes
                    .get(source_start..source_end)
                    .ok_or_else(malformed)?,
                self.account.as_ref(),
            )?;
            drop(source_bytes);

            let result = operation.result.structural().ok_or_else(malformed)?;
            if self.result_place_in_use(result.place)
                || !result.qualification_establishments.is_empty()
                || !self
                    .loan_completions_for_operation(operation.id)?
                    .is_empty()
            {
                return Err(malformed());
            }
            let AccessResultContract::Relocate {
                result: declaration,
                ready,
                moved,
                ..
            } = access_result_contract(&program.catalogs, plan)
                .map_err(TerminalInterpretError::ContentTransformationMismatch)?
            else {
                return Err(malformed());
            };
            if result.structural_type != declaration.structural_type
                || result.multiplicity != declaration.multiplicity
                || result.qualifications != declaration.qualifications
                || result.projected_qualifications != declaration.projected_qualifications
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "relocation result differs from its contract",
                ));
            }
            let [ready_field] = ready.fields.as_slice() else {
                return Err(malformed());
            };
            let mut identity_cursor = self.local_structural_identities.cursor();
            for (argument, parameter) in [source, destination]
                .into_iter()
                .zip(&boundary.structural_parameters)
            {
                let retained = self.stage_owned_value(
                    argument,
                    parameter.structural_type,
                    parameter.multiplicity,
                    &mut identity_cursor,
                )?;
                if retained.claim_paths.as_slice() != [Vec::<StructuralPathSegment>::new()] {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            let mut value = self.mint_staged(&mut identity_cursor, result.structural_type)?;
            let result_path = vec![
                StructuralPathSegment::Case(ready.id),
                StructuralPathSegment::Field(ready_field.identity),
            ];
            let mut run_path = result_path.clone();
            run_path.push(StructuralPathSegment::Field(moved[0].identity));
            let mut vacated_path = result_path;
            vacated_path.push(StructuralPathSegment::Field(moved[1].identity));
            let run_home = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: run_path.clone(),
            };
            let mut new_backing_home = run_home.clone();
            new_backing_home.path.extend_from_slice(&backing_path);
            let vacated_home = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: vacated_path.clone(),
            };
            let mut storage = ConstructorStorage::new(self.account.as_ref());
            storage.stage_case(
                StructuralRuntimePlace::from(&value),
                result.structural_type,
                ready.id,
            )?;
            storage.stage(self, &backing_source, &source_child, &vacated_home, true)?;
            storage.stage(
                self,
                destination,
                &destination_child,
                &new_backing_home,
                true,
            )?;
            storage.take_provider_descriptor(&vacated_home, source_binding)?;
            storage.take_provider_descriptor(&new_backing_home, destination_binding)?;
            storage.stage_scalar_endpoint(
                &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::member(
                    run_home,
                    count_field,
                )),
                count_value,
            )?;
            StructuralPathQualification::append_under(
                &[live],
                &[],
                &run_path,
                &mut value.projected_qualifications,
            );
            StructuralPathQualification::append_under(
                &[plan.storage.granted],
                &[],
                &new_backing_home.path,
                &mut value.projected_qualifications,
            );
            StructuralPathQualification::append_under(
                &[plan.storage.granted, vacant],
                &[],
                &vacated_path,
                &mut value.projected_qualifications,
            );
            value.projected_qualifications.sort();
            storage.validate_projected_memberships(
                self,
                &value,
                &result.projected_qualifications,
            )?;
            let selected = |region: &terminal_psi::StructuralAffineDiscard| {
                storage.case_at_projection(self, &value, &region.path)
            };
            let mut claims = crate::engine::custody::complete_claims(
                &self.live_claims,
                structural_arguments,
                completion_receipts,
                &boundary.structural_parameters,
            )?;
            let mut expected = BTreeSet::from([run_path, vacated_path]);
            for binding in &result.claims {
                if !self.constructor_claim_is_active(result, &binding.path, &selected)? {
                    continue;
                }
                if !expected.remove(&binding.path) {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
                bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
            }
            if !expected.is_empty() {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            let mut frontier = self.live_affine_frontier.clone();
            let mut consumed = Vec::new();
            for argument in [source, destination] {
                if !self.stage_constructor_operand(argument, &mut frontier, &mut consumed)? {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            self.stage_result_affine_frontier(result, &mut frontier, &selected)?;
            let retirement = self.prepare_constructor_operands(frontier, &consumed, claims)?;
            storage.into_detached()?.install(self)?;
            self.structural_values.insert(result.place, value)?;
            self.provider_storage.descriptors.insert_vacant(
                vacated_home,
                ProviderStorageBinding {
                    resident: arena::Handle::default(),
                    ..source_binding
                },
            )?;
            self.provider_storage.descriptors.insert_vacant(
                new_backing_home,
                ProviderStorageBinding {
                    resident: source_binding.resident,
                    ..destination_binding
                },
            )?;
            let ProviderStorageSlot::Live { allocation, .. } = self
                .provider_storage
                .allocations
                .get_mut(destination_binding.allocation)
            else {
                return Err(malformed());
            };
            let destination_start =
                usize::try_from(destination_binding.offset).map_err(|_| malformed())?;
            let destination_end = usize::try_from(destination_end).map_err(|_| malformed())?;
            let mut bytes = allocation
                .bytes
                .write()
                .map_err(TerminalInterpretError::ResidentStorage)?;
            let destination_bytes = bytes
                .get_mut(destination_start..destination_end)
                .ok_or_else(malformed)?;
            let resident = match compatibility {
                Some(compatibility) => {
                    let ResidentStorageSlot::Live {
                        window,
                        compatibility: current,
                        ..
                    } = self
                        .provider_storage
                        .residents
                        .get_mut(source_binding.resident)
                    else {
                        return Err(malformed());
                    };
                    Some((window, current, compatibility))
                }
                None => None,
            };
            destination_bytes.copy_from_slice(image.bytes());
            if let Some((window, current, compatibility)) = resident {
                *window = destination_binding;
                *current = compatibility;
            }
            drop(bytes);
            retirement.commit(self)?;
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            Ok(())
        }
    }
}
pub(crate) mod value_image {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use evaluation_accounting::EvaluationInvocationAccount;
    use layout_plans::{
        AggregateFieldSchema, AggregateFieldValue, ByteOrder, IntegerInterpretation,
        LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    };
    use semantic_vocabulary::{AuthoredFieldStorage, IntegerSign, ScalarType, StructuralTypeId};
    use terminal_psi::{
        StructuralArgument, StructuralFieldDeclaration, StructuralPathSegment,
        StructuralRecordStorage, StructuralTypeShape, TerminalAccessLifecyclePlan,
    };

    pub(crate) mod decoding {
        use super::geometry::ImageGeometry;
        use super::{field_schema, record_layout, require_runtime_fields, zeroed};
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::{
            StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
        };
        use crate::engine::execution::TerminalExecution;
        use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
        use crate::storage::record::storage::ConstructorStorage;
        use layout_plans::ByteOrder;
        use semantic_vocabulary::{ScalarType, StructuralTypeId};
        use terminal_psi::{
            StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeShape,
            TerminalAccessLifecyclePlan,
        };

        pub(crate) fn restore(
            execution: &TerminalExecution<'_>,
            plan: &TerminalAccessLifecyclePlan,
            image: &[u8],
            destination: &StructuralRuntimePlace,
        ) -> Result<ConstructorStorage, TerminalInterpretError> {
            let types = execution.structural_types();
            let Some(StructuralTypeShape::Record { fields, .. }) =
                types.get(&plan.schema).map(|row| &row.shape)
            else {
                return Err(malformed());
            };
            let geometry = ImageGeometry::new(execution, plan.target.as_ref())?;
            require_runtime_fields(&geometry, fields)?;
            let entries = plan.placement.access().plan().entries();
            if entries.len() != plan.image_fields.len() {
                return Err(malformed());
            }
            let mut decoder = ValueImageDecoder {
                execution,
                storage: ConstructorStorage::new(execution.account.as_ref()),
                home: destination.clone(),
                active: Vec::new(),
                geometry,
            };
            for (identity, entry) in plan.image_fields.iter().zip(entries) {
                let field = fields
                    .iter()
                    .find(|field| field.id == *identity)
                    .ok_or_else(malformed)?;
                let layout_entry = plan
                    .placement
                    .layout()
                    .entries
                    .iter()
                    .find(|layout| layout.field == entry.field())
                    .ok_or_else(malformed)?;
                let schema = field_schema(
                    execution,
                    &geometry,
                    field,
                    entry.field(),
                    layout_entry.member_identity,
                )?;
                let mut bytes = zeroed(schema.byte_size, execution.account.as_ref())?;
                layout_plans::decode_aggregate_field_into(
                    plan.placement.layout(),
                    &schema,
                    ByteOrder::LittleEndian,
                    image,
                    bytes.bytes_mut(),
                )
                .map_err(TerminalInterpretError::ValueImage)?;
                decoder.field(field, bytes.bytes(), None)?;
            }
            Ok(decoder.storage)
        }

        struct ValueImageDecoder<'execution, 'program> {
            execution: &'execution TerminalExecution<'program>,
            storage: ConstructorStorage,
            home: StructuralRuntimePlace,
            active: Vec<StructuralTypeId>,
            geometry: ImageGeometry<'execution, 'program>,
        }

        fn scalar(
            scalar: ScalarType,
            bytes: &[u8],
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            let width =
                terminal_semantics::reference_storage_byte_width(scalar).ok_or_else(malformed)?;
            if bytes.len() != usize::from(width) || bytes.len() > 16 {
                return Err(malformed());
            }
            let mut encoded = [0_u8; 16];
            encoded[..bytes.len()].copy_from_slice(bytes);
            crate::storage::primitive_storage::encoding::decode(
                scalar,
                u128::from_le_bytes(encoded),
            )
        }

        impl ValueImageDecoder<'_, '_> {
            fn field(
                &mut self,
                field: &StructuralFieldDeclaration,
                bytes: &[u8],
                case: Option<semantic_vocabulary::StructuralCaseId>,
            ) -> Result<(), TerminalInterpretError> {
                if let Some(carrier) = field.field_type.scalar_type() {
                    self.storage.stage_scalar_endpoint(
                        &PrimitiveStorageEndpoint::ScalarField(match case {
                            Some(_) => StructuralScalarRuntimeField::case_payload(
                                self.home.clone(),
                                field.id,
                            ),
                            None => {
                                StructuralScalarRuntimeField::member(self.home.clone(), field.id)
                            }
                        }),
                        scalar(carrier, bytes)?,
                    )
                } else {
                    if let Some(case) = case {
                        self.home.path.push(StructuralPathSegment::Case(case));
                    }
                    self.home
                        .path
                        .push(StructuralPathSegment::Field(field.identity));
                    self.structure(
                        field.field_type.as_structural().ok_or_else(malformed)?,
                        bytes,
                    )?;
                    self.home.path.pop();
                    if case.is_some() {
                        self.home.path.pop();
                    }
                    Ok(())
                }
            }

            fn structure(
                &mut self,
                root: StructuralTypeId,
                bytes: &[u8],
            ) -> Result<(), TerminalInterpretError> {
                if self.active.contains(&root) || self.active.len() >= 128 {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "resident image has cyclic or excessive structural depth",
                    ));
                }
                self.active.push(root);
                let types = self.execution.structural_types();
                let declaration = types.declaration(&root)?;
                let (size, alignment) = self.geometry.layout(root)?;
                if u64::try_from(bytes.len()).ok() != Some(size) {
                    return Err(malformed());
                }
                match &declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(carrier) => {
                        self.storage.stage_scalar_endpoint(
                            &PrimitiveStorageEndpoint::Scalar(self.home.clone()),
                            scalar(*carrier, bytes)?,
                        )?;
                    }
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } => {
                        let (stride, _) = self.geometry.layout(*element)?;
                        let stride = usize::try_from(stride).map_err(|_| malformed())?;
                        if stride == 0 {
                            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                                "resident zero-runtime elements require authored custody",
                            ));
                        }
                        if let Some(StructuralTypeShape::PrimitiveScalar(carrier)) =
                            types.get(element).map(|row| &row.shape)
                        {
                            let values = bytes
                                .chunks_exact(stride)
                                .map(|bytes| scalar(*carrier, bytes))
                                .collect::<Result<Vec<_>, _>>()?;
                            self.storage
                                .stage_array(self.execution, root, &values, &self.home)?;
                        } else {
                            for element_index in 0..*length {
                                let start = usize::try_from(element_index)
                                    .ok()
                                    .and_then(|index| index.checked_mul(stride))
                                    .ok_or_else(malformed)?;
                                let end = start.checked_add(stride).ok_or_else(malformed)?;
                                self.home
                                    .path
                                    .push(StructuralPathSegment::FixedIndex(element_index));
                                self.structure(
                                    *element,
                                    bytes.get(start..end).ok_or_else(malformed)?,
                                )?;
                                self.home.path.pop();
                            }
                        }
                    }
                    StructuralTypeShape::Record { fields, storage } => {
                        require_runtime_fields(&self.geometry, fields)?;
                        let layout = record_layout(
                            self.execution,
                            &self.geometry,
                            root,
                            fields,
                            storage,
                            size,
                            alignment,
                        )?;
                        for field in fields {
                            let name = format!("field:{:?}", field.id);
                            let schema =
                                field_schema(self.execution, &self.geometry, field, &name, None)?;
                            let mut contents =
                                zeroed(schema.byte_size, self.execution.account.as_ref())?;
                            layout_plans::decode_aggregate_field_into(
                                &layout,
                                &schema,
                                ByteOrder::LittleEndian,
                                bytes,
                                contents.bytes_mut(),
                            )
                            .map_err(TerminalInterpretError::ValueImage)?;
                            self.field(field, contents.bytes(), None)?;
                        }
                    }
                    StructuralTypeShape::Sum { cases } => self.sum(root, &[], cases, bytes)?,
                    StructuralTypeShape::Mixed { fields, cases } => {
                        self.sum(root, fields, cases, bytes)?
                    }
                    _ => {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "resident image is outside the retained schema reflection contract",
                        ));
                    }
                }
                self.active.pop();
                Ok(())
            }

            fn sum(
                &mut self,
                root: StructuralTypeId,
                common: &[StructuralFieldDeclaration],
                cases: &[terminal_psi::StructuralCaseDeclaration],
                bytes: &[u8],
            ) -> Result<(), TerminalInterpretError> {
                let layout = self.geometry.sum_layout(root)?;
                let width = self.geometry.tag_width()?;
                let mut tag = [0_u8; 8];
                tag[..width].copy_from_slice(bytes.get(..width).ok_or_else(malformed)?);
                let case_position =
                    usize::try_from(u64::from_le_bytes(tag)).map_err(|_| malformed())?;
                let case = cases.get(case_position).ok_or(
                    TerminalInterpretError::ContentTransformationMismatch(
                        "resident image contains an invalid case tag",
                    ),
                )?;
                self.storage.stage_case(self.home.clone(), root, case.id)?;
                self.sum_fields(common, &layout.common_field_offsets, bytes, None)?;
                self.sum_fields(
                    &case.fields,
                    &layout.case_field_offsets[case_position],
                    bytes,
                    Some(case.id),
                )
            }

            fn sum_fields(
                &mut self,
                fields: &[StructuralFieldDeclaration],
                offsets: &[u64],
                bytes: &[u8],
                case: Option<semantic_vocabulary::StructuralCaseId>,
            ) -> Result<(), TerminalInterpretError> {
                require_runtime_fields(&self.geometry, fields)?;
                if fields.len() != offsets.len() {
                    return Err(malformed());
                }
                for (field, offset) in fields.iter().zip(offsets) {
                    let schema = field_schema(self.execution, &self.geometry, field, "", None)?;
                    let start = usize::try_from(*offset).map_err(|_| malformed())?;
                    let length = usize::try_from(schema.byte_size).map_err(|_| malformed())?;
                    let end = start.checked_add(length).ok_or_else(malformed)?;
                    self.field(field, bytes.get(start..end).ok_or_else(malformed)?, case)?;
                }
                Ok(())
            }
        }
    }
    pub(crate) mod geometry {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::engine::execution::TerminalExecution;
        use semantic_vocabulary::{ScalarApplicationId, StructuralTypeId, TargetCarrierLayout};
        use terminal_psi::{StructuralPathSegment, TargetLayoutInput};
        use terminal_semantics::{
            ReferenceStorageGeometry, ReferenceStorageRegion, StorageGeometryError,
        };

        #[derive(Clone, Copy)]
        pub(crate) struct ImageGeometry<'execution, 'program> {
            execution: &'execution TerminalExecution<'program>,
            carriers: Option<TargetCarrierLayout>,
        }

        impl<'execution, 'program> ImageGeometry<'execution, 'program> {
            pub(crate) fn new(
                execution: &'execution TerminalExecution<'program>,
                target: Option<&TargetLayoutInput>,
            ) -> Result<Self, TerminalInterpretError> {
                let carriers = target
                    .map(|target| {
                        Ok::<_, TerminalInterpretError>(TargetCarrierLayout {
                            pointer_byte_size: target.pointer_size,
                            pointer_byte_alignment: target.pointer_alignment,
                            descriptor_byte_size: target
                                .pointer_size
                                .checked_mul(2)
                                .ok_or_else(malformed)?,
                            descriptor_byte_alignment: target.pointer_alignment,
                            case_tag_byte_size: target.case_tag_size,
                            case_tag_byte_alignment: target.case_tag_alignment,
                        })
                    })
                    .transpose()?;
                Ok(Self {
                    execution,
                    carriers,
                })
            }

            pub(crate) fn tag_width(&self) -> Result<usize, TerminalInterpretError> {
                self.carriers
                    .and_then(|layout| usize::try_from(layout.case_tag_byte_size).ok())
                    .filter(|width| (1..=8).contains(width))
                    .ok_or(TerminalInterpretError::ValueImageGeometry(
                        StorageGeometryError::MissingCarrierLayout,
                    ))
            }

            pub(crate) fn layout(
                &self,
                root: StructuralTypeId,
            ) -> Result<(u64, u64), TerminalInterpretError> {
                let types = self.execution.structural_types();
                ReferenceStorageGeometry::new(types.values(), unresolved)
                    .with_target_carriers(self.carriers)
                    .layout(root)
                    .map_err(TerminalInterpretError::ValueImageGeometry)
            }

            pub(crate) fn sum_layout(
                &self,
                root: StructuralTypeId,
            ) -> Result<layout_plans::ConventionalSumGeometry, TerminalInterpretError> {
                let types = self.execution.structural_types();
                ReferenceStorageGeometry::new(types.values(), unresolved)
                    .with_target_carriers(self.carriers)
                    .sum_layout(root)
                    .map_err(TerminalInterpretError::ValueImageGeometry)
            }

            pub(crate) fn region(
                &self,
                root: StructuralTypeId,
                path: &[StructuralPathSegment],
            ) -> Option<ReferenceStorageRegion> {
                let types = self.execution.structural_types();
                ReferenceStorageGeometry::new(types.values(), unresolved)
                    .with_target_carriers(self.carriers)
                    .region(root, path)
            }
        }

        fn unresolved(application: ScalarApplicationId) -> Result<u64, StorageGeometryError> {
            Err(StorageGeometryError::UnresolvedExtent(application))
        }
    }
    pub(crate) use decoding::restore;
    use geometry::ImageGeometry;

    pub(crate) fn prepare(
        execution: &TerminalExecution<'_>,
        plan: &TerminalAccessLifecyclePlan,
        source: &StructuralArgument,
        byte_order: ByteOrder,
    ) -> Result<ByteSequenceStorage, TerminalInterpretError> {
        let value = execution.resolve_live_projection(source.place, &source.path, source.access)?;
        if source.access != terminal_psi::StructuralAccess::Owned
            || value.structural_type != plan.schema
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "placement image requires its exact owned schema value",
            ));
        }
        let types = execution.structural_types();
        let Some(StructuralTypeShape::Record { fields, .. }) = types
            .get(&plan.schema)
            .map(|declaration| &declaration.shape)
        else {
            return Err(malformed());
        };
        let geometry = ImageGeometry::new(execution, plan.target.as_ref())?;
        require_runtime_fields(&geometry, fields)?;
        let mut reader = ValueImageReader {
            execution,
            source,
            byte_order,
            path: source.path.clone(),
            active: Vec::new(),
            geometry,
        };
        let mut schemas = Vec::with_capacity(plan.image_fields.len());
        let mut values = Vec::with_capacity(plan.image_fields.len());
        let entries = plan.placement.access().plan().entries();
        if entries.len() != plan.image_fields.len() {
            return Err(malformed());
        }
        for (identity, entry) in plan.image_fields.iter().zip(entries) {
            let field = fields
                .iter()
                .find(|field| field.id == *identity)
                .ok_or_else(malformed)?;
            let layout_entry = plan
                .placement
                .layout()
                .entries
                .iter()
                .find(|layout| layout.field == entry.field())
                .ok_or_else(malformed)?;
            let (schema, value) =
                reader.field(field, entry.field(), layout_entry.member_identity)?;
            schemas.push(schema);
            values.push(value);
        }
        materialize(
            plan.placement.layout(),
            &schemas,
            &values,
            byte_order,
            execution.account.as_ref(),
        )
    }

    struct ValueImageReader<'execution, 'program, 'source> {
        execution: &'execution TerminalExecution<'program>,
        source: &'source StructuralArgument,
        byte_order: ByteOrder,
        path: Vec<StructuralPathSegment>,
        active: Vec<StructuralTypeId>,
        geometry: ImageGeometry<'execution, 'program>,
    }

    impl ValueImageReader<'_, '_, '_> {
        fn scalar(
            &self,
            scalar: ScalarType,
        ) -> Result<ByteSequenceStorage, TerminalInterpretError> {
            let value =
                self.execution
                    .read_primitive_place(self.source.place, &self.path, scalar)?;
            let bits = crate::storage::primitive_storage::encoding::encode(scalar, value)?;
            let width = usize::from(
                terminal_semantics::reference_storage_byte_width(scalar).ok_or_else(malformed)?,
            );
            let encoded = match self.byte_order {
                ByteOrder::LittleEndian => bits.to_le_bytes(),
                ByteOrder::BigEndian => bits.to_be_bytes(),
            };
            let bytes = match self.byte_order {
                ByteOrder::LittleEndian => &encoded[..width],
                ByteOrder::BigEndian => &encoded[16 - width..],
            };
            ByteSequenceStorage::copy(bytes, self.execution.account.as_ref())
        }

        fn field(
            &mut self,
            field: &StructuralFieldDeclaration,
            name: &str,
            member_identity: Option<u64>,
        ) -> Result<
            (
                AggregateFieldSchema,
                AggregateFieldValue<ByteSequenceStorage>,
            ),
            TerminalInterpretError,
        > {
            let schema =
                field_schema(self.execution, &self.geometry, field, name, member_identity)?;
            self.path.push(StructuralPathSegment::Field(field.identity));
            let bytes = match field.field_type.scalar_type() {
                Some(scalar) => self.scalar(scalar),
                None => self.structure(field.field_type.as_structural().ok_or_else(malformed)?),
            }?;
            self.path.pop();
            let value = AggregateFieldValue::from_storage(name, bytes)
                .map_err(TerminalInterpretError::ValueImage)?;
            Ok((schema, value))
        }

        fn structure(
            &mut self,
            root: StructuralTypeId,
        ) -> Result<ByteSequenceStorage, TerminalInterpretError> {
            if self.active.contains(&root) || self.active.len() >= 128 {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "owned image has cyclic or excessive structural depth",
                ));
            }
            self.active.push(root);
            let types = self.execution.structural_types();
            let declaration = types.declaration(&root)?;
            let (size, alignment) = self.geometry.layout(root)?;
            let result = match &declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => self.scalar(*scalar),
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    let (stride, _) = self.geometry.layout(*element)?;
                    let mut bytes = zeroed(size, self.execution.account.as_ref())?;
                    if stride != 0 {
                        for index in 0..*length {
                            self.path.push(StructuralPathSegment::FixedIndex(index));
                            let element = self.structure(*element)?;
                            self.path.pop();
                            let start =
                                usize::try_from(index.checked_mul(stride).ok_or_else(malformed)?)
                                    .map_err(|_| malformed())?;
                            let end = start
                                .checked_add(element.bytes().len())
                                .ok_or_else(malformed)?;
                            bytes
                                .bytes_mut()
                                .get_mut(start..end)
                                .ok_or_else(malformed)?
                                .copy_from_slice(element.bytes());
                        }
                    }
                    Ok(bytes)
                }
                StructuralTypeShape::Record { fields, storage } => {
                    require_runtime_fields(&self.geometry, fields)?;
                    let layout = record_layout(
                        self.execution,
                        &self.geometry,
                        root,
                        fields,
                        storage,
                        size,
                        alignment,
                    )?;
                    let mut schemas = Vec::new();
                    let mut values = Vec::new();
                    for field in fields {
                        let name = format!("field:{:?}", field.id);
                        let (schema, value) = self.field(field, &name, None)?;
                        schemas.push(schema);
                        values.push(value);
                    }
                    materialize(
                        &layout,
                        &schemas,
                        &values,
                        self.byte_order,
                        self.execution.account.as_ref(),
                    )
                }
                StructuralTypeShape::Sum { cases } => self.sum(root, &[], cases),
                StructuralTypeShape::Mixed { fields, cases } => self.sum(root, fields, cases),
                _ => Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "owned image shape is outside the retained schema reflection contract",
                )),
            };
            self.active.pop();
            result
        }

        fn sum(
            &mut self,
            root: StructuralTypeId,
            common: &[StructuralFieldDeclaration],
            cases: &[terminal_psi::StructuralCaseDeclaration],
        ) -> Result<ByteSequenceStorage, TerminalInterpretError> {
            let layout = self.geometry.sum_layout(root)?;
            let selected = self.execution.resolve_live_projection(
                self.source.place,
                &self.path,
                self.source.access,
            )?;
            let contents = self.execution.structural_cases.require(
                &crate::boundary::values::StructuralRuntimePlace::from(&selected),
            )?;
            if contents.structural_type != root {
                return Err(malformed());
            }
            let case_position = cases
                .iter()
                .position(|case| case.id == contents.case)
                .ok_or_else(malformed)?;
            let case = &cases[case_position];
            let mut bytes = zeroed(layout.storage.byte_size, self.execution.account.as_ref())?;
            let width = self.geometry.tag_width()?;
            let tag = u64::try_from(case_position).map_err(|_| malformed())?;
            if width < 8 && tag >= (1_u64 << (width * 8)) {
                return Err(malformed());
            }
            let encoded = match self.byte_order {
                ByteOrder::LittleEndian => tag.to_le_bytes(),
                ByteOrder::BigEndian => tag.to_be_bytes(),
            };
            let encoded = match self.byte_order {
                ByteOrder::LittleEndian => &encoded[..width],
                ByteOrder::BigEndian => &encoded[8 - width..],
            };
            bytes
                .bytes_mut()
                .get_mut(..width)
                .ok_or_else(malformed)?
                .copy_from_slice(encoded);
            self.sum_fields(common, &layout.common_field_offsets, &mut bytes)?;
            self.path.push(StructuralPathSegment::Case(case.id));
            self.sum_fields(
                &case.fields,
                &layout.case_field_offsets[case_position],
                &mut bytes,
            )?;
            self.path.pop();
            Ok(bytes)
        }

        fn sum_fields(
            &mut self,
            fields: &[StructuralFieldDeclaration],
            offsets: &[u64],
            bytes: &mut ByteSequenceStorage,
        ) -> Result<(), TerminalInterpretError> {
            require_runtime_fields(&self.geometry, fields)?;
            if fields.len() != offsets.len() {
                return Err(malformed());
            }
            for (field, offset) in fields.iter().zip(offsets) {
                self.path.push(StructuralPathSegment::Field(field.identity));
                let value = match field.field_type.scalar_type() {
                    Some(scalar) => self.scalar(scalar),
                    None => self.structure(field.field_type.as_structural().ok_or_else(malformed)?),
                }?;
                self.path.pop();
                let start = usize::try_from(*offset).map_err(|_| malformed())?;
                let end = start
                    .checked_add(value.bytes().len())
                    .ok_or_else(malformed)?;
                bytes
                    .bytes_mut()
                    .get_mut(start..end)
                    .ok_or_else(malformed)?
                    .copy_from_slice(value.bytes());
            }
            Ok(())
        }
    }

    fn field_schema(
        execution: &TerminalExecution<'_>,
        geometry: &ImageGeometry<'_, '_>,
        field: &StructuralFieldDeclaration,
        name: &str,
        member_identity: Option<u64>,
    ) -> Result<AggregateFieldSchema, TerminalInterpretError> {
        let types = execution.structural_types();
        let size = match field.field_type.scalar_type() {
            Some(scalar) => terminal_semantics::reference_storage_byte_width(scalar).map(u64::from),
            None => Some(
                geometry
                    .layout(field.field_type.as_structural().ok_or_else(malformed)?)?
                    .0,
            ),
        }
        .ok_or_else(malformed)?;
        let repeated =
            field
                .field_type
                .as_structural()
                .and_then(|child| match types.get(&child)?.shape {
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } => {
                        let (size, alignment) = geometry.layout(element).ok()?;
                        Some((size, alignment, length))
                    }
                    _ => None,
                });
        match (repeated, member_identity) {
            (Some((size, alignment, count)), Some(identity)) => {
                AggregateFieldSchema::new_repeated_numbered(name, identity, size, alignment, count)
            }
            (Some((size, alignment, count)), None) => {
                AggregateFieldSchema::new_repeated(name, size, alignment, count)
            }
            (None, Some(identity)) => AggregateFieldSchema::new_numbered(name, identity, size),
            (None, None) => AggregateFieldSchema::new(name, size),
        }
        .map_err(TerminalInterpretError::ValueImage)
    }

    fn record_layout(
        execution: &TerminalExecution<'_>,
        geometry: &ImageGeometry<'_, '_>,
        root: StructuralTypeId,
        fields: &[StructuralFieldDeclaration],
        storage: &StructuralRecordStorage,
        size: u64,
        alignment: u64,
    ) -> Result<LayoutPlanReport, TerminalInterpretError> {
        let types = execution.structural_types();
        let mut layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            entries: Vec::new(),
            offsets: None,
            size: Some(size),
            align: alignment,
        };
        for field in fields {
            let name = format!("field:{:?}", field.id);
            let append = |layout: &mut LayoutPlanReport, placement| {
                layout.entries.push(LayoutFieldEntryReport {
                    field: name.clone(),
                    member_identity: None,
                    placement,
                });
            };
            match (storage, &field.storage) {
                (StructuralRecordStorage::Packed, _) => {
                    let region = geometry
                        .region(root, &[StructuralPathSegment::Field(field.identity)])
                        .ok_or_else(malformed)?;
                    append(
                        &mut layout,
                        LayoutPlacementReport::At {
                            offset: region.byte_offset,
                        },
                    );
                }
                (
                    StructuralRecordStorage::Authored(_),
                    Some(AuthoredFieldStorage::Fragmented(fragments)),
                ) => {
                    for fragment in fragments.fragments() {
                        append(
                            &mut layout,
                            LayoutPlacementReport::Bits {
                                container: fragment.container_byte_offset,
                                container_width: fragment.container_width_bits,
                                destination_lsb: fragment.destination_lsb,
                                source_lsb: fragment.source_lsb,
                                width: fragment.width,
                            },
                        );
                    }
                }
                (
                    StructuralRecordStorage::Authored(_),
                    Some(AuthoredFieldStorage::Repeated {
                        byte_offset,
                        element_stride,
                    }),
                ) => {
                    let child = field
                        .field_type
                        .as_structural()
                        .and_then(|child| types.get(&child))
                        .ok_or_else(malformed)?;
                    let StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(length),
                        ..
                    } = child.shape
                    else {
                        return Err(malformed());
                    };
                    for index in 0..length {
                        let offset = byte_offset
                            .checked_add(index.checked_mul(*element_stride).ok_or_else(malformed)?)
                            .ok_or_else(malformed)?;
                        append(&mut layout, LayoutPlacementReport::At { offset });
                    }
                }
                (
                    StructuralRecordStorage::Authored(_),
                    Some(AuthoredFieldStorage::Scalar {
                        byte_offset,
                        stored_scalar: Some(ScalarType::Integer(integer)),
                    }),
                ) => {
                    append(
                        &mut layout,
                        LayoutPlacementReport::IntegerAt {
                            offset: *byte_offset,
                            stored_width: u64::from(execution.closed_integer(*integer)?.bits()),
                            interpretation: match integer.sign() {
                                IntegerSign::Signed => IntegerInterpretation::Signed,
                                IntegerSign::Unsigned => IntegerInterpretation::Unsigned,
                            },
                        },
                    );
                }
                (StructuralRecordStorage::Authored(_), Some(placement)) => {
                    append(
                        &mut layout,
                        LayoutPlacementReport::At {
                            offset: placement.byte_offset().ok_or_else(malformed)?,
                        },
                    );
                }
                _ => {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "owned image record has no resolved storage recipe",
                    ));
                }
            }
        }
        Ok(layout)
    }

    fn require_runtime_fields(
        geometry: &ImageGeometry<'_, '_>,
        fields: &[StructuralFieldDeclaration],
    ) -> Result<(), TerminalInterpretError> {
        for field in fields {
            if field.relevance.is_erased()
                || field
                    .field_type
                    .as_structural()
                    .map(|child| geometry.layout(child))
                    .transpose()?
                    .is_some_and(|(size, _)| size == 0)
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "placement with non-runtime fields requires an authored custody agreement",
                ));
            }
        }
        Ok(())
    }

    fn zeroed(
        size: u64,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<ByteSequenceStorage, TerminalInterpretError> {
        let length = usize::try_from(size).map_err(|_| malformed())?;
        ByteSequenceStorage::zeroed(length, account)
    }

    fn materialize(
        layout: &LayoutPlanReport,
        schemas: &[AggregateFieldSchema],
        values: &[AggregateFieldValue<ByteSequenceStorage>],
        byte_order: ByteOrder,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<ByteSequenceStorage, TerminalInterpretError> {
        let mut bytes = zeroed(layout.size.ok_or_else(malformed)?, account)?;
        layout_plans::stage_aggregate_layout(
            layout,
            schemas,
            values,
            byte_order,
            bytes.bytes_mut(),
        )
        .map_err(TerminalInterpretError::ValueImage)?;
        Ok(bytes)
    }
}
pub(crate) mod view {
    use super::initialization::InitializationRejection;
    use super::{ProviderStorageBinding, RuntimeProviderStorage};
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use access_plans::{
        AccessResourceCompatibility, ValidatedAccessPlan, ValidatedResourceProfile,
    };

    impl RuntimeProviderStorage {
        pub(crate) fn prepare_view(
            &self,
            binding: ProviderStorageBinding,
            supplied: &ValidatedResourceProfile,
            placement: &ValidatedAccessPlan,
        ) -> Result<
            Result<AccessResourceCompatibility, InitializationRejection>,
            TerminalInterpretError,
        > {
            if !self.binding_is_live(binding) {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "borrowed placement lost its live original backing",
                ));
            }
            let (allocation, profile, base) = self.live_provider_allocation(&binding)?;
            let required = placement.layout().size.ok_or_else(malformed)?;
            if required > binding.length {
                return Ok(Err(InitializationRejection::RangeTooSmall {
                    required,
                    available: binding.length,
                }));
            }
            let loan = allocation
                .extent
                .loan(binding.offset, binding.length)
                .map_err(|reason| TerminalInterpretError::AccessReference(reason.to_string()))?;
            let admitted = profile
                .restrict_to_loan(&loan)
                .map_err(|reason| TerminalInterpretError::AccessReference(reason.to_string()))?;
            let mut actual = None;
            for profile in [supplied, &admitted] {
                let Ok(compatibility) = access_plans::validate_access_resources(placement, profile)
                else {
                    return Ok(Err(InitializationRejection::IncompatibleSupply));
                };
                let congruence = compatibility.base_congruence();
                if !congruence.admits(base) {
                    return Ok(Err(InitializationRejection::Misaligned {
                        modulus: congruence.modulus(),
                        residue: congruence.residue(),
                    }));
                }
                actual = Some(compatibility);
            }
            Ok(Ok(actual.ok_or_else(malformed)?))
        }
    }
}
pub use entry::{
    TerminalProviderStorageInput, TerminalProviderStorageRejection, TerminalReturnedProviderStorage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderStorageBinding {
    pub(crate) allocation: arena::Handle<ProviderStorageSlot>,
    pub(crate) offset: u64,
    pub(crate) length: u64,
    pub(crate) resident: arena::Handle<ResidentStorageSlot>,
}

#[derive(Default)]
pub(crate) enum ResidentStorageSlot {
    #[default]
    Empty,
    Live {
        window: ProviderStorageBinding,
        initialized_by: semantic_vocabulary::BoundaryMachineId,
        custody: crate::storage::record::owned_value::StagedOwnedValue,
        compatibility: access_plans::InitializationResourceCompatibility,
        _lease: Option<EvaluationStorageLease>,
    },
}

#[derive(Debug, Default)]
pub(crate) enum ProviderStorageSlot {
    #[default]
    Empty,
    Live {
        allocation: ProviderByteAllocation,
        profile: AdmittedResourceProfile,
        _lease: Option<EvaluationStorageLease>,
    },
}

pub(crate) struct RuntimeProviderStorage {
    pub(crate) allocations: arena::Arena<ProviderStorageSlot>,
    pub(crate) residents: arena::Arena<ResidentStorageSlot>,
    pub(crate) descriptors:
        RuntimeRows<crate::boundary::values::StructuralRuntimePlace, ProviderStorageBinding>,
    pub(crate) completed_profiles: RuntimeRows<
        crate::boundary::values::StructuralRuntimePlace,
        profiles::CompletedResourceProfile,
    >,
}

impl RuntimeProviderStorage {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            allocations: Default::default(),
            residents: Default::default(),
            descriptors: RuntimeRows::new(account),
            completed_profiles: RuntimeRows::new(account),
        }
    }

    pub(crate) fn binding_is_live(&self, binding: ProviderStorageBinding) -> bool {
        let ProviderStorageSlot::Live { allocation, .. } = self.allocations.get(binding.allocation)
        else {
            return false;
        };
        if binding
            .offset
            .checked_add(binding.length)
            .is_none_or(|end| end > allocation.extent.length())
        {
            return false;
        }
        if binding.resident != arena::Handle::default() {
            let ResidentStorageSlot::Live { window, .. } = self.residents.get(binding.resident)
            else {
                return false;
            };
            if *window
                != (ProviderStorageBinding {
                    resident: arena::Handle::default(),
                    ..binding
                })
            {
                return false;
            }
        }
        true
    }

    pub(crate) fn live_provider_allocation(
        &self,
        binding: &ProviderStorageBinding,
    ) -> Result<(&ProviderByteAllocation, &AdmittedResourceProfile, u64), TerminalInterpretError>
    {
        let ProviderStorageSlot::Live {
            allocation,
            profile,
            ..
        } = self.allocations.get(binding.allocation)
        else {
            return Err(malformed());
        };
        let base = allocation
            .extent
            .base()
            .checked_add(binding.offset)
            .ok_or_else(malformed)?;
        Ok((allocation, profile, base))
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn live_resident_custody(
        &self,
        resident: arena::Handle<ResidentStorageSlot>,
    ) -> Result<
        (
            &TerminalAccessLifecyclePlan,
            &StagedOwnedValue,
            &InitializationResourceCompatibility,
        ),
        TerminalInterpretError,
    > {
        let ResidentStorageSlot::Live {
            initialized_by,
            custody,
            compatibility,
            ..
        } = self.provider_storage.residents.get(resident)
        else {
            return Err(malformed());
        };
        let initialized = self
            .program
            .access_lifecycle_plans()
            .iter()
            .find(|plan| plan.boundary == *initialized_by)
            .ok_or_else(malformed)?;
        Ok((initialized, custody, compatibility))
    }
}

#[derive(Debug)]
pub(crate) struct ProviderByteAllocation {
    extent: Extent,
    bytes: ProviderBytes,
}

impl ProviderByteAllocation {
    pub(crate) fn extent(&self) -> &Extent {
        &self.extent
    }
}
