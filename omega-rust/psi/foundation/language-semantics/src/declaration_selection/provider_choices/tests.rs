use super::*;

struct Map {
    slot: u32,
    provider: u32,
    required: Vec<u32>,
    rows: Vec<u32>,
    receivers: Vec<(u32, Vec<u32>)>,
    target: &'static str,
    contracts_known: bool,
}
impl Candidate for Map {
    type Slot = u32;
    type Provider = u32;
    fn slot(&self) -> &u32 {
        &self.slot
    }
    fn provider(&self) -> &u32 {
        &self.provider
    }
    fn target(&self) -> &str {
        self.target
    }
    fn has_requirements(&self) -> bool {
        !self.required.is_empty()
    }
    fn covers_requirements(&self) -> bool {
        self.required
            .iter()
            .all(|key| self.rows.iter().filter(|row| *row == key).count() == 1)
            && self.rows.iter().all(|row| self.required.contains(row))
    }
    fn receiver_count(&self) -> usize {
        self.receivers.len()
    }
    fn supplies_receiver(&self, other: &Self) -> Option<bool> {
        if !self.contracts_known || !other.contracts_known {
            return None;
        }
        Some(
            self.receivers
                .iter()
                .any(|receiver| other.receivers.first() == Some(receiver)),
        )
    }
    fn shares_receiver(&self, other: &Self) -> Option<bool> {
        if !self.contracts_known || !other.contracts_known {
            return None;
        }
        Some(
            self.receivers
                .iter()
                .any(|receiver| other.receivers.contains(receiver)),
        )
    }
}
struct Selected {
    slot: u32,
    provider: u32,
}
impl Declaration<u32, u32> for Selected {
    fn target(&self) -> Option<&str> {
        None
    }
    fn slots(&self, _: &[u32]) -> Vec<u32> {
        vec![self.slot]
    }
    fn provider(&self) -> &u32 {
        &self.provider
    }
}
fn map(slot: u32, provider: u32, required: &[u32], rows: &[u32]) -> Map {
    Map {
        slot,
        provider,
        required: required.to_vec(),
        rows: rows.to_vec(),
        receivers: vec![(slot, required.to_vec())],
        target: "",
        contracts_known: true,
    }
}

#[test]
fn partial_maps_never_union_into_a_whole_choice() {
    let maps = [map(1, 10, &[1, 2], &[1]), map(1, 20, &[1, 2], &[2])];
    let outcome = choose::<_, Selected>(&maps, "host", &[], &[]);
    assert!(outcome.choices.is_empty());
    assert!(outcome.errors.is_empty());
}

#[test]
fn override_replaces_the_whole_default_including_partial_default() {
    let maps = [map(1, 10, &[1, 2], &[1]), map(1, 20, &[1, 2], &[1, 2])];
    let outcome = choose(
        &maps,
        "host",
        &[Selected {
            slot: 1,
            provider: 10,
        }],
        &[Selected {
            slot: 1,
            provider: 20,
        }],
    );
    assert_eq!(
        outcome.choices,
        [Choice {
            candidate: 1,
            reason: Reason::Override(vec![0])
        }]
    );
    assert!(outcome.errors.is_empty());
}

#[test]
fn one_descendant_map_owns_its_entire_inherited_receiver() {
    let mut child = map(2, 20, &[1, 2], &[1, 2]);
    child.receivers.push((1, vec![1]));
    let maps = [map(1, 10, &[1], &[1]), child];
    let outcome = choose::<_, Selected>(&maps, "host", &[], &[]);
    assert_eq!(
        outcome.choices,
        [Choice {
            candidate: 1,
            reason: Reason::UniqueCoveringCandidate
        }]
    );
    assert!(outcome.errors.is_empty());
}

#[test]
fn ambiguous_whole_maps_preserve_the_candidate_coordinates() {
    let maps = [map(1, 10, &[1], &[1]), map(1, 20, &[1], &[1])];
    let outcome = choose::<_, Selected>(&maps, "host", &[], &[]);
    assert!(outcome.choices.is_empty());
    assert_eq!(
        outcome.errors,
        [Error::AmbiguousCandidates {
            slot: 1,
            candidates: vec![0, 1]
        }]
    );
}

#[test]
fn an_unavailable_target_is_not_an_empty_successful_choice() {
    let mut foreign = map(1, 10, &[1], &[1]);
    foreign.target = "other";
    let outcome = choose(
        &[foreign],
        "host",
        &[],
        &[Selected {
            slot: 1,
            provider: 10,
        }],
    );
    assert!(outcome.choices.is_empty());
    assert_eq!(
        outcome.errors,
        [Error::MissingChoice {
            tier: Tier::Override,
            declaration: 0,
            slot: 1,
            wrong_target: true
        }]
    );
}

#[test]
fn unknown_contracts_preserve_independent_choices_without_claiming_inheritance() {
    let mut first = map(2, 20, &[1, 2], &[1, 2]);
    first.receivers.push((1, vec![1]));
    first.contracts_known = false;
    let mut second = map(3, 30, &[1, 3], &[1, 3]);
    second.receivers.push((1, vec![1]));
    second.contracts_known = false;
    let maps = [map(1, 10, &[1], &[1]), first, second];
    let outcome = choose(
        &maps,
        "host",
        &[],
        &[
            Selected {
                slot: 2,
                provider: 20,
            },
            Selected {
                slot: 3,
                provider: 30,
            },
        ],
    );
    assert_eq!(
        outcome
            .choices
            .iter()
            .map(|row| row.candidate)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        outcome.errors,
        [
            Error::UnresolvedInheritedReceiver { slot: 1 },
            Error::UnresolvedOverlap {
                first: 2,
                second: 1
            },
        ]
    );
}
