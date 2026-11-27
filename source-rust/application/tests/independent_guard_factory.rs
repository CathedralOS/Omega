//! The selected provider returns a guard to its caller. The caller may keep it
//! across another acquisition or release it first, possibly on a sibling word.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/project.rs"]
mod project;

use std::fs;
use std::path::Path;

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

machine main() reaches |locks|::Protect crashes none {
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
    for (name, source, deadlock) in [
        ("distinct", CALLER, false),
        ("reacquire", &reacquire, true),
        ("released", &released, false),
        ("cleanup of sibling", &sibling, true),
    ] {
        let scratch = project::stage(
            "independent-guard-factory",
            &[
                ("supplier/main.omg", FACTORY),
                ("supplier/build.omg", &package),
                ("application/main.omg", source),
                ("application/build.omg", &build),
            ],
        );
        let root = scratch.path().join("application");
        let (built, report) = project::status(&project::omega(
            &root,
            &[
                "--target",
                "linux_x86_64",
                "--build-dir",
                "out",
                "build.omg",
            ],
        ));
        assert_eq!(built, !deadlock, "{name}: {report}");
        if built {
            project::assert_native_returns(&root.join("out/omega-program"), name);
        }
        let (ran, diagnostic) = project::status(&project::omega(
            &root,
            &[
                "run",
                "--target",
                "linux_x86_64",
                "--psi-work-budget",
                "10000",
                "build.omg",
            ],
        ));
        assert_eq!(ran, !deadlock, "{name}: {diagnostic}");
        if deadlock {
            for failure in [report, diagnostic] {
                assert!(
                    failure.contains("synchronization deadlock")
                        && failure.contains("unchanged private atomic storage"),
                    "{name}: {failure}"
                );
            }
        }
    }
}
