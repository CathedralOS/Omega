use std::collections::BTreeMap;

use omega_optimization_core::OptimizationUnitIdentity;

use super::fixture::{chosen_point, encoded_log};
use crate::{
    OfflinePolicyCorpusError, OfflinePolicySplit, admit_external_decision_logs,
    decode_offline_policy_corpus, split_for_source,
};

#[test]
fn source_group_partition_is_deterministic_complete_and_leak_free() {
    let sources = one_source_per_split();
    let mut logs = Vec::new();
    for (split, source) in &sources {
        logs.push(encoded_log(
            *source,
            [chosen_point(&[split.tag_for_test(), 1])],
        ));
        logs.push(encoded_log(
            *source,
            [chosen_point(&[split.tag_for_test(), 2])],
        ));
    }
    let admitted = admit_external_decision_logs(logs.clone()).unwrap();
    logs.reverse();
    let reversed = admit_external_decision_logs(logs).unwrap();
    assert_eq!(admitted.encode(), reversed.encode());

    for (split, source) in sources {
        let examples = admitted
            .examples()
            .iter()
            .filter(|example| example.source() == source)
            .collect::<Vec<_>>();
        assert_eq!(examples.len(), 2);
        assert!(examples.iter().all(|example| example.split() == split));
        assert_eq!(admitted.receipt().decisions_in(split), 2);
    }
}

#[test]
fn corrupted_claimed_split_rejects_before_it_can_leak_a_source_group() {
    let source = one_source_per_split()[&OfflinePolicySplit::Training];
    let admitted =
        admit_external_decision_logs([encoded_log(source, [chosen_point(b"leakage")])]).unwrap();
    let mut encoded = admitted.encode();
    encoded[48] = OfflinePolicySplit::Regression.tag_for_test();
    assert_eq!(
        decode_offline_policy_corpus(&encoded),
        Err(OfflinePolicyCorpusError::SourceSplitMismatch)
    );
}

#[test]
fn corrupted_member_cannot_cross_a_prior_source_group_partition() {
    let source = one_source_per_split()[&OfflinePolicySplit::Training];
    let admitted = admit_external_decision_logs([
        encoded_log(source, [chosen_point(b"leakage-a")]),
        encoded_log(source, [chosen_point(b"leakage-b")]),
    ])
    .unwrap();
    let mut encoded = admitted.encode();
    let first_length = u32::from_le_bytes(encoded[49..53].try_into().unwrap()) as usize;
    let second_split = 48 + 5 + first_length;
    encoded[second_split] = OfflinePolicySplit::Regression.tag_for_test();
    assert_eq!(
        decode_offline_policy_corpus(&encoded),
        Err(OfflinePolicyCorpusError::SourceSplitLeakage)
    );
}

fn one_source_per_split() -> BTreeMap<OfflinePolicySplit, OptimizationUnitIdentity> {
    let mut found = BTreeMap::new();
    for ordinal in 0_u64..10_000 {
        let source = OptimizationUnitIdentity::from_canonical_bytes(&ordinal.to_le_bytes());
        found.entry(split_for_source(source)).or_insert(source);
        if found.len() == 3 {
            return found;
        }
    }
    panic!("deterministic source partition did not expose all three splits")
}

trait SplitTagForTest {
    fn tag_for_test(self) -> u8;
}

impl SplitTagForTest for OfflinePolicySplit {
    fn tag_for_test(self) -> u8 {
        match self {
            OfflinePolicySplit::Training => 1,
            OfflinePolicySplit::Evaluation => 2,
            OfflinePolicySplit::Regression => 3,
        }
    }
}
