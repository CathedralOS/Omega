//! Untrusted source-family description production. All generated applications
//! are ordinary terms checked against indexed_scheme; no admission follows
//! from this builder or from a source constructor's Boolean verdict.
//!
//! Each source common/payload binder keeps its exact symbol key. Constraint
//! and child-index functions are closed over that ordered source telescope,
//! so moving them under tag/payload eliminators cannot capture a source name.
//! Finite child positions use the profile's boxed strict empty/unit types.

use proof_admission::{IndexedFamily, Level, Term, TermArena, TermHandle};
use symbols::SymbolHandle;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    Source(SymbolHandle),
    Internal(usize),
}

#[derive(Clone)]
enum S {
    Closed(TermHandle),
    SourceDomain(TermHandle, Vec<Key>),
    Var(Key),
    Type,
    Two,
    Zero,
    One,
    Empty,
    Pi(Key, Box<S>, Box<S>),
    Lam(Key, Box<S>, Box<S>),
    Sigma(Key, Box<S>, Box<S>),
    App(Box<S>, Box<S>),
    Pair(Box<S>, Box<S>),
    Fst(Box<S>),
    Snd(Box<S>),
    Case(Box<S>, Box<S>, Box<S>, Box<S>),
    Boxed(Box<S>),
    BoxIntro(Box<S>, Box<S>),
    BoxElim(Box<S>, Box<S>, Box<S>),
    EmptyElim(Box<S>, Box<S>),
    Identity(Box<S>, Box<S>, Box<S>),
    Reflexivity(Box<S>, Box<S>),
    IdentityElimination(Box<S>, Box<S>, Box<S>, Box<S>),
}
fn app(f: S, x: S) -> S {
    S::App(Box::new(f), Box::new(x))
}
fn pair(a: S, b: S) -> S {
    S::Pair(Box::new(a), Box::new(b))
}
fn fst(x: S) -> S {
    S::Fst(Box::new(x))
}
fn snd(x: S) -> S {
    S::Snd(Box::new(x))
}
fn lam(k: Key, a: S, b: S) -> S {
    S::Lam(k, Box::new(a), Box::new(b))
}
fn pi(k: Key, a: S, b: S) -> S {
    S::Pi(k, Box::new(a), Box::new(b))
}
fn sigma(k: Key, a: S, b: S) -> S {
    S::Sigma(k, Box::new(a), Box::new(b))
}
fn choose(m: S, z: S, o: S, t: S) -> S {
    S::Case(Box::new(m), Box::new(z), Box::new(o), Box::new(t))
}

impl S {
    fn term(
        &self,
        arena: &mut TermArena,
        scope: &mut Vec<Key>,
    ) -> Result<TermHandle, &'static str> {
        let node = match self {
            Self::Closed(t) => return Ok(*t),
            Self::SourceDomain(function, parameters) => {
                // These lambdas were just built by source_field_function,
                // not read from authored functions or predicate evidence.
                // Reduce only that administrative telescope, preserving its
                // body and exact source-keyed Type actual under inner binders.
                let mut result = *function;
                for parameter in parameters {
                    let position = scope
                        .iter()
                        .rposition(|key| key == parameter)
                        .ok_or("source payload domain lost its Type binder")?;
                    let actual = arena.insert(Term::Variable((scope.len() - 1 - position) as u32));
                    let Term::Lambda { body, .. } = arena.get(result) else {
                        return Err("source payload domain lost its generated Type telescope");
                    };
                    result = proof_admission::substitute(arena, body, actual);
                }
                return Ok(result);
            }
            Self::Var(k) => Term::Variable(
                scope
                    .iter()
                    .rposition(|v| v == k)
                    .map(|p| (scope.len() - 1 - p) as u32)
                    .ok_or("generated source-family binder escaped its telescope")?,
            ),
            Self::Type => Term::Sort(proof_admission::Sort::Type(Level::Constant(0))),
            Self::Two => Term::Two,
            Self::Zero => Term::TwoZero,
            Self::One => Term::TwoOne,
            Self::Empty => Term::Empty,
            Self::Pi(k, a, b) | Self::Lam(k, a, b) | Self::Sigma(k, a, b) => {
                let domain = a.term(arena, scope)?;
                scope.push(*k);
                let body = b.term(arena, scope);
                scope.pop();
                let body = body?;
                match self {
                    Self::Pi(..) => Term::Pi {
                        domain,
                        codomain: body,
                    },
                    Self::Lam(..) => Term::Lambda { domain, body },
                    _ => Term::Sigma {
                        domain,
                        codomain: body,
                    },
                }
            }
            Self::App(f, x) => Term::Apply {
                function: f.term(arena, scope)?,
                argument: x.term(arena, scope)?,
            },
            Self::Pair(a, b) => Term::Pair {
                first: a.term(arena, scope)?,
                second: b.term(arena, scope)?,
            },
            Self::Fst(a) => Term::Fst {
                pair: a.term(arena, scope)?,
            },
            Self::Snd(a) => Term::Snd {
                pair: a.term(arena, scope)?,
            },
            Self::Case(m, z, o, t) => Term::CaseTwo {
                motive: m.term(arena, scope)?,
                zero_branch: z.term(arena, scope)?,
                one_branch: o.term(arena, scope)?,
                scrutinee: t.term(arena, scope)?,
            },
            Self::Boxed(a) => Term::Box {
                ty: a.term(arena, scope)?,
            },
            Self::BoxIntro(a, x) => Term::BoxIntro {
                ty: a.term(arena, scope)?,
                value: x.term(arena, scope)?,
            },
            Self::BoxElim(m, f, x) => Term::BoxElim {
                motive: m.term(arena, scope)?,
                body: f.term(arena, scope)?,
                scrutinee: x.term(arena, scope)?,
            },
            Self::EmptyElim(a, x) => Term::EmptyElim {
                ty: a.term(arena, scope)?,
                scrutinee: x.term(arena, scope)?,
            },
            Self::Identity(ty, left, right) => Term::Id {
                ty: ty.term(arena, scope)?,
                left: left.term(arena, scope)?,
                right: right.term(arena, scope)?,
            },
            Self::Reflexivity(ty, value) => Term::Refl {
                ty: ty.term(arena, scope)?,
                value: value.term(arena, scope)?,
            },
            Self::IdentityElimination(motive, base, endpoint, proof) => Term::IdElim {
                motive: motive.term(arena, scope)?,
                base: base.term(arena, scope)?,
                endpoint: endpoint.term(arena, scope)?,
                proof: proof.term(arena, scope)?,
            },
        };
        Ok(arena.insert(node))
    }
}

pub(super) struct Case {
    pub payload: Vec<(SymbolHandle, TermHandle)>,
    /// Closed function of common fields then nonrecursive payload fields,
    /// returning the complete type-wide and active-case constraint type.
    pub constraints: TermHandle,
    /// One closed index function per exact recursive source field, in order.
    pub child_indices: Vec<TermHandle>,
}

#[derive(Clone, Copy)]
enum ResultKind {
    Positions,
    Next,
}

/// One binder supply and finite-position algebra for descriptions and supplied
/// children. Selection reconstructs the exact tag/payload pair in its motive;
/// the result may depend on that position, not merely share one carrier.
#[derive(Default)]
struct SourceTerms {
    next: usize,
}
impl SourceTerms {
    fn fresh(&mut self) -> Key {
        let k = Key::Internal(self.next);
        self.next += 1;
        k
    }
    fn empty(&mut self) -> S {
        S::Boxed(Box::new(S::Empty))
    }
    fn unit_strict(&mut self) -> S {
        let k = self.fresh();
        pi(k, S::Empty, S::Empty)
    }
    fn unit(&mut self) -> S {
        let u = self.unit_strict();
        S::Boxed(Box::new(u))
    }
    fn type_motive(&mut self) -> S {
        let k = self.fresh();
        lam(k, S::Two, S::Type)
    }
    fn positions(&mut self, count: usize) -> S {
        match count {
            0 => self.empty(),
            1 => self.unit(),
            _ => {
                let tag = self.fresh();
                let motive = self.type_motive();
                let first = self.unit();
                let rest = self.positions(count - 1);
                sigma(tag, S::Two, choose(motive, first, rest, S::Var(tag)))
            }
        }
    }
    fn select(&mut self, values: &[S], position: S, result_family: S) -> S {
        let Some((first, rest_values)) = values.split_first() else {
            let boxed = self.fresh();
            let impossible = self.fresh();
            let motive = lam(
                boxed,
                self.empty(),
                app(result_family.clone(), S::Var(boxed)),
            );
            let unboxed_position = S::BoxIntro(Box::new(S::Empty), Box::new(S::Var(impossible)));
            let result = app(result_family, unboxed_position);
            let body = lam(
                impossible,
                S::Empty,
                S::EmptyElim(Box::new(result), Box::new(S::Var(impossible))),
            );
            return S::BoxElim(Box::new(motive), Box::new(body), Box::new(position));
        };
        if rest_values.is_empty() {
            return first.clone();
        }
        let tag = self.fresh();
        let payload = self.fresh();
        let type_motive = self.type_motive();
        let unit = self.unit();
        let rest = self.positions(rest_values.len());
        let at = choose(type_motive, unit.clone(), rest.clone(), S::Var(tag));
        let selected_type = app(result_family.clone(), pair(S::Var(tag), S::Var(payload)));
        let motive = lam(tag, S::Two, pi(payload, at, selected_type));
        let singleton = self.fresh();
        let zero = lam(singleton, unit, first.clone());
        let tail = self.fresh();
        let nested_position = self.fresh();
        let nested_family = lam(
            nested_position,
            rest.clone(),
            app(result_family, pair(S::One, S::Var(nested_position))),
        );
        let nested = self.select(rest_values, S::Var(tail), nested_family);
        let one = lam(tail, rest, nested);
        app(
            choose(motive, zero, one, fst(position.clone())),
            snd(position),
        )
    }
}

struct Builder<'a> {
    terms: SourceTerms,
    common: &'a [(SymbolHandle, TermHandle)],
    index_fields: &'a [SymbolHandle],
    cases: &'a [Case],
    type_parameters: &'a [SymbolHandle],
}
impl Builder<'_> {
    fn domain(&self, symbol: SymbolHandle, ty: TermHandle) -> S {
        if self.type_parameters.is_empty() || self.type_parameters.contains(&symbol) {
            S::Closed(ty)
        } else {
            S::SourceDomain(
                ty,
                self.type_parameters
                    .iter()
                    .copied()
                    .map(Key::Source)
                    .collect(),
            )
        }
    }
    fn index_type(&mut self) -> Result<S, &'static str> {
        let mut fields = Vec::new();
        for symbol in self.index_fields {
            let (_, ty) = self
                .common
                .iter()
                .find(|(field, _)| field == symbol)
                .ok_or("source index lost its exact descriptor field")?;
            fields.push((*symbol, *ty));
        }
        let Some((last_symbol, last)) = fields.pop() else {
            return Ok(self.terms.unit());
        };
        let mut body = self.domain(last_symbol, last);
        for (symbol, ty) in fields.into_iter().rev() {
            body = sigma(Key::Source(symbol), self.domain(symbol, ty), body);
        }
        Ok(body)
    }
    fn constraint_application(&self, case: usize) -> S {
        let mut term = S::Closed(self.cases[case].constraints);
        for (symbol, _) in self.common.iter().chain(self.cases[case].payload.iter()) {
            term = app(term, S::Var(Key::Source(*symbol)));
        }
        term
    }
    fn payload_type(&self, case: usize) -> S {
        let mut body = self.constraint_application(case);
        for (symbol, ty) in self.cases[case].payload.iter().rev() {
            body = sigma(Key::Source(*symbol), self.domain(*symbol, *ty), body);
        }
        body
    }
    fn sum_type(&mut self, start: usize) -> S {
        if start == self.cases.len() {
            return self.terms.empty();
        }
        if start + 1 == self.cases.len() {
            return self.payload_type(start);
        }
        let tag = self.terms.fresh();
        let motive = self.terms.type_motive();
        let zero = self.payload_type(start);
        let one = self.sum_type(start + 1);
        sigma(tag, S::Two, choose(motive, zero, one, S::Var(tag)))
    }
    fn carrier(&mut self) -> S {
        let mut body = self.sum_type(0);
        for (symbol, ty) in self.common.iter().rev() {
            body = sigma(Key::Source(*symbol), self.domain(*symbol, *ty), body);
        }
        body
    }
    fn projected(mut value: S, ordinal: usize) -> S {
        for _ in 0..ordinal {
            value = snd(value);
        }
        fst(value)
    }
    fn child_index(&self, case: usize, child: usize, payload: S) -> S {
        let mut term = S::Closed(self.cases[case].child_indices[child]);
        for (symbol, _) in self.common {
            term = app(term, S::Var(Key::Source(*symbol)));
        }
        for ordinal in 0..self.cases[case].payload.len() {
            term = app(term, Self::projected(payload.clone(), ordinal));
        }
        term
    }
    fn case_body(&mut self, case: usize, payload: S, kind: ResultKind) -> Result<S, &'static str> {
        let count = self.cases[case].child_indices.len();
        let positions = self.terms.positions(count);
        if matches!(kind, ResultKind::Positions) {
            return Ok(positions);
        }
        let index = self.index_type()?;
        let p = self.terms.fresh();
        let values = (0..count)
            .map(|child| self.child_index(case, child, payload.clone()))
            .collect::<Vec<_>>();
        let at = self.terms.fresh();
        let result_family = lam(at, positions.clone(), index);
        let body = self.terms.select(&values, S::Var(p), result_family);
        Ok(lam(p, positions, body))
    }
    fn result_type(&mut self, start: usize, value: S, kind: ResultKind) -> Result<S, &'static str> {
        if matches!(kind, ResultKind::Positions) {
            return Ok(S::Type);
        }
        let positions = self.eliminate(start, value, ResultKind::Positions)?;
        let p = self.terms.fresh();
        Ok(pi(p, positions, self.index_type()?))
    }
    fn eliminate(&mut self, start: usize, value: S, kind: ResultKind) -> Result<S, &'static str> {
        if start == self.cases.len() {
            return Err("empty source constructor roster has no eliminator branch");
        }
        if start + 1 == self.cases.len() {
            return self.case_body(start, value, kind);
        }
        let tag = self.terms.fresh();
        let payload = self.terms.fresh();
        let tm = self.terms.type_motive();
        let zero_type = self.payload_type(start);
        let one_type = self.sum_type(start + 1);
        let payload_type = choose(tm, zero_type.clone(), one_type.clone(), S::Var(tag));
        let result = self.result_type(start, pair(S::Var(tag), S::Var(payload)), kind)?;
        let motive = lam(tag, S::Two, pi(payload, payload_type, result));
        let z = self.terms.fresh();
        let zero_body = self.case_body(start, S::Var(z), kind)?;
        let zero = lam(z, zero_type, zero_body);
        let o = self.terms.fresh();
        let one_body = self.eliminate(start + 1, S::Var(o), kind)?;
        let one = lam(o, one_type, one_body);
        Ok(app(
            choose(motive, zero, one, fst(value.clone())),
            snd(value),
        ))
    }
    fn unpack(&mut self, label: S, kind: ResultKind) -> Result<S, &'static str> {
        let tail = self.terms.fresh();
        let sum_type = self.sum_type(0);
        let body = self.eliminate(0, S::Var(tail), kind)?;
        let mut function = lam(tail, sum_type, body);
        for (symbol, ty) in self.common.iter().rev() {
            function = lam(Key::Source(*symbol), self.domain(*symbol, *ty), function);
        }
        let mut tail_value = label.clone();
        for _ in self.common {
            function = app(function, fst(tail_value.clone()));
            tail_value = snd(tail_value);
        }
        Ok(app(function, tail_value))
    }
    fn out(&mut self, label: S) -> Result<S, &'static str> {
        let mut indices = Vec::new();
        for field in self.index_fields {
            let ordinal = self
                .common
                .iter()
                .position(|(symbol, _)| symbol == field)
                .ok_or("source index descriptor is outside its owner")?;
            indices.push(Self::projected(label.clone(), ordinal));
        }
        let Some(mut body) = indices.pop() else {
            return Err("an indexed family requires an index telescope");
        };
        for index in indices.into_iter().rev() {
            body = pair(index, body);
        }
        Ok(body)
    }
}

/// Supply the declaration-ordered children at their exact finite positions.
/// Inputs are closed under term variables in the current signature; temporary
/// source constants remain the constructor schema owner's abstraction duty.
/// This only produces an ordinary term. Its caller checks the full dependent
/// function against the description's B/next telescope before constructing sup.
pub(super) fn constructor_children(
    arena: &mut TermArena,
    description: &IndexedFamily,
    label: TermHandle,
    children: &[TermHandle],
) -> Result<TermHandle, &'static str> {
    let child_domain = arena.insert(Term::Apply {
        function: description.children,
        argument: label,
    });
    let position = arena.insert(Term::Variable(0));
    let next = arena.insert(Term::Apply {
        function: description.next,
        argument: label,
    });
    let child_index = arena.insert(Term::Apply {
        function: next,
        argument: position,
    });
    let child_type = description.indexed_w(arena, child_index);
    let result_family = arena.insert(Term::Lambda {
        domain: child_domain,
        body: child_type,
    });
    let mut terms = SourceTerms::default();
    let position = terms.fresh();
    let values = children.iter().copied().map(S::Closed).collect::<Vec<_>>();
    let body = terms.select(&values, S::Var(position), S::Closed(result_family));
    lam(position, S::Closed(child_domain), body).term(arena, &mut Vec::new())
}

/// Closed producer terms. The caller must check the resulting ordinary scheme
/// application and retain source correspondence before admitting any use.
pub(super) fn description(
    arena: &mut TermArena,
    common: &[(SymbolHandle, TermHandle)],
    index_fields: &[SymbolHandle],
    cases: &[Case],
    type_parameters: &[SymbolHandle],
) -> Result<IndexedFamily, &'static str> {
    let mut b = Builder {
        terms: SourceTerms::default(),
        common,
        index_fields,
        cases,
        type_parameters,
    };
    let carrier = b.carrier();
    let index = b.index_type()?;
    let a = b.terms.fresh();
    let children_body = b.unpack(S::Var(a), ResultKind::Positions)?;
    let children = lam(a, carrier.clone(), children_body);
    let a = b.terms.fresh();
    let out_body = b.out(S::Var(a))?;
    let out = lam(a, carrier.clone(), out_body);
    let a = b.terms.fresh();
    let next_body = b.unpack(S::Var(a), ResultKind::Next)?;
    let next = lam(a, carrier.clone(), next_body);
    Ok(IndexedFamily {
        levels: [
            Level::Constant(u32::from(!type_parameters.is_empty())),
            Level::Constant(u32::from(!type_parameters.is_empty())),
            Level::Constant(0),
        ],
        index: index.term(arena, &mut Vec::new())?,
        carrier: carrier.term(arena, &mut Vec::new())?,
        children: children.term(arena, &mut Vec::new())?,
        out: out.term(arena, &mut Vec::new())?,
        next: next.term(arena, &mut Vec::new())?,
    })
}

/// Share closed descriptions using ordinary typed transparent definitions.
/// Index and label sorts retain their own inferred telescope levels; a Type0
/// carrier parameter is an index value in Type1, never a Type0 universe axiom.
/// The caller still checks every declaration body in the whole signature.
pub(super) fn intern_description(
    arena: &mut TermArena,
    declarations: &mut Vec<proof_admission::Declaration>,
    mut family: IndexedFamily,
) -> Result<IndexedFamily, &'static str> {
    fn retain(
        arena: &mut TermArena,
        declarations: &mut Vec<proof_admission::Declaration>,
        ty: TermHandle,
        body: TermHandle,
    ) -> TermHandle {
        let declaration = declarations.len() as u32;
        declarations.push(proof_admission::Declaration::definition(0, ty, body));
        arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        })
    }
    let sort = arena.insert(Term::Sort(proof_admission::Sort::Type(Level::Constant(0))));
    let index_sort = arena.insert(Term::Sort(proof_admission::Sort::Type(
        family.levels[0].clone(),
    )));
    let carrier_sort = arena.insert(Term::Sort(proof_admission::Sort::Type(
        family.levels[1].clone(),
    )));
    family.index = retain(arena, declarations, index_sort, family.index);
    family.carrier = retain(arena, declarations, carrier_sort, family.carrier);
    let ty = arena.insert(Term::Pi {
        domain: family.carrier,
        codomain: sort,
    });
    family.children = retain(arena, declarations, ty, family.children);
    let ty = arena.insert(Term::Pi {
        domain: family.carrier,
        codomain: family.index,
    });
    family.out = retain(arena, declarations, ty, family.out);
    let a = arena.insert(Term::Variable(0));
    let domain = arena.insert(Term::Apply {
        function: family.children,
        argument: a,
    });
    let codomain = arena.insert(Term::Pi {
        domain,
        codomain: family.index,
    });
    let ty = arena.insert(Term::Pi {
        domain: family.carrier,
        codomain,
    });
    family.next = retain(arena, declarations, ty, family.next);
    Ok(family)
}

/// Abstract temporary scalar assumptions without capturing existing binders.
pub(super) fn abstract_parameters(
    arena: &mut TermArena,
    proof: TermHandle,
    ty: TermHandle,
    parameters: &[(u32, SymbolHandle, TermHandle)],
) -> Result<(TermHandle, TermHandle), &'static str> {
    fn go(
        arena: &mut TermArena,
        term: TermHandle,
        parameters: &[(u32, SymbolHandle, TermHandle)],
        depth: u32,
    ) -> Result<TermHandle, &'static str> {
        macro_rules! at {
            ($value:expr) => {
                go(arena, $value, parameters, depth)?
            };
        }
        macro_rules! under {
            ($value:expr) => {
                go(arena, $value, parameters, depth + 1)?
            };
        }
        let node = match arena.get(term) {
            Term::Constant {
                declaration,
                levels,
            } => {
                if let Some(ordinal) = parameters
                    .iter()
                    .position(|(position, _, _)| *position == declaration)
                {
                    if !levels.is_empty() {
                        return Err("source value placeholder acquired universe arguments");
                    }
                    Term::Variable(depth + (parameters.len() - ordinal - 1) as u32)
                } else {
                    return Ok(term);
                }
            }
            Term::Variable(index) => {
                if index >= depth {
                    return Err("source case proof contains an unbound value before abstraction");
                }
                return Ok(term);
            }
            Term::Pi { domain, codomain } => Term::Pi {
                domain: at!(domain),
                codomain: under!(codomain),
            },
            Term::Lambda { domain, body } => Term::Lambda {
                domain: at!(domain),
                body: under!(body),
            },
            Term::Sigma { domain, codomain } => Term::Sigma {
                domain: at!(domain),
                codomain: under!(codomain),
            },
            Term::Apply { function, argument } => Term::Apply {
                function: at!(function),
                argument: at!(argument),
            },
            Term::Pair { first, second } => Term::Pair {
                first: at!(first),
                second: at!(second),
            },
            Term::Fst { pair } => Term::Fst { pair: at!(pair) },
            Term::Snd { pair } => Term::Snd { pair: at!(pair) },
            Term::Id { ty, left, right } => Term::Id {
                ty: at!(ty),
                left: at!(left),
                right: at!(right),
            },
            Term::Refl { ty, value } => Term::Refl {
                ty: at!(ty),
                value: at!(value),
            },
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => Term::IdElim {
                motive: at!(motive),
                base: at!(base),
                endpoint: at!(endpoint),
                proof: at!(proof),
            },
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => Term::CaseTwo {
                motive: at!(motive),
                zero_branch: at!(zero_branch),
                one_branch: at!(one_branch),
                scrutinee: at!(scrutinee),
            },
            Term::Box { ty } => Term::Box { ty: at!(ty) },
            Term::BoxIntro { ty, value } => Term::BoxIntro {
                ty: at!(ty),
                value: at!(value),
            },
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => Term::BoxElim {
                motive: at!(motive),
                body: at!(body),
                scrutinee: at!(scrutinee),
            },
            Term::EmptyElim { ty, scrutinee } => Term::EmptyElim {
                ty: at!(ty),
                scrutinee: at!(scrutinee),
            },
            Term::W { carrier, children } => Term::W {
                carrier: at!(carrier),
                children: at!(children),
            },
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => Term::Sup {
                carrier: at!(carrier),
                children: at!(children),
                label: at!(label),
                function: at!(function),
            },
            Term::IndW { motive, step, tree } => Term::IndW {
                motive: at!(motive),
                step: at!(step),
                tree: at!(tree),
            },
            Term::Squash { ty } => Term::Squash { ty: at!(ty) },
            Term::SquashIntro { ty, value } => Term::SquashIntro {
                ty: at!(ty),
                value: at!(value),
            },
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => Term::SquashElim {
                proposition: at!(proposition),
                function: at!(function),
                scrutinee: at!(scrutinee),
            },
            Term::Dummy => return Err("source case proof contains a dummy term"),
            Term::Sort(_) | Term::Two | Term::TwoZero | Term::TwoOne | Term::Empty => {
                return Ok(term);
            }
        };
        Ok(arena.insert(node))
    }
    let mut body = go(arena, proof, parameters, 0)?;
    let mut statement = go(arena, ty, parameters, 0)?;
    for (ordinal, &(_, _, domain)) in parameters.iter().enumerate().rev() {
        // A child/premise domain may depend on earlier value parameters. Its
        // binder is formed under that prefix, not under the full body scope.
        let domain = go(arena, domain, &parameters[..ordinal], 0)?;
        body = arena.insert(Term::Lambda { domain, body });
        statement = arena.insert(Term::Pi {
            domain,
            codomain: statement,
        });
    }
    Ok((body, statement))
}

struct InductionBuilder<'a, 'b> {
    b: Builder<'a>,
    family: &'b IndexedFamily,
    iw: TermHandle,
    label_pack: TermHandle,
    logical_result: TermHandle,
    reconstruction_equality: TermHandle,
    invariant: TermHandle,
    lemmas: &'b [TermHandle],
}
impl InductionBuilder<'_, '_> {
    fn reconstructed_label(&mut self, label: S) -> S {
        let mut function = S::Closed(self.label_pack);
        let mut tail = label;
        for _ in self.b.common {
            function = app(function, fst(tail.clone()));
            tail = snd(tail);
        }
        // The outer case motive names the observed tag and its exact
        // dependent payload separately. Preserve that same reconstruction.
        if self.b.cases.len() > 1 {
            tail = pair(fst(tail.clone()), snd(tail));
        }
        app(function, tail)
    }
    fn label(&mut self, start: usize, mut value: S) -> S {
        for _ in 0..start {
            value = pair(S::One, value);
        }
        // The parametric constructor has a retained dependent A result.
        // Reusing its Pi statement avoids rechecking a raw reconstructed
        // dependent label at each B/next/out occurrence in the step type.
        let mut function = S::Closed(self.label_pack);
        for (symbol, _) in self.b.common {
            function = app(function, S::Var(Key::Source(*symbol)));
        }
        app(function, value)
    }
    fn result_type(&mut self, start: usize, value: S) -> S {
        let label = self.label(start, value);
        app(S::Closed(self.logical_result), label)
    }
    fn type_at_label(&mut self, label: S) -> S {
        let positions = app(S::Closed(self.family.children), label.clone());
        let p = self.b.terms.fresh();
        let next = app(app(S::Closed(self.family.next), label.clone()), S::Var(p));
        let children = pi(p, positions, app(S::Closed(self.iw), next));
        let g = self.b.terms.fresh();
        pi(g, children, app(S::Closed(self.logical_result), label))
    }
    fn unit_value(&mut self) -> S {
        let x = self.b.terms.fresh();
        let ty = self.b.terms.unit_strict();
        S::BoxIntro(Box::new(ty), Box::new(lam(x, S::Empty, S::Var(x))))
    }
    fn position(&mut self, count: usize, ordinal: usize) -> S {
        if count == 1 {
            return self.unit_value();
        }
        if ordinal == 0 {
            let unit = self.unit_value();
            return pair(S::Zero, unit);
        }
        let tail = self.position(count - 1, ordinal - 1);
        pair(S::One, tail)
    }
    fn case_body(&mut self, case: usize, payload: S) -> S {
        let sum_value = if case + 1 == self.b.cases.len() {
            payload.clone()
        } else {
            pair(S::Zero, payload.clone())
        };
        let label = self.label(case, sum_value);
        let positions = app(S::Closed(self.family.children), label.clone());
        let p = self.b.terms.fresh();
        let next = app(app(S::Closed(self.family.next), label), S::Var(p));
        let hypotheses = pi(p, positions, app(S::Closed(self.invariant), next));
        let ih = self.b.terms.fresh();
        let mut body = S::Closed(self.lemmas[case]);
        for (symbol, _) in self.b.common {
            body = app(body, S::Var(Key::Source(*symbol)));
        }
        let mut predicate = payload.clone();
        for _ in &self.b.cases[case].payload {
            body = app(body, fst(predicate.clone()));
            predicate = snd(predicate);
        }
        body = app(body, predicate);
        let count = self.b.cases[case].child_indices.len();
        for child in 0..count {
            let position = self.position(count, child);
            body = app(body, app(S::Var(ih), position));
        }
        lam(ih, hypotheses, body)
    }
    fn eliminate(&mut self, start: usize, value: S) -> Result<S, &'static str> {
        if start >= self.b.cases.len() {
            return Err("source induction cannot eliminate an empty constructor roster");
        }
        if start + 1 == self.b.cases.len() {
            return Ok(self.case_body(start, value));
        }
        let tag = self.b.terms.fresh();
        let payload = self.b.terms.fresh();
        let tm = self.b.terms.type_motive();
        let zero_type = self.b.payload_type(start);
        let one_type = self.b.sum_type(start + 1);
        let payload_type = choose(tm, zero_type.clone(), one_type.clone(), S::Var(tag));
        let result = self.result_type(start, pair(S::Var(tag), S::Var(payload)));
        let motive = lam(tag, S::Two, pi(payload, payload_type, result));
        let z = self.b.terms.fresh();
        let zero_body = self.case_body(start, S::Var(z));
        let zero = lam(z, zero_type, zero_body);
        let o = self.b.terms.fresh();
        let one_body = self.eliminate(start + 1, S::Var(o))?;
        let one = lam(o, one_type, one_body);
        Ok(app(
            choose(motive, zero, one, fst(value.clone())),
            snd(value),
        ))
    }
    fn step(&mut self) -> Result<S, &'static str> {
        let a = self.b.terms.fresh();
        let tail = self.b.terms.fresh();
        let sum_type = self.b.sum_type(0);
        let body = self.eliminate(0, S::Var(tail))?;
        let mut function = lam(tail, sum_type, body);
        for (symbol, ty) in self.b.common.iter().rev() {
            function = lam(Key::Source(*symbol), self.b.domain(*symbol, *ty), function);
        }
        let mut value = S::Var(a);
        for _ in self.b.common {
            function = app(function, fst(value.clone()));
            value = snd(value);
        }
        // The motive is index-only, so g does not occur in the hypothesis
        // or conclusion. Keep its exact iindW domain once outside the case
        // eliminator instead of duplicating an IW formation in every motive.
        let p = self.b.terms.fresh();
        let positions = app(S::Closed(self.family.children), S::Var(a));
        let next = app(app(S::Closed(self.family.next), S::Var(a)), S::Var(p));
        let children = pi(p, positions, app(S::Closed(self.iw), next));
        let g = self.b.terms.fresh();
        let endpoint = self.b.terms.fresh();
        let equality = self.b.terms.fresh();
        let reconstructed = self.reconstructed_label(S::Var(a));
        let equality_domain = S::Identity(
            Box::new(S::Closed(self.family.carrier)),
            Box::new(reconstructed),
            Box::new(S::Var(endpoint)),
        );
        let motive = lam(
            endpoint,
            S::Closed(self.family.carrier),
            lam(
                equality,
                equality_domain,
                app(S::Closed(self.logical_result), S::Var(endpoint)),
            ),
        );
        let transported = S::IdentityElimination(
            Box::new(motive),
            Box::new(app(function, value)),
            Box::new(S::Var(a)),
            Box::new(app(S::Closed(self.reconstruction_equality), S::Var(a))),
        );
        Ok(lam(
            a,
            S::Closed(self.family.carrier),
            lam(g, children, transported),
        ))
    }
}

/// Share the exact invariant before any case lemma is produced. The same
/// constant then occurs in every lemma statement and the W induction step;
/// its original authored body remains part of the checked dependency closure.
pub(super) fn intern_invariant(
    arena: &mut TermArena,
    declarations: &mut Vec<proof_admission::Declaration>,
    family: &IndexedFamily,
    invariant: TermHandle,
) -> TermHandle {
    let sort = arena.insert(Term::Sort(proof_admission::Sort::Type(Level::Constant(0))));
    let ty = arena.insert(Term::Pi {
        domain: family.index,
        codomain: sort,
    });
    let declaration = declarations.len() as u32;
    declarations.push(proof_admission::Declaration::definition(0, ty, invariant));
    arena.insert(Term::Constant {
        declaration,
        levels: vec![],
    })
}

/// Abstract a checked closed proposition proof over unused index/evidence
/// binders. The caller must check the complete original Pi statement; this
/// builder does not decide whether the invariant actually ignores its index.
pub(super) fn constant_invariant_theorem(
    arena: &mut TermArena,
    family: &IndexedFamily,
    invariant: TermHandle,
    proof: TermHandle,
) -> (TermHandle, TermHandle) {
    let i = arena.insert(Term::Variable(0));
    let evidence_type = family.indexed_w(arena, i);
    let closed = proof_admission::shift(arena, proof, 0, 2);
    let body = arena.insert(Term::Lambda {
        domain: evidence_type,
        body: closed,
    });
    let theorem = arena.insert(Term::Lambda {
        domain: family.index,
        body,
    });
    let i = arena.insert(Term::Variable(1));
    let proposition = arena.insert(Term::Apply {
        function: invariant,
        argument: i,
    });
    let result = arena.insert(Term::Pi {
        domain: evidence_type,
        codomain: proposition,
    });
    let ty = arena.insert(Term::Pi {
        domain: family.index,
        codomain: result,
    });
    (theorem, ty)
}

/// Construct the ordinary iindW application. Every case lemma is quantified
/// over exact common/payload coordinates, its complete predicate, then one
/// invariant hypothesis per exact child in declaration order.
pub(super) fn induction(
    arena: &mut TermArena,
    family: &IndexedFamily,
    common: &[(SymbolHandle, TermHandle)],
    index_fields: &[SymbolHandle],
    cases: &[Case],
    invariant: TermHandle,
    lemmas: &[TermHandle],
    declarations: &mut Vec<proof_admission::Declaration>,
    type_parameters: &[SymbolHandle],
) -> Result<(TermHandle, TermHandle, TermHandle, TermHandle), &'static str> {
    if cases.len() != lemmas.len() {
        return Err("source induction lemma roster differs from its exact cases");
    }
    fn retain(
        arena: &mut TermArena,
        declarations: &mut Vec<proof_admission::Declaration>,
        ty: TermHandle,
        body: TermHandle,
    ) -> TermHandle {
        let declaration = declarations.len() as u32;
        declarations.push(proof_admission::Declaration::definition(0, ty, body));
        arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        })
    }
    let w_sort = arena.insert(Term::Sort(proof_admission::Sort::Type(
        family
            .levels
            .iter()
            .cloned()
            .reduce(|left, right| left.maximum(right))
            .expect("three W levels"),
    )));
    let function_type = arena.insert(Term::Pi {
        domain: family.index,
        codomain: w_sort,
    });
    let i = arena.insert(Term::Variable(0));
    let iw_body = family.indexed_w(arena, i);
    let iw_function = arena.insert(Term::Lambda {
        domain: family.index,
        body: iw_body,
    });
    let iw = retain(arena, declarations, function_type, iw_function);
    let iw_body = arena.insert(Term::Apply {
        function: iw,
        argument: i,
    });
    let mut builder = Builder {
        terms: SourceTerms::default(),
        common,
        index_fields,
        cases,
        type_parameters,
    };
    let tail = builder.terms.fresh();
    let sum_type = builder.sum_type(0);
    let mut raw_label = S::Var(tail);
    for (symbol, _) in common.iter().rev() {
        raw_label = pair(S::Var(Key::Source(*symbol)), raw_label);
    }
    let mut pack = lam(tail, sum_type.clone(), raw_label);
    let mut pack_type = pi(tail, sum_type, S::Closed(family.carrier));
    for (symbol, ty) in common.iter().rev() {
        pack = lam(Key::Source(*symbol), builder.domain(*symbol, *ty), pack);
        pack_type = pi(
            Key::Source(*symbol),
            builder.domain(*symbol, *ty),
            pack_type,
        );
    }
    let pack = pack.term(arena, &mut Vec::new())?;
    let pack_type = pack_type.term(arena, &mut Vec::new())?;
    let label_pack = retain(arena, declarations, pack_type, pack);
    // All source case branches prove this same index-only result. Sharing
    // its transparent definition avoids reconstructing the dependent IH/out
    // telescope in every motive and every expected-sort query. The recursive
    // tree argument still keeps its full IW level outside this Type0 result.
    let a = arena.insert(Term::Variable(0));
    let positions = arena.insert(Term::Apply {
        function: family.children,
        argument: a,
    });
    let next = arena.insert(Term::Apply {
        function: family.next,
        argument: a,
    });
    let position = arena.insert(Term::Variable(0));
    let next = proof_admission::shift(arena, next, 0, 1);
    let next = arena.insert(Term::Apply {
        function: next,
        argument: position,
    });
    let hypothesis = arena.insert(Term::Apply {
        function: invariant,
        argument: next,
    });
    let hypotheses = arena.insert(Term::Pi {
        domain: positions,
        codomain: hypothesis,
    });
    let out = arena.insert(Term::Apply {
        function: family.out,
        argument: a,
    });
    let goal = arena.insert(Term::Apply {
        function: invariant,
        argument: out,
    });
    let goal = proof_admission::shift(arena, goal, 0, 1);
    let result = arena.insert(Term::Pi {
        domain: hypotheses,
        codomain: goal,
    });
    let result = arena.insert(Term::Lambda {
        domain: family.carrier,
        body: result,
    });
    let proposition_sort =
        arena.insert(Term::Sort(proof_admission::Sort::Type(Level::Constant(0))));
    let result_type = arena.insert(Term::Pi {
        domain: family.carrier,
        codomain: proposition_sort,
    });
    let logical_result = retain(arena, declarations, result_type, result);
    let mut producer = InductionBuilder {
        b: Builder {
            terms: SourceTerms::default(),
            common,
            index_fields,
            cases,
            type_parameters,
        },
        family,
        iw,
        label_pack,
        logical_result,
        reconstruction_equality: TermHandle::default(),
        invariant,
        lemmas,
    };
    // Check typed dependent-pair eta once at the actual label carrier. The
    // case adapter then transports its already proved result along this
    // ordinary equality, rather than repeatedly comparing the reconstructed
    // label beneath B/next and every recursive hypothesis. This introduces
    // neither an equality assumption nor a new computation rule: the full
    // equality definition and the J motive/base remain signature-checked.
    let label = producer.b.terms.fresh();
    let reconstructed = producer.reconstructed_label(S::Var(label));
    let equality_type = pi(
        label,
        S::Closed(family.carrier),
        S::Identity(
            Box::new(S::Closed(family.carrier)),
            Box::new(reconstructed.clone()),
            Box::new(S::Var(label)),
        ),
    )
    .term(arena, &mut Vec::new())?;
    let equality_body = lam(
        label,
        S::Closed(family.carrier),
        S::Reflexivity(Box::new(S::Closed(family.carrier)), Box::new(reconstructed)),
    )
    .term(arena, &mut Vec::new())?;
    producer.reconstruction_equality = retain(arena, declarations, equality_type, equality_body);
    let step_body = producer.step()?.term(arena, &mut Vec::new())?;
    let a = producer.b.terms.fresh();
    let result_type = producer.type_at_label(S::Var(a));
    let step_type = pi(a, S::Closed(family.carrier), result_type).term(arena, &mut Vec::new())?;
    let declaration = declarations.len() as u32;
    declarations.push(proof_admission::Declaration::definition(
        0, step_type, step_body,
    ));
    let step = arena.insert(Term::Constant {
        declaration,
        levels: vec![],
    });
    let i = arena.insert(Term::Variable(1));
    let proposition = arena.insert(Term::Apply {
        function: invariant,
        argument: i,
    });
    let motive_body = arena.insert(Term::Lambda {
        domain: iw_body,
        body: proposition,
    });
    let motive = arena.insert(Term::Lambda {
        domain: family.index,
        body: motive_body,
    });
    let tree = arena.insert(Term::Variable(0));
    let body = family.ind(arena, Level::Constant(0), motive, step, i, tree);
    let body = arena.insert(Term::Lambda {
        domain: iw_body,
        body,
    });
    let theorem = arena.insert(Term::Lambda {
        domain: family.index,
        body,
    });
    let result = arena.insert(Term::Pi {
        domain: iw_body,
        codomain: proposition,
    });
    let ty = arena.insert(Term::Pi {
        domain: family.index,
        codomain: result,
    });
    Ok((theorem, ty, step_body, step_type))
}

#[cfg(test)]
mod induction_tests {
    use super::*;
    use proof_admission::{Budget, Context, DEFAULT_CONVERSION_STEPS, Signature};

    #[test]
    fn constructor_abstraction_binds_dependent_child_and_premise_domains() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let value = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![],
        });
        let predicate = arena.insert(Term::Id {
            ty: two,
            left: value,
            right: value,
        });
        let evidence = arena.insert(Term::Constant {
            declaration: 1,
            levels: vec![],
        });
        let declarations = vec![
            proof_admission::Declaration::assumption(0, two),
            proof_admission::Declaration::assumption(0, predicate),
        ];
        let parameters = [
            (0, SymbolHandle::from_arena_index(31), two),
            (1, SymbolHandle::from_arena_index(32), predicate),
        ];
        let (proof, statement) =
            abstract_parameters(&mut arena, evidence, predicate, &parameters).unwrap();
        assert!(excludes_parameters(
            &arena,
            &declarations,
            &[proof, statement],
            &parameters
        ));
        let context = Context::empty().with_signature(Signature::from_declarations(declarations));
        proof_admission::check_type(
            &mut arena,
            &context,
            proof,
            statement,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();

        // The returned proof is parametric: changing the supplied child's
        // endpoint cannot establish a different receiving equality.
        let zero = arena.insert(Term::TwoZero);
        let one = arena.insert(Term::TwoOne);
        let supplied = arena.insert(Term::Apply {
            function: proof,
            argument: zero,
        });
        let reflexivity = arena.insert(Term::Refl {
            ty: two,
            value: zero,
        });
        let supplied = arena.insert(Term::Apply {
            function: supplied,
            argument: reflexivity,
        });
        let false_result = arena.insert(Term::Id {
            ty: two,
            left: zero,
            right: one,
        });
        assert!(
            proof_admission::check_type(
                &mut arena,
                &context,
                supplied,
                false_result,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .is_err()
        );
    }

    #[test]
    fn source_case_abstraction_preserves_bound_proof_variables_and_dependency_closure() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let mut declarations = vec![proof_admission::Declaration {
            level_arity: 0,
            ty: two,
            body: None,
        }];
        let value = arena.insert(Term::Constant {
            declaration: 0,
            levels: vec![],
        });
        let predicate = arena.insert(Term::Id {
            ty: two,
            left: value,
            right: value,
        });
        let bound = arena.insert(Term::Variable(0));
        let proof = arena.insert(Term::Lambda {
            domain: predicate,
            body: bound,
        });
        let ty = arena.insert(Term::Pi {
            domain: predicate,
            codomain: predicate,
        });
        let parameters = [(0, SymbolHandle::from_arena_index(31), two)];
        assert!(!excludes_parameters(
            &arena,
            &declarations,
            &[proof, ty],
            &parameters
        ));
        let (proof, ty) = abstract_parameters(&mut arena, proof, ty, &parameters).unwrap();
        assert!(excludes_parameters(
            &arena,
            &declarations,
            &[proof, ty],
            &parameters
        ));
        let context =
            Context::empty().with_signature(Signature::from_declarations(declarations.clone()));
        proof_admission::check_type(
            &mut arena,
            &context,
            proof,
            ty,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        declarations.push(proof_admission::Declaration {
            level_arity: 0,
            ty: two,
            body: Some(value),
        });
        let hidden = arena.insert(Term::Constant {
            declaration: 1,
            levels: vec![],
        });
        assert!(!excludes_parameters(
            &arena,
            &declarations,
            &[hidden],
            &parameters
        ));
    }

    #[test]
    fn generated_source_induction_checks_multiple_cases_and_child_positions() {
        let mut arena = TermArena::new();
        let mut declarations = proof_admission::indexed_scheme(&mut arena);
        let two = arena.insert(Term::Two);
        let field = SymbolHandle::from_arena_index(2);
        let x = Key::Source(field);
        let empty = arena.insert(Term::Empty);
        let strict_unit = arena.insert(Term::Pi {
            domain: empty,
            codomain: empty,
        });
        let unit = arena.insert(Term::Box { ty: strict_unit });
        let constraints = lam(x, S::Closed(two), S::Closed(unit))
            .term(&mut arena, &mut Vec::new())
            .unwrap();
        let child_index = lam(x, S::Closed(two), S::Var(x))
            .term(&mut arena, &mut Vec::new())
            .unwrap();
        let cases = [0usize, 1]
            .into_iter()
            .map(|count| Case {
                payload: vec![],
                constraints,
                child_indices: vec![child_index; count],
            })
            .collect::<Vec<_>>();
        let family = description(&mut arena, &[(field, two)], &[field], &cases, &[]).unwrap();
        let family = intern_description(&mut arena, &mut declarations, family).unwrap();
        let index = arena.insert(Term::Variable(0));
        let reflexive = arena.insert(Term::Id {
            ty: two,
            left: index,
            right: index,
        });
        let invariant = arena.insert(Term::Lambda {
            domain: two,
            body: reflexive,
        });
        let invariant = intern_invariant(&mut arena, &mut declarations, &family, invariant);
        let mut lemmas = Vec::new();
        for case in &cases {
            let constant = arena.insert(Term::Variable((case.child_indices.len() + 1) as u32));
            let mut body = arena.insert(Term::Refl {
                ty: two,
                value: constant,
            });
            for ordinal in (0..case.child_indices.len()).rev() {
                let value = arena.insert(Term::Variable((ordinal + 1) as u32));
                let domain = arena.insert(Term::Id {
                    ty: two,
                    left: value,
                    right: value,
                });
                body = arena.insert(Term::Lambda { domain, body });
            }
            body = arena.insert(Term::Lambda { domain: unit, body });
            body = arena.insert(Term::Lambda { domain: two, body });
            let context =
                Context::empty().with_signature(Signature::from_declarations(declarations.clone()));
            let ty = proof_admission::infer_type(
                &mut arena,
                &context,
                body,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            let declaration = declarations.len() as u32;
            declarations.push(proof_admission::Declaration::definition(0, ty, body));
            lemmas.push(arena.insert(Term::Constant {
                declaration,
                levels: vec![],
            }));
        }
        let (theorem, ty, step, step_type) = induction(
            &mut arena,
            &family,
            &[(field, two)],
            &[field],
            &cases,
            invariant,
            &lemmas,
            &mut declarations,
            &[],
        )
        .unwrap();
        let context =
            Context::empty().with_signature(Signature::from_declarations(declarations.clone()));
        proof_admission::check_type(
            &mut arena,
            &context,
            step,
            step_type,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        proof_admission::check_signature(&mut arena, &declarations, &mut budget).unwrap();
        let context = Context::empty().with_signature(Signature::from_declarations(declarations));
        proof_admission::check_type(
            &mut arena,
            &context,
            theorem,
            ty,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        let sort = arena.insert(Term::Sort(proof_admission::Sort::Type(Level::Constant(0))));
        assert!(
            proof_admission::check_type(
                &mut arena,
                &context,
                theorem,
                sort,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .is_err()
        );
    }
}

/// Check actual dependency closure rather than the ambient signature roster:
/// a temporary value assumption may remain stored, but no theorem declaration
/// or type can reach it through a transparent definition or another constant.
pub(super) fn excludes_parameters(
    arena: &TermArena,
    declarations: &[proof_admission::Declaration],
    roots: &[TermHandle],
    parameters: &[(u32, SymbolHandle, TermHandle)],
) -> bool {
    let mut pending = roots.to_vec();
    let mut seen_terms = std::collections::HashSet::new();
    let mut seen_declarations = std::collections::HashSet::new();
    while let Some(term) = pending.pop() {
        if !seen_terms.insert(term) {
            continue;
        }
        match arena.get(term) {
            Term::Constant { declaration, .. } => {
                if parameters
                    .iter()
                    .any(|(position, _, _)| *position == declaration)
                {
                    return false;
                }
                if seen_declarations.insert(declaration) {
                    let Some(declaration) = declarations.get(declaration as usize) else {
                        return false;
                    };
                    pending.push(declaration.ty);
                    pending.extend(declaration.body);
                }
            }
            Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                pending.extend([domain, codomain])
            }
            Term::Lambda { domain, body } => pending.extend([domain, body]),
            Term::Apply { function, argument } => pending.extend([function, argument]),
            Term::Pair { first, second } => pending.extend([first, second]),
            Term::Fst { pair } | Term::Snd { pair } => pending.push(pair),
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => pending.extend([motive, zero_branch, one_branch, scrutinee]),
            Term::Id { ty, left, right } => pending.extend([ty, left, right]),
            Term::Refl { ty, value }
            | Term::SquashIntro { ty, value }
            | Term::BoxIntro { ty, value } => pending.extend([ty, value]),
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => pending.extend([motive, base, endpoint, proof]),
            Term::W { carrier, children } => pending.extend([carrier, children]),
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => pending.extend([carrier, children, label, function]),
            Term::IndW { motive, step, tree } => pending.extend([motive, step, tree]),
            Term::EmptyElim { ty, scrutinee } => pending.extend([ty, scrutinee]),
            Term::Squash { ty } | Term::Box { ty } => pending.push(ty),
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => pending.extend([proposition, function, scrutinee]),
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => pending.extend([motive, body, scrutinee]),
            Term::Dummy => return false,
            Term::Variable(_)
            | Term::Sort(_)
            | Term::Two
            | Term::TwoZero
            | Term::TwoOne
            | Term::Empty => {}
        }
    }
    true
}

#[cfg(test)]
mod indexed_vector_budget_tests {
    use super::*;
    use numerics::bignum::BigInt;
    use proof_admission::{
        Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Signature, SourceIntegerTheory,
    };

    fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
        arena.insert(Term::Apply { function, argument })
    }
    fn binary(
        arena: &mut TermArena,
        operation: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        let function = apply(arena, operation, left);
        apply(arena, function, right)
    }
    fn equality(
        arena: &mut TermArena,
        integer: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        arena.insert(Term::Id {
            ty: integer,
            left,
            right,
        })
    }
    fn and(arena: &mut TermArena, left: TermHandle, right: TermHandle) -> TermHandle {
        let codomain = proof_admission::shift(arena, right, 0, 1);
        arena.insert(Term::Sigma {
            domain: left,
            codomain,
        })
    }
    fn close(
        arena: &mut TermArena,
        integer: TermHandle,
        count: usize,
        mut body: TermHandle,
    ) -> TermHandle {
        for _ in 0..count {
            body = arena.insert(Term::Lambda {
                domain: integer,
                body,
            });
        }
        body
    }
    fn close_source_telescope(
        arena: &mut TermArena,
        declarations: &[Declaration],
        source: TermHandle,
        kernel: TermHandle,
        count: usize,
        body: TermHandle,
    ) -> TermHandle {
        let raw = close(arena, source, count, body);
        let compact = close(arena, kernel, count, body);
        let context =
            Context::empty().with_signature(Signature::from_declarations(declarations.to_vec()));
        // For child-index functions the result is I rather than Type0;
        // conversion is still typed at the independently inferred Pi below.
        let inferred = proof_admission::infer_type(
            arena,
            &context,
            compact,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        let raw_type = proof_admission::infer_type(
            arena,
            &context,
            raw,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        assert_eq!(
            proof_admission::judgment_assumption_closure(
                arena,
                context.signature(),
                &[raw, raw_type]
            ),
            proof_admission::judgment_assumption_closure(
                arena,
                context.signature(),
                &[compact, inferred]
            ),
            "generated primitive-domain compaction must preserve the full judgment assumption closure",
        );
        assert!(
            proof_admission::convertible(
                arena,
                &context,
                raw,
                compact,
                inferred,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .unwrap()
        );
        compact
    }
    fn checked(
        arena: &mut TermArena,
        declarations: &[Declaration],
        term: TermHandle,
        ty: TermHandle,
        role: &str,
    ) {
        let context =
            Context::empty().with_signature(Signature::from_declarations(declarations.to_vec()));
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let result = proof_admission::check_type(arena, &context, term, ty, &mut budget);
        eprintln!(
            "indexed vector {role}: {} kernel steps",
            DEFAULT_CONVERSION_STEPS - budget.remaining()
        );
        result.unwrap_or_else(|error| panic!("indexed vector {role}: {error:?}"));
    }
    fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
        arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        })
    }

    /// Exact structural/arithmetic obligation of the frozen source vector:
    /// indices (Length,Total), Cons payload (previous,previous_total,head),
    /// complete constructor constraints, and the one child induction premise.
    /// Source declaration correspondence is separately exercised by the CLI.
    #[test]
    fn indexed_vector_integer_pair_step_and_theorem_fit_admission_budget() {
        vector_budget(false);
    }
    #[test]
    fn indexed_vector_source_alias_and_embeddings_fit_admission_budget() {
        vector_budget(true);
    }
    fn vector_budget(source_like: bool) {
        let mut arena = TermArena::new();
        let mut declarations = proof_admission::indexed_scheme(&mut arena);
        let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
        let vocabulary = theory.vocabulary();
        let integer = constant(&mut arena, vocabulary.carrier);
        let scalar = if source_like {
            let sort = arena.insert(Term::Sort(proof_admission::Sort::Type(Level::Constant(0))));
            let alias = declarations.len() as u32;
            declarations.push(Declaration::definition(0, sort, integer));
            constant(&mut arena, alias)
        } else {
            integer
        };
        let mut embedding_declaration = None;
        let mut embed =
            |arena: &mut TermArena, declarations: &mut Vec<Declaration>, value: TermHandle| {
                if !source_like {
                    return value;
                }
                let ty = arena.insert(Term::Pi {
                    domain: integer,
                    codomain: integer,
                });
                let variable = arena.insert(Term::Variable(0));
                let body = arena.insert(Term::Lambda {
                    domain: integer,
                    body: variable,
                });
                let declaration = *embedding_declaration.get_or_insert_with(|| {
                    let declaration = declarations.len() as u32;
                    declarations.push(Declaration::definition(0, ty, body));
                    declaration
                });
                let function = constant(arena, declaration);
                let raw = apply(arena, function, value);
                let mut context = Context::empty()
                    .with_signature(Signature::from_declarations(declarations.clone()));
                for _ in 0..5 {
                    context = context.extend(scalar);
                }
                assert!(
                    proof_admission::convertible(
                        arena,
                        &context,
                        raw,
                        value,
                        integer,
                        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                    )
                    .unwrap()
                );
                // Independently selected fixed integer embedding has this
                // denotation; the raw identity is retained above only to
                // check the representation change by kernel conversion.
                value
            };
        let add = constant(&mut arena, vocabulary.add);
        let multiply = constant(&mut arena, vocabulary.multiply);
        let less_or_equal = constant(&mut arena, vocabulary.less_or_equal);
        let zero = theory
            .literal(&mut arena, &mut declarations, BigInt::from_i128(0))
            .unwrap();
        let one = theory
            .literal(&mut arena, &mut declarations, BigInt::from_i128(1))
            .unwrap();
        let ten = theory
            .literal(&mut arena, &mut declarations, BigInt::from_i128(10))
            .unwrap();
        let length_symbol = SymbolHandle::from_arena_index(10);
        let total_symbol = SymbolHandle::from_arena_index(11);
        let payload_symbols = [12, 13, 14].map(SymbolHandle::from_arena_index);
        let length = arena.insert(Term::Variable(1));
        let total = arena.insert(Term::Variable(0));
        let length_zero = equality(&mut arena, integer, length, zero);
        let total_zero = equality(&mut arena, integer, total, zero);
        let nil_predicate = and(&mut arena, length_zero, total_zero);
        let nil_constraints =
            close_source_telescope(&mut arena, &declarations, scalar, integer, 2, nil_predicate);
        let length = arena.insert(Term::Variable(4));
        let total = arena.insert(Term::Variable(3));
        let previous = arena.insert(Term::Variable(2));
        let previous_total = arena.insert(Term::Variable(1));
        let head = arena.insert(Term::Variable(0));
        let head_bound = binary(&mut arena, less_or_equal, head, ten);
        let previous = embed(&mut arena, &mut declarations, previous);
        let embedded_length = embed(&mut arena, &mut declarations, length);
        let successor = binary(&mut arena, add, previous, one);
        let length_equation = equality(&mut arena, integer, successor, embedded_length);
        let embedded_previous_total = embed(&mut arena, &mut declarations, previous_total);
        let embedded_head = embed(&mut arena, &mut declarations, head);
        let embedded_total = embed(&mut arena, &mut declarations, total);
        let sum = binary(&mut arena, add, embedded_previous_total, embedded_head);
        let total_equation = equality(&mut arena, integer, sum, embedded_total);
        let equations = and(&mut arena, length_equation, total_equation);
        let cons_predicate = and(&mut arena, head_bound, equations);
        let cons_constraints = close_source_telescope(
            &mut arena,
            &declarations,
            scalar,
            integer,
            5,
            cons_predicate,
        );
        let previous = arena.insert(Term::Variable(2));
        let index = arena.insert(Term::Pair {
            first: previous,
            second: previous_total,
        });
        let child_index =
            close_source_telescope(&mut arena, &declarations, scalar, integer, 5, index);
        let cases = vec![
            Case {
                payload: vec![],
                constraints: nil_constraints,
                child_indices: vec![],
            },
            Case {
                payload: payload_symbols
                    .iter()
                    .map(|&symbol| (symbol, integer))
                    .collect(),
                constraints: cons_constraints,
                child_indices: vec![child_index],
            },
        ];
        // The internal W uses the fixed primitive carrier; exact authored
        // alias domains remain in the constraint/invariant functions above.
        let common = [(length_symbol, integer), (total_symbol, integer)];
        let indices = [length_symbol, total_symbol];
        let family = description(&mut arena, &common, &indices, &cases, &[]).unwrap();
        let family = intern_description(&mut arena, &mut declarations, family).unwrap();
        let i = arena.insert(Term::Variable(0));
        let length = arena.insert(Term::Fst { pair: i });
        let total = arena.insert(Term::Snd { pair: i });
        let (authored_length, authored_total) = if source_like {
            (
                arena.insert(Term::Variable(1)),
                arena.insert(Term::Variable(0)),
            )
        } else {
            (length, total)
        };
        let authored_length = embed(&mut arena, &mut declarations, authored_length);
        let authored_total = embed(&mut arena, &mut declarations, authored_total);
        let bound = binary(&mut arena, multiply, ten, authored_length);
        let goal = binary(&mut arena, less_or_equal, authored_total, bound);
        let goal = if source_like {
            let authored = close(&mut arena, scalar, 2, goal);
            let applied = apply(&mut arena, authored, length);
            let raw_goal = apply(&mut arena, applied, total);
            // Reduce only the binder/application scaffolding just built by
            // the source invariant producer. No authored source function or
            // predicate declaration is normalized or discarded.
            let Term::Lambda { body, .. } = arena.get(authored) else {
                unreachable!()
            };
            let remaining = proof_admission::substitute(&mut arena, body, length);
            let Term::Lambda { body, .. } = arena.get(remaining) else {
                unreachable!()
            };
            let normalized = proof_admission::substitute(&mut arena, body, total);
            let context = Context::empty()
                .with_signature(Signature::from_declarations(declarations.clone()))
                .extend(family.index);
            let sort = arena.insert(Term::Sort(proof_admission::Sort::Type(Level::Constant(0))));
            assert!(
                proof_admission::convertible(
                    &mut arena,
                    &context,
                    raw_goal,
                    normalized,
                    sort,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap()
            );
            normalized
        } else {
            goal
        };
        let invariant = arena.insert(Term::Lambda {
            domain: family.index,
            body: goal,
        });
        let invariant = intern_invariant(&mut arena, &mut declarations, &family, invariant);
        let mut lemmas = Vec::new();
        for (ordinal, case) in cases.iter().enumerate() {
            let fields = common
                .iter()
                .chain(&case.payload)
                .copied()
                .collect::<Vec<_>>();
            let mut parameters = Vec::new();
            let mut values = Vec::new();
            for &(symbol, domain) in &fields {
                let declaration = declarations.len() as u32;
                declarations.push(Declaration {
                    level_arity: 0,
                    ty: domain,
                    body: None,
                });
                parameters.push((declaration, symbol, domain));
                values.push(constant(&mut arena, declaration));
            }
            let mut predicate = case.constraints;
            for &value in &values {
                predicate = apply(&mut arena, predicate, value);
            }
            let index = arena.insert(Term::Pair {
                first: values[0],
                second: values[1],
            });
            let goal = apply(&mut arena, invariant, index);
            let mut assumptions = vec![predicate];
            for &function in &case.child_indices {
                let mut index = function;
                for &value in &values {
                    index = apply(&mut arena, index, value);
                }
                assumptions.push(apply(&mut arena, invariant, index));
            }
            let proof = theory
                .predicate_from_assumptions(&mut arena, &mut declarations, goal, &assumptions)
                .unwrap()
                .unwrap();
            let mut ty = goal;
            for &domain in assumptions.iter().rev() {
                let codomain = proof_admission::shift(&mut arena, ty, 0, 1);
                ty = arena.insert(Term::Pi { domain, codomain });
            }
            let (proof, ty) = abstract_parameters(&mut arena, proof, ty, &parameters).unwrap();
            assert!(excludes_parameters(
                &arena,
                &declarations,
                &[proof, ty],
                &parameters
            ));
            checked(
                &mut arena,
                &declarations,
                proof,
                ty,
                &format!("case {ordinal}"),
            );
            let declaration = declarations.len() as u32;
            declarations.push(Declaration::definition(0, ty, proof));
            lemmas.push(constant(&mut arena, declaration));
        }
        let (theorem, ty, step, step_type) = induction(
            &mut arena,
            &family,
            &common,
            &indices,
            &cases,
            invariant,
            &lemmas,
            &mut declarations,
            &[],
        )
        .unwrap();
        checked(&mut arena, &declarations, step, step_type, "step");
        checked(&mut arena, &declarations, theorem, ty, "theorem");
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let result = proof_admission::check_signature(&mut arena, &declarations, &mut budget);
        eprintln!(
            "indexed vector signature: {} kernel steps",
            DEFAULT_CONVERSION_STEPS - budget.remaining()
        );
        result.unwrap();
        // Closed source specializations use direct arithmetic evidence instead
        // of replaying the generic induction step for every generated clone.
        let eighty = theory
            .literal(&mut arena, &mut declarations, BigInt::from_i128(80))
            .unwrap();
        let closed_goal = binary(&mut arena, less_or_equal, ten, eighty);
        let closed_proof = theory
            .closed_predicate(&mut arena, &mut declarations, closed_goal)
            .unwrap()
            .unwrap();
        let closed_invariant = arena.insert(Term::Lambda {
            domain: family.index,
            body: closed_goal,
        });
        let (theorem, ty) =
            constant_invariant_theorem(&mut arena, &family, closed_invariant, closed_proof);
        checked(
            &mut arena,
            &declarations,
            theorem,
            ty,
            "closed specialization",
        );
        let false_goal = binary(&mut arena, less_or_equal, eighty, ten);
        let false_invariant = arena.insert(Term::Lambda {
            domain: family.index,
            body: false_goal,
        });
        let (_, false_type) =
            constant_invariant_theorem(&mut arena, &family, false_invariant, closed_proof);
        let context = Context::empty().with_signature(Signature::from_declarations(declarations));
        assert!(
            proof_admission::check_type(
                &mut arena,
                &context,
                theorem,
                false_type,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .is_err()
        );
    }
}
