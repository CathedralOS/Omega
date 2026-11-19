use crate::typing::type_stage::type_admitted_stage;
use crate::{AssemblyTypes, StaticIntegerEvaluator, TypeError, TypedStage, TypingEnvironment};
use omega_resolver::{ResolvedStage, StageSource};
/// The parsed files and resolution result must come from the same admitted stage.
pub fn type_stage(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
) -> Result<TypedStage, TypeError> {
    type_admitted_stage(sources, resolved, false, &TypingEnvironment::default())
}

pub fn type_stage_for_target(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    target: &AssemblyTypes,
) -> Result<TypedStage, TypeError> {
    type_admitted_stage(
        sources,
        resolved,
        false,
        &TypingEnvironment {
            target: Some(target),
            ..Default::default()
        },
    )
}

pub fn type_stage_with_evaluator(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    target: Option<&AssemblyTypes>,
    evaluator: &dyn StaticIntegerEvaluator,
) -> Result<TypedStage, TypeError> {
    type_admitted_stage(
        sources,
        resolved,
        false,
        &TypingEnvironment {
            target,
            integer_evaluator: Some(evaluator),
            ..Default::default()
        },
    )
}
pub fn type_stage_in_environment(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
    environment: &TypingEnvironment<'_>,
) -> Result<TypedStage, TypeError> {
    type_admitted_stage(sources, resolved, false, environment)
}
