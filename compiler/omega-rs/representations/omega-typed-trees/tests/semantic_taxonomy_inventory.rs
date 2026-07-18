//! STR1 — the semantic-taxonomy INVENTORY PINS (staged migration rung 1;
//! record: wiki/architecture/semantic_taxonomy_representation.md).
//!
//! Each test DESTRUCTURES one of the current representation-loss shapes so
//! the migration (STR2+) cannot change a shape without consciously updating
//! its witness here. These are not behavior tests: they are compile-time
//! shape witnesses plus the record's must-survive invariants spelled where
//! the compiler can see them. When a pin breaks, the fix is NEVER to delete
//! it -- it is to re-pin the NEW shape and check the migration carried the
//! distinction the comment names.

use omega_typed_trees::data::DataProperties;
use omega_typed_trees::domain::DomainDefinition;
use omega_typed_trees::machine::Machine;

/// LOSS 1 (record §Domains): every domain is one undifferentiated
/// `DomainDefinition` -- no predicate-vs-semantic facet split, no
/// introduction policy / mint authority, no denotation schema, no
/// normalized `SemanticDomainId`. STR2 lands the facet pair
/// (`predicate: Option<PredicateFacet>` + `semantic: Option<SemanticFacet>`,
/// optional PAIR not an enum -- hybrids are first-class); this destructure
/// then gains fields and the update must carry the invariant: "no
/// checked-stage query infers predicate-vs-semantic behavior by testing
/// whether a domain happens to have facts or operators".
#[test]
fn domain_definition_is_still_the_undifferentiated_shape() {
    fn witness(definition: DomainDefinition) {
        let DomainDefinition {
            symbol: _,
            name: _,
            target_type: _,
            classifier: _,
            // STR4 checked plans (2026-07-19): the normalized SemanticDomainId
            // landed -- LOSS 1's "no normalized SemanticDomainId" clause is
            // PARTIALLY re-pinned (facet split + mint authority still absent).
            semantic_id: _,
            facts: _,
            operators: _,
            body_token_count: _,
        } = definition;
    }
    let _ = witness; // compile-time witness; never called
}

/// LOSS 2 -- PARTIALLY RE-PINNED (TPR2, 2026-07-16): the machine record now
/// carries the normalized `MachineTerminationPlan` -- decision 23's split of
/// the PUBLIC eventual-terminal guarantee (authored by bare `terminates;`,
/// contract identity) from the PRIVATE `RankingWitness` (subjects + explicit
/// view, never contract identity) -- populated ONCE at the syntax->resolved
/// lowering and COPIED downstream. The distinction the old pin named is
/// representable; the invariant test below witnesses the firewall. STILL
/// LOST here: no normalized `MachineSemanticContract`, the effect span is
/// STILL flat (decision 22's kinded rows ride STR4), and the
/// `terminates`/`decreases`/`decrease_order` compatibility shape remains the
/// checker's input until TPR3 migrates it onto the plan (TPR6 retires it).
#[test]
fn machine_record_carries_the_termination_plan_beside_the_compat_bools() {
    fn witness(machine: Machine) {
        let Machine {
            symbol: _,
            name: _,
            attached_data: _,
            boundary: _,     // the compatibility bool (STR7 retires it)
            // STR3 slice 2 (2026-07-16): the first-class supply mode landed,
            // populated once at the syntax->resolved lowering (Boundary |
            // CheckedBody today; Requirement/Accepted when their spellings
            // reach the record) and copied downstream.
            supply_mode: _,
            // TPR2 (2026-07-16): the normalized guarantee/witness split.
            termination_plan: _,
            // STR4 (2026-07-16): the normalized kinded effect-row identity.
            effect_row: _,
            type_parameters: _,
            contains: _,
            owned_data: _,
            satisfies: _,
            terminates: _,   // compatibility: the checker's input until TPR3
            decreases: _,    // compatibility witness material until TPR3
            decrease_order: _,
            decrease_view_arguments: _, // TPR3: argumented-view arguments
            decrease_range: _,          // TPR3: the rank-range constraint
            effects: _,      // STILL decision 22's kinded rows, as a flat name span
            contracts: _,
            states: _,
        } = machine;
    }
    let _ = witness;
}

/// The decision-23 firewall on the LANDED shape: an inherited/public
/// guarantee with an implementation-local witness is representable, and
/// swapping one valid witness for another leaves the published half (the
/// contract-identity carrier) unchanged.
#[test]
fn termination_plan_witness_swap_is_contract_invisible() {
    use omega_core::semantics::{
        MachineTerminationPlan, RankingViewId, RankingWitness, TerminationGuarantee,
    };
    let descending = MachineTerminationPlan {
        published: Some(TerminationGuarantee::EventualTerminal {
            premises: Vec::new(),
        }),
        checked_summary: TerminationGuarantee::NoGuarantee,
        implementation_witness: Some(RankingWitness {
            subjects: vec!["remaining".to_string()],
            ranking_view: RankingViewId::NAT_DESCENDING,
            view_path: "Nat::Descending".to_string(),
            view_arguments: Vec::new(),
            rank_range: None,
        }),
    };
    let swapped = MachineTerminationPlan {
        implementation_witness: Some(RankingWitness {
            subjects: vec!["index".to_string(), "limit".to_string()],
            ranking_view: RankingViewId::NAT_BOUNDED_DISTANCE,
            view_path: "Nat::BoundedDistance".to_string(),
            view_arguments: Vec::new(),
            rank_range: None,
        }),
        ..descending.clone()
    };
    assert_ne!(descending, swapped);
    assert_eq!(descending.published, swapped.published);
}

/// LOSS 3 -- RE-PINNED (STR3 first slice, 2026-07-16): `DataProperties`
/// now carries the first-class `Multiplicity` populated at the
/// syntax->resolved lowering (`[copy]` -> Unrestricted, ordinary data ->
/// Affine; `[linear]` maps to Linear) and COPIED (never re-derived)
/// through resolved->typed. The named distinction survived: one explicit
/// multiplicity per type, `zero_init`/`send` orthogonal. `copy` remains
/// the compatibility bool until STR7 retires it; the retirement updates
/// this pin again.
#[test]
fn data_properties_carries_first_class_multiplicity() {
    use omega_core::semantics::Multiplicity;
    let DataProperties {
        copy,
        zero_init: _,
        send: _,
        multiplicity,
    } = DataProperties::default();
    // The default (ZII) properties describe ordinary data: Affine, and the
    // compatibility bool agrees with the multiplicity's mapping.
    assert_eq!(multiplicity, Multiplicity::Affine);
    assert!(!copy);
}

/// LOSS 4 -- PARTIALLY RE-PINNED (STR4 slice 1, 2026-07-16): machines now
/// carry the NORMALIZED kinded row identity (`effect_row: EffectRowId`
/// into the tree's `EffectRowTable`; members kinded ServiceReach vs
/// OperationalMay via the canonical omega-core catalog; row identity
/// order/duplicate-blind and independent of the legacy bits). STILL LOST:
/// the flat `EffectSet` below remains an INDEPENDENTLY-BUILT compatibility
/// carrier (not yet a derived projection of the row -- STR6/7 flip that,
/// and this pin then asserts the derivation), and the published-ceiling vs
/// checked-inferred split has no carrier yet.
#[test]
fn effect_set_is_still_a_flat_bitset() {
    use omega_effects::EffectSet;
    let mut set = EffectSet::empty();
    // The flat surface: emptiness is bit-emptiness; union is bit-or over
    // name-assigned indices. A kinded row cannot be reconstructed from this
    // object -- that is the loss being pinned (and the ordering constraint:
    // "effect-row identity must not depend on the legacy numeric bit
    // assigned to a name").
    assert!(set.is_empty());
    let grew = set.insert_all(EffectSet::empty());
    assert!(!grew && set.is_empty());
}

/// LOSS 5 (record §Multiplicity, ownership summaries): control-flow
/// RE-PINNED (CML3, 2026-07-17): checked flow and every downstream semantic
/// spine now retain Establish / Transfer / Consume / AffineDrop permission
/// events (including conditional-payload debt). MOVE/DROP remain compatibility
/// fields, not the source taxonomy. CML3 slice 3 added multiplicity, explicit
/// owned/shared/exclusive access, and transfer-stable origin provenance.
/// Borrow activations/weakenings also enter this context, and the linear
/// judgment no longer reads move/drop. The remaining gap is retiring them as
/// transitional producer input.
#[test]
fn downstream_ownership_summary_carries_qualified_permission_events() {
    use omega_control_flow::{StateOwnershipSummary, StatePermissionEvent};
    use omega_core::semantics::{
        Multiplicity, PermissionAccess, PermissionProvenance,
    };
    let StateOwnershipSummary {
        moves: _,
        drops: _,
        permissions: _,
        ..
    } = StateOwnershipSummary::default();
    let event = StatePermissionEvent::default();
    assert_eq!(event.multiplicity, Multiplicity::Affine);
    assert_eq!(event.access, PermissionAccess::Owned);
    assert_eq!(event.provenance, PermissionProvenance::Unknown);
}
