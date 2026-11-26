use super::*;
use terminal_psi::TerminalIdentityIndex;

#[test]
fn case_write_addresses_remain_exact_without_narrowing_alias_bounds() {
    let root = Some(PlaceId::new(1).unwrap());
    let path = |case| {
        vec![
            StructuralPathSegment::Case(
                semantic_vocabulary::StructuralCaseId::new(case).unwrap(),
            ),
            StructuralPathSegment::Field(TerminalIdentityIndex(7)),
        ]
    };
    let mut left = Region {
        root,
        projection: Projection::new(&path(1)),
    };
    let right = Region {
        root,
        projection: Projection::new(&path(2)),
    };
    assert!(
        left.overlaps(&right),
        "case payloads overlay the same storage"
    );
    assert_eq!(left.written_storage_path(), path(1));
    left.projection.forget_address();
    assert_eq!(
        left.written_storage_path(),
        left.projection.path,
        "forgotten addresses retain their conservative write bound"
    );
    assert!(left.overlaps(&right));
}

#[test]
fn a_stripped_element_selector_keeps_the_storage_after_it() {
    use terminal_psi::ReferenceOriginProjection::{Element, Exact};
    let cells = StructuralPathSegment::Field(TerminalIdentityIndex(7));
    let x = StructuralPathSegment::Field(TerminalIdentityIndex(8));
    let selector = StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(3).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
    };
    let address = Projection::new(&[cells.clone(), selector, x.clone()]);
    let literal = |id| (id == ValueId::new(3).unwrap()).then_some(2);
    assert!(
        !address
            .clone()
            .without_runtime_coordinates_with(literal, |_| true)
            .exact
    );
    let selected = address.at((semantic_vocabulary::BlockId::new(1).unwrap(), 0));
    let known = selected
        .clone()
        .without_runtime_coordinates_with(literal, |_| true);
    assert!(known.exact);
    assert_eq!(known.path, known.captured_address_path().unwrap());
    assert_eq!(
        known.captured_address_path(),
        Ok(vec![
            cells.clone(),
            StructuralPathSegment::FixedIndex(2),
            x.clone()
        ])
    );
    let at = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
    let within_window = Projection::new(std::slice::from_ref(&cells))
        .with_window(ValueId::new(1).unwrap(), ValueId::new(2).unwrap(), at)
        .then(
            &Projection::new(&[StructuralPathSegment::RuntimeIndex {
                index: ValueId::new(3).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            }])
            .at(at),
        );
    let absolute = within_window
        .clone()
        .without_runtime_coordinates_with(|id| match id.get() {
            1 => Some(4),
            2 => Some(8),
            3 => Some(2),
            _ => None,
        }, |_| true);
    assert!(absolute.exact);
    assert_eq!(
        absolute.captured_storage_path(),
        Some(vec![cells.clone(), StructuralPathSegment::FixedIndex(6)])
    );
    assert!(
        !within_window
            .without_runtime_coordinates_with(literal, |_| true)
            .exact
    );
    // An open call can promise a region without proving the exact address.
    let mut region = Projection::new(&[]);
    region.exact = false;
    assert!(
        !region
            .without_runtime_coordinates_with(literal, |_| true)
            .exact
    );
    let stripped = selected.clone().without_runtime_coordinates();
    assert!(stripped.captured_address_path().is_err());
    assert_eq!(stripped.path.as_slice(), std::slice::from_ref(&cells));
    assert_eq!(
        stripped.stripped_origin_path(),
        Some(vec![Exact(cells.clone()), Element, Exact(x)])
    );
    assert_eq!(
        stripped.clone().without_runtime_coordinates(),
        stripped,
        "stripping is idempotent"
    );
    let mut forgotten = selected;
    forgotten.forget_address();
    assert_eq!(forgotten.stripped_origin_path(), None);
    let window = Projection::new(std::slice::from_ref(&cells))
        .with_window(
            ValueId::new(1).unwrap(),
            ValueId::new(2).unwrap(),
            (semantic_vocabulary::BlockId::new(1).unwrap(), 0),
        )
        .without_runtime_coordinates();
    assert_eq!(
        window.stripped_origin_path(),
        None,
        "a stripped window has no known start"
    );
}

#[test]
fn captured_address_prefix_keeps_field_extensions_and_each_selector_capture() {
    let first = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
    let second = (semantic_vocabulary::BlockId::new(2).unwrap(), 0);
    let later = (semantic_vocabulary::BlockId::new(2).unwrap(), 1);
    let selector = |value| StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(value).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
    };
    let field = StructuralPathSegment::Field(TerminalIdentityIndex(7));
    let parent = Projection::new(&[selector(1)]).at(first);
    let child = parent.then(&Projection::new(std::slice::from_ref(&field)));
    assert_ne!(parent, child);
    assert!(parent.captured_address_is_prefix_of(&child));
    assert!(!child.captured_address_is_prefix_of(&parent));
    assert!(parent.captured_address_is_prefix_of(&parent));

    let nested = parent.then(&Projection::new(&[field.clone(), selector(2)]).at(second));
    let nested_child = nested.then(&Projection::new(std::slice::from_ref(&field)));
    assert_eq!(
        nested_child.index_captures().collect::<Vec<_>>(),
        [(ValueId::new(1).unwrap(), first), (ValueId::new(2).unwrap(), second)]
    );
    assert!(parent.captured_address_is_prefix_of(&nested_child));
    assert!(nested.captured_address_is_prefix_of(&nested_child));
    let recaptured = parent.then(&Projection::new(&[field, selector(2)]).at(later));
    assert_eq!(nested.captured_at, recaptured.captured_at);
    assert_eq!(
        nested.captured_address_path(),
        recaptured.captured_address_path()
    );
    assert!(!nested.captured_address_is_prefix_of(&recaptured));
    assert!(!recaptured.captured_address_is_prefix_of(&nested_child));
}

#[test]
fn captured_address_prefix_refuses_missing_erased_and_non_path_accounts() {
    let first = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
    let later = (semantic_vocabulary::BlockId::new(1).unwrap(), 1);
    let selector = StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(1).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
    };
    let parent = Projection::new(std::slice::from_ref(&selector)).at(first);
    let recaptured = Projection::new(std::slice::from_ref(&selector)).at(later);
    assert!(!parent.captured_address_is_prefix_of(&recaptured));
    let unsupported = [
        Projection::new(&[selector]),
        parent.clone().without_captured_address(),
        parent.clone().without_runtime_coordinates(),
        parent.clone().with_window(
            ValueId::new(2).unwrap(),
            ValueId::new(3).unwrap(),
            first,
        ),
        parent.then(&Projection::new(&[StructuralPathSegment::AllElements])),
        parent.then(&Projection::new(&[StructuralPathSegment::FixedRange {
            start: 0,
            end: 1,
        }])),
    ];
    for account in unsupported {
        assert!(!account.captured_address_is_prefix_of(&parent));
        assert!(!parent.captured_address_is_prefix_of(&account));
        assert!(!Projection::new(&[]).captured_address_is_prefix_of(&account));
    }
}

#[test]
fn successive_windows_from_one_site_keep_their_source_without_claiming_an_offset() {
    let at = (semantic_vocabulary::BlockId::new(1).unwrap(), 2);
    let start = ValueId::new(1).unwrap();
    let end = ValueId::new(2).unwrap();
    let window = Projection::window(start, end, at);
    let recurring = window.then(&window);
    assert_eq!(recurring.then(&window), recurring);
    assert_eq!(
        recurring.validate_captured_storage_address(|pos| pos == at),
        Ok(None)
    );
    assert!(
        recurring
            .validate_captured_storage_address(|_| false)
            .is_err()
    );
    assert!(!recurring.captured_address_is_prefix_of(&recurring));
    assert_eq!(recurring.captured_at, None);
    // Index zero is in the current window. Its offset in the original
    // descriptor depends on how many times the loop selected a tail.
    let head = Projection::new(&[StructuralPathSegment::FixedIndex(0)]);
    let selected = recurring.then(&head).without_runtime_coordinates_with(
        |value| Some(if value == start { 1 } else { 8 }),
        |_| true,
    );
    assert_eq!(
        selected.stripped_origin_path(),
        Some(vec![terminal_psi::ReferenceOriginProjection::Element])
    );
    assert!(selected.captured_address_path().is_err());
    assert!(
        recurring
            .clone()
            .without_runtime_coordinates()
            .validate_captured_storage_address(|_| true)
            .is_err()
    );
    assert!(
        window
            .then(&head)
            .then(&window)
            .validate_captured_storage_address(|_| true)
            .is_err()
    );
    assert!(
        window
            .then(&Projection::window(end, start, at))
            .validate_captured_storage_address(|_| true)
            .is_err()
    );
}

#[test]
fn full_captured_address_keeps_reference_crossings_without_inline_permission() {
    let path = vec![
        StructuralPathSegment::Referent,
        StructuralPathSegment::Referent,
        StructuralPathSegment::FixedIndex(1),
    ];
    let leading = Projection::new(&[StructuralPathSegment::Referent]);
    assert!(!leading.indirect);
    assert_eq!(leading.has_captured_reference_crossing(), Ok(true));
    assert_eq!(
        leading.captured_storage_path(),
        Some(vec![StructuralPathSegment::Referent])
    );
    let projection = Projection::new(&path);
    assert_eq!(projection.captured_address_path(), Ok(path));
    assert!(
        projection.captured_storage_path().is_none(),
        "physical address recovery must not grant inline loan permission"
    );
    let window = Projection::window(
        ValueId::new(1).unwrap(),
        ValueId::new(2).unwrap(),
        (semantic_vocabulary::BlockId::new(1).unwrap(), 0),
    );
    assert_eq!(window.validate_captured_storage_address(|_| true), Ok(None));
    assert_eq!(
        window.captured_address_path(),
        Err("captured storage window is not a finite structural path")
    );
    assert_eq!(
        projection
            .without_captured_address()
            .captured_address_path(),
        Err("captured storage address was forgotten")
    );
}

#[test]
fn a_slice_keeps_its_input_field_but_not_a_later_field_or_index() {
    let first = StructuralPathSegment::Field(TerminalIdentityIndex(7));
    let projection = Projection::new(std::slice::from_ref(&first))
        .with_window(
            ValueId::new(1).unwrap(),
            ValueId::new(2).unwrap(),
            (semantic_vocabulary::BlockId::new(1).unwrap(), 0),
        )
        .without_runtime_coordinates();
    assert_eq!(
        projection.original_descriptor_path(),
        Some(std::slice::from_ref(&first))
    );
    assert_eq!(
        projection
            .then(&Projection::new(&[]))
            .original_descriptor_path(),
        Some(std::slice::from_ref(&first))
    );
    for later in [
        StructuralPathSegment::Field(TerminalIdentityIndex(8)),
        StructuralPathSegment::FixedIndex(0),
    ] {
        let changed = projection
            .then(&Projection::new(&[later]))
            .without_runtime_coordinates();
        assert_eq!(
            changed.path.as_slice(),
            std::slice::from_ref(&first),
            "the overlap path stays conservative"
        );
        assert!(
            changed.original_descriptor_path().is_none(),
            "the old overlap path cannot identify a later borrowed field"
        );
    }
}
