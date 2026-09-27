use super::{
    BuildTimeEvaluationRequest, BuildTimeSourceContext, SelectedBuildTimeOperators,
    SelectedBuildTimeProviderBody, evaluate_pre_resolution,
};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;
use std::sync::Arc;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;

fn parsed_source(
    source: &str,
    package: PackageKeyIdentity,
) -> (syntax_trees::SyntaxTrees, Arc<SourceMap>) {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("cache/selected/main.omg"),
            source.to_owned(),
            PathBuf::from("cache/selected"),
            Some(package),
            SourceOrigin::User,
        )
        .source_id;
    let syntax = crate::front_end::syntax_program_with_id(source_id, source);
    (syntax, Arc::new(sources))
}

fn typed_after_pre_resolution(
    syntax: &syntax_trees::SyntaxTrees,
    sources: Arc<SourceMap>,
) -> typed_trees::TypedTrees {
    let resolved = crate::syntax_probes::resolve(syntax, Some(sources), &[])
        .expect("resolve pre-evaluated syntax");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type pre-evaluated syntax")
}

#[test]
fn package_aware_probe_retains_authored_symbol_ownership() {
    let source = "machine selected() {}";
    let package = PackageKeyIdentity::from_digest([0x6a; 32]).expect("nonzero package identity");
    let (syntax, sources) = parsed_source(source, package);

    let resolved = crate::syntax_probes::resolve(&syntax, Some(sources), &[])
        .expect("package-aware probe resolution");
    let machine = resolved.machines.first().expect("selected machine");

    assert_eq!(
        resolved.symbols.symbol_package_identity(machine.symbol),
        Some(package)
    );
}

/// Every literal value surviving inside a const-generic argument after
/// pre-resolution: integer const expressions and plain literal names both
/// count as folded results.
fn folded_const_arguments(syntax: &syntax_trees::SyntaxTrees) -> Vec<i64> {
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::types::TypeReferenceNode;
    syntax
        .type_references
        .generic_nodes()
        .iter()
        .flat_map(|node| match syntax.type_references.type_reference(*node) {
            TypeReferenceNode::Generic { arguments, .. } => syntax
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .filter_map(
                    |argument| match syntax.type_references.type_reference(*argument) {
                        TypeReferenceNode::Named(name) => name.as_str().parse::<i64>().ok(),
                        TypeReferenceNode::ConstExpression(expression) => {
                            match syntax.expressions.expression(*expression) {
                                ExpressionNode::Integer(literal) => literal.value_i64(),
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                )
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

/// The rows Omega's settlement derives for the program's selected
/// boundary-operator uses, exactly as `selected_provider_bodies` does from
/// the retained checked facts and the settled plan.
fn provider_body_rows(typed: &typed_trees::TypedTrees) -> Vec<SelectedBuildTimeProviderBody> {
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
    facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter(|fact| {
            typed
                .operators()
                .iter()
                .chain(typed.machine_token_bindings())
                .any(|operator| {
                    operator.symbol == fact.selected_operator_symbol && operator.is_boundary
                })
        })
        .map(|fact| {
            let provider = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::remainder")
                .unwrap();
            let entry = typed.machine_states(provider).first().unwrap();
            SelectedBuildTimeProviderBody {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operands: fact.operands(typed).unwrap(),
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type: provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            }
        })
        .collect()
}

#[test]
fn provider_backed_domain_fact_defers_then_evaluates_its_selected_body() {
    // The provider's body computes `left + right`, which differs from the
    // builtin `%` result, so `is_sum(7)` proving `7 + 2 == 9` is the positive
    // witness that the provider's machine -- not host arithmetic -- ran.
    let package = PackageKeyIdentity::from_digest([0x78; 32]).expect("nonzero package identity");
    let source = r#"
        data Math {}
        boundary operator % Math::remainder(left: u64, right: u64) -> u64;
        data Provider {}
        machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left + right }
        machine is_sum(value: u64) -> bool { value % 2 == 9 }
        domain u64::Summable requires is_sum(self);
        data FixedBuffer<const N: u64>
        where
            N in Summable,
        {
            values: [u8; N];
        }
        data Main { buffer: FixedBuffer<7>; }
        machine Main::main(&mut self) {}
    "#;
    let (syntax, sources) = parsed_source(source, package);
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: &[],
            selection_authority: None,
            retained_base: None,
        }),
    })
    .expect("a provider-backed domain fact survives pre-resolution evaluation");
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_after_pre_resolution(&syntax, sources);

    let pre_check = pre_check
        .evaluate_or_defer(&mut typed)
        .expect("deferral scan")
        .expect("a membership whose fact machine selects a boundary use must defer");

    let rows = provider_body_rows(&typed);
    assert_eq!(rows.len(), 1);
    pre_check
        .evaluate_selected_operators(
            &mut typed,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &rows,
            },
        )
        .expect("the resumed continuation runs the selected provider body");

    let instance = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "FixedBuffer<7>")
        .expect("concrete instance");
    assert!(
        typed
            .proof_facts
            .span_or_empty(instance.where_facts)
            .iter()
            .all(|fact| matches!(fact, ProofFact::Expression(expression)
            if matches!(
                typed.expression_table.expression(*expression),
                ExpressionNode::Boolean(true)
            ))),
        "the selected provider body proved the membership; builtin `%` folds 1 and `1 == 9` is false"
    );
    assert!(
        !instance.zero_gated,
        "a fully discharged fact list clears the instance's zero gate"
    );
}

/// A provider-dependent const application closes into a generated instance
/// once the selected body folds: `Buffer<limit()>` becomes `Named` to a
/// `Buffer<7>` whose member extent carries the literal, so downstream member
/// checks see a concrete extent rather than the open template's `N`.
#[test]
fn selected_provider_const_application_realizes_instance() {
    assert_selected_provider_const_application_realizes_instance("operator");
}

#[test]
fn token_machine_provider_const_application_realizes_instance() {
    assert_selected_provider_const_application_realizes_instance("machine");
}

fn assert_selected_provider_const_application_realizes_instance(introducer: &str) {
    let package = PackageKeyIdentity::from_digest([0x7b; 32]).expect("nonzero package identity");
    let source = r#"
        data Math {}
        boundary operator % Math::remainder(left: u64, right: u64) -> u64;
        data Provider {}
        machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
        machine limit() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
        data Buffer<const N: u64> { values: [u8; N]; }
        data Main { value: Buffer<limit()>; }
        machine Main::main(&mut self) { self.value.values[6] = 70; }
    "#;
    let source = source.replace("boundary operator", &format!("boundary {introducer}"));
    let (syntax, sources) = parsed_source(&source, package);
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: &[],
            selection_authority: None,
            retained_base: None,
        }),
    })
    .expect("pre-resolution evaluation");
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_after_pre_resolution(&syntax, sources);
    let pending = pre_check
        .evaluate_or_defer(&mut typed)
        .expect("defer scan")
        .expect("the provider-dependent application must defer");
    let rows = provider_body_rows(&typed);
    pending
        .evaluate_selected_operators(
            &mut typed,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &rows,
            },
        )
        .expect("selected fold");
    let buffer = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Buffer")
        .expect("Buffer template");
    let main = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("Main");
    let value_field = typed
        .data_members(main)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "value" => {
                Some(field.type_reference)
            }
            _ => None,
        })
        .expect("Main.value field");
    let typed_trees::types::TypeReferenceNode::Named {
        symbol: instance_symbol,
        ..
    } = typed.type_reference_table.type_reference(value_field)
    else {
        panic!("the folded application must close to a Named instance")
    };
    assert_ne!(
        *instance_symbol, buffer.symbol,
        "the instance is a generated definition, not the open template"
    );
    let instance = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *instance_symbol)
        .expect("generated Buffer<7> instance");
    assert!(
        instance.generic_instance.is_some(),
        "the instance retains its application origin"
    );
    let values_field = typed
        .data_members(instance)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "values" => {
                Some(field.type_reference)
            }
            _ => None,
        })
        .expect("instance values field");
    match typed.type_reference_table.type_reference(values_field) {
        typed_trees::types::TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(7),
            ..
        } => {}
        other => panic!("instance values extent must be Literal(7), got {other:?}"),
    }
    // Instance origins keep their authored `Generic` node as provenance; only
    // application sites that are not some instance's origin must be closed.
    let origins: std::collections::HashSet<_> = typed
        .data_definitions()
        .iter()
        .filter_map(|definition| definition.generic_instance)
        .collect();
    assert!(
        typed
            .type_reference_table
            .generic_type_reference_sites()
            .iter()
            .filter(|(site, _, _)| !origins.contains(site))
            .all(|(_, base, _)| *base != buffer.symbol),
        "no open Buffer application may survive the selected fold"
    );
}

#[test]
fn const_generic_zero_argument_call_still_rejects_a_parameterized_entry() {
    let package = PackageKeyIdentity::from_digest([0x77; 32]).expect("nonzero package identity");
    let source = r#"
        machine sized(value: u64) -> u64 {
            transition { _ -> value }
        }

        data FixedBuffer<const N: u64> {
            items: [u8; N];
        }

        data Main {
            buffer: FixedBuffer<sized()>;
        }
    "#;
    let (syntax, sources) = parsed_source(source, package);
    let errors = match evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources,
            source_scoped_top_level_bindings: &[],
            selection_authority: None,
            retained_base: None,
        }),
    }) {
        Ok(_) => panic!("a parameterless call site must keep the zero-argument boundary"),
        Err(errors) => errors,
    };
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("constant call argument count differs from its exact entry")),
        "{errors:?}"
    );
}

#[test]
fn const_generic_argument_calls_evaluate_machine_local_destinations() {
    // The `let folded: Buffer<size(4)>` annotation is parsed once per
    // speculative statement alternative before the real parse; the abandoned
    // clones stay in the type arena sharing the authored span. Only the clone
    // referenced by the local's declared destination may join the destination
    // check -- the copies must not demand destinations of their own.
    let package = PackageKeyIdentity::from_digest([0x7a; 32]).expect("nonzero package identity");
    let source = r#"
        machine size(n: u64) -> u64 {
            transition { _ -> n }
        }

        data Buffer<const N: u64> {
            values: [u8; N];
        }

        data Main {
            value: Buffer<size(4)>;
        }

        machine Main::main(&mut self) {
            let folded: Buffer<size(4)> = Buffer { values: [1, 2, 3, 4] };
        }
    "#;
    let (syntax, sources) = parsed_source(source, package);
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: &[],
            selection_authority: None,
            retained_base: None,
        }),
    })
    .expect(
        "an application call in a machine-local const argument occupies its declared destination",
    );
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let folded = folded_const_arguments(&syntax);
    assert_eq!(folded, vec![4]);
    let machine = syntax
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("the main machine");
    let state = syntax
        .items
        .state(syntax.items.state_handles(machine.states)[0]);
    let local_type = syntax
        .items
        .statements(state.statements)
        .iter()
        .find_map(|statement| match syntax.statements.statement(*statement) {
            syntax_trees::statement::StatementNode::LocalData(local) => Some(local.type_reference),
            _ => None,
        })
        .expect("the annotated local");
    match syntax.type_references.type_reference(local_type) {
        syntax_trees::types::TypeReferenceNode::Named(name) => {
            assert_eq!(name.as_str(), "Buffer<4>")
        }
        node => panic!(
            "the local's application call must fold into its synthesized instance spelling, got {node:?}"
        ),
    }
    let mut typed = typed_after_pre_resolution(&syntax, sources);
    pre_check
        .evaluate(&mut typed)
        .expect("the folded local destination must survive pre-check evaluation");
}
