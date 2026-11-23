//! Source packages import the standard library through its typed interface.
use crate::typing::type_stage::{LibraryContext, type_stage_inner};
use crate::{PendingTyping, TypeError, TypingEnvironment, TypingProgress};
use omega_toolchain::SourcePackage;

/// Type the exact package admitted by the standard profile.
///
/// ```compile_fail,E0308
/// let bare = omega_resolver::resolve_stage(&[], &[]).unwrap();
/// omega_typer::type_source_package(&bare).unwrap();
/// ```
pub fn type_source_package<'text>(resolved: &SourcePackage<'text>) -> Result<crate::TypedTrees<'text>, TypeError> {
    type_package_pass(resolved, &TypingEnvironment { target: resolved.assembly(), ..Default::default() }, false).map(|(trees, _)| trees)
}

/// Type a package up to the point where a static evaluation result is needed.
/// A pending result permits evaluation of the demanded occurrences. To complete
/// typing, resubmit the same package with the accumulated evaluation results.
pub fn type_source_package_for_preparation<'text>(
    resolved: &SourcePackage<'text>,
    environment: &TypingEnvironment<'_>,
) -> Result<TypingProgress<'text>, TypeError> {
    if environment.target.is_some() && environment.target != resolved.assembly() {
        return Err(TypeError::new(
            String::new(),
            String::new(),
            omega_program_contracts::SourceSpan::new(0, 0),
            "assembly catalog differs from the one used to resolve this package",
        ));
    }
    let environment = &TypingEnvironment {
        target: resolved.assembly(),
        ..*environment
    };
    let (trees, demands) = type_package_pass(resolved, environment, true)?;
    if demands.is_empty() {
        return Ok(TypingProgress::Complete(type_package_pass(resolved, environment, false)?.0));
    }
    Ok(TypingProgress::Pending(PendingTyping { trees, demands }))
}

fn type_package_pass<'text>(
    source: &SourcePackage<'text>, environment: &TypingEnvironment<'_>, allow_pending: bool,
) -> Result<(crate::TypedTrees<'text>, Vec<crate::StaticValueDemand>), TypeError> {
    let context = if source.defines_toolchain() {
        LibraryContext::Define(omega_toolchain::core_interface())
    } else {
        LibraryContext::Import(super::core_interface::checked_library())
    };
    type_stage_inner(source, context, environment, allow_pending)
}
