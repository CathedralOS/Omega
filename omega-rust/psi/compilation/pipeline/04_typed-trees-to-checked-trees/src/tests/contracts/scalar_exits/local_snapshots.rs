use super::check;

#[test]
fn computed_snapshots_copy_into_named_fields_without_following_later_writes() {
    for (body, accepted) in [
        (
            "let saved: u8 = 3 + 4; let packet: Packet = Packet { value: saved }; packet.value",
            true,
        ),
        (
            "let saved: u8 = 3 + 4; let mut packet: Packet = Packet { value: saved }; packet.value = 8; packet.value",
            false,
        ),
    ] {
        check(
            &format!(
                "data Packet [copy] {{ value: u8; }} machine produce() -> u8 ensures result == 7 {{ {body} }}"
            ),
            accepted,
        );
    }
}
