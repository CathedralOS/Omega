use super::*;
use semantic_vocabulary::{
    IntegerSign, IntegerType, MachineId, StructuralFieldId, ValueId, WidthParameter,
};

fn layout() -> NativeBorrowedStorageLayout {
    NativeBorrowedStorageLayout::new(
        StructuralTypeId::new(1).unwrap(),
        8,
        4,
        [(
            Vec::new(),
            Vec::new(),
            4,
            ScalarType::Boolean,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 13).unwrap()),
        )],
        [],
        [],
    )
}

fn full_layout() -> NativeBorrowedStorageLayout {
    let path = vec![
        CanonicalStructuralPathSegment::Field(StructuralFieldId::new(2).unwrap()),
        CanonicalStructuralPathSegment::FixedIndex(0),
        CanonicalStructuralPathSegment::Case(StructuralCaseId::new(3).unwrap()),
        CanonicalStructuralPathSegment::Referent,
    ];
    let repetitions = vec![NativeStorageRepetition {
        path_position: 1,
        length: 3,
        byte_stride: 8,
    }];
    NativeBorrowedStorageLayout::new(
        StructuralTypeId::new(1).unwrap(),
        32,
        8,
        [(
            path.clone(),
            repetitions.clone(),
            0,
            ScalarType::Boolean,
            ScalarType::Boolean,
        )],
        [(
            path.clone(),
            repetitions.clone(),
            StructuralTypeId::new(4).unwrap(),
            4,
            vec![NativeStorageCaseTag {
                case: StructuralCaseId::new(3).unwrap(),
                native_tag: -2,
            }],
        )],
        [(path, repetitions, 8, 8)],
    )
}

fn discriminator_mut(
    layout: &mut NativeBorrowedStorageLayout,
) -> &mut NativeBorrowedStorageDiscriminator {
    layout
        .discriminator_storage
        .span_mut(layout.discriminators)
        .unwrap()[0]
        .0
        .as_mut()
        .unwrap()
}

fn case_mut(layout: &mut NativeBorrowedStorageLayout) -> &mut NativeStorageCaseTagSlot {
    let span = discriminator_mut(layout).cases;
    &mut layout.cases.span_mut(span).unwrap()[0]
}

#[test]
fn canonical_bytes_pin_geometry_and_scalar_identity() {
    let roster = NativeBorrowedStorageLayouts::seal(vec![layout()]).unwrap();
    let mut expected = b"omega.native-borrowed-storage.v1\0".to_vec();
    expected.extend_from_slice(&[
        1, 0, 0, 0, 0, 0, 0, 0, // layouts
        1, 0, 0, 0, 0, 0, 0, 0, // structural type
        8, 0, 0, 0, 4, 0, // byte size, alignment
        1, 0, 0, 0, 0, 0, 0, 0, // fields
        0, 0, 0, 0, 0, 0, 0, 0, // path
        0, 0, 0, 0, 0, 0, 0, 0, // repetitions
        4, 0, 0, 0, 1, 2, 1, 2, 13, 0, // offset, Boolean, fixed unsigned 13-bit scalar
        0, 0, 0, 0, 0, 0, 0, 0, // discriminators
        0, 0, 0, 0, 0, 0, 0, 0, // opaque regions
    ]);
    assert_eq!(roster.canonical_bytes(), expected);
    assert_eq!(
        roster.identity_bytes(),
        <[u8; 32]>::from(Sha256::digest(&expected))
    );
    assert_eq!(
        NativeBorrowedStorageLayouts::decode(&expected).unwrap(),
        roster
    );
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![layout()]).unwrap(),
        roster
    );
    assert!(Arc::ptr_eq(&roster.0, &roster.clone().0));
}

#[test]
fn complete_geometry_round_trips_and_binds_each_row() {
    let original = full_layout();
    let roster = NativeBorrowedStorageLayouts::seal(vec![original.clone()]).unwrap();
    let decoded = NativeBorrowedStorageLayouts::decode(roster.canonical_bytes()).unwrap();
    assert_eq!(decoded, roster);
    assert_eq!(
        decoded.layouts()[0]
            .field_path(&decoded.layouts()[0].fields()[0])
            .collect::<Vec<_>>(),
        original
            .field_path(&original.fields()[0])
            .collect::<Vec<_>>()
    );
    let mutations: [fn(&mut NativeBorrowedStorageLayout); 9] = [
        |layout| layout.byte_size += 1,
        |layout| layout.alignment += 1,
        |layout| layout.storage.span_mut(layout.fields).unwrap()[0].byte_offset += 1,
        |layout| {
            let span = layout.fields()[0].repetitions;
            layout.repetitions.span_mut(span).unwrap()[0].byte_stride += 1;
        },
        |layout| {
            let span = layout.fields()[0].path;
            layout.paths.span_mut(span).unwrap()[0].0 =
                CanonicalStructuralPathSegment::FixedIndex(7)
        },
        |layout| discriminator_mut(layout).structural_type = StructuralTypeId::new(9).unwrap(),
        |layout| discriminator_mut(layout).byte_offset += 1,
        |layout| case_mut(layout).0.as_mut().unwrap().native_tag += 1,
        |layout| {
            layout
                .opaque_storage
                .span_mut(layout.opaque_regions)
                .unwrap()[0]
                .byte_size += 1
        },
    ];
    for mutate in mutations {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert_ne!(
            NativeBorrowedStorageLayouts::seal(vec![changed]).unwrap(),
            roster
        );
    }
}

#[test]
fn unreadable_missing_and_unclosed_rows_refuse_before_identity() {
    let mut unreadable = full_layout();
    assert!(unreadable.storage.free(unreadable.fields.start()));
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![unreadable])
            .unwrap_err()
            .reason,
        "unreadable arena span"
    );
    let mut missing = full_layout();
    case_mut(&mut missing).0 = None;
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![missing])
            .unwrap_err()
            .reason,
        "missing discriminator case row"
    );
    let mut missing = full_layout();
    missing
        .discriminator_storage
        .span_mut(missing.discriminators)
        .unwrap()[0]
        .0 = None;
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![missing])
            .unwrap_err()
            .reason,
        "missing discriminator row"
    );
    let mut unclosed = layout();
    unclosed.storage.span_mut(unclosed.fields).unwrap()[0].logical_scalar =
        ScalarType::Integer(IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(2).unwrap(),
            },
        ));
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![unclosed])
            .unwrap_err()
            .reason,
        "unclosed scalar type"
    );
    assert!(NativeBorrowedStorageLayouts::seal(vec![layout(), layout()]).is_err());
}

#[test]
fn foreign_encoding_refuses_truncation_and_noncanonical_tails() {
    let roster = NativeBorrowedStorageLayouts::seal(vec![full_layout()]).unwrap();
    for end in 0..roster.canonical_bytes().len() {
        assert!(
            NativeBorrowedStorageLayouts::decode(&roster.canonical_bytes()[..end]).is_err(),
            "prefix {end}"
        );
    }
    let mut trailing = roster.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(NativeBorrowedStorageLayouts::decode(&trailing).is_err());
    let mut hostile = roster.canonical_bytes()[..33].to_vec();
    hostile.extend_from_slice(&u64::MAX.to_le_bytes());
    assert!(NativeBorrowedStorageLayouts::decode(&hostile).is_err());
}

#[test]
fn ambiguous_geometry_rosters_refuse_without_reordering_rows() {
    let mut duplicate_discriminator = full_layout();
    let row = duplicate_discriminator
        .discriminator_storage
        .span(duplicate_discriminator.discriminators)
        .unwrap()[0]
        .clone();
    duplicate_discriminator.discriminators = duplicate_discriminator
        .discriminator_storage
        .insert_many([row.clone(), row]);
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![duplicate_discriminator])
            .unwrap_err()
            .reason,
        "duplicate storage path"
    );
    let mut duplicate_tag = full_layout();
    let first = case_mut(&mut duplicate_tag).clone();
    let mut second = first.clone();
    second.0.as_mut().unwrap().case = StructuralCaseId::new(5).unwrap();
    let span = duplicate_tag.cases.insert_many([first, second]);
    discriminator_mut(&mut duplicate_tag).cases = span;
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![duplicate_tag])
            .unwrap_err()
            .reason,
        "duplicate native discriminator tag"
    );
    let mut duplicate_case = full_layout();
    let case = case_mut(&mut duplicate_case).clone();
    let span = duplicate_case.cases.insert_many([case.clone(), case]);
    discriminator_mut(&mut duplicate_case).cases = span;
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![duplicate_case])
            .unwrap_err()
            .reason,
        "duplicate discriminator case"
    );
    let mut duplicate_path = layout();
    let field = duplicate_path.fields()[0].clone();
    duplicate_path.fields = duplicate_path.storage.insert_many([field.clone(), field]);
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![duplicate_path])
            .unwrap_err()
            .reason,
        "duplicate storage path"
    );
    let mut repeated_dimension = full_layout();
    let repetition = repeated_dimension.field_repetitions(&repeated_dimension.fields()[0])[0];
    let span = repeated_dimension
        .repetitions
        .insert_many([repetition, repetition]);
    repeated_dimension
        .storage
        .span_mut(repeated_dimension.fields)
        .unwrap()[0]
        .repetitions = span;
    assert_eq!(
        NativeBorrowedStorageLayouts::seal(vec![repeated_dimension])
            .unwrap_err()
            .reason,
        "repeated or unordered array dimensions"
    );
    let mut other = layout();
    other.structural_type = StructuralTypeId::new(2).unwrap();
    let forward = NativeBorrowedStorageLayouts::seal(vec![layout(), other.clone()]).unwrap();
    let reverse = NativeBorrowedStorageLayouts::seal(vec![other, layout()]).unwrap();
    assert_ne!(forward, reverse);
    assert_eq!(
        NativeBorrowedStorageLayouts::decode(reverse.canonical_bytes()).unwrap(),
        reverse
    );
}
