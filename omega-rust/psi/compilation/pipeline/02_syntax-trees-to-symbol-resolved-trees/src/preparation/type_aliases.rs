//! Declaration-owned transparent type expansion before nominal synthesis.
//!
//! Source application rows retain the alias telescope independently from the
//! ordinary type replacing it. This stage forms no checked admission receipt.

use super::declaration_selection::{ConstantSelection, GenericParameterRole};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use source::SourceSpan;
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, TypeAliasDefinition, TypeParameterKind};
use syntax_trees::types::{TypeAliasApplicationOrigin, TypeReferenceHandle, TypeReferenceNode};

struct Expansion<'a, 'base> {
    selection: &'a ConstantSelection<'base>,
    declarations: Vec<&'a SyntaxTrees>,
    active: Vec<SourceSpan>,
    supplied: Vec<TypeReferenceHandle>,
}

impl Expansion<'_, '_> {
    fn reference(
        &mut self,
        syntax: &mut SyntaxTrees,
        reference: TypeReferenceHandle,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        if !reference.is_valid() || self.supplied.contains(&reference) {
            return Ok(());
        }
        let node = syntax.type_references.type_reference(reference).clone();
        let (head, lifetimes, arguments) = match &node {
            TypeReferenceNode::Named(name) => (Some(name.clone()), Vec::new(), Vec::new()),
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => (
                Some(base_name.clone()),
                lifetime_arguments.clone(),
                syntax
                    .type_references
                    .type_reference_handles(*arguments)
                    .to_vec(),
            ),
            _ => (None, Vec::new(), Vec::new()),
        };
        if let Some(head) =
            head.filter(|head| !bound.iter().any(|name| name == head.as_str(syntax)))
        {
            let declaration = self.selection.type_alias_source(syntax, &head);
            let parameters = self.selection.generic_parameter_roles(
                syntax,
                &self.declarations,
                &head,
                &[
                    SymbolKind::BuiltinType,
                    SymbolKind::Data,
                    SymbolKind::TypeAlias,
                    SymbolKind::Trait,
                    SymbolKind::Machine,
                ],
            );
            // Supplied type operands normalize in the caller's scope before
            // entering an alias RHS. Static operands keep their declared role.
            for (position, argument) in arguments.iter().enumerate() {
                if parameters
                    .as_ref()
                    .and_then(|parameters| parameters.get(position))
                    .is_some_and(|parameter| *parameter == GenericParameterRole::Type)
                {
                    self.reference(syntax, *argument, bound)?;
                }
            }
            if let Some(declaration) = declaration {
                return self.application(
                    syntax,
                    reference,
                    &head,
                    declaration,
                    &lifetimes,
                    &arguments,
                    bound,
                );
            }
        }
        match node {
            TypeReferenceNode::Reference { referee, .. } => {
                self.reference(syntax, referee, bound)?
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                self.reference(syntax, base_type, bound)?;
                let constraints = syntax.type_references.constraints(constraints).to_vec();
                for constraint in constraints {
                    if let syntax_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                        let arguments = syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                            .to_vec();
                        self.application_arguments(
                            syntax,
                            &domain.name,
                            &arguments,
                            &[SymbolKind::Domain],
                            bound,
                        )?;
                    }
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                self.reference(syntax, element_type, bound)?
            }
            TypeReferenceNode::DynamicTrait {
                trait_name,
                trait_arguments,
            } => {
                let arguments = syntax
                    .type_references
                    .type_reference_handles(trait_arguments)
                    .to_vec();
                let parameters = self.selection.generic_parameter_roles(
                    syntax,
                    &self.declarations,
                    &trait_name,
                    &[SymbolKind::Trait],
                );
                for (position, argument) in arguments.iter().enumerate() {
                    if parameters
                        .as_ref()
                        .and_then(|parameters| parameters.get(position))
                        .is_some_and(|parameter| *parameter == GenericParameterRole::Type)
                    {
                        self.reference(syntax, *argument, bound)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn application_arguments(
        &mut self,
        syntax: &mut SyntaxTrees,
        name: &Identifier,
        arguments: &[TypeReferenceHandle],
        kinds: &[SymbolKind],
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        let parameters =
            self.selection
                .generic_parameter_roles(syntax, &self.declarations, name, kinds);
        for (position, argument) in arguments.iter().enumerate() {
            if parameters
                .as_ref()
                .and_then(|parameters| parameters.get(position))
                .is_some_and(|parameter| *parameter == GenericParameterRole::Type)
            {
                self.reference(syntax, *argument, bound)?;
            }
        }
        Ok(())
    }

    fn application(
        &mut self,
        syntax: &mut SyntaxTrees,
        reference: TypeReferenceHandle,
        name: &Identifier,
        declaration: SourceSpan,
        lifetimes: &[Identifier],
        arguments: &[TypeReferenceHandle],
        _bound: &[String],
    ) -> Result<(), Diagnostic> {
        let mut selected = None;
        for forest in &self.declarations {
            let mut declarations = forest.root_items().filter_map(|item| match item {
                Item::TypeAlias(alias) if alias.name.source_span() == declaration => Some(alias),
                _ => None,
            });
            if let Some(alias) = declarations.next() {
                if declarations.next().is_some() {
                    return Err(Diagnostic::error(
                        "selected type alias has ambiguous source declarations",
                    )
                    .with_source_span(name.source_span()));
                }
                selected = Some((*forest, alias.clone()));
                break;
            }
        }
        let (forest, alias) = selected.ok_or_else(|| {
            Diagnostic::error("selected type alias has no retained source declaration")
                .with_source_span(name.source_span())
        })?;
        if alias.lifetime_parameters.len() != lifetimes.len()
            || alias.type_parameters.count() as usize != arguments.len()
        {
            return Err(Diagnostic::error(
                "type alias application does not supply its complete declared telescope",
            )
            .with_source_span(name.source_span()));
        }
        if self.active.contains(&declaration) {
            return Err(Diagnostic::error(
                "transparent type alias expansion contains a recursive declaration cycle",
            )
            .with_source_span(name.source_span()));
        }
        let mut types = Vec::new();
        let mut lifetime_uses = Vec::new();
        let mut array_length_uses = Vec::new();
        let mut unresolved_expression_operands = Vec::new();
        bind_type_occurrences(
            forest,
            alias.authored_expansion,
            &alias,
            arguments,
            lifetimes,
            &mut types,
            &mut lifetime_uses,
            &mut array_length_uses,
            &mut unresolved_expression_operands,
        );
        if let Some(operand) = unresolved_expression_operands.first() {
            return Err(Diagnostic::error("transparent alias expansion cannot yet retain the declaration and supplying scopes of this expression operand").with_source_span(forest.expressions.source_span(*operand)));
        }
        let authored = syntax.type_references.type_reference(reference).clone();
        let raw = syntax.type_references.insert(authored);
        syntax
            .type_references
            .retain_copied_operand_owner(reference, raw);
        let (expanded, supplied_copies) = syntax.instantiate_type_reference_from(
            forest,
            alias.authored_expansion,
            &types,
            &[],
            &lifetime_uses,
            &array_length_uses,
        );
        self.active.push(declaration);
        let prior_supplied = self.supplied.len();
        self.supplied.extend_from_slice(arguments);
        self.supplied.extend(supplied_copies);
        let bound = forest
            .items
            .type_parameters(alias.type_parameters)
            .iter()
            .map(|parameter| parameter.name.as_str(forest).to_owned())
            .collect::<Vec<_>>();
        let result = self.reference(syntax, expanded, &bound);
        self.supplied.truncate(prior_supplied);
        self.active.pop();
        result?;
        syntax
            .type_references
            .replace_type_occurrence(reference, expanded);
        syntax
            .type_references
            .retain_type_alias_application(TypeAliasApplicationOrigin {
                instance: reference,
                application: raw,
                declaration,
                definition_references: definition_reference_spans(forest, &alias),
            });
        Ok(())
    }
}

/// These are lexical references in the original declaration, not rendered
/// identities. Supplied actuals carry their own distinct source occurrences.
fn definition_reference_spans(
    forest: &SyntaxTrees,
    alias: &TypeAliasDefinition,
) -> Vec<SourceSpan> {
    let parameters = forest.items.type_parameters(alias.type_parameters);
    let mut positions = Vec::new();
    super::generic_data::collect_type_positions(
        forest,
        alias.authored_expansion,
        &mut positions,
        true,
    );
    let mut references = Vec::new();
    for position in positions {
        let name = match forest.type_references.type_reference(position) {
            TypeReferenceNode::Named(name) => Some(name),
            TypeReferenceNode::Generic { base_name, .. } => Some(base_name),
            TypeReferenceNode::DynamicTrait { trait_name, .. } => Some(trait_name),
            TypeReferenceNode::FixedArray {
                length: syntax_trees::types::FixedArrayLength::ConstParameter(name),
                ..
            } => Some(name),
            _ => None,
        };
        if let Some(name) = name
            && !parameters
                .iter()
                .any(|parameter| parameter.name.as_str(forest) == name.as_str(forest))
        {
            if !references.contains(&name.source_span()) {
                references.push(name.source_span());
            }
        }
    }
    references
}

fn bind_type_occurrences(
    templates: &SyntaxTrees,
    reference: TypeReferenceHandle,
    alias: &TypeAliasDefinition,
    arguments: &[TypeReferenceHandle],
    lifetimes: &[Identifier],
    types: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
    lifetime_uses: &mut Vec<(SourceSpan, String, Identifier)>,
    array_length_uses: &mut Vec<(SourceSpan, String, TypeReferenceHandle)>,
    unresolved_expression_operands: &mut Vec<syntax_trees::expression::ExpressionHandle>,
) {
    for application in templates.type_references.type_alias_applications(reference) {
        bind_type_occurrences(
            templates,
            application.application,
            alias,
            arguments,
            lifetimes,
            types,
            lifetime_uses,
            array_length_uses,
            unresolved_expression_operands,
        );
    }
    let mut lifetime = |name: &Identifier| {
        if let Some(position) = alias
            .lifetime_parameters
            .iter()
            .position(|parameter| parameter.as_str(templates) == name.as_str(templates))
        {
            lifetime_uses.push((
                name.source_span(),
                name.as_str(templates).to_owned(),
                lifetimes[position].clone(),
            ));
        }
    };
    let children = match templates.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name) => {
            if let Some(position) = templates
                .items
                .type_parameters(alias.type_parameters)
                .iter()
                .position(|parameter| parameter.name.as_str(templates) == name.as_str(templates))
            {
                types.push((reference, arguments[position]));
            }
            Vec::new()
        }
        TypeReferenceNode::Reference {
            referee,
            lifetime: region,
            ..
        } => {
            if let Some(region) = region {
                lifetime(region);
            }
            vec![*referee]
        }
        TypeReferenceNode::Generic {
            lifetime_arguments,
            arguments,
            ..
        } => {
            for region in lifetime_arguments {
                lifetime(region);
            }
            templates
                .type_references
                .type_reference_handles(*arguments)
                .to_vec()
        }
        TypeReferenceNode::ConstExpression(expression) => {
            use syntax_trees::expression::ExpressionNode;
            let mut expressions = std::collections::HashSet::new();
            super::generic_data::collect_expression_handles(
                templates,
                *expression,
                &mut expressions,
            );
            let mut children = Vec::new();
            for expression in expressions {
                super::generic_data::visit_expression_type_inputs(
                    templates,
                    expression,
                    &mut |reference| children.push(reference),
                );
                // Until an expression has declaration-owned bindings, copying
                // its names into the use's lexical scope could capture caller
                // locals or rebind a supplied runtime subject. Retain this as
                // unfinished source work, never as an alternative successful
                // expansion or a static-value promotion.
                if matches!(
                    templates.expressions.expression(expression),
                    ExpressionNode::Name(_) | ExpressionNode::Call(_)
                ) {
                    unresolved_expression_operands.push(expression);
                }
            }
            children
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut children = vec![*base_type];
            for constraint in templates.type_references.constraints(*constraints) {
                if let syntax_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                    children.extend_from_slice(
                        templates
                            .type_references
                            .type_reference_handles(domain.arguments),
                    );
                }
            }
            children
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let mut children = vec![*element_type];
            match length {
                syntax_trees::types::FixedArrayLength::Expression(reference) => {
                    children.push(*reference)
                }
                syntax_trees::types::FixedArrayLength::ConstParameter(name) => {
                    if let Some(position) = templates
                        .items
                        .type_parameters(alias.type_parameters)
                        .iter()
                        .position(|parameter| {
                            parameter.name.as_str(templates) == name.as_str(templates)
                        })
                    {
                        array_length_uses.push((
                            name.source_span(),
                            name.as_str(templates).to_owned(),
                            arguments[position],
                        ));
                    }
                }
                _ => {}
            }
            children
        }
        TypeReferenceNode::Slice { element_type } => vec![*element_type],
        TypeReferenceNode::DynamicTrait {
            trait_arguments, ..
        } => templates
            .type_references
            .type_reference_handles(*trait_arguments)
            .to_vec(),
        _ => Vec::new(),
    };
    for child in children {
        bind_type_occurrences(
            templates,
            child,
            alias,
            arguments,
            lifetimes,
            types,
            lifetime_uses,
            array_length_uses,
            unresolved_expression_operands,
        );
    }
}

/// Run before generic-data applications are demanded. Re-entry is idempotent:
/// only authored alias heads expand, while retained application rows are not
/// themselves traversal roots.
pub(crate) fn normalize(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    normalize_with_declarations(syntax, selection, &[])
}

pub(super) fn normalize_with_declarations(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection<'_>,
    declarations: &[&SyntaxTrees],
) -> Result<(), Vec<Diagnostic>> {
    if !std::iter::once(&*syntax)
        .chain(declarations.iter().copied())
        .any(|forest| {
            forest
                .root_items()
                .any(|item| matches!(item, Item::TypeAlias(_)))
        })
    {
        return Ok(());
    }
    let templates = syntax.clone();
    let mut forests = vec![&templates];
    forests.extend_from_slice(declarations);
    let mut expansion = Expansion {
        declarations: forests,
        selection,
        active: Vec::new(),
        supplied: Vec::new(),
    };
    for handle in templates.root_item_handles() {
        let mut item = templates.root_item(*handle).clone();
        if let Item::TypeAlias(alias) = &mut item {
            if alias.expansion == alias.authored_expansion {
                let (working, _) = syntax.instantiate_type_reference_from(
                    &templates,
                    alias.authored_expansion,
                    &[],
                    &[],
                    &[],
                    &[],
                );
                alias.expansion = working;
                syntax.items.replace_item(*handle, item.clone());
            }
            if let Item::TypeAlias(alias) = &item {
                mark_header_references(syntax, alias.authored_expansion);
            }
        }
        if let Err(error) = expansion.item(syntax, &item, &[]) {
            return Err(vec![error]);
        }
        if let Item::TypeAlias(alias) = item {
            mark_header_references(syntax, alias.expansion);
        }
    }
    Ok(())
}

/// Build-time evaluation expands type heads before discovering constant and
/// representation work. Imported declaration forests lend headers only.
pub fn normalize_type_aliases(
    mut request: super::generic_data::GenericDataRequest<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let selection = ConstantSelection::with_retained_base(
        &request.syntax,
        request.sources,
        request.top_level_bindings,
        request.retained_base,
    )?;
    let declarations = request
        .declaration_syntax
        .into_iter()
        .chain(request.retained_declaration_syntax)
        .collect::<Vec<_>>();
    normalize_with_declarations(&mut request.syntax, &selection, &declarations)?;
    Ok(request.syntax)
}

fn mark_header_references(syntax: &mut SyntaxTrees, root: TypeReferenceHandle) {
    let mut pending = vec![root];
    let mut seen = Vec::new();
    while let Some(root) = pending.pop() {
        if seen.contains(&root) {
            continue;
        }
        seen.push(root);
        let mut references = Vec::new();
        super::generic_data::collect_type_positions(syntax, root, &mut references, true);
        for reference in references {
            pending.extend(
                syntax
                    .type_references
                    .type_alias_applications(reference)
                    .map(|origin| origin.application),
            );
            syntax
                .type_references
                .retain_alias_header_reference(reference);
        }
    }
}

impl Expansion<'_, '_> {
    fn parameters(
        &mut self,
        syntax: &mut SyntaxTrees,
        parameters: HandleSpan<syntax_trees::item::TypeParameter>,
        inherited: &[String],
    ) -> Result<Vec<String>, Diagnostic> {
        let parameters = syntax.items.type_parameters(parameters).to_vec();
        let mut bound = inherited.to_vec();
        bound.extend(
            parameters
                .iter()
                .map(|parameter| parameter.name.as_str(syntax).to_owned()),
        );
        for parameter in parameters {
            match parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    self.reference(syntax, type_reference, &bound)?
                }
                TypeParameterKind::Machine {
                    contract:
                        Some(syntax_trees::item::MachineParameterContract::Structural(signature)),
                } => self.signature(syntax, &signature, &bound)?,
                _ => {}
            }
        }
        Ok(bound)
    }

    fn signature(
        &mut self,
        syntax: &mut SyntaxTrees,
        signature: &syntax_trees::item::StateSignature,
        inherited: &[String],
    ) -> Result<(), Diagnostic> {
        let bound = self.parameters(syntax, signature.type_parameters, inherited)?;
        self.reference(syntax, signature.return_type, &bound)?;
        let parameters = syntax.items.state_parameters(signature.parameters).to_vec();
        for parameter in parameters {
            self.reference(
                syntax,
                syntax.items.state_parameter(parameter).type_reference,
                &bound,
            )?;
        }
        let statements = syntax.items.statements(signature.default_body).to_vec();
        self.statements(syntax, &statements, &bound)
    }

    fn statements(
        &mut self,
        syntax: &mut SyntaxTrees,
        statements: &[syntax_trees::statement::StatementHandle],
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        for statement in statements {
            for reference in super::generic_data::statement_type_inputs(syntax, *statement) {
                self.reference(syntax, reference, bound)?;
            }
        }
        Ok(())
    }

    fn machine(
        &mut self,
        syntax: &mut SyntaxTrees,
        machine: &syntax_trees::item::Machine,
        inherited: &[String],
    ) -> Result<(), Diagnostic> {
        let bound = self.parameters(syntax, machine.type_parameters, inherited)?;
        let states = syntax.items.state_handles(machine.states).to_vec();
        for handle in states {
            let state = syntax.items.state(handle).clone();
            self.reference(syntax, state.return_type, &bound)?;
            let parameters = syntax.items.state_parameters(state.parameters).to_vec();
            for parameter in parameters {
                self.reference(
                    syntax,
                    syntax.items.state_parameter(parameter).type_reference,
                    &bound,
                )?;
            }
            let statements = syntax.items.statements(state.statements).to_vec();
            self.statements(syntax, &statements, &bound)?;
        }
        for position in 0..machine.nested_machines.count() {
            let item = syntax
                .items
                .item(machine.nested_machines.handle_at(position))
                .clone();
            self.item(syntax, &item, &bound)?;
        }
        Ok(())
    }

    fn item(
        &mut self,
        syntax: &mut SyntaxTrees,
        item: &Item,
        inherited: &[String],
    ) -> Result<(), Diagnostic> {
        use syntax_trees::item::{ConformanceMember, DataMember};
        match item {
            Item::TypeAlias(alias) => {
                let bound = self.parameters(syntax, alias.type_parameters, inherited)?;
                self.active.push(alias.name.source_span());
                let result = self.reference(syntax, alias.expansion, &bound);
                self.active.pop();
                result?;
            }
            Item::Data(data) => {
                let bound = self.parameters(syntax, data.type_parameters, inherited)?;
                let members = syntax.items.data_members(data.members).to_vec();
                for member in members {
                    match member {
                        DataMember::Field(field) => {
                            self.reference(syntax, field.type_reference, &bound)?
                        }
                        DataMember::Variant(variant) => {
                            let fields = syntax.items.data_payload_fields(variant.payload).to_vec();
                            for field in fields {
                                self.reference(syntax, field.type_reference, &bound)?;
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Machine(machine) => self.machine(syntax, machine, inherited)?,
            Item::Trait(definition) => {
                let bound = self.parameters(syntax, definition.type_parameters, inherited)?;
                let parents = syntax
                    .type_references
                    .type_reference_handles(definition.parents)
                    .to_vec();
                for parent in parents {
                    self.reference(syntax, parent, &bound)?;
                }
                if let Some(reference) = definition.refines {
                    self.reference(syntax, reference, &bound)?;
                }
                let signatures = syntax.items.state_signatures(definition.machines).to_vec();
                for signature in signatures {
                    let signature = syntax.items.state_signature(signature).clone();
                    self.signature(syntax, &signature, &bound)?;
                }
            }
            Item::Conformance(definition) => {
                let bound = self.parameters(syntax, definition.type_parameters, inherited)?;
                let arguments = syntax
                    .type_references
                    .type_reference_handles(definition.trait_arguments)
                    .to_vec();
                for argument in arguments {
                    self.reference(syntax, argument, &bound)?;
                }
                let members = syntax
                    .items
                    .conformance_members(definition.members)
                    .to_vec();
                for member in members {
                    match member {
                        ConformanceMember::Machine(machine)
                        | ConformanceMember::Synthesized(machine)
                        | ConformanceMember::TraitDefault { machine, .. } => {
                            self.machine(syntax, &machine, &bound)?
                        }
                        _ => {}
                    }
                }
            }
            Item::Const(definition) => {
                self.reference(syntax, definition.type_reference, inherited)?
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod type_alias_tests;
