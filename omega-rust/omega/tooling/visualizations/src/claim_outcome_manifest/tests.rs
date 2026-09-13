use super::*;
use crate::test_support::*;

mod behavior_and_claim_outcomes;
mod content_lineage_and_plans;
mod content_manifest_structure;
mod content_partition_inputs;
mod content_partition_results;
mod content_reshuffles;

use behavior_and_claim_outcomes::{
    claim_outcome_validation_fixture, first_claim_outcome_entries_mut,
};
use content_partition_inputs::content_partition_input_validation_fixture;
use content_reshuffles::content_identity_reshuffle_validation_fixture;
