use arena::Handle;
use symbols::SymbolHandle;

use crate::{
    CheckedOperatorUseFact, CheckedValueOrigin, CrashRouteBucket, FlowOperatorInvocationFact,
};

/// One selected operator's crash obligations at its exact captured invocation.
///
/// Operator uses have their own source occurrence identity, including individual
/// Match arms. They are not machine calls and must not borrow a call ordinal.
/// Published buckets retain the selected requirement in formal coordinates;
/// surviving buckets use proven caller-entry values or conservative Truth.
/// An empty surviving set records a proved discharge, not a missing analysis.
/// These checked-source rows are not yet portable Terminal crash evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCrashOperatorSite {
    pub operator_use: Handle<CheckedOperatorUseFact>,
    pub invocation: Handle<FlowOperatorInvocationFact>,
    pub selected_operator: SymbolHandle,
    pub published: Vec<CrashRouteBucket>,
    pub surviving: Vec<CrashRouteBucket>,
}

impl CheckedCrashOperatorSite {
    pub fn location(&self, facts: &crate::CheckFacts) -> OperatorSiteLocation {
        // `facts/operator_crashes.rs` admits only `StateStatement` origins into
        // `checked_operators`, so the caller state and statement ordinal are
        // always recoverable from the retained use handle.
        let origin = facts.operators.uses.get(self.operator_use).origin;
        let (_machine_symbol, state_symbol, statement_index) = match origin {
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            } => (machine_symbol, state_symbol, statement_index),
            _ => unreachable!("operator crash site must have StateStatement origin"),
        };
        OperatorSiteLocation {
            state: state_symbol,
            statement_ordinal: u32::try_from(statement_index).expect("statement ordinal fits u32"),
        }
    }

    pub fn selected_operator(&self) -> SymbolHandle {
        self.selected_operator
    }

    pub fn published(&self) -> &[CrashRouteBucket] {
        &self.published
    }

    pub fn surviving(&self) -> &[CrashRouteBucket] {
        &self.surviving
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorSiteLocation {
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
}

impl std::cmp::Ord for OperatorSiteLocation {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.state.arena_index(),
            self.state.generation(),
            self.statement_ordinal,
        )
            .cmp(&(
                other.state.arena_index(),
                other.state.generation(),
                other.statement_ordinal,
            ))
    }
}

impl std::cmp::PartialOrd for OperatorSiteLocation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl OperatorSiteLocation {
    pub fn state(&self) -> SymbolHandle {
        self.state
    }

    pub fn statement_ordinal(&self) -> u32 {
        self.statement_ordinal
    }
}
