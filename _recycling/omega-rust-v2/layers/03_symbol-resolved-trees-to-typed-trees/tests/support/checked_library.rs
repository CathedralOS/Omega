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
        public data DeviceAction=machine() reaches Device crashes none;
        public machine run(f:DeviceAction) { f() }
        public data Readiness=machine()->bool suspends;
        public machine apply<const F:Readiness>()->bool suspends { suspend F() }",
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
    let source = "use lib\\ByteReader; machine copy(x:u8)->u8{x} Adapter satisfies ByteReader { ByteReader::read = copy; }";
    let stage = consume(source).unwrap();
    let row = stage
        .conformance_rows
        .iter()
        .find(|row| row.implementation.path == "copy")
        .unwrap();
    assert_eq!(row.requirement.declaration.package, "library");
    spec_corpus::observe_handoff!("exported-trait-ancestry/consumer", assert_eq!(row.requirement.declaration.path, "Reader::read"));
    assert_eq!(row.requirement.trait_arguments, [crate::ValueType::Integer { signed: false, width: Some(8) }]);
    let error = consume(&source.replace("x:u8)->u8", "x:bool)->bool")).unwrap_err();
    assert!(error.message.contains("no matching signature"), "{error:?}");
}

#[test]
fn compiled_domains_and_values_keep_their_declared_types_in_clients() {
    let stage = consume("use lib\\u8::Small; machine keep(x:u8 in Small)->u8 in Small{x} machine limit()->u8{lib\\limit}").unwrap();
    let keep = stage
        .signatures
        .iter()
        .find(|s| s.machine.path == "keep")
        .unwrap();
    assert_eq!(keep.parameters[0].1, keep.result);
    assert!(matches!(&keep.result, crate::ValueType::Qualified(_, domains)
        if matches!(domains.as_slice(), [small] if small.declaration.package == "library" && small.declaration.path == "u8::Small")));
    let error = consume("machine wrong()->bool{lib\\limit}").unwrap_err();
    assert!(error.message.contains("expected bool, found u8"), "{error:?}");
}

#[test]
fn imported_index_constraints_apply_to_the_clients_actual_arguments() {
    let stage = consume("machine accept(w:lib\\Word<2>){}").unwrap();
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
    let source = "machine bad(w:lib\\Word<1>){}";
    let error = consume(source).unwrap_err();
    assert_eq!(error.package, "client");
    assert_eq!(error.message, "integer width must be positive");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"lib\\Word<1>".as_slice()));
}

#[test]
fn imported_body_effects_propagate_to_the_selected_caller() {
    let stage = consume("machine action() reaches lib\\Device crashes none {} machine call(){lib\\run(action)}").unwrap();
    let caller = stage
        .machine_effects
        .iter()
        .find(|row| row.machine.path == "call")
        .unwrap();
    assert!(matches!(caller.reach.as_slice(), [row] if row.service.package == "library" && row.service.path == "Device"));
    let run = stage.signatures.iter().find(|signature| signature.machine.path == "run").unwrap();
    let crate::ValueType::Callable(callable) = &run.parameters[0].1 else { panic!("expected the library callback type") };
    let (tree, node) = stage.callable_origin(&callable.contract).expect("imported callable alias RHS");
    let file = tree.resolved().file;
    assert_eq!(file.package, "library");
    assert_eq!(file.nodes[node].kind, omega_parser::NodeKind::MachineType);
    assert_eq!(file.span(node), callable.contract.span);
    assert!(
        matches!(caller.invokes.as_slice(), [crate::InvocationTarget::Declaration(slot)] if slot.declaration.package == "client" && slot.declaration.path == "action")
    );
    consume("machine local()->bool{true} machine proof() requires lib\\apply::<local>() {}").unwrap();
    let error = consume("machine parked()->bool suspends {true} machine proof() requires lib\\apply::<parked>() {}").unwrap_err();
    assert!(error.message.contains("effect-free computation"), "{error:?}");
}

const CHECKED_PRODUCER: &str = "
        public data Point {}
        public machine + add(a:Point,b:Point)->Point{a}
        machine - subtract(a:Point,b:Point)->Point{a}
        public machine difference(a:Point,b:Point)->Point{a-b}
        public trait Add {machine + combine(x:u8,y:u8)->u8;}
        public machine guarded(x:u8)->u8 crashes Trap x == 0 {x}
        public data ByteStep=machine(x:u8)->u8;
        machine relay<const F:ByteStep>(v:u8)->u8{F(v)}
        public machine forward<const H:ByteStep>(v:u8)->u8{relay::<H>(v)}";

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
        "use geo\\Point; machine sum(a:Point,b:Point)->Point{a+b}
        machine via_public(a:Point,b:Point)->Point{geo\\difference(a,b)}",
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
            "use geo\\Point; machine bad(a:Point,b:Point)->Point{a-b}",
            b"a-b".as_slice(),
            "no matching operator -",
        ),
        (
            "use geo\\Point; machine - foreign(a:Point,b:Point)->Point{a}",
            b"-".as_slice(),
            "only a nominal operand's declaring package may bind this operator",
        ),
    ] {
        let (imported, joint) = type_checked_client(source);
        let error = imported.unwrap_err();
        assert_eq!(error, joint.unwrap_err());
        assert_eq!(error.package, "client");
        assert_eq!(error.span.slice(source.as_bytes()), Some(span));
        spec_corpus::observe_handoff!("operator-ownership/refusal", assert!(error.message.starts_with(message), "{error:?}"));
    }
}

#[test]
fn inherited_imported_operator_keeps_its_evidence_in_a_static_index() {
    let (imported, joint) = type_checked_client(
        "trait Bound:geo\\Add {} data Buffer<const K:u8>{}
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
        machine direct(x:u8)->u8{geo\\guarded(x)}
        machine indirect(x:u8)->u8{geo\\forward::<geo\\guarded>(x)}";
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
        spec_corpus::observe_handoff!("guarded-library-calls-and-forwarded-callbacks/producer", assert_eq!(guard.package, "geometry"));
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

const ACCESS_FILE: &str = "language/core/access/access.omg";
const CORE_FILE: &str = "language/core/core.omg";

/// The logical path and text of each toolchain file as `check_toolchain` typed it.
type Texts = &'static [(&'static str, &'static str)];

/// Type the bundled toolchain with `file` replaced by `edit(text)`, under the toolchain's
/// own custody: its package and `Some(core)`, exactly as `checked_library` types it.
fn check_toolchain(
    file: &str,
    edit: impl Fn(&str) -> String,
) -> Result<(&'static CheckedLibrary, Texts), crate::TypeError> {
    let parsed: &'static [(&'static str, &'static str, _, _)] = Vec::leak(
        omega_toolchain::core_interface().sources()
            .iter()
            .map(|&(path, text)| {
                let text: &'static str = if path == file { String::leak(edit(text)) } else { text };
                let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
                let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
                (path, text, tokens, tree)
            })
            .collect(),
    );
    let sources: Vec<_> = parsed
        .iter()
        .map(|(path, text, tokens, tree)| StageSource {
            package: omega_toolchain::TOOLCHAIN_PACKAGE,
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let resolved: &'static SymbolResolvedTrees<'static> = Box::leak(Box::new(omega_resolver::resolve_stage(&sources, &[]).unwrap()));
    let texts: Texts = Vec::leak(parsed.iter().map(|(path, text, _, _)| (*path, *text)).collect());
    CheckedLibrary::check(resolved, Some(omega_toolchain::core_interface())).map(|library| (&*Box::leak(Box::new(library)), texts))
}

fn unchanged(text: &str) -> String {
    text.to_owned()
}

/// Replace the body of `new_plan` by a final semicolon: the bodyless twin of the same signature.
fn bodyless(text: &str) -> String {
    let start = text.find("public machine new_plan").unwrap();
    let signature_end = text[start..].find("\n{").unwrap() + start;
    format!("{};\n", &text[..signature_end])
}

/// The authored text of each `ProveGuarantee` predicate that names `machine`, in row order.
fn guarantee_rows<'a>(
    (library, texts): (&'a CheckedLibrary, Texts),
    machine: &omega_resolver::DeclarationId,
) -> Vec<(&'a crate::TypingObligation, String)> {
    library
        .trees
        .obligations
        .iter()
        .filter_map(|o| match &o.kind {
            crate::ObligationKind::ProveGuarantee { machine: owner, predicate } if owner == machine => {
                let text = texts
                    .iter()
                    .find(|(path, _)| *path == predicate.file)
                    .unwrap()
                    .1;
                let spelled = predicate
                    .span
                    .slice(text.as_bytes())
                    .expect("the predicate site is in its file");
                Some((o, String::from_utf8(spelled.to_vec()).unwrap()))
            }
            _ => None,
        })
        .collect()
}

/// Inspect the declarations independently of L3's signature flags and obligation emission.
#[derive(Default)]
struct ImplementationAudit {
    machines: usize,
    supplied: usize,
    exempt_guarantees: usize,
    eligible: std::collections::BTreeSet<omega_resolver::DeclarationId>,
}

fn audit_library(library: &CheckedLibrary) -> ImplementationAudit {
    use omega_parser::NodeKind::{Body, Ensures, Machine};
    let core = omega_toolchain::core_interface();
    let mut audit = ImplementationAudit::default();
    let mut supplied = std::collections::BTreeSet::new();
    let collections: std::collections::BTreeSet<_> = omega_toolchain::CollectionPrimitive::ALL.into_iter().map(|id| core.collection(id)).collect();
    for tree in library.trees.trees() {
        let source = tree.resolved();
        for (node, machine) in source.declaration_nodes().filter(|(_, id)| id.kind == Machine) {
            audit.machines += 1;
            if !core.supplies(machine) {
                continue;
            }
            assert!(supplied.insert(machine), "duplicate catalog declaration {machine:?}");
            assert!(source.file.child(node, Body).is_none(), "two supplies for {machine:?}");
            if source.file.child(node, Ensures).is_some() {
                if collections.contains(machine) {
                    audit.exempt_guarantees += 1;
                } else {
                    assert!(audit.eligible.insert(machine.clone()), "duplicate eligible machine {machine:?}");
                }
            }
        }
    }

    // A partial forest cannot establish catalog-wide absence. Reconcile the exact
    // identities of both rankings, the layout queries and every collection primitive.
    let mut expected = collections;
    expected.extend(core.ranking_views().into_iter().map(|(ranking, _)| ranking));
    expected.extend([core.id("size_of"), core.id("align_of")]);
    assert_eq!(supplied, expected, "the audited declarations must cover every closed-catalog supply");
    audit.supplied = supplied.len();

    let mut emitted = std::collections::BTreeSet::new();
    for row in &library.trees.obligations {
        if let crate::ObligationKind::VerifyLibraryImplementation(machine) = &row.kind {
            let tree = library.trees.trees().nth(row.node.file).unwrap();
            assert_eq!(tree.resolved().declaration_at(row.node.node), Some(machine));
            assert!(tree.semantics_at(row.node.node).any(|fact| matches!(fact, crate::NodeSemantics::Obligation(found) if found == row)));
            assert!(emitted.insert(machine.clone()), "duplicate implementation debt for {machine:?}");
        }
    }
    assert_eq!(emitted, audit.eligible, "the library must owe every qualifying declaration exactly once");
    audit
}

#[test]
fn audit_closed_catalog_guarantees_against_their_implementation_obligations() {
    let shipped = audit_library(super::checked_library());
    let core = omega_toolchain::core_interface();
    let rank = core.id(r"Nat\Descending");

    // Preserve the authorized identity and change one clause in its own package.
    let (changed, _) = check_toolchain("language/core/Nat/nat.omg", |text| {
        let signature = "public machine Descending(value: u64) -> u64";
        assert_eq!(text.matches(signature).count(), 1);
        text.replace(signature, &format!("{signature} ensures result == value"))
    }).unwrap();
    let mut expected = shipped.eligible.clone();
    expected.insert(rank.clone());
    assert_eq!(audit_library(changed).eligible, expected);

    // An actual caller selects the altered declaration. Its implementation debt
    // belongs to the defining library even when an importer relies on the guarantee.
    let caller = resolve("client", r"use omega\language\core\Nat; machine use_rank(x: u64) -> u64 { Descending(x) }",
        &[Dependency { requester: "client", alias: "omega", package: omega_toolchain::TOOLCHAIN_PACKAGE }], core.exports());
    let client = type_admitted_stage(&caller, Some(changed), &Default::default()).unwrap();
    assert!(client.calls.iter().any(|call| &call.machine == rank));
    assert!(client.obligations.iter().all(|row| !matches!(&row.kind, crate::ObligationKind::VerifyLibraryImplementation(_))));

    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_LIBRARY_IMPLEMENTATION_AUDIT\t{}\t{}\t{}\t{}", shipped.machines, shipped.supplied, shipped.exempt_guarantees, shipped.eligible.len());
    }
}

/// The toolchain types `access\new_plan` as an ordinary checked body. Its four guarantees
/// are open `ProveGuarantee` rows of that body in the library forest. No row claims a
/// bodyless library implementation. Typing the same bytes again yields the same rows.
#[test]
fn the_toolchain_checks_new_plan_as_an_ordinary_body_and_keeps_its_guarantees_open() {
    let core = omega_toolchain::core_interface();
    let constructor = core.id("access\\new_plan");
    let (replay, texts) = check_toolchain("", unchanged).unwrap();
    let library = super::checked_library();
    assert_eq!(replay.trees.obligations, library.trees.obligations, "same input, same obligations");
    let exported: Vec<_> = replay
        .trees
        .resolved()
        .compiled_interface()
        .declarations
        .into_iter()
        .map(|e| e.id)
        .collect();
    let published: Vec<_> = core
        .exports()
        .declarations
        .iter()
        .map(|e| e.id.clone())
        .collect();
    assert_eq!(exported, published, "this file list is the toolchain's");

    let (tree, node) = library
        .trees
        .trees()
        .find_map(|tree| {
            tree.resolved()
                .declaration_nodes()
                .find(|(_, id)| *id == constructor)
                .map(|(node, _)| (tree, node))
        })
        .unwrap();
    assert!(
        tree.resolved()
            .file
            .child(node, omega_parser::NodeKind::Body)
            .is_some()
    );
    assert!(!core.supplies(constructor));
    let rows = guarantee_rows((library, texts), constructor);
    assert_eq!(
        rows.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>(),
        [
            "result.field_count == 0",
            "result.reader_count == 0",
            "result.writer_count == 0",
            "result.reach.service_count == 0"
        ]
    );
    for (row, _) in &rows {
        assert!(
            tree.semantics_at(row.node.node)
                .any(|fact| matches!(fact, crate::NodeSemantics::Obligation(found) if found == *row))
        );
    }
    assert!(!audit_library(library).eligible.contains(constructor));
}

/// The signature/bodyless twin. The same declaration without its body, in the toolchain's own
/// package and file, is the V2 baseline's `new_plan`. Without a closed-catalog entry it has
/// no supply, so L3 refuses it at the declaration.
#[test]
fn a_bodyless_toolchain_new_plan_has_no_authorized_supply() {
    const BASELINE: &str = "use language\\core;
use language\\core\\reflect;

public machine new_plan<'t>(schema: @TypeSchema<'t>, issuance: Issuance<'t>) -> AccessPlan<'t>
    reaches none crashes none terminates
    ensures result.field_count == 0
    ensures result.reader_count == 0
    ensures result.writer_count == 0
    ensures result.reach.service_count == 0;
";
    let access = omega_toolchain::core_interface().sources()
        .iter()
        .find(|(path, _)| *path == ACCESS_FILE)
        .unwrap()
        .1;
    assert_eq!(bodyless(access), BASELINE);
    let Err(error) = check_toolchain(ACCESS_FILE, bodyless) else {
        panic!("a bodyless new_plan must not type")
    };
    let at = BASELINE.find("public machine new_plan").unwrap();
    assert_eq!(
        (
            error.package.as_str(),
            error.file.as_str(),
            error.span.get_start_offset()..error.span.get_end_offset(),
            error.message.as_str()
        ),
        (
            omega_toolchain::TOOLCHAIN_PACKAGE,
            ACCESS_FILE,
            at..BASELINE.len() - 1,
            "bodyless machine has no authorized supply"
        )
    );
}

#[test]
fn an_imported_call_cannot_be_rebound_to_another_definition_with_the_same_signature() {
    let core = omega_toolchain::core_interface();
    let id = core.id("access\\new_plan");
    // Same guarantees, different implementation. The changed digit preserves every
    // source coordinate and the two constructors have identical typed signatures.
    let with_count = |text: &str, count: u8| {
        let fields = "schema: $schema, issuance: issuance, ..";
        assert!(text.contains(fields));
        text.replace(fields, &format!("schema: $schema, issuance: issuance, field_count: {count}u64, .."))
    };
    let (original, _) = check_toolchain(ACCESS_FILE, |text| with_count(text, 0)).unwrap();
    let (changed, _) = check_toolchain(ACCESS_FILE, |text| with_count(text, 1)).unwrap();
    let caller = r"use omega\language\core; use omega\language\core\reflect; use omega\language\core\access;
        machine make<'t>(schema: @TypeSchema<'t>, issuance: Issuance<'t>) -> AccessPlan<'t> {
            new_plan::<'t>(schema, issuance)
        }";
    let resolved = resolve(
        "client",
        caller,
        &[Dependency {
            requester: "client",
            alias: "omega",
            package: omega_toolchain::TOOLCHAIN_PACKAGE,
        }],
        core.exports(),
    );
    let client = |library| type_admitted_stage(&resolved, Some(library), &Default::default()).unwrap();
    let typed = client(original);
    let [call] = typed.calls.as_slice() else { panic!("one constructor call") };
    assert_eq!(&call.machine, id);
    let (_, signature) = typed.imported_machine_definition(&call.machine).unwrap();
    let changed_client = client(changed);
    let (_, other) = changed_client.imported_machine_definition(id).unwrap();
    assert_eq!(signature, other, "the body change is invisible in the signature");
    assert!(
        typed.imported_machine_definition(core.id("size_of")).is_none(),
        "a catalog entry has no ordinary body"
    );

    let mut facts = typed.into_facts();
    facts.library = Some(&changed.trees);
    let rebound = crate::TypedTrees::new(resolved, facts);
    assert!(
        rebound
            .imported_machine_definition(&rebound.calls[0].machine)
            .is_none()
    );
}

/// Guarantees are claims the body owes, not supply. An altered `ensures` on the checked body
/// still types and becomes that body's open row; the same alteration without a body is refused.
#[test]
fn altered_guarantees_become_the_bodys_own_open_rows() {
    let alter = |text: &str| text.replace("ensures result.field_count == 0", "ensures result.field_count == 1");
    let (library, texts) = check_toolchain(ACCESS_FILE, alter).unwrap();
    let constructor = omega_toolchain::core_interface().id("access\\new_plan");
    let claims: Vec<_> = guarantee_rows((library, texts), constructor)
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    assert_eq!(
        claims,
        [
            "result.field_count == 1",
            "result.reader_count == 0",
            "result.writer_count == 0",
            "result.reach.service_count == 0"
        ]
    );
    assert!(!audit_library(library).eligible.contains(constructor));
    let Err(error) = check_toolchain(ACCESS_FILE, |text| bodyless(&alter(text))) else {
        panic!("an altered bodyless new_plan must not type")
    };
    assert_eq!(
        (error.file.as_str(), error.message.as_str()),
        (ACCESS_FILE, "bodyless machine has no authorized supply")
    );
}

/// A closed-catalog identity is supplied by its catalog realization. Giving it a checked body
/// as well would give one declaration two supplies, so L3 refuses it.
#[test]
fn a_closed_catalog_machine_cannot_also_declare_a_checked_body() {
    let declaration = "public machine size_of<T>() -> u64\n    reaches none crashes none terminates;";
    let core = omega_toolchain::core_interface().sources()
        .iter()
        .find(|(path, _)| *path == CORE_FILE)
        .unwrap()
        .1;
    assert!(core.contains(declaration));
    let with_body = |text: &str| {
        text.replace(
            declaration,
            "public machine size_of<T>() -> u64\n    reaches none crashes none terminates { 0 }",
        )
    };
    let Err(error) = check_toolchain(CORE_FILE, with_body) else {
        panic!("a catalog identity with a body must not type")
    };
    let edited = with_body(core);
    let at = edited.find("public machine size_of").unwrap();
    assert_eq!(
        (error.file.as_str(), error.span.get_start_offset(), error.message.as_str()),
        (CORE_FILE, at, "closed-catalog machine cannot also declare a checked body")
    );
    // This harness checks edited declarations against the shipped catalog. Changing
    // the parameter list yields a different identity in that catalog.
    let Err(error) = check_toolchain(CORE_FILE, |text| {
        text.replace(declaration, &declaration.replace("size_of<T>()", "size_of<T>(x: u8)"))
    }) else {
        panic!("an altered primitive signature must not acquire supply")
    };
    assert_eq!(error.message, "bodyless machine has no authorized supply");
    assert!(check_toolchain(CORE_FILE, unchanged).is_ok());
}
