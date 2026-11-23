use super::{TypingEnvironment, TypingProgress, ValueType, resolve, type_source_package_for_preparation};
use omega_preparation::{PreparationFailure, prepare_source_package_diagnosed};

#[test]
fn an_indexed_parent_checks_both_sides_after_evaluation() {
    for (declared, written, value, equal) in [
        ("2", "index()", 2, true),
        ("2", "index()", 3, false),
        ("index()", "2", 2, true),
        ("index()", "3", 2, false),
    ] {
        let source = format!(
            "machine index()->u64{{{value}u64}} domain<const K:u64> u8::Limit<K>;
             domain u8::Limit<{declared}>::Pair; machine read(x:u8 in u8::Limit<{written}>::Pair){{}}"
        );
        let package = resolve(&source);
        let TypingProgress::Pending(first) = type_source_package_for_preparation(&package, &TypingEnvironment::default()).unwrap() else {
            panic!("the parent argument depends on index()");
        };
        let [demand] = first.demands() else { panic!("one index is computed") };
        assert_eq!(demand.request().expression.span.slice(source.as_bytes()), Some(b"index()".as_slice()));
        assert!(!first.trees().signatures.iter().any(|s| s.machine.path == "read"));

        let result = prepare_source_package_diagnosed(&package);
        if !equal {
            let Err(PreparationFailure::ResumedTyping(error)) = result else { panic!("typing must reject unequal parent indices") };
            let start = source.rfind(&format!("Limit<{written}>")).unwrap() + "Limit<".len();
            assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "main.omg"));
            assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (start, start + written.len()));
            assert_eq!(error.message, "parent domain u8::Limit expects 2, found 3");
            continue;
        }

        let prepared = result.unwrap();
        let read = prepared.typed().signatures.iter().find(|s| s.machine.path == "read").unwrap();
        let domains = read.parameters[0].1.domains();
        assert_eq!(domains.len(), 2);
        let parent = domains.iter().find(|d| d.declaration.path == "u8::Limit").unwrap();
        assert_eq!(parent.arguments, [ValueType::StaticInteger {
            carrier: Box::new(ValueType::Integer { signed: false, width: Some(64) }),
            value: 2u8.into(),
        }]);
        assert!(domains.iter().any(|d| d.declaration.path == "u8::Limit::Pair" && d.arguments.is_empty()));
    }
}
