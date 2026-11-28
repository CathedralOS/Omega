//! Rewrite a goal with the equality terms available at that program point.
//! Each visited expression carries a proof from the original left endpoint.

use proof_admission::PlainData;
use proof_admission::{
    Budget, Context, CoreError, Term, TermArena, TermHandle, infer_type, shift, weak_head_normalize,
};
use terminal_check_tables::mathematics::{FunctionCases, SignatureEntry, SignatureEntryKind};

/// Which proof-work limit stopped an equality proof search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetLimit {
    SearchSteps,
    KernelSteps,
    IndexExpansionTerms {
        limit: usize,
        summands: Option<usize>,
    },
}

impl BudgetLimit {
    pub(crate) fn describe(self) -> String {
        match self {
            Self::SearchSteps => {
                "mathematical equality proof budget exhausted (search step limit)".to_owned()
            }
            Self::KernelSteps => {
                "mathematical equality proof budget exhausted (kernel conversion step limit)"
                    .to_owned()
            }
            Self::IndexExpansionTerms {
                limit,
                summands: Some(count),
            } => format!(
                "mathematical equality proof budget exhausted: product expansion requires {count} summands, exceeding `index_expansion_terms` limit {limit}"
            ),
            Self::IndexExpansionTerms {
                limit,
                summands: None,
            } => format!(
                "mathematical equality proof budget exhausted: product expansion summand count overflows, exceeding `index_expansion_terms` limit {limit}"
            ),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SearchError {
    NoProof,
    Budget(BudgetLimit),
    Kernel(CoreError),
}

impl From<CoreError> for SearchError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::StepCeiling => Self::Budget(BudgetLimit::KernelSteps),
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

/// `term` with every occurrence of `target` replaced by one new variable,
/// stated under that variable: bound just outside the term, it gives back
/// `term` when it is `target`. Both terms are in normal form, so an
/// occurrence is a subterm equal to `target` as written.
pub(crate) fn abstracted(
    arena: &mut TermArena,
    term: TermHandle,
    target: TermHandle,
) -> TermHandle {
    let mut current = shift(arena, term, 0, 1);
    let target = shift(arena, target, 0, 1);
    loop {
        let mut pending = vec![(current, Vec::new(), 0u32)];
        let mut found = None;
        while let Some((focus, path, depth)) = pending.pop() {
            if arena.structurally_equal_shifted(target, focus, depth) {
                found = Some((path, depth));
                break;
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
        let Some((path, depth)) = found else {
            return current;
        };
        let hole = arena.insert(Term::Variable(depth));
        current = plug(arena, &path, hole, 0);
    }
}

/// Whether `needle` is a proper subterm of `term`.
fn occurs(arena: &TermArena, needle: TermHandle, term: TermHandle) -> bool {
    arena
        .get(term)
        .children()
        .any(|child| arena.structurally_equal(child, needle) || occurs(arena, needle, child))
}

/// How many term nodes the term has.
fn size(arena: &TermArena, term: TermHandle) -> usize {
    1 + arena
        .get(term)
        .children()
        .map(|child| size(arena, child))
        .sum::<usize>()
}

fn charge(remaining: &mut usize) -> Result<(), SearchError> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or(SearchError::Budget(BudgetLimit::SearchSteps))?;
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
    /// The recursor of a member of a mutual block of data. A definition by
    /// mutual recursion is an abbreviation of it, so it is left to the
    /// kernel, which computes it on a case.
    MutualRecursor,
}

pub(crate) fn spine(arena: &TermArena, term: TermHandle) -> (TermHandle, Vec<TermHandle>) {
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
pub(crate) struct Definitions(
    Vec<Definition>,
    Vec<(u32, PlainData)>,
    /// Whether an abbreviation is for now left as its name: the first
    /// attempt of [`prove_licensed`].
    std::cell::Cell<bool>,
);

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
            std::cell::Cell::new(false),
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
                    SignatureEntryKind::Encoding { data }
                        if shape(*data).is_some_and(|shape| shape.is_mutual()) =>
                    {
                        Definition::MutualRecursor
                    }
                    SignatureEntryKind::Data { .. }
                    | SignatureEntryKind::Record { .. }
                    | SignatureEntryKind::Boolean
                    | SignatureEntryKind::Encoding { .. }
                    | SignatureEntryKind::Scheme
                    | SignatureEntryKind::GenericData { .. }
                    | SignatureEntryKind::Quotient { .. }
                    | SignatureEntryKind::Statement { .. }
                    | SignatureEntryKind::Proof { .. } => Definition::Opaque,
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
            std::cell::Cell::new(false),
        )
    }

    fn get(&self, declaration: u32) -> &Definition {
        match self.0.get(declaration as usize) {
            Some(Definition::Abbreviation) if self.2.get() => &Definition::Opaque,
            Some(definition) => definition,
            None => &Definition::Opaque,
        }
    }

    fn abbreviates(&self) -> bool {
        self.0
            .iter()
            .any(|definition| matches!(definition, Definition::Abbreviation))
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

    /// The definition by cases an induction's arguments start with: the
    /// induction applied to exactly the motive and branches stage 05 built
    /// that definition from, and then at least a subject.
    fn recursion(
        &self,
        arena: &TermArena,
        induction: u32,
        arguments: &[TermHandle],
    ) -> Option<(u32, &FunctionCases)> {
        self.0.iter().enumerate().find_map(|(index, definition)| {
            let Definition::Recursion {
                cases,
                induction: of,
            } = definition
            else {
                return None;
            };
            let count = cases.branches.len();
            (*of == induction
                && arguments.len() >= count + 2
                && arena.structurally_equal(arguments[0], cases.motive)
                && cases
                    .branches
                    .iter()
                    .zip(&arguments[1..=count])
                    .all(|(&known, &given)| arena.structurally_equal(known, given)))
            .then_some((index as u32, cases))
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
pub(crate) fn normal(
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
        &|declaration| {
            !matches!(
                definitions.get(declaration),
                Definition::Abbreviation | Definition::MutualRecursor
            )
        },
    )?;
    let (function, arguments) = spine(arena, head);
    // An induction applied to the motive, branches and subject of a
    // definition by cases is that definition: its entry, and the arguments
    // with the subject back in its place. With too few arguments for that it
    // stays an induction, and its motive and branches stay as written, so
    // that it is still recognized once the rest are supplied.
    let recursion = match arena.get(function) {
        Term::Constant { declaration, .. } => definitions
            .recursion(arena, declaration, &arguments)
            .map(|(entry, cases)| (entry, cases.branches.len(), cases.subject, cases.arity)),
        _ => None,
    };
    let (function, arguments) = match recursion {
        Some((entry, count, subject, arity)) if arguments.len() - count > arity as usize => {
            let constant = arena.insert(Term::Constant {
                declaration: entry,
                levels: Vec::new(),
            });
            let mut restored = arguments[count + 2..].to_vec();
            restored.insert(subject as usize, arguments[count + 1]);
            (constant, restored)
        }
        Some((_, count, ..)) => {
            let mut operands = arguments[..=count].to_vec();
            for &argument in &arguments[count + 1..] {
                operands.push(normal(arena, context, definitions, argument, conversion)?);
            }
            return Ok(arena.apply_arguments(function, &operands));
        }
        None => (function, arguments),
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
            levels: vec![proof_admission::Level::Constant(0)],
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
        // The branch takes the recursion at each field that holds the data
        // itself, after all of the case's fields: at a field holding a
        // function to the data, the recursion at every child, `λx. rec (f x)`.
        let Some((_, shape)) = definitions.1.iter().find(|(entry, _)| *entry == cases.data) else {
            return Ok(arena.apply_arguments(function, &operands));
        };
        let recursive: Vec<(TermHandle, proof_admission::PlainField)> = fields
            .iter()
            .zip(shape.fields(case))
            .filter(|(_, kind)| kind.is_recursive())
            .map(|(&field, &kind)| (field, kind))
            .collect();
        for &(field, kind) in &recursive {
            let at_field = match kind {
                proof_admission::PlainField::Function { argument, .. } => {
                    let function = shift(arena, field, 0, 1);
                    let bound = arena.insert(Term::Variable(0));
                    let child = arena.insert(Term::Apply {
                        function,
                        argument: bound,
                    });
                    let recursor = shift(arena, recursor, 0, 1);
                    let body = arena.insert(Term::Apply {
                        function: recursor,
                        argument: child,
                    });
                    arena.insert(Term::Lambda {
                        domain: argument,
                        body,
                    })
                }
                _ => arena.insert(Term::Apply {
                    function: recursor,
                    argument: field,
                }),
            };
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
    // A projection of a pair is that component. The pair is often what a
    // definition with a record result computes to once its subject is a
    // case, such as `divide_step(Zero, k, q, c).quotient`, and the head was
    // normalized before its child unfolded.
    match (&node, children.as_slice()) {
        (Term::Fst { .. }, [pair]) => {
            if let Term::Pair { first, .. } = arena.get(*pair) {
                return Ok(first);
            }
        }
        (Term::Snd { .. }, [pair]) => {
            if let Term::Pair { second, .. } = arena.get(*pair) {
                return Ok(second);
            }
        }
        _ => {}
    }
    // A pair of both projections of one value is that value: the kernel
    // converts the two, and the search compares terms by their spelling.
    if let (Term::Pair { .. }, [first, second]) = (&node, children.as_slice())
        && let (Term::Fst { pair: left }, Term::Snd { pair: right }) =
            (arena.get(*first), arena.get(*second))
        && arena.structurally_equal(left, right)
    {
        return Ok(left);
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
#[derive(Clone, Copy)]
pub(crate) struct Licenses<'l> {
    pub operations: &'l [License],
    pub distributions: &'l [Distribution],
    pub index_expansion_terms: usize,
}

impl Default for Licenses<'_> {
    fn default() -> Self {
        Self {
            operations: &[],
            distributions: &[],
            index_expansion_terms: MAX_SUMMANDS,
        }
    }
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
    index_expansion_terms: usize,
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
        let count = summands(arena, law, expands, term);
        if count.is_none_or(|count| count > index_expansion_terms) {
            return Err(SearchError::Budget(BudgetLimit::IndexExpansionTerms {
                limit: index_expansion_terms,
                summands: count,
            }));
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
        Term::Lambda { .. } => 4,
        Term::Pi { .. } => 5,
        Term::Pair { .. } => 7,
        Term::Fst { .. } => 8,
        Term::Snd { .. } => 9,
        Term::Sigma { .. } => 10,
        Term::Id { .. } => 11,
        Term::Refl { .. } => 12,
        Term::IdElim { .. } => 13,
        Term::Two => 15,
        Term::TwoZero => 16,
        Term::TwoOne => 17,
        Term::CaseTwo { .. } => 18,
        Term::Sort(_) => 19,
        _ => 20,
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
    index_expansion_terms: usize,
    term: TermHandle,
    slots: &mut Vec<usize>,
) -> Result<Option<Vec<Step>>, SearchError> {
    use std::cmp::Ordering::Greater;
    if let Some(steps) = next_expansion(
        arena,
        licenses,
        distributions,
        index_expansion_terms,
        term,
        slots,
    )? {
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
    // Operands are rearranged wherever they sit beneath a term former that
    // binds nothing: an application, a pair or a projection. A binder is not
    // entered: a law is stated outside it.
    let node = arena.get(term);
    if matches!(
        node,
        Term::Apply { .. } | Term::Pair { .. } | Term::Fst { .. } | Term::Snd { .. }
    ) {
        for (slot, child) in node.children().enumerate() {
            slots.push(slot);
            let found = next_rearrangement(
                arena,
                licenses,
                distributions,
                index_expansion_terms,
                child,
                slots,
            );
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
    index_expansion_terms: usize,
    mut term: TermHandle,
    search_steps: &mut usize,
) -> Result<(TermHandle, Vec<Step>), SearchError> {
    let mut taken = Vec::new();
    if licenses.is_empty() && distributions.is_empty() {
        return Ok((term, taken));
    }
    while let Some(steps) = next_rearrangement(
        arena,
        licenses,
        distributions,
        index_expansion_terms,
        term,
        &mut Vec::new(),
    )? {
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

/// The operands of a chain of `operation`, left to right, however it nests.
fn chain(arena: &TermArena, operation: TermHandle, term: TermHandle, out: &mut Vec<TermHandle>) {
    match operands_of(arena, operation, term) {
        Some((first, second)) => {
            chain(arena, operation, first, out);
            chain(arena, operation, second, out);
        }
        None => out.push(term),
    }
}

/// The operands joined by `operation`, nested to the right, or `None` for no
/// operand.
fn joined(
    arena: &mut TermArena,
    operation: TermHandle,
    operands: &[TermHandle],
) -> Option<TermHandle> {
    let (&last, before) = operands.split_last()?;
    Some(before.iter().rev().fold(last, |rest, &operand| {
        arena.apply_binary(operation, operand, rest)
    }))
}

/// `operands` with one occurrence of each of `taken` removed, or `None` when
/// some term of `taken` has no occurrence left.
fn without(
    arena: &TermArena,
    operands: &[TermHandle],
    taken: &[TermHandle],
) -> Option<Vec<TermHandle>> {
    let mut rest = operands.to_vec();
    for &term in taken {
        let at = rest
            .iter()
            .position(|&operand| arena.structurally_equal(operand, term))?;
        rest.remove(at);
    }
    Some(rest)
}

/// A chain of an operation that both commutes and associates: where it sits
/// in its term, the operation's license and the chain's operands.
struct Chain {
    slots: Vec<usize>,
    license: License,
    operands: Vec<TermHandle>,
}

/// Every such chain in the term outside a binder, the longest at each place:
/// a chain's operands are searched for chains of other operations only.
fn chains(
    arena: &TermArena,
    licenses: &[License],
    term: TermHandle,
    slots: &mut Vec<usize>,
    within: Option<TermHandle>,
    out: &mut Vec<Chain>,
) {
    if let Some((license, first, second)) = licensed(arena, licenses, term)
        && license.commutes.is_some()
        && license.associates.is_some()
    {
        let operation = license.operation;
        if !within.is_some_and(|outer| arena.structurally_equal(outer, operation)) {
            let mut operands = Vec::new();
            chain(arena, operation, term, &mut operands);
            out.push(Chain {
                slots: slots.clone(),
                license: *license,
                operands,
            });
        }
        let depth = slots.len();
        slots.extend([0, 1]);
        chains(arena, licenses, first, slots, Some(operation), out);
        slots.truncate(depth);
        slots.push(1);
        chains(arena, licenses, second, slots, Some(operation), out);
        slots.pop();
        return;
    }
    let node = arena.get(term);
    if matches!(
        node,
        Term::Apply { .. } | Term::Pair { .. } | Term::Fst { .. } | Term::Snd { .. }
    ) {
        for (slot, child) in node.children().enumerate() {
            slots.push(slot);
            chains(arena, licenses, child, slots, None, out);
            slots.pop();
        }
    }
}

/// The ways to write a chain so that it contains `stated`, the left side of
/// a rule, as one operand: each is the rewritten chain and the slots of
/// `stated` in it. `parts` are the operands of `stated` in canonical form.
///
/// The chain may hold the parts as they are: `(x + y) + rest` for a rule
/// about `x + y`. Or, where another licensed operation distributes over the
/// chain's, it may hold every part times one common factor:
/// `(x + y) * c + rest` for operands `x * c` and `y * c`. The caller proves
/// the chain equal to what is returned before it uses the rule there.
fn arrangements(
    arena: &mut TermArena,
    licenses: &[License],
    distributions: &[Distribution],
    found: &Chain,
    parts: &[TermHandle],
    stated: TermHandle,
) -> Vec<(TermHandle, Vec<usize>)> {
    let operation = found.license.operation;
    let mut arrangements = Vec::new();
    if let Some(rest) = without(arena, &found.operands, parts) {
        arrangements.push(match joined(arena, operation, &rest) {
            Some(rest) => (arena.apply_binary(operation, stated, rest), vec![0, 1]),
            None => (stated, Vec::new()),
        });
    }
    let mut products: Vec<License> = Vec::new();
    for law in distributions {
        if !arena.structurally_equal(law.inner, operation) {
            continue;
        }
        let Some(product) = licenses.iter().find(|license| {
            arena.structurally_equal(license.operation, law.outer)
                && license.commutes.is_some()
                && license.associates.is_some()
        }) else {
            continue;
        };
        // Two laws, one from each side, distribute the same product.
        if products
            .iter()
            .any(|known| arena.structurally_equal(known.operation, product.operation))
        {
            continue;
        }
        products.push(*product);
        let factors = |arena: &TermArena, term| {
            let mut factors = Vec::new();
            chain(arena, product.operation, term, &mut factors);
            factors
        };
        let first = factors(arena, parts[0]);
        let mut scales: Vec<Vec<TermHandle>> = Vec::new();
        for &operand in &found.operands {
            // The factors an operand has beyond the first part's.
            let Some(mut scale) = without(arena, &factors(arena, operand), &first) else {
                continue;
            };
            scale.sort_by(|&a, &b| order(arena, a, b));
            let known = |known: &Vec<TermHandle>| {
                known.len() == scale.len()
                    && known
                        .iter()
                        .zip(&scale)
                        .all(|(&a, &b)| arena.structurally_equal(a, b))
            };
            if scale.is_empty() || scales.iter().any(known) {
                continue;
            }
            // Every part times those factors must be an operand of its own.
            let mut rest = found.operands.clone();
            let complete = parts.iter().all(|&part| {
                let mut wanted = factors(arena, part);
                wanted.extend(&scale);
                let at = rest.iter().position(|&candidate| {
                    let have = factors(arena, candidate);
                    have.len() == wanted.len()
                        && without(arena, &have, &wanted).is_some_and(|left| left.is_empty())
                });
                at.map(|at| rest.remove(at)).is_some()
            });
            if complete && let Some(factor) = joined(arena, product.operation, &scale) {
                let scaled = arena.apply_binary(product.operation, stated, factor);
                arrangements.push(match joined(arena, operation, &rest) {
                    Some(rest) => (
                        arena.apply_binary(operation, scaled, rest),
                        vec![0, 1, 0, 1],
                    ),
                    None => (scaled, vec![0, 1]),
                });
            }
            scales.push(scale);
        }
    }
    arrangements
}

/// A term a side of the goal has been rewritten to, and the proof that the
/// side (`origin`) equals it.
#[derive(Clone, Copy)]
struct Rewriting {
    ty: TermHandle,
    origin: TermHandle,
    term: TermHandle,
    proof: TermHandle,
}

impl Rewriting {
    fn step(
        &mut self,
        arena: &mut TermArena,
        slots: &[usize],
        rule: Equality,
    ) -> Result<(), SearchError> {
        (self.term, self.proof) = rewrite(
            arena,
            self.ty,
            self.origin,
            self.term,
            self.proof,
            slots,
            rule,
        )?;
        Ok(())
    }
}

/// Each rule used on part of a chain of a licensed operation: a hypothesis
/// `x + y = z` rewrites `x + w + y` to `z + w`, and under distributivity
/// `x * c + w + y * c` to `z * c + w`. Matching a rule against a whole
/// subterm never finds these.
///
/// The term is put in canonical form, where a chain's operands are the
/// summands the licensed laws give it. For each chain that contains a rule's
/// operands ([`arrangements`]), the chain is rewritten to hold the rule's
/// left side as one operand, the rule is used there, and the result is put
/// in canonical form again, so the same sum reached two ways is one term.
/// Every step is a licensed law or the rule, added to the proof.
#[allow(clippy::too_many_arguments)]
fn exchange(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    licenses: &[License],
    distributions: &[Distribution],
    index_expansion_terms: usize,
    current: Rewriting,
    rules: &[(Equality, TermHandle)],
    search_steps: &mut usize,
    conversion: &mut Budget,
) -> Result<Vec<(TermHandle, TermHandle)>, SearchError> {
    let (canonical, forward) = rearrange(
        arena,
        licenses,
        distributions,
        index_expansion_terms,
        current.term,
        search_steps,
    )?;
    let mut found = Vec::new();
    chains(
        arena,
        licenses,
        canonical,
        &mut Vec::new(),
        None,
        &mut found,
    );
    let mut reached = Vec::new();
    for &(rule, stated) in rules {
        for chain_found in &found {
            let mut parts = Vec::new();
            chain(arena, chain_found.license.operation, stated, &mut parts);
            // A rule about one operand already matches it as a whole subterm.
            if parts.len() < 2 {
                continue;
            }
            for (arrangement, at) in arrangements(
                arena,
                licenses,
                distributions,
                chain_found,
                &parts,
                rule.from,
            ) {
                charge(search_steps)?;
                let path =
                    path_to(arena, canonical, &chain_found.slots).ok_or(SearchError::NoProof)?;
                let arranged = plug(arena, &path, arrangement, 0);
                let (same, backward) = rearrange(
                    arena,
                    licenses,
                    distributions,
                    index_expansion_terms,
                    arranged,
                    search_steps,
                )?;
                if !arena.structurally_equal(same, canonical) {
                    continue;
                }
                let mut rewriting = current;
                for step in &forward {
                    rewriting.step(arena, &step.slots, step.rule)?;
                }
                for step in backward.iter().rev() {
                    let back = step.rule.reversed(arena);
                    rewriting.step(arena, &step.slots, back)?;
                }
                let mut place = chain_found.slots.clone();
                place.extend(at);
                rewriting.step(arena, &place, rule)?;
                // As after any rewrite, the kernel converts the computed form.
                rewriting.term = normal(arena, context, definitions, rewriting.term, conversion)?;
                let (_, settle) = rearrange(
                    arena,
                    licenses,
                    distributions,
                    index_expansion_terms,
                    rewriting.term,
                    search_steps,
                )?;
                for step in &settle {
                    rewriting.step(arena, &step.slots, step.rule)?;
                }
                reached.push((rewriting.term, rewriting.proof));
            }
        }
    }
    Ok(reached)
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
            index_expansion_terms: MAX_SUMMANDS,
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
///
/// The whole is tried twice. First every definition that is not by cases is
/// left as its name, so a goal and its citations are compared as they were
/// written: `rat_gap(p, q)` stays three nodes. A proof that needs what such
/// a definition is defined as is not found that way, and the second try
/// unfolds them all.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_licensed(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    goal: TermHandle,
    citations: &[TermHandle],
    licenses: Licenses<'_>,
    search_steps: usize,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    // The box of a strict statement is introduced from a proof of that
    // statement, and a squash is proved strictly (`squashed`).
    let head = weak_head_normalize(arena, context.signature(), goal, conversion)?;
    match arena.get(head) {
        Term::Box { ty } => {
            let value = prove_licensed(
                arena,
                context,
                definitions,
                ty,
                citations,
                licenses,
                search_steps,
                conversion,
            )?;
            return Ok(arena.insert(Term::BoxIntro { ty, value }));
        }
        Term::Squash { ty } => {
            return squashed(
                arena,
                context,
                definitions,
                head,
                ty,
                citations,
                licenses,
                search_steps,
                conversion,
            );
        }
        _ => {}
    }
    if definitions.abbreviates() {
        definitions.2.set(true);
        let as_written = prove_as(
            arena,
            context,
            definitions,
            goal,
            citations,
            licenses,
            search_steps,
            conversion,
        );
        definitions.2.set(false);
        match as_written {
            Err(SearchError::NoProof | SearchError::Budget(_)) => {}
            found => return found,
        }
    }
    prove_as(
        arena,
        context,
        definitions,
        goal,
        citations,
        licenses,
        search_steps,
        conversion,
    )
}

/*
    A proof of `Squash G`. A citation of that very type proves it. Otherwise
    the first citation that is itself strict, a `Squash P` or a `Box S`, is
    opened: its eliminator targets the strict goal, so the witness inside is
    a citation only of the proof the goal is stated under, and no relevant
    value leaves it. Opening one never loses a proof, so the citations are
    opened in order, each once. With none left to open, a proof of `G` is
    squashed.
*/
#[allow(clippy::too_many_arguments)]
fn squashed(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    goal: TermHandle,
    stated: TermHandle,
    citations: &[TermHandle],
    licenses: Licenses<'_>,
    search_steps: usize,
    conversion: &mut Budget,
) -> Result<TermHandle, SearchError> {
    let mut opened = None;
    for (position, &citation) in citations.iter().enumerate() {
        let ty = infer_type(arena, context, citation, conversion)?;
        let ty = weak_head_normalize(arena, context.signature(), ty, conversion)?;
        if arena.structurally_equal(ty, goal) {
            return Ok(citation);
        }
        if opened.is_none() && matches!(arena.get(ty), Term::Squash { .. } | Term::Box { .. }) {
            opened = Some((position, ty));
        }
    }
    let Some((position, ty)) = opened else {
        let value = prove_licensed(
            arena,
            context,
            definitions,
            stated,
            citations,
            licenses,
            search_steps,
            conversion,
        )?;
        return Ok(arena.insert(Term::SquashIntro { ty: stated, value }));
    };
    let (Term::Squash { ty: inner } | Term::Box { ty: inner }) = arena.get(ty) else {
        unreachable!("the opened citation is strict");
    };
    let scrutinee = citations[position];
    let mut under: Vec<TermHandle> = citations
        .iter()
        .enumerate()
        .filter(|&(other, _)| other != position)
        .map(|(_, &citation)| shift(arena, citation, 0, 1))
        .collect();
    under.push(arena.insert(Term::Variable(0)));
    let lifted = shift(arena, goal, 0, 1);
    let witness = prove_licensed(
        arena,
        &context.extend(inner),
        definitions,
        lifted,
        &under,
        licenses,
        search_steps,
        conversion,
    )?;
    let function = arena.insert(Term::Lambda {
        domain: inner,
        body: witness,
    });
    Ok(match arena.get(ty) {
        Term::Squash { .. } => arena.insert(Term::SquashElim {
            proposition: goal,
            function,
            scrutinee,
        }),
        _ => {
            let motive = arena.insert(Term::Lambda {
                domain: ty,
                body: lifted,
            });
            arena.insert(Term::BoxElim {
                motive,
                body: function,
                scrutinee,
            })
        }
    })
}

/// One try of [`prove_licensed`], with abbreviations as `definitions` has
/// them.
#[allow(clippy::too_many_arguments)]
fn prove_as(
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
    // A citation for every value (a dependent product of an equality) is
    // the equality at each value of its domain the goal names.
    let (stated_left, stated_right) = match arena.get(goal) {
        Term::Id { left, right, .. } => (left, right),
        _ => unreachable!("the goal is an Identity"),
    };
    let instantiated = instances(
        arena,
        context,
        &[stated_left, stated_right, left, right],
        citations,
        conversion,
    )?;
    let citations = instantiated.as_deref().unwrap_or(citations);
    let goal = Goal {
        stated: goal,
        ty,
        left,
        right,
        reflexivity,
    };
    let mut rules = None;
    // What the search may spend, kept for the rewriting that takes over when
    // the search has spent it.
    let allowance = search_steps;
    let mut exhausted: Option<BudgetLimit> = None;
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
        if exhausted.is_none() {
            match search(
                arena,
                context,
                definitions,
                goal,
                citations,
                &mut rules,
                licenses.operations,
                &distributions,
                licenses.index_expansion_terms,
                &mut search_steps,
                conversion,
            ) {
                Err(SearchError::NoProof) => continue,
                Err(SearchError::Budget(limit)) => exhausted = Some(limit),
                found => return found,
            }
        }
        // The search tries every order of every rewrite, which a long
        // argument over large terms outgrows. Rewriting both sides to one
        // normal form tries one order.
        let Some(rules) = rules.as_deref() else {
            return Err(SearchError::Budget(
                exhausted.unwrap_or(BudgetLimit::SearchSteps),
            ));
        };
        let stated: Vec<Equality> = rules.iter().copied().step_by(2).collect();
        let mut steps = allowance;
        match completed(
            arena,
            context,
            definitions,
            goal,
            &stated,
            licenses.operations,
            &distributions,
            licenses.index_expansion_terms,
            &mut steps,
            conversion,
        ) {
            Ok(Some(proof)) => return Ok(proof),
            Ok(None) | Err(SearchError::NoProof) => {}
            Err(error) => return Err(error),
        }
    }
    Err(match exhausted {
        Some(limit) => SearchError::Budget(limit),
        None => SearchError::NoProof,
    })
}

/// The most values a citation for every value is applied at.
const INSTANCES: usize = 64;

/// The citations with each citation for every value, `p: (n: D) -> B`,
/// applied at every value of `D` that `terms` name outside a binder, and the
/// applications that are themselves for every value applied in turn. `None`
/// when no citation is for every value. A value is taken as it is written:
/// the instances an index a premise names needs, `n` or `5n`, are the ones
/// the goal names.
fn instances(
    arena: &mut TermArena,
    context: &Context,
    terms: &[TermHandle],
    citations: &[TermHandle],
    conversion: &mut Budget,
) -> Result<Option<Vec<TermHandle>>, SearchError> {
    let mut domains = Vec::new();
    for &proof in citations {
        let ty = infer_type(arena, context, proof, conversion)?;
        let ty = weak_head_normalize(arena, context.signature(), ty, conversion)?;
        if let Term::Pi { domain, .. } = arena.get(ty) {
            domains.push((proof, domain));
        }
    }
    if domains.is_empty() {
        return Ok(None);
    }
    // Every closed subterm of the terms, each with its type, once.
    let mut values: Vec<(TermHandle, TermHandle)> = Vec::new();
    let mut pending: Vec<TermHandle> = terms.to_vec();
    while let Some(term) = pending.pop() {
        if values.len() >= INSTANCES {
            break;
        }
        pending.extend(
            arena
                .get(term)
                .bound_children()
                .filter(|&(_, bound)| bound == 0)
                .map(|(child, _)| child),
        );
        if values
            .iter()
            .any(|&(known, _)| arena.structurally_equal(known, term))
        {
            continue;
        }
        let Ok(ty) = infer_type(arena, context, term, conversion) else {
            continue;
        };
        let ty = weak_head_normalize(arena, context.signature(), ty, conversion)?;
        values.push((term, ty));
    }
    let mut out = citations.to_vec();
    let mut applied = 0;
    while let Some((proof, domain)) = domains.pop() {
        for &(value, ty) in &values {
            if applied >= INSTANCES {
                break;
            }
            if !arena.structurally_equal(ty, domain) {
                continue;
            }
            applied += 1;
            let instance = arena.insert(Term::Apply {
                function: proof,
                argument: value,
            });
            let ty = infer_type(arena, context, instance, conversion)?;
            let ty = weak_head_normalize(arena, context.signature(), ty, conversion)?;
            if let Term::Pi { domain, .. } = arena.get(ty) {
                domains.push((instance, domain));
            } else {
                out.push(instance);
            }
        }
    }
    Ok(Some(out))
}

/// Whether `left` is the larger term: more nodes, or as many and later in
/// [`order`]. A rewrite from the larger of two terms to the smaller cannot go
/// on for ever.
fn larger(arena: &TermArena, left: TermHandle, right: TermHandle) -> bool {
    (size(arena, left), std::cmp::Ordering::Equal)
        .cmp(&(size(arena, right), order(arena, right, left)))
        == std::cmp::Ordering::Greater
}

/// The first subterm outside a binder, outermost first, that is the left
/// side of a rule: its slots and the rule.
fn first_rewrite(
    arena: &TermArena,
    rules: &[Equality],
    term: TermHandle,
) -> Option<(Vec<usize>, Equality)> {
    let mut pending = vec![(term, Vec::new())];
    while let Some((focus, slots)) = pending.pop() {
        if let Some(&rule) = rules
            .iter()
            .find(|rule| arena.structurally_equal(rule.from, focus))
        {
            return Some((slots, rule));
        }
        let node = arena.get(focus);
        for (slot, (child, bound)) in node.bound_children().enumerate() {
            if bound == 0 {
                let mut inner = slots.clone();
                inner.push(slot);
                pending.push((child, inner));
            }
        }
    }
    None
}

/// Rewrite the term as far as the rules take it: computed, put in canonical
/// form under the licensed laws, and rewritten by the first rule that
/// applies, until none does. Each rule goes from a larger term to a smaller.
#[allow(clippy::too_many_arguments)]
fn settled(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    licenses: &[License],
    distributions: &[Distribution],
    index_expansion_terms: usize,
    rules: &[Equality],
    mut at: Rewriting,
    steps: &mut usize,
    conversion: &mut Budget,
) -> Result<Rewriting, SearchError> {
    loop {
        // As after any rewrite, the kernel converts the computed form.
        at.term = normal(arena, context, definitions, at.term, conversion)?;
        let (_, arrange) = rearrange(
            arena,
            licenses,
            distributions,
            index_expansion_terms,
            at.term,
            steps,
        )?;
        for step in &arrange {
            at.step(arena, &step.slots, step.rule)?;
        }
        let Some((slots, rule)) = first_rewrite(arena, rules, at.term) else {
            return Ok(at);
        };
        charge(steps)?;
        at.step(arena, &slots, rule)?;
    }
}

/// A proof of the goal by rewriting both of its sides to one normal form, or
/// `None` when their normal forms differ.
///
/// The cited equalities are between terms with no variable to fill in: each
/// is a citation at its arguments. Such a set always has rewrite rules that
/// decide it. Each equality is taken in turn with both sides rewritten by the
/// rules so far, and becomes a rule from its larger side to its smaller. A
/// rule whose left side the new rule rewrites is an equality again and is
/// taken later. The goal holds by these equalities when its two sides then
/// rewrite to the same term. Every rewrite is one of the cited equalities or
/// a licensed law, carried in the proof like any other.
#[allow(clippy::too_many_arguments)]
fn completed(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    goal: Goal,
    stated: &[Equality],
    licenses: &[License],
    distributions: &[Distribution],
    index_expansion_terms: usize,
    steps: &mut usize,
    conversion: &mut Budget,
) -> Result<Option<TermHandle>, SearchError> {
    let mut waiting: std::collections::VecDeque<Equality> = stated.iter().copied().collect();
    let mut rules: Vec<Equality> = Vec::new();
    let settle = |arena: &mut TermArena,
                  rules: &[Equality],
                  at,
                  steps: &mut usize,
                  conversion: &mut Budget| {
        settled(
            arena,
            context,
            definitions,
            licenses,
            distributions,
            index_expansion_terms,
            rules,
            at,
            steps,
            conversion,
        )
    };
    while let Some(equality) = waiting.pop_front() {
        charge(steps)?;
        let carrier = equality.carrier;
        let forward = settle(
            arena,
            &rules,
            Rewriting {
                ty: carrier,
                origin: equality.from,
                term: equality.to,
                proof: equality.proof,
            },
            steps,
            conversion,
        )?;
        let turned = Equality {
            carrier,
            from: equality.from,
            to: forward.term,
            proof: forward.proof,
        }
        .reversed(arena);
        let both = settle(
            arena,
            &rules,
            Rewriting {
                ty: carrier,
                origin: turned.from,
                term: turned.to,
                proof: turned.proof,
            },
            steps,
            conversion,
        )?;
        if arena.structurally_equal(both.origin, both.term) {
            continue;
        }
        let mut rule = Equality {
            carrier,
            from: both.origin,
            to: both.term,
            proof: both.proof,
        };
        if larger(arena, rule.to, rule.from) {
            rule = rule.reversed(arena);
        }
        let (again, kept): (Vec<Equality>, Vec<Equality>) = rules.into_iter().partition(|known| {
            arena.structurally_equal(known.from, rule.from) || occurs(arena, rule.from, known.from)
        });
        waiting.extend(again);
        rules = kept;
        rules.push(rule);
    }
    let left = settle(
        arena,
        &rules,
        Rewriting {
            ty: goal.ty,
            origin: goal.left,
            term: goal.left,
            proof: goal.reflexivity,
        },
        steps,
        conversion,
    )?;
    let reflexivity = arena.insert(Term::Refl {
        ty: goal.ty,
        value: goal.right,
    });
    let right = settle(
        arena,
        &rules,
        Rewriting {
            ty: goal.ty,
            origin: goal.right,
            term: goal.right,
            proof: reflexivity,
        },
        steps,
        conversion,
    )?;
    if !arena.structurally_equal(left.term, right.term) {
        return Ok(None);
    }
    // `right = term` read backwards, then `left = term` carried along it to
    // `left = right`.
    let back = Equality {
        carrier: goal.ty,
        from: goal.right,
        to: right.term,
        proof: right.proof,
    }
    .reversed(arena);
    let carrier = shift(arena, goal.ty, 0, 2);
    let start = shift(arena, goal.left, 0, 2);
    let end = arena.insert(Term::Variable(1));
    let conclusion = arena.insert(Term::Id {
        ty: carrier,
        left: start,
        right: end,
    });
    Ok(Some(back.eliminate(arena, conclusion, left.proof)))
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
    index_expansion_terms: usize,
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
    let (canonical, from_right) = rearrange(
        arena,
        licenses,
        distributions,
        index_expansion_terms,
        right,
        search_steps,
    )?;
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
        let (reached, forward) = rearrange(
            arena,
            licenses,
            distributions,
            index_expansion_terms,
            term,
            search_steps,
        )?;
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
    // A rule is never used in the direction that puts its own left side
    // inside its right side: `x` to `plus(x, Zero)` applies again to its own
    // result, without end. Its other direction stays, and because both sides
    // of the goal are rewritten toward each other, a goal that needs the
    // larger term on one side still meets it from that side.
    let usable: Vec<Equality> = rules
        .iter()
        .copied()
        .filter(|rule| !occurs(arena, rule.from, rule.to))
        .collect();
    // Under a licensed operation a rule also applies to some of the operands
    // of a longer chain ([`exchange`]). Each rule is kept with its left side
    // in canonical form, whose operands are what a chain must contain.
    let mut arranged = Vec::new();
    if licenses
        .iter()
        .any(|license| license.commutes.is_some() && license.associates.is_some())
    {
        for &rule in &usable {
            let (stated, _) = rearrange(
                arena,
                licenses,
                distributions,
                index_expansion_terms,
                rule.from,
                search_steps,
            )?;
            arranged.push((rule, stated));
        }
    }
    // The terms reached from the left side and from the right side, each
    // with the proof that its side equals it. The smallest term reached so
    // far, from either side, is rewritten next.
    let right_reflexivity = arena.insert(Term::Refl { ty, value: right });
    let mut reached = [vec![(left, reflexivity)], vec![(right, right_reflexivity)]];
    let mut waiting = vec![
        (size(arena, left), 0usize, 0usize),
        (size(arena, right), 1, 0),
    ];
    while let Some(at) = (0..waiting.len()).min_by_key(|&at| waiting[at]) {
        let (_, side, index) = waiting.swap_remove(at);
        let (current, base) = reached[side][index];
        let origin = if side == 0 { left } else { right };
        // Every term one rewrite of `current` reaches, with the proof that
        // its side of the goal equals it.
        let mut found = Vec::new();
        let mut pending = vec![(current, Vec::new(), 0u32)];
        while let Some((focus, path, depth)) = pending.pop() {
            for &rule in &usable {
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
                let destination = arena.insert(Term::Variable(depth + 1));
                let under_binders = plug(arena, &path, destination, 2);
                let carrier = shift(arena, ty, 0, 2);
                let start = shift(arena, origin, 0, 2);
                let conclusion = arena.insert(Term::Id {
                    ty: carrier,
                    left: start,
                    right: under_binders,
                });
                found.push((rewritten, rule.eliminate(arena, conclusion, base)));
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
        if !arranged.is_empty() {
            let rewriting = Rewriting {
                ty,
                origin,
                term: current,
                proof: base,
            };
            found.extend(exchange(
                arena,
                context,
                definitions,
                licenses,
                distributions,
                index_expansion_terms,
                rewriting,
                &arranged,
                search_steps,
                conversion,
            )?);
        }
        for (rewritten, proof) in found {
            let mut seen = false;
            for &(known, _) in &reached[side] {
                charge(search_steps)?;
                if arena.structurally_equal(known, rewritten) {
                    seen = true;
                    break;
                }
            }
            if seen {
                continue;
            }
            if side == 0
                && let Some(proof) = close(arena, rewritten, proof, search_steps)?
            {
                return Ok(proof);
            }
            // The two sides meet at a term both have reached.
            let mut met = None;
            for &(known, known_proof) in &reached[1 - side] {
                charge(search_steps)?;
                if arena.structurally_equal(known, rewritten) {
                    met = Some(known_proof);
                    break;
                }
            }
            if let Some(met) = met {
                let (from_left, from_right) = if side == 0 {
                    (proof, met)
                } else {
                    (met, proof)
                };
                // `right = term` read backwards, then `left = term`
                // carried along it to `left = right`.
                let back = Equality {
                    carrier: ty,
                    from: right,
                    to: rewritten,
                    proof: from_right,
                }
                .reversed(arena);
                let carrier = shift(arena, ty, 0, 2);
                let start = shift(arena, left, 0, 2);
                let end = arena.insert(Term::Variable(1));
                let conclusion = arena.insert(Term::Id {
                    ty: carrier,
                    left: start,
                    right: end,
                });
                return Ok(back.eliminate(arena, conclusion, from_left));
            }
            waiting.push((size(arena, rewritten), side, reached[side].len()));
            reached[side].push((rewritten, proof));
        }
    }
    Err(SearchError::NoProof)
}

use crate::{call_contracts, graph::Graph, hypotheses};
use proof_admission::{
    Declaration, DeclarationKind, Level, MathematicalCertificate, MathematicalCoreDecision,
    MathematicalJudgmentReceipt, Signature, Sort, check_signature, extend_signature,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ClauseCarrier, ClauseDeclaration, ClauseIdentity,
    ClauseOperand, IntegerType, MachineId, ObligationId, PlaceId, Proposition, ScalarTerm,
    ScalarType, StructuralFieldId, StructuralTypeId, TargetMachineCallable,
};
use terminal_check_tables::mathematics::IdentityTable;
use terminal_check_tables::{
    Checked, Construct, KnowledgeId, MachineTables, ProgramPoint, RefusalReason, UnprovedCause,
};
use terminal_check_tables::{edges::Callee, values::Justification};
use terminal_psi::{TerminalIdentityIndex, TerminalMachine, TerminalModule};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgment {
    pub goal: ClauseIdentity,
    pub citations: Vec<KnowledgeId>,
    pub decision: MathematicalCoreDecision,
    pub certificate: Vec<u8>,
}

/// Whether the same-type integer identity bridge applies to builtin
/// `left == right`: both operands have exactly the comparison's integer
/// type, which is not an address. Mixed carriers keep numerical equality.
fn bridged_integer(scalar_type: IntegerType, left: &ScalarTerm, right: &ScalarTerm) -> bool {
    !scalar_type.is_address()
        && left.scalar_type() == ScalarType::Integer(scalar_type)
        && right.scalar_type() == ScalarType::Integer(scalar_type)
}

/// What one declaration of an obligation's kernel signature stands for.
#[derive(Clone, PartialEq, Eq)]
enum Named {
    Carrier(FieldCarrier),
    Input(PlaceId),
    /// A field of an input whose entry value no clause observed: one opaque
    /// value of its carrier.
    Field(PlaceId, Vec<CanonicalStructuralPathSegment>),
    Scalar(ScalarTerm),
    Operation(ClauseDeclaration),
    Law(MachineId, u32),
    /// The quotient's soundness at the two representatives of one
    /// `QuotientSoundness` row, by its position.
    Soundness(usize),
    /// Builtin `==` of one computational integer type, `T -> T -> bool`.
    IntegerEquality(IntegerType),
    /// The [same-type integer identity](wiki/spec/terminal-psi/integer_certificates.md#same-type-integer-identity)
    /// bridge at one type: `Π x y : T. Id bool (x == y) true -> Id T x y`.
    IntegerIdentity(IntegerType),
    /// Its converse, stated at one subject: `Π x : T. Id bool (x == x) true`.
    /// Rewriting with an identity of `x` and `y` gives `x == y` true.
    IntegerReflexivity(IntegerType),
}

/// The kernel carrier of a record field: either a `ClauseCarrier` that clauses
/// can name directly, or an opaque field carrier (atomic, byte-sequence, or
/// erased) that still contributes to its enclosing record's product identity.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldCarrier {
    Clause(ClauseCarrier),
    Atomic(semantic_vocabulary::ScalarType),
    ByteSequence(StructuralTypeId, StructuralFieldId),
    Erased(TerminalIdentityIndex),
}

#[derive(Clone, Copy, PartialEq)]
enum LawInput {
    Structural(PlaceId),
    Scalar(semantic_vocabulary::ValueId),
    /// Formal `k` of a law in a machine's Identity table, written
    /// `ClauseOperand::Formal(k)`.
    Formal(u32),
}

/// One field of a record denoted as the product of its fields.
#[derive(Clone, Copy)]
struct Field {
    id: StructuralFieldId,
    identity: TerminalIdentityIndex,
    carrier: FieldCarrier,
}

/// A record whose value is the tuple of its fields, in declaration order.
#[derive(Clone)]
struct Product {
    record: StructuralTypeId,
    fields: Vec<Field>,
}

/// Why no proof was found: the search failed, and for an Identity of two
/// records, the first field whose identity nothing proves, outermost first.
struct Unproved {
    error: SearchError,
    field: Vec<TerminalIdentityIndex>,
}

impl From<SearchError> for Unproved {
    fn from(error: SearchError) -> Self {
        Self {
            error,
            field: Vec::new(),
        }
    }
}

struct Declarations<'a> {
    module: &'a TerminalModule,
    machine: &'a TerminalMachine,
    /// What each field of an input held at the goal point. A machine's
    /// Identity table names no input, so it is read without one.
    entry: Option<&'a mut dyn crate::Bindings<Error = crate::Unbound>>,
    /// The values the inputs' fields held at the goal point, as their terms
    /// state them. An equality of one of them can rebuild an input.
    contents: Vec<ScalarTerm>,
    /// Each builtin same-type integer comparison a term names, with its
    /// type and subjects, for the identity bridge.
    integer_comparisons: Vec<(IntegerType, TermHandle, TermHandle, TermHandle)>,
    arena: TermArena,
    entries: Vec<(Named, TermHandle)>,
    signature: Vec<Declaration>,
    products: Vec<Product>,
    /// Records whose carrier is being declared, so a record that contained
    /// itself would be refused rather than declared forever.
    declaring: Vec<StructuralTypeId>,
    /// The const record parameters of the generic check form whose Identity
    /// table the obligation starts from. The table holds every operation of
    /// theirs and every law, so an obligation that names another is refused.
    suppliers: Option<Vec<(u32, u32)>>,
}

/// A generic check form's Identity table, declared and checked by the kernel
/// once (`check_identity_table`). Each Identity obligation of the form starts
/// from a copy and adds only what it names of its own inputs.
pub(crate) struct CheckedIdentityTable {
    suppliers: Vec<(u32, u32)>,
    arena: TermArena,
    /// The table's declarations, every one ordinary and checked by the
    /// kernel together.
    declarations: Vec<Declaration>,
    entries: Vec<(Named, TermHandle)>,
    products: Vec<Product>,
}

/// A generic check form's Identity table, checked, or the reason it could
/// not be.
pub(crate) type IdentityTables = Result<CheckedIdentityTable, String>;

/// The law instances of a module and, for a generic check form, its
/// machines' Identity tables: what an Identity obligation may cite and start
/// from.
#[derive(Clone, Copy)]
pub(crate) struct LawScope<'t> {
    pub(crate) instances: &'t [MachineId],
    pub(crate) identities: Option<&'t IdentityTables>,
}

/// Declares and checks `table` in `module`, the module of its check form,
/// once.
pub(crate) fn check_identity_table(
    module: &TerminalModule,
    table: &IdentityTable,
) -> IdentityTables {
    let machine = module
        .machines
        .first()
        .ok_or("an Identity table for a module with no machine")?;
    let mut declarations = Declarations::new(module, machine, None);
    let read = (|| {
        for &carrier in &table.carriers {
            declarations.carrier(carrier)?;
        }
        for operation in &table.operations {
            let result = declarations.carrier(operation.result)?;
            declarations.operation(operation.declaration, result, &operation.domains)?;
        }
        for law in &table.laws {
            let count = u32::try_from(law.formals.len()).map_err(|_| "too many law formals")?;
            let binders: Vec<_> = (0..count).map(LawInput::Formal).collect();
            let mut statement = declarations.identity(&law.statement, &binders)?;
            for &carrier in law.formals.iter().rev() {
                let domain = declarations.carrier(carrier)?;
                statement = declarations.arena.insert(Term::Pi {
                    domain,
                    codomain: statement,
                });
            }
            declarations.entry(Named::Law(law.instance, law.clause), statement)?;
        }
        Ok::<_, &'static str>(())
    })();
    read.map_err(|why| format!("the machine's Identity table cannot be declared: {why}"))?;
    // A checked table is extended without being checked again, from its
    // declarations alone, which holds only for ordinary declarations: an
    // inductive one is checked with tables of its own.
    if declarations
        .signature
        .iter()
        .any(|declaration| !matches!(declaration.kind, DeclarationKind::Ordinary))
    {
        return Err("the machine's Identity table declares an inductive type".to_owned());
    }
    check_signature(
        &mut declarations.arena,
        &declarations.signature,
        &mut Budget::default(),
    )
    .map_err(|error| {
        format!("mathematical kernel refused the machine's Identity table: {error:?}")
    })?;
    Ok(CheckedIdentityTable {
        suppliers: table
            .suppliers
            .iter()
            .map(|supplier| (supplier.arena_index(), supplier.generation()))
            .collect(),
        arena: declarations.arena,
        declarations: declarations.signature,
        entries: declarations.entries,
        products: declarations.products,
    })
}

impl<'a> Declarations<'a> {
    fn new(
        module: &'a TerminalModule,
        machine: &'a TerminalMachine,
        entry: Option<&'a mut dyn crate::Bindings<Error = crate::Unbound>>,
    ) -> Self {
        Self {
            module,
            machine,
            entry,
            contents: Vec::new(),
            integer_comparisons: Vec::new(),
            arena: TermArena::default(),
            entries: Vec::new(),
            signature: Vec::new(),
            products: Vec::new(),
            declaring: Vec::new(),
            suppliers: None,
        }
    }

    /// An obligation of `machine` starting from its checked Identity table.
    fn from_table(
        module: &'a TerminalModule,
        machine: &'a TerminalMachine,
        table: &CheckedIdentityTable,
        entry: &'a mut dyn crate::Bindings<Error = crate::Unbound>,
    ) -> Self {
        Self {
            module,
            machine,
            entry: Some(entry),
            contents: Vec::new(),
            integer_comparisons: Vec::new(),
            arena: table.arena.clone(),
            entries: table
                .entries
                .iter()
                .map(|(key, term)| (key.clone(), *term))
                .collect(),
            signature: table.declarations.clone(),
            products: table.products.clone(),
            declaring: Vec::new(),
            suppliers: Some(table.suppliers.clone()),
        }
    }

    fn known(&self, key: &Named) -> Option<usize> {
        self.entries.iter().position(|(known, _)| known == key)
    }

    fn entry(&mut self, key: Named, ty: TermHandle) -> Result<TermHandle, &'static str> {
        if let Some(index) = self.known(&key) {
            if !self.arena.structurally_equal(self.signature[index].ty, ty) {
                return Err("the same signature entry has two different types");
            }
            return Ok(self.entries[index].1);
        }
        self.declare(key, Declaration::assumption(0, ty))
    }

    fn declare(
        &mut self,
        key: Named,
        declaration: Declaration,
    ) -> Result<TermHandle, &'static str> {
        // What the table holds is the same in every obligation, so an
        // obligation that names another is refused, never given a
        // declaration of its own.
        if let Some(suppliers) = &self.suppliers {
            match &key {
                Named::Operation(declaration)
                    if declaration
                        .supplier
                        .is_some_and(|supplier| suppliers.contains(&supplier)) =>
                {
                    return Err("a trait operation that is not in the form's Identity table");
                }
                Named::Law(..) => {
                    return Err("a cited law that is not in the form's Identity table");
                }
                _ => {}
            }
        }
        let index =
            u32::try_from(self.signature.len()).map_err(|_| "signature exceeds its index width")?;
        let term = self.arena.insert(Term::Constant {
            declaration: index,
            levels: vec![],
        });
        self.signature.push(declaration);
        self.entries.push((key, term));
        Ok(term)
    }

    /// The fields of a concrete record whose value is the product of all its
    /// fields (including erased, atomic, bounded, view, reference, sum, and
    /// array components). A quotient or restricted scalar stays one opaque
    /// carrier; every field of an ordinary record contributes a component so
    /// an unproved field is refused by name
    /// ([record identity](wiki/spec/proofs/contracts.md#record-identity-from-field-identities)).
    fn product_fields(&self, record: StructuralTypeId) -> Option<Vec<Field>> {
        use semantic_vocabulary::ScalarType;
        use terminal_psi::{StructuralFieldType, StructuralTypeShape};
        let declaration = self.module.structural_type(record)?;
        if declaration.quotient_carrier.is_some() || declaration.scalar_restriction.is_some() {
            return None;
        }
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return None;
        };
        if fields.is_empty() {
            return None;
        }
        Some(
            fields
                .iter()
                .map(|field| {
                    let carrier = match &field.field_type {
                        StructuralFieldType::Scalar(scalar) => {
                            FieldCarrier::Clause(ClauseCarrier::Scalar(*scalar))
                        }
                        StructuralFieldType::IeeeFloat(format) => FieldCarrier::Clause(
                            ClauseCarrier::Scalar(ScalarType::IeeeFloat(*format)),
                        ),
                        StructuralFieldType::BoundedInteger(integer) => FieldCarrier::Clause(
                            ClauseCarrier::Scalar(ScalarType::Integer(integer.integer_type())),
                        ),
                        StructuralFieldType::RefinedScalar(refinement) => {
                            FieldCarrier::Clause(ClauseCarrier::Scalar(refinement.carrier()))
                        }
                        StructuralFieldType::Structural(inner) => {
                            FieldCarrier::Clause(ClauseCarrier::Structural(*inner))
                        }
                        StructuralFieldType::Atomic(scalar) => FieldCarrier::Atomic(*scalar),
                        StructuralFieldType::AtomicBoundedInteger(integer) => {
                            FieldCarrier::Atomic(ScalarType::Integer(integer.integer_type()))
                        }
                        StructuralFieldType::ByteSequence(_)
                        | StructuralFieldType::PredicateByteSequence { .. } => {
                            FieldCarrier::ByteSequence(record, field.id)
                        }
                        StructuralFieldType::Erased { type_identity, .. } => {
                            FieldCarrier::Erased(*type_identity)
                        }
                    };
                    Field {
                        id: field.id,
                        identity: field.identity,
                        carrier,
                    }
                })
                .collect(),
        )
    }

    fn carrier(&mut self, carrier: ClauseCarrier) -> Result<TermHandle, &'static str> {
        self.field_carrier(FieldCarrier::Clause(carrier))
    }

    /// A concrete record is the product of its fields' carriers; every other
    /// carrier (including sums, arrays, views, references, atomics, and erased
    /// fields) is an opaque type.
    fn field_carrier(&mut self, carrier: FieldCarrier) -> Result<TermHandle, &'static str> {
        let key = Named::Carrier(carrier);
        if let Some(index) = self.known(&key) {
            return Ok(self.entries[index].1);
        }
        let universe = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let FieldCarrier::Clause(ClauseCarrier::Structural(record)) = carrier else {
            return self.entry(key, universe);
        };
        let Some(fields) = self.product_fields(record) else {
            return self.entry(key, universe);
        };
        if self.declaring.contains(&record) {
            return Err("a record that contains itself");
        }
        self.declaring.push(record);
        let mut components = Vec::with_capacity(fields.len());
        for field in &fields {
            components.push(self.field_carrier(field.carrier)?);
        }
        self.declaring.pop();
        let body = self
            .arena
            .product_type(&components)
            .ok_or("a product of no fields")?;
        self.products.push(Product { record, fields });
        self.declare(key, Declaration::definition(0, universe, body))
    }

    fn product(&mut self, carrier: FieldCarrier) -> Result<Option<&Product>, &'static str> {
        let FieldCarrier::Clause(ClauseCarrier::Structural(record)) = carrier else {
            return Ok(None);
        };
        self.field_carrier(carrier)?;
        Ok(self
            .products
            .iter()
            .find(|product| product.record == record))
    }

    /// The value at a field path of a record: a projection of the term of
    /// the whole record for each field on the path. A path through anything
    /// that is not a record of fields has no such term.
    fn field_path(
        &mut self,
        mut term: TermHandle,
        mut carrier: ClauseCarrier,
        path: &[CanonicalStructuralPathSegment],
    ) -> Result<(TermHandle, ClauseCarrier), &'static str> {
        for segment in path {
            let CanonicalStructuralPathSegment::Field(field) = segment else {
                return Err("a path step other than a field has no kernel term");
            };
            let product = self
                .product(FieldCarrier::Clause(carrier))?
                .ok_or("a field of a value that is not the product of its fields")?;
            let count = product.fields.len();
            let position = product
                .fields
                .iter()
                .position(|known| known.id == *field)
                .ok_or("a field outside its record")?;
            let FieldCarrier::Clause(next_carrier) = product.fields[position].carrier else {
                return Err("a path step entered a field with no clause carrier");
            };
            carrier = next_carrier;
            term = self.arena.tuple_component(term, position, count);
        }
        Ok((term, carrier))
    }

    /// An input an Identity names at `written`. An immutable input of a
    /// record of fields is the tuple of what its fields held at entry, the
    /// values the entry requirements were stated over; any other input is
    /// one opaque value. A law's formal is its binder. A field path projects
    /// the root, whose carrier is its declared record.
    fn input(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        written: ClauseCarrier,
        binders: &[LawInput],
    ) -> Result<TermHandle, &'static str> {
        let carrier = if path.is_empty() {
            written
        } else {
            ClauseCarrier::Structural(
                self.machine
                    .structural_place_type(root)
                    .ok_or("an input whose root has no declared record")?,
            )
        };
        let ty = self.carrier(carrier)?;
        let whole = if let Some(position) = binders
            .iter()
            .position(|known| *known == LawInput::Structural(root))
        {
            self.arena.insert(Term::Variable(
                u32::try_from(binders.len() - position - 1).map_err(|_| "too many law formals")?,
            ))
        } else if !binders.is_empty() {
            return Err("law names storage outside its telescope");
        } else if self.product(FieldCarrier::Clause(carrier))?.is_some()
            && let Some(contents) =
                self.contents_of(root, &mut Vec::new(), FieldCarrier::Clause(carrier))?
        {
            contents
        } else {
            self.entry(Named::Input(root), ty)?
        };
        let (term, carrier) = self.field_path(whole, carrier, path)?;
        if carrier != written {
            return Err("an Identity operand differs from its written carrier");
        }
        Ok(term)
    }

    /// What the field at `path` beneath `root` held at the goal point, at
    /// `carrier`: the observed value of a scalar field, the tuple of a record
    /// of fields, and otherwise one opaque value of the field.
    fn contents_of(
        &mut self,
        root: PlaceId,
        path: &mut Vec<CanonicalStructuralPathSegment>,
        carrier: FieldCarrier,
    ) -> Result<Option<TermHandle>, &'static str> {
        if let FieldCarrier::Clause(ClauseCarrier::Scalar(scalar)) = carrier {
            let entry = self
                .entry
                .as_deref_mut()
                .ok_or("an Identity table names no input")?;
            let Ok(held) = entry.field(
                semantic_vocabulary::ContentPlaceVersion::Current,
                root,
                path,
                scalar,
            ) else {
                let ty = self.field_carrier(carrier)?;
                return self.entry(Named::Field(root, path.clone()), ty).map(Some);
            };
            let term = self.scalar(&held, &[])?;
            self.contents.push(held);
            return Ok(Some(term));
        }
        let Some(fields) = self.product(carrier)?.map(|product| product.fields.clone()) else {
            let ty = self.field_carrier(carrier)?;
            return self.entry(Named::Field(root, path.clone()), ty).map(Some);
        };
        let mut components = Vec::with_capacity(fields.len());
        for field in fields {
            path.push(CanonicalStructuralPathSegment::Field(field.id));
            let component = self.contents_of(root, path, field.carrier)?;
            path.pop();
            let Some(component) = component else {
                return Ok(None);
            };
            components.push(component);
        }
        Ok(self.arena.tuple(&components))
    }

    /// The entry of an applied declaration, a function from `domains` to
    /// `result`.
    fn operation(
        &mut self,
        declaration: ClauseDeclaration,
        result: TermHandle,
        domains: &[ClauseCarrier],
    ) -> Result<TermHandle, &'static str> {
        let mut function_type = result;
        for &carrier in domains.iter().rev() {
            let domain = self.carrier(carrier)?;
            function_type = self.arena.insert(Term::Pi {
                domain,
                codomain: function_type,
            });
        }
        self.entry(Named::Operation(declaration), function_type)
    }

    fn application(
        &mut self,
        declaration: ClauseDeclaration,
        result: TermHandle,
        domains: &[ClauseCarrier],
        arguments: &[TermHandle],
    ) -> Result<TermHandle, &'static str> {
        let operation = self.operation(declaration, result, domains)?;
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
            ScalarTerm::IntegerEqual {
                scalar_type,
                left,
                right,
            } if bridged_integer(*scalar_type, left, right) => {
                let left = self.scalar(left, binders)?;
                let right = self.scalar(right, binders)?;
                let equality = self.integer_equality(*scalar_type)?;
                let applied = self.arena.apply_arguments(equality, &[left, right]);
                let compared = (*scalar_type, left, right, applied);
                if !self.integer_comparisons.contains(&compared) {
                    self.integer_comparisons.push(compared);
                }
                Ok(applied)
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
            | ScalarTerm::IeeeFloatLiteral(_) => self.entry(Named::Scalar(term.clone()), ty),
            _ if binders.is_empty() => self.entry(Named::Scalar(term.clone()), ty),
            _ => Err("no whole-value mathematical input for this scalar operand"),
        }
    }

    /// Builtin `==` at one computational integer type, as an operation of
    /// the signature.
    fn integer_equality(&mut self, scalar_type: IntegerType) -> Result<TermHandle, &'static str> {
        let subject = self.carrier(ClauseCarrier::Scalar(ScalarType::Integer(scalar_type)))?;
        let boolean = self.carrier(ClauseCarrier::Scalar(ScalarType::Boolean))?;
        let function = self.arena.insert(Term::Pi {
            domain: subject,
            codomain: boolean,
        });
        let function = self.arena.insert(Term::Pi {
            domain: subject,
            codomain: function,
        });
        self.entry(Named::IntegerEquality(scalar_type), function)
    }

    /// Evidence of each same-type identity and true comparison the bridge
    /// gives from what `context` and `citations` already prove
    /// ([same-type integer identity](wiki/spec/terminal-psi/integer_certificates.md#same-type-integer-identity)):
    /// for every comparison a premise or the goal names, its true outcome
    /// at the same subject twice, and, where the premises prove it true at
    /// two subjects, their typed identity.
    fn integer_bridges(
        &mut self,
        context: &Context,
        citations: &[TermHandle],
        budget: &mut Budget,
    ) -> Result<Vec<TermHandle>, &'static str> {
        let mut bridged = Vec::new();
        for (scalar_type, left, right, applied) in self.integer_comparisons.clone() {
            let reflexivity = self.integer_reflexivity(scalar_type)?;
            for subject in [left, right] {
                let evidence = self.arena.apply_arguments(reflexivity, &[subject]);
                if !bridged.contains(&evidence) {
                    bridged.push(evidence);
                }
            }
            let boolean = self.carrier(ClauseCarrier::Scalar(ScalarType::Boolean))?;
            let truth = self.scalar(&ScalarTerm::Boolean(true), &[])?;
            let outcome = self.arena.insert(Term::Id {
                ty: boolean,
                left: applied,
                right: truth,
            });
            let Ok(proof) = prove(&mut self.arena, context, outcome, citations, 4_096, budget)
            else {
                continue;
            };
            let identity = self.integer_identity(scalar_type)?;
            bridged.push(self.arena.apply_arguments(identity, &[left, right, proof]));
        }
        Ok(bridged)
    }

    /// The bridge at each integer type a comparison names, declared before
    /// the signature is checked.
    fn declare_integer_bridges(&mut self) -> Result<(), &'static str> {
        let mut types = self
            .integer_comparisons
            .iter()
            .map(|(scalar_type, ..)| *scalar_type)
            .collect::<Vec<_>>();
        types.dedup();
        for scalar_type in types {
            self.integer_identity(scalar_type)?;
            self.integer_reflexivity(scalar_type)?;
        }
        Ok(())
    }

    /// `Π x y : T. Id bool (x == y) true -> Id T x y`.
    fn integer_identity(&mut self, scalar_type: IntegerType) -> Result<TermHandle, &'static str> {
        let subject = self.carrier(ClauseCarrier::Scalar(ScalarType::Integer(scalar_type)))?;
        let boolean = self.carrier(ClauseCarrier::Scalar(ScalarType::Boolean))?;
        let equality = self.integer_equality(scalar_type)?;
        let truth = self.scalar(&ScalarTerm::Boolean(true), &[])?;
        // Under the binders `x` (1) and `y` (0), then the premise (0).
        let x = self.arena.insert(Term::Variable(1));
        let y = self.arena.insert(Term::Variable(0));
        let compared = self.arena.apply_arguments(equality, &[x, y]);
        let premise = self.arena.insert(Term::Id {
            ty: boolean,
            left: compared,
            right: truth,
        });
        let x = self.arena.insert(Term::Variable(2));
        let y = self.arena.insert(Term::Variable(1));
        let conclusion = self.arena.insert(Term::Id {
            ty: subject,
            left: x,
            right: y,
        });
        let implication = self.arena.insert(Term::Pi {
            domain: premise,
            codomain: conclusion,
        });
        let over_y = self.arena.insert(Term::Pi {
            domain: subject,
            codomain: implication,
        });
        let statement = self.arena.insert(Term::Pi {
            domain: subject,
            codomain: over_y,
        });
        self.entry(Named::IntegerIdentity(scalar_type), statement)
    }

    /// `Π x : T. Id bool (x == x) true`.
    fn integer_reflexivity(
        &mut self,
        scalar_type: IntegerType,
    ) -> Result<TermHandle, &'static str> {
        let subject = self.carrier(ClauseCarrier::Scalar(ScalarType::Integer(scalar_type)))?;
        let boolean = self.carrier(ClauseCarrier::Scalar(ScalarType::Boolean))?;
        let equality = self.integer_equality(scalar_type)?;
        let truth = self.scalar(&ScalarTerm::Boolean(true), &[])?;
        let x = self.arena.insert(Term::Variable(0));
        let compared = self.arena.apply_arguments(equality, &[x, x]);
        let outcome = self.arena.insert(Term::Id {
            ty: boolean,
            left: compared,
            right: truth,
        });
        let statement = self.arena.insert(Term::Pi {
            domain: subject,
            codomain: outcome,
        });
        self.entry(Named::IntegerReflexivity(scalar_type), statement)
    }

    /// `left = right` between two scalar terms, as a kernel Identity at
    /// their scalar carrier.
    fn scalar_identity(
        &mut self,
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Result<TermHandle, &'static str> {
        if left.scalar_type() != right.scalar_type() {
            return Err("an equality between two scalar types");
        }
        let ty = self.carrier(ClauseCarrier::Scalar(left.scalar_type()))?;
        let left = self.scalar(left, &[])?;
        let right = self.scalar(right, &[])?;
        Ok(self.arena.insert(Term::Id { ty, left, right }))
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
                ClauseOperand::Input(input) => {
                    self.input(input.root(), input.path(), value.carrier, binders)?
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
                ClauseOperand::Formal(formal)
                    if let Some(position) = binders
                        .iter()
                        .position(|binder| *binder == LawInput::Formal(*formal)) =>
                {
                    self.arena.insert(Term::Variable(
                        u32::try_from(binders.len() - position - 1)
                            .map_err(|_| "too many law formals")?,
                    ))
                }
                ClauseOperand::Formal(_) | ClauseOperand::FloatMeaning(_) => {
                    return Err("no whole-value mathematical input for this clause operand");
                }
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
            // The supplied value has the same term it has anywhere else in
            // this obligation, so a cited field is the projection the goal
            // names.
            let (binder, operand) = match (&formal.operand, &supplied.operand) {
                (ClauseOperand::Input(input), ClauseOperand::Input(actual))
                    if input.path().is_empty() =>
                {
                    let term = self.input(actual.root(), actual.path(), supplied.carrier, &[])?;
                    (LawInput::Structural(input.root()), term)
                }
                (
                    ClauseOperand::Scalar(ScalarTerm::Value { id, .. }),
                    ClauseOperand::Scalar(actual),
                ) => (LawInput::Scalar(*id), self.scalar(actual, &[])?),
                (left, right) if left == right => continue,
                _ => return Err("citation changed the selected law operation"),
            };
            let position = binders
                .iter()
                .position(|&known| known == binder)
                .ok_or("law has a foreign formal")?;
            let ty = self.carrier(formal.carrier)?;
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
        let law = self.entry(Named::Law(machine, ordinal), statement)?;
        Ok(self.arena.apply_arguments(
            law,
            &actuals.iter().map(|&(_, term)| term).collect::<Vec<_>>(),
        ))
    }

    /// `left = right` at `carrier`. Two records of fields are identical when
    /// every corresponding field is: when no available equality proves the
    /// whole values, each side is rebuilt from its fields and the field
    /// identities are carried into the rebuilt tuple one at a time. The
    /// kernel converts a tuple of a value's projections back into the value.
    /// The quotient's soundness at the representatives of row `row`:
    /// `related → expected`, where `related` is the quotient's relation at
    /// them and `expected` the Identity of their classes. It is declared
    /// before the signature is checked; the proof applies it.
    fn soundness(
        &mut self,
        row: usize,
        related: &ClauseIdentity,
        expected: TermHandle,
    ) -> Result<(TermHandle, TermHandle), &'static str> {
        let statement = self.identity(related, &[])?;
        let codomain = shift(&mut self.arena, expected, 0, 1);
        let soundness = self.arena.insert(Term::Pi {
            domain: statement,
            codomain,
        });
        let sound = self.entry(Named::Soundness(row), soundness)?;
        Ok((statement, sound))
    }

    fn prove_identity(
        &mut self,
        context: &Context,
        carrier: FieldCarrier,
        left: TermHandle,
        right: TermHandle,
        citations: &[TermHandle],
        budget: &mut Budget,
    ) -> Result<TermHandle, Unproved> {
        let ty = self
            .field_carrier(carrier)
            .map_err(|_| SearchError::NoProof)?;
        let goal = self.arena.insert(Term::Id { ty, left, right });
        let whole = prove(&mut self.arena, context, goal, citations, 65_536, budget);
        let Err(SearchError::NoProof) = whole else {
            return whole.map_err(Unproved::from);
        };
        let Some(product) = self.product(carrier).map_err(|_| SearchError::NoProof)? else {
            return Err(SearchError::NoProof.into());
        };
        let fields = product.fields.clone();
        let count = fields.len();
        let mut lefts = Vec::with_capacity(count);
        let mut rights = Vec::with_capacity(count);
        for position in 0..count {
            lefts.push(self.arena.tuple_component(left, position, count));
            rights.push(self.arena.tuple_component(right, position, count));
        }
        let rebuilt = self.arena.tuple(&lefts).ok_or(SearchError::NoProof)?;
        // `rebuilt = rebuilt`, then one field at a time from its left
        // projection to its right one.
        let mut proof = self.arena.insert(Term::Refl { ty, value: rebuilt });
        let mut reached = lefts.clone();
        for (position, field) in fields.iter().enumerate() {
            let component = self
                .prove_identity(
                    context,
                    field.carrier,
                    lefts[position],
                    rights[position],
                    citations,
                    budget,
                )
                .map_err(|mut unproved| {
                    unproved.field.insert(0, field.identity);
                    unproved
                })?;
            let component_type = self
                .field_carrier(field.carrier)
                .map_err(|_| SearchError::NoProof)?;
            // `rebuilt = (.., z, ..)` under `z` and its proof: the fields
            // before this one already at their right projections.
            let mut at = Vec::with_capacity(count);
            for (other, &term) in reached.iter().enumerate() {
                at.push(if other == position {
                    self.arena.insert(Term::Variable(1))
                } else {
                    shift(&mut self.arena, term, 0, 2)
                });
            }
            let target = self.arena.tuple(&at).ok_or(SearchError::NoProof)?;
            let conclusion = Term::Id {
                ty: shift(&mut self.arena, ty, 0, 2),
                left: shift(&mut self.arena, rebuilt, 0, 2),
                right: target,
            };
            let conclusion = self.arena.insert(conclusion);
            proof = Equality {
                carrier: component_type,
                from: lefts[position],
                to: rights[position],
                proof: component,
            }
            .eliminate(&mut self.arena, conclusion, proof);
            reached[position] = rights[position];
        }
        Ok(proof)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn discharge(
    module: &TerminalModule,
    machine: &TerminalMachine,
    tables: &MachineTables,
    graph: &Graph,
    storage: &crate::places::Storage<'_>,
    point: ProgramPoint,
    obligation: ObligationId,
    goal: &ClauseIdentity,
    law_instances: &[MachineId],
    identities: Option<&IdentityTables>,
    entry: &mut dyn crate::Bindings<Error = crate::Unbound>,
) -> Checked<Judgment> {
    let known = hypotheses(graph, tables, machine.id, obligation, point)?;
    let unavailable = |why: &str| {
        crate::refusal(
            RefusalReason::Unsupported,
            machine.id,
            Construct::Obligation(obligation),
            why,
        )
    };
    // A machine of a generic check form with const record parameters
    // starts from the form's Identity table, which the kernel checked once.
    // Any other machine has no trait operation or law to share and declares
    // what each obligation names.
    let table = identities
        .map(|table| table.as_ref().map_err(|why| unavailable(why)))
        .transpose()?;
    let mut declarations = match table {
        Some(table) => Declarations::from_table(module, machine, table, entry),
        None => Declarations::new(module, machine, Some(entry)),
    };
    let expected = declarations.identity(goal, &[]).map_err(unavailable)?;
    let mut proofs = Vec::new();
    let mut citations = Vec::new();
    let mut premises = Vec::new();
    for (&row, premise) in known.assumption_rows.iter().zip(&known.assumptions) {
        let justification = tables.knowledge.get(row).map(|row| &row.justification);
        // A FloatMeaning Identity belongs to the float meaning prover.
        if matches!(premise, Proposition::DeclarationIdentity(actual) if actual.is_float_meaning())
        {
            continue;
        }
        if let Some(Justification::EntryRequirement { clause }) = justification
            && let Proposition::DeclarationIdentity(actual) = premise
        {
            // Only the caller owes this hypothesis. Rejoin the row to this
            // machine's published input clause before binding its proof.
            // A clause over an input's field, such as a raw float Identity
            // of `a.x`, is known over the value that field held, as these
            // bindings state it.
            let declared = machine.requires.get(*clause as usize);
            let mut stated = || {
                let declared = declared?;
                let entry = declarations.entry.as_deref_mut()?;
                crate::entry_snapshot_conjuncts(declared, entry).ok()
            };
            if declared != Some(premise) && !stated().is_some_and(|stated| stated.contains(premise))
            {
                return Err(unavailable(
                    "entry equality differs from its declared requirement",
                ));
            }
            premises.push(declarations.identity(actual, &[]).map_err(unavailable)?);
            citations.push(row);
            continue;
        }
        let Proposition::DeclarationIdentity(actual) = premise else {
            continue;
        };
        // The result of an earlier call of a total comparison is the same
        // application of its operation to the same, unmodified inputs
        // (`contracts.md#equality-bridges-and-comparison-facts`).
        let (invocation, clause, congruence) = match justification {
            Some(Justification::CalleeGuarantee { invocation, clause }) => {
                (invocation, Some(clause), false)
            }
            // A sibling's own guarantee names no clause of the call's
            // contract, so it cites no law; it is still owed by that body.
            Some(Justification::RealizationGuarantee { invocation, .. }) => {
                (invocation, None, false)
            }
            Some(Justification::CallCongruence { invocation }) => (invocation, None, true),
            _ => continue,
        };
        let call = tables
            .invocations
            .get(*invocation)
            .ok_or_else(|| unavailable("citation lost its invocation"))?;
        if let Some(clause) = clause
            && let Callee::Declared(TargetMachineCallable::Machine(callee)) = call.callee
            && law_instances.binary_search(&callee).is_ok()
        {
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
            continue;
        }
        // A cited theorem or helper whose requirements were already proved at
        // the call site supplies the same field or value identity premise as
        // an entry requirement (`contracts.md#record-identity-from-field-identities`),
        // provided every local place it names is unmodified since the call.
        // A repeated comparison relates the values the call read, which are
        // the entry values only if nothing in the machine writes them, a
        // structural parameter declared `mutable` included.
        let fresh = actual.values.iter().all(|value| match &value.operand {
            ClauseOperand::Input(subject) => {
                (!congruence && machine.structural_parameter(subject.root()).is_some())
                    || matches!(
                        call.site,
                        terminal_check_tables::InvocationSite::Operation(operation)
                            if storage.unmodified_since(subject.root(), operation)
                    )
            }
            _ => true,
        });
        if !fresh {
            continue;
        }
        premises.push(declarations.identity(actual, &[]).map_err(unavailable)?);
        citations.push(row);
    }
    // An equality of a value an input's field held at entry, such as the
    // instantiated requirement `a.x == b.x`, is a field identity the goal
    // may rebuild a record from. Other scalar equalities stay with the
    // arithmetic prover.
    for (&row, premise) in known.assumption_rows.iter().zip(&known.assumptions) {
        let Proposition::Equal(left, right) = premise else {
            continue;
        };
        // What a sibling member of the implementation ensures of a call's
        // Boolean result, such as `ensures result`, is an identity the
        // repeated comparison's result joins (`RealizedGuarantee`).
        let realized = left.scalar_type() == semantic_vocabulary::ScalarType::Boolean
            && matches!(
                tables.knowledge.get(row).map(|known| &known.justification),
                Some(Justification::RealizationGuarantee { .. })
            );
        if realized {
            if let Ok(premise) = declarations.scalar_identity(left, right) {
                premises.push(premise);
                citations.push(row);
            }
            continue;
        }
        if !declarations
            .contents
            .iter()
            .any(|held| held == left || held == right)
        {
            continue;
        }
        premises.push(
            declarations
                .scalar_identity(left, right)
                .map_err(unavailable)?,
        );
        citations.push(row);
    }
    // A float value defined as a literal or as another value holds exactly
    // those bits, which is its raw identity (wiki/spec/terminal-psi/mathematical_values.md,
    // "Float equality and source identity"). A float computed by arithmetic
    // stays opaque: its NaN payload is not fixed by its operands.
    for (&row, axiom) in known.axiom_rows.iter().zip(&known.axioms) {
        let Proposition::Equal(left, right) = axiom else {
            continue;
        };
        let exact = |term: &ScalarTerm| {
            matches!(
                term,
                ScalarTerm::Value { .. } | ScalarTerm::IeeeFloatLiteral(_)
            )
        };
        if !matches!(
            left.scalar_type(),
            semantic_vocabulary::ScalarType::IeeeFloat(_)
        ) || left.scalar_type() != right.scalar_type()
            || !exact(left)
            || !exact(right)
        {
            continue;
        }
        premises.push(
            declarations
                .scalar_identity(left, right)
                .map_err(unavailable)?,
        );
        citations.push(row);
    }
    // Two classes of a quotient of data that has storage: the soundness
    // that relates them, declared with everything else the obligation
    // names.
    let soundness = match tables
        .quotient_soundness
        .iter()
        .position(|row| row.classes == *goal)
    {
        Some(position) => {
            let related = &tables.quotient_soundness[position].related;
            let (statement, sound) = declarations
                .soundness(position, related, expected)
                .map_err(unavailable)?;
            let carrier = FieldCarrier::Clause(related.values[related.left as usize].carrier);
            Some((statement, sound, carrier))
        }
        None => None,
    };
    let mut budget = Budget::default();
    let field_name = |identity: TerminalIdentityIndex| {
        let text = module.identities.text(identity).unwrap_or("?");
        text.rsplit(['.', ':', '\\'])
            .next()
            .unwrap_or(text)
            .to_owned()
    };
    let kernel_error = |error| match error {
        SearchError::Budget(limit) => crate::unproved(
            machine.id,
            obligation,
            UnprovedCause::BudgetExhausted,
            limit.describe(),
        ),
        SearchError::NoProof => crate::unproved(
            machine.id,
            obligation,
            UnprovedCause::NoDerivation,
            "no available equality proves the stated Identity".into(),
        ),
        SearchError::Kernel(error) => crate::unproved(
            machine.id,
            obligation,
            UnprovedCause::KernelRefused,
            format!("mathematical kernel: {error:?}"),
        ),
    };
    // A record Identity that no equality proves names the first field
    // whose identity is missing.
    let refused = |unproved: Unproved| {
        if unproved.error != SearchError::NoProof || unproved.field.is_empty() {
            return kernel_error(unproved.error);
        }
        let path: Vec<_> = unproved.field.iter().map(|&f| field_name(f)).collect();
        crate::unproved(
            machine.id,
            obligation,
            UnprovedCause::NoDerivation,
            format!(
                "no available equality proves the stated Identity: \
                 nothing proves the identity of field `{}`",
                path.join(".")
            ),
        )
    };
    declarations
        .declare_integer_bridges()
        .map_err(unavailable)?;
    // Only what this obligation added after the table is checked here.
    let signature = match table {
        Some(table) => extend_signature(
            &mut declarations.arena,
            &Signature::from_declarations(table.declarations.clone()),
            &declarations.signature[table.declarations.len()..],
            &mut budget,
        )
        .map_err(|refused| refused.error),
        None => check_signature(
            &mut declarations.arena,
            &declarations.signature,
            &mut budget,
        ),
    }
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
    // What the premises give through the same-type integer identity bridge.
    let bridged = declarations
        .integer_bridges(&context, &proofs, &mut budget)
        .map_err(unavailable)?;
    proofs.extend(bridged);
    let Term::Id { left, right, .. } = declarations.arena.get(expected) else {
        unreachable!("an Identity denotes kernel Id")
    };
    let proved = declarations.prove_identity(
        &context,
        FieldCarrier::Clause(goal.values[goal.left as usize].carrier),
        left,
        right,
        &proofs,
        &mut budget,
    );
    let proof = match proved {
        Ok(proof) => proof,
        // Two classes of a quotient of data that has storage are equal when
        // their representatives are related: the quotient's soundness at
        // them, applied to a proof of the relation.
        Err(unproved) if unproved.error == SearchError::NoProof => {
            let Some((statement, sound, carrier)) = soundness else {
                return Err(refused(unproved));
            };
            let Term::Id { left, right, .. } = declarations.arena.get(statement) else {
                unreachable!("an Identity denotes kernel Id")
            };
            let evidence = declarations
                .prove_identity(&context, carrier, left, right, &proofs, &mut budget)
                .map_err(refused)?;
            declarations.arena.apply_arguments(sound, &[evidence])
        }
        Err(unproved) => return Err(refused(unproved)),
    };
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

#[cfg(test)]
mod integer_bridge_tests {
    use super::bridged_integer;
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarTerm, ScalarType, ValueId};

    fn subject(position: u64, scalar_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(position).expect("a value id"),
            ScalarType::Integer(scalar_type),
        )
    }

    /// The bridge has two subjects of the comparison's own integer type.
    /// `7i == 7u` is true, but its operands cannot form one identity.
    #[test]
    fn the_bridge_takes_only_two_operands_of_one_integer_type() {
        let narrow = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let wide = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
        let address = IntegerType::supported_address_representation();
        assert!(bridged_integer(
            narrow,
            &subject(1, narrow),
            &subject(2, narrow)
        ));
        assert!(!bridged_integer(
            narrow,
            &subject(1, narrow),
            &subject(2, wide)
        ));
        assert!(!bridged_integer(
            wide,
            &subject(1, narrow),
            &subject(2, wide)
        ));
        assert!(!bridged_integer(
            narrow,
            &subject(1, narrow),
            &subject(2, unsigned)
        ));
        assert!(!bridged_integer(
            address,
            &subject(1, address),
            &subject(2, address)
        ));
    }
}
