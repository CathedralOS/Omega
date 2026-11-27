//! An indexed domain must form for arbitrary const inputs, even when every
//! use supplies the same literal. Only concrete applications publish domain
//! predicates, whose sole value parameter in Psi is `self`.
use crate::read::refuse_at;

use super::contracts::{ClauseScope, ResultTerm};
use super::{PredicateDefinition, Program};
use crate::Unserved;
use crate::source_state::data::TypeParameterKind;
use crate::source_state::domain::{DomainDefinition, ProofFact};
use crate::source_state::references::collect_expression_references;
use semantic_vocabulary::{MachineId, ScalarQualificationSetId, ScalarTerm, ValueId};
use terminal_check_tables::Checked;
use terminal_psi::{MathematicalPredicateBody, ValueDeclaration};

impl Program<'_> {
    /// A family whose clauses read a const needs a separate check with that
    /// const in scope. Keep the ordinary domain reader for all other headers.
    fn has_bound_scalar_predicate(&self, id: MachineId, domain: &DomainDefinition) -> bool {
        let typed = self.typed;
        let binders = typed.domain_type_parameters(domain);
        if binders.is_empty()
            || !binders.iter().all(|parameter| {
                matches!(parameter.kind, TypeParameterKind::Const { .. })
                    && parameter.bounds == Default::default()
            })
            || !crate::source_state::domain::has_direct_index_telescope(typed, domain)
            || domain.access_policy.is_some()
            || !domain.establishment_routes.is_empty()
            || !domain.content_projections.is_empty()
            || domain.cleanup_selection.is_some()
            || !domain.operators.is_empty()
            || domain.alias.is_some()
            || domain.classification.is_some()
            || domain.semantic_roles != Default::default()
            || self.scalar_type(id, domain.target_type).is_err()
        {
            return false;
        }
        let facts = typed.proof_facts.span_or_empty(domain.facts);
        facts
            .iter()
            .all(|fact| matches!(fact, ProofFact::Expression(_)))
            && facts.iter().any(|fact| {
                let ProofFact::Expression(expression) = fact else {
                    return false;
                };
                let reads = collect_expression_references(typed, *expression);
                binders
                    .iter()
                    .any(|binder| reads.symbols.contains(&binder.symbol))
            })
    }

    pub(super) fn require_domain_predicate_form(
        &mut self,
        id: MachineId,
        domain: &DomainDefinition,
    ) -> bool {
        if self.generic_templates.contains(&domain.symbol) {
            return true;
        }
        if self.has_bound_scalar_predicate(id, domain) {
            self.generic_templates.push(domain.symbol);
            return true;
        }
        false
    }

    pub(crate) fn form_domain_predicate(&mut self, domain: &DomainDefinition) -> Checked<()> {
        let id = MachineId::new(1).expect("one is nonzero");
        let invalid = |what| refuse_at(id, Unserved::Declaration(what));
        let typed = self.typed;
        self.served(id, domain.symbol)?;
        if !self.has_bound_scalar_predicate(id, domain) {
            return Err(invalid(
                "domain predicate without a supported scalar parameter telescope",
            ));
        }
        let binders = typed.domain_type_parameters(domain);
        let mut bound = Vec::with_capacity(1 + binders.len());
        let mut parameters = Vec::with_capacity(bound.capacity());
        let mut input = |symbol, scalar_type| -> Checked<()> {
            if !typed.symbols.symbols().nodes().is_valid(symbol)
                || typed.symbols.get(symbol).parent != domain.symbol
                || bound.iter().any(|(existing, _)| *existing == symbol)
            {
                return Err(invalid(
                    "domain predicate input without a unique declaration-owned symbol",
                ));
            }
            let value = ValueId::new(parameters.len() as u64 + 1)
                .ok_or_else(|| invalid("too many domain predicate parameters"))?;
            bound.push((symbol, ScalarTerm::value(value, scalar_type)));
            parameters.push(ValueDeclaration {
                id: value,
                scalar_type,
                qualifications: ScalarQualificationSetId::ZERO,
            });
            Ok(())
        };
        input(
            domain.subject_symbol,
            self.scalar_type(id, domain.target_type)?,
        )?;
        for binder in binders {
            let TypeParameterKind::Const { type_reference } = binder.kind else {
                return Err(invalid("domain predicate index without a const carrier"));
            };
            input(binder.symbol, self.scalar_type(id, type_reference)?)?;
            self.generic_binders.push(binder.symbol);
        }
        self.admitted_declarations.include(domain.symbol);
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &bound,
            owner: domain.symbol,
            result: ResultTerm::Absent("result in a domain predicate"),
            subject: None,
            formal_fields: false,
            logical: None,
        };
        let mut clauses = Vec::new();
        for fact in typed.proof_facts.span_or_empty(domain.facts) {
            let expression = self.fact(id, fact)?;
            clauses.push(self.clause(id, &scope, expression).map_err(|refusal| {
                self.read_failure_at(refusal, domain.symbol, "the domain predicate")
            })?);
        }
        let predicate = crate::canonical::conjunction(clauses, id)?;
        self.predicate_definitions.push(Some(PredicateDefinition {
            symbol: domain.symbol,
            diagnostic: id,
            parameters,
            type_parameters: Vec::new(),
            body: MathematicalPredicateBody::Proposition(predicate),
        }));
        Ok(())
    }
}
