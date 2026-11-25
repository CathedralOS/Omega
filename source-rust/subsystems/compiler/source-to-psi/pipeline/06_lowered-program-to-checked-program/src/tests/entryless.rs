//! Source validation claims the whole subject without inventing a program entry.

use super::boundary_calls::typed;
use super::*;

pub(super) fn judged(text: &str, select_first: bool) -> RouteVerdict {
    let typed = typed(text);
    judged_typed(&typed, select_first)
}

pub(super) fn judged_typed(
    typed: &crate::source_state::TypedTrees,
    select_first: bool,
) -> RouteVerdict {
    judged_typed_result(typed, select_first).unwrap()
}

pub(super) fn judged_typed_result(
    typed: &crate::source_state::TypedTrees,
    select_first: bool,
) -> Result<RouteVerdict, crate::CompilationRefusal> {
    whole_subject_result(judged_parts(typed, select_first))
}

/// A subject passes only if its closure, generic forms and witnesses all pass.
/// Keep the first failure in judgment order. Only an entirely accepted batch
/// returns the original closure, whose tables callers may inspect.
pub(super) fn whole_subject_result(
    mut verdicts: Vec<Result<RouteVerdict, crate::CompilationRefusal>>,
) -> Result<RouteVerdict, crate::CompilationRefusal> {
    assert!(
        !verdicts.is_empty(),
        "a subject must have a closure judgment"
    );
    let first_failure = verdicts
        .iter()
        .position(|verdict| !matches!(verdict, Ok(RouteVerdict::Accepted(_))));
    verdicts.remove(first_failure.unwrap_or(0))
}

fn judged_parts(
    typed: &crate::source_state::TypedTrees,
    select_first: bool,
) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed = &crate::prepare_fixture(typed.clone());
    let selected = if select_first {
        vec![typed.trees().machines()[0].symbol]
    } else {
        Vec::new()
    };
    crate::judge_compilation(
        crate::lowering::prepare_compilation(
            typed,
            crate::lowering::Compilation {
                calling: None,
                providers: None,

                declaration_metadata: &[],
                entries: &[],
                subjects: &[None],
                purpose: source_provenance::DependencyScope::Product,
                selected: &selected,
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
        .program,
    )
}

#[test]
fn declaration_only_library_checks_requirements_without_an_executable_machine() {
    let result = judged(
        "public boundary trait TimeHost {
            machine ticks() -> u64 reaches none crashes none;
            machine frequency() -> u64 reaches none crashes none;
        }",
        false,
    );
    let RouteVerdict::Accepted(mut closure) = result else {
        panic!("{result:?}");
    };
    let elaboration = &mut closure.elaboration;
    assert_eq!(elaboration.module.entry, None);
    assert!(elaboration.module.machines.is_empty());

    let requirement = elaboration
        .static_schemas
        .iter_mut()
        .flat_map(|schema| &mut schema.requirements)
        .next()
        .expect("library retains its boundary requirement");
    let declaration = requirement.declaration;
    // A boundary requirement with no formals cannot invoke parameter zero.
    requirement.envelope.invocations = vec![terminal_psi::TerminalCallableInvocation::Parameter(0)];
    let refusal = crate::static_contracts::admit(elaboration).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert_eq!(
        refusal.site,
        terminal_check_tables::RefusalSite::Declaration(declaration)
    );
}

#[test]
fn transparent_data_aliases_form_in_their_own_binder_scope() {
    let typed = typed(
        "public data Word = u32;
         public data Pair<T> = [T; 2];
         public data Identity<T> = T;
         public data Nested<T> = Pair<Identity<T>>;
         public data KeyHash<K> = machine(key: @K) -> u64;",
    );
    let parts = judged_parts(&typed, false);
    assert_eq!(parts.len(), 5, "expected the closure and its four generic forms");
    for part in parts {
        let Ok(RouteVerdict::Accepted(accepted)) = &part else {
            panic!("declaration-only alias form failed: {part:?}");
        };
        assert_eq!(accepted.elaboration.module.entry, None);
        assert!(accepted.elaboration.module.machines.is_empty());
    }
}

#[test]
fn transparent_data_aliases_reject_a_value_even_if_the_parameter_is_unused() {
    for (uses, expected) in [
        (
            "public data Invalid = Erased<17u64>;",
            "requires a type but was supplied a value",
        ),
        (
            "public data Invalid = Erased<Absent>;",
            "requires a type declaration for `Absent`",
        ),
        // Type lookup can leave Count unresolved: this lexical binder names a value.
        (
            "machine keep<const Count: u64>(value: Erased<Count>) -> u32 { value }",
            "type alias argument 1 requires a type",
        ),
    ] {
        let source = format!("public data Erased<T> = u32; {uses}");
        let Err(error) = super::boundary_calls::try_typed(&source) else {
            panic!("an unused parameter still requires a type argument: {uses}");
        };
        assert!(error.message.contains(expected), "{error:?}");
    }
}

#[test]
fn transparent_data_aliases_cannot_erase_a_parameter_property() {
    let declarations = "data Ticket [linear] { value: u32; }
                        public data CopyOnly<T [copy]> = u32;
                        public data LinearOnly<T [linear]> = u32;";
    let accepted = format!(
        "{declarations}
                           public data A = CopyOnly<u32>;
                           public data B = LinearOnly<Ticket>;
                           public data C = CopyOnly<@u32>;"
    );
    let result = judged(&accepted, false);
    assert!(matches!(result, RouteVerdict::Accepted(_)), "{result:?}");
    for (application, required) in [
        ("CopyOnly<Ticket>", "[copy]"),
        ("CopyOnly<@mutable u32>", "[copy]"),
        ("CopyOnly<@write u32>", "[copy]"),
        ("LinearOnly<u32>", "[linear]"),
    ] {
        let source = format!("{declarations} public data Invalid = {application};");
        let Err(error) = super::boundary_calls::try_typed(&source) else {
            panic!("the property required by {application} was erased");
        };
        assert!(
            error
                .message
                .ends_with(&format!("the binder requires `{required}`")),
            "{error:?}"
        );
    }
}

#[test]
fn specialized_callback_reach_keeps_omission_empty_and_named_services_distinct() {
    use crate::source_state::types::TypeReferenceNode;

    for (clause, expected) in [
        ("", None),
        ("reaches none", Some(vec![])),
        ("reaches Output", Some(vec!["Output"])),
    ] {
        let text = format!(
            "boundary trait Output {{}}
             data Observe = machine(value: u64) -> u64 {clause} crashes none;
             machine read(value: u64) -> u64 reaches none crashes none {{ value }}
             machine choose<T>(operation: Observe, value: T) -> u64 {{ operation(47u64) }}
             machine main() -> u64 {{ choose::<u64>(read, 1u64) }}"
        );
        let program = crate::preparation::prepare_typed_trees(typed(&text), &[], &[]).unwrap();
        let trees = program.trees();
        let template = trees
            .machines()
            .iter()
            .find(|m| m.name.as_str() == "choose")
            .unwrap();
        let instance = trees
            .machine_specializations
            .iter()
            .find(|row| row.template == template.symbol)
            .unwrap();
        let machine = trees.machine_by_symbol(instance.instance).unwrap();
        let state = trees.machine_states(machine).first().unwrap();
        let formal = trees
            .state_parameters
            .span(state.parameters)
            .unwrap()
            .first()
            .unwrap();
        let TypeReferenceNode::Callable { contract } = trees
            .type_reference_table
            .type_reference(formal.type_reference)
        else {
            panic!("the specialized parameter must retain its structural callable");
        };
        let signature = &trees.runtime_callable_contract(*contract).signature;
        let rows = trees
            .authored_service_reach_rows_for(signature.symbol)
            .collect::<Vec<_>>();
        match expected {
            None => assert!(rows.is_empty()),
            Some(services) => {
                let [row] = rows.as_slice() else {
                    panic!("one written clause on the copied signature")
                };
                assert_eq!(row.keyword_source_spans.len(), 1);
                let targets = row
                    .targets
                    .iter()
                    .map(|target| trees.symbols.name(target.service.symbol(trees).unwrap()))
                    .collect::<Vec<_>>();
                assert_eq!(targets, services);
            }
        }
    }
}

#[test]
fn declared_callback_lifetimes_bind_in_the_named_owners_parameter_identity() {
    fn named_key(left: &str, right: &str, captured: &str) -> String {
        let source = format!(
            "public data Visit<'item,T> = machine(item: @'item T) -> u64 reaches none crashes none;
                              machine visit<{left},{right}>(callback: Visit<{captured},u64>) {{}}"
        );
        let program = typed(&source);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "visit")
            .expect("visit declaration");
        program
            .machine_callable_identity_text(machine)
            .expect("named callable identity")
    }
    assert_eq!(
        named_key("'a", "'b", "'a"),
        named_key("'left", "'right", "'left")
    );
    assert_ne!(named_key("'a", "'b", "'a"), named_key("'a", "'b", "'b"));
}

#[test]
fn declared_callback_lifetimes_bind_in_the_requirement_parameter_identity() {
    fn requirement_key(first: &str, second: &str, captured: &str) -> String {
        let source = format!(
            "public data Visit<'item,T> = machine(item: @'item T) -> u64 reaches none crashes none;
             public trait Hooks {{
                 machine attach<{first},{second}>(callback: Visit<{captured},u64>)
                 reaches none crashes none;
             }}"
        );
        let program = typed(&source);
        let owner = program
            .traits()
            .iter()
            .find(|row| row.name.as_str() == "Hooks")
            .unwrap();
        let requirements = program.trait_machine_signatures(owner);
        let [requirement] = requirements.as_array().expect("one callback requirement");
        program.trait_requirement_callable_identity_text(owner, requirement)
    }
    assert_eq!(
        requirement_key("'a", "'b", "'a"),
        requirement_key("'left", "'right", "'left")
    );
    assert_ne!(
        requirement_key("'a", "'b", "'a"),
        requirement_key("'a", "'b", "'b")
    );
}

#[test]
fn declared_callback_lifetime_aliases_publish_without_an_entry() {
    let result = judged(
        "public data Visit<'item,T> = machine(item: @'item T) -> u64;
         public data VisitRenamed<'other,Element> = machine(item: @'other Element) -> u64;
         public data Forward<'enclosing,T> = Visit<'enclosing,T>;
         public data Both<'outer,T> = machine<'call>(stored: @'outer T, passed: @'call T) -> u64;
         public data Borrowed<'item,T> = @'item T;",
        false,
    );
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}");
    };
    assert!(accepted.elaboration.module.entry.is_none());
    assert!(accepted.elaboration.module.machines.is_empty());
}

#[test]
fn declared_callback_lifetimes_cannot_hide_an_undeclared_argument() {
    let source = "public data Erased<'item,T> = T;
                  machine consume<'owned>(value: Erased<'unknown,u64>) -> u64 { value }";
    let Err(error) = super::boundary_calls::try_typed(source) else {
        panic!("an alias may erase the region from the result, not its formation obligation");
    };
    assert!(
        error.message.contains("lifetime argument `'unknown`"),
        "{error:?}"
    );
}

#[test]
fn declared_callback_lifetime_alias_checks_an_open_referents_region() {
    let result = judged("public data Borrowed<'region,T> = @'absent T;", false);
    let RouteVerdict::Rejected { violations, .. } = result else {
        panic!("expected the undeclared region to reject: {result:?}");
    };
    assert!(
        violations.iter().any(|violation| violation.statement.contains("undeclared lifetime")),
        "{violations:?}"
    );
}

#[test]
fn whole_subject_acceptance_requires_every_generic_check_form() {
    let typed = typed("machine identity<T>(value: T) -> T { move value }\n");
    let parts = judged_parts(&typed, false);
    assert!(
        parts.len() > 1,
        "the generic form must be judged: {parts:?}"
    );
    assert!(
        parts
            .iter()
            .all(|part| matches!(part, Ok(RouteVerdict::Accepted(_)))),
        "{parts:?}"
    );
    let result = judged_typed(&typed, false);
    let RouteVerdict::Accepted(closure) = result else {
        panic!("{result:?}");
    };
    assert!(closure.elaboration.module.machines.is_empty());
}

#[test]
fn a_later_generic_guarantee_rejection_prevents_whole_subject_acceptance() {
    let source = "machine unchanged<T>(value: u32) -> u32 \
                  ensures result == value crashes none { value }\n";
    let accepted = judged(source, false);
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    let typed = typed(&source.replace("result == value", "result != value"));
    let parts = judged_parts(&typed, false);
    assert!(
        matches!(parts.first(), Some(Ok(RouteVerdict::Accepted(_)))),
        "{parts:?}"
    );
    assert!(
        parts
            .iter()
            .skip(1)
            .any(|part| matches!(part, Ok(RouteVerdict::Rejected { unproved, .. })
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { clause: 0, .. })
                    && row.cause == UnprovedCause::NoDerivation))),
        "the generic body must owe its false guarantee: {parts:?}"
    );
    let result = judged_typed_result(&typed, false);
    assert!(
        matches!(result, Ok(RouteVerdict::Rejected { ref unproved, .. })
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { clause: 0, .. })
                && row.cause == UnprovedCause::NoDerivation)),
        "the accepted closure must not hide the generic-body rejection: {result:?}"
    );
}

#[test]
#[should_panic(expected = "a subject must have a closure judgment")]
fn a_subject_with_no_judgments_cannot_be_accepted() {
    let _ = whole_subject_result(Vec::new());
}

#[test]
fn a_subject_without_entries_checks_every_machine_without_picking_one_as_entry() {
    let result = judged(
        "machine helper(value: u32) -> u32 { value - 1 }\n\
         machine caller() -> u32 { helper(1) }\n",
        false,
    );
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let elaboration = &accepted.elaboration;
    assert_eq!(elaboration.module.entry, None);
    assert_eq!(elaboration.module.machines.len(), 2);
    // Machine 1 is an ordinary private helper. Making it an invented entry
    // would forbid the closed-subject input inference this body needs.
    assert!(elaboration.private_inputs.contains(&id(1)));
}

#[test]
fn an_unused_invalid_subject_machine_is_still_rejected_without_entries() {
    let result = judged("machine unused(value: u32) -> u32 { value - 1 }\n", false);
    let RouteVerdict::Rejected { unproved, .. } = result else {
        panic!("{result:?}")
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
}

#[test]
fn a_selected_machine_keeps_its_input_boundary_without_an_executable_entry() {
    let result = judged(
        "machine backing(value: u32) -> u32 { value - 1 }\n\
         machine caller() -> u32 { backing(1) }\n",
        true,
    );
    let RouteVerdict::Rejected { unproved, .. } = result else {
        panic!("{result:?}")
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
}

#[test]
fn a_data_only_subject_needs_no_placeholder_machine() {
    let result = judged("data Payload { value: u32 }\n", false);
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    assert_eq!(accepted.elaboration.module.entry, None);
    assert!(accepted.elaboration.module.machines.is_empty());
    assert_eq!(accepted.elaboration.module.types.structural_types.len(), 1);
}

#[test]
fn capacity_guards_reach_the_original_addition_operands() {
    let source = r#"
machine cap(length: u64, position: u64) -> u64
requires position < length {
    transition length - position > 4294967295 {
        true -> receive(length, position + 4294967295),
        false -> position
    }
}
machine advance(length: u64, position: u64, count: i64) -> u64
requires position <= length, count >= 0 {
    let written: u64 = count as u64;
    transition written <= length - position {
        true -> receive(length, position + written),
        false -> position
    }
}
machine receive(length: u64, next: u64) -> u64
requires next <= length { next }
"#;
    let accepted = judged(source, false);
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    let different_literal = source.replace("position + 4294967295", "position + 4294967297");
    let rejected = judged(&different_literal, false);
    let RouteVerdict::Rejected { unproved, .. } = rejected else {
        panic!("{rejected:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
}
