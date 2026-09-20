//! Universe inference reaches source checking and the independent mathematical kernel.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn check_files(sources: &[(&str, &str)]) -> Result<(), String> {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-mathematical-universes-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    for (name, source) in sources {
        fs::write(fixture.0.join(name), source).unwrap();
    }
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: fixture.0.join("main.omg"),
            build_dir: Some(fixture.0.join("build")),
            target_name: None,
        })
        .with_requested_product(RequestedCompileProduct::Check),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .map(|_| ())
    .map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

const IDENTITY: &str = "let identity<u: core::Level, A: core::Type<u>>(x: A): A = x;";

#[test]
fn authored_levels_infer_from_symbolic_and_closed_type_arguments() {
    let source = format!(
        "{IDENTITY}
        let explicit<v: core::Level, B: core::Type<v>>(x: B): B = identity<v,B>(x);
        let inferred<v: core::Level, B: core::Type<v>>(x: B): B = identity<B>(x);
        let closed(x: u64): u64 = identity<u64>(x);"
    );
    check_files(&[("main.omg", &source)])
        .expect("supplied type arguments determine the omitted universe");
}

#[test]
fn interleaved_independent_levels_compose_and_partially_apply() {
    let source = r#"
        let compose<u: core::Level, A: core::Type<u>,
                    v: core::Level, B: core::Type<v>,
                    w: core::Level, C: core::Type<w>>(
            f: B -> C, g: A -> B, x: A
        ): C = f(g(x));
        let use<w: core::Level, v: core::Level, u: core::Level,
                A: core::Type<u>, B: core::Type<v>, C: core::Type<w>>(
            f: B -> C, g: A -> B, x: A
        ): C = compose<A,B,C>(f,g,x);
        let partial<u: core::Level, v: core::Level, w: core::Level,
                    A: core::Type<u>, B: core::Type<v>, C: core::Type<w>>(
            f: B -> C, g: A -> B
        ): A -> C = compose<A,B,C>(f,g);
    "#;
    check_files(&[("main.omg", source)])
        .expect("independent universes and ordinary argument prefixes remain distinct");
}

#[test]
fn inferred_levels_preserve_kernel_type_and_explicit_level_checks() {
    for (body, result) in [("identity<A>(x)", "A"), ("identity<0,B>(x)", "B")] {
        let source = format!(
            "{IDENTITY}
            let invalid<v: core::Level, A: core::Type<v>, B: core::Type<v>>(x: B): {result} = {body};"
        );
        let diagnostics = check_files(&[("main.omg", &source)]).expect_err(
            "level inference cannot change an ordinary argument's type or explicit levels",
        );
        assert!(
            diagnostics.contains("fails kernel checking"),
            "{body}: {diagnostics}"
        );
    }
}

#[test]
fn shared_universe_constraints_must_all_hold() {
    let shared = "let pick<u: core::Level, A: core::Type<u>, B: core::Type<u>>(x: A, y: B): A = x;";
    let valid = format!(
        "{shared}
        let caller<v: core::Level, A: core::Type<v>, B: core::Type<v>>(x: A, y: B): A = pick<A,B>(x,y);"
    );
    check_files(&[("main.omg", &valid)]).expect("both arguments determine the same universe");
    let invalid = format!(
        "{shared}
        let caller(A: core::Type<0>, B: core::Type<1>, x: A, y: B): A = pick<A,B>(x,y);"
    );
    let diagnostics = check_files(&[("main.omg", &invalid)])
        .expect_err("a shared universe cannot silently cumulate unequal type levels");
    assert!(
        diagnostics.contains("fails kernel checking"),
        "{diagnostics}"
    );
}

#[test]
fn inferred_authored_and_generalized_levels_share_the_complete_argument_context() {
    let source = "let pick<u: core::Level, A: core::Type<u>, B>(x: A): A = x;
        let caller<v: core::Level, A: core::Type<v>>(x: A): A = pick<A,u64>(x);";
    check_files(&[("main.omg", source)])
        .expect("later supplied types are available to generalized inference");
}
