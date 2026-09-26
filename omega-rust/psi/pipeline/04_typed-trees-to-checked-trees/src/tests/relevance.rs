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
