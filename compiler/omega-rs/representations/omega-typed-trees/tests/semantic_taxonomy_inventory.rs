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
            facts: _,
            operators: _,
            body_token_count: _,
        } = definition;
    }
    let _ = witness; // compile-time witness; never called
}

/// LOSS 2 (record §Machines): the machine record carries `boundary: bool` +
/// `terminates: bool` + a FLAT effect-name span -- no normalized
/// `MachineSemanticContract`, no `MachineSupplyMode`, and the terminates
/// boolean + decreases span CONFLATE the public eventual-terminal guarantee
/// with the private ranking witness (decision 23 splits them:
/// `TerminationGuarantee` participates in contract identity, the
/// `RankingWitness` never does). When STR3/4 land the split, this pin's
/// update must check: an inherited guarantee with an implementation-local
/// witness is representable, and swapping one valid witness for another
/// leaves caller/import-slot contract identity unchanged.
#[test]
fn machine_record_still_conflates_guarantee_and_witness() {
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
            type_parameters: _,
            contains: _,
            owned_data: _,
            satisfies: _,
            terminates: _,   // STILL guarantee AND witness, as one bool
            decreases: _,    // STILL the private witness material, in the interface record
            decrease_order: _,
            effects: _,      // STILL decision 22's kinded rows, as a flat name span
            contracts: _,
            states: _,
        } = machine;
    }
    let _ = witness;
}

/// LOSS 3 -- RE-PINNED (STR3 first slice, 2026-07-16): `DataProperties`
/// now carries the first-class `Multiplicity` populated at the
/// syntax->resolved lowering (`[copy]` -> Unrestricted, ordinary data ->
/// Affine; `[linear]` has no spelling yet) and COPIED (never re-derived)
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

/// LOSS 4 (record §Effects): `EffectSet` is one flat bitset -- no member
/// kinds (`ServiceReach` vs `OperationalMay`), no normalized row identity,
/// no published-ceiling vs checked-inferred split. The public surface today
/// is bit-flat: an empty set unions/inserts by name-assigned bit. After the
/// kinded `EffectRow` lands, the flat set may survive ONLY as a derived
/// compatibility projection ("no semantic decision depends on projecting
/// back from it"), and this pin flips to assert exactly that derivation.
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
/// ownership summaries record MOVE and DROP events only -- no Establish /
/// Transfer / Consume / AffineDrop distinction, no permission entry with
/// access + provenance. Linear `Join`, transactions, and dependent-linear
/// buffers must not grow on this shape (the record's ordering constraint).
#[test]
fn ownership_summary_is_still_move_and_drop_only() {
    use omega_control_flow::StateOwnershipSummary;
    let StateOwnershipSummary {
        moves: _,
        drops: _,
        ..
    } = StateOwnershipSummary::default();
}
