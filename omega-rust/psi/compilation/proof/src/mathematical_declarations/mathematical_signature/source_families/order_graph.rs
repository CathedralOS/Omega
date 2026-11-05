//! Check a comparison against the structural rank of its actual source values.
//!
//! Source cases and calls come from the complete execution graph. Their integer
//! equations describe ranks, not new source contracts. The W constructor for
//! each case retains the association between that source return and its graph
//! inhabitant; the invariant alone would not establish this association.

use super::terms::{self, Case, Key};
use numerics::bignum::BigInt;
use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Signature,
    SourceIntegerTheory, Term, TermArena, TermHandle, judgment_assumption_closure,
};
use validation::computed_order::{SourceOrderBody, SourceOrderValue, SourceOutcome};

#[derive(Debug, Clone)]
pub(super) struct CheckedGraph {
    pub family: IndexedFamily,
    pub theorem: TermHandle,
    pub ty: TermHandle,
    pub invariant: TermHandle,
    pub constructors: Vec<(TermHandle, TermHandle)>,
    pub assumption_closure: Vec<u32>,
}

pub(super) enum GraphFailure {
    Unavailable(String),
    Kernel(String),
}

/// Untrusted term construction over one append-only integer vocabulary.
/// The kernel checks every returned theorem and constructor schema.
struct Producer<'a> {
    arena: &'a mut TermArena,
    declarations: &'a mut Vec<Declaration>,
    theory: &'a mut SourceIntegerTheory,
    integer: TermHandle,
    function: TermHandle,
    zero: TermHandle,
    one: TermHandle,
}

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}
fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}
fn tuple(arena: &mut TermArena, values: &[TermHandle]) -> Result<TermHandle, GraphFailure> {
    let Some((&last, prefix)) = values.split_last() else {
        return Err(GraphFailure::Unavailable(
            "the order graph has no index coordinates".into(),
        ));
    };
    Ok(prefix.iter().rev().fold(last, |second, &first| {
        arena.insert(Term::Pair { first, second })
    }))
}
fn projection(
    arena: &mut TermArena,
    mut value: TermHandle,
    position: usize,
    count: usize,
) -> TermHandle {
    for _ in 0..position {
        value = arena.insert(Term::Snd { pair: value });
    }
    if position + 1 < count {
        value = arena.insert(Term::Fst { pair: value });
    }
    value
}

impl Producer<'_> {
    fn literal(&mut self, value: usize) -> Result<TermHandle, GraphFailure> {
        self.theory
            .literal(
                self.arena,
                self.declarations,
                BigInt::from_i128(value as i128),
            )
            .map_err(|error| {
                GraphFailure::Unavailable(format!(
                    "the order graph numeral is unavailable: {error:?}"
                ))
            })
    }
    fn binary(&mut self, declaration: u32, left: TermHandle, right: TermHandle) -> TermHandle {
        let operation = constant(self.arena, declaration);
        let function = apply(self.arena, operation, left);
        apply(self.arena, function, right)
    }
    fn add(&mut self, left: TermHandle, right: TermHandle) -> TermHandle {
        self.binary(self.theory.vocabulary().add, left, right)
    }
    fn le(&mut self, left: TermHandle, right: TermHandle) -> TermHandle {
        self.binary(self.theory.vocabulary().less_or_equal, left, right)
    }
    fn equation(&mut self, left: TermHandle, right: TermHandle) -> TermHandle {
        self.arena.insert(Term::Id {
            ty: self.integer,
            left,
            right,
        })
    }
    fn conjunction(&mut self, propositions: &[TermHandle]) -> TermHandle {
        let mut result = self.equation(self.zero, self.zero);
        for &domain in propositions.iter().rev() {
            let codomain = proof_admission::shift(self.arena, result, 0, 1);
            result = self.arena.insert(Term::Sigma { domain, codomain });
        }
        result
    }
    fn close(&mut self, mut value: TermHandle, domains: &[TermHandle]) -> TermHandle {
        for &domain in domains.iter().rev() {
            value = self.arena.insert(Term::Lambda {
                domain,
                body: value,
            });
        }
        value
    }
    fn identity_shift(&mut self) -> TermHandle {
        let body = self.arena.insert(Term::Variable(0));
        self.arena.insert(Term::Lambda {
            domain: self.integer,
            body,
        })
    }
    fn successor_shift(&mut self, child: TermHandle) -> TermHandle {
        let child = proof_admission::shift(self.arena, child, 0, 1);
        let value = self.arena.insert(Term::Variable(0));
        let value = self.add(value, self.one);
        let body = apply(self.arena, child, value);
        self.arena.insert(Term::Lambda {
            domain: self.integer,
            body,
        })
    }
    fn source_value(
        &mut self,
        value: &SourceOrderValue,
        variables: &[(TermHandle, TermHandle)],
    ) -> Result<(TermHandle, TermHandle), GraphFailure> {
        match value {
            SourceOrderValue::Variable(position) => {
                variables.get(*position).copied().ok_or_else(|| {
                    GraphFailure::Unavailable(
                        "the order graph lost an exact source value binder".into(),
                    )
                })
            }
            SourceOrderValue::Base => Ok((self.zero, self.identity_shift())),
            SourceOrderValue::Successor(child) => {
                let (rank, shift) = self.source_value(child, variables)?;
                Ok((self.add(rank, self.one), self.successor_shift(shift)))
            }
        }
    }
    fn allowance(&mut self, count: usize) -> (TermHandle, TermHandle) {
        let mut rank = self.zero;
        let mut shift = self.identity_shift();
        for _ in 0..count {
            rank = self.add(rank, self.one);
            shift = self.successor_shift(shift);
        }
        (rank, shift)
    }
    fn shift_laws(&mut self, rank: TermHandle, function: TermHandle) -> TermHandle {
        let nonnegative = self.le(self.zero, rank);
        let at_zero = apply(self.arena, function, self.zero);
        let zero = self.equation(at_zero, rank);
        let shifted = proof_admission::shift(self.arena, function, 0, 1);
        let value = self.arena.insert(Term::Variable(0));
        let successor = self.add(value, self.one);
        let left = apply(self.arena, shifted, successor);
        let at_value = apply(self.arena, shifted, value);
        let right = self.add(at_value, self.one);
        let equation = self.equation(left, right);
        let successor_law = self.arena.insert(Term::Pi {
            domain: self.integer,
            codomain: equation,
        });
        let left = self.arena.insert(Term::Variable(1));
        let right = self.arena.insert(Term::Variable(0));
        let premise = self.le(left, right);
        let shifted = proof_admission::shift(self.arena, function, 0, 3);
        let left = self.arena.insert(Term::Variable(2));
        let right = self.arena.insert(Term::Variable(1));
        let left = apply(self.arena, shifted, left);
        let right = apply(self.arena, shifted, right);
        let conclusion = self.le(left, right);
        let implication = self.arena.insert(Term::Pi {
            domain: premise,
            codomain: conclusion,
        });
        let right = self.arena.insert(Term::Pi {
            domain: self.integer,
            codomain: implication,
        });
        let monotone = self.arena.insert(Term::Pi {
            domain: self.integer,
            codomain: right,
        });
        self.conjunction(&[nonnegative, zero, successor_law, monotone])
    }
    fn typed_equation(
        &mut self,
        ty: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        self.arena.insert(Term::Id { ty, left, right })
    }

    fn check(&mut self, term: TermHandle, ty: TermHandle, role: &str) -> Result<(), GraphFailure> {
        let context = Context::empty()
            .with_signature(Signature::from_declarations(self.declarations.clone()));
        proof_admission::check_type(
            self.arena,
            &context,
            term,
            ty,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .map_err(|error| {
            GraphFailure::Kernel(format!(
                "generated order {role} fails kernel checking: {error:?}"
            ))
        })
    }
    fn placeholder(
        &mut self,
        ty: TermHandle,
        key: Key,
        parameters: &mut Vec<(u32, Key, TermHandle)>,
    ) -> TermHandle {
        let position = self.declarations.len() as u32;
        self.declarations.push(Declaration {
            level_arity: 0,
            ty,
            body: None,
        });
        parameters.push((position, key, ty));
        constant(self.arena, position)
    }
    fn abstracted(
        &mut self,
        term: TermHandle,
        ty: TermHandle,
        parameters: &[(u32, Key, TermHandle)],
        role: &str,
    ) -> Result<(TermHandle, TermHandle), GraphFailure> {
        let result = terms::abstract_parameters(self.arena, term, ty, parameters)
            .map_err(|error| GraphFailure::Kernel(error.into()))?;
        if !terms::excludes_parameters(
            self.arena,
            self.declarations,
            &[result.0, result.1],
            parameters,
        ) {
            return Err(GraphFailure::Kernel(format!(
                "the order {role} retains a temporary value or evidence assumption"
            )));
        }
        self.check(result.0, result.1, role)?;
        Ok(result)
    }

    fn premise_type(&mut self, mut goal: TermHandle, assumptions: &[TermHandle]) -> TermHandle {
        for &domain in assumptions.iter().rev() {
            let codomain = proof_admission::shift(self.arena, goal, 0, 1);
            goal = self.arena.insert(Term::Pi { domain, codomain });
        }
        goal
    }
    fn without_evidence_binder(&mut self, body: TermHandle) -> Option<TermHandle> {
        // Distinct syntactic substitutions check whether the binder occurs;
        // beta conversion is deliberately not used to erase a dependency.
        let zero = self.arena.insert(Term::TwoZero);
        let one = self.arena.insert(Term::TwoOne);
        let left = proof_admission::substitute(self.arena, body, zero);
        let right = proof_admission::substitute(self.arena, body, one);
        self.arena.structurally_equal(left, right).then_some(left)
    }
    fn describe_goal(&self, term: TermHandle, remaining: &mut usize) -> String {
        if *remaining == 0 {
            return "…".into();
        }
        *remaining -= 1;
        if self.arena.structurally_equal(term, self.zero) {
            return "0".into();
        }
        if self.arena.structurally_equal(term, self.one) {
            return "1".into();
        }
        match self.arena.get(term) {
            Term::Constant { declaration, .. } => {
                let vocabulary = self.theory.vocabulary();
                if declaration == vocabulary.carrier {
                    "Int".into()
                } else if declaration == vocabulary.add {
                    "add".into()
                } else if declaration == vocabulary.less_or_equal {
                    "le".into()
                } else if declaration == vocabulary.less_than {
                    "lt".into()
                } else {
                    format!("constant#{declaration}")
                }
            }
            Term::Variable(position) => format!("bound#{position}"),
            Term::Apply { function, argument } => format!(
                "({} {})",
                self.describe_goal(function, remaining),
                self.describe_goal(argument, remaining)
            ),
            Term::Id { ty, left, right } => format!(
                "(equal {} {} {})",
                self.describe_goal(ty, remaining),
                self.describe_goal(left, remaining),
                self.describe_goal(right, remaining)
            ),
            Term::Pi { domain, codomain } => format!(
                "(forall {} {})",
                self.describe_goal(domain, remaining),
                self.describe_goal(codomain, remaining)
            ),
            Term::Sigma { domain, codomain } => format!(
                "(pair {} {})",
                self.describe_goal(domain, remaining),
                self.describe_goal(codomain, remaining)
            ),
            Term::Lambda { domain, body } => format!(
                "(lambda {} {})",
                self.describe_goal(domain, remaining),
                self.describe_goal(body, remaining)
            ),
            Term::Fst { pair } => format!("(first {})", self.describe_goal(pair, remaining)),
            Term::Snd { pair } => format!("(rest {})", self.describe_goal(pair, remaining)),
            other => format!("{other:?}"),
        }
    }

    fn derive(
        &mut self,
        goal: TermHandle,
        assumptions: &[TermHandle],
        depth: usize,
    ) -> Result<Option<TermHandle>, GraphFailure> {
        if depth > 128 {
            return Err(GraphFailure::Unavailable(
                "the structured order proof exceeds its binder depth".into(),
            ));
        }
        let signature = Signature::from_declarations(self.declarations.clone());
        let goal = proof_admission::weak_head_normalize(
            self.arena,
            &signature,
            goal,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .map_err(|error| {
            GraphFailure::Kernel(format!(
                "the generated order goal cannot normalize: {error:?}"
            ))
        })?;
        match self.arena.get(goal) {
            Term::Sigma { domain, codomain } => {
                let Some(rest) = self.without_evidence_binder(codomain) else {
                    return Ok(None);
                };
                let Some(first) = self.derive(domain, assumptions, depth + 1)? else {
                    return Ok(None);
                };
                let Some(second) = self.derive(rest, assumptions, depth + 1)? else {
                    return Ok(None);
                };
                Ok(Some(terms::pair_under_premises(
                    self.arena,
                    first,
                    second,
                    assumptions,
                )))
            }
            Term::Pi { domain, codomain }
                if self.arena.structurally_equal(domain, self.integer) =>
            {
                let mut parameters = Vec::new();
                let value = self.placeholder(domain, Key::Graph(0), &mut parameters);
                let body = proof_admission::substitute(self.arena, codomain, value);
                let Some(proof) = self.derive(body, assumptions, depth + 1)? else {
                    return Ok(None);
                };
                let ty = self.premise_type(body, assumptions);
                let (function, _) =
                    self.abstracted(proof, ty, &parameters, "quantified order value")?;
                Ok(Some(terms::value_after_premises(
                    self.arena,
                    function,
                    domain,
                    assumptions,
                )))
            }
            Term::Pi { domain, codomain } => {
                let Some(body) = self.without_evidence_binder(codomain) else {
                    return Ok(None);
                };
                let mut premises = assumptions.to_vec();
                premises.push(domain);
                self.derive(body, &premises, depth + 1)
            }
            _ => {
                let proof = self
                    .theory
                    .predicate_from_assumptions(self.arena, self.declarations, goal, assumptions)
                    .map_err(|error| {
                        GraphFailure::Unavailable(format!(
                            "the order case arithmetic is unsupported: {error:?}"
                        ))
                    })?;
                if proof.is_none() {
                    let goal = self.describe_goal(goal, &mut 256);
                    let premises = assumptions
                        .iter()
                        .map(|&premise| self.describe_goal(premise, &mut 512))
                        .collect::<Vec<_>>();
                    return Err(GraphFailure::Unavailable(format!(
                        "the order leaf has no checked derivation: {goal}; supplied premises: {premises:?}"
                    )));
                }
                Ok(proof)
            }
        }
    }

    fn checked_family(
        &mut self,
        common: &[(Key, TermHandle)],
        cases: &[Case<Key>],
        invariant: impl FnOnce(&mut Self, &[TermHandle]) -> TermHandle,
    ) -> Result<CheckedGraph, GraphFailure> {
        if cases.is_empty() {
            return Err(GraphFailure::Unavailable(
                "the order graph has no source return cases".into(),
            ));
        }
        let indices = common.iter().map(|(key, _)| *key).collect::<Vec<_>>();
        let family = terms::description(self.arena, common, &indices, cases, &[])
            .and_then(|family| terms::intern_description(self.arena, self.declarations, family))
            .map_err(|error| GraphFailure::Kernel(error.into()))?;
        let index = self.arena.insert(Term::Variable(0));
        let coordinates = (0..common.len())
            .map(|position| projection(self.arena, index, position, common.len()))
            .collect::<Vec<_>>();
        let goal = invariant(self, &coordinates);
        let invariant = self.arena.insert(Term::Lambda {
            domain: family.index,
            body: goal,
        });
        let invariant = terms::intern_invariant(self.arena, self.declarations, &family, invariant);
        let mut lemmas = Vec::new();
        let mut constructors = Vec::new();
        for (ordinal, case) in cases.iter().enumerate() {
            let fields = common
                .iter()
                .chain(&case.payload)
                .copied()
                .collect::<Vec<_>>();
            let mut parameters = Vec::new();
            let values = fields
                .iter()
                .map(|&(key, ty)| self.placeholder(ty, key, &mut parameters))
                .collect::<Vec<_>>();
            let mut predicate = case.constraints;
            for &value in &values {
                predicate = apply(self.arena, predicate, value);
            }
            let index = tuple(self.arena, &values[..common.len()])?;
            let goal = apply(self.arena, invariant, index);
            let mut assumptions = vec![predicate];
            let mut child_indices = Vec::new();
            for &function in &case.child_indices {
                let mut index = function;
                for &value in &values {
                    index = apply(self.arena, index, value);
                }
                child_indices.push(index);
                assumptions.push(apply(self.arena, invariant, index));
            }
            let proof = self.derive(goal, &assumptions, 0)?.ok_or_else(|| {
                GraphFailure::Unavailable(format!(
                    "source return case {ordinal} has no checked order derivation"
                ))
            })?;
            let mut ty = goal;
            for &domain in assumptions.iter().rev() {
                let codomain = proof_admission::shift(self.arena, ty, 0, 1);
                ty = self.arena.insert(Term::Pi { domain, codomain });
            }
            self.check(proof, ty, "case derivation")?;
            let (lemma, ty) = self.abstracted(proof, ty, &parameters, "case abstraction")?;
            let position = self.declarations.len() as u32;
            self.declarations
                .push(Declaration::definition(0, ty, lemma));
            lemmas.push(constant(self.arena, position));

            // The source return supplies this complete label, including its
            // equations, and one actual graph child for every ordered call.
            let predicate_value =
                self.placeholder(predicate, Key::Graph(fields.len()), &mut parameters);
            let mut children = Vec::new();
            for (child, &index) in child_indices.iter().enumerate() {
                let ty = family.indexed_w(self.arena, index);
                children.push(self.placeholder(
                    ty,
                    Key::Graph(fields.len() + 1 + child),
                    &mut parameters,
                ));
            }
            let mut label = predicate_value;
            for &value in values[common.len()..].iter().rev() {
                label = self.arena.insert(Term::Pair {
                    first: value,
                    second: label,
                });
            }
            if ordinal + 1 < cases.len() {
                let tag = self.arena.insert(Term::TwoZero);
                label = self.arena.insert(Term::Pair {
                    first: tag,
                    second: label,
                });
            }
            for _ in 0..ordinal {
                let tag = self.arena.insert(Term::TwoOne);
                label = self.arena.insert(Term::Pair {
                    first: tag,
                    second: label,
                });
            }
            for &value in values[..common.len()].iter().rev() {
                label = self.arena.insert(Term::Pair {
                    first: value,
                    second: label,
                });
            }
            self.check(label, family.carrier, "source return label")?;
            let supplied = terms::constructor_children(self.arena, &family, label, &children)
                .map_err(|error| GraphFailure::Kernel(error.into()))?;
            let constructor = family.sup(self.arena, label, supplied);
            let ty = family.indexed_w(self.arena, index);
            constructors.push(self.abstracted(
                constructor,
                ty,
                &parameters,
                "source return constructor",
            )?);
        }
        let (theorem, ty, step, step_type) = terms::induction(
            self.arena,
            &family,
            common,
            &indices,
            cases,
            invariant,
            &lemmas,
            self.declarations,
            &[],
        )
        .map_err(|error| GraphFailure::Kernel(error.into()))?;
        self.check(step, step_type, "induction step")?;
        self.check(theorem, ty, "induction theorem")?;
        let signature = Signature::from_declarations(self.declarations.clone());
        let mut roots = vec![theorem, ty];
        roots.extend(constructors.iter().flat_map(|&(term, ty)| [term, ty]));
        let assumption_closure = judgment_assumption_closure(self.arena, &signature, &roots);
        Ok(CheckedGraph {
            family,
            theorem,
            ty,
            invariant,
            constructors,
            assumption_closure: assumption_closure.into_iter().collect(),
        })
    }
}

fn producer<'a>(
    arena: &'a mut TermArena,
    declarations: &'a mut Vec<Declaration>,
    theory: &'a mut SourceIntegerTheory,
) -> Result<Producer<'a>, GraphFailure> {
    let integer = constant(arena, theory.vocabulary().carrier);
    let function = arena.insert(Term::Pi {
        domain: integer,
        codomain: integer,
    });
    let zero = theory
        .literal(arena, declarations, BigInt::from_i128(0))
        .map_err(|error| {
            GraphFailure::Unavailable(format!("zero rank is unavailable: {error:?}"))
        })?;
    let one = theory
        .literal(arena, declarations, BigInt::from_i128(1))
        .map_err(|error| {
            GraphFailure::Unavailable(format!("successor rank is unavailable: {error:?}"))
        })?;
    Ok(Producer {
        arena,
        declarations,
        theory,
        integer,
        function,
        zero,
        one,
    })
}

fn coordinates(arity: usize, integer: TermHandle, function: TermHandle) -> Vec<(Key, TermHandle)> {
    // Callable tag, allowance rank/shift, each input rank/shift, result rank/shift.
    let mut types = vec![integer];
    for _ in 0..arity + 2 {
        types.extend([integer, function]);
    }
    types
        .into_iter()
        .enumerate()
        .map(|(position, ty)| (Key::Graph(position), ty))
        .collect()
}

pub(super) fn comparison_graph(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    theory: &mut SourceIntegerTheory,
    bodies: &[SourceOrderBody],
    allowances: &[usize],
) -> Result<CheckedGraph, GraphFailure> {
    if bodies.len() != allowances.len() || bodies.is_empty() {
        return Err(GraphFailure::Unavailable(
            "the comparison source roster is incomplete".into(),
        ));
    }
    let mut p = producer(arena, declarations, theory)?;
    let arity = bodies
        .iter()
        .map(|body| body.parameter_types.len())
        .max()
        .unwrap_or(0);
    if arity == 0 {
        return Err(GraphFailure::Unavailable(
            "the comparison graph has no ranked input".into(),
        ));
    }
    let common = coordinates(arity, p.integer, p.function);
    let count = common.len();
    let mut cases = Vec::new();
    for (body_position, body) in bodies.iter().enumerate() {
        for source in &body.cases {
            let SourceOutcome::Value(output) = &source.outcome else {
                return Err(GraphFailure::Unavailable(
                    "the comparison has a source path without a represented normal value".into(),
                ));
            };
            let payload = (0..source.values.len())
                .flat_map(|position| {
                    [
                        (Key::Graph(count + position * 2), p.integer),
                        (Key::Graph(count + position * 2 + 1), p.function),
                    ]
                })
                .collect::<Vec<_>>();
            let fields = common.iter().chain(&payload).copied().collect::<Vec<_>>();
            let values = (0..fields.len())
                .map(|position| {
                    p.arena
                        .insert(Term::Variable((fields.len() - position - 1) as u32))
                })
                .collect::<Vec<_>>();
            let (coordinates, locals) = values.split_at(count);
            let locals = locals
                .chunks_exact(2)
                .map(|pair| (pair[0], pair[1]))
                .collect::<Vec<_>>();
            let mut actuals = vec![p.literal(body_position)?];
            let (rank, shift) = p.allowance(allowances[body_position]);
            actuals.extend([rank, shift]);
            for position in 0..arity {
                let (rank, shift) = match source.inputs.get(position) {
                    Some(value) => p.source_value(value, &locals)?,
                    None => (p.zero, p.identity_shift()),
                };
                actuals.extend([rank, shift]);
            }
            let (rank, shift) = p.source_value(output, &locals)?;
            actuals.extend([rank, shift]);
            let mut predicates = Vec::new();
            for ((&(_, ty), &coordinate), &value) in common.iter().zip(coordinates).zip(&actuals) {
                predicates.push(p.typed_equation(ty, coordinate, value));
            }
            for (left, right) in &source.equations {
                let (left_rank, left_shift) = p.source_value(left, &locals)?;
                let (right_rank, right_shift) = p.source_value(right, &locals)?;
                predicates.push(p.equation(left_rank, right_rank));
                predicates.push(p.typed_equation(p.function, left_shift, right_shift));
            }
            for &(rank, shift) in &locals {
                predicates.push(p.shift_laws(rank, shift));
            }
            let predicate = p.conjunction(&predicates);
            let domains = fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
            let constraints = p.close(predicate, &domains);
            let mut children = Vec::new();
            for call in &source.calls {
                let selected = bodies.iter().position(|body| body.entry == call.target).ok_or_else(|| GraphFailure::Unavailable("a comparison call has no complete source body in its structural closure".into()))?;
                let mut index = vec![p.literal(selected)?];
                let (rank, shift) = p.allowance(allowances[selected]);
                index.extend([rank, shift]);
                for position in 0..arity {
                    let (rank, shift) = match call.arguments.get(position) {
                        Some(value) => p.source_value(value, &locals)?,
                        None => (p.zero, p.identity_shift()),
                    };
                    index.extend([rank, shift]);
                }
                let result = call.result.ok_or_else(|| {
                    GraphFailure::Unavailable(
                        "a comparison call has no ordinary structural result".into(),
                    )
                })?;
                let &(rank, shift) = locals.get(result).ok_or_else(|| {
                    GraphFailure::Unavailable(
                        "the comparison call result lost its exact source binder".into(),
                    )
                })?;
                index.extend([rank, shift]);
                let index = tuple(p.arena, &index)?;
                children.push(p.close(index, &domains));
            }
            cases.push(Case {
                payload,
                constraints,
                child_indices: children,
            });
        }
    }
    p.checked_family(&common, &cases, |p, values| {
        let mut bound = values[count - 2];
        for position in 1..arity {
            bound = apply(p.arena, values[4 + position * 2], bound);
        }
        bound = apply(p.arena, values[2], bound);
        p.le(values[3], bound)
    })
}

/// A finite source value supplies both its rank and the checked fold that
/// adds its successors to another rank. Keeping that fold makes the actual
/// pred/recursive-call composition exact without an assumed reassociation law.
pub(super) fn rank_graph(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    theory: &mut SourceIntegerTheory,
) -> Result<CheckedGraph, GraphFailure> {
    let mut p = producer(arena, declarations, theory)?;
    let common = [(Key::Graph(0), p.integer), (Key::Graph(1), p.function)];
    let rank = p.arena.insert(Term::Variable(1));
    let shift = p.arena.insert(Term::Variable(0));
    let identity = p.identity_shift();
    let rank_eq = p.equation(rank, p.zero);
    let shift_eq = p.typed_equation(p.function, shift, identity);
    let predicate = p.conjunction(&[rank_eq, shift_eq]);
    let base = p.close(predicate, &[p.integer, p.function]);
    let rank = p.arena.insert(Term::Variable(3));
    let shift = p.arena.insert(Term::Variable(2));
    let child_rank = p.arena.insert(Term::Variable(1));
    let child_shift = p.arena.insert(Term::Variable(0));
    let successor_rank = p.add(child_rank, p.one);
    let successor_shift = p.successor_shift(child_shift);
    let rank_eq = p.equation(rank, successor_rank);
    let shift_eq = p.typed_equation(p.function, shift, successor_shift);
    let predicate = p.conjunction(&[rank_eq, shift_eq]);
    let domains = [p.integer, p.function, p.integer, p.function];
    let step = p.close(predicate, &domains);
    let child_index = tuple(p.arena, &[child_rank, child_shift])?;
    let child_index = p.close(child_index, &domains);
    let cases = [
        Case {
            payload: Vec::new(),
            constraints: base,
            child_indices: Vec::new(),
        },
        Case {
            payload: vec![(Key::Graph(2), p.integer), (Key::Graph(3), p.function)],
            constraints: step,
            child_indices: vec![child_index],
        },
    ];
    p.checked_family(&common, &cases, |p, values| {
        p.shift_laws(values[0], values[1])
    })
}

/// Compose the source return constructors with the exact finite-rank witnesses
/// of their values. Rank laws are proved by the rank theorem, not asserted by
/// a caller as part of the comparison's contract.
pub(super) fn source_return_schemas(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    theory: &mut SourceIntegerTheory,
    ranks: &CheckedGraph,
    graph: &CheckedGraph,
    bodies: &[SourceOrderBody],
    allowances: &[usize],
) -> Result<Vec<(TermHandle, TermHandle)>, GraphFailure> {
    let mut p = producer(arena, declarations, theory)?;
    let arity = bodies
        .iter()
        .map(|body| body.parameter_types.len())
        .max()
        .unwrap_or(0);
    let fields = coordinates(arity, p.integer, p.function);
    let mut ordinal = 0;
    let mut schemas = Vec::new();
    for (body_position, body) in bodies.iter().enumerate() {
        for source in &body.cases {
            let SourceOutcome::Value(output) = &source.outcome else {
                return Err(GraphFailure::Unavailable(
                    "the source return has no complete value association".into(),
                ));
            };
            let mut parameters = Vec::new();
            let mut locals = Vec::new();
            for _ in &source.values {
                let key = Key::Graph(parameters.len());
                let rank = p.placeholder(p.integer, key, &mut parameters);
                let key = Key::Graph(parameters.len());
                let shift = p.placeholder(p.function, key, &mut parameters);
                locals.push((rank, shift));
            }
            let mut common = vec![p.literal(body_position)?];
            let (rank, shift) = p.allowance(allowances[body_position]);
            common.extend([rank, shift]);
            for position in 0..arity {
                let (rank, shift) = match source.inputs.get(position) {
                    Some(value) => p.source_value(value, &locals)?,
                    None => (p.zero, p.identity_shift()),
                };
                common.extend([rank, shift]);
            }
            let (rank, shift) = p.source_value(output, &locals)?;
            common.extend([rank, shift]);
            let mut source_equations = Vec::new();
            for (left, right) in &source.equations {
                let (left_rank, left_shift) = p.source_value(left, &locals)?;
                let (right_rank, right_shift) = p.source_value(right, &locals)?;
                for (ty, left, right) in [
                    (p.integer, left_rank, right_rank),
                    (p.function, left_shift, right_shift),
                ] {
                    let ty = p.typed_equation(ty, left, right);
                    let key = Key::Graph(parameters.len());
                    source_equations.push(p.placeholder(ty, key, &mut parameters));
                }
            }
            let mut rank_laws = Vec::new();
            for &(rank, shift) in &locals {
                let index = tuple(p.arena, &[rank, shift])?;
                let ty = ranks.family.indexed_w(p.arena, index);
                let key = Key::Graph(parameters.len());
                let witness = p.placeholder(ty, key, &mut parameters);
                let theorem = apply(p.arena, ranks.theorem, index);
                rank_laws.push(apply(p.arena, theorem, witness));
            }
            let mut proofs = fields
                .iter()
                .zip(&common)
                .map(|(&(_, ty), &value)| p.arena.insert(Term::Refl { ty, value }))
                .collect::<Vec<_>>();
            proofs.extend(source_equations);
            proofs.extend(rank_laws);
            let mut predicate = p.arena.insert(Term::Refl {
                ty: p.integer,
                value: p.zero,
            });
            for first in proofs.into_iter().rev() {
                predicate = p.arena.insert(Term::Pair {
                    first,
                    second: predicate,
                });
            }
            let (constructor, _) = *graph.constructors.get(ordinal).ok_or_else(|| {
                GraphFailure::Kernel(
                    "the complete graph has no constructor for a source return".into(),
                )
            })?;
            let mut result = constructor;
            for &value in &common {
                result = apply(p.arena, result, value);
            }
            for &(rank, shift) in &locals {
                result = apply(p.arena, result, rank);
                result = apply(p.arena, result, shift);
            }
            result = apply(p.arena, result, predicate);
            for call in &source.calls {
                let selected = bodies
                    .iter()
                    .position(|body| body.entry == call.target)
                    .ok_or_else(|| {
                        GraphFailure::Unavailable("the source call has no exact graph child".into())
                    })?;
                let mut index = vec![p.literal(selected)?];
                let (rank, shift) = p.allowance(allowances[selected]);
                index.extend([rank, shift]);
                for position in 0..arity {
                    let (rank, shift) = match call.arguments.get(position) {
                        Some(value) => p.source_value(value, &locals)?,
                        None => (p.zero, p.identity_shift()),
                    };
                    index.extend([rank, shift]);
                }
                let position = call.result.ok_or_else(|| {
                    GraphFailure::Unavailable("the source call has no exact normal result".into())
                })?;
                let &(rank, shift) = locals.get(position).ok_or_else(|| {
                    GraphFailure::Kernel(
                        "the source call's result binder is outside its case".into(),
                    )
                })?;
                index.extend([rank, shift]);
                let index = tuple(p.arena, &index)?;
                let ty = graph.family.indexed_w(p.arena, index);
                let key = Key::Graph(parameters.len());
                let witness = p.placeholder(ty, key, &mut parameters);
                result = apply(p.arena, result, witness);
            }
            let index = tuple(p.arena, &common)?;
            let ty = graph.family.indexed_w(p.arena, index);
            schemas.push(p.abstracted(result, ty, &parameters, "source evaluation association")?);
            ordinal += 1;
        }
    }
    if ordinal != graph.constructors.len() {
        return Err(GraphFailure::Kernel(
            "the graph constructor roster differs from the source return roster".into(),
        ));
    }
    Ok(schemas)
}

/// The exact zero comparison gives strict decrease of the successor's child.
/// This schema consumes actual finite-rank and evaluation witnesses; a bare
/// equality from an arbitrary comparison cannot supply those witnesses.
pub(super) fn zero_comparison_schema(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    theory: &mut SourceIntegerTheory,
    ranks: &CheckedGraph,
    graph: &CheckedGraph,
    arity: usize,
) -> Result<(TermHandle, TermHandle, Vec<u32>), GraphFailure> {
    if arity < 2 {
        return Err(GraphFailure::Kernel(
            "the zero comparison has no complete pair of ranked operands".into(),
        ));
    }
    let mut p = producer(arena, declarations, theory)?;
    let mut parameters = Vec::new();
    let next_rank = p.placeholder(p.integer, Key::Graph(0), &mut parameters);
    let next_shift = p.placeholder(p.function, Key::Graph(1), &mut parameters);
    let current_rank = p.placeholder(p.integer, Key::Graph(2), &mut parameters);
    let current_shift = p.placeholder(p.function, Key::Graph(3), &mut parameters);
    let next_index = tuple(p.arena, &[next_rank, next_shift])?;
    let current_index = tuple(p.arena, &[current_rank, current_shift])?;
    let mut index = vec![p.zero, p.zero, p.identity_shift()];
    let successor_rank = p.add(next_rank, p.one);
    let successor_shift = p.successor_shift(next_shift);
    index.extend([successor_rank, successor_shift, current_rank, current_shift]);
    for _ in 2..arity {
        index.extend([p.zero, p.identity_shift()]);
    }
    index.extend([p.zero, p.identity_shift()]);
    let index = tuple(p.arena, &index)?;
    let assumptions = [
        p.shift_laws(next_rank, next_shift),
        p.shift_laws(current_rank, current_shift),
        apply(p.arena, graph.invariant, index),
    ];
    let goal = p.binary(p.theory.vocabulary().less_than, next_rank, current_rank);
    let proof = p.derive(goal, &assumptions, 0)?.ok_or_else(|| {
        GraphFailure::Kernel(
            "the checked zero comparison does not derive strict structural descent".into(),
        )
    })?;
    let next_type = ranks.family.indexed_w(p.arena, next_index);
    let current_type = ranks.family.indexed_w(p.arena, current_index);
    let evaluation_type = graph.family.indexed_w(p.arena, index);
    let domains = [next_type, current_type, evaluation_type];
    let next_witness = p.arena.insert(Term::Variable(2));
    let current_witness = p.arena.insert(Term::Variable(1));
    let evaluation = p.arena.insert(Term::Variable(0));
    let next_theorem = apply(p.arena, ranks.theorem, next_index);
    let current_theorem = apply(p.arena, ranks.theorem, current_index);
    let evaluation_theorem = apply(p.arena, graph.theorem, index);
    let next_laws = apply(p.arena, next_theorem, next_witness);
    let current_laws = apply(p.arena, current_theorem, current_witness);
    let property = apply(p.arena, evaluation_theorem, evaluation);
    let mut body = proof;
    for evidence in [next_laws, current_laws, property] {
        body = apply(p.arena, body, evidence);
    }
    for &domain in domains.iter().rev() {
        body = p.arena.insert(Term::Lambda { domain, body });
    }
    let ty = p.premise_type(goal, &domains);
    let (schema, ty) = p.abstracted(body, ty, &parameters, "zero comparison to strict descent")?;
    let signature = Signature::from_declarations(p.declarations.clone());
    let closure = judgment_assumption_closure(p.arena, &signature, &[schema, ty]);
    Ok((schema, ty, closure.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::SymbolHandle;
    use validation::computed_order::{
        SourceApplication, SourceCall, SourceCase, SourceReturn, SourceValueOrigin,
    };

    fn checked_signature(arena: &mut TermArena, declarations: &[Declaration]) -> Signature {
        proof_admission::check_signature(
            arena,
            declarations,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap()
    }
    fn error(failure: GraphFailure) -> String {
        match failure {
            GraphFailure::Unavailable(reason) | GraphFailure::Kernel(reason) => reason,
        }
    }
    fn variable(position: usize) -> SourceOrderValue {
        SourceOrderValue::Variable(position)
    }
    fn successor(value: SourceOrderValue) -> SourceOrderValue {
        SourceOrderValue::Successor(Box::new(value))
    }
    fn source_case(
        parameters: usize,
        values: usize,
        equations: Vec<(SourceOrderValue, SourceOrderValue)>,
        calls: Vec<SourceCall>,
        output: SourceOrderValue,
    ) -> SourceCase {
        SourceCase {
            source_return: SourceReturn {
                state: Default::default(),
                statement: 0,
                target: None,
            },
            values: (0..values)
                .map(|_| SourceValueOrigin::Parameter {
                    symbol: Default::default(),
                    reference: Default::default(),
                })
                .collect(),
            inputs: (0..parameters).map(variable).collect(),
            equations,
            guards: Vec::new(),
            calls,
            outcome: SourceOutcome::Value(output),
        }
    }
    fn body(entry: SymbolHandle, parameters: usize, cases: Vec<SourceCase>) -> SourceOrderBody {
        SourceOrderBody {
            machine: entry,
            entry,
            states: Default::default(),
            parameter_types: vec![Default::default(); parameters],
            result_type: Default::default(),
            cases,
        }
    }
    fn call(target: SymbolHandle, arguments: Vec<SourceOrderValue>, result: usize) -> SourceCall {
        SourceCall {
            owner: Default::default(),
            state: Default::default(),
            statement: 0,
            target,
            application: SourceApplication::Statement {
                statement: Default::default(),
            },
            arguments,
            result_type: Default::default(),
            result: Some(result),
        }
    }

    #[test]
    fn finite_rank_fold_requires_a_witness_and_only_existing_arithmetic_laws() {
        let mut arena = TermArena::new();
        let mut declarations = proof_admission::indexed_scheme(&mut arena);
        let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
        let rank = rank_graph(&mut arena, &mut declarations, &mut theory)
            .map_err(error)
            .unwrap();
        let accepted = theory.source_safety_roster();
        assert!(
            rank.assumption_closure
                .iter()
                .all(|position| accepted.contains(position))
        );
        let negative = theory
            .literal(&mut arena, &mut declarations, BigInt::from_i128(-1))
            .unwrap();
        let integer = constant(&mut arena, theory.vocabulary().carrier);
        let value = arena.insert(Term::Variable(0));
        let identity = arena.insert(Term::Lambda {
            domain: integer,
            body: value,
        });
        let index = tuple(&mut arena, &[negative, identity])
            .map_err(error)
            .unwrap();
        let signature = checked_signature(&mut arena, &declarations);
        proof_admission::check_type(
            &mut arena,
            &Context::empty().with_signature(signature.clone()),
            negative,
            integer,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        let theorem = apply(&mut arena, rank.theorem, index);
        // A typed integer is not evidence that this impossible negative-rank
        // value was built by the finite base/successor constructors.
        let forged = apply(&mut arena, theorem, negative);
        let goal = apply(&mut arena, rank.invariant, index);
        assert!(
            proof_admission::check_type(
                &mut arena,
                &Context::empty().with_signature(signature),
                forged,
                goal,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .is_err()
        );
    }

    #[test]
    fn computed_order_checks_pred_composition_and_refuses_constant_zero() {
        // These are equation graphs for the mathematical producer, not source
        // admission records. The actual source-check control separately tests
        // the typed source-to-graph association and recursive component gate.
        let relation = SymbolHandle::from_arena_index(1);
        let pred = SymbolHandle::from_arena_index(2);
        let comparison = body(
            relation,
            2,
            vec![
                source_case(
                    2,
                    2,
                    vec![(variable(1), SourceOrderValue::Base)],
                    vec![],
                    variable(0),
                ),
                source_case(
                    2,
                    5,
                    vec![(variable(1), successor(variable(2)))],
                    vec![
                        call(relation, vec![variable(0), variable(2)], 3),
                        call(pred, vec![variable(3)], 4),
                    ],
                    variable(4),
                ),
            ],
        );
        let predecessor = body(
            pred,
            1,
            vec![
                source_case(
                    1,
                    1,
                    vec![(variable(0), SourceOrderValue::Base)],
                    vec![],
                    SourceOrderValue::Base,
                ),
                source_case(
                    1,
                    2,
                    vec![(variable(0), successor(variable(1)))],
                    vec![],
                    variable(1),
                ),
            ],
        );
        let mut arena = TermArena::new();
        let mut declarations = proof_admission::indexed_scheme(&mut arena);
        let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
        let ranks = rank_graph(&mut arena, &mut declarations, &mut theory)
            .map_err(error)
            .unwrap();
        let bodies = [comparison, predecessor];
        let graph = comparison_graph(&mut arena, &mut declarations, &mut theory, &bodies, &[0, 1])
            .map_err(error)
            .unwrap();
        let associations = source_return_schemas(
            &mut arena,
            &mut declarations,
            &mut theory,
            &ranks,
            &graph,
            &bodies,
            &[0, 1],
        )
        .map_err(error)
        .unwrap();
        assert_eq!(associations.len(), 4);
        let (_, _, closure) = zero_comparison_schema(
            &mut arena,
            &mut declarations,
            &mut theory,
            &ranks,
            &graph,
            2,
        )
        .map_err(error)
        .unwrap();
        let accepted = theory.source_safety_roster();
        assert!(closure.iter().all(|position| accepted.contains(position)));
        checked_signature(&mut arena, &declarations);
        let fake = body(
            relation,
            2,
            vec![source_case(2, 2, vec![], vec![], SourceOrderValue::Base)],
        );
        match comparison_graph(&mut arena, &mut declarations, &mut theory, &[fake], &[0]) {
            Err(GraphFailure::Unavailable(_)) => {}
            Err(GraphFailure::Kernel(reason)) => {
                panic!("the negative graph caused a malformed proof: {reason}")
            }
            Ok(_) => panic!("an honest constant-zero function cannot justify unchanged recursion"),
        }
        checked_signature(&mut arena, &declarations);
    }
}
