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
use omega_typed_trees::types::DomainConstraint;

/// LOSS 1 -- PARTIALLY RE-PINNED (DOM1/STR2, 2026-07-23): every domain now
/// carries the normalized predicate/semantic facet pair. Introduction policy,
/// mint authority, denotation schema, and full facet bodies remain absent.
/// The pair is not an enum: hybrids are first-class. No checked-stage query
/// may infer a facet role by testing whether facts or operators happen to be
/// present.
#[test]
fn domain_definition_carries_normalized_facet_roles() {
    fn witness(definition: DomainDefinition) {
        let DomainDefinition {
            symbol: _,
            name: _,
            target_type: _,
            // STR4 checked plans (2026-07-19): the normalized SemanticDomainId
            // landed -- LOSS 1's "no normalized SemanticDomainId" clause is
            // re-pinned. It remains a compatibility identity beside the
            // normalized facet pair until downstream migration completes.
            semantic_id: _,
            facets: _,
            facts: _,
            operators: _,
            body_token_count: _,
        } = definition;
    }
    let _ = witness; // compile-time witness; never called
}

#[test]
fn domain_constraint_carries_carrier_resolved_identity_and_facets() {
    fn witness(constraint: DomainConstraint) {
        let DomainConstraint {
            name: _,
            symbol: _,
            semantic_id: _,
            facets: _,
        } = constraint;
    }
    let _ = witness;
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
            boundary: _, // the compatibility bool (STR7 retires it)
            // STR3 slice 2 (2026-07-16): the first-class supply mode landed,
            // populated once at the syntax->resolved lowering (Boundary |
            // CheckedBody today; Requirement/Accepted when their spellings
            // reach the record) and copied downstream.
            supply_mode: _,
            // TPR2 (2026-07-16): the normalized guarantee/witness split.
            termination_plan: _,
            // EFX: the normalized, symbol-resolved service row is the only
            // durable reach identity on the machine record.
            service_reach_row: _,
            lifetime_parameters: _,
            type_parameters: _,
            owned_data: _,
            satisfies: _,
            terminates: _, // compatibility: the checker's input until TPR3
            decreases: _,  // compatibility witness material until TPR3
            decrease_order: _,
            decrease_view_arguments: _, // TPR3: argumented-view arguments
            decrease_range: _,          // TPR3: the rank-range constraint
            effects: _,                 // STILL decision 22's kinded rows, as a flat name span
            suspends: _,                // EFX: independent authored suspension ceiling
            blocks: _,                  // EFX: independent authored blocking ceiling
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
        TerminationInterface,
    };
    let descending = MachineTerminationPlan {
        interface: TerminationInterface::Published(TerminationGuarantee::EventualTerminal {
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
    assert_eq!(descending.interface, swapped.interface);
}

/// LOSS 3 -- RE-PINNED (STR3 first slice, 2026-07-16): `DataProperties`
/// now carries the first-class `Multiplicity` populated at the
/// syntax->resolved lowering (`[copy]` -> Unrestricted, ordinary data ->
/// Affine; `[linear]` maps to Linear) and COPIED (never re-derived)
/// through resolved->typed. The named distinction survived: one explicit
/// multiplicity per type, `zero_init` and the normalized carry record
/// orthogonal. `copy` remains
/// the compatibility bool until STR7 retires it; the retirement updates
/// this pin again.
#[test]
fn data_properties_carries_first_class_multiplicity() {
    use omega_core::semantics::Multiplicity;
    let DataProperties {
        copy,
        zero_init: _,
        carry: _,
        multiplicity,
    } = DataProperties::default();
    // The default (ZII) properties describe ordinary data: Affine, and the
    // compatibility bool agrees with the multiplicity's mapping.
    assert_eq!(multiplicity, Multiplicity::Affine);
    assert!(!copy);
}

/// LOSS 4 -- RE-PINNED (EFX, 2026-07-24): machines carry only the
/// symbol-resolved `ServiceReachRowId`, while suspension and blocking have
/// independent plans. The former global name/u64 effect engine is absent;
/// service rows normalize resolved identities as deterministic sets.
#[test]
fn service_reach_rows_are_identity_sets_without_global_name_bits() {
    use omega_core::semantics::{ServiceReachId, ServiceReachRowTable};
    let mut rows = ServiceReachRowTable::default();
    let console = ServiceReachId(2);
    let filesystem = ServiceReachId(3);
    let row = rows.intern(vec![filesystem, console, filesystem]);
    assert_eq!(rows.services(row), &[console, filesystem]);
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
    use omega_core::semantics::{Multiplicity, PermissionAccess, PermissionProvenance};
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
