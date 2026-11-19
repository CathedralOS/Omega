//! Source packages import the standard library through its typed interface.
use crate::typing::type_stage::{type_admitted_stage, type_stage_inner};
use crate::{PendingTyping, TypeError, TypingEnvironment, TypingProgress};
use omega_toolchain::SourcePackage;

/// Type the exact package admitted by the standard profile.
///
/// ```compile_fail,E0308
/// let bare = omega_resolver::resolve_stage(&[], &[]).unwrap();
/// omega_typer::type_source_package(&bare).unwrap();
/// ```
pub fn type_source_package<'text>(resolved: &SourcePackage<'text>) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_admitted_stage(resolved, Some(super::core_interface::checked_library()), &TypingEnvironment::default())
}

/// Type a package up to the point where a static evaluation result is needed.
/// A pending result permits evaluation of the demanded occurrences. To complete
/// typing, resubmit the same package with the accumulated evaluation results.
pub fn type_source_package_for_preparation<'text>(
    resolved: &SourcePackage<'text>,
    environment: &TypingEnvironment<'_>,
) -> Result<TypingProgress<'text>, TypeError> {
    let library = Some(super::core_interface::checked_library());
    let (trees, demands) = type_stage_inner(resolved, library, environment, true)?;
    if demands.is_empty() {
        return Ok(TypingProgress::Complete(type_admitted_stage(resolved, library, environment)?));
    }
    Ok(TypingProgress::Pending(PendingTyping { trees, demands }))
}
