use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::{checked_program_result, typed_program};

fn rejected(source: &str, expected: &str) {
    let diagnostics = checked_program_result(source).expect_err("program should be rejected");
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
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
    .expect("transparent erased record should check");
}

#[test]
fn erased_proof_only_containment_does_not_poison_runtime_holder() {
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
    .expect("the unique nullary constructor should supply the erased term");
}

#[test]
fn selected_case_payload_gets_unique_nullary_erased_initializer() {
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
            r#"
        data Certified { value: i32; proof [erased]: i32; }
        machine Certified::read(&self) -> i32 { self.value }
        data Main {}
        machine Main::run() -> i32 {
            let certified: Certified = Certified { value: 7, proof: 11 };
            certified.read()
        }
        "#,
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
            r#"
        data Box<T> { value: T; proof [erased]: i32; }
        machine Box::read<T>(&self) -> i32 { 0 }
        "#,
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
            r#"
        data Certified {
            proof [erased]: i32;
            case Valid(case_proof [erased]: i32);
            case Invalid;
        }
        machine Certified::read(&self) -> i32 { 0 }
        "#,
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
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
    lower_typed_trees(
        typed_program(
            r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine proof_value(value: Nat) -> i32 [0..=7] { 7 }
        machine proof_twice(value: Nat) -> i32 {
            proof_value(value) + proof_value(value)
        }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("proof-machine results remain available to proof computation");
}

#[test]
fn proof_machine_result_may_initialize_an_erased_binding() {
    lower_typed_trees(
        typed_program(
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
        ),
        &CheckingRequest::settled(),
    )
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

#[test]
fn erased_mutable_parameter_refuses_the_qualifier() {
    // `mut` promises runtime storage to write through; an erased binding owns
    // none, so the combination is refused by name rather than accepted and
    // then silently dropped from the calling plan.
    rejected(
        r#"
        data Main { value: i32; }

        machine Main::run(&mut self) {
            self.record(3, 4);
        }

        machine Main::record(&mut self, mut x [erased]: i32, y: i32) {
            self.value = y;
        }
        "#,
        "erased parameter `x` cannot be `mut`",
    );
}

#[test]
fn erased_const_parameter_refuses_the_qualifier() {
    rejected(
        r#"
        data Main { value: i32; }

        machine Main::run(&mut self) {
            self.record(3, 4);
        }

        machine Main::record(&mut self, const x [erased]: i32, y: i32) {
            self.value = y;
        }
        "#,
        "erased parameter `x` cannot be `const`",
    );
}

#[test]
fn erased_mutable_local_refuses_the_qualifier() {
    rejected(
        r#"
        data Main { value: i32; }

        machine Main::run(&mut self) {
            let mut x [erased]: i32 = 3;
            self.value = 4;
        }
        "#,
        "erased local `x` cannot be `mut`",
    );
}

#[test]
fn erased_trait_requirement_qualifiers_refuse_the_qualifier() {
    rejected(
        r#"
        trait Measure {
            machine measure(&self, mut proof [erased]: i32) -> bool;
        }

        data Item { value: bool; }

        Primary: Item satisfies Measure {
            machine measure(&self, mut proof [erased]: i32) -> bool {
                transition { _ -> self.value }
            }
        }

        data Main { item: Item; }
        "#,
        "erased parameter `proof` cannot be `mut`",
    );
}

#[test]
fn erased_record_parameter_on_runtime_machine_rides_the_term_lane() {
    // A closed checked-shape record is a contract term carrier: the erased
    // formal publishes on the proof lane and its actual lowers to a
    // construction term whose scalar fields are scalar leaves.
    lower_typed_trees(
        typed_program(
            r#"
        data Proof { tag: i32; }
        data Main { value: i32; }

        machine Main::run(&mut self) {
            self.record(Proof { tag: 1 }, 4);
        }

        machine Main::record(&mut self, w [erased]: Proof, y: i32) {
            self.value = y;
        }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("a contract-term erased record formal checks and plans");
}

#[test]
fn erased_enum_parameter_on_runtime_machine_rides_the_term_lane() {
    // A closed checked-shape enum is a contract term carrier too: the
    // selected case keeps its identity in the construction term and each
    // payload field lowers to a scalar leaf.
    lower_typed_trees(
        typed_program(
            r#"
        data Proof {
            case Tag(value: i32);
            case Empty;
        }
        data Main { value: i32; }

        machine Main::run(&mut self) {
            self.record(Proof::Tag { value: 1 }, 4);
        }

        machine Main::record(&mut self, w [erased]: Proof, y: i32) {
            self.value = y;
        }
        "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("a contract-term erased enum formal checks and plans");
}

#[test]
fn erased_generic_carrier_parameter_refuses_the_term_lane() {
    // A generic data reference is not a closed contract term carrier — only
    // the normalized instantiation could be — so shape admission names the
    // refusal on the raw generic definition.
    rejected(
        r#"
        data Box<T> { value: T; }
        data Main { value: i32; }

        machine Main::run(&mut self) {
            self.record(Box { value: 1 }, 4);
        }

        machine Main::record(&mut self, w [erased]: Box<i32>, y: i32) {
            self.value = y;
        }
        "#,
        "erased parameter `w` is not scalar",
    );
}

#[test]
fn erased_record_parameter_with_array_field_refuses_the_term_lane() {
    // A closed record is a carrier only when every field is
    // term-expressible; a fixed array has no proof-term form, so the record
    // stays inadmissible and shape admission names the refusal. This is the
    // corpus pin fail/relevance/erased_nonscalar_parameter.
    rejected(
        r#"
        data Proof { tag: i32; table: [i32; 2]; }
        data Main { value: i32; }

        machine Main::run(&mut self) {
            self.record(Proof { tag: 1, table: [7, 8] }, 4);
        }

        machine Main::record(&mut self, w [erased]: Proof, y: i32) {
            self.value = y;
        }
        "#,
        "erased parameter `w` is not scalar",
    );
}

#[test]
fn erased_scalar_parameter_stays_admitted() {
    typed_program(
        r#"
        data Main { value: i32; }

        machine Main::run(&mut self) {
            self.record(3, 4);
        }

        machine Main::record(&mut self, w [erased]: i32, y: i32) {
            self.value = y;
        }
        "#,
    );
}
