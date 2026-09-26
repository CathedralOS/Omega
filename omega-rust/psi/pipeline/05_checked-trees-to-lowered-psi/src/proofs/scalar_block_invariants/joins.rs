//! Propose guarantee/operation demands and literal ranges at acyclic scalar joins.
//!
//! Reconstruction deliberately intersects ordinary path facts. A lost join
//! equation needs checked arrivals, not unioned branch facts. Work backwards
//! from the unchanged guarantee using the destination scope as a stopping
//! boundary for equation transport. This is optional producer search: every
//! resulting predicate is separately proved on every actual arrival, and the
//! original guarantee remains an independent final obligation. No call body is
//! inspected and no source shape selects a compiler path.
//! Operation and call demands use the same arrival proofs as authored guarantees.
//! Mathematical exact-add questions propose scalar operand bounds; the original
//! arithmetic question is still proved separately after retaining those bounds.
//!
//! Downstream arrivals supply conditional demands for upstream joins. Preserve
//! their established Boolean polarities through call/copy equations; ordinary
//! aliases are not path guards. Never wrap a demand already covered by a header
//! conjunct, or repeatedly add guards around its own imported assertion. Values
//! absent from a header's scope must resolve through established arrival facts;
//! a later block cannot supply its not-yet-defined parameters to an earlier edge.
//!
//! Different literal arrivals lose their equalities at an ordinary intersection,
//! but their interval hull can still justify later arithmetic. Propose that hull
//! only when every actual scalar arrival supplies an exact same-type constant.
//! This is a seed, not authority: the common retention pass reconstructs and
//! proves every arrival, including SSA availability, before publishing the range.
//!
//! The hull is hypothesis, not product: header axioms never leave their machine,
//! so a machine owning no reconstructed obligation has nothing that can cite the
//! interval. Publishing the row anyway would manufacture only its own arrival
//! goals while marking the module proof-bearing, freezing every
//! parameter-carrying rewrite the reconstructed question cannot carry verbatim.
//! Defer the interval until a question in the same machine exists to answer; a
//! machine already under proof keeps the proposal because its refusal boundary
//! is already in force.

use std::collections::{BTreeMap, BTreeSet};

use proof_admission::{check_predicate_denotations, check_value_equality_denotation};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    OperationKind, OperationResult, ScalarBlockInvariant, TerminalMachine, TerminalModule,
    Terminator,
};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

pub(super) fn candidates(
    module: &TerminalModule,
    questions: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
    let Ok(validated) = terminal_verifier::validate_module(module) else {
        return candidates;
    };
    for machine in &module.machines {
        let Ok(context) = validated.value_context(machine) else {
            continue;
        };
        let Ok(components) = terminal_verifier::control_cycle_members(machine) else {
            continue;
        };
        let literals = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                let OperationKind::IntegerConstant { value } = &operation.kind else {
                    return None;
                };
                let OperationResult::Scalar(result) = &operation.result else {
                    return None;
                };
                Some((result.id, result.scalar_type, *value))
            })
            .collect::<Vec<_>>();
        let mut arrivals = BTreeMap::<_, usize>::new();
        let mut structural_targets = BTreeSet::new();
        for block in &machine.blocks {
            match &block.terminator {
                Terminator::Jump { target, .. } => *arrivals.entry(*target).or_default() += 1,
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for successor in [when_true, when_false] {
                        *arrivals.entry(successor.target).or_default() += 1;
                    }
                }
                Terminator::StructuralCase { cases, .. } => {
                    structural_targets.extend(cases.iter().map(|successor| successor.target));
                }
                _ => {}
            }
        }
        // A retained interval row creates its own arrival obligations, so it
        // alone can make this machine proof-bearing. With no machine-owned
        // question able to cite the hull, defer it rather than freeze the
        // rewrites for unconsumed evidence.
        let machine_has_question = questions
            .obligations()
            .iter()
            .any(|site| site.owner.machine() == machine.id);
        for header in &machine.blocks {
            if header.id == machine.entry
                || arrivals.get(&header.id).copied().unwrap_or(0) < 2
                || structural_targets.contains(&header.id)
                || components
                    .iter()
                    .any(|component| component.contains(&header.id))
            {
                continue;
            }
            let Ok(scope) = PropositionContext::from_value_types(
                machine
                    .parameters
                    .iter()
                    .chain(&header.parameters)
                    .map(|value| (value.id, value.scalar_type)),
            ) else {
                continue;
            };
            let allowed = machine
                .parameters
                .iter()
                .chain(&header.parameters)
                .map(|value| value.id)
                .collect::<BTreeSet<_>>();
            let mut imported = module
                .scalar_block_invariants
                .iter()
                .filter(|invariant| {
                    invariant.machine == machine.id && invariant.header == header.id
                })
                .map(|invariant| &invariant.predicate)
                .collect::<Vec<_>>();
            let mut cursor = 0;
            while cursor < imported.len() {
                let Some(next) = remaining.checked_sub(1) else {
                    return candidates;
                };
                *remaining = next;
                if let Proposition::Conjunction(members) = imported[cursor] {
                    if members.len() > *remaining {
                        return candidates;
                    }
                    imported.extend(members);
                }
                cursor += 1;
            }
            if machine_has_question {
                for position in 0..header.parameters.len() {
                    let Some(predicate) =
                        literal_arrival_range(machine, header, position, &literals, remaining)
                    else {
                        continue;
                    };
                    if !imported.contains(&&predicate) {
                        candidates.push(ScalarBlockInvariant {
                            machine: machine.id,
                            header: header.id,
                            predicate,
                            arrivals: Vec::new(),
                        });
                    }
                }
            }
            for site in questions
                .obligations()
                .iter()
                .filter(|site| match site.owner {
                    ReconstructedTerminalObligationOwner::ContractEnsures {
                        machine: owner,
                        ..
                    }
                    | ReconstructedTerminalObligationOwner::Operation { machine: owner, .. }
                    | ReconstructedTerminalObligationOwner::CallRequires {
                        machine: owner, ..
                    } => owner == machine.id,
                    ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
                        machine: owner,
                        header: destination,
                        ..
                    } => owner == machine.id && destination != header.id,
                    _ => false,
                })
            {
                let Some(next) = remaining.checked_sub(1) else {
                    return candidates;
                };
                *remaining = next;
                let Some(equations) = ArrivalEquations::new(&site.semantic_axioms, remaining)
                else {
                    continue;
                };
                let Some(mut predicate) = scoped_goal(
                    &site.obligation.proposition,
                    &equations,
                    &allowed,
                    &scope,
                    &context,
                    None,
                    remaining,
                ) else {
                    continue;
                };
                let Ok(canonical) = super::canonical_predicate(predicate) else {
                    continue;
                };
                predicate = canonical;
                if imported.contains(&&predicate) {
                    continue;
                }
                if matches!(
                    site.owner,
                    ReconstructedTerminalObligationOwner::ScalarBlockInvariant { .. }
                        | ReconstructedTerminalObligationOwner::Operation { .. }
                        | ReconstructedTerminalObligationOwner::CallRequires { .. }
                ) {
                    let mut guards = site
                        .semantic_axioms
                        .iter()
                        // Retain established Boolean polarities for this path.
                        // Copy equations and imported assertions describe values,
                        // not whether the downstream arrival is taken.
                        .filter(|fact| {
                            matches!(fact, Proposition::Equal(_, ScalarTerm::Boolean(_)))
                        })
                        // A call or copy between joins can carry the selected
                        // condition. Transport guards through the same exact
                        // arrival equations as the goal, stopping at this scope.
                        .filter_map(|fact| {
                            scoped_goal(
                                fact,
                                &equations,
                                &allowed,
                                &scope,
                                &context,
                                Some(fact),
                                remaining,
                            )
                        })
                        .filter_map(|fact| super::canonical_predicate(fact).ok())
                        .filter(|fact| !imported.contains(&fact) && uses_header(fact, header))
                        .collect::<Vec<_>>();
                    if !guards.is_empty() {
                        // The downstream arrival is conditional. Demand its goal
                        // only under the already-established facts expressible at
                        // this earlier join, not unconditionally on every path.
                        let guarded = Proposition::Implication {
                            premise: Box::new(if guards.len() == 1 {
                                guards.remove(0)
                            } else {
                                Proposition::Conjunction(guards)
                            }),
                            conclusion: Box::new(predicate),
                        };
                        let Ok(normalized) =
                            check_predicate_denotations(&scope, &guarded, &[], &[])
                        else {
                            continue;
                        };
                        predicate = normalized.goal().clone();
                    }
                }
                let Ok(predicate) = super::canonical_predicate(predicate) else {
                    continue;
                };
                // Entry-only tautologies do not justify adding a join roster.
                if uses_header(&predicate, header) && !imported.contains(&&predicate) {
                    candidates.push(ScalarBlockInvariant {
                        machine: machine.id,
                        header: header.id,
                        predicate,
                        arrivals: Vec::new(),
                    });
                }
            }
        }
    }
    candidates
}

fn literal_arrival_range(
    machine: &TerminalMachine,
    header: &terminal_psi::Block,
    position: usize,
    literals: &[(ValueId, ScalarType, IntegerValue)],
    remaining: &mut usize,
) -> Option<Proposition> {
    let parameter = header.parameters.get(position)?;
    let ScalarType::Integer(integer) = parameter.scalar_type else {
        return None;
    };
    let mut minimum = integer.maximum_value();
    let mut maximum = integer.minimum_value();
    let mut arrivals = 0usize;
    let mut visit = |target, arguments: &[ValueId]| -> Option<()> {
        if target != header.id {
            return Some(());
        }
        *remaining = remaining.checked_sub(1)?;
        let argument = *arguments.get(position)?;
        let mut definitions = literals.iter().filter(|(value, _, _)| *value == argument);
        let (_, scalar_type, value) = definitions.next()?;
        if definitions.next().is_some()
            || *scalar_type != parameter.scalar_type
            || !integer.admits(*value)
        {
            return None;
        }
        minimum = minimum.min(*value);
        maximum = maximum.max(*value);
        arrivals += 1;
        Some(())
    };
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => visit(*target, arguments)?,
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                visit(when_true.target, &when_true.arguments)?;
                visit(when_false.target, &when_false.arguments)?;
            }
            Terminator::StructuralCase { cases, .. }
                if cases.iter().any(|case| case.target == header.id) =>
            {
                return None;
            }
            _ => {}
        }
    }
    if arrivals < 2 || (minimum == integer.minimum_value() && maximum == integer.maximum_value()) {
        return None;
    }
    let subject = ScalarTerm::value(parameter.id, parameter.scalar_type);
    Some(Proposition::Conjunction(vec![
        Proposition::LessOrEqual(ScalarTerm::integer(integer, minimum).ok()?, subject.clone()),
        Proposition::LessOrEqual(subject, ScalarTerm::integer(integer, maximum).ok()?),
    ]))
}

pub(super) fn uses_header(predicate: &Proposition, header: &terminal_psi::Block) -> bool {
    let mut uses_header = false;
    predicate.visit_value_ids(|value| {
        uses_header |= header
            .parameters
            .iter()
            .any(|parameter| parameter.id == value);
    });
    uses_header
}

pub(super) fn scoped_goal(
    goal: &Proposition,
    arrival: &ArrivalEquations<'_>,
    allowed: &BTreeSet<ValueId>,
    scope: &PropositionContext,
    context: &PropositionContext,
    excluded: Option<&Proposition>,
    remaining: &mut usize,
) -> Option<Proposition> {
    *remaining = remaining.checked_sub(1)?;
    let mut pending = Vec::new();
    if !goal.visit_value_ids(|value| pending.push(value)) {
        return None;
    }
    let mut visited = BTreeSet::new();
    let mut equations = Vec::new();
    while let Some(value) = pending.pop() {
        *remaining = remaining.checked_sub(1)?;
        if allowed.contains(&value) || !visited.insert(value) {
            continue;
        }
        // Canonical call guarantees can spell argument = result. Prefer an
        // actual defining equation, then orient an alias towards its source.
        // This proposes a predicate only; arrival proofs still cite/check the
        // original equations, including explicit symmetry where needed.
        let definition = arrival
            .definitions
            .get(&value)
            .copied()
            // A guard's selected truth is not its defining computation. Using
            // that very fact as a definition would turn every transported guard
            // into Truth instead of carrying its call/copy alias to the header.
            .filter(|definition| Some(*definition) != excluded)
            .or_else(|| arrival.reversed.get(&value))?;
        if !definition.visit_value_ids(|dependency| {
            if dependency != value {
                pending.push(dependency);
            }
        }) {
            return None;
        }
        equations.push(definition);
    }
    // The transport validates all participating values before substitution.
    // A narrow final context then rejects any unresolved body-local identity.
    // An arrival already in scope needs no equations, but still passes through
    // the same bounded denotation/normalization rather than an unchecked clone.
    let mut predicate = if equations.is_empty() {
        check_predicate_denotations(context, goal, &[], &[])
            .ok()?
            .goal()
            .clone()
    } else {
        check_value_equality_denotation(context, goal, equations.iter().copied()).ok()?
    };
    transport_math_leaves(&mut predicate, &equations, remaining)?;
    scalar_math_demands(&mut predicate, remaining)?;
    scope.validate(&predicate).ok()?;
    Some(predicate)
}

/// Join invariants admit scalar predicates, not mathematical syntax. Pull an
/// exact-add demand back to a bound on its nonliteral operand. Saturation asks
/// for the carrier endpoint when the mathematical bound is outside the carrier;
/// this is only a proposal, never evidence for the original operation.
fn scalar_math_demands(predicate: &mut Proposition, remaining: &mut usize) -> Option<()> {
    let mut pending = vec![predicate];
    while let Some(predicate) = pending.pop() {
        *remaining = remaining.checked_sub(1)?;
        match predicate {
            Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
                pending.extend(members);
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                pending.extend([premise.as_mut(), conclusion.as_mut()]);
            }
            Proposition::IntegerMathLessOrEqual(left, right) => {
                if let Some(scalar) = crate::proofs::contract_predicates::scalar_relation_for_math(
                    &Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()),
                ) {
                    *predicate = scalar;
                    continue;
                }
                let (sum, endpoint, lower) = match (&*left, &*right) {
                    (sum @ IntegerMathTerm::Add(..), IntegerMathTerm::IntegerLiteral(endpoint)) => {
                        (sum, endpoint, false)
                    }
                    (IntegerMathTerm::IntegerLiteral(endpoint), sum @ IntegerMathTerm::Add(..)) => {
                        (sum, endpoint, true)
                    }
                    _ => return None,
                };
                let IntegerMathTerm::Add(left, right) = sum else {
                    return None;
                };
                let (source_type, value, literal) = match (left.as_ref(), right.as_ref()) {
                    (
                        IntegerMathTerm::MathValue { source_type, value },
                        IntegerMathTerm::IntegerLiteral(literal),
                    )
                    | (
                        IntegerMathTerm::IntegerLiteral(literal),
                        IntegerMathTerm::MathValue { source_type, value },
                    ) => (*source_type, *value, literal),
                    _ => return None,
                };
                let bound = source_type.saturating_sub(
                    endpoint.as_integer_value(source_type)?,
                    literal.as_integer_value(source_type)?,
                )?;
                let subject = ScalarTerm::value(value, ScalarType::Integer(source_type));
                let bound = ScalarTerm::integer(source_type, bound).ok()?;
                *predicate = if lower {
                    Proposition::LessOrEqual(bound, subject)
                } else {
                    Proposition::LessOrEqual(subject, bound)
                };
            }
            Proposition::IntegerMathEqual(..) | Proposition::IntegerMathLessThan(..) => {
                *predicate =
                    crate::proofs::contract_predicates::scalar_relation_for_math(predicate)?;
            }
            _ => {}
        }
    }
    Some(())
}

/// Scalar denotation transport deliberately leaves mathematical terms alone.
/// For a proposed join demand, same-carrier aliases and literals can also name
/// their mathematical values. Do not interpret machine arithmetic as unbounded
/// arithmetic here. This is candidate discovery only: every arrival and the
/// original operation obligation still require independent certificates.
fn transport_math_leaves(
    predicate: &mut Proposition,
    equations: &[&Proposition],
    remaining: &mut usize,
) -> Option<()> {
    let mut propositions = vec![predicate];
    while let Some(predicate) = propositions.pop() {
        *remaining = remaining.checked_sub(1)?;
        match predicate {
            Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
                propositions.extend(members);
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                propositions.extend([premise.as_mut(), conclusion.as_mut()]);
            }
            Proposition::IntegerMathEqual(left, right)
            | Proposition::IntegerMathLessThan(left, right)
            | Proposition::IntegerMathLessOrEqual(left, right) => {
                let mut terms = vec![left, right];
                while let Some(term) = terms.pop() {
                    *remaining = remaining.checked_sub(1)?;
                    match term {
                        IntegerMathTerm::MathValue { source_type, value } => {
                            while let Some(definition) =
                                equations.iter().find_map(|equation| match equation {
                                    Proposition::Equal(ScalarTerm::Value { id, .. }, right)
                                        if id == value =>
                                    {
                                        Some(right)
                                    }
                                    _ => None,
                                })
                            {
                                // Alias cycles exhaust this shared search budget.
                                *remaining = remaining.checked_sub(1)?;
                                match definition {
                                    ScalarTerm::Value { id, scalar_type }
                                        if *scalar_type == ScalarType::Integer(*source_type) =>
                                    {
                                        *value = *id;
                                    }
                                    ScalarTerm::Integer {
                                        scalar_type,
                                        value: literal,
                                    } if scalar_type == source_type => {
                                        *term = IntegerMathTerm::literal(*literal);
                                        break;
                                    }
                                    _ => return None,
                                }
                            }
                        }
                        IntegerMathTerm::Add(left, right)
                        | IntegerMathTerm::Subtract(left, right)
                        | IntegerMathTerm::Multiply(left, right) => {
                            terms.extend([left.as_mut(), right.as_mut()]);
                        }
                        IntegerMathTerm::ShiftLeft { value, count } => {
                            terms.extend([value.as_mut(), count.as_mut()]);
                        }
                        IntegerMathTerm::IntegerLiteral(_) => {}
                    }
                }
            }
            _ => {}
        }
    }
    Some(())
}

/// The goal and its path guards share one arrival's defining equations. Build
/// their index once; neither conditional implications nor alternative branches
/// become unconditional definitions.
pub(super) struct ArrivalEquations<'input> {
    definitions: BTreeMap<ValueId, &'input Proposition>,
    reversed: BTreeMap<ValueId, Proposition>,
}

impl<'input> ArrivalEquations<'input> {
    pub(super) fn new(axioms: &'input [Proposition], remaining: &mut usize) -> Option<Self> {
        if axioms.len() > *remaining {
            return None;
        }
        let mut pending = axioms.iter().collect::<Vec<_>>();
        let mut definitions = BTreeMap::new();
        let mut reversed = BTreeMap::new();
        while let Some(fact) = pending.pop() {
            *remaining = remaining.checked_sub(1)?;
            match fact {
                Proposition::Conjunction(members) => {
                    if members.len() > *remaining {
                        return None;
                    }
                    pending.extend(members);
                }
                Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right) if left != right => {
                    // Reverse traversal with replacement retains the first
                    // defining equation, matching value-equality proof production.
                    definitions.insert(*id, fact);
                    if let ScalarTerm::Value { id: result, .. } = right {
                        reversed.insert(*result, Proposition::Equal(right.clone(), left.clone()));
                    }
                }
                _ => {}
            }
        }
        Some(Self {
            definitions,
            reversed,
        })
    }
}
