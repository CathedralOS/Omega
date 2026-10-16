//! Selected type, boundary, and service catalogs for composed Unit control.
use super::super::super::{
    BoundaryMachineId, ServiceId, ServiceReachId, StructuralTypeDeclaration,
};
use super::super::{StructuralPlaceDeclaration, StructuralTypeId};
use super::scalar_calls;
use crate::terminal_identities::StructuralDomainBinding;
use crate::unit::attached_unit::operation_frame::BoundaryParameters;
use crate::unit::attached_unit::signatures::MachineSignature;
use std::borrow::Cow;

/// Standalone roots own their publication tables; shared callees borrow the
/// already allocated closure. Only owned type tables admit generated carriers.
/// The call targets (`signatures`, `boundary_parameters`, `closure`) are the
/// same records an ordinary body's calls
/// resolve against, so a state's calls emit through the one frame emitter.
pub(crate) struct ComposedCatalogs<'a> {
    pub(crate) cleanup_control: &'a checked_trees::CheckedCleanupControlPlans,
    pub(crate) structural_types: Cow<'a, [StructuralTypeDeclaration]>,
    pub(crate) type_ids: Cow<'a, [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)]>,
    pub(crate) domain_ids: Cow<'a, [StructuralDomainBinding]>,
    pub(crate) domain_declarations: &'a [terminal_psi::StructuralDomainDeclaration],
    /// The boundaries a provider attachment names, by checked source.
    pub(crate) lowered_boundaries: Vec<LoweredComposedBoundary>,
    pub(in crate::unit::attached_unit) boundary_parameters: Cow<'a, [BoundaryParameters]>,
    /// The allocated signature of every Unit body the closure emits; a
    /// state's internal calls substitute into the callee's.
    pub(in crate::unit::attached_unit) signatures: Cow<'a, [MachineSignature]>,
    /// The closure's Unit bodies and prepared scalar callees.
    pub(in crate::unit::attached_unit) closure: &'a [symbols::SymbolHandle],
    /// The closure's scalar qualification namespace; a call's emitted
    /// result republishes the callee's qualified result through it.
    pub(in crate::unit::attached_unit) prepared_scalar_qualifications:
        Option<&'a crate::expression_preparation::qualifications::PreparedScalarQualifications>,
    pub(crate) service_ids: Cow<'a, [(ServiceReachId, ServiceId)]>,
    pub(crate) next_place: u64,
    /// Private constructor, join, and literal places; not authored result ordinals.
    pub(crate) temporary_places: Vec<StructuralPlaceDeclaration>,
    pub(crate) result_places: Vec<StructuralPlaceDeclaration>,
    pub(crate) scalar_calls: scalar_calls::ComposedScalarCalls,
    pub(crate) root_crash_routes: Vec<checked_trees::CrashRouteBucket>,
    pub(crate) next_value: u64,
    pub(crate) next_block: u64,
    pub(crate) next_operation: u64,
    pub(crate) next_edge: u64,
}

pub(crate) struct LoweredComposedBoundary {
    pub(crate) source: symbols::SymbolHandle,
    pub(crate) id: BoundaryMachineId,
}

impl LoweredComposedBoundary {
    /// The provider-facing `(source, identity)` view of the call roster.
    pub(super) fn roster(boundaries: &[BoundaryParameters]) -> Vec<Self> {
        boundaries
            .iter()
            .map(|boundary| Self {
                source: boundary.source,
                id: boundary.id,
            })
            .collect()
    }
}
