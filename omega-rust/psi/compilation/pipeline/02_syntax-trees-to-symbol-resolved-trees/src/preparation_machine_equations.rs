/*
    Closed machine applications discharge the same structural equations as data.

    Ordinary provisional resolution supplies exact selected declaration handles.
    The lowering pass records private source links while producing those handles;
    source coordinates and leaf spellings never reconstruct call identity. This
    preparation route combines fixed static arguments with declaration-backed
    ordinary argument/result types before solving structural equations. It never
    observes initializer contents or flow bounds. Open applications remain
    obligations and reject rather than entering later inference unchecked.
    Selected scalar constants share declaration canonicalization across direct
    and nested arguments. Nested constructor slots consume the selected declared
    carrier before scalar normalization erases it; direct arguments retain their
    authored syntax for the ordinary immutable static-argument checker.
*/

use super::{
    declaration_selection::ConstantSelection,
    generic_data::module_constants,
    type_equations::{self, EquationTemplate},
};
use crate::tables::syntax_items;
use crate::tables::syntax_type_references;
use arena::Handle;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::literals::{IntegerLiteral, IntegerRadix};
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::{
    SyntaxTrees,
    expression::StaticMachineArgument,
    item::{Machine, TypeParameterKind},
    types::{TypeReferenceHandle, TypeReferenceNode},
};

mod argument_types {
    /*
        Declared type observations for one exact selected ordinary call.

        Resolution owns the declaration selection; lowering records handle links to
        the corresponding authored type. These links survive provisional resolution
        without reselecting by a source coordinate or a leaf spelling. An initializer,
        literal element, flow predicate or private postcondition supplies no type.
        Open callee results first consume the exact selected argument substitution
        from the shared structural solver. Unapplied/underdetermined results remain
        obligations; this reader never inspects a callee body or replaces the later
        compatibility, formation, property and invocation-contract checks.
    */

    use super::{ConstantSelection, SourceLinks};
    use crate::tables::body_expressions;
    use crate::tables::resolved_declarations;
    use arena::{Handle, HandleSpan};
    use diagnostics::Diagnostic;
    use expression::ExpressionNode;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::types::TypeReference;
    use symbol_resolved_trees::{SymbolResolvedTrees, expression, statement, types};
    use symbols::{SymbolHandle, SymbolKind};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    mod projections {
        /*
            Declared field observations of completed generic results.

            A result can name an existing instance or retain a closed application until
            later synthesis. Both select the declaration before reading its field; the
            latter applies the ordinary equation tuple and constructor substitution.
        */

        use super::{
            ConstantSelection, DeclaredTypeOrigin, SourceLinks, closed_type, declared_type,
            expression_types,
        };
        use crate::preparation::{
            generic_data,
            type_equations::{self, EquationTemplate},
        };
        use crate::tables::syntax_items;
        use crate::tables::syntax_type_references;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees::{SymbolResolvedTrees, expression, types};
        use symbols::{SymbolHandle, SymbolKind};
        use syntax_trees::{
            SyntaxTrees,
            item::TypeParameterKind,
            types::{TypeReferenceHandle, TypeReferenceNode},
        };
        use types::TypeReference;

        pub(super) fn member_types<'trees>(
            syntax: &mut SyntaxTrees,
            trees: &'trees SymbolResolvedTrees,
            sources: &SourceLinks,
            member: &expression::TableMemberExpression,
            selection: &ConstantSelection<'_>,
            pending: &mut Vec<Diagnostic>,
        ) -> Option<Vec<(TypeReferenceHandle, &'trees TypeReference)>> {
            if member.member_symbol.is_valid()
                && let Some(field) =
                    declared_type(syntax, trees, sources, member.member_symbol, false)
            {
                return Some(vec![field]);
            }
            if member.case_variant.is_some() {
                return None;
            }
            let receivers =
                expression_types(syntax, trees, sources, member.receiver, selection, pending)?;
            let mut fields = Vec::new();
            for (authored, resolved) in receivers {
                let mut reference = authored;
                loop {
                    reference = match syntax.type_references.type_reference(reference) {
                        TypeReferenceNode::Reference { referee, .. } => *referee,
                        TypeReferenceNode::Constrained { base_type, .. } => *base_type,
                        _ => break,
                    };
                }
                let (selected, application) = match syntax.type_references.type_reference(reference)
                {
                    TypeReferenceNode::Named(name) => (
                        generic_data::selected_data_item(syntax, Some(selection), name),
                        None,
                    ),
                    TypeReferenceNode::Generic {
                        base_name,
                        arguments,
                        ..
                    } => (
                        generic_data::selected_data_item(syntax, Some(selection), base_name),
                        Some((
                            base_name.clone(),
                            syntax_type_references(syntax)
                                .type_reference_handles(*arguments)
                                .to_vec(),
                        )),
                    ),
                    _ => (None, None),
                };
                let owner = selected
                    .and_then(|declaration| {
                        sources
                            .data_declarations
                            .iter()
                            .find(|(item, _)| *item == declaration)
                    })
                    .filter(|(_, ordinal)| *ordinal < trees.data_definitions.len())
                    .map(|(_, ordinal)| trees.data_definitions[*ordinal].symbol)
                    .or_else(|| closed_nominal_owner(trees, resolved))?;
                let field = trees.symbols.find_child_by_name_and_kind(
                    owner,
                    member.member.as_str(),
                    SymbolKind::Field,
                )?;
                if let Some((base_name, arguments)) = application {
                    let declaration = syntax.root_item(selected?).as_data()?.clone();
                    let (field_type, resolved_type) =
                        sources
                            .declared_types
                            .iter()
                            .find_map(|&(origin, authored)| {
                                if !matches!(origin, DeclaredTypeOrigin::Field(_)) {
                                    return None;
                                }
                                let (symbol, resolved) = origin.declaration(trees)?;
                                (symbol == field).then_some((authored, resolved))
                            })?;
                    let parameters = syntax_items(syntax)
                        .type_parameters(declaration.type_parameters)
                        .to_vec();
                    let names = parameters
                        .iter()
                        .map(|parameter| parameter.name.as_str(syntax).to_owned())
                        .collect::<Vec<_>>();
                    let const_types = parameters
                        .iter()
                        .map(|parameter| match parameter.kind {
                            TypeParameterKind::Const { type_reference } => Some(type_reference),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let equations = type_equations::classify_type_equations(
                        syntax,
                        &parameters,
                        syntax.items.proof_facts(declaration.requires_facts),
                    );
                    let name = declaration.name.as_str(syntax).to_owned();
                    let template = EquationTemplate {
                        kind: "data",
                        name: &name,
                        parameters: declaration.type_parameters,
                        invocation_lifetimes: &[],
                        parameter_names: &names,
                        const_parameter_types: &const_types,
                        type_equations: &equations,
                    };
                    let constants =
                        generic_data::module_constants::lexical_integer_const_values(syntax);
                    let mut warnings = Vec::new();
                    let completed = type_equations::complete_equation_arguments(
                        syntax,
                        template,
                        &base_name,
                        &arguments,
                        &constants,
                        Some(selection),
                        &mut warnings,
                    )
                    .and_then(|tuple| {
                        type_equations::materialize_completed_type(
                            syntax,
                            template,
                            &base_name,
                            &tuple,
                            field_type,
                            &constants,
                            Some(selection),
                        )
                    });
                    match completed {
                        Ok(field_type) => fields.push((field_type, resolved_type)),
                        Err(reason) => {
                            pending.push(reason);
                            return None;
                        }
                    }
                } else {
                    fields.push(declared_type(syntax, trees, sources, field, false)?);
                }
            }
            Some(fields)
        }

        fn closed_nominal_owner(
            trees: &SymbolResolvedTrees,
            reference: &TypeReference,
        ) -> Option<SymbolHandle> {
            if !closed_type(trees, reference) {
                return None;
            }
            let mut reference = reference;
            loop {
                reference = match reference {
                    TypeReference::Reference(reference) => {
                        trees.child_type_reference(reference.referee)
                    }
                    TypeReference::Constrained(reference) => {
                        trees.child_type_reference(reference.base_type)
                    }
                    _ => break,
                };
            }
            match reference {
                TypeReference::Named { symbol, .. }
                    if trees.data_definition_by_symbol(*symbol).is_some() =>
                {
                    Some(*symbol)
                }
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub(crate) enum DeclaredTypeOrigin {
        Parameter(Handle<symbol_resolved_trees::signature::StateParameter>),
        Result(Handle<symbol_resolved_trees::state::State>),
        Field(Handle<DataMember>),
        Payload(Handle<symbol_resolved_trees::data::DataField>),
        Constant(usize),
    }

    impl DeclaredTypeOrigin {
        fn declaration(
            self,
            trees: &SymbolResolvedTrees,
        ) -> Option<(SymbolHandle, &TypeReference)> {
            let declarations = &trees.tables.declarations;
            match self {
                Self::Parameter(handle) => {
                    let parameter = declarations.state_parameters.get(handle);
                    Some((parameter.symbol, &parameter.type_reference))
                }
                Self::Result(handle) => {
                    let state = trees.machine_state(handle);
                    Some((state.symbol, state.return_type.as_ref()?))
                }
                Self::Field(handle) => {
                    let DataMember::Field(field) = declarations.data_members.get(handle) else {
                        return None;
                    };
                    Some((field.symbol, &field.type_reference))
                }
                Self::Payload(handle) => {
                    let field = declarations.data_payload_fields.get(handle);
                    Some((field.symbol, &field.type_reference))
                }
                Self::Constant(index) => {
                    let constant = &trees.roots.const_declarations[index];
                    Some((constant.symbol, &constant.declared_type))
                }
            }
        }
    }

    pub(super) fn closed_type(trees: &SymbolResolvedTrees, root: &TypeReference) -> bool {
        let binder = |symbol: SymbolHandle| {
            symbol.is_valid()
                && matches!(
                    trees.symbols.get(symbol).kind,
                    SymbolKind::TypeParameter
                        | SymbolKind::Parameter
                        | SymbolKind::MachineParameter
                        | SymbolKind::ConformanceParameter
                )
        };
        let mut pending = vec![root];
        while let Some(reference) = pending.pop() {
            match reference {
                TypeReference::AliasUse { expansion, .. } => pending.push(expansion),
                TypeReference::Named { symbol, .. }
                | TypeReference::DynamicTrait { symbol, .. } => {
                    if binder(*symbol) {
                        return false;
                    }
                }
                TypeReference::Reference(reference) => {
                    pending.push(trees.child_type_reference(reference.referee))
                }
                TypeReference::Slice(slice) => {
                    pending.push(trees.child_type_reference(slice.element_type))
                }
                TypeReference::FixedArray(array) => {
                    match &array.length {
                        types::FixedArrayLength::Literal(_) => {}
                        types::FixedArrayLength::ConstParameter { symbol, .. }
                            if symbol.is_valid()
                                && trees.symbols.get(*symbol).kind == SymbolKind::Const => {}
                        _ => return false,
                    }
                    pending.push(trees.child_type_reference(array.element_type));
                }
                TypeReference::Constrained(constrained) => {
                    pending.push(trees.child_type_reference(constrained.base_type));
                    for constraint in trees
                        .tables
                        .types
                        .constraints
                        .span_or_empty(constrained.constraints)
                    {
                        if let types::TypeConstraint::Domain(domain) = constraint {
                            pending.extend(
                                domain
                                    .arguments
                                    .handles()
                                    .map(|handle| trees.child_type_reference(handle)),
                            );
                        }
                    }
                }
                TypeReference::Generic(generic) => {
                    if binder(generic.base_symbol) {
                        return false;
                    }
                    pending.extend(
                        generic
                            .arguments
                            .handles()
                            .map(|handle| trees.child_type_reference(handle)),
                    );
                }
                TypeReference::Callable { contract, .. } => {
                    let binders = super::BinderClosures::build(trees);
                    if super::classify_callable_contract(&binders, trees, *contract, &[], 0)
                        != super::ArgumentClosure::Closed
                    {
                        return false;
                    }
                }
                TypeReference::ConstExpression(_) => return false,
                TypeReference::Unit => {}
            }
        }
        true
    }

    fn declared_type<'trees>(
        syntax: &SyntaxTrees,
        trees: &'trees SymbolResolvedTrees,
        sources: &SourceLinks,
        symbol: SymbolHandle,
        result: bool,
    ) -> Option<(TypeReferenceHandle, &'trees TypeReference)> {
        if !symbol.is_valid() {
            return None;
        }
        for &(origin, authored) in &sources.declared_types {
            if result != matches!(origin, DeclaredTypeOrigin::Result(_)) {
                continue;
            }
            if let Some((selected, reference)) = origin.declaration(trees)
                && selected == symbol
                && closed_type(trees, reference)
            {
                return Some((authored, reference));
            }
        }
        if !result {
            for &(authored, resolved) in &sources.statements {
                if let statement::Statement::LocalData(local) =
                    trees.tables.declarations.state_statements.get(resolved)
                    && local.symbol == symbol
                    && !local.type_is_inferred
                    && closed_type(trees, &local.type_reference)
                    && let syntax_trees::statement::StatementNode::LocalData(authored_local) =
                        syntax.statements.statement(authored)
                {
                    return authored_local
                        .type_reference
                        .is_valid()
                        .then_some((authored_local.type_reference, &local.type_reference));
                }
            }
        }
        None
    }

    /*
        Early type observations are evidence for inference, not a second call
        admission boundary. A producer whose telescope cannot yet be observed keeps
        its diagnostic here; an independently determined consumer may proceed to
        ordinary checking of the unchanged call. If the consumer also lacks a tuple,
        its refusal includes the retained producer explanation.
    */
    pub(super) struct CallTypeObservations {
        pub(super) types: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
        pub(super) pending_diagnostics: Vec<Diagnostic>,
    }

    fn expression_types<'trees>(
        syntax: &mut SyntaxTrees,
        trees: &'trees SymbolResolvedTrees,
        sources: &SourceLinks,
        expression: expression::ExpressionHandle,
        selection: &ConstantSelection<'_>,
        pending_diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Vec<(TypeReferenceHandle, &'trees TypeReference)>> {
        match trees.tables.bodies.expressions.expression(expression) {
            ExpressionNode::Name(name) => declared_type(syntax, trees, sources, name.symbol, false)
                .map(|reference| vec![reference]),
            ExpressionNode::Borrow(borrow) => {
                let targets = expression_types(
                    syntax,
                    trees,
                    sources,
                    borrow.target,
                    selection,
                    pending_diagnostics,
                )?;
                let mut borrowed = Vec::with_capacity(targets.len());
                for (target, declaration) in targets {
                    let referee = match syntax.type_references.type_reference(target) {
                        TypeReferenceNode::Reference { referee, .. } => *referee,
                        _ => target,
                    };
                    let reference = syntax
                        .tables
                        .type_references
                        .insert_reference(referee, borrow.access);
                    borrowed.push((reference, declaration));
                }
                Some(borrowed)
            }
            ExpressionNode::Member(member) => projections::member_types(
                syntax,
                trees,
                sources,
                member,
                selection,
                pending_diagnostics,
            ),
            ExpressionNode::Call(call) => {
                if let Some(reference) =
                    declared_type(syntax, trees, sources, call.target_symbol, true)
                {
                    return Some(vec![reference]);
                }
                let index = super::selected_machine(trees, call.target_symbol)?;
                let (authored_result, resolved_result) =
                    sources
                        .declared_types
                        .iter()
                        .find_map(|&(origin, authored)| {
                            if !matches!(origin, DeclaredTypeOrigin::Result(_)) {
                                return None;
                            }
                            let (symbol, resolved) = origin.declaration(trees)?;
                            (symbol == call.target_symbol).then_some((authored, resolved))
                        })?;
                let (original, _) = sources
                    .expressions
                    .iter()
                    .find(|(_, resolved)| *resolved == expression)?;
                let syntax_trees::expression::ExpressionNode::Call(authored) =
                    syntax.expressions.expression(*original).clone()
                else {
                    return None;
                };
                let ordinary_types =
                    call_constraints(syntax, trees, sources, call, Some(expression), selection);
                pending_diagnostics.extend(ordinary_types.pending_diagnostics);
                let completed = super::complete_arguments(
                    syntax,
                    index,
                    &authored.target,
                    &authored.machine_arguments,
                    Some(authored_result),
                    &ordinary_types.types,
                    sources,
                    selection,
                );
                match completed {
                    Ok((_, result)) => result.map(|result| vec![(result, resolved_result)]),
                    Err(reason) => {
                        pending_diagnostics.push(reason);
                        None
                    }
                }
            }
            ExpressionNode::Match(dispatch) => {
                let mut references = Vec::new();
                for arm in trees.tables.bodies.expressions.match_arms(dispatch.arms) {
                    let observed = expression_types(
                        syntax,
                        trees,
                        sources,
                        arm.value,
                        selection,
                        pending_diagnostics,
                    )?;
                    references.extend(observed);
                }
                (!references.is_empty()).then_some(references)
            }
            _ => None,
        }
    }

    fn constraints(
        syntax: &mut SyntaxTrees,
        trees: &SymbolResolvedTrees,
        sources: &SourceLinks,
        target: SymbolHandle,
        arguments: HandleSpan<expression::ExpressionHandle>,
        selection: &ConstantSelection<'_>,
        pending_diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<(TypeReferenceHandle, TypeReferenceHandle)> {
        let mut observations = Vec::new();
        let Some((_, state)) = resolved_declarations(trees)
            .machine_states
            .iter()
            .find(|(_, state)| state.symbol == target)
        else {
            return observations;
        };
        let arguments = body_expressions(trees).expression_handles(arguments);
        if arguments.len() != state.parameters.count() as usize {
            return observations;
        }
        for (parameter, argument) in state.parameters.handles().zip(arguments) {
            let Some((_, formal)) = sources
                .declared_types
                .iter()
                .find(|(origin, _)| *origin == DeclaredTypeOrigin::Parameter(parameter))
            else {
                continue;
            };
            if let Some(actuals) = expression_types(
                syntax,
                trees,
                sources,
                *argument,
                selection,
                pending_diagnostics,
            ) {
                observations.extend(actuals.into_iter().map(|(actual, _)| (*formal, actual)));
            }
        }
        observations
    }

    pub(super) fn call_constraints(
        syntax: &mut SyntaxTrees,
        trees: &SymbolResolvedTrees,
        sources: &SourceLinks,
        call: &expression::TableCallExpression,
        occurrence: Option<expression::ExpressionHandle>,
        selection: &ConstantSelection<'_>,
    ) -> CallTypeObservations {
        let mut pending_diagnostics = Vec::new();
        let mut observations = constraints(
            syntax,
            trees,
            sources,
            call.target_symbol,
            call.arguments,
            selection,
            &mut pending_diagnostics,
        );
        let formal = sources
            .declared_types
            .iter()
            .find_map(|&(origin, authored)| {
                if let DeclaredTypeOrigin::Result(_) = origin
                    && origin
                        .declaration(trees)
                        .is_some_and(|(symbol, _)| symbol == call.target_symbol)
                {
                    Some(authored)
                } else {
                    None
                }
            });
        if let (Some(formal), Some(occurrence)) = (formal, occurrence) {
            for &(authored, resolved) in &sources.statements {
                if let statement::Statement::LocalData(local) =
                    trees.tables.declarations.state_statements.get(resolved)
                    && local.initial_value == occurrence
                    && !local.type_is_inferred
                    && closed_type(trees, &local.type_reference)
                    && let syntax_trees::statement::StatementNode::LocalData(local) =
                        syntax.statements.statement(authored)
                    && local.type_reference.is_valid()
                {
                    observations.push((formal, local.type_reference));
                }
            }
        }
        CallTypeObservations {
            types: observations,
            pending_diagnostics,
        }
    }

    pub(super) fn statement_constraints(
        syntax: &mut SyntaxTrees,
        trees: &SymbolResolvedTrees,
        sources: &SourceLinks,
        call: &statement::Call,
        selection: &ConstantSelection<'_>,
    ) -> CallTypeObservations {
        let mut pending_diagnostics = Vec::new();
        let types = constraints(
            syntax,
            trees,
            sources,
            call.target_symbol,
            call.arguments,
            selection,
            &mut pending_diagnostics,
        );
        CallTypeObservations {
            types,
            pending_diagnostics,
        }
    }
}
mod completion {
    /* Complete the selected machine application without changing static binder kinds. */

    use super::super::generic_data::closed_argument_identity;
    use super::*;
    use crate::tables::syntax_items;
    use type_equations::type_reference_parameter_mentions;

    pub(super) fn complete_arguments(
        syntax: &mut SyntaxTrees,
        selected: usize,
        target: &syntax_trees::identifier::Identifier,
        supplied: &[StaticMachineArgument],
        result_type: Option<TypeReferenceHandle>,
        ordinary_types: &[(TypeReferenceHandle, TypeReferenceHandle)],
        sources: &SourceLinks,
        selection: &ConstantSelection<'_>,
    ) -> Result<(Box<[StaticMachineArgument]>, Option<TypeReferenceHandle>), Diagnostic> {
        let Some((_, declaration)) = sources
            .declarations
            .iter()
            .find(|(index, _)| *index == selected)
        else {
            return Err(Diagnostic::error(
                "retained machine structural equation requires its original syntax for discharge",
            )
            .with_source_span(target.source_span()));
        };
        let all_parameters = syntax_items(syntax)
            .type_parameters(declaration.type_parameters)
            .to_vec();
        let evidence_count = declaration.conformance_bounds.len();
        let Some(prefix_count) = supplied.len().checked_sub(evidence_count) else {
            return Err(Diagnostic::error(
                "machine equation application requires its explicit conformance arguments",
            )
            .with_source_span(target.source_span()));
        };
        let (supplied, evidence) = supplied.split_at(prefix_count);
        let original_equations = type_equations::classify_type_equations(
            syntax,
            &all_parameters,
            &syntax.items.requirement_facts(declaration.contracts),
        );
        if supplied.len() > all_parameters.len() {
            return Err(Diagnostic::error(
                "machine equation application supplies too many static arguments",
            )
            .with_source_span(target.source_span()));
        }
        let mut all_names = all_parameters
            .iter()
            .map(|parameter| parameter.name.as_str(syntax).to_owned())
            .collect::<Vec<_>>();
        all_names.extend(
            declaration
                .conformance_bounds
                .iter()
                .map(|bound| bound.binder.as_str(syntax).to_owned()),
        );
        let signature_mentions = syntax_items(syntax)
            .state_handles(declaration.states)
            .first()
            .map(|entry| syntax.items.state(*entry))
            .into_iter()
            .flat_map(|entry| {
                syntax_items(syntax)
                    .state_parameters(entry.parameters)
                    .iter()
                    .map(|parameter| syntax.items.state_parameter(*parameter).type_reference)
                    .chain(std::iter::once(entry.return_type))
            })
            .chain(result_type)
            .flat_map(|reference| type_reference_parameter_mentions(syntax, reference, &all_names))
            .collect::<Vec<_>>();
        for (index, name) in all_names.iter().enumerate() {
            if all_parameters.get(index).is_some_and(|parameter| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Type
                        | TypeParameterKind::Const { .. }
                        | TypeParameterKind::Value { .. }
                )
            }) {
                continue;
            }
            if signature_mentions.contains(&index)
                || original_equations
                    .iter()
                    .any(|equation| equation.mentions_parameter(syntax, &all_names, index))
            {
                return Err(Diagnostic::error(format!(
                "machine structural inference cannot replace non-type binder `{name}` with a global declaration"
            ))
            .with_source_span(target.source_span()));
            }
        }
        let mut equation_positions = Vec::new();
        for (index, parameter) in all_parameters.iter().enumerate() {
            if matches!(
                parameter.kind,
                TypeParameterKind::Type
                    | TypeParameterKind::Const { .. }
                    | TypeParameterKind::Value { .. }
            ) {
                equation_positions.push(index);
            } else {
                if index >= supplied.len() {
                    return Err(Diagnostic::error(format!(
                        "machine equation application requires an explicit argument for `{}`",
                        all_names[index]
                    ))
                    .with_source_span(target.source_span()));
                }
            }
        }
        let parameters = equation_positions
            .iter()
            .map(|index| all_parameters[*index].clone())
            .collect::<Vec<_>>();
        let mut first = Handle::invalid();
        for parameter in &parameters {
            let handle = syntax.items.append_type_parameter(parameter.clone());
            if !first.is_valid() {
                first = handle;
            }
        }
        let equation_span = arena::HandleSpan::from_parts_or_empty(first, parameters.len() as u32);
        let equations = type_equations::classify_type_equations(
            syntax,
            &parameters,
            &syntax.items.requirement_facts(declaration.contracts),
        );
        let parameter_names = parameters
            .iter()
            .map(|parameter| parameter.name.as_str(syntax).to_owned())
            .collect::<Vec<_>>();
        let mut const_parameter_types = Vec::new();
        for parameter in &parameters {
            const_parameter_types.push(match parameter.kind {
                TypeParameterKind::Type => None,
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => Some(type_reference),
                _ => {
                    return Err(Diagnostic::error(
                        "machine equation completion requires type or value binders",
                    )
                    .with_source_span(target.source_span()));
                }
            });
        }
        let mut arguments = Vec::new();
        for (index, parameter) in equation_positions.iter().zip(&parameters) {
            let Some(argument) = supplied.get(*index) else {
                break;
            };
            let reference = match &parameter.kind {
            TypeParameterKind::Type if argument.type_reference.is_valid() => argument.type_reference,
            TypeParameterKind::Type if argument.application.is_some() => {
                application_argument_reference(syntax, argument, &parameter.kind, selection)?
            }
            TypeParameterKind::Type if argument.const_literal.is_none() && argument.application.is_none() && argument.evidence_projection.is_none() && !argument.path.is_empty() => {
                let name = argument.path.joined_path_in(syntax);
                syntax.type_references.insert(TypeReferenceNode::Named(syntax_trees::identifier::Identifier::new(name, argument.path[0].source_span())))
            }
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } if argument.const_literal.is_some() => {
                let value = argument.const_literal.as_ref().and_then(IntegerLiteral::value_bignum).ok_or_else(|| Diagnostic::error("machine equation const argument is not an exact integer"))?;
                syntax.type_references.insert(TypeReferenceNode::Named(syntax_trees::identifier::Identifier::generated(value.to_string())))
            }
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                supplied_const_argument_reference(syntax, argument, selection)?
            }
            _ => return Err(Diagnostic::error("machine equation argument mixes type and value kinds or retains an open static argument").with_source_span(target.source_span())),
        };
            if matches!(parameter.kind, TypeParameterKind::Type)
                && closed_argument_identity(syntax, Some(selection), reference, false).is_none()
            {
                return Err(Diagnostic::error(
                    "machine equation requires a closed explicit type argument",
                )
                .with_source_span(target.source_span()));
            }
            arguments.push(reference);
        }
        let const_values = module_constants::lexical_integer_const_values(syntax);
        let mut warnings = Vec::new();
        let declaration_name = declaration.name.as_str(syntax).to_owned();
        let tuple = type_equations::complete_equation_arguments_with_types(
            syntax,
            EquationTemplate {
                kind: "machine",
                name: &declaration_name,
                parameters: equation_span,
                invocation_lifetimes: &declaration.lifetime_parameters,
                parameter_names: &parameter_names,
                const_parameter_types: &const_parameter_types,
                type_equations: &equations,
            },
            target,
            &arguments,
            ordinary_types,
            &const_values,
            Some(selection),
            &mut warnings,
        )?;
        let result_type = result_type
            .map(|result| {
                type_equations::materialize_completed_type(
                    syntax,
                    EquationTemplate {
                        kind: "machine",
                        name: &declaration_name,
                        parameters: equation_span,
                        invocation_lifetimes: &declaration.lifetime_parameters,
                        parameter_names: &parameter_names,
                        const_parameter_types: &const_parameter_types,
                        type_equations: &equations,
                    },
                    target,
                    &tuple,
                    result,
                    &const_values,
                    Some(selection),
                )
            })
            .transpose()?;
        // Landing errors reject; see report_preparation_diagnostics.
        if let Some(error) = warnings.iter().find(|warning| warning.is_error()) {
            return Err(error.clone());
        }
        for warning in warnings {
            eprintln!("{warning}");
        }
        let mut completed = supplied.to_vec();
        for ((index, reference), parameter) in
            equation_positions.iter().zip(&tuple).zip(&parameters)
        {
            let Some(argument) = completed.get_mut(*index) else {
                continue;
            };
            if matches!(parameter.kind, TypeParameterKind::Type) && argument.application.is_some() {
                argument.type_reference = *reference;
                argument.path = Box::default();
                argument.application = None;
            }
        }
        for ((index, reference), parameter) in
            equation_positions.into_iter().zip(tuple).zip(parameters)
        {
            if index < supplied.len() {
                continue;
            }
            let mut argument = StaticMachineArgument {
                type_reference: TypeReferenceHandle::invalid(),
                path: Box::default(),
                application: None,
                const_literal: None,
                evidence_projection: None,
            };
            match parameter.kind {
                TypeParameterKind::Type => argument.type_reference = reference,
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    let Some(value) = syntax.type_references.type_reference(reference).as_named()
                    else {
                        return Err(Diagnostic::error(
                            "machine equation did not produce a closed const argument",
                        ));
                    };
                    let text = value.as_str(syntax);
                    if let Some(value) = type_equations::normalized_boolean_argument(text) {
                        argument.path = vec![syntax_trees::identifier::Identifier::generated(
                            CanonicalConstValue::boolean(value).atom(),
                        )]
                        .into_boxed_slice();
                        completed.push(argument);
                        continue;
                    }
                    if CanonicalConstValue::from_atom(text).is_some() {
                        argument = inferred_const_argument(syntax, reference, selection)?;
                        completed.push(argument);
                        continue;
                    }
                    if text.parse::<i128>().is_err() {
                        argument.path = vec![value.clone()].into_boxed_slice();
                        completed.push(argument);
                        continue;
                    }
                    argument.const_literal = Some(
                        IntegerLiteral::from_parts(
                            text.starts_with('-'),
                            IntegerRadix::Decimal,
                            text.trim_start_matches('-'),
                        )
                        .map_err(Diagnostic::error)?,
                    );
                }
                _ => unreachable!("parameter kinds were admitted above"),
            }
            completed.push(argument);
        }
        completed.extend_from_slice(evidence);
        Ok((completed.into_boxed_slice(), result_type))
    }
}
use super::generic_data::canonicalize_selected_declared_const_definition;
use super::generic_data::closed_argument_identity;
pub(crate) use argument_types::DeclaredTypeOrigin;
use argument_types::call_constraints;
use completion::complete_arguments;
use numerics::arithmetic::ArithmeticDomain;
use std::iter::once;
use symbol_resolved_trees::expression::ExpressionHandle;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeConstraint;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ItemHandle;
use syntax_trees::statement::StatementHandle;
use syntax_trees::statement::StatementNode;
use type_equations::const_binder_envelope;

#[derive(Debug, Clone, Default, compiler_storage::Record)]
pub(crate) struct SourceLinks {
    pub(crate) declared_types: Vec<(DeclaredTypeOrigin, TypeReferenceHandle)>,
    pub(crate) declarations: Vec<(usize, Machine)>,
    pub(crate) data_declarations: Vec<(ItemHandle, usize)>,
    pub(crate) expressions: Vec<(syntax_trees::expression::ExpressionHandle, ExpressionHandle)>,
    pub(crate) statements: Vec<(StatementHandle, Handle<Statement>)>,
}

impl SourceLinks {
    pub(crate) fn same_syntax(
        &self,
        other: &Self,
        left: &syntax_trees::SyntaxTextOwner,
        right: &syntax_trees::SyntaxTextOwner,
    ) -> bool {
        use syntax_trees::SyntaxEquivalent;
        let Self {
            declared_types,
            declarations,
            data_declarations,
            expressions,
            statements,
        } = self;
        *declared_types == other.declared_types
            && *data_declarations == other.data_declarations
            && *expressions == other.expressions
            && *statements == other.statements
            && declarations.same_syntax(&other.declarations, left, right)
    }
}

pub(crate) fn required(syntax: &SyntaxTrees, base: Option<&SymbolResolvedTrees>) -> bool {
    syntax.root_items().any(|item| {
        item.as_machine()
            .is_some_and(|machine| !requirement_equations(syntax, machine).is_empty())
    }) || base.is_some_and(|base| {
        base.machines
            .iter()
            .any(|machine| machine.has_structural_type_equations)
    })
}

/*
    The type equations among a machine's `requires` facts. Other facts remain
    ordinary entry obligations.
*/
pub(crate) fn machine_type_equations(
    syntax: &SyntaxTrees,
    machine: &Machine,
) -> Vec<type_equations::TypeEquation> {
    type_equations::classify_machine_equations(
        syntax,
        syntax.items.type_parameters(machine.type_parameters),
        &machine.lifetime_parameters,
        &syntax.items.requirement_facts(machine.contracts),
    )
}

pub(crate) fn validate_declarations(
    syntax: &SyntaxTrees,
    selection: Option<&ConstantSelection<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    for item in syntax.root_items() {
        let Some(machine) = item.as_machine() else {
            continue;
        };
        let equations = machine_type_equations(syntax, machine);
        if !equations.is_empty() {
            if (machine.spelling.is_some() || !machine.satisfies.is_empty())
                && equations
                    .iter()
                    .any(type_equations::TypeEquation::requires_argument_recovery)
            {
                return Err(vec![Diagnostic::error("machine structural equations currently require explicit calls; operator and conformance supplies retain undisclosed equations").with_source_span(machine.name.source_span())]);
            }
            for equation in equations {
                equation
                    .validate_kind("machine")
                    .map_err(|error| vec![error])?;
            }
        }
        for equation in type_equations::classify_type_equations(
            syntax,
            syntax.items.type_parameters(machine.type_parameters),
            &syntax.items.requirement_facts(machine.contracts),
        ) {
            equation
                .reject_unclosed_names(syntax, selection)
                .map_err(|error| {
                    let mut diagnostic = Diagnostic::error(format!(
                        "generic machine `{}` {}",
                        machine.name.as_str(syntax),
                        error.message
                    ));
                    if let Some(span) = error.source_span {
                        diagnostic = diagnostic.with_source_span(span);
                    }
                    vec![diagnostic]
                })?;
        }
    }
    Ok(())
}

fn selected_machine(trees: &SymbolResolvedTrees, symbol: symbols::SymbolHandle) -> Option<usize> {
    let declaration = trees.symbols.get(symbol);
    let owner = match declaration.kind {
        symbols::SymbolKind::Machine => symbol,
        symbols::SymbolKind::State => declaration.parent,
        _ => return None,
    };
    trees
        .machines
        .iter()
        .position(|machine| machine.symbol == owner)
}

pub(crate) fn complete(
    syntax: &mut SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    selection: &ConstantSelection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let mut completed_type_roots = Vec::new();
    for conformance in &trees.conformances {
        for row in &conformance.rows {
            if [row.realization_machine, row.realization_state]
                .into_iter()
                .any(|symbol| {
                    selected_machine(trees, symbol)
                        .is_some_and(|index| trees.machines[index].has_structural_type_equations)
                })
            {
                return Err(vec![Diagnostic::error(
                    "conformance realization retains an undisclosed machine structural equation",
                )]);
            }
        }
    }
    let binders = BinderClosures::build(trees);
    for (original, resolved) in &sources.expressions {
        match trees.tables.bodies.expressions.expression(*resolved) {
            ExpressionNode::Call(call) => {
                let callee = selected_machine(trees, call.target_symbol);
                reject_machine_arguments(trees, callee, &call.machine_arguments)?;
                let Some(index) = callee else {
                    continue;
                };
                if !completes_call(
                    syntax,
                    trees,
                    sources,
                    &binders,
                    index,
                    &call.machine_arguments,
                )? {
                    continue;
                }
                require_closed_selected_arguments(&binders, trees, &call.machine_arguments)?;
                let syntax_trees::expression::ExpressionNode::Call(mut authored) =
                    syntax.expressions.expression(*original).clone()
                else {
                    return Err(vec![Diagnostic::error(
                        "machine equation application lost its authored call",
                    )]);
                };
                let ordinary_types =
                    call_constraints(syntax, trees, sources, call, Some(*resolved), selection);
                complete_authored_arguments(
                    syntax,
                    index,
                    &authored.target,
                    &mut authored.machine_arguments,
                    &ordinary_types.types,
                    &ordinary_types.pending_diagnostics,
                    sources,
                    selection,
                    &mut completed_type_roots,
                )?;
                syntax.expressions.replace_expression(
                    *original,
                    syntax_trees::expression::ExpressionNode::Call(authored),
                );
            }
            ExpressionNode::Name(path)
                if selected_machine(trees, path.symbol)
                    .is_some_and(|index| trees.machines[index].has_structural_type_equations) =>
            {
                return Err(vec![Diagnostic::error(
                    "machine with structural equations requires a closed explicit call; noncall selection retains an unsolved equation",
                )]);
            }
            _ => {}
        }
    }
    for (original, resolved) in &sources.statements {
        let Statement::Call(call) = trees.tables.declarations.state_statements.get(*resolved)
        else {
            continue;
        };
        let callee = selected_machine(trees, call.target_symbol);
        reject_machine_arguments(trees, callee, &call.machine_arguments)?;
        let Some(index) = callee else {
            continue;
        };
        if !completes_call(
            syntax,
            trees,
            sources,
            &binders,
            index,
            &call.machine_arguments,
        )? {
            continue;
        }
        require_closed_selected_arguments(&binders, trees, &call.machine_arguments)?;
        let StatementNode::Call(mut authored) = syntax.statements.statement(*original).clone()
        else {
            return Err(vec![Diagnostic::error(
                "machine equation application lost its authored statement",
            )]);
        };
        let ordinary_types =
            argument_types::statement_constraints(syntax, trees, sources, call, selection);
        complete_authored_arguments(
            syntax,
            index,
            &authored.target,
            &mut authored.machine_arguments,
            &ordinary_types.types,
            &ordinary_types.pending_diagnostics,
            sources,
            selection,
            &mut completed_type_roots,
        )?;
        syntax
            .statements
            .replace_statement(*original, StatementNode::Call(authored));
    }
    let mut positions = Vec::new();
    for &root in &completed_type_roots {
        super::generic_data::collect_type_positions(syntax, root, &mut positions, false);
    }
    let mut warnings = Vec::new();
    super::generic_data::desugar_generic_data_instances_in(
        syntax,
        &mut warnings,
        Some(selection),
        super::generic_data::GenericDataWorkDomain::Selected {
            type_roots: &completed_type_roots,
            declarations: &[],
        },
    )?;
    if positions.iter().any(|reference| {
        matches!(
            syntax.type_references.type_reference(*reference),
            TypeReferenceNode::Generic { .. }
        )
    }) {
        return Err(vec![Diagnostic::error(
            "machine equation type argument retains an unnormalized data application",
        )]);
    }
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(())
}

/*
    Whether a call's static arguments go through equation completion.

    A machine whose `requires` facts define a type binder completes every
    call whose selected arguments are closed, including a partial tuple that
    still has to recover an omitted binder. A complete tuple that only
    forwards open type binders stays with the later type-predicate check:
    `fixed_vec_field<Kind, i32, 4>` is valid on the arm that already has
    `Kind == FixedVec<i32, 4>`. A complete tuple that forwards an open const
    or value binder is rejected here. Structural matching must not read that
    binder as a same-named closed declaration (`const Count` beside
    `relay<const Count>`).
*/
fn completes_call(
    syntax: &SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    binders: &BinderClosures,
    index: usize,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<bool, Vec<Diagnostic>> {
    let declares = trees.machines[index].has_structural_type_equations
        || sources
            .declarations
            .iter()
            .find(|(declaration, _)| *declaration == index)
            .is_some_and(|(_, machine)| {
                requirement_equations(syntax, machine)
                    .iter()
                    .any(type_equations::TypeEquation::requires_argument_recovery)
            });
    if !declares {
        return Ok(false);
    }
    let closure = classify_selected_arguments(binders, trees, arguments);
    if supplies_complete_tuple(trees, index, arguments) && closure == ArgumentClosure::ConstBinder {
        return Err(open_caller_binder());
    }
    Ok(closure != ArgumentClosure::TypeBinder || !supplies_complete_tuple(trees, index, arguments))
}

/*
    Every type equation among a machine's `requires` facts, closed definitions
    included; completion solves omitted arguments from all of them.
*/
fn requirement_equations(
    syntax: &SyntaxTrees,
    machine: &Machine,
) -> Vec<type_equations::TypeEquation> {
    type_equations::classify_type_equations(
        syntax,
        syntax.items.type_parameters(machine.type_parameters),
        &syntax.items.requirement_facts(machine.contracts),
    )
}

fn supplies_complete_tuple(
    trees: &SymbolResolvedTrees,
    index: usize,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> bool {
    let machine = &trees.machines[index];
    arguments.len() == machine.type_parameters.count() as usize + machine.conformance_bounds.len()
}

/*
    A machine with structural equations can be selected without a call only
    where a callee's `machine` requirement parameter receives it. That callee
    invokes it through the requirement, and each invocation is an ordinary call
    whose selection must recover the remaining binders from its actual argument
    types and then prove the equation as the callee's type predicate; a binder
    it cannot recover leaves the call incomplete and rejects. Reflection visitors
    take this route: `visit_runtime_fields<T, CompactBinary::packed_field>`
    supplies only `Kind` per field, and `Element`/`N` come from the field loan.
    Any other noncall position, including a nested application argument, would
    keep the equation unsolved and still rejects here.
*/
fn reject_machine_arguments(
    trees: &SymbolResolvedTrees,
    callee: Option<usize>,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    let parameters = callee.map(|index| trees.machine_type_parameters(&trees.machines[index]));
    for (position, argument) in arguments.iter().enumerate() {
        let fills_requirement = parameters
            .and_then(|parameters| parameters.get(position))
            .is_some_and(|parameter| {
                matches!(
                    parameter.kind,
                    symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
                )
            });
        if !fills_requirement
            && selected_machine(trees, argument.symbol)
                .is_some_and(|index| trees.machines[index].has_structural_type_equations)
        {
            return Err(vec![Diagnostic::error(
                "machine structural equation remains unsolved in a noncall static argument",
            )]);
        }
        if let Some(application) = &argument.application {
            reject_machine_arguments(trees, None, &application.arguments)?;
        }
    }
    Ok(())
}

/*
    Syntax identity must not reinterpret an exact caller binder as a same-named
    global declaration. Inspect the selected type tree before structural matching;
    this is a closedness fence, not a second equation or type inference solver.
*/
fn require_closed_selected_arguments(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    if classify_selected_arguments(binders, trees, arguments) == ArgumentClosure::Closed {
        Ok(())
    } else {
        Err(open_caller_binder())
    }
}

fn open_caller_binder() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "machine equation retains an open caller binder; supply a closed explicit static argument",
    )]
}

/*
    How open a selected static tuple is. A type binder can still be discharged
    by a later predicate or guard. A const or value binder cannot: matching
    would treat its spelling as a closed declaration of the same name.
*/
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArgumentClosure {
    Closed,
    TypeBinder,
    ConstBinder,
}

impl ArgumentClosure {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::ConstBinder, _) | (_, Self::ConstBinder) => Self::ConstBinder,
            (Self::TypeBinder, _) | (_, Self::TypeBinder) => Self::TypeBinder,
            (Self::Closed, Self::Closed) => Self::Closed,
        }
    }
}

/*
    Type-parameter closure keyed by symbol arena index. Built once per
    completion so each selected argument does not rescan the parameter arena.
*/
struct BinderClosures {
    by_symbol: Vec<Option<ArgumentClosure>>,
}

impl BinderClosures {
    fn build(trees: &SymbolResolvedTrees) -> Self {
        let mut by_symbol = vec![None; trees.symbols.symbols().len()];
        for (_, parameter) in trees.tables.declarations.data_type_parameters.iter() {
            let index = parameter.symbol.arena_index() as usize;
            if index >= by_symbol.len() {
                continue;
            }
            by_symbol[index] = Some(match &parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Type
                | symbol_resolved_trees::data::TypeParameterKind::Implementation => {
                    ArgumentClosure::TypeBinder
                }
                _ => ArgumentClosure::ConstBinder,
            });
        }
        Self { by_symbol }
    }

    fn get(&self, symbol: symbols::SymbolHandle) -> Option<ArgumentClosure> {
        self.by_symbol
            .get(symbol.arena_index() as usize)
            .copied()
            .flatten()
    }
}

fn classify_selected_arguments(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> ArgumentClosure {
    let mut closure = ArgumentClosure::Closed;
    let mut pending = Vec::new();
    for argument in arguments {
        closure = closure.join(binder_closure(binders, trees, argument.symbol));
        if let Some(application) = &argument.application {
            closure = closure.join(classify_selected_arguments(
                binders,
                trees,
                &application.arguments,
            ));
        }
        if argument.type_reference.is_valid() {
            pending.push(argument.type_reference);
        }
    }
    for handle in pending {
        closure = closure.join(classify_type_reference(
            binders,
            trees,
            trees.child_type_reference(handle),
            0,
        ));
    }
    closure
}

fn classify_type_reference(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    reference: &symbol_resolved_trees::types::TypeReference,
    depth: usize,
) -> ArgumentClosure {
    classify_scoped_type_reference(binders, trees, reference, &[], false, depth)
}

fn classify_callable_contract(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    contract: symbol_resolved_trees::callable_values::RuntimeCallableContractHandle,
    bound: &[symbols::SymbolHandle],
    depth: usize,
) -> ArgumentClosure {
    let declaration = trees
        .tables
        .declarations
        .runtime_callable_contracts
        .get(contract);
    let inputs = symbol_resolved_trees::callable_values::ResolvedSignatureInputs::of(
        trees,
        &declaration.signature,
    );
    classify_signature_inputs(binders, trees, &inputs, bound, depth)
}

fn classify_signature_inputs(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    inputs: &symbol_resolved_trees::callable_values::ResolvedSignatureInputs<'_>,
    inherited: &[symbols::SymbolHandle],
    depth: usize,
) -> ArgumentClosure {
    if inputs.has_unresolved_inputs() || depth >= 128 {
        return ArgumentClosure::ConstBinder;
    }
    let mut closure = ArgumentClosure::Closed;
    for (scope, reference) in &inputs.types {
        let mut bound = inherited.to_vec();
        bound.extend(inputs.bound_symbols(*scope));
        closure = closure.join(classify_scoped_type_reference(
            binders,
            trees,
            reference,
            &bound,
            true,
            depth + 1,
        ));
    }
    for (scope, symbol) in &inputs.symbols {
        if !inputs.is_bound(*scope, *symbol) && !inherited.contains(symbol) {
            closure = closure.join(binder_closure(binders, trees, *symbol));
        }
    }
    closure
}

fn classify_scoped_type_reference(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    reference: &symbol_resolved_trees::types::TypeReference,
    bound: &[symbols::SymbolHandle],
    in_signature: bool,
    depth: usize,
) -> ArgumentClosure {
    use symbol_resolved_trees::types::{FixedArrayLength, TypeReference};
    if depth >= 128 {
        return ArgumentClosure::ConstBinder;
    }
    let classify_symbol = |symbol| {
        if bound.contains(&symbol) {
            ArgumentClosure::Closed
        } else if in_signature && !symbol.is_valid() {
            ArgumentClosure::ConstBinder
        } else {
            binder_closure(binders, trees, symbol)
        }
    };
    let classify_child = |reference: &TypeReference| {
        classify_scoped_type_reference(binders, trees, reference, bound, in_signature, depth + 1)
    };
    match reference {
        TypeReference::AliasUse { expansion, .. } => classify_child(expansion),
        TypeReference::Named { symbol, .. } => classify_symbol(*symbol),
        TypeReference::DynamicTrait {
            symbol, arguments, ..
        } => {
            let mut closure = classify_symbol(*symbol);
            for argument in trees.child_type_references(*arguments) {
                closure = closure.join(classify_child(argument));
            }
            closure
        }
        TypeReference::Callable { contract, .. } => {
            classify_callable_contract(binders, trees, *contract, bound, depth + 1)
        }
        TypeReference::Reference(reference) => {
            classify_child(trees.child_type_reference(reference.referee))
        }
        TypeReference::Slice(slice) => {
            classify_child(trees.child_type_reference(slice.element_type))
        }
        TypeReference::FixedArray(array) => {
            let extent = match &array.length {
                FixedArrayLength::Literal(_) => ArgumentClosure::Closed,
                FixedArrayLength::ConstParameter { symbol, .. }
                    if symbol.is_valid()
                        && trees.symbols.get(*symbol).kind == symbols::SymbolKind::Const =>
                {
                    ArgumentClosure::Closed
                }
                FixedArrayLength::ConstParameter { symbol, .. } if in_signature => {
                    classify_symbol(*symbol)
                }
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    let extent = binder_closure(binders, trees, *symbol);
                    if extent == ArgumentClosure::Closed {
                        ArgumentClosure::ConstBinder
                    } else {
                        extent
                    }
                }
                FixedArrayLength::Expression(reference) if in_signature => {
                    classify_child(trees.child_type_reference(*reference))
                }
                FixedArrayLength::ConstCall { machine, .. } if in_signature => {
                    classify_symbol(*machine)
                }
                FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. } => {
                    ArgumentClosure::ConstBinder
                }
            };
            extent.join(classify_child(
                trees.child_type_reference(array.element_type),
            ))
        }
        TypeReference::Constrained(constrained) => {
            let mut closure = classify_child(trees.child_type_reference(constrained.base_type));
            for constraint in trees
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
            {
                if let TypeConstraint::Domain(domain) = constraint {
                    for reference in trees.child_type_references(domain.arguments) {
                        closure = closure.join(classify_child(reference));
                    }
                }
            }
            closure
        }
        TypeReference::Generic(generic) => {
            let mut closure = classify_symbol(generic.base_symbol);
            for reference in trees.child_type_references(generic.arguments) {
                closure = closure.join(classify_child(reference));
            }
            closure
        }
        TypeReference::ConstExpression(expression) if in_signature => {
            let inputs =
                symbol_resolved_trees::callable_values::ResolvedSignatureInputs::of_expression(
                    trees,
                    expression.expression,
                );
            classify_signature_inputs(binders, trees, &inputs, bound, depth + 1)
        }
        TypeReference::ConstExpression(_) => ArgumentClosure::ConstBinder,
        TypeReference::Unit => ArgumentClosure::Closed,
    }
}

/*
    A caller binder's closure. Closed names, including a selected `const`
    declaration, are not binders. A type or implementation parameter can be
    refined later; every other binder is a const or value the equation must
    not recover by spelling.
*/
fn binder_closure(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    symbol: symbols::SymbolHandle,
) -> ArgumentClosure {
    if !symbol.is_valid() {
        return ArgumentClosure::Closed;
    }
    match trees.symbols.get(symbol).kind {
        symbols::SymbolKind::TypeParameter => {
            binders.get(symbol).unwrap_or(ArgumentClosure::ConstBinder)
        }
        symbols::SymbolKind::Parameter
        | symbols::SymbolKind::MachineParameter
        | symbols::SymbolKind::ConformanceParameter => ArgumentClosure::ConstBinder,
        _ => ArgumentClosure::Closed,
    }
}

/*
    Complete one authored call's machine-argument list and record the type
    roots the completion minted.
*/
fn complete_authored_arguments(
    syntax: &mut SyntaxTrees,
    index: usize,
    target: &Identifier,
    machine_arguments: &mut Box<[StaticMachineArgument]>,
    ordinary_types: &[(TypeReferenceHandle, TypeReferenceHandle)],
    pending_diagnostics: &[Diagnostic],
    sources: &SourceLinks,
    selection: &ConstantSelection<'_>,
    completed_type_roots: &mut Vec<syntax_trees::types::TypeReferenceHandle>,
) -> Result<(), Vec<Diagnostic>> {
    *machine_arguments = complete_arguments(
        syntax,
        index,
        target,
        machine_arguments,
        None,
        ordinary_types,
        sources,
        selection,
    )
    .map_err(|error| {
        once(error)
            .chain(pending_diagnostics.iter().cloned())
            .collect::<Vec<_>>()
    })?
    .0;
    completed_type_roots.extend(
        machine_arguments
            .iter()
            .map(|argument| argument.type_reference)
            .filter(|reference| reference.is_valid()),
    );
    Ok(())
}

/*
    A syntax-stage atom cannot grant resolved nominal carrier authority.
    Restore its retained source operand so ordinary static-argument admission
    checks the exact declaration and carrier. The original occurrence remains
    the lexical selection scope even when a type reaches a different caller.
*/
fn inferred_const_argument(
    syntax: &mut SyntaxTrees,
    reference: TypeReferenceHandle,
    selection: &ConstantSelection<'_>,
) -> Result<StaticMachineArgument, Diagnostic> {
    let reject =
        || Diagnostic::error("inferred structural constant lost its authored operand custody");
    let normalization = syntax_type_references(syntax)
        .const_argument_normalization(reference)
        .ok_or_else(reject)?
        .clone();
    if normalization.authored_expression.is_valid() {
        let type_reference = if normalization.materialized_expression.is_valid() {
            reference
        } else {
            syntax
                .type_references
                .insert(TypeReferenceNode::ConstExpression(
                    normalization.authored_expression,
                ))
        };
        return Ok(StaticMachineArgument {
            type_reference,
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
        });
    }
    let mut origins = syntax_type_references(syntax)
        .const_argument_origins(normalization.selections)
        .iter()
        .filter(|origin| origin.reference == normalization.reference);
    let origin = origins.next().ok_or_else(reject)?;
    if origins.next().is_some() {
        return Err(reject());
    }
    let name = selection.retained_const_name(syntax, origin)?;
    Ok(StaticMachineArgument {
        type_reference: TypeReferenceHandle::invalid(),
        path: vec![name].into_boxed_slice(),
        application: None,
        const_literal: None,
        evidence_projection: None,
    })
}

/*
    Expose an authored scalar const's value to equation matching without
    replacing its source argument. The immutable static-argument checker still
    owns the selected declaration's exact carrier and policy. Constructor slots
    below have different custody: normalization erases their nested arguments,
    so they must validate those arguments before constructing the type.
*/
fn supplied_const_argument_reference(
    syntax: &mut SyntaxTrees,
    argument: &StaticMachineArgument,
    selection: &ConstantSelection<'_>,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let (value, _) = scalar_const_argument(syntax, argument, selection)?;
    if matches!(
        value.decode_encoding(),
        Some(
            DecodedCanonicalConstValue::Array { .. }
                | DecodedCanonicalConstValue::Record { .. }
                | DecodedCanonicalConstValue::Variant { .. }
        )
    ) {
        let first = argument.path.first().ok_or_else(|| {
            Diagnostic::error("structural const argument lost its source selection")
        })?;
        let name = Identifier::new(argument.path.joined_path_in(syntax), first.source_span());
        return Ok(syntax
            .type_references
            .insert(TypeReferenceNode::Named(name)));
    }
    scalar_const_argument_reference(syntax, value)
}

/*
    Select one scalar value while retaining its declaration's type separately.
    A Boolean keyword/atom has known Boolean authority and no declaration type.
    Direct machine arguments keep their original syntax for immutable checking;
    constructor normalization must consume the declared type before erasure.
*/
fn scalar_const_argument(
    syntax: &SyntaxTrees,
    argument: &StaticMachineArgument,
    selection: &ConstantSelection<'_>,
) -> Result<(CanonicalConstValue, TypeReferenceHandle), Diagnostic> {
    let reject = |reason: &str| Diagnostic::error(format!("machine equation application {reason}"));
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
    {
        return Err(reject("requires a closed scalar const argument"));
    }
    let Some(first) = argument.path.first() else {
        return Err(reject("requires a selected scalar const argument"));
    };
    let name = Identifier::new(argument.path.joined_path_in(syntax), first.source_span());
    if let Some(value) = type_equations::normalized_boolean_argument(name.as_str(syntax)) {
        return Ok((
            CanonicalConstValue::boolean(value),
            TypeReferenceHandle::default(),
        ));
    }
    let definition = selection.select(syntax, &name)?.ok_or_else(|| {
        reject("requires an unambiguous selected scalar const argument")
            .with_source_span(name.source_span())
    })?;
    let value =
        canonicalize_selected_declared_const_definition(syntax, &definition, Some(selection))
            .map_err(|reason| reject(&reason).with_source_span(name.source_span()))?;
    Ok((value, definition.type_reference))
}

fn scalar_const_argument_reference(
    syntax: &mut SyntaxTrees,
    value: CanonicalConstValue,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let spelling = match value.decode_encoding() {
        Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
        Some(DecodedCanonicalConstValue::Boolean(value)) => {
            CanonicalConstValue::boolean(value).atom()
        }
        _ => {
            return Err(Diagnostic::error(
                "machine equation application requires an integer or Boolean const argument",
            ));
        }
    };
    Ok(syntax
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated(spelling))))
}

/*
    The selected type slot determines how a static application is read.
    Reuse the declaration's telescope rather than treating every static path as
    a type, and retain each occurrence's source context for normal selection.
*/
fn application_argument_reference(
    syntax: &mut SyntaxTrees,
    argument: &StaticMachineArgument,
    kind: &TypeParameterKind,
    selection: &ConstantSelection<'_>,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reject = |reason: &str| Diagnostic::error(format!("machine equation application {reason}"));
    if argument.evidence_projection.is_some() {
        return Err(reject("cannot use evidence as a type or integer argument"));
    }
    match kind {
        TypeParameterKind::Const { type_reference } => {
            let Some(carrier) = syntax_type_references(syntax)
                .type_reference(*type_reference)
                .as_named()
            else {
                return Err(reject("requires a builtin integer constructor parameter"));
            };
            let required_carrier =
                super::generic_data::closed_name_identity(syntax, Some(selection), carrier);
            let boolean = required_carrier
                == Some(super::generic_data::ClosedArgumentIdentity::Builtin(
                    symbols::BuiltinTypeAtom::Bool,
                ));
            if argument.const_literal.is_none() {
                let (value, declared_type) = scalar_const_argument(syntax, argument, selection)?;
                if declared_type.is_valid()
                    && closed_argument_identity(syntax, Some(selection), declared_type, false)
                        != required_carrier
                {
                    return Err(reject(
                        "has a constructor argument with a different declared carrier",
                    ));
                }
                match value.decode_encoding() {
                    Some(DecodedCanonicalConstValue::Boolean(_)) if boolean => {}
                    Some(DecodedCanonicalConstValue::Integer { .. })
                        if !boolean && const_binder_envelope(carrier.as_str(syntax)).is_some() => {}
                    Some(
                        DecodedCanonicalConstValue::Array { .. }
                        | DecodedCanonicalConstValue::Record { .. }
                        | DecodedCanonicalConstValue::Variant { .. },
                    ) if declared_type.is_valid() && required_carrier.is_some() => {
                        let first = argument.path.first().ok_or_else(|| {
                            reject("lost its structural constructor argument selection")
                        })?;
                        let name = Identifier::new(
                            argument.path.joined_path_in(syntax),
                            first.source_span(),
                        );
                        return Ok(syntax
                            .type_references
                            .insert(TypeReferenceNode::Named(name)));
                    }
                    _ => {
                        return Err(reject(
                            "has a constructor argument with a different scalar kind",
                        ));
                    }
                }
                return scalar_const_argument_reference(syntax, value);
            }
            if boolean {
                return Err(reject("requires a closed Boolean constructor argument"));
            }
            let literal = argument
                .const_literal
                .as_ref()
                .ok_or_else(|| reject("requires a closed integer constructor argument"))?;
            if literal.landing().is_some_and(|landing| {
                landing.landed_type.name() != carrier.as_str(syntax)
                    || landing.domain != ArithmeticDomain::Exact
            }) {
                return Err(reject(
                    "has a constructor argument with a different integer meaning",
                ));
            }
            let (minimum, maximum) = const_binder_envelope(carrier.as_str(syntax))
                .ok_or_else(|| reject("requires a builtin integer constructor parameter"))?;
            let value = literal
                .value_bignum()
                .ok_or_else(|| reject("requires an exact integer"))?;
            if value < minimum || value > maximum {
                return Err(reject(
                    "has a constructor constant outside its declared carrier",
                ));
            }
            Ok(syntax
                .type_references
                .insert(TypeReferenceNode::Named(Identifier::generated(
                    value.to_string(),
                ))))
        }
        TypeParameterKind::Type => {
            if argument.const_literal.is_some() {
                return Err(reject("mixes type and value kinds"));
            }
            if argument.type_reference.is_valid() {
                return Ok(argument.type_reference);
            }
            let Some(first) = argument.path.first() else {
                return Err(reject("requires a selected type name"));
            };
            let name = Identifier::new(argument.path.joined_path_in(syntax), first.source_span());
            let Some(application) = &argument.application else {
                return Ok(syntax
                    .type_references
                    .insert(TypeReferenceNode::Named(name)));
            };
            let definition = selection.data(syntax, &name).map_err(Diagnostic::error)?;
            if !application.lifetime_arguments.is_empty()
                || !definition.lifetime_parameters.is_empty()
            {
                return Err(reject("requires lifetime-free constructor arguments"));
            }
            let parameters = syntax_items(syntax)
                .type_parameters(definition.type_parameters)
                .to_vec();
            if parameters.len() != application.arguments.len() {
                return Err(reject("requires a complete constructor argument tuple"));
            }
            let arguments = application
                .arguments
                .iter()
                .zip(parameters)
                .map(|(argument, parameter)| {
                    application_argument_reference(syntax, argument, &parameter.kind, selection)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = syntax
                .type_references
                .insert_type_reference_handles(arguments);
            Ok(syntax.type_references.insert(TypeReferenceNode::Generic {
                base_name: name,
                lifetime_arguments: Default::default(),
                arguments,
            }))
        }
        _ => Err(reject(
            "requires type and integer const constructor parameters",
        )),
    }
}
