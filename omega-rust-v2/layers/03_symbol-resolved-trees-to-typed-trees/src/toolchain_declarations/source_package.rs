//! Source packages import the standard library through its typed interface.
use crate::typing::type_stage::type_admitted_stage;
use crate::{TypeError, TypingEnvironment};
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
