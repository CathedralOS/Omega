pub(crate) mod constant_evaluation;
pub(crate) mod floating_point;
pub(crate) mod integer_arithmetic;
pub(crate) mod owned_values;

use omega_typed::TypeError;

pub(crate) fn refusal(
    site: &omega_typed::Site,
    reason: &str,
) -> TypeError {
    TypeError::new(
        &site.package,
        &site.file,
        site.span,
        format!("static evaluation cannot admit this invocation: {reason}"),
    )
}
