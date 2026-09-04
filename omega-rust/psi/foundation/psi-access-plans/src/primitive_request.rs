use psi_extents::ResidentClaimId;

use super::{
    AccessOperation, AccessPlanDiagnostic, AdmittedSchemaDeviceCorrespondence, BorrowPolarity,
    BoundaryReach, EffectFootprint, EffectiveFieldSupply, LogicalFieldExtent, ObservationModel,
    PlacedOccurrenceId, PlacementAdmissionId, PlacementPlanId, PrimitiveAccessRequest,
    ResourceProfileReceiptId, authorize_descriptor, effect_footprints_conflict,
};

impl PrimitiveAccessRequest<'_, '_> {
    pub const fn plan(&self) -> PlacementPlanId {
        self.plan
    }

    pub const fn admission(&self) -> PlacementAdmissionId {
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

    /// Whether this event and another event require mutually exclusive effect
    /// footprints.
    ///
    /// Repeatable reads may share an overlapping transfer container. Atomic
    /// events may share only the same exact atomic container; a partial or
    /// mixed-width overlap rejects. Every destructive read, ordinary write,
    /// or stable read-modify-write reserves its complete transfer footprint.
    pub const fn conflicts_with(&self, other: &Self) -> bool {
        effect_footprints_conflict(
            self.effect_footprint,
            self.operation,
            other.effect_footprint,
            other.operation,
        )
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

    pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
        self.resident_claim
    }

    pub const fn placed_occurrence(&self) -> Option<PlacedOccurrenceId> {
        self.placed_occurrence
    }

    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self._authority.correspondence()
    }

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
                    "primitive lowering supply offset overflows the retained authority base".into(),
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
            || self.effect_footprint.length_bytes != self.descriptor.effect_footprint.length_bytes
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
        let placement = authority.placement_plan();
        if placement.identity() != self.plan
            || authority.profile_receipt() != self.profile_receipt
            || authority.profile().receipt() != self.profile_receipt
            || authority.admission() != self.admission
            || placement.reach() != &self.reach
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the copied plan, profile, admission, and reach to match the retained placement authority"
                    .into(),
            ));
        }

        if authority.resources().placement != placement.identity()
            || authority.resources().field(self.key) != Some(&self.effective_supply)
            || placement.access().field_descriptor(self.key) != Some(&self.descriptor)
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the retained resource row and field descriptor to belong to the exact placement authority"
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
                "primitive lowering replayed resource compatibility differs from the retained placement authority"
                    .into(),
            ));
        }
        authority.replay_correspondence("primitive lowering")?;
        authority.replay_resident_content("primitive lowering")?;

        if authority.source_loan() != self.source_loan
            || authority.resident_claim() != self.resident_claim
            || authority.placed_occurrence() != self.placed_occurrence
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires source-loan and resident identities to match the retained placement authority"
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
