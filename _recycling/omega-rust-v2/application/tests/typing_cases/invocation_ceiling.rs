use super::{Case, check};

const REASON: &str = "call exceeds the declared synchronous invocation ceiling";

#[test]
fn indirect_callback_parameter() {
    let good = "data Action=machine() reaches none crashes none;
        machine run(left:Action,right:Action) invokes right {right()}";
    let bad = good.replace("{right()}", "{left()}");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("callbacks.omg", good)],
        bad: &[("callbacks.omg", &bad)],
        file: "callbacks.omg",
        occurrence: "left()",
        reason: REASON,
        rule: "Evidence and effects",
    });
}

#[test]
fn propagated_invocation() {
    let declarations = "trait Handler<A,B>{machine enter();}
        boundary trait Dispatch<T>{machine run<U>() invokes Handler<T,U>;}";
    let good = "machine outer() invokes Handler<u8,bool>{Dispatch::<u8>::run::<bool>()}";
    let bad = good.replace("Handler<u8,bool>", "Handler<u8,u8>");
    check(Case {
        layer: "L3",
        producer: "typer",
        good: &[("schema.omg", declarations), ("caller.omg", good)],
        bad: &[("schema.omg", declarations), ("caller.omg", &bad)],
        file: "caller.omg",
        occurrence: "Dispatch::<u8>::run::<bool>()",
        reason: REASON,
        rule: "Evidence and effects",
    });
}
