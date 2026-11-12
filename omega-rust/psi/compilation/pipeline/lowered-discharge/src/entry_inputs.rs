/*
A private helper may rely on what every caller establishes when its input
contract was omitted. For example, a helper that subtracts one can be called
under `n > 0` without repeating that condition in its header. A written
contract, including `requires true`, remains the whole input promise.

Describe each arrival in the callee's formal values, join those descriptions,
and make that predicate an ordinary entry requirement. Every call then owes
it to the same prover and kernel as an authored requirement. Describing an
arrival is inference, never evidence: the final discharge checks all calls,
body operations and joins before anything can be accepted.

Only facts about exact scalar actuals cross here. Unrelated caller values or
storage cannot become callee identities. A written boundary stops caller
inference, while its own declared facts can justify calls within its body.
*/

use proof_admission::check_value_equality_denotation;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IdTable, InvocationSite, MachineId, PlaceId, Proposition,
    ScalarTerm, ScalarType, StructuralCaseId, ValueId,
};
use std::collections::VecDeque;
use structure_elaboration::{Elaboration, SourceOrigin};
use terminal_check_tables::edges::Callee;
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::values::Justification;
use terminal_check_tables::{
    Checked, Construct, InvocationId, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{OperationKind, TerminalMachine, Terminator};

use crate::{
    Bindings, Knowledge, Unbound, graph::Graph, joins, prepare, refusal, scalar_actuals,
    substitute, terminal_machine,
};

/// Callers precede helpers. The lowering of terminal call components already
/// turns their internal calls into block arrivals; an unresolved dependency
/// here supplies no inferred condition and still faces ordinary discharge.
pub(crate) fn infer(elaboration: &mut Elaboration, knowledge: Knowledge) -> Checked<()> {
    if elaboration.private_inputs.is_empty() {
        return Ok(());
    }
    let sites = CallSites::new(elaboration)?;
    let mut waiting = sites.incoming.map(|_, callers| callers.len());
    let mut bodies = elaboration.tables.machines.map(|_, _| true);
    for requirement in &elaboration.requirements {
        *machine_row_mut(&mut bodies, *requirement, *requirement)? = false;
    }
    let mut ready: VecDeque<_> = bodies
        .iter()
        .filter_map(|(id, body)| {
            (*body && (sites.private.get(id) == Some(&false) || waiting.get(id) == Some(&0)))
                .then_some(id)
        })
        .collect();
    let mut incoming: IdTable<MachineId, Vec<Proposition>> =
        elaboration.tables.machines.map(|_, _| Vec::new());
    while let Some(id) = ready.pop_front() {
        if sites.private.get(id) == Some(&true) {
            let arrivals =
                terminal_check_tables::row(&incoming, id, id, Construct::Machine)?.clone();
            if !arrivals.is_empty() {
                let predicate = canonical(joins::connective(arrivals, false), id)?;
                if predicate != Proposition::Truth {
                    require(elaboration, id, predicate, &sites)?;
                }
            }
        }
        describe_calls(elaboration, id, knowledge, &sites, &mut incoming)?;
        for &site in terminal_check_tables::row(&sites.outgoing, id, id, Construct::Machine)? {
            let callee = sites.arrivals[site].callee;
            let remaining = machine_row_mut(&mut waiting, callee, id)?;
            *remaining -= 1;
            if *remaining == 0 && bodies.get(callee) == Some(&true) {
                ready.push_back(callee);
            }
        }
    }
    Ok(())
}

fn describe_calls(
    elaboration: &Elaboration,
    id: MachineId,
    knowledge: Knowledge,
    sites: &CallSites,
    incoming: &mut IdTable<MachineId, Vec<Proposition>>,
) -> Checked<()> {
    let original =
        terminal_check_tables::row(&elaboration.tables.machines, id, id, Construct::Machine)?;
    let outgoing = terminal_check_tables::row(&sites.outgoing, id, id, Construct::Machine)?;
    if outgoing.is_empty() {
        return Ok(());
    }
    // One caller's construction scratch; the final evidence tables remain
    // untouched. No typed trees or completed proof products are copied.
    let mut tables = original.clone();
    let prepared = prepare(
        &elaboration.module.machines,
        &elaboration.module.structural_types,
        &elaboration.domains,
        id,
        &mut tables,
        knowledge,
    )?;
    for &index in outgoing {
        let site = &sites.arrivals[index];
        let callee = site.callee;
        let call = terminal_check_tables::row(
            &tables.invocations,
            site.invocation,
            id,
            Construct::Invocation(site.invocation),
        )?;
        let actuals = scalar_actuals(&call.actuals);
        let target = terminal_machine(&elaboration.module.machines, callee, id)?;
        let predicate = describe(
            &tables,
            &prepared.graph,
            &prepared.context,
            id,
            site.before,
            target,
            &actuals,
        )?;
        incoming
            .get_mut(callee)
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Machine,
                    "a private callee has no arrival roster",
                )
            })?
            .push(predicate);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum CallPosition {
    Operation { block: usize, operation: usize },
    NoNormal { block: usize },
}

struct Arrival {
    caller: MachineId,
    callee: MachineId,
    invocation: InvocationId,
    before: ProgramPoint,
    position: CallPosition,
}

/// Resolve the call graph and physical call positions once. Updating an
/// inferred contract follows its own arrivals, not every machine again.
/// Inference appends clauses and obligations; it never moves a block, an
/// operation or an invocation, so these positions remain valid throughout.
struct CallSites {
    private: IdTable<MachineId, bool>,
    arrivals: Vec<Arrival>,
    incoming: IdTable<MachineId, Vec<usize>>,
    outgoing: IdTable<MachineId, Vec<usize>>,
}

impl CallSites {
    fn new(elaboration: &Elaboration) -> Checked<Self> {
        let mut private = elaboration.tables.machines.map(|_, _| false);
        for id in &elaboration.private_inputs {
            *machine_row_mut(&mut private, *id, *id)? = true;
        }
        let mut result = Self {
            private,
            arrivals: Vec::new(),
            incoming: elaboration.tables.machines.map(|_, _| Vec::new()),
            outgoing: elaboration.tables.machines.map(|_, _| Vec::new()),
        };
        for (id, tables) in elaboration.tables.machines.iter() {
            if !tables.invocations.values().any(|call| {
                matches!(call.callee, Callee::Machine(callee) if result.private.get(callee) == Some(&true))
            }) {
                continue;
            }
            let machine = terminal_machine(&elaboration.module.machines, id, id)?;
            let mut operations: Vec<Option<(CallPosition, ProgramPoint)>> = Vec::new();
            for (block_index, block) in machine.blocks.iter().enumerate() {
                let mut before = ProgramPoint::BlockEntry(block.id);
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    let index = usize::try_from(operation.id.get() - 1).map_err(|_| {
                        refusal(
                            RefusalReason::ForeignIdentity,
                            id,
                            Construct::Operation(operation.id),
                            "operation position exceeds usize",
                        )
                    })?;
                    if operations.len() <= index {
                        operations.resize(index + 1, None);
                    }
                    operations[index] = Some((
                        CallPosition::Operation {
                            block: block_index,
                            operation: operation_index,
                        },
                        before,
                    ));
                    before = ProgramPoint::AfterOperation(operation.id);
                }
            }
            for (invocation, call) in tables.invocations.iter() {
                let Callee::Machine(callee) = call.callee else {
                    continue;
                };
                if result.private.get(callee) != Some(&true) {
                    continue;
                }
                let foreign = || {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        Construct::Invocation(invocation),
                        "a private invocation has no call position",
                    )
                };
                let (position, before) = match call.site {
                    InvocationSite::Operation(operation) => usize::try_from(operation.get() - 1)
                        .ok()
                        .and_then(|index| operations.get(index))
                        .copied()
                        .flatten()
                        .ok_or_else(foreign)?,
                    InvocationSite::InvokeNoNormal(edge) => {
                        let row = terminal_check_tables::row(
                            &tables.edges,
                            edge,
                            id,
                            Construct::Edge(edge),
                        )?;
                        let index = usize::try_from(row.source.get() - 1).map_err(|_| foreign())?;
                        let block = machine
                            .blocks
                            .get(index)
                            .filter(|block| block.id == row.source)
                            .ok_or_else(foreign)?;
                        if !matches!(&block.terminator, Terminator::InvokeNoNormal { edge: actual, .. } if *actual == edge)
                        {
                            return Err(foreign());
                        }
                        (
                            CallPosition::NoNormal { block: index },
                            block
                                .operations
                                .last()
                                .map_or(ProgramPoint::BlockEntry(block.id), |operation| {
                                    ProgramPoint::AfterOperation(operation.id)
                                }),
                        )
                    }
                    _ => return Err(foreign()),
                };
                let index = result.arrivals.len();
                result.arrivals.push(Arrival {
                    caller: id,
                    callee,
                    invocation,
                    before,
                    position,
                });
                machine_row_mut(&mut result.incoming, callee, id)?.push(index);
                machine_row_mut(&mut result.outgoing, id, id)?.push(index);
            }
        }
        Ok(result)
    }
}

fn machine_row_mut<T>(
    table: &mut IdTable<MachineId, T>,
    id: MachineId,
    from: MachineId,
) -> Checked<&mut T> {
    table.get_mut(id).ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            from,
            Construct::Machine,
            "machine identity is past the end of an entry-inference table",
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn describe(
    tables: &MachineTables,
    graph: &Graph,
    context: &semantic_vocabulary::PropositionContext,
    id: MachineId,
    point: ProgramPoint,
    target: &TerminalMachine,
    actuals: &[ValueId],
) -> Checked<Proposition> {
    let mut available = Vec::new();
    for (_, row) in tables.knowledge.iter() {
        if graph.dominates(row.available_from, point)? {
            let proposition = terminal_check_tables::row(
                &tables.propositions,
                row.proposition,
                id,
                Construct::Machine,
            )?;
            available.push((row.justification, proposition));
        }
    }
    // Use the same literal/Boolean definitions as join inference. Numeric
    // operation results keep their own identities, rather than becoming
    // invented equations about the callee's inputs.
    let equations: Vec<&Proposition> = available
        .iter()
        .filter_map(|(why, proposition)| {
            (matches!(why, Justification::OperationSemantics(_)) && matches!(proposition,
            Proposition::Equal(subject @ ScalarTerm::Value { .. }, definition)
                if subject.scalar_type() == ScalarType::Boolean
                    || matches!(definition, ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_))
        )).then_some(*proposition)
        })
        .collect();
    let definitions: Vec<_> = equations
        .iter()
        .filter_map(|equation| match equation {
            Proposition::Equal(ScalarTerm::Value { id, .. }, definition) => Some((*id, definition)),
            _ => None,
        })
        .collect();
    let mut bindings = InputBindings { actuals, target };
    let mut parts = Vec::new();
    for (formal, actual) in target.parameters.iter().zip(actuals) {
        let definition = joins::unfold(
            &ScalarTerm::value(*actual, formal.scalar_type),
            &definitions,
            0,
        );
        let equality = Proposition::Equal(
            ScalarTerm::value(formal.id, formal.scalar_type),
            match crate::bound(&definition, &mut bindings) {
                Ok(term) => term,
                Err(_) => continue,
            },
        );
        if !matches!(&equality, Proposition::Equal(left, right) if left == right) {
            parts.push(equality);
        }
    }
    for (why, proposition) in available {
        if matches!(why, Justification::OperationSemantics(_)) {
            continue;
        }
        let denotation =
            check_value_equality_denotation(context, proposition, equations.iter().copied())
                .unwrap_or_else(|_| proposition.clone());
        if let Ok(stated) = substitute(&denotation, &mut bindings) {
            parts.push(stated);
        }
    }
    Ok(joins::connective(parts, true))
}

struct InputBindings<'a> {
    actuals: &'a [ValueId],
    target: &'a TerminalMachine,
}

impl Bindings for InputBindings<'_> {
    fn value(&mut self, value: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Unbound> {
        self.actuals
            .iter()
            .position(|actual| *actual == value)
            .and_then(|position| self.target.parameters.get(position))
            .filter(|formal| formal.scalar_type == scalar_type)
            .map(|formal| ScalarTerm::value(formal.id, scalar_type))
            .ok_or(Unbound::Unproved("a caller value is not an actual input"))
    }
    fn field(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        Err(Unbound::Unproved(
            "a caller storage path is not a callee input value",
        ))
    }
    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Unbound> {
        Err(Unbound::Unproved(
            "a caller storage case is not a callee input value",
        ))
    }
}

fn canonical(proposition: Proposition, id: MachineId) -> Checked<Proposition> {
    let encode_error = |_| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            Construct::Machine,
            "an inferred input predicate is not encodable",
        )
    };
    Ok(match proposition {
        Proposition::Equal(mut left, mut right) => {
            if terminal_codec::canonical_scalar_term_order_key(&left).map_err(encode_error)?
                > terminal_codec::canonical_scalar_term_order_key(&right).map_err(encode_error)?
            {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::Equal(left, right)
        }
        Proposition::Conjunction(parts) => canonical_parts(parts, true, id)?,
        Proposition::Disjunction(parts) => canonical_parts(parts, false, id)?,
        other => other,
    })
}

fn canonical_parts(
    parts: Vec<Proposition>,
    conjunction: bool,
    id: MachineId,
) -> Checked<Proposition> {
    let mut keyed = Vec::with_capacity(parts.len());
    for part in parts {
        let part = canonical(part, id)?;
        keyed.push((
            terminal_codec::canonical_proposition_order_key(&part).map_err(|_| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Machine,
                    "an inferred input predicate is not encodable",
                )
            })?,
            part,
        ));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    keyed.dedup_by(|left, right| left.0 == right.0);
    let mut parts: Vec<_> = keyed.into_iter().map(|(_, part)| part).collect();
    Ok(match parts.len() {
        0 if conjunction => Proposition::Truth,
        0 => Proposition::Falsehood,
        1 => parts.pop().unwrap_or(Proposition::Truth),
        _ if conjunction => Proposition::Conjunction(parts),
        _ => Proposition::Disjunction(parts),
    })
}

/// Add the inferred predicate to the contract and mint its obligation at
/// every real call. Existing clauses retain their obligation identities when
/// canonical ordering moves their positions.
fn require(
    elaboration: &mut Elaboration,
    callee: MachineId,
    predicate: Proposition,
    sites: &CallSites,
) -> Checked<()> {
    let machine = usize::try_from(callee.get() - 1).map_err(|_| {
        refusal(
            RefusalReason::ForeignIdentity,
            callee,
            Construct::Machine,
            "callee position exceeds usize",
        )
    })?;
    let target = elaboration
        .module
        .machines
        .get_mut(machine)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                callee,
                Construct::Machine,
                "callee is past the module",
            )
        })?;
    let previous = target.contract.requires.clone();
    let mut clauses = previous.clone();
    clauses.push(predicate);
    let mut keyed = Vec::with_capacity(clauses.len());
    for clause in clauses {
        keyed.push((
            terminal_codec::canonical_proposition_order_key(&clause).map_err(|_| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    callee,
                    Construct::Machine,
                    "input clause is not encodable",
                )
            })?,
            clause,
        ));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    keyed.dedup_by(|left, right| left.0 == right.0);
    let clauses: Vec<_> = keyed.into_iter().map(|(_, clause)| clause).collect();
    target.contract.requires.clone_from(&clauses);
    // Synthesized clauses have no authored text. Keep diagnostics aligned
    // with the canonical positions of any existing authored clauses.
    if let Some(names) = elaboration.names.get_mut(callee) {
        let old = names.requires.clone();
        names.requires = clauses
            .iter()
            .map(|clause| {
                previous
                    .iter()
                    .position(|old| old == clause)
                    .and_then(|position| old.get(position))
                    .cloned()
                    .unwrap_or_else(|| {
                        structure_elaboration::ClauseText(vec![structure_elaboration::Piece::Text(
                            String::from("inferred input condition"),
                        )])
                    })
            })
            .collect();
    }
    for &index in terminal_check_tables::row(&sites.incoming, callee, callee, Construct::Machine)? {
        let site = &sites.arrivals[index];
        let caller_index = usize::try_from(site.caller.get() - 1).map_err(|_| {
            refusal(
                RefusalReason::ForeignIdentity,
                callee,
                Construct::Machine,
                "caller position exceeds usize",
            )
        })?;
        let caller = elaboration
            .module
            .machines
            .get_mut(caller_index)
            .filter(|caller| caller.id == site.caller)
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    callee,
                    Construct::Machine,
                    "caller is past the module",
                )
            })?;
        let tables = elaboration
            .tables
            .machines
            .get_mut(caller.id)
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    caller.id,
                    Construct::Machine,
                    "caller has no tables",
                )
            })?;
        let sources = elaboration.sources.get_mut(caller.id).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                caller.id,
                Construct::Machine,
                "caller has no source correspondence",
            )
        })?;
        let invocation = site.invocation;
        let call = terminal_check_tables::row(
            &tables.invocations,
            invocation,
            caller.id,
            Construct::Invocation(invocation),
        )?
        .clone();
        if call.callee != Callee::Machine(callee) || call.requirements.len() != previous.len() {
            return Err(refusal(
                RefusalReason::ForeignIdentity,
                caller.id,
                Construct::Invocation(invocation),
                "a planned private arrival does not match its invocation row",
            ));
        }
        let point = site.before;
        let source: SourceOrigin = match call.site {
            InvocationSite::Operation(operation) => *terminal_check_tables::row(
                &sources.operations,
                operation,
                caller.id,
                Construct::Operation(operation),
            )?,
            InvocationSite::InvokeNoNormal(edge) => {
                *terminal_check_tables::row(&sources.edges, edge, caller.id, Construct::Edge(edge))?
            }
            _ => {
                return Err(refusal(
                    RefusalReason::ForeignIdentity,
                    caller.id,
                    Construct::Invocation(invocation),
                    "invocation site is not a direct call",
                ));
            }
        };
        let mut requirements = Vec::with_capacity(clauses.len());
        for (position, clause) in clauses.iter().enumerate() {
            let clause_position = crate::position(position, caller.id)?;
            let kind = ObligationKind::CallRequirement {
                invocation,
                clause: clause_position,
            };
            let obligation = if let Some(old) = previous.iter().position(|old| old == clause) {
                let obligation = *call.requirements.get(old).ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        caller.id,
                        Construct::Invocation(invocation),
                        "call lacks an existing input obligation",
                    )
                })?;
                let row = terminal_check_tables::row(
                    &tables.obligations,
                    obligation,
                    caller.id,
                    Construct::Obligation(obligation),
                )?;
                if row.kind
                    != (ObligationKind::CallRequirement {
                        invocation,
                        clause: crate::position(old, caller.id)?,
                    })
                    || row.created_at != point
                {
                    return Err(refusal(
                        RefusalReason::ForeignIdentity,
                        caller.id,
                        Construct::Obligation(obligation),
                        "an existing input obligation does not match its call",
                    ));
                }
                tables
                    .obligations
                    .get_mut(obligation)
                    .expect("the row was checked")
                    .kind = kind;
                obligation
            } else {
                let obligation = tables.obligations.push(ObligationRow {
                    kind,
                    created_at: point,
                });
                if sources.obligations.push(source) != obligation {
                    return Err(refusal(
                        RefusalReason::ForeignIdentity,
                        caller.id,
                        Construct::Obligation(obligation),
                        "an input obligation and its source correspondence have different identities",
                    ));
                }
                obligation
            };
            requirements.push(obligation);
        }
        tables
            .invocations
            .get_mut(invocation)
            .expect("the call was read above")
            .requirements
            .clone_from(&requirements);
        match site.position {
            CallPosition::Operation { block, operation } => {
                let found = caller
                    .blocks
                    .get_mut(block)
                    .and_then(|block| block.operations.get_mut(operation))
                    .filter(|operation| call.site == InvocationSite::Operation(operation.id))
                    .ok_or_else(|| {
                        refusal(
                            RefusalReason::ForeignIdentity,
                            caller.id,
                            Construct::Invocation(invocation),
                            "a planned private arrival has no call operation",
                        )
                    })?;
                let OperationKind::Call {
                    requirement_obligations,
                    ..
                } = &mut found.kind
                else {
                    return Err(refusal(
                        RefusalReason::ForeignIdentity,
                        caller.id,
                        Construct::Invocation(invocation),
                        "a planned private arrival is not a call operation",
                    ));
                };
                requirement_obligations.clone_from(&requirements);
            }
            CallPosition::NoNormal { block } => {
                let found = caller.blocks.get_mut(block).ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        caller.id,
                        Construct::Invocation(invocation),
                        "a planned private arrival has no call block",
                    )
                })?;
                let Terminator::InvokeNoNormal {
                    edge,
                    requirement_obligations,
                    ..
                } = &mut found.terminator
                else {
                    return Err(refusal(
                        RefusalReason::ForeignIdentity,
                        caller.id,
                        Construct::Invocation(invocation),
                        "a planned private arrival is not a non-returning call",
                    ));
                };
                if call.site != InvocationSite::InvokeNoNormal(*edge) {
                    return Err(refusal(
                        RefusalReason::ForeignIdentity,
                        caller.id,
                        Construct::Invocation(invocation),
                        "a planned private arrival names another call edge",
                    ));
                }
                requirement_obligations.clone_from(&requirements);
            }
        }
    }
    Ok(())
}
