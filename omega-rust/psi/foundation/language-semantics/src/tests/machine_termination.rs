use crate::RankingViewId;

#[test]
fn canonical_view_catalog_round_trips() {
    // Fixed, deterministic ids: the catalog may enter proof-cache keys,
    // so a builtin's id and spelling must round-trip exactly.
    for id in [
        RankingViewId::NAT_DESCENDING,
        RankingViewId::NAT_BOUNDED_DISTANCE,
        RankingViewId::SLICE_LENGTH,
        RankingViewId::NAT_INCREASING_TO,
    ] {
        assert!(id.is_valid());
        let path = id.canonical_path().expect("builtin has a spelling");
        assert_eq!(RankingViewId::canonical(path), Some(id));
    }
    // Declared measures are NOT canonical builtins.
    assert_eq!(RankingViewId::canonical("Card::PowerOrder"), None);
    assert_eq!(RankingViewId::NULL.canonical_path(), None);
}
