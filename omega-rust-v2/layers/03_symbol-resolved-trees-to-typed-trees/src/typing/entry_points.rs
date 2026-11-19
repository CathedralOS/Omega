use crate::typing::type_stage::type_admitted_stage;
use crate::{AssemblyTypes, TypeError, TypingEnvironment};
use omega_resolver::SymbolResolvedTrees;
/// Type the exact trees whose names L2 resolved.
pub fn type_stage<'text>(resolved: &SymbolResolvedTrees<'text>) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_stage_in_environment(resolved, &TypingEnvironment::default())
}

pub fn type_stage_for_target<'text>(
    resolved: &SymbolResolvedTrees<'text>,
    target: &AssemblyTypes,
) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_stage_in_environment(
        resolved,
        &TypingEnvironment {
            target: Some(target),
            ..Default::default()
        },
    )
}

pub fn type_stage_in_environment<'text>(
    resolved: &SymbolResolvedTrees<'text>,
    environment: &TypingEnvironment<'_>,
) -> Result<crate::TypedTrees<'text>, TypeError> {
    // Raw L2 callers can supply their own package graph. Collection operations still
    // use the sealed toolchain identities even when no standard exports are installed.
    for tree in resolved.trees() {
        omega_toolchain::check_source_identity(tree.file.package, tree.file.path).map_err(|e| TypeError {
            package: e.package,
            file: e.file,
            span: e.span,
            message: e.message,
        })?;
    }
    type_admitted_stage(resolved, None, environment)
}
