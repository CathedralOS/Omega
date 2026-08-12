use super::*;

fn typed(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn rejected(source: &str, expected: &str) {
    let diagnostics = lower_typed_trees(typed(source)).expect_err("program should be rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected diagnostic containing {expected:?}, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn transparent_record_accepts_explicit_erased_initializer() {
    lower_typed_trees(typed(
        r#"
        data Certified {
            value: i32;
            proof [erased]: i32;
        }
        data Main {}

        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7, proof: 11 };
            certified.value
        }
        "#,
    ))
    .expect("transparent erased record should check");
}

#[test]
fn erased_proof_only_containment_does_not_poison_runtime_holder() {
    lower_typed_trees(typed(
        r#"
        data Nat {
            case Zero;
            case Succ(previous: Nat);
        }
        data Certified {
            value: i32;
            proof [erased]: Nat;
        }
        data Main {}

        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7, proof: Nat::Zero };
            certified.value
        }
        "#,
    ))
    .expect("proof-only data should be legal behind an erased occurrence");
}

#[test]
fn construction_still_requires_erased_initializer() {
    rejected(
        r#"
        data Certified { value: i32; proof [erased]: i32; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7 };
            certified.value
        }
        "#,
        "omits erased field `proof`",
    );
}

#[test]
fn runtime_projection_of_erased_field_is_rejected() {
    rejected(
        r#"
        data Certified { value: i32; proof [erased]: i32; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7, proof: 11 };
            certified.proof
        }
        "#,
        "erased field `proof` has no runtime value",
    );
}

#[test]
fn case_bearing_erased_fields_fail_closed() {
    rejected(
        r#"
        data Certified { case Proven(value: i32, proof [erased]: i32); }
        "#,
        "erased-stripped runtime support for case-bearing data is not implemented",
    );
}

#[test]
fn erased_linear_field_retains_its_multiplicity_obligation() {
    rejected(
        r#"
        data Receipt [linear] { code: i32; }
        data Certified { proof [erased]: Receipt; }
        data Main {}
        machine Main::run() -> i32 {
            let receipt: Receipt = Receipt { code: 1 };
            let certified: Certified = Certified { proof: receipt };
            0
        }
        "#,
        "linear value `certified.proof` reaches scope exit",
    );
}
