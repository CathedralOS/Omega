use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use super::conversion::Budget;
use super::term::{Term, TermArena, TermHandle};
use super::typing::{Context, CoreError, check_type, infer_sort};

#[derive(Clone, Debug, PartialEq, Eq, record_storage::Record)]
pub struct Declaration {
    pub level_arity: u32,
    pub ty: TermHandle,
    pub body: Option<TermHandle>,
}

impl Declaration {
    pub fn definition(level_arity: u32, ty: TermHandle, body: TermHandle) -> Self {
        Self {
            level_arity,
            ty,
            body: Some(body),
        }
    }

    pub fn assumption(level_arity: u32, ty: TermHandle) -> Self {
        Self {
            level_arity,
            ty,
            body: None,
        }
    }

    pub fn is_assumption(&self) -> bool {
        self.body.is_none()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Signature {
    declarations: Rc<Vec<Declaration>>,
}

impl Signature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_declarations(declarations: Vec<Declaration>) -> Self {
        Self {
            declarations: Rc::new(declarations),
        }
    }

    pub fn get(&self, index: u32) -> Option<&Declaration> {
        self.declarations.get(usize::try_from(index).ok()?)
    }

    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    fn push(&mut self, declaration: Declaration) {
        Rc::make_mut(&mut self.declarations).push(declaration);
    }
}

#[derive(Debug)]
pub struct SignatureCheckError {
    pub declaration: usize,
    pub error: CoreError,
}

pub fn check_signature(
    arena: &mut TermArena,
    declarations: &[Declaration],
    budget: &mut Budget,
) -> Result<Signature, CoreError> {
    check_signature_with_attribution(arena, declarations, budget)
        .map_err(|failure| failure.error)
}

pub fn check_signature_with_attribution(
    arena: &mut TermArena,
    declarations: &[Declaration],
    budget: &mut Budget,
) -> Result<Signature, SignatureCheckError> {
    let mut signature = Signature::new();
    for (position, declaration) in declarations.iter().enumerate() {
        let formed = {
            let context = Context::with_level_arity(declaration.level_arity)
                .with_signature(signature.clone());
            (|| {
                infer_sort(arena, &context, declaration.ty, budget)?;
                if let Some(body) = declaration.body {
                    check_type(arena, &context, body, declaration.ty, budget)?;
                }
                Ok::<_, CoreError>(())
            })()
        };
        formed.map_err(|error| SignatureCheckError {
            declaration: position,
            error,
        })?;
        signature.push(declaration.clone());
    }
    Ok(signature)
}

struct TermReferenceWalk<'arena> {
    arena: &'arena TermArena,
    seen: HashSet<TermHandle>,
    pending: Vec<TermHandle>,
}

impl<'arena> TermReferenceWalk<'arena> {
    fn new(arena: &'arena TermArena) -> Self {
        Self {
            arena,
            seen: HashSet::new(),
            pending: Vec::new(),
        }
    }

    fn collect(&mut self, term: TermHandle, out: &mut BTreeSet<u32>) {
        self.pending.push(term);
        while let Some(term) = self.pending.pop() {
            if !self.seen.insert(term) {
                continue;
            }
            let node = self.arena.node(term);
            if let Term::Constant { declaration, .. } = node {
                out.insert(*declaration);
            } else {
                self.pending
                    .extend(node.subterms().iter().rev().map(|(child, _)| child));
            }
        }
    }
}

fn closure_over(
    references: &mut TermReferenceWalk<'_>,
    signature: &Signature,
    seeds: BTreeSet<u32>,
) -> BTreeSet<u32> {
    let mut assumptions = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut worklist: Vec<u32> = seeds.into_iter().collect();
    while let Some(index) = worklist.pop() {
        if !visited.insert(index) {
            continue;
        }
        let Some(declaration) = signature.get(index) else {
            continue;
        };
        if declaration.is_assumption() {
            assumptions.insert(index);
        }
        let mut referenced = BTreeSet::new();
        references.collect(declaration.ty, &mut referenced);
        if let Some(body) = declaration.body {
            references.collect(body, &mut referenced);
        }
        worklist.extend(referenced);
    }
    assumptions
}

pub fn assumption_closure(
    arena: &TermArena,
    signature: &Signature,
    roots: &[u32],
) -> BTreeSet<u32> {
    let mut references = TermReferenceWalk::new(arena);
    closure_over(&mut references, signature, roots.iter().copied().collect())
}

pub fn judgment_assumption_closure(
    arena: &TermArena,
    signature: &Signature,
    terms: &[TermHandle],
) -> BTreeSet<u32> {
    let mut references = TermReferenceWalk::new(arena);
    let mut seeds = BTreeSet::new();
    for &term in terms {
        references.collect(term, &mut seeds);
    }
    closure_over(&mut references, signature, seeds)
}
