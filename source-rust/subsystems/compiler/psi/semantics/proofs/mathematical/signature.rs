use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use super::conversion::Budget;
use super::inductive::{ConstructorInfo, RecursorInfo, check_inductive_block};
use super::term::{Term, TermArena, TermHandle};
use super::typing::{Context, CoreError, check_type, infer_sort};

#[derive(Clone, Debug, PartialEq, Eq, Default, record_storage::Record)]
pub enum DeclarationKind {
    #[default]
    Ordinary,
    Inductive {
        num_mutual: u32,
        mutual_index: u32,
        num_params: u32,
        num_constructors: u32,
    },
    Constructor {
        inductive: u32,
        constructor_index: u32,
    },
    Recursor {
        inductive: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, record_storage::Record)]
pub struct Declaration {
    pub level_arity: u32,
    pub ty: TermHandle,
    pub body: Option<TermHandle>,
    pub kind: DeclarationKind,
}

impl Declaration {
    pub fn definition(level_arity: u32, ty: TermHandle, body: TermHandle) -> Self {
        Self {
            level_arity,
            ty,
            body: Some(body),
            kind: DeclarationKind::Ordinary,
        }
    }

    pub fn assumption(level_arity: u32, ty: TermHandle) -> Self {
        Self {
            level_arity,
            ty,
            body: None,
            kind: DeclarationKind::Ordinary,
        }
    }

    pub fn inductive(
        level_arity: u32,
        num_params: u32,
        num_constructors: u32,
        ty: TermHandle,
    ) -> Self {
        Self::mutual_inductive(level_arity, 1, 0, num_params, num_constructors, ty)
    }

    pub fn mutual_inductive(
        level_arity: u32,
        num_mutual: u32,
        mutual_index: u32,
        num_params: u32,
        num_constructors: u32,
        ty: TermHandle,
    ) -> Self {
        Self {
            level_arity,
            ty,
            body: None,
            kind: DeclarationKind::Inductive {
                num_mutual,
                mutual_index,
                num_params,
                num_constructors,
            },
        }
    }

    pub fn constructor(
        level_arity: u32,
        inductive: u32,
        constructor_index: u32,
        ty: TermHandle,
    ) -> Self {
        Self {
            level_arity,
            ty,
            body: None,
            kind: DeclarationKind::Constructor {
                inductive,
                constructor_index,
            },
        }
    }

    pub fn recursor(level_arity: u32, inductive: u32, ty: TermHandle) -> Self {
        Self {
            level_arity,
            ty,
            body: None,
            kind: DeclarationKind::Recursor { inductive },
        }
    }

    pub fn is_assumption(&self) -> bool {
        self.body.is_none() && matches!(self.kind, DeclarationKind::Ordinary)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Signature {
    declarations: Rc<Vec<Declaration>>,
    constructors: Rc<HashMap<u32, ConstructorInfo>>,
    recursors: Rc<HashMap<u32, RecursorInfo>>,
}

impl Signature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_declarations(declarations: Vec<Declaration>) -> Self {
        Self {
            declarations: Rc::new(declarations),
            constructors: Rc::new(HashMap::new()),
            recursors: Rc::new(HashMap::new()),
        }
    }

    /// Returns a checked prefix of length `len`, preserving all admitted
    /// constructor and recursor tables for declarations below `len`.
    pub fn prefix(&self, len: usize) -> Self {
        let len = len.min(self.declarations.len());
        let limit = len as u32;
        let constructors = self
            .constructors
            .iter()
            .filter(|&(&idx, _)| idx < limit)
            .map(|(&k, v)| (k, v.clone()))
            .collect();
        let recursors = self
            .recursors
            .iter()
            .filter(|&(&idx, _)| idx < limit)
            .map(|(&k, v)| (k, v.clone()))
            .collect();
        Self {
            declarations: Rc::new(self.declarations[..len].to_vec()),
            constructors: Rc::new(constructors),
            recursors: Rc::new(recursors),
        }
    }

    pub fn get(&self, index: u32) -> Option<&Declaration> {
        self.declarations.get(usize::try_from(index).ok()?)
    }

    pub(crate) fn get_constructor(&self, index: u32) -> Option<&ConstructorInfo> {
        self.constructors.get(&index)
    }

    pub(crate) fn get_recursor(&self, index: u32) -> Option<&RecursorInfo> {
        self.recursors.get(&index)
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

    pub(crate) fn push(&mut self, declaration: Declaration) {
        Rc::make_mut(&mut self.declarations).push(declaration);
    }

    pub(crate) fn register_constructor(&mut self, index: u32, info: ConstructorInfo) {
        Rc::make_mut(&mut self.constructors).insert(index, info);
    }

    pub(crate) fn register_recursor(&mut self, index: u32, info: RecursorInfo) {
        Rc::make_mut(&mut self.recursors).insert(index, info);
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
    check_signature_with_attribution(arena, declarations, budget).map_err(|failure| failure.error)
}

pub fn check_signature_with_attribution(
    arena: &mut TermArena,
    declarations: &[Declaration],
    budget: &mut Budget,
) -> Result<Signature, SignatureCheckError> {
    let mut signature = Signature::new();
    let mut position = 0;
    while position < declarations.len() {
        let declaration = &declarations[position];
        match declaration.kind {
            DeclarationKind::Ordinary => {
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
                position += 1;
            }
            DeclarationKind::Inductive {
                num_mutual,
                mutual_index,
                num_params,
                num_constructors,
            } => {
                if num_mutual == 0 || mutual_index != 0 {
                    return Err(SignatureCheckError {
                        declaration: position,
                        error: CoreError::InvalidInductiveDeclaration,
                    });
                }
                let consumed = check_inductive_block(
                    arena,
                    &mut signature,
                    declarations,
                    position,
                    num_mutual,
                    num_params,
                    num_constructors,
                    budget,
                )?;
                position += consumed;
            }
            DeclarationKind::Constructor { .. } | DeclarationKind::Recursor { .. } => {
                return Err(SignatureCheckError {
                    declaration: position,
                    error: CoreError::InvalidInductiveDeclaration,
                });
            }
        }
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
