const LOCAL: &str = "data Step = machine(x:u8)->u8;\nmachine apply<const F:Step>(x:u8)->u8 { F(x) }\n";
const FORWARDED: &str = "data Step<T> = machine(left:T,right:T)->T;\nmachine apply<T,const F:Step<T>>(x:T,y:T)->T { F(x,y) }\nmachine wrap<T,const G:machine>(x:T,y:T)->T { let _ = apply::<T,G>(x,y); G(y,x) }\n";
const RUNTIME: &str = "data Step=machine(x:u8)->u8 reaches none crashes none;\nmachine run(cb:Step,x:u8)->u8 { cb(x) }\n";
const COMPUTED: &str = "data Step=machine(x:u8)->u8 reaches none crashes none;\ndata Ops{step:Step;}\nmachine run(ops:Ops,x:u8)->u8 { (ops.step)(x) }\n";
const NULLARY: &str = "data Ready=machine()->bool reaches none crashes none; machine f(cb:Ready)->bool{cb()}";

use omega_typer::{InvocationTarget, Site, TypedTrees, ValueType};

/// Check half-open package byte ranges against the source, not ranges obtained from L3.
fn at(
    site: &Site,
    start: usize,
    text: &str,
) {
    assert_eq!((site.package.as_str(), site.file.as_str()), ("root", "main.omg"));
    assert_eq!(site.span, omega_lexer::SourceSpan::new(start, start + text.len()));
}

enum Authority {
    Runtime(usize),
    Static(usize),
    Field,
}

fn indirect(
    typed: &TypedTrees<'_>,
    source: &str,
    caller: &str,
    callee: &str,
    args: &[&str],
    authority: Authority,
) {
    let mut calls = typed
        .indirect_calls
        .iter()
        .filter(|c| c.caller.as_ref().is_some_and(|id| id.path == caller));
    let call = calls.next().expect("missing indirect call");
    assert!(calls.next().is_none(), "expected one indirect call in {caller}");
    let owner = call.caller.as_ref().unwrap();
    assert_eq!(owner.package, "root");
    let expr = format!("{callee}({})", args.join(","));
    let start = source.rfind(&expr).unwrap();
    at(&call.site, start, &expr);
    at(&call.callee, start, callee);
    assert_eq!(call.operands.len(), args.len());
    let mut pos = start + callee.len() + 1;
    for (operand, spelling) in call.operands.iter().zip(args) {
        at(&operand.site, pos, spelling);
        pos += spelling.len() + 1;
    }
    assert_eq!(call.contract.parameters.len(), args.len());
    let rhs = source
        .split_once('=')
        .unwrap()
        .1
        .split_once(';')
        .unwrap()
        .0
        .trim();
    at(&call.contract.contract, source.find(rhs).unwrap(), rhs);
    match authority {
        Authority::Static(slot) => {
            assert!(matches!(&call.entry, InvocationTarget::StaticParameter { owner: id, index } if id == owner && *index == slot));
            assert!(
                typed
                    .callable_bounds
                    .iter()
                    .any(|(id, i, bound)| id == owner && *i == slot && bound == &call.contract)
            );
        }
        Authority::Runtime(slot) => {
            assert!(matches!(&call.entry, InvocationTarget::RuntimeParameter { owner: id, index } if id == owner && *index == slot));
            let sig = typed
                .signatures
                .iter()
                .find(|sig| &sig.machine == owner)
                .unwrap();
            assert_eq!(sig.parameters[slot].1, ValueType::Callable(Box::new(call.contract.clone())));
        }
        Authority::Field => {
            let InvocationTarget::StoredCallable(site) = &call.entry else {
                panic!("expected stored field")
            };
            at(site, start + 1, "ops.step");
        }
    }
}

fn accepted(
    typed: &TypedTrees<'_>,
    src: &str,
) {
    match src {
        LOCAL => indirect(typed, src, "apply", "F", &["x"], Authority::Static(0)),
        NULLARY => indirect(typed, src, "f", "cb", &[], Authority::Runtime(0)),
        RUNTIME => indirect(typed, src, "run", "cb", &["x"], Authority::Runtime(0)),
        COMPUTED => indirect(typed, src, "run", "(ops.step)", &["x"], Authority::Field),
        FORWARDED => {
            indirect(typed, src, "apply", "F", &["x", "y"], Authority::Static(1));
            indirect(typed, src, "wrap", "G", &["y", "x"], Authority::Static(1));
            let [relay] = typed.calls.as_slice() else {
                panic!("expected one direct relay")
            };
            let owner = relay.caller.as_ref().unwrap();
            assert_eq!((owner.path.as_str(), relay.machine.path.as_str()), ("wrap", "apply"));
            let expr = "apply::<T,G>(x,y)";
            at(&relay.site, src.find(expr).unwrap(), expr);
            let formal = |index| ValueType::Parameter {
                owner: Box::new(owner.clone()),
                index,
            };
            assert_eq!(relay.type_arguments, [formal(0), formal(1)]);
            let (_, _, bound) = typed
                .callable_bounds
                .iter()
                .find(|(id, i, _)| id == owner && *i == 1)
                .unwrap();
            assert_eq!(bound.parameters, [formal(0), formal(0)]);
            assert_eq!(bound.result, formal(0));
        }
        _ => panic!("unregistered positive source"),
    }
}

use super::diagnostic_twins::{Case, check_with_reason, resolve_root_package};
use omega_typer::IndirectCallShapeError::{self, *};

fn twin(
    src: &str,
    old: &str,
    new: &str,
    blame: &str,
    message: &str,
    cause: Option<IndirectCallShapeError>,
) {
    let good = [("main.omg", src)];
    let package = resolve_root_package(&good).unwrap();
    accepted(&omega_typer::type_source_package(&package).unwrap(), src);
    assert_eq!(src.matches(old).count(), 1);
    let bad = src.replacen(old, new, 1);
    check_with_reason(
        Case {
            layer: "L3",
            producer: "typer",
            rule: "Calls",
            file: "main.omg",
            good: &good,
            bad: &[("main.omg", &bad)],
            occurrence: blame,
            reason: message,
        },
        None,
        cause.map(Into::into),
    );
}

pub(super) fn local_short() {
    twin(LOCAL, "F(x)", "F()", "F()", "incorrect callback arity", Some(OperandCountMismatch));
}

#[test]
fn local_long() {
    twin(LOCAL, "F(x)", "F(x,x)", "F(x,x)", "incorrect callback arity", Some(OperandCountMismatch));
}

#[test]
fn forwarded_short() {
    twin(FORWARDED, "G(y,x)", "G(y)", "G(y)", "incorrect callback arity", Some(OperandCountMismatch));
}

#[test]
fn forwarded_long() {
    twin(
        FORWARDED,
        "G(y,x)",
        "G(y,x,y)",
        "G(y,x,y)",
        "incorrect callback arity",
        Some(OperandCountMismatch),
    );
}

#[test]
fn runtime_short() {
    twin(RUNTIME, "cb(x)", "cb()", "cb()", "incorrect callback arity", Some(OperandCountMismatch));
}

#[test]
fn runtime_long() {
    twin(RUNTIME, "cb(x)", "cb(x,x)", "cb(x,x)", "incorrect callback arity", Some(OperandCountMismatch));
}

#[test]
fn computed_short() {
    twin(
        COMPUTED,
        "(ops.step)(x)",
        "(ops.step)()",
        "(ops.step)()",
        "incorrect callback arity",
        Some(OperandCountMismatch),
    );
}

#[test]
fn computed_long() {
    twin(
        COMPUTED,
        "(ops.step)(x)",
        "(ops.step)(x,x)",
        "(ops.step)(x,x)",
        "incorrect callback arity",
        Some(OperandCountMismatch),
    );
}

pub(super) fn runtime_value() {
    twin(
        NULLARY,
        "cb:Ready",
        "cb:bool",
        "cb",
        "indirect call requires a callable value",
        Some(NotCallableValue),
    );
}

#[test]
fn computed_value() {
    twin(
        COMPUTED,
        "step:Step",
        "step:u8",
        "(ops.step)",
        "indirect call requires a callable value",
        Some(NotCallableValue),
    );
}

#[test]
fn static_contract() {
    twin(
        LOCAL,
        "F:Step",
        "F:u8",
        "F",
        "static binder requires a callable contract",
        Some(MissingCallableContract),
    );
}

#[test]
fn direct_short() {
    twin(
        FORWARDED,
        "apply::<T,G>(x,y)",
        "apply::<T,G>(x)",
        "apply::<T,G>",
        "no matching overload for apply(root\\wrap<parameter 0>)",
        None,
    );
}

#[test]
fn direct_long() {
    twin(
        FORWARDED,
        "apply::<T,G>(x,y)",
        "apply::<T,G>(x,y,y)",
        "apply::<T,G>",
        "no matching overload for apply(root\\wrap<parameter 0>, root\\wrap<parameter 0>, root\\wrap<parameter 0>)",
        None,
    );
}
