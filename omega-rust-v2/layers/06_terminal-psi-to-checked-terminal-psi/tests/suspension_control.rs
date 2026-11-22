use omega_checker::check_local_suspension_control;
use omega_psi::{CallSite, CallTarget, CompletionContinuation, LoweredPackage};
use omega_resolver::{ParsedSource, StageSource};

fn program(subject: &str) -> LoweredPackage {
    let text = format!(
        "machine wait(x:i32)->i32 suspends {{x}}
        machine ready(x:i32)->bool suspends {{let v:i32 = suspend wait(x); v == 7}}
        machine forward(x:i32)->i32 suspends {{transition {{_ -> suspend wait(x),}}}}
        machine choose(x:i32)->u8 suspends {{{subject} {{true -> (17u8), _ -> (23u8),}}}}"
    );
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = ParsedSource {
        logical_path: "test.omg",
        text: &text,
        tokens: &tokens,
        tree: &tree,
    };
    let package = omega_toolchain::resolve_source_package(&[StageSource { package: "root", source }]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    omega_lowering::lower_package(&prepared).unwrap()
}

#[test]
fn named_and_direct_subjects_publish_the_same_nested_call_and_resume_control() {
    for subject in [
        "let selected:bool = suspend ready(x); transition selected",
        "transition suspend ready(x)",
    ] {
        let mut unit = program(subject);
        for (caller, callee) in [("choose", "ready"), ("ready", "wait")] {
            let m = unit
                .machines
                .iter()
                .find(|m| m.declaration.path == caller)
                .unwrap();
            let [crossing] = m.body.suspension_sites.as_slice() else {
                panic!("one nested call in {caller}")
            };
            let decision = check_local_suspension_control(&unit, m.id, crossing.call).unwrap();
            assert!(std::ptr::eq(decision.caller(), m));
            assert_eq!(decision.callee().declaration.path, callee);
            assert!(matches!(decision.call(), CallSite::Operation(_)));
            let continuation = decision.continuation();
            if caller == "choose" {
                assert!(matches!(continuation, CompletionContinuation::Terminator(_)));
            } else {
                assert!(matches!(continuation, CompletionContinuation::Operation(_)));
            }
        }
        let pos = unit
            .machines
            .iter()
            .position(|m| m.declaration.path == "choose")
            .unwrap();
        let caller = unit.machines[pos].id;
        let wait = unit
            .machines
            .iter()
            .find(|m| m.declaration.path == "wait")
            .unwrap()
            .id;
        let original = unit.machines[pos].body.suspension_sites[0].clone();
        unit.machines[pos].body.suspension_sites[0].callee = CallTarget::Machine(wait);
        assert!(check_local_suspension_control(&unit, caller, original.call).is_err());
        unit.machines[pos].body.suspension_sites[0] = original.clone();
        unit.machines[pos].body.suspension_sites[0].continuation = CompletionContinuation::Return;
        assert!(check_local_suspension_control(&unit, caller, original.call).is_err());
        unit.machines[pos].body.suspension_sites[0] = original.clone();
        unit.machines[pos].body.suspension_sites.push(original.clone());
        assert!(check_local_suspension_control(&unit, caller, original.call).is_err());
        unit.machines[pos].body.suspension_sites.pop();
        unit.machines[pos].body.call_acknowledgements.clear();
        assert!(check_local_suspension_control(&unit, caller, original.call).is_err());
    }
}

#[test]
fn terminal_completion_requires_an_unambiguous_edge() {
    let mut unit = program("transition suspend ready(x)");
    let forward = unit
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let id = forward.id;
    let [crossing] = forward.body.suspension_sites.as_slice() else {
        panic!("one terminal call")
    };
    let call = crossing.call;
    let CallSite::Edge(edge) = call else { panic!("terminal edge") };
    let decision = check_local_suspension_control(&unit, id, call).unwrap();
    assert_eq!(decision.callee().declaration.path, "wait");
    assert_eq!(decision.continuation(), CompletionContinuation::Return);
    let body = &mut unit.machines.iter_mut().find(|m| m.id == id).unwrap().body;
    body.blocks.push(omega_psi::BranchBlock {
        id: omega_psi::BlockId((body.blocks.len() + 1).try_into().unwrap()),
        operations: 0..0,
        scalar_parameters: vec![],
        structural_parameters: vec![],
        scalar_domain_invariants: vec![],
        terminator: omega_psi::BlockTerminator::Return {
            id: edge,
            result: omega_psi::ReturnEdge::Unit,
            cleanup: vec![],
        },
    });
    assert!(check_local_suspension_control(&unit, id, call).is_err());
}

#[test]
fn the_wait_follows_the_selected_package_even_with_another_ready_in_scope() {
    for branch in [
        r"let v:bool = suspend kit\ready(x); transition v",
        r"transition suspend kit\ready(x)",
    ] {
        let source = format!(
            r"use kit; use other;
            machine choose(x:i32)->u8 suspends {{{branch} {{true -> (17u8), _ -> (23u8),}}}}
            machine sibling(x:i32)->bool suspends {{suspend other\ready(x)}}"
        );
        let files = [
            ("root", source.as_str()),
            ("admitted", "public machine ready(x:i32)->bool suspends { true }"),
            ("decoy", "public machine ready(x:i32)->bool suspends { false }"),
        ];
        let parsed: Vec<_> = files
            .iter()
            .map(|(_, text)| {
                let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
                let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
                (tokens, tree)
            })
            .collect();
        let input: Vec<_> = files
            .iter()
            .zip(&parsed)
            .map(|((package, text), (tokens, tree))| StageSource {
                package,
                source: ParsedSource {
                    logical_path: "main.omg",
                    text,
                    tokens,
                    tree,
                },
            })
            .collect();
        let missing = omega_toolchain::resolve_admitted_packages(&input, &[]).unwrap_err();
        assert_eq!(missing.message, "unavailable import kit");
        for selected in ["admitted", "decoy"] {
            let edge = omega_resolver::Dependency {
                requester: "root",
                alias: "kit",
                package: selected,
            };
            let second = if selected == "admitted" { "decoy" } else { "admitted" };
            let other = omega_resolver::Dependency {
                requester: "root",
                alias: "other",
                package: second,
            };
            let resolved = omega_toolchain::resolve_admitted_packages(&input, &[edge, other]).unwrap();
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let unit = omega_lowering::lower_package(&prepared).unwrap();
            let m = unit
                .machines
                .iter()
                .find(|m| m.declaration.path == "choose")
                .unwrap();
            let [site] = m.body.suspension_sites.as_slice() else { panic!("one wait") };
            let resolved_call = check_local_suspension_control(&unit, m.id, site.call).unwrap();
            assert_eq!(resolved_call.callee().declaration.path, "ready");
            assert_eq!(resolved_call.callee().declaration.package, selected);
            let sibling = unit
                .machines
                .iter()
                .find(|m| m.declaration.path == "sibling")
                .unwrap();
            let [other] = sibling.body.suspension_sites.as_slice() else {
                panic!("one sibling call")
            };
            assert_eq!(
                check_local_suspension_control(&unit, sibling.id, other.call)
                    .unwrap()
                    .callee()
                    .declaration
                    .package,
                second
            );
        }
    }
}

#[test]
fn checking_the_arm_and_the_helper_uses_the_imported_value_type() {
    for subject in [
        "let v:bool = suspend ready(x); transition v",
        "transition suspend ready(x)",
    ] {
        let text = format!(
            r"use kit;
            machine wait(x:i32)->i32 suspends {{x}}
            machine ready(x:i32)->bool suspends {{ let v:i32 = suspend wait(x); let observed:i32 = kit\v(); observed == v }}
            machine choose(x:i32)->i32 suspends {{ {subject} {{ true -> accept(), _ -> other(), }}
                machine accept()->i32 {{ kit\v() }}
                machine other()->i32 {{ 0 }}
            }}"
        );
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        for (export, accepts) in [
            ("public machine v()->i32 {7}", true),
            ("public machine v()->i32 {8}", true),
            ("public machine v()->bool {true}", false),
        ] {
            let dependency_tokens = omega_lexer::lex(export.as_bytes()).unwrap();
            let dependency_tree = omega_parser::parse_file(export.as_bytes(), &dependency_tokens).unwrap();
            let input = [
                StageSource {
                    package: "root",
                    source: ParsedSource {
                        logical_path: "main.omg",
                        text: &text,
                        tokens: &tokens,
                        tree: &tree,
                    },
                },
                StageSource {
                    package: "library",
                    source: ParsedSource {
                        logical_path: "main.omg",
                        text: export,
                        tokens: &dependency_tokens,
                        tree: &dependency_tree,
                    },
                },
            ];
            let edge = omega_resolver::Dependency {
                requester: "root",
                alias: "kit",
                package: "library",
            };
            let resolved = omega_toolchain::resolve_admitted_packages(&input, &[edge]).unwrap();
            let prepared = omega_preparation::prepare_source_package(&resolved);
            if !accepts {
                let error = prepared.unwrap_err();
                assert_eq!(error.package, "root");
                assert!(text[error.span.get_start_offset()..error.span.get_end_offset()].contains(r"kit\v"), "{error:?}");
                continue;
            }
            let unit = omega_lowering::lower_package(&prepared.unwrap()).unwrap();
            let choose = unit
                .machines
                .iter()
                .find(|m| m.declaration.path == "choose")
                .unwrap();
            let [site] = choose.body.suspension_sites.as_slice() else {
                panic!("one waiting subject")
            };
            assert_eq!(
                check_local_suspension_control(&unit, choose.id, site.call)
                    .unwrap()
                    .callee()
                    .declaration
                    .path,
                "ready"
            );
            let value = unit
                .machines
                .iter()
                .find(|m| m.declaration.package == "library" && m.declaration.path == "v")
                .unwrap();
            for local in ["ready", "accept"] {
                let caller = unit
                    .machines
                    .iter()
                    .find(|m| m.declaration.path.rsplit("::").next() == Some(local))
                    .unwrap();
                let calls = caller
                    .body
                    .operations
                    .iter()
                    .filter_map(|op| match &op.kind {
                        omega_psi::OperationKind::Call { callee, .. } => Some(*callee),
                        _ => None,
                    })
                    .chain(
                        caller
                            .body
                            .blocks
                            .iter()
                            .filter_map(|block| match &block.terminator {
                                omega_psi::BlockTerminator::TerminalCall { callee, .. } => Some(*callee),
                                _ => None,
                            }),
                    );
                assert_eq!(calls.filter(|c| *c == CallTarget::Machine(value.id)).count(), 1, "{local}");
            }
        }
    }
}
