//! Pure placed-field projection and authorization judgments.

use psi_extents::ResidentClaimId;
use psi_language_core::atomic::MemoryOrdering;

use super::{
    AccessFieldKey, AccessOperation, AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence,
    AtomicAccessOperation, AuthorizedFieldAccess, BorrowPolarity, BoundaryReach, EffectFootprint,
    EffectiveFieldSupply, ObservationModel, PlacedOccurrenceId, PlacementAdmissionId,
    PlacementAuthorityRef, PlacementPlanId, PlacementResourceCompatibility, PrimitiveAccessRequest,
    ResourceProfileReceiptId, ValidatedPlacementPlan, authorize_descriptor,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn project_placed_field<'view, 'extent>(
    plan: &ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    resources: &PlacementResourceCompatibility,
    admission: PlacementAdmissionId,
    base: u64,
    key: AccessFieldKey,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    required_observation: Option<ObservationModel>,
    authority: PlacementAuthorityRef<'view, 'extent>,
) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
    if authority.placement_plan() != plan
        || authority.profile_receipt() != profile_receipt
        || authority.resources() != resources
        || authority.admission() != admission
        || authority.base() != base
        || authority.source_loan() != source_loan
    {
        return Err(AccessPlanDiagnostic(
            "placed field projection arguments do not match the retained placement authority"
                .into(),
        ));
    }
    if authority.profile().receipt() != profile_receipt {
        return Err(AccessPlanDiagnostic(
            "placed field projection profile receipt differs from its retained admitted profile"
                .into(),
        ));
    }
    let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "placed field projection could not replay the retained placement authority: {diagnostic}"
        ))
    })?;
    if &replayed_resources != resources {
        return Err(AccessPlanDiagnostic(
            "placed field projection replayed resource compatibility differs from the retained authority"
                .into(),
        ));
    }
    authority.replay_correspondence("placed field projection")?;
    authority.replay_resident_content("placed field projection")?;
    let descriptor = plan.access.field_descriptor(key).cloned().ok_or_else(|| {
        AccessPlanDiagnostic(format!(
            "field key in canonical slot {} does not expose a placed accessor",
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
    Ok(PlacedFieldProjection {
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
        placed_occurrence: authority.placed_occurrence(),
        _authority: authority,
    })
}

/// Pure field projection from one placed view.
///
/// This carrier is deliberately not `Clone`: re-projecting is the explicit
/// route to another accessor. Named operation methods are the only route from
/// this pure projection to a sealed memory event.
#[derive(Debug)]
pub struct PlacedFieldProjection<'view, 'extent> {
    descriptor: super::FieldAccessDescriptor,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    pub(super) primitive_address: u64,
    pub(super) plan: PlacementPlanId,
    profile_receipt: ResourceProfileReceiptId,
    pub(super) supply: EffectiveFieldSupply,
    reach: BoundaryReach,
    admission: PlacementAdmissionId,
    pub(super) resident_claim: Option<ResidentClaimId>,
    pub(super) placed_occurrence: Option<PlacedOccurrenceId>,
    pub(super) _authority: PlacementAuthorityRef<'view, 'extent>,
}

impl<'view, 'extent> PlacedFieldProjection<'view, 'extent> {
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

    pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
        self.resident_claim
    }

    pub const fn placed_occurrence(&self) -> Option<PlacedOccurrenceId> {
        self.placed_occurrence
    }

    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self._authority.correspondence()
    }

    pub fn read<'access>(
        &'access self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Read)
    }

    pub fn take<'access>(
        &'access mut self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Take)
    }

    pub fn write<'access>(
        &'access mut self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Write)
    }

    pub fn compound_mutation<'access>(
        &'access mut self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::CompoundMutation)
    }

    pub fn atomic_load<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Load(
            ordering,
        )))
    }

    pub fn atomic_store<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Store(
            ordering,
        )))
    }

    pub fn atomic_fetch_add<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(
            ordering,
        )))
    }

    pub fn atomic_fetch_sub<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchSub(
            ordering,
        )))
    }

    pub fn atomic_fetch_xor<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchXor(
            ordering,
        )))
    }

    pub fn atomic_fetch_or<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchOr(
            ordering,
        )))
    }

    pub fn atomic_fetch_and<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchAnd(
            ordering,
        )))
    }

    pub fn atomic_swap<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Swap(
            ordering,
        )))
    }

    pub fn atomic_compare_exchange<'access>(
        &'access self,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(
            AtomicAccessOperation::CompareExchange { success, failure },
        ))
    }

    pub fn atomic_compare_exchange_once<'access>(
        &'access self,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(
            AtomicAccessOperation::CompareExchangeOnce { success, failure },
        ))
    }

    fn validate_authority_binding(&self) -> Result<(), AccessPlanDiagnostic> {
        let authority = self._authority;
        let placement = authority.placement_plan();
        if placement.identity() != self.plan
            || authority.profile_receipt() != self.profile_receipt
            || authority.profile().receipt() != self.profile_receipt
            || authority.admission() != self.admission
            || placement.reach() != &self.reach
            || authority.source_loan() != self.source_loan
            || authority.resident_claim() != self.resident_claim
            || authority.placed_occurrence() != self.placed_occurrence
        {
            return Err(AccessPlanDiagnostic(
                "placed field authorization requires copied placement, profile, admission, reach, loan, and resident identities to match the retained authority"
                    .into(),
            ));
        }

        let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "placed field authorization could not replay the retained placement authority: {diagnostic}"
            ))
        })?;
        if &replayed_resources != authority.resources()
            || authority.resources().placement != placement.identity()
            || authority.resources().field(self.descriptor.key()) != Some(&self.supply)
            || placement.access().field_descriptor(self.descriptor.key()) != Some(&self.descriptor)
        {
            return Err(AccessPlanDiagnostic(
                "placed field authorization requires the exact replayed resource row and descriptor from the retained placement authority"
                    .into(),
            ));
        }
        authority.replay_correspondence("placed field authorization")?;
        authority.replay_resident_content("placed field authorization")?;

        let descriptor_address = authority
            .base()
            .checked_add(self.descriptor.container_byte_offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(
                    "placed field authorization descriptor address overflows the retained authority base"
                        .into(),
                )
            })?;
        let supply_address = authority
            .base()
            .checked_add(self.supply.offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(
                    "placed field authorization supply address overflows the retained authority base"
                        .into(),
                )
            })?;
        if descriptor_address != self.primitive_address || supply_address != self.primitive_address
        {
            return Err(AccessPlanDiagnostic(
                "placed field authorization requires descriptor and supply geometry to reproduce the projected primitive address"
                    .into(),
            ));
        }
        Ok(())
    }

    fn authorize<'access>(
        &'access self,
        operation: AccessOperation,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.validate_authority_binding()?;
        authorize_descriptor(
            &self.descriptor,
            self.current_borrow,
            self.source_loan,
            operation,
        )?;
        Ok(PlacedFieldAccess {
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
            placed_occurrence: self.placed_occurrence,
            _authority: self._authority,
        })
    }
}

/// Sealed lowering input carrying both authorized field geometry and the
/// exact borrowed or established-owned authority from which its polarity was
/// derived.
#[derive(Debug)]
pub struct PlacedFieldAccess<'view, 'extent> {
    access: AuthorizedFieldAccess,
    primitive_address: u64,
    plan: PlacementPlanId,
    profile_receipt: ResourceProfileReceiptId,
    supply: EffectiveFieldSupply,
    reach: BoundaryReach,
    admission: PlacementAdmissionId,
    resident_claim: Option<ResidentClaimId>,
    placed_occurrence: Option<PlacedOccurrenceId>,
    _authority: PlacementAuthorityRef<'view, 'extent>,
}

impl<'view, 'extent> PlacedFieldAccess<'view, 'extent> {
    pub const fn access(&self) -> &AuthorizedFieldAccess {
        &self.access
    }

    pub const fn primitive_address(&self) -> u64 {
        self.primitive_address
    }

    pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
        self.resident_claim
    }

    pub const fn placed_occurrence(&self) -> Option<PlacedOccurrenceId> {
        self.placed_occurrence
    }

    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self._authority.correspondence()
    }

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
            placed_occurrence: self.placed_occurrence,
            descriptor,
            authorization: self.access,
            _authority: self._authority,
        }
    }
}
