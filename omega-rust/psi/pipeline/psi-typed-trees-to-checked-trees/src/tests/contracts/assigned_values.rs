use super::*;

#[test]
fn assignment_values_prove_nested_domain_outputs_and_preserve_disjoint_fields() {
    for body in [
        "player.health = 40; player.mana = 5;",
        "player.health = 40; player.mana = 5; player.tag = 200;",
    ] {
        let source = format!(
            r#"
            data Player {{ health: i32; mana: i32; tag: i32; }}
            domain Player::Valid requires self.health >= 0; self.health <= 100;
            domain Player::Ready requires self in Player::Valid; self.mana >= 0;
            machine establish(player: &mut Player) ensures player in Player::Ready {{ {body} }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn assignment_values_cannot_survive_overlapping_or_unknown_writes() {
    for body in [
        "player.health = 40; player.health = 200;",
        "player.health = 40; let alias: &mut i32 = &mut player.health; alias = 200;",
        "player.health = 40; corrupt(&mut player.health);",
        "player.health = unknown;",
    ] {
        let source = format!(
            r#"
            data Player {{ health: i32; tag: i32; }}
            domain Player::Valid requires self.health >= 0; self.health <= 100;
            machine corrupt(value: &mut i32) {{ value = 200; }}
            machine establish(player: &mut Player, unknown: i32)
            ensures player in Player::Valid {{ {body} }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn assignment_values_do_not_grant_text_after_an_alias_or_index_write() {
    for body in [
        "line = \"okay\"; line[0] = 255;",
        "line = \"okay\"; let alias: &mut [u8; 4] = &mut line; alias[0] = 255;",
        "line = \"okay\"; corrupt(line);",
    ] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine corrupt(line: &mut [u8; 4]) {{ line[0] = 255; }}
            machine establish(line: &mut [u8; 4]) ensures line in Utf8 {{ {body} }}
        "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source)).expect_err(body);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn ascii_byte_replacement_does_not_preserve_arbitrary_utf8() {
    for index in [0, 1] {
        let source = format!(
            r#"
            domain [u8; 2]::Utf8 requires valid_utf8(self);
            machine invalid(line: &mut [u8; 2]) ensures line in Utf8 {{
                line = "é";
                line[{index}] = 65;
            }}
        "#
        );
        assert!(
            lower_typed_trees(parse_typed_trees(&source)).is_err(),
            "ASCII replacement may split a multibyte UTF-8 scalar"
        );
    }
}
