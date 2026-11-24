//! A value-indexed `data` declaration as an indexed W family.
//!
//! [`IndexedFamily`] in `indexed.rs` turns five functions (`index`,
//! `carrier`, `children`, `out`, `next`) into indexed trees, their
//! constructor `sup` and their induction `ind`. This module builds those five
//! functions from a description of a declaration such as
//!
//! ```text
//! data Vector<Length: u64, Total: u64> requires ... {
//!     case Nil;
//!     case Cons(previous: u64, previous_total: u64, head: u64,
//!               tail [erased]: Vector<previous, previous_total>);
//! }
//! ```
//!
//! and builds a constructor's value and the step of an induction over an
//! invariant of the index. The
//! [inductive profile](../../../../../../wiki/spec/proofs/inductive_profile.md#derived-indexed-families)
//! defines the scheme: an unindexed W tree paired with a recursively defined
//! indexing condition. This module adds no kernel rule and no assumption; the
//! kernel checks every declaration it emits and every term it returns.
//!
//! The encoding, in kernel types:
//!
//! - The index is the tuple of the index fields, `Sigma` nested to the right,
//!   or the boxed unit `Box(Empty -> Empty)` when there are none.
//! - A label (an element of the W carrier) is the tuple of the shared fields,
//!   followed by the sum of the cases. The sum tags case `k` of `n` with
//!   `k` copies of `TwoOne`, then `TwoZero` unless it is the last case; a
//!   single case carries no tag. Each case's payload is the tuple of its
//!   payload fields followed by a proof of its constraints, so a label
//!   carries the constraint proof and the W tree cannot hold a node whose
//!   constraints fail.
//! - A case with `c` children has `c` child positions: `Box(Empty)` for none,
//!   the boxed unit for one, and `Sigma(Two, case(unit, rest))` for more.
//!   `next(label, position)` selects that child's index function, applied
//!   to the shared fields and the payload.
//! - `out(label)` is the tuple of the label's index fields.
//!
//! Every field domain, constraint and child index is a closed term chosen by
//! the caller. Fields and binders are named by dense numbers the caller
//! chooses (`u32`); this crate does not depend on the compiler's symbol
//! table. With parameters, each non-parameter field and payload domain is a
//! function of all parameters (`\(parameters). domain`); a constraint is a
//! function of the parameters, the shared fields that are not parameters,
//! and the case's payload fields, in that order; a child index function also
//! takes those arguments and returns the index. A shared field whose name is
//! a parameter's name stores that parameter in every label, which makes a
//! non-uniform type argument an index. The other, uniform, parameters stay
//! outside the tree: the five functions are declared as definitions over
//! them, and [`DeclaredIndexedData::at`] applies them to actual arguments.
//!
//! Universe levels. The carrier's `CaseTwo` motive needs one level for all
//! cases, so the payloads of a family with several cases must share a level.
//! A payload below that level is raised explicitly, never by resizing a
//! universe: when the cases' payload types are above `Type0`, `declare`
//! declares one shared product of every case's complete payload type (the
//! `payload_lifts` constant) and each case's payload is paired with a
//! canonical element of `Box((x : P) -> squash(Id P x x))` for that product
//! `P`. That box lives exactly at `P`'s level and is inhabited even when `P`
//! is empty, so the padding assumes no case is inhabited, and the second
//! projection recovers the payload. A single case, or payloads at `Type0`,
//! need no padding.
//!
//! The scheme's five declarations must sit at positions 0..5 of the caller's
//! declarations (`indexed_scheme`), because [`IndexedFamily`] names them by
//! those positions.

use crate::conversion::{Budget, DEFAULT_CONVERSION_STEPS};
use crate::indexed::IndexedFamily;
use crate::signature::{Declaration, Signature};
use crate::substitution::{shift, substitute, universe_parameter_arity};
use crate::term::{Level, Sort, Term, TermArena, TermHandle};
use crate::typing::{Context, CoreError, infer_type};

/// A named field, payload field or parameter and its closed domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedField {
    pub name: u32,
    pub domain: TermHandle,
}

/// One constructor case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedCase {
    /// The fields only this case stores, in order.
    pub payload: Vec<IndexedField>,
    /// A closed function from the parameters, the shared non-parameter
    /// fields and the payload to the type of the case's constraint proof.
    pub constraints: TermHandle,
    /// One closed function per recursive field, from the same arguments to
    /// the index that child must have.
    pub child_indices: Vec<TermHandle>,
}

/// A value-indexed data declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedData {
    /// Every type parameter, with its sort. Domains of the other fields are
    /// functions of all of them.
    pub parameters: Vec<IndexedField>,
    /// The fields every label stores, in order. A field named like a
    /// parameter stores that parameter.
    pub fields: Vec<IndexedField>,
    /// The names of the fields forming the index, in index order.
    pub indices: Vec<u32>,
    pub cases: Vec<IndexedCase>,
    /// The universe parameters the domains and constraints may use.
    pub level_arity: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexedDataError {
    Kernel(CoreError),
    Malformed(&'static str),
}

impl From<CoreError> for IndexedDataError {
    fn from(error: CoreError) -> Self {
        Self::Kernel(error)
    }
}

impl std::fmt::Display for IndexedDataError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "{self:?}")
    }
}

impl std::error::Error for IndexedDataError {}

/// A family whose five functions are declarations of the caller's signature.
#[derive(Clone, Debug)]
pub struct DeclaredIndexedData {
    pub data: IndexedData,
    /// The five declared constants. With uniform parameters each is a
    /// function of them; [`DeclaredIndexedData::at`] applies it.
    pub family: IndexedFamily,
    /// The one level shared by every case's payload.
    pub payload_level: Level,
    /// The shared product padding raises each payload by (see the module
    /// comment); empty when no padding is needed.
    pub payload_lifts: Vec<TermHandle>,
}

/// The family at actual uniform parameters.
#[derive(Clone, Debug)]
pub struct IndexedInstance<'a> {
    pub declared: &'a DeclaredIndexedData,
    /// One actual per uniform parameter, in parameter order.
    pub uniform: Vec<TermHandle>,
    pub family: IndexedFamily,
}

/// The values of one constructor application.
#[derive(Clone, Copy, Debug)]
pub struct ConstructorArguments<'a> {
    pub case: usize,
    /// One value per shared field.
    pub fields: &'a [TermHandle],
    /// One value per payload field of the case.
    pub payload: &'a [TermHandle],
    /// A proof of the case's constraints at these values.
    pub constraints: TermHandle,
    /// One indexed tree per child index, in order.
    pub children: &'a [TermHandle],
}

/// A constructor's value of type `indexed_w(index)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedValue {
    pub label: TermHandle,
    /// `out(label)`, the index the value has.
    pub index: TermHandle,
    pub value: TermHandle,
}

/// An induction over an invariant of the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedInduction {
    /// `\i. \t. ind(...)`, of type `theorem_type`.
    pub theorem: TermHandle,
    /// `(i : index) -> indexed_w(i) -> invariant(i)`.
    pub theorem_type: TermHandle,
    /// The step's body, also declared as a definition the theorem names.
    pub step: TermHandle,
    pub step_type: TermHandle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Named(u32),
    Internal(usize),
}

/// Terms with named binders. `term` resolves each name to its de Bruijn
/// index, so the builders below can say which field they mean.
#[derive(Clone)]
enum S {
    Closed(TermHandle),
    /// A domain function of all parameters, applied to their actuals.
    ParameterDomain(TermHandle, Vec<S>),
    Var(Key),
    Type,
    Universe(Level),
    Squashed(Box<S>),
    SquashValue(Box<S>, Box<S>),
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
    ) -> Result<TermHandle, IndexedDataError> {
        let node = match self {
            Self::Closed(t) => return Ok(*t),
            Self::ParameterDomain(function, parameters) => {
                let mut result = *function;
                for parameter in parameters {
                    let actual = parameter.term(arena, scope)?;
                    let Term::Lambda { body, .. } = arena.get(result) else {
                        return Err(IndexedDataError::Malformed(
                            "a field domain is not a function of every parameter",
                        ));
                    };
                    result = substitute(arena, body, actual);
                }
                return Ok(result);
            }
            Self::Var(k) => Term::Variable(
                scope
                    .iter()
                    .rposition(|v| v == k)
                    .map(|p| (scope.len() - 1 - p) as u32)
                    .ok_or(IndexedDataError::Malformed(
                        "a field is used outside the binders that bind it",
                    ))?,
            ),
            Self::Type => Term::Sort(Sort::Type(Level::Constant(0))),
            Self::Universe(level) => Term::Sort(Sort::Type(level.clone())),
            Self::Squashed(ty) => Term::Squash {
                ty: ty.term(arena, scope)?,
            },
            Self::SquashValue(ty, value) => Term::SquashIntro {
                ty: ty.term(arena, scope)?,
                value: value.term(arena, scope)?,
            },
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

    fn closed(&self, arena: &mut TermArena) -> Result<TermHandle, IndexedDataError> {
        self.term(arena, &mut Vec::new())
    }
}

fn context(declarations: &[Declaration], level_arity: u32) -> Context {
    Context::with_level_arity(level_arity)
        .with_signature(Signature::from_declarations(declarations.to_vec()))
}

/// The level of the universe a type-valued function returns.
fn result_sort_level(
    arena: &mut TermArena,
    context: &Context,
    function: TermHandle,
) -> Result<Level, IndexedDataError> {
    let mut ty = infer_type(
        arena,
        context,
        function,
        &mut Budget::new(DEFAULT_CONVERSION_STEPS),
    )?;
    while let Term::Pi { codomain, .. } = arena.get(ty) {
        ty = codomain;
    }
    match arena.get(ty) {
        Term::Sort(sort) => Ok(sort.level()),
        _ => Err(IndexedDataError::Malformed(
            "a field domain or constraint does not return a universe",
        )),
    }
}

/// A definition at the given level arity, named by a constant over all its
/// universe parameters.
fn retain(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    level_arity: u32,
    ty: TermHandle,
    body: TermHandle,
) -> TermHandle {
    let declaration = declarations.len() as u32;
    declarations.push(Declaration::definition(level_arity, ty, body));
    arena.insert(Term::Constant {
        declaration,
        levels: (0..level_arity).map(Level::Parameter).collect(),
    })
}

/// A definition at the universe arity its own terms need.
fn retain_closed(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    ty: TermHandle,
    body: TermHandle,
) -> TermHandle {
    let arity = universe_parameter_arity(arena, &[ty, body]);
    retain(arena, declarations, arity, ty, body)
}

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

/// Pair a payload with the padding witnesses for `domains` (see the module
/// comment). Dropping these first components recovers the exact payload.
fn lift_payload(
    arena: &mut TermArena,
    domains: &[TermHandle],
    mut payload: TermHandle,
) -> Result<TermHandle, IndexedDataError> {
    let mut terms = Terms::default();
    for &domain in domains.iter().rev() {
        let (_, witness) = terms.level_unit(S::Closed(domain));
        let first = witness.closed(arena)?;
        payload = arena.insert(Term::Pair {
            first,
            second: payload,
        });
    }
    Ok(payload)
}

#[derive(Clone, Copy)]
enum ResultKind {
    Positions,
    Next,
}

/// Fresh binder names and the finite types positions are built from.
#[derive(Default)]
struct Terms {
    next: usize,
}

impl Terms {
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
    fn unit_value(&mut self) -> S {
        let domain = self.unit_strict();
        let value = self.fresh();
        S::BoxIntro(
            Box::new(domain),
            Box::new(lam(value, S::Empty, S::Var(value))),
        )
    }
    fn type_motive(&mut self) -> S {
        self.type_motive_at(Level::Constant(0))
    }
    fn type_motive_at(&mut self, level: Level) -> S {
        let k = self.fresh();
        lam(k, S::Two, S::Universe(level))
    }
    /// `Box((x : A) -> squash(Id A x x))` and its canonical element.
    fn level_unit(&mut self, carrier: S) -> (S, S) {
        // This boxed strict function is inhabited for every A : Type u,
        // including empty A, and lives at exactly Type u. Pairing it with a
        // lower payload is an explicit lift; projecting the payload is its
        // inverse up to strict proof irrelevance. No universe is resized.
        let x = self.fresh();
        let identity = S::Identity(
            Box::new(carrier.clone()),
            Box::new(S::Var(x)),
            Box::new(S::Var(x)),
        );
        let proposition = pi(x, carrier.clone(), S::Squashed(Box::new(identity.clone())));
        let reflexivity = S::Reflexivity(Box::new(carrier.clone()), Box::new(S::Var(x)));
        let proof = lam(
            x,
            carrier,
            S::SquashValue(Box::new(identity), Box::new(reflexivity)),
        );
        (
            S::Boxed(Box::new(proposition.clone())),
            S::BoxIntro(Box::new(proposition), Box::new(proof)),
        )
    }
    /// A type with exactly `count` child positions.
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
    /// The value at `position` among `values`, of type
    /// `result_family(position)`.
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

/// Builds the family's terms from its description. `actuals` replaces
/// uniform parameters by closed terms; without an actual a parameter stays a
/// named variable.
struct Builder<'a> {
    terms: Terms,
    data: &'a IndexedData,
    actuals: &'a [(u32, TermHandle)],
    payload_level: Level,
    payload_lifts: &'a [TermHandle],
}

impl<'a> Builder<'a> {
    fn new(
        data: &'a IndexedData,
        actuals: &'a [(u32, TermHandle)],
        payload_level: Level,
        payload_lifts: &'a [TermHandle],
    ) -> Self {
        Self {
            terms: Terms::default(),
            data,
            actuals,
            payload_level,
            payload_lifts,
        }
    }
    fn is_parameter(&self, name: u32) -> bool {
        self.data
            .parameters
            .iter()
            .any(|parameter| parameter.name == name)
    }
    /// The shared fields that are not stored parameters.
    fn value_fields(&self) -> impl Iterator<Item = &'a IndexedField> + '_ {
        self.data
            .fields
            .iter()
            .filter(|field| !self.is_parameter(field.name))
    }
    fn type_actual(&self, name: u32) -> S {
        self.actuals
            .iter()
            .find(|(parameter, _)| *parameter == name)
            .map(|(_, actual)| S::Closed(*actual))
            .unwrap_or(S::Var(Key::Named(name)))
    }
    fn domain(&self, field: &IndexedField) -> S {
        if self.data.parameters.is_empty() || self.is_parameter(field.name) {
            S::Closed(field.domain)
        } else {
            S::ParameterDomain(
                field.domain,
                self.data
                    .parameters
                    .iter()
                    .map(|parameter| self.type_actual(parameter.name))
                    .collect(),
            )
        }
    }
    fn field(&self, name: u32) -> Result<&'a IndexedField, IndexedDataError> {
        self.data
            .fields
            .iter()
            .find(|field| field.name == name)
            .ok_or(IndexedDataError::Malformed(
                "an index names no shared field",
            ))
    }
    fn index_type(&mut self) -> Result<S, IndexedDataError> {
        let mut fields = Vec::with_capacity(self.data.indices.len());
        for &name in &self.data.indices {
            fields.push(*self.field(name)?);
        }
        let Some(last) = fields.pop() else {
            return Ok(self.terms.unit());
        };
        let mut body = self.domain(&last);
        for field in fields.iter().rev() {
            body = sigma(Key::Named(field.name), self.domain(field), body);
        }
        Ok(body)
    }
    /// `function` applied to the parameters and the shared value fields.
    fn common_application(&self, mut function: S) -> S {
        for parameter in &self.data.parameters {
            function = app(function, self.type_actual(parameter.name));
        }
        for field in self.value_fields() {
            function = app(function, S::Var(Key::Named(field.name)));
        }
        function
    }
    fn constraint_application(&self, case: usize) -> S {
        let mut term = self.common_application(S::Closed(self.data.cases[case].constraints));
        for field in &self.data.cases[case].payload {
            term = app(term, S::Var(Key::Named(field.name)));
        }
        term
    }
    fn unlift_payload(&self, mut payload: S) -> S {
        for _ in 0..self.payload_lifts.len() {
            payload = snd(payload);
        }
        payload
    }
    fn raw_payload_type(&self, case: usize) -> S {
        let mut body = self.constraint_application(case);
        for field in self.data.cases[case].payload.iter().rev() {
            body = sigma(Key::Named(field.name), self.domain(field), body);
        }
        body
    }
    fn payload_type(&mut self, case: usize) -> S {
        let mut body = self.raw_payload_type(case);
        for &function in self.payload_lifts.iter().rev() {
            let carrier = self.common_application(S::Closed(function));
            let (unit, _) = self.terms.level_unit(carrier);
            let binder = self.terms.fresh();
            body = sigma(binder, unit, body);
        }
        body
    }
    /// Close `body` over the parameters and the shared value fields.
    fn close_common(&self, mut body: S) -> S {
        let fields = self.value_fields().collect::<Vec<_>>();
        for field in fields.into_iter().rev() {
            body = lam(Key::Named(field.name), self.domain(field), body);
        }
        for parameter in self.data.parameters.iter().rev() {
            body = lam(
                Key::Named(parameter.name),
                S::Closed(parameter.domain),
                body,
            );
        }
        body
    }
    /// Each case's complete payload type as a closed function.
    fn payload_type_functions(
        &self,
        arena: &mut TermArena,
    ) -> Result<Vec<TermHandle>, IndexedDataError> {
        (0..self.data.cases.len())
            .map(|case| self.close_common(self.raw_payload_type(case)).closed(arena))
            .collect()
    }
    /// The product of every case's complete payload type, as a closed
    /// function. Its level is exactly their maximum.
    fn shared_payload_type_function(
        &mut self,
        arena: &mut TermArena,
    ) -> Result<TermHandle, IndexedDataError> {
        let Some(last) = self.data.cases.len().checked_sub(1) else {
            return Err(IndexedDataError::Malformed("the declaration has no cases"));
        };
        let mut body = self.raw_payload_type(last);
        for case in (0..last).rev() {
            let binder = self.terms.fresh();
            body = sigma(binder, self.raw_payload_type(case), body);
        }
        self.close_common(body).closed(arena)
    }
    fn sum_type(&mut self, start: usize) -> S {
        if start == self.data.cases.len() {
            return self.terms.empty();
        }
        if start + 1 == self.data.cases.len() {
            return self.payload_type(start);
        }
        let tag = self.terms.fresh();
        let motive = self.terms.type_motive_at(self.payload_level.clone());
        let zero = self.payload_type(start);
        let one = self.sum_type(start + 1);
        sigma(tag, S::Two, choose(motive, zero, one, S::Var(tag)))
    }
    fn carrier(&mut self) -> S {
        let mut body = self.sum_type(0);
        for field in self.data.fields.iter().rev() {
            body = sigma(Key::Named(field.name), self.domain(field), body);
        }
        body
    }
    fn projected(mut value: S, ordinal: usize) -> S {
        for _ in 0..ordinal {
            value = snd(value);
        }
        fst(value)
    }
    /// Child `child`'s index at the given payload values.
    fn child_index(&self, case: usize, child: usize, payload: &[S]) -> S {
        let mut term =
            self.common_application(S::Closed(self.data.cases[case].child_indices[child]));
        for value in payload {
            term = app(term, value.clone());
        }
        term
    }
    fn payload_projections(&self, case: usize, payload: &S) -> Vec<S> {
        (0..self.data.cases[case].payload.len())
            .map(|ordinal| Self::projected(payload.clone(), ordinal))
            .collect()
    }
    fn case_body(
        &mut self,
        case: usize,
        payload: S,
        kind: ResultKind,
    ) -> Result<S, IndexedDataError> {
        let payload = self.unlift_payload(payload);
        let count = self.data.cases[case].child_indices.len();
        let positions = self.terms.positions(count);
        if matches!(kind, ResultKind::Positions) {
            return Ok(positions);
        }
        let index = self.index_type()?;
        let p = self.terms.fresh();
        let projections = self.payload_projections(case, &payload);
        let values = (0..count)
            .map(|child| self.child_index(case, child, &projections))
            .collect::<Vec<_>>();
        let at = self.terms.fresh();
        let result_family = lam(at, positions.clone(), index);
        let body = self.terms.select(&values, S::Var(p), result_family);
        Ok(lam(p, positions, body))
    }
    fn result_type(
        &mut self,
        start: usize,
        value: S,
        kind: ResultKind,
    ) -> Result<S, IndexedDataError> {
        if matches!(kind, ResultKind::Positions) {
            return Ok(S::Type);
        }
        let positions = self.eliminate(start, value, ResultKind::Positions)?;
        let p = self.terms.fresh();
        Ok(pi(p, positions, self.index_type()?))
    }
    /// Case analysis on the sum of cases from `start`, giving the positions
    /// type or the `next` function of the case `value` holds.
    fn eliminate(
        &mut self,
        start: usize,
        value: S,
        kind: ResultKind,
    ) -> Result<S, IndexedDataError> {
        if start == self.data.cases.len() {
            return Err(IndexedDataError::Malformed("the declaration has no cases"));
        }
        if start + 1 == self.data.cases.len() {
            return self.case_body(start, value, kind);
        }
        let tag = self.terms.fresh();
        let payload = self.terms.fresh();
        let tm = self.terms.type_motive_at(self.payload_level.clone());
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
    /// Open a label's shared fields as named binders, then eliminate its sum.
    fn unpack(&mut self, label: S, kind: ResultKind) -> Result<S, IndexedDataError> {
        let tail = self.terms.fresh();
        let sum_type = self.sum_type(0);
        let body = self.eliminate(0, S::Var(tail), kind)?;
        let mut function = lam(tail, sum_type, body);
        for field in self.data.fields.iter().rev() {
            function = lam(Key::Named(field.name), self.domain(field), function);
        }
        let mut tail_value = label;
        for _ in &self.data.fields {
            function = app(function, fst(tail_value.clone()));
            tail_value = snd(tail_value);
        }
        Ok(app(function, tail_value))
    }
    /// The index tuple built from one value per index field.
    fn index_value(&mut self, mut values: Vec<S>) -> S {
        let Some(mut body) = values.pop() else {
            return self.terms.unit_value();
        };
        for value in values.into_iter().rev() {
            body = pair(value, body);
        }
        body
    }
    fn index_positions(&self) -> Result<Vec<usize>, IndexedDataError> {
        self.data
            .indices
            .iter()
            .map(|&name| {
                self.data
                    .fields
                    .iter()
                    .position(|field| field.name == name)
                    .ok_or(IndexedDataError::Malformed(
                        "an index names no shared field",
                    ))
            })
            .collect()
    }
    fn out(&mut self, label: S) -> Result<S, IndexedDataError> {
        let values = self
            .index_positions()?
            .into_iter()
            .map(|ordinal| Self::projected(label.clone(), ordinal))
            .collect();
        Ok(self.index_value(values))
    }
}

/// The five functions, still open over the uniform parameters, and the
/// payload level and padding they use.
fn describe(
    data: &IndexedData,
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    uniform: &[u32],
) -> Result<(IndexedFamily, Level, Vec<TermHandle>), IndexedDataError> {
    let mut b = Builder::new(data, &[], Level::Constant(0), &[]);
    // Close every entire case type over the shared telescope. A constraint
    // can live above all case-local fields, so field domains alone cannot
    // supply the required lift. No inhabitant of another case is assumed.
    let functions = b.payload_type_functions(arena)?;
    let checking = context(declarations, data.level_arity);
    let mut payload_level = Level::Constant(0);
    for &function in &functions {
        payload_level = payload_level.maximum(result_sort_level(arena, &checking, function)?);
    }
    let payload_lifts = if data.cases.len() > 1 && payload_level != Level::Constant(0) {
        // One shared product has the maximum of all complete case sorts.
        // Its padding needs no case inhabitant and adds one component per
        // case, rather than one per pair of cases.
        let body = b.shared_payload_type_function(arena)?;
        let ty = infer_type(
            arena,
            &checking,
            body,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )?;
        vec![retain(arena, declarations, data.level_arity, ty, body)]
    } else {
        Vec::new()
    };
    b.payload_level = payload_level.clone();
    b.payload_lifts = &payload_lifts;
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
    let checking = context(declarations, data.level_arity);
    let mut index_level = Level::Constant(0);
    let mut carrier_level = payload_level.clone();
    for field in &data.fields {
        let level = result_sort_level(arena, &checking, field.domain)?;
        carrier_level = carrier_level.maximum(level.clone());
        if data.indices.contains(&field.name) {
            index_level = index_level.maximum(level);
        }
    }
    let mut scope = uniform.iter().copied().map(Key::Named).collect::<Vec<_>>();
    let family = IndexedFamily {
        levels: [index_level, carrier_level, Level::Constant(0)],
        index: index.term(arena, &mut scope)?,
        carrier: carrier.term(arena, &mut scope)?,
        children: children.term(arena, &mut scope)?,
        out: out.term(arena, &mut scope)?,
        next: next.term(arena, &mut scope)?,
    };
    Ok((family, payload_level, payload_lifts))
}

/// Declare the five functions as definitions over the uniform parameters,
/// so later terms name constants rather than repeat the bodies.
fn intern(
    arena: &mut TermArena,
    declarations: &mut Vec<Declaration>,
    mut family: IndexedFamily,
    parameter_domains: &[TermHandle],
    level_arity: u32,
) -> IndexedFamily {
    let mut declare = |arena: &mut TermArena, mut ty: TermHandle, mut body: TermHandle| {
        for &domain in parameter_domains.iter().rev() {
            body = arena.insert(Term::Lambda { domain, body });
            ty = arena.insert(Term::Pi {
                domain,
                codomain: ty,
            });
        }
        let constant = retain(arena, declarations, level_arity, ty, body);
        let count = parameter_domains.len();
        let mut applied = constant;
        for ordinal in 0..count {
            let argument = arena.insert(Term::Variable((count - ordinal - 1) as u32));
            applied = apply(arena, applied, argument);
        }
        (constant, applied)
    };
    let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let index_sort = arena.insert(Term::Sort(Sort::Type(family.levels[0].clone())));
    let carrier_sort = arena.insert(Term::Sort(Sort::Type(family.levels[1].clone())));
    let (index, applied_index) = declare(arena, index_sort, family.index);
    let (carrier, applied_carrier) = declare(arena, carrier_sort, family.carrier);
    let ty = arena.insert(Term::Pi {
        domain: applied_carrier,
        codomain: sort,
    });
    let (children, applied_children) = declare(arena, ty, family.children);
    let index_under_label = shift(arena, applied_index, 0, 1);
    let ty = arena.insert(Term::Pi {
        domain: applied_carrier,
        codomain: index_under_label,
    });
    let (out, _) = declare(arena, ty, family.out);
    let a = arena.insert(Term::Variable(0));
    let children_under_label = shift(arena, applied_children, 0, 1);
    let domain = apply(arena, children_under_label, a);
    let index_under_position = shift(arena, applied_index, 0, 2);
    let codomain = arena.insert(Term::Pi {
        domain,
        codomain: index_under_position,
    });
    let ty = arena.insert(Term::Pi {
        domain: applied_carrier,
        codomain,
    });
    let (next, _) = declare(arena, ty, family.next);
    family.index = index;
    family.carrier = carrier;
    family.children = children;
    family.out = out;
    family.next = next;
    family
}

impl IndexedData {
    /// Declare the family's five functions (and the payload padding, when
    /// needed) in `declarations`. The caller's signature must start with
    /// `indexed_scheme`.
    pub fn declare(
        self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
    ) -> Result<DeclaredIndexedData, IndexedDataError> {
        let uniform = self
            .uniform_parameters()
            .map(|p| p.name)
            .collect::<Vec<_>>();
        let domains = self
            .uniform_parameters()
            .map(|p| p.domain)
            .collect::<Vec<_>>();
        let (family, payload_level, payload_lifts) =
            describe(&self, arena, declarations, &uniform)?;
        let family = intern(arena, declarations, family, &domains, self.level_arity);
        Ok(DeclaredIndexedData {
            data: self,
            family,
            payload_level,
            payload_lifts,
        })
    }

    /// The parameters no shared field stores, in parameter order.
    fn uniform_parameters(&self) -> impl Iterator<Item = &IndexedField> {
        self.parameters
            .iter()
            .filter(|parameter| !self.fields.iter().any(|field| field.name == parameter.name))
    }
}

impl DeclaredIndexedData {
    /// The family at actual uniform parameters, one per parameter no shared
    /// field stores, in parameter order.
    pub fn at(
        &self,
        arena: &mut TermArena,
        uniform: &[TermHandle],
    ) -> Result<IndexedInstance<'_>, IndexedDataError> {
        if uniform.len() != self.data.uniform_parameters().count() {
            return Err(IndexedDataError::Malformed(
                "the family needs one actual per uniform parameter",
            ));
        }
        let template = &self.family;
        let family = IndexedFamily {
            levels: template.levels.clone(),
            index: arena.apply_arguments(template.index, uniform),
            carrier: arena.apply_arguments(template.carrier, uniform),
            children: arena.apply_arguments(template.children, uniform),
            out: arena.apply_arguments(template.out, uniform),
            next: arena.apply_arguments(template.next, uniform),
        };
        Ok(IndexedInstance {
            declared: self,
            uniform: uniform.to_vec(),
            family,
        })
    }
}

impl IndexedInstance<'_> {
    fn actuals(&self) -> Vec<(u32, TermHandle)> {
        self.declared
            .data
            .uniform_parameters()
            .map(|parameter| parameter.name)
            .zip(self.uniform.iter().copied())
            .collect()
    }

    fn builder<'b>(&'b self, actuals: &'b [(u32, TermHandle)]) -> Builder<'b> {
        Builder::new(
            &self.declared.data,
            actuals,
            self.declared.payload_level.clone(),
            &self.declared.payload_lifts,
        )
    }

    /// A constructor's label and value. The kernel's type check of `value`
    /// against `indexed_w(index)` is the caller's: it is what rejects wrong
    /// constraint proofs and children at the wrong index.
    pub fn constructor(
        &self,
        arena: &mut TermArena,
        arguments: ConstructorArguments<'_>,
    ) -> Result<IndexedValue, IndexedDataError> {
        let data = &self.declared.data;
        let case = data
            .cases
            .get(arguments.case)
            .ok_or(IndexedDataError::Malformed("the constructor names no case"))?;
        if arguments.fields.len() != data.fields.len()
            || arguments.payload.len() != case.payload.len()
            || arguments.children.len() != case.child_indices.len()
        {
            return Err(IndexedDataError::Malformed(
                "the constructor's values do not match its case",
            ));
        }
        let mut label = arguments.constraints;
        for &value in arguments.payload.iter().rev() {
            label = arena.insert(Term::Pair {
                first: value,
                second: label,
            });
        }
        if !self.declared.payload_lifts.is_empty() {
            // The padding's domain is the shared product at this label's
            // parameters and shared value fields.
            let mut common = Vec::with_capacity(data.parameters.len() + data.fields.len());
            let mut uniform = self.uniform.iter().copied();
            for parameter in &data.parameters {
                let stored = data
                    .fields
                    .iter()
                    .position(|field| field.name == parameter.name);
                let actual = match stored {
                    Some(ordinal) => arguments.fields[ordinal],
                    None => uniform.next().ok_or(IndexedDataError::Malformed(
                        "the family needs one actual per uniform parameter",
                    ))?,
                };
                common.push(actual);
            }
            for (field, &value) in data.fields.iter().zip(arguments.fields) {
                if !data.parameters.iter().any(|p| p.name == field.name) {
                    common.push(value);
                }
            }
            let domains = self
                .declared
                .payload_lifts
                .iter()
                .map(|&function| arena.apply_arguments(function, &common))
                .collect::<Vec<_>>();
            label = lift_payload(arena, &domains, label)?;
        }
        if arguments.case + 1 < data.cases.len() {
            let first = arena.insert(Term::TwoZero);
            label = arena.insert(Term::Pair {
                first,
                second: label,
            });
        }
        for _ in 0..arguments.case {
            let first = arena.insert(Term::TwoOne);
            label = arena.insert(Term::Pair {
                first,
                second: label,
            });
        }
        for &value in arguments.fields.iter().rev() {
            label = arena.insert(Term::Pair {
                first: value,
                second: label,
            });
        }
        let children = constructor_children(arena, &self.family, label, arguments.children)?;
        let value = self.family.sup(arena, label, children);
        let index = apply(arena, self.family.out, label);
        Ok(IndexedValue {
            label,
            index,
            value,
        })
    }

    /// Declare `invariant : index -> Type(result_level)` as a definition and
    /// return its constant. Lemmas and the induction name it.
    pub fn declare_invariant(
        &self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        invariant: TermHandle,
        result_level: Level,
    ) -> TermHandle {
        let sort = arena.insert(Term::Sort(Sort::Type(result_level)));
        let ty = arena.insert(Term::Pi {
            domain: self.family.index,
            codomain: sort,
        });
        retain_closed(arena, declarations, ty, invariant)
    }

    /// The type the induction needs of case `case`'s lemma:
    /// `(fields) -> (payload) -> constraints -> invariant(child index)... ->
    /// invariant(index)`, one premise per child.
    pub fn lemma_type(
        &self,
        arena: &mut TermArena,
        invariant: TermHandle,
        case: usize,
    ) -> Result<TermHandle, IndexedDataError> {
        let actuals = self.actuals();
        let mut b = self.builder(&actuals);
        let data = &self.declared.data;
        let shape = data
            .cases
            .get(case)
            .ok_or(IndexedDataError::Malformed("the lemma names no case"))?;
        let payload = shape
            .payload
            .iter()
            .map(|field| S::Var(Key::Named(field.name)))
            .collect::<Vec<_>>();
        let indices = b
            .index_positions()?
            .into_iter()
            .map(|ordinal| S::Var(Key::Named(data.fields[ordinal].name)))
            .collect();
        let goal = b.index_value(indices);
        let mut result = app(S::Closed(invariant), goal);
        for child in (0..shape.child_indices.len()).rev() {
            let premise = app(S::Closed(invariant), b.child_index(case, child, &payload));
            let binder = b.terms.fresh();
            result = pi(binder, premise, result);
        }
        let binder = b.terms.fresh();
        result = pi(binder, b.constraint_application(case), result);
        for field in shape.payload.iter().rev() {
            result = pi(Key::Named(field.name), b.domain(field), result);
        }
        for field in data.fields.iter().rev() {
            result = pi(Key::Named(field.name), b.domain(field), result);
        }
        result.closed(arena)
    }

    /// Induction over `invariant` (a constant from `declare_invariant`),
    /// given one lemma per case of the type `lemma_type` states. The uniform
    /// actuals must be closed: the step and its helpers are declarations.
    pub fn induction(
        &self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        invariant: TermHandle,
        lemmas: &[TermHandle],
    ) -> Result<IndexedInduction, IndexedDataError> {
        let data = &self.declared.data;
        let family = &self.family;
        if data.cases.len() != lemmas.len() {
            return Err(IndexedDataError::Malformed(
                "the induction needs one lemma per case",
            ));
        }
        let level_arity =
            universe_parameter_arity(arena, &[family.index, family.carrier, invariant]);
        let result_level =
            result_sort_level(arena, &context(declarations, level_arity), invariant)?;
        let actuals = self.actuals();
        let w_sort = arena.insert(Term::Sort(Sort::Type(
            family.levels[0]
                .clone()
                .maximum(family.levels[1].clone())
                .maximum(family.levels[2].clone()),
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
        let iw = retain_closed(arena, declarations, function_type, iw_function);
        let iw_body = apply(arena, iw, i);

        // `label_pack(fields..., sum)` rebuilds a label from its parts; the
        // step eliminates the sum under it and transports back.
        let mut builder = self.builder(&actuals);
        let tail = builder.terms.fresh();
        let sum_type = builder.sum_type(0);
        let mut raw_label = S::Var(tail);
        for field in data.fields.iter().rev() {
            raw_label = pair(S::Var(Key::Named(field.name)), raw_label);
        }
        let mut pack = lam(tail, sum_type.clone(), raw_label);
        let mut pack_type = pi(tail, sum_type, S::Closed(family.carrier));
        for field in data.fields.iter().rev() {
            pack = lam(Key::Named(field.name), builder.domain(field), pack);
            pack_type = pi(Key::Named(field.name), builder.domain(field), pack_type);
        }
        let pack = pack.closed(arena)?;
        let pack_type = pack_type.closed(arena)?;
        let label_pack = retain_closed(arena, declarations, pack_type, pack);

        // `logical_result(a) = (hypotheses at a's children) -> invariant(out a)`.
        let a = arena.insert(Term::Variable(0));
        let positions = apply(arena, family.children, a);
        let next = apply(arena, family.next, a);
        let position = arena.insert(Term::Variable(0));
        let next = shift(arena, next, 0, 1);
        let next = apply(arena, next, position);
        let hypothesis = apply(arena, invariant, next);
        let hypotheses = arena.insert(Term::Pi {
            domain: positions,
            codomain: hypothesis,
        });
        let out = apply(arena, family.out, a);
        let goal = apply(arena, invariant, out);
        let goal = shift(arena, goal, 0, 1);
        let result = arena.insert(Term::Pi {
            domain: hypotheses,
            codomain: goal,
        });
        let result = arena.insert(Term::Lambda {
            domain: family.carrier,
            body: result,
        });
        let proposition_sort = arena.insert(Term::Sort(Sort::Type(result_level.clone())));
        let result_type = arena.insert(Term::Pi {
            domain: family.carrier,
            codomain: proposition_sort,
        });
        let logical_result = retain_closed(arena, declarations, result_type, result);

        // `reconstruction(a) : Id carrier (label_pack(a's parts)) a`, by refl.
        let mut producer = InductionBuilder {
            b: self.builder(&actuals),
            family,
            iw,
            label_pack,
            logical_result,
            reconstruction_equality: None,
            invariant,
            lemmas,
        };
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
        .closed(arena)?;
        let equality_body = lam(
            label,
            S::Closed(family.carrier),
            S::Reflexivity(Box::new(S::Closed(family.carrier)), Box::new(reconstructed)),
        )
        .closed(arena)?;
        producer.reconstruction_equality = Some(retain_closed(
            arena,
            declarations,
            equality_type,
            equality_body,
        ));
        let step_body = producer.step()?.closed(arena)?;
        let a = producer.b.terms.fresh();
        let result_type = producer.type_at_label(S::Var(a));
        let step_type = pi(a, S::Closed(family.carrier), result_type).closed(arena)?;
        let step = retain_closed(arena, declarations, step_type, step_body);

        let i = arena.insert(Term::Variable(1));
        let proposition = apply(arena, invariant, i);
        let motive_body = arena.insert(Term::Lambda {
            domain: iw_body,
            body: proposition,
        });
        let motive = arena.insert(Term::Lambda {
            domain: family.index,
            body: motive_body,
        });
        let tree = arena.insert(Term::Variable(0));
        let body = family.ind(arena, result_level, motive, step, i, tree);
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
        let theorem_type = arena.insert(Term::Pi {
            domain: family.index,
            codomain: result,
        });
        Ok(IndexedInduction {
            theorem,
            theorem_type,
            step: step_body,
            step_type,
        })
    }

    /// The theorem `\i. \t. proof` for an invariant that does not depend on
    /// the index, where `proof` is a closed proof of it. A closed
    /// specialization needs no induction step.
    pub fn constant_invariant_theorem(
        &self,
        arena: &mut TermArena,
        invariant: TermHandle,
        proof: TermHandle,
    ) -> (TermHandle, TermHandle) {
        let family = &self.family;
        let i = arena.insert(Term::Variable(0));
        let evidence_type = family.indexed_w(arena, i);
        let closed = shift(arena, proof, 0, 2);
        let body = arena.insert(Term::Lambda {
            domain: evidence_type,
            body: closed,
        });
        let theorem = arena.insert(Term::Lambda {
            domain: family.index,
            body,
        });
        let i = arena.insert(Term::Variable(1));
        let proposition = apply(arena, invariant, i);
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
}

/// The child function `\position. children[position]` of a constructor.
fn constructor_children(
    arena: &mut TermArena,
    family: &IndexedFamily,
    label: TermHandle,
    children: &[TermHandle],
) -> Result<TermHandle, IndexedDataError> {
    let child_domain = apply(arena, family.children, label);
    let position = arena.insert(Term::Variable(0));
    let next = apply(arena, family.next, label);
    let child_index = apply(arena, next, position);
    let child_type = family.indexed_w(arena, child_index);
    let result_family = arena.insert(Term::Lambda {
        domain: child_domain,
        body: child_type,
    });
    let mut terms = Terms::default();
    let position = terms.fresh();
    let values = children.iter().copied().map(S::Closed).collect::<Vec<_>>();
    let body = terms.select(&values, S::Var(position), S::Closed(result_family));
    lam(position, S::Closed(child_domain), body).closed(arena)
}

struct InductionBuilder<'a, 'b> {
    b: Builder<'a>,
    family: &'b IndexedFamily,
    iw: TermHandle,
    label_pack: TermHandle,
    logical_result: TermHandle,
    reconstruction_equality: Option<TermHandle>,
    invariant: TermHandle,
    lemmas: &'b [TermHandle],
}

impl InductionBuilder<'_, '_> {
    fn reconstructed_label(&mut self, label: S) -> S {
        let mut function = S::Closed(self.label_pack);
        let mut tail = label;
        for _ in &self.b.data.fields {
            function = app(function, fst(tail.clone()));
            tail = snd(tail);
        }
        if self.b.data.cases.len() > 1 {
            tail = pair(fst(tail.clone()), snd(tail));
        }
        app(function, tail)
    }
    fn label(&mut self, start: usize, mut value: S) -> S {
        for _ in 0..start {
            value = pair(S::One, value);
        }
        let mut function = S::Closed(self.label_pack);
        for field in &self.b.data.fields {
            function = app(function, S::Var(Key::Named(field.name)));
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
    /// The lemma of `case` applied to the label's fields, payload,
    /// constraint proof and one hypothesis per child.
    fn case_body(&mut self, case: usize, payload: S) -> S {
        let sum_value = if case + 1 == self.b.data.cases.len() {
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
        for field in &self.b.data.fields {
            body = app(body, S::Var(Key::Named(field.name)));
        }
        let mut predicate = self.b.unlift_payload(payload);
        for _ in &self.b.data.cases[case].payload {
            body = app(body, fst(predicate.clone()));
            predicate = snd(predicate);
        }
        body = app(body, predicate);
        let count = self.b.data.cases[case].child_indices.len();
        for child in 0..count {
            let position = self.position(count, child);
            body = app(body, app(S::Var(ih), position));
        }
        lam(ih, hypotheses, body)
    }
    fn eliminate(&mut self, start: usize, value: S) -> Result<S, IndexedDataError> {
        if start >= self.b.data.cases.len() {
            return Err(IndexedDataError::Malformed("the declaration has no cases"));
        }
        if start + 1 == self.b.data.cases.len() {
            return Ok(self.case_body(start, value));
        }
        let tag = self.b.terms.fresh();
        let payload = self.b.terms.fresh();
        let tm = self.b.terms.type_motive_at(self.b.payload_level.clone());
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
    /// `\a. \g. J(...)`: open `a`'s fields, eliminate its sum into the
    /// lemma of its case, and transport along the reconstruction equality
    /// from the rebuilt label back to `a`.
    fn step(&mut self) -> Result<S, IndexedDataError> {
        let reconstruction_equality =
            self.reconstruction_equality
                .ok_or(IndexedDataError::Malformed(
                    "the step needs the reconstruction equality",
                ))?;
        let a = self.b.terms.fresh();
        let tail = self.b.terms.fresh();
        let sum_type = self.b.sum_type(0);
        let body = self.eliminate(0, S::Var(tail))?;
        let mut function = lam(tail, sum_type, body);
        for field in self.b.data.fields.iter().rev() {
            function = lam(Key::Named(field.name), self.b.domain(field), function);
        }
        let mut value = S::Var(a);
        for _ in &self.b.data.fields {
            function = app(function, fst(value.clone()));
            value = snd(value);
        }
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
            Box::new(app(S::Closed(reconstruction_equality), S::Var(a))),
        );
        Ok(lam(
            a,
            S::Closed(self.family.carrier),
            lam(g, children, transported),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MathematicalCertificate, abstract_certificate_assumptions, check_signature, check_type,
        convertible, indexed_scheme, instantiate_levels, judgment_assumption_closure,
    };

    fn assume(
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        ty: TermHandle,
    ) -> TermHandle {
        let declaration = declarations.len() as u32;
        declarations.push(Declaration::assumption(0, ty));
        arena.insert(Term::Constant {
            declaration,
            levels: Vec::new(),
        })
    }

    fn signature_context(declarations: &[Declaration], level_arity: u32) -> Context {
        context(declarations, level_arity)
    }

    /// Check `term : ty` within the default conversion budget and print the
    /// steps the kernel spent.
    fn checked(
        arena: &mut TermArena,
        context: &Context,
        term: TermHandle,
        ty: TermHandle,
        role: &str,
    ) {
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let result = check_type(arena, context, term, ty, &mut budget);
        eprintln!(
            "{role}: {} kernel steps",
            DEFAULT_CONVERSION_STEPS - budget.remaining()
        );
        result.unwrap_or_else(|error| panic!("{role}: {error:?}"));
    }

    fn checked_signature(arena: &mut TermArena, declarations: &[Declaration], role: &str) {
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let result = check_signature(arena, declarations, &mut budget);
        eprintln!(
            "{role} signature: {} kernel steps",
            DEFAULT_CONVERSION_STEPS - budget.remaining()
        );
        result.unwrap_or_else(|error| panic!("{role} signature: {error:?}"));
    }

    #[test]
    fn explicit_payload_lift_preserves_its_value_at_the_required_universe() {
        let mut arena = TermArena::new();
        let higher = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let mut terms = Terms::default();
        let (unit, witness) = terms.level_unit(S::Closed(higher));
        let unit = unit.closed(&mut arena).unwrap();
        let witness = witness.closed(&mut arena).unwrap();
        let context = Context::with_level_arity(1);
        check_type(&mut arena, &context, witness, unit, &mut Budget::default()).unwrap();
        // Every boxed inhabitant contracts to the canonical witness. The
        // eliminator enters the strict proof binder, where irrelevance makes
        // its boxed value agree with the canonical proof; no Box eta axiom is
        // needed to prove this relevant identity.
        let Term::Box { ty: proposition } = arena.get(unit) else {
            unreachable!()
        };
        let boxed = arena.insert(Term::Variable(0));
        let conclusion = arena.insert(Term::Id {
            ty: unit,
            left: witness,
            right: boxed,
        });
        let motive = arena.insert(Term::Lambda {
            domain: unit,
            body: conclusion,
        });
        let proof = arena.insert(Term::Variable(0));
        let boxed_proof = arena.insert(Term::BoxIntro {
            ty: proposition,
            value: proof,
        });
        let reflexive = arena.insert(Term::Refl {
            ty: unit,
            value: boxed_proof,
        });
        let branch = arena.insert(Term::Lambda {
            domain: proposition,
            body: reflexive,
        });
        let contraction = arena.insert(Term::BoxElim {
            motive,
            body: branch,
            scrutinee: boxed,
        });
        check_type(
            &mut arena,
            &context.extend(unit),
            contraction,
            conclusion,
            &mut Budget::default(),
        )
        .unwrap();

        let unit_sort = arena.insert(Term::Sort(Sort::Type(
            Level::Parameter(0).successor().unwrap(),
        )));
        check_type(
            &mut arena,
            &context,
            unit,
            unit_sort,
            &mut Budget::default(),
        )
        .unwrap();
        assert!(check_type(&mut arena, &context, unit, higher, &mut Budget::default()).is_err());
        let lifted = lift_payload(&mut arena, &[higher], zero).unwrap();
        let expected = arena.insert(Term::Sigma {
            domain: unit,
            codomain: two,
        });
        check_type(
            &mut arena,
            &context,
            lifted,
            expected,
            &mut Budget::default(),
        )
        .unwrap();
        let recovered = arena.insert(Term::Snd { pair: lifted });
        assert!(
            convertible(
                &mut arena,
                &context,
                recovered,
                zero,
                two,
                &mut Budget::default()
            )
            .unwrap()
        );
        // No implicit resizing: this lifted value is not an ordinary Two.
        assert!(check_type(&mut arena, &context, lifted, two, &mut Budget::default()).is_err());
        assert!(
            check_type(
                &mut arena,
                &Context::empty(),
                witness,
                unit,
                &mut Budget::default()
            )
            .is_err()
        );
    }

    #[test]
    fn shared_high_universe_constraint_is_lifted_without_assuming_its_truth() {
        let mut arena = TermArena::new();
        let mut declarations = indexed_scheme(&mut arena);
        let (a_name, x_name, y_name) = (71, 72, 73);
        let a = Key::Named(a_name);
        let x = Key::Named(x_name);
        let y = Key::Named(y_name);
        let sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let field = lam(a, S::Closed(sort), S::Var(a))
            .closed(&mut arena)
            .unwrap();
        let mut terms = Terms::default();
        let unit = terms.unit().closed(&mut arena).unwrap();
        let unit_value = terms.unit_value().closed(&mut arena).unwrap();
        let high = S::Identity(
            Box::new(S::Var(a)),
            Box::new(S::Var(x)),
            Box::new(S::Var(y)),
        );
        let close = |body: S| {
            lam(
                a,
                S::Closed(sort),
                lam(x, S::Var(a), lam(y, S::Var(a), body)),
            )
        };
        let high = close(high).closed(&mut arena).unwrap();
        let low = close(S::Closed(unit)).closed(&mut arena).unwrap();
        let case = |constraints| IndexedCase {
            payload: Vec::new(),
            constraints,
            child_indices: Vec::new(),
        };
        let parameter = IndexedField {
            name: a_name,
            domain: sort,
        };
        let declared = IndexedData {
            parameters: vec![parameter],
            fields: vec![
                IndexedField {
                    name: x_name,
                    domain: field,
                },
                IndexedField {
                    name: y_name,
                    domain: field,
                },
            ],
            indices: Vec::new(),
            cases: vec![case(high), case(low)],
            level_arity: 1,
        }
        .declare(&mut arena, &mut declarations)
        .unwrap();
        assert_eq!(declared.payload_lifts.len(), 1);
        let signature = check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let one = arena.insert(Term::TwoOne);
        let carrier =
            instantiate_levels(&mut arena, declared.family.carrier, &[Level::Constant(0)]).unwrap();
        let carrier = apply(&mut arena, carrier, two);
        let mut lift_domains = Vec::new();
        for &function in &declared.payload_lifts {
            let domain = instantiate_levels(&mut arena, function, &[Level::Constant(0)]).unwrap();
            lift_domains.push(arena.apply_arguments(domain, &[two, zero, one]));
        }
        let lifted = lift_payload(&mut arena, &lift_domains, unit_value).unwrap();
        let mut label = arena.insert(Term::Pair {
            first: one,
            second: lifted,
        });
        label = arena.insert(Term::Pair {
            first: one,
            second: label,
        });
        label = arena.insert(Term::Pair {
            first: zero,
            second: label,
        });
        check_type(
            &mut arena,
            &Context::empty().with_signature(signature.clone()),
            label,
            carrier,
            &mut Budget::default(),
        )
        .unwrap();
        // The other case's Id Two zero one is uninhabited. Its lift needs
        // only the type, and cannot turn the low case into that proof.
        assert!(
            check_type(
                &mut arena,
                &Context::empty().with_signature(signature),
                unit_value,
                lift_domains[0],
                &mut Budget::default()
            )
            .is_err()
        );

        // An unused high-universe argument is outside the represented sum.
        let phantom = lam(a, S::Closed(sort), S::Closed(unit))
            .closed(&mut arena)
            .unwrap();
        let phantom = IndexedData {
            parameters: vec![parameter],
            fields: Vec::new(),
            indices: Vec::new(),
            cases: vec![case(phantom), case(phantom)],
            level_arity: 1,
        }
        .declare(&mut arena, &mut declarations)
        .unwrap();
        assert!(phantom.payload_lifts.is_empty());
        assert_eq!(
            phantom.family.levels,
            [Level::Constant(0), Level::Constant(0), Level::Constant(0)]
        );
    }

    /// The deleted elaborator closed each case lemma over its fields with
    /// its own `abstract_parameters`; `abstract_certificate_assumptions` is
    /// that operation in this crate.
    #[test]
    fn constructor_abstraction_binds_dependent_child_and_premise_domains() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let value = arena.insert(Term::Constant {
            declaration: 0,
            levels: Vec::new(),
        });
        let predicate = arena.insert(Term::Id {
            ty: two,
            left: value,
            right: value,
        });
        let evidence = arena.insert(Term::Constant {
            declaration: 1,
            levels: Vec::new(),
        });
        let declarations = vec![
            Declaration::assumption(0, two),
            Declaration::assumption(0, predicate),
        ];
        let abstracted = abstract_certificate_assumptions(
            &mut arena,
            &MathematicalCertificate {
                signature: declarations,
                level_arity: 0,
                context: Vec::new(),
                term: evidence,
                expected: predicate,
            },
            &[0, 1],
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        assert_eq!(abstracted.retained_declarations, vec![None, None]);
        let proof = abstracted.certificate.term;
        let statement = abstracted.certificate.expected;
        let signature = Signature::from_declarations(abstracted.certificate.signature);
        assert!(judgment_assumption_closure(&arena, &signature, &[proof, statement]).is_empty());
        let context = Context::empty().with_signature(signature);
        check_type(
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
        let reflexivity = arena.insert(Term::Refl {
            ty: two,
            value: zero,
        });
        let supplied = arena.apply_arguments(proof, &[zero, reflexivity]);
        let false_result = arena.insert(Term::Id {
            ty: two,
            left: zero,
            right: one,
        });
        assert!(
            check_type(
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
    fn lifted_constructor_projection_and_induction_compute_at_a_generalized_level() {
        let mut arena = TermArena::new();
        let mut declarations = indexed_scheme(&mut arena);
        let sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let carrier_position = declarations.len() as u32;
        declarations.push(Declaration::assumption(1, sort));
        let carrier = arena.insert(Term::Constant {
            declaration: carrier_position,
            levels: vec![Level::Parameter(0)],
        });
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let one = arena.insert(Term::TwoOne);
        let mut terms = Terms::default();
        let unit = terms.unit().closed(&mut arena).unwrap();
        let unit_value = terms.unit_value().closed(&mut arena).unwrap();
        let cases = [(81, carrier), (82, two)]
            .into_iter()
            .map(|(name, domain)| IndexedCase {
                payload: vec![IndexedField { name, domain }],
                constraints: lam(Key::Named(name), S::Closed(domain), S::Closed(unit))
                    .closed(&mut arena)
                    .unwrap(),
                child_indices: Vec::new(),
            })
            .collect::<Vec<_>>();
        let declared = IndexedData {
            parameters: Vec::new(),
            fields: Vec::new(),
            indices: Vec::new(),
            cases: cases.clone(),
            level_arity: 1,
        }
        .declare(&mut arena, &mut declarations)
        .unwrap();
        assert_eq!(declared.payload_lifts.len(), 1);
        assert_eq!(declared.family.levels[1].parameter_arity(), 1);
        let instance = declared.at(&mut arena, &[]).unwrap();
        let family = instance.family.clone();

        // Build the low case in the same high-level carrier. The padding is
        // generated from the declared product, without an A value.
        let built = instance
            .constructor(
                &mut arena,
                ConstructorArguments {
                    case: 1,
                    fields: &[],
                    payload: &[one],
                    constraints: unit_value,
                    children: &[],
                },
            )
            .unwrap();
        let label = built.label;
        let projected = arena.insert(Term::Snd { pair: label });
        let projected = arena.insert(Term::Snd { pair: projected });
        let projected = arena.insert(Term::Fst { pair: projected });
        let constructor_type = family.indexed_w(&mut arena, built.index);

        // The result itself lives at the generalized level. Returning the
        // low case's actual field makes dropping the padding incorrectly
        // observable in the derived computation, rather than just formation.
        let (result_unit, result_witness) = terms.level_unit(S::Closed(carrier));
        let result_unit = result_unit.closed(&mut arena).unwrap();
        let result_witness = result_witness.closed(&mut arena).unwrap();
        let result_type = arena.insert(Term::Sigma {
            domain: result_unit,
            codomain: two,
        });
        let invariant = arena.insert(Term::Lambda {
            domain: family.index,
            body: result_type,
        });
        let invariant = instance.declare_invariant(
            &mut arena,
            &mut declarations,
            invariant,
            Level::Parameter(0),
        );
        let mut lemmas = Vec::new();
        for (ordinal, case) in cases.iter().enumerate() {
            let field = case.payload[0];
            let value = if ordinal == 0 {
                S::Closed(zero)
            } else {
                S::Var(Key::Named(field.name))
            };
            let predicate = terms.fresh();
            let body = lam(
                Key::Named(field.name),
                S::Closed(field.domain),
                lam(
                    predicate,
                    S::Closed(unit),
                    pair(S::Closed(result_witness), value),
                ),
            )
            .closed(&mut arena)
            .unwrap();
            let context = signature_context(&declarations, 1);
            let ty = infer_type(&mut arena, &context, body, &mut Budget::default()).unwrap();
            let expected = instance.lemma_type(&mut arena, invariant, ordinal).unwrap();
            checked(
                &mut arena,
                &context,
                body,
                expected,
                "lifted lemma against lemma_type",
            );
            let declaration = declarations.len() as u32;
            declarations.push(Declaration::definition(1, ty, body));
            lemmas.push(arena.insert(Term::Constant {
                declaration,
                levels: vec![Level::Parameter(0)],
            }));
        }
        let induction = instance
            .induction(&mut arena, &mut declarations, invariant, &lemmas)
            .unwrap();
        checked_signature(&mut arena, &declarations, "lifted");
        let signature = check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
        let context = Context::with_level_arity(1).with_signature(signature.clone());
        for (term, ty, role) in [
            (family.carrier, sort, "lifted carrier"),
            (label, family.carrier, "lifted label"),
            (built.value, constructor_type, "lifted constructor"),
            (induction.step, induction.step_type, "lifted step"),
            (induction.theorem, induction.theorem_type, "lifted theorem"),
        ] {
            checked(&mut arena, &context, term, ty, role);
        }
        assert!(
            convertible(
                &mut arena,
                &context,
                projected,
                one,
                two,
                &mut Budget::default()
            )
            .unwrap()
        );
        let computed = arena.apply_arguments(induction.theorem, &[built.index, built.value]);
        let expected = arena.insert(Term::Pair {
            first: result_witness,
            second: one,
        });
        let wrong_result = arena.insert(Term::Pair {
            first: result_witness,
            second: zero,
        });
        checked(
            &mut arena,
            &context,
            computed,
            result_type,
            "lifted computation",
        );
        assert!(
            convertible(
                &mut arena,
                &context,
                computed,
                expected,
                result_type,
                &mut Budget::default()
            )
            .unwrap()
        );
        assert!(
            !convertible(
                &mut arena,
                &context,
                computed,
                wrong_result,
                result_type,
                &mut Budget::default()
            )
            .unwrap()
        );
        assert!(
            check_type(
                &mut arena,
                &Context::empty().with_signature(signature),
                induction.theorem,
                induction.theorem_type,
                &mut Budget::default(),
            )
            .is_err()
        );
    }

    /// As above, with `abstract_certificate_assumptions` and
    /// `judgment_assumption_closure` in place of the deleted elaborator's
    /// `abstract_parameters` and `excludes_parameters`.
    #[test]
    fn source_case_abstraction_preserves_bound_proof_variables_and_dependency_closure() {
        let mut arena = TermArena::new();
        let two = arena.insert(Term::Two);
        let mut declarations = vec![Declaration::assumption(0, two)];
        let value = arena.insert(Term::Constant {
            declaration: 0,
            levels: Vec::new(),
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
        let signature = Signature::from_declarations(declarations.clone());
        assert!(judgment_assumption_closure(&arena, &signature, &[proof, ty]).contains(&0));
        let abstracted = abstract_certificate_assumptions(
            &mut arena,
            &MathematicalCertificate {
                signature: declarations.clone(),
                level_arity: 0,
                context: Vec::new(),
                term: proof,
                expected: ty,
            },
            &[0],
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        let (proof, ty) = (abstracted.certificate.term, abstracted.certificate.expected);
        let signature = Signature::from_declarations(abstracted.certificate.signature);
        assert!(judgment_assumption_closure(&arena, &signature, &[proof, ty]).is_empty());
        check_type(
            &mut arena,
            &Context::empty().with_signature(signature),
            proof,
            ty,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();

        // A definition that unfolds to the assumption still depends on it.
        declarations.push(Declaration::definition(0, two, value));
        let hidden = arena.insert(Term::Constant {
            declaration: 1,
            levels: Vec::new(),
        });
        let signature = Signature::from_declarations(declarations);
        assert!(judgment_assumption_closure(&arena, &signature, &[hidden]).contains(&0));
    }

    /// `(field : Two)`, indexed by the field itself, with a case of no
    /// children and a case with one child at the node's own index.
    fn two_indexed(arena: &mut TermArena) -> (Vec<Declaration>, IndexedData, TermHandle) {
        let declarations = indexed_scheme(arena);
        let two = arena.insert(Term::Two);
        let name = 2;
        let x = Key::Named(name);
        let empty = arena.insert(Term::Empty);
        let strict_unit = arena.insert(Term::Pi {
            domain: empty,
            codomain: empty,
        });
        let unit = arena.insert(Term::Box { ty: strict_unit });
        let constraints = lam(x, S::Closed(two), S::Closed(unit))
            .closed(arena)
            .unwrap();
        let child_index = lam(x, S::Closed(two), S::Var(x)).closed(arena).unwrap();
        let cases = [0usize, 1]
            .into_iter()
            .map(|count| IndexedCase {
                payload: Vec::new(),
                constraints,
                child_indices: vec![child_index; count],
            })
            .collect();
        let data = IndexedData {
            parameters: Vec::new(),
            fields: vec![IndexedField { name, domain: two }],
            indices: vec![name],
            cases,
            level_arity: 0,
        };
        (declarations, data, unit)
    }

    #[test]
    fn generated_source_induction_checks_multiple_cases_and_child_positions() {
        let mut arena = TermArena::new();
        let (mut declarations, data, unit) = two_indexed(&mut arena);
        let two = arena.insert(Term::Two);
        let declared = data.declare(&mut arena, &mut declarations).unwrap();
        let instance = declared.at(&mut arena, &[]).unwrap();
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
        let invariant = instance.declare_invariant(
            &mut arena,
            &mut declarations,
            invariant,
            Level::Constant(0),
        );
        let mut lemmas = Vec::new();
        for (ordinal, case) in declared.data.cases.iter().enumerate() {
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
            let context = signature_context(&declarations, 0);
            let ty = infer_type(
                &mut arena,
                &context,
                body,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            let expected = instance.lemma_type(&mut arena, invariant, ordinal).unwrap();
            checked(
                &mut arena,
                &context,
                body,
                expected,
                "two-indexed lemma against lemma_type",
            );
            let declaration = declarations.len() as u32;
            declarations.push(Declaration::definition(0, ty, body));
            lemmas.push(arena.insert(Term::Constant {
                declaration,
                levels: Vec::new(),
            }));
        }
        let induction = instance
            .induction(&mut arena, &mut declarations, invariant, &lemmas)
            .unwrap();
        let context = signature_context(&declarations, 0);
        checked(
            &mut arena,
            &context,
            induction.step,
            induction.step_type,
            "two-indexed step",
        );
        checked_signature(&mut arena, &declarations, "two-indexed");
        checked(
            &mut arena,
            &context,
            induction.theorem,
            induction.theorem_type,
            "two-indexed theorem",
        );
        let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        assert!(
            check_type(
                &mut arena,
                &context,
                induction.theorem,
                sort,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .is_err()
        );
    }

    /// The stand-in integer vocabulary: opaque constants in place of the
    /// integer theory this crate does not have.
    struct Vector {
        arena: TermArena,
        declarations: Vec<Declaration>,
        source_like: bool,
        integer: TermHandle,
        scalar: TermHandle,
        embedding: Option<u32>,
        multiply: TermHandle,
        less_or_equal: TermHandle,
        zero: TermHandle,
        one: TermHandle,
        ten: TermHandle,
        eighty: TermHandle,
    }

    fn binary(
        arena: &mut TermArena,
        operation: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        arena.apply_arguments(operation, &[left, right])
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
        let codomain = shift(arena, right, 0, 1);
        arena.insert(Term::Sigma {
            domain: left,
            codomain,
        })
    }

    fn close(
        arena: &mut TermArena,
        domain: TermHandle,
        count: usize,
        mut body: TermHandle,
    ) -> TermHandle {
        for _ in 0..count {
            body = arena.insert(Term::Lambda { domain, body });
        }
        body
    }

    impl Vector {
        /// The identity embedding from the source alias to the integers,
        /// checked by kernel conversion; the value itself is returned.
        fn embed(&mut self, value: TermHandle) -> TermHandle {
            if !self.source_like {
                return value;
            }
            let arena = &mut self.arena;
            let integer = self.integer;
            let declaration = match self.embedding {
                Some(declaration) => declaration,
                None => {
                    let ty = arena.insert(Term::Pi {
                        domain: integer,
                        codomain: integer,
                    });
                    let variable = arena.insert(Term::Variable(0));
                    let body = arena.insert(Term::Lambda {
                        domain: integer,
                        body: variable,
                    });
                    let declaration = self.declarations.len() as u32;
                    self.declarations.push(Declaration::definition(0, ty, body));
                    self.embedding = Some(declaration);
                    declaration
                }
            };
            let function = arena.insert(Term::Constant {
                declaration,
                levels: Vec::new(),
            });
            let raw = apply(arena, function, value);
            let mut context = signature_context(&self.declarations, 0);
            for _ in 0..5 {
                context = context.extend(self.scalar);
            }
            assert!(
                convertible(
                    arena,
                    &context,
                    raw,
                    value,
                    integer,
                    &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                )
                .unwrap()
            );
            value
        }

        /// Close `body` over `count` source fields, once with the source
        /// alias and once with the integer carrier, and check both agree.
        fn close_source_telescope(&mut self, count: usize, body: TermHandle) -> TermHandle {
            let arena = &mut self.arena;
            let raw = close(arena, self.scalar, count, body);
            let compact = close(arena, self.integer, count, body);
            let context = signature_context(&self.declarations, 0);
            let inferred = infer_type(
                arena,
                &context,
                compact,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            let raw_type = infer_type(
                arena,
                &context,
                raw,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            assert_eq!(
                judgment_assumption_closure(arena, context.signature(), &[raw, raw_type]),
                judgment_assumption_closure(arena, context.signature(), &[compact, inferred]),
            );
            assert!(
                convertible(
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

        /// `Vector<Length, Total>` with
        /// `Nil requires Length == 0 && Total == 0` and
        /// `Cons(previous, previous_total, head, tail: Vector<previous, previous_total>)
        /// requires head <= 10 && previous + 1 == Length && previous_total + head == Total`.
        fn new(source_like: bool) -> (Self, DeclaredIndexedData) {
            let mut arena = TermArena::new();
            let mut declarations = indexed_scheme(&mut arena);
            let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
            let integer = assume(&mut arena, &mut declarations, type_zero);
            let operation = arena.insert(Term::Pi {
                domain: integer,
                codomain: integer,
            });
            let operation = arena.insert(Term::Pi {
                domain: integer,
                codomain: operation,
            });
            let add = assume(&mut arena, &mut declarations, operation);
            let multiply = assume(&mut arena, &mut declarations, operation);
            let relation = arena.insert(Term::Pi {
                domain: integer,
                codomain: type_zero,
            });
            let relation = arena.insert(Term::Pi {
                domain: integer,
                codomain: relation,
            });
            let less_or_equal = assume(&mut arena, &mut declarations, relation);
            let [zero, one, ten, eighty] =
                [(); 4].map(|()| assume(&mut arena, &mut declarations, integer));
            let scalar = if source_like {
                let alias = declarations.len() as u32;
                declarations.push(Declaration::definition(0, type_zero, integer));
                arena.insert(Term::Constant {
                    declaration: alias,
                    levels: Vec::new(),
                })
            } else {
                integer
            };
            let mut vector = Vector {
                arena,
                declarations,
                source_like,
                integer,
                scalar,
                embedding: None,
                multiply,
                less_or_equal,
                zero,
                one,
                ten,
                eighty,
            };
            let (length_name, total_name) = (10, 11);
            let payload_names = [12, 13, 14];
            let arena = &mut vector.arena;
            let length = arena.insert(Term::Variable(1));
            let total = arena.insert(Term::Variable(0));
            let length_zero = equality(arena, integer, length, zero);
            let total_zero = equality(arena, integer, total, zero);
            let nil_predicate = and(arena, length_zero, total_zero);
            let nil_constraints = vector.close_source_telescope(2, nil_predicate);
            let arena = &mut vector.arena;
            let length = arena.insert(Term::Variable(4));
            let total = arena.insert(Term::Variable(3));
            let previous = arena.insert(Term::Variable(2));
            let previous_total = arena.insert(Term::Variable(1));
            let head = arena.insert(Term::Variable(0));
            let head_bound = binary(arena, less_or_equal, head, ten);
            let previous = vector.embed(previous);
            let embedded_length = vector.embed(length);
            let successor = binary(&mut vector.arena, add, previous, one);
            let length_equation = equality(&mut vector.arena, integer, successor, embedded_length);
            let embedded_previous_total = vector.embed(previous_total);
            let embedded_head = vector.embed(head);
            let embedded_total = vector.embed(total);
            let arena = &mut vector.arena;
            let sum = binary(arena, add, embedded_previous_total, embedded_head);
            let total_equation = equality(arena, integer, sum, embedded_total);
            let equations = and(arena, length_equation, total_equation);
            let cons_predicate = and(arena, head_bound, equations);
            let cons_constraints = vector.close_source_telescope(5, cons_predicate);
            let arena = &mut vector.arena;
            let previous = arena.insert(Term::Variable(2));
            let index = arena.insert(Term::Pair {
                first: previous,
                second: previous_total,
            });
            let child_index = vector.close_source_telescope(5, index);
            let field = |name| IndexedField {
                name,
                domain: integer,
            };
            // The W tree uses the integer carrier; the authored alias
            // domains remain in the constraint and invariant functions.
            let data = IndexedData {
                parameters: Vec::new(),
                fields: vec![field(length_name), field(total_name)],
                indices: vec![length_name, total_name],
                cases: vec![
                    IndexedCase {
                        payload: Vec::new(),
                        constraints: nil_constraints,
                        child_indices: Vec::new(),
                    },
                    IndexedCase {
                        payload: payload_names.map(field).to_vec(),
                        constraints: cons_constraints,
                        child_indices: vec![child_index],
                    },
                ],
                level_arity: 0,
            };
            let declared = data
                .declare(&mut vector.arena, &mut vector.declarations)
                .unwrap();
            (vector, declared)
        }

        /// An assumed proof of Cons's constraints at these values.
        fn cons_evidence(
            &mut self,
            declared: &DeclaredIndexedData,
            values: [TermHandle; 5],
        ) -> TermHandle {
            let constraints = declared.data.cases[1].constraints;
            let ty = self.arena.apply_arguments(constraints, &values);
            assume(&mut self.arena, &mut self.declarations, ty)
        }
    }

    fn vector_budget(source_like: bool) {
        let (mut vector, declared) = Vector::new(source_like);
        let instance = declared.at(&mut vector.arena, &[]).unwrap();
        let family = instance.family.clone();
        let (zero, one, ten, eighty) = (vector.zero, vector.one, vector.ten, vector.eighty);

        // (a) the family at an index, and (b) its constructor values.
        let arena = &mut vector.arena;
        let zero_zero = arena.insert(Term::Pair {
            first: zero,
            second: zero,
        });
        let at_zero = family.indexed_w(arena, zero_zero);
        let reflexive = arena.insert(Term::Refl {
            ty: vector.integer,
            value: zero,
        });
        let nil_evidence = arena.insert(Term::Pair {
            first: reflexive,
            second: reflexive,
        });
        let nil = instance
            .constructor(
                arena,
                ConstructorArguments {
                    case: 0,
                    fields: &[zero, zero],
                    payload: &[],
                    constraints: nil_evidence,
                    children: &[],
                },
            )
            .unwrap();
        let cons_evidence = vector.cons_evidence(&declared, [one, one, zero, zero, one]);
        let cons = instance
            .constructor(
                &mut vector.arena,
                ConstructorArguments {
                    case: 1,
                    fields: &[one, one],
                    payload: &[zero, zero, one],
                    constraints: cons_evidence,
                    children: &[nil.value],
                },
            )
            .unwrap();
        let context = signature_context(&vector.declarations, 0);
        checked(
            &mut vector.arena,
            &context,
            nil.value,
            at_zero,
            "indexed vector Nil at (0, 0)",
        );
        let one_one = vector.arena.insert(Term::Pair {
            first: one,
            second: one,
        });
        let at_one = family.indexed_w(&mut vector.arena, one_one);
        checked(
            &mut vector.arena,
            &context,
            cons.value,
            at_one,
            "indexed vector Cons at (1, 1)",
        );

        // (c) the induction over Total <= 10 * Length.
        let arena = &mut vector.arena;
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
        let authored_length = vector.embed(authored_length);
        let authored_total = vector.embed(authored_total);
        let arena = &mut vector.arena;
        let bound = binary(arena, vector.multiply, ten, authored_length);
        let goal = binary(arena, vector.less_or_equal, authored_total, bound);
        let goal = if source_like {
            let authored = close(arena, vector.scalar, 2, goal);
            let applied = apply(arena, authored, length);
            let raw_goal = apply(arena, applied, total);
            // Reduce only the binder and application scaffolding built just
            // above; no authored function or predicate is normalized.
            let Term::Lambda { body, .. } = arena.get(authored) else {
                unreachable!()
            };
            let remaining = substitute(arena, body, length);
            let Term::Lambda { body, .. } = arena.get(remaining) else {
                unreachable!()
            };
            let normalized = substitute(arena, body, total);
            let context = signature_context(&vector.declarations, 0).extend(family.index);
            let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
            assert!(
                convertible(
                    arena,
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
        let invariant = instance.declare_invariant(
            arena,
            &mut vector.declarations,
            invariant,
            Level::Constant(0),
        );
        // The per-case arithmetic lemmas, assumed at the type the induction
        // asks for: the integer theory would prove them.
        let mut lemmas = Vec::new();
        for case in 0..declared.data.cases.len() {
            let ty = instance
                .lemma_type(&mut vector.arena, invariant, case)
                .unwrap();
            lemmas.push(assume(&mut vector.arena, &mut vector.declarations, ty));
        }
        let induction = instance
            .induction(
                &mut vector.arena,
                &mut vector.declarations,
                invariant,
                &lemmas,
            )
            .unwrap();
        let context = signature_context(&vector.declarations, 0);
        checked(
            &mut vector.arena,
            &context,
            induction.step,
            induction.step_type,
            "indexed vector step",
        );
        checked(
            &mut vector.arena,
            &context,
            induction.theorem,
            induction.theorem_type,
            "indexed vector theorem",
        );
        checked_signature(&mut vector.arena, &vector.declarations, "indexed vector");
        let applied = vector
            .arena
            .apply_arguments(induction.theorem, &[cons.index, cons.value]);
        let goal = apply(&mut vector.arena, invariant, cons.index);
        checked(
            &mut vector.arena,
            &context,
            applied,
            goal,
            "indexed vector theorem at Cons",
        );

        // A closed specialization uses direct evidence instead of the step.
        let arena = &mut vector.arena;
        let closed_goal = binary(arena, vector.less_or_equal, ten, eighty);
        let closed_proof = assume(arena, &mut vector.declarations, closed_goal);
        let arena = &mut vector.arena;
        let closed_invariant = arena.insert(Term::Lambda {
            domain: family.index,
            body: closed_goal,
        });
        let (theorem, ty) =
            instance.constant_invariant_theorem(arena, closed_invariant, closed_proof);
        let context = signature_context(&vector.declarations, 0);
        checked(
            arena,
            &context,
            theorem,
            ty,
            "indexed vector closed specialization",
        );
        let false_goal = binary(arena, vector.less_or_equal, eighty, ten);
        let false_invariant = arena.insert(Term::Lambda {
            domain: family.index,
            body: false_goal,
        });
        let (_, false_type) =
            instance.constant_invariant_theorem(arena, false_invariant, closed_proof);
        assert!(
            check_type(
                arena,
                &context,
                theorem,
                false_type,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .is_err()
        );
    }

    #[test]
    fn indexed_vector_constructors_and_induction_fit_the_default_budget() {
        vector_budget(false);
    }

    #[test]
    fn indexed_vector_with_source_alias_and_embeddings_fits_the_default_budget() {
        vector_budget(true);
    }

    /// `Cons(previous: 1, ...)` requires its tail at `(1, 0)`; a tail at
    /// `(0, 0)` is refused. Its positive twin, with `previous: 0`, checks.
    #[test]
    fn constructor_with_a_child_at_the_wrong_index_is_rejected() {
        let (mut vector, declared) = Vector::new(false);
        let instance = declared.at(&mut vector.arena, &[]).unwrap();
        let (zero, one) = (vector.zero, vector.one);
        let reflexive = vector.arena.insert(Term::Refl {
            ty: vector.integer,
            value: zero,
        });
        let nil_evidence = vector.arena.insert(Term::Pair {
            first: reflexive,
            second: reflexive,
        });
        let nil = instance
            .constructor(
                &mut vector.arena,
                ConstructorArguments {
                    case: 0,
                    fields: &[zero, zero],
                    payload: &[],
                    constraints: nil_evidence,
                    children: &[],
                },
            )
            .unwrap();
        let cons = |vector: &mut Vector, previous| {
            let evidence = vector.cons_evidence(&declared, [one, one, previous, zero, one]);
            instance
                .constructor(
                    &mut vector.arena,
                    ConstructorArguments {
                        case: 1,
                        fields: &[one, one],
                        payload: &[previous, zero, one],
                        constraints: evidence,
                        children: &[nil.value],
                    },
                )
                .unwrap()
        };
        let right = cons(&mut vector, zero);
        let wrong = cons(&mut vector, one);
        let context = signature_context(&vector.declarations, 0);
        let family = &instance.family;
        let right_type = family.indexed_w(&mut vector.arena, right.index);
        checked(
            &mut vector.arena,
            &context,
            right.value,
            right_type,
            "Cons with its tail at (0, 0)",
        );
        // The wrong Cons's label, with its constraint proof, is well formed:
        // only its child is refused.
        checked(
            &mut vector.arena,
            &context,
            wrong.label,
            family.carrier,
            "label of Cons requiring its tail at (1, 0)",
        );
        let wrong_type = family.indexed_w(&mut vector.arena, wrong.index);
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let refused = check_type(
            &mut vector.arena,
            &context,
            wrong.value,
            wrong_type,
            &mut budget,
        );
        eprintln!(
            "Cons requiring its tail at (1, 0): {} kernel steps, {refused:?}",
            DEFAULT_CONVERSION_STEPS - budget.remaining()
        );
        assert!(
            matches!(refused, Err(CoreError::ArgumentTypeMismatch { .. })),
            "{refused:?}"
        );
        // The refused argument is the child function: it is accepted at the
        // right Cons's child type and refused at the wrong one's.
        for (built, accepted) in [(right, true), (wrong, false)] {
            let Term::Apply {
                argument: children, ..
            } = vector.arena.get(built.value)
            else {
                unreachable!("a constructor value is `sup` applied to its child function")
            };
            let arena = &mut vector.arena;
            let positions = apply(arena, family.children, built.label);
            let next = apply(arena, family.next, built.label);
            let next = shift(arena, next, 0, 1);
            let position = arena.insert(Term::Variable(0));
            let index = apply(arena, next, position);
            let child = family.indexed_w(arena, index);
            let expected = arena.insert(Term::Pi {
                domain: positions,
                codomain: child,
            });
            let result = check_type(
                arena,
                &context,
                children,
                expected,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            );
            assert_eq!(result.is_ok(), accepted, "{result:?}");
        }
    }
}
