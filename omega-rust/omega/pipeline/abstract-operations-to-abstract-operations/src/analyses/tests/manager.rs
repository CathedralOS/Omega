//! Revision cache, invalidation, and deterministic scheduling coverage.

use super::fixtures::*;
use crate::*;
use optimization_core::*;
use optimization_unit::*;

#[test]
fn bound_revision_reuses_products_without_rechecking_content() {
    let unit = unit(
        vec![function(
            100,
            1,
            vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
        )],
        b"borrowed-analysis-content",
    );
    let requested = AnalysisSet::new([AnalysisKind::Dominators]);
    let mut manager = AnalysisManager::new(&unit);
    let checks = || super::super::manager::CONTENT_IDENTITY_CHECKS.with(std::cell::Cell::get);
    let initial_checks = checks();
    // Unbound requests must check arbitrary caller-provided content every time.
    for _ in 0..8 {
        manager.require_all(&unit, requested).unwrap();
    }
    assert_eq!(checks() - initial_checks, 8);
    let initial_checks = checks();
    let mut revision = manager.bind_revision(&unit).unwrap();
    let first = revision.require_all(requested).unwrap();
    let original_product = first.get(AnalysisKind::Dominators).unwrap() as *const AnalysisProduct;
    assert!(first.get(AnalysisKind::ControlFlowGraph).is_none());
    for _ in 0..8 {
        let products = revision.require_all(requested).unwrap();
        assert_eq!(
            products.get(AnalysisKind::Dominators).unwrap() as *const AnalysisProduct,
            original_product
        );
        assert!(products.get(AnalysisKind::ControlFlowGraph).is_none());
    }
    assert_eq!(checks() - initial_checks, 1);
}

#[test]
fn rebinding_rejects_mutated_content_and_uncommitted_revisions() {
    let mut unit = unit(
        vec![function(100, 1, vec![(1, Terminator::Return)])],
        b"rebound-analysis-content",
    );
    let mut manager = AnalysisManager::new(&unit);
    manager
        .bind_revision(&unit)
        .unwrap()
        .require_all(AnalysisSet::new([AnalysisKind::ControlFlowGraph]))
        .unwrap();
    unit.functions[0].blocks[0].nodes[0].effect.output += 1;
    assert!(matches!(
        manager.bind_revision(&unit),
        Err(AnalysisManagerError::StaleUnitIdentity { .. })
    ));
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        manager.bind_revision(&unit),
        Err(AnalysisManagerError::RevisionMismatch { .. })
    ));
}

#[test]
fn cache_audits_undeclared_invalidation_atomically() {
    let original = unit(
        vec![function(100, 1, vec![(1, Terminator::Return)])],
        b"original",
    );
    let changed = unit(
        vec![function(
            100,
            1,
            vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
        )],
        b"changed",
    );
    let mut manager = AnalysisManager::new(&original);
    manager
        .require(&original, AnalysisKind::Dominators)
        .unwrap();
    let prior_revision = manager.revision();
    let prior_cache = manager.cached_kinds().collect::<Vec<_>>();
    assert_eq!(
        manager.commit_revision(&changed, AnalysisInvalidationSet::default(), true,),
        Err(AnalysisManagerError::UndeclaredInvalidation(
            AnalysisKind::ControlFlowGraph
        ))
    );
    assert_eq!(manager.revision(), prior_revision);
    assert_eq!(manager.cached_kinds().collect::<Vec<_>>(), prior_cache);

    let committed = manager
        .commit_revision(
            &changed,
            AnalysisInvalidationSet::new([AnalysisKind::ControlFlowGraph]),
            true,
        )
        .unwrap();
    assert_eq!(committed.current, changed.identity);
    assert!(committed.invalidated.contains(&AnalysisKind::Dominators));
    assert!(manager.cached_kinds().next().is_none());
}

#[test]
fn cached_cold_and_parallel_schedules_have_canonical_output() {
    let unit = unit(
        vec![function(
            100,
            1,
            vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
        )],
        b"parallel",
    );
    let requested = AnalysisSet::new([
        AnalysisKind::CallGraph,
        AnalysisKind::LoopForest,
        AnalysisKind::ControlFlowGraph,
        AnalysisKind::StronglyConnectedComponents,
        AnalysisKind::Dominators,
        AnalysisKind::OwnershipFrontiers,
        AnalysisKind::PostDominators,
    ]);
    let cold = AnalysisManager::compute_cold_parallel(&unit, requested).unwrap();
    assert_eq!(
        cold.iter().map(AnalysisProduct::kind).collect::<Vec<_>>(),
        requested.iter().collect::<Vec<_>>()
    );
    let mut manager = AnalysisManager::new(&unit);
    let cached = manager
        .require_all(&unit, requested)
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(cached, cold);
    let mut revision = manager.bind_revision(&unit).unwrap();
    let borrowed = revision.require_all(requested).unwrap();
    for product in &cold {
        assert_eq!(borrowed.get(product.kind()), Some(product));
    }
}

#[test]
fn ownership_frontiers_are_exact_and_never_retained_across_revisions() {
    let mut original = unit(
        vec![function(100, 1, vec![(1, Terminator::Return)])],
        b"ownership-original",
    );
    let fact = OwnershipFrontierFact::new(
        original.psi,
        original.functions[0].machine,
        OwnershipFrontierSite::BlockEntry(original.functions[0].entry),
        OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
        },
    );
    original.ownership_frontier_facts = vec![fact.clone()];
    original.identity = recompute_psi_optimization_unit_identity(&original);

    let AnalysisProduct::OwnershipFrontiers(frontiers) =
        compute_analysis(&original, AnalysisKind::OwnershipFrontiers).unwrap()
    else {
        unreachable!()
    };
    let projected = frontiers
        .fact(fact.machine, fact.site)
        .expect("exact source site is queryable");
    assert_eq!(projected.identity, fact.identity);
    assert_eq!(projected.snapshot, fact.snapshot);
    assert_eq!(projected.revision, original.identity);

    let mut changed = unit(
        vec![function(
            100,
            1,
            vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
        )],
        b"ownership-changed",
    );
    changed.ownership_frontier_facts = vec![fact];
    changed.identity = recompute_psi_optimization_unit_identity(&changed);

    let mut manager = AnalysisManager::new(&original);
    manager
        .require(&original, AnalysisKind::OwnershipFrontiers)
        .unwrap();
    let commit = manager
        .commit_revision(&changed, AnalysisInvalidationSet::default(), true)
        .unwrap();
    assert_eq!(
        commit.invalidated,
        vec![AnalysisKind::ValueRanges, AnalysisKind::OwnershipFrontiers]
    );
    assert!(commit.retained.is_empty());
    let AnalysisProduct::OwnershipFrontiers(rebound) = manager
        .require(&changed, AnalysisKind::OwnershipFrontiers)
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(rebound.facts[0].revision, changed.identity);
}

#[test]
fn analysis_manager_rejects_stale_content_on_require_cold_and_commit_paths() {
    let valid = unit(
        vec![function(100, 1, vec![(1, Terminator::Return)])],
        b"valid-analysis-content",
    );
    let mut stale = valid.clone();
    stale.functions[0].blocks[0].nodes[0].effect.output += 1;
    let recomputed = recompute_psi_optimization_unit_identity(&stale);
    let is_stale = |error: AnalysisManagerError| {
        matches!(
            error,
            AnalysisManagerError::StaleUnitIdentity {
                stored,
                recomputed: actual,
            } if stored == stale.identity && actual == recomputed
        )
    };

    let mut require_manager = AnalysisManager::new(&valid);
    assert!(is_stale(
        require_manager
            .require(&stale, AnalysisKind::ControlFlowGraph)
            .unwrap_err()
    ));
    assert!(is_stale(
        AnalysisManager::compute_cold_parallel(
            &stale,
            AnalysisSet::new([AnalysisKind::ControlFlowGraph]),
        )
        .unwrap_err()
    ));
    let mut commit_manager = AnalysisManager::new(&valid);
    assert!(is_stale(
        commit_manager
            .commit_revision(&stale, AnalysisInvalidationSet::default(), false)
            .unwrap_err()
    ));
}
