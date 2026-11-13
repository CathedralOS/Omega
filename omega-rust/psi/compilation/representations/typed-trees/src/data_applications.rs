//! Applied data keeps its authored members and the actual substituted types.
//! This is source metadata formation, never constructor or argument admission.

use crate::TypedTrees;
use crate::data::{DataMember, TypeParameterKind};
use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};

impl TypedTrees {
    /// Retain views for the new source references before constructor landing.
    /// Views do not publish declarations or establish their generic premises.
    pub fn prepare_data_application_views(&mut self, first: usize) -> Result<(), String> {
        let references: Vec<_> = self
            .type_reference_table
            .type_reference_rows_from(first)
            .filter_map(|(reference, node)| match node {
                TypeReferenceNode::Generic { base_symbol, .. }
                    if self
                        .unique_data_definition_by_symbol(*base_symbol)
                        .is_some() =>
                {
                    Some(reference)
                }
                _ => None,
            })
            .collect();
        let mut preparation = ApplicationPreparation::new(self);
        for reference in references {
            preparation.reference(reference)?;
        }
        Ok(())
    }

    /// Form one already-resolved application, including its argument views.
    /// `true` means its types are closed; callers still owe every source
    /// requirement and must judge the actual value or call they construct.
    pub fn prepare_data_application_view(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<bool, String> {
        ApplicationPreparation::new(self).reference(reference)
    }
}

struct ApplicationPreparation<'source> {
    source: &'source mut TypedTrees,
    closed: Vec<Option<bool>>,
    active_references: Vec<TypeReferenceHandle>,
    active_declarations: Vec<SymbolHandle>,
}

impl<'source> ApplicationPreparation<'source> {
    fn new(source: &'source mut TypedTrees) -> Self {
        Self {
            closed: vec![None; source.type_reference_table.type_reference_count()],
            source,
            active_references: Vec::new(),
            active_declarations: Vec::new(),
        }
    }

    fn reference(&mut self, reference: TypeReferenceHandle) -> Result<bool, String> {
        if !self
            .source
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("data application refers to a missing type reference".into());
        }
        let position = reference.arena_index() as usize;
        self.closed.resize(
            self.source.type_reference_table.type_reference_count(),
            None,
        );
        if let Some(closed) = self.closed[position] {
            return Ok(closed);
        }
        if self.active_references.contains(&reference) {
            return Err("data application has a cyclic type-reference graph".into());
        }
        self.active_references.push(reference);
        let result = self.form(reference);
        self.active_references.pop();
        if let Ok(closed) = result {
            self.closed[position] = Some(closed);
        }
        result
    }

    fn form(&mut self, reference: TypeReferenceHandle) -> Result<bool, String> {
        match self
            .source
            .type_reference_table
            .type_reference(reference)
            .clone()
        {
            TypeReferenceNode::Named { symbol, .. } => {
                if !self.source.symbols.symbols().nodes().is_valid(symbol) {
                    return Err("data application has an unresolved type argument".into());
                }
                if self.source.symbols.builtin_type_atom(symbol).is_some() {
                    return Ok(true);
                }
                Ok(self
                    .source
                    .unique_data_definition_by_symbol(symbol)
                    .is_some_and(|data| data.type_parameters.is_empty()))
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => self.application(
                reference,
                base_symbol,
                self.source
                    .type_reference_table
                    .type_reference_handles(arguments)
                    .to_vec(),
                lifetime_arguments.len(),
            ),
            TypeReferenceNode::Reference { referee, .. } => self.reference(referee),
            TypeReferenceNode::Slice { element_type } => self.reference(element_type),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => Ok(matches!(length, FixedArrayLength::Literal(_)) && self.reference(element_type)?),
            TypeReferenceNode::Constrained { base_type, .. } => self.reference(base_type),
            TypeReferenceNode::Unit => Ok(true),
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => {
                Ok(false)
            }
        }
    }

    fn application(
        &mut self,
        reference: TypeReferenceHandle,
        owner: SymbolHandle,
        arguments: Vec<TypeReferenceHandle>,
        lifetimes: usize,
    ) -> Result<bool, String> {
        let Some(template) = self.source.unique_data_definition_by_symbol(owner).cloned() else {
            // Applications of machines and other namespaces retain their own
            // source records; this operation does not reinterpret them as data.
            return Ok(false);
        };
        if !self.source.symbols.symbols().nodes().is_valid(owner)
            || self.source.symbols.get(owner).kind != SymbolKind::Data
        {
            return Err("data application owner has a non-data symbol".into());
        }
        let parameters = self.source.data_type_parameters(&template).to_vec();
        if lifetimes != template.lifetime_parameters.len() || parameters.len() != arguments.len() {
            return Err("data application does not retain its complete argument tuple".into());
        }
        if parameters.is_empty() {
            return Ok(true);
        }
        if lifetimes != 0
            || parameters
                .iter()
                .any(|parameter| !matches!(parameter.kind, TypeParameterKind::Type))
        {
            return Ok(false);
        }
        let mut substitutions = Vec::with_capacity(parameters.len());
        let mut closed = true;
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !self
                .source
                .symbols
                .symbols()
                .nodes()
                .is_valid(parameter.symbol)
                || self.source.symbols.get(parameter.symbol).kind != SymbolKind::TypeParameter
                || self.source.symbols.get(parameter.symbol).parent != owner
                || substitutions
                    .iter()
                    .any(|(known, _)| *known == parameter.symbol)
            {
                return Err(
                    "data application has a foreign or duplicated declaration binder".into(),
                );
            }
            closed &= self.reference(argument)?;
            substitutions.push((parameter.symbol, argument));
        }
        let mut definition = template.clone();
        definition.type_parameters = HandleSpan::empty();
        definition.generic_instance = Some(reference);
        definition.predicate_subject_type = self
            .source
            .type_reference_table
            .instantiate(template.predicate_subject_type, &substitutions)
            .map_err(str::to_owned)?;
        let mut members = self.source.data_members(&template).to_vec();
        let mut children = Vec::new();
        for member in &mut members {
            match member {
                DataMember::Field(field) => {
                    field.type_reference = self
                        .source
                        .type_reference_table
                        .instantiate(field.type_reference, &substitutions)
                        .map_err(str::to_owned)?;
                    children.push(field.type_reference);
                }
                DataMember::Variant(variant) => {
                    let mut fields = self.source.data_payload_fields(variant).to_vec();
                    for field in &mut fields {
                        field.type_reference = self
                            .source
                            .type_reference_table
                            .instantiate(field.type_reference, &substitutions)
                            .map_err(str::to_owned)?;
                        children.push(field.type_reference);
                    }
                    variant.payload = self.source.tables.data_payload_fields.insert_many(fields);
                }
            }
        }
        definition.members = self.source.tables.data_members.insert_many(members);
        // Keep the authored member symbols, requirements, storage properties
        // and source origins. Only the type telescope is applied here.
        closed &= children
            .iter()
            .all(|child| crate::domain::index_argument_is_closed(self.source, *child));
        self.source
            .retain_data_application_definition(reference, definition, closed);
        if self.active_declarations.contains(&owner) {
            // Repeated declarations through their member definitions may
            // describe recursive storage. Retain this exact shallow view;
            // do not expand an unbounded family or grant it a finite layout.
            // Actual type arguments were prepared before entering the owner,
            // so finite nested applications still retain all their views.
            return Ok(closed);
        }
        self.active_declarations.push(owner);
        let prepared = (|| {
            for child in children {
                self.reference(child)?;
            }
            Ok(closed)
        })();
        self.active_declarations.pop();
        prepared
    }
}
