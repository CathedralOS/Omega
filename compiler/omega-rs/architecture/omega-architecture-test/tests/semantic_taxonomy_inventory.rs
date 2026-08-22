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

use psi_typed_trees::data::DataProperties;
use psi_typed_trees::domain::DomainDefinition;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::types::DomainConstraint;

/// LOSS 1 -- PARTIALLY RE-PINNED (DOM theory records, 2026-07-28): predicate
/// body, role-keyed semantic contributions, and normalized establishment
/// relationships are independent records. Canonical qualification, package
/// owner coherence, and denotation schema remain absent. No checked-stage
/// query may infer these records from facts, operators, or attachment names.
#[test]
fn domain_definition_carries_independent_domain_theory_records() {
    fn witness(definition: DomainDefinition) {
        let DomainDefinition {
            symbol: _,
            name: _,
            type_parameters: _,
            target_type: _,
            index_arguments: _,
            is_public: _,
            // Transparent alias theory is retained independently from facts,
            // so aliases cannot masquerade as bodyless establishment.
            alias: _,
            predicate_body: _,
            // The normalized identity remains independent from contribution
            // roles: qualification and trust can name a declaration even when
            // it contributes no operator-selection role.
            semantic_id: _,
            semantic_roles: _,
            establishment_routes: _,
            facts: _,
            operators: _,
            semantic_clause_token_count: _,
        } = definition;
    }
    let _ = witness; // compile-time witness; never called
}

#[test]
fn domain_constraint_carries_carrier_resolved_identity_and_roles() {
    fn witness(constraint: DomainConstraint) {
        let DomainConstraint {
            name: _,
            arguments: _,
            symbol: _,
            semantic_id: _,
            predicate_body: _,
            semantic_roles: _,
            establishment_routes: _,
        } = constraint;
    }
    let _ = witness;
}

/// LOSS 2 -- CLOSED: the machine record carries the normalized
/// `MachineTerminationPlan` -- decision 23's split of
/// the PUBLIC termination guarantee (authored by bare `terminates;`,
/// contract identity) from the PRIVATE `RankingWitness` (subjects + explicit
/// view, never contract identity) -- populated ONCE at the syntax->resolved
/// lowering and COPIED downstream. The distinction the old pin named is
/// representable; the invariant test below witnesses the firewall. Authored
/// ranking spans stop before typed trees; downstream consumers resolve only
/// the private normalized witness.
#[test]
fn machine_record_carries_one_public_termination_interface() {
    fn witness(machine: Machine) {
        let Machine {
            symbol: _,
            name: _,
            attached_data: _,
            // STR7: the first-class supply mode is populated once at the
            // syntax-to-resolved boundary and copied downstream. There is no
            // parallel source-spelling boolean on semantic machine records.
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
            conformance_bounds: _,
            invokes: _,  // authored synchronous-invocation ceiling
            suspends: _, // EFX: independent authored suspension ceiling
            blocks: _,   // EFX: independent authored blocking ceiling
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
    use psi_language_semantics::{
        MachineTerminationPlan, RankingViewId, RankingWitness, TerminationGuarantee,
        TerminationInterface,
    };
    let descending = MachineTerminationPlan {
        interface: TerminationInterface::Published(TerminationGuarantee::Terminates {
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

/// LOSS 3 -- RE-PINNED (STR/EFX normalization, 2026-08-10):
/// `DataProperties` carries the first-class `Multiplicity` populated at the
/// syntax->resolved lowering (`[copy]` -> Unrestricted, ordinary data ->
/// Affine; `[linear]` maps to Linear) and copied through resolved->typed. No
/// parallel `copy` boolean survives normalization: one explicit multiplicity
/// per type travels beside the orthogonal normalized carry record.
#[test]
fn data_properties_carries_first_class_multiplicity() {
    use psi_language_semantics::Multiplicity;
    let DataProperties {
        carry: _,
        multiplicity,
    } = DataProperties::default();
    // The default (ZII) properties describe ordinary data: Affine.
    assert_eq!(multiplicity, Multiplicity::Affine);
}

/// LOSS 4 -- RE-PINNED (EFX, 2026-07-24): machines carry only the
/// symbol-resolved `ServiceReachRowId`, while suspension and blocking have
/// independent plans. The former global name/u64 effect engine is absent;
/// service rows normalize resolved identities as deterministic sets.
#[test]
fn service_reach_rows_are_identity_sets_without_global_name_bits() {
    use psi_language_semantics::{ServiceReachId, ServiceReachRowTable};
    let mut rows = ServiceReachRowTable::default();
    let console = ServiceReachId(2);
    let filesystem = ServiceReachId(3);
    let row = rows.intern(vec![filesystem, console, filesystem]);
    assert_eq!(rows.services(row), &[console, filesystem]);
}

/// LOSS 5 (record §Multiplicity, ownership summaries): control-flow
/// RE-PINNED (CML3, 2026-07-17): checked flow and every downstream semantic
/// spine now retain Establish / Transfer / Consume / AffineDrop permission
/// events (including conditional-payload debt). No parallel move/drop fields
/// remain. CML3 slice 3 added multiplicity, explicit
/// owned/shared/exclusive access, transfer-stable root-lineage provenance, and
/// an independent transfer-stable permission claim identity.
/// Borrow activations/weakenings also enter this context, and the linear
/// judgment and downstream IRs carry no parallel move/drop summary.
#[test]
fn downstream_ownership_summary_carries_qualified_permission_events() {
    use omega_control_flow::{StateOwnershipSummary, StatePermissionEvent};
    use psi_language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionProvenance,
    };
    let StateOwnershipSummary { permissions: _ } = StateOwnershipSummary::default();
    let event = StatePermissionEvent::default();
    assert_eq!(event.multiplicity, Multiplicity::Affine);
    assert_eq!(event.access, PermissionAccess::Owned);
    assert_eq!(event.claim_identity, PermissionClaimIdentity::Unknown);
    assert_eq!(event.provenance, PermissionProvenance::Unknown);
}
