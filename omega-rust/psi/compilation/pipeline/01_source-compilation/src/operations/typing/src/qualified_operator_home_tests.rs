//! A token binding keeps its resolved semantic home without changing its lexical owner.

use std::sync::Arc;

fn typed(units: &[(&str, &str)]) -> crate::source_state::TypedTrees {
    checked(units).unwrap()
}

pub(crate) fn checked(units: &[(&str, &str)]) -> Result<crate::source_state::TypedTrees, String> {
    packages(
        &units
            .iter()
            .map(|(path, text)| (31, *path, *text))
            .collect::<Vec<_>>(),
        &[],
        &[],
    )
}

pub(crate) fn checked_with_equality(
    units: &[(&str, &str)],
) -> Result<crate::source_state::TypedTrees, String> {
    let mut sources = vec![(
        0,
        "equality.omg",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../source/library/core/equality.omg"
        )),
    )];
    sources.extend(units.iter().map(|(path, text)| (31, *path, *text)));
    packages(&sources, &[], &[])
}

/* `packages` with the bundled `Equatable` declaration as an ambient toolchain
package ahead of `units`; import positions index `units`. */
pub(crate) fn packages_with_equality(
    units: &[(u8, &str, &str)],
    dependencies: &[(u8, &str, u8)],
    imports: &[(usize, usize, &str, usize)],
) -> Result<crate::source_state::TypedTrees, String> {
    let mut sources = vec![(
        0,
        "equality.omg",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../source/library/core/equality.omg"
        )),
    )];
    sources.extend_from_slice(units);
    let imports = imports
        .iter()
        .map(|(reference, declaration, path, prefix)| {
            (reference + 1, declaration + 1, *path, *prefix)
        })
        .collect::<Vec<_>>();
    packages(&sources, dependencies, &imports)
}

fn packages(
    units: &[(u8, &str, &str)],
    dependencies: &[(u8, &str, u8)],
    imports: &[(usize, usize, &str, usize)],
) -> Result<crate::source_state::TypedTrees, String> {
    let mut sources = crate::source_inputs::SourceMap::default();
    let mut source_ids = Vec::with_capacity(units.len());
    let mut parsed_units = Vec::with_capacity(units.len());
    let identity = |key| semantic_vocabulary::PackageKeyIdentity::from_digest([key; 32]).unwrap();
    for (package, path, text) in units {
        let root = std::path::PathBuf::from(format!("package-{package}"));
        let id = sources
            .add_with_metadata(
                root.join(path),
                (*text).to_owned(),
                root,
                (*package != 0).then(|| identity(*package)),
                if *package == 0 {
                    source_admission::SourceOrigin::Toolchain
                } else {
                    source_admission::SourceOrigin::User
                },
            )
            .source_id;
        source_ids.push(id);
        let tokens = crate::typing::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let parsed = crate::typing::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        parsed_units.push(parsed);
    }
    sources.retain_product_dependency_scope(
        dependencies
            .iter()
            .map(|(from, alias, to)| (identity(*from), *alias, identity(*to))),
    );
    let mut top_level_bindings = imports
        .iter()
        .map(|(reference, declaration, path, prefix)| {
            crate::source_symbols::SourceScopedTopLevelBinding::logical_module_import(
                source_ids[*reference],
                source_ids[*declaration],
                path,
                *prefix,
            )
        })
        .collect::<Vec<_>>();
    // Package 0 contains the admitted Core fixtures. Each file keeps its own
    // authored imports; caller associations remain explicitly supplied above.
    if let Some(anchor) = units.iter().position(|(package, _, _)| *package == 0) {
        use syntax_trees::identifier::JoinedSyntaxPath;
        for ((package, _, _), unit) in units.iter().zip(&parsed_units) {
            if *package != 0 {
                continue;
            }
            for item in unit.root_items() {
                let syntax_trees::item::Item::Use(import) = item else {
                    continue;
                };
                let members = unit.items.identifier_path_members(import.path);
                if members
                    .iter()
                    .take(3)
                    .map(|member| member.as_str(unit))
                    .eq(["omega", "language", "core"])
                {
                    top_level_bindings.push(
                        crate::source_symbols::SourceScopedTopLevelBinding::logical_module_import(
                            unit.source_id,
                            source_ids[anchor],
                            &members.joined_path_in(unit),
                            3,
                        ),
                    );
                }
            }
        }
    }
    let syntax = parsed_units
        .into_iter()
        .reduce(|mut combined, unit| {
            combined.extend_from(&unit);
            combined
        })
        .unwrap();
    let mut request = crate::resolution::ResolutionRequest::new(syntax);
    // Package-qualified imports enter resolution with the loader's exact
    // source/alias association, alongside the authored `use` declaration.
    request.top_level_bindings = top_level_bindings;
    request.sources = Some(Arc::new(sources));
    let resolved = crate::resolution::resolve(request).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    crate::typing::lower_symbol_resolved_trees(&resolved).map_err(|error| error.message)
}

const DOMAIN: &str = "module units; public domain u32::Distance;";
const OPERATOR: &str = "public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { ((left as -Distance) +% (right as -Distance)) as +Distance }";

const GENERIC_DOMAIN: &str = "module units; public domain<T> T::Distance;";
const GENERIC_OPERATOR: &str = "use units; public machine + Distance::add<T [copy]>(left: T in Distance, right: T) -> T in Distance requires T == u32 { ((left as -Distance) +% right) as +Distance }";

#[test]
fn bundled_core_fixtures_resolve_their_authored_imports() {
    let core =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../source/library/core");
    let mut files = std::fs::read_dir(core)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "omg"))
        .collect::<Vec<_>>();
    files.sort_unstable();
    let files = files
        .into_iter()
        .map(|path| {
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                std::fs::read_to_string(path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let mut units = files
        .iter()
        .map(|(name, text)| (0, name.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    units.push((
        31,
        "unimported.omg",
        "machine unimported(value: f32) { classify_f32(value); }",
    ));
    let imported = units.len();
    units.push((31, "imported.omg", "use omega::language::core::float_accuracy::classify_f32; machine imported(value: f32) { classify_f32(value); }"));
    let program = packages(
        &units,
        &[],
        &[(
            imported,
            0,
            "omega::language::core::float_accuracy::classify_f32",
            3,
        )],
    )
    .expect("bundled Core imports resolve against the admitted Core package");
    use crate::source_state::deferred_refusals::DeferredRequest;
    for (name, expected) in [
        ("unimported", Some(DeferredRequest::UndeclaredCallName)),
        ("imported", None),
    ] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        assert_eq!(
            program
                .deferred_refusal(machine.symbol)
                .map(|row| row.request),
            expected,
            "Core's imports do not expose classify_f32 in another file: {name}"
        );
    }
}

#[test]
fn a_carrier_generic_domain_retains_each_exact_application() {
    use crate::source_state::types::{TypeConstraintNode, TypeReferenceNode};
    let program = typed(&[
        ("units.omg", GENERIC_DOMAIN),
        (
            "client.omg",
            "use units; machine first(value: u32 in Distance) -> u32 in Distance { value } machine second(value: u64 in Distance) -> u64 in Distance { value } machine repeated(value: u32 in Distance) -> u32 in Distance { value }",
        ),
    ]);
    let applied = |name| {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let state = program.machine_states(machine).first().unwrap();
        let reference = program.state_parameters(state)[0].type_reference;
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            panic!("the formal keeps its qualification");
        };
        let [TypeConstraintNode::Domain(domain)] =
            program.type_reference_table.constraints(*constraints)
        else {
            panic!("one selected domain application");
        };
        (*base_type, domain)
    };
    let (first_carrier, first) = applied("first");
    let (second_carrier, second) = applied("second");
    let (_, repeated) = applied("repeated");
    assert_eq!(
        first.symbol, second.symbol,
        "one declaration owns the family"
    );
    assert_ne!(
        first.semantic_id, second.semantic_id,
        "membership cannot cross carriers"
    );
    assert_eq!(
        first.semantic_id, repeated.semantic_id,
        "the same application is shared"
    );
    for (carrier, constraint) in [(first_carrier, first), (second_carrier, second)] {
        let arguments = program
            .domain_arguments_for_instance(constraint.symbol, constraint.semantic_id)
            .expect("typing retains the whole declaration application");
        let [applied_carrier] = arguments else {
            panic!("the generic carrier is the family's only coordinate");
        };
        assert_eq!(
            program.package_qualified_type_identity(*applied_carrier),
            program.package_qualified_type_identity(carrier),
        );
        assert!(
            program
                .domain_definition_for_instance(constraint.symbol, constraint.semantic_id)
                .is_none(),
            "typing records no applied theory or checking verdict"
        );
    }
    assert_eq!(
        program
            .domain_type_parameters(&program.domain_definitions()[0])
            .len(),
        1,
        "applying a family does not rewrite its declaration"
    );
}

#[test]
fn a_generic_operator_retains_its_inferred_application_as_an_ordinary_call() {
    use crate::source_state::expression::ExpressionNode;
    let program = typed(&[
        ("units.omg", GENERIC_DOMAIN),
        ("operator.omg", GENERIC_OPERATOR),
        (
            "client.omg",
            "use units; machine sum(left: u32 in Distance, right: u32) -> u32 in Distance { left + right }",
        ),
    ]);
    let target = program.machine_token_bindings()[0].symbol;
    let calls = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Call(call) if call.target_symbol == target => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one selected generic operator call: {calls:?}");
    };
    assert!(call.authored_machine_application.is_valid());
    let arguments = program
        .expression_table
        .static_machine_application_arguments(call.authored_machine_application);
    assert_eq!(arguments.len(), 1);
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "sum")
        .unwrap();
    let state = program.machine_states(caller).first().unwrap();
    assert_eq!(
        program.normalized_type_identity_text(arguments[0].type_reference),
        program.normalized_type_identity_text(program.state_parameters(state)[1].type_reference)
    );
    let application = program
        .expression_table
        .static_machine_application(call.authored_machine_application);
    assert_eq!(application.lexical_owner, caller.symbol);
    assert_eq!(application.registration_operation, target);
    assert_eq!(application.binding_declaration, target);
}

#[test]
fn an_anonymous_operand_cannot_select_a_qualified_generic_operator() {
    use crate::source_state::expression::ExpressionNode;
    for literal in ["1", "1u32"] {
        let client = format!("use units; machine sum(right: u32) -> u32 {{ {literal} + right }}");
        let program = typed(&[
            ("units.omg", GENERIC_DOMAIN),
            ("operator.omg", GENERIC_OPERATOR),
            ("client.omg", &client),
        ]);
        let target = program.machine_token_bindings()[0].symbol;
        assert!(!program.expression_table.iter_expressions().any(|(_, node)| matches!(node, ExpressionNode::Call(call) if call.target_symbol == target)),
            "a literal has no Distance qualification");
    }
}

#[test]
fn an_enclosing_cast_reads_a_generic_operators_applied_return() {
    use crate::source_state::expression::ExpressionNode;
    let program = typed(&[
        ("units.omg", GENERIC_DOMAIN),
        ("operator.omg", GENERIC_OPERATOR),
        (
            "client.omg",
            "use units; machine sum(left: u32 in Distance, right: u32) -> u32 { (left + right) as -Distance }",
        ),
    ]);
    assert!(
        program.deferred_refusals.is_empty(),
        "{:?}",
        program.deferred_refusals
    );
    assert!(program.expression_table.iter_expressions().any(|(_, node)| matches!(node,
        ExpressionNode::Cast(cast) if cast.result_type.is_valid() && matches!(program.expression_table.expression(cast.value), ExpressionNode::Call(_)))));
}

#[test]
fn generic_operator_shapes_match_complete_qualifications_in_both_orders() {
    use crate::source_state::expression::ExpressionNode;
    let declarations = "use units;
        public machine + Distance::both<T [copy]>(left: T in Distance, right: T in Distance) -> T in Distance { left }
        public machine + Distance::left<T [copy]>(left: T in Distance, right: T) -> T in Distance { left }
        public machine + Distance::right<T [copy]>(left: T, right: T in Distance) -> T in Distance { right }";
    // domains.md, "Operands": with no declaration for its exact type, a
    // qualified operand has no builtin operator either, so typing rejects it.
    let unqualified_only = |value: &str, written: &str| {
        Err(format!(
            "builtin operator `+` takes unqualified operands only, but `left` has type \
             `{written}` and no operator `+` is declared for it; remove the domain with a cast \
             first, such as `(left as {value})`"
        ))
    };
    for (left, right, selected) in [
        (
            "u32 in Distance",
            "u32 in Distance",
            Ok(Some("Distance::both")),
        ),
        ("u32 in Distance", "u32", Ok(Some("Distance::left"))),
        ("u32", "u32 in Distance", Ok(Some("Distance::right"))),
        (
            "u32 in Distance & Other",
            "u32",
            unqualified_only("-Distance -Other", "u32 in Distance & Other"),
        ),
        (
            "u32 in Distance",
            "u64",
            unqualified_only("-Distance", "u32 in Distance"),
        ),
        ("u32", "u32", Ok(None)),
    ] {
        let client =
            format!("use units; machine sum(left: {left}, right: {right}) {{ left + right; }}");
        let checked = checked(&[
            (
                "units.omg",
                "module units; public domain<T> T::Distance; public domain<T> T::Other;",
            ),
            ("operator.omg", declarations),
            ("client.omg", &client),
        ]);
        let selected = match (checked, selected) {
            (Ok(program), Ok(selected)) => (program, selected),
            (Err(error), Err(expected)) => {
                assert_eq!(error, expected, "{left} + {right}");
                continue;
            }
            (checked, expected) => {
                panic!("{left} + {right}: {:?} against {expected:?}", checked.err())
            }
        };
        let (program, selected) = selected;
        let calls = program
            .expression_table
            .iter_expressions()
            .filter_map(|(_, node)| match node {
                ExpressionNode::Call(call)
                    if program
                        .machine_token_bindings()
                        .iter()
                        .any(|binding| binding.symbol == call.target_symbol) =>
                {
                    Some(call.target.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls,
            selected.into_iter().collect::<Vec<_>>(),
            "{left} + {right}"
        );
    }
}

#[test]
fn a_generic_operator_retains_an_inferred_const_argument_with_its_carrier() {
    use crate::source_state::expression::ExpressionNode;
    let program = typed(&[
        ("units.omg", GENERIC_DOMAIN),
        (
            "operator.omg",
            "use units; public machine + Distance::merge<T [copy], const N: u64>(left: T in Distance, right: [T; N]) -> T in Distance { left }",
        ),
        (
            "client.omg",
            "use units; machine sum(left: u32 in Distance, right: [u32; 3]) -> u32 in Distance { left + right }",
        ),
    ]);
    let target = program.machine_token_bindings()[0].symbol;
    let call = program
        .expression_table
        .iter_expressions()
        .find_map(|(_, node)| match node {
            ExpressionNode::Call(call) if call.target_symbol == target => Some(call),
            _ => None,
        })
        .expect("the array shape closes both static arguments");
    let arguments = program
        .expression_table
        .static_machine_application_arguments(call.authored_machine_application);
    assert_eq!(arguments.len(), 2);
    let crate::source_state::types::TypeReferenceNode::Named { name, .. } = program
        .type_reference_table
        .type_reference(arguments[1].type_reference)
    else {
        panic!("canonical const operand")
    };
    let value = application_identity::const_value::CanonicalConstValue::from_atom(name.as_str())
        .expect("canonical constant, not re-parsed literal syntax");
    assert!(
        matches!(value.decode_encoding(), Some(application_identity::const_value::DecodedCanonicalConstValue::Integer { value, .. }) if value.to_string() == "3")
    );
}

#[test]
fn unresolved_names_keep_their_occurrences_scope_instead_of_a_global_leaf() {
    use crate::source_state::deferred_refusals::DeferredRequest;
    let refusal = |program: &crate::source_state::TypedTrees| {
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .unwrap();
        program
            .deferred_refusal(caller.symbol)
            .map(|row| row.request)
    };
    for extra in [
        "",
        "machine unrelated(ghost: u32) {}",
        "data Other { ghost: u32; }",
    ] {
        let text = format!("machine caller() {{ ghost(); }} {extra}");
        assert_eq!(
            refusal(&typed(&[("client.omg", &text)])),
            Some(DeferredRequest::UndeclaredCallName),
            "{text}"
        );
        let text = format!("machine caller() -> u32 {{ ghost }} {extra}");
        assert_eq!(
            refusal(&typed(&[("client.omg", &text)])),
            Some(DeferredRequest::UndeclaredValueName),
            "{text}"
        );
    }
    let program = typed(&[
        ("client.omg", "machine caller() { ghost(); }"),
        ("other.omg", "module elsewhere; public machine ghost() {}"),
    ]);
    assert_eq!(refusal(&program), Some(DeferredRequest::UndeclaredCallName));
    let program = typed(&[
        (
            "client.omg",
            "use elsewhere; machine caller() { elsewhere::ghost(); }",
        ),
        ("other.omg", "module elsewhere; public machine ghost() {}"),
    ]);
    assert_eq!(refusal(&program), None);
    let program = typed(&[(
        "client.omg",
        "machine caller() requires false { ghost(); } machine unrelated(ghost: u32) {}",
    )]);
    assert_eq!(refusal(&program), Some(DeferredRequest::UndeclaredCallName));
    let program = typed(&[(
        "client.omg",
        "machine caller() requires ghost() {} machine unrelated(ghost: u32) {}",
    )]);
    assert_eq!(refusal(&program), Some(DeferredRequest::UndeclaredCallName));
}

#[test]
fn qualified_unknown_names_keep_dependency_and_visibility_scope() {
    use crate::source_state::deferred_refusals::DeferredRequest;
    for spelling in ["ghost", "foreign::ghost"] {
        let text = format!("machine caller() {{ {spelling}(); }}");
        let program = packages(
            &[
                (31, "client.omg", &text),
                (
                    41,
                    "foreign.omg",
                    "module foreign; public machine ghost() {}",
                ),
            ],
            &[],
            &[],
        )
        .unwrap();
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .unwrap();
        assert_eq!(
            program
                .deferred_refusal(caller.symbol)
                .map(|row| row.request),
            Some(DeferredRequest::UndeclaredCallName),
            "{spelling}"
        );
    }
}

/// modules.md, "Resolution": case members are not a bare-name resolution
/// step, so a bare case spelling is an ordinary unknown name of its own
/// holder. Whether some carrier anywhere in the forest, visible or not,
/// declares that case does not turn the deferred row into a resolver failure.
#[test]
fn a_bare_case_spelling_stays_an_unknown_name_of_its_holder() {
    use crate::source_state::deferred_refusals::DeferredRequest;
    let client = "machine main() {} machine unused() -> u64 { Missing }";
    let carrier = "public data Choice { case Missing; }";
    let same_source = format!("{client} {carrier}");
    for units in [
        vec![(31, "client.omg", client)],
        vec![
            (31, "client.omg", client),
            (
                41,
                "choice.omg",
                "module choice; public data Choice { case Missing; }",
            ),
        ],
        vec![(31, "client.omg", same_source.as_str())],
    ] {
        let program = packages(&units, &[], &[]).unwrap();
        let refusal = |name: &str| {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap();
            program
                .deferred_refusal(machine.symbol)
                .map(|row| row.request)
        };
        assert_eq!(refusal("main"), None, "{units:?}");
        assert_eq!(
            refusal("unused"),
            Some(DeferredRequest::UndeclaredValueName),
            "{units:?}"
        );
    }
}

#[test]
fn selected_operator_is_an_ordinary_call_with_its_exact_source_choice() {
    use crate::declaration_selection::AuthoredDeclarationSelectionKind;
use crate::declaration_selection::AuthoredDeclarationSelectionTarget;
    use crate::source_state::expression::ExpressionNode;
    let declaration = format!("use units; {OPERATOR}");
    let program = typed(&[
        ("units.omg", DOMAIN),
        ("operator.omg", &declaration),
        (
            "client.omg",
            "use units; machine original(value: u32 in Distance) -> u32 in Distance { value } machine sum(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { original(left) + right }",
        ),
    ]);
    let target = program.machine_token_bindings()[0].symbol;
    let calls = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target_symbol == target => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one selected operator call: {calls:?}");
    };
    let selection = program
        .authored_declaration_selections()
        .get(call.authored_call_selection.unwrap())
        .unwrap();
    assert_eq!(selection.kind(), AuthoredDeclarationSelectionKind::Operator);
    assert!(
        matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(selected) if selected.selected_symbol() == target)
    );
    let [left, right] = program.expression_table.expression_handles(call.arguments) else {
        panic!("two ordered operands");
    };
    assert!(
        matches!(program.expression_table.expression(*left), ExpressionNode::Call(inner) if inner.target.as_str() == "original")
    );
    assert!(matches!(
        program.expression_table.expression(*right),
        ExpressionNode::Name(_)
    ));
}

#[test]
fn imported_domain_operator_keeps_its_exact_home_and_lexical_module() {
    let program = typed(&[
        (
            "package/units.omg",
            "module units; public domain u32::Distance;",
        ),
        (
            "package/main.omg",
            "use units; public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { ((left as -Distance) +% (right as -Distance)) as +Distance }",
        ),
    ]);
    let domain = program.domain_definitions().first().unwrap();
    let binding = program.machine_token_bindings().first().unwrap();
    assert_eq!(binding.home_domain, domain.symbol);
    assert!(!program.symbols.symbol_module(binding.symbol).is_valid());
    assert!(domain.semantic_roles.denotation_dimension.is_none());
}

#[test]
fn an_enclosing_domain_cast_reads_the_selected_operators_declared_return() {
    use crate::source_state::expression::ExpressionNode;
    let declaration = format!("use units; {OPERATOR}");
    let program = typed(&[
        ("units.omg", DOMAIN),
        ("operator.omg", &declaration),
        (
            "client.omg",
            "use units; machine sum(left: u32 in Distance, right: u32 in Distance) -> u32 { (left + right) as -Distance }",
        ),
    ]);
    assert!(
        program.deferred_refusals.is_empty(),
        "{:?}",
        program.deferred_refusals
    );
    let target = program.machine_token_bindings()[0].symbol;
    assert!(program.expression_table.iter_expressions().any(|(_, node)| {
        matches!(node, ExpressionNode::Cast(cast) if cast.result_type.is_valid()
            && matches!(program.expression_table.expression(cast.value), ExpressionNode::Call(call) if call.target_symbol == target))
    }));
}

#[test]
fn operator_can_be_declared_beside_the_domain_or_in_another_owning_module() {
    for declaration in [
        format!("{DOMAIN} {OPERATOR}"),
        format!("module operations; use units; {OPERATOR}"),
    ] {
        let units = if declaration.starts_with(DOMAIN) {
            vec![("units.omg", declaration.as_str())]
        } else {
            vec![
                ("units.omg", DOMAIN),
                ("operations.omg", declaration.as_str()),
            ]
        };
        let program = typed(&units);
        assert_eq!(
            program.machine_token_bindings()[0].home_domain,
            program.domain_definitions()[0].symbol
        );
    }
}

#[test]
fn unknown_and_ambiguous_homes_reject_without_any_operator_use() {
    let unknown = format!(
        "use units; {}",
        OPERATOR.replace("Distance::add", "Missing::add")
    );
    let error = checked(&[("units.omg", DOMAIN), ("main.omg", &unknown)])
        .err()
        .unwrap();
    assert!(error.contains("Missing"), "{error}");

    let ambiguous = format!(
        "use one; use two; {}",
        OPERATOR.replace("u32 in Distance", "u32 in one::u32::Distance")
    );
    assert!(
        checked(&[
            ("one.omg", "module one; public domain u32::Distance;"),
            ("two.omg", "module two; public domain u32::Distance;"),
            ("main.omg", &ambiguous),
        ])
        .is_err()
    );
}

#[test]
fn foreign_package_cannot_declare_the_imported_domains_operator() {
    let declaration = format!("use dep::units; {OPERATOR}");
    let error = packages(
        &[(31, "units.omg", DOMAIN), (32, "main.omg", &declaration)],
        &[(32, "dep", 31)],
        &[(1, 0, "dep::units", 1)],
    )
    .err()
    .unwrap();
    assert!(error.contains("another package owns"), "{error}");
}

#[test]
fn duplicate_shapes_across_owning_modules_reject_without_a_use() {
    let first = format!("module first; use units; {OPERATOR}");
    let second = format!(
        "module second; use units; {}",
        OPERATOR.replace("Distance::add", "Distance::another")
    );
    let error = checked(&[
        ("units.omg", DOMAIN),
        ("first.omg", &first),
        ("second.omg", &second),
    ])
    .err()
    .unwrap();
    assert!(error.contains("same owner and operand shape"), "{error}");
}

#[test]
fn a_domain_home_must_participate_in_the_operand_tuple() {
    let declaration =
        "use units; machine + Distance::add(left: u32, right: u32) -> u32 { left +% right }";
    let error = checked(&[("units.omg", DOMAIN), ("main.omg", declaration)])
        .err()
        .unwrap();
    assert!(
        error.contains("no operand carries its home domain"),
        "{error}"
    );
}

#[test]
fn anonymous_arithmetic_does_not_choose_between_unrelated_domain_families() {
    use crate::source_state::expression::ExpressionNode;
    let declaration = format!(
        "use units; {OPERATOR} {}",
        OPERATOR.replace("Distance", "Span")
    );
    let program = typed(&[
        (
            "units.omg",
            "module units; public domain u32::Distance; public domain u32::Span;",
        ),
        ("operator.omg", &declaration),
        (
            "client.omg",
            "machine sum() -> u32 { 2 + 3 } machine difference() -> f64 { 0.0 - 1.57079632679489656 }",
        ),
    ]);
    assert!(program.expression_table.iter_expressions().any(|(_, expression)| {
        matches!(expression, ExpressionNode::Binary(binary) if matches!(program.expression_table.expression(binary.left), ExpressionNode::Integer(_)))
    }));
}

#[test]
fn body_imports_belong_to_the_declaring_file() {
    use crate::source_state::expression::ExpressionNode;
    let body = "machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { helper((left as -Distance) +% (right as -Distance)) as +Distance }";
    let own_import = format!("use units; use helpers::helper; {body}");
    let borrowed_import = format!("use units; {body}");
    let domain = "module units; use helpers::helper; public domain u32::Distance;";
    let helpers = "module helpers; public machine helper(value: u32) -> u32 { value }";
    let own = typed(&[
        ("units.omg", domain),
        ("helpers.omg", helpers),
        ("main.omg", &own_import),
    ]);
    let borrowed = typed(&[
        ("units.omg", domain),
        ("helpers.omg", helpers),
        ("main.omg", &borrowed_import),
    ]);
    let targets = |program: &crate::source_state::TypedTrees| {
        program
            .expression_table
            .iter_expressions()
            .filter_map(|(_, node)| match node {
                ExpressionNode::Call(call) if call.target.as_str() == "helper" => {
                    Some(call.target_symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let own_targets = targets(&own);
    assert!(!own_targets.is_empty());
    assert!(
        own_targets.iter().all(|target| target.is_valid()),
        "{own_targets:?}"
    );
    let borrowed_targets = targets(&borrowed);
    assert!(!borrowed_targets.is_empty());
    assert!(
        borrowed_targets.iter().all(|target| !target.is_valid()),
        "association borrowed another file's imports"
    );
}

#[test]
fn domain_family_discovery_preserves_visibility_without_importing_its_module() {
    use language_core::operator_spelling::OperatorSpelling;
    for (public, expected) in [(true, 1), (false, 0)] {
        let declaration = format!(
            "module operations; use units; {} {}",
            if public {
                OPERATOR.to_owned()
            } else {
                OPERATOR.replacen("public ", "", 1)
            },
            OPERATOR.replace("Distance", "Span"),
        );
        let program = packages(&[
            (31, "units.omg", "module units; public domain u32::Distance; public domain u32::Span;"),
            (31, "operations.omg", &declaration),
            (32, "client.omg", "use dep::units; machine sum(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { left }"),
        ], &[(32, "dep", 31)], &[(2, 0, "dep::units", 1)]).unwrap();
        let binding = &program.machine_token_bindings()[0];
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "sum")
            .unwrap();
        let entry = program.machine_states(caller).first().unwrap();
        let operands = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| Some(parameter.type_reference))
            .collect::<Vec<_>>();
        let at = program
            .symbols
            .symbol_provenance_source_span(caller.symbol)
            .unwrap();
        let candidates = crate::source_state::operator::resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Add,
            &operands,
            at,
        );
        assert_eq!(candidates.len(), expected, "public={public}");
        if let Some(selected) = candidates.first() {
            assert_eq!(selected.operator.symbol, binding.symbol);
            assert_eq!(selected.domain.unwrap().symbol, binding.home_domain);
        }
    }
}
