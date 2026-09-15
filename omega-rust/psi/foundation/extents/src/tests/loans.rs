use super::{grant, provider_issuance};
use crate::LoanPolarity;

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
