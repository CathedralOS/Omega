/* Relabel authored uses only from the exact expected-type context available here. */

mod alias_constructors {
    /*
        Infer a qualified alias constructor from its complete expected application.
        The alias is never a nominal carrier or owner of synthetic case symbols.
    */

    use super::super::{Instantiation, SyntaxTrees, TypeReferenceHandle};
    use crate::preparation::declaration_selection::ConstantSelection;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{DataMember, ItemHandle};

    pub(super) struct AliasConstructor {
        pub template: ItemHandle,
        pub case: Option<Identifier>,
        pub application: TypeReferenceHandle,
        pub carrier: Identifier,
    }

    pub(super) fn select(
        syntax: &mut SyntaxTrees,
        selection: Option<&ConstantSelection<'_>>,
        expected: TypeReferenceHandle,
        instance: &Instantiation,
        authored: &Identifier,
        payload_free: bool,
    ) -> Option<AliasConstructor> {
        let selection = selection?;
        if payload_free && selection.constructor_prefix_is_value(syntax, authored) {
            return None;
        }
        let (carrier, case) = if selection.type_alias_source(syntax, authored).is_some() {
            (authored.clone(), None)
        } else {
            let (prefix, case) = authored.as_str(syntax).rsplit_once("::")?;
            let mut span = authored.source_span();
            span.span.end = span.span.start + prefix.len();
            (
                Identifier::new(prefix, span),
                Some(Identifier::new(case, authored.trailing_span(case.len()))),
            )
        };
        let declaration = selection.type_alias_source(syntax, &carrier)?;
        let mut origins = syntax
            .type_references
            .type_alias_applications(expected)
            .filter(|origin| origin.declaration == declaration);
        origins.next()?;
        if origins.next().is_some() {
            return None;
        }
        drop(origins);
        let template = syntax.root_item(instance.template).as_data()?;
        if let Some(case) = &case {
            let mut variants = syntax
                .items
                .data_members(template.members)
                .iter()
                .filter_map(|member| {
                    if let DataMember::Variant(variant) = member {
                        (variant.name.as_str(syntax) == case.as_str(syntax)).then_some(variant)
                    } else {
                        None
                    }
                });
            let variant = variants.next()?;
            if variants.next().is_some() || (payload_free && !variant.payload.is_empty()) {
                return None;
            }
        } else if payload_free {
            return None;
        }
        let lookup =
            selection.data_lookup_path_for_declaration(syntax, &template.name, &carrier)?;
        let lookup = lookup.rsplit_once("::").map_or_else(
            || instance.synthetic_name.clone(),
            |(prefix, _)| format!("{prefix}::{}", instance.synthetic_name),
        );
        let application =
            syntax
                .type_references
                .alias_use_at(expected, declaration, carrier.clone())?;
        Some(AliasConstructor {
            template: instance.template,
            case,
            application,
            carrier: Identifier::new(lookup, carrier.source_span()),
        })
    }
}
mod assignments {
    /* Expected types from constant/local declarations and assignments. */
    use super::super::{ExpressionNode, Item, StatementNode, SyntaxTrees};
    use crate::preparation::generic_data::uses::{
        ConstructorFrontier, concrete_state_local_types, relabel_data_literal_for_expected_type,
    };

    use crate::preparation::declaration_selection;
    use crate::preparation::generic_data::Instantiation;

    /*
        A constant's declared type constrains its constructors before the value is
        copied into consumers. Otherwise a closed `Box<u64>` declaration can leave
        an open `Box<T>` constructor and field identity in every later projection.
        Reuse the exact-owner join used by local destinations; a matching shape or
        name cannot substitute an unrelated template. Retained authored initializers
        need the same closed identity for independent evaluation replay.
    */
    pub(in crate::preparation::generic_data) fn relabel_closed_data_uses_in_constants(
        syntax: &mut SyntaxTrees,
        instances: &[Instantiation],
        selection: Option<&declaration_selection::ConstantSelection>,
        declarations: &[syntax_trees::item::ItemHandle],
    ) {
        let constants = declarations
            .iter()
            .map(|declaration| syntax.root_item(*declaration))
            .filter_map(|item| {
                let Item::Const(constant) = item else {
                    return None;
                };
                Some((
                    constant.value,
                    constant.type_reference,
                    constant
                        .normalization
                        .as_ref()
                        .map(|normalization| normalization.authored_expression),
                ))
            })
            .collect::<Vec<_>>();
        let frontier = ConstructorFrontier {
            parameters: Default::default(),
            prior_statements: &[],
        };
        for (value, destination, original) in constants {
            relabel_data_literal_for_expected_type(
                syntax,
                value,
                destination,
                instances,
                selection,
                &frontier,
            );
            if let Some(original) = original {
                relabel_data_literal_for_expected_type(
                    syntax,
                    original,
                    destination,
                    instances,
                    selection,
                    &frontier,
                );
            }
        }
    }

    /*
        An assignment target is another explicit destination type. Relabel a bare
        generic literal only when that type is available directly from a local or
        parameter; computed targets remain fail-closed.
    */
    pub(in crate::preparation::generic_data) fn relabel_closed_data_uses_in_exact_assignments(
        syntax: &mut SyntaxTrees,
        instances: &[Instantiation],
        selection: Option<&declaration_selection::ConstantSelection>,
        declarations: &[syntax_trees::item::ItemHandle],
    ) {
        for (state, local_types, statements) in concrete_state_local_types(syntax, declarations) {
            for (index, statement) in statements.iter().enumerate() {
                let frontier = ConstructorFrontier {
                    parameters: state.parameters,
                    prior_statements: &statements[..index],
                };
                let assignment = match syntax.statements.statement(*statement) {
                    StatementNode::LocalData(local) => {
                        relabel_data_literal_for_expected_type(
                            syntax,
                            local.initial_value,
                            local.type_reference,
                            instances,
                            selection,
                            &frontier,
                        );
                        continue;
                    }
                    StatementNode::Assignment(assignment) => *assignment,
                    _ => continue,
                };
                let expected_type = match syntax.expressions.expression(assignment.target) {
                    ExpressionNode::Name(path) => {
                        let [name] = syntax.expressions.identifier_path_members(*path) else {
                            continue;
                        };
                        local_types.get(name.as_str(syntax)).copied()
                    }
                    _ => None,
                };
                if let Some(expected_type) = expected_type {
                    relabel_data_literal_for_expected_type(
                        syntax,
                        assignment.value,
                        expected_type,
                        instances,
                        selection,
                        &frontier,
                    );
                }
            }
        }
    }
}
mod calls {
    /* Expected types from calls, returns and receiver paths. */
    use super::super::{
        ExpressionHandle, ExpressionNode, HandleSpan, HashMap, HashSet, Identifier, StatementNode,
        SyntaxTrees, TypeReferenceHandle, TypeReferenceNode,
    };
    use crate::preparation::generic_data::uses::{
        ConstructorFrontier, collect_statement_expression_handles, named_type_name,
        relabel_data_literal_for_expected_type,
    };
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;

    use crate::preparation::generic_data::Instantiation;

    use crate::preparation::declaration_selection;

    /*
        Closed callable signatures provide exact contextual types without inferring
        from literal fields. Free-machine overloads and concrete attached-machine
        overloads each contribute a context only when every same-name candidate on
        the exact owner agrees on one parameter signature. An explicitly typed
        binding receiver names one exact nominal owner. Computed, chained, and
        dynamic receiver selection remains resolver-owned and fails closed here.
    */
    pub(in crate::preparation::generic_data) fn relabel_closed_data_uses_in_exact_calls_and_returns(
        syntax: &mut SyntaxTrees,
        instances: &[Instantiation],
        selection: Option<&declaration_selection::ConstantSelection>,
        declarations: &[syntax_trees::item::ItemHandle],
    ) {
        let mut signatures = HashMap::<String, Option<Vec<TypeReferenceHandle>>>::new();
        let mut attached_signatures =
            HashMap::<(String, String), Option<Vec<TypeReferenceHandle>>>::new();
        for item in syntax.root_items() {
            let Some(machine) = item.as_machine() else {
                continue;
            };
            if !machine.type_parameters.is_empty() {
                continue;
            }
            let Some(entry) = syntax.tables.items.state_handles(machine.states).first() else {
                continue;
            };
            let parameters = syntax_items(syntax)
                .state_parameters(syntax.tables.items.state(*entry).parameters)
                .iter()
                .map(|parameter| {
                    syntax_items(syntax)
                        .state_parameter(*parameter)
                        .type_reference
                })
                .collect::<Vec<_>>();
            let (signature_map, name) = if let Some(attached) = machine.attached_data.as_ref() {
                let method = machine
                    .name
                    .as_str(syntax)
                    .rsplit("::")
                    .next()
                    .unwrap_or(machine.name.as_str(syntax))
                    .to_owned();
                (
                    &mut attached_signatures,
                    (attached.as_str(syntax).to_owned(), method),
                )
            } else {
                let name = machine.name.as_str(syntax).to_owned();
                if let Some(signature) = signatures.get_mut(&name) {
                    let agrees = signature.as_ref().is_some_and(|prior| {
                        call_context_parameter_types_agree(syntax, prior, &parameters)
                    });
                    if !agrees {
                        *signature = None;
                    }
                } else {
                    signatures.insert(name, Some(parameters));
                }
                continue;
            };
            if let Some(signature) = signature_map.get_mut(&name) {
                let agrees = signature.as_ref().is_some_and(|prior| {
                    call_context_parameter_types_agree(syntax, prior, &parameters)
                });
                if !agrees {
                    *signature = None;
                }
            } else {
                signature_map.insert(name, Some(parameters));
            }
        }

        let concrete_states = super::selected_machine_state_handles(syntax, declarations);
        for state_handle in concrete_states {
            let state = syntax.tables.items.state(state_handle).clone();
            let statements = syntax.tables.items.statements(state.statements).to_vec();
            let final_statement = statements.last().copied();
            let mut local_owner_types = HashMap::<String, String>::new();
            for parameter in syntax.tables.items.state_parameters(state.parameters) {
                let parameter = syntax.tables.items.state_parameter(*parameter);
                if let Some(type_name) = named_type_name(syntax, parameter.type_reference) {
                    local_owner_types.insert(parameter.name.as_str(syntax).to_owned(), type_name);
                }
            }
            for statement in &statements {
                if let StatementNode::LocalData(local) =
                    syntax.tables.statements.statement(*statement)
                    && let Some(type_name) = named_type_name(syntax, local.type_reference)
                {
                    local_owner_types.insert(local.name.as_str(syntax).to_owned(), type_name);
                }
            }
            for (index, statement) in statements.iter().enumerate() {
                let frontier = ConstructorFrontier {
                    parameters: state.parameters,
                    prior_statements: &statements[..index],
                };
                let mut reachable = HashSet::new();
                collect_statement_expression_handles(syntax, *statement, &mut reachable);
                let mut reachable = reachable.into_iter().collect::<Vec<_>>();
                reachable
                    .sort_unstable_by_key(|handle| (handle.generation(), handle.arena_index()));
                let calls = reachable
                    .into_iter()
                    .filter_map(|handle| match syntax.expressions.expression(handle) {
                        ExpressionNode::Call(call) if !call.receiver.is_valid() => {
                            Some((call.target.clone(), call.arguments, None))
                        }
                        ExpressionNode::Call(call) => exact_expression_receiver_owner(
                            syntax,
                            call.receiver,
                            &local_owner_types,
                        )
                        .map(|owner| (call.target.clone(), call.arguments, Some(owner))),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                for (target, arguments, owner) in calls {
                    let parameters = if let Some(owner) = owner {
                        attached_signatures.get(&(owner, target.as_str(syntax).to_owned()))
                    } else {
                        signatures.get(target.as_str(syntax))
                    };
                    let Some(Some(parameters)) = parameters else {
                        continue;
                    };
                    let arguments = syntax.expressions.expression_handles(arguments).to_vec();
                    for (argument, expected_type) in arguments.into_iter().zip(parameters) {
                        relabel_data_literal_for_expected_type(
                            syntax,
                            argument,
                            *expected_type,
                            instances,
                            selection,
                            &frontier,
                        );
                    }
                }
                match syntax.tables.statements.statement(*statement) {
                    StatementNode::Expression(value)
                        if state.return_type.is_valid() && Some(*statement) == final_statement =>
                    {
                        relabel_data_literal_for_expected_type(
                            syntax,
                            *value,
                            state.return_type,
                            instances,
                            selection,
                            &frontier,
                        );
                    }
                    StatementNode::Call(call) => {
                        let owner = exact_statement_receiver_owner(
                            syntax,
                            call.receiver,
                            &local_owner_types,
                        );
                        let parameters = if call.receiver.is_empty() {
                            signatures.get(call.target.as_str(syntax))
                        } else {
                            let Some(owner) = owner else {
                                continue;
                            };
                            attached_signatures.get(&(owner, call.target.as_str(syntax).to_owned()))
                        };
                        let Some(Some(parameters)) = parameters else {
                            continue;
                        };
                        let arguments = syntax
                            .tables
                            .statements
                            .expression_handles(call.arguments)
                            .to_vec();
                        for (argument, expected_type) in arguments.into_iter().zip(parameters) {
                            relabel_data_literal_for_expected_type(
                                syntax,
                                argument,
                                *expected_type,
                                instances,
                                selection,
                                &frontier,
                            );
                        }
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            if let syntax_trees::statement::TransitionTargetNode::Value(value) =
                                syntax.tables.statements.transition_target(target)
                            {
                                relabel_data_literal_for_expected_type(
                                    syntax,
                                    *value,
                                    state.return_type,
                                    instances,
                                    selection,
                                    &frontier,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub(in crate::preparation::generic_data) fn exact_statement_receiver_owner(
        syntax: &SyntaxTrees,
        receiver: HandleSpan<Identifier>,
        local_owner_types: &HashMap<String, String>,
    ) -> Option<String> {
        let members = syntax.tables.statements.identifier_path_members(receiver);
        match members {
            [local] => local_owner_types.get(local.as_str(syntax)).cloned(),
            _ => None,
        }
    }

    pub(in crate::preparation::generic_data) fn exact_expression_receiver_owner(
        syntax: &SyntaxTrees,
        receiver: ExpressionHandle,
        local_owner_types: &HashMap<String, String>,
    ) -> Option<String> {
        match syntax.expressions.expression(receiver) {
            ExpressionNode::Name(path) => {
                let [local] = syntax.expressions.identifier_path_members(*path) else {
                    return None;
                };
                local_owner_types.get(local.as_str(syntax)).cloned()
            }
            _ => None,
        }
    }

    /*
        Conservative pre-resolution equality for the expected parameter types that
        drive generic-literal relabeling. Handles are arena-local occurrences, so
        comparing the handles themselves would make two textually identical
        overload signatures look different. Unsupported or expression-bearing
        details fail closed rather than broadening contextual inference.
    */
    pub(in crate::preparation::generic_data) fn call_context_parameter_types_agree(
        syntax: &SyntaxTrees,
        left: &[TypeReferenceHandle],
        right: &[TypeReferenceHandle],
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| call_context_types_agree(syntax, *left, *right))
    }

    pub(in crate::preparation::generic_data) fn call_context_types_agree(
        syntax: &SyntaxTrees,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> bool {
        match (
            syntax.tables.type_references.type_reference(left),
            syntax.tables.type_references.type_reference(right),
        ) {
            (TypeReferenceNode::Named(left), TypeReferenceNode::Named(right)) => {
                left.as_str(syntax) == right.as_str(syntax)
            }
            (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
            (
                TypeReferenceNode::Reference {
                    referee: left,
                    access: left_access,
                    lifetime: left_lifetime,
                },
                TypeReferenceNode::Reference {
                    referee: right,
                    access: right_access,
                    lifetime: right_lifetime,
                },
            ) => {
                left_access == right_access
                    && left_lifetime.as_ref().map(|name| name.as_str(syntax))
                        == right_lifetime.as_ref().map(|name| name.as_str(syntax))
                    && call_context_types_agree(syntax, *left, *right)
            }
            (
                TypeReferenceNode::Slice { element_type: left },
                TypeReferenceNode::Slice {
                    element_type: right,
                },
            ) => call_context_types_agree(syntax, *left, *right),
            (
                TypeReferenceNode::FixedArray {
                    element_type: left,
                    length: left_length,
                },
                TypeReferenceNode::FixedArray {
                    element_type: right,
                    length: right_length,
                },
            ) => {
                left_length.same_value(right_length, syntax)
                    && call_context_types_agree(syntax, *left, *right)
            }
            (
                TypeReferenceNode::Generic {
                    base_name: left_base,
                    lifetime_arguments: left_lifetimes,
                    arguments: left_arguments,
                },
                TypeReferenceNode::Generic {
                    base_name: right_base,
                    lifetime_arguments: right_lifetimes,
                    arguments: right_arguments,
                },
            ) => {
                left_base.as_str(syntax) == right_base.as_str(syntax)
                    && left_lifetimes
                        .iter()
                        .map(|name| name.as_str(syntax))
                        .eq(right_lifetimes.iter().map(|name| name.as_str(syntax)))
                    && call_context_parameter_types_agree(
                        syntax,
                        syntax_type_references(syntax).type_reference_handles(*left_arguments),
                        syntax_type_references(syntax).type_reference_handles(*right_arguments),
                    )
            }
            (
                TypeReferenceNode::DynamicTrait {
                    trait_name: left_name,
                    trait_arguments: left_arguments,
                },
                TypeReferenceNode::DynamicTrait {
                    trait_name: right_name,
                    trait_arguments: right_arguments,
                },
            ) => {
                left_name.as_str(syntax) == right_name.as_str(syntax)
                    && call_context_parameter_types_agree(
                        syntax,
                        syntax_type_references(syntax).type_reference_handles(*left_arguments),
                        syntax_type_references(syntax).type_reference_handles(*right_arguments),
                    )
            }
            (TypeReferenceNode::Constrained { .. }, TypeReferenceNode::Constrained { .. })
            | (TypeReferenceNode::ConstExpression(_), TypeReferenceNode::ConstExpression(_)) => {
                false
            }
            _ => false,
        }
    }
}
mod constructors {
    /* Constructor relabeling against an exact expected type. */
    use super::super::{
        DataMember, ExpressionHandle, ExpressionNode, HandleSpan, Identifier, Item, SyntaxTrees,
        TypeReferenceHandle, TypeReferenceNode,
    };
    use crate::preparation::generic_data::ClosedArgumentIdentity;
    use crate::preparation::generic_data::uses::{ConstructorFrontier, closed_sum_path};
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use syntax_trees::identifier::JoinedSyntaxPath;

    use crate::preparation::generic_data::Instantiation;
    use crate::preparation::generic_data::closed_argument_identity;

    use crate::preparation::declaration_selection;
    use crate::preparation::generic_data::selected_data_item;

    /*
        Copied type references retain their original application. Reuse that exact
        template/argument identity rather than indexing generated display spellings.
    */
    pub(in crate::preparation::generic_data) fn expected_instance<'a>(
        syntax: &SyntaxTrees,
        instances: &'a [Instantiation],
        selection: Option<&declaration_selection::ConstantSelection>,
        expected: TypeReferenceHandle,
    ) -> Option<&'a Instantiation> {
        let expected = match syntax.type_references.type_reference(expected) {
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => expected,
        };
        let ClosedArgumentIdentity::Instance(template, arguments) =
            closed_argument_identity(syntax, selection, expected, false)?
        else {
            return None;
        };
        let mut candidates = instances.iter().filter(|instance| {
            instance.template == template && instance.argument_identity == arguments
        });
        let instance = candidates.next()?;
        candidates.next().is_none().then_some(instance)
    }

    pub(in crate::preparation::generic_data) fn selected_constructor(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
    ) -> Option<(syntax_trees::item::ItemHandle, Option<Identifier>)> {
        if let Some(selection) = selection {
            let (definition, case) = selection.constructor(syntax, name).ok()?;
            let owner = syntax.root_item_handles().iter().copied().find(|handle|
            matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, definition)))?;
            return Some((owner, case));
        }
        if let Some(owner) = selected_data_item(syntax, None, name) {
            return Some((owner, None));
        }
        let (carrier, case) = name.as_str(syntax).rsplit_once("::")?;
        let owner =
            selected_data_item(syntax, None, &Identifier::new(carrier, name.source_span()))?;
        let data = syntax.root_item(owner).as_data()?;
        syntax.items.data_members(data.members).iter().any(|member|
        matches!(member, DataMember::Variant(variant) if variant.name.as_str(syntax) == case))
        .then(|| {
            (owner, Some(Identifier::new(case, name.trailing_span(case.len()))))
        })
    }

    pub(in crate::preparation::generic_data) fn selected_case_value(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
    ) -> Option<(syntax_trees::item::ItemHandle, Option<Identifier>)> {
        if let Some(selection) = selection {
            let (definition, case) = selection.qualified_payload_free_case(syntax, name).ok()??;
            let owner = syntax.root_item_handles().iter().copied().find(|handle|
            matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, definition)))?;
            return Some((owner, Some(case)));
        }
        let (prefix, _) = name.as_str(syntax).split_once("::")?;
        if syntax.root_items().any(|item| matches!(item, Item::Const(constant) if crate::constant::semantic_const_name(syntax, constant) == prefix)) {
        return None;
    }
        selected_constructor(syntax, None, name)
    }

    pub(in crate::preparation::generic_data) fn constructor_path_name(
        syntax: &SyntaxTrees,
        path: HandleSpan<Identifier>,
    ) -> Option<Identifier> {
        let members = syntax.expressions.identifier_path_members(path);
        let first = members.first()?;
        let last = members.last()?;
        let mut span = first.source_span();
        span.span.end = last.source_span().span.end;
        Some(Identifier::new(members.joined_path_in(syntax), span))
    }

    /*
        Keep the authored carrier occurrence separate from the generated lookup
        spelling and final case. Qualified carriers retain their complete prefix.
    */
    pub(in crate::preparation::generic_data) fn constructor_carrier_span(
        syntax: &SyntaxTrees,
        path: HandleSpan<Identifier>,
    ) -> Option<source::SourceSpan> {
        let members = syntax.expressions.identifier_path_members(path);
        let (_, carrier) = members.split_last()?;
        let mut span = carrier.first()?.source_span();
        span.span.end = carrier.last()?.source_span().span.end;
        Some(span)
    }

    /*
        Derive lookup metadata in the constructor's own source context, after its
        authored template has been selected. The roster remains the identity owner.
    */
    pub(in crate::preparation::generic_data) fn closed_constructor_carrier(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        instance: &Instantiation,
        constructor: &Identifier,
        has_case: bool,
    ) -> Option<Identifier> {
        let carrier = if has_case {
            constructor.as_str(syntax).rsplit_once("::")?.0
        } else {
            constructor.as_str(syntax)
        };
        let carrier = Identifier::new(carrier, constructor.source_span());
        let lookup = if let Some(selection) = selection {
            let path = selection.data_lookup_path(syntax, &carrier)?;
            path.rsplit_once("::").map_or_else(
                || instance.synthetic_name.clone(),
                |(prefix, _)| format!("{prefix}::{}", instance.synthetic_name),
            )
        } else {
            instance.synthetic_name.clone()
        };
        Some(Identifier::new(lookup, constructor.source_span()))
    }

    pub(in crate::preparation::generic_data) fn relabel_data_literal_for_expected_type(
        syntax: &mut SyntaxTrees,
        expression: ExpressionHandle,
        expected_type: TypeReferenceHandle,
        instances: &[Instantiation],
        selection: Option<&declaration_selection::ConstantSelection>,
        frontier: &ConstructorFrontier<'_>,
    ) {
        if let ExpressionNode::Match(dispatch) = syntax.expressions.expression(expression) {
            let arms = syntax.expressions.match_arms(dispatch.arms).to_vec();
            for arm in arms {
                relabel_data_literal_for_expected_type(
                    syntax,
                    arm.value,
                    expected_type,
                    instances,
                    selection,
                    frontier,
                );
            }
            return;
        }
        if let ExpressionNode::Name(path) = syntax.expressions.expression(expression)
            && frontier.captures(syntax, *path)
        {
            return;
        }
        let expected_type = match syntax.type_references.type_reference(expected_type) {
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => expected_type,
        };
        if let TypeReferenceNode::FixedArray { element_type, .. } =
            syntax.type_references.type_reference(expected_type)
            && let ExpressionNode::ArrayLiteral(elements) =
                syntax.expressions.expression(expression)
        {
            let element_type = *element_type;
            let elements = syntax.expressions.expression_handles(*elements).to_vec();
            for element in elements {
                relabel_data_literal_for_expected_type(
                    syntax,
                    element,
                    element_type,
                    instances,
                    selection,
                    frontier,
                );
            }
            return;
        }
        let instance = expected_instance(syntax, instances, selection, expected_type);
        let expected_owner = if let Some(instance) = instance {
            instance.declaration
        } else {
            let Some(name) = syntax_type_references(syntax)
                .type_reference(expected_type)
                .as_named()
            else {
                return;
            };
            let Some(owner) = selected_data_item(syntax, selection, name) else {
                return;
            };
            owner
        };
        let Some(definition) = syntax.root_item(expected_owner).as_data() else {
            return;
        };
        let definition = definition.clone();
        let authored_name = match syntax.expressions.expression(expression) {
            ExpressionNode::Name(path) => constructor_path_name(syntax, *path),
            ExpressionNode::StructLiteral(literal) => Some(literal.constructor_name.clone()),
            _ => None,
        };
        let Some(authored_name) = authored_name else {
            return;
        };
        let payload_free = matches!(
            syntax.expressions.expression(expression),
            ExpressionNode::Name(_)
        );
        let alias = instance.and_then(|instance| {
            super::alias_constructors::select(
                syntax,
                selection,
                expected_type,
                instance,
                &authored_name,
                payload_free,
            )
        });
        let selected = alias
            .as_ref()
            .map(|alias| (alias.template, alias.case.clone()))
            .or_else(|| {
                if matches!(
                    syntax.expressions.expression(expression),
                    ExpressionNode::Name(_)
                ) {
                    selected_case_value(syntax, selection, &authored_name)
                } else {
                    selected_constructor(syntax, selection, &authored_name)
                }
            });
        let Some((owner, case)) = selected else {
            return;
        };
        if owner != instance.map_or(expected_owner, |instance| instance.template) {
            return;
        }
        let closed = if let Some(alias) = &alias {
            alias.carrier.clone()
        } else if let Some(instance) = instance {
            let Some(closed) = closed_constructor_carrier(
                syntax,
                selection,
                instance,
                &authored_name,
                case.is_some(),
            ) else {
                return;
            };
            closed
        } else {
            Identifier::new(definition.name.as_str(syntax), authored_name.source_span())
        };
        if let Some(alias) = &alias
            && matches!(
                syntax.expressions.expression(expression),
                ExpressionNode::Name(_)
            )
            && let Some(case) = &case
        {
            let name = format!("{}::{}", closed.as_str(syntax), case.as_str(syntax));
            syntax.expressions.replace_expression(
                expression,
                ExpressionNode::StructLiteral(syntax_trees::expression::TableStructLiteral {
                    application_type: alias.application,
                    constructor_name: Identifier::new(name, authored_name.source_span()),
                    fields: HandleSpan::empty(),
                }),
            );
            return;
        }
        if let ExpressionNode::Name(path) = syntax.expressions.expression(expression) {
            if instance.is_some()
                && let Some(case) = case
                && let Some(carrier_span) = constructor_carrier_span(syntax, *path)
            {
                let path = closed_sum_path(syntax, closed, carrier_span, case);
                syntax
                    .expressions
                    .replace_expression(expression, ExpressionNode::Name(path));
            }
            return;
        }
        let ExpressionNode::StructLiteral(mut literal) =
            syntax.expressions.expression(expression).clone()
        else {
            return;
        };
        if let Some(alias) = &alias {
            literal.application_type = alias.application;
        }
        let case_name = case.as_ref().map(|case| case.as_str(syntax).to_owned());
        if instance.is_some() {
            let name = case_name.as_ref().map_or_else(
                || closed.as_str(syntax).to_owned(),
                |case| format!("{closed}::{case}", closed = closed.as_str(syntax)),
            );
            literal.constructor_name = Identifier::new(name, authored_name.source_span());
            syntax
                .expressions
                .replace_expression(expression, ExpressionNode::StructLiteral(literal.clone()));
        }

        let mut declared_fields = syntax_items(syntax)
            .data_members(definition.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => {
                    Some((field.name.as_str(syntax).to_owned(), field.type_reference))
                }
                DataMember::Variant(_) | DataMember::Retired(_) => None,
            })
            .collect::<Vec<_>>();
        if let Some(case_name) = case_name.as_ref()
            && let Some(variant) = syntax_items(syntax)
                .data_members(definition.members)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant)
                        if variant.name.as_str(syntax) == case_name.as_str() =>
                    {
                        Some(variant)
                    }
                    _ => None,
                })
        {
            declared_fields.extend(
                syntax_items(syntax)
                    .data_payload_fields(variant.payload)
                    .iter()
                    .map(|field| (field.name.as_str(syntax).to_owned(), field.type_reference)),
            );
        }
        let authored = syntax.expressions.struct_fields(literal.fields).to_vec();
        for field in authored {
            let Some((_, field_type)) = declared_fields
                .iter()
                .find(|(name, _)| name == field.name.as_str(syntax))
            else {
                continue;
            };
            relabel_data_literal_for_expected_type(
                syntax,
                field.value,
                *field_type,
                instances,
                selection,
                frontier,
            );
        }
    }
}
mod expressions {
    /* Executable expression collection outside generic templates. */

    use super::super::{
        ExpressionHandle, ExpressionNode, HandleSpan, HashSet, Identifier, Item, StatementNode,
        SyntaxTrees,
    };
    use crate::tables::syntax_type_references;
    /*
        Marks expressions reached by a collection walk; `mark` reports whether
        the expression was new, so a shared subtree is walked once.
    */
    pub(in crate::preparation) trait ExpressionMarks {
        fn mark(&mut self, expression: ExpressionHandle) -> bool;
    }

    impl ExpressionMarks for HashSet<ExpressionHandle> {
        fn mark(&mut self, expression: ExpressionHandle) -> bool {
            self.insert(expression)
        }
    }

    /* Construction-order inventory; no lexical selection is performed here. */
    #[derive(Default)]
    struct ExpressionInventory {
        seen: ConcreteExpressions,
        handles: Vec<ExpressionHandle>,
    }

    impl ExpressionMarks for ExpressionInventory {
        fn mark(&mut self, expression: ExpressionHandle) -> bool {
            if !self.seen.mark(expression) {
                return false;
            }
            self.handles.push(expression);
            true
        }
    }

    pub(crate) fn statement_type_inputs(
        syntax: &SyntaxTrees,
        statement: syntax_trees::statement::StatementHandle,
    ) -> Vec<syntax_trees::types::TypeReferenceHandle> {
        let mut inputs = Vec::new();
        match syntax.statements.statement(statement) {
            StatementNode::LocalData(local) => inputs.push(local.type_reference),
            StatementNode::Call(call) => {
                visit_static_argument_type_inputs(
                    syntax,
                    &call.machine_arguments,
                    &mut |reference| inputs.push(reference),
                );
            }
            _ => {}
        }
        let mut expressions = ExpressionInventory::default();
        collect_statement_expression_handles(syntax, statement, &mut expressions);
        for expression in expressions.handles {
            visit_expression_type_inputs(syntax, expression, &mut |reference| {
                inputs.push(reference)
            });
        }
        inputs.retain(|reference| reference.is_valid());
        inputs
    }

    pub(in crate::preparation) fn visit_expression_type_inputs(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        visit: &mut impl FnMut(syntax_trees::types::TypeReferenceHandle),
    ) {
        match syntax.expressions.expression(expression) {
            ExpressionNode::StructLiteral(literal) if literal.application_type.is_valid() => {
                visit(literal.application_type)
            }
            ExpressionNode::Match(dispatch) => {
                for arm in syntax.expressions.match_arms(dispatch.arms) {
                    if let syntax_trees::expression::MatchPattern::Destructure(pattern) =
                        arm.pattern
                        && pattern.application_type.is_valid()
                    {
                        visit(pattern.application_type);
                    }
                }
            }
            ExpressionNode::Cast(cast) => {
                visit(cast.target_type);
                for &argument in syntax_type_references(syntax)
                    .type_reference_handles(cast.semantic_domain_arguments)
                {
                    visit(argument);
                }
            }
            ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
                visit(*reference)
            }
            ExpressionNode::Call(call) => {
                visit_static_argument_type_inputs(syntax, &call.machine_arguments, visit)
            }
            _ => {}
        }
    }

    pub(in crate::preparation::generic_data) fn visit_static_argument_type_inputs(
        syntax: &SyntaxTrees,
        arguments: &[syntax_trees::expression::StaticMachineArgument],
        visit: &mut impl FnMut(syntax_trees::types::TypeReferenceHandle),
    ) {
        for argument in arguments {
            if argument.type_reference.is_valid() {
                visit(argument.type_reference);
            }
            if let Some(application) = &argument.application {
                visit_static_argument_type_inputs(syntax, &application.arguments, visit);
                if let Some(member) = syntax.expressions.static_member(application.member) {
                    visit_static_argument_type_inputs(syntax, std::slice::from_ref(member), visit);
                }
            }
        }
    }

    #[derive(Default)]
    pub(in crate::preparation::generic_data) struct ConcreteExpressions(
        std::collections::HashSet<ExpressionHandle>,
    );

    impl ConcreteExpressions {
        pub(in crate::preparation::generic_data) fn contains(
            &self,
            expression: ExpressionHandle,
        ) -> bool {
            self.0.contains(&expression)
        }
    }

    impl ExpressionMarks for ConcreteExpressions {
        fn mark(&mut self, expression: ExpressionHandle) -> bool {
            self.0.insert(expression)
        }
    }

    pub(in crate::preparation::generic_data) fn concrete_machine_expression_handles(
        syntax: &SyntaxTrees,
    ) -> ConcreteExpressions {
        selected_machine_expression_handles(syntax, syntax.root_item_handles())
    }

    pub(in crate::preparation::generic_data) fn selected_machine_expression_handles(
        syntax: &SyntaxTrees,
        declarations: &[syntax_trees::item::ItemHandle],
    ) -> ConcreteExpressions {
        let mut handles = ConcreteExpressions::default();
        for state in selected_machine_state_handles(syntax, declarations) {
            let state = syntax.items.state(state);
            for statement in syntax.items.statements(state.statements) {
                collect_statement_expression_handles(syntax, *statement, &mut handles);
            }
        }
        handles
    }

    pub(in crate::preparation::generic_data) fn selected_machine_state_handles(
        syntax: &SyntaxTrees,
        declarations: &[syntax_trees::item::ItemHandle],
    ) -> Vec<syntax_trees::item::StateHandle> {
        concrete_machine_declarations(syntax, declarations)
            .into_iter()
            .flat_map(|machine| syntax.items.state_handles(machine.states).iter().copied())
            .collect()
    }

    /*
        Inline conformance bodies own ordinary machines, including every state.
        Their closed type uses must join the same normalization inventory as free
        machines; lowering the conformance later cannot repair stale constructor
        identities. Open enclosing binders remain templates, not concrete work.
    */
    pub(in crate::preparation::generic_data) fn concrete_machine_declarations<'syntax>(
        syntax: &'syntax SyntaxTrees,
        declarations: &[syntax_trees::item::ItemHandle],
    ) -> Vec<&'syntax syntax_trees::item::Machine> {
        let generic_data = syntax
            .root_items()
            .filter_map(|item| match item {
                Item::Data(data) if !data.type_parameters.is_empty() => {
                    Some(data.name.as_str(syntax))
                }
                _ => None,
            })
            .collect::<HashSet<&str>>();
        let is_concrete = |machine: &syntax_trees::item::Machine| {
            machine.type_parameters.is_empty()
                && !machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|attached| generic_data.contains(attached.as_str(syntax)))
        };
        let mut machines = Vec::new();
        let mut pending = syntax
            .root_item_handles()
            .iter()
            .rev()
            .map(|handle| (*handle, false))
            .collect::<Vec<_>>();
        while let Some((declaration, enclosing_work)) = pending.pop() {
            let owned = enclosing_work || declarations.contains(&declaration);
            let mut enter_machine = |machine: &'syntax syntax_trees::item::Machine| {
                if !is_concrete(machine) {
                    return;
                }
                if owned {
                    machines.push(machine);
                }
                pending.extend(
                    (0..machine.nested_machines.count())
                        .rev()
                        .map(|offset| (machine.nested_machines.handle_at(offset), owned)),
                );
            };
            match syntax.root_item(declaration) {
                Item::Machine(machine) => enter_machine(machine),
                Item::Conformance(conformance) if conformance.type_parameters.is_empty() => {
                    for member in syntax.items.conformance_members(conformance.members) {
                        use syntax_trees::item::ConformanceMember;
                        let machine = match member {
                            ConformanceMember::Machine(machine)
                            | ConformanceMember::Synthesized(machine)
                            | ConformanceMember::TraitDefault { machine, .. } => machine,
                            ConformanceMember::Selection { .. } => continue,
                        };
                        enter_machine(machine);
                    }
                }
                _ => {}
            }
        }
        machines
    }

    pub(in crate::preparation) fn collect_statement_expression_handles(
        syntax: &SyntaxTrees,
        statement: syntax_trees::statement::StatementHandle,
        handles: &mut impl ExpressionMarks,
    ) {
        use syntax_trees::statement::{TransitionGuardNode, TransitionTargetNode};
        match syntax.tables.statements.statement(statement) {
            StatementNode::Assembly(handle) => {
                let graph = syntax.tables.statements.assembly(*handle);
                for block in syntax.tables.statements.assembly_blocks(graph.blocks) {
                    for expression in syntax
                        .tables
                        .statements
                        .assembly_expressions(block.requires)
                        .iter()
                        .chain(syntax.tables.statements.assembly_expressions(block.ensures))
                    {
                        collect_expression_handles(syntax, *expression, handles);
                    }
                    for instruction in syntax
                        .tables
                        .statements
                        .assembly_instructions(block.instructions)
                    {
                        collect_statement_expression_handles(syntax, *instruction, handles);
                    }
                }
            }
            StatementNode::RootBinding(binding) => {
                collect_expression_handles(syntax, binding.receiver, handles);
                if binding.implementation_operand.is_valid() {
                    collect_expression_handles(syntax, binding.implementation_operand, handles);
                }
            }
            StatementNode::AssemblyFact(fact) => {
                collect_expression_handles(syntax, fact.expression, handles)
            }
            StatementNode::Assignment(assignment) => {
                collect_expression_handles(syntax, assignment.target, handles);
                collect_expression_handles(syntax, assignment.value, handles);
            }
            StatementNode::Call(call) => {
                for argument in syntax.tables.statements.expression_handles(call.arguments) {
                    collect_expression_handles(syntax, *argument, handles);
                }
            }
            StatementNode::ProofOutputBindingStatement(binding) => {
                collect_expression_handles(syntax, binding.call, handles)
            }
            StatementNode::Expression(expression) => {
                collect_expression_handles(syntax, *expression, handles)
            }
            StatementNode::LocalData(local) => {
                collect_expression_handles(syntax, local.initial_value, handles)
            }
            StatementNode::Transition(transition) => {
                for retained in syntax.statements.transition_patterns(transition.patterns) {
                    collect_expression_handles(syntax, retained.subject, handles);
                    collect_expression_handles(syntax, retained.pattern.head, handles);
                    collect_expression_handles(syntax, retained.pattern.guard, handles);
                }

                if let TransitionGuardNode::When(guard) = transition.guard {
                    collect_expression_handles(syntax, guard, handles);
                }
                for target in transition.targets() {
                    match syntax.tables.statements.transition_target(target) {
                        TransitionTargetNode::Named { arguments, .. } => {
                            for argument in syntax.tables.statements.expression_handles(*arguments)
                            {
                                collect_expression_handles(syntax, *argument, handles);
                            }
                        }
                        TransitionTargetNode::Value(value) => {
                            collect_expression_handles(syntax, *value, handles)
                        }
                        TransitionTargetNode::Terminal => {}
                    }
                }
            }
        }
    }

    pub(in crate::preparation) fn collect_expression_handles(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        handles: &mut impl ExpressionMarks,
    ) {
        if !expression.is_valid() || !handles.mark(expression) {
            return;
        }
        match syntax.expressions.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                collect_expression_handles(syntax, dispatch.subject, handles);
                for arm in syntax.expressions.match_arms(dispatch.arms) {
                    for pattern in syntax.expressions.match_pattern_dependencies(arm.pattern) {
                        collect_expression_handles(syntax, pattern, handles);
                    }
                    collect_expression_handles(syntax, arm.value, handles);
                }
            }
            ExpressionNode::SavedSubject(saved) => {
                collect_expression_handles(
                    syntax,
                    syntax.expressions.match_subject(saved.capture).subject,
                    handles,
                );
                collect_expression_handles(syntax, saved.pattern_head, handles);
            }
            ExpressionNode::ArrayLiteral(expressions) => {
                for expression in syntax.expressions.expression_handles(*expressions) {
                    collect_expression_handles(syntax, *expression, handles);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                collect_expression_handles(syntax, atomic.value, handles);
                collect_expression_handles(syntax, atomic.result, handles);
            }
            ExpressionNode::Binary(binary) => {
                collect_expression_handles(syntax, binary.left, handles);
                collect_expression_handles(syntax, binary.right, handles);
            }
            ExpressionNode::Cast(cast) => collect_expression_handles(syntax, cast.value, handles),
            ExpressionNode::Call(call) => {
                collect_expression_handles(syntax, call.receiver, handles);
                for argument in syntax.expressions.expression_handles(call.arguments) {
                    collect_expression_handles(syntax, *argument, handles);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                collect_expression_handles(syntax, indexed.collection, handles);
                collect_expression_handles(syntax, indexed.index, handles);
            }
            ExpressionNode::Membership(membership) => {
                collect_expression_handles(syntax, membership.value, handles)
            }
            ExpressionNode::Member(member) => {
                collect_expression_handles(syntax, member.receiver, handles)
            }
            ExpressionNode::Borrow(inner) => {
                collect_expression_handles(syntax, inner.target, handles)
            }
            ExpressionNode::Range(range) => {
                collect_expression_handles(syntax, range.start, handles);
                collect_expression_handles(syntax, range.end, handles);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in syntax.expressions.struct_fields(literal.fields) {
                    collect_expression_handles(syntax, field.value, handles);
                }
            }
            ExpressionNode::Unary(unary) => {
                collect_expression_handles(syntax, unary.operand, handles)
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_)
            | ExpressionNode::TypeExpression(_) => {}
        }
    }

    /*
        Advisory constructor rewriting borrows the actual value frontier. A captured
        Name remains authored syntax for the ordinary resolver; no binding is chosen
        here. Callers visit concrete machines, whose template binders are already gone.
    */
    pub(in crate::preparation::generic_data) struct ConstructorFrontier<'a> {
        pub parameters: HandleSpan<syntax_trees::item::StateParameterHandle>,
        pub prior_statements: &'a [syntax_trees::statement::StatementHandle],
    }

    impl ConstructorFrontier<'_> {
        pub fn captures(&self, syntax: &SyntaxTrees, path: HandleSpan<Identifier>) -> bool {
            let Some(head) = syntax.expressions.identifier_path_members(path).first() else {
                return false;
            };
            self.captures_name(syntax, head)
        }

        pub fn captures_name(&self, syntax: &SyntaxTrees, name: &Identifier) -> bool {
            self.prior_statements.iter().rev().any(|statement| matches!(
            syntax.statements.statement(*statement), StatementNode::LocalData(local) if local.name.as_str(syntax) == name.as_str(syntax)
        )) || syntax.items.state_parameters(self.parameters).iter().any(|parameter|
            syntax.items.state_parameter(*parameter).name.as_str(syntax) == name.as_str(syntax))
        }
    }

    /*
        Call-result input inventory. A selected call retains its whole receiver
        and argument subtree through ordinary lowering, so nested calls are not
        independently materialized a second time. A lexical match capture is also
        indivisible: extracting an arm call would detach its saved-subject reads
        from the definition required to resolve them. Retain the enclosing match
        when it contains calls, without installing it as an executable body.
    */
    pub(crate) fn statement_call_inputs(
        syntax: &SyntaxTrees,
        statement: syntax_trees::statement::StatementHandle,
    ) -> Vec<ExpressionHandle> {
        struct Calls<'a> {
            syntax: &'a SyntaxTrees,
            handles: Vec<ExpressionHandle>,
        }
        impl ExpressionMarks for Calls<'_> {
            fn mark(&mut self, expression: ExpressionHandle) -> bool {
                let retain = match self.syntax.expressions.expression(expression) {
                    ExpressionNode::Call(_) => true,
                    ExpressionNode::Match(dispatch) if dispatch.capture.is_valid() => {
                        struct CallSearch<'a> {
                            syntax: &'a SyntaxTrees,
                            found: bool,
                        }
                        impl ExpressionMarks for CallSearch<'_> {
                            fn mark(&mut self, expression: ExpressionHandle) -> bool {
                                self.found |= matches!(
                                    self.syntax.expressions.expression(expression),
                                    ExpressionNode::Call(_)
                                );
                                !self.found
                            }
                        }
                        let mut search = CallSearch {
                            syntax: self.syntax,
                            found: false,
                        };
                        collect_expression_handles(self.syntax, expression, &mut search);
                        search.found
                    }
                    _ => false,
                };
                if retain {
                    self.handles.push(expression);
                    false
                } else {
                    true
                }
            }
        }
        let mut calls = Calls {
            syntax,
            handles: Vec::new(),
        };
        collect_statement_expression_handles(syntax, statement, &mut calls);
        calls.handles
    }
}
mod patterns {
    /* Membership and sum-pattern relabeling. */
    use super::super::{
        ExpressionNode, HandleSpan, HashSet, Identifier, SyntaxTrees, TypeReferenceHandle,
        TypeReferenceNode,
    };
    use crate::preparation::generic_data::uses::{
        closed_constructor_carrier, collect_statement_expression_handles,
        concrete_state_local_types, constructor_carrier_span, constructor_path_name,
        expected_instance, selected_constructor,
    };

    use crate::preparation::generic_data::Instantiation;

    use crate::preparation::declaration_selection;
    use syntax_trees::expression::TableMembershipExpression;
    use syntax_trees::item::ItemHandle;

    /*
        Destructure syntax lowers to `subject in Base::Case` before this pass. When
        the subject is a state parameter or local with an exact synthesized type,
        that annotation selects the corresponding closed case identity even when
        another closed instance of the same generic sum exists in the program.
    */
    pub(in crate::preparation::generic_data) fn relabel_closed_sum_memberships_from_local_types(
        syntax: &mut SyntaxTrees,
        instances: &[Instantiation],
        selection: Option<&declaration_selection::ConstantSelection>,
        declarations: &[ItemHandle],
    ) {
        for (state, local_types, statements) in concrete_state_local_types(syntax, declarations) {
            for (position, statement) in statements.iter().enumerate() {
                let frontier = super::ConstructorFrontier {
                    parameters: state.parameters,
                    prior_statements: &statements[..position],
                };
                let transition = match syntax.statements.statement(*statement) {
                    syntax_trees::statement::StatementNode::Transition(transition) => {
                        Some(*transition)
                    }
                    _ => None,
                };
                if let Some(transition) = transition {
                    let patterns = syntax
                        .statements
                        .transition_patterns(transition.patterns)
                        .to_vec();
                    for (ordinal, mut retained) in patterns.into_iter().enumerate() {
                        if !retained.is_structural
                            && let ExpressionNode::Name(path) =
                                syntax.expressions.expression(retained.pattern.head)
                            && frontier.captures(syntax, *path)
                        {
                            continue;
                        }
                        if !retained.is_structural
                            && let ExpressionNode::Name(path) =
                                syntax.expressions.expression(retained.pattern.head)
                            && let Some(name) = constructor_path_name(syntax, *path)
                            && selection.is_some_and(|selection| {
                                selection.constructor_prefix_is_value(syntax, &name)
                            })
                        {
                            continue;
                        }
                        if let Some(expected) = subject_annotation(
                            syntax,
                            retained.subject,
                            &local_types,
                            &statements,
                            &mut Vec::new(),
                        ) {
                            let application = relabel_pattern_head(
                                syntax,
                                instances,
                                selection,
                                expected,
                                retained.pattern.head,
                            );
                            if application.is_valid() {
                                retained.pattern.application_type = application;
                            }
                        }
                        *syntax.statements.transition_pattern_mut(
                            transition.patterns.handle_at(ordinal as u32),
                        ) = retained;
                    }
                }
            }
            let mut reachable = HashSet::new();
            for statement in &statements {
                collect_statement_expression_handles(syntax, *statement, &mut reachable);
            }
            let mut reachable = reachable.into_iter().collect::<Vec<_>>();
            reachable.sort_unstable_by_key(|handle| (handle.generation(), handle.arena_index()));
            for expression in reachable.iter().copied() {
                if let ExpressionNode::Match(dispatch) =
                    syntax.expressions.expression(expression).clone()
                    && let Some(expected) = subject_annotation(
                        syntax,
                        dispatch.subject,
                        &local_types,
                        &statements,
                        &mut Vec::new(),
                    )
                {
                    let mut arms = syntax.expressions.match_arms(dispatch.arms).to_vec();
                    for arm in &mut arms {
                        if let syntax_trees::expression::MatchPattern::Destructure(pattern) =
                            &mut arm.pattern
                        {
                            let application = relabel_pattern_head(
                                syntax,
                                instances,
                                selection,
                                expected,
                                pattern.head,
                            );
                            if application.is_valid() {
                                pattern.application_type = application;
                            }
                        }
                    }
                    let arms = syntax.expressions.insert_match_arms(arms);
                    syntax.expressions.replace_expression(
                        expression,
                        ExpressionNode::Match(syntax_trees::expression::TableMatchExpression {
                            arms,
                            ..dispatch
                        }),
                    );
                }
            }
            let replacements = reachable
                .into_iter()
                .filter_map(|handle| {
                    let expression = syntax.expressions.expression(handle);
                    let ExpressionNode::Membership(membership) = expression else {
                        return None;
                    };
                    let closed = match syntax.expressions.expression(membership.value) {
                        ExpressionNode::Name(subject_path) => {
                            let [subject] =
                                syntax.expressions.identifier_path_members(*subject_path)
                            else {
                                return None;
                            };
                            local_types.get(subject.as_str(syntax))?
                        }
                        _ => return None,
                    };
                    let closed = membership_subject_type(syntax, *closed);
                    if !closed.is_valid() {
                        return None;
                    }
                    let instance = expected_instance(syntax, instances, selection, closed)?;
                    let name = constructor_path_name(syntax, membership.domain)?;
                    let (owner, case) = selected_constructor(syntax, selection, &name)?;
                    if owner != instance.template {
                        return None;
                    }
                    let case = case?;
                    let closed =
                        closed_constructor_carrier(syntax, selection, instance, &name, true)?;
                    Some((
                        handle,
                        membership.value,
                        closed,
                        constructor_carrier_span(syntax, membership.domain)?,
                        case,
                    ))
                })
                .collect::<Vec<_>>();
            for (handle, value, closed, carrier_span, case) in replacements {
                let domain = closed_sum_path(syntax, closed, carrier_span, case);
                syntax.expressions.replace_expression(
                    handle,
                    ExpressionNode::Membership(TableMembershipExpression { value, domain }),
                );
            }
        }
    }

    fn membership_subject_type(
        syntax: &SyntaxTrees,
        mut reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        for _ in 0..syntax.type_references.type_reference_count() {
            if !reference.is_valid() {
                return reference;
            }
            reference = match syntax.type_references.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => *referee,
                TypeReferenceNode::Constrained { base_type, .. } => *base_type,
                _ => return reference,
            };
        }
        TypeReferenceHandle::default()
    }

    pub(in crate::preparation::generic_data) fn named_type_name(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
    ) -> Option<String> {
        let type_reference = match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            TypeReferenceNode::Named(_) => type_reference,
            _ => return None,
        };
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(name) => Some(name.as_str(syntax).to_owned()),
            _ => None,
        }
    }

    pub(in crate::preparation::generic_data) fn closed_sum_path(
        syntax: &mut SyntaxTrees,
        closed: Identifier,
        carrier_span: source::SourceSpan,
        case: Identifier,
    ) -> HandleSpan<Identifier> {
        let mut path = HandleSpan::empty();
        let closed = Identifier::new(closed.as_str(syntax), carrier_span);
        syntax
            .expressions
            .append_identifier_path_member_to_span(&mut path, closed);
        syntax
            .expressions
            .append_identifier_path_member_to_span(&mut path, case);
        path
    }

    /*
        Read only declared source carriers and completed dispatch-local aliases.
        This supplies type inference, never a value, case fact or access grant.
    */
    fn subject_annotation(
        syntax: &SyntaxTrees,
        expression: syntax_trees::expression::ExpressionHandle,
        types: &super::super::HashMap<String, TypeReferenceHandle>,
        statements: &[syntax_trees::statement::StatementHandle],
        active: &mut Vec<syntax_trees::expression::ExpressionHandle>,
    ) -> Option<TypeReferenceHandle> {
        if active.contains(&expression) {
            return None;
        }
        active.push(expression);
        let result = match syntax.expressions.expression(expression) {
            ExpressionNode::Name(path) => {
                let [name] = syntax.expressions.identifier_path_members(*path) else {
                    return None;
                };
                if let Some(reference) = types
                    .get(name.as_str(syntax))
                    .filter(|reference| reference.is_valid())
                {
                    Some(membership_subject_type(syntax, *reference))
                } else {
                    statements.iter().find_map(|statement| {
                        let syntax_trees::statement::StatementNode::LocalData(local) =
                            syntax.statements.statement(*statement)
                        else {
                            return None;
                        };
                        (local.role == language_core::LocalBindingRole::DispatchSubject
                            && local.name.as_str(syntax) == name.as_str(syntax))
                        .then(|| {
                            subject_annotation(
                                syntax,
                                local.initial_value,
                                types,
                                statements,
                                active,
                            )
                        })
                        .flatten()
                    })
                }
            }
            ExpressionNode::SavedSubject(saved) => subject_annotation(
                syntax,
                syntax.expressions.match_subject(saved.capture).subject,
                types,
                statements,
                active,
            ),
            ExpressionNode::Borrow(borrow) => {
                subject_annotation(syntax, borrow.target, types, statements, active)
            }
            ExpressionNode::StructLiteral(literal) if literal.application_type.is_valid() => {
                Some(membership_subject_type(syntax, literal.application_type))
            }
            _ => None,
        };
        active.pop();
        result
    }

    fn relabel_pattern_head(
        syntax: &mut SyntaxTrees,
        instances: &[Instantiation],
        selection: Option<&declaration_selection::ConstantSelection>,
        expected: TypeReferenceHandle,
        head: syntax_trees::expression::ExpressionHandle,
    ) -> TypeReferenceHandle {
        let ExpressionNode::Name(path) = syntax.expressions.expression(head) else {
            return Default::default();
        };
        let original_path = *path;
        let Some(name) = constructor_path_name(syntax, original_path) else {
            return Default::default();
        };
        let Some(instance) = expected_instance(syntax, instances, selection, expected) else {
            return Default::default();
        };
        let alias =
            super::alias_constructors::select(syntax, selection, expected, instance, &name, false);
        let (carrier, case, application) = if let Some(alias) = alias {
            (alias.carrier, alias.case, alias.application)
        } else {
            let Some((owner, case)) = selected_constructor(syntax, selection, &name) else {
                return Default::default();
            };
            if owner != instance.template {
                return Default::default();
            }
            let Some(carrier) =
                closed_constructor_carrier(syntax, selection, instance, &name, case.is_some())
            else {
                return Default::default();
            };
            (carrier, case, Default::default())
        };
        let selected = if let Some(case) = case {
            closed_sum_path(
                syntax,
                carrier,
                constructor_carrier_span(syntax, original_path).unwrap_or(name.source_span()),
                case,
            )
        } else {
            let start = syntax.expressions.append_identifier_path_member(carrier);
            HandleSpan::from_parts(start, 1)
        };
        syntax
            .expressions
            .replace_expression(head, ExpressionNode::Name(selected));
        application
    }
}

pub(in crate::preparation) use expressions::visit_expression_type_inputs;
pub(super) use expressions::visit_static_argument_type_inputs;
pub(crate) use expressions::{statement_call_inputs, statement_type_inputs};

pub(super) use assignments::{
    relabel_closed_data_uses_in_constants, relabel_closed_data_uses_in_exact_assignments,
};
pub(super) use calls::relabel_closed_data_uses_in_exact_calls_and_returns;
pub(super) use constructors::{
    closed_constructor_carrier, constructor_carrier_span, constructor_path_name, expected_instance,
    relabel_data_literal_for_expected_type, selected_constructor,
};
pub(super) use expressions::{
    ConstructorFrontier, concrete_machine_declarations, concrete_machine_expression_handles,
    selected_machine_expression_handles, selected_machine_state_handles,
};
pub(in crate::preparation) use expressions::{
    collect_expression_handles, collect_statement_expression_handles,
};
pub(super) use patterns::{
    closed_sum_path, named_type_name, relabel_closed_sum_memberships_from_local_types,
};

/*
    Concrete machine states and their declared parameter/local types — the
    shared prelude every local-typed relabel pass walks before its own scan.
*/
pub(super) fn concrete_state_local_types(
    syntax: &super::SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Vec<(
    syntax_trees::item::State,
    super::HashMap<String, super::TypeReferenceHandle>,
    Vec<syntax_trees::statement::StatementHandle>,
)> {
    use super::StatementNode;
    let concrete_states = selected_machine_state_handles(syntax, declarations);

    let mut states = Vec::with_capacity(concrete_states.len());
    for state_handle in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let mut local_types = super::HashMap::<String, super::TypeReferenceHandle>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            local_types.insert(
                parameter.name.as_str(syntax).to_owned(),
                parameter.type_reference,
            );
        }
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
            {
                local_types.insert(local.name.as_str(syntax).to_owned(), local.type_reference);
            }
        }
        states.push((state, local_types, statements));
    }
    states
}
