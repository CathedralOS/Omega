use crate::crash_member_source::{
    FOURTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path,
};

#[test]
fn fourteen_field_nested_mixed_aggregate_equality_replays_every_prefixed_path() {
    assert_nested_mixed_aggregate_equality_replays_every_prefixed_path(
        FOURTEEN_FIELD_NESTED_MIXED_AGGREGATE_EQUALITY_SOURCE,
        &[
            "supreme",
            "absolute",
            "final",
            "outermost",
            "ultimate",
            "furthest",
            "further",
            "beyond",
            "outside",
            "exterior",
            "envelope",
            "middle",
            "inner",
            "message",
        ],
    );
}
