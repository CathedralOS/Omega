/*
Structural ownership, multiplicity (`StructuralMultiplicity`), access capabilities
(`StructuralAccess`, `ReferenceAccessCapability`), placed loan regions, affine cleanup actions
(`TerminalAffineCleanupAction`), and claim transfer contracts in `TerminalModule`.
*/

pub(crate) mod access {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralMultiplicity {
        Unrestricted,
        Affine,
        Linear,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralAccess {
        Owned,
        SharedBorrow,
        MutableBorrow,
        WriteOnlyBorrow,
    }

    #[macro_export]
    macro_rules! reading_borrow_pair_pattern {
        () => {
            $crate::StructuralAccess::SharedBorrow | $crate::StructuralAccess::MutableBorrow
        };
    }

    #[macro_export]
    macro_rules! exclusive_borrow_pair_pattern {
        () => {
            $crate::StructuralAccess::MutableBorrow | $crate::StructuralAccess::WriteOnlyBorrow
        };
    }

    #[macro_export]
    macro_rules! borrowed_access_pair_pattern {
        () => {
            $crate::ownership::StructuralAccess::SharedBorrow
                | $crate::ownership::StructuralAccess::MutableBorrow
        };
    }

    #[macro_export]
    macro_rules! mutating_access_pair_pattern {
        () => {
            $crate::ownership::StructuralAccess::MutableBorrow
                | $crate::ownership::StructuralAccess::Owned
        };
    }

    #[macro_export]
    macro_rules! nonlinear_multiplicity_pair_pattern {
        () => {
            $crate::ownership::StructuralMultiplicity::Affine
                | $crate::ownership::StructuralMultiplicity::Unrestricted
        };
    }

    impl StructuralAccess {
        /// Forms a loan without transferring ownership of the granted storage.
        pub fn admits(self, granted: StructuralAccess) -> bool {
            self != Self::Owned && structural_access_covers(granted, self)
        }

        /// Attenuates a referent's loan; owning its reference carrier grants no referent ownership.
        pub fn admits_referent(self, referent: StructuralAccess) -> bool {
            referent != Self::Owned && self.admits(referent)
        }
    }

    /// Compares access modes without establishing custody, lifetime, or storage validity.
    /// Callers must separately check those obligations and whether the requested operation
    /// reads contents, mutates storage, or transfers ownership.
    pub fn structural_access_covers(
        available: StructuralAccess,
        requested: StructuralAccess,
    ) -> bool {
        available == requested
            || available == StructuralAccess::Owned
            || available == StructuralAccess::MutableBorrow
                && matches!(
                    requested,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                )
    }

    #[cfg(test)]
    mod tests {
        use super::{StructuralAccess, structural_access_covers};
        use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};

        const MODES: [StructuralAccess; 4] = [Owned, SharedBorrow, MutableBorrow, WriteOnlyBorrow];

        #[test]
        fn structural_access_coverage_preserves_read_and_write_limits() {
            let expected = [
                [true, true, true, true],
                [false, true, false, false],
                [false, true, true, true],
                [false, false, false, true],
            ];
            for (available_index, available) in MODES.into_iter().enumerate() {
                for (requested_index, requested) in MODES.into_iter().enumerate() {
                    assert_eq!(
                        structural_access_covers(available, requested),
                        expected[available_index][requested_index],
                        "{available:?} -> {requested:?}"
                    );
                }
            }
        }

        #[test]
        fn structural_access_borrows_never_manufacture_ownership() {
            let loans = [
                [false, true, true, true],
                [false, true, false, false],
                [false, true, true, true],
                [false, false, false, true],
            ];
            let referents = [
                [false, false, false, false],
                [false, true, false, false],
                [false, true, true, true],
                [false, false, false, true],
            ];
            for (available_index, available) in MODES.into_iter().enumerate() {
                for (requested_index, requested) in MODES.into_iter().enumerate() {
                    assert_eq!(
                        requested.admits(available),
                        loans[available_index][requested_index],
                        "loan {available:?} -> {requested:?}"
                    );
                    assert_eq!(
                        requested.admits_referent(available),
                        referents[available_index][requested_index],
                        "referent {available:?} -> {requested:?}"
                    );
                }
            }
        }
    }
}
pub(crate) mod borrows {
    use crate::terminal_module::TerminalIdentityIndex;
    use crate::{StructuralAccess, StructuralContentProjection, StructuralMultiplicity};
    use semantic_vocabulary::{DomainSemanticId, MachineId, OperationId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalReborrowRestorationClass {
        ExclusiveReactivation,
        SharedFreezeRestoration,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalReborrowSharedCohortMember {
        pub child_owner_identity: TerminalIdentityIndex,
        pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
        pub child_place: TerminalBorrowPlace,
        pub child_access: StructuralAccess,
        pub child_activation: TerminalBorrowBoundarySource,
        pub child_weakening: TerminalBorrowBoundarySource,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalBorrowBoundarySource {
        Statement {
            statement_index: u64,
        },
        Call {
            statement_index: u64,
            call_ordinal: u64,
            target_identity: TerminalIdentityIndex,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalBorrowOwnerSegment {
        Field(TerminalIdentityIndex),
        Case(TerminalIdentityIndex),
        FixedIndex(u64),
        DynamicIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalBorrowPlaceSegment {
        Field(TerminalIdentityIndex),
        Case(TerminalIdentityIndex),
        FixedIndex(u64),
        FixedRange { start: u64, end: u64 },
        CapturedSelector(TerminalIdentityIndex),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalBorrowPlace {
        pub root_identity: TerminalIdentityIndex,
        pub segments: Vec<TerminalBorrowPlaceSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalReborrowRestoredCallUse {
        pub machine: MachineId,
        pub operation: OperationId,
        pub restoration_class: TerminalReborrowRestorationClass,
        pub call_boundary: TerminalBorrowBoundarySource,
        pub call_target_machine: MachineId,
        pub source_machine_identity: TerminalIdentityIndex,
        pub source_state_identity: TerminalIdentityIndex,
        pub direct_root_owner_identity: TerminalIdentityIndex,
        pub direct_root_owner_path: Vec<TerminalBorrowOwnerSegment>,
        pub direct_root_place: TerminalBorrowPlace,
        pub direct_root_activation: TerminalBorrowBoundarySource,
        pub direct_root_weakening: TerminalBorrowBoundarySource,
        pub direct_root_lifetime_identity: TerminalIdentityIndex,
        pub child_owner_identity: TerminalIdentityIndex,
        pub child_owner_path: Vec<TerminalBorrowOwnerSegment>,
        pub child_place: TerminalBorrowPlace,
        pub projection_remainder: Vec<TerminalBorrowPlaceSegment>,
        pub child_access: StructuralAccess,
        pub child_activation: TerminalBorrowBoundarySource,
        pub formation_boundary: TerminalBorrowBoundarySource,
        pub child_weakening: TerminalBorrowBoundarySource,
        pub shared_cohort: Vec<TerminalReborrowSharedCohortMember>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct RetainedBorrowCustody {
        pub callable_identity: TerminalIdentityIndex,
        pub source: RetainedBorrowPlace,
        pub result: RetainedBorrowPlace,
        pub access: StructuralAccess,
        pub callable_lifetime_parameter_count: u32,
        pub callable_lifetime_parameter_ordinal: u32,
        pub result_nominal_identity: TerminalIdentityIndex,
        pub result_multiplicity: StructuralMultiplicity,
        pub result_lifetime_argument_count: u32,
        pub result_lifetime_argument_ordinal: u32,
        pub result_lifetime_slot_is_erased: bool,
        pub retained_semantic_domain: DomainSemanticId,
        pub source_projection: RetainedBorrowContentProjection,
        pub result_projection: RetainedBorrowContentProjection,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct RetainedBorrowContentProjection {
        pub semantic_domain: DomainSemanticId,
        pub carrier_identity: TerminalIdentityIndex,
        pub projection: StructuralContentProjection,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct RetainedBorrowPlace {
        pub version: semantic_vocabulary::ContentPlaceVersion,
        pub root: RetainedBorrowPlaceRoot,
        pub segments: Vec<semantic_vocabulary::ContentPlaceSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum RetainedBorrowPlaceRoot {
        Parameter {
            position: u32,
            identity: TerminalIdentityIndex,
        },
        Result,
    }
}
pub(crate) mod claims {
    use crate::StructuralPathSegment;
    use semantic_vocabulary::{ClaimId, PlaceId};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LoanCompletion {
        pub operation: semantic_vocabulary::OperationId,
        pub place: PlaceId,
        pub path: Vec<StructuralPathSegment>,
        pub multiplicity: crate::StructuralMultiplicity,
        pub claims: Vec<crate::StructuralResultClaimBinding>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EntryClaim {
        pub claim: ClaimId,
        pub input: PlaceId,
        pub path: Vec<StructuralPathSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ClaimTransfer {
        pub claim: ClaimId,
        pub argument_index: u32,
        pub path: Vec<StructuralPathSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CompletionReceipt {
        pub claim: ClaimId,
        pub argument_index: u32,
        pub path: Vec<StructuralPathSegment>,
    }
}
pub(crate) mod cleanup {
    use crate::StructuralPathSegment;
    use semantic_vocabulary::{ClaimId, MachineId, ObligationId, PlaceId, StructuralTypeId};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralAffineDiscard {
        pub place: PlaceId,
        pub path: Vec<StructuralPathSegment>,
        pub structural_type: StructuralTypeId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct NominalAffineCleanup {
        pub retains_descendants: bool,
        pub place: PlaceId,
        pub path: Vec<StructuralPathSegment>,
        pub structural_type: StructuralTypeId,
        pub cleanup_machine: MachineId,
        pub consumed_claims: Vec<ClaimId>,
        pub cleanup_receiver: Option<PlaceId>,
        pub requirement_obligations: Vec<ObligationId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalAffineCleanupAction {
        DiscardRoot(PlaceId),
        DiscardResidual(StructuralAffineDiscard),
        InvokeNominal(NominalAffineCleanup),
        RetireHome(PlaceId),
    }

    impl TerminalAffineCleanupAction {
        pub fn path(&self) -> &[StructuralPathSegment] {
            match self {
                Self::DiscardRoot(_) | Self::RetireHome(_) => &[],
                Self::DiscardResidual(discard) => &discard.path,
                Self::InvokeNominal(cleanup) => &cleanup.path,
            }
        }

        pub fn invoked_machine(&self) -> Option<MachineId> {
            match self {
                Self::InvokeNominal(cleanup) => Some(cleanup.cleanup_machine),
                _ => None,
            }
        }

        pub fn place(&self) -> PlaceId {
            match self {
                Self::DiscardRoot(place) | Self::RetireHome(place) => *place,
                Self::DiscardResidual(discard) => discard.place,
                Self::InvokeNominal(cleanup) => cleanup.place,
            }
        }
    }
}
pub(crate) mod placement {
    use crate::StructuralAccess;
    use crate::terminal_module::TerminalIdentityIndex;
    use semantic_vocabulary::MachineId;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalAccessViewInput {
        pub machine: MachineId,
        pub position: u32,
        pub runtime_position: u32,
        pub carrier_identity: TerminalIdentityIndex,
        pub source_machine_identity: TerminalIdentityIndex,
        pub source_state_identity: TerminalIdentityIndex,
        pub source_parameter_identity: TerminalIdentityIndex,
        pub access: StructuralAccess,
        pub binding_is_const: bool,
        pub binding_is_mutable: bool,
        pub view_identity: TerminalIdentityIndex,
        pub policy_identity: TerminalIdentityIndex,
        pub policy_plan_machine_identity: TerminalIdentityIndex,
        pub schema_identity: TerminalIdentityIndex,
        pub placement_report_fingerprint: u64,
        pub placement_commitment: [u8; 32],
    }

    pub fn canonical_access_view_identity(policy: &str, schema: &str) -> String {
        format!(
            "access-view:{}:{policy}:{}:{schema}",
            policy.len(),
            schema.len()
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalAccessViewPlan {
        pub carrier: semantic_vocabulary::StructuralTypeId,
        pub schema: semantic_vocabulary::StructuralTypeId,
        pub view_identity: TerminalIdentityIndex,
        pub policy_identity: TerminalIdentityIndex,
        pub policy_plan_machine_identity: TerminalIdentityIndex,
        pub schema_identity: TerminalIdentityIndex,
        pub placement: access_plans::ValidatedAccessPlan,
        pub fields: Vec<TerminalAccessFieldPlan>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalAccessLifecyclePlan {
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub target: Option<crate::TargetLayoutInput>,
        pub operation: TerminalAccessLifecycleOperation,
        pub execution: TerminalAccessLifecycleExecution,
        pub storage: TerminalAccessStorage,
        pub schema: semantic_vocabulary::StructuralTypeId,
        pub image_fields: Vec<semantic_vocabulary::StructuralFieldId>,
        pub policy_identity: TerminalIdentityIndex,
        pub policy_plan_machine_identity: TerminalIdentityIndex,
        pub schema_identity: TerminalIdentityIndex,
        pub placement: access_plans::ValidatedAccessPlan,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TerminalAccessLifecycleExecution {
        SelectedProvider,
        CompilerPrimitive {
            profile_type: Option<semantic_vocabulary::StructuralTypeId>,
            profile_requirements: Vec<TerminalAccessProfileRequirement>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalAccessProfileRequirement {
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub owner: crate::NominalDeclarationIdentity,
        pub callable: TerminalIdentityIndex,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalAccessStorage {
        pub carrier: semantic_vocabulary::StructuralTypeId,
        pub owner: TerminalAccessOwner,
        pub base: semantic_vocabulary::StructuralFieldId,
        pub length: semantic_vocabulary::StructuralFieldId,
        pub granted: semantic_vocabulary::StructuralDomainId,
        pub vacant: Option<semantic_vocabulary::StructuralDomainId>,
        pub resident: Option<semantic_vocabulary::StructuralDomainId>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TerminalAccessOwner {
        Extent,
        Run {
            carrier: semantic_vocabulary::StructuralTypeId,
            storage: semantic_vocabulary::StructuralFieldId,
            count: semantic_vocabulary::StructuralFieldId,
            singleton: Option<semantic_vocabulary::StructuralDomainId>,
        },
    }

    impl TerminalAccessStorage {
        pub fn same_custody(&self, other: &Self) -> bool {
            let owner_roles = |owner| match owner {
                TerminalAccessOwner::Run {
                    carrier,
                    storage,
                    count,
                    ..
                } => TerminalAccessOwner::Run {
                    carrier,
                    storage,
                    count,
                    singleton: None,
                },
                TerminalAccessOwner::Extent => TerminalAccessOwner::Extent,
            };
            self.carrier == other.carrier
                && owner_roles(self.owner) == owner_roles(other.owner)
                && self.base == other.base
                && self.length == other.length
                && self.granted == other.granted
                && self.vacant == other.vacant
                && self.resident == other.resident
        }

        pub fn owner_storage_path(
            &self,
            types: &[crate::StructuralTypeDeclaration],
        ) -> Option<Vec<crate::StructuralPathSegment>> {
            match self.owner {
                TerminalAccessOwner::Extent => Some(Vec::new()),
                TerminalAccessOwner::Run {
                    carrier, storage, ..
                } => {
                    let crate::StructuralTypeShape::Record { fields, .. } =
                        &crate::find_structural_type(types, carrier)?.shape
                    else {
                        return None;
                    };
                    let backing = fields.iter().find(|field| field.id == storage)?;
                    Some(vec![crate::StructuralPathSegment::Field(backing.identity)])
                }
            }
        }

        pub fn owner_carrier(&self) -> semantic_vocabulary::StructuralTypeId {
            match self.owner {
                TerminalAccessOwner::Extent => self.carrier,
                TerminalAccessOwner::Run { carrier, .. } => carrier,
            }
        }

        pub fn owned_content(
            &self,
        ) -> Result<
            (
                semantic_vocabulary::StructuralDomainId,
                semantic_vocabulary::StructuralDomainId,
            ),
            &'static str,
        > {
            match (self.vacant, self.resident) {
                (Some(vacant), Some(resident)) => Ok((vacant, resident)),
                _ => Err("owned access lifecycle lost its exact content qualification roles"),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ResidentReferenceSource {
        pub base: semantic_vocabulary::StructuralFieldId,
        pub length: semantic_vocabulary::StructuralFieldId,
        pub granted: semantic_vocabulary::StructuralDomainId,
        pub resident: semantic_vocabulary::StructuralDomainId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalAccessLifecycleOperation {
        Initialize,
        MoveOut,
        View,
        Relocate,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct TerminalAccessProfileOperand {
        pub storage: usize,
        pub profile: usize,
        pub occupied: bool,
    }

    impl TerminalAccessLifecycleOperation {
        pub const fn profile_operands(self) -> &'static [TerminalAccessProfileOperand] {
            match self {
                Self::Initialize => &[TerminalAccessProfileOperand {
                    storage: 0,
                    profile: 2,
                    occupied: false,
                }],
                Self::MoveOut => &[TerminalAccessProfileOperand {
                    storage: 0,
                    profile: 1,
                    occupied: true,
                }],
                Self::View => &[TerminalAccessProfileOperand {
                    storage: 0,
                    profile: 1,
                    occupied: false,
                }],
                Self::Relocate => &[
                    TerminalAccessProfileOperand {
                        storage: 0,
                        profile: 2,
                        occupied: true,
                    },
                    TerminalAccessProfileOperand {
                        storage: 1,
                        profile: 3,
                        occupied: false,
                    },
                ],
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalAccessFieldPlan {
        pub access_slot: u32,
        pub view_field: TerminalIdentityIndex,
        pub schema_field: TerminalIdentityIndex,
        pub accessor: semantic_vocabulary::StructuralTypeId,
        pub operations: Vec<TerminalAccessor>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalAccessorKind {
        Read,
        Take,
        Write,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalAccessor {
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub kind: TerminalAccessorKind,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AccessRamFieldCorrespondence {
        pub access_slot: u32,
        pub source_field: semantic_vocabulary::StructuralFieldId,
        pub byte_offset: u64,
        pub transfer_width_bits: u16,
        pub logical_scalar: semantic_vocabulary::ScalarType,
        pub stored_scalar: semantic_vocabulary::ScalarType,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AccessRamCorrespondence {
        pub schema: semantic_vocabulary::StructuralTypeId,
        pub carrier: semantic_vocabulary::StructuralTypeId,
        pub source_byte_length: u64,
        pub required_base: access_plans::BaseCongruence,
        pub fields: Vec<AccessRamFieldCorrespondence>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AccessorScalarBoundaryDemand {
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub carrier: semantic_vocabulary::StructuralTypeId,
        pub schema: semantic_vocabulary::StructuralTypeId,
        pub accessor: semantic_vocabulary::StructuralTypeId,
        pub view_field: crate::TerminalIdentityIndex,
        pub access_slot: u32,
        pub placement_commitment: [u8; 32],
        pub operation: TerminalAccessorKind,
        pub access: crate::StructuralAccess,
        pub scalar: semantic_vocabulary::ScalarType,
        pub byte_offset: u64,
        pub transfer_width_bits: u16,
        pub placement_byte_extent: u64,
        pub observation: access_plans::ObservationModel,
    }
}
pub(crate) mod suspension {

    use crate::{
        StructuralAccess, StructuralArgument, StructuralPathSegment,
        terminal_module::TerminalIdentityTable,
    };
    use arena::{Arena, Handle, HandleSpan};
    use language_semantics::CarryPolicy;
    use semantic_vocabulary::{
        BoundaryMachineId, ClaimId, IeeeFloatFormat, IntegerCarrier, IntegerSign, MachineId,
        OperationId, ScalarType, StructuralTypeId, SuspensionCrossingId, ValueId,
    };

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TerminalSuspensionCallSite {
        pub operation: OperationId,
        pub crossing: SuspensionCrossingId,
        pub target: TerminalSuspensionCallTarget,
        pub frontier_commitment: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalSuspensionCallPlan {
        pub operation: OperationId,
        pub crossing: SuspensionCrossingId,
        pub target: TerminalSuspensionCallTarget,
        pub effective: CarryPolicy,
        pub live_value_count: u32,
        pub live_values: Vec<TerminalSuspensionLiveValue>,
        pub loans: TerminalSuspensionLoans,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct TerminalSuspensionLoans {
        pub occurrences: Arena<TerminalSuspensionLoan>,
        pub paths: Arena<StructuralPathSegment>,
        pub value_links: Arena<Handle<TerminalSuspensionLoan>>,
        pub supplier_links: Arena<Handle<TerminalSuspensionLoan>>,
        pub input_links: Arena<Handle<TerminalSuspensionLoan>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalSuspensionLoan {
        pub capture: TerminalSuspensionLoanCapture,
        pub backing: TerminalSuspensionLoanBacking,
        pub resource: TerminalSuspensionLoanResource,
        pub parent: Handle<TerminalSuspensionLoan>,
        pub suppliers: HandleSpan<Handle<TerminalSuspensionLoan>>,
        pub inputs: HandleSpan<Handle<TerminalSuspensionLoan>>,
        pub access: StructuralAccess,
        pub active_at_call: bool,
    }

    impl Default for TerminalSuspensionLoan {
        fn default() -> Self {
            Self {
                capture: Default::default(),
                backing: Default::default(),
                resource: Default::default(),
                parent: Handle::invalid(),
                suppliers: HandleSpan::empty(),
                inputs: HandleSpan::empty(),
                access: StructuralAccess::SharedBorrow,
                active_at_call: false,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub enum TerminalSuspensionLoanResource {
        #[default]
        Unresolved,
        DirectRoot,
        Reborrow,
        IncomingReference,
        CompletedOutput,
        LifetimeDependency,
        OutputInput,
        DescriptorTransfer,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum TerminalSuspensionLoanCapture {
        #[default]
        Unresolved,
        Reference {
            formation: OperationId,
            carrier: semantic_vocabulary::PlaceId,
            source: semantic_vocabulary::PlaceId,
            source_access: StructuralAccess,
            source_path: HandleSpan<StructuralPathSegment>,
        },
        RetainedValue {
            live_value: u32,
            path: HandleSpan<StructuralPathSegment>,
        },
        CompletedOutput {
            call: OperationId,
            argument: StructuralArgument,
            path: HandleSpan<StructuralPathSegment>,
        },
        CallInput {
            call: OperationId,
            argument: StructuralArgument,
            path: HandleSpan<StructuralPathSegment>,
            stored: bool,
        },
        InputDependencies,
        DescriptorTransfer,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub enum TerminalSuspensionLoanBacking {
        #[default]
        Unresolved,
        RetainedOwner {
            live_value: u32,
        },
        IncomingReferent {
            argument: StructuralArgument,
        },
        CompletedOutput {
            loan: Handle<TerminalSuspensionLoan>,
        },
        InputDependencies,
        CapturedInput,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub enum TerminalSuspensionValueLoans {
        #[default]
        Unresolved,
        ViewFree,
        Retained {
            links: HandleSpan<Handle<TerminalSuspensionLoan>>,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalSuspensionCallTarget {
        Machine(MachineId),
        Boundary(BoundaryMachineId),
        Dynamic { interface: u32, slot: u32 },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalSuspensionLiveValue {
        pub place: TerminalSuspensionPlace,
        pub value_type: TerminalSuspensionValueType,
        pub storage: TerminalSuspensionStorage,
        pub claim_count: u32,
        pub claims: Vec<ClaimId>,
        pub effective: CarryPolicy,
        pub loans: TerminalSuspensionValueLoans,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalSuspensionPlace {
        Scalar(ValueId),
        Structural(StructuralArgument),
    }

    impl TerminalSuspensionPlace {
        pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId)) {
            match self {
                Self::Scalar(value) => visit(*value),
                Self::Structural(argument) => {
                    for segment in &argument.path {
                        if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                            visit(*index);
                        }
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalSuspensionValueType {
        Scalar(ScalarType),
        Structural(StructuralTypeId),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerminalSuspensionStorage {
        Parameter,
        Local,
        CallArgument,
    }

    pub fn suspension_frontier_commitment(
        identities: &TerminalIdentityTable,
        plan: &TerminalSuspensionCallPlan,
    ) -> [u8; 32] {
        let mut hash =
            crate::digest_encoder::DomainDigestEncoder::new(b"terminal-suspension-frontier-v9\0");
        hash.raw(&plan.operation.get().to_le_bytes());
        hash.raw(&plan.crossing.get().to_le_bytes());
        match plan.target {
            TerminalSuspensionCallTarget::Machine(machine) => {
                hash.byte(1);
                hash.raw(&machine.get().to_le_bytes());
            }
            TerminalSuspensionCallTarget::Boundary(boundary) => {
                hash.byte(2);
                hash.raw(&boundary.get().to_le_bytes());
            }
            TerminalSuspensionCallTarget::Dynamic { interface, slot } => {
                hash.byte(5);
                hash.raw(&interface.to_le_bytes());
                hash.raw(&slot.to_le_bytes());
            }
        }
        hash_carry_policy(&mut hash, plan.effective);
        hash.raw(&plan.live_value_count.to_le_bytes());
        hash.u64(plan.live_values.len() as u64);
        for live in &plan.live_values {
            match &live.place {
                TerminalSuspensionPlace::Scalar(value) => {
                    hash.byte(1);
                    hash.raw(&value.get().to_le_bytes());
                }
                TerminalSuspensionPlace::Structural(argument) => {
                    hash.byte(2);
                    hash.raw(&argument.place.get().to_le_bytes());
                    hash_access(&mut hash, argument.access);
                    hash.u64(argument.path.len() as u64);
                    for segment in &argument.path {
                        hash_loan_path_segment(identities, &mut hash, segment);
                    }
                }
            }
            match live.value_type {
                TerminalSuspensionValueType::Scalar(scalar) => {
                    hash.byte(1);
                    hash_scalar_type(&mut hash, scalar);
                }
                TerminalSuspensionValueType::Structural(structural) => {
                    hash.byte(2);
                    hash.raw(&structural.get().to_le_bytes());
                }
            }
            hash.byte(match live.storage {
                TerminalSuspensionStorage::Parameter => 2,
                TerminalSuspensionStorage::Local => 3,
                TerminalSuspensionStorage::CallArgument => 4,
            });
            hash.raw(&live.claim_count.to_le_bytes());
            hash.u64(live.claims.len() as u64);
            for claim in &live.claims {
                hash.raw(&claim.get().to_le_bytes());
            }
            hash_carry_policy(&mut hash, live.effective);
            match live.loans {
                TerminalSuspensionValueLoans::Unresolved => hash.byte(0),
                TerminalSuspensionValueLoans::ViewFree => hash.byte(1),
                TerminalSuspensionValueLoans::Retained { links } => {
                    hash.byte(2);
                    hash_span(&mut hash, links);
                }
            }
        }
        hash_loans(identities, &mut hash, &plan.loans);
        hash.finish()
    }

    impl TerminalSuspensionCallPlan {
        pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId)) {
            for live in &self.live_values {
                live.place.visit_value_ids(&mut visit);
            }
            for (_, path) in self.loans.paths.iter() {
                if let StructuralPathSegment::RuntimeIndex { index, .. } = path {
                    visit(*index);
                }
            }
            for (_, loan) in self.loans.occurrences.iter() {
                if let TerminalSuspensionLoanCapture::CompletedOutput { argument, .. }
                | TerminalSuspensionLoanCapture::CallInput { argument, .. } = &loan.capture
                {
                    for segment in &argument.path {
                        if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                            visit(*index);
                        }
                    }
                }
                if let TerminalSuspensionLoanBacking::IncomingReferent { argument } = &loan.backing
                {
                    for segment in &argument.path {
                        if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                            visit(*index);
                        }
                    }
                }
            }
        }
    }

    fn hash_handle<T>(hash: &mut crate::digest_encoder::DomainDigestEncoder, handle: Handle<T>) {
        hash.raw(&handle.arena_index().to_le_bytes());
        hash.raw(&handle.generation().to_le_bytes());
    }

    fn hash_span<T>(hash: &mut crate::digest_encoder::DomainDigestEncoder, span: HandleSpan<T>) {
        hash_handle(hash, span.start());
        hash.raw(&span.count().to_le_bytes());
    }

    fn hash_loans(
        identities: &TerminalIdentityTable,
        hash: &mut crate::digest_encoder::DomainDigestEncoder,
        loans: &TerminalSuspensionLoans,
    ) {
        hash.u64(loans.paths.len() as u64);
        for (_, segment) in loans.paths.iter() {
            hash_loan_path_segment(identities, hash, segment);
        }
        hash.u64(loans.value_links.len() as u64);
        for (_, link) in loans.value_links.iter() {
            hash_handle(hash, *link);
        }
        hash.u64(loans.supplier_links.len() as u64);
        for (_, supplier) in loans.supplier_links.iter() {
            hash_handle(hash, *supplier);
        }
        hash.u64(loans.input_links.len() as u64);
        for (_, input) in loans.input_links.iter() {
            hash_handle(hash, *input);
        }
        hash.u64(loans.occurrences.len() as u64);
        for (_, loan) in loans.occurrences.iter() {
            match &loan.capture {
                TerminalSuspensionLoanCapture::Unresolved => hash.byte(0),
                TerminalSuspensionLoanCapture::Reference {
                    formation,
                    carrier,
                    source,
                    source_access,
                    source_path,
                } => {
                    hash.byte(1);
                    hash.raw(&formation.get().to_le_bytes());
                    hash.raw(&carrier.get().to_le_bytes());
                    hash.raw(&source.get().to_le_bytes());
                    hash_access(hash, *source_access);
                    hash_span(hash, *source_path);
                }
                TerminalSuspensionLoanCapture::RetainedValue { live_value, path } => {
                    hash.byte(2);
                    hash.raw(&live_value.to_le_bytes());
                    hash_span(hash, *path);
                }
                TerminalSuspensionLoanCapture::CompletedOutput {
                    call,
                    argument,
                    path,
                } => {
                    hash.byte(3);
                    hash.raw(&call.get().to_le_bytes());
                    hash_argument(identities, hash, argument);
                    hash_span(hash, *path);
                }
                TerminalSuspensionLoanCapture::CallInput {
                    call,
                    argument,
                    path,
                    stored,
                } => {
                    hash.byte(4);
                    hash.raw(&call.get().to_le_bytes());
                    hash_argument(identities, hash, argument);
                    hash_span(hash, *path);
                    hash.byte(u8::from(*stored));
                }
                TerminalSuspensionLoanCapture::InputDependencies => hash.byte(5),
                TerminalSuspensionLoanCapture::DescriptorTransfer => hash.byte(6),
            }
            match &loan.backing {
                TerminalSuspensionLoanBacking::Unresolved => hash.byte(0),
                TerminalSuspensionLoanBacking::RetainedOwner { live_value } => {
                    hash.byte(1);
                    hash.raw(&live_value.to_le_bytes());
                }
                TerminalSuspensionLoanBacking::IncomingReferent { argument } => {
                    hash.byte(2);
                    hash.raw(&argument.place.get().to_le_bytes());
                    hash_access(hash, argument.access);
                    hash.u64(argument.path.len() as u64);
                    for segment in &argument.path {
                        hash_loan_path_segment(identities, hash, segment);
                    }
                }
                TerminalSuspensionLoanBacking::CompletedOutput { loan } => {
                    hash.byte(3);
                    hash_handle(hash, *loan);
                }
                TerminalSuspensionLoanBacking::InputDependencies => hash.byte(4),
                TerminalSuspensionLoanBacking::CapturedInput => hash.byte(5),
            }
            hash.byte(match loan.resource {
                TerminalSuspensionLoanResource::Unresolved => 0,
                TerminalSuspensionLoanResource::DirectRoot => 1,
                TerminalSuspensionLoanResource::Reborrow => 2,
                TerminalSuspensionLoanResource::IncomingReference => 3,
                TerminalSuspensionLoanResource::CompletedOutput => 4,
                TerminalSuspensionLoanResource::LifetimeDependency => 5,
                TerminalSuspensionLoanResource::OutputInput => 6,
                TerminalSuspensionLoanResource::DescriptorTransfer => 7,
            });
            hash_handle(hash, loan.parent);
            hash_span(hash, loan.suppliers);
            hash_span(hash, loan.inputs);
            hash_access(hash, loan.access);
            hash.byte(u8::from(loan.active_at_call));
        }
    }

    fn hash_access(
        hash: &mut crate::digest_encoder::DomainDigestEncoder,
        access: StructuralAccess,
    ) {
        hash.byte(match access {
            StructuralAccess::Owned => 1,
            StructuralAccess::SharedBorrow => 2,
            StructuralAccess::MutableBorrow => 3,
            StructuralAccess::WriteOnlyBorrow => 4,
        });
    }

    fn hash_loan_path_segment(
        identities: &TerminalIdentityTable,
        hash: &mut crate::digest_encoder::DomainDigestEncoder,
        segment: &StructuralPathSegment,
    ) {
        match segment {
            StructuralPathSegment::AllElements => hash.byte(10),
            StructuralPathSegment::Case(case) => {
                hash.byte(7);
                hash.raw(&case.get().to_le_bytes());
            }
            StructuralPathSegment::Field(field) => {
                hash.byte(1);
                let field = identities.text(*field).unwrap_or_default();
                hash.u64(field.len() as u64);
                hash.raw(field.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                hash.byte(2);
                hash.raw(&index.to_le_bytes());
            }
            StructuralPathSegment::Referent => hash.byte(3),
            StructuralPathSegment::FixedRange { start, end } => {
                hash.byte(4);
                hash.raw(&start.to_le_bytes());
                hash.raw(&end.to_le_bytes());
            }
            StructuralPathSegment::RuntimeIndex { index, obligation } => {
                hash.byte(5);
                hash.raw(&index.get().to_le_bytes());
                hash.raw(&obligation.get().to_le_bytes());
            }
            StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } => {
                hash.byte(8);
                hash.raw(&structural_type.get().to_le_bytes());
                hash.byte(u8::from(stored_scalar.is_some()));
                if let Some(scalar) = stored_scalar {
                    hash_scalar_type(hash, *scalar);
                }
            }
        }
    }

    fn hash_scalar_type(hash: &mut crate::digest_encoder::DomainDigestEncoder, scalar: ScalarType) {
        match scalar {
            ScalarType::Boolean => hash.byte(1),
            ScalarType::Integer(integer) => {
                hash.byte(2);
                hash.byte(match integer.carrier() {
                    IntegerCarrier::Fixed => 1,
                    IntegerCarrier::Address => 2,
                });
                hash.byte(match integer.sign() {
                    IntegerSign::Signed => 1,
                    IntegerSign::Unsigned => 2,
                });
                integer.width().write_identity(|bytes| hash.raw(bytes));
            }
            ScalarType::IeeeFloat(format) => {
                hash.byte(3);
                hash.byte(match format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
        }
    }

    fn hash_carry_policy(
        hash: &mut crate::digest_encoder::DomainDigestEncoder,
        policy: CarryPolicy,
    ) {
        hash.byte(u8::from(
            policy.suspension == language_semantics::CarrySuspension::Allowed,
        ));
        hash.byte(u8::from(policy.cpu == language_semantics::CarryCpu::Any));
        hash.byte(u8::from(
            policy.host_thread == language_semantics::CarryHostThread::Any,
        ));
        hash.byte(u8::from(
            policy.address == language_semantics::CarryAddress::Movable,
        ));
    }

    fn hash_argument(
        identities: &TerminalIdentityTable,
        hash: &mut crate::digest_encoder::DomainDigestEncoder,
        argument: &StructuralArgument,
    ) {
        hash.raw(&argument.place.get().to_le_bytes());
        hash_access(hash, argument.access);
        hash.u64(argument.path.len() as u64);
        for segment in &argument.path {
            hash_loan_path_segment(identities, hash, segment);
        }
    }
}

pub use access::{StructuralAccess, StructuralMultiplicity, structural_access_covers};
pub use borrows::{
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass,
    TerminalReborrowRestoredCallUse, TerminalReborrowSharedCohortMember,
};
pub use claims::{ClaimTransfer, CompletionReceipt, EntryClaim, LoanCompletion};
pub use cleanup::{NominalAffineCleanup, StructuralAffineDiscard, TerminalAffineCleanupAction};
pub use placement::{
    AccessRamCorrespondence, AccessRamFieldCorrespondence, AccessorScalarBoundaryDemand,
    ResidentReferenceSource, TerminalAccessFieldPlan, TerminalAccessLifecycleExecution,
    TerminalAccessLifecycleOperation, TerminalAccessLifecyclePlan, TerminalAccessOwner,
    TerminalAccessProfileOperand, TerminalAccessProfileRequirement, TerminalAccessStorage,
    TerminalAccessViewInput, TerminalAccessViewPlan, TerminalAccessor, TerminalAccessorKind,
    canonical_access_view_identity,
};
pub use suspension::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
    TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource, TerminalSuspensionLoans,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueLoans,
    TerminalSuspensionValueType, suspension_frontier_commitment,
};
