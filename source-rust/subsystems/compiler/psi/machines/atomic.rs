use language_core::atomic::AtomicOrderingPlan;
pub use language_core::atomic::{FenceOrdering, MemoryOrdering};
use semantic_vocabulary::ValueId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicReadModifyWrite {
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchOr,
    FetchXor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicAccessEvent<Operand = ValueId> {
    Load {
        ordering: MemoryOrdering,
    },
    Store {
        value: Operand,
        ordering: MemoryOrdering,
    },
    ReadModifyWrite {
        operation: AtomicReadModifyWrite,
        operand: Operand,
        ordering: MemoryOrdering,
    },
    Swap {
        value: Operand,
        ordering: MemoryOrdering,
    },
    CompareExchange {
        expected: Operand,
        replacement: Operand,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
    CompareExchangeOnce {
        custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
        expected: Operand,
        replacement: Operand,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

impl<Operand: Copy> AtomicAccessEvent<Operand> {
    /// Keep the same atomic family and ordering when an operand consumer
    /// asks what access the instruction performs. Ordinary read/write labels
    /// cannot express the atomic contract or its shared receiver.
    pub const fn access_operation(self) -> access_vocabulary::AtomicAccessOperation {
        use access_vocabulary::AtomicAccessOperation as Access;
        match self {
            Self::Load { ordering } => Access::Load(ordering),
            Self::Store { ordering, .. } => Access::Store(ordering),
            Self::ReadModifyWrite {
                operation,
                ordering,
                ..
            } => match operation {
                AtomicReadModifyWrite::FetchAdd => Access::FetchAdd(ordering),
                AtomicReadModifyWrite::FetchSub => Access::FetchSub(ordering),
                AtomicReadModifyWrite::FetchAnd => Access::FetchAnd(ordering),
                AtomicReadModifyWrite::FetchOr => Access::FetchOr(ordering),
                AtomicReadModifyWrite::FetchXor => Access::FetchXor(ordering),
            },
            Self::Swap { ordering, .. } => Access::Swap(ordering),
            Self::CompareExchange {
                success, failure, ..
            } => Access::CompareExchange { success, failure },
            Self::CompareExchangeOnce {
                custody,
                success,
                failure,
                ..
            } => Access::single_attempt(custody.observation, success, failure),
        }
    }

    pub const fn replacement(self) -> Option<Operand> {
        match self {
            Self::Store { value, .. } | Self::Swap { value, .. } => Some(value),
            Self::CompareExchange { replacement, .. }
            | Self::CompareExchangeOnce { replacement, .. } => Some(replacement),
            _ => None,
        }
    }

    pub const fn single_attempt_custody(
        self,
    ) -> Option<language_core::atomic::AtomicCompareExchangeOnceResultCustody> {
        match self {
            Self::CompareExchangeOnce { custody, .. } => Some(custody),
            _ => None,
        }
    }
    pub const fn ordering_plan(self) -> AtomicOrderingPlan {
        match self {
            Self::Load { ordering } => AtomicOrderingPlan::Load(ordering),
            Self::Store { ordering, .. } => AtomicOrderingPlan::Store(ordering),
            Self::ReadModifyWrite { ordering, .. } => AtomicOrderingPlan::ReadModifyWrite(ordering),
            Self::Swap { ordering, .. } => AtomicOrderingPlan::Swap(ordering),
            Self::CompareExchange {
                success, failure, ..
            } => AtomicOrderingPlan::CompareExchange { success, failure },
            Self::CompareExchangeOnce {
                success, failure, ..
            } => AtomicOrderingPlan::CompareExchangeOnce { success, failure },
        }
    }

    pub const fn ordering_is_legal(self) -> bool {
        match self {
            Self::Load { ordering } => ordering.valid_for_load(),
            Self::Store { ordering, .. } => ordering.valid_for_store(),
            Self::ReadModifyWrite { .. } | Self::Swap { .. } => true,
            Self::CompareExchange {
                success, failure, ..
            }
            | Self::CompareExchangeOnce {
                success, failure, ..
            } => failure.valid_compare_exchange_failure(success),
        }
    }

    pub const fn observes_resident(self) -> bool {
        !matches!(self, Self::Store { .. })
    }

    pub const fn modifies_resident(self) -> bool {
        !matches!(self, Self::Load { .. })
    }

    pub fn operands(self) -> Vec<Operand> {
        match self {
            Self::Load { .. } => Vec::new(),
            Self::Store { value, .. } | Self::Swap { value, .. } => vec![value],
            Self::ReadModifyWrite { operand, .. } => vec![operand],
            Self::CompareExchange {
                expected,
                replacement,
                ..
            }
            | Self::CompareExchangeOnce {
                expected,
                replacement,
                ..
            } => vec![expected, replacement],
        }
    }

    /// Change only the operand representation at a stage boundary. Event
    /// identity, ordering and result custody remain the same operation.
    pub fn try_map<Next, Error>(
        self,
        mut map: impl FnMut(Operand) -> Result<Next, Error>,
    ) -> Result<AtomicAccessEvent<Next>, Error> {
        Ok(match self {
            Self::Load { ordering } => AtomicAccessEvent::Load { ordering },
            Self::Store { value, ordering } => AtomicAccessEvent::Store {
                value: map(value)?,
                ordering,
            },
            Self::ReadModifyWrite {
                operation,
                operand,
                ordering,
            } => AtomicAccessEvent::ReadModifyWrite {
                operation,
                operand: map(operand)?,
                ordering,
            },
            Self::Swap { value, ordering } => AtomicAccessEvent::Swap {
                value: map(value)?,
                ordering,
            },
            Self::CompareExchange {
                expected,
                replacement,
                success,
                failure,
            } => AtomicAccessEvent::CompareExchange {
                expected: map(expected)?,
                replacement: map(replacement)?,
                success,
                failure,
            },
            Self::CompareExchangeOnce {
                custody,
                expected,
                replacement,
                success,
                failure,
            } => AtomicAccessEvent::CompareExchangeOnce {
                custody,
                expected: map(expected)?,
                replacement: map(replacement)?,
                success,
                failure,
            },
        })
    }

    pub fn map_operands(&mut self, map: &mut impl FnMut(Operand) -> Operand) {
        match self {
            Self::Load { .. } => {}
            Self::Store { value, .. } | Self::Swap { value, .. } => *value = map(*value),
            Self::ReadModifyWrite { operand, .. } => *operand = map(*operand),
            Self::CompareExchange {
                expected,
                replacement,
                ..
            }
            | Self::CompareExchangeOnce {
                expected,
                replacement,
                ..
            } => {
                *expected = map(*expected);
                *replacement = map(*replacement);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomicAccessEvent, AtomicReadModifyWrite, MemoryOrdering, ValueId};

    fn value(index: u64) -> ValueId {
        ValueId::new(index).expect("nonzero value identity")
    }

    #[test]
    fn legality_replays_the_source_ordering_matrix() {
        assert!(
            AtomicAccessEvent::<ValueId>::Load {
                ordering: MemoryOrdering::Receive
            }
            .ordering_is_legal()
        );
        assert!(
            !AtomicAccessEvent::<ValueId>::Load {
                ordering: MemoryOrdering::Publish
            }
            .ordering_is_legal()
        );
        assert!(
            !AtomicAccessEvent::Store {
                value: value(1),
                ordering: MemoryOrdering::ReceivePublish,
            }
            .ordering_is_legal()
        );
        assert!(
            AtomicAccessEvent::CompareExchange {
                expected: value(1),
                replacement: value(2),
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            }
            .ordering_is_legal()
        );
        assert!(
            !AtomicAccessEvent::CompareExchange {
                expected: value(1),
                replacement: value(2),
                success: MemoryOrdering::Receive,
                failure: MemoryOrdering::GlobalOrder,
            }
            .ordering_is_legal()
        );
    }

    #[test]
    fn operands_and_result_shape_follow_the_event() {
        let fetch = AtomicAccessEvent::ReadModifyWrite {
            operation: AtomicReadModifyWrite::FetchXor,
            operand: value(3),
            ordering: MemoryOrdering::NoOrdering,
        };
        assert_eq!(fetch.operands(), vec![value(3)]);
        assert!(fetch.observes_resident() && fetch.modifies_resident());
        let store = AtomicAccessEvent::Store {
            value: value(4),
            ordering: MemoryOrdering::GlobalOrder,
        };
        assert!(!store.observes_resident() && store.modifies_resident());
        let mut exchange = AtomicAccessEvent::CompareExchange {
            expected: value(5),
            replacement: value(6),
            success: MemoryOrdering::GlobalOrder,
            failure: MemoryOrdering::GlobalOrder,
        };
        exchange.map_operands(&mut |operand| value(operand.get() + 10));
        assert_eq!(exchange.operands(), vec![value(15), value(16)]);
    }
}
