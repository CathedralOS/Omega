//! A write to a field of data whose `where` facts each compare one of its own
//! fields with a literal. Those facts are the fields' bounds, so the store's
//! range obligation re-proves them as it does a bracketed field's. A fact
//! relating two fields is evidence no store retains.
