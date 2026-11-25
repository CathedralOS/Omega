//! A value-indexed `data` declaration as a unified native inductive family.
//!
//! This module lowers a description of a value-indexed declaration such as
//!
//! ```text
//! data Vector<Length: u64, Total: u64> requires ... {
//!     case Nil;
//!     case Cons(previous: u64, previous_total: u64, head: u64,
//!               tail [erased]: Vector<previous, previous_total>);
//! }
//! ```
//!
//! directly to a native inductive family via [`declare_inductive_family`],
//! builds constructor applications, and builds induction principles over an
//! invariant of the index using the native recursor `I.rec`. This module adds
//! no kernel rule and no assumption; the kernel checks every declaration it
//! emits and every term it returns.
//!
//! The encoding, in kernel types:
//!
//! - The index type (`index_type_decl`) is a definition over the uniform
//!   parameters returning the right-nested `Sigma` of the index field domains,
//!   or the boxed unit `Box(Empty -> Empty)` when there are no indices.
//! - The inductive type former `I` has type
//!   `Π(uniform_params...). Π(idx : index_type_decl uniform_params...). Type family_level`,
//!   where `family_level` is the maximum of `0`, the index level, and the sort
//!   levels of all shared fields, payload fields, and constraints.
//! - Each case `c` becomes constructor `C_c` of type
//!   `Π(uniform_params...). Π(shared_fields...). Π(payload...). Π(constraints). Π(children...). I uniform_params... out_index`.
//! - Induction over `invariant : index_type -> Type result_level` applies
//!   `I.rec[levels..., result_level]` directly with motive
//!   `λidx. λ_. invariant idx` and one minor premise per case forwarding to its
//!   lemma, adding zero auxiliary declarations.
//!
//! Every field domain, constraint and child index is a closed term chosen by
//! the caller. Fields and binders are named by dense numbers (`u32`); this
//! crate does not depend on the compiler's symbol table. With parameters, each
//! non-parameter field and payload domain is a function of all parameters
//! (`\(parameters). domain`); a constraint is a function of the parameters,
//! the shared fields that are not parameters, and the case's payload fields,
//! in that order; a child index function also takes those arguments and
//! returns the index. A shared field whose name is a parameter's name stores
//! that parameter in every constructor, making a non-uniform type argument an
//! index. The other, uniform, parameters stay as native inductive parameters.

use crate::conversion::{Budget, DEFAULT_CONVERSION_STEPS, weak_head_normalize};
use crate::inductive::declare_inductive_family;
use crate::signature::{Declaration, Signature};
use crate::substitution::{shift, substitute, universe_parameter_arity};
use crate::term::{Level, Sort, Term, TermArena, TermHandle, level_leq};
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
    /// The fields every constructor stores, in order. A field named like a
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

/// A value-indexed family declared as a native inductive in the caller's signature.
#[derive(Clone, Debug)]
pub struct DeclaredIndexedData {
    pub data: IndexedData,
    /// Declaration index of `index_type : Π(uniform_params...). Type index_level`.
    pub index_type_decl: u32,
    /// Declaration index of the native inductive `I`.
    /// Constructors `C_0..C_{k-1}` sit at `inductive + 1 .. inductive + k`,
    /// and recursor `I.rec` sits at `inductive + 1 + k`.
    pub inductive: u32,
    /// Universe level of the inductive family.
    pub family_level: Level,
    /// Number of uniform parameters of the inductive family.
    pub uniform_param_count: u32,
}

/// The family instantiated at actual uniform parameters and universe levels.
#[derive(Clone, Debug)]
pub struct IndexedInstance<'a> {
    pub declared: &'a DeclaredIndexedData,
    /// One actual per uniform parameter, in parameter order.
    pub uniform: Vec<TermHandle>,
    /// `index_type_decl[levels] uniform...`.
    pub index_type: TermHandle,
    /// Universe levels `[u_0, ..., u_{level_arity - 1}]` for the family.
    pub levels: Vec<Level>,
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

/// A constructor's value of type `I[levels] uniform... index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedValue {
    /// Same as `value` under native inductives; retained for caller compatibility.
    pub label: TermHandle,
    /// The index tuple the value has.
    pub index: TermHandle,
    pub value: TermHandle,
}

/// An induction over an invariant of the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedInduction {
    /// `\i. \t. I.rec(...)`, of type `theorem_type`.
    pub theorem: TermHandle,
    /// `(i : index_type) -> I[levels] uniform... i -> invariant(i)`.
    pub theorem_type: TermHandle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Named(u32),
    Internal(usize),
}

/// Terms with named binders. `term` resolves each name to its de Bruijn index.
#[derive(Clone)]
enum S {
    Closed(TermHandle),
    /// A domain function of all parameters, applied to their actuals.
    ParameterDomain(TermHandle, Vec<S>),
    Var(Key),
    Universe(Level),
    Empty,
    Pi(Key, Box<S>, Box<S>),
    Lam(Key, Box<S>, Box<S>),
    Sigma(Key, Box<S>, Box<S>),
    App(Box<S>, Box<S>),
    Pair(Box<S>, Box<S>),
    Boxed(Box<S>),
    BoxIntro(Box<S>, Box<S>),
}

fn app(f: S, x: S) -> S {
    S::App(Box::new(f), Box::new(x))
}
fn pair(a: S, b: S) -> S {
    S::Pair(Box::new(a), Box::new(b))
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
            Self::Universe(level) => Term::Sort(Sort::Type(level.clone())),
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
            Self::Boxed(a) => Term::Box {
                ty: a.term(arena, scope)?,
            },
            Self::BoxIntro(a, x) => Term::BoxIntro {
                ty: a.term(arena, scope)?,
                value: x.term(arena, scope)?,
            },
        };
        Ok(arena.insert(node))
    }

    fn closed(&self, arena: &mut TermArena) -> Result<TermHandle, IndexedDataError> {
        self.term(arena, &mut Vec::new())
    }
}

fn max_level(a: Level, b: Level) -> Level {
    if level_leq(&b, &a) {
        a
    } else if level_leq(&a, &b) {
        b
    } else {
        a.maximum(b)
    }
}

fn identity_levels(arity: u32) -> Vec<Level> {
    (0..arity).map(Level::Parameter).collect()
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
    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
    let mut ty = infer_type(arena, context, function, &mut budget)?;
    loop {
        let head = weak_head_normalize(arena, context.signature(), ty, &mut budget)?;
        match arena.get(head) {
            Term::Pi { codomain, .. } => ty = codomain,
            Term::Sort(sort) => return Ok(sort.level()),
            _ => {
                return Err(IndexedDataError::Malformed(
                    "a field domain or constraint does not return a universe",
                ));
            }
        }
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
        levels: identity_levels(level_arity),
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

/// Fresh binder names and the unit type used when a family has no indices.
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
}

/// Builds the family's terms from its description. `actuals` replaces
/// uniform parameters by closed terms; without an actual a parameter stays a
/// named variable.
struct Builder<'a> {
    terms: Terms,
    data: &'a IndexedData,
    actuals: &'a [(u32, TermHandle)],
}

impl<'a> Builder<'a> {
    fn new(data: &'a IndexedData, actuals: &'a [(u32, TermHandle)]) -> Self {
        Self {
            terms: Terms::default(),
            data,
            actuals,
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
    /// Child `child`'s index at the given payload values.
    fn child_index(&self, case: usize, child: usize, payload: &[S]) -> S {
        let mut term =
            self.common_application(S::Closed(self.data.cases[case].child_indices[child]));
        for value in payload {
            term = app(term, value.clone());
        }
        term
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
}

impl IndexedData {
    /// Declare `index_type_decl` and the native inductive block
    /// `[I, C_0, ..., C_{k-1}, I.rec]` in `declarations`.
    pub fn declare(
        self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
    ) -> Result<DeclaredIndexedData, IndexedDataError> {
        let uniform_params: Vec<IndexedField> = self.uniform_parameters().copied().collect();
        let uniform_param_count = uniform_params.len() as u32;
        let checking = context(declarations, self.level_arity);

        let mut index_level = Level::Constant(0);
        let mut family_level = Level::Constant(0);
        for field in &self.fields {
            let level = result_sort_level(arena, &checking, field.domain)?;
            family_level = max_level(family_level, level.clone());
            if self.indices.contains(&field.name) {
                index_level = max_level(index_level, level);
            }
        }
        for case in &self.cases {
            for field in &case.payload {
                let level = result_sort_level(arena, &checking, field.domain)?;
                family_level = max_level(family_level, level);
            }
            let level = result_sort_level(arena, &checking, case.constraints)?;
            family_level = max_level(family_level, level);
        }
        family_level = max_level(family_level, index_level.clone());

        let mut b = Builder::new(&self, &[]);
        let mut index_body = b.index_type()?;
        let mut index_ty = S::Universe(index_level);
        for param in uniform_params.iter().rev() {
            index_body = lam(Key::Named(param.name), S::Closed(param.domain), index_body);
            index_ty = pi(Key::Named(param.name), S::Closed(param.domain), index_ty);
        }
        let index_decl_ty = index_ty.closed(arena)?;
        let index_decl_body = index_body.closed(arena)?;
        let index_type_decl = declarations.len() as u32;
        declarations.push(Declaration::definition(
            self.level_arity,
            index_decl_ty,
            index_decl_body,
        ));

        let id_levels = identity_levels(self.level_arity);
        let idx_type_const = arena.insert(Term::Constant {
            declaration: index_type_decl,
            levels: id_levels.clone(),
        });
        let base = declarations.len() as u32;
        let ind_const = arena.insert(Term::Constant {
            declaration: base,
            levels: id_levels,
        });

        let apply_uniform = |head: TermHandle| {
            let mut term = S::Closed(head);
            for param in &uniform_params {
                term = app(term, S::Var(Key::Named(param.name)));
            }
            term
        };

        let idx_binder = b.terms.fresh();
        let mut inductive_ty_s = pi(
            idx_binder,
            apply_uniform(idx_type_const),
            S::Universe(family_level.clone()),
        );
        for param in uniform_params.iter().rev() {
            inductive_ty_s = pi(
                Key::Named(param.name),
                S::Closed(param.domain),
                inductive_ty_s,
            );
        }
        let inductive_ty = inductive_ty_s.closed(arena)?;

        let index_positions = b.index_positions()?;
        let mut ctor_types = Vec::with_capacity(self.cases.len());
        for (case_idx, case) in self.cases.iter().enumerate() {
            let payload_vars: Vec<S> = case
                .payload
                .iter()
                .map(|field| S::Var(Key::Named(field.name)))
                .collect();
            let out_values: Vec<S> = index_positions
                .iter()
                .map(|&pos| S::Var(Key::Named(self.fields[pos].name)))
                .collect();
            let out_index = b.index_value(out_values);
            let mut ctor_s = app(apply_uniform(ind_const), out_index);

            for child in (0..case.child_indices.len()).rev() {
                let child_idx = b.child_index(case_idx, child, &payload_vars);
                let child_ty = app(apply_uniform(ind_const), child_idx);
                let binder = b.terms.fresh();
                ctor_s = pi(binder, child_ty, ctor_s);
            }
            let constraint_binder = b.terms.fresh();
            ctor_s = pi(
                constraint_binder,
                b.constraint_application(case_idx),
                ctor_s,
            );
            for field in case.payload.iter().rev() {
                ctor_s = pi(Key::Named(field.name), b.domain(field), ctor_s);
            }
            for field in self.fields.iter().rev() {
                ctor_s = pi(Key::Named(field.name), b.domain(field), ctor_s);
            }
            for param in uniform_params.iter().rev() {
                ctor_s = pi(Key::Named(param.name), S::Closed(param.domain), ctor_s);
            }
            ctor_types.push(ctor_s.closed(arena)?);
        }

        let block = declare_inductive_family(
            arena,
            base,
            self.level_arity,
            uniform_param_count,
            inductive_ty,
            &ctor_types,
        );
        declarations.extend(block);

        Ok(DeclaredIndexedData {
            data: self,
            index_type_decl,
            inductive: base,
            family_level,
            uniform_param_count,
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
        let levels = identity_levels(self.data.level_arity);
        self.at_levels(arena, uniform, &levels)
    }

    /// The family at actual uniform parameters and explicit universe levels.
    pub fn at_levels(
        &self,
        arena: &mut TermArena,
        uniform: &[TermHandle],
        levels: &[Level],
    ) -> Result<IndexedInstance<'_>, IndexedDataError> {
        if uniform.len() != self.uniform_param_count as usize {
            return Err(IndexedDataError::Malformed(
                "the family needs one actual per uniform parameter",
            ));
        }
        if levels.len() != self.data.level_arity as usize {
            return Err(IndexedDataError::Malformed(
                "the family needs one universe level per level parameter",
            ));
        }
        let idx_type_const = arena.insert(Term::Constant {
            declaration: self.index_type_decl,
            levels: levels.to_vec(),
        });
        let index_type = arena.apply_arguments(idx_type_const, uniform);
        Ok(IndexedInstance {
            declared: self,
            uniform: uniform.to_vec(),
            index_type,
            levels: levels.to_vec(),
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
        Builder::new(&self.declared.data, actuals)
    }

    /// The type `I[levels] uniform... index`.
    pub fn family_type(&self, arena: &mut TermArena, index: TermHandle) -> TermHandle {
        let ind = arena.insert(Term::Constant {
            declaration: self.declared.inductive,
            levels: self.levels.clone(),
        });
        let applied = arena.apply_arguments(ind, &self.uniform);
        apply(arena, applied, index)
    }

    /// A constructor's value `C_c[levels] uniform... fields... payload... constraints children...`.
    /// The kernel's type check of `value` against `family_type(index)` is the
    /// caller's: it rejects wrong constraint proofs and children at the wrong index.
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
        let ctor_decl = self.declared.inductive + 1 + arguments.case as u32;
        let ctor_const = arena.insert(Term::Constant {
            declaration: ctor_decl,
            levels: self.levels.clone(),
        });
        let mut value = arena.apply_arguments(ctor_const, &self.uniform);
        value = arena.apply_arguments(value, arguments.fields);
        value = arena.apply_arguments(value, arguments.payload);
        value = apply(arena, value, arguments.constraints);
        value = arena.apply_arguments(value, arguments.children);

        let actuals = self.actuals();
        let mut b = self.builder(&actuals);
        let index_values: Vec<S> = b
            .index_positions()?
            .into_iter()
            .map(|pos| S::Closed(arguments.fields[pos]))
            .collect();
        let index = b.index_value(index_values).closed(arena)?;

        Ok(IndexedValue {
            label: value,
            index,
            value,
        })
    }

    /// Declare `invariant : index_type -> Type(result_level)` as a definition
    /// and return its constant. Lemmas and the induction name it.
    pub fn declare_invariant(
        &self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        invariant: TermHandle,
        result_level: Level,
    ) -> TermHandle {
        let sort = arena.insert(Term::Sort(Sort::Type(result_level)));
        let ty = arena.insert(Term::Pi {
            domain: self.index_type,
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
    /// given one lemma per case of the type `lemma_type` states.
    pub fn induction(
        &self,
        arena: &mut TermArena,
        declarations: &[Declaration],
        invariant: TermHandle,
        lemmas: &[TermHandle],
    ) -> Result<IndexedInduction, IndexedDataError> {
        let data = &self.declared.data;
        if data.cases.len() != lemmas.len() {
            return Err(IndexedDataError::Malformed(
                "the induction needs one lemma per case",
            ));
        }
        let level_arity = data.level_arity.max(universe_parameter_arity(
            arena,
            &[self.index_type, invariant],
        ));
        let result_level =
            result_sort_level(arena, &context(declarations, level_arity), invariant)?;

        let idx = arena.insert(Term::Variable(0));
        let major_domain = self.family_type(arena, idx);
        let idx_under_major = arena.insert(Term::Variable(1));
        let invariant_under = shift(arena, invariant, 0, 2);
        let motive_result = apply(arena, invariant_under, idx_under_major);
        let motive_inner = arena.insert(Term::Lambda {
            domain: major_domain,
            body: motive_result,
        });
        let motive = arena.insert(Term::Lambda {
            domain: self.index_type,
            body: motive_inner,
        });

        let ind_const = arena.insert(Term::Constant {
            declaration: self.declared.inductive,
            levels: self.levels.clone(),
        });
        let ind_at_uniform = arena.apply_arguments(ind_const, &self.uniform);

        let actuals = self.actuals();
        let mut b = self.builder(&actuals);
        let mut minors = Vec::with_capacity(data.cases.len());
        for (case, &lemma) in lemmas.iter().enumerate() {
            let shape = &data.cases[case];
            let payload_vars: Vec<S> = shape
                .payload
                .iter()
                .map(|field| S::Var(Key::Named(field.name)))
                .collect();
            let child_indices: Vec<S> = (0..shape.child_indices.len())
                .map(|child| b.child_index(case, child, &payload_vars))
                .collect();
            let constraint_key = b.terms.fresh();
            let child_keys: Vec<Key> = (0..child_indices.len()).map(|_| b.terms.fresh()).collect();
            let ih_keys: Vec<Key> = (0..child_indices.len()).map(|_| b.terms.fresh()).collect();

            let mut body = S::Closed(lemma);
            for field in &data.fields {
                body = app(body, S::Var(Key::Named(field.name)));
            }
            for field in &shape.payload {
                body = app(body, S::Var(Key::Named(field.name)));
            }
            body = app(body, S::Var(constraint_key));
            for &ih_key in &ih_keys {
                body = app(body, S::Var(ih_key));
            }

            for (child, &ih_key) in ih_keys.iter().enumerate().rev() {
                let ih_ty = app(S::Closed(invariant), child_indices[child].clone());
                body = lam(ih_key, ih_ty, body);
            }
            for (child, &child_key) in child_keys.iter().enumerate().rev() {
                let child_ty = app(S::Closed(ind_at_uniform), child_indices[child].clone());
                body = lam(child_key, child_ty, body);
            }
            body = lam(constraint_key, b.constraint_application(case), body);
            for field in shape.payload.iter().rev() {
                body = lam(Key::Named(field.name), b.domain(field), body);
            }
            for field in data.fields.iter().rev() {
                body = lam(Key::Named(field.name), b.domain(field), body);
            }
            minors.push(body.closed(arena)?);
        }

        let rec_decl = self.declared.inductive + 1 + data.cases.len() as u32;
        let mut rec_levels = self.levels.clone();
        rec_levels.push(result_level);
        let rec_const = arena.insert(Term::Constant {
            declaration: rec_decl,
            levels: rec_levels,
        });
        let mut rec_app = arena.apply_arguments(rec_const, &self.uniform);
        rec_app = apply(arena, rec_app, motive);
        rec_app = arena.apply_arguments(rec_app, &minors);

        let rec_app_under = shift(arena, rec_app, 0, 2);
        let idx_var = arena.insert(Term::Variable(1));
        let major_var = arena.insert(Term::Variable(0));
        let at_idx = apply(arena, rec_app_under, idx_var);
        let body = apply(arena, at_idx, major_var);
        let theorem_inner = arena.insert(Term::Lambda {
            domain: major_domain,
            body,
        });
        let theorem = arena.insert(Term::Lambda {
            domain: self.index_type,
            body: theorem_inner,
        });
        let theorem_type_inner = arena.insert(Term::Pi {
            domain: major_domain,
            codomain: motive_result,
        });
        let theorem_type = arena.insert(Term::Pi {
            domain: self.index_type,
            codomain: theorem_type_inner,
        });
        Ok(IndexedInduction {
            theorem,
            theorem_type,
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
        let i = arena.insert(Term::Variable(0));
        let evidence_type = self.family_type(arena, i);
        let closed = shift(arena, proof, 0, 2);
        let body = arena.insert(Term::Lambda {
            domain: evidence_type,
            body: closed,
        });
        let theorem = arena.insert(Term::Lambda {
            domain: self.index_type,
            body,
        });
        let i = arena.insert(Term::Variable(1));
        let proposition = apply(arena, invariant, i);
        let result = arena.insert(Term::Pi {
            domain: evidence_type,
            codomain: proposition,
        });
        let ty = arena.insert(Term::Pi {
            domain: self.index_type,
            codomain: result,
        });
        (theorem, ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MathematicalCertificate, abstract_certificate_assumptions, check_signature, check_type,
        convertible, judgment_assumption_closure,
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

    /// Build `Box((x : carrier) -> squash(Id carrier x x))` and its canonical inhabitant.
    fn level_unit(arena: &mut TermArena, carrier: TermHandle) -> (TermHandle, TermHandle) {
        let x = arena.insert(Term::Variable(0));
        let id = arena.insert(Term::Id {
            ty: carrier,
            left: x,
            right: x,
        });
        let squashed = arena.insert(Term::Squash { ty: id });
        let proposition = arena.insert(Term::Pi {
            domain: carrier,
            codomain: squashed,
        });
        let refl = arena.insert(Term::Refl {
            ty: carrier,
            value: x,
        });
        let squash_val = arena.insert(Term::SquashIntro {
            ty: id,
            value: refl,
        });
        let proof = arena.insert(Term::Lambda {
            domain: carrier,
            body: squash_val,
        });
        let unit = arena.insert(Term::Box { ty: proposition });
        let witness = arena.insert(Term::BoxIntro {
            ty: proposition,
            value: proof,
        });
        (unit, witness)
    }

    fn checked_context(
        arena: &mut TermArena,
        declarations: &[Declaration],
        level_arity: u32,
    ) -> Context {
        let signature = check_signature(
            arena,
            declarations,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .expect("signature must check");
        Context::with_level_arity(level_arity).with_signature(signature)
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

    fn checked_signature(
        arena: &mut TermArena,
        declarations: &[Declaration],
        role: &str,
    ) -> Signature {
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let result = check_signature(arena, declarations, &mut budget);
        eprintln!(
            "{role} signature: {} kernel steps",
            DEFAULT_CONVERSION_STEPS - budget.remaining()
        );
        result.unwrap_or_else(|error| panic!("{role} signature: {error:?}"))
    }

    #[test]
    fn shared_high_universe_constraint_is_lifted_without_assuming_its_truth() {
        let mut arena = TermArena::new();
        let mut declarations = Vec::new();
        let (a_name, x_name, y_name) = (71, 72, 73);
        let a = Key::Named(a_name);
        let sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let field = lam(a, S::Closed(sort), S::Var(a))
            .closed(&mut arena)
            .unwrap();
        let mut terms = Terms::default();
        let unit = terms.unit().closed(&mut arena).unwrap();
        let unit_value = terms.unit_value().closed(&mut arena).unwrap();
        let a_var = arena.insert(Term::Variable(2));
        let x_var = arena.insert(Term::Variable(1));
        let y_var = arena.insert(Term::Variable(0));
        let id_body = arena.insert(Term::Id {
            ty: a_var,
            left: x_var,
            right: y_var,
        });
        let a_var_1 = arena.insert(Term::Variable(1));
        let a_var_0 = arena.insert(Term::Variable(0));
        let close_axy = |arena: &mut TermArena, body: TermHandle| {
            let body = arena.insert(Term::Lambda {
                domain: a_var_1,
                body,
            });
            let body = arena.insert(Term::Lambda {
                domain: a_var_0,
                body,
            });
            arena.insert(Term::Lambda { domain: sort, body })
        };
        let high = close_axy(&mut arena, id_body);
        let low = close_axy(&mut arena, unit);
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
        assert_eq!(declared.family_level, Level::Parameter(0));
        let signature = checked_signature(&mut arena, &declarations, "high-universe constraint");
        let two = arena.insert(Term::Two);
        let zero = arena.insert(Term::TwoZero);
        let one = arena.insert(Term::TwoOne);
        let instance = declared
            .at_levels(&mut arena, &[two], &[Level::Constant(0)])
            .unwrap();
        let low_val = instance
            .constructor(
                &mut arena,
                ConstructorArguments {
                    case: 1,
                    fields: &[zero, one],
                    payload: &[],
                    constraints: unit_value,
                    children: &[],
                },
            )
            .unwrap();
        let family_ty = instance.family_type(&mut arena, low_val.index);
        checked(
            &mut arena,
            &Context::empty().with_signature(signature.clone()),
            low_val.value,
            family_ty,
            "low case value in high-universe family",
        );
        // The other case's Id Two zero one is uninhabited; supplying unit_value
        // for the high case must be rejected.
        let bad_high = instance
            .constructor(
                &mut arena,
                ConstructorArguments {
                    case: 0,
                    fields: &[zero, one],
                    payload: &[],
                    constraints: unit_value,
                    children: &[],
                },
            )
            .unwrap();
        assert!(
            check_type(
                &mut arena,
                &Context::empty().with_signature(signature),
                bad_high.value,
                family_ty,
                &mut Budget::default()
            )
            .is_err()
        );

        // An unused high-universe parameter does not raise the inductive's universe.
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
        assert_eq!(phantom.family_level, Level::Constant(0));
        checked_signature(&mut arena, &declarations, "phantom parameter");
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
        let mut declarations = Vec::new();
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
        assert_eq!(declared.family_level.parameter_arity(), 1);
        let instance = declared.at(&mut arena, &[]).unwrap();

        // Build the low case directly in the native inductive without padding.
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
        let constructor_type = instance.family_type(&mut arena, built.index);

        // The result itself lives at the generalized level. Returning the
        // low case's actual field proves iota reduction extracts it directly.
        let (result_unit, result_witness) = level_unit(&mut arena, carrier);
        let result_type = arena.insert(Term::Sigma {
            domain: result_unit,
            codomain: two,
        });
        let invariant = arena.insert(Term::Lambda {
            domain: instance.index_type,
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
            let context = context(&declarations, 1);
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
            .induction(&mut arena, &declarations, invariant, &lemmas)
            .unwrap();
        let signature = checked_signature(&mut arena, &declarations, "lifted");
        let context = Context::with_level_arity(1).with_signature(signature.clone());
        for (term, ty, role) in [
            (built.value, constructor_type, "lifted constructor"),
            (induction.theorem, induction.theorem_type, "lifted theorem"),
        ] {
            checked(&mut arena, &context, term, ty, role);
        }
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
        let declarations = Vec::new();
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
            let ctx = context(&declarations, 0);
            let ty = infer_type(
                &mut arena,
                &ctx,
                body,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            let expected = instance.lemma_type(&mut arena, invariant, ordinal).unwrap();
            checked(
                &mut arena,
                &ctx,
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
            .induction(&mut arena, &declarations, invariant, &lemmas)
            .unwrap();
        let signature = checked_signature(&mut arena, &declarations, "two-indexed");
        let ctx = Context::empty().with_signature(signature);
        checked(
            &mut arena,
            &ctx,
            induction.theorem,
            induction.theorem_type,
            "two-indexed theorem",
        );
        let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        assert!(
            check_type(
                &mut arena,
                &ctx,
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
            let mut ctx = context(&self.declarations, 0);
            for _ in 0..5 {
                ctx = ctx.extend(self.scalar);
            }
            assert!(
                convertible(
                    arena,
                    &ctx,
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
            let ctx = context(&self.declarations, 0);
            let inferred = infer_type(
                arena,
                &ctx,
                compact,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .unwrap();
            let raw_type =
                infer_type(arena, &ctx, raw, &mut Budget::new(DEFAULT_CONVERSION_STEPS)).unwrap();
            assert_eq!(
                judgment_assumption_closure(arena, ctx.signature(), &[raw, raw_type]),
                judgment_assumption_closure(arena, ctx.signature(), &[compact, inferred]),
            );
            assert!(
                convertible(
                    arena,
                    &ctx,
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
            let mut declarations = Vec::new();
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
        let (zero, one, ten, eighty) = (vector.zero, vector.one, vector.ten, vector.eighty);

        // (a) the family at an index, and (b) its constructor values.
        let arena = &mut vector.arena;
        let zero_zero = arena.insert(Term::Pair {
            first: zero,
            second: zero,
        });
        let at_zero = instance.family_type(arena, zero_zero);
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
        let ctx = checked_context(&mut vector.arena, &vector.declarations, 0);
        checked(
            &mut vector.arena,
            &ctx,
            nil.value,
            at_zero,
            "indexed vector Nil at (0, 0)",
        );
        let one_one = vector.arena.insert(Term::Pair {
            first: one,
            second: one,
        });
        let at_one = instance.family_type(&mut vector.arena, one_one);
        checked(
            &mut vector.arena,
            &ctx,
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
            let Term::Lambda { body, .. } = arena.get(authored) else {
                unreachable!()
            };
            let remaining = substitute(arena, body, length);
            let Term::Lambda { body, .. } = arena.get(remaining) else {
                unreachable!()
            };
            let normalized = substitute(arena, body, total);
            let ctx = context(&vector.declarations, 0).extend(instance.index_type);
            let sort = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
            assert!(
                convertible(
                    arena,
                    &ctx,
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
            domain: instance.index_type,
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
            .induction(&mut vector.arena, &vector.declarations, invariant, &lemmas)
            .unwrap();
        let signature =
            checked_signature(&mut vector.arena, &vector.declarations, "indexed vector");
        let ctx = Context::empty().with_signature(signature);
        checked(
            &mut vector.arena,
            &ctx,
            induction.theorem,
            induction.theorem_type,
            "indexed vector theorem",
        );
        let applied = vector
            .arena
            .apply_arguments(induction.theorem, &[cons.index, cons.value]);
        let goal = apply(&mut vector.arena, invariant, cons.index);
        checked(
            &mut vector.arena,
            &ctx,
            applied,
            goal,
            "indexed vector theorem at Cons",
        );

        // A closed specialization uses direct evidence instead of the recursor.
        let arena = &mut vector.arena;
        let closed_goal = binary(arena, vector.less_or_equal, ten, eighty);
        let closed_proof = assume(arena, &mut vector.declarations, closed_goal);
        let arena = &mut vector.arena;
        let closed_invariant = arena.insert(Term::Lambda {
            domain: instance.index_type,
            body: closed_goal,
        });
        let (theorem, ty) =
            instance.constant_invariant_theorem(arena, closed_invariant, closed_proof);
        let ctx = checked_context(arena, &vector.declarations, 0);
        checked(
            arena,
            &ctx,
            theorem,
            ty,
            "indexed vector closed specialization",
        );
        let false_goal = binary(arena, vector.less_or_equal, eighty, ten);
        let false_invariant = arena.insert(Term::Lambda {
            domain: instance.index_type,
            body: false_goal,
        });
        let (_, false_type) =
            instance.constant_invariant_theorem(arena, false_invariant, closed_proof);
        assert!(
            check_type(
                arena,
                &ctx,
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
        let ctx = checked_context(&mut vector.arena, &vector.declarations, 0);
        let right_type = instance.family_type(&mut vector.arena, right.index);
        checked(
            &mut vector.arena,
            &ctx,
            right.value,
            right_type,
            "Cons with its tail at (0, 0)",
        );
        let wrong_type = instance.family_type(&mut vector.arena, wrong.index);
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let refused = check_type(
            &mut vector.arena,
            &ctx,
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
    }
}
