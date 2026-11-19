use super::CheckedLibrary;
use crate::typing::type_stage::type_admitted_stage;
use omega_resolver::{CompiledInterface, Dependency, ParsedSource, StageSource, SymbolResolvedTrees};
use std::sync::OnceLock;

fn resolve<'a>(
    package: &'a str,
    text: &'a str,
    dependencies: &[Dependency<'_>],
    exports: &CompiledInterface,
) -> SymbolResolvedTrees<'a> {
    resolve_packages(&[(package, text)], dependencies, exports)
}

fn resolve_packages<'a>(
    sources: &[(&'a str, &'a str)],
    dependencies: &[Dependency<'_>],
    exports: &CompiledInterface,
) -> SymbolResolvedTrees<'a> {
    let parsed = sources
        .iter()
        .map(|&(_, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect::<Vec<_>>();
    let sources = sources
        .iter()
        .zip(&parsed)
        .map(|(&(package, text), (tokens, tree))| StageSource {
            package,
            source: ParsedSource {
                logical_path: "main.omg",
                text,
                tokens,
                tree,
            },
        })
        .collect::<Vec<_>>();
    omega_resolver::resolve_stage_with_exports(&sources, dependencies, &Default::default(), exports).unwrap()
}

fn consume(text: &str) -> Result<crate::TypedTrees<'_>, crate::TypeError> {
    static RESOLVED: OnceLock<SymbolResolvedTrees<'static>> = OnceLock::new();
    static CHECKED: OnceLock<CheckedLibrary> = OnceLock::new();
    let resolved = RESOLVED.get_or_init(|| {
        resolve(
            "library",
            "
        public boundary trait Device {}
        public trait Reader<T> { machine read(x:T)->T; }
        public trait ByteReader:Reader<u8> {}
        public domain u8::Small;
        public const limit:u8=7;
        public data Word<const N:u64> { bits:u<N-1>; }
        public machine run(f:machine() reaches Device crashes none) { f() }
        public machine apply<const F:machine()->bool suspends>()->bool suspends { suspend F() }",
            &[],
            &Default::default(),
        )
    });
    let library = CHECKED.get_or_init(|| CheckedLibrary::check(resolved, None).unwrap());
    let exports = resolved.compiled_interface();
    let client = resolve(
        "client",
        text,
        &[Dependency {
            requester: "client",
            alias: "lib",
            package: "library",
        }],
        &exports,
    );
    type_admitted_stage(&client, Some(library), &Default::default())
}

#[test]
fn imported_inherited_rows_keep_the_declaring_requirement_and_its_applied_type() {
    let source = "use lib::ByteReader; machine copy(x:u8)->u8{x} Adapter satisfies ByteReader { ByteReader::read = copy; }";
    let stage = consume(source).unwrap();
    let row = stage
        .conformance_rows
        .iter()
        .find(|row| row.implementation.path == "copy")
        .unwrap();
    assert_eq!(row.requirement.declaration.package, "library");
    assert_eq!(row.requirement.declaration.path, "Reader::read");
    assert_eq!(row.requirement.trait_arguments, [crate::ValueType::Integer { signed: false, width: Some(8) }]);
    let error = consume(&source.replace("x:u8)->u8", "x:bool)->bool")).unwrap_err();
    assert!(error.message.contains("no matching signature"), "{error:?}");
}

#[test]
fn compiled_domains_and_values_keep_their_declared_types_in_clients() {
    let stage = consume("use lib::u8::Small; machine keep(x:u8 in Small)->u8 in Small{x} machine limit()->u8{lib::limit}").unwrap();
    let keep = stage
        .signatures
        .iter()
        .find(|s| s.machine.path == "keep")
        .unwrap();
    assert_eq!(keep.parameters[0].1, keep.result);
    assert!(matches!(&keep.result, crate::ValueType::Qualified(_, domains)
        if matches!(domains.as_slice(), [small] if small.declaration.package == "library" && small.declaration.path == "u8::Small")));
    let error = consume("machine wrong()->bool{lib::limit}").unwrap_err();
    assert!(error.message.contains("expected bool, found u8"), "{error:?}");
}

#[test]
fn imported_index_constraints_apply_to_the_clients_actual_arguments() {
    let stage = consume("machine accept(w:lib::Word<2>){}").unwrap();
    assert_eq!(stage.trees().len(), 1);
    assert!(stage.operations.is_empty());
    assert!(
        stage
            .library
            .unwrap()
            .operations
            .iter()
            .any(|op| op.token == "-")
    );
    let source = "machine bad(w:lib::Word<1>){}";
    let error = consume(source).unwrap_err();
    assert_eq!(error.package, "client");
    assert_eq!(error.message, "integer width must be positive");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"lib::Word<1>".as_slice()));
}

#[test]
fn imported_body_effects_propagate_to_the_selected_caller() {
    let stage = consume("machine action() reaches lib::Device crashes none {} machine call(){lib::run(action)}").unwrap();
    let caller = stage
        .machine_effects
        .iter()
        .find(|row| row.machine.path == "call")
        .unwrap();
    assert!(matches!(caller.reach.as_slice(), [row] if row.service.package == "library" && row.service.path == "Device"));
    assert!(
        matches!(caller.invokes.as_slice(), [crate::InvocationTarget::Declaration(slot)] if slot.declaration.package == "client" && slot.declaration.path == "action")
    );
    consume("machine local()->bool{true} machine proof() requires lib::apply::<local>() {}").unwrap();
    let error = consume("machine parked()->bool suspends {true} machine proof() requires lib::apply::<parked>() {}").unwrap_err();
    assert!(error.message.contains("effect-free computation"), "{error:?}");
}

const CHECKED_PRODUCER: &str = "
        public data Point {}
        public machine + add(a:Point,b:Point)->Point{a}
        machine - subtract(a:Point,b:Point)->Point{a}
        public machine difference(a:Point,b:Point)->Point{a-b}
        public trait Add {machine + combine(x:u8,y:u8)->u8;}
        public machine guarded(x:u8)->u8 crashes Trap x == 0 {x}
        machine relay<const F:machine(x:u8)->u8>(v:u8)->u8{F(v)}
        public machine forward<const H:machine(x:u8)->u8>(v:u8)->u8{relay::<H>(v)}";

fn type_checked_client(source: &str) -> (Result<crate::TypedTrees<'_>, crate::TypeError>, Result<crate::TypedTrees<'_>, crate::TypeError>) {
    static RESOLVED: OnceLock<SymbolResolvedTrees<'static>> = OnceLock::new();
    static CHECKED: OnceLock<CheckedLibrary> = OnceLock::new();
    let resolved = RESOLVED.get_or_init(|| resolve("geometry", CHECKED_PRODUCER, &[], &Default::default()));
    let checked = CHECKED.get_or_init(|| CheckedLibrary::check(resolved, None).unwrap());
    let exports = resolved.compiled_interface();
    let dependencies = [Dependency {
        requester: "client",
        alias: "geo",
        package: "geometry",
    }];
    let client = resolve("client", source, &dependencies, &exports);
    let all = resolve_packages(&[("geometry", CHECKED_PRODUCER), ("client", source)], &dependencies, &Default::default());
    (type_admitted_stage(&client, Some(checked), &Default::default()), crate::type_stage(&all))
}

#[test]
fn checked_and_joint_typing_agree_on_operator_visibility_and_ownership() {
    let (imported, joint) = type_checked_client(
        "use geo::Point; machine sum(a:Point,b:Point)->Point{a+b}
        machine via_public(a:Point,b:Point)->Point{geo::difference(a,b)}",
    );
    let imported = imported.unwrap();
    let joint = joint.unwrap();
    let client_call = |stage: &crate::TypedTrees<'_>, path: &str| {
        let call = stage
            .calls
            .iter()
            .find(|call| call.site.package == "client" && call.machine.path == path)
            .unwrap();
        (call.machine.clone(), call.result.clone(), call.site.clone())
    };
    for path in ["add", "difference"] {
        let import = client_call(&imported, path);
        assert_eq!(import.0.package, "geometry");
        assert_eq!(import, client_call(&joint, path));
    }
    assert!(
        imported
            .library
            .unwrap()
            .calls
            .iter()
            .any(|call| call.machine.path == "subtract")
    );

    for (source, span, message) in [
        (
            "use geo::Point; machine bad(a:Point,b:Point)->Point{a-b}",
            b"a-b".as_slice(),
            "no matching operator -",
        ),
        (
            "use geo::Point; machine - foreign(a:Point,b:Point)->Point{a}",
            b"-".as_slice(),
            "only a nominal operand's declaring package may bind this operator",
        ),
    ] {
        let (imported, joint) = type_checked_client(source);
        let error = imported.unwrap_err();
        assert_eq!(error, joint.unwrap_err());
        assert_eq!(error.package, "client");
        assert_eq!(error.span.slice(source.as_bytes()), Some(span));
        assert!(error.message.starts_with(message), "{error:?}");
    }
}

#[test]
fn inherited_imported_operator_keeps_its_evidence_in_a_static_index() {
    let (imported, joint) = type_checked_client(
        "trait Bound:geo::Add {} data Buffer<const K:u8>{}
         machine buffer<const N:u8,const E:Bound>()->Buffer<N+1>{Buffer::<N+1>{}}",
    );
    for stage in [imported.unwrap(), joint.unwrap()] {
        let buffer = stage
            .signatures
            .iter()
            .find(|signature| signature.machine.path == "buffer")
            .unwrap();
        assert_eq!(buffer.machine.package, "client");
        let expected_evidence = crate::ValueType::Parameter {
            owner: Box::new(buffer.machine.clone()),
            index: 1, // E in buffer<const N:u8, const E:Bound>
        };
        let crate::ValueType::Applied(_, arguments) = &buffer.result else {
            panic!("expected Buffer<N+1>")
        };
        let [
            crate::ValueType::StaticFormula {
                term: crate::IntegerIndex::Deferred { template, .. },
                ..
            },
        ] = arguments.as_slice()
        else {
            panic!("expected a deferred index for N+1");
        };
        let request = template.request();
        let tree = stage.trees().nth(request.node.file).unwrap();
        assert_eq!(tree.resolved().file.package, "client");
        let calls: Vec<_> = tree
            .semantics_at(request.node.node)
            .filter_map(|fact| match fact {
                crate::NodeSemantics::Call(call) => Some(call),
                _ => None,
            })
            .collect();
        let [selected] = calls.as_slice() else {
            panic!("expected one selected call at N+1")
        };
        assert_eq!(selected.use_site, crate::CallUse::StaticArgument);
        assert_eq!((&*selected.machine.package, &*selected.machine.path), ("geometry", "Add::combine"));
        assert_eq!(selected.evidence.as_ref(), Some(&expected_evidence));
    }
}

#[test]
fn imported_crash_routes_keep_the_guard_and_the_forwarded_call_operands() {
    use crate::{CrashCause, CrashRouteRef, GuardedCall, GuardedInvocationRef};
    const CLIENT: &str = "
        machine local(x:u8)->u8 crashes Abort x == 1 {x}
        machine direct(x:u8)->u8{geo::guarded(x)}
        machine indirect(x:u8)->u8{geo::forward::<geo::guarded>(x)}";
    let (imported, joint) = type_checked_client(CLIENT);
    let imported = imported.unwrap();
    let joint = joint.unwrap();
    for stage in [&imported, &joint] {
        let direct = stage
            .guarded_calls
            .iter()
            .find_map(|edge| match *edge {
                GuardedCall::Direct { call, route } if stage.calls[call].machine.path == "guarded" => Some((call, route)),
                _ => None,
            })
            .expect("the direct call carries the producer's guard");
        let indirect = stage
            .guarded_calls
            .iter()
            .find_map(|edge| match *edge {
                GuardedCall::Indirect {
                    call,
                    route,
                    specialization: Some(supplies),
                } if stage.calls[supplies].machine.path == "forward" => Some((call, route, supplies)),
                _ => None,
            })
            .expect("the producer's relay carries the guard for the client-supplied callback");
        assert_eq!(direct.1, indirect.1);
        let route = stage.crash_route(direct.1).unwrap();
        assert_eq!(route.cause, CrashCause::Trap);
        let guard = route.guard.as_ref().unwrap();
        assert_eq!(guard.package, "geometry");
        assert_eq!(guard.span.slice(CHECKED_PRODUCER.as_bytes()), Some(b"x == 0".as_slice()));
        let invocation = stage.guarded_invocation(indirect.0).unwrap();
        assert_eq!(invocation.site.package, "geometry");
        assert_eq!(invocation.caller.as_ref().unwrap().path, "relay");
        assert_eq!(invocation.site.span.slice(CHECKED_PRODUCER.as_bytes()), Some(b"F(v)".as_slice()));
        assert_eq!(
            invocation.operands[0]
                .site
                .span
                .slice(CHECKED_PRODUCER.as_bytes()),
            Some(b"v".as_slice())
        );
        let supplies = &stage.calls[indirect.2];
        assert_eq!(supplies.site.package, "client");
        assert_eq!(supplies.caller.as_ref().unwrap().path, "indirect");
        assert_eq!(supplies.operands[0].site.span.slice(CLIENT.as_bytes()), Some(b"x".as_slice()));
        if stage.library.is_some() {
            assert!(matches!(direct.1, CrashRouteRef::Library(_)));
            assert!(matches!(indirect.0, GuardedInvocationRef::Library(_)));
            assert_eq!(stage.crash_routes.len(), 1);
            assert_eq!(stage.crash_routes[0].cause, CrashCause::Abort);
        } else {
            assert!(matches!(direct.1, CrashRouteRef::Local(_)));
            assert!(matches!(indirect.0, GuardedInvocationRef::Local(_)));
        }
    }
}
