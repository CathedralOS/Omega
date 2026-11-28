use super::*;
use crate::source_state::signature::StateSignature;

fn selection(invocation: StaticMachineApplicationHandle) -> AppliedCallableSignature {
    AppliedCallableSignature {
        owner: AppliedCallableOwner::MachineSelection {
            invocation,
            ordinal: 0,
            demand: Default::default(),
            selected: Default::default(),
            provider: Default::default(),
            required: None,
            source_required: Default::default(),
            caller_operands: Default::default(),
            caller_bindings: Default::default(),
            caller_lifetimes: Default::default(),
            caller_lifetime_declaration: Default::default(),
            caller_lifetime_owner: Default::default(),
            provider_lifetime_declaration: Default::default(),
            provider_lifetime_owner: Default::default(),
            provider_lifetimes: Default::default(),
        },
        signature: Default::default(),
    }
}

fn rows(
    table: &AppliedCallableTable,
    invocation: StaticMachineApplicationHandle,
) -> Vec<CallableRow> {
    table
        .machine_rows(invocation)
        .map(|(handle, _)| handle)
        .collect()
}

#[test]
fn callback_contract_rows_keep_their_promise_on_storage_recovery() {
    let application = StaticMachineApplicationHandle::from_parts(3, 1);
    let other_generation = StaticMachineApplicationHandle::from_parts(3, 2);
    let source = StateSignature {
        symbol: SymbolHandle::from_parts(7, 1),
        termination_guarantee: source_declarations::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        },
        ..Default::default()
    };
    let mut table = AppliedCallableTable::default();
    let stored = table.insert(AppliedCallableSignature {
        owner: AppliedCallableOwner::StaticCallback {
            parameter: source.symbol,
            application,
            lexical_owner: SymbolHandle::from_parts(8, 1),
            source: source.clone(),
            bindings: Default::default(),
            lifetimes: Default::default(),
        },
        signature: source.clone(),
    });
    let encoded = record_storage::encode(&table, Default::default()).unwrap();
    let mut recovered: AppliedCallableTable =
        record_storage::decode(&encoded, Default::default()).unwrap();
    assert!(recovered.machine_rows(application).next().is_none());
    assert!(recovered.callback_rows(other_generation).next().is_none());
    let rows = recovered.callback_rows(application).collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, stored);
    assert_eq!(rows[0].1.signature, source);
    assert!(
        matches!(&rows[0].1.owner, AppliedCallableOwner::StaticCallback { source: retained, .. } if retained == &source)
    );
    assert!(recovered.free(stored));
    assert!(recovered.callback_rows(application).next().is_none());
}

#[test]
fn invocation_generations_keep_separate_rows_in_arena_order() {
    let older = StaticMachineApplicationHandle::from_parts(17, 2);
    let newer = StaticMachineApplicationHandle::from_parts(17, 3);
    let unrelated = StaticMachineApplicationHandle::from_parts(18, 2);
    let mut table = AppliedCallableTable::default();
    let first = table.insert(selection(older));
    let second = table.insert(selection(newer));
    table.insert(selection(unrelated));
    let last = table.insert(selection(older));
    assert_eq!(rows(&table, older), [first, last]);
    assert_eq!(rows(&table, newer), [second]);
    assert!(rows(&table, StaticMachineApplicationHandle::from_parts(17, 4)).is_empty());
    table.free(second);
    assert_eq!(rows(&table, older), [first, last]);
    assert!(rows(&table, newer).is_empty());
}

#[test]
fn recycled_row_slots_return_to_their_arena_position() {
    let invocation = StaticMachineApplicationHandle::from_parts(1, 1);
    let other = StaticMachineApplicationHandle::from_parts(2, 1);
    let mut table = AppliedCallableTable::default();
    let first = table.insert(selection(invocation));
    let middle = table.insert(selection(other));
    let last = table.insert(selection(invocation));
    assert!(table.free(first));
    assert!(!table.free(first));
    let recycled = table.insert(selection(invocation));
    assert_eq!(recycled.arena_index(), first.arena_index());
    assert_ne!(recycled.generation(), first.generation());
    assert_eq!(rows(&table, invocation), [recycled, last]);
    assert_eq!(rows(&table, other), [middle]);
    table.replace(last, selection(other)).unwrap();
    assert_eq!(rows(&table, invocation), [recycled]);
    assert_eq!(rows(&table, other), [middle, last]);
    assert!(table.replace(first, selection(other)).is_err());
}

#[test]
fn clones_replacement_and_record_recovery_keep_their_own_associations() {
    let first = StaticMachineApplicationHandle::from_parts(3, 1);
    let second = StaticMachineApplicationHandle::from_parts(3, 2);
    let mut table = AppliedCallableTable::default();
    let one = table.insert(selection(first));
    let two = table.insert(selection(first));
    let clone = table.clone();
    table.free(one);
    assert_eq!(rows(&clone, first), [one, two]);
    assert_eq!(rows(&table, first), [two]);
    let mut replacement = Arena::default();
    let replacement_one = replacement.insert(selection(second));
    let replacement_two = replacement.insert(selection(second));
    assert_ne!(replacement.revision(), clone.revision());
    table = replacement.into();
    assert!(rows(&table, first).is_empty());
    assert_eq!(rows(&table, second), [replacement_one, replacement_two]);
    let limits = record_storage::RecordLimits::default();
    let encoded = record_storage::encode(&table, limits).unwrap();
    assert_eq!(
        encoded,
        record_storage::encode(&table.arena, record_storage::RecordLimits::default()).unwrap()
    );
    let restored: AppliedCallableTable =
        record_storage::decode(&encoded, record_storage::RecordLimits::default()).unwrap();
    assert_eq!(restored, table);
    assert_eq!(rows(&restored, second), [replacement_one, replacement_two]);
    assert!(rows(&restored, first).is_empty());
    #[derive(record_storage::Record)]
    struct PriorTableRecord {
        arena: Arena<AppliedCallableSignature>,
    }
    let prior = PriorTableRecord {
        arena: record_storage::decode(&encoded, Default::default()).unwrap(),
    };
    assert_eq!(
        record_storage::encode(&prior, record_storage::RecordLimits::default()).unwrap(),
        encoded
    );
    let mut saw_write_success = false;
    let mut saw_read_success = false;
    for depth in 0..=record_storage::RecordLimits::default().depth {
        let limits = record_storage::RecordLimits {
            depth,
            ..Default::default()
        };
        let old_write = record_storage::encode(&prior, limits);
        let new_write = record_storage::encode(&table, limits);
        assert_eq!(old_write.is_ok(), new_write.is_ok(), "write depth {depth}");
        if let Ok(old_bytes) = old_write {
            saw_write_success = true;
            assert_eq!(old_bytes, new_write.unwrap());
        }
        let old_read = record_storage::decode::<PriorTableRecord>(&encoded, limits);
        let new_read = record_storage::decode::<AppliedCallableTable>(&encoded, limits);
        assert_eq!(old_read.is_ok(), new_read.is_ok(), "read depth {depth}");
        if let Ok(restored) = new_read {
            saw_read_success = true;
            assert_eq!(rows(&restored, second), [replacement_one, replacement_two]);
        }
        if depth == 0 {
            assert!(
                record_storage::encode(&table, limits)
                    .unwrap_err()
                    .reason
                    .contains("nesting")
            );
            assert!(
                record_storage::decode::<AppliedCallableTable>(&encoded, limits)
                    .unwrap_err()
                    .reason
                    .contains("nesting")
            );
        }
    }
    assert!(saw_write_success && saw_read_success);
    // Both purposes share the same links without borrowing each other's rows.
    // Lexical-owner generations remain part of a requirement's exact identity.
    let owner = SymbolHandle::from_parts(29, 1);
    let other_owner = SymbolHandle::from_parts(29, 2);
    let requirement = |application, lexical_owner| AppliedCallableSignature {
        owner: AppliedCallableOwner::RequirementInvocation {
            declaration: Default::default(),
            application,
            lexical_owner,
            source: Default::default(),
            bindings: Default::default(),
            lifetimes: Default::default(),
            arguments: Default::default(),
        },
        signature: Default::default(),
    };
    let requirement_rows = |table: &AppliedCallableTable, application, lexical_owner| {
        table
            .invocation_rows(application, InvocationPurpose::Requirement(lexical_owner))
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>()
    };
    let mut mixed = AppliedCallableTable::default();
    let machine = mixed.insert(selection(first));
    let early = mixed.insert(requirement(first, owner));
    let separate = mixed.insert(requirement(first, other_owner));
    let late = mixed.insert(requirement(first, owner));
    assert_eq!(rows(&mixed, first), [machine]);
    assert_eq!(requirement_rows(&mixed, first, owner), [early, late]);
    assert_eq!(requirement_rows(&mixed, first, other_owner), [separate]);
    assert!(requirement_rows(&mixed, second, owner).is_empty());
    let unchanged = mixed.clone();
    mixed.free(early);
    let recycled = mixed.insert(requirement(first, owner));
    assert_eq!(recycled.arena_index(), early.arena_index());
    assert_ne!(recycled.generation(), early.generation());
    assert_eq!(requirement_rows(&mixed, first, owner), [recycled, late]);
    assert_eq!(requirement_rows(&unchanged, first, owner), [early, late]);
    mixed.replace(late, requirement(second, owner)).unwrap();
    assert_eq!(requirement_rows(&mixed, first, owner), [recycled]);
    assert_eq!(requirement_rows(&mixed, second, owner), [late]);
    assert_eq!(requirement_rows(&mixed, first, other_owner), [separate]);
    let encoded = record_storage::encode(&mixed, Default::default()).unwrap();
    let restored: AppliedCallableTable =
        record_storage::decode(&encoded, Default::default()).unwrap();
    assert_eq!(restored, mixed);
    assert_eq!(rows(&restored, first), [machine]);
    assert_eq!(requirement_rows(&restored, first, owner), [recycled]);
    assert_eq!(requirement_rows(&restored, second, owner), [late]);
    assert_eq!(requirement_rows(&restored, first, other_owner), [separate]);
}

#[test]
fn signature_replacement_preserves_invocation_ownership() {
    let invocation = StaticMachineApplicationHandle::from_parts(1, 1);
    let other = StaticMachineApplicationHandle::from_parts(1, 2);
    let mut table = AppliedCallableTable::default();
    let handle = table.insert(selection(invocation));
    let predecessor = table.clone();
    assert!(std::ptr::eq(table.get(handle), predecessor.get(handle)));
    let signature = StateSignature {
        symbol: symbols::SymbolHandle::from_arena_index(9),
        ..Default::default()
    };
    let mut replacement = table.get(handle).clone();
    replacement.signature = signature.clone();
    table.replace(handle, replacement).unwrap();
    assert_eq!(rows(&table, invocation), [handle]);
    assert!(rows(&table, other).is_empty());
    assert_eq!(table.get(handle).signature, signature);
    assert_eq!(predecessor.get(handle).signature, StateSignature::default());
    assert_eq!(rows(&predecessor, invocation), [handle]);
}

#[test]
fn absent_invocations_and_trait_only_tables_have_no_machine_rows() {
    let invocation = StaticMachineApplicationHandle::from_parts(99, 4);
    let mut table = AppliedCallableTable::default();
    assert!(rows(&table, invocation).is_empty());
    table.insert(AppliedCallableSignature {
        owner: AppliedCallableOwner::Trait {
            declaration: Default::default(),
            application: Default::default(),
            scope: Default::default(),
        },
        signature: Default::default(),
    });
    assert!(rows(&table, invocation).is_empty());
    assert!(rows(&table, StaticMachineApplicationHandle::default()).is_empty());
}

#[test]
fn record_recovery_budgets_association_before_allocating_for_handles() {
    let mut arena = Arena::default();
    arena.insert(selection(StaticMachineApplicationHandle::from_parts(
        u32::MAX,
        1,
    )));
    let bytes = record_storage::encode(&arena, record_storage::RecordLimits::default()).unwrap();
    let error = record_storage::decode::<AppliedCallableTable>(
        &bytes,
        record_storage::RecordLimits::default(),
    )
    .unwrap_err();
    assert!(error.reason.contains("allocation"));
}

#[test]
fn small_record_recovery_charges_every_preallocated_association_buffer() {
    let mut arena = Arena::default();
    arena.insert(selection(StaticMachineApplicationHandle::from_parts(1, 1)));
    let arena = RootedArena::from(arena);
    let bytes = record_storage::encode(&arena, record_storage::RecordLimits::default()).unwrap();
    let mut low = 0;
    let mut high = record_storage::RecordLimits::default().allocation;
    while low < high {
        let middle = low + (high - low) / 2;
        let limits = record_storage::RecordLimits {
            allocation: middle,
            ..Default::default()
        };
        if record_storage::decode::<RootedArena<AppliedCallableSignature>>(&bytes, limits).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let association = AssociationLayout::measure(&arena)
        .unwrap()
        .allocation_bytes()
        .unwrap();
    assert!(association > 0);
    let tight = record_storage::RecordLimits {
        allocation: low + association - 1,
        ..Default::default()
    };
    assert!(record_storage::decode::<RootedArena<AppliedCallableSignature>>(&bytes, tight).is_ok());
    let error = record_storage::decode::<AppliedCallableTable>(&bytes, tight).unwrap_err();
    assert!(error.reason.contains("allocation"));
    let exact = record_storage::RecordLimits {
        allocation: low + association,
        ..Default::default()
    };
    let restored: AppliedCallableTable = record_storage::decode(&bytes, exact).unwrap();
    assert_eq!(
        rows(&restored, StaticMachineApplicationHandle::from_parts(1, 1)).len(),
        1
    );
}
