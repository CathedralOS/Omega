//! Scalar state plans: a state's scalar terminator and its successors, and
//! every state's ordered guarded exits. Unit planning, composed admission and
//! scalar completion read them.

use crate::checked_trees::flow::terminal::CheckedStructuralControlSuccessorPlan;
use symbols::SymbolHandle;

/// Every state's ordered guarded exits:
/// `tails` holds one row per state whose authored transition guards an exit,
/// and its `arms` span `exits`.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedGuardedExitPlans {
    pub exits: arena::Arena<CheckedScalarGuardedExit>,
    pub tails: Vec<CheckedScalarGuardedTail>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedScalarGuardedTail {
    pub state: SymbolHandle,
    pub arms: arena::HandleSpan<CheckedScalarGuardedExit>,
    pub fallback: Option<CheckedScalarBranchDestination>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedScalarGuardedExit {
    pub guard_statement_ordinal: u32,
    pub destination: CheckedScalarBranchDestination,
}

impl Default for CheckedScalarGuardedExit {
    fn default() -> Self {
        Self {
            guard_statement_ordinal: 0,
            destination: CheckedScalarBranchDestination::Return {
                statement_ordinal: 0,
                is_continuation: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedScalarStateTerminator {
    /// Guards are tested in authored order. No fallback requires independently
    /// checked exhaustive coverage; the final guarded row remains explicit.
    Guarded {
        arms: arena::HandleSpan<CheckedScalarGuardedExit>,
        fallback: Option<CheckedScalarBranchDestination>,
    },
    Return {
        statement_ordinal: u32,
    },
    Crash {
        statement_ordinal: u32,
    },
    Jump(CheckedStructuralControlSuccessorPlan),
    Conditional {
        guard_statement_ordinal: u32,
        when_true: CheckedScalarBranchDestination,
        when_false: CheckedScalarBranchDestination,
    },
}

/// A selected scalar arm transfers into a state, returns, or crashes without a successor.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedScalarBranchDestination {
    /// An exact checked module invocation with no normal result or successor.
    /// Its ordinary entry obligations and argument evaluation remain live.
    InvokeNoNormal {
        statement_ordinal: u32,
        is_continuation: bool,
    },
    Jump(CheckedStructuralControlSuccessorPlan),
    /// The exact standalone source crash retains its cause and site custody.
    Crash {
        statement_ordinal: u32,
    },
    Return {
        statement_ordinal: u32,
        /// The false sibling of a combined transition has its own value role.
        is_continuation: bool,
    },
}

/// A returning branch can use a local before cleanup may release it. Keep
/// that branch's evaluated guards and final value together during planning.
/// Guarded prefixes borrow the retained roster rather than copying it per arm.
pub struct ScalarReturnPath<'a> {
    pub destination: CheckedScalarBranchDestination,
    guarded_prefix: &'a [CheckedScalarGuardedExit],
    conditional_guard: Option<u32>,
}

impl ScalarReturnPath<'_> {
    pub fn guard_ordinals(&self) -> impl Iterator<Item = u32> + '_ {
        self.conditional_guard.into_iter().chain(
            self.guarded_prefix
                .iter()
                .map(|arm| arm.guard_statement_ordinal),
        )
    }
}

impl CheckedGuardedExitPlans {
    pub fn returning_exits(
        &self,
        arms: arena::HandleSpan<CheckedScalarGuardedExit>,
        fallback: Option<&CheckedScalarBranchDestination>,
    ) -> Result<&[CheckedScalarGuardedExit], &'static str> {
        let arms = self
            .exits
            .span(arms)
            .ok_or("ordered scalar guard roster is stale")?;
        if arms.is_empty() {
            return Err("ordered scalar guard roster is empty");
        }
        if arms
            .iter()
            .map(|arm| &arm.destination)
            .chain(fallback)
            .any(|destination| {
                !matches!(
                    destination,
                    CheckedScalarBranchDestination::Return {
                        is_continuation: false,
                        ..
                    }
                )
            })
        {
            return Err("ordered scalar exit transfers or crashes instead of returning");
        }
        Ok(arms)
    }
}

impl CheckedScalarStateTerminator {
    /// Follow the control plan's returning branches in execution order.
    /// Authored cleanup rows may also name unreachable suffixes; this roster
    /// comes from executable control and preserves each guard's evaluation.
    pub fn returning_paths<'a>(
        &self,
        guarded: &'a CheckedGuardedExitPlans,
    ) -> Result<Vec<ScalarReturnPath<'a>>, &'static str> {
        let paths = match self {
            Self::Return { statement_ordinal } => vec![ScalarReturnPath {
                destination: CheckedScalarBranchDestination::Return {
                    statement_ordinal: *statement_ordinal,
                    is_continuation: false,
                },
                guarded_prefix: &[],
                conditional_guard: None,
            }],
            Self::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => [when_true, when_false]
                .into_iter()
                .map(|destination| ScalarReturnPath {
                    destination: destination.clone(),
                    guarded_prefix: &[],
                    conditional_guard: Some(*guard_statement_ordinal),
                })
                .collect(),
            Self::Guarded { arms, fallback } => {
                let arms = guarded.returning_exits(*arms, fallback.as_ref())?;
                arms.iter()
                    .enumerate()
                    .map(|(index, arm)| ScalarReturnPath {
                        destination: arm.destination.clone(),
                        guarded_prefix: &arms[..=index],
                        conditional_guard: None,
                    })
                    .chain(fallback.iter().map(|destination| ScalarReturnPath {
                        destination: destination.clone(),
                        guarded_prefix: arms,
                        conditional_guard: None,
                    }))
                    .collect()
            }
            _ => return Err("scalar completion has no normal return destination"),
        };
        if paths.iter().any(|path| {
            !matches!(
                path.destination,
                CheckedScalarBranchDestination::Return { .. }
            )
        }) {
            return Err("scalar completion has a nonreturning destination");
        }
        Ok(paths)
    }
}
