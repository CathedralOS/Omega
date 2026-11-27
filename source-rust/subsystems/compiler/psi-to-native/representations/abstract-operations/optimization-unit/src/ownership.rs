/*
We represent path-sensitive ownership-frontier snapshots (`OwnershipFrontierFact`)
and their one-time verifier attachment (`attach_ownership_frontier_facts`). Each
site records live claims, owned places, and partially moved structural paths in
strictly increasing canonical order, and binds that snapshot into both its own
fact identity and the enclosing `OptimizationUnitIdentity`.
*/

pub use operation_evidence::ownership_frontier::{
    OwnershipFrontierFact, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
};
