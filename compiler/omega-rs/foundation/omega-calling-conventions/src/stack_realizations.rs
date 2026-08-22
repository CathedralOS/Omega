//! Target-neutral external-entry stack context and epoch plans.
//!
//! These carriers describe the finite shape a target/provider claims. The
//! validated wrapper proves only structural closure and canonical identity;
//! installation still has to bind the plan to sealed target facts, emitted
//! adapter bytes, or an admitted opaque-provider receipt.

use crate::{EntryStack, PlanDiagnostic, Preemption};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArrivalContextId(u64);

impl ArrivalContextId {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StackDomainRef {
    Interrupted,
    Dedicated { class: u16 },
    ProviderSelected,
}

impl From<EntryStack> for StackDomainRef {
    fn from(value: EntryStack) -> Self {
        match value {
            EntryStack::Interrupted => Self::Interrupted,
            EntryStack::Dedicated { class } => Self::Dedicated { class },
            EntryStack::ProviderSelected => Self::ProviderSelected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStackStage {
    Enter,
    Body,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackOccupancy {
    pub domain: StackDomainRef,
    pub bytes: u64,
    pub alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryStackEpoch {
    pub stage: EntryStackStage,
    pub active_domain: StackDomainRef,
    pub occupancy_by_domain: Vec<StackOccupancy>,
    pub nesting: Preemption,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalContextRealization {
    pub context: ArrivalContextId,
    pub epochs: Vec<EntryStackEpoch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryStackRealization {
    pub contexts: Vec<ArrivalContextRealization>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEntryStackRealization {
    realization: EntryStackRealization,
    fingerprint: u64,
}

impl ValidatedEntryStackRealization {
    pub const fn realization(&self) -> &EntryStackRealization {
        &self.realization
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

pub fn validate_entry_stack_realization(
    mut realization: EntryStackRealization,
) -> Result<ValidatedEntryStackRealization, PlanDiagnostic> {
    if realization.contexts.is_empty() {
        return Err(PlanDiagnostic(
            "entry stack realization has no admissible arrival context".into(),
        ));
    }
    realization.contexts.sort_by_key(|context| context.context);
    for index in 0..realization.contexts.len() {
        if index > 0
            && realization.contexts[index - 1].context == realization.contexts[index].context
        {
            return Err(PlanDiagnostic(
                "entry stack realization repeats an arrival-context identity".into(),
            ));
        }
        validate_context(&mut realization.contexts[index])?;
    }
    let fingerprint = fingerprint_realization(&realization);
    Ok(ValidatedEntryStackRealization {
        realization,
        fingerprint,
    })
}

fn validate_context(context: &mut ArrivalContextRealization) -> Result<(), PlanDiagnostic> {
    if context.epochs.is_empty() {
        return Err(PlanDiagnostic(format!(
            "arrival context 0x{:016x} has no stack epoch",
            context.context.get()
        )));
    }
    let mut body_count = 0usize;
    let mut phase = EntryStackStage::Enter;
    for (epoch_index, epoch) in context.epochs.iter_mut().enumerate() {
        if epoch.active_domain == StackDomainRef::ProviderSelected {
            return Err(PlanDiagnostic(format!(
                "arrival context 0x{:016x} epoch {epoch_index} retains an unresolved provider-selected active stack domain",
                context.context.get()
            )));
        }
        match epoch.nesting {
            Preemption::NotApplicable | Preemption::Masked => {}
            Preemption::Nestable { maximum_depth } if maximum_depth > 0 => {}
            Preemption::Nestable { .. } => {
                return Err(PlanDiagnostic(format!(
                    "arrival context 0x{:016x} epoch {epoch_index} has zero finite nesting depth",
                    context.context.get()
                )));
            }
            Preemption::ProviderDefined => {
                return Err(PlanDiagnostic(format!(
                    "arrival context 0x{:016x} epoch {epoch_index} retains unresolved provider-defined nesting",
                    context.context.get()
                )));
            }
        }
        match epoch.stage {
            EntryStackStage::Enter if phase == EntryStackStage::Enter => {}
            EntryStackStage::Body if phase == EntryStackStage::Enter => {
                phase = EntryStackStage::Body;
                body_count += 1;
            }
            EntryStackStage::Exit
                if matches!(phase, EntryStackStage::Body | EntryStackStage::Exit) =>
            {
                phase = EntryStackStage::Exit;
            }
            _ => {
                return Err(PlanDiagnostic(format!(
                    "arrival context 0x{:016x} has a noncanonical enter/body/exit epoch order",
                    context.context.get()
                )));
            }
        }
        epoch
            .occupancy_by_domain
            .sort_by_key(|occupancy| occupancy.domain);
        for occupancy_index in 0..epoch.occupancy_by_domain.len() {
            let occupancy = epoch.occupancy_by_domain[occupancy_index];
            if occupancy.domain == StackDomainRef::ProviderSelected {
                return Err(PlanDiagnostic(format!(
                    "arrival context 0x{:016x} epoch {epoch_index} retains unresolved provider-selected occupancy",
                    context.context.get()
                )));
            }
            if occupancy.bytes == 0 {
                return Err(PlanDiagnostic(format!(
                    "arrival context 0x{:016x} epoch {epoch_index} has zero-byte occupancy",
                    context.context.get()
                )));
            }
            if occupancy.alignment == 0 || !occupancy.alignment.is_power_of_two() {
                return Err(PlanDiagnostic(format!(
                    "arrival context 0x{:016x} epoch {epoch_index} has non-power-of-two occupancy alignment {}",
                    context.context.get(),
                    occupancy.alignment
                )));
            }
            if occupancy_index > 0
                && epoch.occupancy_by_domain[occupancy_index - 1].domain == occupancy.domain
            {
                return Err(PlanDiagnostic(format!(
                    "arrival context 0x{:016x} epoch {epoch_index} repeats one occupancy domain",
                    context.context.get()
                )));
            }
        }
    }
    if body_count != 1 {
        return Err(PlanDiagnostic(format!(
            "arrival context 0x{:016x} has {body_count} body epochs instead of exactly one",
            context.context.get()
        )));
    }
    Ok(())
}

fn fingerprint_realization(realization: &EntryStackRealization) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(realization.contexts.len() as u64);
    for context in &realization.contexts {
        hash.u64(context.context.get());
        hash.u64(context.epochs.len() as u64);
        for epoch in &context.epochs {
            hash.u64(match epoch.stage {
                EntryStackStage::Enter => 1,
                EntryStackStage::Body => 2,
                EntryStackStage::Exit => 3,
            });
            fingerprint_domain(&mut hash, epoch.active_domain);
            match epoch.nesting {
                Preemption::NotApplicable => hash.u64(0),
                Preemption::Masked => hash.u64(1),
                Preemption::Nestable { maximum_depth } => {
                    hash.u64(2);
                    hash.u64(u64::from(maximum_depth));
                }
                Preemption::ProviderDefined => unreachable!("validated above"),
            }
            hash.u64(epoch.occupancy_by_domain.len() as u64);
            for occupancy in &epoch.occupancy_by_domain {
                fingerprint_domain(&mut hash, occupancy.domain);
                hash.u64(occupancy.bytes);
                hash.u64(occupancy.alignment);
            }
        }
    }
    hash.finish()
}

fn fingerprint_domain(hash: &mut Fnv1a, domain: StackDomainRef) {
    match domain {
        StackDomainRef::Interrupted => hash.u64(0),
        StackDomainRef::Dedicated { class } => {
            hash.u64(1);
            hash.u64(u64::from(class));
        }
        StackDomainRef::ProviderSelected => unreachable!("validated above"),
    }
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(0xcbf29ce484222325)
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(value: u64, occupancy: Vec<StackOccupancy>) -> ArrivalContextRealization {
        ArrivalContextRealization {
            context: ArrivalContextId::new(value).expect("context identity"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain: StackDomainRef::Interrupted,
                occupancy_by_domain: occupancy,
                nesting: Preemption::Nestable { maximum_depth: 2 },
            }],
        }
    }

    #[test]
    fn canonical_identity_ignores_context_and_occupancy_input_order() {
        let interrupted = StackOccupancy {
            domain: StackDomainRef::Interrupted,
            bytes: 24,
            alignment: 8,
        };
        let dedicated = StackOccupancy {
            domain: StackDomainRef::Dedicated { class: 7 },
            bytes: 16,
            alignment: 16,
        };
        let first = validate_entry_stack_realization(EntryStackRealization {
            contexts: vec![
                context(2, vec![interrupted, dedicated]),
                context(1, vec![dedicated, interrupted]),
            ],
        })
        .expect("first realization");
        let second = validate_entry_stack_realization(EntryStackRealization {
            contexts: vec![
                context(1, vec![interrupted, dedicated]),
                context(2, vec![dedicated, interrupted]),
            ],
        })
        .expect("second realization");

        assert_eq!(first, second);
        assert_eq!(first.fingerprint(), second.fingerprint());
    }

    #[test]
    fn structural_validation_rejects_open_or_malformed_realizations() {
        let valid_context = context(
            1,
            vec![StackOccupancy {
                domain: StackDomainRef::Interrupted,
                bytes: 8,
                alignment: 8,
            }],
        );
        let cases = [
            (
                EntryStackRealization::default(),
                "no admissible arrival context",
            ),
            (
                EntryStackRealization {
                    contexts: vec![valid_context.clone(), valid_context.clone()],
                },
                "repeats an arrival-context identity",
            ),
            (
                EntryStackRealization {
                    contexts: vec![ArrivalContextRealization {
                        context: ArrivalContextId::new(1).expect("identity"),
                        epochs: vec![EntryStackEpoch {
                            stage: EntryStackStage::Body,
                            active_domain: StackDomainRef::ProviderSelected,
                            occupancy_by_domain: Vec::new(),
                            nesting: Preemption::Masked,
                        }],
                    }],
                },
                "unresolved provider-selected active stack domain",
            ),
            (
                EntryStackRealization {
                    contexts: vec![ArrivalContextRealization {
                        context: ArrivalContextId::new(1).expect("identity"),
                        epochs: vec![EntryStackEpoch {
                            stage: EntryStackStage::Body,
                            active_domain: StackDomainRef::Interrupted,
                            occupancy_by_domain: Vec::new(),
                            nesting: Preemption::ProviderDefined,
                        }],
                    }],
                },
                "unresolved provider-defined nesting",
            ),
        ];
        for (realization, expected) in cases {
            let error = validate_entry_stack_realization(realization)
                .expect_err("malformed realization must reject");
            assert!(error.0.contains(expected), "{}", error.0);
        }
    }

    #[test]
    fn epoch_order_and_body_cardinality_are_closed() {
        let epoch = |stage| EntryStackEpoch {
            stage,
            active_domain: StackDomainRef::Interrupted,
            occupancy_by_domain: Vec::new(),
            nesting: Preemption::Masked,
        };
        let wrong_order = EntryStackRealization {
            contexts: vec![ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("identity"),
                epochs: vec![epoch(EntryStackStage::Body), epoch(EntryStackStage::Enter)],
            }],
        };
        assert!(
            validate_entry_stack_realization(wrong_order)
                .expect_err("enter after body must reject")
                .0
                .contains("noncanonical")
        );

        let two_bodies = EntryStackRealization {
            contexts: vec![ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("identity"),
                epochs: vec![epoch(EntryStackStage::Body), epoch(EntryStackStage::Body)],
            }],
        };
        assert!(
            validate_entry_stack_realization(two_bodies)
                .expect_err("two body epochs must reject")
                .0
                .contains("noncanonical")
        );
    }
}
