use omega_preparation::{PreparedValue, prepare_source_package};
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{SourcePackage, resolve_source_package};

fn resolve(code: &str) -> SourcePackage<'_> {
    let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
    resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "app.omg",
            text: code,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap()
}

#[test]
fn generic_forwarders_use_the_exact_selected_static_machine() {
    let code = "machine bump(v:u8)->u8 { v +% 1 }
                machine apply<const F:machine(v:u8)->u8>(v:u8)->u8 { F(v) }
                machine forward<const H:machine(v:u8)->u8>(v:u8)->u8 { apply::<H>(v) }
                const VALUE:u8 = forward::<bump>(3u8);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(4.into()))
    );
}

#[test]
fn a_skipped_callback_still_requires_termination() {
    let code = "machine recur(v:u8)->u8 { recur(v) }
                machine identity(v:u8)->u8 {v}
                machine choose<const F:machine(v:u8)->u8>(flag:bool,v:u8)->u8 {
                    match flag { true -> F(v), false -> v, }
                }
                const VALUE:u8 = choose::<identity>(false,3u8);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(3.into()))
    );
    let with_cycle = code.replace("choose::<identity>", "choose::<recur>");
    let error = prepare_source_package(&resolve(&with_cycle)).unwrap_err();
    assert!(error.message.contains("requires a checked termination proof"), "{}", error.message);
}

#[test]
fn a_skipped_callback_still_requires_non_suspending_selected_effects() {
    let code = "machine ready()->bool {true}
                machine parked()->bool suspends {true}
                machine choose<const F:machine()->bool suspends>(flag:bool)->bool suspends {
                    transition flag { true -> suspend F(), false -> false, }
                }
                const VALUE:bool = choose::<ready>(false);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration), Some(&PreparedValue::Bool(false)));
    let with_suspension = code.replace("choose::<ready>", "choose::<parked>");
    let error = prepare_source_package(&resolve(&with_suspension)).unwrap_err();
    assert!(
        error
            .message
            .contains("fact or static argument requires an effect-free computation"),
        "{}",
        error.message
    );
}

#[test]
fn a_static_callback_executes_the_closed_application_of_a_generic_body() {
    let code = "machine identity<T>(v:T)->T {v}
                machine apply<const F:machine(v:u8)->u8>(v:u8)->u8 { F(v) }
                const VALUE:u8 = apply::<identity::<u8>>(7u8);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(7.into()))
    );
}
