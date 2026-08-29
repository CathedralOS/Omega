use super::super::{GitExecutionTransport, git_cache_identity};

#[test]
fn git_cache_identity_is_full_policy_versioned_and_injectively_framed() {
    let first = git_cache_identity("a\0b", "c", GitExecutionTransport::Https);
    let second = git_cache_identity("a", "b\0c", GitExecutionTransport::Https);

    assert_eq!(first.len(), 64);
    assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
    assert_ne!(first, second);
    assert_ne!(
        first,
        git_cache_identity("a\0b", "C", GitExecutionTransport::Https)
    );
    assert_ne!(
        first,
        git_cache_identity("a\0b", "c", GitExecutionTransport::Ssh)
    );
}
