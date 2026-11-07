//! Reachability and guard edges over the actual selected control graph.
use super::Error;
use super::expressions::{Binary, Expr, Expressions};
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedInstructionId, SelectedInstructionKind as Kind,
    SelectedTerminator,
};

pub(super) struct Graph<'a> {
    function: &'a SelectedFunction,
    operation: semantic_vocabulary::OperationId,
    positions: Vec<(SelectedInstructionId, (usize, usize))>,
    successors: Vec<Vec<usize>>,
    dominators: Vec<Vec<bool>>,
    reachable: Vec<bool>,
}
impl<'a> Graph<'a> {
    pub(super) fn new(
        function: &'a SelectedFunction,
        operation: semantic_vocabulary::OperationId,
    ) -> Result<Self, Error> {
        let count = function.blocks.len();
        let mut positions = Vec::new();
        let mut successors = vec![Vec::new(); count];
        for (index, block) in function.blocks.iter().enumerate() {
            if block.id.0 as usize != index {
                return Err(Error::Mismatch(
                    "installation replay requires the selected block arena",
                ));
            }
            for (position, row) in block
                .instructions
                .iter()
                .chain(std::iter::once(
                    selected_instructions::terminator_instruction(&block.terminator),
                ))
                .enumerate()
            {
                positions.push((row.id, (index, position)));
            }
            for edge in selected_instructions::terminator_successors(&block.terminator) {
                if edge.block.0 as usize >= count {
                    return Err(Error::Mismatch("installation branch leaves its function"));
                }
                successors[index].push(edge.block.0 as usize);
            }
        }
        positions.sort_unstable_by_key(|row| row.0);
        if positions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::Mismatch(
                "installation has duplicated selected instruction IDs",
            ));
        }
        let entry = function.entry_block.0 as usize;
        if entry >= count {
            return Err(Error::Mismatch("installation function has no entry"));
        }
        let mut reachable = vec![false; count];
        let mut pending = vec![entry];
        while let Some(block) = pending.pop() {
            if reachable[block] {
                continue;
            }
            reachable[block] = true;
            pending.extend(successors[block].iter().copied());
        }
        let mut predecessors = vec![Vec::new(); count];
        for (from, edges) in successors.iter().enumerate() {
            for &to in edges {
                predecessors[to].push(from);
            }
        }
        let mut dominators = vec![reachable.clone(); count];
        dominators[entry].fill(false);
        dominators[entry][entry] = true;
        loop {
            let mut changed = false;
            for block in 0..count {
                if block == entry || !reachable[block] {
                    continue;
                }
                let mut next = reachable.clone();
                for &predecessor in &predecessors[block] {
                    if reachable[predecessor] {
                        for (value, previous) in next.iter_mut().zip(&dominators[predecessor]) {
                            *value &= *previous;
                        }
                    }
                }
                next[block] = true;
                if next != dominators[block] {
                    dominators[block] = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(Self {
            function,
            operation,
            positions,
            successors,
            dominators,
            reachable,
        })
    }
    pub(super) fn position(&self, id: SelectedInstructionId) -> Result<(usize, usize), Error> {
        self.positions
            .binary_search_by_key(&id, |row| row.0)
            .ok()
            .map(|index| self.positions[index].1)
            .ok_or(Error::Mismatch(
                "installation instruction is outside its CFG",
            ))
    }
    pub(super) fn require_owned_observation(&self, id: SelectedInstructionId) -> Result<(), Error> {
        let (block, position) = self.position(id)?;
        let row = self.function.blocks[block]
            .instructions
            .get(position)
            .ok_or(Error::Mismatch(
                "installation address observation is not an instruction",
            ))?;
        if row.provenance.operations.as_slice() != [self.operation]
            || !row.provenance.edges.is_empty()
        {
            return Err(Error::Mismatch(
                "installation address belongs to a different operation",
            ));
        }
        Ok(())
    }
    pub(super) fn precedes(
        &self,
        first: SelectedInstructionId,
        last: SelectedInstructionId,
    ) -> Result<bool, Error> {
        let (a, ai) = self.position(first)?;
        let (b, bi) = self.position(last)?;
        if !self.reachable[a] || !self.reachable[b] {
            return Ok(false);
        }
        Ok(if a == b {
            ai < bi
        } else {
            self.dominators[b][a]
        })
    }
    pub(super) fn pair_capture_error(&self, detail: String) -> Error {
        Error::InstallationPairCapture {
            machine: self.function.machine,
            operation: self.operation,
            detail,
        }
    }

    /// Show where this predicate is read when a capture check fails. Several
    /// pairs can have the same expression, so these are diagnostic candidates,
    /// not another judgment about which guard belongs to the current scan.
    pub(super) fn matching_guard_locations(
        &self,
        expressions: &mut Expressions<'_>,
        expected: &Expr,
        load: SelectedInstructionId,
        matched: SelectedInstructionId,
    ) -> Result<String, Error> {
        let mut sites = Vec::new();
        for (index, block) in self.function.blocks.iter().enumerate() {
            if let Some((actual, _, _)) = self.predicate(index, expressions)?
                && actual == *expected
            {
                let branch = selected_instructions::terminator_instruction(&block.terminator).id;
                sites.push(format!(
                    "{branch:?} at {:?}: matched_precedes_guard={}, load_precedes_guard={}",
                    self.position(branch)?,
                    self.precedes(matched, branch)?,
                    self.precedes(load, branch)?,
                ));
            }
        }
        Ok(sites.join("; "))
    }
    pub(super) fn block_dominates(
        &self,
        block: SelectedBlockId,
        instruction: SelectedInstructionId,
    ) -> Result<bool, Error> {
        let source = block.0 as usize;
        let (destination, _) = self.position(instruction)?;
        Ok(self.reachable.get(source).copied().unwrap_or(false)
            && self.reachable[destination]
            && self.dominators[destination][source])
    }
    pub(super) fn header_instruction(&self, header: usize) -> SelectedInstructionId {
        selected_instructions::terminator_instruction(&self.function.blocks[header].terminator).id
    }
    pub(super) fn require_loop_exit(
        &self,
        header: usize,
        before: SelectedInstructionId,
        expressions: &mut Expressions<'_>,
    ) -> Result<(), Error> {
        let (_, _, exit) = self
            .predicate(header, expressions)?
            .ok_or(Error::Mismatch("installation loop lost its exit"))?;
        let (sink, _) = self.position(before)?;
        if !self.precedes(self.header_instruction(header), before)?
            || self.reachable_without(sink, (header, exit))
        {
            return Err(Error::Mismatch(
                "installation uses a loop result before completing every iteration",
            ));
        }
        Ok(())
    }
    /// With all proven counted backedges removed, every route through
    /// the installation must finish at refusal or retained work. No other
    /// cycle may hide a new divergence inside a guard or matching iteration.
    pub(super) fn bounded_region(
        &self,
        start: SelectedInstructionId,
        work: SelectedInstructionId,
        refusal: SelectedBlockId,
        counted: &[(SelectedInstructionId, usize)],
    ) -> Result<(), Error> {
        let (start, _) = self.position(start)?;
        let (work, _) = self.position(work)?;
        let removed = counted
            .iter()
            .map(|(advance, header)| self.position(*advance).map(|(block, _)| (block, *header)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut colors = vec![0u8; self.successors.len()];
        let mut pending = vec![(start, false)];
        while let Some((block, exit)) = pending.pop() {
            if block == work || block == refusal.0 as usize {
                continue;
            }
            if exit {
                colors[block] = 2;
                continue;
            }
            if colors[block] == 1 {
                return Err(Error::Mismatch(
                    "installation has an unbounded internal cycle",
                ));
            }
            if colors[block] == 2 {
                continue;
            }
            colors[block] = 1;
            pending.push((block, true));
            if self.successors[block].is_empty() {
                return Err(Error::Mismatch(
                    "installation exits without refusal or retained work",
                ));
            }
            for &next in &self.successors[block] {
                if !removed.contains(&(block, next)) {
                    pending.push((next, false));
                }
            }
        }
        Ok(())
    }
    /// Once validation starts, another source operation cannot change the
    /// observed table or CPU state before installation finishes. The ordinary
    /// refusal continuation is outside this interval and resumes the caller.
    pub(super) fn protected_interval(
        &self,
        start: SelectedInstructionId,
        work: SelectedInstructionId,
        refusal: SelectedBlockId,
        expressions: &mut Expressions<'_>,
    ) -> Result<(), Error> {
        let (start_block, start_position) = self.position(start)?;
        let (work_block, work_position) = self.position(work)?;
        let mut seen = vec![false; self.successors.len()];
        let mut pending = vec![(start_block, start_position)];
        let mut reached_work = false;
        let mut interruption_count = 0usize;
        let mut interruptions = Vec::new();
        while let Some((block, first)) = pending.pop() {
            if block == refusal.0 as usize || seen[block] {
                continue;
            }
            seen[block] = true;
            let source = &self.function.blocks[block];
            for (position, row) in source
                .instructions
                .iter()
                .chain(std::iter::once(
                    selected_instructions::terminator_instruction(&source.terminator),
                ))
                .enumerate()
                .skip(first)
            {
                if row.provenance.operations.as_slice() != [self.operation]
                    || !row.provenance.edges.is_empty()
                {
                    // Allocation can insert private stack traffic and register
                    // copies. Empty attribution alone is not permission: the
                    // exact access or effect-free copy must be checked.
                    let bookkeeping = if row.provenance == Default::default() {
                        expressions.allocation_transport(row)
                    } else {
                        Ok(false)
                    };
                    if matches!(&bookkeeping, Ok(true)) {
                        continue;
                    }
                    interruption_count += 1;
                    if interruptions.len() < 12 {
                        interruptions.push(format!(
                            "block={:?}, position={position}, block origin={:?}, instruction={:?}, kind={:?}, provenance={:?}, operands={:?}, registers={:?}, allocation transport check={bookkeeping:?}",
                            source.id, source.origin, row.id, row.kind, row.provenance,
                            row.operands, row.operands.iter().map(|operand|
                                self.function.virtual_register(operand.virtual_register)).collect::<Vec<_>>(),
                        ));
                    }
                }
                if block == work_block && position == work_position {
                    reached_work = true;
                    break;
                }
            }
            if block != work_block {
                pending.extend(self.successors[block].iter().map(|next| (*next, 0)));
            }
        }
        if interruption_count != 0 {
            return Err(Error::InstallationInterval {
                machine: self.function.machine,
                operation: self.operation,
                detail: format!(
                    "offending rows={interruption_count}; Validate={start:?}, Work={work:?}, refusal={refusal:?}; {}",
                    interruptions.join("; ")
                ),
            });
        }
        if !reached_work {
            return Err(Error::Mismatch(
                "installation validation cannot reach its work call",
            ));
        }
        Ok(())
    }
    fn reachable_without(&self, destination: usize, removed: (usize, usize)) -> bool {
        let mut seen = vec![false; self.successors.len()];
        let mut pending = vec![self.function.entry_block.0 as usize];
        while let Some(block) = pending.pop() {
            if seen[block] {
                continue;
            }
            if block == destination {
                return true;
            }
            seen[block] = true;
            for &next in &self.successors[block] {
                if (block, next) != removed {
                    pending.push(next);
                }
            }
        }
        false
    }
    pub(super) fn predicate(
        &self,
        block: usize,
        expressions: &mut Expressions<'_>,
    ) -> Result<Option<(Expr, usize, usize)>, Error> {
        let row = &self.function.blocks[block];
        if !selected_instructions::terminator_instruction(&row.terminator)
            .provenance
            .operations
            .contains(&self.operation)
        {
            return Ok(None);
        }
        match &row.terminator {
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => {
                let compare = row
                    .instructions
                    .last()
                    .ok_or(Error::Mismatch("installation branch has no predicate"))?;
                if compare.kind != Kind::CompareI64Zero {
                    return Err(Error::Mismatch(
                        "installation branch does not test its recorded Boolean",
                    ));
                }
                Ok(Some((
                    expressions.input(compare, 0)?,
                    when_nonzero.block.0 as usize,
                    when_zero.block.0 as usize,
                )))
            }
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            } => {
                let compare = row.instructions.last().ok_or(Error::Mismatch(
                    "installation ordered branch has no comparison",
                ))?;
                if compare.kind != Kind::CompareI64 {
                    return Err(Error::Mismatch(
                        "installation ordered branch lost its operands",
                    ));
                }
                Ok(Some((
                    Expr::binary(
                        Binary::Less,
                        expressions.input(compare, 0)?,
                        expressions.input(compare, 1)?,
                    ),
                    when_less.block.0 as usize,
                    when_not_less.block.0 as usize,
                )))
            }
            _ => Ok(None),
        }
    }
    pub(super) fn require_guard(
        &self,
        expressions: &mut Expressions<'_>,
        expected: &Expr,
        before: SelectedInstructionId,
        refusal: SelectedBlockId,
    ) -> Result<Expr, Error> {
        if *expected == Expr::constant(1) {
            return Ok(expected.clone());
        }
        let (sink, _) = self.position(before)?;
        for (index, block) in self.function.blocks.iter().enumerate() {
            let Some((predicate, success, failure)) = self.predicate(index, expressions)? else {
                continue;
            };
            if predicate == *expected
                && self.continuation(SelectedBlockId(failure as u32))? == refusal
                && self.precedes(
                    selected_instructions::terminator_instruction(&block.terminator).id,
                    before,
                )?
                && !self.reachable_without(sink, (index, success))
            {
                return Ok(predicate);
            }
        }
        let mut count = 0usize;
        let mut candidates = Vec::new();
        for (index, block) in self.function.blocks.iter().enumerate() {
            let Some((predicate, success, failure)) = self.predicate(index, expressions)? else {
                continue;
            };
            count += 1;
            if candidates.len() < 48 {
                let branch = selected_instructions::terminator_instruction(&block.terminator).id;
                candidates.push(format!(
                    "block={index}, branch={branch:?}, success={success}, failure={failure}, continuation={:?}, expression_matches={}, precedes={}, bypass={}, predicate={predicate:?}",
                    self.continuation(SelectedBlockId(failure as u32))?,
                    predicate == *expected,
                    self.precedes(branch, before)?,
                    self.reachable_without(sink, (index, success)),
                ));
                if predicate == *expected {
                    let destination = &self.function.blocks[failure];
                    candidates.push(format!(
                        "matching failure block: origin={:?}, instructions={}, first instructions={:?}, terminator={:?}",
                        destination.origin, destination.instructions.len(),
                        &destination.instructions[..destination.instructions.len().min(8)],
                        destination.terminator,
                    ));
                }
            }
        }
        Err(Error::InstallationGuard {
            machine: self.function.machine,
            operation: self.operation,
            detail: format!(
                "before={before:?}, refusal={refusal:?}, expected={expected:?}, predicates={count}; {}",
                candidates.join("; ")
            ),
        })
    }
    /// Layout can insert an empty jump before a shared continuation. Follow
    /// only that operation's control-only route, never a source edge or a block
    /// that computes, reads, writes, transfers values, or settles obligations.
    pub(super) fn continuation(
        &self,
        mut block: SelectedBlockId,
    ) -> Result<SelectedBlockId, Error> {
        // A jump reads and replaces RIP. No other implicit state may change.
        let instruction_pointer = &isa_x86_64::validated_x86_64_physical_register_model()
            .model()
            .view_named("rip")
            .ok_or(Error::Mismatch(
                "installation target has no instruction-pointer view",
            ))?
            .units;
        let mut seen = vec![false; self.function.blocks.len()];
        loop {
            let source = self
                .function
                .blocks
                .get(block.0 as usize)
                .ok_or(Error::Mismatch(
                    "installation continuation leaves its function",
                ))?;
            if std::mem::replace(&mut seen[block.0 as usize], true) {
                return Err(Error::Mismatch(
                    "installation continuation contains a cycle",
                ));
            }
            let SelectedTerminator::Jump {
                instruction,
                successor,
            } = &source.terminator
            else {
                return Ok(block);
            };
            if !source.instructions.is_empty()
                || !matches!(source.origin, selected_instructions::SelectedBlockOrigin::Operation { source, operation }
                    if operation == self.operation && source == successor.source_target)
                || instruction.kind != Kind::Jump
                || !instruction.operands.is_empty()
                || instruction.implicit_uses != *instruction_pointer
                || instruction.implicit_defs != *instruction_pointer
                || !instruction.clobbers.is_empty()
                || instruction.provenance.operations.as_slice() != [self.operation]
                || !instruction.provenance.values.is_empty()
                || !instruction.provenance.edges.is_empty()
                || !instruction.provenance.obligations.is_empty()
                || !instruction.provenance.fuel.is_empty()
                || successor.role
                    != (selected_instructions::SelectedSuccessorRole::OperationContinuation {
                        operation: self.operation,
                    })
                || successor.psi_edge.is_some()
                || !successor.bindings.is_empty()
                || !successor.structural_bindings.is_empty()
                || successor.structural_case.is_some()
                || !successor.fuel.is_empty()
            {
                return Ok(block);
            }
            block = successor.block;
        }
    }
    pub(super) fn only_success_path(
        &self,
        first: SelectedInstructionId,
        last: SelectedInstructionId,
        refusal: SelectedBlockId,
    ) -> Result<(), Error> {
        let (start, _) = self.position(first)?;
        let (end, _) = self.position(last)?;
        let mut seen = vec![false; self.successors.len()];
        let mut block = start;
        while block != end {
            if std::mem::replace(&mut seen[block], true) {
                return Err(Error::Mismatch(
                    "installation cycles after publication without reaching its retained work",
                ));
            }
            if block == refusal.0 as usize || self.successors[block].len() != 1 {
                return Err(Error::Mismatch(
                    "installation can refuse or branch after publication begins",
                ));
            }
            block = self.successors[block][0];
        }
        if !self.precedes(first, last)? {
            return Err(Error::Mismatch(
                "installation publication does not reach its retained work",
            ));
        }
        Ok(())
    }
}

impl Graph<'_> {
    /// Prove a single bounded loop from its actual header and unique backedge.
    /// Reaching publication must cross the loop's false edge; reaching its
    /// backedge must cross the true edge and perform the prescribed update.
    pub(super) fn counted_loop(
        &self,
        expressions: &mut Expressions<'_>,
        condition: &Expr,
        initialize: SelectedInstructionId,
        advance: SelectedInstructionId,
        publication: SelectedInstructionId,
    ) -> Result<usize, Error> {
        let (advance_block, _) = self.position(advance)?;
        let (publication_block, _) = self.position(publication)?;
        for (header, block) in self.function.blocks.iter().enumerate() {
            let Some((predicate, body, exit)) = self.predicate(header, expressions)? else {
                continue;
            };
            if predicate != *condition {
                continue;
            }
            // The loop must reload its counter in the header on every visit.
            let super::Word::Binary(Binary::Less, expected_counter, _) = condition.word() else {
                return Err(Error::Mismatch(
                    "installation loop condition is not an ordered bound",
                ));
            };
            let super::Word::Binary(Binary::Less, actual_counter, _) = predicate.word() else {
                continue;
            };
            let super::Word::Load {
                instruction: Some(observed),
                ..
            } = actual_counter.word()
            else {
                continue;
            };
            if actual_counter != expected_counter || self.position(*observed)?.0 != header {
                continue;
            }
            let SelectedTerminator::Jump { successor, .. } =
                &self.function.blocks[advance_block].terminator
            else {
                continue;
            };
            if successor.block.0 as usize != header
                || !self.precedes(
                    initialize,
                    selected_instructions::terminator_instruction(&block.terminator).id,
                )?
                || !self.precedes(
                    selected_instructions::terminator_instruction(&block.terminator).id,
                    advance,
                )?
                || self.reachable_without(advance_block, (header, body))
                || self.reachable_without(publication_block, (header, exit))
            {
                continue;
            }
            let predecessors = self
                .successors
                .iter()
                .enumerate()
                .filter(|(_, edges)| edges.contains(&header))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let (initial_block, _) = self.position(initialize)?;
            if predecessors.len() != 2
                || !predecessors.contains(&advance_block)
                || !predecessors.contains(&initial_block)
            {
                continue;
            }
            return Ok(header);
        }
        Err(Error::Mismatch(
            "installation loop changed its zero-entry, condition, update or publication exit",
        ))
    }
    fn reaches_before(
        &self,
        start: usize,
        target: usize,
        stop: usize,
        removed: Option<usize>,
    ) -> bool {
        let mut seen = vec![false; self.successors.len()];
        let mut pending = vec![start];
        while let Some(block) = pending.pop() {
            if block == stop || Some(block) == removed || seen[block] {
                continue;
            }
            if block == target {
                return true;
            }
            seen[block] = true;
            pending.extend(self.successors[block].iter().copied());
        }
        false
    }
    pub(super) fn selected_iteration(
        &self,
        expressions: &mut Expressions<'_>,
        predicate: &Expr,
        header: usize,
        write: SelectedInstructionId,
        advance: SelectedInstructionId,
    ) -> Result<Expr, Error> {
        let (write_block, _) = self.position(write)?;
        let (advance_block, _) = self.position(advance)?;
        let mut found = None;
        for (branch, block) in self.function.blocks.iter().enumerate() {
            let Some((actual, yes, no)) = self.predicate(branch, expressions)? else {
                continue;
            };
            if actual != *predicate
                || !self.block_dominates(
                    SelectedBlockId(header as u32),
                    selected_instructions::terminator_instruction(&block.terminator).id,
                )?
                || !self.precedes(
                    selected_instructions::terminator_instruction(&block.terminator).id,
                    write,
                )?
            {
                continue;
            }
            if self.reaches_before(yes, advance_block, header, Some(write_block))
                || self.reaches_before(no, write_block, header, None)
                || !self.reaches_before(yes, write_block, header, None)
            {
                continue;
            }
            if found.replace(actual).is_some() {
                return Err(Error::Mismatch(
                    "installation iteration has multiple matching selectors",
                ));
            }
        }
        found.ok_or(Error::Mismatch(
            "installation stack publication is not confined to the exact matching iteration",
        ))
    }
}

impl Graph<'_> {
    pub(super) fn require_edge(
        &self,
        expressions: &mut Expressions<'_>,
        expected: &Expr,
        before: SelectedInstructionId,
    ) -> Result<usize, Error> {
        let (sink, _) = self.position(before)?;
        for (index, block) in self.function.blocks.iter().enumerate() {
            let Some((predicate, success, _)) = self.predicate(index, expressions)? else {
                continue;
            };
            if predicate == *expected
                && self.precedes(
                    selected_instructions::terminator_instruction(&block.terminator).id,
                    before,
                )?
                && !self.reachable_without(sink, (index, success))
            {
                return Ok(index);
            }
        }
        Err(Error::Mismatch(
            "installation read is not dominated by its actual index bound",
        ))
    }
}

impl Graph<'_> {
    /// Counter cells are reused by consecutive scans. A guard belongs to the
    /// current scan only inside this exact initialization-to-next-phase window;
    /// expression equality deliberately does not identify a load incarnation.
    pub(super) fn require_guard_between(
        &self,
        expressions: &mut Expressions<'_>,
        expected: &Expr,
        after: SelectedInstructionId,
        before: SelectedInstructionId,
        refusal: SelectedBlockId,
    ) -> Result<Expr, Error> {
        let (sink, _) = self.position(before)?;
        let mut found = None;
        for (index, block) in self.function.blocks.iter().enumerate() {
            let Some((predicate, success, failure)) = self.predicate(index, expressions)? else {
                continue;
            };
            let branch = selected_instructions::terminator_instruction(&block.terminator).id;
            if predicate == *expected
                && self.precedes(after, branch)?
                && self.precedes(branch, before)?
                && self.continuation(SelectedBlockId(failure as u32))? == refusal
                && !self.reachable_without(sink, (index, success))
                && found.replace(predicate).is_some()
            {
                return Err(Error::Mismatch(
                    "installation phase has multiple matching guards",
                ));
            }
        }
        found.ok_or(Error::Mismatch(
            "installation phase has no exact guard before its next counter reset",
        ))
    }

    /// A matched iteration can finish without writing a root (fatal entries).
    /// Its guards must nevertheless lie on every path from the exact selected
    /// count update to this iteration's advance; failure reaches only refusal.
    pub(super) fn require_iteration_guard(
        &self,
        expressions: &mut Expressions<'_>,
        expected: &Expr,
        header: usize,
        matched: SelectedInstructionId,
        advance: SelectedInstructionId,
        refusal: SelectedBlockId,
    ) -> Result<Expr, Error> {
        let (start, _) = self.position(matched)?;
        let (sink, _) = self.position(advance)?;
        let mut found = None;
        for (index, block) in self.function.blocks.iter().enumerate() {
            let Some((predicate, success, failure)) = self.predicate(index, expressions)? else {
                continue;
            };
            let branch = selected_instructions::terminator_instruction(&block.terminator).id;
            if predicate != *expected
                || !self.precedes(matched, branch)?
                || self.continuation(SelectedBlockId(failure as u32))? != refusal
                || !self.reaches_before(start, index, header, None)
                || self.reaches_before(start, sink, header, Some(index))
                || !self.reaches_before(success, sink, header, None)
                || self.reaches_before(failure, sink, header, None)
            {
                continue;
            }
            if found.replace(predicate).is_some() {
                return Err(Error::Mismatch(
                    "installation matched iteration has ambiguous guards",
                ));
            }
        }
        found.ok_or(Error::Mismatch(
            "installation matched iteration can bypass its required guard",
        ))
    }
}
