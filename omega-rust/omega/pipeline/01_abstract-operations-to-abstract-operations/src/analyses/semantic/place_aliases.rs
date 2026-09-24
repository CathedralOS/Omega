//! Declared-place overlap evidence for one unit revision.
//!
//! Each declared root is a distinct verifier-owned storage site, so views of
//! different roots are disjoint as long as neither path crosses a `Referent`
//! boundary — a reference's pointee is storage owned through some other root,
//! and two roots can hold borrows into shared storage. Custody bookkeeping
//! (owned places, partial custody, claim multiplicity) moves ownership, never
//! bytes, so it carries no aliasing evidence and is not retained here.

use std::collections::{BTreeMap, BTreeSet};

use optimization_unit::{OwnershipFrontierSite, PsiOptimizationUnit};
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};
use terminal_psi::StructuralPathSegment;

/// One storage view: a declared root plus an exact projection path in the
/// frontier-claim vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaceView {
    pub root: PlaceId,
    pub path: Vec<StructuralPathSegment>,
}

/// Whether two views provably share storage, provably cannot, or sit outside
/// what the declared roster establishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(test, feature = "test-support"))]
pub enum PlaceAliasRelation {
    Overlapping,
    Disjoint,
    Unknown,
}

/// One custody view the verifier admits live on this machine, with every
/// frontier site holding it as evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAliasClaim {
    pub view: PlaceView,
    pub sites: Vec<OwnershipFrontierSite>,
}

/// One declared storage root. `kind` is `None` when the place is named in the
/// function's referenced-place set but missing from the declared roster —
/// a state only an unverified seed can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAliasRoot {
    pub place: PlaceId,
    pub kind: Option<StructuralPlaceKind>,
}

/// One machine's complete declared-place roster plus the custody views the
/// verifier projected onto it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAliasFunction {
    pub machine: MachineId,
    pub roots: Vec<PlaceAliasRoot>,
    pub claims: Vec<PlaceAliasClaim>,
    /// Live claims whose custody is not rooted at a declared place.
    pub unrooted_claims: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceAliasesAnalysis {
    pub functions: Vec<PlaceAliasFunction>,
}

impl PlaceAliasesAnalysis {
    #[cfg(any(test, feature = "test-support"))]
    pub fn function(&self, machine: MachineId) -> Option<&PlaceAliasFunction> {
        self.functions
            .binary_search_by_key(&machine, |function| function.machine)
            .ok()
            .map(|index| &self.functions[index])
    }
}

impl PlaceAliasFunction {
    /// Every declared root names a distinct storage site with a known,
    /// placeable kind — the premise memory rewrites rely on to reason about
    /// distinct `PlaceId`s without re-deriving the roster.
    #[cfg(any(test, feature = "test-support"))]
    pub fn declared_roots_disjoint(&self) -> bool {
        self.roots.iter().all(|root| {
            matches!(root.kind, Some(kind) if !matches!(kind, StructuralPlaceKind::ProviderAttachment { .. }))
        })
    }

    /// Provable storage relation between two views. Views reach outside their
    /// root's own extent only through `Referent`, so that crossing is the only
    /// route by which declared storage can meet another root's content.
    #[cfg(any(test, feature = "test-support"))]
    pub fn relation(&self, a: &PlaceView, b: &PlaceView) -> PlaceAliasRelation {
        let Some(a_kind) = self.root_kind(a.root) else {
            return PlaceAliasRelation::Unknown;
        };
        let Some(b_kind) = self.root_kind(b.root) else {
            return PlaceAliasRelation::Unknown;
        };
        // Provider attachments are boundary evidence without a target layout;
        // nothing in the roster bounds where their content lives.
        if matches!(a_kind, StructuralPlaceKind::ProviderAttachment { .. })
            || matches!(b_kind, StructuralPlaceKind::ProviderAttachment { .. })
        {
            return PlaceAliasRelation::Unknown;
        }
        if a.root == b.root {
            return view_path_relation(&a.path, &b.path);
        }
        if a.path.iter().chain(b.path.iter()).any(is_referent) {
            return PlaceAliasRelation::Unknown;
        }
        PlaceAliasRelation::Disjoint
    }

    #[cfg(any(test, feature = "test-support"))]
    fn root_kind(&self, place: PlaceId) -> Option<StructuralPlaceKind> {
        self.roots
            .binary_search_by_key(&place, |root| root.place)
            .ok()
            .and_then(|index| self.roots[index].kind)
    }
}

/// Same-root path relation. Any divergence — different field, case index, or
/// one path crossing `Referent` where the other stays in carrier storage —
/// names disjoint extents. A prefix extends into the longer view's storage
/// only while the continuation stays inside it, so a continuation beginning
/// at `Referent` is disjoint rather than contained.
#[cfg(any(test, feature = "test-support"))]
fn view_path_relation(
    a: &[StructuralPathSegment],
    b: &[StructuralPathSegment],
) -> PlaceAliasRelation {
    let shared = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    if shared < a.len() && shared < b.len() {
        return PlaceAliasRelation::Disjoint;
    }
    let continuation = if a.len() < b.len() {
        &b[shared..]
    } else {
        &a[shared..]
    };
    match continuation.first() {
        Some(StructuralPathSegment::Referent) => PlaceAliasRelation::Disjoint,
        _ => PlaceAliasRelation::Overlapping,
    }
}

#[cfg(any(test, feature = "test-support"))]
fn is_referent(segment: &StructuralPathSegment) -> bool {
    matches!(segment, StructuralPathSegment::Referent)
}

pub(in crate::analyses) fn place_aliases(unit: &PsiOptimizationUnit) -> PlaceAliasesAnalysis {
    let mut claim_views: BTreeMap<MachineId, BTreeMap<PlaceView, BTreeSet<OwnershipFrontierSite>>> =
        BTreeMap::new();
    let mut unrooted_claims: BTreeMap<MachineId, u64> = BTreeMap::new();
    for fact in &unit.ownership_frontier_facts {
        for claim in &fact.snapshot.claims {
            match claim.input {
                Some(root) => {
                    claim_views
                        .entry(fact.machine)
                        .or_default()
                        .entry(PlaceView {
                            root,
                            path: claim.path.clone(),
                        })
                        .or_default()
                        .insert(fact.site);
                }
                None => *unrooted_claims.entry(fact.machine).or_default() += 1,
            }
        }
    }
    let mut functions = Vec::with_capacity(unit.functions.len());
    for function in &unit.functions {
        let mut roots: BTreeMap<PlaceId, Option<StructuralPlaceKind>> = function
            .declared_places
            .iter()
            .map(|place| (*place, None))
            .collect();
        for declaration in &function.structural_places {
            roots.insert(declaration.id, Some(declaration.kind));
        }
        let claims = claim_views
            .remove(&function.machine)
            .map(|views| {
                views
                    .into_iter()
                    .map(|(view, sites)| PlaceAliasClaim {
                        view,
                        sites: sites.into_iter().collect(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        functions.push(PlaceAliasFunction {
            machine: function.machine,
            roots: roots
                .into_iter()
                .map(|(place, kind)| PlaceAliasRoot { place, kind })
                .collect(),
            claims,
            unrooted_claims: unrooted_claims.remove(&function.machine).unwrap_or(0),
        });
    }
    functions.sort_by_key(|function| function.machine);
    PlaceAliasesAnalysis { functions }
}
