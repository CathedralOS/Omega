//! Fixtures shared by the provider planning tests: bound plan facts,
//! package selections and admitted facts.

use crate::ProviderPlanDerivation;
mod hosted_byte_supply;
mod independent_components;
mod probe_integer;
mod provider_derivation;
mod receipts_and_families;
mod schemas_and_syscalls;
mod selection_and_bridges;
mod spelled_operator_custody;

use crate::provider_planning::{
    Arc, ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema, TypedTrees,
    bind_selected_provider_plan_facts, derive_satisfies_plans,
};

fn bind_selected_provider_plan_facts_for_test(
    checked: &mut checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    facts: effects::SelectedProviderPlanFacts,
    root_grants: &[String],
) -> Result<effects::SelectedProviderPlanFacts, Vec<diagnostics::Diagnostic>> {
    let original = Arc::new(std::mem::take(checked));
    match bind_selected_provider_plan_facts(&original, candidates, facts, root_grants, &[]) {
        Ok(binding) => {
            let (program, selected, _) = binding.into_parts();
            *checked = Arc::try_unwrap(program).unwrap_or_else(|shared| (*shared).clone());
            Ok(selected)
        }
        Err(diagnostics) => {
            *checked = Arc::try_unwrap(original).unwrap_or_else(|shared| (*shared).clone());
            Err(diagnostics)
        }
    }
}

fn normalized_machine_identity(typed: &TypedTrees, name: &str) -> String {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing typed machine `{name}`"));
    typed
        .normalized_machine_overload_identity(machine)
        .expect("typed machine must have an entry overload")
        .identity()
}

fn typed_fixture(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize provider fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse provider fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve provider fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type provider fixture")
}

/// Like [`typed_fixture`], but parsed under a registered source so
/// selecting-machine provenance spans survive for provenance replay.
fn typed_fixture_with_source(file_name: &str, source: &str) -> TypedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(std::path::PathBuf::from(file_name), source.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize provider fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse provider fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("resolve provider fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type provider fixture")
}

fn operator_symbol_at_path(typed: &TypedTrees, path: &str) -> symbols::SymbolHandle {
    typed
        .operators()
        .iter()
        .find(|operator| {
            typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
                == path
        })
        .unwrap_or_else(|| panic!("missing typed operator `{path}`"))
        .symbol
}

fn data_symbol(typed: &TypedTrees, name: &str) -> symbols::SymbolHandle {
    typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing typed data `{name}`"))
        .symbol
}

/// One build-authored family selection as `harvest_provider_selections`
/// would retain it: the roster derived from the resolved operator symbol,
/// the provider data symbol, and the selecting machine's authored span.
fn family_selection_from_typed(
    typed: &TypedTrees,
    family_path: &str,
    provider_type: &str,
    selecting_machine: &str,
) -> crate::ProviderSelection {
    let selecting_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == selecting_machine)
        .unwrap_or_else(|| panic!("missing selecting machine `{selecting_machine}`"))
        .symbol;
    let source_span = typed
        .symbols
        .symbol_provenance_source_span(selecting_machine)
        .expect("selecting machine retains its authored span");
    crate::ProviderSelection {
        subject: crate::ProviderSelectionSubject::BoundaryOperatorFamily(
            crate::ProviderOperatorFamilySelection::derive(
                typed,
                operator_symbol_at_path(typed, family_path),
                family_path.to_owned(),
            )
            .expect("family roster derives from one resolved overload"),
        ),
        provider_type: crate::ProviderSelectionIdentity {
            symbol: data_symbol(typed, provider_type),
            package: None,
            canonical_path: provider_type.to_owned(),
            authored_path: provider_type.to_owned(),
        },
        composition_mode: crate::CompositionMode::Fused,
        selecting_machine,
        source_span,
    }
}

fn derive_provider_fixture(source: &str) -> (TypedTrees, ProviderPlan) {
    let typed = typed_fixture(source);
    let plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!(
            "provider fixture must derive exactly one plan, got {}",
            plans.len()
        );
    };
    (typed, plan.clone())
}

fn selected_operator_binding_fixture() -> (checked_trees::CheckedTrees, ProviderPlan) {
    let source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32
        requires value == value
        ensures result == value + 0 && value == value;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }

        machine run() -> i32 {
            transition { _ -> (CheckedMath::offset_zero(70)) }
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize selected-operator binding fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse selected-operator binding fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve selected-operator binding fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type selected-operator binding fixture");
    let plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!("selected-operator fixture must derive one provider plan")
    };
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check selected-operator binding fixture");
    (checked, plan.clone())
}

fn selection_plan(name: &str, methods: &[&str], rows: &[&str]) -> ProviderPlan {
    ProviderPlan {
        name: name.to_owned(),
        provider_type: name.to_owned(),
        provider_type_package_identity: None,
        target: String::new(),
        schema: ServiceSchema {
            trait_name: "Pair".to_owned(),
            trait_package_identity: None,
            methods: methods
                .iter()
                .map(|method| effects::provider_plan::ServiceMethod {
                    name: (*method).to_owned(),
                    requirement_owner: "Pair".to_owned(),
                    requirement_owner_package_identity: None,
                    requirement_identity: format!("Pair::{method}"),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["Pair".to_owned()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                })
                .collect(),
        },
        rows: rows
            .iter()
            .map(|method| ProviderPlanRow {
                method: (*method).to_owned(),
                requirement_identity: format!("Pair::{method}"),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::VtableSlot { index: 0 },
            })
            .collect(),
        origin_package_identity: None,
        origin_package: String::new(),
    }
}

fn package_selection(
    boundary_trait: &str,
    boundary_package: semantic_vocabulary::PackageKeyIdentity,
    provider_type: &str,
    provider_package: semantic_vocabulary::PackageKeyIdentity,
) -> crate::ProviderSelection {
    let mut selection = crate::ProviderSelection::exact_for_test(boundary_trait, provider_type);
    let crate::ProviderSelectionSubject::BoundaryTrait(subject) = &mut selection.subject else {
        panic!("package selection fixture uses one boundary trait")
    };
    subject.package = Some(boundary_package);
    selection.provider_type.package = Some(provider_package);
    selection
}

fn selected_plan_names(plans: &[ProviderPlan]) -> Vec<String> {
    plans.iter().map(|plan| plan.name.clone()).collect()
}

fn push_boundary_requirement(
    checked: &mut checked_trees::CheckedTrees,
    owner_symbol: symbols::SymbolHandle,
    owner_name: &str,
    requirement_symbol: symbols::SymbolHandle,
    requirement_name: &str,
) -> String {
    let mut owner = typed_trees::trait_definition::TraitDefinition {
        symbol: owner_symbol,
        is_boundary: true,
        name: typed_trees::name::Identifier::generated(owner_name),
        ..Default::default()
    };
    checked.typed.push_trait_machine_signature(
        &mut owner,
        typed_trees::signature::StateSignature {
            symbol: requirement_symbol,
            name: typed_trees::name::Identifier::generated(requirement_name),
            ..Default::default()
        },
    );
    checked.typed.push_trait_definition(owner);
    let owner = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == owner_symbol)
        .expect("inserted boundary owner");
    let requirement = checked
        .typed
        .trait_machine_signatures(owner)
        .iter()
        .find(|requirement| requirement.symbol == requirement_symbol)
        .expect("inserted boundary requirement");
    checked
        .typed
        .normalized_trait_requirement_overload_identity(owner, requirement)
        .identity()
}

fn set_exact_requirement(
    plan: &mut ProviderPlan,
    schema: &str,
    owner: &str,
    requirement_identity: &str,
) {
    plan.schema.trait_name = schema.to_owned();
    plan.schema.methods[0].requirement_owner = owner.to_owned();
    plan.schema.methods[0].requirement_identity = requirement_identity.to_owned();
    plan.rows[0].requirement_identity = requirement_identity.to_owned();
}

fn append_admitted_fact(
    checked: &mut checked_trees::CheckedTrees,
    subject_symbol: symbols::SymbolHandle,
    domain_symbol: symbols::SymbolHandle,
    owner_symbol: symbols::SymbolHandle,
    requirement_symbol: symbols::SymbolHandle,
) -> facts::FactHandle {
    let place = checked.facts.semantic.append_symbol_place(subject_symbol);
    checked.facts.semantic.append_fact(facts::Fact {
        place: facts::FactPlace::Place(place),
        point: facts::ProgramPoint::Global,
        origin: facts::FactOrigin::CallEnsures,
        evidence: facts::QualificationEvidence::from_admitted_requirement(
            owner_symbol,
            requirement_symbol,
        ),
        payload: facts::FactPayload::DomainMembership {
            value: Default::default(),
            domain: Default::default(),
            domain_symbol,
            semantic_domain: Default::default(),
        },
    })
}

#[derive(Clone, Copy, Debug)]
enum SelectedInvocationDrift {
    None,
    EmptyPlanName,
    DuplicatePlan,
    DuplicateSelectedSchema,
    EmptyMethodIdentity,
    DuplicateMethodIdentity,
    EmptyRowIdentity,
    CrossRowIdentity,
    MissingRow,
    DuplicateRow,
    EmptyInvocation,
    DuplicateInvocation,
}

fn boundary_trait(symbol: u32, name: &str) -> typed_trees::trait_definition::TraitDefinition {
    typed_trees::trait_definition::TraitDefinition {
        symbol: symbols::SymbolHandle::from_arena_index(symbol),
        name: typed_trees::name::Identifier::generated(name),
        is_boundary: true,
        ..Default::default()
    }
}

fn checked_invocation_fixture(
    parameter_trait: u32,
    parameter_count: usize,
) -> (
    TypedTrees,
    ProviderPlan,
    flow_effects::InvocationInferencePlan,
) {
    let source = symbols::SymbolHandle::from_arena_index(31);
    let target = symbols::SymbolHandle::from_arena_index(32);
    let foreign_source = symbols::SymbolHandle::from_arena_index(33);
    let machine_symbol = symbols::SymbolHandle::from_arena_index(34);
    let mut typed = TypedTrees::default();
    typed.push_trait_definition(boundary_trait(31, "pkg::Source"));
    typed.push_trait_definition(boundary_trait(32, "pkg::Target"));
    typed.push_trait_definition(boundary_trait(33, "other::Source"));
    let type_symbol = match parameter_trait {
        31 => source,
        32 => target,
        33 => foreign_source,
        other => symbols::SymbolHandle::from_arena_index(other),
    };
    let type_reference =
        typed
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: type_symbol,
                name: typed_trees::name::Identifier::generated("binding"),
            });
    let mut entry = typed_trees::state::State::default();
    typed.push_state_parameter(
        &mut entry,
        typed_trees::signature::StateParameter {
            type_reference,
            name: typed_trees::name::Identifier::generated("binding"),
            ..Default::default()
        },
    );
    let mut machine = typed_trees::machine::Machine {
        symbol: machine_symbol,
        name: typed_trees::name::Identifier::generated("Provider::run"),
        attached_data: Some(typed_trees::name::Identifier::generated("Provider")),
        ..Default::default()
    };
    typed.push_machine_state(&mut machine, entry);
    let machine_identity = typed
        .normalized_machine_overload_identity(&machine)
        .expect("checked invocation machine must have an entry overload")
        .identity();
    typed.push_machine(machine);

    let mut plan = selection_plan("provider", &["run"], &["run"]);
    plan.provider_type = "Provider".to_owned();
    plan.schema.trait_name = "pkg::Source".to_owned();
    plan.schema.methods[0].requirement_owner = "pkg::Source".to_owned();
    plan.schema.methods[0].parameter_count = parameter_count;
    plan.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity: None,
    };
    let inferred = flow_effects::InvocationInferencePlan {
        machines: vec![flow_effects::MachineInvocationInference {
            machine: machine_symbol,
            published: Vec::new(),
            inferred_direct: vec![flow_effects::InvocationTarget::Parameter(0)],
            inferred_transitive: vec![flow_effects::InvocationTarget::Parameter(0)],
            effective: vec![flow_effects::InvocationTarget::Parameter(0)],
        }],
    };
    (typed, plan, inferred)
}

#[derive(Clone, Copy, Debug)]
enum CheckedInvocationDrift {
    None,
    MissingOwner,
    DuplicateOwner,
    AbsentMachine,
    DuplicateMachine,
    AbsentInference,
    DuplicateInference,
    OutOfRangeParameter,
    UnknownParameterType,
    InvalidService,
    UnknownService,
    NonBoundaryService,
    DuplicateBoundarySymbol,
}

fn operator_coordinate_plan(name: &str, coordinate: &str, provider_type: &str) -> ProviderPlan {
    let mut plan = selection_plan(name, &["invoke"], &["invoke"]);
    plan.schema.trait_name = coordinate.to_owned();
    plan.provider_type = provider_type.to_owned();
    plan
}
