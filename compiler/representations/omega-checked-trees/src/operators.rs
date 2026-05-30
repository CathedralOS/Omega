use crate::CheckedValueOrigin;
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
    pub origin: CheckedValueOrigin,
    pub spelling: OperatorSpelling,
    pub selected_operator_symbol: SymbolHandle,
    pub candidates: HandleSpan<CheckedOperatorCandidateFact>,
    pub candidate_count: usize,
    pub status: CheckedOperatorResolutionStatus,
}

impl Default for CheckedOperatorUseFact {
    fn default() -> Self {
        Self {
            expression: ExpressionHandle::invalid(),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: HandleSpan::empty(),
            candidate_count: 0,
            status: CheckedOperatorResolutionStatus::Missing,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedOperatorCandidateFact {
    pub operator_symbol: SymbolHandle,
    pub domain_symbol: SymbolHandle,
}

impl CheckedOperatorCandidateFact {
    pub const fn root(operator_symbol: SymbolHandle) -> Self {
        Self {
            operator_symbol,
            domain_symbol: SymbolHandle::invalid(),
        }
    }

    pub const fn domain(operator_symbol: SymbolHandle, domain_symbol: SymbolHandle) -> Self {
        Self {
            operator_symbol,
            domain_symbol,
        }
    }

    pub const fn is_domain_owned(self) -> bool {
        self.domain_symbol.is_valid()
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
    pub candidates: Arena<CheckedOperatorCandidateFact>,
}

impl CheckedOperatorFacts {
    pub fn with_roots(
        uses: Arena<CheckedOperatorUseFact>,
        candidates: Arena<CheckedOperatorCandidateFact>,
    ) -> Self {
        Self { uses, candidates }
    }

    pub fn expression_use(&self, expression: ExpressionHandle) -> Option<&CheckedOperatorUseFact> {
        self.uses.iter().find_map(|(_, operator_use)| {
            (operator_use.expression == expression).then_some(operator_use)
        })
    }

    pub fn expression_use_in_origin(
        &self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> Option<&CheckedOperatorUseFact> {
        self.uses.iter().find_map(|(_, operator_use)| {
            (operator_use.expression == expression && operator_use.origin == origin)
                .then_some(operator_use)
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

    pub fn candidates(
        &self,
        operator_use: &CheckedOperatorUseFact,
    ) -> &[CheckedOperatorCandidateFact] {
        self.candidates.span_or_empty(operator_use.candidates)
    }

    pub fn candidate_symbols(
        &self,
        operator_use: &CheckedOperatorUseFact,
    ) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.candidates(operator_use)
            .iter()
            .map(|candidate| candidate.operator_symbol)
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
        let mut candidates = Arena::with_capacity(3);
        let resolved_candidates = candidates.insert_many([CheckedOperatorCandidateFact::root(
            SymbolHandle::from_arena_index(2),
        )]);
        let ambiguous_candidates = candidates.insert_many([
            CheckedOperatorCandidateFact::root(SymbolHandle::from_arena_index(4)),
            CheckedOperatorCandidateFact::domain(
                SymbolHandle::from_arena_index(5),
                SymbolHandle::from_arena_index(6),
            ),
        ]);

        let mut uses = Arena::with_capacity(2);
        let expression = ExpressionHandle::from_arena_index(1);
        uses.append(CheckedOperatorUseFact {
            expression,
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            selected_operator_symbol: SymbolHandle::from_arena_index(2),
            candidates: resolved_candidates,
            candidate_count: 1,
            status: CheckedOperatorResolutionStatus::Resolved,
        });
        uses.append(CheckedOperatorUseFact {
            expression: ExpressionHandle::from_arena_index(3),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Range,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: ambiguous_candidates,
            candidate_count: 2,
            status: CheckedOperatorResolutionStatus::Ambiguous,
        });

        let facts = CheckedOperatorFacts::with_roots(uses.clone(), candidates.clone());

        assert_eq!(facts.uses, uses);
        assert_eq!(facts.candidates, candidates);
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
        assert_eq!(facts.candidates(ambiguous_use).len(), 2);
        assert_eq!(
            facts.candidate_symbols(ambiguous_use).collect::<Vec<_>>(),
            vec![
                SymbolHandle::from_arena_index(4),
                SymbolHandle::from_arena_index(5),
            ]
        );
        assert!(facts.candidates(ambiguous_use)[1].is_domain_owned());
    }
}
