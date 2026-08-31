//! Compiler-owned atomic memory-order vocabulary and operation legality.
//!
//! These names are semantic commitments, not arbitrary identifiers that an
//! atomic desugar may discard. Target lowering may implement a request with a
//! stronger instruction, but source admission first rejects orderings that the
//! operation cannot mean.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryOrdering {
    NoOrdering,
    Receive,
    Publish,
    ReceivePublish,
    GlobalOrder,
}

/// The operation-specific ordering commitment attached to an atomic source
/// expression. This survives compiler-authored carrier lowering so later
/// phases do not have to rediscover semantics from an expression shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicOrderingPlan {
    Load(MemoryOrdering),
    Store(MemoryOrdering),
    ReadModifyWrite(MemoryOrdering),
    Swap(MemoryOrdering),
    CompareExchange {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
    /// One observing compare-exchange attempt. Unlike decisive
    /// compare-exchange, this operation may report an uncommitted attempt.
    CompareExchangeOnce {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

/// The observing compare-exchange operation whose result shape is retained by
/// a checked placed-field contract.
///
/// This identifies source semantics only. It does not authorize an atomic
/// attempt or describe a target retry strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicObservingCompareExchangeOperation {
    Decisive,
    SingleAttempt,
}

/// Closed result-shape identity for one observing compare-exchange operation.
///
/// `Observed` means that the failure arm carries the exact resident type. The
/// carrier that retains this shape must separately prove that resident is
/// copyable; this enum carries no value custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicObservingCompareExchangeResultShape {
    ExchangedOrMismatchedObserved,
    ExchangedOrMismatchedOrUncommittedObserved,
}

/// Checked result custody carried by one compiler-authored atomic expression.
///
/// The legacy scalar form covers atomic operations whose current checked node
/// writes one primitive observation. The distinct single-attempt form retains
/// the complete three-arm public outcome identity before execution support is
/// admitted. Naming this carrier grants no atomic, provider, Terminal, or
/// native authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicExpressionResultCustody {
    Scalar,
    ObservingCompareExchangeOnce(AtomicCompareExchangeOnceResultCustody),
}

/// Exact observing single-attempt result identity retained through checked
/// expression lowering.
///
/// The three fields are deliberately redundant. Every phase rechecks their
/// canonical agreement so a decisive operation, two-arm shape, or sibling
/// outcome identity cannot substitute under an unchanged ordering plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtomicCompareExchangeOnceResultCustody {
    pub operation: AtomicObservingCompareExchangeOperation,
    pub result_shape: AtomicObservingCompareExchangeResultShape,
    pub outcome_identity: AtomicCompareExchangeOutcomeIdentity,
}

/// Whether a compare-exchange result describes a decisive operation or one
/// single attempt.
///
/// This is an identity axis only. In particular, naming `SingleAttempt` does
/// not authorize an atomic attempt or select an implementation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicCompareExchangeAttemptAxis {
    Decisive,
    SingleAttempt,
}

/// Whether compare-exchange failure exposes the resident value.
///
/// This is an identity axis only. Custody, copyability, and access authority
/// remain obligations of the carrier that uses an outcome identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicCompareExchangeObservationAxis {
    Observing,
    NonObserving,
}

/// One canonical case in the flat public compare-exchange outcome family.
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

/// Semantic role and exact field name of a compare-exchange outcome payload.
///
/// Every payload has the outcome's sole type parameter `T`; `Key` and any
/// selected encoding law are deliberately absent from the runtime result.
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

/// Canonical tag and optional payload for one flat outcome case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtomicCompareExchangeOutcomeCaseSchema {
    pub case: AtomicCompareExchangeOutcomeCase,
    pub tag: u8,
    pub payload: Option<AtomicCompareExchangeOutcomePayload>,
}

const OBSERVING_DECISIVE_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 2] = [
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Mismatched,
        tag: 0,
        payload: Some(AtomicCompareExchangeOutcomePayload::Observed),
    },
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Exchanged,
        tag: 1,
        payload: None,
    },
];

const OBSERVING_SINGLE_ATTEMPT_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 3] = [
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Mismatched,
        tag: 0,
        payload: Some(AtomicCompareExchangeOutcomePayload::Observed),
    },
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Exchanged,
        tag: 1,
        payload: None,
    },
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Uncommitted,
        tag: 2,
        payload: Some(AtomicCompareExchangeOutcomePayload::Observed),
    },
];

const NON_OBSERVING_DECISIVE_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 2] = [
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Mismatched,
        tag: 0,
        payload: Some(AtomicCompareExchangeOutcomePayload::Proposed),
    },
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Exchanged,
        tag: 1,
        payload: Some(AtomicCompareExchangeOutcomePayload::Displaced),
    },
];

const NON_OBSERVING_SINGLE_ATTEMPT_CASES: [AtomicCompareExchangeOutcomeCaseSchema; 3] = [
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Mismatched,
        tag: 0,
        payload: Some(AtomicCompareExchangeOutcomePayload::Proposed),
    },
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Exchanged,
        tag: 1,
        payload: Some(AtomicCompareExchangeOutcomePayload::Displaced),
    },
    AtomicCompareExchangeOutcomeCaseSchema {
        case: AtomicCompareExchangeOutcomeCase::Uncommitted,
        tag: 2,
        payload: Some(AtomicCompareExchangeOutcomePayload::Proposed),
    },
];

const OUTCOME_TYPE_PARAMETERS: [&str; 1] = ["T"];

/// Compiler-owned nominal identity for one of the four flat public atomic
/// compare-exchange outcome types.
///
/// The identity and its schema are descriptive compiler vocabulary. They do
/// not prove that an operation occurred, grant access or value custody, select
/// a provider, authorize lowering, or prescribe retry behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// Exact public generic parameter list. The non-observing operation's
    /// comparison `Key` is intentionally not part of any outcome identity.
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

    /// The canonical failure-first case order. Tags are explicit so a caller
    /// never has to derive public schema from Rust enum discriminants.
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
    /// Recover the full public outcome identity represented by the legacy
    /// observing-only checked-contract shape.
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
    pub const CANONICAL: Self = Self {
        operation: AtomicObservingCompareExchangeOperation::SingleAttempt,
        result_shape:
            AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedOrUncommittedObserved,
        outcome_identity: AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome,
    };

    pub const fn is_canonical(self) -> bool {
        matches!(
            self,
            Self {
                operation: AtomicObservingCompareExchangeOperation::SingleAttempt,
                result_shape: AtomicObservingCompareExchangeResultShape::
                    ExchangedOrMismatchedOrUncommittedObserved,
                outcome_identity:
                    AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome,
            }
        )
    }
}

impl AtomicExpressionResultCustody {
    /// Recheck result identity against the independently retained operation
    /// ordering axis.
    pub const fn is_valid_for(self, ordering: AtomicOrderingPlan) -> bool {
        match (self, ordering) {
            (Self::Scalar, AtomicOrderingPlan::CompareExchangeOnce { .. }) => false,
            (Self::Scalar, _) => true,
            (
                Self::ObservingCompareExchangeOnce(custody),
                AtomicOrderingPlan::CompareExchangeOnce { .. },
            ) => custody.is_canonical(),
            (Self::ObservingCompareExchangeOnce(_), _) => false,
        }
    }

    /// Whether this custody form requires a compiler-authored result
    /// destination in the expression table.
    pub const fn requires_result_destination(self) -> bool {
        matches!(self, Self::ObservingCompareExchangeOnce(_))
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

    /// Compare-exchange failure performs only a load: it cannot publish, and
    /// it cannot demand synchronization stronger than the success ordering.
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
    fn single_attempt_expression_custody_rejects_axis_substitution() {
        use super::{
            AtomicCompareExchangeOnceResultCustody as OnceCustody,
            AtomicCompareExchangeOutcomeIdentity as Outcome,
            AtomicExpressionResultCustody as ResultCustody,
            AtomicObservingCompareExchangeOperation as Operation,
            AtomicObservingCompareExchangeResultShape as Shape, AtomicOrderingPlan,
        };

        let once_ordering = AtomicOrderingPlan::CompareExchangeOnce {
            success: O::ReceivePublish,
            failure: O::Receive,
        };
        assert!(
            ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL)
                .is_valid_for(once_ordering)
        );
        assert!(!ResultCustody::Scalar.is_valid_for(once_ordering));

        let decisive_ordering = AtomicOrderingPlan::CompareExchange {
            success: O::ReceivePublish,
            failure: O::Receive,
        };
        assert!(ResultCustody::Scalar.is_valid_for(decisive_ordering));
        assert!(
            !ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL)
                .is_valid_for(decisive_ordering)
        );

        for drifted in [
            OnceCustody {
                operation: Operation::Decisive,
                ..OnceCustody::CANONICAL
            },
            OnceCustody {
                result_shape: Shape::ExchangedOrMismatchedObserved,
                ..OnceCustody::CANONICAL
            },
            OnceCustody {
                outcome_identity: Outcome::AtomicCompareExchangeOutcome,
                ..OnceCustody::CANONICAL
            },
            OnceCustody {
                outcome_identity: Outcome::AtomicTryExchangeOnceOutcome,
                ..OnceCustody::CANONICAL
            },
        ] {
            assert!(
                !ResultCustody::ObservingCompareExchangeOnce(drifted).is_valid_for(once_ordering)
            );
        }
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

        let expected = [
            (
                Identity::AtomicCompareExchangeOutcome,
                "AtomicCompareExchangeOutcome",
                Attempt::Decisive,
                Observation::Observing,
                vec![
                    CaseSchema {
                        case: Case::Mismatched,
                        tag: 0,
                        payload: Some(Payload::Observed),
                    },
                    CaseSchema {
                        case: Case::Exchanged,
                        tag: 1,
                        payload: None,
                    },
                ],
            ),
            (
                Identity::AtomicCompareExchangeOnceOutcome,
                "AtomicCompareExchangeOnceOutcome",
                Attempt::SingleAttempt,
                Observation::Observing,
                vec![
                    CaseSchema {
                        case: Case::Mismatched,
                        tag: 0,
                        payload: Some(Payload::Observed),
                    },
                    CaseSchema {
                        case: Case::Exchanged,
                        tag: 1,
                        payload: None,
                    },
                    CaseSchema {
                        case: Case::Uncommitted,
                        tag: 2,
                        payload: Some(Payload::Observed),
                    },
                ],
            ),
            (
                Identity::AtomicTryExchangeOutcome,
                "AtomicTryExchangeOutcome",
                Attempt::Decisive,
                Observation::NonObserving,
                vec![
                    CaseSchema {
                        case: Case::Mismatched,
                        tag: 0,
                        payload: Some(Payload::Proposed),
                    },
                    CaseSchema {
                        case: Case::Exchanged,
                        tag: 1,
                        payload: Some(Payload::Displaced),
                    },
                ],
            ),
            (
                Identity::AtomicTryExchangeOnceOutcome,
                "AtomicTryExchangeOnceOutcome",
                Attempt::SingleAttempt,
                Observation::NonObserving,
                vec![
                    CaseSchema {
                        case: Case::Mismatched,
                        tag: 0,
                        payload: Some(Payload::Proposed),
                    },
                    CaseSchema {
                        case: Case::Exchanged,
                        tag: 1,
                        payload: Some(Payload::Displaced),
                    },
                    CaseSchema {
                        case: Case::Uncommitted,
                        tag: 2,
                        payload: Some(Payload::Proposed),
                    },
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
