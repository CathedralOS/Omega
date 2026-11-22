pub(crate) mod policy_schema;
mod policy_execution;
pub(crate) mod policy_validation;
pub(crate) mod constant_evaluation;
pub(crate) mod exact_comparisons;
pub(crate) mod floating_point;
pub(crate) mod integer_arithmetic;
pub(crate) mod owned_values;

use crate::reasons::{PreparationRefusal, RefusalCause};

pub(crate) fn refusal(
    site: &omega_typed::Site,
    cause: impl Into<RefusalCause>,
) -> PreparationRefusal {
    PreparationRefusal::at(site, cause)
}
