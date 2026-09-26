use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::{
    checked_program, checked_program_result, resolved_program, typed_program,
};

/// TPR3 slice 4: the checked termination facts -- the `checked_summary`'s
/// producer. Every acyclic checked body derives Terminates without a
/// witness; a proven witness establishes it WITH the resolved explicit
/// view. The local summary remains separate from the authored public promise.
#[test]
fn checked_termination_plans_record_summaries_and_resolved_views() {
    use language_semantics::TerminationGuarantee;

    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let a: u64 = self.promise();
        let b: u64 = self.countdown(2);
    }

    machine Main::promise(&mut self) -> u64 terminates; { 7 }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }
    "#;

    let typed = typed_program(source);

    let machine = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
    };

    let promise = crate::checks::termination::build_checked_termination_plan(
        &typed,
        machine("Main::promise"),
    );
    assert_eq!(
        promise.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert!(promise.implementation_witness.is_none());

    let countdown = crate::checks::termination::build_checked_termination_plan(
        &typed,
        machine("Main::countdown"),
    );
    assert_eq!(
        countdown.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert_eq!(
        countdown
            .implementation_witness
            .as_ref()
            .expect("proven witness")
            .view_path,
        "Nat::Descending"
    );

    let inferred =
        crate::checks::termination::build_checked_termination_plan(&typed, machine("Main::main"));
    assert_eq!(
        inferred.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert!(inferred.implementation_witness.is_none());
}

#[test]
fn checked_proof_scc_retains_every_exact_structural_subterm_call_site() {
    let source = r#"
    data ProofTree {
        case Leaf;
        case Branch(first: ProofTree, second: ProofTree);
    }

    data Main {}
    machine Main::main(&mut self) {}

    machine left(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: right(first),
                second: right(second),
            }
        }
    }

    machine right(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: left(first),
                second: left(second),
            }
        }
    }
    "#;

    let typed = typed_program(source);
    let machine_symbol = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let left = machine_symbol("left");
    let right = machine_symbol("right");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("measured proof SCC should check");

    let [component] = checked
        .facts
        .termination
        .proof_recursive_components
        .as_slice()
    else {
        panic!("one proof SCC should be retained")
    };
    assert_eq!(
        component.ranking_relation,
        checked_trees::CheckedProofRankingRelation::StructuralSubterm
    );
    assert!(component.rank_type_identity.contains("ProofTree"));
    assert_eq!(
        component
            .members
            .iter()
            .map(|member| member.machine)
            .collect::<Vec<_>>(),
        vec![left, right]
    );
    assert_eq!(component.edges.len(), 4);
    assert_eq!(
        component
            .edges
            .iter()
            .filter(|edge| edge.caller == left && edge.callee == right)
            .count(),
        2,
        "two exact calls between the same machine pair must not collapse"
    );
    assert_eq!(
        component
            .edges
            .iter()
            .filter(|edge| edge.caller == right && edge.callee == left)
            .count(),
        2,
        "the reverse pair must retain both exact calls too"
    );
    assert!(component.edges.iter().all(|edge| matches!(
        edge.site,
        checked_trees::CheckedProofRecursiveCallSite::Expression { .. }
    )));
    let exact_sites = component
        .edges
        .iter()
        .map(|edge| match edge.site {
            checked_trees::CheckedProofRecursiveCallSite::Expression {
                state,
                statement_index,
                expression_ordinal,
            } => (
                state.arena_index(),
                state.generation(),
                statement_index,
                expression_ordinal,
            ),
            _ => unreachable!("all retained sites are expression calls"),
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        exact_sites.len(),
        4,
        "checked coordinates must distinguish every recursive call without arena expression handles"
    );
    assert!(
        component
            .edges
            .iter()
            .all(|edge| edge.strict_member_path.len() == 1)
    );
    let unique_paths = component
        .edges
        .iter()
        .map(|edge| {
            edge.strict_member_path
                .iter()
                .map(|member| (member.arena_index(), member.generation()))
                .collect::<Vec<_>>()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        unique_paths.len(),
        2,
        "the two payload declarations must remain distinct exact witnesses"
    );
}

#[test]
fn checked_singleton_proof_scc_retains_its_exact_self_edge() {
    let source = r#"
    data ProofTree {
        case Leaf;
        case Branch(first: ProofTree, second: ProofTree);
    }

    machine descend(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: descend(first),
                second: second,
            }
        }
    }
    "#;

    let typed = typed_program(source);
    let descend = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "descend")
        .expect("descend machine")
        .symbol;
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("measured self recursion should check");

    let [component] = checked
        .facts
        .termination
        .proof_recursive_components
        .as_slice()
    else {
        panic!("one singleton proof SCC should be retained")
    };
    assert_eq!(component.members.len(), 1);
    assert_eq!(component.members[0].machine, descend);
    assert_eq!(component.edges.len(), 1);
    assert_eq!(component.edges[0].caller, descend);
    assert_eq!(component.edges[0].callee, descend);
    assert_eq!(component.edges[0].strict_member_path.len(), 1);
    assert!(matches!(
        component.edges[0].site,
        checked_trees::CheckedProofRecursiveCallSite::Expression { .. }
    ));
}

#[test]
fn inferred_completion_never_publishes_a_promise() {
    use language_semantics::TerminationGuarantee;

    let source = r#"
    data Main {}
    machine Main::run(&mut self) -> u64 { self.inferred() }
    machine Main::inferred(&mut self) -> u64 { 1 }
    machine Main::promised(&mut self) -> u64 terminates; { 1 }
    "#;

    let typed = typed_program(source);
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let inferred = symbol_of("Main::inferred");
    let promised = symbol_of("Main::promised");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("checked lowering should succeed");

    assert_eq!(
        checked
            .facts
            .termination
            .for_machine(inferred)
            .expect("inferred local summary")
            .checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert_eq!(
        checked
            .facts
            .termination
            .for_machine(inferred)
            .expect("inferred contract plan")
            .interface,
        language_semantics::TerminationInterface::InternalDerived,
        "body inference must never redefine the published contract"
    );
    assert_eq!(
        checked
            .facts
            .termination
            .for_machine(promised)
            .expect("promised contract plan")
            .interface,
        language_semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new()
        })
    );
}

/// TPR4 slice 2: the requirement's authored guarantee PROPAGATES into the
/// resolved trait-signature record (populated at syntax->resolved, per
/// signature -- inheritance at conformance consumes it next).
#[test]
fn trait_requirement_guarantee_propagates_to_resolved_signatures() {
    let source = r#"
    trait Worker {
        machine run(&mut self, n: u64) -> u64 terminates;
        machine peek(&self) -> u64;
    }

    data Main {}

    machine Main::main(&mut self) -> u64 { 7 }
    "#;

    let resolved = resolved_program(source);

    let worker = resolved
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Worker")
        .expect("Worker trait");
    let signatures = resolved.trait_machine_signatures(worker.machines);
    let flag_of = |name: &str| {
        signatures
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .unwrap_or_else(|| panic!("signature {name}"))
            .terminates_guarantee
    };
    assert!(flag_of("run"), "run authored the guarantee");
    assert!(!flag_of("peek"), "peek promised nothing");
}

/// TPR4 slice 3 (decision 23): an implementation satisfying a requirement
/// that authored `terminates;` INHERITS the published guarantee -- it does
/// not repeat the clause. A cyclic inheritor must then supply the
/// discharging witness or FAIL (the inherited claim is not optional), and
/// with a witness it proves like any measured machine.
#[test]
fn implementation_inherits_requirement_guarantee() {
    use language_semantics::TerminationGuarantee;

    let source = r#"
    trait Worker {
        machine run(&mut self, n: u64) -> u64 terminates;
    }

    data Main {}

    machine Main::run(&mut self, n: u64) -> u64 satisfies Worker::run {
        n
    }

    machine Main::main(&mut self) -> u64 {
        let value: u64 = self.run(7);
        value
    }
    "#;

    let typed = typed_program(source);

    let run = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("run machine");
    assert_eq!(
        &run.termination_plan.interface,
        &language_semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new()
        }),
        "the implementation inherits the requirement's published guarantee"
    );
    assert!(run.termination_plan.implementation_witness.is_none());

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("an acyclic inheritor discharges the claim for free");
}

/// TPR4: omission has different normalized meaning on a private body and a
/// public requirement. The implementation inherits the requirement's
/// published `NoGuarantee`; its locally inferred completion remains
/// implementation evidence rather than silently strengthening that contract.
#[test]
fn public_termination_omission_is_distinct_from_private_derivation() {
    let source = r#"
    trait Worker {
        machine run(&self) -> u64;
    }

    data Main {}

    machine Main::run(&self) -> u64 satisfies Worker::run {
        1
    }

    machine Main::local(&self) -> u64 {
        2
    }

    machine Main::main(&mut self) -> u64 {
        0
    }
    "#;

    let typed = typed_program(source);

    let plan_of = |name: &str| {
        &typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .termination_plan
            .interface
    };
    assert_eq!(
        plan_of("Main::run"),
        &language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::NoGuarantee,
        )
    );
    assert_eq!(
        plan_of("Main::local"),
        &language_semantics::TerminationInterface::InternalDerived
    );
}

/// TPR4 slice 3: a CYCLIC implementation inheriting the guarantee without a
/// witness FAILS with the missing-witness diagnostic -- the inherited claim
/// is enforced by the same plan gate as an authored one. Supplying the
/// witness (`terminates by n;`) discharges it.
#[test]
fn cyclic_inheritor_without_witness_fails_and_witness_discharges() {
    let template = |witness_clause: &str| {
        format!(
            r#"
    trait Worker {{
        machine run(&mut self, n: u64) -> u64 terminates;
    }}

    data Main {{}}

    machine Main::run(&mut self, n: u64) -> u64 satisfies Worker::run
    {witness_clause}
    {{
        transition n > 0 {{
            true -> self.run(n - 1)
            false -> 0
        }}
    }}

    machine Main::main(&mut self) -> u64 {{
        let value: u64 = self.run(7);
        value
    }}
    "#
        )
    };

    // Without a witness: the inherited claim cannot be checked on a cycle.
    let source = template("");
    let diagnostics = checked_program_result(&source)
        .expect_err("a cyclic inheritor without a witness must fail");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("recursive cycle")
                && diagnostic.message.contains("ranking witness")
        }),
        "expected the missing-witness diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );

    // With the witness: the inherited claim discharges.
    let source = template("terminates by n;");
    checked_program(&source);
}
