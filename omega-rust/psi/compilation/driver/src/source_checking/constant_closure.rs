//! Discharge retained constant obligations against their exact value applications.
//!
//! Provider and layout owners supply completed values and exact provider edges.
//! Psi owns application discovery, proof discharge, complete coverage and the final
//! checked facts. A selected application borrows this closure exclusively: its
//! row positions cannot be applied to another program or an unrelated accumulator.

use checked_trees::{CheckFacts, CheckedTrees, IntegerRangeModel};
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

mod applications;
use applications::ProofApplications;

/// Invocation-local proof closure over a private, evaluated copy of a checked program.
/// Provider selection stays with the caller; only complete coverage can publish.
pub struct ConstantProofClosure<'source> {
    source: &'source CheckedTrees,
    applications: ProofApplications<'source>,
    program: TypedTrees,
    facts: CheckFacts,
    value_scope: String,
    predicates_closed: Vec<bool>,
    ranges_closed: Vec<bool>,
    recipes_closed: Vec<bool>,
}

/// Selected obligations retain their original positions and their sole writer.
/// This is temporary checking work, not another published program representation.
pub struct ConstantProofApplication<'closure, 'source> {
    closure: &'closure mut ConstantProofClosure<'source>,
    target_scope: &'closure str,
    predicates: Vec<usize>,
    ranges: Vec<usize>,
    recipes: Vec<usize>,
}

impl<'source> ConstantProofClosure<'source> {
    /// Retain the shared judgment unchanged while closing a private value view.
    pub fn new(source: &'source CheckedTrees, program: TypedTrees, value_scope: &str) -> Self {
        let mut facts = source.facts.clone();
        facts.constant_predicate_obligations.clear();
        facts.integer_range_obligations.clear();
        facts.constant_recipe_obligations.clear();
        Self {
            source,
            applications: ProofApplications::new(source),
            program,
            facts,
            value_scope: value_scope.to_owned(),
            predicates_closed: vec![false; source.facts.constant_predicate_obligations.len()],
            ranges_closed: vec![false; source.facts.integer_range_obligations.len()],
            recipes_closed: vec![false; source.facts.constant_recipe_obligations.len()],
        }
    }

    /// Selected scope first, then every distinct authored proof scope. Target
    /// identity validation stays with the caller before it requests providers.
    pub fn target_scopes(&self) -> Vec<&str> {
        self.applications.scopes(&self.value_scope)
    }

    /// Conservative source demand discovery does not select or authorize a
    /// provider. A false result avoids unrelated supplemental provider work.
    pub fn needs_application(&self, target_scope: &str) -> bool {
        self.applications
            .needs_scope(target_scope, &self.value_scope)
    }

    /// Select obligations by their recorded caller or declaration owner. Ground
    /// declaration models also close in the primary value capsule, whose retained
    /// metadata describes those declarations even without an executing caller.
    pub fn application<'closure>(
        &'closure mut self,
        target_scope: &'closure str,
        providers: &[(SymbolHandle, SymbolHandle)],
    ) -> Option<ConstantProofApplication<'closure, 'source>> {
        let (machines, declarations) =
            self.applications
                .membership(target_scope, target_scope == self.value_scope, providers);
        let all_declarations = self.applications.declarations();
        let predicates: Vec<_> = self
            .source
            .facts
            .constant_predicate_obligations
            .iter()
            .enumerate()
            .filter(|(_, obligation)| machines.contains(&obligation.caller()))
            .map(|(position, _)| position)
            .collect();
        let ranges: Vec<_> = self
            .source
            .facts
            .integer_range_obligations
            .iter()
            .enumerate()
            .filter(|(_, obligation)| {
                machines.contains(&obligation.owner())
                    || (obligation.needs_address_model()
                        && (declarations.contains(&obligation.owner())
                            || (target_scope == self.value_scope
                                && all_declarations.contains(&obligation.owner()))))
            })
            .map(|(position, _)| position)
            .collect();
        let recipes: Vec<_> = self
            .source
            .facts
            .constant_recipe_obligations
            .iter()
            .enumerate()
            .filter(|(_, obligation)| {
                !obligation.owner().is_valid() || machines.contains(&obligation.owner())
            })
            .map(|(position, _)| position)
            .collect();
        if predicates.is_empty() && ranges.is_empty() && recipes.is_empty() {
            return None;
        }
        Some(ConstantProofApplication {
            closure: self,
            target_scope,
            predicates,
            ranges,
            recipes,
        })
    }

    /// Require complete coverage before publishing the closed checked program.
    /// Pending boundary clauses use the same Psi execution settlement as before;
    /// consumers never rebuild checking facts after this operation.
    pub fn finish(mut self) -> Result<Arc<CheckedTrees>, Vec<Diagnostic>> {
        if self.predicates_closed.contains(&false)
            || self.ranges_closed.contains(&false)
            || self.recipes_closed.contains(&false)
        {
            return Err(vec![Diagnostic::error(
                "retained proof obligation has no target application",
            )]);
        }
        self.facts
            .operators
            .decided_formations
            .sort_by_key(|expression| checked_trees::operators::handle_position(*expression));
        self.facts.operators.decided_formations.dedup();
        let receipts = &mut self.program.constant_recipe_closure.receipts;
        receipts.sort_by(|left, right| {
            left.target_scope
                .cmp(&right.target_scope)
                .then(
                    left.requirement
                        .arena_index()
                        .cmp(&right.requirement.arena_index()),
                )
                .then(left.plan_identity.cmp(&right.plan_identity))
        });
        receipts.dedup();
        let target_bodies = &mut self.program.constant_recipe_closure.target_bodies;
        target_bodies.sort_by_key(|row| {
            (
                row.target_scope.clone(),
                row.head.arena_index(),
                row.head.generation(),
                row.member.arena_index(),
                row.member.generation(),
            )
        });
        target_bodies.dedup();
        if !receipts.is_empty() {
            self.program.constant_recipe_closure.target_scope = self.value_scope;
        }
        let closed = CheckedTrees::with_roots(self.program, self.facts);
        let pending_boundary_clauses = closed
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines
            .iter()
            .any(|plan| {
                !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty()
            });
        let closed = if pending_boundary_clauses {
            typed_trees_to_checked_trees::settle_closed_constant_execution(closed)?
        } else {
            closed
        };
        Ok(Arc::new(closed))
    }
}

impl ConstantProofApplication<'_, '_> {
    /// Whether any selected range obligation consumes a platform address model.
    pub fn needs_address_model(&self) -> bool {
        self.ranges.iter().any(|position| {
            self.closure.source.facts.integer_range_obligations[*position].needs_address_model()
        })
    }

    /// Discharge predicates, recipes, then range formations in that order.
    /// A model refusal is retained as input until range checking reaches it:
    /// it must not hide an earlier predicate or recipe diagnostic.
    pub fn discharge(
        self,
        supplemental_program: Option<TypedTrees>,
        address_model: Result<Option<IntegerRangeModel>, Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        let program = supplemental_program
            .as_ref()
            .unwrap_or(&self.closure.program);
        for position in self.predicates {
            let mut obligation =
                self.closure.source.facts.constant_predicate_obligations[position].clone();
            checked_trees::ConstantPredicateObligation::discharge_all(
                std::iter::once(&mut obligation),
                program,
                self.target_scope,
            )?;
            self.closure
                .facts
                .constant_predicate_obligations
                .push(obligation);
            self.closure.predicates_closed[position] = true;
        }
        for position in self.recipes {
            let mut obligation =
                self.closure.source.facts.constant_recipe_obligations[position].clone();
            checked_trees::ConstantRecipeObligation::discharge_all(
                std::slice::from_mut(&mut obligation),
                program,
            )?;
            self.closure
                .facts
                .constant_recipe_obligations
                .push(obligation);
            self.closure.recipes_closed[position] = true;
        }
        let address_model = address_model?;
        for position in self.ranges {
            let mut obligation =
                self.closure.source.facts.integer_range_obligations[position].clone();
            obligation
                .discharge(program, self.target_scope, address_model.as_ref())
                .map_err(|reason| vec![reason])?;
            self.closure
                .facts
                .operators
                .decided_formations
                .push(obligation.expression());
            self.closure
                .facts
                .integer_range_obligations
                .push(obligation);
            self.closure.ranges_closed[position] = true;
        }
        if let Some(program) = supplemental_program {
            self.closure
                .program
                .constant_recipe_closure
                .target_bodies
                .extend(
                    program
                        .constant_recipe_closure
                        .target_bodies
                        .iter()
                        .cloned(),
                );
            self.closure
                .program
                .constant_recipe_closure
                .receipts
                .extend(program.constant_recipe_closure.receipts);
        }
        Ok(())
    }
}
