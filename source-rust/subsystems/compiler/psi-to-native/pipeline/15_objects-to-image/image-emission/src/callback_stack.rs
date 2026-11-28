//! Compose reachable callback bodies in call order. Planning supplies physical
//! call edges and object replay supplies its independently retained addresses.
use function_identity::MachineFunctionIdentity;
use machine_code::physical_stack::{
    PhysicalStackProgram, StackDemand, StackDemandError, derive_physical_stack_demand,
};
use semantic_vocabulary::MachineId;

pub struct CallbackStackBody<'a> {
    pub program: &'a PhysicalStackProgram,
    pub entry: MachineId,
    pub callbacks: Vec<MachineFunctionIdentity>,
    pub entry_cost: u64,
}

#[derive(Debug)]
pub enum Error {
    MissingBody,
    Cycle(MachineId),
    Body(StackDemandError),
    EntryCostOverflow(MachineFunctionIdentity),
}

#[derive(Clone, Copy)]
enum State {
    Unseen,
    Visiting(MachineId),
    Finished,
}

struct Frame<'a> {
    body: CallbackStackBody<'a>,
    row: Option<usize>,
    children: Vec<usize>,
}

impl<'a> Frame<'a> {
    fn new(mut body: CallbackStackBody<'a>, row: Option<usize>) -> Self {
        if body.callbacks.len() > 1 {
            let mut seen = std::collections::HashSet::with_capacity(body.callbacks.len());
            body.callbacks.retain(|identity| seen.insert(*identity));
        }
        Self {
            body,
            row,
            children: Vec::new(),
        }
    }
}

type ChildDemands = Vec<(MachineFunctionIdentity, StackDemand)>;

fn measure(
    body: &CallbackStackBody<'_>,
    callback: Option<MachineFunctionIdentity>,
    children: ChildDemands,
) -> Result<StackDemand, Error> {
    let demand =
        derive_physical_stack_demand(body.program, body.entry, children).map_err(Error::Body)?;
    let Some(callback) = callback else {
        return Ok(demand);
    };
    let alignment = demand.stack_alignment();
    demand
        .with_entry_cost(body.entry_cost, alignment)
        .ok_or(Error::EntryCostOverflow(callback))
}

pub fn derive<'a, T>(
    root: CallbackStackBody<'a>,
    roster: &'a [T],
    identity: impl Fn(&T) -> MachineFunctionIdentity,
    describe: impl FnMut(&'a T) -> CallbackStackBody<'a>,
) -> Result<StackDemand, Error> {
    compose(root, roster, identity, describe, measure)
}

fn compose<'a, T>(
    root: CallbackStackBody<'a>,
    roster: &'a [T],
    identity: impl Fn(&T) -> MachineFunctionIdentity,
    mut describe: impl FnMut(&'a T) -> CallbackStackBody<'a>,
    mut measure: impl FnMut(
        &CallbackStackBody<'_>,
        Option<MachineFunctionIdentity>,
        ChildDemands,
    ) -> Result<StackDemand, Error>,
) -> Result<StackDemand, Error> {
    if root.callbacks.is_empty() {
        return measure(&root, None, Vec::new());
    }
    // Include the row in the sort so duplicate identities resolve to their
    // first retained body, just as an ordered roster lookup does.
    let mut index: Vec<_> = roster
        .iter()
        .enumerate()
        .map(|(row, body)| (identity(body).to_bytes(), row))
        .collect();
    index.sort_unstable();
    let mut states = vec![State::Unseen; roster.len()];
    let mut callers = vec![0usize; roster.len()];
    let mut order = Vec::new();
    let mut pending = vec![Frame::new(root, None)];
    // Keep the successful prefix even on a bad edge. An earlier child's
    // physical stack or bridge may be the first refusal in call order.
    let graph_result: Result<(), Error> = loop {
        let Some(frame) = pending.last_mut() else {
            break Ok(());
        };
        let Some(callback) = frame.body.callbacks.get(frame.children.len()) else {
            let finished = pending.pop().expect("current body");
            if let Some(row) = finished.row {
                states[row] = State::Finished;
            }
            order.push(finished);
            continue;
        };
        let key = callback.to_bytes();
        let position = index.partition_point(|(candidate, _)| *candidate < key);
        let Some(&(_, row)) = index
            .get(position)
            .filter(|(candidate, _)| *candidate == key)
        else {
            break Err(Error::MissingBody);
        };
        frame.children.push(row);
        callers[row] += 1;
        match states[row] {
            State::Visiting(entry) => break Err(Error::Cycle(entry)),
            State::Unseen => {
                let body = describe(&roster[row]);
                states[row] = State::Visiting(body.entry);
                pending.push(Frame::new(body, Some(row)));
            }
            State::Finished => {}
        }
    };

    let mut completed: Vec<Option<StackDemand>> = vec![None; roster.len()];
    for frame in order {
        let demands = frame
            .children
            .into_iter()
            .map(|row| {
                callers[row] -= 1;
                // Move a child's owned evidence to its last caller. Only callers
                // that share a child need a copy in the returned demand tree.
                let demand = if callers[row] == 0 {
                    completed[row]
                        .take()
                        .expect("child completed before caller")
                } else {
                    completed[row]
                        .as_ref()
                        .expect("child completed before caller")
                        .clone()
                };
                (identity(&roster[row]), demand)
            })
            .collect();
        let demand = measure(
            &frame.body,
            frame.row.map(|row| identity(&roster[row])),
            demands,
        )?;
        let Some(row) = frame.row else {
            return Ok(demand);
        };
        completed[row] = Some(demand);
    }
    Err(graph_result.expect_err("a complete traversal evaluates the root"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::physical_stack::{LocalStackExtent, PhysicalStackFunction};
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn leaf_program() -> PhysicalStackProgram {
        let psi = TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        };
        let entry = MachineId::new(1).unwrap();
        PhysicalStackProgram {
            psi,
            entry,
            target: target::NativeTarget::linux_x64(),
            functions: vec![PhysicalStackFunction {
                machine: entry,
                local: LocalStackExtent::Finite(16),
                calls: vec![],
                foreign_calls: vec![],
            }],
        }
    }

    #[test]
    fn diamond_evaluates_each_reachable_body_once_and_preserves_bridge_costs() {
        let program = leaf_program();
        let psi = program.psi;
        let entry = program.functions[0].machine;
        let identity = |row| MachineFunctionIdentity::callback_thunk(psi, row).unwrap();
        let body = |rows: &[usize], entry_cost| CallbackStackBody {
            program: &program,
            entry,
            callbacks: rows.iter().copied().map(identity).collect(),
            entry_cost,
        };
        // Root calls 0 and 1. Both call 2. An unreachable body calls itself.
        let edges: &[&[usize]] = &[&[2, 2], &[2], &[], &[3]];
        let mut visits = vec![];
        let actual = compose(
            body(&[0, 1, 0], 0),
            &[0, 1, 2, 3],
            |row| identity(*row),
            |&row| body(edges[row], 8 * (row as u64 + 1)),
            |body, callback, children| {
                visits.push(callback);
                measure(body, callback, children)
            },
        )
        .unwrap();
        assert_eq!(
            visits,
            [
                Some(identity(2)),
                Some(identity(0)),
                Some(identity(1)),
                None
            ]
        );

        let demand = |children| derive_physical_stack_demand(&program, entry, children).unwrap();
        let shared = demand(vec![]).with_entry_cost(24, 16).unwrap();
        let left = demand(vec![(identity(2), shared.clone())])
            .with_entry_cost(8, 16)
            .unwrap();
        let right = demand(vec![(identity(2), shared)])
            .with_entry_cost(16, 16)
            .unwrap();
        assert_eq!(
            actual,
            demand(vec![(identity(0), left), (identity(1), right)])
        );
    }

    #[test]
    fn first_body_or_bridge_failure_precedes_a_later_missing_or_cyclic_edge() {
        let mut program = leaf_program();
        let entry = program.functions[0].machine;
        let child = MachineFunctionIdentity::callback_thunk(program.psi, 0).unwrap();
        let missing = MachineFunctionIdentity::callback_thunk(program.psi, 1).unwrap();
        let run =
            |program: &PhysicalStackProgram, root_edges: Vec<_>, child_edges: Vec<_>, cost| {
                let body = |callbacks, entry_cost| CallbackStackBody {
                    program,
                    entry,
                    callbacks,
                    entry_cost,
                };
                derive(
                    body(root_edges, 0),
                    &[child],
                    |id| *id,
                    |_| body(child_edges.clone(), cost),
                )
            };
        assert!(
            matches!(run(&program, vec![child, missing], vec![], u64::MAX),
            Err(Error::EntryCostOverflow(id)) if id == child)
        );
        assert!(matches!(
            run(&program, vec![child, missing], vec![], 0),
            Err(Error::MissingBody)
        ));
        assert!(
            matches!(run(&program, vec![child, missing], vec![child], 0),
            Err(Error::Cycle(machine)) if machine == entry)
        );
        program.functions[0].local = LocalStackExtent::Unaccounted;
        assert!(matches!(run(&program, vec![child, missing], vec![], 0),
            Err(Error::Body(StackDemandError::UnaccountedTerminalStack(machine))) if machine == entry));
    }
}
