//! Valid L4 products that L5 cannot consume yet. Each test checks the exact L4
//! product first, then pins the L5 refusal that stops the consumer. When L5 lowers
//! one of these sources, replace the pin with a consumer comparison and register it.
#[path = "support/known_gap.rs"]
mod known_gap;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_preparation::{PreparedValue, prepare_source_package};
use omega_typer::{StaticValue, ValueType};

/// L4 CONTRACT "A local declaration after a tail expression or transition": a nested
/// helper receives its enclosing static argument when called directly and as a callback.
/// The frozen constants are what a Psi reader of `FIRST` and `SECOND` would consume.
#[test]
fn nested_helper_constants_stop_at_the_open_enclosing_template() {
    for entry in ["helper(input)", "invoke::<helper>(input)"] {
        let outer = format!(
            "machine outer<const FLAG:bool>(input:bool)->bool {{
                 {entry}
                 machine helper(value:bool)->bool {{FLAG}}
             }}"
        );
        let source = format!(
            "data BoolStep=machine(input:bool)->bool;
             machine invoke<const K:BoolStep>(input:bool)->bool {{K(input)}}
             {outer}
             const FIRST:bool=outer::<true>(false);
             const SECOND:bool=outer::<false>(true);
             machine first()->bool{{FIRST}}
             machine second()->bool{{SECOND}}"
        );
        let package = sources::resolve(&[("helper.omg", &source)]);
        let prepared = prepare_source_package(&package).unwrap_or_else(|error| panic!("{entry}: {error:?}"));
        let frozen: Vec<_> = ["FIRST", "SECOND"]
            .into_iter()
            .map(|name| {
                let constant = prepared
                    .typed()
                    .constants
                    .iter()
                    .find(|constant| constant.declaration.path == name)
                    .unwrap();
                prepared.constant(&constant.declaration).cloned()
            })
            .collect();
        // The value comes from the enclosing argument, not from the runtime operand.
        assert_eq!(
            frozen,
            [
                Some(PreparedValue::Bool(true)),
                Some(PreparedValue::Bool(false))
            ],
            "{entry}"
        );

        // L5 lowers every declaration. `outer` has no runtime application and its
        // Boolean binder is not a callable bound, so no template or instance exists.
        let error = sources::refuse_prepared(
            &prepared,
            &[("helper.omg", &source)],
            "helper.omg",
            sources::unique_span(&source, &outer),
            "open generic machine requires a closed application before lowering",
        );
        known_gap::report(
            &format!("nested helper enclosing static argument via {entry}"),
            "The runtime reads of FIRST and SECOND need the package with the open enclosing machine",
            &source,
            &error,
        );
    }
}

/// L4 CONTRACT "A specialized field request carries the declarations that own its
/// arguments, in order from the enclosing declaration to the field's immediate owner."
#[test]
fn nested_field_owner_arguments_stop_at_generic_data_lowering() {
    let source = "machine inc(n:u64)->u64{n +% 1}
                  machine same(n:u64)->u64{n}
                  data Outer<const A:u64>{data Inner<const B:u64>{
                    outer:[u8;same(inc(A))];
                    inner:[u8;same(inc(B))];
                  }}
                  machine outer(x:Outer<2>::Inner<4>)->[u8;3]{x.outer}
                  machine inner(x:Outer<2>::Inner<4>)->[u8;5]{x.inner}";
    let package = sources::resolve(&[("fields.omg", source)]);
    let prepared = prepare_source_package(&package).unwrap_or_else(|error| panic!("{error:?}"));
    let integer = |ty: &ValueType| match ty {
        ValueType::StaticInteger { value, .. } => value.to_string(),
        ValueType::StaticValue {
            value: StaticValue::Integer(value),
            ..
        } => value.to_string(),
        other => panic!("expected a closed integer owner argument, found {other:?}"),
    };
    let answers: Vec<_> = prepared
        .specialized_indices()
        .iter()
        .map(|answer| (answer.arguments.iter().map(integer).collect::<Vec<_>>(), answer.value.clone()))
        .collect();
    // Both fields are answered under Outer's argument followed by Inner's argument.
    for value in [3, 5] {
        assert!(
            answers.contains(&(vec!["2".to_owned(), "4".to_owned()], StaticValue::Integer(value.into()))),
            "{answers:?}"
        );
    }
    assert!(answers.iter().all(|(arguments, _)| arguments == &["2", "4"]), "{answers:?}");

    // Psi data must be closed, a value-index record, or a type-parameter template.
    // A data declaration with `const` integer binders is none of these.
    let error = sources::refuse_prepared(
        &prepared,
        &[("fields.omg", source)],
        "fields.omg",
        sources::unique_span(source, "<const A:u64>"),
        "generic data requires a closed application before lowering",
    );
    known_gap::report(
        "specialized field owner arguments",
        "The applied record needs its field extents from the specialized answers",
        source,
        &error,
    );
}
