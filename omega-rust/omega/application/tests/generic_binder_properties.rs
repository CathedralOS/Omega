/*

End-to-end CLI check that a generic machine's `[copy]` or `[linear]` binder
refuses a type argument without that property (`omega run --both`).

`machine keep<T [copy]>(item: T) -> T` promises its callers that `T` is
copyable, and ownership.md says every use of `T` receives the property its
binder declares. The compiler makes one machine for each application and then
checks that copy as a closed machine; that check judges what the body does
with the value, not the binder's contract, so `keep<Coin>` with an affine
`Coin` used to compile. Each application now compares the supplied type's
multiplicity with the binder's.

The `[copy]` twins differ only in the property on `Coin`: the positive runs in
the Psi interpreter and natively and exits 0; the negative is refused with the
binder it violates. A `[linear]` binder is held to the same exact match, so an
affine `Coin` is refused there too. Its positive twin is not run here: the
checker does not yet accept linear data with a cleanup machine in a signature,
an unrelated refusal.

*/

use std::process::Command;

const COPY_BINDER: &str = "data Coin [copy] { value: u8; }
machine keep<T [copy]>(item: T) -> T { item }
machine main() crashes Trap {
    let kept: Coin = keep<Coin>(Coin { value: 3 });
    transition kept.value == 3 { true -> done() false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
";

const AFFINE_TO_LINEAR_BINDER: &str = "data Coin { value: u8; }
machine keep<T [linear]>(item: T) -> T { item }
machine main() crashes Trap {
    let kept: Coin = keep<Coin>(Coin { value: 3 });
    transition kept.value == 3 { true -> done() false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
";

const BUILD: &str = "machine build(builder: &mut Build) {
    Build::application(builder, \"binder_properties\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
";

/// Runs `source` as a one-file program in both the Psi interpreter and
/// natively, returning whether both exited 0 and the combined diagnostics.
fn run_both(name: &str, source: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-generic-binder-properties-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), source).expect("staged program");
    std::fs::write(root.join("build.omg"), BUILD).expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

#[test]
fn a_copy_binder_accepts_a_copy_type_and_refuses_an_affine_one() {
    let (ran, output) = run_both("copy", COPY_BINDER);
    assert!(ran, "a `[copy]` type meets a `[copy]` binder:\n{output}");
    let affine = COPY_BINDER.replace("data Coin [copy]", "data Coin");
    let (ran, output) = run_both("copy-affine", &affine);
    assert!(!ran, "an affine type must not meet a `[copy]` binder");
    assert!(
        output.contains(
            "type argument `Coin` for `T` has neither `[copy]` nor `[linear]` but the binder \
             requires `[copy]`"
        ),
        "{output}"
    );
}

#[test]
fn a_linear_binder_refuses_an_affine_type() {
    let (ran, output) = run_both("linear-affine", AFFINE_TO_LINEAR_BINDER);
    assert!(!ran, "an affine type must not meet a `[linear]` binder");
    assert!(
        output.contains(
            "type argument `Coin` for `T` has neither `[copy]` nor `[linear]` but the binder \
             requires `[linear]`"
        ),
        "{output}"
    );
}
