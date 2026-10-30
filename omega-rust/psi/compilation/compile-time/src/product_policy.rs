//! Source admission of root trust acceptances and behavior restrictions.
//! These operations retain exact authored choices, never decide whether the
//! selected provider closure satisfies them or grant native execution authority.

mod exclusions;
mod root_grants;

pub use exclusions::resolve as collect_behavior_exclusions;
pub use root_grants::collect_product_root_grants;
