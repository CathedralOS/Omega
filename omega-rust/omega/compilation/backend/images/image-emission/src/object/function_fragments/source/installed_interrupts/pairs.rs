//! Replay simultaneous stack pairs from exact placements and actual guard phases.
use super::guards::EntryInput;
use super::{Binary as B, Error, Expr};
use super::{Contract, Graph, Inputs, Scan, Writes, expressions::Expressions, publication};
use selected_instructions::{SelectedBlockId, SelectedInstructionId};

pub(super) struct PairContract {
    pub distinct_ists: Expr,
    pub matching_left: Expr,
    pub matching_right: Expr,
    pub bounded_slice: [Expr; 2],
    pub captured_base: Expr,
    pub captured_end: Expr,
    pub disjoint: Expr,
}

fn binary(kind: B, left: Expr, right: Expr) -> Expr {
    Expr::binary(kind, left, right)
}
fn c(value: u64) -> Expr {
    Expr::constant(value)
}

pub(super) fn derive(
    entries: &[EntryInput<'_>],
    table: &Expr,
    stack: &Expr,
    home: &Expr,
    layout: &target_operations_to_selected_instructions::InstalledInterruptOperandLayout,
) -> Result<Vec<PairContract>, Error> {
    let mut result = Vec::new();
    for left_entry in entries {
        let left_gate = left_entry.gate;
        let left = left_entry.requirement;
        let Some(demand) = &left.arrival else {
            continue;
        };
        if demand
            .simultaneous_placements
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::Mismatch(
                "simultaneous placement roster is not strictly ordered",
            ));
        }
        for &placement in &demand.simultaneous_placements {
            if placement == left.placement_index {
                return Err(Error::Mismatch(
                    "simultaneous dedicated entries share a fixed top",
                ));
            }
            let mut candidates = entries
                .iter()
                .filter(|entry| entry.requirement.placement_index == placement);
            let right_entry = candidates.next().ok_or(Error::Mismatch(
                "simultaneous placement has no exact admitted entry",
            ))?;
            if candidates.next().is_some() {
                return Err(Error::Mismatch("simultaneous placement is duplicated"));
            }
            let right = right_entry.requirement;
            let right_gate = right_entry.gate;
            if !matches!(
                left_entry.stack,
                calling_conventions::EntryStack::Dedicated { .. }
            ) || !matches!(
                right_entry.stack,
                calling_conventions::EntryStack::Dedicated { .. }
            ) {
                return Err(Error::Mismatch(
                    "simultaneous pair lacks two admitted dedicated stacks",
                ));
            }
            let peer = right.arrival.as_ref().ok_or(Error::Mismatch(
                "simultaneous placement lost its arrival demand",
            ))?;
            if peer
                .simultaneous_placements
                .binary_search(&left.placement_index)
                .is_err()
            {
                return Err(Error::Mismatch(
                    "simultaneous placement relation is not reciprocal",
                ));
            }
            if left.placement_index > placement {
                continue;
            }
            // Pair demand applies only to the exact simultaneously live
            // Dedicated domains; Interrupted arrivals use their active parent.
            let left_ist = table.clone().offset(left_gate + 4).load(1);
            let right_ist = table.clone().offset(right_gate + 4).load(1);
            let actual_ist = stack.clone().offset(layout.stack_ist).load(1);
            let backing = stack.clone().offset(layout.stack_words).load(8);
            let length = stack.clone().offset(layout.stack_words + 8).load(8);
            let end = binary(
                B::Add,
                backing.clone(),
                binary(B::Multiply, length.clone(), c(8)),
            );
            result.push(PairContract {
                distinct_ists: binary(
                    B::Equal,
                    binary(B::Equal, left_ist.clone(), right_ist.clone()),
                    c(0),
                ),
                matching_left: binary(B::Equal, actual_ist.clone(), left_ist),
                matching_right: binary(B::Equal, actual_ist, right_ist),
                bounded_slice: [
                    binary(B::AtMost, length, c(u64::MAX / 8)),
                    binary(B::AtMost, backing.clone(), end.clone()),
                ],
                captured_base: backing.clone(),
                captured_end: end.clone(),
                disjoint: binary(
                    B::Or,
                    binary(B::AtMost, home.clone().offset(120).load(8), backing),
                    binary(B::AtMost, end, home.clone().offset(112).load(8)),
                ),
            });
        }
    }
    Ok(result)
}

pub(super) fn consume(
    home: &Expr,
    count: usize,
    stores: &mut Writes<'_>,
) -> Result<Vec<[Scan; 2]>, Error> {
    (0..count)
        .map(|_| {
            Ok([
                Scan::consume(home, 80, stores)?,
                Scan::consume(home, 80, stores)?,
            ])
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    contract: &Contract,
    scans: &[[Scan; 2]],
    mut previous_header: usize,
    publication: SelectedInstructionId,
    refusal: SelectedBlockId,
    graph: &Graph<'_>,
    expressions: &mut Expressions<'_>,
    stores: &mut Writes<'_>,
    input: &Inputs,
    validate: SelectedInstructionId,
    backedges: &mut Vec<(SelectedInstructionId, usize)>,
) -> Result<(), Error> {
    if scans.len() != contract.pairs.len() {
        return Err(Error::Mismatch(
            "simultaneous pairs lost their exact scan roster",
        ));
    }
    for (ordinal, (pair, scans_for_pair)) in contract.pairs.iter().zip(scans).enumerate() {
        let different = graph.require_guard_between(
            expressions,
            &pair.distinct_ists,
            graph.header_instruction(previous_header),
            scans_for_pair[0].count_init,
            refusal,
        )?;
        publication::fresh_observations(&different, input, validate, graph)?;
        let captures = [
            stores.take(
                contract.home.clone().offset(112),
                pair.captured_base.clone(),
                8,
            )?,
            stores.take(
                contract.home.clone().offset(120),
                pair.captured_end.clone(),
                8,
            )?,
        ];
        let mut left_header = None;
        for (side, scan) in scans_for_pair.iter().enumerate() {
            let next = if side == 0 {
                scans_for_pair[1].count_init
            } else {
                scans
                    .get(ordinal + 1)
                    .map_or(publication, |pair| pair[0].count_init)
            };
            if !graph.precedes(scan.count_init, scan.initialize)? {
                return Err(Error::Mismatch(
                    "simultaneous stack scan changed its initialization",
                ));
            }
            graph.require_loop_exit(previous_header, scan.count_init, expressions)?;
            let header = graph.counted_loop(
                expressions,
                &binary(
                    B::Less,
                    contract.stack_index.clone(),
                    contract.stack_limit.clone(),
                ),
                scan.initialize,
                scan.advance,
                next,
            )?;
            for (advance, offset) in [(scan.advance, 80), (scan.count_next, 88)] {
                super::increment_observation(
                    stores.value(advance)?,
                    &contract.home.clone().offset(offset),
                    header,
                    advance,
                    graph,
                )?;
            }
            let matching = if side == 0 {
                &pair.matching_left
            } else {
                &pair.matching_right
            };
            let actual = graph.selected_iteration(
                expressions,
                matching,
                header,
                scan.count_next,
                scan.advance,
            )?;
            publication::fresh_observations(&actual, input, validate, graph)?;
            super::iteration_observations(&actual, &contract.home, header, graph)?;
            for predicate in &pair.bounded_slice {
                let actual = graph.require_iteration_guard(
                    expressions,
                    predicate,
                    header,
                    scan.count_next,
                    scan.advance,
                    refusal,
                )?;
                publication::fresh_observations(&actual, input, validate, graph)?;
                super::iteration_observations(&actual, &contract.home, header, graph)?;
            }
            if side == 0 {
                if !graph.precedes(scan.count_next, captures[0])?
                    || !graph.precedes(captures[0], captures[1])?
                {
                    return Err(Error::Mismatch(
                        "simultaneous left range capture changed order",
                    ));
                }
                for capture in captures {
                    graph.selected_iteration(
                        expressions,
                        matching,
                        header,
                        capture,
                        scan.advance,
                    )?;
                    for predicate in &pair.bounded_slice {
                        let actual = graph.require_guard_between(
                            expressions,
                            predicate,
                            scan.count_next,
                            capture,
                            refusal,
                        )?;
                        super::iteration_observations(&actual, &contract.home, header, graph)?;
                    }
                    let value = stores.value(capture)?;
                    publication::fresh_observations(value, input, validate, graph)?;
                    super::iteration_observations(value, &contract.home, header, graph)?;
                }
                left_header = Some(header);
            } else {
                let (actual, guard) = graph.require_iteration_guard_with_site(
                    expressions,
                    &pair.disjoint,
                    header,
                    scan.count_next,
                    scan.advance,
                    refusal,
                )?;
                publication::fresh_observations(&actual, input, validate, graph)?;
                super::iteration_observations(&actual, &contract.home, header, graph)?;
                let left = left_header
                    .ok_or(Error::Mismatch("simultaneous right range has no left scan"))?;
                let mut observed = [false; 2];
                actual.visit_loads(&mut |address, bytes, instruction| {
                    for (index, offset) in [112, 120].into_iter().enumerate() {
                        if bytes == 8 && *address == contract.home.clone().offset(offset) {
                            let load = instruction.ok_or(Error::Mismatch(
                                "captured range has no actual observation",
                            ))?;
                            // The exact-one left guard establishes that its
                            // selected writes happened. The right phase and
                            // left exit exclude a prior pair's cached load.
                            // The load must reach this exact matched guard;
                            // unmatched iterations also reach the advance.
                            graph.require_loop_exit(left, load, expressions)?;
                            let initialized = graph.precedes(scan.count_init, load)?;
                            let before_guard = graph.precedes(load, guard)?;
                            if !initialized || !before_guard {
                                return Err(graph.pair_capture_error(format!(
                                    "simultaneous range uses another pair's capture; \
                                     pair {ordinal}, side {side}, endpoint {index} at home+{offset}; \
                                     left_header {left}, right_header {header}; \
                                     captures {:?} at {:?}, {:?} at {:?}; \
                                     init {:?} at {:?}, matched {:?} at {:?}, \
                                     load {load:?} at {:?}, advance {:?} at {:?}; \
                                     init_precedes_load={initialized}, \
                                     load_precedes_guard={before_guard}; \
                                     guard {guard:?} at {:?}",
                                    captures[0], graph.position(captures[0])?,
                                    captures[1], graph.position(captures[1])?,
                                    scan.count_init, graph.position(scan.count_init)?,
                                    scan.count_next, graph.position(scan.count_next)?,
                                    graph.position(load)?,
                                    scan.advance, graph.position(scan.advance)?,
                                    graph.position(guard)?,
                                )));
                            }
                            observed[index] = true;
                        }
                    }
                    Ok(())
                })?;
                if observed.contains(&false) {
                    return Err(Error::Mismatch(
                        "simultaneous guard omitted one captured endpoint",
                    ));
                }
            }
            let count = graph.require_guard_between(
                expressions,
                &binary(B::Equal, contract.home.clone().offset(88).load(8), c(1)),
                graph.header_instruction(header),
                next,
                refusal,
            )?;
            count.visit_loads(&mut |address, bytes, instruction| {
                if bytes == 8 && *address == contract.home.clone().offset(88) {
                    graph.require_loop_exit(
                        header,
                        instruction.ok_or(Error::Mismatch(
                            "simultaneous count has no actual observation",
                        ))?,
                        expressions,
                    )?;
                }
                Ok(())
            })?;
            previous_header = header;
            backedges.push((scan.advance, header));
        }
    }
    Ok(())
}
