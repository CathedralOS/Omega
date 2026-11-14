/*
What a block parameter carries across a join.

A branch fact holds only on its edge, and fact_validity.md says "A fact from
one arm does not survive the join." A use after the join therefore learns
nothing about a join parameter from the arms' facts unless the join has a
checked predicate: "Scalar block invariant arrival questions come from the
semantic predicate roster and every actual incoming edge ... Every arrival
must check before the destination predicate can justify downstream
obligations" (fact_validity.md), and "Every arrival proves the predicate
under simultaneous destination-parameter to actual-argument substitution"
(control_flow.md, "Equality relations").

Lowering mints one `ArrivalInvariant` obligation per edge into a block with
parameters. The predicate is the checker's to choose; it is inferred here as
the disjunction over arrivals of what each arrival knows that the join does
not, stated in the predicate's scope (machine formals and the join's own
parameters, control_flow.md):

- for each parameter `p` and its argument `a` on that arrival, `p == a`,
  with `a` replaced by its definition through the Boolean and literal
  definitions available on the arrival edge (the same substitution
  `ValueEqualityTransport` performs);
- each fact available on the arrival edge but not at the join, transported
  through those definitions into its denotation;
- each preceding join's predicate that names a value this edge fixes to a
  literal, read with that value fixed. `a && b && c` is two joins, and the
  first's result is free again at the second; on the edge where it is true
  the first's predicate is the one alternative that produced it, and that
  is what the second join can still say.

What a formal's storage held at entry is in the scope beside the formals: it
is one value for the whole invocation, and a read no store reaches is defined
as it, so a guard over a formal's fields crosses a join as a guard over the
formals does.

A part that still names a value outside the scope is left out, which only
weakens the disjunct. Each arrival's obligation is then that predicate under
its parameter-to-argument substitution, proved by the one prover and decided
by the kernel. When some arrival cannot be proved, the predicate is replaced
by `Truth`, the predicate every arrival establishes, and the join is
reported in `MachineDischarge::weakened_joins`; nothing downstream can then
rely on it. A block on a cycle gets no inferred predicate: cyclic cuts need induction,
which this route does not do. Nor does a dispatch arm, entered by a
`StructuralCase` edge that passes no arguments: its payload parameters are
bound by `DispatchBinding` rows, and lowering mints no arrival for it.

Nothing here reads a name or text: values are compared by identity.
*/

use proof_admission::check_value_equality_denotation;
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, ObligationId, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::{Justification, ValueOrigin};
use terminal_check_tables::{
    Checked, Construct, KnowledgeId, MachineTables, ProgramPoint, RefusalReason,
};
use terminal_psi::{ScalarBlockInvariant, SuccessorEdge, TerminalMachine, Terminator};

use crate::graph::Graph;
use crate::{Discharged, arrivals, decide, goal, push_knowledge, refusal};

/// The declared and inferred predicates available at block entries. Both
/// ordinary discharge and private-input inference prepare the same rows;
/// `check` verifies their arrivals before downstream obligations use them.
pub(crate) fn prepare(
    lowered: &TerminalMachine,
    id: MachineId,
    tables: &mut MachineTables,
    graph: &Graph,
    context: &PropositionContext,
    invariants: &[ScalarBlockInvariant],
    infer_joins: bool,
) -> Checked<Vec<BlockId>> {
    let mut declared = Vec::new();
    for invariant in invariants.iter().filter(|row| row.machine == id) {
        arrivals::admit(id, lowered, tables, invariant)?;
        push_knowledge(
            tables,
            invariant.predicate.clone(),
            ProgramPoint::BlockEntry(invariant.header),
            Justification::BlockInvariant {
                block: invariant.header,
            },
        );
        declared.push(invariant.header);
    }
    if infer_joins {
        infer(lowered, id, tables, graph, context, &declared)?;
    }
    Ok(declared)
}

/// How deep a definition may unfold into another before it is left alone.
const UNFOLD_DEPTH: usize = 64;

/// The arguments an edge passes to its destination, and that destination.
fn arrival(lowered: &TerminalMachine, edge: EdgeId) -> Option<(BlockId, &[ValueId])> {
    fn successor(successor: &SuccessorEdge, edge: EdgeId) -> Option<(BlockId, &[ValueId])> {
        (successor.edge == edge).then_some((successor.target, successor.arguments.as_slice()))
    }
    lowered
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Jump {
                edge: jump,
                target,
                arguments,
                ..
            } if *jump == edge => Some((*target, arguments.as_slice())),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => successor(when_true, edge).or_else(|| successor(when_false, edge)),
            _ => None,
        })
}

fn block(lowered: &TerminalMachine, id: BlockId) -> Option<&terminal_psi::Block> {
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| lowered.blocks.get(position))
        .filter(|block| block.id == id)
}

/// The knowledge row holding `block`'s predicate, if it has one.
fn predicate_row(tables: &MachineTables, block: BlockId) -> Option<KnowledgeId> {
    tables
        .knowledge
        .iter()
        .find(|(_, row)| row.justification == Justification::BlockInvariant { block })
        .map(|(row, _)| row)
}

/// Whether every value `proposition` names is a machine formal, what a
/// formal's storage held at entry, or a parameter of `block`: the predicate
/// scope of control_flow.md. fact_validity.md binds entry hypotheses to
/// "exact invocation formals or structural entry paths", and an entry
/// content is such a path's one value for the whole invocation.
fn in_scope(
    lowered: &TerminalMachine,
    tables: &MachineTables,
    block: BlockId,
    proposition: &Proposition,
) -> bool {
    let mut inside = true;
    proposition.visit_value_ids(|value| {
        inside &= match tables.values.get(value).map(|row| row.origin) {
            Some(ValueOrigin::MachineParameter { .. }) => true,
            Some(ValueOrigin::InitialContent { place }) => {
                crate::places::input_content(lowered, tables, place)
            }
            Some(ValueOrigin::BlockParameter { block: owner, .. }) => owner == block,
            _ => false,
        };
    });
    inside
}

/// `term` with each defined value replaced by its definition, recursively.
pub(crate) fn unfold(
    term: &ScalarTerm,
    definitions: &[(ValueId, &ScalarTerm)],
    depth: usize,
) -> ScalarTerm {
    if depth > UNFOLD_DEPTH {
        return term.clone();
    }
    if let ScalarTerm::Value { id, .. } = term
        && let Some((_, definition)) = definitions.iter().find(|(subject, _)| subject == id)
    {
        return unfold(definition, definitions, depth + 1);
    }
    let mut unfolded = term.clone();
    for operand in unfolded.operands_mut() {
        *operand = unfold(operand, definitions, depth + 1);
    }
    unfolded
}

pub(crate) fn connective(mut parts: Vec<Proposition>, conjunction: bool) -> Proposition {
    let (unit, zero) = if conjunction {
        (Proposition::Truth, Proposition::Falsehood)
    } else {
        (Proposition::Falsehood, Proposition::Truth)
    };
    if parts.contains(&zero) {
        return zero;
    }
    parts.retain(|part| *part != unit);
    parts.sort_unstable();
    parts.dedup();
    match parts.len() {
        0 => unit,
        1 => parts.pop().unwrap_or(unit),
        _ if conjunction => Proposition::Conjunction(parts),
        _ => Proposition::Disjunction(parts),
    }
}

/// Forget atoms whose values cannot cross this boundary. Both connectives
/// are monotone: replacing an unavailable atom with `Truth` weakens the
/// predicate, including inside a disjunction. Dropping a disjunct instead
/// would strengthen it and incorrectly lose a possible arrival.
pub(crate) fn project(
    proposition: &Proposition,
    atom: &mut impl FnMut(&Proposition) -> Option<Proposition>,
) -> Proposition {
    match proposition {
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => connective(
            parts.iter().map(|part| project(part, atom)).collect(),
            matches!(proposition, Proposition::Conjunction(_)),
        ),
        other => atom(other).unwrap_or(Proposition::Truth),
    }
}

/// Infers a predicate for every acyclic block with parameters and records it
/// as that block's `BlockInvariant` knowledge row at its entry. `check` must
/// run before any other obligation reads it. A block in `declared` already
/// has the producer's predicate and gets none inferred.
pub(crate) fn infer(
    lowered: &TerminalMachine,
    id: MachineId,
    tables: &mut MachineTables,
    graph: &Graph,
    context: &PropositionContext,
    declared: &[BlockId],
) -> Checked<()> {
    for block in graph.reverse_postorder()? {
        let join = &lowered.blocks[usize::try_from(block.get() - 1).expect("dense block position")];
        if join.parameters.is_empty() || declared.contains(&join.id) {
            continue;
        }
        let entry = ProgramPoint::BlockEntry(join.id);
        let mut disjuncts = Vec::new();
        // A block entered by a cycle, or by an edge that passes no arguments
        // (a dispatch arm, whose parameters the payload binds through
        // `Justification::DispatchBinding`), gets no inferred predicate.
        let mut skip = false;
        for (edge, row) in tables.edges.iter() {
            if row.destination != Destination::Block(join.id) {
                continue;
            }
            let at_edge = ProgramPoint::Edge(edge);
            if graph.dominates(entry, at_edge)? {
                skip = true;
                break;
            }
            let Some((_, arguments)) = arrival(lowered, edge) else {
                skip = true;
                break;
            };
            disjuncts.push(describe(
                lowered, id, tables, graph, context, join, at_edge, entry, arguments,
            )?);
        }
        if skip {
            continue;
        }
        let predicate = connective(disjuncts, false);
        if predicate != Proposition::Truth {
            push_knowledge(
                tables,
                predicate,
                ProgramPoint::BlockEntry(join.id),
                Justification::BlockInvariant { block: join.id },
            );
        }
    }
    Ok(())
}

/// What one arrival knows that the join does not, in the join's scope.
#[allow(clippy::too_many_arguments)]
fn describe(
    lowered: &TerminalMachine,
    id: MachineId,
    tables: &MachineTables,
    graph: &Graph,
    context: &PropositionContext,
    join: &terminal_psi::Block,
    at_edge: ProgramPoint,
    entry: ProgramPoint,
    arguments: &[ValueId],
) -> Checked<Proposition> {
    let mut available = Vec::new();
    for (_, row) in tables.knowledge.iter() {
        if !graph.dominates(row.available_from, at_edge)? {
            continue;
        }
        let proposition = tables.propositions.get(row.proposition).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Block(join.id),
                "a knowledge row has no proposition",
            )
        })?;
        let path = !graph.dominates(row.available_from, entry)?;
        available.push((row.justification, proposition, path));
    }
    // The definitions transport substitutes: operation equations whose
    // subject is Boolean, whose definition is a literal, or whose definition
    // is what a formal's storage held at entry (a read that no store
    // reaches: its result is that content, which the join can name).
    let entry_content = |term: &ScalarTerm| {
        matches!(term, ScalarTerm::Value { id, .. }
        if matches!(
            tables.values.get(*id).map(|row| row.origin),
            Some(ValueOrigin::InitialContent { place }) if crate::places::input_content(lowered, tables, place)
        ))
    };
    let equations: Vec<&Proposition> = available
        .iter()
        .filter(|(justification, proposition, _)| {
            matches!(justification, Justification::OperationSemantics(_))
                && matches!(
                    proposition,
                    Proposition::Equal(subject @ ScalarTerm::Value { .. }, definition)
                        if subject.scalar_type() == ScalarType::Boolean
                            || matches!(
                                definition,
                                ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
                            )
                            || entry_content(definition)
                )
        })
        .map(|(_, proposition, _)| *proposition)
        .collect();
    let definitions: Vec<(ValueId, &ScalarTerm)> = equations
        .iter()
        .filter_map(|equation| match equation {
            Proposition::Equal(ScalarTerm::Value { id, .. }, definition) => Some((*id, definition)),
            _ => None,
        })
        .collect();
    // What this edge fixes: a value it knows equal to a literal and that no
    // operation defines, which is a preceding join's result under the guard
    // that chose this edge. A fact that names such a value says more here
    // than it does at the join, where the value is free again.
    let fixed: Vec<&Proposition> = available
        .iter()
        .filter(|(justification, proposition, path)| {
            *path
                && !matches!(justification, Justification::OperationSemantics(_))
                && matches!(
                    proposition,
                    Proposition::Equal(
                        ScalarTerm::Value { id, .. },
                        ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
                    ) if !definitions.iter().any(|(subject, _)| subject == id)
                )
        })
        .map(|(_, proposition, _)| *proposition)
        .collect();
    let names_fixed = |proposition: &Proposition| {
        let mut named = false;
        proposition.visit_value_ids(|value| {
            named |= fixed.iter().any(|equation| {
                matches!(equation, Proposition::Equal(ScalarTerm::Value { id, .. }, _) if *id == value)
            });
        });
        named
    };
    let mut parts = Vec::new();
    let renaming: Vec<_> = arguments
        .iter()
        .copied()
        .zip(join.parameters.iter().map(|parameter| parameter.id))
        .collect();
    for (parameter, argument) in join.parameters.iter().zip(arguments) {
        let defined = unfold(
            &ScalarTerm::value(*argument, parameter.scalar_type),
            &definitions,
            0,
        );
        parts.push(Proposition::Equal(
            ScalarTerm::value(parameter.id, parameter.scalar_type),
            defined,
        ));
    }
    for (justification, proposition, path) in &available {
        if matches!(justification, Justification::OperationSemantics(_)) {
            continue;
        }
        if !path {
            // Known at the join too, so it is restated only for what this
            // edge adds to it: a preceding join's predicate, read with that
            // join's result fixed, is the one alternative that produced it.
            if matches!(justification, Justification::BlockInvariant { .. })
                && names_fixed(proposition)
                && let Ok(denotation) = check_value_equality_denotation(
                    context,
                    proposition,
                    equations.iter().chain(&fixed).copied(),
                )
                && let Some(denotation) = rename(&denotation, &renaming)
            {
                parts.push(project(&denotation, &mut |atom| {
                    in_scope(lowered, tables, join.id, atom).then(|| atom.clone())
                }));
            }
            continue;
        }
        // Keep a predicate that already crosses under the exact argument
        // substitution. Expanding its Boolean equalities into cases would
        // discard the reusable form the preceding join established.
        if let Some(stated) = rename(proposition, &renaming)
            && in_scope(lowered, tables, join.id, &stated)
        {
            parts.push(stated);
            continue;
        }
        let denotation = if equations.is_empty() {
            (*proposition).clone()
        } else {
            check_value_equality_denotation(context, proposition, equations.iter().copied())
                .unwrap_or_else(|_| (*proposition).clone())
        };
        // A preceding join's result becomes this join's formal on this
        // exact edge. Its predicate follows that identity too, rather than
        // being discarded for naming the preceding result.
        let Some(denotation) = rename(&denotation, &renaming) else {
            continue;
        };
        parts.push(project(&denotation, &mut |atom| {
            in_scope(lowered, tables, join.id, atom).then(|| atom.clone())
        }));
    }
    // A part outside the scope, or one this route does not state (a
    // contradiction, which would make the disjunct unprovable here), is left
    // out: that weakens the disjunct, never strengthens it.
    parts
        .retain(|part| *part != Proposition::Falsehood && in_scope(lowered, tables, join.id, part));
    Ok(connective(parts, true))
}

/// The goal of `ArrivalInvariant { edge }`: its destination's predicate with
/// each parameter replaced by the argument this edge passes. `Truth` when the
/// destination has no predicate.
pub(crate) fn arrival_goal(
    lowered: &TerminalMachine,
    id: MachineId,
    obligation: ObligationId,
    edge: EdgeId,
    tables: &MachineTables,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    let (target, arguments) = arrival(lowered, edge).ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the arrival edge is not a successor in the module",
        )
    })?;
    let Some(row) = predicate_row(tables, target) else {
        return Ok(Proposition::Truth);
    };
    let predicate = tables
        .knowledge
        .get(row)
        .and_then(|row| tables.propositions.get(row.proposition))
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                concerns,
                "a join predicate row has no proposition",
            )
        })?;
    let join = block(lowered, target).ok_or_else(|| {
        refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the arrival enters a block past the end",
        )
    })?;
    if join.parameters.len() != arguments.len() {
        return Err(refusal(
            RefusalReason::ForeignIdentity,
            id,
            concerns,
            "the arrival's arguments do not match the join's parameters",
        ));
    }
    let renaming: Vec<(ValueId, ValueId)> = join
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .zip(arguments.iter().copied())
        .collect();
    rename(predicate, &renaming).ok_or_else(|| {
        refusal(
            RefusalReason::Unsupported,
            id,
            concerns,
            "join predicate form outside the scalar subset",
        )
    })
}

/// Restate a predicate on an arrival using the same checked clause traversal
/// as calls. Unmapped values keep their identity: machine inputs and values
/// already in the destination's scope do not become block parameters.
pub(crate) fn rename(
    proposition: &Proposition,
    renaming: &[(ValueId, ValueId)],
) -> Option<Proposition> {
    crate::substitute(proposition, &mut RenamedValues(renaming)).ok()
}

struct RenamedValues<'a>(&'a [(ValueId, ValueId)]);

impl crate::Bindings for RenamedValues<'_> {
    type Error = crate::Unbound;

    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Self::Error> {
        let renamed = self
            .0
            .iter()
            .find(|(from, _)| *from == id)
            .map_or(id, |(_, to)| *to);
        Ok(ScalarTerm::value(renamed, scalar_type))
    }

    fn field(
        &mut self,
        root: semantic_vocabulary::PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        // This is an identity rename within one machine, not observation of
        // another frame. Place identities and their projected paths stay put.
        Ok(match scalar_type {
            ScalarType::Integer(scalar_type) => ScalarTerm::IntegerField {
                root,
                path: path.to_vec(),
                scalar_type,
            },
            ScalarType::Boolean => ScalarTerm::BooleanField {
                root,
                path: path.to_vec(),
            },
            _ => {
                return Err(crate::Unbound::Unsupported(
                    "join field has no scalar observation",
                ));
            }
        })
    }

    fn case(
        &mut self,
        _: semantic_vocabulary::PlaceId,
        _: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        _: semantic_vocabulary::StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err(crate::Unbound::Unsupported(
            "join predicate has no captured case",
        ))
    }
}

/// What `check` needs from the machine being discharged.
pub(crate) struct Machine<'a> {
    pub(crate) module: &'a terminal_psi::TerminalModule,
    pub(crate) lowered: &'a TerminalMachine,
    pub(crate) storage: &'a crate::places::Storage<'a>,
    pub(crate) id: MachineId,
    pub(crate) types: &'a semantic_vocabulary::IdTable<ValueId, ScalarType>,
    pub(crate) graph: &'a Graph,
    pub(crate) context: &'a PropositionContext,
    pub(crate) relations: bool,
    /// Blocks whose predicate the producer declared (a loop head's). Such a
    /// predicate is the program's claim, so an arrival that fails it is an
    /// unproved obligation and the predicate is never weakened.
    pub(crate) declared: &'a [BlockId],
    /// The machine's values the integer rules do not reach
    /// (`crate::Unreached`).
    pub(crate) unreached: &'a [ValueId],
}

/// Decides every `ArrivalInvariant` obligation into `decided`. An inferred
/// join any of whose arrivals is not proved has its predicate replaced by
/// `Truth` (a declared one keeps its predicate and its failed arrival), and
/// every arrival is decided again, since one join's predicate may be a fact
/// on another's arrival. Each round weakens at least one predicate, so this
/// ends. Returns the weakened joins.
pub(crate) fn check(
    machine: &Machine<'_>,
    tables: &mut MachineTables,
    decided: &mut [Option<Checked<Discharged>>],
) -> Checked<Vec<BlockId>> {
    let arrivals: Vec<(usize, ObligationId, EdgeId, ProgramPoint)> = tables
        .obligations
        .iter()
        .enumerate()
        .filter_map(|(index, (obligation, row))| match row.kind {
            ObligationKind::ArrivalInvariant { edge } => {
                Some((index, obligation, edge, row.created_at))
            }
            _ => None,
        })
        .collect();
    let mut weakened = Vec::new();
    loop {
        let mut failed = Vec::new();
        for &(index, obligation, edge, point) in &arrivals {
            let kind = ObligationKind::ArrivalInvariant { edge };
            let verdict = goal(
                machine.module,
                machine.lowered,
                machine.storage,
                machine.id,
                obligation,
                &kind,
                tables,
                machine.types,
            )
            .and_then(|goal| {
                decide(
                    machine.context,
                    machine.graph,
                    tables,
                    machine.id,
                    obligation,
                    point,
                    goal,
                    machine.relations,
                    machine.unreached,
                )
                .map(Discharged::Certificate)
            });
            let refuse = |detail: &str| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    machine.id,
                    Construct::Obligation(obligation),
                    detail,
                )
            };
            if verdict.is_err() {
                let (target, _) = arrival(machine.lowered, edge)
                    .ok_or_else(|| refuse("the arrival edge is not a successor in the module"))?;
                if let Some(row) = predicate_row(tables, target)
                    && !machine.declared.contains(&target)
                    && !failed.contains(&(target, row))
                {
                    failed.push((target, row));
                }
            }
            *decided
                .get_mut(index)
                .ok_or_else(|| refuse("the obligation is past the end of the verdicts"))? =
                Some(verdict);
        }
        let mut changed = false;
        for (target, row) in failed {
            let predicate = tables
                .knowledge
                .get(row)
                .map(|row| row.proposition)
                .and_then(|proposition| tables.propositions.get_mut(proposition))
                .ok_or_else(|| {
                    refusal(
                        RefusalReason::ForeignIdentity,
                        machine.id,
                        Construct::Block(target),
                        "a join predicate row has no proposition",
                    )
                })?;
            if *predicate != Proposition::Truth {
                *predicate = Proposition::Truth;
                weakened.push(target);
                changed = true;
            }
        }
        if !changed {
            return Ok(weakened);
        }
    }
}
