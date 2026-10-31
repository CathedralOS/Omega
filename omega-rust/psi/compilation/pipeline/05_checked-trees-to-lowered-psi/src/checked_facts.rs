//! Read the checked fact tables lowering walks most.
//!
//! Each accessor names the table it returns, so a plan family reads as the
//! table it walks instead of restating the fact collection's shape at every
//! site. They are pure reads: a site that needs the tables mutably still
//! writes `checked.facts` directly.

use checked_trees::{CheckedTrees, CheckedValueFacts, FlowFacts};

/// The checked flow tables: states, ownership, exits and unit effects.
pub(crate) fn flow_facts(checked: &CheckedTrees) -> &FlowFacts {
    &checked.facts.flow
}

/// The checked value tables: scalar computations and structural values.
pub(crate) fn value_facts(checked: &CheckedTrees) -> &CheckedValueFacts {
    &checked.facts.values
}
