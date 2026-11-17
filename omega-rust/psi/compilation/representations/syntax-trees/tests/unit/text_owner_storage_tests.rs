use crate::text_owner::*;
use compiler_storage::{RecordLimits, decode, encode};

#[test]
fn restored_forests_keep_shared_pages_without_colliding_with_live_names() {
    let mut first = SyntaxTextOwner::default();
    let first_name = first.retain(Arc::from("first"));
    let mut second = SyntaxTextOwner::default();
    let second_name = second.retain(Arc::from("second"));
    let mut both = first.clone();
    both.retain_from(&second);
    let bytes = encode(
        &(second, both, first_name, second_name),
        RecordLimits::default(),
    )
    .unwrap();
    let mut live = SyntaxTextOwner::default();
    let live_name = live.retain(Arc::from("live"));
    let (second, both, restored_first, restored_second): (
        SyntaxTextOwner,
        SyntaxTextOwner,
        Handle<RetainedSource>,
        Handle<RetainedSource>,
    ) = decode(&bytes, RecordLimits::default()).unwrap();
    assert_ne!(first_name, restored_first);
    assert_ne!(second_name, restored_second);
    assert_eq!(both.source_text(restored_first), "first");
    assert_eq!(both.source_text(restored_second), "second");
    assert_eq!(second.source_text(restored_second), "second");
    live.retain_from(&both);
    assert_eq!(live.source_text(live_name), "live");
    assert_eq!(live.source_text(restored_first), "first");
    assert_eq!(live.source_text(restored_second), "second");
}

#[test]
fn duplicate_page_identity_cannot_change_its_retained_text() {
    let mut first = SyntaxTextOwner::default();
    let handle = first.retain(Arc::from("original"));
    let conflicting = SyntaxTextOwner {
        sources: vec![RetainedSource {
            generation: handle.generation(),
            text: Arc::from("substituted"),
        }],
    };
    let bytes = encode(&(first, conflicting), RecordLimits::default()).unwrap();
    assert!(decode::<(SyntaxTextOwner, SyntaxTextOwner)>(&bytes, RecordLimits::default()).is_err());
}
