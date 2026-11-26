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
    )
}

pub(crate) fn checked_with_equality(
    units: &[(&str, &str)],
) -> Result<crate::source_state::TypedTrees, String> {
    let units: Vec<_> = units.iter().map(|&(path, text)| (31, path, text)).collect();
    packages_with_equality(&units, &[])
}

pub(crate) fn packages_with_equality(
    units: &[(u8, &str, &str)],
    dependencies: &[(u8, &str, u8)],
) -> Result<crate::source_state::TypedTrees, String> {
    let mut sources = vec![(
        0,
        "equality.omg",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/core/equality.omg"
        )),
    )];
    sources.extend_from_slice(units);
    packages(&sources, dependencies)
}

pub(crate) fn packages(
    units: &[(u8, &str, &str)],
    dependencies: &[(u8, &str, u8)],
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
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let parsed = crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        parsed_units.push(parsed);
    }
    sources.retain_product_dependency_scope(
        dependencies
            .iter()
            .map(|(from, alias, to)| (identity(*from), *alias, identity(*to))),
    );
    let mut top_level_bindings = Vec::new();
    // Bind each authored file alias to its selected source, as package admission does.
    for ((owner, _, _), unit) in units.iter().zip(&parsed_units) {
        for item in unit.root_items() {
            let syntax_trees::item::Item::Use(syntax_trees::item::UseItem::File {
                root,
                components,
                alias,
                ..
            }) = item
            else {
                continue;
            };
            let mut components = unit
                .items
                .identifier_path_members(*components)
                .iter()
                .map(|part| part.as_str(unit));
            let root = root.as_ref().map(|name| name.as_str(unit));
            let dependency = root.filter(|root| *root != "omega").map(|alias| {
                let (_, _, target) = dependencies
                    .iter()
                    .find(|(from, name, _)| from == owner && *name == alias)
                    .expect("the requester declares the dependency alias");
                (alias, *target)
            });
            let target_owner = match root {
                Some("omega") => {
                    assert_eq!(
                        (components.next(), components.next()),
                        (Some("language"), Some("core"))
                    );
                    0
                }
                Some(_) => dependency.unwrap().1,
                None => *owner,
            };
            let mut path = components.collect::<std::path::PathBuf>();
            path.set_extension("omg");
            let target = units
                .iter()
                .position(|(owner, file, _)| {
                    *owner == target_owner && std::path::Path::new(file) == path
                })
                .expect("the fixture supplies the imported file");
            top_level_bindings.push(
                crate::source_symbols::SourceScopedTopLevelBinding::file_import(
                    unit.source_id,
                    source_ids[target],
                    alias.as_str(unit),
                )
                .from_dependency(dependency.map(|(alias, package)| (alias, identity(package)))),
            );
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
    crate::lower_symbol_resolved_trees(&resolved).map_err(|error| error.message)
}

const DOMAIN: &str = "public domain u32::Distance;";
const LOCAL_OPERATOR: &str = "public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { ((left as -Distance) +% (right as -Distance)) as +Distance }";
const OPERATOR: &str = "public machine + |units|::Distance::add(left: u32 in |units|::Distance, right: u32 in |units|::Distance) -> u32 in |units|::Distance { ((left as - |units|::Distance) +% (right as - |units|::Distance)) as + |units|::Distance }";

const GENERIC_DOMAIN: &str = "public domain<T> T::Distance;";
const GENERIC_OPERATOR: &str = "use units/domains.omg as units; public machine + |units|::Distance::add<T [copy]>(left: T in |units|::Distance, right: T) -> T in |units|::Distance requires T == u32 { ((left as - |units|::Distance) +% right) as + |units|::Distance }";

fn bundled_core_sources() -> Vec<(String, String)> {
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../../source/library/core");
    let mut directories = vec![core.clone()];
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "omg") {
                let relative = path
                    .strip_prefix(&core)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                files.push((relative, std::fs::read_to_string(path).unwrap()));
            }
        }
    }
    files.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    files
}

#[test]
fn bundled_core_fixtures_resolve_their_authored_imports() {
    let files = bundled_core_sources();
    let mut units = files
        .iter()
        .map(|(name, text)| (0, name.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    units.push((
        31,
        "unimported.omg",
        "machine unimported(value: f32) { classify_f32(value); }",
    ));
    units.push((31, "imported.omg", "use {omega}/language/core/float_accuracy/float_accuracy.omg; machine imported(value: f32) { |float_accuracy|::classify_f32(value); }"));
    let program = packages(&units, &[])
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
fn core_policy_casts_of_imported_call_results_retain_the_subtraction_signature() {
    use crate::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionTarget as Target,
    };
    use crate::source_state::expression::{BinaryOperator, ExpressionNode};
    use numerics::intrinsics::{
        CoreIntegerOperand, CoreIntegerOperatorSignature, ScalarBinaryOperation,
    };
    use numerics::{arithmetic::ArithmeticDomain, integer_carrier::FixedInteger};

    let files = bundled_core_sources();
    let mut units = files
        .iter()
        .map(|(name, text)| (0, name.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    units.push((
        31,
        "difference.omg",
        r#"
use {omega}/language/core/numeric_conversion.omg;
data Time [copy] { seconds: i64; }
machine difference(time: @Time, earlier: Time) -> u64 in Wrapping {
    let seconds: u64 in Wrapping =
        (|numeric_conversion|::narrow_i64_to_u64_wrapping(time.seconds) as +Wrapping)
        - (|numeric_conversion|::narrow_i64_to_u64_wrapping(earlier.seconds) as +Wrapping);
    seconds
}
"#,
    ));
    let program = packages(&units, &[])
    .expect("the actual Core converter and its authored imports type successfully");
    let owner = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "difference")
        .unwrap()
        .symbol;
    let owners = crate::source_state::references::collect_program_operand_owners(&program);
    let subtractions = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| {
            (matches!(node, ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::Subtract)
                && owners.expression_owners(handle).any(|found| found == owner))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    let [subtraction] = subtractions.as_slice() else {
        panic!("one authored subtraction belongs to difference: {subtractions:?}");
    };
    let operand = CoreIntegerOperand {
        carrier: FixedInteger::unsigned(64),
        policy: ArithmeticDomain::Wrapping,
    };
    let expected =
        CoreIntegerOperatorSignature::select(ScalarBinaryOperation::Subtract, [operand; 2])
            .unwrap();
    let selected = program
        .expression_table
        .authored_selection_occurrences(*subtraction)
        .map(|occurrence| {
            program
                .authored_declaration_selections()
                .get(occurrence)
                .unwrap()
                .target()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected,
        vec![Target::Intrinsic(Intrinsic::CoreIntegerOperator(expected))],
        "the imported call results retain the complete selected policy, not CheckedOperator"
    );
}

#[test]
fn core_policy_comparisons_read_selected_collection_lengths() {
    use crate::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionTarget as Target,
    };
    use crate::source_state::expression::{BinaryOperator, ExpressionNode};
    use numerics::intrinsics::{
        CoreIntegerOperand, CoreIntegerOperatorSignature, ScalarBinaryOperation,
    };
    use numerics::{arithmetic::ArithmeticDomain, integer_carrier::FixedInteger};

    let files = bundled_core_sources();
    let mut units = files
        .iter()
        .map(|(name, text)| (0, name.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    units.push((
        31,
        "lengths.omg",
        r#"
machine equal_length(items: @[u8], count: u64 in Trapping) -> bool {
    count == items.length
}
machine bounded_length(items: @[u8], count: u64 in Trapping) -> bool
    requires count < items.length
{ true }
machine reversed_length(items: @[u8], count: u64 in Trapping) -> bool {
    items.length > count
}
machine fixed_length(items: @[u8; 4], count: u64 in Trapping) -> bool {
    count <= items.length
}
"#,
    ));
    let program = packages(&units, &[]).expect("the actual Core declarations type");
    let owners = crate::source_state::references::collect_program_operand_owners(&program);
    for (name, binary, operation, reversed) in [
        (
            "equal_length",
            BinaryOperator::Equal,
            ScalarBinaryOperation::Equal,
            false,
        ),
        (
            "bounded_length",
            BinaryOperator::Less,
            ScalarBinaryOperation::Less,
            false,
        ),
        (
            "reversed_length",
            BinaryOperator::Greater,
            ScalarBinaryOperation::Greater,
            true,
        ),
        (
            "fixed_length",
            BinaryOperator::LessOrEqual,
            ScalarBinaryOperation::LessOrEqual,
            false,
        ),
    ] {
        let owner = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap()
            .symbol;
        let expressions = program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, node)| {
                (matches!(node, ExpressionNode::Binary(found) if found.operator == binary)
                    && owners.expression_owners(handle).any(|found| found == owner))
                .then_some(handle)
            })
            .collect::<Vec<_>>();
        let [expression] = expressions.as_slice() else {
            panic!("one authored comparison in {name}: {expressions:?}");
        };
        let operand = |policy| CoreIntegerOperand {
            carrier: FixedInteger::unsigned(64),
            policy,
        };
        let mut operands = [
            operand(ArithmeticDomain::Trapping),
            operand(ArithmeticDomain::Exact),
        ];
        if reversed {
            operands.reverse();
        }
        let expected = CoreIntegerOperatorSignature::select(operation, operands).unwrap();
        let selected = program
            .expression_table
            .authored_selection_occurrences(*expression)
            .map(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .unwrap()
                    .target()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            vec![Target::Intrinsic(Intrinsic::CoreIntegerOperator(expected))],
            "{name} must retain its exact mixed-policy comparison rather than CheckedOperator"
        );
    }
}

#[test]
fn local_initializers_retain_one_read_per_written_expression() {
    use crate::source_state::expression::{ExpressionNode, MemberSelection};
    use crate::source_state::statement::StatementNode;

    let program = typed(&[(
        "members.omg",
        r#"
data Row [copy] { length: u16 in Wrapping; }
machine position() -> u64 { 0 }
machine field(row: @Row) -> u16 in Wrapping { let value: u16 in Wrapping = row.length; value }
machine measure(items: @[u8]) -> u64 { let value: u64 = items.length; value }
machine element(items: @[u8]) -> u8 { let value: u8 = items[position()]; value }
"#,
    )]);
    let owners = crate::source_state::references::collect_program_operand_owners(&program);
    let mut measures = 0;
    let mut fields = 0;
    let mut selectors = 0;
    for (handle, node) in program.expression_table.iter_expressions() {
        match node {
            ExpressionNode::Member(member) => match member.selection {
                MemberSelection::Field(_) => fields += 1,
                MemberSelection::CollectionMeasure(
                    crate::declaration_selection::CollectionMeasure::Length,
                ) => measures += 1,
                other => panic!("unselected member {other:?}"),
            },
            ExpressionNode::Call(call) if call.target.as_str() == "position" => selectors += 1,
            _ => continue,
        }
        assert!(
            owners.expression_owners(handle).next().is_some(),
            "unused typed expression {node:?}"
        );
    }
    assert_eq!((fields, measures, selectors), (1, 1, 1));
    for name in ["field", "measure", "element"] {
        let machine = program.realized_machine_named(name).unwrap();
        let state = &program.machine_states(machine)[0];
        let local = state
            .statement_nodes
            .handles()
            .find_map(|handle| match program.statement_table.statement(handle) {
                StatementNode::LocalData(local) if local.name.as_str() == "value" => Some(local),
                _ => None,
            })
            .unwrap();
        let types = &program.type_reference_table;
        assert_eq!(
            types.type_symbol(local.type_reference),
            types.type_symbol(state.return_type),
            "{name}"
        );
        assert_eq!(
            types.arithmetic_domain(local.type_reference),
            types.arithmetic_domain(state.return_type),
            "{name}"
        );
    }
}

#[test]
fn collection_length_type_queries_use_the_retained_selection() {
    use crate::declaration_selection::CollectionMeasure;
    use crate::source_state::expression::{ExpressionNode, MemberSelection};
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::integer_carrier::FixedInteger;
    use symbols::BuiltinTypeAtom;

    let mut program = typed(&[(
        "members.omg",
        r#"
data Row [copy] { length: u16 in Wrapping; }
machine selected(items: @[u8]) -> u64 { items.length }
machine field(row: @Row) -> u16 in Wrapping { row.length }
"#,
    )]);
    for (name, atom, policy) in [
        ("selected", BuiltinTypeAtom::U64, ArithmeticDomain::Exact),
        (
            "field",
            BuiltinTypeAtom::Integer(FixedInteger::unsigned(16)),
            ArithmeticDomain::Wrapping,
        ),
    ] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let owner = machine.symbol;
        let state = program.machine_states(machine).first().unwrap().clone();
        let owners = crate::source_state::references::collect_program_operand_owners(&program);
        let members = program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, node)| {
                (matches!(node, ExpressionNode::Member(_))
                    && owners.expression_owners(handle).any(|found| found == owner))
                .then_some(handle)
            })
            .collect::<Vec<_>>();
        let [member] = members.as_slice() else {
            panic!("one selected member in {name}");
        };
        let member = *member;
        let expected = program
            .type_reference_table
            .find_arithmetic_result_type_reference(
                program.symbols.builtin_atom_symbol(atom).unwrap(),
                policy,
            )
            .unwrap();
        let query = |program: &crate::source_state::TypedTrees| {
            assert_eq!(
                crate::types::typing_queries::declared_expression_type(program, owner, member),
                Some(expected)
            );
            assert_eq!(
                crate::types::typing_queries::scoped_expression_type_reference(
                    program,
                    program.state_parameters(&state),
                    state.return_type,
                    member,
                    Some(&state)
                )
                .unwrap(),
                Some(expected)
            );
        };
        query(&program);
        let ExpressionNode::Member(node) = program.expression_table.expression_mut(member) else {
            unreachable!()
        };
        let selected = node.selection;
        node.member = "diagnostic_spelling_only".into();
        query(&program);
        let ExpressionNode::Member(node) = program.expression_table.expression_mut(member) else {
            unreachable!()
        };
        node.member = "length".into();
        node.selection = MemberSelection::Unresolved;
        assert_eq!(
            crate::types::typing_queries::declared_expression_type(&program, owner, member),
            None
        );
        assert_eq!(
            crate::types::typing_queries::scoped_expression_type_reference(
                &program,
                program.state_parameters(&state),
                state.return_type,
                member,
                Some(&state)
            )
            .unwrap(),
            None
        );
        let ExpressionNode::Member(node) = program.expression_table.expression_mut(member) else {
            unreachable!()
        };
        node.selection = if name == "selected" {
            MemberSelection::CollectionMeasure(CollectionMeasure::Capacity)
        } else {
            MemberSelection::CollectionMeasure(CollectionMeasure::Length)
        };
        assert_eq!(
            crate::types::typing_queries::declared_expression_type(&program, owner, member),
            None,
            "an unsupported measure or a noncollection receiver supplies no type"
        );
        let ExpressionNode::Member(node) = program.expression_table.expression_mut(member) else {
            unreachable!()
        };
        node.selection = selected;
    }
}

#[test]
fn a_carrier_generic_domain_retains_each_exact_application() {
    use crate::source_state::types::{TypeConstraintNode, TypeReferenceNode};
    let program = typed(&[
        ("units/domains.omg", GENERIC_DOMAIN),
        (
            "client.omg",
            "use units/domains.omg as units; machine first(value: u32 in |units|::Distance) -> u32 in |units|::Distance { value } machine second(value: u64 in |units|::Distance) -> u64 in |units|::Distance { value } machine repeated(value: u32 in |units|::Distance) -> u32 in |units|::Distance { value }",
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
        ("units/domains.omg", GENERIC_DOMAIN),
        ("operator.omg", GENERIC_OPERATOR),
        (
            "client.omg",
            "use units/domains.omg as units; machine sum(left: u32 in |units|::Distance, right: u32) -> u32 in |units|::Distance { left + right }",
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
        let client = format!(
            "use units/domains.omg as units; machine sum(right: u32) -> u32 {{ {literal} + right }}"
        );
        let program = typed(&[
            ("units/domains.omg", GENERIC_DOMAIN),
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
        ("units/domains.omg", GENERIC_DOMAIN),
        ("operator.omg", GENERIC_OPERATOR),
        (
            "client.omg",
            "use units/domains.omg as units; machine sum(left: u32 in |units|::Distance, right: u32) -> u32 { (left + right) as - |units|::Distance }",
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
    let declarations = "use units/domains.omg as units;
        public machine + |units|::Distance::both<T [copy]>(left: T in |units|::Distance, right: T in |units|::Distance) -> T in |units|::Distance { left }
        public machine + |units|::Distance::left<T [copy]>(left: T in |units|::Distance, right: T) -> T in |units|::Distance { left }
        public machine + |units|::Distance::right<T [copy]>(left: T, right: T in |units|::Distance) -> T in |units|::Distance { right }";
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
            "u32 in |units|::Distance",
            "u32 in |units|::Distance",
            Ok(Some("|units|::Distance::both")),
        ),
        (
            "u32 in |units|::Distance",
            "u32",
            Ok(Some("|units|::Distance::left")),
        ),
        (
            "u32",
            "u32 in |units|::Distance",
            Ok(Some("|units|::Distance::right")),
        ),
        (
            "u32 in |units|::Distance & |units|::Other",
            "u32",
            unqualified_only(
                "-Distance -Other",
                "u32 in |units|::Distance & |units|::Other",
            ),
        ),
        (
            "u32 in |units|::Distance",
            "u64",
            unqualified_only("-Distance", "u32 in |units|::Distance"),
        ),
        ("u32", "u32", Ok(None)),
    ] {
        let client = format!(
            "use units/domains.omg as units; machine sum(left: {left}, right: {right}) {{ left + right; }}"
        );
        let checked = checked(&[
            (
                "units/domains.omg",
                "public domain<T> T::Distance; public domain<T> T::Other;",
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
        ("units/domains.omg", GENERIC_DOMAIN),
        (
            "operator.omg",
            "use units/domains.omg as units; public machine + |units|::Distance::merge<T [copy], const N: u64>(left: T in |units|::Distance, right: [T; N]) -> T in |units|::Distance { left }",
        ),
        (
            "client.omg",
            "use units/domains.omg as units; machine sum(left: u32 in |units|::Distance, right: [u32; 3]) -> u32 in |units|::Distance { left + right }",
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
        ("elsewhere/ghost.omg", "public machine ghost() {}"),
    ]);
    assert_eq!(refusal(&program), Some(DeferredRequest::UndeclaredCallName));
    let program = typed(&[
        (
            "client.omg",
            "use elsewhere/ghost.omg as elsewhere; machine caller() { |elsewhere|::ghost(); }",
        ),
        ("elsewhere/ghost.omg", "public machine ghost() {}"),
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
                (41, "foreign/ghost.omg", "public machine ghost() {}"),
            ],
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
                "choice/cases.omg",
                "public data Choice { case Missing; }",
            ),
        ],
        vec![(31, "client.omg", same_source.as_str())],
    ] {
        let program = packages(&units, &[]).unwrap();
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
    let declaration = format!("use units/domains.omg as units; {OPERATOR}");
    let program = typed(&[
        ("units/domains.omg", DOMAIN),
        ("operator.omg", &declaration),
        (
            "client.omg",
            "use units/domains.omg as units; machine original(value: u32 in |units|::Distance) -> u32 in |units|::Distance { value } machine sum(left: u32 in |units|::Distance, right: u32 in |units|::Distance) -> u32 in |units|::Distance { original(left) + right }",
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
fn named_constants_select_operators_with_their_declared_type() {
    use crate::source_state::expression::ExpressionNode;
    let declaration = format!("use units/domains.omg as units; {OPERATOR}");
    let program = typed(&[
        ("units/domains.omg", DOMAIN),
        ("operator.omg", &declaration),
        (
            "readings/values.omg",
            r"use units/domains.omg as units;
              machine read() -> u32 in |units|::Distance { 4u32 as + |units|::Distance }
              public const SAMPLE: u32 in |units|::Distance = read();",
        ),
        (
            "client.omg",
            r"use units/domains.omg as units; use readings/values.omg as readings;
              machine summed(value: u32 in |units|::Distance) -> u32 in |units|::Distance { |readings|::SAMPLE + value }
              machine reversed(value: u32 in |units|::Distance) -> u32 in |units|::Distance { value + |readings|::SAMPLE }
              machine shadowed(SAMPLE: u32) -> u32 { SAMPLE + 4u32 }",
        ),
    ]);
    let target = program.machine_token_bindings()[0].symbol;
    let owners = crate::source_state::references::collect_program_operand_owners(&program);
    for (name, expected) in [("summed", 1), ("reversed", 1), ("shadowed", 0)] {
        let owner = program
            .machines()
            .iter()
            .find(|m| m.name.as_str() == name)
            .unwrap()
            .symbol;
        let selected = program
            .expression_table
            .iter_expressions()
            .filter(|(id, node)| {
                matches!(node, ExpressionNode::Call(call) if call.target_symbol == target)
                    && owners.expression_owners(*id).any(|found| found == owner)
            })
            .count();
        assert_eq!(selected, expected, "{name}");
    }
}

#[test]
fn imported_domain_operator_keeps_its_exact_home_and_lexical_module() {
    let program = typed(&[
        ("units/domains.omg", "public domain u32::Distance;"),
        (
            "main.omg",
            "use units/domains.omg as units; public machine + |units|::Distance::add(left: u32 in |units|::Distance, right: u32 in |units|::Distance) -> u32 in |units|::Distance { ((left as - |units|::Distance) +% (right as - |units|::Distance)) as + |units|::Distance }",
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
    let declaration = format!("use units/domains.omg as units; {OPERATOR}");
    let program = typed(&[
        ("units/domains.omg", DOMAIN),
        ("operator.omg", &declaration),
        (
            "client.omg",
            "use units/domains.omg as units; machine sum(left: u32 in |units|::Distance, right: u32 in |units|::Distance) -> u32 { (left + right) as - |units|::Distance }",
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
        format!("{DOMAIN} {LOCAL_OPERATOR}"),
        format!("use units/domains.omg as units; {OPERATOR}"),
    ] {
        let units = if declaration.starts_with(DOMAIN) {
            vec![("units/domains.omg", declaration.as_str())]
        } else {
            vec![
                ("units/domains.omg", DOMAIN),
                ("operations/operators.omg", declaration.as_str()),
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
fn unknown_and_mismatched_homes_reject_without_any_operator_use() {
    let unknown = format!(
        "use units/domains.omg as units; {}",
        OPERATOR.replace("Distance::add", "Missing::add")
    );
    let error = checked(&[("units/domains.omg", DOMAIN), ("main.omg", &unknown)])
        .err()
        .unwrap();
    assert!(error.contains("Missing"), "{error}");

    let ambiguous = format!(
        "use one/domain.omg as units; use two/domain.omg as two; {}",
        OPERATOR.replace("in |units|::Distance", "in |two|::Distance")
    );
    assert!(
        checked(&[
            ("one/domain.omg", "public domain u32::Distance;"),
            ("two/domain.omg", "public domain u32::Distance;"),
            ("main.omg", &ambiguous),
        ])
        .is_err()
    );
}

#[test]
fn foreign_package_cannot_declare_the_imported_domains_operator() {
    let declaration = format!("use {{dep}}/units/domains.omg as units; {OPERATOR}");
    let error = packages(
        &[
            (31, "units/domains.omg", DOMAIN),
            (32, "main.omg", &declaration),
        ],
        &[(32, "dep", 31)],
    )
    .err()
    .unwrap();
    assert!(error.contains("another package owns"), "{error}");
}

#[test]
fn duplicate_shapes_across_owning_modules_reject_without_a_use() {
    let first = format!("use units/domains.omg as units; {OPERATOR}");
    let second = format!(
        "use units/domains.omg as units; {}",
        OPERATOR.replace("Distance::add", "Distance::another")
    );
    let error = checked(&[
        ("units/domains.omg", DOMAIN),
        ("first/operator.omg", &first),
        ("second/operator.omg", &second),
    ])
    .err()
    .unwrap();
    assert!(error.contains("same owner and operand shape"), "{error}");
}

#[test]
fn a_domain_home_must_participate_in_the_operand_tuple() {
    let declaration = "use units/domains.omg as units; machine + |units|::Distance::add(left: u32, right: u32) -> u32 { left +% right }";
    let error = checked(&[("units/domains.omg", DOMAIN), ("main.omg", declaration)])
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
        "use units/domains.omg as units; {OPERATOR} {}",
        OPERATOR.replace("Distance", "Span")
    );
    let program = typed(&[
        (
            "units/domains.omg",
            "public domain u32::Distance; public domain u32::Span;",
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
    let body = "machine + |units|::Distance::add(left: u32 in |units|::Distance, right: u32 in |units|::Distance) -> u32 in |units|::Distance { |helpers|::helper((left as - |units|::Distance) +% (right as - |units|::Distance)) as + |units|::Distance }";
    let own_import =
        format!("use units/domains.omg as units; use helpers/helper.omg as helpers; {body}");
    let borrowed_import = format!("use units/domains.omg as units; {body}");
    let domain = "use helpers/helper.omg as helpers; public domain u32::Distance;";
    let helpers = "public machine helper(value: u32) -> u32 { value }";
    let own = typed(&[
        ("units/domains.omg", domain),
        ("helpers/helper.omg", helpers),
        ("main.omg", &own_import),
    ]);
    let borrowed = typed(&[
        ("units/domains.omg", domain),
        ("helpers/helper.omg", helpers),
        ("main.omg", &borrowed_import),
    ]);
    let targets = |program: &crate::source_state::TypedTrees| {
        program
            .expression_table
            .iter_expressions()
            .filter_map(|(_, node)| match node {
                ExpressionNode::Call(call) if call.target.as_str().ends_with("helper") => {
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
            "use units/domains.omg as units; {} {}",
            if public {
                OPERATOR.to_owned()
            } else {
                OPERATOR.replacen("public ", "", 1)
            },
            OPERATOR.replace("Distance", "Span"),
        );
        let program = packages(&[
            (31, "units/domains.omg", "public domain u32::Distance; public domain u32::Span;"),
            (31, "operations/operators.omg", &declaration),
            (32, "client.omg", "use {dep}/units/domains.omg as units; machine sum(left: u32 in |units|::Distance, right: u32 in |units|::Distance) -> u32 in |units|::Distance { left }"),
        ], &[(32, "dep", 31)]).unwrap();
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

#[test]
fn the_typed_generic_bound_decides_if_a_local_name_conflicts() {
    for body in [
        "machine pick<const current: u32>(sample: u32) -> u32 { match sample { current -> current, _ -> 0u32 } }",
        "machine pick<const current: u32>(sample: u32) -> u32 { let current: u32 = sample; current }",
        "machine pick<const current: u32>(current: u32) -> u32 { current }",
        "machine pick<const current: u32>(sample: u32) -> u32 { transition (sample, sample) { (current, _) -> current, _ -> 0u32 } }",
        "data Reading { value: u32; } machine pick<const current: u32>(reading: Reading) -> u32 { match reading { Reading { value as current } -> current } }",
    ] {
        let negative = format!(
            "data Bound = u32; {}",
            body.replace("const current: u32", "const current: Bound")
        );
        let error = checked(&[("main.omg", &negative)]).unwrap_err();
        assert!(
            error.contains("binding `current` conflicts with an existing name: a static value"),
            "{error}"
        );
    }
    checked(&[(
        "main.omg",
        "machine pick<current>(sample: u32) -> u32 { let current: u32 = sample; current }",
    )])
    .unwrap();
    for bound in ["|proof|::Type<0>", "Sort"] {
        let positive = format!(
            "use {{omega}}/language/core/foundation.omg as proof; data Sort = |proof|::Type<0>; machine pick<const current: {bound}>(sample: u32) -> u32 {{ let current: u32 = sample; current }}"
        );
        packages(
            &[
                (
                    0,
                    "foundation.omg",
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../../../../../source/library/core/foundation.omg"
                    )),
                ),
                (31, "main.omg", &positive),
            ],
            &[],
        )
        .unwrap();
    }
}
