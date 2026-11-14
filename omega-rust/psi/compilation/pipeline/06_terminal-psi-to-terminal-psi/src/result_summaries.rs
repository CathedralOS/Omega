/*
A caller in the same package may rely on what a private helper's body returns,
not only on the `ensures` its author wrote. For example, a helper that widens
its `u16` input under `requires narrow >= 300 && narrow <= 500` and declares no
guarantee still returns a value between 300 and 500, and its caller may pass
that value to a machine that requires exactly that range.

state_contracts.md, "Local call summaries" (decision
`callee-result-body-summary`): "An exact checked call inside the same
fingerprinted verification unit may use the callee's written contract plus
checked body-derived result and state facts. A written `ensures` is a minimum
guarantee, not an instruction to forget stronger local facts." And: "Derive
and check a reusable summary against the body; each call substitutes its
evaluated actual arguments and establishes the entry conditions."

The summary is one more `ensures` clause of the helper, over its formals and
`result`: what every normal return establishes. Each return is described in
those names, as the comparisons known there that name `result` (`describe`,
the description a private helper's arrivals get in the other direction) and
the literal bounds the prover finds for the returned value. The clause keeps
each comparison every return's description holds and the widest of the
returns' bounds. It is added only after the prover and the kernel have
decided it on every normal return; it then owes one `NormalGuarantee` per
return and one `PublishedGuarantee` like an authored clause, and reaches
callers as ordinary `CalleeGuarantee` knowledge under the call's actuals.
Describing a return is inference, never evidence.

Which machines get one is elaboration's decision (`Elaboration::local_results`):
private machines of the subject package, which no other verification unit can
call. A public machine, a dependency, a requirement, a selected implementation
and a Build selection keep their written contract.

Helpers are summarized callees first, so a helper's summary can use those of
the helpers it calls. Nothing is assumed around a call cycle: a summary enters
a contract only once it is proved, so a machine never reads its own, and
"assuming a proposed summary around a cycle is not a proof" cannot happen.

Summaries run before input inference and once more after it
(`discharge_with`). A helper that omits `requires` is first summarized under
no input, and what it returns because of the input its callers establish is
a further clause the second run adds; a clause already in the contract is
not added again.

Limits. Only a scalar result is summarized, and only by comparisons that name
it: what it equals, what bounds it, how it relates to a scalar formal. A
helper whose returns differ is summarized by what they share, never by one
alternative per return: a helper that returns its input inside `[20, 60]`
and 40 otherwise is known to return a value in `[20, 60]`, not `value` or
40. A caller's proof splits at most once over a disjunction, and an inferred
input condition described from one could not be proved again at its own
arrival (`pass/calls/private_arrival_guarded_helper_range_exit` was rejected
there when alternatives were kept). Storage a helper writes through a
borrowed formal gets no summary.
*/

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, EdgeId, IntegerType, IntegerValue, MachineId, ObligationId,
    PlaceId, Proposition, ScalarTerm, ScalarType, StructuralCaseId, ValueId,
};
use terminal_check_tables::edges::Callee;
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::{Checked, Construct, ProgramPoint, RefusalReason};
use terminal_check_tables::{
    names::{ClauseText, Piece},
    program::{Elaboration, SourceOrigin, Unpositioned},
};
use terminal_psi::{ContractClause, TerminalMachine, TerminalMachineResult, Terminator};

use crate::{
    Bindings, Knowledge, Unbound, decide, entry_inputs, hypotheses, joins, prepare, prove, refusal,
    returned_for_result, terminal_machine,
};

/// Adds a proved result summary to the contract of each machine in
/// `Elaboration::local_results` that has one, callees first.
pub(crate) fn infer(
    elaboration: &mut Elaboration,
    knowledge: Knowledge,
    unreached: &mut crate::Unreached,
) -> Checked<()> {
    if elaboration.local_results.is_empty() {
        return Ok(());
    }
    for id in callees_first(elaboration)? {
        summarize(elaboration, id, knowledge, unreached)?;
    }
    Ok(())
}

fn foreign(id: MachineId, detail: &str) -> terminal_check_tables::Refusal {
    refusal(
        RefusalReason::ForeignIdentity,
        id,
        Construct::Machine,
        detail,
    )
}

/// The machines `id` calls directly, in invocation order.
fn callees(elaboration: &Elaboration, id: MachineId) -> Checked<Vec<MachineId>> {
    Ok(
        terminal_check_tables::row(&elaboration.tables.machines, id, id, Construct::Machine)?
            .invocations
            .values()
            .filter_map(|call| match call.callee {
                Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(callee)) => {
                    Some(callee)
                }
                Callee::Selected { .. }
                | Callee::Declared(semantic_vocabulary::TargetMachineCallable::Boundary(_)) => None,
            })
            .collect(),
    )
}

/// The summarized machines, each after every machine it calls that is not
/// still open when it is reached (depth-first postorder). A call cycle is
/// thereby cut at the member reached first.
fn callees_first(elaboration: &Elaboration) -> Checked<Vec<MachineId>> {
    let machines = &elaboration.tables.machines;
    let mut local = machines.map(|_, _| false);
    for id in &elaboration.local_results {
        *local
            .get_mut(*id)
            .ok_or_else(|| foreign(*id, "a summarized machine is past the module"))? = true;
    }
    let mut seen = machines.map(|_, _| false);
    let mut order = Vec::with_capacity(elaboration.local_results.len());
    for root in &elaboration.local_results {
        if seen.get(*root) != Some(&false) {
            continue;
        }
        *seen.get_mut(*root).expect("the root was read above") = true;
        let mut open = vec![(*root, callees(elaboration, *root)?)];
        while let Some((id, pending)) = open.last_mut() {
            let id = *id;
            match pending.pop() {
                Some(callee) => {
                    let reached = seen
                        .get_mut(callee)
                        .ok_or_else(|| foreign(id, "a call names a machine past the module"))?;
                    if !std::mem::replace(reached, true) {
                        open.push((callee, callees(elaboration, callee)?));
                    }
                }
                None => {
                    open.pop();
                    if local.get(id) == Some(&true) {
                        order.push(id);
                    }
                }
            }
        }
    }
    Ok(order)
}

/// A return's names on the caller's side of the contract: a scalar formal is
/// itself, and the returned value is `result`. Nothing else the body computed
/// has a name there.
struct Returned<'a> {
    lowered: &'a TerminalMachine,
    value: ValueId,
    result: &'a ScalarTerm,
}

impl Bindings for Returned<'_> {
    type Error = Unbound;
    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Unbound> {
        if self
            .lowered
            .parameters
            .iter()
            .any(|formal| formal.id == id && formal.scalar_type == scalar_type)
        {
            return Ok(ScalarTerm::value(id, scalar_type));
        }
        if id == self.value && self.result.scalar_type() == scalar_type {
            return Ok(self.result.clone());
        }
        Err(Unbound::Unproved(
            "a value of the body is neither a formal nor the returned value",
        ))
    }
    fn field(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        Err(Unbound::Unproved("storage has no name in a result summary"))
    }
    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Unbound> {
        Err(Unbound::Unproved("a case has no name in a result summary"))
    }
}

/// The parts of one return's description a summary keeps: each comparison
/// that names `result`, and names it only as a whole operand. A return's
/// guarantee replaces `result` there and nowhere deeper
/// (`returned_for_result`), so a comparison that names it inside a term could
/// not be proved on any return. A part about the formals alone, and a
/// disjunction, are the return's path condition and stay behind (see the
/// limits above).
fn about_result(description: Proposition, result: ValueId) -> Vec<Proposition> {
    let whole = |term: &ScalarTerm| {
        matches!(term, ScalarTerm::Value { .. }) || !term.any_value_id(|value| value == result)
    };
    let parts = match description {
        Proposition::Conjunction(parts) => parts,
        other => vec![other],
    };
    parts
        .into_iter()
        .filter(|part| match part {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                whole(left) && whole(right) && part.any_value_id(|value| value == result)
            }
            _ => false,
        })
        .collect()
}

/// The bound every return seen so far satisfies, after one more return whose
/// own bound is `next`: the one further out in the direction `outward`
/// (`Less` for a lower bound). `Some(None)` before any return; `None` once a
/// return has no bound, or two bounds cannot be compared.
fn widest(
    so_far: Option<Option<IntegerValue>>,
    next: Option<IntegerValue>,
    integer: IntegerType,
    outward: std::cmp::Ordering,
) -> Option<Option<IntegerValue>> {
    let next = next?;
    Some(Some(match so_far? {
        None => next,
        Some(held) => match integer.compare(next, held)? {
            ordering if ordering == outward => next,
            _ => held,
        },
    }))
}

fn summarize(
    elaboration: &mut Elaboration,
    id: MachineId,
    knowledge: Knowledge,
    unreached: &mut crate::Unreached,
) -> Checked<()> {
    let module = &elaboration.module;
    let lowered = terminal_machine(&module.machines, id, id)?;
    let TerminalMachineResult::Scalar(declared) = &lowered.result else {
        return Ok(());
    };
    let result = ScalarTerm::value(declared.id, declared.scalar_type);
    let exits: Vec<(EdgeId, ValueId)> = lowered
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Return { edge, value, .. } => Some((*edge, *value)),
            _ => None,
        })
        .collect();
    if exits.is_empty() {
        return Ok(());
    }
    // One machine's construction scratch, as private-input description
    // uses: the evidence tables stay without knowledge until discharge.
    let original =
        terminal_check_tables::row(&elaboration.tables.machines, id, id, Construct::Machine)?;
    let mut scratch = original.clone();
    let prepared = prepare(module, &elaboration.domains, id, &mut scratch, knowledge)?;
    let declared_joins = joins::prepare(
        lowered,
        id,
        &mut scratch,
        &prepared.graph,
        &prepared.context,
        &module.scalar_block_invariants,
        knowledge == Knowledge::All,
    )?;
    let references = prepared.references(&module.structural_types);
    let storage = crate::places::Storage::new(
        id,
        lowered,
        module,
        &elaboration.domains,
        &prepared.graph,
        &scratch,
        &prepared.types,
        &references,
    )?;
    let relations = knowledge != Knowledge::LiteralIntervals;
    let beyond = unreached.values(lowered, &scratch);
    let mut decided = (0..scratch.obligations.len())
        .map(|_| None)
        .collect::<Vec<_>>();
    joins::check(
        &joins::Machine {
            module,
            lowered,
            storage: &storage,
            id,
            types: &prepared.types,
            graph: &prepared.graph,
            context: &prepared.context,
            relations,
            declared: &declared_joins,
            unreached: &beyond,
        },
        &mut scratch,
        &mut decided,
    )?;
    // The identities the summary's rows take once it is added: its published
    // row, then one guarantee per return.
    let first = scratch.obligations.len() as u64 + 1;
    let row_identity = |offset: usize| {
        ObligationId::new(first + offset as u64)
            .ok_or_else(|| foreign(id, "a summary obligation identity overflows"))
    };
    // What every return establishes: a comparison each return's description
    // holds, and the widest of the returns' literal bounds.
    let mut common: Option<Vec<Proposition>> = None;
    let mut lowest = Some(None);
    let mut highest = Some(None);
    for (edge, value) in &exits {
        let point = ProgramPoint::Edge(*edge);
        let parts = about_result(
            entry_inputs::describe(
                &scratch,
                &prepared.graph,
                &prepared.context,
                id,
                point,
                &[(result.clone(), *value)],
                &mut Returned {
                    lowered,
                    value: *value,
                    result: &result,
                },
            )?,
            declared.id,
        );
        // What the returned value is bounded by, where the prover can say:
        // a cast, a sum or a shift of bounded formals has no fact of its own
        // to project, only the bounds its definition gives it.
        if let ScalarType::Integer(integer) = declared.scalar_type {
            let Ok(known) = hypotheses(&prepared.graph, &scratch, id, row_identity(0)?, point)
            else {
                return Ok(());
            };
            let mut prover = prove::Prover::new(
                &prepared.context,
                &known.assumptions,
                &known.axioms,
                &scratch.values,
                relations,
            );
            // A caller's goal on this result then depends on an operation
            // the integer rules do not reach, whatever is described of it.
            if prover.value_beyond_rules(*value, &beyond) {
                unreached.mark_result(id);
            }
            let (lower, upper) = prover.literal_bounds(*value);
            lowest = widest(lowest, lower, integer, std::cmp::Ordering::Less);
            highest = widest(highest, upper, integer, std::cmp::Ordering::Greater);
        }
        common = Some(match common {
            None => parts,
            Some(mut common) => {
                common.retain(|part| parts.contains(part));
                common
            }
        });
    }
    let mut parts = common.unwrap_or_default();
    if let ScalarType::Integer(integer) = declared.scalar_type {
        let literal = |value| ScalarTerm::Integer {
            scalar_type: integer,
            value,
        };
        if let Some(Some(lower)) = lowest {
            parts.push(Proposition::LessOrEqual(literal(lower), result.clone()));
        }
        if let Some(Some(upper)) = highest {
            parts.push(Proposition::LessOrEqual(result.clone(), literal(upper)));
        }
    }
    let Ok(summary) = entry_inputs::canonical(joins::connective(parts, true), id) else {
        return Ok(());
    };
    if matches!(summary, Proposition::Truth | Proposition::Falsehood)
        || lowered
            .contract
            .ensures
            .iter()
            .any(|clause| clause.proposition == summary)
    {
        return Ok(());
    }
    // Derived, now checked: the clause is added only when every normal
    // return establishes it by a certificate the kernel accepts. A return
    // that does not leaves the machine with its written contract.
    for (index, (edge, value)) in exits.iter().enumerate() {
        let goal = returned_for_result(&summary, declared.id, *value);
        if decide(
            &prepared.context,
            &prepared.graph,
            &scratch,
            id,
            row_identity(index + 1)?,
            ProgramPoint::Edge(*edge),
            goal,
            relations,
            &beyond,
        )
        .is_err()
        {
            return Ok(());
        }
    }
    let entry = lowered.entry;
    let clause = crate::position(lowered.contract.ensures.len(), id)?;
    let tables = elaboration
        .tables
        .machines
        .get_mut(id)
        .ok_or_else(|| foreign(id, "a summarized machine has no tables"))?;
    let sources = elaboration
        .sources
        .get_mut(id)
        .ok_or_else(|| foreign(id, "a summarized machine has no source correspondence"))?;
    let mut returns = Vec::with_capacity(exits.len());
    for (edge, _) in &exits {
        returns.push(*terminal_check_tables::row(
            &sources.edges,
            *edge,
            id,
            Construct::Edge(*edge),
        )?);
    }
    let mut mint = |kind, created_at, source: SourceOrigin| {
        let obligation = tables.obligations.push(ObligationRow { kind, created_at });
        if sources.obligations.push(source) == obligation {
            Ok(obligation)
        } else {
            Err(foreign(
                id,
                "a summary obligation and its source correspondence have different identities",
            ))
        }
    };
    let published = mint(
        ObligationKind::PublishedGuarantee { clause },
        ProgramPoint::BlockEntry(entry),
        SourceOrigin::Unpositioned(Unpositioned::MachineEntry),
    )?;
    for ((edge, _), source) in exits.iter().zip(returns) {
        mint(
            ObligationKind::NormalGuarantee {
                exit: *edge,
                clause,
                published,
            },
            ProgramPoint::Edge(*edge),
            source,
        )?;
    }
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| elaboration.module.machines.get_mut(position))
        .filter(|machine| machine.id == id)
        .ok_or_else(|| foreign(id, "a summarized machine is not at its position"))?
        .contract
        .ensures
        .push(ContractClause {
            obligation: published,
            proposition: summary,
        });
    // An inferred clause has no authored text; the position keeps every
    // authored clause's diagnostic aligned.
    if let Some(names) = elaboration.names.get_mut(id) {
        names.ensures.push(ClauseText(vec![Piece::Text(String::from(
            "inferred result summary",
        ))]));
    }
    Ok(())
}
