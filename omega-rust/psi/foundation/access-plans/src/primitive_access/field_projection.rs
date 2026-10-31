//! Pure placed-field projection and authorization judgments.

use extents::ResidentClaimId;
use language_core::atomic::MemoryOrdering;

use crate::placements::access_authority::AccessAuthorityRef;
use crate::plan_policy::authorization::authorize_descriptor;
use crate::{
    AccessAdmissionId, AccessFieldKey, AccessOccurrenceId, AccessOperation, AccessPlanDiagnostic,
    AccessPlanId, AccessResourceCompatibility, AdmittedSchemaDeviceCorrespondence,
    AtomicAccessOperation, AuthorizedFieldAccess, BorrowPolarity, BoundaryReach, EffectFootprint,
    EffectiveFieldSupply, ObservationModel, PrimitiveAccessRequest, ResourceProfileReceiptId,
    ValidatedAccessPlan,
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
            "access field projection arguments do not match the retained access authority".into(),
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

/// Pure field projection from one access view.
///
/// This carrier is deliberately not `Clone`: re-projecting is the explicit
/// route to another accessor. Named operation methods are the only route from
/// this pure projection to a sealed memory event.
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

    pub fn atomic_load<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Load(
            ordering,
        )))
    }

    pub fn atomic_store<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Store(
            ordering,
        )))
    }

    pub fn atomic_fetch_add<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(
            ordering,
        )))
    }

    pub fn atomic_fetch_sub<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchSub(
            ordering,
        )))
    }

    pub fn atomic_fetch_xor<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchXor(
            ordering,
        )))
    }

    pub fn atomic_fetch_or<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchOr(
            ordering,
        )))
    }

    pub fn atomic_fetch_and<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchAnd(
            ordering,
        )))
    }

    pub fn atomic_swap<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<AccessFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Swap(
            ordering,
        )))
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
            || placement.access().field_descriptor(self.descriptor.key()) != Some(&self.descriptor)
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
        if descriptor_address != self.primitive_address || supply_address != self.primitive_address
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

/// Sealed lowering input carrying both authorized field geometry and the
/// exact borrowed or established-owned authority from which its polarity was
/// derived.
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

    /// Consume one authorized access event into the only request primitive
    /// lowering accepts.
    ///
    /// The request remains bound to the normalized plan, exact admission and
    /// source loan, address, width, observation model, operation ordering, and
    /// static service reach that produced it. It contains no author-supplied
    /// offset.
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
