//! The build machine's facets: what an `omega build` evaluation reads, writes
//! and must account for, as opposed to ordinary program execution.
//!
//! Each module is one facet the build machine exposes to authored `build.omg`
//! code, and each contributes methods to `Evaluator` rather than a type of its
//! own: `paths` and `log` are the rooted path vocabulary and the build log,
//! `root_bindings` and `provider_selections` are what the build declares,
//! `product_entries`, `product_providers` and `product_schemas` are what it
//! produces, `output_obligations` is what it still owes at the end, and
//! `behavior_exclusions` is what it has been told not to do.

pub(in crate::interpreter::evaluator) mod behavior_exclusions;
pub(in crate::interpreter::evaluator) mod log;
pub(in crate::interpreter::evaluator) mod output_obligations;
pub(in crate::interpreter::evaluator) mod paths;
pub(in crate::interpreter::evaluator) mod product_entries;
pub(in crate::interpreter::evaluator) mod product_providers;
pub(in crate::interpreter::evaluator) mod product_schemas;
pub(in crate::interpreter::evaluator) mod provider_selections;
pub(in crate::interpreter::evaluator) mod root_bindings;

use super::{Cell, EvalResult, Evaluator, Halt};

impl Evaluator<'_> {
    /// A Build declaration borrows this activation's original cell. A value
    /// with the same type is not authority to configure the product.
    pub(in crate::interpreter::evaluator) fn require_root_build_argument(
        &self,
        builder: Cell,
        spelling: &str,
    ) -> EvalResult<()> {
        if self
            .root_build
            .as_ref()
            .is_some_and(|root| Cell::ptr_eq(&builder, root))
        {
            return Ok(());
        }
        Err(Halt::Trap(format!(
            "Build operation `{spelling}` requires the current activation's original Build value"
        )))
    }
}
