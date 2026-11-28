//! The selected provider returns a guard to its caller. The caller may keep it
//! across another acquisition or release it first, possibly on a sibling word.
#[path = "support/components.rs"]
mod components;

#[path = "support/project.rs"]
mod project;

use psi_provider_selections::invocation_progress_failure;
use std::fs;
use std::path::Path;
use terminal_semantics::synchronization::{ProgressFailure, ProgressScope};

const FACTORY: &str = r#"public data Lock { held: AtomicU32; }

machine acquire(lock: @Lock) {
    let prior: u32 = lock.held.swap(1, ReceivePublish);
    transition prior == 1 {
        true -> acquire(lock),
        _ -> acquired()
    }
    machine acquired() {}
}

public data Guard<'storage> dropped by unlock { lock: @'storage Lock; }

machine unlock<'storage>(guard: Guard<'storage>) {
    let { lock } = move guard;
    lock.held.store(0, Publish);
}

public boundary trait Protect {
    machine hold_guard<'storage>(lock: @'storage Lock, home: @'storage Lock) -> Guard<'storage> reaches none crashes none;
}

public GuardedPair satisfies Protect {
    machine hold_guard<'storage>(lock: @'storage Lock, home: @'storage Lock) -> Guard<'storage> {
        acquire(lock);
        Guard { lock: home }
    }
}
"#;

const CALLER: &str = r#"use {signals}/main.omg as locks;

machine main() reaches |locks|::Protect crashes none { work(); }

machine work() reaches |locks|::Protect crashes none {
    let mutable first: |locks|::Lock;
    let mutable second: |locks|::Lock;
    let outer: |locks|::Guard = |locks|::Protect::hold_guard(@first, @first);
    let inner: |locks|::Guard = |locks|::Protect::hold_guard(@second, @second);
    let kept: |locks|::Guard = move outer;
}
"#;

#[test]
fn checked_factory_returns_the_exact_borrow_until_its_consumer_drops_it() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_wait");
    let package = fs::read_to_string(fixtures.join("supplier/build.omg")).unwrap();
    let build = fs::read_to_string(fixtures.join("application/build.omg"))
        .unwrap()
        .replace("::Wait,", "::Protect,")
        .replace("::ObservedWait>", "::GuardedPair>");
    let reacquire = CALLER.replace("(@second, @second)", "(@first, @first)");
    let released = reacquire.replace("let kept: |locks|::Guard = move outer;", "");
    let sibling = released.replacen("(@first, @first)", "(@first, @second)", 1);
    let scratch = project::stage(
        "independent-guard-factory",
        &[
            ("supplier/main.omg", FACTORY),
            ("supplier/build.omg", &package),
            ("application/main.omg", CALLER),
            ("application/build.omg", &build),
        ],
    );
    let supplier_dir = scratch.path().join("supplier");
    let (ok, report) = project::status(&project::omega(
        &supplier_dir,
        &["--emit-psi", "factory.psi", "build.omg"],
    ));
    assert!(ok, "{report}");
    let supplier = terminal_codec::CanonicalTerminalArtifact::from_bytes(
        &fs::read(supplier_dir.join("factory.psi")).unwrap(),
    )
    .unwrap();
    let [factory] = supplier
        .semantic_module()
        .interfaces
        .provider_candidates
        .as_slice()
    else {
        panic!("expected one checked factory");
    };
    let app = scratch.path().join("application");
    for (name, source, expected) in [
        ("distinct", CALLER, None),
        ("reacquire", &reacquire, Some(ProgressFailure::Deadlock)),
        ("released", &released, None),
        (
            "cleanup of sibling",
            &sibling,
            Some(ProgressFailure::Deadlock),
        ),
    ] {
        project::write(&app, &[("main.omg", source)]);
        let receiver = components::inspect(&app, "work");
        let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
            receiver.semantic_module().entry
        else {
            panic!("expected the consumer's checked work entry");
        };
        let admitted = components::admit(&receiver, &supplier, factory);
        for scope in [ProgressScope::Executable, ProgressScope::OpenInterface] {
            let failure =
                invocation_progress_failure(receiver.view(), entry, scope, [&admitted], |_| None);
            assert_eq!(failure.map(|failure| failure.cause), expected, "{name}");
        }
    }
}
