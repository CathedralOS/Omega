//! The declaration model of the common mathematical core: the ambient
//! signature `Σ` a judgment `Σ; Δ; Γ ⊢ t : T` is checked under, and the
//! exact assumption closure the foundation requires of every checked
//! root.
//!
//! A [`Declaration`] is the kernel's unit of named mathematics: a closed
//! statement `ty` under `level_arity` universe parameters, and either a
//! closed `body` (a definition, `d : ty := body`) or none (an
//! assumption — the foundation's "named assumption with an exact
//! statement", the only axioms the calculus admits). Terms reference a
//! declaration through `Term::Constant { declaration, levels }` by
//! position, exactly as terms reference binders by de Bruijn index;
//! attaching source names is an elaboration concern, not a kernel one.
//!
//! A [`Signature`] is the ordered, append-only declaration list. It is
//! checked sequentially by [`check_signature`]: declaration `i` is
//! checked under the signature of declarations `0..i`, so a declaration
//! can only reference strictly earlier ones — no self-reference, no
//! forward reference, and therefore no recursive definitions. That
//! ordering is also what makes δ-unfolding terminate: each constant
//! strictly lowers the greatest reachable declaration index.
//!
//! The signature is producer evidence like the rest of a certificate:
//! the kernel re-decides every declaration — each statement must be a
//! type under its own level arity, and each body must inhabit its own
//! statement — rather than trusting that the producer checked them.
//!
//! [`assumption_closure`] and [`judgment_assumption_closure`] compute
//! the foundation's required evidence record: the exact set of
//! assumption declarations a checked declaration graph or judgment
//! transitively depends on. The traversal is syntactic over stored
//! statements *and* bodies — it never relies on conversion having
//! unfolded a definition, so a theorem whose statement references an
//! axiom-dependent definition retains that assumption even when the
//! final proof term never unfolds it. Unused declarations and discarded
//! proof-search attempts are not roots and never enter a closure.

use std::collections::BTreeSet;
use std::rc::Rc;

use super::conversion::Budget;
use super::term::{Term, TermArena, TermHandle};
use super::typing::{Context, CoreError, check_type, infer_sort};

/// One named top-level declaration: `d : ty` with `level_arity`
/// universe parameters, plus `body` for a definition or `None` for an
/// assumption.
///
/// Both `ty` and `body` are closed terms — a top-level declaration
/// carries no local context, so a free `Variable` inside either is a
/// malformed declaration that `check_signature` rejects with
/// `UnboundVariable`. `Level::Parameter(i)` inside either names the
/// declaration's own `i`-th universe parameter, scope-checked against
/// `level_arity`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    /// The number of universe parameters this declaration is
    /// polymorphic over. A `Term::Constant` reference must supply
    /// exactly this many level arguments.
    pub level_arity: u32,
    /// The declared statement — the type a `Constant` reference to this
    /// declaration instantiates. Must itself be a type under the
    /// declaration's level arity and the signature prefix.
    pub ty: TermHandle,
    /// `Some(body)` makes this a definition `d : ty := body`, checked
    /// to inhabit `ty`; `None` makes it an assumption — a named axiom
    /// whose exact statement is `ty`.
    pub body: Option<TermHandle>,
}

impl Declaration {
    /// A definition `d : ty := body` polymorphic over `level_arity`
    /// universe parameters.
    pub fn definition(level_arity: u32, ty: TermHandle, body: TermHandle) -> Self {
        Self {
            level_arity,
            ty,
            body: Some(body),
        }
    }

    /// An assumption `d : ty` polymorphic over `level_arity` universe
    /// parameters — a named axiom whose exact statement is `ty`.
    pub fn assumption(level_arity: u32, ty: TermHandle) -> Self {
        Self {
            level_arity,
            ty,
            body: None,
        }
    }

    /// Whether this declaration is an assumption: no body was supplied,
    /// so its constants stay neutral and its statement is an axiom.
    pub fn is_assumption(&self) -> bool {
        self.body.is_none()
    }
}

/// The ordered declaration list a judgment is checked under. Cloning
/// shares the storage — a `Context` carries one through every binder
/// extension, so the signature must stay cheap to clone.
#[derive(Clone, Debug, Default)]
pub struct Signature {
    declarations: Rc<Vec<Declaration>>,
}

impl Signature {
    /// The empty signature: no declaration is in scope.
    pub fn new() -> Self {
        Self::default()
    }

    /// A signature over an owned declaration list. The list is producer
    /// evidence until [`check_signature`] re-decides it.
    pub fn from_declarations(declarations: Vec<Declaration>) -> Self {
        Self {
            declarations: Rc::new(declarations),
        }
    }

    /// The declaration at `index`, or `None` when the signature has no
    /// such position — the `UnknownDeclaration` case.
    pub fn get(&self, index: u32) -> Option<&Declaration> {
        self.declarations.get(usize::try_from(index).ok()?)
    }

    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// The stored declarations in signature order.
    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    /// Append `declaration`, reusing the shared storage when this is the
    /// only reference. `check_signature` is the only writer: it pushes
    /// each declaration after the prefix context that referenced this
    /// signature has been dropped, so the append is in place.
    fn push(&mut self, declaration: Declaration) {
        Rc::make_mut(&mut self.declarations).push(declaration);
    }
}

/// Check each declaration of `declarations` under the signature formed
/// by the ones before it, and return the checked signature.
///
/// Declaration `i` checks in a context with no local bindings, its own
/// `level_arity` universe scope, and the already-checked signature
/// `0..i`: its statement must be a type, and a definition's body must
/// inhabit the statement. A `Constant` naming `i` itself or any later
/// position fails `UnknownDeclaration` under the prefix — the ordering
/// is what rules out recursive definitions.
pub fn check_signature(
    arena: &mut TermArena,
    declarations: &[Declaration],
    budget: &mut Budget,
) -> Result<Signature, CoreError> {
    let mut signature = Signature::new();
    for declaration in declarations {
        {
            let context = Context::with_level_arity(declaration.level_arity)
                .with_signature(signature.clone());
            infer_sort(arena, &context, declaration.ty, budget)?;
            if let Some(body) = declaration.body {
                check_type(arena, &context, body, declaration.ty, budget)?;
            }
        }
        signature.push(declaration.clone());
    }
    Ok(signature)
}

/// Every `Constant` declaration index `term` names. A `Constant` node's
/// level arguments are levels, which name no declarations — so the
/// traversal only descends into term children.
fn collect_references(arena: &TermArena, term: TermHandle, out: &mut BTreeSet<u32>) {
    match arena.get(term) {
        Term::Dummy
        | Term::Variable(_)
        | Term::Sort(_)
        | Term::Two
        | Term::TwoZero
        | Term::TwoOne
        | Term::Empty => {}
        Term::Constant { declaration, .. } => {
            out.insert(declaration);
        }
        Term::Pi { domain, codomain }
        | Term::Lambda {
            domain,
            body: codomain,
        }
        | Term::Apply {
            function: domain,
            argument: codomain,
        }
        | Term::Sigma { domain, codomain } => {
            collect_references(arena, domain, out);
            collect_references(arena, codomain, out);
        }
        Term::Pair { first, second } => {
            collect_references(arena, first, out);
            collect_references(arena, second, out);
        }
        Term::Fst { pair } | Term::Snd { pair } => {
            collect_references(arena, pair, out);
        }
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => {
            collect_references(arena, motive, out);
            collect_references(arena, zero_branch, out);
            collect_references(arena, one_branch, out);
            collect_references(arena, scrutinee, out);
        }
        Term::Id { ty, left, right } => {
            collect_references(arena, ty, out);
            collect_references(arena, left, out);
            collect_references(arena, right, out);
        }
        Term::Refl { ty, value } => {
            collect_references(arena, ty, out);
            collect_references(arena, value, out);
        }
        Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        } => {
            collect_references(arena, motive, out);
            collect_references(arena, base, out);
            collect_references(arena, endpoint, out);
            collect_references(arena, proof, out);
        }
        Term::W { carrier, children } => {
            collect_references(arena, carrier, out);
            collect_references(arena, children, out);
        }
        Term::Sup {
            carrier,
            children,
            label,
            function,
        } => {
            collect_references(arena, carrier, out);
            collect_references(arena, children, out);
            collect_references(arena, label, out);
            collect_references(arena, function, out);
        }
        Term::IndW { motive, step, tree } => {
            collect_references(arena, motive, out);
            collect_references(arena, step, out);
            collect_references(arena, tree, out);
        }
        Term::EmptyElim { ty, scrutinee } => {
            collect_references(arena, ty, out);
            collect_references(arena, scrutinee, out);
        }
        Term::Squash { ty } | Term::Box { ty } => {
            collect_references(arena, ty, out);
        }
        Term::SquashIntro { ty, value } | Term::BoxIntro { ty, value } => {
            collect_references(arena, ty, out);
            collect_references(arena, value, out);
        }
        Term::SquashElim {
            proposition,
            function,
            scrutinee,
        }
        | Term::BoxElim {
            motive: proposition,
            body: function,
            scrutinee,
        } => {
            collect_references(arena, proposition, out);
            collect_references(arena, function, out);
            collect_references(arena, scrutinee, out);
        }
    }
}

/// The shared traversal: from the seed declaration indices, visit every
/// declaration a constant can reach. Each visited declaration that is an
/// assumption lands in the result; each visited declaration — definition
/// or assumption — contributes the constants in its statement and its
/// body (when present) to the worklist. Out-of-range indices are ignored
/// here: reachability over malformed references is typing's rejection,
/// already decided before a receiver reads a closure.
fn closure_over(arena: &TermArena, signature: &Signature, seeds: BTreeSet<u32>) -> BTreeSet<u32> {
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
        collect_references(arena, declaration.ty, &mut referenced);
        if let Some(body) = declaration.body {
            collect_references(arena, body, &mut referenced);
        }
        worklist.extend(referenced);
    }
    assumptions
}

/// The exact assumption closure of the root declarations `roots`: each
/// root's statement and body contribute their constants, referenced
/// declarations contribute theirs transitively, and a root that is
/// itself an assumption belongs to its own closure. This is the
/// foundation's required record — computed over the stored declaration
/// graph, so it covers statements and bodies that conversion never
/// unfolded and is unchanged by irrelevance, erasure or normalization.
pub fn assumption_closure(
    arena: &TermArena,
    signature: &Signature,
    roots: &[u32],
) -> BTreeSet<u32> {
    closure_over(arena, signature, roots.iter().copied().collect())
}

/// The exact assumption closure of a checked judgment whose evidence is
/// `terms` — for a certificate, its context bindings, evidence term and
/// claimed type. Every `Constant` those terms name seeds the same
/// transitive traversal [`assumption_closure`] runs over declaration
/// roots, so a judgment depending on an axiom only through another
/// declaration's *statement* still records the axiom.
pub fn judgment_assumption_closure(
    arena: &TermArena,
    signature: &Signature,
    terms: &[TermHandle],
) -> BTreeSet<u32> {
    let mut seeds = BTreeSet::new();
    for &term in terms {
        collect_references(arena, term, &mut seeds);
    }
    closure_over(arena, signature, seeds)
}
