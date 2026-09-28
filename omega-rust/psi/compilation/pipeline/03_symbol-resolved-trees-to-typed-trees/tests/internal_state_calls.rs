//! Internal states are transition destinations, never callables
//! (`wiki/spec/language/state_contracts.md`: "Ordinary calls invoke machines,
//! not internal states"). A body that still spells a sibling state as a call is
//! rejected by name, with the transition spelling to migrate to, instead of a
//! diagnostic about a missing return annotation on some value machine.

#[test]
fn internal_state_called_as_a_value_names_the_state() {
    let source = r#"
data SlotParse {
    found: bool;
    slot_index: u64;
}

machine parse_slot(name: &[u8], index: u64) -> SlotParse {
    transition index < name.len {
        true -> accept_at(name, index)
        false -> (SlotParse { found: false, slot_index: 0 })
    }
    state accept_at(name: &[u8], index: u64) -> SlotParse {
        accept_slot(name[index])
    }
    state accept_slot(digit: u8) -> SlotParse {
        (SlotParse { found: digit == 49, slot_index: 0 })
    }
}
"#;
    let diagnostic = crate::front_end::typed_program_result(source)
        .expect_err("a sibling state is not a callable machine");
    let message = diagnostic.to_string();
    assert!(
        message.contains("`accept_slot` is an internal state of this machine")
            && message.contains("transition { _ -> accept_slot(..) }"),
        "unexpected diagnostic: {message}"
    );
}

#[test]
fn internal_state_entered_by_transition_types() {
    let source = r#"
data SlotParse {
    found: bool;
    slot_index: u64;
}

machine parse_slot(name: &[u8], index: u64) -> SlotParse {
    transition index < name.len {
        true -> accept_at(name, index)
        false -> (SlotParse { found: false, slot_index: 0 })
    }
    state accept_at(name: &[u8], index: u64) -> SlotParse {
        transition {
            _ -> accept_slot(name[index])
        }
    }
    state accept_slot(digit: u8) -> SlotParse {
        (SlotParse { found: digit == 49, slot_index: 0 })
    }
}
"#;
    crate::front_end::typed_program_result(source).expect("a transition enters the internal state");
}
