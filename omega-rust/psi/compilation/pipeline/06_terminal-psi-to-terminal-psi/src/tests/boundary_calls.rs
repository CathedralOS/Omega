//! Interface contracts must survive ordinary source preparation and the
//! actual call, including its borrowed inputs. These tests do not install a
//! provider or claim native execution.

use super::*;
use std::sync::Arc;

pub(super) fn typed(text: &str) -> typed_trees::TypedTrees {
    try_typed(text).expect("boundary-call fixture must pass source typing")
}

pub(super) fn try_typed(text: &str) -> Result<typed_trees::TypedTrees, diagnostics::Diagnostic> {
    // The boundary belongs to this fixture's user package. Its declared
    // owner is required by ordinary header publication; it grants no
    // toolchain identity or provider-execution authority.
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xbc; 32])
        .expect("nonempty fixture package identity");
    let mut sources = source::SourceMap::default();
    let source = sources
        .add_checked_instance(
            "boundary_calls/source.omg".into(),
            text,
            "boundary_calls".into(),
            Some(package),
            source::SourceOrigin::User,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
}

pub(super) fn entry(typed: &typed_trees::TypedTrees) -> symbols::SymbolHandle {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "entry")
        .unwrap()
        .symbol
}

fn invocation(typed: typed_trees::TypedTrees) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    // These are the package's actual declarations in this invocation's
    // activation scope. No provider is installed or executed by the test.
    let entry = entry(&typed);
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    typed_trees_to_terminal_psi::elaborate_for(
        &frozen,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,

            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

fn boundary_source(reach: &str, actual: u32) -> String {
    format!(
        r#"
boundary trait Probe {{
    machine read(value: u32) -> u32
    {reach}
    requires value <= 10
    ensures result <= 10
    crashes none;
}}
machine entry() -> u32 crashes none {{ Probe::read({actual}) }}
"#
    )
}

fn boundary(reach: &str, actual: u32) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let typed = typed(&boundary_source(reach, actual));
    invocation(typed)
}

#[test]
fn a_source_boundary_requires_an_explicit_reach_ceiling() {
    let text = boundary_source("", 7);
    let mut sources = source::SourceMap::default();
    let source = sources
        .add("boundary_calls/omitted.omg".into(), text.clone())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(&text)
        .tokenize()
        .unwrap();
    let refusal = match tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens) {
        Ok(_) => panic!("an open boundary reach contract must refuse"),
        Err(refusal) => refusal,
    };
    assert_eq!(
        refusal.message,
        "boundary-trait machine requirement `Probe::read` must declare an explicit `reaches` ceiling; write `reaches none` for empty reach"
    );
    let elaboration = boundary("reaches none", 7).unwrap();
    assert_eq!(elaboration.module.boundary_machines.len(), 1);
    let boundary = &elaboration.module.boundary_machines[0];
    assert!(boundary.owning_service.is_some());
    assert!(boundary.fixed_service_reach.is_empty());
    assert!(boundary.published_service_ceiling.is_empty());
}

#[test]
fn source_boundary_calls_keep_their_actual_input_obligations() {
    let elaboration = boundary("reaches none", 7).unwrap();
    let module = &elaboration.module;
    let [called] = module.boundary_machines.as_slice() else {
        panic!("the source calls exactly one ordinary requirement");
    };
    let service = module
        .services
        .iter()
        .find(|service| Some(service.id) == called.owning_service)
        .unwrap();
    let schema = module
        .service_schema(called.owner.identity(), service.identity)
        .unwrap();
    assert_eq!(schema.surface.methods.len(), 1);
    assert_eq!(
        module.identities.text(called.identity),
        Some(
            schema
                .surface
                .identity_text(schema.surface.methods[0].requirement_identity)
        )
    );
    let same = terminal_psi::TerminalServiceSchema::join(
        std::slice::from_ref(schema),
        std::slice::from_ref(schema),
    )
    .unwrap();
    assert_eq!(same.as_slice(), std::slice::from_ref(schema));
    let mut changed = schema.clone();
    Arc::make_mut(&mut changed.surface).methods[0].name = "other".into();
    assert!(
        terminal_psi::TerminalServiceSchema::join(std::slice::from_ref(schema), &[changed])
            .is_err()
    );
    assert!(
        terminal_psi::TerminalServiceSchema::join(&[schema.clone(), schema.clone()], &[]).is_err()
    );
    let mut changed = schema.clone();
    changed.application = terminal_psi::TerminalIdentityIndex::ABSENT;
    assert!(terminal_psi::TerminalServiceSchema::join(&[], &[changed]).is_err());
    assert!(matches!(
        judge(Ok(elaboration)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    let verdict = judge(boundary("reaches none", 11)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{verdict:?}"
    );
}

fn source_service_signature(typed: &typed_trees::TypedTrees, owner: &str) -> [u8; 32] {
    use typed_trees::service_declarations::{
        projection::from_typed, provider_declarations::ProviderPlanningContext,
    };
    let declaration = typed
        .traits()
        .iter()
        .find(|declaration| declaration.name.as_str() == owner)
        .unwrap();
    let surface = from_typed(
        ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        declaration,
    )
    .unwrap();
    let [method] = surface.declaration.methods.as_slice() else {
        panic!("the source declares one service method");
    };
    method.declared_type_signature
}

fn published_service_signature(elaboration: &typed_trees_to_terminal_psi::Elaboration) -> [u8; 32] {
    let [boundary] = elaboration.module.boundary_machines.as_slice() else {
        panic!("one requirement is called");
    };
    let source = &boundary.declaration.as_ref().unwrap().source;
    let parameters = source
        .parameter_types
        .iter()
        .map(|identity| elaboration.module.identities.text(*identity).unwrap());
    language_semantics::service_surface::declared_type_signature(
        parameters,
        elaboration
            .module
            .identities
            .text(source.result_type)
            .unwrap(),
    )
    .unwrap()
}

// Inherited methods retain their parent's actual type argument and declaration.
// Two paths applying that same parent differently cannot supply one schema.
#[test]
fn inherited_boundary_schemas_keep_declaring_applications_and_refuse_conflicts() {
    let source = r#"
boundary trait Parent<Value> {
    machine echo(value: Value) -> Value reaches none crashes none;
}
boundary trait Child: Parent<i32> {}
boundary trait Concrete {
    machine echo(value: i32) -> i32 reaches none crashes none;
}
machine entry() reaches Child {}
"#;
    let program = typed(source);
    assert_eq!(
        source_service_signature(&program, "Child"),
        source_service_signature(&program, "Concrete"),
        "the inherited signature must use i32, not its unapplied parent binder"
    );
    let parent = program
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Parent")
        .unwrap();
    let child = program
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Child")
        .unwrap();
    let projected = typed_trees::service_declarations::projection::from_typed(
        typed_trees::provider_declarations::ProviderPlanningContext::new(
            &program,
            source::DependencyScope::Product,
        ),
        child,
    )
    .unwrap();
    projected.validate_correspondence().unwrap();
    assert_eq!(projected.requirements.len(), 1);
    assert_eq!(projected.requirements[0].declaring_trait, parent.symbol);
    assert_eq!(
        projected.requirements[0].symbol,
        program.trait_machine_signatures(parent)[0].symbol
    );
    let form = |program: typed_trees::TypedTrees| {
        let child = program
            .traits()
            .iter()
            .find(|owner| owner.name.as_str() == "Child")
            .unwrap()
            .symbol;
        let entry = entry(&program);
        let frozen: typed_trees::FrozenTypedTrees = program.into();
        typed_trees_to_terminal_psi::elaborate_for(
            &frozen,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[child],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[child],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
    };
    // The generic comparison above checks the typed projection. Standalone
    // generic service publication remains unavailable. These nongeneric
    // parents match the real hosted entry and must also lower and check.
    let entry_program = typed(
        r#"
boundary trait Parent {
    machine echo(value: i32) -> i32 reaches none crashes none;
}
boundary trait Child: Parent {}
machine entry() reaches Child {}
"#,
    );
    let parent = entry_program
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Parent")
        .unwrap();
    let parent_symbol = parent.symbol;
    let required = entry_program.trait_machine_signatures(parent)[0].symbol;
    let elaboration = form(entry_program).expect("the inherited schema keeps its original parent");
    let parent_schema = elaboration
        .static_schemas
        .iter()
        .find(|schema| schema.declaration == parent_symbol)
        .expect("the parent still owns its requirement schema");
    assert_eq!(parent_schema.requirements.len(), 1);
    assert_eq!(parent_schema.requirements[0].declaration, required);
    assert!(matches!(
        judge(Ok(elaboration)).unwrap(),
        RouteVerdict::Accepted(_)
    ));

    let conflicting = source.replace(
        "boundary trait Child: Parent<i32> {}",
        "boundary trait Other: Parent<bool> {}
boundary trait Child: Parent<i32> + Other {}",
    );
    let conflicting = typed(&conflicting);
    let child = conflicting
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Child")
        .unwrap();
    let refused = match typed_trees::service_declarations::projection::from_typed(
        typed_trees::provider_declarations::ProviderPlanningContext::new(
            &conflicting,
            source::DependencyScope::Product,
        ),
        child,
    ) {
        Err(refused) => refused,
        Ok(_) => panic!("two different parent applications cannot become one schema"),
    };
    assert!(
        refused
            .message
            .contains("inherited with different applications"),
        "{refused:?}"
    );
}

#[test]
fn source_boundary_applied_signature_keeps_actual_types_and_caller_lifetimes() {
    use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
    use typed_trees::type_identity::{
        TypeIdentityRequest, descriptive_type_identity_in_region_scope,
    };
    let program = typed(
        r#"
boundary trait Write<'storage, Value> {
    machine write(value: &'storage mut Value) -> &'storage mut Value reaches none crashes none;
}
machine carrier<'left, 'right>(first: &'left mut i32, second: &'right mut i64) -> i64 { 0 }
"#,
    );
    let owner = program
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Write")
        .unwrap();
    let signature = &program.trait_machine_signatures(owner)[0];
    let caller = program
        .machines()
        .iter()
        .find(|caller| caller.name.as_str() == "carrier")
        .unwrap();
    let mut states = program.machine_states(caller).iter();
    let state = states.next().expect("one caller state");
    assert!(states.next().is_none(), "one caller state");
    let parameters = program.state_parameters(state);
    let scalar = |position: usize| {
        let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = program
            .type_reference_table
            .type_reference(parameters[position].type_reference)
        else {
            panic!("the actual caller declares a reference");
        };
        *referee
    };
    let applied = |value, lifetime: &str| {
        let scope = AppliedTraitScope::new(
            &program,
            owner,
            TraitApplicationArguments {
                lexical_owner: caller.symbol,
                enclosing_data_applications: &[],
                arguments: &[value],
                lifetimes: &[lifetime.into()],
            },
        )
        .unwrap();
        program
            .requirement_definition_types(&scope, signature)
            .unwrap()
    };
    let left_i32 = applied(scalar(0), "left");
    let left_i64 = applied(scalar(1), "left");
    let right_i32 = applied(scalar(0), "right");
    let caller_type = descriptive_type_identity_in_region_scope(
        &program,
        TypeIdentityRequest::package_qualified(parameters[0].type_reference),
        caller.symbol,
        caller.symbol,
        &[],
        &[],
    )
    .and_then(|identity| program.type_identity_text(identity))
    .unwrap();
    assert_eq!(left_i32.0, std::slice::from_ref(&caller_type));
    assert_eq!(
        left_i32.1, caller_type,
        "the result keeps the same captured lifetime"
    );
    assert_eq!(
        left_i32.2, 0,
        "a supplied lifetime introduces no elided binder"
    );
    assert_ne!(
        left_i32.0, left_i64.0,
        "the actual type argument must survive"
    );
    assert_ne!(
        left_i32.0, right_i32.0,
        "caller lifetimes must remain distinct"
    );
    assert!(
        !left_i32.0[0].contains("owner(schema)"),
        "a captured caller lifetime is not a new invocation binder"
    );
}

#[test]
fn source_boundary_top_level_schema_keeps_independent_input_lifetimes() {
    use typed_trees::service_declarations::{
        projection::from_typed_boundary_requirement, provider_declarations::ProviderPlanningContext,
    };
    let program = typed(
        r#"
data Host {}
boundary machine + Operations::write(left: &mut Host, right: &mut Host) reaches none crashes none;
machine entry() {}
"#,
    );
    let (program, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        program,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let requirement = program
        .machines()
        .iter()
        .find(|requirement| requirement.name.as_str() == "Operations::write")
        .unwrap();
    let surface = from_typed_boundary_requirement(
        ProviderPlanningContext::new(&program, source::DependencyScope::Product),
        requirement,
    )
    .unwrap();
    let (parameters, result) = program.machine_definition_types(requirement).unwrap();
    assert!(parameters[0].contains("region(owner(schema),ordinal(0))"));
    assert!(parameters[1].contains("region(owner(schema),ordinal(1))"));
    let expected =
        language_semantics::service_surface::declared_type_signature(parameters.iter(), &result)
            .unwrap();
    assert_eq!(
        surface.declaration.methods[0].declared_type_signature,
        expected
    );
}

#[test]
fn source_boundary_schema_and_header_share_elided_input_lifetimes() {
    let source = r#"
boundary trait Write {
    machine write(left: &mut i64, right: &mut i64)
    reaches none crashes none;
}
machine entry() {
    let mut left: i64 = 17;
    let mut right: i64 = 23;
    Write::write(&mut left, &mut right);
}
"#;
    let typed = typed(source);
    let expected = source_service_signature(&typed, "Write");
    let elaboration = invocation(typed).unwrap();
    assert_eq!(published_service_signature(&elaboration), expected);
}

#[test]
fn source_boundary_calls_retain_independent_input_lifetime_binders() {
    let source = r#"
boundary trait Write {
    machine write<'left, 'right>(left: &'left mut i64, right: &'right mut i64)
    reaches none crashes none;
}
machine entry() {
    let mut left: i64 = 17;
    let mut right: i64 = 23;
    Write::write(&mut left, &mut right);
}
"#;
    let typed = typed(source);
    let expected = source_service_signature(&typed, "Write");
    let elaboration = invocation(typed).unwrap();
    assert_eq!(published_service_signature(&elaboration), expected);
    let [boundary] = elaboration.module.boundary_machines.as_slice() else {
        panic!("one requirement is called");
    };
    let declaration = boundary.declaration.as_ref().unwrap();
    let [left, right] = declaration.source.parameter_types.as_slice() else {
        panic!("the public signature carries both borrowed parameters");
    };
    assert_ne!(left, right, "each input keeps its own lifetime");
    assert!(declaration.method_arguments.is_empty());
    assert!(declaration.source.specialization.is_none());
    let verdict = judge(Ok(elaboration)).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("independent mutable inputs must be accepted, got {verdict:?}");
    };
    accepted.publish_invocation().unwrap();
}

#[test]
fn source_boundary_contract_alternatives_keep_each_actual_argument() {
    for actual in [34603008, 36700160] {
        let source = boundary_source("reaches none", actual).replace(
            "requires value <= 10",
            "requires value == 34603008 || value == 36700160",
        );
        let verdict = judge(invocation(typed(&source))).unwrap();
        assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    }
    let source = boundary_source("reaches none", 0).replace(
        "requires value <= 10",
        "requires value == 34603008 || value == 36700160",
    );
    let verdict = judge(invocation(typed(&source))).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{verdict:?}",
    );
}

/// A boundary requirement may take a type parameter of its own. It has no
/// body, so a call that applies it, `Probe::take<Reading>(..)`, is held to
/// the requirement's signature and contract with `Reading` where it named
/// `Item` (generics.md, "Closed applications and canonical identity"). Typed
/// preparation closes the requirement once per application, as it generates
/// one machine per application of a generic machine, so the program is read
/// after that stage.
fn applied_boundary_prepared(limit: u32) -> typed_trees::FrozenTypedTrees {
    let typed = typed(&format!(
        r#"
data Reading {{ level: u32; }}
boundary trait Probe {{
    machine take<Item>(item: Item, limit: u32) -> u32
    reaches none
    requires limit <= 10
    ensures result <= 10
    crashes none;
}}
machine entry() -> u32 crashes none {{
    let first: u32 = Probe::take<u32>(7, {limit});
    let room: u32 = 10u32 - first;
    let second: u32 = Probe::take<Reading>(Reading {{ level: 3 }}, 4);
    let third: u32 = Probe::take<u32>(9, 2);
    room
}}
"#
    ));
    let (frozen, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    frozen
}

fn read_applied_boundary(
    frozen: &typed_trees::FrozenTypedTrees,
) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let entry = entry(frozen);
    typed_trees_to_terminal_psi::elaborate_for(
        frozen,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,

            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

fn applied_boundary(limit: u32) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    read_applied_boundary(&applied_boundary_prepared(limit))
}

#[test]
fn a_boundary_requirement_is_judged_at_the_type_a_call_applies_it_at() {
    let elaboration = applied_boundary(10).unwrap();
    // `take` at `u32` and at `Reading` are two declarations of one
    // requirement; the two calls at `u32` name one of them.
    let boundaries = &elaboration.module.boundary_machines;
    assert_eq!(boundaries.len(), 2);
    let declared: Vec<_> = boundaries
        .iter()
        .map(|boundary| boundary.declaration.as_ref().unwrap())
        .collect();
    assert!(declared.iter().all(|declared| {
        declared.method_arguments.len() == 1 && declared.source.specialization.is_some()
    }));
    assert_ne!(
        declared[0]
            .source
            .specialization
            .as_ref()
            .unwrap()
            .commitment,
        declared[1]
            .source
            .specialization
            .as_ref()
            .unwrap()
            .commitment
    );
    // What the requirement guarantees at `u32` keeps `10 - first` exact.
    assert!(matches!(
        judge(Ok(elaboration)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    // The twin differs only in one argument: `limit` of 11 breaks
    // `limit <= 10`.
    let verdict = judge(applied_boundary(11)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{verdict:?}"
    );

    // Separate authored calls at the same tuple share a closed signature,
    // but an application at another tuple must never authenticate that call.
    let mut typed = applied_boundary_prepared(10).into_inner();
    let calls = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            typed.closed_requirement_application(call.target_symbol)?;
            Some((
                handle,
                call.target_symbol,
                call.authored_machine_application,
            ))
        })
        .collect::<Vec<_>>();
    let [first, second, third] = calls.as_slice() else {
        panic!("three authored applications of the boundary requirement");
    };
    assert_eq!(first.1, third.1);
    assert_ne!(first.2, third.2);
    assert_ne!(first.1, second.1);
    assert!(
        typed
            .require_closed_requirement_call_application(first.1, third.2)
            .is_ok()
    );
    let stale = typed_trees::static_applications::StaticMachineApplicationHandle::from_parts(
        first.2.arena_index(),
        first.2.generation() + 1,
    );
    assert_eq!(
        typed.require_closed_requirement_call_application(first.1, stale),
        Err("closed requirement call has a stale declaration or application"),
    );
    let original = typed
        .expression_table
        .static_machine_application_bindings(third.2)[0]
        .value
        .clone();
    let changed = typed
        .expression_table
        .static_machine_application_bindings(second.2)[0]
        .value
        .clone();
    typed
        .expression_table
        .static_machine_application_bindings_mut(third.2)[0]
        .value = changed;
    assert_eq!(
        typed.require_closed_requirement_call_application(first.1, third.2),
        Err("closed requirement call argument differs from its retained binding"),
    );
    let typed_trees::static_applications::StaticMachineBindingValue::Type(reference) = &original
    else {
        panic!("the original application binds its type parameter");
    };
    let stale_reference = typed_trees::types::TypeReferenceHandle::from_parts(
        reference.arena_index(),
        reference.generation() + 1,
    );
    typed
        .expression_table
        .static_machine_application_bindings_mut(third.2)[0]
        .value = typed_trees::static_applications::StaticMachineBindingValue::Type(stale_reference);
    assert_eq!(
        typed.require_closed_requirement_call_application(first.1, third.2),
        Err("closed requirement call has a stale operand input"),
    );
    typed
        .expression_table
        .static_machine_application_bindings_mut(third.2)[0]
        .value = original;
    assert_eq!(
        typed.require_closed_requirement_call_application(first.1, second.2),
        Err("closed requirement call selects another bound application"),
    );
    let typed_trees::expression::ExpressionNode::Call(call) =
        typed.expression_table.expression_mut(first.0)
    else {
        panic!("the retained source occurrence is a call");
    };
    call.authored_machine_application = second.2;
    let refusal = read_applied_boundary(&typed.into()).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(
        refusal.detail,
        "call closed requirement call selects another bound application"
    );
}

/// A boundary requirement with a value parameter, `take<const Count: u64>`,
/// whose contract compares an `i32` formal with that `u64` parameter. Typed
/// preparation closes it once per application, so `take<4>` has an array
/// formal of four bytes and the clause `used <= 4`. `calls` is the entry's
/// body; it may apply the requirement or not.
fn value_generic_boundary(calls: &str) -> typed_trees::FrozenTypedTrees {
    let typed = typed(&format!(
        r#"
boundary trait Sink {{
    machine take<const Count: u64>(items: &[u8; Count], used: i32) -> u32
    reaches none
    requires used <= Count
    crashes none;
}}
machine entry() -> u32 crashes none {{
    let buffer: [u8; 4] = [0, 0, 0, 0];
    {calls}
}}
"#
    ));
    typed_trees_to_typed_trees::prepare_typed_trees(
        typed,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap()
    .0
}

/// What a native binding of `Sink::take` claims: the generic requirement,
/// named as the Build names it.
fn claimed(
    frozen: &typed_trees::FrozenTypedTrees,
    requirement: symbols::SymbolHandle,
) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let entry = entry(frozen);
    typed_trees_to_terminal_psi::elaborate_for(
        frozen,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[requirement],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

fn sink_requirement(frozen: &typed_trees::FrozenTypedTrees) -> symbols::SymbolHandle {
    let sink = frozen
        .traits()
        .iter()
        .find(|declared| declared.name.as_str() == "Sink")
        .expect("the program declares `Sink`");
    frozen.trait_machine_signatures(sink)[0].symbol
}

/// numeric_values.md, "Integer equality and order": integers of different
/// types compare by their exact values, in contracts too. A native binding
/// claims each closed application of a value-generic requirement, because
/// only those have a contract with literal array lengths, and claims nothing
/// when nothing applies it.
#[test]
fn a_value_generic_boundary_is_claimed_and_judged_at_each_application() {
    let applied = value_generic_boundary("Sink::take<4>(&buffer, 3)");
    let requirement = sink_requirement(&applied);
    let elaboration = claimed(&applied, requirement).unwrap();
    // The claim and the call name the one closed application.
    assert_eq!(elaboration.module.boundary_machines.len(), 1);
    let verdict = judge(Ok(elaboration)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");

    // The twin differs in one argument: an `i32` of 5 is past the `u64` 4.
    let verdict = judge(claimed(
        &value_generic_boundary("Sink::take<4>(&buffer, 5)"),
        requirement,
    ))
    .unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{verdict:?}"
    );

    // With no application there is no contract to read, so the claim
    // publishes no boundary; a consumer's call would close its own.
    let unapplied = value_generic_boundary("7u32");
    let elaboration = claimed(&unapplied, sink_requirement(&unapplied)).unwrap();
    assert!(elaboration.module.boundary_machines.is_empty());
}

#[test]
fn a_closed_boundary_application_does_not_authorize_an_unrelated_selection() {
    use typed_trees::expression::ExpressionNode;

    let mut prepared = applied_boundary_prepared(10).into_inner();
    let unrelated = entry(&prepared);
    let expression = prepared
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| match node {
            ExpressionNode::Call(call)
                if prepared
                    .closed_requirement_application(call.target_symbol)
                    .is_some() =>
            {
                Some(expression)
            }
            _ => None,
        })
        .expect("the prepared entry calls a closed boundary requirement");
    let ExpressionNode::Call(call) = prepared.expression_table.expression(expression) else {
        unreachable!()
    };
    let source = prepared
        .authored_declaration_selections()
        .get(call.authored_call_selection.unwrap())
        .unwrap()
        .source_span();
    let selection = prepared
        .record_resolved_authored_declaration_selection_once(
            source,
            typed_trees::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            typed_trees::AuthoredDeclarationSelectionKind::Call,
            unrelated,
        )
        .unwrap();
    let ExpressionNode::Call(call) = prepared.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    call.authored_call_selection = Some(selection);
    let refusal = read_applied_boundary(&prepared.into()).unwrap_err();
    assert!(
        refusal
            .detail
            .contains("selection resolved to another declaration"),
        "{refusal:?}"
    );
}

#[test]
fn changed_boundary_effects_must_match_the_published_call_envelope() {
    for blocks in [false, true] {
        let mut elaboration = boundary("reaches none", 7).unwrap();
        let declaration = &mut elaboration.module.boundary_machines[0]
            .declaration
            .as_mut()
            .unwrap()
            .source;
        declaration.published_may_block = blocks;
        declaration.published_may_suspend = !blocks;
        let refusal =
            discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap_err();
        assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
        assert_eq!(
            refusal.detail,
            "published operational contract differs from its declared or inferred envelope"
        );
    }
}

#[test]
fn source_boundary_effects_still_require_their_invocation_proof() {
    for (effect, acknowledgement) in [("suspends", "suspend"), ("blocks", "block")] {
        // The source declares the effect; no mutation of the published
        // metadata supplies it. This ordinary call has no operational proof.
        let text = boundary_source(&format!("reaches none {effect}"), 7);
        let mut elaboration = invocation(typed(&text))
            .expect("the declared boundary contract reaches invocation checking");
        let published = &elaboration.module.boundary_machines[0]
            .declaration
            .as_ref()
            .unwrap()
            .source;
        assert_eq!(published.published_may_suspend, effect == "suspends");
        assert_eq!(published.published_may_block, effect == "blocks");
        let refusal =
            discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap_err();
        assert_eq!(refusal.reason, RefusalReason::Violation);
        let site = refusal
            .source_span
            .expect("the authored call's operational violation");
        let call = text.rfind("read(").expect("the authored boundary call");
        assert_eq!(site.span, source::Span::new(call, call + "read".len()));
        assert_eq!(
            refusal.detail,
            format!("call may {acknowledgement} and requires a {acknowledgement} acknowledgement")
        );
    }
}

#[test]
fn source_boundary_crashes_are_not_erased_by_the_call_operand_shape() {
    let call = |actual| {
        let typed = typed(&format!(
            r#"
boundary trait Probe {{
    machine read(value: u32) -> u32
    reaches none
    crashes Trap value == 0;
}}
machine entry() -> u32 crashes none {{ Probe::read({actual}) }}
"#
        ));
        invocation(typed)
    };
    assert!(matches!(judge(call(1)).unwrap(), RouteVerdict::Accepted(_)));
    let verdict = judge(call(0)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CrashCoverage { .. }))),
        "{verdict:?}"
    );
}

/// Both total carrier arithmetic and mathematical arithmetic are restated at
/// the call, rather than rejected by a narrower crash-only term reader.
#[test]
fn source_crash_guards_substitute_total_operations_at_the_call() {
    for guard in ["value +% 1 == 0", "embed(value) + 1 == 4294967296"] {
        let call = |actual| {
            invocation(typed(&format!(
                r#"
boundary trait Probe {{
    machine read(value: u32) -> u32
    reaches none
    crashes Trap {guard};
}}
machine entry() -> u32 crashes none {{ Probe::read({actual}) }}
"#
            )))
        };
        let accepted = judge(call(1)).unwrap();
        let RouteVerdict::Accepted(accepted) = accepted else {
            panic!("{guard}: {accepted:?}");
        };
        accepted.publish_invocation().unwrap();
        let rejected = judge(call(u32::MAX)).unwrap();
        assert!(
            matches!(rejected, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CrashCoverage { .. }))),
            "{guard}: {rejected:?}"
        );
    }
}

fn borrowed_requirement(
    second: &str,
    caller_bound: &str,
) -> typed_trees_to_terminal_psi::Elaboration {
    let typed = typed(&format!(
        r#"
trait Touch {{
    machine write(left: &mut u32, right: &mut u32)
    requires left < 10
    crashes none;
}}
machine entry<Selected satisfies Touch>(left: &mut u32, right: &mut u32)
requires left {caller_bound}
crashes none
{{
    Selected::write(left, {second});
}}
"#
    ));
    typed_trees_to_terminal_psi::elaborate_generic(&typed, entry(&typed)).unwrap()
}

#[test]
fn borrowed_generic_requirements_preserve_input_proofs_and_exclusive_loans() {
    let mut valid = borrowed_requirement("right", "< 10");
    let discharged = discharge(&mut valid, &mut crate::ProofSearchActivity::default()).unwrap();
    let entry = discharged.machines.get(id(1)).unwrap();
    assert!(entry.conflicts.is_empty());
    assert!(entry.unestablished().next().is_none());
    assert!(
        entry.verdicts.values().all(Result::is_ok),
        "{:?}",
        entry.verdicts
    );

    let mut missing_input = borrowed_requirement("right", "<= 10");
    let discharged = discharge(
        &mut missing_input,
        &mut crate::ProofSearchActivity::default(),
    )
    .unwrap();
    assert!(
        discharged
            .machines
            .get(id(1))
            .unwrap()
            .verdicts
            .values()
            .any(Result::is_err)
    );

    let mut alias = borrowed_requirement("left", "< 10");
    let discharged = discharge(&mut alias, &mut crate::ProofSearchActivity::default()).unwrap();
    assert!(!discharged.machines.get(id(1)).unwrap().conflicts.is_empty());
}

fn conformer_contracts(member: &str, plain: &str) -> Vec<RouteVerdict> {
    let text = format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0
    ensures result < value
    crashes none;
}}
Previous satisfies Step {{
    {member}
}}
{plain}
machine entry() crashes none {{}}
"#
    );
    let typed = typed(&text);
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[None],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    verdicts.into_iter().map(Result::unwrap).collect()
}

#[test]
fn inline_conformers_inherit_the_requirements_logical_contract() {
    // The member deliberately uses another formal name: inherited clauses
    // refer to the same logical input by position, not by its spelling.
    let verdicts = conformer_contracts(
        "machine apply(input: u32) -> u32 crashes none { input - 1 }",
        "",
    );
    for verdict in verdicts {
        assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    }
}

#[test]
fn an_omitted_member_guarantee_still_has_to_hold_of_the_actual_body() {
    let verdicts = conformer_contracts(
        "machine apply(input: u32) -> u32 crashes none { input }",
        "",
    );
    assert!(
        verdicts.iter().all(|verdict| matches!(
            verdict,
            RouteVerdict::Accepted(_) | RouteVerdict::Rejected { .. }
        )),
        "{verdicts:?}"
    );
    assert!(
        verdicts.iter().any(|verdict| matches!(
            verdict, RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))
        )),
        "{verdicts:?}"
    );
}

#[test]
fn a_requirement_does_not_replace_written_inputs_or_donate_them_to_a_selection() {
    for (member, plain) in [
        // The originating requirement admits one, so this omission must
        // not infer the narrower input >= 2 that a private helper could.
        (
            "machine apply(input: u32) -> u32 crashes none { input - 2 }",
            "",
        ),
        (
            "machine apply(input: u32) -> u32 requires true crashes none { input - 1 }",
            "",
        ),
        (
            "Step::apply = previous;",
            "machine previous(input: u32) -> u32 requires true crashes none { input - 1 }",
        ),
    ] {
        let verdicts = conformer_contracts(member, plain);
        assert!(
            verdicts.iter().all(|verdict| matches!(
                verdict,
                RouteVerdict::Accepted(_) | RouteVerdict::Rejected { .. }
            )),
            "{verdicts:?}"
        );
        assert!(verdicts.iter().any(|verdict| matches!(
            verdict, RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
        )), "{verdicts:?}");
    }
}

fn declared_source(contract: &str, body: &str, actual: &str, promised: &str) -> String {
    format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0
    ensures result < value
    crashes none;
}}
machine previous(input: u32) -> u32 satisfies Step::apply
{contract}
crashes none
{{ {body} }}
machine entry() -> u32
{promised}
crashes none
{{ previous({actual}) }}
"#
    )
}

fn declared_verdicts(text: &str) -> Vec<RouteVerdict> {
    compilation_verdicts_of(&typed(text))
}

fn compilation_verdicts_of(typed: &typed_trees::TypedTrees) -> Vec<RouteVerdict> {
    compilation_results_of(typed)
        .into_iter()
        .map(Result::unwrap)
        .collect()
}

pub(super) fn compilation_results_of(
    typed: &typed_trees::TypedTrees,
) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let entry = entry(typed);
    let prepared = typed_trees_to_terminal_psi::prepare_compilation(
        typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .program;
    // Every prepared proof module is judged, including independent body and
    // applied-selection witnesses. Their number is not the number of source
    // conformers; the behavioral assertions below apply to every verdict.
    let expected_verdicts = match &prepared {
        terminal_check_tables::compilation::TerminalCompilation::Prepared(program) => {
            1 + program.forms.len() + program.witnesses.len()
        }
        _ => 1,
    };
    let verdicts = crate::judge_compilation(prepared);
    assert_eq!(verdicts.len(), expected_verdicts);
    verdicts
}

#[test]
fn a_declared_conformers_name_invokes_its_requirement() {
    let text = declared_source("", "input - 1", "7", "ensures result < 7");
    let mut verdicts = declared_verdicts(&text);
    for verdict in &verdicts {
        assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    }
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert_eq!(
        status,
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        32
                    )
                    .unwrap(),
                    value: semantic_vocabulary::IntegerValue::Unsigned(6),
                }
            )
        )
    );
}

#[test]
fn a_declared_conformers_written_body_contract_is_independently_owed() {
    for (contract, body, expected) in [("", "input", false), ("requires true", "input - 1", true)] {
        let verdicts = declared_verdicts(&declared_source(contract, body, "7", ""));
        assert!(
            matches!(&verdicts[1], RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| if expected {
                matches!(row.kind, ObligationKind::OperationSafety { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            })),
            "{verdicts:?}"
        );
    }
}

#[test]
fn a_conformer_has_no_callable_direct_contract_beside_its_requirement() {
    for (actual, promised, input_failure) in [("0", "", true), ("7", "ensures result == 0", false)]
    {
        let verdicts = declared_verdicts(&declared_source(
            "requires true ensures result == 0",
            "0",
            actual,
            promised,
        ));
        assert!(
            matches!(&verdicts[1], RouteVerdict::Accepted(_)),
            "{verdicts:?}"
        );
        assert!(
            matches!(&verdicts[0], RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| if input_failure {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            })),
            "{verdicts:?}"
        );
    }
}

#[test]
fn recursive_named_calls_in_a_body_witness_keep_the_requirement_inputs() {
    // A postcondition on a call cycle has no served inference rule. This
    // control isolates the named recursive call's input obligation.
    let text = declared_source("requires true", "previous(0)", "7", "")
        .replace("ensures result < value", "");
    let typed = typed(&text);
    let member = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "previous")
        .unwrap();
    let [edge] = typed.machine_trait_conformances(member) else {
        panic!("one declared edge")
    };
    let mut witness = typed_trees_to_terminal_psi::elaborate_witness(
        &typed,
        edge.symbol,
        edge.requirement_symbol,
        &[],
        member.symbol,
    )
    .unwrap();
    let discharged = discharge(&mut witness, &mut crate::ProofSearchActivity::default()).unwrap();
    let body = id(2);
    let tables = witness.tables.machines.get(body).unwrap();
    let judgments = discharged.machines.get(body).unwrap();
    assert!(
        tables.obligations.iter().any(|(obligation, row)| matches!(
            row.kind,
            ObligationKind::CallRequirement { .. }
        ) && judgments
            .verdicts
            .get(obligation)
            .is_some_and(Result::is_err)),
        "{judgments:?}"
    );
}

#[test]
fn a_boundary_conformer_is_not_a_named_direct_entry() {
    let text = declared_source("", "input - 1", "7", "")
        .replacen("trait Step", "boundary trait Step", 1)
        .replacen("requires value > 0", "reaches none requires value > 0", 1);
    let typed = typed(&text);
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refusal))]
        if refusal.detail.contains("boundary conformer is callable only through its provider slot")),
        "{verdicts:?}"
    );
}

#[test]
fn a_declared_conformers_call_keeps_the_requirement_reach_ceiling() {
    let typed = typed(
        r#"
boundary trait Service {}
trait Read {
    machine value() -> u32 reaches Service crashes none;
}
machine chosen() -> u32 satisfies Read::value crashes none { 7 }
machine entry() -> u32 reaches none crashes none { chosen() }
"#,
    );
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    assert!(
        matches!(&verdicts[0], Ok(RouteVerdict::Rejected { exceeded, .. })
        if !exceeded.is_empty()),
        "{verdicts:?}"
    );
    assert!(
        matches!(&verdicts[1], Ok(RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
}

#[test]
fn named_selected_calls_keep_their_public_reach_ceiling() {
    // A literal-returning implementation still contributes the public service
    // ceiling at its selected call. Only the caller's written ceiling changes.
    for (ceiling, accepts) in [("Service", true), ("none", false)] {
        let typed = typed(&format!(
            "boundary trait Service {{}}
             trait Read {{ machine value() -> u32 reaches Service crashes none; }}
             Named satisfies Read {{
                 machine value() -> u32 reaches none crashes none {{ 7 }}
             }}
             machine entry() -> u32 reaches {ceiling} crashes none {{ Named::value() }}"
        ));
        let entry = entry(&typed);
        let verdicts = crate::judge_compilation(
            typed_trees_to_terminal_psi::prepare_compilation(
                &typed,
                typed_trees_to_terminal_psi::Compilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[entry],
                    subjects: &[],
                    purpose: source::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .program,
        );
        if accepts {
            let Ok(RouteVerdict::Accepted(accepted)) = &verdicts[0] else {
                panic!("the written public ceiling covers the selected call: {verdicts:?}")
            };
            let module = &accepted.elaboration.module;
            let call = module.machines[0].module_invocations().next().unwrap();
            let contract = module
                .invocation_machine_contract(call.inputs.callee, call.inputs.required_contract)
                .unwrap();
            assert!(call.inputs.required_contract.is_some());
            assert_eq!(contract.published_service_ceiling.len(), 1);
            assert_eq!(
                module.machines[0].published_service_ceiling,
                contract.published_service_ceiling
            );
        } else {
            assert!(
                matches!(&verdicts[0], Ok(RouteVerdict::Rejected { exceeded, .. })
                    if !exceeded.is_empty()),
                "{verdicts:?}"
            );
        }
    }
}

#[test]
fn a_declared_conformer_cannot_be_rebound_by_a_selection_row() {
    let text = declared_source("", "input - 1", "7", "")
        + r#"
trait Other {
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value crashes none;
}

Rebound satisfies Other { Other::apply = previous; }
"#;
    let typed = typed(&text);
    let entry = entry(&typed);
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[None],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refusal))]
        if refusal.detail.contains("selection row cannot rebind a declared conformer")),
        "the existing bound entry must not acquire another route"
    );
    // Reached dependency obligations use the same witness admission even
    // when this compilation does not claim that declaration as a subject.
    let other = typed
        .traits()
        .iter()
        .find(|declared| declared.name.as_str() == "Other")
        .unwrap();
    let requirement = &typed.trait_machine_signatures(other)[0];
    let member = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "previous")
        .unwrap();
    let Err(refusal) = typed_trees_to_terminal_psi::elaborate_witness(
        &typed,
        other.symbol,
        requirement.symbol,
        &[],
        member.symbol,
    ) else {
        panic!("matching text cannot rebind an existing nominal route");
    };
    assert!(
        refusal
            .detail
            .contains("conformance witness cannot rebind a declared requirement")
    );
}

#[test]
fn a_reference_row_cannot_rebind_an_instantiated_requirement() {
    for (selected_type, accepts) in [("u32", true), ("u64", false)] {
        let text = format!(
            r#"
trait G<T> {{
    machine go(value: u32) -> u32 reaches none crashes none;
}}
Owner satisfies G<u32> {{
    machine go(value: u32) -> u32 reaches none crashes none {{ value }}
}}
Selected satisfies G<{selected_type}> {{ G::go = Owner::go; }}
machine entry() -> u32 reaches none crashes none {{ 7 }}
"#
        );
        let typed = typed(&text);
        let verdicts = crate::judge_compilation(
            typed_trees_to_terminal_psi::prepare_compilation(
                &typed,
                typed_trees_to_terminal_psi::Compilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[entry(&typed)],
                    subjects: &[None],
                    purpose: source::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .program,
        );
        if accepts {
            assert!(
                verdicts
                    .iter()
                    .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
                "{verdicts:?}"
            );
        } else {
            let [Ok(RouteVerdict::FallBack(refusal))] = verdicts.as_slice() else {
                panic!("equal signatures cannot rebind another trait application");
            };
            assert!(
                refusal
                    .detail
                    .contains("selection row cannot rebind a declared conformer"),
                "{refusal:?}"
            );
        }
    }
}

#[test]
fn an_own_progress_promise_does_not_change_a_silent_requirement_contract() {
    let mut published = Vec::new();
    for contract in ["", "terminates"] {
        let verdicts = declared_verdicts(&declared_source(contract, "input - 1", "7", ""));
        assert!(
            verdicts
                .iter()
                .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_)))
        );
        let RouteVerdict::Accepted(closure) = &verdicts[0] else {
            unreachable!()
        };
        let member = closure.elaboration.module.machine(id(2)).unwrap();
        published.push(member.declaration.as_ref().unwrap().contract);
    }
    assert_eq!(
        published[0], published[1],
        "only the requirement's advertised progress belongs in the routed contract"
    );
}

fn inline_parameter_prepared(actual: &str, guarantee: &str) -> typed_trees::FrozenTypedTrees {
    let text = format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value crashes none;
}}
Inline satisfies Step {{
    machine apply(input: u32) -> u32
    requires true ensures result == 0 crashes none {{ 0 }}
}}
machine relay<Chosen satisfies Step>() -> u32
requires true
{guarantee}
crashes none
{{ Chosen::apply({actual}) }}
machine entry() -> u32 crashes none {{ relay<Inline>() }}
"#
    );
    // Use the actual specialization pass, retaining the exact source
    // parameter dispatch and selected realization it produces.
    let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed(&text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    prepared
}

fn inline_parameter_read(
    prepared: &typed_trees::FrozenTypedTrees,
    publication: terminal_codec::TerminalPublication,
) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let entry = entry(prepared);
    typed_trees_to_terminal_psi::elaborate_for(
        prepared,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,

            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[None],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        publication,
    )
}

fn inline_parameter_dispatch(
    actual: &str,
    guarantee: &str,
) -> typed_trees_to_terminal_psi::Elaboration {
    let prepared = inline_parameter_prepared(actual, guarantee);
    let elaboration =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation).unwrap();
    assert_eq!(
        elaboration.selections.len(),
        1,
        "one genuinely prepared parameter dispatch"
    );
    let selected = &elaboration.selections[0];
    assert!(
        !elaboration.local_results.contains(&selected.implementation),
        "the selected requirement does not expose private body summaries"
    );
    elaboration
}

#[test]
fn an_owned_inline_parameter_dispatch_owes_the_requirement_inputs() {
    for (actual, accepted) in [("7", true), ("0", false)] {
        let mut elaboration = inline_parameter_dispatch(actual, "");
        let caller = elaboration.selections[0].caller;
        let found =
            discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap();
        let tables = elaboration.tables.machines.get(caller).unwrap();
        let judgments = found.machines.get(caller).unwrap();
        let calls: Vec<_> = tables
            .obligations
            .iter()
            .filter(|(_, row)| matches!(row.kind, ObligationKind::CallRequirement { .. }))
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "own requires true must not erase the requirement input"
        );
        assert_eq!(
            judgments.verdicts.get(calls[0].0).unwrap().is_ok(),
            accepted
        );
    }
}

#[test]
fn an_owned_inline_parameter_dispatch_exposes_only_the_requirement_guarantee() {
    let mut elaboration = inline_parameter_dispatch("7", "ensures result == 0");
    let caller = elaboration.selections[0].caller;
    let found = discharge(&mut elaboration, &mut crate::ProofSearchActivity::default()).unwrap();
    let tables = elaboration.tables.machines.get(caller).unwrap();
    let judgments = found.machines.get(caller).unwrap();
    assert!(
        tables.obligations.iter().any(|(obligation, row)| matches!(
            row.kind,
            ObligationKind::NormalGuarantee { .. }
        ) && judgments
            .verdicts
            .get(obligation)
            .is_some_and(Result::is_err)),
        "the implementation's stronger result == 0 is not the routed guarantee"
    );
}

#[test]
fn an_owned_inline_parameter_dispatch_keeps_exact_selection_checks() {
    use typed_trees::expression::ExpressionNode;

    for (corruption, expected) in [
        (0, "requirement dispatch with no requirement"),
        (1, "static requirement dispatch"),
        (2, "requirement dispatch that targets another machine"),
    ] {
        let mut prepared = inline_parameter_prepared("7", "").into_inner();
        let expression = prepared
            .expression_table
            .iter_expressions()
            .find_map(|(expression, node)| {
                matches!(node, ExpressionNode::Call(call) if call.static_requirement_dispatch.is_some())
                    .then_some(expression)
            })
            .expect("one generated requirement dispatch");
        let ExpressionNode::Call(call) = prepared.expression_table.expression_mut(expression)
        else {
            panic!("selected call");
        };
        let dispatch = call.static_requirement_dispatch.as_mut().unwrap();
        match corruption {
            0 => dispatch.requirement = Default::default(),
            1 => dispatch.evidence_parameter = Default::default(),
            2 => dispatch.realization_state = Default::default(),
            _ => unreachable!(),
        }
        let prepared = prepared.into();
        let Err(refusal) =
            inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation)
        else {
            panic!("a changed selection must refuse: {expected}");
        };
        assert!(refusal.detail.contains(expected), "{refusal:?}");
    }
}

#[test]
fn an_anonymous_parameter_application_cannot_transfer_its_template_identity() {
    let prepared = inline_parameter_prepared("7", "");
    let first =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation).unwrap();
    let second =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation).unwrap();
    let template = |elaboration: &typed_trees_to_terminal_psi::Elaboration| {
        elaboration
            .module
            .machines
            .iter()
            .find_map(|machine| machine.declaration.as_ref()?.specialization.as_ref())
            .unwrap()
            .template
            .clone()
    };
    let first_template = template(&first);
    let second_template = template(&second);
    assert!(
        first
            .module
            .identities
            .is_invocation_local(first_template.identity())
    );
    assert!(
        !first_template.same_identity_as(
            &first.module.identities,
            &second_template,
            &second.module.identities,
        ),
        "equal source bytes do not transfer an invocation's declaration authority"
    );
    let Err(refusal) =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Portable)
    else {
        panic!("anonymous specialization must not acquire a hermetic package owner");
    };
    // Portable publication needs a real source owner before the anonymous
    // specialization can even ask for a hermetic template identity.
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(refusal.concerns, terminal_check_tables::Construct::Machine);
    assert!(
        refusal
            .detail
            .contains("no exact package or toolchain owner"),
        "{refusal:?}"
    );
}

#[test]
fn an_anonymous_parameter_application_rejects_a_foreign_template_handle() {
    let mut prepared = inline_parameter_prepared("7", "").into_inner();
    let template = prepared.machine_specializations[0].template;
    prepared.machine_specializations.edit(|rows| {
        rows[0].template =
            symbols::SymbolHandle::from_parts(template.arena_index(), template.generation() + 1)
    });
    assert!(
        !prepared
            .symbols
            .symbols()
            .nodes()
            .is_valid(prepared.machine_specializations[0].template)
    );
    let prepared = prepared.into();
    let Err(refusal) =
        inline_parameter_read(&prepared, terminal_codec::TerminalPublication::Invocation)
    else {
        panic!("a foreign generation cannot supply the specialization's template");
    };
    assert!(
        refusal
            .detail
            .contains("executable application lost its template machine"),
        "{refusal:?}"
    );
}

fn required_progress_source(body: &str) -> String {
    format!(
        r#"
trait Finish {{
    machine finish() -> u32 terminates crashes none;
}}
machine finish() -> u32 satisfies Finish::finish crashes none {{ {body} }}
machine entry() -> u32 terminates crashes none {{ finish() }}
"#
    )
}

#[test]
fn a_declared_conformer_inherits_a_bare_required_termination_promise() {
    let mut verdicts = declared_verdicts(&required_progress_source("7"));
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(7),
                        ..
                    }
                )
            )
        ),
        "{status:?}"
    );
}

#[test]
fn an_inherited_termination_promise_does_not_validate_an_unranked_cycle() {
    let source = required_progress_source("finish()");
    // The second caller supplies no separate progress obligation. The
    // callee must still keep the promise inherited from its requirement.
    let silent_caller = source.replacen(
        "machine entry() -> u32 terminates",
        "machine entry() -> u32",
        1,
    );
    assert_ne!(source, silent_caller);
    for source in [&source, &silent_caller] {
        let Err(refusal) = invocation(typed(source)) else {
            panic!("the requirement promise is owed by the actual recursive body");
        };
        assert_eq!(refusal.reason, RefusalReason::Unsupported);
        assert!(
            refusal
                .detail
                .contains("termination promise over a call cycle"),
            "{refusal:?}"
        );
    }
}

fn named_owned_source(actual: &str, promised: &str, body: &str) -> String {
    format!(
        r#"
trait Step {{
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value crashes none;
}}
Inline satisfies Step {{
    machine apply(input: u32) -> u32
    requires true ensures result == 0 crashes none {{ {body} }}
}}
machine entry() -> u32 {promised} crashes none {{ Inline::apply({actual}) }}
"#
    )
}

#[test]
fn a_named_owned_inline_member_calls_through_its_requirement() {
    let text = named_owned_source("7", "", "0");
    let prepared = typed(&text);
    for node in prepared.expression_table.expression_nodes() {
        if let typed_trees::expression::ExpressionNode::Call(call) = node {
            let selection = prepared
                .authored_declaration_selections()
                .get(call.authored_call_selection.unwrap())
                .unwrap();
            assert!(
                matches!(selection.target(), typed_trees::AuthoredDeclarationSelectionTarget::Resolved(selected)
                if selected.selected_symbol() == call.target_symbol),
                "the public requirement must be closed before lowering"
            );
            let namespace = prepared
                .expression_table
                .name_path_node(call.receiver)
                .unwrap()
                .symbol;
            let (owner, row, _) = prepared
                .named_conformance_member(namespace, call.target_symbol)
                .unwrap()
                .unwrap();
            assert_eq!(owner.symbol, namespace);
            assert_eq!(row.requirement, call.target_symbol);
        }
    }
    let mut verdicts = declared_verdicts(&text);
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(0),
                        ..
                    }
                )
            )
        ),
        "{status:?}"
    );
}

#[test]
fn a_named_owned_member_keeps_public_inputs_and_guarantees() {
    for (actual, promised, bad_input) in [("0", "", true), ("7", "ensures result == 0", false)] {
        let program = typed(&named_owned_source(actual, promised, "0"));
        let entry = entry(&program);
        let mut conformances = program.conformances().iter();
        let conformance = conformances
            .next()
            .expect("the authored Inline conformance");
        assert!(conformances.next().is_none());
        let [row] = conformance.rows.as_slice() else {
            panic!("the named map owns exactly its authored Step::apply row");
        };
        assert_eq!(row.declaring_trait, conformance.trait_symbol);
        assert!(row.source.owns_realization());
        let prepared_source = typed_trees_to_terminal_psi::prepare_compilation(
            &program,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        );
        assert_eq!(prepared_source.source.forms.len(), 3);
        assert!(prepared_source.source.forms.iter().all(Option::is_some));
        let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) =
            prepared_source.program
        else {
            panic!("the same authored program must reach ordinary Terminal judgment");
        };
        assert!(prepared.forms.is_empty());
        assert!(prepared.arrivals.is_empty());
        assert_eq!(prepared.witnesses.len(), 2);
        // The body's independent conformance comes first. The separate
        // selected-call signature witness belongs to this exact selection.
        assert_eq!(prepared.selection_witnesses, [(0, 1)]);
        let closure = prepared.closure.as_ref().expect("the actual entry closure");
        assert_eq!(closure.machines.get(id(1)), Some(&entry));
        let [selected] = closure.selections.as_slice() else {
            panic!("the actual source makes one named call");
        };
        assert_eq!(selected.caller, id(1));
        assert_eq!(selected.declaring_trait, row.declaring_trait);
        assert_eq!(selected.requirement, row.requirement);
        assert_eq!(
            closure.machines.get(selected.implementation),
            Some(&row.realization_machine)
        );
        assert!(selected.required_contract.is_some());
        for witness in &prepared.witnesses {
            let witness = witness
                .as_ref()
                .expect("both source witnesses were prepared");
            let Some(semantic_vocabulary::TargetMachineCallable::Machine(root)) =
                witness.module.entry
            else {
                panic!("each logical witness has its actual requirement root");
            };
            assert_eq!(witness.machines.get(root), Some(&row.requirement));
            let bodies = witness
                .machines
                .values()
                .filter(|symbol| **symbol == row.realization_machine)
                .count();
            assert_eq!(
                bodies, 1,
                "each witness checks the exact authored Inline body"
            );
        }
        let verdicts = crate::judge_compilation(
            terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared),
        )
        .into_iter()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
        assert_eq!(verdicts.len(), 3, "{verdicts:?}");
        let RouteVerdict::Rejected { unproved, .. } = &verdicts[0] else {
            panic!("{verdicts:?}")
        };
        assert!(
            unproved.iter().any(|row| if bad_input {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            }),
            "{verdicts:?}"
        );
        assert!(
            matches!(verdicts[1], RouteVerdict::Accepted(_)),
            "the own-body witness remains valid: {verdicts:?}"
        );
        assert!(
            matches!(verdicts[2], RouteVerdict::Accepted(_)),
            "the exact selected public signature witness remains valid: {verdicts:?}"
        );
    }
}

#[test]
fn a_named_owned_member_still_owes_its_actual_body() {
    let verdicts = declared_verdicts(&named_owned_source("7", "", "1"));
    assert!(
        verdicts.iter().any(|verdict| matches!(verdict,
        RouteVerdict::Rejected { unproved, .. } if unproved.iter().any(|row|
            matches!(row.kind, ObligationKind::NormalGuarantee { .. })))),
        "{verdicts:?}"
    );
}

#[test]
fn a_named_owned_resultless_call_keeps_its_requirement() {
    for (actual, accepted) in [(7, true), (0, false)] {
        let text = format!(
            r#"
trait Step {{ machine apply(value: u32) requires value > 0 crashes none; }}
Inline satisfies Step {{ machine apply(input: u32) requires true crashes none {{ }} }}
machine entry() crashes none {{ Inline::apply({actual}); }}
"#
        );
        let verdicts = declared_verdicts(&text);
        assert_eq!(
            matches!(verdicts[0], RouteVerdict::Accepted(_)),
            accepted,
            "{verdicts:?}"
        );
        if !accepted {
            let RouteVerdict::Rejected { unproved, .. } = &verdicts[0] else {
                panic!("{verdicts:?}")
            };
            assert!(
                unproved
                    .iter()
                    .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. })),
                "{verdicts:?}"
            );
        }
    }
}

#[test]
fn a_named_boundary_implementation_has_no_direct_entry() {
    let text = r#"
boundary trait Step {
    machine apply(value: u32) -> u32
    requires value > 0 ensures result < value reaches none crashes none;
}
Inline satisfies Step {
    machine apply(input: u32) -> u32
    requires true ensures result == 0 reaches none crashes none { 0 }
}
machine entry() -> u32 reaches none crashes none { Inline::apply(7) }
"#;
    let Err(refusal) = invocation(typed(text)) else {
        panic!("a boundary implementation requires its provider slot")
    };
    assert!(
        refusal
            .detail
            .contains("boundary conformer is callable only through its provider slot"),
        "{refusal:?}"
    );
}

fn named_two_packages(public_map: bool) -> typed_trees::TypedTrees {
    let library = named_owned_source("7", "", "0")
        .split("machine entry()")
        .next()
        .unwrap()
        .to_owned()
        .replace("trait Step", "public trait Step")
        .replace(
            "Inline satisfies",
            if public_map {
                "public Inline satisfies"
            } else {
                "Inline satisfies"
            },
        );
    let client = "machine entry() -> u32 crashes none { Inline::apply(7) }";
    let mut sources = source::SourceMap::default();
    let mut syntax = None;
    let mut source_ids = Vec::new();
    for (path, root, text, identity) in [
        ("library/api.omg", "library", library.as_str(), [1; 32]),
        ("client/main.omg", "client", client, [2; 32]),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                root.into(),
                Some(semantic_vocabulary::PackageKeyIdentity::from_digest(identity).unwrap()),
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Product,
            )
            .source_id;
        source_ids.push(id);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        if let Some(syntax) = &mut syntax {
            tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
        } else {
            syntax = Some(tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap());
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    request
        .top_level_bindings
        .push(symbols::SourceScopedTopLevelBinding::new(
            source_ids[1],
            source_ids[0],
            "Inline",
        ));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn a_named_map_controls_visibility_without_publishing_its_private_body() {
    let prepared = named_two_packages(true);
    let verdicts = compilation_verdicts_of(&prepared);
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let private_map = named_two_packages(false);
    let Err(refusal) = invocation(private_map) else {
        panic!("a private map cannot be named by another package")
    };
    assert!(
        refusal
            .detail
            .contains("private machine of another package"),
        "{refusal:?}"
    );

    // A generated bare call cannot borrow the public map's visibility.
    // Retain the real source and body, removing only the routed selection.
    let mut bare = prepared;
    let (handle, body) = bare
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            let namespace = bare.expression_table.name_path_node(call.receiver)?.symbol;
            let (_, row, _) = bare
                .named_conformance_member(namespace, call.target_symbol)
                .ok()??;
            Some((handle, row.realization_machine))
        })
        .unwrap();
    assert!(!bare.machine_by_symbol(body).unwrap().is_public);
    let typed_trees::expression::ExpressionNode::Call(call) =
        bare.expression_table.expression_mut(handle)
    else {
        unreachable!()
    };
    call.target_symbol = body;
    call.receiver = Default::default();
    call.authored_call_selection = None;
    call.authored_call_binding = None;
    call.authored_machine_application = Default::default();
    let Err(refusal) = invocation(bare) else {
        panic!("the private body has no direct foreign entry")
    };
    assert!(
        refusal
            .detail
            .contains("private machine of another package"),
        "{refusal:?}"
    );
}

fn closed_named_source(actual: &str, promised: &str, body: &str) -> String {
    named_owned_source(actual, promised, body)
        .replacen("trait Step {", "trait Step<Value> {", 1)
        .replacen(
            "machine apply(value: u32) -> u32",
            "machine apply(value: Value) -> Value",
            1,
        )
        .replacen("Inline satisfies Step {", "Inline satisfies Step<u32> {", 1)
}

fn assert_named_invocation_value(
    mut verdicts: Vec<RouteVerdict>,
    expected: semantic_vocabulary::IntegerValue,
) {
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0) else {
        unreachable!()
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            &status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value,
                        ..
                    }
                )
            ) if value == &expected
        ),
        "{status:?}"
    );
}

#[test]
fn a_closed_named_trait_application_keeps_its_applied_contract() {
    assert_named_invocation_value(
        declared_verdicts(&closed_named_source("7", "", "0")),
        semantic_vocabulary::IntegerValue::Unsigned(0),
    );
}

#[test]
fn a_closed_named_member_exposes_only_its_applied_requirement() {
    for (actual, promised, bad_input) in [("0", "", true), ("7", "ensures result == 0", false)] {
        let verdicts = declared_verdicts(&closed_named_source(actual, promised, "0"));
        assert!(
            matches!(&verdicts[0], RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| if bad_input {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            })),
            "{verdicts:?}"
        );
        assert!(
            matches!(verdicts[1], RouteVerdict::Accepted(_)),
            "the owning body's private clauses remain valid: {verdicts:?}"
        );
    }
}

#[test]
fn a_closed_named_member_still_owes_its_weaker_written_inputs() {
    let text = closed_named_source("7", "", "input - 1").replacen(
        "ensures result == 0",
        "ensures result < input",
        1,
    );
    let verdicts = declared_verdicts(&text);
    assert!(
        matches!(&verdicts[0], RouteVerdict::Accepted(_)),
        "the runtime body is safe under the public positive input: {verdicts:?}"
    );
    assert!(
        matches!(&verdicts[1], RouteVerdict::Rejected { unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))),
        "the own body still admits zero under its written requires true: {verdicts:?}"
    );
}

#[test]
fn a_closed_named_member_inherits_its_applied_logical_contract() {
    for (body, accepted) in [("0", true), ("input", false)] {
        let text =
            closed_named_source("7", "", body).replacen("requires true ensures result == 0", "", 1);
        let verdicts = declared_verdicts(&text);
        if accepted {
            assert!(
                verdicts
                    .iter()
                    .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
                "{verdicts:?}"
            );
        } else {
            assert!(
                matches!(&verdicts[1], RouteVerdict::Rejected { unproved, .. }
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
                "the actual body owes the inherited result clause: {verdicts:?}"
            );
        }
    }
}

#[test]
fn closed_named_members_keep_two_applications_of_one_trait_distinct() {
    let text = r#"
trait Step<Value> {
    machine apply(value: Value) -> Value
    requires value > 0 ensures result < value crashes none;
}
Small satisfies Step<u32> {
    machine apply(input: u32) -> u32
    requires true ensures result == 0 crashes none { 0 }
}
Large satisfies Step<u64> {
    machine apply(input: u64) -> u64
    requires input > 0 ensures result < input crashes none { input - 1 }
}
machine entry() -> u64 crashes none {
    let first: u32 = Small::apply(7);
    Large::apply(9)
}
"#;
    let prepared = typed(text);
    let verdicts = compilation_verdicts_of(&prepared);
    let RouteVerdict::Accepted(accepted) = &verdicts[0] else {
        panic!("both applied calls must be accepted")
    };
    let [small, large] = accepted.elaboration.selections.as_slice() else {
        panic!("the entry must retain both selected operations")
    };
    assert_eq!(small.declaring_trait, large.declaring_trait);
    assert_eq!(small.requirement, large.requirement);
    assert!(small.required_contract.is_some());
    assert!(large.required_contract.is_some());
    assert_ne!(small.required_contract, large.required_contract);
    assert_ne!(small.implementation, large.implementation);
    assert_named_invocation_value(verdicts, semantic_vocabulary::IntegerValue::Unsigned(8));
}

#[test]
fn a_closed_named_witness_cannot_borrow_another_application_scope() {
    let wrong_application = closed_named_source("7", "", "0").replacen("Step<u32>", "Step<u64>", 1);
    let Err(refusal) = invocation(typed(&wrong_application)) else {
        panic!("a u32 body cannot serve the u64 application")
    };
    assert!(
        refusal
            .detail
            .contains("member signature differs from its requirement"),
        "{refusal:?}"
    );

    let text = format!(
        "{}{}",
        closed_named_source("7", "", "0"),
        r#"
trait Other<Value> {
    machine apply(value: Value) -> Value
    requires value > 0 ensures result < value crashes none;
}
machine plain(input: u32) -> u32
requires input > 0 ensures result < input crashes none { input - 1 }
"#
    );
    let prepared = typed(&text);
    let step = prepared
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Step")
        .unwrap();
    let other = prepared
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Other")
        .unwrap();
    let owned = prepared
        .machines()
        .iter()
        .find(|machine| {
            prepared
                .owned_conformance_realization(machine.symbol)
                .is_some()
        })
        .unwrap();
    let plain = prepared
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "plain")
        .unwrap();
    let Err(refusal) = typed_trees_to_terminal_psi::elaborate_witness(
        &prepared,
        other.symbol,
        prepared.trait_machine_signatures(other)[0].symbol,
        &[],
        owned.symbol,
    ) else {
        panic!("matching shapes cannot rebind the owning requirement")
    };
    assert!(
        refusal.detail.contains("conformance witness cannot rebind"),
        "{refusal:?}"
    );

    let Err(refusal) = typed_trees_to_terminal_psi::elaborate_witness(
        &prepared,
        step.symbol,
        prepared.trait_machine_signatures(step)[0].symbol,
        &[],
        plain.symbol,
    ) else {
        panic!("a plain body cannot infer an application from an available map")
    };
    assert!(
        refusal
            .detail
            .contains("witness needs its exact closed trait application"),
        "{refusal:?}"
    );
}

fn generic_named_map_source() -> &'static str {
    r#"
trait Step<Value> {
    machine apply(value: Value) -> Value reaches none crashes none;
}
Selected<Value> satisfies Step<Value> {
    machine apply(input: Value) -> Value reaches none crashes none { input }
}
machine entry() -> u32 reaches none crashes none { Selected<u32>::apply(7) }
"#
}

fn generic_named_map_call(
    program: &typed_trees::TypedTrees,
) -> &typed_trees::expression::TableCallExpression {
    let mut calls = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            program
                .expression_table
                .name_path_node(call.receiver)
                .filter(|path| program.conformance_by_symbol(path.symbol).is_some())
                .map(|_| call)
        });
    let call = calls.next().expect("one direct named map call");
    assert!(calls.next().is_none());
    call
}

#[test]
fn generic_named_map_source_retains_its_explicit_application() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (owner, row, signature) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    assert_eq!(program.conformance_type_parameters(owner).len(), 1);
    assert_eq!(row.requirement, call.target_symbol);
    assert_eq!(signature.symbol, row.requirement);
    let carrier = program
        .primitive_type_reference(signature.return_type)
        .and_then(|primitive| primitive.fixed_integer())
        .expect("the public result retains its canonical integer carrier");
    assert_eq!((carrier.width(), carrier.is_signed()), (32, false));
    let selection = program
        .authored_declaration_selections()
        .get(call.authored_call_selection.unwrap())
        .unwrap();
    assert_eq!(
        selection.target().as_resolved().unwrap().selected_symbol(),
        row.requirement
    );
    assert_ne!(row.requirement, row.realization_state);
    assert!(
        program
            .named_conformance_member(owner.symbol, row.requirement)
            .is_err(),
        "a bare generic map cannot borrow the call's explicit application"
    );
}

#[test]
fn generic_named_map_source_refuses_missing_foreign_or_rebound_applications() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (_, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let mut missing = call.clone();
    missing.requirement_application = typed_trees::types::TypeReferenceHandle::invalid();
    assert!(program.named_conformance_call_member(&missing).is_err());
    let mut foreign = call.clone();
    foreign.requirement_application = typed_trees::types::TypeReferenceHandle::from_parts(
        call.requirement_application.arena_index(),
        call.requirement_application.generation() + 1,
    );
    assert!(program.named_conformance_call_member(&foreign).is_err());
    let mut source = call.clone();
    source.authored_machine_application =
        typed_trees::static_applications::StaticMachineApplicationHandle::from_parts(
            call.authored_machine_application.arena_index(),
            call.authored_machine_application.generation() + 1,
        );
    assert!(program.named_conformance_call_member(&source).is_err());
    let mut body = call.clone();
    body.target_symbol = row.realization_state;
    assert!(
        program.named_conformance_call_member(&body).is_err(),
        "the owning body cannot replace its public requirement selection"
    );
}

#[test]
fn generic_named_map_invokes_its_closed_own_telescope() {
    let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed(generic_named_map_source()),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let verdicts = compilation_verdicts_of(&prepared);
    assert_named_invocation_value(verdicts, semantic_vocabulary::IntegerValue::Unsigned(7));
}

#[test]
fn generic_named_map_body_uses_its_own_rigid_telescope() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (_, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let mut form =
        typed_trees_to_terminal_psi::elaborate_generic(&program, row.realization_machine).unwrap();
    let discharged = discharge(&mut form, &mut crate::ProofSearchActivity::default()).unwrap();
    let root = discharged
        .machines
        .get(id(1))
        .expect("the actual generic body was judged");
    assert!(root.conflicts.is_empty(), "{:?}", root.conflicts);
    assert!(root.unestablished().next().is_none());
    assert!(
        root.verdicts.values().all(Result::is_ok),
        "{:?}",
        root.verdicts
    );
}

#[test]
fn generic_named_map_body_cannot_be_an_unapplied_runtime_entry() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (_, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let refusal = typed_trees_to_terminal_psi::elaborate_closure(&program, row.realization_machine)
        .unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        refusal
            .detail
            .contains("generic map body needs its complete selected application"),
        "{refusal:?}"
    );
}

#[test]
fn generic_named_map_source_keeps_parameter_dispatch_separate() {
    let program = typed(generic_named_map_source());
    let call = generic_named_map_call(&program);
    let (owner, row, _) = program
        .named_conformance_call_member(call)
        .unwrap()
        .unwrap();
    let mut mixed = call.clone();
    mixed.static_requirement_dispatch = Some(typed_trees::StaticRequirementDispatch {
        evidence_parameter: program.conformance_type_parameters(owner)[0].symbol,
        declaring_trait: row.declaring_trait,
        requirement: row.requirement,
        realization_machine: row.realization_machine,
        realization_state: row.realization_state,
    });
    assert!(
        program
            .named_conformance_call_member(&mixed)
            .unwrap()
            .is_none(),
        "a nominal receiver cannot replace an existing parameter dispatch"
    );
    assert!(
        program
            .named_conformance_capture_key(&mixed)
            .unwrap()
            .is_none()
    );
}

#[test]
fn generic_named_map_source_capture_identity_keeps_unused_binders() {
    let text = generic_named_map_source()
        .replacen(
            "Selected<Value> satisfies",
            "Selected<Value, Unused> satisfies",
            1,
        )
        .replacen(
            "Selected<u32>::apply(7)",
            "let first: u32 = Selected<u32, u64>::apply(7); Selected<u32, bool>::apply(8)",
            1,
        );
    let program = typed(&text);
    let calls = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            program
                .expression_table
                .name_path_node(call.receiver)
                .filter(|path| program.conformance_by_symbol(path.symbol).is_some())
                .map(|_| call)
        })
        .collect::<Vec<_>>();
    let [left, right] = calls[..] else {
        panic!("two explicit applications")
    };
    let left_key = program
        .named_conformance_capture_key(left)
        .unwrap()
        .unwrap();
    let right_key = program
        .named_conformance_capture_key(right)
        .unwrap()
        .unwrap();
    assert_eq!(left_key.0, right_key.0, "one actual lexical source owner");
    assert_ne!(
        left_key.1, right_key.1,
        "the map's unused binder still belongs to its body identity"
    );
    let left_scope = program
        .applied_requirement_scope(left_key.0, left.requirement_application)
        .unwrap();
    let right_scope = program
        .applied_requirement_scope(right_key.0, right.requirement_application)
        .unwrap();
    assert!(
        left_scope.same_application(&program, &right_scope),
        "equal public trait scopes must not collapse the independently bound map tuples"
    );
}

#[test]
fn generic_named_map_source_application_retains_actual_constructor_bindings() {
    let text = generic_named_map_source()
        .replacen(
            "Selected<Value> satisfies",
            "Selected<Value, Unused> satisfies",
            1,
        )
        .replacen(
            "Selected<u32>::apply(7)",
            "let first: u32 = Selected<u32, u64>::apply(7); Selected<u32, bool>::apply(8)",
            1,
        );
    let mut program = typed(&text);
    let calls = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| {
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                return None;
            };
            program
                .expression_table
                .name_path_node(call.receiver)
                .filter(|path| program.conformance_by_symbol(path.symbol).is_some())
                .map(|_| call.clone())
        })
        .collect::<Vec<_>>();
    let [left, right] = &calls[..] else {
        panic!("two explicit constructor applications")
    };
    for call in [left, right] {
        let operand = program
            .static_operand_from_type_reference(call.requirement_application)
            .unwrap();
        let typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = program
            .type_reference_table
            .type_reference(call.requirement_application)
        else {
            panic!("the source retains its complete map application")
        };
        let references = program
            .type_reference_table
            .type_reference_handles(*arguments);
        let actual = &operand.application.as_ref().unwrap().arguments;
        assert_eq!(actual.len(), references.len());
        for (argument, reference) in actual.iter().zip(references) {
            assert_eq!(
                argument.type_reference, *reference,
                "the same source payload survives conversion for every used or unused map argument"
            );
        }
    }
    let expected_owner = program
        .named_conformance_capture_key(left)
        .unwrap()
        .unwrap()
        .0;
    let left_application = program
        .prepare_named_conformance_application(left)
        .unwrap()
        .unwrap();
    let right_application = program
        .prepare_named_conformance_application(right)
        .unwrap()
        .unwrap();
    assert_ne!(
        left_application, right_application,
        "different unused arguments are different owned bodies"
    );
    let left_scope = left_application.trait_scope.scope(&program).unwrap();
    let right_scope = right_application.trait_scope.scope(&program).unwrap();
    assert!(left_scope.same_application(&program, &right_scope));
    for application in [left_application, right_application] {
        assert_eq!(application.arguments.len(), 2);
        assert_eq!(application.type_arguments.len(), 2);
        assert!(application.const_arguments.is_empty() && application.machine_arguments.is_empty());
        assert!(!application.identity_encoding.is_empty());
        assert!(!application.commitment.is_zero());
        let source = program
            .expression_table
            .static_machine_application(application.callable_application);
        assert_eq!(source.lexical_owner, expected_owner);
        assert_eq!(source.registration_operation, application.declaration);
        assert_eq!(source.binding_declaration, application.declaration);
        let parameters = program.conformance_type_parameters(
            program
                .conformance_by_symbol(application.declaration)
                .unwrap(),
        );
        let bindings = program
            .expression_table
            .static_machine_application_bindings(application.callable_application);
        assert_eq!(bindings.len(), parameters.len());
        for ((parameter, binding), argument) in parameters
            .iter()
            .zip(bindings)
            .zip(application.arguments.iter())
        {
            assert_eq!(binding.parameter, parameter.symbol);
            assert_eq!(
                binding.value,
                typed_trees::static_applications::StaticMachineBindingValue::Type(
                    argument.type_reference
                )
            );
        }
    }
}

#[test]
fn generic_named_map_source_rejects_a_foreign_unused_argument() {
    let text = generic_named_map_source()
        .replacen(
            "Selected<Value> satisfies",
            "Selected<Value, Unused> satisfies",
            1,
        )
        .replacen("Selected<u32>::apply(7)", "Selected<u32, u64>::apply(7)", 1);
    let mut program = typed(&text);
    let mut call = generic_named_map_call(&program).clone();
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        lifetime_arguments,
        arguments,
    } = program
        .type_reference_table
        .type_reference(call.requirement_application)
        .clone()
    else {
        panic!("a complete map application")
    };
    let mut arguments = program
        .type_reference_table
        .type_reference_handles(arguments)
        .to_vec();
    arguments[1] = typed_trees::types::TypeReferenceHandle::from_parts(
        arguments[1].arena_index(),
        arguments[1].generation() + 1,
    );
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles(arguments);
    call.requirement_application =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            });
    let failure = program.named_conformance_call_member(&call).unwrap_err();
    assert!(failure.contains("region-free map argument"), "{failure}");
    assert!(program.named_conformance_capture_key(&call).is_err());
    assert!(
        program
            .prepare_named_conformance_application(&call)
            .is_err()
    );
}

fn prepare_named_map(text: &str) -> typed_trees::FrozenTypedTrees {
    typed_trees_to_typed_trees::prepare_typed_trees(
        typed(text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap()
    .0
}

fn generic_owned_source(actual: &str, promised: &str, body: &str) -> String {
    named_owned_source(actual, promised, body)
        .replace("Inline satisfies", "Selected<Unused> satisfies")
        .replace("Inline::apply", "Selected<u64>::apply")
}

#[test]
fn generic_named_map_captures_keep_every_unused_binder() {
    let text = generic_named_map_source()
        .replace(
            "Selected<Value> satisfies",
            "Selected<Value, Unused> satisfies",
        )
        .replace(
            "Selected<u32>::apply(7)",
            "let first: u32 = Selected<u32, u64>::apply(3); Selected<u32, bool>::apply(7)",
        );
    let prepared = prepare_named_map(&text);
    let captures = prepared
        .captured_machine_applications
        .iter()
        .map(|(_, row)| row)
        .collect::<Vec<_>>();
    assert_eq!(captures.len(), 2);
    assert_ne!(captures[0].instance, captures[1].instance);
    assert_ne!(
        captures[0].conformance_application.identity_encoding,
        captures[1].conformance_application.identity_encoding
    );
    let verdicts = compilation_verdicts_of(&prepared);
    let RouteVerdict::Accepted(invocation) = &verdicts[0] else {
        panic!("expected the closed invocation: {verdicts:?}");
    };
    let module = &invocation.elaboration.module;
    terminal_semantics::closed_reach::validate_closed_reach_applications(module).unwrap();
    let retained = module
        .machines
        .iter()
        .filter_map(|machine| machine.closed_reach_application.as_ref()?.capture.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 2);
    assert!(
        retained
            .iter()
            .all(|capture| capture.parameter_kinds == [0, 0])
    );
    assert_named_invocation_value(verdicts, semantic_vocabulary::IntegerValue::Unsigned(7));
    let mut missing = (*prepared).clone();
    missing.captured_machine_applications = Default::default();
    missing
        .captured_machine_applications
        .insert(captures[1].clone());
    let verdicts = compilation_verdicts_of(&missing);
    assert!(
        matches!(&verdicts[0], RouteVerdict::FallBack(refusal)
        if refusal.detail.contains("named map body has no prepared capture")),
        "{verdicts:?}"
    );
}

#[test]
fn generic_named_map_captures_keep_the_actual_source_owner() {
    let text = generic_named_map_source().replace(
        "machine entry() -> u32 reaches none crashes none { Selected<u32>::apply(7) }",
        "machine other() -> u32 reaches none crashes none { Selected<u32>::apply(3) }
         machine entry() -> u32 reaches none crashes none { let first: u32 = other(); Selected<u32>::apply(7) }",
    );
    let prepared = prepare_named_map(&text);
    let captures = prepared
        .captured_machine_applications
        .iter()
        .map(|(_, row)| row)
        .collect::<Vec<_>>();
    assert_eq!(captures.len(), 2);
    assert_ne!(captures[0].instance, captures[1].instance);
    assert_ne!(
        prepared
            .expression_table
            .static_machine_application(captures[0].binding_application)
            .lexical_owner,
        prepared
            .expression_table
            .static_machine_application(captures[1].binding_application)
            .lexical_owner
    );
    assert_named_invocation_value(
        compilation_verdicts_of(&prepared),
        semantic_vocabulary::IntegerValue::Unsigned(7),
    );
}

#[test]
fn generic_named_map_calls_keep_public_requirements_and_guarantees() {
    for (actual, promised, bad_input) in [("0", "", true), ("7", "ensures result == 0", false)] {
        let prepared = prepare_named_map(&generic_owned_source(actual, promised, "0"));
        let verdicts = compilation_verdicts_of(&prepared);
        assert!(
            matches!(&verdicts[0], RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| if bad_input {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
            } else {
                matches!(row.kind, ObligationKind::NormalGuarantee { .. })
            })),
            "{verdicts:?}"
        );
        assert!(
            verdicts
                .iter()
                .skip(1)
                .all(|verdict| matches!(verdict, RouteVerdict::Accepted(_))),
            "{verdicts:?}"
        );
    }
}

#[test]
fn generic_named_map_body_still_owes_its_weaker_written_input() {
    let text = generic_owned_source("7", "", "input - 1").replacen(
        "ensures result == 0",
        "ensures result < input",
        1,
    );
    let prepared = prepare_named_map(&text);
    let verdicts = compilation_verdicts_of(&prepared);
    assert!(
        matches!(&verdicts[0], RouteVerdict::Accepted(_)),
        "{verdicts:?}"
    );
    assert!(
        verdicts.iter().skip(1).any(|verdict| matches!(verdict,
        RouteVerdict::Rejected { unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. })))),
        "{verdicts:?}"
    );
}

#[test]
fn generic_named_map_preserves_the_written_member_reach_ceiling() {
    let text = format!(
        "boundary trait Service {{}}\n{}",
        generic_named_map_source()
    )
    .replace(
        "machine apply(input: Value) -> Value reaches none",
        "machine apply(input: Value) -> Value reaches Service",
    );
    let prepared = prepare_named_map(&text);
    let verdicts = compilation_verdicts_of(&prepared);
    assert!(
        matches!(&verdicts[0], RouteVerdict::Accepted(_)),
        "{verdicts:?}"
    );
    assert!(
        verdicts.iter().skip(1).any(|verdict| matches!(verdict,
        RouteVerdict::Rejected { exceeded, .. } if !exceeded.is_empty())),
        "{verdicts:?}"
    );
}

#[test]
fn generic_named_map_closed_call_also_judges_its_original_template() {
    let prepared = prepare_named_map(&generic_named_map_source().replace("{ input }", "{ 0 }"));
    let verdicts = compilation_verdicts_of(&prepared);
    assert!(
        verdicts
            .iter()
            .any(|verdict| !matches!(verdict, RouteVerdict::Accepted(_))),
        "a selected integer instance cannot validate an arbitrary Value body"
    );
}

fn full_named_map_verdicts(typed: &typed_trees::TypedTrees) -> Vec<RouteVerdict> {
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry(typed)],
                subjects: &[None],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
    .into_iter()
    .map(Result::unwrap)
    .collect()
}

#[test]
fn generic_named_map_full_subject_judges_its_schema_and_owning_body() {
    let prepared = prepare_named_map(generic_named_map_source());
    let verdicts = full_named_map_verdicts(&prepared);
    for (position, verdict) in verdicts.iter().enumerate().skip(1) {
        if let RouteVerdict::Accepted(proof) = verdict {
            let rigid = proof
                .elaboration
                .tables
                .machines
                .iter()
                .any(|(_, machine)| {
                    machine
                        .values
                        .iter()
                        .any(|(_, value)| value.rigid.is_some())
                });
            if position > 2 && !rigid {
                continue; // Ordinary closed witnesses retain their existing behavior.
            }
            assert!(
                proof.publish().is_err(),
                "a generic form or witness cannot export stand-in types"
            );
            assert!(
                proof.publish_invocation().is_err(),
                "a proof form cannot become executable source"
            );
        }
    }
    assert_named_invocation_value(verdicts, semantic_vocabulary::IntegerValue::Unsigned(7));
}

#[test]
fn generic_named_map_full_subject_refuses_the_universal_literal_body() {
    let prepared = prepare_named_map(&generic_named_map_source().replace("{ input }", "{ 0 }"));
    let verdicts = full_named_map_verdicts(&prepared);
    assert!(
        verdicts
            .iter()
            .any(|verdict| !matches!(verdict, RouteVerdict::Accepted(_))),
        "one integer instance is not proof for arbitrary Value"
    );
}

#[test]
fn generic_named_map_full_subject_reads_unused_generic_requirement_headers() {
    let text = generic_named_map_source().replace(
        "machine entry()",
        r#"
trait Unused<Value> {
    machine bad(input: Value) requires input > 0 crashes none;
}
machine entry()"#,
    );
    let prepared = prepare_named_map(&text);
    let verdicts = full_named_map_verdicts(&prepared);
    assert!(
        verdicts
            .iter()
            .any(|verdict| !matches!(verdict, RouteVerdict::Accepted(_))),
        "the unused header must still support its arbitrary Value parameter"
    );
}
