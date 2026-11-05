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
        let trace = std::env::var_os("OMEGA_STRUCT_TRACE").is_some();
        let rank_start = self.declarations.len();
        let rank_started = trace.then(std::time::Instant::now);
        if trace {
            eprintln!("STRUCT comparison rank proof start declaration={rank_start}");
        }
        let Some(theory) = self.integer_theory.as_mut() else {
            return Err(self.refuse(
                "computed order evidence requires the shared mathematical integer vocabulary"
                    .into(),
            ));
        };
        let program = self.program;
        let completed = (|| -> Result<Vec<PreparedComputedOrder>, GraphFailure> {
            let mut proofs = order_graph::prepare(&mut self.arena, &mut self.declarations, theory)?;
            let ranks = proofs.rank_graph()?;
            if let Some(started) = rank_started {
                eprintln!(
                    "STRUCT comparison rank proof complete declarations={rank_start}..{} micros={}",
                    proofs.declaration_count(),
                    started.elapsed().as_micros()
                );
            }
            let mut sources = validation::computed_order::SourceOrderPreparation::new(program);
            let mut completed = Vec::with_capacity(demands.len());
            for demand in demands {
                let start = proofs.declaration_count();
                let started = trace.then(std::time::Instant::now);
                if trace {
                    eprintln!(
                        "STRUCT comparison source proof start target={:?} carrier={:?} declaration={start}",
                        demand.target, demand.carrier
                    );
                }
                let judgment = match computed_order(program, &demand, &ranks, &mut sources, &mut proofs) {
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
                    Err(failure @ GraphFailure::Kernel(_)) => return Err(failure),
                };
                if let Some(started) = started {
                    eprintln!(
                        "STRUCT comparison source proof complete target={:?} carrier={:?} declarations={start}..{} proved={} micros={}",
                        demand.target,
                        demand.carrier,
                        proofs.declaration_count(),
                        judgment.is_ok(),
                        started.elapsed().as_micros()
                    );
                }
                completed.push(PreparedComputedOrder {
                    target: demand.target,
                    carrier: demand.carrier,
                    judgment,
                });
            }
            Ok(completed)
        })()
        .map_err(|failure| {
            self.refuse(match failure {
                GraphFailure::Unavailable(reason) | GraphFailure::Kernel(reason) => reason,
            })
        })?;
        for order in completed {
            self.prepared.computed_orders.insert(order);
        }
        Ok(())
    }
}

fn computed_order(
    program: &dyn validation::CheckingInput,
    demand: &OrderDemand,
    ranks: &order_graph::CheckedGraph,
    sources: &mut validation::computed_order::SourceOrderPreparation<'_>,
    proofs: &mut order_graph::OrderProofPreparation<'_>,
) -> Result<PreparedComputedOrderEvidence, GraphFailure> {
    let dependencies = validation::structurally_admitted_call_closure(program, demand.target)
        .map_err(|reason| GraphFailure::Unavailable(reason.into()))?;
    let root = sources
        .body(&demand.carrier, demand.target)
        .map_err(|reason| GraphFailure::Unavailable(reason.into()))?;
    let mut body_positions = symbols::SymbolKeyMap::default();
    body_positions.insert(root.entry, 0);
    let mut bodies = vec![root];
    let eligible_machines = dependencies
        .machines
        .iter()
        .map(|&machine| (machine, ()))
        .collect::<symbols::SymbolKeyMap<_, _>>();
    // Proof dependencies can include contracts and erased citations. Only
    // actual source calls supply children of the evaluation graph.
    let mut position = 0;
    while position < bodies.len() {
        let targets = bodies[position]
            .cases
            .iter()
            .flat_map(|case| case.calls.iter().map(|call| call.target))
            .collect::<Vec<_>>();
        for target in targets {
            if body_positions.contains_key(&target) {
                continue;
            }
            let body = sources
                .body(&demand.carrier, target)
                .map_err(|reason| GraphFailure::Unavailable(reason.into()))?;
            if body_positions.contains_key(&body.entry) {
                continue;
            }
            if !eligible_machines.contains_key(&body.machine) {
                return Err(GraphFailure::Unavailable(
                    "the comparison call is outside its complete structural dependency closure"
                        .into(),
                ));
            }
            body_positions.insert(body.entry, bodies.len());
            bodies.push(body);
        }
        position += 1;
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
        match proofs.comparison_graph(&bodies, &body_positions, &allowances) {
            Ok(graph) => {
                let return_constructors = proofs.source_return_schemas(
                    ranks,
                    &graph,
                    &bodies,
                    &body_positions,
                    &allowances,
                )?;
                let arity = bodies
                    .iter()
                    .map(|body| body.parameter_types.len())
                    .max()
                    .unwrap_or(0);
                let (strict_schema, strict_type, strict_closure) =
                    proofs.zero_comparison_schema(ranks, &graph, arity)?;
                let mut closure = ranks.assumption_closure.clone();
                closure.extend(strict_closure);
                closure.extend_from_slice(&graph.assumption_closure);
                let roots = return_constructors
                    .iter()
                    .flat_map(|&(term, ty)| [term, ty])
                    .collect::<Vec<_>>();
                closure.extend(proofs.assumption_closure(&roots));
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
