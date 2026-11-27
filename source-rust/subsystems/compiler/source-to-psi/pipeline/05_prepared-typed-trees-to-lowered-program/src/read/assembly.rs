//! Read the local edges of one assembly machine. Register names have already
//! resolved to slots. Only writes in the reached instruction stream initialize them.
use super::contracts::{ClauseScope, ResultTerm};
use super::{Program, Slot, refuse_at};
use crate::body::{ArmGuard, Expression, ExpressionKind, Statement, Target, ValueType};
use crate::source_state::expression::ExpressionNode;
use crate::source_state::statement::{
    AssemblyControlFlow, AssemblyControlPoint, AssemblyExit, AssemblyFactKind, StatementNode,
};
use crate::{SourceOrigin, Unserved, reject};
use language_core::inline_assembly::AsmRegisterView;
use semantic_vocabulary::{MachineId, ValueId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

impl Program<'_> {
    pub(super) fn assembly_machine(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        slots: &[Slot],
        graph: &AssemblyControlFlow,
        statements: &[StatementNode],
        result: Option<ValueType>,
    ) -> Checked<Vec<Statement>> {
        let typed = self.typed;
        let unsupported = |what| refuse_at(id, Unserved::Statement(what));
        let mut registers = Vec::new();
        for (position, operand) in graph
            .scratch
            .iter()
            .enumerate()
            .filter(|(_, value)| value.is_valid())
        {
            let view = AsmRegisterView::from_scratch_slot(position).ok_or_else(|| {
                unsupported("unmodeled working register at a checked assembly return")
            })?;
            let ExpressionNode::Name(path) = self.node(id, *operand)? else {
                return Err(unsupported("assembly register has no resolved slot"));
            };
            registers.push((self.slot_of(id, slots, path)?, view));
        }
        let mut initialized = Vec::new();
        let blocks = typed.statement_table.assembly_blocks(graph.blocks);
        let mut body = Vec::new();
        for (index, block) in blocks.iter().enumerate() {
            let start = block.start as usize;
            let local = statements
                .get(start..start + block.count as usize)
                .ok_or_else(|| unsupported("assembly block outside its instruction stream"))?;
            let Some((StatementNode::Assembly(AssemblyControlPoint::Label(label)), local)) =
                local.split_first()
            else {
                return Err(unsupported("assembly block has no local entry"));
            };
            if *label != graph.blocks.handle_at(index as u32) {
                return Err(unsupported("assembly block names another entry"));
            }
            let default_site = self
                .typed
                .symbols
                .symbol_provenance_source_span(owner)
                .map(SourceOrigin::of)
                .unwrap_or(SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent));
            let facts = local.iter().take_while(|row| matches!(row, StatementNode::AssemblyFact(fact) if fact.kind == AssemblyFactKind::Requires)).count();
            if index != 0 {
                // Put the initialized formals first so their ordinals are exactly the
                // parameter list used by the eventual physical edge. Keep the other
                // names in scope to diagnose an actual uninitialized register read.
                let working = initialized.iter().copied().chain(
                    registers
                        .iter()
                        .map(|(position, _)| *position)
                        .filter(|position| !initialized.contains(position)),
                );
                let available = working
                    .map(|position: usize| slots[position].clone())
                    .collect::<Vec<_>>();
                let values = (0..available.len())
                    .map(|n| ValueId::new(n as u64 + 1))
                    .collect::<Vec<_>>();
                let scope = ClauseScope {
                    slots: &available,
                    values: &values,
                    bound: &[],
                    owner,
                    result: ResultTerm::Absent(
                        "a block precondition cannot name the machine result",
                    ),
                    subject: None,
                    formal_fields: false,
                    logical: None,
                };
                let mut requires = Vec::new();
                let mut source = default_site;
                for row in &local[..facts] {
                    let StatementNode::AssemblyFact(fact) = row else {
                        unreachable!()
                    };
                    let fact = typed.proof_facts.get(fact.fact);
                    let expression = self.fact(id, fact)?;
                    source = SourceOrigin::of(typed.expression_table.source_span(expression));
                    let predicate = self.clause(id, &scope, expression)?;
                    let mut initialized_operands = true;
                    let complete = predicate.visit_value_ids(|value| {
                        initialized_operands &= values[..initialized.len()].contains(&value)
                    });
                    if !complete || !initialized_operands {
                        return Err(reject(
                            id,
                            source,
                            "assembly block entry requires an initialized register on its incoming edge",
                        ));
                    }
                    requires.push(predicate);
                }
                body.push(Statement::AssemblyAdjacent {
                    slots: initialized.clone(),
                    requires,
                    label: block.label.as_str().to_owned(),
                    source,
                });
            } else if facts > 0 {
                return Err(unsupported(
                    "assembly entry requires an established incoming register state",
                ));
            }
            for row in &local[facts..] {
                let StatementNode::Assignment(assignment) = row else {
                    return Err(unsupported(
                        "assembly instruction or block guarantee without checked semantics",
                    ));
                };
                let statement = self.assignment(id, slots, assignment)?;
                let Statement::Bind { slot, value } = &statement else {
                    return Err(unsupported(
                        "assembly move into storage without a checked placement",
                    ));
                };
                if !registers.iter().any(|(working, _)| working == slot) {
                    return Err(unsupported(
                        "assembly move without a modeled register destination",
                    ));
                }
                match value.kind {
                    ExpressionKind::Integer(_) => {}
                    ExpressionKind::Slot(from) if initialized.contains(&from) => {}
                    ExpressionKind::Slot(_) => {
                        return Err(reject(
                            id,
                            value.source,
                            "assembly reads a register before an instruction or entry binding initialized it",
                        ));
                    }
                    _ => {
                        return Err(unsupported(
                            "assembly move without an established scalar operand",
                        ));
                    }
                }
                if !initialized.contains(slot) {
                    initialized.push(*slot);
                }
                body.push(statement);
            }
            match block.exit {
                AssemblyExit::Adjacent if index + 1 < blocks.len() => {}
                AssemblyExit::Return if index + 1 == blocks.len() => {
                    let [slot] = initialized.as_slice() else {
                        return Err(reject(
                            id,
                            default_site,
                            "checked assembly return needs one initialized scalar register for its result",
                        ));
                    };
                    let view = registers
                        .iter()
                        .find(|(position, _)| position == slot)
                        .unwrap()
                        .1;
                    let value_type = slots[*slot].value_type;
                    if !self.agrees(Some(value_type), result) {
                        return Err(reject(
                            id,
                            default_site,
                            "the initialized register's scalar type does not agree with the assembly machine's declared result",
                        ));
                    }
                    let value = Expression {
                        call_context: None,
                        kind: ExpressionKind::Slot(*slot),
                        result,
                        source: default_site,
                    };
                    body.push(Statement::Arm {
                        guard: ArmGuard::Always,
                        target: Target::AssemblyReturn(view, value),
                        source: default_site,
                    });
                }
                _ => {
                    return Err(unsupported(
                        "assembly machine needs a checked local successor or return",
                    ));
                }
            }
        }
        Ok(body)
    }
}
