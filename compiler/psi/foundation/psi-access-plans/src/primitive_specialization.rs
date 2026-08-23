use super::*;

/// Operation subset accepted by ordinary Stable primitive lowering.
///
/// Compound mutation needs its distinct bounded read-patch-write realization;
/// External and atomic events retain their own transfer laws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StablePrimitiveOperation {
    Read,
    Write,
}

/// Stable-only consumer contract for a sealed primitive access.
///
/// The original request remains intact inside this carrier, retaining its
/// exact admission, profile, geometry, and lifetime authority. A future
/// interpreter or native execution binding adds its result/value operand and
/// target-owned storage realization to this already-specialized event.
#[derive(Debug)]
#[must_use = "Stable primitive access retains its exact placed authority"]
pub struct StablePrimitiveAccessRequest<'view, 'extent> {
    pub(super) request: PrimitiveAccessRequest<'view, 'extent>,
    pub(super) operation: StablePrimitiveOperation,
}

impl<'view, 'extent> StablePrimitiveAccessRequest<'view, 'extent> {
    pub const fn operation(&self) -> StablePrimitiveOperation {
        self.operation
    }

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

    /// Retained physical correspondence, when the originating placed view
    /// was provider-corresponded. This borrows the exact admitted fact; it
    /// does not copy provider/device identities or require correspondence for
    /// ordinary Stable storage.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this
    /// specialization. Consumers may inspect its complete placement and
    /// lifetime authority but cannot reconstruct, mutate, or respecialize it.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority and Stable
    /// operation specialization before an outward lowering consumer accepts
    /// this request. Rejection only borrows the carrier, so its exact loan,
    /// resident content, and authorization remain available for corrected
    /// retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let operation = validate_stable_primitive_request(&self.request)?;
        if operation != self.operation {
            return Err(AccessPlanDiagnostic(
                "Stable primitive lowering operation differs from its retained specialization"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed specialization returns the exact sealed request so its authority
/// and content-custody lifetime remain available to the caller.
#[derive(Debug)]
pub struct StablePrimitiveAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> StablePrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the narrow contract accepted by
    /// ordinary Stable read/write lowering.
    pub fn into_stable_primitive_access(
        self,
    ) -> Result<
        StablePrimitiveAccessRequest<'view, 'extent>,
        StablePrimitiveAccessRejection<'view, 'extent>,
    > {
        let operation = match validate_stable_primitive_request(&self) {
            Ok(operation) => operation,
            Err(diagnostic) => {
                return Err(StablePrimitiveAccessRejection {
                    request: self,
                    diagnostic,
                });
            }
        };
        Ok(StablePrimitiveAccessRequest {
            request: self,
            operation,
        })
    }
}

fn validate_stable_primitive_request(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> Result<StablePrimitiveOperation, AccessPlanDiagnostic> {
    if request.observation != ObservationModel::Stable {
        return Err(AccessPlanDiagnostic(
            "ordinary Stable lowering requires a Stable observation".into(),
        ));
    }
    if request.effective_supply.kind() != EffectiveSupplyKind::Stable {
        return Err(AccessPlanDiagnostic(
            "ordinary Stable lowering requires admitted Stable supply".into(),
        ));
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

/// Stable-only consumer contract for one bounded compound mutation.
///
/// This carrier remains distinct from an ordinary Stable read or write: its
/// consumer must realize one read-patch-write sequence over the complete
/// retained effect footprint without weakening either exclusive borrow.
#[derive(Debug)]
#[must_use = "Stable compound mutation retains its exact placed authority"]
pub struct StableCompoundMutationAccessRequest<'view, 'extent> {
    pub(super) request: PrimitiveAccessRequest<'view, 'extent>,
}

impl<'view, 'extent> StableCompoundMutationAccessRequest<'view, 'extent> {
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

    /// Retained physical correspondence, when present on the originating
    /// placed view. The bounded mutation specialization does not manufacture
    /// or require such evidence.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this bounded
    /// mutation specialization without weakening its exclusive authority.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority and bounded
    /// read-patch-write specialization before an outward lowering consumer
    /// accepts this request. Rejection only borrows the carrier, preserving
    /// its exact exclusive loan and resident-content custody for retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        validate_stable_compound_mutation_request(&self.request)
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed compound specialization returns the exact sealed request so its
/// content-custody lifetime and exclusive authority remain available.
#[derive(Debug)]
pub struct StableCompoundMutationAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> StableCompoundMutationAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the exact contract accepted by one
    /// bounded Stable read-patch-write realization.
    pub fn into_stable_compound_mutation_access(
        self,
    ) -> Result<
        StableCompoundMutationAccessRequest<'view, 'extent>,
        StableCompoundMutationAccessRejection<'view, 'extent>,
    > {
        if let Err(diagnostic) = validate_stable_compound_mutation_request(&self) {
            return Err(StableCompoundMutationAccessRejection {
                request: self,
                diagnostic,
            });
        }
        Ok(StableCompoundMutationAccessRequest { request: self })
    }
}

fn validate_stable_compound_mutation_request(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> Result<(), AccessPlanDiagnostic> {
    if request.observation != ObservationModel::Stable {
        return Err(AccessPlanDiagnostic(
            "Stable compound mutation requires a Stable observation".into(),
        ));
    }
    if request.effective_supply.kind() != EffectiveSupplyKind::Stable {
        return Err(AccessPlanDiagnostic(
            "Stable compound mutation requires admitted Stable supply".into(),
        ));
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

/// Operation subset accepted by one exact External primitive transfer.
///
/// Repeatable reads, destructive reads, and whole-container writes remain
/// distinct. External compound mutation and atomic operations have no member
/// in this closed lowering contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPrimitiveOperation {
    Read,
    Take,
    Write,
}

/// External-only consumer contract for one sealed primitive access.
///
/// A conservative External demand may have been satisfied by admitted Stable
/// supply, but the requested observation remains External: lowering must still
/// emit one non-elided exact-width transfer. The original request remains
/// intact inside this linear carrier.
#[derive(Debug)]
#[must_use = "External primitive access retains its exact placed authority"]
pub struct ExternalPrimitiveAccessRequest<'view, 'extent> {
    pub(super) request: PrimitiveAccessRequest<'view, 'extent>,
    pub(super) operation: ExternalPrimitiveOperation,
}

impl<'view, 'extent> ExternalPrimitiveAccessRequest<'view, 'extent> {
    pub const fn operation(&self) -> ExternalPrimitiveOperation {
        self.operation
    }

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

    /// Retained physical correspondence, when the originating placed view
    /// was provider-corresponded. This remains distinct from External supply
    /// compatibility and establishes no device operation.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this External
    /// specialization. The borrow exposes provenance for a later consumer but
    /// establishes no transfer or device operation.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority, admitted supply
    /// substitution, and exact External operation before an outward lowering
    /// consumer accepts this request. Rejection only borrows the carrier, so
    /// no external event occurs and the same request remains available for
    /// corrected retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let operation = validate_external_primitive_request(&self.request)?;
        if operation != self.operation {
            return Err(AccessPlanDiagnostic(
                "External primitive lowering operation differs from its retained specialization"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed External specialization returns the exact sealed request so its
/// range authority and content-custody lifetime remain available to the
/// caller.
#[derive(Debug)]
pub struct ExternalPrimitiveAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> ExternalPrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the narrow contract accepted by one
    /// exact External read, destructive read, or whole-container write.
    pub fn into_external_primitive_access(
        self,
    ) -> Result<
        ExternalPrimitiveAccessRequest<'view, 'extent>,
        ExternalPrimitiveAccessRejection<'view, 'extent>,
    > {
        let operation = match validate_external_primitive_request(&self) {
            Ok(operation) => operation,
            Err(diagnostic) => {
                return Err(ExternalPrimitiveAccessRejection {
                    request: self,
                    diagnostic,
                });
            }
        };
        Ok(ExternalPrimitiveAccessRequest {
            request: self,
            operation,
        })
    }
}

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

/// Atomic operation and proof-static ordering accepted by primitive lowering.
///
/// Each family remains distinct, including the independent success and
/// failure orderings of compare-exchange. No ordinary read, write, or
/// synthesized retry operation has a member in this closed contract.
#[derive(Debug)]
#[must_use = "Atomic primitive access retains its exact placed authority"]
pub struct AtomicPrimitiveAccessRequest<'view, 'extent> {
    pub(super) request: PrimitiveAccessRequest<'view, 'extent>,
    pub(super) operation: AtomicAccessOperation,
}

impl<'view, 'extent> AtomicPrimitiveAccessRequest<'view, 'extent> {
    pub const fn operation(&self) -> AtomicAccessOperation {
        self.operation
    }

    pub const fn ordering_plan(&self) -> AtomicOrderingPlan {
        self.operation.ordering_plan()
    }

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

    /// Retained physical correspondence, when present on the originating
    /// placed view. Atomic specialization neither manufactures nor requires
    /// this separate provider-issued fact.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this Atomic
    /// specialization without weakening its operation or ordering identity.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority, admitted Atomic
    /// supply, operation family, and ordering law before an outward lowering
    /// consumer accepts this request. Rejection only borrows the carrier; it
    /// performs no atomic attempt and preserves the same request for retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let operation = validate_atomic_primitive_request(&self.request)?;
        if operation != self.operation {
            return Err(AccessPlanDiagnostic(
                "Atomic primitive lowering operation differs from its retained specialization"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed Atomic specialization returns the exact sealed request so its range
/// authority and operation-specific custody remain available to the caller.
#[derive(Debug)]
pub struct AtomicPrimitiveAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> AtomicPrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the narrow contract accepted by one
    /// exact, explicitly admitted Atomic operation.
    pub fn into_atomic_primitive_access(
        self,
    ) -> Result<
        AtomicPrimitiveAccessRequest<'view, 'extent>,
        AtomicPrimitiveAccessRejection<'view, 'extent>,
    > {
        let operation = match validate_atomic_primitive_request(&self) {
            Ok(operation) => operation,
            Err(diagnostic) => {
                return Err(AtomicPrimitiveAccessRejection {
                    request: self,
                    diagnostic,
                });
            }
        };
        Ok(AtomicPrimitiveAccessRequest {
            request: self,
            operation,
        })
    }
}

fn validate_atomic_primitive_request(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> Result<AtomicAccessOperation, AccessPlanDiagnostic> {
    if request.observation != ObservationModel::Atomic {
        return Err(AccessPlanDiagnostic(
            "Atomic lowering requires an Atomic observation".into(),
        ));
    }
    if request.effective_supply.kind() != EffectiveSupplyKind::Atomic {
        return Err(AccessPlanDiagnostic(
            "Atomic lowering requires explicitly admitted Atomic supply".into(),
        ));
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
