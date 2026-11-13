use std::collections::{BTreeMap, BTreeSet, VecDeque};

use semantic_vocabulary::{BlockId, PlaceId};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralTypeDeclaration,
    TerminalAffineCleanupAction as Action, TerminalMachine, Terminator,
};

use super::{
    PlainAffineFailure as Failure, PlainAffinePlan, PlainAffineSite as Site, PlainAffineUse,
};
use super::{context::Context, effects};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Custody {
    Absent,
    Live,
    Moved,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    custody: Vec<Custody>,
    /// Execution order of establishments whose homes have not been retired.
    order: Vec<usize>,
}

impl State {
    fn observe(&self, context: &Context, place: PlaceId, site: Site) -> Result<(), PlainAffineUse> {
        if context
            .indexes
            .get(&place)
            .is_some_and(|&index| self.custody[index] != Custody::Live)
        {
            return Err(PlainAffineUse {
                site,
                place,
                reason: "an owned value is used after it moved or before it was established",
            });
        }
        Ok(())
    }

    fn consume(
        &mut self,
        context: &Context,
        argument: &StructuralArgument,
        site: Site,
        machine: &TerminalMachine,
    ) -> Result<(), Failure> {
        if argument.access != StructuralAccess::Owned {
            return Ok(());
        }
        if machine
            .structural_parameter(argument.place)
            .is_some_and(|parameter| parameter.access != StructuralAccess::Owned)
        {
            return Err(Failure::InvalidUse(PlainAffineUse {
                site,
                place: argument.place,
                reason: "an owned transfer cannot move out of borrowed storage",
            }));
        }
        let Some(&index) = context.indexes.get(&argument.place) else {
            return Ok(());
        };
        self.observe(context, argument.place, site)
            .map_err(Failure::InvalidUse)?;
        if !argument.path.is_empty() {
            return Err(Failure::Unsupported(
                "partial extraction of an owned affine record",
            ));
        }
        self.custody[index] = Custody::Moved;
        Ok(())
    }

    fn cleanup(&mut self, context: &Context, needed: &BTreeSet<PlaceId>) -> Vec<Action> {
        let mut actions = Vec::new();
        for &index in self.order.iter().rev() {
            let place = context.places[index];
            if needed.contains(&place) {
                continue;
            }
            match self.custody[index] {
                Custody::Live => actions.push(Action::DiscardRoot(place)),
                Custody::Moved => actions.push(Action::RetireHome(place)),
                Custody::Absent => {}
            }
            self.custody[index] = Custody::Absent;
        }
        self.order
            .retain(|&index| self.custody[index] != Custody::Absent);
        actions
    }
}

pub(super) fn reconstruct(
    types: &[StructuralTypeDeclaration],
    machine: &TerminalMachine,
) -> Result<PlainAffinePlan, Failure> {
    let context = Context::new(types, machine)?;
    let mut plan = PlainAffinePlan {
        cleanup: BTreeMap::new(),
        owned: context.places.iter().copied().collect(),
        invalid: None,
    };
    let borrowed = machine
        .structural_parameters
        .iter()
        .chain(machine.block_structural_parameters())
        .any(|parameter| parameter.access != StructuralAccess::Owned);
    if context.places.is_empty() && !borrowed {
        return Ok(plan);
    }
    let blocks = machine
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.id, index))
        .collect::<BTreeMap<_, _>>();
    if blocks.len() != machine.blocks.len() {
        return Err(Failure::Malformed("owned flow repeats a block identity"));
    }
    let entry = *blocks
        .get(&machine.entry)
        .ok_or(Failure::Malformed("owned flow has no entry block"))?;
    let needed = liveness(machine, &blocks)?;
    let mut initial = State {
        custody: vec![Custody::Absent; context.places.len()],
        order: context.entry.clone(),
    };
    for &index in &context.entry {
        initial.custody[index] = Custody::Live;
    }
    let mut incoming = vec![None; machine.blocks.len()];
    incoming[entry] = Some(initial);
    let mut pending = VecDeque::from([entry]);
    while let Some(index) = pending.pop_front() {
        let block = &machine.blocks[index];
        let mut state = incoming[index]
            .clone()
            .ok_or(Failure::Malformed("owned flow lost its predecessor"))?;
        let process = (|| -> Result<(), Failure> {
            for operation in &block.operations {
                let site = Site::Operation(operation.id);
                for place in effects::reads(&operation.kind) {
                    state
                        .observe(&context, place, site)
                        .map_err(Failure::InvalidUse)?;
                }
                for argument in effects::arguments(&operation.kind) {
                    state.consume(&context, argument, site, machine)?;
                }
                if let Some(result) = operation.result.structural() {
                    if let Some(&root) = context.indexes.get(&result.place) {
                        if state.custody[root] != Custody::Absent {
                            return Err(Failure::InvalidUse(PlainAffineUse {
                                site,
                                place: result.place,
                                reason: "establishment overwrites a live value or an unretired home",
                            }));
                        }
                        state.custody[root] = Custody::Live;
                        state.order.push(root);
                    }
                }
            }
            let site = Site::Block(block.id);
            for place in effects::terminal_reads(&block.terminator) {
                state
                    .observe(&context, place, site)
                    .map_err(Failure::InvalidUse)?;
            }
            for argument in effects::terminal_arguments(&block.terminator) {
                state.consume(&context, argument, site, machine)?;
            }
            if let Terminator::ReturnStructural { source, .. } = block.terminator {
                state.consume(
                    &context,
                    &StructuralArgument::whole(source, StructuralAccess::Owned),
                    site,
                    machine,
                )?;
            }
            Ok(())
        })();
        match process {
            Err(Failure::InvalidUse(invalid)) => {
                plan.invalid = Some(invalid);
                return Ok(plan);
            }
            Err(error) => return Err(error),
            Ok(()) => {}
        }
        let successors = block.terminator.successor_edges();
        for (edge, destination) in successors {
            let target = *blocks
                .get(&destination)
                .ok_or(Failure::Malformed("owned successor names no block"))?;
            let mut outgoing = state.clone();
            let actions = outgoing.cleanup(&context, &needed[target]);
            plan.cleanup.insert(edge, actions);
            match &incoming[target] {
                Some(previous) if previous != &outgoing => {
                    let root = context
                        .places
                        .iter()
                        .enumerate()
                        .find(|(root, _)| previous.custody[*root] != outgoing.custody[*root])
                        .map(|(_, place)| *place)
                        .or_else(|| outgoing.order.first().map(|&root| context.places[root]))
                        .ok_or(Failure::Malformed("owned order mismatch has no live home"))?;
                    plan.invalid = Some(PlainAffineUse {
                        site: Site::Edge(edge),
                        place: root,
                        reason: "incoming paths disagree on the live ownership frontier",
                    });
                    return Ok(plan);
                }
                Some(_) => {}
                None => {
                    incoming[target] = Some(outgoing);
                    pending.push_back(target);
                }
            }
        }
        if matches!(
            block.terminator,
            Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnStructural { .. }
        ) {
            plan.cleanup.insert(
                block.terminator.edge(),
                state.cleanup(&context, &BTreeSet::new()),
            );
        }
        // A crash abandons its frame through its actual outcome; a no-normal
        // invocation suspends it. Neither is a normal disposal continuation.
    }
    Ok(plan)
}

fn liveness(
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, usize>,
) -> Result<Vec<BTreeSet<PlaceId>>, Failure> {
    let mut uses = Vec::with_capacity(machine.blocks.len());
    let mut definitions = Vec::with_capacity(machine.blocks.len());
    for block in &machine.blocks {
        let mut read = BTreeSet::new();
        let mut defined = BTreeSet::new();
        for operation in &block.operations {
            read.extend(
                effects::reads(&operation.kind)
                    .into_iter()
                    .filter(|place| !defined.contains(place)),
            );
            if let Some(result) = operation.result.structural() {
                defined.insert(result.place);
            }
        }
        read.extend(
            effects::terminal_reads(&block.terminator)
                .into_iter()
                .filter(|place| !defined.contains(place)),
        );
        uses.push(read);
        definitions.push(defined);
    }
    let mut needed = uses.clone();
    loop {
        let mut changed = false;
        for (index, block) in machine.blocks.iter().enumerate().rev() {
            let mut current = uses[index].clone();
            for target in block.terminator.successor_targets() {
                let target = *blocks
                    .get(&target)
                    .ok_or(Failure::Malformed("owned liveness names no successor"))?;
                current.extend(
                    needed[target]
                        .iter()
                        .filter(|place| !definitions[index].contains(place))
                        .copied(),
                );
            }
            if current != needed[index] {
                needed[index] = current;
                changed = true;
            }
        }
        if !changed {
            return Ok(needed);
        }
    }
}
