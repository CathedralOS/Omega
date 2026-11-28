//! The caller's two arguments can alias even when the separately checked
//! provider declares distinct formals. Its first guard is live at the second
//! acquisition, and the selected destructor determines when the cell clears.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/project.rs"]
mod project;

use std::fs;
use std::path::Path;

#[test]
fn independent_guards_use_the_callers_resource_identity() {
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/synchronization/run");
    let read = |file: &str| fs::read_to_string(fixtures.join(file)).unwrap();
    let supplier_build = read("independent_wait/supplier/build.omg");
    let build = read("independent_wait/application/build.omg")
        .replace("::Wait,", "::Protect,")
        .replace("::ObservedWait>", "::GuardedPair>");
    let supplier = read("independent_guard_order/supplier/main.omg");
    let caller = read("independent_guard_order/application/main.omg");
    let early = supplier.replace("let retained: Guard = move outer;", "");
    let reacquire = early.replace("hold(second, second)", "hold(first, first)");
    let sibling = reacquire.replacen("hold(first, first)", "hold(first, second)", 1);
    let mark_before_acquiring = |value| {
        let body = supplier.replace(
            "    let outer: Guard",
            &format!("    mark(first, {value});\n    let outer: Guard"),
        );
        format!(
            "{body}\nmachine mark(lock: @Lock, value: u32) {{ lock.held.store(value, Publish); }}"
        )
    };
    let clear_before_acquiring = mark_before_acquiring(0);
    let held_before_acquiring = mark_before_acquiring(1);
    for (name, body, actual, stuck) in [
        ("distinct", &supplier, "@second", false),
        (
            "acquire observes clear",
            &clear_before_acquiring,
            "@second",
            false,
        ),
        (
            "acquire observes held",
            &held_before_acquiring,
            "@second",
            true,
        ),
        ("same", &supplier, "@first", true),
        ("released", &early, "@first", false),
        ("matching cleanup", &reacquire, "@second", false),
        ("other cleanup target", &sibling, "@second", true),
    ] {
        let main = caller.replace("(@first, @second)", &format!("(@first, {actual})"));
        let scratch = project::stage(
            "independent-guard-order",
            &[
                ("supplier/main.omg", body),
                ("supplier/build.omg", &supplier_build),
                ("application/main.omg", &main),
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
        assert_eq!(built, !stuck, "{name}: {report}");
        if stuck {
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
            assert!(!ran, "{diagnostic}");
            for failure in [report, diagnostic] {
                assert!(
                    failure.contains("synchronization deadlock")
                        && failure.contains("unchanged private atomic storage"),
                    "{failure}"
                );
            }
        } else {
            let components = fs::read_to_string(root.join("out/omega-components.tsv")).unwrap();
            assert!(components.contains(".so"), "{components}");
            project::assert_native_returns(&root.join("out/omega-program"), name);
        }
    }
}
