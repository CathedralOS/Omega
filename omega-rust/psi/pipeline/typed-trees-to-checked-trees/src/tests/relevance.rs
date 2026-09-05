use super::*;

fn typed(source: &str) -> typed_trees::TypedTrees {
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
fn unique_nullary_constructor_supplies_omitted_erased_initializer() {
    lower_typed_trees(typed(
        r#"
        data Nat {
            case Zero;
            case Succ(previous: Nat);
        }
        data Certified { value: i32; proof [erased]: Nat; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7 };
            certified.value
        }
        "#,
    ))
    .expect("the unique nullary constructor should supply the erased term");
}

#[test]
fn selected_case_payload_gets_unique_nullary_erased_initializer() {
    lower_typed_trees(typed(
        r#"
        data Evidence { case Only; case WithPayload(value: i32); }
        data Certified {
            case Proven(value: i32, proof [erased]: Evidence);
            case Unproven(value: i32, reason [erased]: i32);
        }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified::Proven { value: 7 };
            0
        }
        "#,
    ))
    .expect("only the selected case's omitted erased payload should elaborate");
}

#[test]
fn ambiguous_nullary_constructors_do_not_supply_an_erased_initializer() {
    rejected(
        r#"
        data Evidence { case First; case Second; }
        data Certified { value: i32; proof [erased]: Evidence; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7 };
            certified.value
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn payload_only_evidence_does_not_supply_an_erased_initializer() {
    rejected(
        r#"
        data Evidence { case WithPayload(value: i32); }
        data Certified { value: i32; proof [erased]: Evidence; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7 };
            certified.value
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn nullary_case_with_common_fields_does_not_supply_an_erased_initializer() {
    rejected(
        r#"
        data Evidence { code: i32; case Only; }
        data Certified { value: i32; proof [erased]: Evidence; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7 };
            certified.value
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn generic_evidence_does_not_supply_an_erased_initializer() {
    rejected(
        r#"
        data Evidence<T> { case Only; }
        data Certified { value: i32; proof [erased]: Evidence<i32>; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7 };
            certified.value
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn ambiguous_nullary_evidence_remains_legal_when_explicitly_supplied() {
    lower_typed_trees(typed(
        r#"
        data Evidence { case First; case Second; }
        data Certified { value: i32; proof [erased]: Evidence; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified {
                value: 7,
                proof: Evidence::Second,
            };
            certified.value
        }
        "#,
    ))
    .expect("an explicit term should resolve ambiguous nullary evidence");
}

#[test]
fn synthesized_erased_linear_evidence_retains_its_obligation() {
    rejected(
        r#"
        data Receipt [linear] { case Issued; }
        data Certified { proof [erased]: Receipt; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified {};
            0
        }
        "#,
        "linear value `certified.proof` reaches scope exit",
    );
}

#[test]
fn explicit_erased_linear_nullary_evidence_retains_its_obligation() {
    rejected(
        r#"
        data Receipt [linear] { case Issued; }
        data Certified { proof [erased]: Receipt; }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { proof: Receipt::Issued };
            0
        }
        "#,
        "linear value `certified.proof` reaches scope exit",
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
fn checked_attached_machine_accepts_erased_record_and_reads_material_self_field() {
    lower_typed_trees(typed(
        r#"
        data Certified { value: i32; proof [erased]: i32; }
        machine Certified::read(&self) -> i32 { self.value }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7, proof: 11 };
            certified.read()
        }
        "#,
    ))
    .expect("a closed checked record may use its erased-stripped attached machine");
}

#[test]
fn checked_attached_machine_rejects_runtime_erased_self_projection() {
    rejected(
        r#"
        data Certified { value: i32; proof [erased]: i32; }
        machine Certified::leak(&self) -> i32 { self.proof }
        "#,
        "erased field `proof` has no runtime value",
    );
}

#[test]
fn erased_linear_field_on_attached_record_retains_its_obligation() {
    rejected(
        r#"
        data Receipt [linear] { case Issued; }
        data Certified { value: i32; proof [erased]: Receipt; }
        machine Certified::read(&self) -> i32 { self.value }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7 };
            certified.read()
        }
        "#,
        "linear value `certified.proof` reaches scope exit",
    );
}

#[test]
fn unused_generic_erased_record_with_attached_machine_is_schema_only() {
    lower_typed_trees(typed(
        r#"
        data Box<T> { value: T; proof [erased]: i32; }
        machine Box::read<T>(&self) -> i32 { 0 }
        "#,
    ))
    .expect("an unused generic schema and method template have no runtime storage");
}

#[test]
fn unresolved_generic_erased_record_with_attached_machine_remains_fenced_at_use() {
    rejected(
        r#"
        data Box<T> { value: T; proof [erased]: i32; }
        machine Box::read<T>(&self) -> i32 { 0 }
        data Holder { box: Box<i32>; }
        "#,
        "uses unresolved erased generic data `Box`",
    );
}

#[test]
fn erased_record_with_generic_attached_machine_remains_fenced() {
    rejected(
        r#"
        data Certified { value: i32; proof [erased]: i32; }
        machine Certified::read<T>(&self, ignored: T) -> i32 { self.value }
        "#,
        "data with attached machines",
    );
}

#[test]
fn case_bearing_erased_data_with_attached_machine_is_accepted() {
    lower_typed_trees(typed(
        r#"
        data Certified {
            proof [erased]: i32;
            case Valid(case_proof [erased]: i32);
            case Invalid;
        }
        machine Certified::read(&self) -> i32 { 0 }
        "#,
    ))
    .expect("closed checked case-bearing data may use an erased-stripped attached machine");
}

#[test]
fn boundary_attached_machine_on_erased_record_remains_fenced() {
    rejected(
        r#"
        data Certified { value: i32; proof [erased]: i32; }
        boundary machine Certified::read(&self) -> i32;
        "#,
        "data with attached machines",
    );
}

#[test]
fn exact_case_payload_accepts_explicit_erased_initializer() {
    lower_typed_trees(typed(
        r#"
        data Certified {
            case First(value: i32, first_proof [erased]: i32);
            case Second(value: i32, second_proof [erased]: i32);
        }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified::First {
                value: 7,
                first_proof: 11,
            };
            0
        }
        "#,
    ))
    .expect("only the constructed case's erased payload is required");
}

#[test]
fn exact_case_payload_requires_erased_initializer() {
    rejected(
        r#"
        data Certified { case Proven(value: i32, proof [erased]: i32); }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified::Proven { value: 7 };
            0
        }
        "#,
        "omits erased field `proof`",
    );
}

#[test]
fn runtime_destructure_of_erased_payload_is_rejected() {
    rejected(
        r#"
        data Certified { case Proven(value: i32, proof [erased]: i32); }
        machine inspect(certified: Certified) -> i32 {
            transition certified {
                Certified::Proven { value as _, proof } -> proof
            }
        }
        "#,
        "erased field `proof` has no runtime value",
    );
}

#[test]
fn runtime_projection_of_erased_payload_is_rejected() {
    rejected(
        r#"
        data Certified { case Proven(value: i32, proof [erased]: i32); }
        machine inspect(certified: Certified) -> i32 {
            certified.proof
        }
        "#,
        "erased field `proof` has no runtime value",
    );
}

#[test]
fn erased_payload_may_flow_into_another_erased_payload() {
    lower_typed_trees(typed(
        r#"
        data Source { case Proven(value: i32, proof [erased]: i32); }
        data Target { case Proven(value: i32, proof [erased]: i32); }
        machine convert(source: Source) -> Target {
            transition source {
                Source::Proven { value, proof } -> Target::Proven {
                    value: value,
                    proof: proof,
                }
            }
        }
        "#,
    ))
    .expect("erased payload use inside another erased initializer should check");
}

#[test]
fn proof_machine_result_cannot_determine_runtime_data() {
    rejected(
        r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine proof_value(value: Nat) -> i32 { 7 }
        machine run() -> i32 { proof_value(Nat::Zero) }
        "#,
        "proof machine `proof_value` has no runtime result",
    );
}

#[test]
fn proof_machine_result_may_determine_proof_computation() {
    lower_typed_trees(typed(
        r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine proof_value(value: Nat) -> i32 { 7 }
        machine proof_twice(value: Nat) -> i32 {
            proof_value(value) + proof_value(value)
        }
        "#,
    ))
    .expect("proof-machine results remain available to proof computation");
}

#[test]
fn proof_machine_result_may_initialize_an_erased_binding() {
    lower_typed_trees(typed(
        r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        data Certified { value: i32; proof [erased]: i32; }
        machine proof_value(value: Nat) -> i32 { 7 }
        machine run() -> i32 {
            let certified: Certified = Certified {
                value: 11,
                proof: proof_value(Nat::Zero),
            };
            certified.value
        }
        "#,
    ))
    .expect("proof-machine results remain available to erased initializers");
}

#[test]
fn destructure_exhaustiveness_still_includes_erased_payload() {
    rejected(
        r#"
        data Certified { case Proven(value: i32, proof [erased]: i32); }
        machine inspect(certified: Certified) -> i32 {
            transition certified {
                Certified::Proven { value } -> value
            }
        }
        "#,
        "does not mention field `proof`",
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

#[test]
fn erased_linear_case_payload_retains_its_multiplicity_obligation() {
    rejected(
        r#"
        data Receipt [linear] { code: i32; }
        data Certified { case Proven(proof [erased]: Receipt); }
        data Main {}
        machine Main::run() -> i32 {
            let receipt: Receipt = Receipt { code: 1 };
            let certified: Certified = Certified::Proven { proof: receipt };
            0
        }
        "#,
        "linear value `certified",
    );
}
