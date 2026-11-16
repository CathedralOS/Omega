//! The real bundled Run declarations must survive ordinary parsing, generic
//! preparation, source lowering and call proof checking. Incoming Live is a
//! public caller obligation; these controls do not establish runtime cells.

use super::*;
use language_core::sealed_intrinsics::RunCustodyOperation;
use semantic_vocabulary::{IntegerSign, TargetMachineCallable};
use std::sync::Arc;
use terminal_check_tables::edges::Callee;
use terminal_psi::{CheckedBoundaryExecution, ClosedReachArgument, StructuralFieldType};

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/fixtures/pass/collections/run_split_merge/main.omg"
));

fn prepared(text: &str) -> typed_trees::FrozenTypedTrees {
    let mut sources = source::SourceMap::default();
    let product = sources
        .add_checked_instance(
            "package/main.omg".into(),
            text,
            "package".into(),
            Some(super::build_purpose::package()),
            source::SourceOrigin::User,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(product, &tokens).unwrap();
    // Run's actual file also declares profile-backed operations. Load the real
    // core frontier, as the product loader does, rather than fabricating their
    // dependencies or editing the sealed file down to a different contract.
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../source/library/core");
    let mut files = std::fs::read_dir(&root)
        .unwrap()
        .map(|row| row.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "omg"))
        .collect::<Vec<_>>();
    files.sort_unstable();
    let mut run_source = None;
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let id = sources
            .add_with_metadata(
                path.clone(),
                text.clone(),
                root.clone(),
                None,
                source::SourceOrigin::Toolchain,
            )
            .source_id;
        if path.file_name().is_some_and(|name| name == "run.omg") {
            run_source = Some(id);
        }
        let tokens = source_files_to_tokens::Lexer::new(&text)
            .tokenize()
            .unwrap();
        tokens_to_syntax_trees::parse(&mut syntax, id, &tokens).unwrap();
    }
    let anchor = run_source.expect("actual bundled Run unit");
    let bindings = [
        "Run",
        "RunParts",
        "Live",
        "AccessPolicy",
        "AccessPlan",
        "Issuance",
        "reflect::TypeSchema",
        "access",
    ]
    .map(|declaration| {
        symbols::SourceScopedTopLevelBinding::logical_module_import(
            product,
            anchor,
            &format!("omega::language::core::{declaration}"),
            3,
        )
    });
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: bindings.into(),
            retained_base: None,
        },
    )
    .expect("the actual imported Run program resolves");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("the actual imported Run program types");
    for machine in typed.machines() {
        if !matches!(
            typed
                .symbols
                .sealed_declaration(machine.symbol, machine.name.as_str()),
            Some(
                language_core::sealed_intrinsics::SealedIntrinsic::CompilerPrimitive(
                    language_core::sealed_intrinsics::CompilerPrimitive::RunCustody(_)
                )
            )
        ) {
            continue;
        }
        if let Err(reason) = typed.compiler_primitive_schema(machine.symbol) {
            let application = machine.attached_data_application;
            let attachment = application
                .is_valid()
                .then(|| typed.type_reference_table.type_reference(application));
            eprintln!(
                "Run schema {}: {reason:?}; attachment={attachment:?}; binders={:?}",
                machine.name,
                typed.machine_type_parameters(machine)
            );
            if let Some(typed_trees::types::TypeReferenceNode::Generic { arguments, .. }) =
                attachment
            {
                for argument in typed
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    eprintln!(
                        "attachment argument {argument:?}: {:?}",
                        typed.type_reference_table.type_reference(*argument)
                    );
                }
            }
            let entry_contracts = typed
                .machine_states
                .span_or_empty(machine.states)
                .iter()
                .flat_map(|entry| typed.signature_contracts.span_or_empty(entry.contracts));
            for contract in typed
                .signature_contracts
                .span_or_empty(machine.contracts)
                .iter()
                .chain(entry_contracts)
            {
                for fact in typed.proof_facts.span_or_empty(contract.facts) {
                    let typed_trees::domain::ProofFact::Expression(root) = fact else {
                        eprintln!("unserved signature fact {fact:?}");
                        continue;
                    };
                    typed.subtree_for_each(*root, typed_trees::SubtreeEdges::default(), |handle,node| {
                        use typed_trees::expression::{ExpressionNode, MemberSelection};
                        let unresolved = match node {
                            ExpressionNode::Member(member) => matches!(member.selection, MemberSelection::Unresolved) || matches!(member.selection, MemberSelection::Field(symbol) if !symbol.is_valid()),
                            ExpressionNode::Call(call) => !call.target_symbol.is_valid(),
                            ExpressionNode::Name(name) => !name.symbol.is_valid() && !name.head_symbol.is_valid() && !name.reserved_result_of.is_valid(),
                            _ => false,
                        };
                        if unresolved { eprintln!("unresolved schema expression {handle:?}: {node:?}"); }
                    });
                }
            }
        }
    }
    typed_trees_to_typed_trees::prepare_typed_trees(
        typed,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .expect("ordinary generic preparation closes each actual Run application")
    .0
}

fn lowered(text: &str) -> terminal_check_tables::program::Elaboration {
    let typed = prepared(text);
    typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[],
            subjects: &[Some(super::build_purpose::package())],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            calling: None,
            providers: None,
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .expect("closed Run source must lower through its real checked header")
}

#[test]
fn closed_run_split_merge_keeps_actual_types_specializations_and_call_proofs() {
    let elaboration = lowered(SOURCE);
    let module = &elaboration.module;
    let mut rows = Vec::new();
    for boundary in &module.boundary_machines {
        let Some(checked) = &boundary.declaration else {
            continue;
        };
        let CheckedBoundaryExecution::RunCustody {
            operation,
            policy,
            element,
        } = &checked.execution
        else {
            continue;
        };
        let [
            ClosedReachArgument::Const(actual_policy),
            ClosedReachArgument::Type(actual_element),
        ] = checked.method_arguments.as_slice()
        else {
            panic!("both actual P/T identities must survive");
        };
        assert_eq!(policy, actual_policy);
        let StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(integer)) =
            element
        else {
            panic!("each concrete scalar T must retain its actual scalar type");
        };
        assert_eq!(integer.sign(), IntegerSign::Unsigned);
        assert!(matches!(integer.bits(), Some(32 | 64)));
        let specialization = checked
            .source
            .specialization
            .as_ref()
            .expect("closed source application");
        assert!(specialization.capture.is_none());
        assert!(
            module
                .identities
                .text(*actual_element)
                .is_some_and(|text| !text.is_empty())
        );
        assert!(checked.source.parameter_types.iter().all(|identity| {
            module
                .identities
                .text(*identity)
                .is_some_and(|text| !text.is_empty())
        }));
        // Focused executable evidence can print this retained source metadata
        // with --nocapture while the independent receiver is being integrated.
        eprintln!(
            "Run {operation:?} P={} T={} machine={} state={} semantic={} callable={} template-callable={} parameter-types={:?} result-type={}",
            module.identities.text(*actual_policy).unwrap(),
            module.identities.text(*actual_element).unwrap(),
            module
                .identities
                .text(checked.source.machine.identity())
                .unwrap(),
            module
                .identities
                .text(checked.source.state.identity())
                .unwrap(),
            module
                .identities
                .text(checked.source.semantic_declaration)
                .unwrap(),
            module.identities.text(checked.source.callable).unwrap(),
            module
                .identities
                .text(specialization.template_callable)
                .unwrap(),
            checked
                .source
                .parameter_types
                .iter()
                .map(|identity| module.identities.text(*identity).unwrap())
                .collect::<Vec<_>>(),
            module.identities.text(checked.source.result_type).unwrap(),
        );
        rows.push((
            *operation,
            *actual_policy,
            *actual_element,
            specialization.commitment,
        ));
    }
    assert_eq!(
        rows.len(),
        4,
        "two real operations at each distinct P/T application"
    );
    for operation in [RunCustodyOperation::Split, RunCustodyOperation::Merge] {
        let matching = rows
            .iter()
            .filter(|row| row.0 == operation)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 2);
        assert_ne!(matching[0].1, matching[1].1);
        assert_ne!(matching[0].2, matching[1].2);
        assert_ne!(
            matching[0].3, matching[1].3,
            "one template cache must not poison another application"
        );
    }
    let verdict =
        judge(Ok(elaboration)).expect("the receiving checker must judge the exact source result");
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn a_public_run_split_without_its_cut_bound_fails_the_real_call_requirement() {
    let elaboration = lowered(&SOURCE.replacen("requires cut <= run.count", "", 1));
    let mut splits = Vec::new();
    for (machine, tables) in elaboration.tables.machines.iter() {
        for (invocation, row) in tables.invocations.iter() {
            let Callee::Declared(TargetMachineCallable::Boundary(boundary)) = row.callee else {
                continue;
            };
            if elaboration
                .module
                .boundary_machine(boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
                .is_some_and(|checked| {
                    checked.execution.run_custody_operation() == Some(RunCustodyOperation::Split)
                })
            {
                splits.push((machine, invocation));
            }
        }
    }
    let verdict =
        judge(Ok(elaboration)).expect("the negative must reach receiving call proof validation");
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("the actual missing cut bound must reject: {verdict:?}");
    };
    assert!(unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { invocation, clause: 0 } if splits.contains(&(row.machine, invocation)))), "{unproved:?}");
}
