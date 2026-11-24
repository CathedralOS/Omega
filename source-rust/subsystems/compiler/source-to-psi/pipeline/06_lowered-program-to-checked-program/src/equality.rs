//! Rewrite a goal with the equality terms available at that program point.
//! Each visited expression carries a proof from the original left endpoint.

use proof_admission::PlainData;
use proof_admission::{
    Budget, Context, CoreError, Term, TermArena, TermHandle, infer_type, shift, weak_head_normalize,
};
use terminal_check_tables::mathematics::{FunctionCases, SignatureEntry, SignatureEntryKind};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SearchError {
    NoProof,
    Budget,
    Kernel(CoreError),
}

impl From<CoreError> for SearchError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::StepCeiling => Self::Budget,
            error => Self::Kernel(error),
        }
    }
}

#[derive(Clone, Copy)]
struct Equality {
    carrier: TermHandle,
    from: TermHandle,
    to: TermHandle,
    proof: TermHandle,
}

impl Equality {
    /// Eliminate this equality with a conclusion under two new binders:
    /// the destination and the proof that the source equals it.
    fn eliminate(
        self,
        arena: &mut TermArena,
        conclusion: TermHandle,
        base: TermHandle,
    ) -> TermHandle {
        let ty = shift(arena, self.carrier, 0, 1);
        let left = shift(arena, self.from, 0, 1);
        let right = arena.insert(Term::Variable(0));
        let domain = arena.insert(Term::Id { ty, left, right });
        let body = arena.insert(Term::Lambda {
            domain,
            body: conclusion,
        });
        let motive = arena.insert(Term::Lambda {
            domain: self.carrier,
            body,
        });
        arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: self.to,
            proof: self.proof,
        })
    }

    fn reversed(self, arena: &mut TermArena) -> Self {
        let ty = shift(arena, self.carrier, 0, 2);
        let left = arena.insert(Term::Variable(1));
        let right = shift(arena, self.from, 0, 2);
        let conclusion = arena.insert(Term::Id { ty, left, right });
        let base = arena.insert(Term::Refl {
            ty: self.carrier,
            value: self.from,
        });
        Self {
            from: self.to,
            to: self.from,
            proof: self.eliminate(arena, conclusion, base),
            ..self
        }
    }
}

/// One step from a term into one of its subterms: the term, which slot was
/// entered, and how many binders of the whole expression lie above the term.
#[derive(Clone, Copy)]
struct Surrounding {
    parent: TermHandle,
    slot: usize,
    depth: u32,
}

/// Rebuild the expression around `value`, as seen under `binders` more
/// binders than the expression itself.
fn plug(
    arena: &mut TermArena,
    path: &[Surrounding],
    mut value: TermHandle,
    binders: u32,
) -> TermHandle {
    for frame in path.iter().rev() {
        let node = arena.get(frame.parent);
        let mut children = Vec::with_capacity(4);
        for (slot, (child, bound)) in node.bound_children().enumerate() {
            children.push(if slot == frame.slot {
                value
            } else {
                shift(arena, child, frame.depth + bound, binders)
            });
        }
        value = arena.insert(node.rebuilt(&children));
    }
    value
}

fn charge(remaining: &mut usize) -> Result<(), SearchError> {
    *remaining = remaining.checked_sub(1).ok_or(SearchError::Budget)?;
    Ok(())
}

/// How a constant of the signature takes part in normalization.
#[derive(Clone)]
enum Definition {
    /// No body, a type, or a part of a data's encoding: the constant stays
    /// as written.
    Opaque,
    /// A case of the data at entry `data`. It stays as written, and a
    /// function by cases computes on it once it has all `arity` fields.
    Constructor { data: u32, case: u32, arity: u32 },
    /// A function defined by cases on one argument. It computes only when
    /// that argument is a case, so everywhere else its application stays
    /// folded. `induction` is the entry of the data's induction principle.
    Recursion {
        cases: FunctionCases,
        induction: u32,
    },
    /// Any other body: an abbreviation, unfolded where it is met.
    Abbreviation,
}

fn spine(arena: &TermArena, term: TermHandle) -> (TermHandle, Vec<TermHandle>) {
    let mut head = term;
    let mut arguments = Vec::new();
    while let Term::Apply { function, argument } = arena.get(head) {
        arguments.push(argument);
        head = function;
    }
    arguments.reverse();
    (head, arguments)
}

/// What each declaration of a signature is to the normalizer, and the shape
/// of each data declaration by its entry.
pub(crate) struct Definitions(Vec<Definition>, Vec<(u32, PlainData)>);

impl Definitions {
    /// A signature with no data: every body is an abbreviation.
    pub(crate) fn plain(context: &Context) -> Self {
        Self(
            context
                .signature()
                .declarations()
                .iter()
                .map(|declaration| match declaration.body {
                    Some(_) => Definition::Abbreviation,
                    None => Definition::Opaque,
                })
                .collect(),
            Vec::new(),
        )
    }

    /// The compilation's mathematical signature, as stage 05 described each
    /// entry. Nothing is read off the shape of a term.
    pub(crate) fn of_signature(entries: &[SignatureEntry]) -> Self {
        let shape = |data: u32| match entries.get(data as usize).map(|entry| &entry.kind) {
            Some(SignatureEntryKind::Data { shape, .. }) => Some(shape),
            _ => None,
        };
        Self(
            entries
                .iter()
                .map(|entry| match &entry.kind {
                    SignatureEntryKind::Data { .. }
                    | SignatureEntryKind::Encoding { .. }
                    | SignatureEntryKind::Statement { .. } => Definition::Opaque,
                    SignatureEntryKind::Constructor { data, case } => match shape(*data) {
                        Some(shape) => Definition::Constructor {
                            data: *data,
                            case: *case,
                            arity: shape.arity(*case as usize),
                        },
                        None => Definition::Opaque,
                    },
                    SignatureEntryKind::Function {
                        cases: Some(cases), ..
                    } => match shape(cases.data) {
                        Some(shape) => Definition::Recursion {
                            cases: cases.clone(),
                            induction: shape.induction_entry(cases.data),
                        },
                        None => Definition::Opaque,
                    },
                    SignatureEntryKind::Function { cases: None, .. } => Definition::Abbreviation,
                })
                .collect(),
            entries
                .iter()
                .enumerate()
                .filter_map(|(entry, described)| match &described.kind {
                    SignatureEntryKind::Data { shape, .. } => Some((entry as u32, shape.clone())),
                    _ => None,
                })
                .collect(),
        )
    }

    fn get(&self, declaration: u32) -> &Definition {
        self.0
            .get(declaration as usize)
            .unwrap_or(&Definition::Opaque)
    }

    /// The case and fields of a term that is a case of `data` applied to
    /// all of its fields.
    fn constructed(
        &self,
        arena: &TermArena,
        data: u32,
        term: TermHandle,
    ) -> Option<(usize, Vec<TermHandle>)> {
        let (head, arguments) = spine(arena, term);
        let Term::Constant { declaration, .. } = arena.get(head) else {
            return None;
        };
        match self.get(declaration) {
            Definition::Constructor {
                data: of,
                case,
                arity,
            } if *of == data && *arity as usize == arguments.len() => {
                Some((*case as usize, arguments))
            }
            _ => None,
        }
    }

    /// Both sides of an equality as cases of one data declaration, each
    /// applied to all of its fields.
    #[allow(clippy::type_complexity)]
    fn cases(
        &self,
        arena: &TermArena,
        equality: Equality,
    ) -> Option<(
        u32,
        &PlainData,
        (usize, Vec<TermHandle>),
        (usize, Vec<TermHandle>),
    )> {
        let (head, _) = spine(arena, equality.from);
        let Term::Constant { declaration, .. } = arena.get(head) else {
            return None;
        };
        let Definition::Constructor { data, .. } = self.get(declaration) else {
            return None;
        };
        let data = *data;
        let (_, shape) = self.1.iter().find(|(entry, _)| *entry == data)?;
        Some((
            data,
            shape,
            self.constructed(arena, data, equality.from)?,
            self.constructed(arena, data, equality.to)?,
        ))
    }

    /// A proof of `goal` from an equality of two different cases, which no
    /// value satisfies. A function sends the left side's case to one element
    /// of `Two` and every other case to the other, so the equality becomes
    /// an equality of those two elements, and a family of types that is
    /// inhabited at the first and is `goal` at the second carries a proof
    /// across it. `None` when the two sides are not different cases.
    fn absurd(
        &self,
        arena: &mut TermArena,
        equality: Equality,
        goal: TermHandle,
    ) -> Option<TermHandle> {
        let (data, shape, (case, _), (other, _)) = self.cases(arena, equality)?;
        if case == other {
            return None;
        }
        let shape = shape.clone();
        let ty = arena.insert(Term::Constant {
            declaration: data,
            levels: Vec::new(),
        });
        let two = arena.insert(Term::Two);
        let (zero, one) = (arena.insert(Term::TwoZero), arena.insert(Term::TwoOne));
        let motive = arena.insert(Term::Lambda {
            domain: ty,
            body: two,
        });
        let branches: Vec<_> = (0..shape.cases())
            .map(|each| {
                let body = if each == case { zero } else { one };
                shape.branch(arena, data, motive, each, body)
            })
            .collect();
        let subject = arena.insert(Term::Variable(0));
        let selected = shape.induct(arena, data, motive, &branches, subject);
        let discriminator = arena.insert(Term::Lambda {
            domain: ty,
            body: selected,
        });
        let apply = |arena: &mut TermArena, function, argument| {
            arena.insert(Term::Apply { function, argument })
        };
        // The equality of the two sides' elements of `Two`.
        let from = shift(arena, equality.from, 0, 2);
        let left = apply(arena, discriminator, from);
        let destination = arena.insert(Term::Variable(1));
        let right = apply(arena, discriminator, destination);
        let conclusion = arena.insert(Term::Id {
            ty: two,
            left,
            right,
        });
        let value = apply(arena, discriminator, equality.from);
        let base = arena.insert(Term::Refl { ty: two, value });
        let distinct = Equality {
            carrier: two,
            from: value,
            to: apply(arena, discriminator, equality.to),
            proof: equality.eliminate(arena, conclusion, base),
        };
        // The family: an inhabited type at the first element, the goal at
        // the second.
        let universe = arena.insert(Term::Sort(proof_admission::Sort::Type(
            proof_admission::Level::Constant(0),
        )));
        let family_motive = arena.insert(Term::Lambda {
            domain: two,
            body: universe,
        });
        let inhabited = arena.insert(Term::Id {
            ty: two,
            left: zero,
            right: zero,
        });
        let goal_under = shift(arena, goal, 0, 2);
        let scrutinee = arena.insert(Term::Variable(1));
        let conclusion = arena.insert(Term::CaseTwo {
            motive: family_motive,
            zero_branch: inhabited,
            one_branch: goal_under,
            scrutinee,
        });
        let base = arena.insert(Term::Refl {
            ty: two,
            value: zero,
        });
        Some(distinct.eliminate(arena, conclusion, base))
    }

    /// What a constructor's injectivity gives from an equality of two values
    /// of one case: the equality of each pair of fields. The proof applies
    /// to both sides a function that returns that field of the case and
    /// leaves every other case as it is, so it computes to the field on
    /// each; the kernel checks that computation where the result is used.
    fn injective(&self, arena: &mut TermArena, equality: Equality) -> Vec<Equality> {
        let Some((data, shape, (case, left), (right_case, right))) = self.cases(arena, equality)
        else {
            return Vec::new();
        };
        if right_case != case {
            return Vec::new();
        }
        let ty = arena.insert(Term::Constant {
            declaration: data,
            levels: Vec::new(),
        });
        let motive = arena.insert(Term::Lambda {
            domain: ty,
            body: ty,
        });
        let mut derived = Vec::with_capacity(left.len());
        for field in 0..shape.arity(case) {
            let branches: Vec<_> = (0..shape.cases())
                .map(|other| {
                    let body = if other == case {
                        shape.field(arena, other, field, 0)
                    } else {
                        shape.case_value(arena, data, other, 0)
                    };
                    shape.branch(arena, data, motive, other, body)
                })
                .collect();
            let subject = arena.insert(Term::Variable(0));
            let selected = shape.induct(arena, data, motive, &branches, subject);
            let projection = arena.insert(Term::Lambda {
                domain: ty,
                body: selected,
            });
            let apply = |arena: &mut TermArena, argument| {
                arena.insert(Term::Apply {
                    function: projection,
                    argument,
                })
            };
            let from = shift(arena, equality.from, 0, 2);
            let left_side = apply(arena, from);
            let destination = arena.insert(Term::Variable(1));
            let right_side = apply(arena, destination);
            let conclusion = arena.insert(Term::Id {
                ty,
                left: left_side,
                right: right_side,
            });
            let value = apply(arena, equality.from);
            let base = arena.insert(Term::Refl { ty, value });
            derived.push(Equality {
                carrier: ty,
                from: left[field as usize],
                to: right[field as usize],
                proof: equality.eliminate(arena, conclusion, base),
            });
        }
        derived
    }

    /// An induction applied to a motive, its branches and a subject is the
    /// function stage 05 built from exactly those: its entry, and the
    /// function's arguments with the subject back in its place.
    fn folded(
        &self,
        arena: &TermArena,
        induction: u32,
        arguments: &[TermHandle],
    ) -> Option<(u32, Vec<TermHandle>)> {
        self.0.iter().enumerate().find_map(|(index, definition)| {
            let Definition::Recursion {
                cases,
                induction: of,
            } = definition
            else {
                return None;
            };
            let count = cases.branches.len();
            if *of != induction || arguments.len() < count + 2 {
                return None;
            }
            let rest = &arguments[count + 2..];
            (rest.len() + 1 >= cases.arity as usize
                && arena.structurally_equal(arguments[0], cases.motive)
                && cases
                    .branches
                    .iter()
                    .zip(&arguments[1..=count])
                    .all(|(&known, &given)| arena.structurally_equal(known, given)))
            .then(|| {
                let mut restored = rest.to_vec();
                restored.insert(cases.subject as usize, arguments[count + 1]);
                (index as u32, restored)
            })
        })
    }
}

/// The term after every computation the kernel can carry out, with each
/// function defined by cases left as an application wherever its argument is
/// not a case. Over a case the function computes one step: the case's branch
/// over its fields, the function itself at each field, and the other
/// arguments. Two terms the kernel converts by computation alone get one
/// normal form, and that form still shows which operations are applied to
/// what, which the search and the reordering below read. The kernel checks
/// the proof built from these forms against the original goal, so a step the
/// kernel would not take cannot make a goal provable.
fn normal(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    term: TermHandle,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    let signature = context.signature();
    let head = proof_admission::weak_head_normalize_retaining(
        arena,
        signature,
        term,
        conversion,
        &|declaration| !matches!(definitions.get(declaration), Definition::Abbreviation),
    )?;
    let (function, arguments) = spine(arena, head);
    let (function, arguments) = match arena.get(function) {
        Term::Constant { declaration, .. } => {
            match definitions.folded(arena, declaration, &arguments) {
                Some((folded, restored)) => {
                    let constant = arena.insert(Term::Constant {
                        declaration: folded,
                        levels: Vec::new(),
                    });
                    (constant, restored)
                }
                None => (function, arguments),
            }
        }
        _ => (function, arguments),
    };
    if let Term::Constant { declaration, .. } = arena.get(function)
        && let Definition::Recursion { cases, induction } = definitions.get(declaration)
        && arguments.len() >= cases.arity as usize
    {
        let mut operands = Vec::with_capacity(arguments.len());
        for &argument in &arguments {
            operands.push(normal(arena, context, definitions, argument, conversion)?);
        }
        let subject = cases.subject as usize;
        let Some((case, fields)) = definitions.constructed(arena, cases.data, operands[subject])
        else {
            return Ok(arena.apply_arguments(function, &operands));
        };
        let recursor = arena.insert(Term::Constant {
            declaration: *induction,
            levels: Vec::new(),
        });
        let mut recursor = arena.insert(Term::Apply {
            function: recursor,
            argument: cases.motive,
        });
        for &branch in &cases.branches {
            recursor = arena.insert(Term::Apply {
                function: recursor,
                argument: branch,
            });
        }
        let mut applied = arena.apply_arguments(cases.branches[case], &fields);
        for &field in &fields {
            let at_field = arena.insert(Term::Apply {
                function: recursor,
                argument: field,
            });
            applied = arena.insert(Term::Apply {
                function: applied,
                argument: at_field,
            });
        }
        operands.remove(subject);
        let applied = arena.apply_arguments(applied, &operands);
        let stepped = proof_admission::weak_head_normalize_retaining(
            arena,
            signature,
            applied,
            conversion,
            &|_| true,
        )?;
        return normal(arena, context, definitions, stepped, conversion);
    }
    if !arguments.is_empty() {
        let function = match arena.get(function) {
            Term::Constant { .. } | Term::Variable(_) => function,
            _ => normal_children(arena, context, definitions, function, conversion)?,
        };
        let mut operands = Vec::with_capacity(arguments.len());
        for &argument in &arguments {
            operands.push(normal(arena, context, definitions, argument, conversion)?);
        }
        return Ok(arena.apply_arguments(function, &operands));
    }
    normal_children(arena, context, definitions, function, conversion)
}

fn normal_children(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    term: TermHandle,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    let node = arena.get(term);
    let mut children = Vec::with_capacity(4);
    let mut changed = false;
    for child in node.children() {
        let normal = normal(arena, context, definitions, child, conversion)?;
        changed |= normal != child;
        children.push(normal);
    }
    Ok(if changed {
        arena.insert(node.rebuilt(&children))
    } else {
        term
    })
}

/// A binary operation with the proved laws that let the search regroup and
/// reorder its operands. Each law is a closed term of the signature; using
/// it puts it in the proof, so the certificate names what it relied on.
#[derive(Clone, Copy)]
pub(crate) struct License {
    pub operation: TermHandle,
    pub carrier: TermHandle,
    /// `op(op(x, y), z) = op(x, op(y, z))` for all operands, or its mirror
    /// when the flag is false.
    pub associates: Option<(TermHandle, bool)>,
    /// `op(x, y) = op(y, x)` for all operands.
    pub commutes: Option<TermHandle>,
}

/// A proved law that one binary operation distributes over another. Using
/// it puts it in the proof, like a [`License`]'s laws.
#[derive(Clone, Copy)]
pub(crate) struct Distribution {
    /// The operation that is expanded: multiplication in a semiring.
    pub outer: TermHandle,
    /// The operation it distributes over: addition in a semiring.
    pub inner: TermHandle,
    pub carrier: TermHandle,
    pub law: TermHandle,
    /// `outer(x, inner(y, z)) = inner(outer(x, y), outer(x, z))` when true;
    /// `outer(inner(x, y), z) = inner(outer(x, z), outer(y, z))` when false.
    pub from_left: bool,
    /// Whether the law's statement has the product on its left side.
    pub product_first: bool,
}

/// The laws a proof may rearrange under without citing them: each licensed
/// operation's associativity and commutativity, and distributivity of one
/// licensed operation over another.
#[derive(Clone, Copy, Default)]
pub(crate) struct Licenses<'l> {
    pub operations: &'l [License],
    pub distributions: &'l [Distribution],
}

/// The most summands one product may expand to, counted before the
/// expansion is built: the default of the automatic expansion strategy
/// (`wiki/spec/proofs/contracts.md`, "Proved index compatibility").
/// A product that would expand further stops the proof with
/// [`SearchError::Budget`], which says nothing about whether the goal holds.
pub(crate) const MAX_SUMMANDS: usize = 64;

/// One rewrite of the term at the subterm reached by taking these child
/// slots in turn.
struct Step {
    slots: Vec<usize>,
    rule: Equality,
}

/// The operation and the two operands of a binary application.
fn binary(arena: &TermArena, term: TermHandle) -> Option<(TermHandle, TermHandle, TermHandle)> {
    let Term::Apply {
        function,
        argument: second,
    } = arena.get(term)
    else {
        return None;
    };
    let Term::Apply {
        function: operation,
        argument: first,
    } = arena.get(function)
    else {
        return None;
    };
    Some((operation, first, second))
}

/// The two operands of `term` when it applies `operation`.
fn operands_of(
    arena: &TermArena,
    operation: TermHandle,
    term: TermHandle,
) -> Option<(TermHandle, TermHandle)> {
    binary(arena, term)
        .filter(|&(head, ..)| arena.structurally_equal(head, operation))
        .map(|(_, first, second)| (first, second))
}

/// Which operands of a product the licensed laws expand.
#[derive(Clone, Copy)]
enum Expands {
    /// Only a sum on the right, as a left distributivity law states it.
    Right,
    /// Only a sum on the left.
    Left,
    /// A sum on either side: both laws, or one law and commutativity.
    Both,
}

/// How many summands of `law.inner` the term expands to under `law` and
/// `expands`, or `None` past `usize`. The count walks the term once and
/// builds nothing.
fn summands(
    arena: &TermArena,
    law: &Distribution,
    expands: Expands,
    term: TermHandle,
) -> Option<usize> {
    let count = |term| summands(arena, law, expands, term);
    if let Some((first, second)) = operands_of(arena, law.inner, term) {
        return count(first)?.checked_add(count(second)?);
    }
    if let Some((first, second)) = operands_of(arena, law.outer, term) {
        return match expands {
            Expands::Right => count(second),
            Expands::Left => count(first),
            Expands::Both => count(first)?.checked_mul(count(second)?),
        };
    }
    Some(1)
}

/// Whether the term applies `operation` anywhere, beneath any term former.
fn mentions(arena: &TermArena, operation: TermHandle, term: TermHandle) -> bool {
    arena.structurally_equal(term, operation)
        || arena
            .get(term)
            .children()
            .any(|child| mentions(arena, operation, child))
}

/// The law at `x`, `y`, `z`, oriented from the product to its expansion.
fn distributivity(
    arena: &mut TermArena,
    law: &Distribution,
    x: TermHandle,
    y: TermHandle,
    z: TermHandle,
) -> Equality {
    let (product, sum) = if law.from_left {
        let inner = arena.apply_binary(law.inner, y, z);
        let left = arena.apply_binary(law.outer, x, y);
        let right = arena.apply_binary(law.outer, x, z);
        (
            arena.apply_binary(law.outer, x, inner),
            arena.apply_binary(law.inner, left, right),
        )
    } else {
        let inner = arena.apply_binary(law.inner, x, y);
        let left = arena.apply_binary(law.outer, x, z);
        let right = arena.apply_binary(law.outer, y, z);
        (
            arena.apply_binary(law.outer, inner, z),
            arena.apply_binary(law.inner, left, right),
        )
    };
    let proof = arena.apply_arguments(law.law, &[x, y, z]);
    if law.product_first {
        Equality {
            carrier: law.carrier,
            from: product,
            to: sum,
            proof,
        }
    } else {
        Equality {
            carrier: law.carrier,
            from: sum,
            to: product,
            proof,
        }
        .reversed(arena)
    }
}

/// The rewrites that expand the product at `term` by one licensed
/// distributivity law: the law itself where the sum is on the side the law
/// states, or a commutation first where the product's operation commutes and
/// the sum is on the other side. The product's full expansion is counted
/// before anything is built.
fn next_expansion(
    arena: &mut TermArena,
    licenses: &[License],
    distributions: &[Distribution],
    term: TermHandle,
    slots: &[usize],
) -> Result<Option<Vec<Step>>, SearchError> {
    let Some((operation, first, second)) = binary(arena, term) else {
        return Ok(None);
    };
    for law in distributions {
        if !arena.structurally_equal(law.outer, operation) {
            continue;
        }
        let (stated, other) = if law.from_left {
            (second, first)
        } else {
            (first, second)
        };
        let commutes = licenses
            .iter()
            .find(|license| arena.structurally_equal(license.operation, operation))
            .and_then(|license| license.commutes.map(|_| *license));
        let swap = if operands_of(arena, law.inner, stated).is_some() {
            None
        } else if operands_of(arena, law.inner, other).is_some()
            && let Some(license) = &commutes
            && let Some(swap) = commutativity(arena, license, first, second)
        {
            Some(swap)
        } else {
            continue;
        };
        let mirrored = distributions.iter().any(|other| {
            other.from_left != law.from_left
                && arena.structurally_equal(other.outer, law.outer)
                && arena.structurally_equal(other.inner, law.inner)
        });
        let expands = match (commutes.is_some() || mirrored, law.from_left) {
            (true, _) => Expands::Both,
            (false, true) => Expands::Right,
            (false, false) => Expands::Left,
        };
        if summands(arena, law, expands, term).is_none_or(|count| count > MAX_SUMMANDS) {
            return Err(SearchError::Budget);
        }
        let (first, second) = if swap.is_some() {
            (second, first)
        } else {
            (first, second)
        };
        let rule = if law.from_left {
            let (y, z) = operands_of(arena, law.inner, second).ok_or(SearchError::NoProof)?;
            distributivity(arena, law, first, y, z)
        } else {
            let (x, y) = operands_of(arena, law.inner, first).ok_or(SearchError::NoProof)?;
            distributivity(arena, law, x, y, second)
        };
        let mut steps = Vec::with_capacity(2);
        if let Some(rule) = swap {
            steps.push(Step {
                slots: slots.to_vec(),
                rule,
            });
        }
        steps.push(Step {
            slots: slots.to_vec(),
            rule,
        });
        return Ok(Some(steps));
    }
    Ok(None)
}

fn licensed<'l>(
    arena: &TermArena,
    licenses: &'l [License],
    term: TermHandle,
) -> Option<(&'l License, TermHandle, TermHandle)> {
    let (operation, first, second) = binary(arena, term)?;
    licenses
        .iter()
        .find(|license| arena.structurally_equal(license.operation, operation))
        .map(|license| (license, first, second))
}

/// A total order on terms that depends only on their structure, never on
/// where the arena happens to store them.
fn order(arena: &TermArena, left: TermHandle, right: TermHandle) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let rank = |term: &Term| match term {
        Term::Variable(_) => 0u8,
        Term::Constant { .. } => 1,
        Term::Apply { .. } => 2,
        Term::Sup { .. } => 3,
        Term::Lambda { .. } => 4,
        Term::Pi { .. } => 5,
        Term::IndW { .. } => 6,
        _ => 7,
    };
    let (a, b) = (arena.get(left), arena.get(right));
    let by_shape = rank(&a).cmp(&rank(&b)).then_with(|| match (&a, &b) {
        (Term::Variable(a), Term::Variable(b)) => a.cmp(b),
        (Term::Constant { declaration: a, .. }, Term::Constant { declaration: b, .. }) => a.cmp(b),
        _ => Ordering::Equal,
    });
    if by_shape != Ordering::Equal {
        return by_shape;
    }
    let mut pairs = a.children().zip(b.children());
    let by_children = pairs
        .by_ref()
        .map(|(a, b)| order(arena, a, b))
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal);
    by_children.then_with(|| a.children().count().cmp(&b.children().count()))
}

fn associativity(
    arena: &mut TermArena,
    license: &License,
    first: TermHandle,
    second: TermHandle,
    third: TermHandle,
) -> Option<Equality> {
    let (law, left_nested_first) = license.associates?;
    let inner = arena.apply_binary(license.operation, first, second);
    let left_nested = arena.apply_binary(license.operation, inner, third);
    let inner = arena.apply_binary(license.operation, second, third);
    let right_nested = arena.apply_binary(license.operation, first, inner);
    let proof = arena.apply_arguments(law, &[first, second, third]);
    let stated = Equality {
        carrier: license.carrier,
        from: if left_nested_first {
            left_nested
        } else {
            right_nested
        },
        to: if left_nested_first {
            right_nested
        } else {
            left_nested
        },
        proof,
    };
    Some(if left_nested_first {
        stated
    } else {
        stated.reversed(arena)
    })
}

fn commutativity(
    arena: &mut TermArena,
    license: &License,
    first: TermHandle,
    second: TermHandle,
) -> Option<Equality> {
    let law = license.commutes?;
    Some(Equality {
        carrier: license.carrier,
        from: arena.apply_binary(license.operation, first, second),
        to: arena.apply_binary(license.operation, second, first),
        proof: arena.apply_arguments(law, &[first, second]),
    })
}

/// The next rewrites toward the canonical form of a licensed operation's
/// chains: products expanded under the distributivity laws in
/// `distributions`, then each chain nested to the right and, where the
/// operation also commutes, with operands in `order`. One call returns one
/// expansion, one regrouping, or the three rewrites that swap two
/// neighbours. `None` means the term is canonical.
fn next_rearrangement(
    arena: &mut TermArena,
    licenses: &[License],
    distributions: &[Distribution],
    term: TermHandle,
    slots: &mut Vec<usize>,
) -> Result<Option<Vec<Step>>, SearchError> {
    use std::cmp::Ordering::Greater;
    if let Some(steps) = next_expansion(arena, licenses, distributions, term, slots)? {
        return Ok(Some(steps));
    }
    if let Some((license, first, second)) = licensed(arena, licenses, term) {
        let license = *license;
        // op(op(x, y), z) regroups to op(x, op(y, z)).
        if let Some((inner, x, y)) = licensed(arena, licenses, first)
            && arena.structurally_equal(inner.operation, license.operation)
            && let Some(rule) = associativity(arena, &license, x, y, second)
        {
            return Ok(Some(vec![Step {
                slots: slots.clone(),
                rule,
            }]));
        }
        if license.commutes.is_some() {
            let tail = licensed(arena, licenses, second)
                .filter(|(inner, ..)| arena.structurally_equal(inner.operation, license.operation))
                .map(|(_, next, rest)| (next, rest));
            match tail {
                // op(x, op(y, rest)) with x after y: regroup to the left,
                // commute the pair, regroup back.
                Some((next, rest)) => {
                    if order(arena, first, next) == Greater
                        && let Some(regroup) = associativity(arena, &license, first, next, rest)
                        && let Some(swap) = commutativity(arena, &license, first, next)
                        && let Some(back) = associativity(arena, &license, next, first, rest)
                    {
                        let mut pair = slots.clone();
                        pair.extend([0, 1]);
                        return Ok(Some(vec![
                            Step {
                                slots: slots.clone(),
                                rule: regroup.reversed(arena),
                            },
                            Step {
                                slots: pair,
                                rule: swap,
                            },
                            Step {
                                slots: slots.clone(),
                                rule: back,
                            },
                        ]));
                    }
                }
                None => {
                    if order(arena, first, second) == Greater
                        && let Some(rule) = commutativity(arena, &license, first, second)
                    {
                        return Ok(Some(vec![Step {
                            slots: slots.clone(),
                            rule,
                        }]));
                    }
                }
            }
        }
    }
    // Operands are rearranged wherever they sit beneath applications. A
    // binder is not entered: a law is stated outside it.
    if let Term::Apply { function, argument } = arena.get(term) {
        for (slot, child) in [function, argument].into_iter().enumerate() {
            slots.push(slot);
            let found = next_rearrangement(arena, licenses, distributions, child, slots);
            slots.pop();
            if !matches!(found, Ok(None)) {
                return found;
            }
        }
    }
    Ok(None)
}

/// The path of surrounding terms for a list of child slots.
fn path_to(arena: &TermArena, term: TermHandle, slots: &[usize]) -> Option<Vec<Surrounding>> {
    let mut path = Vec::with_capacity(slots.len());
    let mut focus = term;
    for &slot in slots {
        path.push(Surrounding {
            parent: focus,
            slot,
            depth: 0,
        });
        focus = arena.get(focus).children().nth(slot)?;
    }
    Some(path)
}

/// The canonical form of the term under the licenses, and the rewrites that
/// reach it in order. Every rewrite is charged to `search_steps`, so a
/// rearrangement that does not finish within it stops with
/// [`SearchError::Budget`].
fn rearrange(
    arena: &mut TermArena,
    licenses: &[License],
    distributions: &[Distribution],
    mut term: TermHandle,
    search_steps: &mut usize,
) -> Result<(TermHandle, Vec<Step>), SearchError> {
    let mut taken = Vec::new();
    if licenses.is_empty() && distributions.is_empty() {
        return Ok((term, taken));
    }
    while let Some(steps) =
        next_rearrangement(arena, licenses, distributions, term, &mut Vec::new())?
    {
        for step in steps {
            charge(search_steps)?;
            let path = path_to(arena, term, &step.slots).ok_or(SearchError::NoProof)?;
            term = plug(arena, &path, step.rule.to, 0);
            taken.push(step);
        }
    }
    Ok((term, taken))
}

/// Extend a proof that `left` equals `term` by one rewrite of `term`.
fn rewrite(
    arena: &mut TermArena,
    ty: TermHandle,
    left: TermHandle,
    term: TermHandle,
    base: TermHandle,
    slots: &[usize],
    rule: Equality,
) -> Result<(TermHandle, TermHandle), SearchError> {
    let path = path_to(arena, term, slots).ok_or(SearchError::NoProof)?;
    let rewritten = plug(arena, &path, rule.to, 0);
    let destination = arena.insert(Term::Variable(1));
    let under_binders = plug(arena, &path, destination, 2);
    let carrier = shift(arena, ty, 0, 2);
    let start = shift(arena, left, 0, 2);
    let conclusion = arena.insert(Term::Id {
        ty: carrier,
        left: start,
        right: under_binders,
    });
    Ok((rewritten, rule.eliminate(arena, conclusion, base)))
}

/// Citations are instantiated terms from the checked signature. Inferring
/// their types keeps both endpoints tied to the actual evidence supplied.
pub(crate) fn prove(
    arena: &mut TermArena,
    context: &Context,
    goal: TermHandle,
    citations: &[TermHandle],
    search_steps: usize,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    let definitions = Definitions::plain(context);
    prove_under(
        arena,
        context,
        &definitions,
        goal,
        citations,
        &[],
        search_steps,
        conversion,
    )
}

/// [`prove_licensed`] with no distributivity law.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_under(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    goal: TermHandle,
    citations: &[TermHandle],
    licenses: &[License],
    search_steps: usize,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    prove_licensed(
        arena,
        context,
        definitions,
        goal,
        citations,
        Licenses {
            operations: licenses,
            distributions: &[],
        },
        search_steps,
        conversion,
    )
}

/// The distributivity laws each attempt may expand products under, in the
/// order they are tried. The first attempt expands nothing, so a goal that
/// regrouping and reordering already prove gets the same proof it had
/// without the laws. When two operations each distribute over the other,
/// expanding both ways would undo itself, so each attempt takes one of the
/// two directions and every other law: neither law disables the other.
fn expansion_attempts(arena: &TermArena, distributions: &[Distribution]) -> Vec<Vec<Distribution>> {
    let mut attempts = vec![Vec::new()];
    if distributions.is_empty() {
        return attempts;
    }
    let same = |a: &Distribution, b: &Distribution| {
        arena.structurally_equal(a.outer, b.outer) && arena.structurally_equal(a.inner, b.inner)
    };
    let mutual = |law: &Distribution| {
        distributions.iter().any(|other| {
            arena.structurally_equal(other.outer, law.inner)
                && arena.structurally_equal(other.inner, law.outer)
        })
    };
    let one_way: Vec<_> = distributions
        .iter()
        .copied()
        .filter(|law| !mutual(law))
        .collect();
    let mut directions: Vec<Distribution> = Vec::new();
    for law in distributions.iter().filter(|law| mutual(law)) {
        if !directions.iter().any(|known| same(known, law)) {
            directions.push(*law);
        }
    }
    if directions.is_empty() {
        attempts.push(one_way);
        return attempts;
    }
    for direction in &directions {
        let mut attempt = one_way.clone();
        attempt.extend(
            distributions
                .iter()
                .copied()
                .filter(|law| same(law, direction)),
        );
        attempts.push(attempt);
    }
    attempts
}

/// Both sides of the goal and both endpoints of every citation are
/// normalized first. The search then rewrites one side with the citations,
/// beneath any term former and any binder. A term it reaches proves the goal
/// when it and the other side have one canonical form under the licensed
/// laws: products expanded under distributivity, then each operation's
/// operands regrouped and reordered. Every rewrite, a citation's or a law's,
/// is an identity elimination, so the result is one kernel term from the
/// left side to the right. The term is a candidate: the caller has the
/// kernel check it, once, with everything else it is part of.
///
/// Expansion is tried only after the search without it finds no proof, once
/// per entry of [`expansion_attempts`], and only when a product it could
/// expand appears in the goal or a citation. All attempts share
/// `search_steps`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_licensed(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    goal: TermHandle,
    citations: &[TermHandle],
    licenses: Licenses<'_>,
    mut search_steps: usize,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    let Term::Id { ty, left, right } = arena.get(goal) else {
        return Err(SearchError::NoProof);
    };
    let left = normal(arena, context, definitions, left, conversion)?;
    let right = normal(arena, context, definitions, right, conversion)?;
    let reflexivity = arena.insert(Term::Refl { ty, value: left });
    if arena.structurally_equal(left, right) {
        return Ok(reflexivity);
    }
    let goal = Goal {
        stated: goal,
        ty,
        left,
        right,
        reflexivity,
    };
    let mut rules = None;
    for distributions in expansion_attempts(arena, licenses.distributions) {
        if !distributions.is_empty() {
            let expands = |arena: &TermArena, term| {
                distributions
                    .iter()
                    .any(|law| mentions(arena, law.outer, term))
            };
            let cited = rules
                .iter()
                .flatten()
                .any(|rule: &Equality| expands(arena, rule.from) || expands(arena, rule.to));
            if !cited && !expands(arena, left) && !expands(arena, right) {
                continue;
            }
        }
        match search(
            arena,
            context,
            definitions,
            goal,
            citations,
            &mut rules,
            licenses.operations,
            &distributions,
            &mut search_steps,
            conversion,
        ) {
            Err(SearchError::NoProof) => {}
            found => return found,
        }
    }
    Err(SearchError::NoProof)
}

/// The goal as stated, its two sides normalized, and the proof that its left
/// side is itself.
#[derive(Clone, Copy)]
struct Goal {
    stated: TermHandle,
    ty: TermHandle,
    left: TermHandle,
    right: TermHandle,
    reflexivity: TermHandle,
}

/// One attempt of [`prove_licensed`] under one set of distributivity laws.
/// The citations' rewrite rules are built on the first attempt that needs
/// them and kept in `rules` for the others.
#[allow(clippy::too_many_arguments)]
fn search(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    goal: Goal,
    citations: &[TermHandle],
    rules: &mut Option<Vec<Equality>>,
    licenses: &[License],
    distributions: &[Distribution],
    search_steps: &mut usize,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    let Goal {
        stated: stated_goal,
        ty,
        left,
        right,
        reflexivity,
    } = goal;
    let (canonical, from_right) = rearrange(arena, licenses, distributions, right, search_steps)?;
    // Whether the reached term closes the goal: rearrange it, then undo the
    // right side's rearrangement from the shared canonical form.
    let close = |arena: &mut TermArena,
                 term: TermHandle,
                 base: TermHandle,
                 search_steps: &mut usize|
     -> Result<Option<TermHandle>, SearchError> {
        if arena.structurally_equal(term, right) {
            return Ok(Some(base));
        }
        if licenses.is_empty() && distributions.is_empty() {
            return Ok(None);
        }
        let (reached, forward) = rearrange(arena, licenses, distributions, term, search_steps)?;
        if !arena.structurally_equal(reached, canonical) {
            return Ok(None);
        }
        let (mut term, mut proof) = (term, base);
        for step in &forward {
            (term, proof) = rewrite(arena, ty, left, term, proof, &step.slots, step.rule)?;
        }
        for step in from_right.iter().rev() {
            let back = step.rule.reversed(arena);
            (term, proof) = rewrite(arena, ty, left, term, proof, &step.slots, back)?;
        }
        Ok(Some(proof))
    };
    if let Some(proof) = close(arena, left, reflexivity, search_steps)? {
        return Ok(proof);
    }
    if rules.is_none() {
        let mut built = Vec::new();
        for &proof in citations {
            charge(search_steps)?;
            let inferred = infer_type(arena, context, proof, conversion)?;
            let head = weak_head_normalize(arena, context.signature(), inferred, conversion)?;
            if let Term::Id {
                ty: carrier,
                left: from,
                right: to,
            } = arena.get(head)
            {
                // The equality as stated, then what injectivity gives from it,
                // down to the fields of fields.
                let mut stated = vec![Equality {
                    carrier,
                    from: normal(arena, context, definitions, from, conversion)?,
                    to: normal(arena, context, definitions, to, conversion)?,
                    proof,
                }];
                let mut next = 0;
                while let Some(&equality) = stated.get(next) {
                    next += 1;
                    charge(search_steps)?;
                    stated.extend(definitions.injective(arena, equality));
                }
                for equality in stated {
                    if let Some(proof) = definitions.absurd(arena, equality, stated_goal) {
                        return Ok(proof);
                    }
                    built.extend([equality, equality.reversed(arena)]);
                }
            }
        }
        *rules = Some(built);
    }
    let rules = match rules.as_deref() {
        Some(rules) if !rules.is_empty() => rules,
        _ => return Err(SearchError::NoProof),
    };
    let mut reached = vec![(left, reflexivity)];
    let mut next = 0;
    while let Some(&(current, base)) = reached.get(next) {
        next += 1;
        let mut pending = vec![(current, Vec::new(), 0u32)];
        while let Some((focus, path, depth)) = pending.pop() {
            for &rule in rules.iter() {
                charge(search_steps)?;
                // A citation is stated outside every binder the focus is under.
                if !arena.structurally_equal_shifted(rule.from, focus, depth) {
                    continue;
                }
                let replacement = shift(arena, rule.to, 0, depth);
                let rewritten = plug(arena, &path, replacement, 0);
                // The rewrite may have put a case where a function dispatches.
                // The kernel converts the computed form, so the proof of the
                // rewritten term is a proof of it too.
                let rewritten = normal(arena, context, definitions, rewritten, conversion)?;
                let mut seen = false;
                for &(known, _) in &reached {
                    charge(search_steps)?;
                    if arena.structurally_equal(known, rewritten) {
                        seen = true;
                        break;
                    }
                }
                if seen {
                    continue;
                }
                let destination = arena.insert(Term::Variable(depth + 1));
                let under_binders = plug(arena, &path, destination, 2);
                let carrier = shift(arena, ty, 0, 2);
                let start = shift(arena, left, 0, 2);
                let conclusion = arena.insert(Term::Id {
                    ty: carrier,
                    left: start,
                    right: under_binders,
                });
                let proof = rule.eliminate(arena, conclusion, base);
                if let Some(proof) = close(arena, rewritten, proof, search_steps)? {
                    return Ok(proof);
                }
                reached.push((rewritten, proof));
            }
            let node = arena.get(focus);
            for (slot, (child, bound)) in node.bound_children().enumerate() {
                let mut inner = path.clone();
                inner.push(Surrounding {
                    parent: focus,
                    slot,
                    depth,
                });
                pending.push((child, inner, depth + bound));
            }
        }
    }
    Err(SearchError::NoProof)
}

use super::{call_contracts, graph::Graph, hypotheses};
use proof_admission::{
    Declaration, Level, MathematicalCertificate, MathematicalCoreDecision,
    MathematicalJudgmentReceipt, Sort, check_signature,
};
use semantic_vocabulary::{
    ClauseCarrier, ClauseDeclaration, ClauseIdentity, ClauseOperand, MachineId, ObligationId,
    PlaceId, Proposition, ScalarTerm, TargetMachineCallable,
};
use terminal_check_tables::{
    Checked, Construct, KnowledgeId, MachineTables, ProgramPoint, RefusalReason, UnprovedCause,
};
use terminal_check_tables::{edges::Callee, values::Justification};
use terminal_psi::{TerminalMachine, TerminalModule};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgment {
    pub goal: ClauseIdentity,
    pub citations: Vec<KnowledgeId>,
    pub decision: MathematicalCoreDecision,
    pub certificate: Vec<u8>,
}

#[derive(PartialEq, Eq)]
enum Entry {
    Carrier(ClauseCarrier),
    Input(PlaceId),
    Scalar(ScalarTerm),
    Operation(ClauseDeclaration),
    Law(MachineId, u32),
}

#[derive(Clone, Copy, PartialEq)]
enum LawInput {
    Structural(PlaceId),
    Scalar(semantic_vocabulary::ValueId),
}

#[derive(Default)]
struct Declarations {
    arena: TermArena,
    entries: Vec<(Entry, TermHandle)>,
    signature: Vec<Declaration>,
}

impl Declarations {
    fn entry(&mut self, key: Entry, ty: TermHandle) -> Result<TermHandle, &'static str> {
        if let Some(index) = self.entries.iter().position(|(known, _)| *known == key) {
            if !self.arena.structurally_equal(self.signature[index].ty, ty) {
                return Err("the same signature entry has two different types");
            }
            return Ok(self.entries[index].1);
        }
        let declaration =
            u32::try_from(self.signature.len()).map_err(|_| "signature exceeds its index width")?;
        let term = self.arena.insert(Term::Constant {
            declaration,
            levels: vec![],
        });
        self.signature.push(Declaration::assumption(0, ty));
        self.entries.push((key, term));
        Ok(term)
    }

    fn carrier(&mut self, carrier: ClauseCarrier) -> Result<TermHandle, &'static str> {
        let universe = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        self.entry(Entry::Carrier(carrier), universe)
    }

    fn application(
        &mut self,
        declaration: ClauseDeclaration,
        result: TermHandle,
        domains: &[ClauseCarrier],
        arguments: &[TermHandle],
    ) -> Result<TermHandle, &'static str> {
        let mut function_type = result;
        for &carrier in domains.iter().rev() {
            let domain = self.carrier(carrier)?;
            function_type = self.arena.insert(Term::Pi {
                domain,
                codomain: function_type,
            });
        }
        let operation = self.entry(Entry::Operation(declaration), function_type)?;
        Ok(self.arena.apply_arguments(operation, arguments))
    }

    fn scalar(
        &mut self,
        term: &ScalarTerm,
        binders: &[LawInput],
    ) -> Result<TermHandle, &'static str> {
        let ty = self.carrier(ClauseCarrier::Scalar(term.scalar_type()))?;
        match term {
            ScalarTerm::DeclarationApplication {
                declaration,
                arguments,
                ..
            } => {
                let mut terms = Vec::with_capacity(arguments.len());
                let mut domains = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    domains.push(ClauseCarrier::Scalar(argument.scalar_type()));
                    terms.push(self.scalar(argument, binders)?);
                }
                self.application(*declaration, ty, &domains, &terms)
            }
            ScalarTerm::Value { id, .. } if !binders.is_empty() => {
                let position = binders
                    .iter()
                    .position(|binder| *binder == LawInput::Scalar(*id))
                    .ok_or("law scalar names a foreign input")?;
                Ok(self.arena.insert(Term::Variable(
                    u32::try_from(binders.len() - position - 1)
                        .map_err(|_| "too many law formals")?,
                )))
            }
            ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => self.entry(Entry::Scalar(term.clone()), ty),
            _ if binders.is_empty() => self.entry(Entry::Scalar(term.clone()), ty),
            _ => Err("no whole-value mathematical input for this scalar operand"),
        }
    }

    /// A closed law quantifies over its formals. An instantiated clause
    /// names the invocation inputs to which the storage analysis traced it.
    fn identity(
        &mut self,
        clause: &ClauseIdentity,
        binders: &[LawInput],
    ) -> Result<TermHandle, &'static str> {
        clause
            .validate()
            .map_err(|_| "malformed declaration Identity")?;
        let mut terms = Vec::<TermHandle>::with_capacity(clause.values.len());
        for value in &clause.values {
            let ty = self.carrier(value.carrier)?;
            let term = match &value.operand {
                ClauseOperand::Input(input) if input.path().is_empty() => {
                    if let Some(position) = binders
                        .iter()
                        .position(|known| *known == LawInput::Structural(input.root()))
                    {
                        self.arena.insert(Term::Variable(
                            u32::try_from(binders.len() - position - 1)
                                .map_err(|_| "too many law formals")?,
                        ))
                    } else if binders.is_empty() {
                        self.entry(Entry::Input(input.root()), ty)?
                    } else {
                        return Err("law names storage outside its telescope");
                    }
                }
                ClauseOperand::Scalar(term) => self.scalar(term, binders)?,
                ClauseOperand::Application {
                    declaration,
                    arguments,
                } => {
                    let domains = arguments
                        .iter()
                        .map(|&arg| clause.values[arg as usize].carrier)
                        .collect::<Vec<_>>();
                    let operands = arguments
                        .iter()
                        .map(|&arg| terms[arg as usize])
                        .collect::<Vec<_>>();
                    self.application(*declaration, ty, &domains, &operands)?
                }
                _ => return Err("no whole-value mathematical input for this clause operand"),
            };
            terms.push(term);
        }
        let ty = self.carrier(clause.values[clause.left as usize].carrier)?;
        Ok(self.arena.insert(Term::Id {
            ty,
            left: terms[clause.left as usize],
            right: terms[clause.right as usize],
        }))
    }

    fn cite(
        &mut self,
        machine: MachineId,
        ordinal: u32,
        original: &ClauseIdentity,
        actual: &ClauseIdentity,
        contract: &call_contracts::PublicContract<'_>,
    ) -> Result<TermHandle, &'static str> {
        if original.left != actual.left
            || original.right != actual.right
            || original.values.len() != actual.values.len()
        {
            return Err("citation changed the law's endpoints or term table");
        }
        let call_contracts::ScalarParameters::Values(scalars) = contract.scalar_parameters else {
            return Err("law has no declaration identities for its scalar inputs");
        };
        let binders: Vec<_> = contract
            .structural_parameters
            .iter()
            .map(|formal| LawInput::Structural(formal.place))
            .chain(scalars.iter().map(|formal| LawInput::Scalar(formal.id)))
            .collect();
        let mut actuals = vec![None; binders.len()];
        for (formal, supplied) in original.values.iter().zip(&actual.values) {
            if formal.carrier != supplied.carrier {
                return Err("citation changed a carrier");
            }
            let (binder, operand) = match (&formal.operand, &supplied.operand) {
                (ClauseOperand::Input(input), ClauseOperand::Input(actual))
                    if input.path().is_empty() && actual.path().is_empty() =>
                {
                    (
                        LawInput::Structural(input.root()),
                        Entry::Input(actual.root()),
                    )
                }
                (
                    ClauseOperand::Scalar(ScalarTerm::Value { id, .. }),
                    ClauseOperand::Scalar(actual),
                ) => (LawInput::Scalar(*id), Entry::Scalar(actual.clone())),
                (left, right) if left == right => continue,
                _ => return Err("citation changed the selected law operation"),
            };
            let position = binders
                .iter()
                .position(|&known| known == binder)
                .ok_or("law has a foreign formal")?;
            let ty = self.carrier(formal.carrier)?;
            let operand = self.entry(operand, ty)?;
            if actuals[position]
                .replace((ty, operand))
                .is_some_and(|previous| previous != (ty, operand))
            {
                return Err("one formal received different values at a single citation");
            }
        }
        let mut statement = self.identity(original, &binders)?;
        let actuals: Vec<_> = actuals
            .into_iter()
            .collect::<Option<_>>()
            .ok_or("law needs a supplied term for each formal")?;
        for &(domain, _) in actuals.iter().rev() {
            statement = self.arena.insert(Term::Pi {
                domain,
                codomain: statement,
            });
        }
        let law = self.entry(Entry::Law(machine, ordinal), statement)?;
        Ok(self.arena.apply_arguments(
            law,
            &actuals.iter().map(|&(_, term)| term).collect::<Vec<_>>(),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn discharge(
    module: &TerminalModule,
    machine: &TerminalMachine,
    tables: &MachineTables,
    graph: &Graph,
    point: ProgramPoint,
    obligation: ObligationId,
    goal: &ClauseIdentity,
    law_instances: &[MachineId],
) -> Checked<Judgment> {
    let known = hypotheses(graph, tables, machine.id, obligation, point)?;
    let unavailable = |why: &str| {
        super::refusal(
            RefusalReason::Unsupported,
            machine.id,
            Construct::Obligation(obligation),
            why,
        )
    };
    let mut declarations = Declarations::default();
    let expected = declarations.identity(goal, &[]).map_err(unavailable)?;
    let mut proofs = Vec::new();
    let mut citations = Vec::new();
    let mut premises = Vec::new();
    for (&row, premise) in known.assumption_rows.iter().zip(&known.assumptions) {
        let Proposition::DeclarationIdentity(actual) = premise else {
            continue;
        };
        let justification = tables.knowledge.get(row).map(|row| &row.justification);
        if let Some(Justification::EntryRequirement { clause }) = justification {
            // Only the caller owes this hypothesis. Rejoin the row to this
            // machine's published input clause before binding its proof.
            if machine.requires.get(*clause as usize) != Some(premise) {
                return Err(unavailable(
                    "entry equality differs from its declared requirement",
                ));
            }
            premises.push(declarations.identity(actual, &[]).map_err(unavailable)?);
            citations.push(row);
            continue;
        }
        let Some(Justification::CalleeGuarantee { invocation, clause }) = justification else {
            continue;
        };
        let call = tables
            .invocations
            .get(*invocation)
            .ok_or_else(|| unavailable("citation lost its invocation"))?;
        let Callee::Declared(TargetMachineCallable::Machine(callee)) = call.callee else {
            continue;
        };
        if law_instances.binary_search(&callee).is_err() {
            continue;
        }
        let call = call_contracts::invocation(module, machine, tables, *invocation, call)?;
        if !call.contract.requires.is_empty() {
            return Err(unavailable("conditional law needs its proved premises"));
        }
        let Some(Proposition::DeclarationIdentity(original)) =
            call.contract.ensures.iter().nth(*clause as usize)
        else {
            return Err(unavailable("citation lost its declared equality"));
        };
        proofs.push(
            declarations
                .cite(callee, *clause, original, actual, &call.contract)
                .map_err(unavailable)?,
        );
        citations.push(row);
    }
    let mut budget = Budget::default();
    let kernel_error = |error| match error {
        SearchError::Budget => super::unproved(
            machine.id,
            obligation,
            UnprovedCause::BudgetExhausted,
            "mathematical equality proof budget exhausted".into(),
        ),
        SearchError::NoProof => super::unproved(
            machine.id,
            obligation,
            UnprovedCause::NoDerivation,
            "no available equality proves the stated Identity".into(),
        ),
        SearchError::Kernel(error) => super::unproved(
            machine.id,
            obligation,
            UnprovedCause::KernelRefused,
            format!("mathematical kernel: {error:?}"),
        ),
    };
    let signature = check_signature(
        &mut declarations.arena,
        &declarations.signature,
        &mut budget,
    )
    .map_err(|error| kernel_error(error.into()))?;
    let mut context = Context::empty().with_signature(signature);
    for index in 0..premises.len() {
        let index =
            u32::try_from(index).map_err(|_| unavailable("too many equality hypotheses"))?;
        proofs.push(declarations.arena.insert(Term::Variable(index)));
    }
    for &premise in &premises {
        context = context.extend(premise);
    }
    let proof = prove(
        &mut declarations.arena,
        &context,
        expected,
        &proofs,
        65_536,
        &mut budget,
    )
    .map_err(kernel_error)?;
    let certificate =
        MathematicalCertificate::with_signature(declarations.signature, premises, proof, expected);
    let receipt =
        MathematicalJudgmentReceipt::verify(&mut declarations.arena, &certificate, &mut budget)
            .map_err(|error| kernel_error(error.into()))?;
    let bytes = terminal_codec::encode_mathematical_certificate(&declarations.arena, &certificate)
        .map_err(|_| unavailable("could not encode the judged mathematical term"))?;
    Ok(Judgment {
        goal: goal.clone(),
        citations,
        decision: MathematicalCoreDecision::Judged(receipt),
        certificate: bytes,
    })
}
