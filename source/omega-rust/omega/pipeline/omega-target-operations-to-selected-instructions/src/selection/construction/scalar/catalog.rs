//! Sole ordered inventory of admitted scalar construction families.

use crate::selection::shared::*;

use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::{
    active_resident_exact_add_bridge_chain, active_resident_exact_add_chain, exact_binary_pair,
    immediate_pair, parameter_pair,
};

type CandidateClassifier = fn(&SourceFunction) -> bool;
type FamilyBuilder = for<'a> fn(
    &ScalarConstructionContext<'a>,
) -> Result<ConstructedScalarBody, SelectedInstructionError>;

#[derive(Clone, Copy)]
struct ScalarFamilyDescriptor {
    name: &'static str,
    is_candidate: CandidateClassifier,
    build: FamilyBuilder,
}

const SCALAR_FAMILIES: &[ScalarFamilyDescriptor] = &[
    ScalarFamilyDescriptor::new(
        "immediate-pair",
        immediate_pair::is_candidate,
        immediate_pair::build,
    ),
    ScalarFamilyDescriptor::new(
        "entry-parameter-pair",
        parameter_pair::is_candidate,
        parameter_pair::build,
    ),
    ScalarFamilyDescriptor::new(
        "exact-add-pair",
        exact_binary_pair::is_exact_add,
        exact_binary_pair::build_exact_add,
    ),
    ScalarFamilyDescriptor::new(
        "exact-subtract-pair",
        exact_binary_pair::is_exact_subtract,
        exact_binary_pair::build_exact_subtract,
    ),
    ScalarFamilyDescriptor::new(
        "widened-exact-add-pair",
        exact_binary_pair::is_widened_exact_add,
        exact_binary_pair::build_widened_exact_add,
    ),
    ScalarFamilyDescriptor::new(
        "widened-exact-subtract-pair",
        exact_binary_pair::is_widened_exact_subtract,
        exact_binary_pair::build_widened_exact_subtract,
    ),
    ScalarFamilyDescriptor::new(
        "active-resident-exact-add-chain",
        active_resident_exact_add_chain::is_candidate,
        active_resident_exact_add_chain::build,
    ),
    ScalarFamilyDescriptor::new(
        "active-resident-exact-add-bridge-chain",
        active_resident_exact_add_bridge_chain::is_candidate,
        active_resident_exact_add_bridge_chain::build,
    ),
];

impl ScalarFamilyDescriptor {
    const fn new(
        name: &'static str,
        is_candidate: CandidateClassifier,
        build: FamilyBuilder,
    ) -> Self {
        Self {
            name,
            is_candidate,
            build,
        }
    }
}

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    let descriptor = unique_match(
        context.function,
        SCALAR_FAMILIES
            .iter()
            .filter(|descriptor| (descriptor.is_candidate)(context.source)),
    )?;
    (descriptor.build)(context)
}

fn unique_match<'a>(
    function: usize,
    mut matches: impl Iterator<Item = &'a ScalarFamilyDescriptor>,
) -> Result<&'a ScalarFamilyDescriptor, SelectedInstructionError> {
    let first = matches
        .next()
        .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
    if let Some(second) = matches.next() {
        return Err(SelectedInstructionError::AmbiguousSourceShape {
            function,
            first: first.name,
            second: second.name,
        });
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omission_and_overlap_fail_closed() {
        assert!(matches!(
            unique_match(7, std::iter::empty()),
            Err(SelectedInstructionError::UnsupportedSourceShape { function: 7 })
        ));
        assert_eq!(
            unique_match(7, std::iter::once(&SCALAR_FAMILIES[0]))
                .expect("one row is exact")
                .name,
            "immediate-pair"
        );
        assert!(matches!(
            unique_match(7, [&SCALAR_FAMILIES[0], &SCALAR_FAMILIES[1]].into_iter()),
            Err(SelectedInstructionError::AmbiguousSourceShape {
                function: 7,
                first: "immediate-pair",
                second: "entry-parameter-pair",
            })
        ));
    }
}
