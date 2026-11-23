use super::{Case, check};

#[test]
fn access_requires_a_policy_implementation() {
    let good = r"use omega\language\core;
use omega\language\core\reflect;
use omega\language\core\access;
Rule satisfies AccessPolicy {
    machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
        new_plan::<'t>(@schema, Issuance::Unrestricted{})
    }
}
data C{}
domain C::Locked access Rule;";
    // Both heads resolve, but the array type does not select an implementation.
    let bad = good.replace("access Rule;", "access [C; 2];");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("policy.omg", good)],
        bad: &[("policy.omg", &bad)],
        file: "policy.omg",
        occurrence: "[C; 2]",
        reason: "access requires an exact policy implementation",
        rule: "Access plans and progress",
    });
}

#[test]
fn static_callback_rejects_inferred_suspension() {
    let good = "data Step = machine(x: u8) -> u8 reaches none crashes none;
        machine apply<const F: Step>(x: u8) -> u8 { F(x) }
        machine parked() suspends {}
        machine inner(x: u8) -> u8 { x }
        machine caller() -> u8 { apply::<inner>(1) }";
    // The signature still fits. Only inference through the acknowledged call adds a pause.
    let bad = good.replace("{ x }", "{ suspend parked(); x }");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("callback.omg", good)],
        bad: &[("callback.omg", &bad)],
        file: "callback.omg",
        occurrence: "apply::<inner>(1)",
        reason: "selected machine exceeds the callable's inferred effect bounds",
        rule: "Evidence and effects",
    });
}

#[test]
fn static_callback_rejects_declared_suspension() {
    let good = "data Step = machine() -> u8 reaches none crashes none;
        machine tick() -> u8 reaches none crashes none { 1 }
        machine apply<const F: Step>() -> u8 reaches none { F() }
        machine run() -> u8 reaches none { apply::<tick>() }";
    // No change to the body: the selected machine's declared permission alone is too broad.
    let bad = good.replace("crashes none { 1 }", "crashes none suspends { 1 }");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("pause.omg", good)],
        bad: &[("pause.omg", &bad)],
        file: "pause.omg",
        occurrence: "apply::<tick>()",
        reason: "selected machine exceeds callable pause bounds",
        rule: "Evidence and effects",
    });
}

#[test]
fn a_sum_case_is_not_a_machine_callee() {
    let good = "data S { case A(a:u8); case B; }
        machine make_a(n:u8) -> S { S::A{a:n} }
        machine f() -> S { make_a(1) }";
    // The case names a payload constructor, not a machine with make_a's signature.
    let bad = good.replace("make_a(1)", "S::A(1)");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("case.omg", good)],
        bad: &[("case.omg", &bad)],
        file: "case.omg",
        occurrence: "S::A",
        reason: "unsupported indirect call or non-machine callee",
        rule: "Calls",
    });
}

#[test]
fn an_ineligible_overload_preserves_the_conflicting_type_argument() {
    let good = "machine id<T>(x:T){} machine f(){id::<u8>(2u8);} machine id(x:u8,y:u8){}";
    let bad = good.replace("::<u8>", "::<bool>");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("call.omg", good)],
        bad: &[("call.omg", &bad)],
        file: "call.omg",
        occurrence: "bool",
        reason: "conflicting type argument bool",
        rule: "Calls",
    });
}
