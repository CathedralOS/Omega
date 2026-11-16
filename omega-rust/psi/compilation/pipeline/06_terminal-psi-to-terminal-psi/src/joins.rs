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
parameters. The checker infers a predicate from the facts on each arrival.
Besides the join's own parameters, inference can keep a formal whose binding
dominates the join. Authored invariant scope follows control_flow.md.

- for each parameter `p` and its argument `a` on that arrival, `p == a`,
  with `a` replaced by the exact scalar definitions available on the
  arrival edge, through the same bounded substitution that
  `ValueEqualityTransport` checks;
- for each integer parameter, the literal range the prover finds for its
  argument on that arrival (`argument_bounds`), which the unfolded
  definition alone may no longer give;
- each fact available on the arrival edge but not at the join, transported
  through those definitions into its denotation;
- each preceding join's predicate that names a value this edge fixes to a
  literal, read with that value fixed. `a && b && c` is two joins, and the
  first's result is free again at the second; on the edge where it is true
  the first's predicate is the one alternative that produced it, and that
  is what the second join can still say.

Demanded entry snapshots of structural formals also gain candidate equalities
to the contents supplied by each incoming edge. The ordinary arrival check
substitutes those snapshots again and proves every alternative before use.

An exact storage snapshot has the same scope as the formal that owns it.
A block formal's snapshot is rebound at every entry. A read with no
overlapping store sees that snapshot, so a guard over a field crosses the
same join as a guard over the formal itself.

A part that still names a value outside the scope is left out, which only
weakens the disjunct. Each arrival's obligation is then that predicate under
its parameter-to-argument substitution, proved by the one prover and decided
by the kernel. When some arrival cannot be proved, the inferred facts are
discarded and the join is reported in `MachineDischarge::weakened_joins`.
The original declared predicate, if any, stays mandatory and its arrivals are
checked again. A join that dominates one of its arrivals needs an induction
hypothesis. This inference skips it. A dispatch arm's payload parameters are
bound by `DispatchBinding` rows, and lowering mints no arrival for them.

Nothing here reads a name or text: values are compared by identity.
*/

use proof_admission::{check_value_equality_denotation, check_value_equality_substitution};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, ObligationId, Proposition, PropositionContext, ScalarTerm,
    ScalarType, StructuralPlaceKind, ValueId,
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
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare<'a>(
    lowered: &TerminalMachine,
    id: MachineId,
    tables: &mut MachineTables,
    graph: &Graph,
    context: &PropositionContext,
    invariants: &[ScalarBlockInvariant],
    storage: &crate::places::Storage<'a>,
    references: &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<Vec<(BlockId, Proposition)>> {
    let mut declared = Vec::new();
    for invariant in invariants.iter().filter(|row| row.machine == id) {
        arrivals::admit(id, lowered, tables, invariant)?;
        let header = block(lowered, invariant.header).ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Block(invariant.header),
                "block predicate has no header",
            )
        })?;
        let arguments = header
            .structural_parameters
            .iter()
            .map(|formal| terminal_psi::StructuralArgument::whole(formal.place, formal.access))
            .collect::<Vec<_>>();
        let predicate = crate::substitute(
            &invariant.predicate,
            &mut storage.at_join(
                references,
                tables,
                &header.structural_parameters,
                &arguments,
                &[],
                ProgramPoint::BlockEntry(header.id),
                Construct::Block(header.id),
            )?,
        )
        .map_err(|error| error.refusal(id, Construct::Block(header.id)))?;
        push_knowledge(
            tables,
            predicate.clone(),
            ProgramPoint::BlockEntry(invariant.header),
            Justification::BlockInvariant {
                block: invariant.header,
            },
        );
        declared.push((invariant.header, predicate));
    }
    infer(
        lowered, id, tables, graph, context, storage, references, activity,
    )?;
    Ok(declared)
}

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

/// Whether the inferred predicate uses values bound on every arrival.
/// A dominating block parameter is already bound when the join runs, and
/// any use the join dominates must revisit the join after a new binding.
/// The authored invariant roster keeps its scope in `arrivals::admit`.
fn in_scope(
    lowered: &TerminalMachine,
    tables: &MachineTables,
    dominating_bindings: Option<&Graph>,
    block: BlockId,
    proposition: &Proposition,
) -> bool {
    let mut inside = true;
    proposition.visit_value_ids(|value| {
        inside &= available_value(lowered, tables, dominating_bindings, block, value);
    });
    inside
}

/// An inferred predicate may also name an integer operation result whose
/// definition dominates the join, such as a call result. Every path to the
/// join passes it, so it is one value on every arrival and at the join, and
/// fact_validity.md requires only that a premise "dominate its use". Without
/// it a guard over a call result could not cross the join `&&` lowers to:
/// `let s = f(x); transition s >= 100 && s <= 200` left the predicate
/// `Truth`, and `s - 100` on the taken edge was refused. A result defined in
/// one arm does not dominate the join and stays out.
///
/// Only integers are admitted, because they carry the bounds a guard
/// establishes. A dominating Boolean result adds no bound, only another
/// alternative per arrival: `left == right && count == 2` over byte views
/// then left a predicate whose case split exhausted the prover's bounded
/// disjunction search (`byte_view_elements.rs`). Leaving a value out only
/// weakens the predicate.
fn available_value(
    lowered: &TerminalMachine,
    tables: &MachineTables,
    dominating_bindings: Option<&Graph>,
    block: BlockId,
    value: ValueId,
) -> bool {
    let dominates = |point: ProgramPoint| {
        dominating_bindings.is_some_and(|graph| {
            graph
                .dominates(point, ProgramPoint::BlockEntry(block))
                .is_ok_and(|dominates| dominates)
        })
    };
    match tables
        .values
        .get(value)
        .map(|row| (row.origin, row.scalar_type))
    {
        Some((ValueOrigin::MachineParameter { .. }, _)) => true,
        Some((ValueOrigin::StorageContent { place, after: None }, _)) => {
            match crate::places::content_root_kind(lowered, tables, place) {
                Some(StructuralPlaceKind::Parameter { .. }) => true,
                Some(StructuralPlaceKind::BlockParameter { block: owner, .. }) => {
                    owner == block || dominates(ProgramPoint::BlockEntry(owner))
                }
                _ => false,
            }
        }
        Some((ValueOrigin::BlockParameter { block: owner, .. }, _)) => {
            owner == block || dominates(ProgramPoint::BlockEntry(owner))
        }
        Some((ValueOrigin::Operation(operation), ScalarType::Integer(_))) => {
            dominates(ProgramPoint::AfterOperation(operation))
        }
        _ => false,
    }
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

/// Records an inferred scalar join predicate as `BlockInvariant` knowledge
/// at its entry. `check` must run before any other obligation reads it. A
/// declared predicate keeps its own scope and gains incoming facts expressed
/// in that scope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn infer<'a>(
    lowered: &TerminalMachine,
    id: MachineId,
    tables: &mut MachineTables,
    graph: &Graph,
    context: &PropositionContext,
    storage: &crate::places::Storage<'a>,
    references: &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    // Only actual demanded snapshots participate. Group them once by the
    // owning block, using the already validated dense block roster rather
    // than scanning all values again for each join or allocating by raw IDs.
    let mut snapshots = vec![Vec::new(); lowered.blocks.len()];
    for (value, row) in tables.values.iter() {
        let ValueOrigin::StorageContent { place, after: None } = row.origin else {
            continue;
        };
        let Some(StructuralPlaceKind::BlockParameter { block, .. }) =
            crate::places::content_root_kind(lowered, tables, place)
        else {
            continue;
        };
        let position = usize::try_from(block.get() - 1)
            .ok()
            .filter(|position| {
                lowered
                    .blocks
                    .get(*position)
                    .is_some_and(|row| row.id == block)
            })
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    id,
                    Construct::Value(value),
                    "a join snapshot names no actual block",
                )
            })?;
        if !matches!(row.scalar_type, ScalarType::Callable(_)) {
            snapshots[position].push((value, row.scalar_type));
        }
    }
    for &block in graph.reverse_postorder()? {
        let join = &lowered.blocks[usize::try_from(block.get() - 1).expect("dense block position")];
        let snapshots = &snapshots[usize::try_from(block.get() - 1).expect("dense block position")];
        if join.parameters.is_empty() && snapshots.is_empty() {
            continue;
        }
        let entry = ProgramPoint::BlockEntry(join.id);
        let declared = predicate_row(tables, join.id);
        // A declared invariant keeps its own telescope even when one member
        // of a ranked call cycle dominates another member's head.
        let dominating_bindings = declared.is_none().then_some(graph);
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
            let bounds = argument_bounds(
                id, tables, graph, context, join, at_edge, arguments, activity,
            );
            let mut snapshot_bindings = Vec::new();
            if !snapshots.is_empty() {
                let concerns = Construct::Edge(edge);
                let (target, _, structural) = storage.arrival(edge, concerns)?;
                if target != join.id || arguments.len() != join.parameters.len() {
                    return Err(refusal(
                        RefusalReason::ForeignIdentity,
                        id,
                        concerns,
                        "a snapshot arrival differs from its destination telescope",
                    ));
                }
                let renaming = join
                    .parameters
                    .iter()
                    .zip(arguments)
                    .map(|(formal, actual)| (formal.id, *actual))
                    .collect::<Vec<_>>();
                let mut inputs = storage.at_join(
                    references,
                    tables,
                    &join.structural_parameters,
                    structural,
                    &renaming,
                    at_edge,
                    concerns,
                )?;
                for &(value, scalar_type) in snapshots {
                    match crate::Bindings::value(&mut inputs, value, scalar_type) {
                        Ok(actual) => snapshot_bindings.push(Proposition::Equal(
                            ScalarTerm::value(value, scalar_type),
                            actual,
                        )),
                        Err(crate::Unbound::Unproved(_) | crate::Unbound::Unsupported(_)) => {}
                        Err(crate::Unbound::Refused(refused)) => return Err(refused),
                    }
                }
            }
            disjuncts.push(describe(
                lowered,
                id,
                tables,
                graph,
                context,
                join,
                at_edge,
                dominating_bindings,
                arguments,
                snapshot_bindings,
                bounds,
            )?);
        }
        if skip {
            continue;
        }
        let predicate = connective(disjuncts, false);
        if predicate != Proposition::Truth {
            if let Some(row) = declared {
                let previous = tables
                    .knowledge
                    .get(row)
                    .map(|row| row.proposition)
                    .and_then(|proposition| tables.propositions.get_mut(proposition))
                    .ok_or_else(|| {
                        refusal(
                            RefusalReason::ForeignIdentity,
                            id,
                            Construct::Block(join.id),
                            "a declared block predicate has no proposition",
                        )
                    })?;
                *previous = connective(vec![previous.clone(), predicate], true);
            } else {
                push_knowledge(
                    tables,
                    predicate,
                    ProgramPoint::BlockEntry(join.id),
                    Justification::BlockInvariant { block: join.id },
                );
            }
        }
    }
    Ok(())
}

/// The literal range of each integer argument an arrival passes, stated on
/// the destination parameter it binds. The parameter's equation keeps the
/// argument's definition, but a definition the integer rules bound only
/// through its own result equations, such as `(x & 32) as u8 as u64`, says
/// nothing once it is unfolded into a block assumption: an assumption is no
/// operation axiom, so neither the mask's endpoint step nor the cast's bound
/// transport can cite it. The arrival still has those axioms, so its range is
/// found there, by the prover the arrival obligation itself is decided with,
/// and crosses as an inclusive range conjunct ("Inclusive integer ranges are
/// ordinary conjunction predicates in this same roster", control_flow.md).
/// Every arrival proves it again with the rest of the predicate. A bound that
/// is only the carrier's, or an arrival whose hypotheses cannot be read, adds
/// nothing.
#[allow(clippy::too_many_arguments)]
fn argument_bounds(
    id: MachineId,
    tables: &MachineTables,
    graph: &Graph,
    context: &PropositionContext,
    join: &terminal_psi::Block,
    at_edge: ProgramPoint,
    arguments: &[ValueId],
    activity: &mut crate::ProofSearchActivity,
) -> Vec<Proposition> {
    let mut bounds = Vec::new();
    if !join
        .parameters
        .iter()
        .any(|parameter| matches!(parameter.scalar_type, ScalarType::Integer(_)))
    {
        return bounds;
    }
    let Some(identity) = ObligationId::new(1) else {
        return bounds;
    };
    let Ok(known) = crate::hypotheses(graph, tables, id, identity, at_edge) else {
        return bounds;
    };
    let mut prover = crate::prove::Prover::new(
        context,
        &known.assumptions,
        &known.axioms,
        &tables.values,
        activity,
    );
    for (parameter, argument) in join.parameters.iter().zip(arguments) {
        let ScalarType::Integer(integer) = parameter.scalar_type else {
            continue;
        };
        let subject = ScalarTerm::value(parameter.id, parameter.scalar_type);
        let literal = |value| ScalarTerm::Integer {
            scalar_type: integer,
            value,
        };
        let (lower, upper) = prover.literal_bounds(*argument);
        if let Some(lower) = lower {
            bounds.push(Proposition::LessOrEqual(literal(lower), subject.clone()));
        }
        if let Some(upper) = upper {
            bounds.push(Proposition::LessOrEqual(subject, literal(upper)));
        }
    }
    bounds
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
    dominating_bindings: Option<&Graph>,
    arguments: &[ValueId],
    snapshot_bindings: Vec<Proposition>,
    bounds: Vec<Proposition>,
) -> Checked<Proposition> {
    let entry = ProgramPoint::BlockEntry(join.id);
    let in_scope = |proposition: &Proposition| {
        in_scope(lowered, tables, dominating_bindings, join.id, proposition)
    };
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
    // Operation equations name immutable scalar results, including computed
    // integers. Storage-read equations come from the write log, so expanding
    // one preserves its exact reaching version. The final scope projection
    // still rejects branch-local values that cannot be restated at this join.
    // Other carriers retain only what a formal's storage held at entry.
    let entry_content = |term: &ScalarTerm| {
        matches!(term, ScalarTerm::Value { id, .. }
        if matches!(
            tables.values.get(*id).map(|row| row.origin),
            Some(ValueOrigin::StorageContent { after: None, .. })
                if available_value(lowered, tables, dominating_bindings, join.id, *id)
        ))
    };
    let mut equations: Vec<&Proposition> = available
        .iter()
        .filter(|(justification, proposition, _)| {
            matches!(justification, Justification::OperationSemantics(_))
                && matches!(
                    proposition,
                    Proposition::Equal(subject @ ScalarTerm::Value { .. }, definition)
                        if matches!(subject.scalar_type(), ScalarType::Boolean | ScalarType::Integer(_))
                            || entry_content(definition)
                )
        })
        .map(|(_, proposition, _)| *proposition)
        .collect();
    let mut defined_values: Vec<ValueId> = equations
        .iter()
        .filter_map(|equation| match equation {
            Proposition::Equal(ScalarTerm::Value { id, .. }, _) => Some(*id),
            _ => None,
        })
        .collect();
    defined_values.sort_unstable();
    defined_values.dedup();
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
                    ) if defined_values.binary_search(id).is_err()
                )
        })
        .map(|(_, proposition, _)| *proposition)
        .collect();
    let operation_equations = equations.len();
    // A checked block predicate can also relate an earlier parameter to its
    // argument. These conjuncts extend transport after the operation roster
    // above has identified true definitions: an edge fixing a join result to
    // a literal must remain a path selection, not become its definition.
    // Operation equations keep priority, and alternatives are never flattened.
    // Keep an already visible value as itself. The guard and the next call
    // use the current head parameter, even when an earlier arrival proved
    // an equality between that parameter and an incoming expression.
    let mut pending: Vec<&Proposition> = available
        .iter()
        .filter(|(justification, _, _)| {
            matches!(justification, Justification::BlockInvariant { .. })
        })
        .map(|(_, proposition, _)| *proposition)
        .collect();
    while let Some(proposition) = pending.pop() {
        match proposition {
            Proposition::Conjunction(parts) => pending.extend(parts),
            Proposition::Equal(subject @ ScalarTerm::Value { id: value, .. }, definition)
                if (dominating_bindings.is_none()
                    || !available_value(lowered, tables, dominating_bindings, join.id, *value))
                    && (matches!(
                        subject.scalar_type(),
                        ScalarType::Boolean | ScalarType::Integer(_)
                    ) || entry_content(definition)) =>
            {
                equations.push(proposition)
            }
            _ => {}
        }
    }
    let names_fixed = |proposition: &Proposition| {
        let mut named = false;
        proposition.visit_value_ids(|value| {
            named |= fixed.iter().any(|equation| {
                matches!(equation, Proposition::Equal(ScalarTerm::Value { id, .. }, _) if *id == value)
            });
        });
        named
    };
    let mut parts = bounds;
    let renaming: Vec<_> = arguments
        .iter()
        .copied()
        .zip(join.parameters.iter().map(|parameter| parameter.id))
        .collect();
    let scalar_bindings = join
        .parameters
        .iter()
        .zip(arguments)
        .filter(|(parameter, _)| !matches!(parameter.scalar_type, ScalarType::Callable(_)))
        .map(|(parameter, argument)| {
            Proposition::Equal(
                ScalarTerm::value(parameter.id, parameter.scalar_type),
                ScalarTerm::value(*argument, parameter.scalar_type),
            )
        });
    for binding in scalar_bindings.chain(snapshot_bindings) {
        // Reuse the kernel's bounded definition traversal. Expanding an
        // arithmetic DAG by recursive cloning here can grow exponentially.
        // A failed or exhausted transport leaves the original binding for
        // the ordinary scope filter to weaken, never an assumed relation.
        let defined =
            check_value_equality_substitution(context, &binding, equations.iter().copied())
                .unwrap_or(binding);
        parts.push(defined);
    }
    for (justification, proposition, path) in &available {
        // A dominating fact about an argument is not yet a fact about the
        // destination's fresh parameter. Carry its exact renaming even when
        // the original fact dominates this join (including read premises
        // supplied by operation semantics). Every arrival still proves the
        // inferred predicate; no storage observation is re-read here.
        let stated = rename(proposition, &renaming);
        if stated != **proposition && in_scope(&stated) {
            parts.push(stated);
        }
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
                    equations[..operation_equations]
                        .iter()
                        .chain(&fixed)
                        .chain(&equations[operation_equations..])
                        .copied(),
                )
            {
                let denotation = rename(&denotation, &renaming);
                parts.push(project(&denotation, &mut |atom| {
                    in_scope(atom).then(|| atom.clone())
                }));
            }
            continue;
        }
        // Keep a predicate that already crosses under the exact argument
        // substitution. Expanding its Boolean equalities into cases would
        // discard the reusable form the preceding join established.
        let stated = rename(proposition, &renaming);
        if in_scope(&stated) {
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
        let denotation = rename(&denotation, &renaming);
        parts.push(project(&denotation, &mut |atom| {
            in_scope(atom).then(|| atom.clone())
        }));
    }
    // A part outside the scope, or one this route does not state (a
    // contradiction, which would make the disjunct unprovable here), is left
    // out: that weakens the disjunct, never strengthens it.
    parts.retain(|part| *part != Proposition::Falsehood && in_scope(part));
    Ok(connective(parts, true))
}

/// The goal of `ArrivalInvariant { edge }`: its destination's predicate with
/// each parameter replaced by the argument this edge passes. `Truth` when the
/// destination has no predicate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn arrival_goal<'a>(
    lowered: &TerminalMachine,
    id: MachineId,
    obligation: ObligationId,
    edge: EdgeId,
    tables: &MachineTables,
    storage: &crate::places::Storage<'a>,
    invariants: &[ScalarBlockInvariant],
    references: &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
) -> Checked<Proposition> {
    let concerns = Construct::Obligation(obligation);
    let (target, arguments, structural) = storage.arrival(edge, concerns)?;
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
    let mut bindings = storage.at_join(
        references,
        tables,
        &join.structural_parameters,
        structural,
        &renaming,
        ProgramPoint::Edge(edge),
        concerns,
    )?;
    let explain = |error: crate::Unbound| {
        let mut refused = error.refusal(id, concerns);
        refused.detail = format!(
            "arrival edge {} into block {}: {}",
            edge.get(),
            target.get(),
            refused.detail
        );
        refused
    };
    let stated = crate::substitute(predicate, &mut bindings).map_err(explain)?;
    // The original contract remains an arrival goal after its header's
    // storage observations have been stated over exact logical values.
    let original = invariants
        .iter()
        .find(|row| row.machine == id && row.header == target);
    if let Some(original) = original {
        let original = crate::substitute(&original.predicate, &mut bindings).map_err(explain)?;
        Ok(connective(vec![stated, original], true))
    } else {
        Ok(stated)
    }
}

/// Rename scalar identities simultaneously within the same machine. Storage
/// observations keep their full subject and are resolved separately at the
/// actual arrival point; renaming cannot decide a case or an extent.
pub(crate) fn rename(proposition: &Proposition, renaming: &[(ValueId, ValueId)]) -> Proposition {
    let mut renamed = proposition.clone();
    renamed.remap_value_ids(|value| {
        renaming
            .iter()
            .find(|(from, _)| *from == value)
            .map_or(value, |(_, to)| *to)
    });
    renamed
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
    pub(crate) ranking_bindings: &'a crate::rankings::Bindings,
    /// Keep the producer's original predicates separate from inferred facts.
    /// An inference can be discarded; a failed declared claim still rejects.
    pub(crate) declared: &'a [(BlockId, Proposition)],
    /// The machine's values the integer rules do not reach
    /// (`crate::Unreached`).
    pub(crate) unreached: &'a [ValueId],
}

/// Decides every `ArrivalInvariant` obligation into `decided`. An inferred
/// join any of whose arrivals is not proved loses its inferred predicate.
/// Its original declared predicate, when present, remains mandatory. Then
/// every arrival is decided again, since one join's predicate may be a fact
/// on another's arrival. Each round weakens at least one predicate, so this
/// ends. Returns the weakened joins.
pub(crate) fn check<'a>(
    machine: &Machine<'a>,
    tables: &mut MachineTables,
    decided: &mut [Option<Checked<Discharged>>],
    activity: &mut crate::ProofSearchActivity,
    references: &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
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
                (machine.storage, references),
                machine.id,
                obligation,
                &kind,
                tables,
                machine.types,
                machine.ranking_bindings,
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
                    machine.unreached,
                    activity,
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
            let required = machine
                .declared
                .iter()
                .find(|(block, _)| *block == target)
                .map_or(Proposition::Truth, |(_, predicate)| predicate.clone());
            if *predicate != required {
                *predicate = required;
                weakened.push(target);
                changed = true;
            }
        }
        if !changed {
            return Ok(weakened);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_only_join_snapshots_keep_every_distinct_arrival() {
        fn judged(text: &str) -> Vec<crate::RouteVerdict> {
            let mut sources = source::SourceMap::default();
            let source = sources
                .add_checked_instance(
                    "package/main.omg".into(),
                    text.to_owned(),
                    "package".into(),
                    None,
                    source::SourceOrigin::User,
                    source::SourceResolutionStratum::Base,
                    source::DependencyScope::Product,
                )
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            let syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
            let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
            request.sources = Some(std::sync::Arc::new(sources));
            let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
            let typed =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .unwrap();
            let prepared = typed_trees_to_terminal_psi::prepare_compilation(
                &typed,
                typed_trees_to_terminal_psi::Compilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[],
                    subjects: &[None],
                    purpose: source::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            );
            let verdicts = crate::judge_compilation(prepared.program);
            assert!(!verdicts.is_empty(), "the real source must be checked");
            verdicts.into_iter().map(Result::unwrap).collect()
        }
        let source = r#"
            data Cell [copy] requires value <= 2u8 { value: u8; }
            public machine main(flag: bool) -> u8
            ensures result == 1u8 || result == 2u8 {
                let cell: Cell = match flag {
                    true -> Cell { value: 1u8 },
                    false -> Cell { value: 2u8 }
                };
                cell.value
            }
        "#;
        let positive = judged(source);
        assert!(
            positive
                .iter()
                .all(|verdict| matches!(verdict, crate::RouteVerdict::Accepted(_))),
            "{positive:?}"
        );
        let changed = source.replace("result == 1u8 || result == 2u8", "result == 1u8");
        assert_ne!(source, changed);
        let negative = judged(&changed);
        let mut guarantee_refused = false;
        for verdict in negative {
            match verdict {
                crate::RouteVerdict::Accepted(_) => {}
                crate::RouteVerdict::Rejected {
                    formations,
                    violations,
                    unproved,
                    unestablished,
                    conflicts,
                    exceeded,
                } => {
                    assert!(
                        formations.is_empty()
                            && violations.is_empty()
                            && unestablished.is_empty()
                            && conflicts.is_empty()
                            && exceeded.is_empty(),
                        "the twin must fail only its actual result guarantee"
                    );
                    assert!(!unproved.is_empty());
                    assert!(
                        unproved.iter().all(|failure| matches!(
                            failure.kind,
                            ObligationKind::NormalGuarantee { .. }
                                | ObligationKind::PublishedGuarantee { .. }
                        )),
                        "an unrelated proof refusal is not the intended twin: {unproved:?}"
                    );
                    guarantee_refused = true;
                }
                other => panic!("an unserved or malformed program is not the twin: {other:?}"),
            }
        }
        assert!(guarantee_refused, "both real arrivals must remain possible");
    }

    #[test]
    fn renaming_preserves_local_storage_subjects_and_swaps_values_simultaneously() {
        let from = ValueId::new(1).unwrap();
        let to = ValueId::new(2).unwrap();
        let root = semantic_vocabulary::PlaceId::new(7).unwrap();
        let integer =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .unwrap();
        let extent = ScalarTerm::ViewExtent {
            root,
            path: Vec::new(),
            scalar_type: integer,
        };
        let membership = Proposition::StructuralCaseMembership {
            subject: semantic_vocabulary::StructuralCaseSubject::new(root, Vec::new()),
            case: semantic_vocabulary::StructuralCaseId::new(3).unwrap(),
        };
        let predicate = Proposition::Conjunction(vec![
            Proposition::LessThan(
                ScalarTerm::value(from, ScalarType::Integer(integer)),
                extent.clone(),
            ),
            Proposition::Equal(
                ScalarTerm::value(to, ScalarType::Integer(integer)),
                ScalarTerm::value(from, ScalarType::Integer(integer)),
            ),
            membership.clone(),
        ]);
        let renamed = rename(&predicate, &[(from, to), (to, from)]);
        assert_eq!(
            renamed,
            Proposition::Conjunction(vec![
                Proposition::LessThan(ScalarTerm::value(to, ScalarType::Integer(integer)), extent),
                Proposition::Equal(
                    ScalarTerm::value(from, ScalarType::Integer(integer)),
                    ScalarTerm::value(to, ScalarType::Integer(integer))
                ),
                membership,
            ])
        );
    }
}
