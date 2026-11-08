/*
We define compiler-owned atomic memory-order vocabulary, operation legality,
and compare-exchange outcome schemas. These names are semantic commitments that
survive carrier lowering so later phases do not have to rediscover atomic
semantics from an expression shape; source admission first rejects orderings
that the operation cannot mean.
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum MemoryOrdering {
    NoOrdering,
    Receive,
    Publish,
    ReceivePublish,
    GlobalOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum FenceOrdering {
    Receive,
    Publish,
    ReceivePublish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum AtomicOrderingPlan {
    Load(MemoryOrdering),
    Store(MemoryOrdering),
    ReadModifyWrite(MemoryOrdering),
    Swap(MemoryOrdering),
    CompareExchange {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
    CompareExchangeOnce {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AtomicObservingCompareExchangeOperation {
    Decisive,
    SingleAttempt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AtomicObservingCompareExchangeResultShape {
    ExchangedOrMismatchedObserved,
    ExchangedOrMismatchedOrUncommittedObserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum AtomicExpressionResultCustody {
    Scalar,
    CompareExchangeOnce(AtomicCompareExchangeOnceResultCustody),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
pub struct AtomicCompareExchangeOnceResultCustody {
    pub attempt: AtomicCompareExchangeAttemptAxis,
    pub observation: AtomicCompareExchangeObservationAxis,
    pub outcome_identity: AtomicCompareExchangeOutcomeIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AtomicCompareExchangeAttemptAxis {
    Decisive,
    SingleAttempt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AtomicCompareExchangeObservationAxis {
    Observing,
    NonObserving,
}

impl AtomicCompareExchangeObservationAxis {
    pub fn single_attempt_member(name: &str) -> Option<Self> {
        match name {
            "compare_exchange_once" => Some(Self::Observing),
            "try_exchange_once" => Some(Self::NonObserving),
            _ => None,
        }
    }

    pub const fn single_attempt_member_name(self) -> &'static str {
        match self {
            Self::Observing => "compare_exchange_once",
            Self::NonObserving => "try_exchange_once",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicCompareExchangeOutcomeCase {
    Mismatched,
    Exchanged,
    Uncommitted,
}

impl AtomicCompareExchangeOutcomeCase {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mismatched => "Mismatched",
            Self::Exchanged => "Exchanged",
            Self::Uncommitted => "Uncommitted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicCompareExchangeOutcomePayload {
    Observed,
    Proposed,
    Displaced,
}

impl AtomicCompareExchangeOutcomePayload {
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Proposed => "proposed",
            Self::Displaced => "displaced",
        }
    }

    pub const fn type_parameter_name(self) -> &'static str {
        "T"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtomicCompareExchangeOutcomeCaseSchema {
    pub case: AtomicCompareExchangeOutcomeCase,
    pub tag: u8,
    pub payload: Option<AtomicCompareExchangeOutcomePayload>,
}

macro_rules! exchange_case {
    ($case:ident => $tag:literal => $payload:expr) => {
        AtomicCompareExchangeOutcomeCaseSchema {
            case: AtomicCompareExchangeOutcomeCase::$case,
            tag: $tag,
            payload: $payload,
        }
    };
}

const OBSERVING_DECISIVE_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 2] = [
    exchange_case!(Mismatched => 0 => Some(AtomicCompareExchangeOutcomePayload::Observed)),
    exchange_case!(Exchanged => 1 => None),
];

const OBSERVING_SINGLE_ATTEMPT_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 3] = [
    exchange_case!(Mismatched => 0 => Some(AtomicCompareExchangeOutcomePayload::Observed)),
    exchange_case!(Exchanged => 1 => None),
    exchange_case!(Uncommitted => 2 => Some(AtomicCompareExchangeOutcomePayload::Observed)),
];

const NON_OBSERVING_DECISIVE_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 2] = [
    exchange_case!(Mismatched => 0 => Some(AtomicCompareExchangeOutcomePayload::Proposed)),
    exchange_case!(Exchanged => 1 => Some(AtomicCompareExchangeOutcomePayload::Displaced)),
];

const NON_OBSERVING_SINGLE_ATTEMPT_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 3] = [
    exchange_case!(Mismatched => 0 => Some(AtomicCompareExchangeOutcomePayload::Proposed)),
    exchange_case!(Exchanged => 1 => Some(AtomicCompareExchangeOutcomePayload::Displaced)),
    exchange_case!(Uncommitted => 2 => Some(AtomicCompareExchangeOutcomePayload::Proposed)),
];

const OUTCOME_TYPE_PARAMETERS: [&str; 1] = ["T"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AtomicCompareExchangeOutcomeIdentity {
    AtomicCompareExchangeOutcome,
    AtomicCompareExchangeOnceOutcome,
    AtomicTryExchangeOutcome,
    AtomicTryExchangeOnceOutcome,
}

impl AtomicCompareExchangeOutcomeIdentity {
    pub const ALL: [Self; 4] = [
        Self::AtomicCompareExchangeOutcome,
        Self::AtomicCompareExchangeOnceOutcome,
        Self::AtomicTryExchangeOutcome,
        Self::AtomicTryExchangeOnceOutcome,
    ];

    pub const fn from_axes(
        attempt: AtomicCompareExchangeAttemptAxis,
        observation: AtomicCompareExchangeObservationAxis,
    ) -> Self {
        match (attempt, observation) {
            (
                AtomicCompareExchangeAttemptAxis::Decisive,
                AtomicCompareExchangeObservationAxis::Observing,
            ) => Self::AtomicCompareExchangeOutcome,
            (
                AtomicCompareExchangeAttemptAxis::SingleAttempt,
                AtomicCompareExchangeObservationAxis::Observing,
            ) => Self::AtomicCompareExchangeOnceOutcome,
            (
                AtomicCompareExchangeAttemptAxis::Decisive,
                AtomicCompareExchangeObservationAxis::NonObserving,
            ) => Self::AtomicTryExchangeOutcome,
            (
                AtomicCompareExchangeAttemptAxis::SingleAttempt,
                AtomicCompareExchangeObservationAxis::NonObserving,
            ) => Self::AtomicTryExchangeOnceOutcome,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::AtomicCompareExchangeOutcome => "AtomicCompareExchangeOutcome",
            Self::AtomicCompareExchangeOnceOutcome => "AtomicCompareExchangeOnceOutcome",
            Self::AtomicTryExchangeOutcome => "AtomicTryExchangeOutcome",
            Self::AtomicTryExchangeOnceOutcome => "AtomicTryExchangeOnceOutcome",
        }
    }

    pub const fn type_parameters(self) -> &'static [&'static str] {
        &OUTCOME_TYPE_PARAMETERS
    }

    pub const fn attempt(self) -> AtomicCompareExchangeAttemptAxis {
        match self {
            Self::AtomicCompareExchangeOutcome | Self::AtomicTryExchangeOutcome => {
                AtomicCompareExchangeAttemptAxis::Decisive
            }
            Self::AtomicCompareExchangeOnceOutcome | Self::AtomicTryExchangeOnceOutcome => {
                AtomicCompareExchangeAttemptAxis::SingleAttempt
            }
        }
    }

    pub const fn observation(self) -> AtomicCompareExchangeObservationAxis {
        match self {
            Self::AtomicCompareExchangeOutcome | Self::AtomicCompareExchangeOnceOutcome => {
                AtomicCompareExchangeObservationAxis::Observing
            }
            Self::AtomicTryExchangeOutcome | Self::AtomicTryExchangeOnceOutcome => {
                AtomicCompareExchangeObservationAxis::NonObserving
            }
        }
    }

    pub const fn cases(self) -> &'static [AtomicCompareExchangeOutcomeCaseSchema] {
        match self {
            Self::AtomicCompareExchangeOutcome => &OBSERVING_DECISIVE_CASES,
            Self::AtomicCompareExchangeOnceOutcome => &OBSERVING_SINGLE_ATTEMPT_CASES,
            Self::AtomicTryExchangeOutcome => &NON_OBSERVING_DECISIVE_CASES,
            Self::AtomicTryExchangeOnceOutcome => &NON_OBSERVING_SINGLE_ATTEMPT_CASES,
        }
    }
}

impl AtomicObservingCompareExchangeOperation {
    pub const fn result_shape(self) -> AtomicObservingCompareExchangeResultShape {
        match self {
            Self::Decisive => {
                AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved
            }
            Self::SingleAttempt => AtomicObservingCompareExchangeResultShape::
                ExchangedOrMismatchedOrUncommittedObserved,
        }
    }

    pub const fn outcome_identity(self) -> AtomicCompareExchangeOutcomeIdentity {
        match self {
            Self::Decisive => AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOutcome,
            Self::SingleAttempt => {
                AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome
            }
        }
    }
}

impl AtomicObservingCompareExchangeResultShape {
    pub const fn outcome_identity(self) -> AtomicCompareExchangeOutcomeIdentity {
        match self {
            Self::ExchangedOrMismatchedObserved => {
                AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOutcome
            }
            Self::ExchangedOrMismatchedOrUncommittedObserved => {
                AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome
            }
        }
    }
}

impl AtomicCompareExchangeOnceResultCustody {
    pub const fn for_observation(observation: AtomicCompareExchangeObservationAxis) -> Self {
        let attempt = AtomicCompareExchangeAttemptAxis::SingleAttempt;
        Self {
            attempt,
            observation,
            outcome_identity: AtomicCompareExchangeOutcomeIdentity::from_axes(attempt, observation),
        }
    }

    pub const fn is_canonical(self) -> bool {
        matches!(
            (self.attempt, self.observation, self.outcome_identity),
            (
                AtomicCompareExchangeAttemptAxis::SingleAttempt,
                AtomicCompareExchangeObservationAxis::Observing,
                AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome,
            ) | (
                AtomicCompareExchangeAttemptAxis::SingleAttempt,
                AtomicCompareExchangeObservationAxis::NonObserving,
                AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOnceOutcome,
            )
        )
    }
}

impl AtomicExpressionResultCustody {
    pub const fn is_valid_for(self, ordering: AtomicOrderingPlan) -> bool {
        match (self, ordering) {
            (Self::Scalar, AtomicOrderingPlan::CompareExchangeOnce { .. }) => false,
            (Self::Scalar, _) => true,
            (
                Self::CompareExchangeOnce(custody),
                AtomicOrderingPlan::CompareExchangeOnce { .. },
            ) => custody.is_canonical(),
            (Self::CompareExchangeOnce(_), _) => false,
        }
    }

    pub const fn has_structural_result(self) -> bool {
        matches!(self, Self::CompareExchangeOnce(_))
    }
}

impl AtomicOrderingPlan {
    pub const fn success(self) -> MemoryOrdering {
        match self {
            Self::Load(ordering)
            | Self::Store(ordering)
            | Self::ReadModifyWrite(ordering)
            | Self::Swap(ordering) => ordering,
            Self::CompareExchange { success, .. } | Self::CompareExchangeOnce { success, .. } => {
                success
            }
        }
    }

    pub const fn failure(self) -> Option<MemoryOrdering> {
        match self {
            Self::CompareExchange { failure, .. } | Self::CompareExchangeOnce { failure, .. } => {
                Some(failure)
            }
            _ => None,
        }
    }
}

impl MemoryOrdering {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "NoOrdering" => Some(Self::NoOrdering),
            "Receive" => Some(Self::Receive),
            "Publish" => Some(Self::Publish),
            "ReceivePublish" => Some(Self::ReceivePublish),
            "GlobalOrder" => Some(Self::GlobalOrder),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::NoOrdering => "NoOrdering",
            Self::Receive => "Receive",
            Self::Publish => "Publish",
            Self::ReceivePublish => "ReceivePublish",
            Self::GlobalOrder => "GlobalOrder",
        }
    }

    pub const fn valid_for_load(self) -> bool {
        matches!(self, Self::NoOrdering | Self::Receive | Self::GlobalOrder)
    }

    pub const fn valid_for_store(self) -> bool {
        matches!(self, Self::NoOrdering | Self::Publish | Self::GlobalOrder)
    }

    pub const fn valid_compare_exchange_failure(self, success: Self) -> bool {
        match success {
            Self::NoOrdering => matches!(self, Self::NoOrdering),
            Self::Receive => matches!(self, Self::NoOrdering | Self::Receive),
            Self::Publish => matches!(self, Self::NoOrdering),
            Self::ReceivePublish => matches!(self, Self::NoOrdering | Self::Receive),
            Self::GlobalOrder => {
                matches!(self, Self::NoOrdering | Self::Receive | Self::GlobalOrder)
            }
        }
    }
}

pub const ATOMIC_CORE_SOURCE: &str = "atomic.omg";
pub const ATOMIC_FENCE_NAMESPACE: &str = "Atomic";
pub const ATOMIC_FENCE_NAME: &str = "fence";

#[cfg(test)]
mod tests {
    use super::MemoryOrdering as O;

    #[test]
    fn operation_legality_matches_the_c11_matrix() {
        assert!(O::NoOrdering.valid_for_load());
        assert!(O::Receive.valid_for_load());
        assert!(O::GlobalOrder.valid_for_load());
        assert!(!O::Publish.valid_for_load());
        assert!(!O::ReceivePublish.valid_for_load());

        assert!(O::NoOrdering.valid_for_store());
        assert!(O::Publish.valid_for_store());
        assert!(O::GlobalOrder.valid_for_store());
        assert!(!O::Receive.valid_for_store());
        assert!(!O::ReceivePublish.valid_for_store());

        assert!(O::Receive.valid_compare_exchange_failure(O::ReceivePublish));
        assert!(O::GlobalOrder.valid_compare_exchange_failure(O::GlobalOrder));
        assert!(!O::Publish.valid_compare_exchange_failure(O::GlobalOrder));
        assert!(!O::GlobalOrder.valid_compare_exchange_failure(O::Receive));
        assert!(!O::Receive.valid_compare_exchange_failure(O::Publish));
    }

    #[test]
    fn operation_plan_keeps_compare_exchange_axes_separate() {
        let decisive = super::AtomicOrderingPlan::CompareExchange {
            success: O::ReceivePublish,
            failure: O::Receive,
        };
        let once = super::AtomicOrderingPlan::CompareExchangeOnce {
            success: O::ReceivePublish,
            failure: O::Receive,
        };
        assert_ne!(decisive, once);
        assert_eq!(decisive.success(), O::ReceivePublish);
        assert_eq!(decisive.failure(), Some(O::Receive));
        assert_eq!(once.success(), O::ReceivePublish);
        assert_eq!(once.failure(), Some(O::Receive));
        assert_eq!(
            super::AtomicOrderingPlan::Load(O::GlobalOrder).failure(),
            None
        );
    }

    #[test]
    fn observing_compare_exchange_operations_have_closed_distinct_result_shapes() {
        use super::{
            AtomicObservingCompareExchangeOperation as Operation,
            AtomicObservingCompareExchangeResultShape as Shape,
        };

        assert_eq!(
            Operation::Decisive.result_shape(),
            Shape::ExchangedOrMismatchedObserved
        );
        assert_eq!(
            Operation::SingleAttempt.result_shape(),
            Shape::ExchangedOrMismatchedOrUncommittedObserved
        );
        assert_eq!(
            Operation::Decisive.outcome_identity(),
            Shape::ExchangedOrMismatchedObserved.outcome_identity()
        );
        assert_eq!(
            Operation::SingleAttempt.outcome_identity(),
            Shape::ExchangedOrMismatchedOrUncommittedObserved.outcome_identity()
        );
    }

    #[test]
    fn compare_exchange_outcomes_retain_the_complete_settled_two_axis_schema() {
        use super::{
            AtomicCompareExchangeAttemptAxis as Attempt,
            AtomicCompareExchangeObservationAxis as Observation,
            AtomicCompareExchangeOutcomeCase as Case,
            AtomicCompareExchangeOutcomeCaseSchema as CaseSchema,
            AtomicCompareExchangeOutcomeIdentity as Identity,
            AtomicCompareExchangeOutcomePayload as Payload,
        };

        macro_rules! case {
            ($case:ident => $tag:literal => $payload:expr) => {
                CaseSchema {
                    case: Case::$case,
                    tag: $tag,
                    payload: $payload,
                }
            };
        }

        let expected = [
            (
                Identity::AtomicCompareExchangeOutcome,
                "AtomicCompareExchangeOutcome",
                Attempt::Decisive,
                Observation::Observing,
                vec![
                    case!(Mismatched => 0 => Some(Payload::Observed)),
                    case!(Exchanged => 1 => None),
                ],
            ),
            (
                Identity::AtomicCompareExchangeOnceOutcome,
                "AtomicCompareExchangeOnceOutcome",
                Attempt::SingleAttempt,
                Observation::Observing,
                vec![
                    case!(Mismatched => 0 => Some(Payload::Observed)),
                    case!(Exchanged => 1 => None),
                    case!(Uncommitted => 2 => Some(Payload::Observed)),
                ],
            ),
            (
                Identity::AtomicTryExchangeOutcome,
                "AtomicTryExchangeOutcome",
                Attempt::Decisive,
                Observation::NonObserving,
                vec![
                    case!(Mismatched => 0 => Some(Payload::Proposed)),
                    case!(Exchanged => 1 => Some(Payload::Displaced)),
                ],
            ),
            (
                Identity::AtomicTryExchangeOnceOutcome,
                "AtomicTryExchangeOnceOutcome",
                Attempt::SingleAttempt,
                Observation::NonObserving,
                vec![
                    case!(Mismatched => 0 => Some(Payload::Proposed)),
                    case!(Exchanged => 1 => Some(Payload::Displaced)),
                    case!(Uncommitted => 2 => Some(Payload::Proposed)),
                ],
            ),
        ];

        for (index, (identity, name, attempt, observation, cases)) in
            expected.into_iter().enumerate()
        {
            assert_eq!(Identity::ALL[index], identity);
            assert_eq!(identity.name(), name);
            assert_eq!(identity.type_parameters(), ["T"]);
            assert_eq!(identity.attempt(), attempt);
            assert_eq!(identity.observation(), observation);
            assert_eq!(Identity::from_axes(attempt, observation), identity);
            assert_eq!(identity.cases(), cases);
        }

        assert_eq!(Case::Mismatched.name(), "Mismatched");
        assert_eq!(Case::Exchanged.name(), "Exchanged");
        assert_eq!(Case::Uncommitted.name(), "Uncommitted");
        assert_eq!(Payload::Observed.field_name(), "observed");
        assert_eq!(Payload::Proposed.field_name(), "proposed");
        assert_eq!(Payload::Displaced.field_name(), "displaced");
        assert!(
            [Payload::Observed, Payload::Proposed, Payload::Displaced]
                .into_iter()
                .all(|payload| payload.type_parameter_name() == "T")
        );
    }
}
