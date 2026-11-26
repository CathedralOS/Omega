/*

End-to-end CLI check that a generic machine's `[copy]` or `[linear]` binder
refuses a type argument without that property in both application engines.

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


const COPY_BINDER: &str = "data Coin [copy] { value: u8; }
machine keep<T [copy]>(item: T) -> T { item }
machine main() crashes Trap {
    let kept: Coin = keep::<Coin>(Coin { value: 3 });
    transition kept.value == 3 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
";

const AFFINE_TO_LINEAR_BINDER: &str = "data Coin { value: u8; }
machine keep<T [linear]>(item: T) -> T { item }
machine main() crashes Trap {
    let kept: Coin = keep::<Coin>(Coin { value: 3 });
    transition kept.value == 3 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
";

const BUILD: &str = "machine build(builder: @mutable Build) {
    Build::application(builder, \"binder_properties\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
";

#[path = "support/observations.rs"]
mod observations;

fn project(source: &str) -> platform_custody::temporary_directory::TemporaryDirectory {
    let root =
        platform_custody::temporary_directory::TemporaryDirectory::create("generic-binder", false)
            .unwrap();
    std::fs::write(root.path().join("main.omg"), source).unwrap();
    std::fs::write(root.path().join("build.omg"), BUILD).unwrap();
    root
}

#[test]
fn a_copy_binder_accepts_a_copy_type_and_refuses_an_affine_one() {
    observations::assert_silent_return(|| project(COPY_BINDER), &["main.omg"]);
    let affine = COPY_BINDER.replace("data Coin [copy]", "data Coin");
    let (result, output) = observations::run(&project(&affine), &["--check"], &["main.omg"]);
    assert!(
        !result.status.success(),
        "an affine type must not meet a `[copy]` binder"
    );
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
    let (result, output) = observations::run(
        &project(AFFINE_TO_LINEAR_BINDER),
        &["--check"],
        &["main.omg"],
    );
    assert!(
        !result.status.success(),
        "an affine type must not meet a `[linear]` binder"
    );
    assert!(
        output.contains(
            "type argument `Coin` for `T` has neither `[copy]` nor `[linear]` but the binder \
             requires `[linear]`"
        ),
        "{output}"
    );
}
