//! Arena tests.

use crate::{Arena, Handle};

#[test]
fn resolves_zero_to_dummy_slot() {
    let mut arena = Arena::new();
    let invalid = Handle::<String>::invalid();
    let first = arena.insert("alpha".to_owned());
    let second = arena.insert("beta".to_owned());

    assert!(!invalid.is_valid());
    assert_eq!(arena.len(), 2);
    assert_eq!(first.arena_index(), 1);
    assert_eq!(second.arena_index(), 2);
    assert_eq!(arena.get(invalid).as_str(), "");
    assert_eq!(arena.get(first).as_str(), "alpha");
    assert_eq!(arena.get(second).as_str(), "beta");
}

#[test]
fn invalidates_freed_handles() {
    let mut arena = Arena::new();
    let first = arena.insert("alpha".to_owned());

    assert_eq!(arena.get(first).as_str(), "alpha");
    assert!(arena.is_valid(first));
    assert!(arena.free(first));
    assert!(!arena.is_valid(first));
    assert_eq!(arena.get(first).as_str(), "");

    let reused = arena.insert("beta".to_owned());

    assert_eq!(reused.arena_index(), first.arena_index());
    assert_ne!(reused.generation(), first.generation());
    assert_eq!(arena.get(first).as_str(), "");
    assert_eq!(arena.get(reused).as_str(), "beta");
}

#[test]
fn stores_contiguous_handle_spans() {
    let mut arena = Arena::new();
    let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);

    assert_eq!(span.start().arena_index(), 1);
    assert_eq!(span.count(), 3);
    assert_eq!(
        arena.span(span).expect("span should resolve"),
        &["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]
    );

    arena.span_mut(span).expect("span should resolve")[1] = "bravo".to_owned();

    assert_eq!(
        arena.span(span).expect("span should resolve"),
        &["alpha".to_owned(), "bravo".to_owned(), "gamma".to_owned()]
    );
}

#[test]
fn relocates_stale_spans_to_the_tail_before_appending() {
    let mut arena = Arena::new();
    let mut span = crate::HandleSpan::empty();
    arena.append_to_span(&mut span, "alpha".to_owned());
    let gap = arena.append("gap".to_owned());

    let beta = arena.append_to_span(&mut span, "beta".to_owned());

    // The stale span is copied to the tail so the appended row sequence stays
    // readable; unrelated rows and earlier member handles still resolve.
    assert_eq!(beta.arena_index(), 4);
    assert_eq!(span.start().arena_index(), 3);
    assert_eq!(span.count(), 2);
    assert_eq!(
        arena.span(span).expect("span should resolve"),
        &["alpha".to_owned(), "beta".to_owned()]
    );
    assert_eq!(arena.get(gap).as_str(), "gap");
}

#[test]
#[should_panic(expected = "arena span append on an unresolvable span")]
fn panics_when_extending_a_span_with_freed_rows() {
    let mut arena = Arena::new();
    let mut span = crate::HandleSpan::empty();
    let alpha = arena.append_to_span(&mut span, "alpha".to_owned());
    assert!(arena.free(alpha));
    arena.append("gap".to_owned());

    arena.append_to_span(&mut span, "beta".to_owned());
}

#[test]
fn fallible_span_insert_rolls_back_partial_appends() {
    let mut arena = Arena::new();
    let first = arena.insert("alpha".to_owned());

    let result =
        arena.try_insert_many([Ok("beta".to_owned()), Err("boom"), Ok("gamma".to_owned())]);

    assert_eq!(result, Err("boom"));
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.get(first).as_str(), "alpha");

    let next = arena.append("delta".to_owned());

    assert_eq!(next.arena_index(), 2);
    assert_eq!(arena.get(next).as_str(), "delta");
}

#[test]
fn fallible_span_insert_returns_contiguous_span_on_success() {
    let mut arena = Arena::new();
    let span = arena
        .try_insert_many::<&str>([Ok("alpha".to_owned()), Ok("beta".to_owned())])
        .expect("fallible insert should succeed");

    assert_eq!(span.start().arena_index(), 1);
    assert_eq!(span.count(), 2);
    assert_eq!(
        arena.span(span).expect("span should resolve"),
        &["alpha".to_owned(), "beta".to_owned()]
    );
}

#[test]
fn fallible_direct_span_insert_rolls_back_partial_appends() {
    let mut arena = Arena::new();
    let first = arena.insert("alpha".to_owned());

    let result = arena.try_insert_many_with(|inserter| {
        inserter.insert("beta".to_owned());
        Err("boom")
    });

    assert_eq!(result, Err("boom"));
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.get(first).as_str(), "alpha");

    let next = arena.append("delta".to_owned());

    assert_eq!(next.arena_index(), 2);
    assert_eq!(arena.get(next).as_str(), "delta");
}

#[test]
fn fallible_direct_span_insert_returns_contiguous_span_on_success() {
    let mut arena = Arena::new();
    let span = arena
        .try_insert_many_with::<&str>(|inserter| {
            inserter.insert("alpha".to_owned());
            inserter.insert("beta".to_owned());
            Ok(())
        })
        .expect("fallible direct insert should succeed");

    assert_eq!(span.start().arena_index(), 1);
    assert_eq!(span.count(), 2);
    assert_eq!(
        arena.span(span).expect("span should resolve"),
        &["alpha".to_owned(), "beta".to_owned()]
    );
}

#[test]
fn copies_two_spans_into_one_contiguous_span() {
    let mut arena = Arena::new();
    let first = arena.insert_many(["alpha".to_owned(), "beta".to_owned()]);
    let _gap = arena.insert_many(["gap".to_owned()]);
    let second = arena.insert_many(["gamma".to_owned(), "delta".to_owned()]);

    let copied = arena.copy_span_pair(first, second);

    assert_eq!(copied.start().arena_index(), 6);
    assert_eq!(copied.count(), 4);
    assert_eq!(
        arena.span(copied).expect("span should resolve"),
        &[
            "alpha".to_owned(),
            "beta".to_owned(),
            "gamma".to_owned(),
            "delta".to_owned()
        ]
    );
}

#[test]
fn rejects_spans_with_freed_slots() {
    let mut arena = Arena::new();
    let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);
    let middle = Handle::from_arena_index(2);

    assert!(arena.span(span).is_some());
    assert!(arena.free(middle));
    assert!(arena.span(span).is_none());
    assert!(arena.span_mut(span).is_none());
}

#[test]
fn rejects_spans_with_reused_slots() {
    let mut arena = Arena::new();
    let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);
    let middle = Handle::from_arena_index(2);

    assert!(arena.free(middle));
    let reused = arena.insert("delta".to_owned());

    assert_eq!(reused.arena_index(), middle.arena_index());
    assert!(arena.span(span).is_none());
    assert!(arena.span_mut(span).is_none());
}

#[test]
fn clear_invalidates_existing_handles() {
    let mut arena = Arena::new();
    let first = arena.insert("alpha".to_owned());

    arena.clear();

    assert_eq!(arena.len(), 0);
    assert!(!arena.is_valid(first));
    assert_eq!(arena.get(first).as_str(), "");

    let reused = arena.insert("beta".to_owned());

    assert_eq!(reused.arena_index(), first.arena_index());
    assert_ne!(reused.generation(), first.generation());
    assert_eq!(arena.get(first).as_str(), "");
    assert_eq!(arena.get(reused).as_str(), "beta");
}

#[test]
fn clone_from_preserves_free_slots_generations_and_later_operations() {
    let mut source = Arena::new();
    let first = source.insert("alpha".to_owned());
    let retired = source.insert("beta".to_owned());
    let free = source.insert("gamma".to_owned());
    assert!(source.free(retired));
    let replacement = source.insert("replacement".to_owned());
    assert_ne!(replacement.generation(), retired.generation());
    assert!(source.free(free));
    *source.get_mut(Handle::invalid()) = "source dummy".to_owned();

    let mut copied = Arena::new();
    copied.insert("old".to_owned());
    copied.clone_from(&source);
    let mut expected = source.clone();
    assert_eq!(copied, expected);
    assert!(!copied.is_valid(retired));
    assert!(!copied.is_valid(free));
    assert_eq!(copied.get(replacement), source.get(replacement));
    assert_eq!(copied.get(retired), source.dummy());

    assert_eq!(
        copied.insert("reuse".to_owned()),
        expected.insert("reuse".to_owned())
    );
    assert_eq!(
        copied.append("append".to_owned()),
        expected.append("append".to_owned())
    );
    assert_eq!(copied.free(first), expected.free(first));
    assert_eq!(copied, expected);

    copied.get_mut(replacement).push_str(" changed");
    assert_eq!(source.get(replacement), "replacement");
    copied.clone_from(&Arena::default());
    assert_eq!(copied, Arena::default());
    copied.clone_from(&source);
    assert_eq!(copied, source.clone());
}

#[test]
fn clone_from_reuses_storage_and_nested_payload_capacity_when_source_fits() {
    let mut source = Arena::new();
    source.append(vec!["short".to_owned()]);
    let free = source.append(vec!["unused".to_owned()]);
    assert!(source.free(free));
    *source.get_mut(Handle::invalid()) = vec!["dummy".to_owned()];

    let mut copied = Arena::with_capacity(8);
    copied.slot_metadata_mut().free_indices.reserve(8);
    for _ in 0..4 {
        copied.append(vec!["longer destination payload".to_owned()]);
    }
    // A retired source needs explicit slot metadata. Make that capacity part
    // of the destination before asserting that clone_from reuses it.
    let retired = copied.append(vec!["retired destination payload".to_owned()]);
    assert!(copied.free(retired));
    *copied.get_mut(Handle::invalid()) = vec!["longer destination dummy".to_owned()];
    let storage_pointers = (
        copied.items.as_ptr(),
        copied.slot_metadata().generations.as_ptr(),
        copied.slot_metadata().occupied.as_ptr(),
        copied.slot_metadata().free_indices.as_ptr(),
    );
    let capacities = (
        copied.items.capacity(),
        copied.slot_metadata().generations.capacity(),
        copied.slot_metadata().occupied.capacity(),
        copied.slot_metadata().free_indices.capacity(),
    );
    let nested_pointer = copied.items[0].as_ptr();
    let payload_pointer = copied.items[0][0].as_ptr();
    let dummy_pointer = copied.dummy[0].as_ptr();

    copied.clone_from(&source);

    assert_eq!(copied, source.clone());
    assert_eq!(
        storage_pointers,
        (
            copied.items.as_ptr(),
            copied.slot_metadata().generations.as_ptr(),
            copied.slot_metadata().occupied.as_ptr(),
            copied.slot_metadata().free_indices.as_ptr(),
        )
    );
    assert_eq!(
        capacities,
        (
            copied.items.capacity(),
            copied.slot_metadata().generations.capacity(),
            copied.slot_metadata().occupied.capacity(),
            copied.slot_metadata().free_indices.capacity(),
        )
    );
    assert_eq!(nested_pointer, copied.items[0].as_ptr());
    assert_eq!(payload_pointer, copied.items[0][0].as_ptr());
    assert_eq!(dummy_pointer, copied.dummy[0].as_ptr());
    copied.items[0][0].push_str(" changed");
    assert_eq!(source.items[0][0], "short");
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PanicOnClone {
    value: u32,
    panic_on_clone: bool,
}

impl Clone for PanicOnClone {
    fn clone(&self) -> Self {
        assert!(!self.panic_on_clone, "requested payload clone panic");
        Self {
            value: self.value,
            panic_on_clone: false,
        }
    }
}

fn assert_usable_after_clone_panic(arena: &mut Arena<PanicOnClone>) {
    assert_eq!(arena.items.len(), arena.slot_metadata().generations.len());
    assert_eq!(arena.items.len(), arena.slot_metadata().occupied.len());
    assert_eq!(arena.len(), arena.iter().count());
    for arena_index in &arena.slot_metadata().free_indices {
        let index = super::storage_index_from_arena_index(*arena_index);
        assert!(!arena.slot_metadata().occupied[index]);
    }
    for (handle, value) in arena.iter() {
        assert!(arena.is_valid(handle));
        assert_eq!(arena.get(handle), value);
    }
    let count = arena.len();
    let inserted = arena.insert(PanicOnClone {
        value: 100,
        ..Default::default()
    });
    let appended = arena.append(PanicOnClone {
        value: 101,
        ..Default::default()
    });
    assert_eq!(arena.len(), count + 2);
    assert_eq!(arena.get(inserted).value, 100);
    assert_eq!(arena.get(appended).value, 101);
    assert!(arena.free(inserted));
    assert!(!arena.is_valid(inserted));
    assert_eq!(arena.len(), count + 1);
    assert_eq!(arena.len(), arena.iter().count());
}

#[test]
fn clone_from_payload_panic_after_shrinking_preserves_arena_structure() {
    let mut source = Arena::new();
    source.append(PanicOnClone::default());
    source.append(PanicOnClone {
        value: 1,
        panic_on_clone: true,
    });
    let free = source.append(PanicOnClone::default());
    assert!(source.free(free));
    let mut copied = Arena::new();
    for _ in 0..6 {
        copied.append(PanicOnClone::default());
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        copied.clone_from(&source);
    }));

    assert!(result.is_err());
    assert_eq!(copied.items.len(), 3);
    assert_eq!(
        copied.slot_metadata().generations,
        source.slot_metadata().generations
    );
    assert_eq!(
        copied.slot_metadata().free_indices,
        source.slot_metadata().free_indices
    );
    assert_usable_after_clone_panic(&mut copied);
}

#[test]
fn clone_from_payload_panic_after_partial_growth_preserves_arena_structure() {
    let mut source = Arena::new();
    for value in 0..6 {
        source.append(PanicOnClone {
            value,
            panic_on_clone: value == 2,
        });
    }
    assert!(source.free(Handle::from_arena_index(5)));
    let mut copied = Arena::new();
    copied.append(PanicOnClone::default());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        copied.clone_from(&source);
    }));

    assert!(result.is_err());
    assert_eq!(copied.items.len(), 2);
    assert!(copied.slot_metadata().free_indices.is_empty());
    assert_usable_after_clone_panic(&mut copied);
}

/// A span resolves by bounds alone only while no slot was ever freed; once a
/// row inside it is freed, or recycled at a later generation, the span stops
/// resolving exactly as the per-row check decides.
#[test]
fn span_resolution_rechecks_rows_once_a_slot_was_freed() {
    let mut arena = Arena::new();
    let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);
    let past_end = crate::HandleSpan::from_parts(span.start(), 4);
    assert_eq!(arena.span(span).map(<[String]>::len), Some(3));
    assert!(
        arena.span(past_end).is_none(),
        "a fresh arena still bounds spans"
    );

    let middle = span.handle_at(1);
    assert!(arena.free(middle));
    assert!(arena.span(span).is_none(), "a freed row breaks the span");

    arena.insert("bravo".to_owned());
    assert!(
        arena.span(span).is_none(),
        "a row recycled at a later generation does not rejoin the span"
    );

    arena.reset_retain_capacity();
    let fresh = arena.insert_many(["delta".to_owned(), "echo".to_owned()]);
    assert_eq!(arena.span(fresh).map(<[String]>::len), Some(2));
}

#[test]
fn uniform_page_retirement_preserves_unretired_handles_and_rejects_stale_slot() {
    let mut arena = Arena::with_capacity_and_generation(8, 17);
    let span = arena.insert_many([10_u32, 20, 30]);
    let first = span.handle_at(0);
    let retired = span.handle_at(1);
    let last = span.handle_at(2);
    assert_eq!(arena.clone().into_items().collect::<Vec<_>>(), [10, 20, 30]);
    arena.for_each_mut(|_, value| *value += 1);
    assert!(arena.free(retired));
    let replacement = arena.insert(42);
    assert_eq!(replacement.arena_index(), retired.arena_index());
    assert_ne!(replacement.generation(), retired.generation());
    assert!(!arena.is_valid(retired));
    assert_eq!(*arena.get(first), 11);
    assert_eq!(*arena.get(last), 31);
    assert!(arena.span(span).is_none());
    let copy = arena.clone().map(u64::from);
    assert_eq!(copy.into_items().collect::<Vec<_>>(), [11, 42, 31]);
    arena.clear();
    for handle in [first, retired, last, replacement] {
        assert!(!arena.is_valid(handle));
    }
    assert_eq!(arena.into_items().count(), 0);
}

#[test]
fn uniform_source_clone_unwind_keeps_the_initialized_prefix_usable() {
    let mut source = Arena::new();
    for value in 0..6 {
        source.append(PanicOnClone {
            value,
            panic_on_clone: value == 2,
        });
    }
    let mut copied = Arena::new();
    let retired = copied.insert(PanicOnClone::default());
    copied.free(retired);
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| copied.clone_from(&source)));
    assert!(result.is_err());
    assert_eq!(
        copied
            .iter()
            .map(|(_, value)| value.value)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(copied.len(), 2);
    let handle = copied.insert(PanicOnClone {
        value: 10,
        ..Default::default()
    });
    assert_eq!(copied.get(handle).value, 10);
    assert!(copied.free(handle));
    assert!(!copied.is_valid(handle));
}

#[test]
fn failed_first_retirement_preserves_logical_equality() {
    #[derive(Clone, Debug, PartialEq)]
    struct Payload(u32);
    thread_local! { static FAIL_DEFAULT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    impl Default for Payload {
        fn default() -> Self {
            assert!(
                !FAIL_DEFAULT.with(|flag| flag.get()),
                "requested default panic"
            );
            Self(0)
        }
    }
    for clear_all in [false, true] {
        let mut rows = Arena::new();
        let handle = rows.append(Payload(42));
        let expected = rows.clone();
        FAIL_DEFAULT.with(|flag| flag.set(true));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if clear_all {
                rows.clear();
            } else {
                rows.free(handle);
            }
        }));
        FAIL_DEFAULT.with(|flag| flag.set(false));
        assert!(result.is_err());
        assert_eq!(rows, expected);
        assert!(rows.equal_by(&expected, |left, right| left == right));
        assert!(expected.equal_by(&rows, |left, right| left == right));
        // Refusal before changing a payload preserves append-only rewind,
        // even when clear has already materialized uniform slot metadata.
        rows.rewind_appended(1);
        let next = rows.append(Payload(7));
        assert_eq!(rows.get(handle), &Payload(42));
        assert_eq!(rows.get(next), &Payload(7));
        assert!(rows.free(handle));
        assert!(!rows.is_valid(handle));
    }
}
