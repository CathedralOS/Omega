//! Synthesis reads the full declaration context but mutates only owned work.
//!
//! Equation completion introduces exact type roots after earlier declarations
//! have been published. Those declarations remain lookup context, not another
//! normalization worklist. New instances and their attached machines extend the
//! same selected domain so nested applications and constructor uses settle by
//! the ordinary fixed point without changing an unrelated retained declaration.
//! Authored applications are a finite root roster. A synthesized field adds
//! inline storage edges; a reference target remains identity unless declaration
//! specialization owns unfinished work there. That work includes transitive
//! const obligations, case facts, constrained fields and attached bodies. The
//! same synthesizer discharges it; skipping a layout must not skip admission.
//! Typed elaboration owns the later field views of identity-only applications.

use super::GenericData;
use super::selected_generic_data;
use std::collections::HashMap;
use syntax_trees::item::{DataMember, DataProperties, TypeParameterKind};
use syntax_trees::types::{FixedArrayLength, TypeReferenceNode};
use syntax_trees::{SyntaxTrees, item::ItemHandle, types::TypeReferenceHandle};

/// Compute declaration-owned work on the finite template graph, before closed
/// application expansion. Indirection removes a storage demand, not a const or
/// fact obligation: `Outer<T> -> &Middle<T> -> &Constrained<T>` must retain the
/// last declaration's checks. Conversely, a fact-free recursive reference such
/// as `Chain<T> -> &Chain<[T; 2]>` needs identity, not endlessly copied records.
pub(super) fn required_declaration_specializations(
    syntax: &SyntaxTrees,
    templates: &HashMap<ItemHandle, GenericData>,
    attached_machines: &HashMap<ItemHandle, Vec<usize>>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
) -> Vec<ItemHandle> {
    let mut required = Vec::new();
    let mut dependencies = Vec::new();
    for (&declaration, template) in templates {
        let Some(definition) = syntax.root_item(declaration).as_data() else {
            continue;
        };
        let parameters = syntax.items.type_parameters(definition.type_parameters);
        let mut needs_specialization = !template.where_facts.is_empty()
            || parameters.iter().any(|parameter| {
                !matches!(parameter.kind, TypeParameterKind::Type)
                    || parameter.bounds != DataProperties::default()
            })
            || attached_machines.contains_key(&declaration);
        let mut positions = Vec::new();
        for member in syntax.items.data_members(template.members) {
            match member {
                DataMember::Field(field) => super::collect_type_positions(
                    syntax,
                    field.type_reference,
                    &mut positions,
                    true,
                ),
                DataMember::Variant(variant) => {
                    needs_specialization |= !variant.where_facts.is_empty();
                    for field in syntax.items.data_payload_fields(variant.payload) {
                        super::collect_type_positions(
                            syntax,
                            field.type_reference,
                            &mut positions,
                            true,
                        );
                    }
                }
                DataMember::Retired(_) => {}
            }
        }
        let mut referenced = Vec::new();
        for position in positions {
            match syntax.type_references.type_reference(position) {
                TypeReferenceNode::Constrained { .. } | TypeReferenceNode::ConstExpression(_) => {
                    needs_specialization = true
                }
                TypeReferenceNode::FixedArray { length, .. }
                    if !matches!(length, FixedArrayLength::Literal(_)) =>
                {
                    needs_specialization = true;
                }
                TypeReferenceNode::Generic { base_name, .. } => {
                    let head = base_name
                        .as_str()
                        .split_once("::")
                        .map_or(base_name.as_str(), |(head, _)| head);
                    if template.parameter_names.iter().any(|name| name == head) {
                        continue;
                    }
                    if let Some(dependency) =
                        selected_generic_data(syntax, templates, selection, base_name)
                    {
                        referenced.push(dependency.declaration);
                    }
                }
                _ => {}
            }
        }
        if needs_specialization {
            required.push(declaration);
        }
        dependencies.push((declaration, referenced));
    }
    loop {
        let previous_count = required.len();
        for (declaration, referenced) in &dependencies {
            if !required.contains(declaration)
                && referenced
                    .iter()
                    .any(|dependency| required.contains(dependency))
            {
                required.push(*declaration);
            }
        }
        if required.len() == previous_count {
            return required;
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum GenericDataWorkDomain<'input> {
    All {
        additional_type_roots: &'input [TypeReferenceHandle],
    },
    Selected {
        type_roots: &'input [TypeReferenceHandle],
        declarations: &'input [ItemHandle],
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TypePositionPurpose {
    Identity,
    Materialization,
}

pub(super) struct SynthesisWork<'input> {
    domain: GenericDataWorkDomain<'input>,
    declaration_frontier: usize,
    expression_frontier: syntax_trees::expression::ExpressionFrontier,
}

impl<'input> SynthesisWork<'input> {
    pub(super) fn new(syntax: &SyntaxTrees, domain: GenericDataWorkDomain<'input>) -> Self {
        Self {
            domain,
            declaration_frontier: syntax.root_item_handles().len(),
            expression_frontier: syntax.expressions.frontier(),
        }
    }

    pub(super) fn is_all(&self) -> bool {
        matches!(self.domain, GenericDataWorkDomain::All { .. })
    }

    /// Synthesis also copies data facts and other declaration-side expressions,
    /// not just machine bodies. Their new arena suffix belongs to this work
    /// even when no executable machine inventory reaches them.
    pub(super) fn owns_new_expression(
        &self,
        expression: syntax_trees::expression::ExpressionHandle,
    ) -> bool {
        expression.is_valid() && self.expression_frontier.contains(expression)
    }

    pub(super) fn declarations(&self, syntax: &SyntaxTrees) -> Vec<ItemHandle> {
        match self.domain {
            GenericDataWorkDomain::All { .. } => syntax.root_item_handles().to_vec(),
            GenericDataWorkDomain::Selected { declarations, .. } => {
                let mut selected = declarations.to_vec();
                for &declaration in &syntax.root_item_handles()[self.declaration_frontier..] {
                    if !selected.contains(&declaration) {
                        selected.push(declaration);
                    }
                }
                selected
            }
        }
    }

    pub(super) fn type_positions(
        &self,
        syntax: &SyntaxTrees,
        purpose: TypePositionPurpose,
        include_domain_arguments: bool,
    ) -> Vec<TypeReferenceHandle> {
        let declarations = if purpose == TypePositionPurpose::Identity {
            self.declarations(syntax)
        } else {
            match self.domain {
                GenericDataWorkDomain::All { .. } => {
                    syntax.root_item_handles()[..self.declaration_frontier].to_vec()
                }
                GenericDataWorkDomain::Selected { declarations, .. } => declarations.to_vec(),
            }
        };
        let mut positions = super::const_evaluation::collect_selected_type_reference_positions(
            syntax,
            &declarations,
        );
        if purpose == TypePositionPurpose::Materialization {
            positions.extend(
                super::const_evaluation::collect_stored_type_reference_positions(
                    syntax,
                    &syntax.root_item_handles()[self.declaration_frontier..],
                ),
            );
        }
        let roots = match self.domain {
            GenericDataWorkDomain::All {
                additional_type_roots,
            } => additional_type_roots,
            GenericDataWorkDomain::Selected { type_roots, .. } => type_roots,
        };
        for &root in roots {
            super::collect_type_positions(syntax, root, &mut positions, include_domain_arguments);
        }
        positions
    }
}
