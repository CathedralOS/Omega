//! Sole ordered inventory of admitted scalar construction families.

use crate::selection::shared::*;

use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::{
    active_resident_exact_add_bridge_chain, active_resident_exact_add_chain,
    active_resident_exact_add_original_victim_chain, comparison_immediate_pair, exact_binary_pair,
    immediate_pair, parameter_pair, zero_comparison_immediate_pair,
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
        "zero-comparison-immediate-pair",
        zero_comparison_immediate_pair::is_candidate,
        zero_comparison_immediate_pair::build,
    ),
    ScalarFamilyDescriptor::new(
        "comparison-immediate-pair",
        comparison_immediate_pair::is_candidate,
        comparison_immediate_pair::build,
    ),
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
    ScalarFamilyDescriptor::new(
        "active-resident-exact-add-original-victim-chain",
        active_resident_exact_add_original_victim_chain::is_candidate,
        active_resident_exact_add_original_victim_chain::build,
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
    fn catalog_keeps_zero_comparisons_distinct_and_first() {
        let names = SCALAR_FAMILIES
            .iter()
            .map(|descriptor| descriptor.name)
            .collect::<Vec<_>>();
        assert_eq!(names[0], "zero-comparison-immediate-pair");
        assert_eq!(
            names
                .iter()
                .filter(|name| **name == "zero-comparison-immediate-pair")
                .count(),
            1
        );
    }

    #[test]
    fn omission_and_overlap_fail_closed() {
        let family = |name| {
            SCALAR_FAMILIES
                .iter()
                .find(|descriptor| descriptor.name == name)
                .expect("named scalar family exists")
        };
        let immediate = family("immediate-pair");
        let parameter = family("entry-parameter-pair");
        assert!(matches!(
            unique_match(7, std::iter::empty()),
            Err(SelectedInstructionError::UnsupportedSourceShape { function: 7 })
        ));
        assert_eq!(
            unique_match(7, std::iter::once(immediate))
                .expect("one row is exact")
                .name,
            "immediate-pair"
        );
        assert!(matches!(
            unique_match(7, [immediate, parameter].into_iter()),
            Err(SelectedInstructionError::AmbiguousSourceShape {
                function: 7,
                first: "immediate-pair",
                second: "entry-parameter-pair",
            })
        ));
    }
}
