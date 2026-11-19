use crate::typing::type_stage::type_admitted_stage;
use crate::{AssemblyTypes, StaticIntegerEvaluator, TypeError, TypingEnvironment};
use omega_resolver::SymbolResolvedTrees;
/// Type the exact trees whose names L2 resolved.
pub fn type_stage<'text>(resolved: &SymbolResolvedTrees<'text>) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_admitted_stage(resolved, false, &TypingEnvironment::default())
}

pub fn type_stage_for_target<'text>(
    resolved: &SymbolResolvedTrees<'text>,
    target: &AssemblyTypes,
) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_admitted_stage(
        resolved,
        false,
        &TypingEnvironment {
            target: Some(target),
            ..Default::default()
        },
    )
}

pub fn type_stage_with_evaluator<'text>(
    resolved: &SymbolResolvedTrees<'text>,
    target: Option<&AssemblyTypes>,
    evaluator: &dyn StaticIntegerEvaluator,
) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_admitted_stage(
        resolved,
        false,
        &TypingEnvironment {
            target,
            integer_evaluator: Some(evaluator),
            ..Default::default()
        },
    )
}
pub fn type_stage_in_environment<'text>(
    resolved: &SymbolResolvedTrees<'text>,
    environment: &TypingEnvironment<'_>,
) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_admitted_stage(resolved, false, environment)
}
