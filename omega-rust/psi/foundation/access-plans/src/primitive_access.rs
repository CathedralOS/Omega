/*
We project placed access views into field accessors (`field_projection`), seal
primitive access requests (`primitive_request`), specialize them for stable,
atomic, and external lowering (`primitive_specialization`), and bind physical
device correspondence (`corresponded_stable`, `corresponded_stable_compound`,
`corresponded_atomic`, `corresponded_external`).
*/

macro_rules! corresponded_primitive_access {
    (
        $request:ident,
        $rejection:ident,
        $access:ident,
        $accessor:ident,
        $into_access:ident,
        $require:ident,
        $validator:ident,
        must_use = $must_use:literal,
        specialization = $specialization:literal,
        placed_doc = $placed_doc:literal,
        require_doc = $require_doc:literal,
        storage_doc = $storage_doc:literal,
        rejection_event = $rejection_event:literal,
        required_message = $required_message:literal,
        retained_message = $retained_message:literal $(,)?
    ) => {
        #[doc = concat!("One exact ", $specialization, " specialization joined to the physical correspondence retained by its originating access view.")]
        #[derive(Debug)]
        #[must_use = $must_use]
        pub struct $request<'view, 'extent> {
            access: $access<'view, 'extent>,
            correspondence: &'view AdmittedSchemaDeviceCorrespondence,
        }

        impl<'view, 'extent> $request<'view, 'extent> {
            #[doc = concat!("The exact ", $specialization, " specialization retained by this custody join.")]
            pub const fn $accessor(&self) -> &$access<'view, 'extent> {
                &self.access
            }

            pub const fn correspondence(&self) -> &'view AdmittedSchemaDeviceCorrespondence {
                self.correspondence
            }

            #[doc = concat!("Replay both the complete ", $specialization, " specialization and its exact correspondence identity before a provider/device consumer proceeds. Rejection only borrows this carrier, so no ", $rejection_event, " occurs and the complete input remains available for repair and retry.")]
            pub fn validate_for_provider_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
                let replayed = $validator(&self.access)?;
                if !std::ptr::eq(replayed, self.correspondence) {
                    return Err(AccessPlanDiagnostic($retained_message.into()));
                }
                Ok(())
            }

            #[doc = concat!("Remove only this correspondence-required staging layer. The original ", $placed_doc)]
            pub fn $into_access(self) -> $access<'view, 'extent> {
                self.access
            }

            #[cfg(test)]
            pub(crate) fn replace_request_plan_for_test(
                &mut self,
                placement: AccessPlanId,
            ) -> AccessPlanId {
                std::mem::replace(&mut self.access.request.plan, placement)
            }

            #[cfg(test)]
            pub(crate) fn replace_correspondence_for_test(
                &mut self,
                correspondence: &'view AdmittedSchemaDeviceCorrespondence,
            ) -> &'view AdmittedSchemaDeviceCorrespondence {
                std::mem::replace(&mut self.correspondence, correspondence)
            }
        }

        #[doc = concat!("Failed correspondence-required staging returns the exact already- specialized ", $specialization, " request. No provider/device operation is selected or attempted.")]
        #[derive(Debug)]
        pub struct $rejection<'view, 'extent> {
            access: $access<'view, 'extent>,
            diagnostic: AccessPlanDiagnostic,
        }

        impl<'view, 'extent> $rejection<'view, 'extent> {
            diagnostic_accessor!();

            pub fn into_parts(
                self,
            ) -> ($access<'view, 'extent>, AccessPlanDiagnostic) {
                (self.access, self.diagnostic)
            }
        }

        impl<'view, 'extent> $access<'view, 'extent> {
            #[doc = concat!("Require the exact schema/device correspondence retained by this ", $require_doc)]
            #[doc = concat!("Ordinary correspondence-free ", $storage_doc)]
            pub fn $require(self) -> Result<$request<'view, 'extent>, $rejection<'view, 'extent>> {
                let correspondence = match $validator(&self) {
                    Ok(correspondence) => correspondence,
                    Err(diagnostic) => {
                        return Err($rejection {
                            access: self,
                            diagnostic,
                        });
                    }
                };
                Ok($request {
                    access: self,
                    correspondence,
                })
            }
        }

        fn $validator<'access, 'view, 'extent>(
            access: &'access $access<'view, 'extent>,
        ) -> Result<&'view AdmittedSchemaDeviceCorrespondence, AccessPlanDiagnostic> {
            access.validate_for_lowering()?;
            access
                .request
                ._authority
                .correspondence()
                .ok_or_else(|| AccessPlanDiagnostic($required_message.into()))
        }
    };
}

macro_rules! atomic_ordering_accessors {
    ($($method:ident => $operation:ident),+ $(,)?) => {
        $(
            pub fn $method<'access>(
                &'access self,
                ordering: MemoryOrdering,
            ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
                self.authorize(AccessOperation::Atomic(AtomicAccessOperation::$operation(
                    ordering,
                )))
            }
        )+
    };
}

macro_rules! authority_accessors {
    () => {
        pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
            self.resident_claim
        }

        pub const fn access_occurrence(&self) -> Option<AccessOccurrenceId> {
            self.access_occurrence
        }

        pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
            self._authority.correspondence()
        }
    };
}

pub(crate) mod corresponded_atomic {
    #[cfg(test)]
    use crate::AccessPlanId;
    use crate::{
        AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, AtomicPrimitiveAccessRequest,
    };

    corresponded_primitive_access!(
        CorrespondedAtomicPrimitiveAccessRequest,
        CorrespondedAtomicPrimitiveAccessRejection,
        AtomicPrimitiveAccessRequest,
        atomic_access,
        into_atomic_access,
        into_corresponded_atomic_access,
        validate_corresponded_atomic_access,
        must_use = "corresponded Atomic access retains placed and physical provenance",
        specialization = "Atomic",
        placed_doc = "Atomic specialization and all of its placed authority remain intact.",
        require_doc = "Atomic request before handing it to a provider/device-specific consumer.",
        storage_doc = "atomic storage rejects and returns this complete specialization unchanged.",
        rejection_event = "atomic attempt",
        required_message =
            "provider/device Atomic lowering requires admitted schema/device correspondence",
        retained_message = "corresponded Atomic lowering retained a different schema/device correspondence authority",
    );
}
pub(crate) mod corresponded_external {
    #[cfg(test)]
    use crate::AccessPlanId;
    use crate::{
        AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, ExternalPrimitiveAccessRequest,
    };

    corresponded_primitive_access!(
        CorrespondedExternalPrimitiveAccessRequest,
        CorrespondedExternalPrimitiveAccessRejection,
        ExternalPrimitiveAccessRequest,
        external_access,
        into_external_access,
        into_corresponded_external_access,
        validate_corresponded_external_access,
        must_use = "corresponded External access retains placed and physical provenance",
        specialization = "External",
        placed_doc = "External specialization and all of its placed authority remain intact.",
        require_doc = "External request before handing it to a provider/device-specific consumer.",
        storage_doc =
            "External storage rejects and returns this complete specialization unchanged.",
        rejection_event = "transfer",
        required_message =
            "provider/device External lowering requires admitted schema/device correspondence",
        retained_message = "corresponded External lowering retained a different schema/device correspondence authority",
    );
}
pub(crate) mod corresponded_stable {
    #[cfg(test)]
    use crate::AccessPlanId;
    use crate::{
        AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, StablePrimitiveAccessRequest,
    };

    corresponded_primitive_access!(
        CorrespondedStablePrimitiveAccessRequest,
        CorrespondedStablePrimitiveAccessRejection,
        StablePrimitiveAccessRequest,
        stable_access,
        into_stable_access,
        into_corresponded_stable_access,
        validate_corresponded_stable_access,
        must_use = "corresponded Stable access retains placed and physical provenance",
        specialization = "Stable",
        placed_doc = "Stable specialization and all of its placed authority remain intact.",
        require_doc = "Stable request before handing it to a provider/device-specific consumer.",
        storage_doc = "Stable storage rejects and returns this complete specialization unchanged.",
        rejection_event = "memory event",
        required_message =
            "provider/device Stable lowering requires admitted schema/device correspondence",
        retained_message = "corresponded Stable lowering retained a different schema/device correspondence authority",
    );
}
pub(crate) mod corresponded_stable_compound {
    #[cfg(test)]
    use crate::AccessPlanId;
    use crate::{
        AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence,
        StableCompoundMutationAccessRequest,
    };

    corresponded_primitive_access!(
        CorrespondedStableCompoundMutationAccessRequest,
        CorrespondedStableCompoundMutationAccessRejection,
        StableCompoundMutationAccessRequest,
        compound_access,
        into_compound_access,
        into_corresponded_stable_compound_access,
        validate_corresponded_stable_compound_access,
        must_use = "corresponded Stable compound access retains placed and physical provenance",
        specialization = "bounded Stable compound",
        placed_doc = "bounded compound specialization and its exclusive authority remain intact.",
        require_doc =
            "bounded compound request before handing it to a provider/device-specific consumer.",
        storage_doc = "Stable storage rejects and returns this complete specialization unchanged.",
        rejection_event = "read-patch-write",
        required_message = "provider/device Stable compound lowering requires admitted schema/device correspondence",
        retained_message = "corresponded Stable compound lowering retained a different schema/device correspondence authority",
    );
}
pub(crate) mod field_projection {
    use extents::ResidentClaimId;
    use language_core::atomic::MemoryOrdering;

    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::plan_policy::authorization::authorize_descriptor;
    use crate::{
        AccessAdmissionId, AccessFieldKey, AccessOccurrenceId, AccessOperation,
        AccessPlanDiagnostic, AccessPlanId, AccessResourceCompatibility,
        AdmittedSchemaDeviceCorrespondence, AtomicAccessOperation, AuthorizedFieldAccess,
        BorrowPolarity, BoundaryReach, EffectFootprint, EffectiveFieldSupply, ObservationModel,
        PrimitiveAccessRequest, ResourceProfileReceiptId, ValidatedAccessPlan,
    };

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn project_access_field<'view, 'extent>(
        plan: &ValidatedAccessPlan,
        profile_receipt: ResourceProfileReceiptId,
        resources: &AccessResourceCompatibility,
        admission: AccessAdmissionId,
        base: u64,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        required_observation: Option<ObservationModel>,
        authority: AccessAuthorityRef<'view, 'extent>,
    ) -> Result<AccessFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        if authority.access_plan() != plan
            || authority.profile_receipt() != profile_receipt
            || authority.resources() != resources
            || authority.admission() != admission
            || authority.base() != base
            || authority.source_loan() != source_loan
        {
            return Err(AccessPlanDiagnostic(
                "access field projection arguments do not match the retained access authority"
                    .into(),
            ));
        }
        if authority.profile().receipt() != profile_receipt {
            return Err(AccessPlanDiagnostic(
                "access field projection profile receipt differs from its retained admitted profile"
                    .into(),
            ));
        }
        let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "access field projection could not replay the retained access authority: {diagnostic}"
            ))
        })?;
        if &replayed_resources != resources {
            return Err(AccessPlanDiagnostic(
                "access field projection replayed resource compatibility differs from the retained authority"
                    .into(),
            ));
        }
        authority.replay_correspondence("access field projection")?;
        authority.replay_resident_content("access field projection")?;
        let descriptor = plan.access.field_descriptor(key).cloned().ok_or_else(|| {
            AccessPlanDiagnostic(format!(
                "field key in canonical slot {} does not expose a accessor",
                key.slot()
            ))
        })?;
        if let Some(required) = required_observation
            && descriptor.observation() != required
        {
            return Err(AccessPlanDiagnostic(format!(
                "field `{}` observation is not valid for established owned {:?} access",
                descriptor.field(),
                required
            )));
        }
        let supply = resources.field(key).ok_or_else(|| {
            AccessPlanDiagnostic(format!(
                "field `{}` has no sealed resource compatibility",
                descriptor.field()
            ))
        })?;
        let primitive_address = base
            .checked_add(descriptor.container_byte_offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{}` primitive address overflows address width",
                    descriptor.field()
                ))
            })?;
        Ok(AccessFieldProjection {
            descriptor,
            current_borrow,
            source_loan,
            primitive_address,
            plan: plan.identity(),
            profile_receipt,
            supply: supply.clone(),
            reach: plan.reach.clone(),
            admission,
            resident_claim: authority.resident_claim(),
            access_occurrence: authority.access_occurrence(),
            _authority: authority,
        })
    }

    #[derive(Debug)]
    pub struct AccessFieldProjection<'view, 'extent> {
        descriptor: crate::access_fields::FieldAccessDescriptor,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        pub(crate) primitive_address: u64,
        pub(crate) plan: AccessPlanId,
        profile_receipt: ResourceProfileReceiptId,
        pub(crate) supply: EffectiveFieldSupply,
        reach: BoundaryReach,
        admission: AccessAdmissionId,
        pub(crate) resident_claim: Option<ResidentClaimId>,
        pub(crate) access_occurrence: Option<AccessOccurrenceId>,
        pub(crate) _authority: AccessAuthorityRef<'view, 'extent>,
    }

    impl<'view, 'extent> AccessFieldProjection<'view, 'extent> {
        pub fn field(&self) -> &str {
            self.descriptor.field()
        }

        pub const fn key(&self) -> AccessFieldKey {
            self.descriptor.key()
        }

        pub const fn primitive_address(&self) -> u64 {
            self.primitive_address
        }

        pub const fn observation(&self) -> ObservationModel {
            self.descriptor.observation()
        }

        authority_accessors!();

        pub fn read<'access>(
            &'access self,
        ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
            self.authorize(AccessOperation::Read)
        }

        pub fn take<'access>(
            &'access mut self,
        ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
            self.authorize(AccessOperation::Take)
        }

        pub fn write<'access>(
            &'access mut self,
        ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
            self.authorize(AccessOperation::Write)
        }

        pub fn compound_mutation<'access>(
            &'access mut self,
        ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
            self.authorize(AccessOperation::CompoundMutation)
        }

        atomic_ordering_accessors! {
            atomic_load => Load,
            atomic_store => Store,
            atomic_fetch_add => FetchAdd,
            atomic_fetch_sub => FetchSub,
            atomic_fetch_xor => FetchXor,
            atomic_fetch_or => FetchOr,
            atomic_fetch_and => FetchAnd,
            atomic_swap => Swap,
        }

        pub fn atomic_compare_exchange<'access>(
            &'access self,
            success: MemoryOrdering,
            failure: MemoryOrdering,
        ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
            self.authorize(AccessOperation::Atomic(
                AtomicAccessOperation::CompareExchange { success, failure },
            ))
        }

        pub fn atomic_compare_exchange_once<'access>(
            &'access self,
            success: MemoryOrdering,
            failure: MemoryOrdering,
        ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
            self.authorize(AccessOperation::Atomic(
                AtomicAccessOperation::CompareExchangeOnce { success, failure },
            ))
        }

        fn validate_authority_binding(&self) -> Result<(), AccessPlanDiagnostic> {
            let authority = self._authority;
            let placement = authority.access_plan();
            if placement.identity() != self.plan
                || authority.profile_receipt() != self.profile_receipt
                || authority.profile().receipt() != self.profile_receipt
                || authority.admission() != self.admission
                || placement.reach() != &self.reach
                || authority.source_loan() != self.source_loan
                || authority.resident_claim() != self.resident_claim
                || authority.access_occurrence() != self.access_occurrence
            {
                return Err(AccessPlanDiagnostic(
                    "access field authorization requires copied placement, profile, admission, reach, loan, and resident identities to match the retained authority"
                        .into(),
                ));
            }

            let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "access field authorization could not replay the retained access authority: {diagnostic}"
                ))
            })?;
            if &replayed_resources != authority.resources()
                || authority.resources().placement != placement.identity()
                || authority.resources().field(self.descriptor.key()) != Some(&self.supply)
                || placement.access().field_descriptor(self.descriptor.key())
                    != Some(&self.descriptor)
            {
                return Err(AccessPlanDiagnostic(
                    "access field authorization requires the exact replayed resource row and descriptor from the retained access authority"
                        .into(),
                ));
            }
            authority.replay_correspondence("access field authorization")?;
            authority.replay_resident_content("access field authorization")?;

            let descriptor_address = authority
                .base()
                .checked_add(self.descriptor.container_byte_offset())
                .ok_or_else(|| {
                    AccessPlanDiagnostic(
                        "access field authorization descriptor address overflows the retained authority base"
                            .into(),
                    )
                })?;
            let supply_address = authority
                .base()
                .checked_add(self.supply.offset())
                .ok_or_else(|| {
                    AccessPlanDiagnostic(
                        "access field authorization supply address overflows the retained authority base"
                            .into(),
                    )
                })?;
            if descriptor_address != self.primitive_address
                || supply_address != self.primitive_address
            {
                return Err(AccessPlanDiagnostic(
                    "access field authorization requires descriptor and supply geometry to reproduce the projected primitive address"
                        .into(),
                ));
            }
            Ok(())
        }

        fn authorize<'access>(
            &'access self,
            operation: AccessOperation,
        ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
            self.validate_authority_binding()?;
            authorize_descriptor(
                &self.descriptor,
                self.current_borrow,
                self.source_loan,
                operation,
            )?;
            Ok(AccessFieldAccess {
                access: AuthorizedFieldAccess {
                    descriptor: self.descriptor.clone(),
                    current_borrow: self.current_borrow,
                    source_loan: self.source_loan,
                    operation,
                },
                primitive_address: self.primitive_address,
                plan: self.plan,
                profile_receipt: self.profile_receipt,
                supply: self.supply.clone(),
                reach: self.reach.clone(),
                admission: self.admission,
                resident_claim: self.resident_claim,
                access_occurrence: self.access_occurrence,
                _authority: self._authority,
            })
        }
    }

    #[derive(Debug)]
    pub struct AccessFieldAccess<'view, 'extent> {
        access: AuthorizedFieldAccess,
        primitive_address: u64,
        plan: AccessPlanId,
        profile_receipt: ResourceProfileReceiptId,
        supply: EffectiveFieldSupply,
        reach: BoundaryReach,
        admission: AccessAdmissionId,
        resident_claim: Option<ResidentClaimId>,
        access_occurrence: Option<AccessOccurrenceId>,
        _authority: AccessAuthorityRef<'view, 'extent>,
    }

    impl<'view, 'extent> AccessFieldAccess<'view, 'extent> {
        pub const fn access(&self) -> &AuthorizedFieldAccess {
            &self.access
        }

        pub const fn primitive_address(&self) -> u64 {
            self.primitive_address
        }

        authority_accessors!();

        pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
            let descriptor = self.access.descriptor.clone();
            PrimitiveAccessRequest {
                plan: self.plan,
                profile_receipt: self.profile_receipt,
                effective_supply: self.supply,
                admission: self.admission,
                primitive_address: self.primitive_address,
                key: descriptor.key,
                field: descriptor.field.clone(),
                transfer_width_bits: descriptor.transfer_width_bits,
                logical_extent: descriptor.logical_extent.clone(),
                effect_footprint: EffectFootprint {
                    address: self.primitive_address,
                    length_bytes: descriptor.effect_footprint.length_bytes,
                },
                observation: descriptor.observation,
                current_borrow: self.access.current_borrow,
                source_loan: self.access.source_loan,
                operation: self.access.operation,
                reach: self.reach,
                resident_claim: self.resident_claim,
                access_occurrence: self.access_occurrence,
                descriptor,
                authorization: self.access,
                _authority: self._authority,
            }
        }
    }
}
pub(crate) mod primitive_request {
    use extents::ResidentClaimId;

    use crate::placements::access_authority::AccessAuthorityRef;
    use crate::plan_policy::authorization::authorize_descriptor;
    use crate::{
        AccessAdmissionId, AccessOccurrenceId, AccessOperation, AccessPlanDiagnostic, AccessPlanId,
        AdmittedSchemaDeviceCorrespondence, BorrowPolarity, BoundaryReach, EffectFootprint,
        EffectiveFieldSupply, LogicalFieldExtent, ObservationModel, ResourceProfileReceiptId,
    };
    use crate::{AccessFieldKey, AuthorizedFieldAccess, FieldAccessDescriptor};

    #[derive(Debug)]
    pub struct PrimitiveAccessRequest<'view, 'extent> {
        pub(crate) plan: AccessPlanId,
        pub(crate) profile_receipt: ResourceProfileReceiptId,
        pub(crate) effective_supply: EffectiveFieldSupply,
        pub(crate) admission: AccessAdmissionId,
        pub(crate) primitive_address: u64,
        pub(crate) key: AccessFieldKey,
        pub(crate) field: String,
        pub(crate) transfer_width_bits: u16,
        pub(crate) logical_extent: LogicalFieldExtent,
        pub(crate) effect_footprint: EffectFootprint,
        pub(crate) observation: ObservationModel,
        pub(crate) current_borrow: BorrowPolarity,
        pub(crate) source_loan: BorrowPolarity,
        pub(crate) operation: AccessOperation,
        pub(crate) reach: BoundaryReach,
        pub(crate) resident_claim: Option<ResidentClaimId>,
        pub(crate) access_occurrence: Option<AccessOccurrenceId>,
        pub(crate) descriptor: FieldAccessDescriptor,
        pub(crate) authorization: AuthorizedFieldAccess,
        pub(crate) _authority: AccessAuthorityRef<'view, 'extent>,
    }

    impl PrimitiveAccessRequest<'_, '_> {
        pub const fn plan(&self) -> AccessPlanId {
            self.plan
        }

        pub const fn admission(&self) -> AccessAdmissionId {
            self.admission
        }

        pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
            self.profile_receipt
        }

        pub const fn effective_supply(&self) -> &EffectiveFieldSupply {
            &self.effective_supply
        }

        pub const fn primitive_address(&self) -> u64 {
            self.primitive_address
        }

        pub fn field(&self) -> &str {
            &self.field
        }

        pub const fn transfer_width_bits(&self) -> u16 {
            self.transfer_width_bits
        }

        pub const fn logical_extent(&self) -> &LogicalFieldExtent {
            &self.logical_extent
        }

        pub const fn effect_footprint(&self) -> EffectFootprint {
            self.effect_footprint
        }

        pub const fn observation(&self) -> ObservationModel {
            self.observation
        }

        pub const fn current_borrow(&self) -> BorrowPolarity {
            self.current_borrow
        }

        pub const fn source_loan(&self) -> BorrowPolarity {
            self.source_loan
        }

        pub const fn operation(&self) -> AccessOperation {
            self.operation
        }

        pub const fn reach(&self) -> &BoundaryReach {
            &self.reach
        }

        authority_accessors!();

        pub(super) fn validate_effective_supply_binding(&self) -> Result<(), AccessPlanDiagnostic> {
            if self.effective_supply.key != self.key
                || self.effective_supply.field != self.field
                || self.effective_supply.width_bits != self.transfer_width_bits
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires the admitted supply key and width to match the sealed request, including its field identity"
                        .into(),
                ));
            }

            let expected_address = self
                ._authority
                .base()
                .checked_add(self.effective_supply.offset)
                .ok_or_else(|| {
                    AccessPlanDiagnostic(
                        "primitive lowering supply offset overflows the retained authority base"
                            .into(),
                    )
                })?;
            if expected_address != self.primitive_address {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires the admitted supply offset to reproduce the sealed primitive address"
                        .into(),
                ));
            }

            let alignment = self.effective_supply.alignment_bytes;
            if alignment == 0
                || !alignment.is_power_of_two()
                || !self.primitive_address.is_multiple_of(alignment)
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires the admitted supply alignment to hold at the sealed primitive address"
                        .into(),
                ));
            }
            Ok(())
        }

        pub(super) fn validate_descriptor_binding(&self) -> Result<(), AccessPlanDiagnostic> {
            if self.descriptor.key != self.key
                || self.descriptor.field != self.field
                || self.descriptor.container_byte_offset != self.effective_supply.offset
                || self.descriptor.transfer_width_bits != self.transfer_width_bits
                || self.descriptor.logical_extent != self.logical_extent
                || self.descriptor.observation != self.observation
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires the copied request facts to match the retained validated field descriptor"
                        .into(),
                ));
            }

            let expected_footprint_address = self
                ._authority
                .base()
                .checked_add(self.descriptor.effect_footprint.byte_offset)
                .ok_or_else(|| {
                    AccessPlanDiagnostic(
                        "primitive lowering descriptor footprint overflows the retained authority base"
                            .into(),
                    )
                })?;
            if self.effect_footprint.address != expected_footprint_address
                || self.effect_footprint.length_bytes
                    != self.descriptor.effect_footprint.length_bytes
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires the concrete effect footprint to match the retained validated field descriptor"
                        .into(),
                ));
            }

            authorize_descriptor(
                &self.descriptor,
                self.current_borrow,
                self.source_loan,
                self.operation,
            )
        }

        pub(super) fn validate_authority_binding(&self) -> Result<(), AccessPlanDiagnostic> {
            let authority = self._authority;
            let placement = authority.access_plan();
            if placement.identity() != self.plan
                || authority.profile_receipt() != self.profile_receipt
                || authority.profile().receipt() != self.profile_receipt
                || authority.admission() != self.admission
                || placement.reach() != &self.reach
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires the copied plan, profile, admission, and reach to match the retained access authority"
                        .into(),
                ));
            }

            if authority.resources().placement != placement.identity()
                || authority.resources().field(self.key) != Some(&self.effective_supply)
                || placement.access().field_descriptor(self.key) != Some(&self.descriptor)
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires the retained resource row and field descriptor to belong to the exact access authority"
                        .into(),
                ));
            }

            let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "primitive lowering could not replay the retained admitted resource profile: {diagnostic}"
                ))
            })?;
            if &replayed_resources != authority.resources() {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering replayed resource compatibility differs from the retained access authority"
                        .into(),
                ));
            }
            authority.replay_correspondence("primitive lowering")?;
            authority.replay_resident_content("primitive lowering")?;

            if authority.source_loan() != self.source_loan
                || authority.resident_claim() != self.resident_claim
                || authority.access_occurrence() != self.access_occurrence
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires source-loan and resident identities to match the retained access authority"
                        .into(),
                ));
            }
            Ok(())
        }

        pub(super) fn validate_authorization_binding(&self) -> Result<(), AccessPlanDiagnostic> {
            if self.authorization.descriptor != self.descriptor
                || self.authorization.current_borrow != self.current_borrow
                || self.authorization.source_loan != self.source_loan
                || self.authorization.operation != self.operation
            {
                return Err(AccessPlanDiagnostic(
                    "primitive lowering requires copied operation and borrow facts to match the retained field authorization"
                        .into(),
                ));
            }
            authorize_descriptor(
                &self.authorization.descriptor,
                self.authorization.current_borrow,
                self.authorization.source_loan,
                self.authorization.operation,
            )
        }
    }
}
pub(crate) mod primitive_specialization {
    use crate::access_fields::diagnostic::reject_when;
    use crate::plan_policy::authorization::validate_operation_ordering;
    use crate::{
        AccessOperation, AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence,
        AtomicAccessOperation, BorrowPolarity, EffectFootprint, EffectiveSupplyKind,
        LogicalFieldExtent, ObservationModel, PrimitiveAccessRequest, ValidatedAccessPlan,
    };
    use language_core::atomic::AtomicOrderingPlan;

    macro_rules! rejected_specialization {
        ($rejection:ident, $rejection_doc:literal) => {
            #[doc = $rejection_doc]
            #[derive(Debug)]
            pub struct $rejection<'view, 'extent> {
                request: PrimitiveAccessRequest<'view, 'extent>,
                diagnostic: AccessPlanDiagnostic,
            }

            impl<'view, 'extent> $rejection<'view, 'extent> {
                diagnostic_accessor!();

                pub fn into_parts(
                    self,
                ) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
                    (self.request, self.diagnostic)
                }
            }
        };
    }

    macro_rules! specialization_boundary {
        ($request:ident, $rejection:ident, $require:ident, $validator:ident,
         $rejection_doc:literal, $require_doc:literal,
         with_operation($operation:ident)) => {
            rejected_specialization!($rejection, $rejection_doc);

            impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
                #[doc = $require_doc]
                pub fn $require(
                    self,
                ) -> Result<$request<'view, 'extent>, $rejection<'view, 'extent>> {
                    let $operation = match $validator(&self) {
                        Ok(operation) => operation,
                        Err(diagnostic) => {
                            return Err($rejection {
                                request: self,
                                diagnostic,
                            });
                        }
                    };
                    Ok($request {
                        request: self,
                        $operation,
                    })
                }
            }
        };
        ($request:ident, $rejection:ident, $require:ident, $validator:ident,
         $rejection_doc:literal, $require_doc:literal,
         no_operation) => {
            rejected_specialization!($rejection, $rejection_doc);

            impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
                #[doc = $require_doc]
                pub fn $require(
                    self,
                ) -> Result<$request<'view, 'extent>, $rejection<'view, 'extent>> {
                    if let Err(diagnostic) = $validator(&self) {
                        return Err($rejection {
                            request: self,
                            diagnostic,
                        });
                    }
                    Ok($request { request: self })
                }
            }
        };
    }

    macro_rules! replayed_operation_lowering {
        ($validator:ident, $message:literal, $doc:literal) => {
            #[doc = $doc]
            pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
                let operation = $validator(&self.request)?;
                if operation != self.operation {
                    return Err(AccessPlanDiagnostic($message.into()));
                }
                Ok(())
            }
        };
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum StablePrimitiveOperation {
        Read,
        Write,
    }

    macro_rules! primitive_request_read_surface {
        () => {
            pub const fn primitive_address(&self) -> u64 {
                self.request.primitive_address
            }

            pub const fn transfer_width_bits(&self) -> u16 {
                self.request.transfer_width_bits
            }

            pub const fn logical_extent(&self) -> &LogicalFieldExtent {
                &self.request.logical_extent
            }

            pub const fn effect_footprint(&self) -> EffectFootprint {
                self.request.effect_footprint
            }

            pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
                self.request.correspondence()
            }

            pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
                &self.request
            }
        };
    }

    #[derive(Debug)]
    #[must_use = "Stable primitive access retains its exact placed authority"]
    pub struct StablePrimitiveAccessRequest<'view, 'extent> {
        pub(crate) request: PrimitiveAccessRequest<'view, 'extent>,
        pub(crate) operation: StablePrimitiveOperation,
    }

    impl<'view, 'extent> StablePrimitiveAccessRequest<'view, 'extent> {
        pub const fn operation(&self) -> StablePrimitiveOperation {
            self.operation
        }

        primitive_request_read_surface!();
        replayed_operation_lowering!(
            validate_stable_primitive_request,
            "Stable primitive lowering operation differs from its retained specialization",
            "Independently replay the complete placed authority and Stable operation specialization before an outward lowering consumer accepts this request. Rejection only borrows the carrier, so its exact loan, resident content, and authorization remain available for corrected retry."
        );

        pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
            self.request
        }
    }

    specialization_boundary!(
        StablePrimitiveAccessRequest,
        StablePrimitiveAccessRejection,
        into_stable_primitive_access,
        validate_stable_primitive_request,
        "Failed specialization returns the exact sealed request so its authority and content-custody lifetime remain available to the caller.",
        "Consume this general request into the narrow contract accepted by ordinary Stable read/write lowering.",
        with_operation(operation)
    );

    fn validate_stable_primitive_request(
        request: &PrimitiveAccessRequest<'_, '_>,
    ) -> Result<StablePrimitiveOperation, AccessPlanDiagnostic> {
        reject_when! {
            request.observation != ObservationModel::Stable
                => "ordinary Stable lowering requires a Stable observation",
            request.effective_supply.kind() != EffectiveSupplyKind::Stable
                => "ordinary Stable lowering requires admitted Stable supply",
        }
        let operation = match request.operation {
            AccessOperation::Read => StablePrimitiveOperation::Read,
            AccessOperation::Write => StablePrimitiveOperation::Write,
            _ => {
                return Err(AccessPlanDiagnostic(
                    "ordinary Stable lowering accepts only one sealed Read or Write event".into(),
                ));
            }
        };
        request.validate_effective_supply_binding()?;
        request.validate_descriptor_binding()?;
        request.validate_authority_binding()?;
        request.validate_authorization_binding()?;
        Ok(operation)
    }

    #[derive(Debug)]
    #[must_use = "Stable compound mutation retains its exact placed authority"]
    pub struct StableCompoundMutationAccessRequest<'view, 'extent> {
        pub(crate) request: PrimitiveAccessRequest<'view, 'extent>,
    }

    impl<'view, 'extent> StableCompoundMutationAccessRequest<'view, 'extent> {
        primitive_request_read_surface!();

        pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
            validate_stable_compound_mutation_request(&self.request)
        }

        pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
            self.request
        }
    }

    specialization_boundary!(
        StableCompoundMutationAccessRequest,
        StableCompoundMutationAccessRejection,
        into_stable_compound_mutation_access,
        validate_stable_compound_mutation_request,
        "Failed compound specialization returns the exact sealed request so its content-custody lifetime and exclusive authority remain available.",
        "Consume this general request into the exact contract accepted by one bounded Stable read-patch-write realization.",
        no_operation
    );

    fn validate_stable_compound_mutation_request(
        request: &PrimitiveAccessRequest<'_, '_>,
    ) -> Result<(), AccessPlanDiagnostic> {
        reject_when! {
            request.observation != ObservationModel::Stable
                => "Stable compound mutation requires a Stable observation",
            request.effective_supply.kind() != EffectiveSupplyKind::Stable
                => "Stable compound mutation requires admitted Stable supply",
        }
        request.validate_effective_supply_binding()?;
        if request.current_borrow != BorrowPolarity::Exclusive
            || request.source_loan != BorrowPolarity::Exclusive
        {
            return Err(AccessPlanDiagnostic(
                "Stable compound mutation requires exclusive current and source borrows".into(),
            ));
        }
        if request.operation != AccessOperation::CompoundMutation {
            return Err(AccessPlanDiagnostic(
                "Stable compound lowering accepts only one sealed CompoundMutation event".into(),
            ));
        }
        request.validate_descriptor_binding()?;
        request.validate_authority_binding()?;
        request.validate_authorization_binding()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExternalPrimitiveOperation {
        Read,
        Take,
        Write,
    }

    #[derive(Debug)]
    #[must_use = "External primitive access retains its exact placed authority"]
    pub struct ExternalPrimitiveAccessRequest<'view, 'extent> {
        pub(crate) request: PrimitiveAccessRequest<'view, 'extent>,
        pub(crate) operation: ExternalPrimitiveOperation,
    }

    impl<'view, 'extent> ExternalPrimitiveAccessRequest<'view, 'extent> {
        pub const fn operation(&self) -> ExternalPrimitiveOperation {
            self.operation
        }

        primitive_request_read_surface!();
        replayed_operation_lowering!(
            validate_external_primitive_request,
            "External primitive lowering operation differs from its retained specialization",
            "Independently replay the complete placed authority, admitted supply substitution, and exact External operation before an outward lowering consumer accepts this request. Rejection only borrows the carrier, so no external event occurs and the same request remains available for corrected retry."
        );

        pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
            self.request
        }
    }

    specialization_boundary!(
        ExternalPrimitiveAccessRequest,
        ExternalPrimitiveAccessRejection,
        into_external_primitive_access,
        validate_external_primitive_request,
        "Failed External specialization returns the exact sealed request so its range authority and content-custody lifetime remain available to the caller.",
        "Consume this general request into the narrow contract accepted by one exact External read, destructive read, or whole-container write.",
        with_operation(operation)
    );

    fn validate_external_primitive_request(
        request: &PrimitiveAccessRequest<'_, '_>,
    ) -> Result<ExternalPrimitiveOperation, AccessPlanDiagnostic> {
        if request.observation != ObservationModel::External {
            return Err(AccessPlanDiagnostic(
                "External lowering requires an External observation".into(),
            ));
        }
        let operation = match request.operation {
            AccessOperation::Read => ExternalPrimitiveOperation::Read,
            AccessOperation::Take => ExternalPrimitiveOperation::Take,
            AccessOperation::Write => ExternalPrimitiveOperation::Write,
            AccessOperation::CompoundMutation | AccessOperation::Atomic(_) => {
                return Err(AccessPlanDiagnostic(
                    "External lowering accepts only one sealed Read, Take, or Write event".into(),
                ));
            }
        };
        let supply_is_compatible = match request.effective_supply.kind() {
            EffectiveSupplyKind::External => true,
            EffectiveSupplyKind::Stable => matches!(
                operation,
                ExternalPrimitiveOperation::Read | ExternalPrimitiveOperation::Write
            ),
            EffectiveSupplyKind::Atomic => false,
        };
        if !supply_is_compatible {
            return Err(AccessPlanDiagnostic(
                "External lowering requires admitted External supply, or conservative Stable supply for Read or Write"
                    .into(),
            ));
        }
        request.validate_effective_supply_binding()?;
        request.validate_descriptor_binding()?;
        request.validate_authority_binding()?;
        request.validate_authorization_binding()?;
        Ok(operation)
    }

    #[derive(Debug)]
    #[must_use = "Atomic primitive access retains its exact placed authority"]
    pub struct AtomicPrimitiveAccessRequest<'view, 'extent> {
        pub(crate) request: PrimitiveAccessRequest<'view, 'extent>,
        pub(crate) operation: AtomicAccessOperation,
    }

    impl<'view, 'extent> AtomicPrimitiveAccessRequest<'view, 'extent> {
        pub const fn operation(&self) -> AtomicAccessOperation {
            self.operation
        }

        pub const fn ordering_plan(&self) -> AtomicOrderingPlan {
            self.operation.ordering_plan()
        }

        primitive_request_read_surface!();
        replayed_operation_lowering!(
            validate_atomic_primitive_request,
            "Atomic primitive lowering operation differs from its retained specialization",
            "Independently replay the complete placed authority, admitted Atomic supply, operation family, and ordering law before an outward lowering consumer accepts this request. Rejection only borrows the carrier; it performs no atomic attempt and preserves the same request for retry."
        );

        pub fn validate_against_checked_access_plan(
            &self,
            checked: &ValidatedAccessPlan,
        ) -> Result<(), AccessPlanDiagnostic> {
            self.validate_for_lowering()?;
            if self.request._authority.access_plan() != checked {
                return Err(AccessPlanDiagnostic(
                    "Atomic primitive access retained placement structure differs from the independently checked placement"
                        .into(),
                ));
            }
            Ok(())
        }

        pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
            self.request
        }
    }

    specialization_boundary!(
        AtomicPrimitiveAccessRequest,
        AtomicPrimitiveAccessRejection,
        into_atomic_primitive_access,
        validate_atomic_primitive_request,
        "Failed Atomic specialization returns the exact sealed request so its range authority and operation-specific custody remain available to the caller.",
        "Consume this general request into the narrow contract accepted by one exact, explicitly admitted Atomic operation.",
        with_operation(operation)
    );

    fn validate_atomic_primitive_request(
        request: &PrimitiveAccessRequest<'_, '_>,
    ) -> Result<AtomicAccessOperation, AccessPlanDiagnostic> {
        reject_when! {
            request.observation != ObservationModel::Atomic
                => "Atomic lowering requires an Atomic observation",
            request.effective_supply.kind() != EffectiveSupplyKind::Atomic
                => "Atomic lowering requires explicitly admitted Atomic supply",
        }
        request.validate_effective_supply_binding()?;
        let AccessOperation::Atomic(operation) = request.operation else {
            return Err(AccessPlanDiagnostic(
                "Atomic lowering accepts only one sealed Atomic operation".into(),
            ));
        };
        validate_operation_ordering(request.operation)?;
        request.validate_descriptor_binding()?;
        request.validate_authority_binding()?;
        request.validate_authorization_binding()?;
        Ok(operation)
    }
}
