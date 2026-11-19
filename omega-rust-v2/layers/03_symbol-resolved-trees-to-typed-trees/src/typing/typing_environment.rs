use crate::{AssemblyTypes, BoundarySlot, EvaluatedIntegerCall, RankingView, SpecializedIndexValue, StaticIntegerEvaluator};
use omega_resolver::DeclarationId;
/// Declarations and capabilities admitted by the Build for this exact stage.
#[derive(Default)]
pub struct TypingEnvironment<'a> {
    pub target: Option<&'a AssemblyTypes>,
    pub integer_evaluator: Option<&'a dyn StaticIntegerEvaluator>,
    pub evaluated_integer_calls: &'a [EvaluatedIntegerCall],
    pub specialized_indices: &'a [SpecializedIndexValue],
    pub access_plan_requirement: Option<&'a DeclarationId>,
    pub ranking_views: &'a [RankingView],
    pub boundary_slots: &'a [BoundarySlot],
}
