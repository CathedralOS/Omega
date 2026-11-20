use super::owned_structure::validate_claim_transfers;
use semantic_vocabulary::{ClaimId, PlaceId, StructuralCaseId};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralPathSegment as Path, TerminalIdentityIndex,
};

fn field(raw: u32) -> Path {
    Path::Field(TerminalIdentityIndex(raw))
}

fn argument(home: u64, path: Vec<Path>) -> StructuralArgument {
    StructuralArgument {
        place: PlaceId::new(home).unwrap(),
        path,
        access: StructuralAccess::Owned,
    }
}

#[test]
fn transfers_exact_current_siblings_beneath_a_projected_argument() {
    let home = PlaceId::new(7).unwrap();
    let first = [field(10), field(2)];
    let second = [field(10), field(3)];
    let unrelated = [field(11)];
    let held = [
        (ClaimId::new(2).unwrap(), home, second.as_slice()),
        (ClaimId::new(3).unwrap(), home, unrelated.as_slice()),
        (ClaimId::new(1).unwrap(), home, first.as_slice()),
    ];
    let arguments = [argument(7, vec![field(10)])];
    let rows = [
        (ClaimId::new(1).unwrap(), 0, &first[1..]),
        (ClaimId::new(2).unwrap(), 0, &second[1..]),
    ];
    validate_claim_transfers(&arguments, held, rows).unwrap();
    assert!(validate_claim_transfers(&arguments, held, rows[..1].iter().copied()).is_err());
    assert!(validate_claim_transfers(&arguments, held, [rows[1], rows[0]]).is_err());
    assert!(validate_claim_transfers(&arguments, held, [rows[0], rows[0]]).is_err());
    assert!(
        validate_claim_transfers(&arguments, held, [rows[0], (rows[1].0, 1, rows[1].2)])
            .is_err()
    );
    assert!(
        validate_claim_transfers(&arguments, held, [rows[0], (rows[1].0, 0, rows[0].2)])
            .is_err()
    );
    assert!(
        validate_claim_transfers(
            &arguments,
            held,
            [rows[0], (ClaimId::new(99).unwrap(), 0, rows[1].2)]
        )
        .is_err()
    );
    let mut borrowed = arguments.clone();
    borrowed[0].access = StructuralAccess::SharedBorrow;
    assert!(validate_claim_transfers(&borrowed, held, rows).is_err());
    let other_home = [argument(8, vec![field(10)])];
    assert!(validate_claim_transfers(&other_home, held, rows).is_err());
}

#[test]
fn overlapping_arguments_cannot_duplicate_a_claim_but_claim_free_copies_are_allowed() {
    let home = PlaceId::new(7).unwrap();
    let path = [field(10), field(2)];
    let claim = ClaimId::new(1).unwrap();
    let arguments = [argument(7, Vec::new()), argument(7, vec![field(10)])];
    assert!(
        validate_claim_transfers(
            &arguments,
            [(claim, home, path.as_slice())],
            [(claim, 0, path.as_slice())],
        )
        .is_err()
    );
    let duplicates = [argument(7, Vec::new()), argument(7, Vec::new())];
    assert!(
        validate_claim_transfers(
            &duplicates,
            [(claim, home, path.as_slice())],
            [(claim, 0, path.as_slice())],
        )
        .is_err()
    );
    validate_claim_transfers(&duplicates, [], []).unwrap();
    assert!(
        validate_claim_transfers(
            &[argument(7, Vec::new())],
            [
                (claim, home, path.as_slice()),
                (claim, home, path.as_slice())
            ],
            [(claim, 0, path.as_slice())],
        )
        .is_err()
    );
}

#[test]
fn independent_claims_and_selected_case_paths_remain_distinct() {
    let home = PlaceId::new(7).unwrap();
    let active = [Path::Case(StructuralCaseId::new(1).unwrap()), field(2)];
    let other = [Path::Case(StructuralCaseId::new(2).unwrap()), field(2)];
    let first = ClaimId::new(1).unwrap();
    let second = ClaimId::new(u64::MAX).unwrap();
    let held = [
        (first, home, active.as_slice()),
        (second, home, active.as_slice()),
    ];
    let arguments = [argument(7, Vec::new())];
    let rows = [
        (first, 0, active.as_slice()),
        (second, 0, active.as_slice()),
    ];
    validate_claim_transfers(&arguments, held, rows).unwrap();
    assert!(validate_claim_transfers(&arguments, held, [rows[0]]).is_err());
    assert!(
        validate_claim_transfers(&arguments, held, [rows[0], (second, 0, other.as_slice())])
            .is_err()
    );
}
