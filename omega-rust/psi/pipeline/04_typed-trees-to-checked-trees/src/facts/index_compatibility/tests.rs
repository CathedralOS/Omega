//! A restating write over a value-position call compares the instance the
//! callee's `ensures result in Family<...>` establishes, not only the family
//! symbol. The declared return type may name no instance at all (`-> i64`
//! here, `-> Extent` in the bump-allocator canary); the ensures fact is the
//! only carrier of the identity, and a restatement under another index is a
//! distinct normalized instance exactly as it is when the source is a
//! declared field.
use crate::tests::front_end::checked_program_result;

/// Lower `source`, then require acceptance or one diagnostic carrying every
/// listed fragment.
fn check_rejecting(source: &str, accepted: bool, fragments: &[&str]) {
    match checked_program_result(source) {
        Ok(_) => assert!(accepted, "restatement accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    fragments
                        .iter()
                        .all(|fragment| diagnostic.message.contains(fragment))
                }),
                "{diagnostics:#?}\n{source}"
            );
        }
    }
}

/// The declared return type carries the instance just as an `ensures` does, so
/// an evidenced restatement keeps compiling: the refusal above must fire on a
/// missing family, never on one the callee's signature names.
#[test]
fn restating_let_accepts_the_instance_the_declared_return_type_names() {
    check_rejecting(
        r#"
            domain<const Axis: u64> i64::Coordinate<Axis>;
            boundary trait Marker {
                machine mark(value: i64) -> i64 in Coordinate<7>;
                machine relay(value: i64 in Coordinate<7>) -> i64 in Coordinate<7>;
            }
            machine run(host: &Marker, value: i64) -> i64 in Coordinate<7>
            reaches Marker
            {
                let placed: i64 in Coordinate<7> = host.mark(value);
                host.relay(placed)
            }
        "#,
        true,
        &[],
    );
}
