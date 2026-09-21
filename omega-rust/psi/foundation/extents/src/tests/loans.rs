use super::{grant, provider_issuance, root_grant};
use crate::{ExtentSharingMode, LoanPolarity};

#[test]
fn loans_are_bounded_and_derive_parent_borrow_polarity() {
    let mut extent = grant(1, 0x1000, 64);
    let shared = extent.loan(4, 8).expect("shared loan");
    assert_eq!((shared.base(), shared.length()), (0x1004, 8));
    assert_eq!(shared.polarity(), LoanPolarity::Shared);
    assert_eq!(shared.provider_issuance(), Some(provider_issuance(1)));

    let exclusive = extent.loan_mut(16, 8).expect("exclusive loan");
    assert_eq!(exclusive.polarity(), LoanPolarity::Exclusive);

    assert!(extent.loan(60, 8).is_err());
}

#[test]
fn peer_writable_backing_never_yields_an_exclusive_borrow() {
    let mut extent = root_grant(1)
        .admitting_peer_shared_backing()
        .mint(0x1000, 64)
        .expect("peer-shared root extent");
    assert_eq!(extent.sharing(), ExtentSharingMode::PeerShared);

    let refused = extent
        .loan_mut(0, 8)
        .expect_err("writable peer has no exclusive borrow");
    assert!(refused.0.contains("writable peer"));

    // The refused borrow cannot have burned the extent's shared reads.
    let shared = extent.loan(0, 8).expect("shared reads remain");
    assert_eq!(shared.polarity(), LoanPolarity::Shared);

    // Every descendant keeps the admission; no split or partition recovers
    // exclusivity over backing a writable peer can still reach.
    let (mut lower, mut upper) = extent.split_at(32).expect("peer-shared split");
    assert!(lower.loan_mut(0, 8).is_err());
    assert!(upper.loan_mut(0, 8).is_err());
    let partition = upper.partition_owned(0, 16).expect("peer-shared partition");
    assert_eq!(
        partition.selected().sharing(),
        ExtentSharingMode::PeerShared
    );
    let (before, mut selected, after) = partition.into_parts();
    assert!(selected.loan_mut(0, 8).is_err());
    for piece in [before, after].into_iter().flatten() {
        assert_eq!(piece.sharing(), ExtentSharingMode::PeerShared);
    }
    assert!(lower.loan(0, 8).is_ok());
}
