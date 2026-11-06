use super::*;

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
    assert_eq!(replacement.revision(), clone.revision());
    table = replacement.into();
    assert!(rows(&table, first).is_empty());
    assert_eq!(rows(&table, second), [replacement_one, replacement_two]);
    let limits = compiler_storage::RecordLimits::default();
    let encoded = compiler_storage::encode(&table, limits).unwrap();
    assert_eq!(
        encoded,
        compiler_storage::encode(&table.arena, compiler_storage::RecordLimits::default()).unwrap()
    );
    let restored: AppliedCallableTable =
        compiler_storage::decode(&encoded, compiler_storage::RecordLimits::default()).unwrap();
    assert_eq!(restored, table);
    assert_eq!(rows(&restored, second), [replacement_one, replacement_two]);
    assert!(rows(&restored, first).is_empty());
    // The old derived wrapper added one record-depth boundary around Arena.
    // Compare both directions against that independent generated implementation.
    #[derive(compiler_storage::Record)]
    struct PriorTableRecord {
        arena: Arena<AppliedCallableSignature>,
    }
    let prior = PriorTableRecord {
        arena: table.arena.clone(),
    };
    assert_eq!(
        compiler_storage::encode(&prior, compiler_storage::RecordLimits::default()).unwrap(),
        encoded
    );
    let mut saw_write_success = false;
    let mut saw_read_success = false;
    for depth in 0..=compiler_storage::RecordLimits::default().depth {
        let limits = compiler_storage::RecordLimits {
            depth,
            ..Default::default()
        };
        let old_write = compiler_storage::encode(&prior, limits);
        let new_write = compiler_storage::encode(&table, limits);
        assert_eq!(old_write.is_ok(), new_write.is_ok(), "write depth {depth}");
        if let Ok(old_bytes) = old_write {
            saw_write_success = true;
            assert_eq!(old_bytes, new_write.unwrap());
        }
        let old_read = compiler_storage::decode::<PriorTableRecord>(&encoded, limits);
        let new_read = compiler_storage::decode::<AppliedCallableTable>(&encoded, limits);
        assert_eq!(old_read.is_ok(), new_read.is_ok(), "read depth {depth}");
        if let Ok(restored) = new_read {
            saw_read_success = true;
            assert_eq!(rows(&restored, second), [replacement_one, replacement_two]);
        }
        if depth == 0 {
            assert!(
                compiler_storage::encode(&table, limits)
                    .unwrap_err()
                    .reason
                    .contains("nesting")
            );
            assert!(
                compiler_storage::decode::<AppliedCallableTable>(&encoded, limits)
                    .unwrap_err()
                    .reason
                    .contains("nesting")
            );
        }
    }
    assert!(saw_write_success && saw_read_success);
}

#[test]
fn signature_and_requirement_updates_cannot_change_invocation_ownership() {
    let invocation = StaticMachineApplicationHandle::from_parts(1, 1);
    let other = StaticMachineApplicationHandle::from_parts(1, 2);
    let mut table = AppliedCallableTable::default();
    let handle = table.insert(selection(invocation));
    let signature = StateSignature {
        symbol: symbols::SymbolHandle::from_arena_index(9),
        ..Default::default()
    };
    table.replace_signature(handle, signature.clone()).unwrap();
    table.set_required(handle, signature.clone()).unwrap();
    assert_eq!(rows(&table, invocation), [handle]);
    assert!(rows(&table, other).is_empty());
    assert_eq!(table.get(handle).signature, signature);
    assert!(
        matches!(&table.get(handle).owner, AppliedCallableOwner::MachineSelection { required: Some(required), .. } if *required == signature)
    );
    let non_selection = table.insert(AppliedCallableSignature::default());
    assert!(
        table
            .set_required(non_selection, StateSignature::default())
            .is_err()
    );
    assert_eq!(rows(&table, invocation), [handle]);
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
    let bytes =
        compiler_storage::encode(&arena, compiler_storage::RecordLimits::default()).unwrap();
    let error = compiler_storage::decode::<AppliedCallableTable>(
        &bytes,
        compiler_storage::RecordLimits::default(),
    )
    .unwrap_err();
    assert!(error.reason.contains("allocation"));
}

#[test]
fn small_record_recovery_charges_every_preallocated_association_buffer() {
    let mut arena = Arena::default();
    arena.insert(selection(StaticMachineApplicationHandle::from_parts(1, 1)));
    let bytes =
        compiler_storage::encode(&arena, compiler_storage::RecordLimits::default()).unwrap();
    // Find the actual arena decoder's allocation requirement, independently
    // of this table's association accounting and nested signature layout.
    let mut low = 0;
    let mut high = compiler_storage::RecordLimits::default().allocation;
    while low < high {
        let middle = low + (high - low) / 2;
        let limits = compiler_storage::RecordLimits {
            allocation: middle,
            ..Default::default()
        };
        if compiler_storage::decode::<Arena<AppliedCallableSignature>>(&bytes, limits).is_ok() {
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
    let tight = compiler_storage::RecordLimits {
        allocation: low + association - 1,
        ..Default::default()
    };
    assert!(compiler_storage::decode::<Arena<AppliedCallableSignature>>(&bytes, tight).is_ok());
    let error = compiler_storage::decode::<AppliedCallableTable>(&bytes, tight).unwrap_err();
    assert!(error.reason.contains("allocation"));
    let exact = compiler_storage::RecordLimits {
        allocation: low + association,
        ..Default::default()
    };
    let restored: AppliedCallableTable = compiler_storage::decode(&bytes, exact).unwrap();
    assert_eq!(
        rows(&restored, StaticMachineApplicationHandle::from_parts(1, 1)).len(),
        1
    );
}
