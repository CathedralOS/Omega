//! A source comparison receives order authority only with a checked body
//! theorem and complete source association. Unsupported ordinary callables
//! remain ordinary callables; they do not become termination evidence.

use super::super::Elaborator;
use super::order_graph::{self, GraphFailure};
use diagnostics::Diagnostic;
use validation::computed_order::{OrderDemand, SourceOrderBody, SourceOrderValue};
use validation::mathematical_applications::{PreparedComputedOrder, PreparedComputedOrderEvidence};

fn depth(value: &SourceOrderValue) -> usize {
    match value {
        SourceOrderValue::Successor(child) => depth(child).saturating_add(1),
        _ => 0,
    }
}

impl Elaborator<'_> {
    pub(in super::super) fn retain_computed_orders(&mut self) -> Result<(), Vec<Diagnostic>> {
        let demands = std::mem::take(&mut self.order_demands);
        if demands.is_empty() {
            return Ok(());
        }
        let Some(theory) = self.integer_theory.as_mut() else {
            return Err(self.refuse(
                "computed order evidence requires the shared mathematical integer vocabulary"
                    .into(),
            ));
        };
        let ranks = order_graph::rank_graph(&mut self.arena, &mut self.declarations, theory)
            .map_err(|failure| {
                self.refuse(match failure {
                    GraphFailure::Unavailable(reason) | GraphFailure::Kernel(reason) => reason,
                })
            })?;
        for demand in demands {
            let judgment = match self.computed_order(&demand, &ranks) {
                Ok(evidence) => Ok(evidence),
                Err(GraphFailure::Unavailable(reason)) => {
                    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                        eprintln!(
                            "STRUCT comparison order proof unavailable target={:?} carrier={:?}: {reason}",
                            demand.target, demand.carrier.declaration
                        );
                    }
                    Err(reason)
                }
                Err(GraphFailure::Kernel(reason)) => return Err(self.refuse(reason)),
            };
            self.prepared.computed_orders.insert(PreparedComputedOrder {
                target: demand.target,
                carrier: demand.carrier,
                judgment,
            });
        }
        Ok(())
    }

    fn computed_order(
        &mut self,
        demand: &OrderDemand,
        ranks: &order_graph::CheckedGraph,
    ) -> Result<PreparedComputedOrderEvidence, GraphFailure> {
        let dependencies =
            validation::structurally_admitted_call_closure(self.program, demand.target)
                .map_err(|reason| GraphFailure::Unavailable(reason.into()))?;
        let root = validation::computed_order::source_order_body(
            self.program,
            &demand.carrier,
            demand.target,
        )
        .map_err(|reason| GraphFailure::Unavailable(reason.into()))?;
        let mut bodies = vec![root];
        for &symbol in &dependencies.machines {
            if bodies.iter().any(|body| body.machine == symbol) {
                continue;
            }
            let machine = self.program.machine_by_symbol(symbol).ok_or_else(|| {
                GraphFailure::Unavailable(
                    "the comparison dependency lost its exact source machine".into(),
                )
            })?;
            let entry = self
                .program
                .machine_states(machine)
                .first()
                .ok_or_else(|| {
                    GraphFailure::Unavailable(
                        "the comparison dependency lost its source entry".into(),
                    )
                })?;
            bodies.push(
                validation::computed_order::source_order_body(
                    self.program,
                    &demand.carrier,
                    entry.symbol,
                )
                .map_err(|reason| GraphFailure::Unavailable(reason.into()))?,
            );
        }
        // These are candidate proof goals, not recognized function meanings.
        // A helper may lose a finite number of successors only if every case
        // actually proves that bound. The selected comparison always owes zero.
        let limit = bodies
            .iter()
            .skip(1)
            .map(helper_allowance_limit)
            .max()
            .unwrap_or(0);
        let mut last_reason = "the comparison has no checked structural-order theorem".to_owned();
        for allowance in 0..=limit {
            let mut allowances = vec![allowance; bodies.len()];
            allowances[0] = 0;
            let theory = self.integer_theory.as_mut().ok_or_else(|| {
                GraphFailure::Kernel("the comparison lost its integer vocabulary".into())
            })?;
            match order_graph::comparison_graph(
                &mut self.arena,
                &mut self.declarations,
                theory,
                &bodies,
                &allowances,
            ) {
                Ok(graph) => {
                    let return_constructors = order_graph::source_return_schemas(
                        &mut self.arena,
                        &mut self.declarations,
                        theory,
                        ranks,
                        &graph,
                        &bodies,
                        &allowances,
                    )?;
                    let arity = bodies
                        .iter()
                        .map(|body| body.parameter_types.len())
                        .max()
                        .unwrap_or(0);
                    let (strict_schema, strict_type, strict_closure) =
                        order_graph::zero_comparison_schema(
                            &mut self.arena,
                            &mut self.declarations,
                            theory,
                            ranks,
                            &graph,
                            arity,
                        )?;
                    let mut closure = ranks.assumption_closure.clone();
                    closure.extend(strict_closure);
                    closure.extend_from_slice(&graph.assumption_closure);
                    let signature =
                        proof_admission::Signature::from_declarations(self.declarations.clone());
                    let roots = return_constructors
                        .iter()
                        .flat_map(|&(term, ty)| [term, ty])
                        .collect::<Vec<_>>();
                    closure.extend(proof_admission::judgment_assumption_closure(
                        &self.arena,
                        &signature,
                        &roots,
                    ));
                    closure.sort_unstable();
                    closure.dedup();
                    return Ok(PreparedComputedOrderEvidence {
                        dependencies,
                        bodies,
                        allowances,
                        ranks: ranks.family.clone(),
                        rank_theorem: ranks.theorem,
                        rank_type: ranks.ty,
                        rank_constructors: ranks.constructors.clone(),
                        evaluations: graph.family,
                        theorem: graph.theorem,
                        ty: graph.ty,
                        return_constructors,
                        strict_schema,
                        strict_type,
                        assumption_closure: closure,
                    });
                }
                Err(GraphFailure::Unavailable(reason)) => last_reason = reason,
                Err(failure @ GraphFailure::Kernel(_)) => return Err(failure),
            }
        }
        Err(GraphFailure::Unavailable(last_reason))
    }
}

fn helper_allowance_limit(body: &SourceOrderBody) -> usize {
    body.cases
        .iter()
        .map(|case| {
            case.equations
                .iter()
                .map(|(left, right)| depth(left).saturating_add(depth(right)))
                .sum::<usize>()
                .saturating_add(case.calls.len())
        })
        .max()
        .unwrap_or(0)
}
