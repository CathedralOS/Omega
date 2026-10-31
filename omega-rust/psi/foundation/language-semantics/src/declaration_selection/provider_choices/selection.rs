use super::{Candidate, Choice, Declaration, Error, Outcome, Reason, Tier};

pub fn choose<CandidateView, DeclarationView>(
    candidates: &[CandidateView],
    target: &str,
    defaults: &[DeclarationView],
    overrides: &[DeclarationView],
) -> Outcome<CandidateView::Slot, CandidateView::Provider>
where
    CandidateView: Candidate,
    DeclarationView: Declaration<CandidateView::Slot, CandidateView::Provider>,
{
    let mut errors = Vec::new();
    let mut choices: Vec<Choice> = Vec::new();
    let mut slots = candidates
        .iter()
        .filter(|row| row.has_requirements())
        .map(|row| row.slot().clone())
        .collect::<Vec<_>>();
    slots.sort_unstable();
    slots.dedup();
    let mut resolved = |declarations: &[DeclarationView], tier| {
        let mut rows = Vec::new();
        for (index, declaration) in declarations
            .iter()
            .enumerate()
            .filter(|(_, row)| row.target().is_none_or(|scope| scope == target))
        {
            for slot in declaration.slots(&slots) {
                if slots.contains(&slot) {
                    rows.push((slot, index));
                } else {
                    errors.push(Error::UnknownSlot {
                        tier,
                        declaration: index,
                        slot,
                    });
                }
            }
        }
        rows
    };
    let requests = resolved(overrides, Tier::Override);
    let fallbacks = resolved(defaults, Tier::Default);
    for slot in &slots {
        let declarations = requests
            .iter()
            .filter(|(selected, _)| selected == slot)
            .map(|(_, index)| *index)
            .collect::<Vec<_>>();
        if declarations.len() > 1 {
            errors.push(Error::RepeatedOverride {
                slot: slot.clone(),
                declarations,
            });
        }
    }
    let applies =
        |candidate: &CandidateView| candidate.target().is_empty() || candidate.target() == target;
    let mut pending = slots
        .into_iter()
        .map(|slot| {
            let authored = requests
                .iter()
                .chain(&fallbacks)
                .any(|(selected, _)| *selected == slot);
            let receivers = candidates
                .iter()
                .filter(|row| row.slot() == &slot && applies(row))
                .map(Candidate::receiver_count)
                .max()
                .unwrap_or(0);
            (slot, authored, receivers)
        })
        .collect::<Vec<_>>();
    pending.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    for (slot, _, _) in pending {
        let explicit = requests
            .iter()
            .find(|(selected, _)| *selected == slot)
            .map(|(_, index)| *index);
        let slot_defaults = fallbacks
            .iter()
            .filter(|(selected, _)| *selected == slot)
            .map(|(_, index)| *index)
            .collect::<Vec<_>>();
        let applicable = candidates
            .iter()
            .enumerate()
            .filter(|(_, row)| row.slot() == &slot && applies(row))
            .collect::<Vec<_>>();
        let covering = applicable
            .iter()
            .copied()
            .filter(|(_, row)| row.covers_requirements())
            .collect::<Vec<_>>();
        let selected = if let Some(index) = explicit {
            Some((
                Tier::Override,
                index,
                &overrides[index],
                Reason::Override(vec![index]),
            ))
        } else if let Some(&index) = slot_defaults.first() {
            let mut providers = slot_defaults
                .iter()
                .map(|index| defaults[*index].provider().clone())
                .collect::<Vec<_>>();
            providers.sort_unstable();
            providers.dedup();
            if providers.len() > 1 {
                errors.push(Error::ConflictingDefaults { slot, providers });
                continue;
            }
            Some((
                Tier::Default,
                index,
                &defaults[index],
                Reason::Default(slot_defaults),
            ))
        } else {
            None
        };
        if let Some((tier, declaration, selected, reason)) = selected {
            let provider = selected.provider();
            let matching = applicable
                .iter()
                .copied()
                .filter(|(_, row)| row.provider() == provider)
                .collect::<Vec<_>>();
            match matching.as_slice() {
                [(candidate, row)] if row.covers_requirements() => choices.push(Choice {
                    candidate: *candidate,
                    reason,
                }),
                [(candidate, _)] => errors.push(Error::PartialChoice {
                    tier,
                    declaration,
                    candidate: *candidate,
                }),
                [] => errors.push(Error::MissingChoice {
                    tier,
                    declaration,
                    wrong_target: candidates
                        .iter()
                        .any(|row| row.slot() == &slot && row.provider() == provider),
                    slot,
                }),
                _ => errors.push(Error::RepeatedCandidate {
                    tier,
                    declaration,
                    slot,
                }),
            }
            continue;
        }
        if !applicable.is_empty() {
            let mut inherited = 0;
            let mut unresolved = false;
            for choice in &choices {
                let whole = &candidates[choice.candidate];
                if !whole.covers_requirements() {
                    continue;
                }
                let mut possible = true;
                let mut known = true;
                for (_, candidate) in &applicable {
                    match whole.supplies_receiver(candidate) {
                        Some(true) => {}
                        Some(false) => {
                            possible = false;
                            break;
                        }
                        None => known = false,
                    }
                }
                if possible {
                    if known {
                        inherited += 1;
                    } else {
                        unresolved = true;
                    }
                }
            }
            if inherited > 1 {
                errors.push(Error::RepeatedInheritedReceiver { slot });
                continue;
            }
            if unresolved {
                errors.push(Error::UnresolvedInheritedReceiver { slot });
                continue;
            }
            if inherited == 1 {
                continue;
            }
        }
        match covering.as_slice() {
            [] => {}
            [(candidate, _)] => choices.push(Choice {
                candidate: *candidate,
                reason: Reason::UniqueCoveringCandidate,
            }),
            rows => errors.push(Error::AmbiguousCandidates {
                slot,
                candidates: rows.iter().map(|(index, _)| *index).collect(),
            }),
        }
    }
    for (position, choice) in choices.iter().enumerate() {
        for other in &choices[..position] {
            match candidates[choice.candidate].shares_receiver(&candidates[other.candidate]) {
                Some(true) => errors.push(Error::OverlappingChoices {
                    first: choice.candidate,
                    second: other.candidate,
                }),
                Some(false) => {}
                None => errors.push(Error::UnresolvedOverlap {
                    first: choice.candidate,
                    second: other.candidate,
                }),
            }
        }
    }
    Outcome { choices, errors }
}
