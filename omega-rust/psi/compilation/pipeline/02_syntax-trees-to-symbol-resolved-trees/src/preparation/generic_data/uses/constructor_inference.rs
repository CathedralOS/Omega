//! Infer a constructor application from its selected fields' ordinary types.
//!
//! Literal octets provide fixed-array geometry, never an erased byte carrier.
//! Nested constructors supply their complete nominal application before an
//! enclosing constructor solves its own telescope. This pass chooses arguments;
//! ordinary construction checking still owns field presence, types, domains and
//! value fit. Undetermined user binders remain undetermined. Only the exact
//! compiler-owned locator vocabulary has its prescribed unused-coordinate zero
//! constraints; a same-named declaration gains no such default.

use super::super::{GenericData, Instantiation};
use super::{
    collect_expression_handles, collect_statement_expression_handles, concrete_state_local_types,
    relabel_data_literal_for_expected_type, selected_constructor,
};
use crate::preparation::declaration_selection::ConstantSelection;
use crate::preparation::type_equations::{
    EquationTemplate, complete_equation_arguments_with_types, type_reference_parameter_mentions,
};
use diagnostics::Diagnostic;
use std::collections::HashMap;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::{Identifier, JoinedSyntaxPath};
use syntax_trees::item::{DataMember, Item, ItemHandle};
use syntax_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(in crate::preparation::generic_data) struct InferredConstructor {
    expression: ExpressionHandle,
    pub(in crate::preparation::generic_data) reference: TypeReferenceHandle,
}

impl InferredConstructor {
    pub(in crate::preparation::generic_data) fn relabel(
        &self,
        syntax: &mut SyntaxTrees,
        instances: &[Instantiation],
        selection: Option<&ConstantSelection<'_>>,
    ) {
        relabel_data_literal_for_expected_type(
            syntax,
            self.expression,
            self.reference,
            instances,
            selection,
            &super::ConstructorFrontier {
                parameters: Default::default(),
                prior_statements: &[],
            },
        );
    }
}

pub(in crate::preparation::generic_data) fn infer_constructor_types(
    syntax: &mut SyntaxTrees,
    templates: &HashMap<ItemHandle, GenericData>,
    const_values: &HashMap<String, i128>,
    selection: Option<&ConstantSelection<'_>>,
    warnings: &mut Vec<Diagnostic>,
    declarations: &[ItemHandle],
) -> Vec<InferredConstructor> {
    let mut inferred = Vec::new();
    let mut known = Vec::new();
    for (_, locals, statements) in concrete_state_local_types(syntax, declarations) {
        let mut expressions = super::expressions::ExpressionInventory::default();
        for statement in statements {
            collect_statement_expression_handles(syntax, statement, &mut expressions);
        }
        for expression in expressions.into_handles() {
            if !matches!(
                syntax.expressions.expression(expression),
                ExpressionNode::StructLiteral(_)
            ) {
                continue;
            }
            observe_type(
                syntax,
                templates,
                const_values,
                selection,
                warnings,
                &locals,
                expression,
                None,
                &mut known,
                &mut inferred,
                &mut Vec::new(),
            );
        }
    }
    for &declaration in declarations {
        let Item::Const(constant) = syntax.root_item(declaration) else {
            continue;
        };
        let mut expressions = super::expressions::ExpressionInventory::default();
        collect_expression_handles(syntax, constant.value, &mut expressions);
        for expression in expressions.into_handles() {
            if !matches!(
                syntax.expressions.expression(expression),
                ExpressionNode::StructLiteral(_)
            ) {
                continue;
            }
            observe_type(
                syntax,
                templates,
                const_values,
                selection,
                warnings,
                &HashMap::new(),
                expression,
                None,
                &mut known,
                &mut inferred,
                &mut Vec::new(),
            );
        }
    }
    inferred
}

#[allow(clippy::too_many_arguments)]
fn observe_type(
    syntax: &mut SyntaxTrees,
    templates: &HashMap<ItemHandle, GenericData>,
    const_values: &HashMap<String, i128>,
    selection: Option<&ConstantSelection<'_>>,
    warnings: &mut Vec<Diagnostic>,
    locals: &HashMap<String, TypeReferenceHandle>,
    expression: ExpressionHandle,
    expected: Option<TypeReferenceHandle>,
    known: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
    inferred: &mut Vec<InferredConstructor>,
    active: &mut Vec<ExpressionHandle>,
) -> Option<TypeReferenceHandle> {
    if let Some((_, reference)) = known.iter().find(|(known, _)| *known == expression) {
        return Some(*reference);
    }
    if !expression.is_valid() || active.len() >= 64 || active.contains(&expression) {
        return None;
    }
    active.push(expression);
    let observed = (|| match syntax.expressions.expression(expression).clone() {
        ExpressionNode::String(bytes) => {
            let element = syntax
                .type_references
                .insert_named(Identifier::generated("u8"));
            Some(
                syntax
                    .type_references
                    .insert_fixed_array(element, FixedArrayLength::Literal(bytes.len())),
            )
        }
        ExpressionNode::Name(path) => {
            let members = syntax.expressions.identifier_path_members(path);
            if let [name] = members {
                if let Some(reference) = locals.get(name.as_str(syntax)) {
                    return Some(*reference);
                }
            }
            let first = members.first()?;
            let last = members.last()?;
            let name = Identifier::new(
                members.joined_path_in(syntax),
                source::SourceSpan::new(
                    first.source_span().source_id,
                    source::Span::new(first.source_span().span.start, last.source_span().span.end),
                ),
            );
            let declaration = selection?.select(syntax, &name).ok()??;
            declaration
                .type_reference
                .is_valid()
                .then_some(declaration.type_reference)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let elements = syntax.expressions.expression_handles(elements).to_vec();
            let expected_element = expected.and_then(|reference| {
                match syntax.type_references.type_reference(reference) {
                    TypeReferenceNode::FixedArray { element_type, .. } => Some(*element_type),
                    _ => None,
                }
            });
            let element = elements
                .first()
                .and_then(|first| {
                    observe_type(
                        syntax,
                        templates,
                        const_values,
                        selection,
                        warnings,
                        locals,
                        *first,
                        expected_element,
                        known,
                        inferred,
                        active,
                    )
                })
                .or(expected_element)?;
            Some(
                syntax
                    .type_references
                    .insert_fixed_array(element, FixedArrayLength::Literal(elements.len())),
            )
        }
        ExpressionNode::StructLiteral(literal) => {
            let (owner, case) = selected_constructor(syntax, selection, &literal.constructor_name)?;
            let definition = syntax.root_item(owner).as_data()?.clone();
            let mut fields = syntax
                .items
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => {
                        Some((field.name.as_str(syntax).to_owned(), field.type_reference))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if let Some(case) = &case {
                let variant = syntax
                    .items
                    .data_members(definition.members)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.name.as_str(syntax) == case.as_str(syntax) =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })?;
                fields.extend(
                    syntax
                        .items
                        .data_payload_fields(variant.payload)
                        .iter()
                        .map(|field| (field.name.as_str(syntax).to_owned(), field.type_reference)),
                );
            }
            let mut observations = Vec::new();
            for field in syntax.expressions.struct_fields(literal.fields).to_vec() {
                let (_, formal) = fields
                    .iter()
                    .find(|(name, _)| name == field.name.as_str(syntax))?;
                if let Some(actual) = observe_type(
                    syntax,
                    templates,
                    const_values,
                    selection,
                    warnings,
                    locals,
                    field.value,
                    Some(*formal),
                    known,
                    inferred,
                    active,
                ) {
                    observations.push((*formal, actual));
                }
            }
            let carrier = if case.is_some() {
                literal
                    .constructor_name
                    .as_str(syntax)
                    .rsplit_once("::")?
                    .0
                    .to_owned()
            } else {
                literal.constructor_name.as_str(syntax).to_owned()
            };
            let base_name = Identifier::new(carrier, literal.constructor_name.source_span());
            if definition.type_parameters.is_empty() {
                return Some(syntax.type_references.insert_named(base_name));
            }
            let template = templates.get(&owner)?;
            if !template.lifetime_parameters.is_empty() {
                return None;
            }
            if locator_unused_defaults(syntax, selection, template, case.as_ref()) {
                for (index, parameter) in template.parameter_names.iter().enumerate() {
                    if fields.iter().any(|(_, reference)| {
                        type_reference_parameter_mentions(
                            syntax,
                            *reference,
                            &template.parameter_names,
                        )
                        .contains(&index)
                    }) {
                        continue;
                    }
                    let element = syntax
                        .type_references
                        .insert_named(Identifier::generated("u8"));
                    let formal = syntax.type_references.insert_fixed_array(
                        element,
                        FixedArrayLength::ConstParameter(Identifier::new(
                            parameter,
                            definition.name.source_span(),
                        )),
                    );
                    let actual = syntax
                        .type_references
                        .insert_fixed_array(element, FixedArrayLength::Literal(0));
                    observations.push((formal, actual));
                }
            }
            let arguments = complete_equation_arguments_with_types(
                syntax,
                EquationTemplate {
                    kind: "constructor",
                    name: &template.name,
                    parameters: definition.type_parameters,
                    invocation_lifetimes: &[],
                    parameter_names: &template.parameter_names,
                    const_parameter_types: &template.const_parameter_types,
                    type_equations: &template.type_equations,
                },
                &base_name,
                &[],
                &observations,
                const_values,
                selection,
                warnings,
            )
            .ok()?;
            let arguments = syntax
                .type_references
                .insert_type_reference_handles(arguments);
            let reference = syntax.type_references.insert_generic(base_name, arguments);
            inferred.push(InferredConstructor {
                expression,
                reference,
            });
            Some(reference)
        }
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Integer(_) => expected,
        _ => None,
    })();
    active.pop();
    // Only a selected constructor has an intrinsic nominal application here.
    // An anonymous literal/array may have borrowed its field's expected type;
    // that contextual observation must not become a global expression type.
    if let Some(reference) = observed
        && matches!(
            syntax.expressions.expression(expression),
            ExpressionNode::StructLiteral(_)
        )
    {
        known.push((expression, reference));
    }
    observed
}

fn locator_unused_defaults(
    syntax: &SyntaxTrees,
    selection: Option<&ConstantSelection<'_>>,
    template: &GenericData,
    case: Option<&Identifier>,
) -> bool {
    let Some(selection) = selection else {
        return false;
    };
    let Some(case) = case else {
        return false;
    };
    if !selection.is_native_locator_source(template.origin_name.source_span())
        || template.parameter_names != ["ObjectLength", "SymbolLength", "VersionLength"]
        || !template.const_parameter_types.iter().all(|reference| {
            reference.is_some_and(|reference| {
                match syntax.type_references.type_reference(reference) {
                    TypeReferenceNode::Named(name) => {
                        selection.builtin_type(syntax, name) == Some(symbols::BuiltinTypeAtom::U64)
                    }
                    _ => false,
                }
            })
        })
    {
        return false;
    }
    match template.name.as_str() {
        "ForeignBinding" => matches!(case.as_str(syntax), "DllImport" | "Syscall"),
        "DllImport" => matches!(
            case.as_str(syntax),
            "PeByName" | "PeByOrdinal" | "ElfVersioned" | "MachODylibSymbol"
        ),
        _ => false,
    }
}
