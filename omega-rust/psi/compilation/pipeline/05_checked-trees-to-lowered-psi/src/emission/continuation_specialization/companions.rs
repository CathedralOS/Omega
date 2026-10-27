//! Preserve exact source and proof joins for every retained static occurrence.

use super::{CompanionRows, Counters, Instance};
use crate::emission::selected_continuations::Specialization;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, obligation_id};
use lowered_psi::LoweredSourceCallSite;
use terminal_psi::TerminalMachine;

pub(super) fn instantiate(
    machine: &mut TerminalMachine,
    topology: &Specialization,
    instances: &mut [Instance],
    rows: &mut CompanionRows<'_>,
    counters: &mut Counters<'_>,
) -> Result<(), LoweringError> {
    let calls = std::mem::take(rows.source_calls);
    let returns = std::mem::take(rows.source_returns);
    let comparisons = std::mem::take(rows.selected_comparisons);
    let completions = std::mem::take(&mut machine.loan_completions);
    let invariants = std::mem::take(rows.scalar_block_invariants);
    let coercions = std::mem::take(rows.qualification_coercions);
    let source_checked = std::mem::take(rows.source_checked_obligations);
    for instance in instances.iter_mut() {
        let namespace = &mut instance.namespace;
        for original in &returns {
            if let Some((_, edge)) = namespace
                .edges
                .iter()
                .find(|(source, _)| *source == original.edge)
            {
                let mut returned = original.clone();
                returned.edge = *edge;
                rows.source_returns.push(returned);
            }
        }
        for original in &calls {
            let owned = match original.terminal_site {
                LoweredSourceCallSite::Operation(operation) => namespace
                    .operations
                    .iter()
                    .any(|(source, _)| *source == operation),
                LoweredSourceCallSite::InvokeNoNormal(edge)
                | LoweredSourceCallSite::ExternalCompletion(edge) => {
                    namespace.edges.iter().any(|(source, _)| *source == edge)
                }
            };
            if owned {
                let mut call = original.clone();
                namespace.source_call(&mut call);
                rows.source_calls.push(call);
            }
        }
        for original in &comparisons {
            if let Some((_, target)) = namespace
                .operations
                .iter()
                .find(|(source, _)| *source == original.terminal_operation)
            {
                let mut comparison = *original;
                comparison.terminal_operation = *target;
                rows.selected_comparisons.push(comparison);
            }
        }
        for original in &completions {
            if namespace
                .operations
                .iter()
                .any(|(source, _)| *source == original.operation)
            {
                let mut completion = original.clone();
                namespace.loan_completion(&mut completion);
                completion.claims.sort_by_key(|binding| binding.claim);
                machine.loan_completions.push(completion);
            }
        }
        for original in &source_checked {
            if let Some((_, target)) = namespace
                .obligations
                .iter()
                .rev()
                .find(|(source, _)| source == original)
            {
                rows.source_checked_obligations.push(*target);
            }
        }
    }
    // Instance discovery follows control reachability, not operation identity.
    // Publish the remapped companion catalogue in its canonical key order.
    machine.loan_completions.sort_by(|left, right| {
        (left.operation, left.place, &left.path).cmp(&(right.operation, right.place, &right.path))
    });
    // Signature-level source decisions are outside any private block's
    // namespace. Their identities and original evidence remain unchanged.
    rows.source_checked_obligations
        .extend(source_checked.into_iter().filter(|original| {
            !instances.iter().any(|instance| {
                instance
                    .namespace
                    .obligations
                    .iter()
                    .any(|(source, _)| source == original)
            })
        }));
    rows.source_checked_obligations.sort();
    rows.source_checked_obligations.dedup();

    for position in 0..instances.len() {
        let original_block =
            topology.continuations[instances[position].continuation].original_block;
        for original in invariants.iter().filter(|invariant| {
            invariant.machine == machine.id && invariant.header == machine.blocks[original_block].id
        }) {
            let mut invariant = original.clone();
            invariant.arrivals.clear();
            instances[position]
                .namespace
                .block_invariant(&mut invariant)?;
            for predecessor in instances.iter_mut() {
                for (edge, successor) in &predecessor.successors {
                    if *successor != position {
                        continue;
                    }
                    let obligation = if let Some(arrival) = original
                        .arrivals
                        .iter()
                        .find(|arrival| arrival.edge == *edge)
                    {
                        predecessor.namespace.obligation(arrival.obligation)
                    } else {
                        // A bypass can add an arrival to an existing header.
                        // The predicate is unchanged and this new edge owes its
                        // own proof; it inherits no source-checked certificate.
                        obligation_id(allocate_dense(counters.next_obligation)?)
                    };
                    invariant
                        .arrivals
                        .push(terminal_psi::ScalarBlockInvariantArrival {
                            edge: predecessor.namespace.edge(*edge),
                            obligation,
                        });
                }
            }
            if position == 0 {
                invariant.arrivals.extend(
                    original
                        .arrivals
                        .iter()
                        .filter(|arrival| {
                            !machine.blocks.iter().any(|block| {
                                block
                                    .terminator
                                    .successor_edges()
                                    .iter()
                                    .any(|(edge, _)| *edge == arrival.edge)
                            })
                        })
                        .cloned(),
                );
            }
            invariant.arrivals.sort_by_key(|arrival| arrival.edge);
            rows.scalar_block_invariants.push(invariant);
        }
    }
    rows.scalar_block_invariants.extend(
        invariants
            .into_iter()
            .filter(|invariant| invariant.machine != machine.id),
    );
    rows.scalar_block_invariants
        .sort_by_key(|invariant| (invariant.machine, invariant.header));

    for position in 0..instances.len() {
        let original_block =
            topology.continuations[instances[position].continuation].original_block;
        let original_edges = machine.blocks[original_block].terminator.successor_edges();
        for original in coercions
            .iter()
            .filter(|row| original_edges.iter().any(|(edge, _)| *edge == row.edge))
        {
            let Some((_, successor)) = instances[position]
                .successors
                .iter()
                .find(|(edge, _)| *edge == original.edge)
            else {
                return unsupported("qualification arrival lost its selected continuation");
            };
            let successor = *successor;
            let source_target = original_edges
                .iter()
                .find(|(edge, _)| *edge == original.edge)
                .unwrap()
                .1;
            let source_block = machine
                .block(&source_target)
                .ok_or(LoweringError::Unsupported(
                    "qualification arrival has no original target",
                ))?;
            let ordinal = source_block
                .parameters
                .iter()
                .position(|parameter| parameter.id == original.destination)
                .ok_or(LoweringError::Unsupported(
                    "qualification destination is not a target parameter",
                ))?;
            let destination_original =
                topology.continuations[instances[successor].continuation].original_block;
            let destination = machine.blocks[destination_original]
                .parameters
                .get(ordinal)
                .ok_or(LoweringError::Unsupported(
                    "qualification bypass lost its parameter",
                ))?
                .id;
            let mut row = *original;
            row.edge = instances[position].namespace.edge(row.edge);
            row.source = instances[position].namespace.value(row.source);
            row.destination = instances[successor].namespace.value(destination);
            rows.qualification_coercions.push(row);
        }
    }
    rows.qualification_coercions.extend(
        coercions
            .into_iter()
            .filter(|row| row.machine != machine.id),
    );
    rows.qualification_coercions
        .sort_by_key(|row| (row.machine, row.edge, row.argument_ordinal));
    Ok(())
}
