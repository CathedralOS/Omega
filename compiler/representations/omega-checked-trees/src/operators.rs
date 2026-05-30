use omega_core::arena::{Arena, HandleSpan};
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedOperatorResolutionStatus {
    #[default]
    Missing,
    Resolved,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedOperatorUseFact {
    pub expression: ExpressionHandle,
    pub spelling: OperatorSpelling,
    pub selected_operator_symbol: SymbolHandle,
    pub candidate_symbols: HandleSpan<SymbolHandle>,
    pub candidate_count: usize,
    pub status: CheckedOperatorResolutionStatus,
}

impl Default for CheckedOperatorUseFact {
    fn default() -> Self {
        Self {
            expression: ExpressionHandle::invalid(),
            spelling: OperatorSpelling::Index,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidate_symbols: HandleSpan::empty(),
            candidate_count: 0,
            status: CheckedOperatorResolutionStatus::Missing,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedOperatorResolutionSummary {
    pub resolved: usize,
    pub missing: usize,
    pub ambiguous: usize,
}

impl CheckedOperatorResolutionSummary {
    pub const fn all_resolved(self) -> bool {
        self.missing == 0 && self.ambiguous == 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedOperatorFacts {
    pub uses: Arena<CheckedOperatorUseFact>,
    pub candidate_symbols: Arena<SymbolHandle>,
}

impl CheckedOperatorFacts {
    pub fn with_roots(
        uses: Arena<CheckedOperatorUseFact>,
        candidate_symbols: Arena<SymbolHandle>,
    ) -> Self {
        Self {
            uses,
            candidate_symbols,
        }
    }

    pub fn expression_use(&self, expression: ExpressionHandle) -> Option<&CheckedOperatorUseFact> {
        self.uses.iter().find_map(|(_, operator_use)| {
            (operator_use.expression == expression).then_some(operator_use)
        })
    }

    pub fn uses_with_status(
        &self,
        status: CheckedOperatorResolutionStatus,
    ) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses.iter().filter_map(move |(_, operator_use)| {
            (operator_use.status == status).then_some(operator_use)
        })
    }

    pub fn resolved_uses(&self) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses_with_status(CheckedOperatorResolutionStatus::Resolved)
    }

    pub fn missing_uses(&self) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses_with_status(CheckedOperatorResolutionStatus::Missing)
    }

    pub fn ambiguous_uses(&self) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses_with_status(CheckedOperatorResolutionStatus::Ambiguous)
    }

    pub fn candidate_symbols(&self, operator_use: &CheckedOperatorUseFact) -> &[SymbolHandle] {
        self.candidate_symbols
            .span_or_empty(operator_use.candidate_symbols)
    }

    pub fn resolution_summary(&self) -> CheckedOperatorResolutionSummary {
        let mut summary = CheckedOperatorResolutionSummary::default();
        for (_, operator_use) in self.uses.iter() {
            match operator_use.status {
                CheckedOperatorResolutionStatus::Resolved => {
                    summary.resolved = summary.resolved.saturating_add(1);
                }
                CheckedOperatorResolutionStatus::Missing => {
                    summary.missing = summary.missing.saturating_add(1);
                }
                CheckedOperatorResolutionStatus::Ambiguous => {
                    summary.ambiguous = summary.ambiguous.saturating_add(1);
                }
            }
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_operator_facts_constructor_keeps_use_root_explicit() {
        let mut candidate_symbols = Arena::with_capacity(3);
        let resolved_candidates =
            candidate_symbols.insert_many([SymbolHandle::from_arena_index(2)]);
        let ambiguous_candidates = candidate_symbols.insert_many([
            SymbolHandle::from_arena_index(4),
            SymbolHandle::from_arena_index(5),
        ]);

        let mut uses = Arena::with_capacity(2);
        let expression = ExpressionHandle::from_arena_index(1);
        uses.append(CheckedOperatorUseFact {
            expression,
            spelling: OperatorSpelling::Index,
            selected_operator_symbol: SymbolHandle::from_arena_index(2),
            candidate_symbols: resolved_candidates,
            candidate_count: 1,
            status: CheckedOperatorResolutionStatus::Resolved,
        });
        uses.append(CheckedOperatorUseFact {
            expression: ExpressionHandle::from_arena_index(3),
            spelling: OperatorSpelling::Range,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidate_symbols: ambiguous_candidates,
            candidate_count: 2,
            status: CheckedOperatorResolutionStatus::Ambiguous,
        });

        let facts = CheckedOperatorFacts::with_roots(uses.clone(), candidate_symbols.clone());

        assert_eq!(facts.uses, uses);
        assert_eq!(facts.candidate_symbols, candidate_symbols);
        assert_eq!(
            facts
                .expression_use(expression)
                .map(|operator_use| operator_use.status),
            Some(CheckedOperatorResolutionStatus::Resolved)
        );
        assert_eq!(
            facts.resolution_summary(),
            CheckedOperatorResolutionSummary {
                resolved: 1,
                missing: 0,
                ambiguous: 1,
            }
        );
        assert!(!facts.resolution_summary().all_resolved());
        assert_eq!(facts.resolved_uses().count(), 1);
        assert_eq!(facts.ambiguous_uses().count(), 1);
        assert_eq!(facts.missing_uses().count(), 0);
        let ambiguous_use = facts.ambiguous_uses().next().expect("ambiguous use");
        assert_eq!(facts.candidate_symbols(ambiguous_use).len(), 2);
    }
}
