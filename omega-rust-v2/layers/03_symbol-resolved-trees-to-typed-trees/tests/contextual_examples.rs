//! Add the declarations omitted by a published fragment, and check its actual source operation.
//! A block with several deliberate errors runs each bad statement separately.
mod support;

use omega_typer::{ObligationKind, ValueType};
use support::check;

const DOMAINS: &str = include_str!("../../../wiki/spec/language/domains.md");
const EXPRESSIONS: &str = include_str!("../../../wiki/spec/language/expressions.md");
const NUMBERS: &str = include_str!("../../../wiki/spec/language/numeric_values.md");

fn block(
    chapter: &str,
    line: usize,
) -> String {
    let mut examples = Vec::new();
    spec_corpus::extract_examples_from_markdown("chapter", chapter, &mut examples);
    examples
        .into_iter()
        .find(|e| e.fence_line_number == line)
        .unwrap_or_else(|| panic!("missing Omega block at line {line}"))
        .source_text
}

#[test]
fn published_cleanup_clauses_select_owned_hooks() {
    for (line, declarations, owner, clause) in [
        (
            435,
            "data Guard {} machine release_guard(g:Guard) {}",
            "Guard::Held",
            "dropped by release_guard",
        ),
        (443, "machine release_guard<T>(g:Guard<T>) {}", "Guard", "dropped by release_guard<T>"),
    ] {
        let example = block(DOMAINS, line);
        let files = [("declarations.omg", declarations), ("published.omg", &example)];
        let stage = check(&files).unwrap();
        let [selection] = stage.cleanups.as_slice() else {
            panic!("expected the published cleanup selection")
        };
        assert_eq!(selection.owner.path, owner);
        assert_eq!(selection.machine.path, "release_guard");
        assert_eq!(selection.site.file, "published.omg");
        assert_eq!(selection.site.span.slice(example.as_bytes()), Some(clause.as_bytes()));
        let borrowed = declarations.replace("g:Guard", "g:@Guard");
        let error = check(&[("declarations.omg", &borrowed), files[1]]).unwrap_err();
        assert_eq!(error.file, "published.omg");
        assert_eq!(error.span.slice(example.as_bytes()), Some(clause.as_bytes()));
        assert!(error.message.contains("must consume exactly one owned carrier"), "{error:?}");
    }
}

#[test]
fn cleanup_selection_checks_bounds_even_when_the_parameter_is_absent_from_the_hook_signature() {
    let src = "data R dropped by dispose<u8> {} machine dispose<T [copy]>(r:R) {}";
    let stage = check(&[("cleanup.omg", src)]).unwrap();
    assert_eq!(stage.cleanups[0].arguments[0].to_string(), "u8");
    let wrong = src.replace("dispose<u8>", "dispose<R>");
    let error = check(&[("cleanup.omg", &wrong)]).unwrap_err();
    assert!(error.message.contains("does not satisfy Copy"), "{error:?}");
}

#[test]
fn published_domain_casts_require_complete_accounting() {
    // The spec declares the parent and the unit elsewhere in the chapter. Keep the
    // original good declarations in scope for each independently failing cast.
    let declarations = "domain i32::Km; domain i32::Positive requires self>0;";
    // The child predicate receives an i32 in Positive, so its comparison uses
    // the operator declared for that parent domain.
    let comparison = "machine < Positive::less(left:i32 in Positive,right:i32)->bool {(left as -Positive)<right}";
    for (line, expected) in [
        (
            623,
            [
                ("both as =Km", "cast omits domain i32::Positive"),
                ("+Km", "invalid + entry for domain i32::Km"),
            ],
        ),
        (
            666,
            [
                ("small as -Positive =Positive::Small", "cast omits ancestor i32::Positive"),
                ("7 as +Positive::Small", "cast omits ancestor i32::Positive"),
            ],
        ),
    ] {
        let example = block(DOMAINS, line);
        let (before_let, statements) = example.split_at(example.find("let ").unwrap());
        let (good, bad) = statements.split_once("// Rejected:").unwrap();
        let wrap = |extra: &str| format!("{declarations}\n{comparison}\n{before_let}machine example() {{\n{good}\n{extra}\n}}");
        let source = wrap("");
        let stage = check(&[("published.omg", &source)]).unwrap();
        let added: Vec<_> = stage
            .obligations
            .iter()
            .filter_map(|o| match &o.kind {
                ObligationKind::EstablishDomain(d) => Some(d.declaration.path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            added,
            if line == 623 {
                ["i32::Km", "i32::Positive"]
            } else {
                ["i32::Positive", "i32::Positive::Small"]
            }
        );
        if line == 666 {
            let call = stage
                .calls
                .iter()
                .find(|call| call.site.span.slice(source.as_bytes()) == Some(b"self < 100"))
                .unwrap();
            assert_eq!(call.machine.path, "Positive::less");
            let reversed = source.replace("7 as +Positive +Positive::Small", "7 as +Positive::Small +Positive");
            check(&[("published.omg", &reversed)]).unwrap();
        }
        let invalid: Vec<_> = bad.lines().filter(|line| line.starts_with("let ")).collect();
        assert_eq!(invalid.len(), expected.len());
        for (statement, (site, message)) in invalid.into_iter().zip(expected) {
            let source = wrap(statement);
            let error = check(&[("published.omg", &source)]).unwrap_err();
            assert_eq!(error.message, message, "{statement}");
            assert_eq!(error.file, "published.omg");
            assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()), "{statement}");
        }
    }
}

#[test]
fn published_unit_operands_and_operator_homes_survive_typing() {
    let declarations = "domain i32::Km; domain i32::Wrapping; domain u16::Small;";
    let literal = block(DOMAINS, 736);
    let comparison = block(DOMAINS, 759);
    let addition = block(DOMAINS, 897);
    let greater = block(DOMAINS, 910);
    let caller = format!(
        "machine establish() {{ {literal} }} machine sum(left:i32 in Km & Wrapping,right:i32 in Km & Wrapping)->i32 in Km & Wrapping {{left+right}} machine test(x:u16 in Small,y:u16)->bool {{x>y}}"
    );
    let stage = check(&[
        ("defs.omg", declarations),
        ("compare.omg", &comparison),
        ("addition.omg", &addition),
        ("greater.omg", &greater),
        ("caller.omg", &caller),
    ])
    .unwrap();
    assert!(stage.obligations.iter().any(|o| o.site.file == "caller.omg"
        && o.site.span.slice(caller.as_bytes()) == Some(b"5 as +Km")
        && matches!(&o.kind,ObligationKind::EstablishDomain(d) if d.declaration.path=="i32::Km")));
    let op = stage
        .operations
        .iter()
        .find(|op| op.site.file == "compare.omg" && op.token == ">")
        .unwrap();
    assert_eq!(op.result, ValueType::Bool);
    assert_eq!(op.operands.iter().map(ToString::to_string).collect::<Vec<_>>(), ["i32", "i32"]);
    for (site, name) in [("left+right", "Km::add_wrapping"), ("x>y", "Small::greater")] {
        let call = stage
            .calls
            .iter()
            .find(|c| c.site.file == "caller.omg" && c.site.span.slice(caller.as_bytes()) == Some(site.as_bytes()))
            .unwrap();
        assert_eq!(call.machine.path, name);
    }
}

#[test]
fn published_range_views_preserve_bounds_and_authority() {
    let example = block(EXPRESSIONS, 505);
    let lines: Vec<_> = example.lines().collect();
    let [shared, exclusive, commented] = lines.as_slice() else {
        panic!("expected the three published alternatives")
    };
    for (line, expected, result) in [
        (*shared, "indexing::range", "@[u8]"),
        (*exclusive, "indexing::range_mut", "@mut [u8]"),
    ] {
        let source = format!("machine read(view:@mut [u8]) {{ {line} }}");
        let resolved = support::resolve_packages(&[("game", "published.omg", &source)], &[]);
        let stage = omega_typer::type_stage(&resolved).unwrap();
        let tree = stage.trees().next().unwrap();
        let syntax = tree.resolved().file;
        let [access] = stage.collection_accesses.as_slice() else {
            panic!("expected one selected range")
        };
        assert_eq!(access.machine.path, expected);
        assert_eq!(access.result.to_string(), result);
        assert!(
            tree.semantics_at(access.node.node)
                .any(|fact| matches!(fact, omega_typer::NodeSemantics::CollectionAccess(a) if a == access))
        );
        let [_, selector] = *syntax.nodes[access.node.node].children.as_slice() else {
            panic!("expected receiver and selector")
        };
        let Some(omega_parser::RangeBounds {
            start: Some(start),
            end: Some(end),
            inclusive: false,
        }) = syntax.range_bounds(selector)
        else {
            panic!("expected a half-open range")
        };
        for endpoint in [start, end] {
            assert!(
                tree.semantics_at(endpoint)
                    .any(|fact| matches!(fact, omega_typer::NodeSemantics::Landing(e) if e.ty.to_string() == "u64"))
            );
        }
        assert_eq!(syntax.span(start).slice(source.as_bytes()), Some(b"2".as_slice()));
        assert_eq!(syntax.span(end).slice(source.as_bytes()), Some(b"4".as_slice()));
    }
    let bad = commented.strip_prefix("// Rejected: ").unwrap();
    let source = format!("machine read(view:@mut [u8]) {{ {bad} }}");
    let error = check(&[("published.omg", &source)]).unwrap_err();
    assert!(error.message.starts_with("expected @mut [u8], found @[u8]"), "{error:?}");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"view[2..4]".as_slice()));
}

#[test]
fn published_policy_arithmetic_keeps_each_operator_and_carrier() {
    let example = block(NUMBERS, 643);
    let source = format!("machine compute(cursor:u64,width:u64,hash:u32,level:u8,step:u8) crashes Trap {{\n{example}\n}}");
    let stage = check(&[("published.omg", &source)]).unwrap();
    let contracts: Vec<_> = stage
        .obligations
        .iter()
        .filter_map(|o| match &o.kind {
            ObligationKind::OperationContract { token, operands } => Some((
                o.site.span.slice(source.as_bytes()).unwrap(),
                token.as_str(),
                operands.iter().map(ToString::to_string).collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .collect();
    for ((span, token, operands), (expr, expected, ty)) in contracts.iter().zip([
        ("cursor +! width", "+!", "u64"),
        ("hash *% 31", "*%", "u32"),
        ("level +| step", "+|", "u8"),
    ]) {
        assert_eq!(*span, expr.as_bytes());
        assert_eq!(*token, expected);
        assert_eq!(*operands, [ty, ty]);
    }
    assert_eq!(contracts.len(), 3);
}

#[test]
fn published_scoped_constants_keep_qualified_identity_and_carrier() {
    let example = block(include_str!("../../../wiki/spec/language/constants.md"), 6);
    let stage = check(&[
        ("status.omg", "data EfiStatus {code:u32;}"),
        ("constants.omg", &example),
    ])
    .unwrap();
    let mut constants: Vec<_> = stage
        .constants
        .iter()
        .map(|constant| (constant.declaration.path.as_str(), constant.initializer.carrier.to_string()))
        .collect();
    constants.sort();
    assert_eq!(
        constants,
        [
            ("EfiStatus::SUCCESS", "game::EfiStatus".into()),
            ("PAGE_SIZE", "u64".into())
        ]
    );
}

#[test]
fn published_qualified_carrier_changes_check_every_rejected_form() {
    let example = block(DOMAINS, 701);
    let (decl, rest) = example.split_once("\n\n").unwrap();
    let (valid, invalid) = rest.split_once("// Rejected:").unwrap();
    let source = format!("{decl}\nmachine example() {{\n{valid}\n}}");
    let stage = check(&[("published.omg", &source)]).unwrap();
    let far = stage
        .operations
        .iter()
        .find(|op| op.site.span.slice(source.as_bytes()) == Some(b"near as -Metres as i64 +Metres"))
        .unwrap();
    assert!(
        matches!(&far.result,ValueType::Qualified(base,domains) if base.to_string()=="i64" && domains.len()==1 && domains[0].declaration.path=="T::Metres")
    );
    let bad_forms: Vec<_> = invalid.lines().filter(|l| l.starts_with("let ")).collect();
    assert_eq!(bad_forms.len(), 3);
    for statement in bad_forms {
        let source = format!("{decl}\nmachine example() {{\n{valid}\n{statement}\n}}");
        let error = check(&[("published.omg", &source)]).unwrap_err();
        assert_eq!(error.message, "carrier cast requires an unqualified operand");
        assert_eq!(error.span.slice(source.as_bytes()), Some(b"near".as_slice()));
        assert_eq!(error.span.get_start_offset(), source.rfind("near").unwrap());
    }
}

#[test]
fn published_numeric_landings_do_not_mask_each_others_verdicts() {
    let example = block(NUMBERS, 498);
    let lines: Vec<_> = example.lines().collect();
    let [implicit, exact, truncated, fractional, overflow] = lines.as_slice() else {
        panic!("expected five published landing cases")
    };
    let wrap = |line: &str| format!("machine example() {{\n{line}\n}}");
    let source = wrap(exact);
    let stage = check(&[("published.omg", &source)]).unwrap();
    let cast = stage
        .operations
        .iter()
        .find(|op| op.site.span.slice(source.as_bytes()) == Some(b"((5 / 2) * 2) as u32"))
        .unwrap();
    assert_eq!(cast.token, "as");
    assert_eq!(cast.operands.len(), 1);
    assert_eq!(cast.operands[0].to_string(), "5");
    assert_eq!(cast.result.to_string(), "u32");
    let source = wrap(truncated);
    let stage = check(&[("published.omg", &source)]).unwrap();
    let ops: Vec<_> = stage
        .operations
        .iter()
        .map(|op| (op.token.as_str(), op.result.to_string()))
        .collect();
    assert_eq!(ops, [("/", "u32".into()), ("*", "u32".into())]);
    for (line, site, reason) in [
        (implicit, "(5 / 2) * 2", "fractional intermediate"),
        (fractional, "(5 / 2)", "found 5/2"),
        (overflow, "(256 / 2 * 2)", "found 256"),
    ] {
        let source = wrap(line);
        let error = check(&[("published.omg", &source)]).unwrap_err();
        assert!(error.message.contains(reason), "{error:?}");
        assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()));
    }
}

#[test]
fn published_operator_in_its_own_module_keeps_the_domain_home() {
    let example = block(EXPRESSIONS, 343);
    let (module, root) = example.split_once("// The same package's root file:").unwrap();
    let (_, root) = root.split_once('\n').unwrap();
    let caller = "use units; machine sum(x:u64 in Distance,y:u64 in Distance)->u64 in Distance {x+y}";
    let stage = check(&[
        ("units/domain.omg", module),
        ("operator.omg", root),
        ("caller.omg", caller),
    ])
    .unwrap();
    let [call] = stage.calls.as_slice() else {
        panic!("only the caller selects an authored operator")
    };
    assert_eq!(call.site.file, "caller.omg");
    assert_eq!(call.machine.path, "Distance::add");
    assert!(matches!(&call.result, ValueType::Qualified(_,domains) if domains.len()==1 && domains[0].declaration.path=="units::u64::Distance"));
}

#[test]
fn published_static_bound_is_inferred_from_the_operand_domain() {
    let generics = include_str!("../../../wiki/spec/language/generics.md");
    let definitions = block(generics, 363);
    let bound = block(generics, 406);
    for capacity in [4, 8] {
        let caller = format!("machine bound(value:u64::AtMost<{capacity}>)->u64 {{upper_bound(value)}}");
        let stage = check(&[
            ("definitions.omg", &definitions),
            ("function.omg", &bound),
            ("call.omg", &caller),
        ])
        .unwrap();
        let [call] = stage.calls.as_slice() else {
            panic!("expected one selected call")
        };
        assert_eq!(call.machine.path, "upper_bound");
        assert_eq!(call.type_arguments.len(), 1);
        assert_eq!(call.type_arguments[0].to_string(), capacity.to_string());
    }
}

#[test]
fn published_domain_predicate_reads_the_carriers_field() {
    let domain = block(DOMAINS, 35);
    let stage = check(&[
        ("carrier.omg", "data Player {health:i32;}"),
        ("predicate.omg", &domain),
    ])
    .unwrap();
    let [comparison] = stage.operations.as_slice() else {
        panic!("expected a single comparison in the domain predicate")
    };
    assert_eq!(comparison.site.file, "predicate.omg");
    assert_eq!(comparison.site.span.slice(domain.as_bytes()), Some(b"self.health > 0".as_slice()));
    assert_eq!(comparison.token, ">");
    assert!(comparison.operands.iter().all(|ty| ty.to_string() == "i32"));
    assert_eq!(comparison.result, ValueType::Bool);
    let error = check(&[
        ("carrier.omg", "data Player {health:bool;}"),
        ("predicate.omg", &domain),
    ])
    .unwrap_err();
    assert_eq!(error.file, "predicate.omg");
    assert_eq!(error.span, comparison.site.span);
    assert!(error.message.contains("no matching operator >"), "{error:?}");
}

#[test]
fn published_domain_alias_normalizes_to_the_two_socket_domains() {
    let example = block(DOMAINS, 995);
    let atoms = "public data Socket {} public domain Socket::Connected; public domain Socket::Authenticated;";
    let caller = "machine use_socket(s:Socket in Usable) {} machine forward(s:Socket in Connected & Authenticated) {use_socket(s)}";
    let files = [
        ("atoms.omg", atoms),
        ("alias.omg", example.as_str()),
        ("caller.omg", caller),
    ];
    let stage = check(&files).unwrap();
    let signature = stage
        .signatures
        .iter()
        .find(|s| s.machine.path == "use_socket")
        .unwrap();
    let domains: Vec<_> = signature.parameters[0]
        .1
        .domains()
        .iter()
        .map(|d| d.declaration.path.as_str())
        .collect();
    assert_eq!(domains, ["Socket::Authenticated", "Socket::Connected"]);
    let incomplete = caller.replace("Connected & Authenticated", "Connected");
    let error = check(&[files[0], files[1], ("caller.omg", &incomplete)]).unwrap_err();
    assert_eq!(error.file, "caller.omg");
    assert_eq!(error.span.slice(incomplete.as_bytes()), Some(b"use_socket".as_slice()));
    assert!(error.message.contains("no matching overload"), "{error:?}");
}

#[test]
fn published_record_pattern_binds_the_version_only_in_its_arm() {
    let pattern = block(include_str!("../../../wiki/spec/language/patterns.md"), 26);
    let definitions = "data Header {ok:u8;version:u16;} machine accept(version:u16)->bool {true} machine reject()->bool {false}";
    let body = format!("machine inspect(header:Header)->bool {{\n{pattern}\n}}");
    let stage = check(&[("definitions.omg", definitions), ("pattern.omg", &body)]).unwrap();
    let calls: Vec<_> = stage
        .calls
        .iter()
        .map(|c| (c.machine.path.as_str(), c.site.span.slice(body.as_bytes()).unwrap()))
        .collect();
    assert_eq!(
        calls,
        [
            ("accept", b"accept(version)".as_slice()),
            ("reject", b"reject()".as_slice())
        ]
    );
    let wrong = definitions.replace("accept(version:u16)", "accept(version:bool)");
    let error = check(&[("definitions.omg", &wrong), ("pattern.omg", &body)]).unwrap_err();
    assert_eq!(error.file, "pattern.omg");
    assert_eq!(error.span.slice(body.as_bytes()), Some(b"accept".as_slice()));
}

#[test]
fn published_quantity_keeps_its_carrier_and_unit_index() {
    let declaration = block(DOMAINS, 1020);
    for (carrier, spelling) in [
        ("u16", "u16 in Quantity<1>"),
        ("u32", "u32::Quantity<1>"),
        ("Word", "Word::Quantity<1>"),
        ("Box<u8>", "Box<u8>::Quantity<1>"),
    ] {
        let defs = "data Unit = u8; data Word = u16; data Box<X>{value:X;}";
        let caller = format!("machine keep(x:{spelling})->{carrier} in Quantity<1> {{x}}");
        let stage = check(&[
            ("unit.omg", defs),
            ("quantity.omg", &declaration),
            ("caller.omg", &caller),
        ])
        .unwrap();
        let signature = &stage.signatures[0];
        assert_eq!(signature.parameters[0].1, signature.result);
        let [atom] = signature.result.domains() else {
            panic!("expected one Quantity index")
        };
        assert_eq!(atom.declaration.path, "T::Quantity");
        assert_eq!(atom.arguments[0].to_string(), "1");
        let wrong = caller.replace(" in Quantity<1> {x}", " in Quantity<2> {x}");
        let error = check(&[
            ("unit.omg", defs),
            ("quantity.omg", &declaration),
            ("caller.omg", &wrong),
        ])
        .unwrap_err();
        assert_eq!(error.file, "caller.omg");
        assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"x".as_slice()));
        assert!(error.message.contains("Quantity<2>") && error.message.contains("Quantity<1>"), "{error:?}");
    }
}

const EFFECTS: &str = include_str!("../../../wiki/spec/language/effects.md");
const GENERICS: &str = include_str!("../../../wiki/spec/language/generics.md");

#[test]
fn published_inherited_call_contributes_its_declared_reach() {
    let text = block(EFFECTS, 51);
    let defs = "data NoState {} data Entry {at:u64;} boundary trait TimeObservation {} boundary trait ClockOps<S>{machine now(s:@mut S)->u64 reaches TimeObservation;}";
    let rejected = text.replace("    entry.at =", "    let mut none:NoState;\n    entry.at =");
    let error = check(&[("defs.omg", defs), ("clock.omg", &rejected)]).unwrap_err();
    assert_eq!(error.message, "call exceeds service reach for TimeObservation");
    assert_eq!(error.file, "clock.omg");
    assert_eq!(error.span.slice(rejected.as_bytes()), Some(b"WallClock::now(@mut none)".as_slice()));
    let admitted = rejected.replace(") reaches WallClock", ") reaches TimeObservation");
    let stage = check(&[("defs.omg", defs), ("clock.omg", &admitted)]).unwrap();
    let [call] = stage.calls.as_slice() else { panic!("expected inherited now") };
    assert_eq!(call.machine.path, "ClockOps::now");
    assert!(
        call.enclosing_arguments
            .iter()
            .any(|(owner, args)| owner.path == "ClockOps" && args[0].to_string() == "game::NoState")
    );
}

#[test]
fn published_counter_fields_retain_their_callable_contracts() {
    let fields = block(EFFECTS, 201);
    let caller = "machine invoke(o:CounterOps,a:@mut Counter,b:@Counter)->u64 {(o.clear)(a);(o.value)(b)}";
    let defs = "data Counter {}";
    let stage = check(&[
        ("counter.omg", defs),
        ("fields.omg", &fields),
        ("invoke.omg", caller),
    ])
    .unwrap();
    let contracts: Vec<_> = stage.indirect_calls.iter().map(|c| &c.contract).collect();
    assert_eq!(contracts.len(), 2);
    for (c, authority) in contracts
        .iter()
        .zip([omega_typer::Access::Mut, omega_typer::Access::Shared])
    {
        assert!(matches!(c.parameters.as_slice(),[ValueType::Reference(a,t)] if a==&authority && t.to_string()=="game::Counter"));
        assert!(c.closed_runtime && c.terminates && c.reach.as_ref().is_some_and(Vec::is_empty) && c.crash_ceiling.as_ref().is_some_and(Vec::is_empty));
    }
    let wrong = caller.replace("a:@mut Counter", "a:@Counter");
    let err = check(&[
        ("counter.omg", defs),
        ("fields.omg", &fields),
        ("invoke.omg", &wrong),
    ])
    .unwrap_err();
    assert_eq!(err.message, "expected @mut game::Counter, found @game::Counter");
}

#[test]
fn published_pause_markers_cover_each_required_effect() {
    let defs = "machine ordinary_call() {} machine may_park() suspends {} machine may_wait() blocks {} machine may_do_either() suspends blocks {}";
    let body = format!("machine caller() {{ {} }}", block(EFFECTS, 871));
    let stage = check(&[("defs.omg", defs), ("calls.omg", &body)]).unwrap();
    assert_eq!(
        stage
            .calls
            .iter()
            .map(|c| (c.machine.path.as_str(), c.acknowledgements.suspend, c.acknowledgements.block))
            .collect::<Vec<_>>(),
        [
            ("ordinary_call", false, false),
            ("may_park", true, false),
            ("may_wait", false, true),
            ("may_do_either", true, true)
        ]
    );
    for (proper, missing, effect) in [
        ("suspend may_park", "may_park", "suspend"),
        ("block may_wait", "may_wait", "block"),
        ("suspend block may_do_either", "block may_do_either", "suspend"),
        ("suspend block may_do_either", "suspend may_do_either", "block"),
    ] {
        let wrong = body.replace(proper, missing);
        let error = check(&[("defs.omg", defs), ("calls.omg", &wrong)]).unwrap_err();
        assert_eq!(error.file, "calls.omg");
        assert!(error.message.contains(effect) && error.message.contains("acknowledgement"), "{error:?}");
    }
}

#[test]
fn published_sort_requirement_binds_each_calls_element_and_ordering() {
    let sorting = block(GENERICS, 61);
    let ordering = "trait Ordering<T>{machine precedes(a:T,b:T)->bool;} ByteOrder satisfies Ordering<u8>{machine precedes(a:u8,b:u8)->bool{a<b}} WideOrder satisfies Ordering<u16>{machine precedes(a:u16,b:u16)->bool{a<b}}";
    let calls = "machine byte(xs:@mut [u8]) {Sorting::sort::<u8,ByteOrder>(xs)} machine wide(xs:@mut [u16]) {Sorting::sort::<u16,WideOrder>(xs)}";
    let stage = check(&[
        ("defs.omg", ordering),
        ("sort.omg", &sorting),
        ("calls.omg", calls),
    ])
    .unwrap();
    for (call, ty) in stage.calls.iter().zip(["u8", "u16"]) {
        assert_eq!(call.machine.path, "Sorting::sort");
        assert_eq!(call.type_arguments[0].to_string(), ty);
    }
    assert_eq!(stage.calls.len(), 2);
    let wrong = calls.replace("u8,ByteOrder", "u8,WideOrder");
    let error = check(&[
        ("defs.omg", ordering),
        ("sort.omg", &sorting),
        ("calls.omg", &wrong),
    ])
    .unwrap_err();
    assert_eq!(error.message, "selected evidence has the wrong trait application");
}

#[test]
fn published_static_parameters_require_the_declared_argument_role() {
    let binders = block(GENERICS, 636).replace("{ ... }", "{     }");
    let defs = "trait Ranked<T>{machine before(a:T,b:T)->bool;} trait HookProcedure {machine call(x:u32);} data HookSlot{} machine identity(v:u32)->u32{v} ByteRank satisfies Ranked<u8>{machine before(a:u8,b:u8)->bool{a<b}} machine handle(x:u32) satisfies HookProcedure::call {}";
    let caller = "machine examples(xs:@mut [u8],slot:@mut HookSlot)->u32{sort::<u8,ByteRank>(xs);install::<handle>(slot);apply::<identity>(1)}";
    let stage = check(&[
        ("defs.omg", defs),
        ("binders.omg", &binders),
        ("caller.omg", caller),
    ])
    .unwrap();
    assert_eq!(
        stage
            .calls
            .iter()
            .filter(|c| c.site.file == "caller.omg")
            .map(|c| c.machine.path.as_str())
            .collect::<Vec<_>>(),
        ["sort", "install", "apply"]
    );
    for (bad_defs, bad_call, reason) in [
        (
            defs.replace(" satisfies HookProcedure::call {}", " {}"),
            caller.to_owned(),
            "exact satisfaction edge",
        ),
        (defs.to_owned(), caller.replace("::<identity>", "::<handle>"), "signature"),
        (
            defs.replace("ByteRank satisfies Ranked<u8>", "ByteRank satisfies Ranked<u16>")
                .replace("before(a:u8,b:u8)", "before(a:u16,b:u16)"),
            caller.to_owned(),
            "wrong trait application",
        ),
    ] {
        let error = check(&[
            ("defs.omg", &bad_defs),
            ("binders.omg", &binders),
            ("caller.omg", &bad_call),
        ])
        .unwrap_err();
        assert_eq!(error.file, "caller.omg");
        assert!(error.message.contains(reason), "{error:?}");
    }
}

#[test]
fn published_record_call_selects_the_shared_counter_field() {
    let text = block(include_str!("../../../wiki/spec/language/machine_values.md"), 17);
    let (decls, operations) = text.split_once("let operations").unwrap();
    let caller = format!("machine observe(counter:Counter) {{ let operations{operations} }}");
    let stage = check(&[("defs.omg", decls), ("observe.omg", &caller)]).unwrap();
    let [call] = stage.indirect_calls.as_slice() else {
        panic!("expected field call")
    };
    assert_eq!(call.contract.result.to_string(), "u64");
    assert_eq!(call.contract.parameters[0].to_string(), "@game::Counter");
    assert!(call.contract.terminates);
    let wrong = decls.replace("machine value(counter: @Counter)", "machine value(counter: @mut Counter)");
    let error = check(&[("defs.omg", &wrong), ("observe.omg", &caller)]).unwrap_err();
    assert_eq!(error.file, "observe.omg");
    assert!(error.message.contains("incompatible callable signature"));
}

#[test]
fn published_static_type_disjunction_reaches_typing() {
    let clause = block(GENERICS, 306);
    let source = format!("machine restrict<T>() {clause} {{}}");
    let stage = check(&[("clause.omg", &source)]).unwrap();
    let comparisons = stage
        .expressions
        .iter()
        .filter(|e| matches!(e.site.span.slice(source.as_bytes()), Some(b"T == u16" | b"T == u32" | b"T == u64")) && e.ty == ValueType::Bool)
        .count();
    assert_eq!(comparisons, 3);
    let shadow = source.replace("restrict<T>()", "restrict<T>(u32:u16)");
    let error = check(&[("clause.omg", &shadow)]).unwrap_err();
    assert_eq!(error.message, "type equality mixes a type and a value");
    assert_eq!(error.span.slice(shadow.as_bytes()), Some(b"T == u32".as_slice()));
}

#[test]
fn explicit_quantity_carrier_matches_the_subject_in_every_domain_position() {
    let declaration = block(DOMAINS, 1020);
    let defs = "data Unit=u8;";
    for body in [
        "machine f(x:u16 in u16::Quantity<1>) {}",
        "machine f(x:u16){let y=x as +u16::Quantity<1>;}",
        "machine f(x:u16)->bool{x in u16::Quantity::<1>}",
    ] {
        check(&[
            ("defs.omg", defs),
            ("quantity.omg", &declaration),
            ("use.omg", body),
        ])
        .unwrap();
        let wrong = body.replace("x:u16", "x:u8");
        let error = check(&[
            ("defs.omg", defs),
            ("quantity.omg", &declaration),
            ("use.omg", &wrong),
        ])
        .unwrap_err();
        assert_eq!(error.message, "domain qualifier names u16, found u8");
    }
}

#[test]
fn domain_qualification_keeps_the_bound_carrier_and_indices_separate() {
    let definitions = "domain<T, const N:u8> T::Level<N>; data Packet<V>{field:V;}";
    for source in [
        "machine f<U>(x:U in U::Level<2>) {}",
        "machine f<U>(x:U)->bool {x in U::Level::<2>}",
        "machine f(x:Packet<u32> in Packet<u32>::Level<2>) {}",
    ] {
        check(&[("definitions.omg", definitions), ("use.omg", source)]).unwrap();
    }
    for binder in ["U", "const U:u8"] {
        let code = format!("data U {{}} machine f<{binder}>(x:U in U::Level<2>) {{}}");
        let typed = check(&[("domains.omg", definitions), ("shadow.omg", &code)]).unwrap();
        let arg = &typed
            .signatures
            .iter()
            .find(|s| s.machine.path == "f")
            .unwrap()
            .parameters[0]
            .1;
        if binder == "U" {
            assert!(matches!(arg.carrier(), ValueType::Parameter{owner,..} if owner.path=="f"));
        } else {
            assert!(matches!(arg.carrier(), ValueType::Nominal(id) if id.path=="U"));
        }
    }
    let fixed = "data Packet<T>{field:T;} domain<const N:u8> Packet<u32>::Length<N>;";
    let good = "machine f(x:Packet<u32> in Length<2>) {}";
    check(&[("defs.omg", fixed), ("use.omg", good)]).unwrap();
    let wrong_qualifier = "machine f(x:Packet<u32> in Packet<u16>::Length<2>) {}";
    assert!(
        check(&[("defs.omg", fixed), ("use.omg", wrong_qualifier)])
            .unwrap_err()
            .message
            .contains("domain qualifier names")
    );
    let wrong_carrier = good.replace("Packet<u32>", "Packet<u16>");
    assert!(
        check(&[("defs.omg", fixed), ("use.omg", &wrong_carrier)])
            .unwrap_err()
            .message
            .contains("belongs to")
    );
    let wrong = "machine f<U,V>(x:V in U::Level<2>) {}";
    let err = check(&[("definitions.omg", definitions), ("wrong.omg", wrong)]).unwrap_err();
    assert!(err.message.contains("domain qualifier names"), "{err:?}");
}

#[test]
fn an_annotated_domain_carrier_takes_its_role_from_its_bound() {
    let aliases = ("aliases.omg", "data Sort=core::Type<0>; data Byte=u8;");
    for binder in ["T:core::Type<0>", "T:Sort", "const T:Sort"] {
        let domain = format!("domain<{binder}, const N:u8> T::Level<N>;");
        let caller = "machine f(x:u16 in u16::Level<2>) {}";
        for shadow in ["", "data T {}"] {
            let stage = check(&[
                aliases,
                ("domain.omg", &domain),
                ("shadow.omg", shadow),
                ("main.omg", caller),
            ])
            .unwrap();
            let parameter = &stage
                .signatures
                .iter()
                .find(|s| s.machine.path == "f")
                .unwrap()
                .parameters[0]
                .1;
            assert_eq!(
                parameter.carrier(),
                &ValueType::Integer {
                    width: Some(16),
                    signed: false
                }
            );
            assert_eq!(parameter.domains()[0].declaration.path, "T::Level");
        }
    }
    let domain = ("domain.omg", "domain<const T:Byte> T::Level<T>;");
    let invalid = check(&[aliases, domain]).unwrap_err();
    assert!(invalid.message.contains("type"), "{invalid:?}");
    let shadow = ("shadow.omg", "data T {}");
    let caller = ("main.omg", "machine f(x:T in T::Level<2>) {}");
    let typed = check(&[aliases, domain, shadow, caller]).unwrap();
    assert!(matches!(typed.signatures[0].parameters[0].1.carrier(), ValueType::Nominal(id) if id.path == "T"));
}

#[test]
fn a_bound_decides_which_visible_domain_applies_to_the_written_carrier() {
    let defs = "data T {} data Byte=u8; domain<const T:Byte> T::Tag<T>; domain<const N:u8> u16::Tag<N>;";
    for source in [
        "data Applied=u16::Tag<2>; machine f(x:Applied) {}",
        "machine f(x:u16)->bool {x in u16::Tag::<2>}",
        "machine f(x:u16) {let tagged=x as +u16::Tag<2>;}",
        "machine f() {let tagged=1 as +u16::Tag<2>;}",
    ] {
        let stage = check(&[("first.omg", source), ("last.omg", defs)]).unwrap();
        assert!(
            stage
                .memberships
                .iter()
                .all(|m| m.domain.declaration.path == "u16::Tag")
        );
    }
    let both = "data T{} domain<T:core::Type<0>,const N:u8> T::Tag<N>; domain<const N:u8> u16::Tag<N>;";
    let ambiguous = check(&[
        ("domains.omg", both),
        ("use.omg", "machine f(x:u16 in u16::Tag<1>) {}"),
    ])
    .unwrap_err();
    for owner in ["game::u16::Tag", "game::T::Tag"] {
        assert!(ambiguous.message.contains(owner), "{ambiguous:?}");
    }
    let machine = "machine u16::Tag::inspect(x:u16) {}";
    check(&[("domain.omg", defs), ("machine.omg", machine)]).unwrap();
    let operator = "machine + u16::Tag::add(x:u16 in u16::Tag<2>, y:u16 in u16::Tag<2>)->u16 { 0 } machine sum(x:u16 in u16::Tag<2>)->u16 {x+x}";
    let typed = check(&[("domain.omg", defs), ("machine.omg", operator)]).unwrap();
    assert!(typed.calls.iter().any(|c| c.machine.path == "u16::Tag::add"));
    let names = "data Key=u8; const Key:u8=2; data Applied=u16::Tag<Key>; machine f(x:Applied) {}";
    check(&[("aliases.omg", names), ("domains.omg", defs)]).unwrap();
    let indirect = defs.replace("data T {}", "data Box<X>{} data T=Box<Applied>;");
    check(&[
        ("aliases.omg", "data Applied=u16::Tag<2>;"),
        ("domains.omg", &indirect),
    ])
    .unwrap();
    for (x, domain) in [("T", "T::Tag"), ("u16", "u16::Tag")] {
        for name in [format!("{x}::Tag"), "Tag".into()] {
            let caller = format!("machine f(x:{x} in {name}<2>) {{}}");
            let stage = check(&[("defs.omg", defs), ("caller.omg", &caller)]).unwrap();
            assert_eq!(
                stage.signatures[0].parameters[0].1.domains()[0]
                    .declaration
                    .path,
                domain
            );
        }
    }
}

#[test]
fn a_foreign_domain_selection_checks_access_to_the_selected_carrier() {
    let edges = [
        omega_resolver::Dependency {
            requester: "policy",
            alias: "geom",
            package: "geometry",
        },
        omega_resolver::Dependency {
            requester: "app",
            alias: "lib",
            package: "policy",
        },
        omega_resolver::Dependency {
            requester: "app",
            alias: "geom",
            package: "geometry",
        },
    ];
    for (binder, index, generic) in [
        ("const T:core::Type<0>, const N:u8", "N", true),
        ("const T:u8", "T", false),
    ] {
        let policy = format!("use geom::T; public domain<{binder}> T::Mark<{index}>;");
        let caller = "use lib::T::Mark;";
        let files = [
            ("geometry", "types.omg", "public data T {}"),
            ("policy", "domains.omg", &policy),
            ("app", "main.omg", caller),
        ];
        let resolved = support::resolve_packages(&files, &edges[..2]);
        let [check] = resolved.carrier_visibility.as_slice() else {
            panic!("expected one conditional carrier access")
        };
        assert_eq!(check.domain.path, "T::Mark");
        assert_eq!(check.error.span.slice(caller.as_bytes()), Some(b"lib::T::Mark".as_slice()));
        let typed = omega_typer::type_stage(&resolved);
        if generic {
            typed.unwrap();
        } else {
            assert_eq!(typed.unwrap_err().message, "no authority to name carrier geometry::T");
        }
        if !generic {
            let local = "use lib; domain<const N:u8> u16::Mark<N>; machine f(x:u16 in Mark<2>) {}";
            let different = [files[0], files[1], ("app", "main.omg", local)];
            let selected = support::resolve_packages(&different, &edges[..2]);
            let typed = omega_typer::type_stage(&selected).unwrap();
            assert_eq!(
                typed.signatures[0].parameters[0].1.domains()[0]
                    .declaration
                    .package,
                "app"
            );
        }
        let allowed = support::resolve_packages(&files, &edges);
        assert!(allowed.carrier_visibility.is_empty());
        omega_typer::type_stage(&allowed).unwrap();
    }
}

#[test]
fn an_imported_parent_domain_travels_with_its_child() {
    let parent = ("units/p.omg", "public domain u8::Positive;");
    let source = "use units::u8::Positive; domain Positive::Small; machine f(x:u8 in Positive::Small) {}";
    let stage = check(&[parent, ("use.omg", source)]).unwrap();
    let sig = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
    let mut atoms: Vec<_> = sig.parameters[0]
        .1
        .domains()
        .iter()
        .map(|a| a.declaration.path.as_str())
        .collect();
    atoms.sort();
    assert_eq!(atoms, ["Positive::Small", "units::u8::Positive"]);
}

#[test]
fn a_child_preserves_the_exact_parent_application() {
    let defs = "domain<const K:u8> u8::Limit<K>; use u8::Limit; domain Limit<2>::Pair; domain<const N:u8> Limit<N>::Nested<N>;";
    for (child, index) in [
        ("Limit::Pair", "2"),
        ("Limit::Nested<3>", "3"),
        ("Limit<1+1>::Pair", "2"),
        ("Limit<3>::Nested<3>", "3"),
    ] {
        let source = format!("use u8::Limit; machine f(x:u8 in {child}) {{}}");
        let stage = check(&[("use.omg", &source), ("domains.omg", defs)]).unwrap();
        let sig = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
        let parent = sig.parameters[0]
            .1
            .domains()
            .iter()
            .find(|d| d.declaration.path == "u8::Limit")
            .unwrap();
        assert_eq!(parent.arguments.len(), 1);
        assert_eq!(parent.arguments[0].to_string(), index);
        assert!(matches!(&parent.arguments[0], ValueType::StaticInteger {carrier,..} if carrier.to_string()=="u8"));
    }
    for annotation in ["u8 in Limit<7>::Pair", "Limit<7>::Nested<3>"] {
        let wrong = format!("use u8::Limit; machine f(x:{annotation}) {{}}");
        let error = check(&[("use.omg", &wrong), ("domains.omg", defs)]).unwrap_err();
        assert!(error.message.contains("parent domain"), "{error:?}");
    }
}

#[test]
fn aliases_share_their_targets_ownership_properties_across_files() {
    let owner = "data Item [linear,carry(suspension:allowed,cpu:same,thread:any,address:movable,)] {}";
    let alias = "data Resource [linear]=Item; data Again=Resource;";
    let stage = check(&[("alias.omg", alias), ("owner.omg", owner)]).unwrap();
    let [(id, properties)] = stage.data_properties.as_slice() else {
        panic!("expected only the nominal declaration")
    };
    assert_eq!(id.path, "Item");
    assert_eq!(properties.multiplicity, omega_typer::Multiplicity::Linear);
    let carry = properties.carry.as_ref().unwrap();
    assert!(carry.allows_suspension && carry.requires_same_cpu && !carry.requires_same_thread && !carry.requires_stable_address);
    let wrong = alias.replace("[linear]", "[copy]");
    let error = check(&[("alias.omg", &wrong), ("owner.omg", owner)]).unwrap_err();
    assert_eq!(error.file, "alias.omg");
    assert!(
        error
            .message
            .contains("cannot change its target's multiplicity"),
        "{error:?}"
    );
}
