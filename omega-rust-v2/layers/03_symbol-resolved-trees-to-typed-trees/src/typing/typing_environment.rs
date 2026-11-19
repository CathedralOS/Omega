use crate::{AssemblyTypes, BoundarySlot, RankingView, SpecializedIndexValue, StaticIntegerRequest};
use omega_resolver::DeclarationId;
/// Declarations and capabilities admitted by the Build for this exact stage.
#[derive(Default)]
pub struct TypingEnvironment<'a> {
    pub target: Option<&'a AssemblyTypes>,
    /// Previously evaluated values for this stage, keyed by occurrence and destination carrier.
    pub evaluated_indices: &'a [(StaticIntegerRequest, num_bigint::BigInt)],
    pub specialized_indices: &'a [SpecializedIndexValue],
    pub access_plan_requirement: Option<&'a DeclarationId>,
    pub ranking_views: &'a [RankingView],
    pub boundary_slots: &'a [BoundarySlot],
}
