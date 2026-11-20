#[path = "support/evaluation.rs"]
mod evaluation;

use omega_parser::NodeKind;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{
    MachineApplication, Site, StaticValueDemand, StaticValueRequest, TypingProgress, ValueType, resolve_source_package, type_source_package_for_preparation,
};

#[test]
fn a_pending_width_depends_on_the_called_slot_of_each_closed_tuple() {
    let source = "data Flag<const B:bool>[copy]{}
        machine enabled()->bool{true}
        machine pick(x:Flag<true>)->u64{2u64}
        machine pick(x:Flag<false>)->u64{3u64}
        data Size=machine()->u64 reaches none crashes none;
        machine two()->u64{pick(Flag::<enabled()>{})}
        machine five()->u64{5u64}
        machine relay<const H:Size>()->u64{H()}
        data Word<const F:Size,const G:Size>{x:u<F()>;}
        machine first(x:Word<relay::<two>,five>){}
        machine second(x:Word<relay::<five>,two>){}
        machine third(x:Word<relay::<five>,five>){}";
    // `two` has no checked body until enabled() decides its overload. Switching
    // only the invoked slot reverses which application can go to L4 now.
    for (slot, leaf, second_slot) in [("F()", "five", "two"), ("G()", "two", "five")] {
        let source = source.replace("x:u<F()>", &format!("x:u<{slot}>"));
        let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
        let package = resolve_source_package(&[StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: "width.omg",
                text: &source,
                tokens: &tokens,
                tree: &tree,
            },
        }])
        .unwrap();
        let TypingProgress::Pending(pass) = type_source_package_for_preparation(&package, &Default::default()).unwrap() else {
            panic!("expected pending widths");
        };
        let request = |text: &str, carrier| {
            let start = source.rfind(text).unwrap();
            StaticValueRequest {
                node: evaluation::node(&source, start, text, NodeKind::Call),
                expression: Site {
                    package: "app".into(),
                    file: "width.omg".into(),
                    span: omega_lexer::SourceSpan::new(start, start + text.len()),
                },
                carrier,
            }
        };
        let select = |name, arguments| ValueType::MachineSelection(MachineApplication {
            machine: Box::new(package.declarations.iter().find(|d| d.path == name).unwrap().clone()),
            arguments,
            enclosing_arguments: vec![],
        });
        let width = request(slot, ValueType::Integer { signed: false, width: Some(64) });
        let specialized = |leaf, second_slot| StaticValueDemand::Specialized {
            request: width.clone(),
            arguments: vec![select("relay", vec![select(leaf, vec![])]), select(second_slot, vec![])],
        };
        let expected = [
            StaticValueDemand::Closed(request("enabled()", ValueType::Bool)),
            specialized(leaf, second_slot),
            specialized("five", "five"), // same called selection, different unused argument
        ];
        // Demand order is not part of the handoff. Check multiplicity as well
        // as membership, including the unopened tuple's *absence*.
        let demands = pass.demands();
        spec_corpus::observe_handoff!(
            "closed-callback-readiness/producer",
            assert!(demands.len() == expected.len() && expected.iter().all(|d| demands.contains(d)), "{slot}: {demands:?}")
        );
    }
}
