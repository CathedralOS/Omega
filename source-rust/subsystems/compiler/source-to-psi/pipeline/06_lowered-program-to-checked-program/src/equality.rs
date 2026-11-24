//! Rewrite a goal with the equality terms available at that program point.
//! Each visited expression carries a proof from the original left endpoint.

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

/// What each declaration of a signature is to the normalizer.
pub(crate) struct Definitions(Vec<Definition>);

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

/// One rewrite of the term at the subterm reached by taking these child
/// slots in turn.
struct Step {
    slots: Vec<usize>,
    rule: Equality,
}

fn licensed<'l>(
    arena: &TermArena,
    licenses: &'l [License],
    term: TermHandle,
) -> Option<(&'l License, TermHandle, TermHandle)> {
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
/// chains: nested to the right and, where the operation also commutes, with
/// operands in `order`. One call returns one regrouping, or the three
/// rewrites that swap two neighbours. `None` means the term is canonical.
fn next_rearrangement(
    arena: &mut TermArena,
    licenses: &[License],
    term: TermHandle,
    slots: &mut Vec<usize>,
) -> Option<Vec<Step>> {
    use std::cmp::Ordering::Greater;
    if let Some((license, first, second)) = licensed(arena, licenses, term) {
        let license = *license;
        // op(op(x, y), z) regroups to op(x, op(y, z)).
        if let Some((inner, x, y)) = licensed(arena, licenses, first)
            && arena.structurally_equal(inner.operation, license.operation)
            && let Some(rule) = associativity(arena, &license, x, y, second)
        {
            return Some(vec![Step {
                slots: slots.clone(),
                rule,
            }]);
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
                        return Some(vec![
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
                        ]);
                    }
                }
                None => {
                    if order(arena, first, second) == Greater
                        && let Some(rule) = commutativity(arena, &license, first, second)
                    {
                        return Some(vec![Step {
                            slots: slots.clone(),
                            rule,
                        }]);
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
            let found = next_rearrangement(arena, licenses, child, slots);
            slots.pop();
            if found.is_some() {
                return found;
            }
        }
    }
    None
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
/// reach it in order.
fn rearrange(
    arena: &mut TermArena,
    licenses: &[License],
    mut term: TermHandle,
    search_steps: &mut usize,
) -> Result<(TermHandle, Vec<Step>), SearchError> {
    let mut taken = Vec::new();
    if licenses.is_empty() {
        return Ok((term, taken));
    }
    while let Some(steps) = next_rearrangement(arena, licenses, term, &mut Vec::new()) {
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

/// Both sides of the goal and both endpoints of every citation are
/// normalized first. The search then rewrites one side with the citations,
/// beneath any term former and any binder. A term it reaches proves the goal
/// when it and the other side have one canonical form under the licensed
/// operations' laws. Every rewrite, a citation's or a law's, is an identity
/// elimination, so the result is one kernel term from the left side to the
/// right. The term is a candidate: the caller has the kernel check it, once,
/// with everything else it is part of.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_under(
    arena: &mut TermArena,
    context: &Context,
    definitions: &Definitions,
    goal: TermHandle,
    citations: &[TermHandle],
    licenses: &[License],
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
    let (canonical, from_right) = rearrange(arena, licenses, right, &mut search_steps)?;
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
        if licenses.is_empty() {
            return Ok(None);
        }
        let (reached, forward) = rearrange(arena, licenses, term, search_steps)?;
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
    if let Some(proof) = close(arena, left, reflexivity, &mut search_steps)? {
        return Ok(proof);
    }
    let mut rules = Vec::new();
    for &proof in citations {
        charge(&mut search_steps)?;
        let inferred = infer_type(arena, context, proof, conversion)?;
        let head = weak_head_normalize(arena, context.signature(), inferred, conversion)?;
        if let Term::Id {
            ty: carrier,
            left: from,
            right: to,
        } = arena.get(head)
        {
            let equality = Equality {
                carrier,
                from: normal(arena, context, definitions, from, conversion)?,
                to: normal(arena, context, definitions, to, conversion)?,
                proof,
            };
            rules.extend([equality, equality.reversed(arena)]);
        }
    }
    if rules.is_empty() {
        return Err(SearchError::NoProof);
    }
    let mut reached = vec![(left, reflexivity)];
    let mut next = 0;
    while let Some(&(current, base)) = reached.get(next) {
        next += 1;
        let mut pending = vec![(current, Vec::new(), 0u32)];
        while let Some((focus, path, depth)) = pending.pop() {
            for &rule in &rules {
                charge(&mut search_steps)?;
                // A citation is stated outside every binder the focus is under.
                if !arena.structurally_equal_shifted(rule.from, focus, depth) {
                    continue;
                }
                let replacement = shift(arena, rule.to, 0, depth);
                let rewritten = plug(arena, &path, replacement, 0);
                let mut seen = false;
                for &(known, _) in &reached {
                    charge(&mut search_steps)?;
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
                if let Some(proof) = close(arena, rewritten, proof, &mut search_steps)? {
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
