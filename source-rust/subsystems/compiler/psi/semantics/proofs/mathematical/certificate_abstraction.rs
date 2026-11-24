//! A theorem must hold for arbitrary supplied subjects, not fixed global
//! constants. Turn explicitly selected assumptions into ordinary Pi/Lambda
//! binders, retaining every independent assumption and checking both judgments.

use crate::{
    Budget, Context, CoreError, Declaration, MathematicalCertificate, Signature, Term, TermArena,
    TermHandle, check_signature, infer_sort, verify_mathematical_certificate,
};

#[derive(Debug)]
pub struct AbstractedCertificate {
    pub certificate: MathematicalCertificate,
    /// Original declaration positions map only to their retained global rows.
    /// Abstracted inputs and inlined definitions no longer need global grants.
    pub retained_declarations: Vec<Option<u32>>,
}

/// A type judgment has no proof term. Forming a required theorem type must
/// not require a proof of the conclusion it asks its caller to supply.
pub struct MathematicalJudgmentType<'a> {
    pub signature: &'a [Declaration],
    pub level_arity: u32,
    pub context: &'a [TermHandle],
    pub ty: TermHandle,
}

pub struct AbstractedType {
    pub signature: Vec<Declaration>,
    pub level_arity: u32,
    pub ty: TermHandle,
    pub retained_declarations: Vec<Option<u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateAbstractionError {
    Kernel(CoreError),
    UnknownAssumption(u32),
    DuplicateAssumption(u32),
    NotAnAssumption(u32),
    PolymorphicAssumption(u32),
    DependentAssumption(u32),
    LaterBinder(u32),
    DepthOverflow,
}

impl From<CoreError> for CertificateAbstractionError {
    fn from(error: CoreError) -> Self {
        Self::Kernel(error)
    }
}

impl std::fmt::Display for CertificateAbstractionError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "{self:?}")
    }
}

impl std::error::Error for CertificateAbstractionError {}

#[derive(Clone, Copy)]
enum Binding {
    Global(u32),
    Bound(u32),
    Definition(TermHandle),
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Scope {
    available: u32,
    context: u32,
    depth: u32,
}

impl Scope {
    fn nested(self, binders: u32) -> Result<Self, CertificateAbstractionError> {
        Ok(Self {
            depth: self
                .depth
                .checked_add(binders)
                .ok_or(CertificateAbstractionError::DepthOverflow)?,
            ..self
        })
    }
}

struct Rewriter<'a> {
    arena: &'a mut TermArena,
    bindings: &'a [Binding],
    budget: &'a mut Budget,
    // Scratch belongs to the frozen input arena, before checking appends terms.
    by_slot: Vec<Option<usize>>,
    rewritten: Vec<Rewritten>,
}

struct Rewritten {
    scope: Scope,
    result: TermHandle,
    next: Option<usize>,
}

fn depends_on_selected(
    arena: &TermArena,
    root: TermHandle,
    declarations: &[bool],
    memo: &mut [Option<bool>],
    budget: &mut Budget,
) -> Result<bool, CertificateAbstractionError> {
    let mut pending = vec![(root, false)];
    while let Some((handle, finish)) = pending.pop() {
        budget.consume()?;
        let node = arena.try_node(handle).ok_or(CoreError::DummyTerm)?;
        let slot = handle.arena_index() as usize;
        if memo.get(slot).ok_or(CoreError::DummyTerm)?.is_some() {
            continue;
        }
        if !finish {
            pending.push((handle, true));
            pending.extend(node.children().map(|child| (child, false)));
            continue;
        }
        let dependent = if let Term::Constant { declaration, .. } = node {
            *declarations
                .get(*declaration as usize)
                .ok_or(CoreError::UnknownDeclaration {
                    declaration: *declaration,
                    signature_len: declarations.len(),
                })?
        } else {
            node.children().any(|child| {
                memo[child.arena_index() as usize]
                    .expect("the verified input's children were visited")
            })
        };
        memo[slot] = Some(dependent);
    }
    Ok(memo[root.arena_index() as usize].expect("the root was visited"))
}

impl Rewriter<'_> {
    fn find(&self, handle: TermHandle, scope: Scope) -> Result<Option<TermHandle>, CoreError> {
        let mut row = *self
            .by_slot
            .get(handle.arena_index() as usize)
            .ok_or(CoreError::DummyTerm)?;
        while let Some(index) = row {
            let saved = &self.rewritten[index];
            if saved.scope == scope {
                return Ok(Some(saved.result));
            }
            row = saved.next;
        }
        Ok(None)
    }

    fn remember(&mut self, handle: TermHandle, scope: Scope, result: TermHandle) {
        let slot = &mut self.by_slot[handle.arena_index() as usize];
        let index = self.rewritten.len();
        self.rewritten.push(Rewritten {
            scope,
            result,
            next: *slot,
        });
        *slot = Some(index);
    }

    fn term(
        &mut self,
        root: TermHandle,
        scope: Scope,
    ) -> Result<TermHandle, CertificateAbstractionError> {
        enum Work {
            Visit(TermHandle, Scope),
            Finish {
                original: TermHandle,
                scope: Scope,
                node: Term,
                first: usize,
            },
            Alias(TermHandle, Scope),
        }
        let mut pending = vec![Work::Visit(root, scope)];
        let mut values = Vec::new();
        while let Some(work) = pending.pop() {
            self.budget.consume()?;
            match work {
                Work::Visit(handle, scope) => {
                    let node = self
                        .arena
                        .try_node(handle)
                        .ok_or(CoreError::DummyTerm)?
                        .clone();
                    if let Some(result) = self.find(handle, scope)? {
                        values.push(result);
                        continue;
                    }
                    if let Term::Constant {
                        declaration,
                        levels,
                    } = &node
                    {
                        let binding = self.bindings.get(*declaration as usize).ok_or(
                            CoreError::UnknownDeclaration {
                                declaration: *declaration,
                                signature_len: self.bindings.len(),
                            },
                        )?;
                        let replacement = match *binding {
                            Binding::Global(mapped) => Term::Constant {
                                declaration: mapped,
                                levels: levels.clone(),
                            },
                            Binding::Bound(ordinal) => {
                                let relative = scope.available.checked_sub(ordinal + 1).ok_or(
                                    CertificateAbstractionError::LaterBinder(*declaration),
                                )?;
                                Term::Variable(
                                    scope
                                        .depth
                                        .checked_add(scope.context)
                                        .and_then(|depth| depth.checked_add(relative))
                                        .ok_or(CertificateAbstractionError::DepthOverflow)?,
                                )
                            }
                            Binding::Definition(body) => {
                                // A dependent definition is expanded under this
                                // exact context; it adds no hypothesis or grant.
                                if !levels.is_empty() {
                                    return Err(
                                        CertificateAbstractionError::PolymorphicAssumption(
                                            *declaration,
                                        ),
                                    );
                                }
                                pending.push(Work::Alias(handle, scope));
                                pending.push(Work::Visit(body, scope));
                                continue;
                            }
                        };
                        let result = self.arena.insert(replacement);
                        self.remember(handle, scope, result);
                        values.push(result);
                        continue;
                    }
                    let children = node.subterms();
                    pending.push(Work::Finish {
                        original: handle,
                        scope,
                        node,
                        first: values.len(),
                    });
                    for (child, binders) in children.iter().rev() {
                        pending.push(Work::Visit(child, scope.nested(binders)?));
                    }
                }
                Work::Finish {
                    original,
                    scope,
                    node,
                    first,
                } => {
                    let result = self.arena.insert(node.with_children(&values[first..]));
                    values.truncate(first);
                    values.push(result);
                    self.remember(original, scope, result);
                }
                Work::Alias(original, scope) => {
                    let result = *values.last().expect("an inlined definition has one result");
                    self.remember(original, scope, result);
                }
            }
        }
        Ok(values.pop().expect("a root has one rewritten term"))
    }
}

/// Select assumptions in the caller's intended binder order. A retained global
/// assumption cannot depend on a new local input: the caller must explicitly
/// include that premise too. Definitions may follow their subjects by inlining.
/// The original proof context closes after those new inputs, in its own order.
/// Neither a successful source query nor an equal declaration number is proof.
pub fn abstract_certificate_assumptions(
    arena: &mut TermArena,
    certificate: &MathematicalCertificate,
    selected: &[u32],
    budget: &mut Budget,
) -> Result<AbstractedCertificate, CertificateAbstractionError> {
    let source_slots = arena
        .len()
        .checked_add(1)
        .ok_or(CertificateAbstractionError::DepthOverflow)?;
    verify_mathematical_certificate(arena, certificate, budget)?;
    let mut selection = vec![None; certificate.signature.len()];
    for (ordinal, index) in selected.iter().copied().enumerate() {
        let declaration = certificate
            .signature
            .get(index as usize)
            .ok_or(CertificateAbstractionError::UnknownAssumption(index))?;
        if declaration.body.is_some() {
            return Err(CertificateAbstractionError::NotAnAssumption(index));
        }
        if declaration.level_arity != 0 {
            return Err(CertificateAbstractionError::PolymorphicAssumption(index));
        }
        if selection[index as usize].is_some() {
            return Err(CertificateAbstractionError::DuplicateAssumption(index));
        }
        selection[index as usize] =
            Some(u32::try_from(ordinal).map_err(|_| CertificateAbstractionError::DepthOverflow)?);
    }
    let mut dependent = Vec::with_capacity(certificate.signature.len());
    let mut bindings = Vec::with_capacity(certificate.signature.len());
    let mut retained_declarations = Vec::with_capacity(certificate.signature.len());
    let mut global_count = 0u32;
    let mut dependencies = vec![None; source_slots];
    for (index, declaration) in certificate.signature.iter().enumerate() {
        let mut depends = selection[index].is_some();
        for root in std::iter::once(declaration.ty).chain(declaration.body) {
            depends |= depends_on_selected(arena, root, &dependent, &mut dependencies, budget)?;
        }
        dependent.push(depends);
        if let Some(ordinal) = selection[index] {
            bindings.push(Binding::Bound(ordinal));
            retained_declarations.push(None);
        } else if depends {
            let body = declaration
                .body
                .ok_or(CertificateAbstractionError::DependentAssumption(
                    u32::try_from(index).map_err(|_| CertificateAbstractionError::DepthOverflow)?,
                ))?;
            if declaration.level_arity != 0 {
                return Err(CertificateAbstractionError::PolymorphicAssumption(
                    u32::try_from(index).map_err(|_| CertificateAbstractionError::DepthOverflow)?,
                ));
            }
            bindings.push(Binding::Definition(body));
            retained_declarations.push(None);
        } else {
            bindings.push(Binding::Global(global_count));
            retained_declarations.push(Some(global_count));
            global_count = global_count
                .checked_add(1)
                .ok_or(CertificateAbstractionError::DepthOverflow)?;
        }
    }
    let mut rewrite = Rewriter {
        arena,
        bindings: &bindings,
        budget,
        by_slot: vec![None; source_slots],
        rewritten: Vec::with_capacity(source_slots),
    };
    let empty = Scope {
        available: 0,
        context: 0,
        depth: 0,
    };
    let mut signature = Vec::with_capacity(global_count as usize);
    for (index, declaration) in certificate.signature.iter().enumerate() {
        if retained_declarations[index].is_some() {
            signature.push(Declaration {
                level_arity: declaration.level_arity,
                ty: rewrite.term(declaration.ty, empty)?,
                body: declaration
                    .body
                    .map(|body| rewrite.term(body, empty))
                    .transpose()?,
            });
        }
    }
    let count =
        u32::try_from(selected.len()).map_err(|_| CertificateAbstractionError::DepthOverflow)?;
    let mut context = Vec::with_capacity(selected.len() + certificate.context.len());
    for (ordinal, index) in selected.iter().copied().enumerate() {
        context.push(
            rewrite.term(
                certificate.signature[index as usize].ty,
                Scope {
                    available: u32::try_from(ordinal)
                        .map_err(|_| CertificateAbstractionError::DepthOverflow)?,
                    ..empty
                },
            )?,
        );
    }
    for (position, binding) in certificate.context.iter().copied().enumerate() {
        context.push(
            rewrite.term(
                binding,
                Scope {
                    available: count,
                    context: u32::try_from(position)
                        .map_err(|_| CertificateAbstractionError::DepthOverflow)?,
                    depth: 0,
                },
            )?,
        );
    }
    let scope = Scope {
        available: count,
        context: u32::try_from(certificate.context.len())
            .map_err(|_| CertificateAbstractionError::DepthOverflow)?,
        depth: 0,
    };
    let mut term = rewrite.term(certificate.term, scope)?;
    let mut expected = rewrite.term(certificate.expected, scope)?;
    for domain in context.into_iter().rev() {
        term = rewrite.arena.insert(Term::Lambda { domain, body: term });
        expected = rewrite.arena.insert(Term::Pi {
            domain,
            codomain: expected,
        });
    }
    let certificate = MathematicalCertificate {
        signature,
        level_arity: certificate.level_arity,
        context: Vec::new(),
        term,
        expected,
    };
    verify_mathematical_certificate(rewrite.arena, &certificate, rewrite.budget)?;
    Ok(AbstractedCertificate {
        certificate,
        retained_declarations,
    })
}

/// Close the same subject and premise telescope for a type alone. The checked
/// type is first abstracted as a type-valued function; replacing its outer
/// lambdas with Pi binders forms the requested implication, without inventing
/// a witness of its conclusion.
pub fn abstract_judgment_type(
    arena: &mut TermArena,
    source: MathematicalJudgmentType<'_>,
    selected: &[u32],
    budget: &mut Budget,
) -> Result<AbstractedType, CertificateAbstractionError> {
    let signature = check_signature(arena, source.signature, budget)?;
    let mut context = Context::with_level_arity(source.level_arity).with_signature(signature);
    for &binding in source.context {
        infer_sort(arena, &context, binding, budget)?;
        context = context.extend(binding);
    }
    let sort = infer_sort(arena, &context, source.ty, budget)?;
    let expected = arena.insert(Term::Sort(sort));
    let abstracted = abstract_certificate_assumptions(
        arena,
        &MathematicalCertificate {
            signature: source.signature.to_vec(),
            level_arity: source.level_arity,
            context: source.context.to_vec(),
            term: source.ty,
            expected,
        },
        selected,
        budget,
    )?;
    let count = selected
        .len()
        .checked_add(source.context.len())
        .ok_or(CertificateAbstractionError::DepthOverflow)?;
    let mut ty = abstracted.certificate.term;
    let mut domains = Vec::with_capacity(count);
    for _ in 0..count {
        let Term::Lambda { domain, body } = arena.get(ty) else {
            return Err(CertificateAbstractionError::Kernel(CoreError::DummyTerm));
        };
        domains.push(domain);
        ty = body;
    }
    for domain in domains.into_iter().rev() {
        ty = arena.insert(Term::Pi {
            domain,
            codomain: ty,
        });
    }
    let context = Context::with_level_arity(source.level_arity).with_signature(
        Signature::from_declarations(abstracted.certificate.signature.clone()),
    );
    infer_sort(arena, &context, ty, budget)?;
    Ok(AbstractedType {
        signature: abstracted.certificate.signature,
        level_arity: source.level_arity,
        ty,
        retained_declarations: abstracted.retained_declarations,
    })
}
