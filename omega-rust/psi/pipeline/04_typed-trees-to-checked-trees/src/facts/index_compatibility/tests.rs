//! A restating write over a value-position call compares the instance the
//! callee's `ensures result in Family<...>` establishes, not only the family
//! symbol. The declared return type may name no instance at all (`-> i64`
//! here, `-> Extent` in the bump-allocator canary); the ensures fact is the
//! only carrier of the identity, and a restatement under another index is a
//! distinct normalized instance exactly as it is when the source is a
//! declared field.
use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool) {
    check_rejecting(
        source,
        accepted,
        &[
            "actual `Coordinate<7>` and expected `Coordinate<9>`",
            "are distinct normalized instances",
        ],
    );
}

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

#[test]
fn restating_let_compares_the_instance_the_callee_ensures_on_its_result() {
    for declared_index in [7, 9] {
        check(
            &format!(
                r#"
            domain<const Axis: u64> i64::Coordinate<Axis>;
            boundary trait Marker {{
                machine mark(value: i64) -> i64
                ensures
                    result in Coordinate<7>;
                machine relay(value: i64 in Coordinate<{declared_index}>) -> i64 in Coordinate<{declared_index}>;
            }}
            machine run(host: &Marker, value: i64) -> i64 in Coordinate<{declared_index}>
            reaches Marker
            {{
                let placed: i64 in Coordinate<{declared_index}> = host.mark(value);
                host.relay(placed)
            }}
        "#
            ),
            declared_index == 7,
        );
    }
}

/// A callee whose contract names no instance of the family establishes none.
/// `Coordinate` is bodyless, so the write has no predicate to prove either:
/// the restating `let` is refused instead of seeding the local's declared
/// instance, which is what previously let `relay`'s `requires` consume a fact
/// nothing had established.
#[test]
fn restating_let_of_an_unestablished_instance_is_refused() {
    check_rejecting(
        r#"
            domain<const Axis: u64> i64::Coordinate<Axis>;
            boundary trait Marker {
                machine mark(value: i64) -> i64;
                machine relay(value: i64 in Coordinate<7>) -> i64 in Coordinate<7>;
            }
            machine run(host: &Marker, value: i64) -> i64 in Coordinate<7>
            reaches Marker
            {
                let placed: i64 in Coordinate<7> = host.mark(value);
                host.relay(placed)
            }
        "#,
        false,
        &[
            "declared instance `Coordinate<7>` has no establishment",
            "the call establishes no instance of domain family `Coordinate`",
        ],
    );
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

/// On a carrier whose domain set is all predicate-free and route-free the
/// staged mint stays the sanctioned introduction: a `Left -> Right` retag is
/// a value tag, not custody, and the family needs no establishment route.
#[test]
fn restating_let_keeps_a_cast_mint_on_an_all_vacuous_domain_carrier() {
    check_rejecting(
        r#"
            data Region [linear] { id: u64; }
            domain Region::Left;
            domain Region::Right;
            machine retag(region: Region in Left) -> Region in Right
            {
                let moved: Region in Right = region as Region in Right;
                moved
            }
        "#,
        true,
        &[],
    );
}

/// A predicate-bearing sibling does not make a freely duplicable carrier's
/// vacuous member custody. `domains.md` admits `5 as i32::Km` with no
/// obligation beyond carrier compatibility, and whether some other part of the
/// program declares `i32::Positive` cannot decide that: duplicating a tagged
/// `i32` duplicates nothing an owner granted. The mint refusal needs the
/// carrier to be linear as well as route-managed.
#[test]
fn a_predicate_bearing_sibling_does_not_mark_an_unrestricted_carrier() {
    check_rejecting(
        r#"
            domain i32::Positive requires self >= 0;
            domain i32::Km;
            machine tag(distance: i32) -> i32 in Km
            {
                let tagged: i32 in Km = distance as i32 in Km;
                tagged
            }
        "#,
        true,
        &[],
    );
}

/// The same shape on a linear carrier keeps the refusal: `Held` is used
/// exactly once and `Checked` route-manages it, so the vacuous `Tag` names
/// custody the cast would fabricate. This is the control for the test above --
/// only the carrier's multiplicity differs.
#[test]
fn a_predicate_bearing_sibling_marks_a_linear_carrier() {
    check_rejecting(
        r#"
            data Held [linear] { id: u64; }
            domain Held::Checked requires self.id > 0;
            domain Held::Tag;
            machine tag(held: Held) -> Held in Tag
            {
                let tagged: Held in Tag = held as Held in Tag;
                tagged
            }
        "#,
        false,
        &[
            "`as` mints an instance of domain family `Tag`",
            "carrier `Held`",
        ],
    );
}
