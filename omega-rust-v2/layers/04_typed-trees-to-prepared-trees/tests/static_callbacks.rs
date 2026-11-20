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
                data ByteStep=machine(v:u8)->u8;
                machine apply<const F:ByteStep>(v:u8)->u8 { F(v) }
                machine forward<const H:ByteStep>(v:u8)->u8 { apply::<H>(v) }
                const VALUE:u8 = forward::<bump>(3u8);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(4.into()))
    );
}

#[test]
fn nested_selections_of_the_same_machine_reach_the_leaf_callback() {
    let code = "machine bump(v:u8)->u8 {v +% 1u8}
                data ByteStep=machine(v:u8)->u8;
                machine apply<const F:ByteStep>(v:u8)->u8 {F(v)}
                machine relay(v:u8)->u8 {apply::<bump>(v)}
                const RELAY:u8=apply::<relay>(7u8);
                const DOUBLE:u8=apply::<apply::<bump>>(3u8);
                const TRIPLE:u8=apply::<apply::<apply::<bump>>>(5u8);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    for (name, expected) in [("DOUBLE", 4u8), ("TRIPLE", 6u8), ("RELAY", 8u8)] {
        let constant = prepared
            .typed()
            .constants
            .iter()
            .find(|c| c.declaration.path == name)
            .unwrap();
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Integer(expected.into())));
    }

    let cycle = code.replace("{v +% 1u8}", "{apply::<bump>(v)}");
    let error = prepare_source_package(&resolve(&cycle)).unwrap_err();
    assert!(
        error.message.contains("cyclic") && error.message.contains("termination proof"),
        "{}",
        error.message
    );
}

#[test]
fn a_skipped_callback_still_requires_termination() {
    let code = "machine recur(v:u8)->u8 { recur(v) }
                machine identity(v:u8)->u8 {v}
                data ByteStep=machine(v:u8)->u8;
                machine choose<const F:ByteStep>(flag:bool,v:u8)->u8 {
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
                data Readiness=machine()->bool suspends;
                machine choose<const F:Readiness>(flag:bool)->bool suspends {
                    transition flag { true -> suspend F(), false -> false, }
                }
                const VALUE:bool = choose::<ready>(false);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration), Some(&PreparedValue::Bool(false)));
    // Twin: both selections satisfy the same suspending `Readiness` bound and
    // the skipped branch never runs. Only the selected callback's own pause
    // effect, specialized at the call, decides whether the constant can freeze.
    let outcome = |callback: &str| -> Result<PreparedValue, String> {
        let selected = code.replace("choose::<ready>", &format!("choose::<{callback}>"));
        let package = resolve(&selected);
        let prepared = prepare_source_package(&package).map_err(|error| error.message)?;
        let value = prepared
            .constant(&prepared.typed().constants[0].declaration)
            .cloned()
            .unwrap();
        Ok(value)
    };
    let outcomes = [outcome("ready"), outcome("parked")];
    spec_corpus::observe_handoff!(
        "selected-callback-effects/consumer",
        assert!(
            matches!(&outcomes, [Ok(PreparedValue::Bool(false)), Err(refusal)]
                if refusal.contains("fact or static argument requires an effect-free computation")),
            "{outcomes:?}"
        )
    );
}

#[test]
fn a_static_callback_executes_the_closed_application_of_a_generic_body() {
    let code = "machine identity<T>(v:T)->T {v}
                data ByteStep=machine(v:u8)->u8;
                machine apply<const F:ByteStep>(v:u8)->u8 { F(v) }
                const VALUE:u8 = apply::<identity::<u8>>(7u8);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(
        prepared.constant(&prepared.typed().constants[0].declaration),
        Some(&PreparedValue::Integer(7.into()))
    );
}

/// The explicit static arguments written on a callback selection reach the
/// invocation that preparation executes. Two applications of one declaration
/// differ only in those arguments, so each value identifies its own application.
#[test]
fn an_explicitly_applied_callback_executes_with_its_own_static_arguments() {
    let declarations = "machine level<const N:u8>(v:u8)->u8 {N}
                data ByteStep=machine(v:u8)->u8;
                machine apply<const F:ByteStep>(v:u8)->u8 {F(v)}";
    for expected in [2u8, 5] {
        let code = format!("{declarations}\nconst VALUE:u8=apply::<level::<{expected}>>(7u8);");
        let package = resolve(&code);
        let prepared = prepare_source_package(&package).unwrap();
        let [value] = prepared.typed().constants.as_slice() else {
            panic!("expected VALUE")
        };
        spec_corpus::observe_handoff!(
            "explicit-callback-application/consumer",
            assert_eq!(prepared.constant(&value.declaration), Some(&PreparedValue::Integer(expected.into())))
        );
    }
}

#[test]
fn a_skipped_fact_call_checks_termination_of_its_exact_callback() {
    let code = "machine identity(input:bool)->bool {input}
                data BoolStep=machine(input:bool)->bool;
                machine predicate<const F:BoolStep>(value:bool)->bool
                reaches none crashes none terminates {F(value)}
                machine guarded(value:bool)->bool requires true || predicate::<identity>(value) {value}
                machine helper(value:bool)->bool {value && guarded(value)}
                machine invoke<const K:BoolStep>(input:bool)->bool {K(input)}
                const VALUE:bool=invoke::<helper>(false);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    assert_eq!(prepared.constant(&prepared.typed().constants[0].declaration), Some(&PreparedValue::Bool(false)));

    let cyclic = code.replace("->bool {input}", "->bool {identity(input)}");
    let error = prepare_source_package(&resolve(&cyclic)).unwrap_err();
    assert!(error.message.contains("requires a checked termination proof"), "{}", error.message);
}

#[test]
fn a_nested_callback_retains_the_selection_from_each_enclosing_application() {
    let code = "machine always(input:bool)->bool {true}
                machine never(input:bool)->bool {false}
                data BoolStep=machine(input:bool)->bool;
                machine invoke<const K:BoolStep>(input:bool)->bool {K(input)}
                machine outer<const F:BoolStep>(input:bool)->bool {
                    invoke::<helper>(input)
                    machine helper(value:bool)->bool {F(value)}
                }
                const FIRST:bool=outer::<always>(false);
                const SECOND:bool=outer::<never>(true);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    for (name, result) in [("FIRST", true), ("SECOND", false)] {
        let constant = prepared
            .typed()
            .constants
            .iter()
            .find(|constant| constant.declaration.path == name)
            .unwrap();
        assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Bool(result)));
    }

    // The typed helper may accept a narrower entry contract, but a caller
    // selecting it for an unrestricted slot still needs a refinement.
    let restricted = code.replace("helper(value:bool)->bool {", "helper(value:bool)->bool requires value {");
    let error = prepare_source_package(&resolve(&restricted)).unwrap_err();
    assert!(error.message.contains("needs a checked refinement"), "{}", error.message);
}

#[test]
fn a_nested_callback_reads_the_same_static_value_as_a_direct_call() {
    for entry in ["helper(input)", "invoke::<helper>(input)"] {
        let code = format!(
            "data BoolStep=machine(input:bool)->bool;
             machine invoke<const K:BoolStep>(input:bool)->bool {{K(input)}}
             machine outer<const FLAG:bool>(input:bool)->bool {{
                 {entry}
                 machine helper(value:bool)->bool {{FLAG}}
             }}
             const FIRST:bool=outer::<true>(false);
             const SECOND:bool=outer::<false>(true);"
        );
        let prepared = prepare_source_package(&resolve(&code)).unwrap();
        for (name, result) in [("FIRST", true), ("SECOND", false)] {
            let constant = prepared
                .typed()
                .constants
                .iter()
                .find(|constant| constant.declaration.path == name)
                .unwrap();
            spec_corpus::observe_handoff!("nested-helpers-and-tail-expressions/producer", assert_eq!(prepared.constant(&constant.declaration), Some(&PreparedValue::Bool(result))));
        }
    }
}

#[test]
fn nested_callback_and_its_helpers_use_resolved_visibility_for_termination() {
    let code = "data ByteStep=machine(v:u8)->u8;
                machine apply<const F:ByteStep>(v:u8)->u8 {F(v)}
                machine outer(v:u8)->u8 {
                    apply::<step>(v)
                    public machine step(w:u8)->u8 {
                        bump(w)
                        public machine bump(n:u8)->u8 {n +% 1u8}
                    }
                }
                const VALUE:u8=outer(3u8);";
    let prepared = prepare_source_package(&resolve(code)).unwrap();
    let [value] = prepared.typed().constants.as_slice() else {
        panic!("expected VALUE")
    };
    assert_eq!(prepared.constant(&value.declaration), Some(&PreparedValue::Integer(4.into())));
}
