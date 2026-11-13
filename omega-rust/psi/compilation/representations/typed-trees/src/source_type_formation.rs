//! Source type formation retains exact declarations and operand scopes.
//! These queries do not grant executable facts, ownership, or call effects.

use crate::data::{DataMember, TypeParameter, TypeParameterKind};
use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashSet;

pub fn validate_typed_const_index_type(
    program: &crate::TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<symbols::SymbolHandle>,
) -> Result<(), String> {
    use crate::types::PrimitiveType;
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(primitive) = program.primitive_type_reference(type_reference) {
                return match primitive {
                    PrimitiveType::Bool | PrimitiveType::I8 | PrimitiveType::I16
                    | PrimitiveType::I32 | PrimitiveType::I64 | PrimitiveType::U8
                    | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
                    | PrimitiveType::Addr => Ok(()),
                    _ => Err(format!(
                        "`{name}` does not have canonical structural index identity"
                    )),
                };
            }
            let definition = program.unique_data_definition_by_symbol(*symbol).or_else(|| {
                (!program.symbols.has_source_metadata())
                    .then(|| program.data_definition_by_name_str(name.as_str()))
                    .flatten()
            }).ok_or_else(|| format!("`{name}` is not declared data"))?;
            if !visiting.insert(definition.symbol) {
                return Ok(());
            }
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!("boundary-opaque data `{name}` is not structural"));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` has no compiler-verified canonical representative"
                ));
            }
            if !definition.requires_facts.is_empty() {
                return Err(format!(
                    "data `{name}` has default-domain facts whose index-site proof is not implemented"
                ));
            }
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => validate_typed_const_index_type(
                        program, field.type_reference, visiting,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in program.data_payload_fields(variant) {
                            validate_typed_const_index_type(program, field.type_reference, visiting)?;
                        }
                    }
                }
            }
            visiting.remove(&definition.symbol);
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type, length: FixedArrayLength::Literal(_),
        } => validate_typed_const_index_type(program, *element_type, visiting),
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_typed_const_index_type(program, *base_type, visiting)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Callable { .. } | TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. } | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. } => Err(
            "indices require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
    }
}

#[derive(Debug)]
pub struct BoundaryValuePlacement {
    pub declaration: symbols::SymbolHandle,
    pub name: crate::name::Identifier,
    pub generic_depth: usize,
}

/// Boundary declarations are call namespaces, not local value carriers. Follow
/// exact applications, including callable signatures, without interpreting names.
pub fn boundary_value_placements(
    program: &crate::TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<Vec<BoundaryValuePlacement>, &'static str> {
    let mut placements = Vec::new();
    let mut pending = vec![(reference, 0)];
    let mut seen = HashSet::new();
    while let Some((reference, depth)) = pending.pop() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("value placement names no current type reference");
        }
        if !seen.insert(reference) {
            continue;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => pending.push((*referee, depth)),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push((*element_type, depth)),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                if program
                    .traits()
                    .iter()
                    .any(|row| row.symbol == *base_symbol && row.is_boundary)
                {
                    placements.push(BoundaryValuePlacement {
                        declaration: *base_symbol,
                        name: base_name.clone(),
                        generic_depth: depth,
                    });
                } else {
                    pending.extend(
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments)
                            .iter()
                            .map(|argument| (*argument, depth + 1)),
                    );
                }
            }
            TypeReferenceNode::Named { symbol, name } => {
                if program
                    .traits()
                    .iter()
                    .any(|row| row.symbol == *symbol && row.is_boundary)
                {
                    placements.push(BoundaryValuePlacement {
                        declaration: *symbol,
                        name: name.clone(),
                        generic_depth: depth,
                    });
                } else if let Some(origin) = program
                    .unique_data_definition_by_symbol(*symbol)
                    .and_then(|row| row.generic_instance)
                {
                    pending.push((origin, depth + 1));
                }
            }
            TypeReferenceNode::Callable { contract } => {
                if !program.runtime_callable_contracts.is_valid(*contract) {
                    return Err("value placement names no current callable contract");
                }
                let signature = &program.runtime_callable_contract(*contract).signature;
                pending.extend(
                    program
                        .state_signature_parameters(signature)
                        .iter()
                        .map(|parameter| (parameter.type_reference, depth)),
                );
                if signature.return_type.is_valid() {
                    pending.push((signature.return_type, depth));
                }
            }
            TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => {}
        }
    }
    Ok(placements)
}

#[derive(Debug, Clone, Copy)]
pub struct TypeParameterScope<'program> {
    pub operand_lifetime_owner: symbols::SymbolHandle,
    pub lexical_owner: symbols::SymbolHandle,
    pub type_parameters: &'program [TypeParameter],
    pub lifetime_parameters: &'program [crate::name::Identifier],
}

impl<'program> TypeParameterScope<'program> {
    pub fn authored(
        lexical_owner: symbols::SymbolHandle,
        type_parameters: &'program [TypeParameter],
        lifetime_parameters: &'program [crate::name::Identifier],
    ) -> Self {
        Self {
            operand_lifetime_owner: symbols::SymbolHandle::invalid(),
            lexical_owner,
            type_parameters,
            lifetime_parameters,
        }
    }

    pub fn contains(self, name: &str) -> bool {
        self.type_parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == name)
    }

    pub fn contains_operand_lifetime(
        self,
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        slot: usize,
        name: &str,
    ) -> bool {
        let retained = program.type_reference_table.operand_lifetimes(reference);
        if !retained.is_empty() {
            let expected = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Generic {
                    lifetime_arguments, ..
                } => lifetime_arguments.len(),
                TypeReferenceNode::Reference {
                    lifetime: Some(_), ..
                } => 1,
                _ => 0,
            };
            if retained.len() != expected {
                return false;
            }
            if let Some(region) = retained
                .get(slot)
                .filter(|region| region.declaration.is_valid())
            {
                return crate::type_identity::lifetime_parameter_name(program, *region)
                    .is_some_and(|parameter| parameter.as_str() == name);
            }
        }
        self.contains_lifetime(program, name)
    }

    pub fn contains_lifetime(self, program: &crate::TypedTrees, name: &str) -> bool {
        let supplying_owner = self.operand_lifetime_owner;
        if supplying_owner.is_valid() {
            return crate::type_identity::lifetime_parameter_identity(
                program,
                supplying_owner,
                name,
            )
            .is_some();
        }
        self.lifetime_parameters
            .iter()
            .any(|parameter| parameter.as_str() == name)
    }

    pub fn machine_parameter(
        self,
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> Option<&'program TypeParameter> {
        self.type_parameters.iter().find(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Machine { .. })
                && ((symbol.is_valid() && parameter.symbol == symbol)
                    || parameter.name.as_str() == name)
        })
    }
}

/// Type preparation owns traversal and the signature's binder scope. The caller
/// supplies the existing declaration, conformance, range and domain obligations;
/// this interface grants no body summary or executable proof.
pub trait SourceTypeFormation<Owner: Copy + std::fmt::Display> {
    fn diagnostics(&mut self) -> &mut Vec<diagnostics::Diagnostic>;
    fn aliases(
        &mut self,
        reference: TypeReferenceHandle,
        owner: Owner,
        scope: TypeParameterScope<'_>,
    );
    fn constraints(
        &mut self,
        base: TypeReferenceHandle,
        constraints: arena::HandleSpan<crate::types::TypeConstraintNode>,
        owner: Owner,
        scope: TypeParameterScope<'_>,
    );
    fn array_length(
        &mut self,
        length: &FixedArrayLength,
        owner: Owner,
        scope: TypeParameterScope<'_>,
    );
    fn nominal(
        &mut self,
        reference: TypeReferenceHandle,
        owner: Owner,
        scope: TypeParameterScope<'_>,
    );
}

pub fn validate_type_node<Owner: Copy + std::fmt::Display>(
    program: &crate::TypedTrees,
    formation: &mut impl SourceTypeFormation<Owner>,
    reference: TypeReferenceHandle,
    owner: Owner,
    mut scope: TypeParameterScope<'_>,
) {
    use diagnostics::Diagnostic;
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        formation.diagnostics().push(Diagnostic::error(format!(
            "{owner} names no current type reference"
        )));
        return;
    }
    let supplying_owner = program
        .type_reference_table
        .operand_lifetime_owner(reference);
    if supplying_owner.is_valid() {
        scope.operand_lifetime_owner = supplying_owner;
    }
    formation.aliases(reference, owner, scope);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, lifetime, .. } => {
            if let Some(lifetime) = lifetime
                && !scope.contains_operand_lifetime(program, reference, 0, lifetime.as_str()) {
                formation.diagnostics().push(Diagnostic::error(format!(
                    "{owner} uses undeclared lifetime `'{}'; declare it in the owner's generic parameter list", lifetime.as_str()
                )));
            }
            validate_type_node(program, formation, *referee, owner, scope);
        }
        TypeReferenceNode::Constrained { base_type, constraints } => {
            validate_type_node(program, formation, *base_type, owner, scope);
            formation.constraints(*base_type, *constraints, owner, scope);
        }
        TypeReferenceNode::FixedArray { element_type, length } => {
            formation.array_length(length, owner, scope);
            validate_type_node(program, formation, *element_type, owner, scope);
        }
        TypeReferenceNode::Slice { element_type } => validate_type_node(program, formation, *element_type, owner, scope),
        TypeReferenceNode::Generic { .. } | TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Named { .. } => formation.nominal(reference, owner, scope),
        TypeReferenceNode::Callable { contract } => {
            // Normalize the complete published contract, including free outer
            // declarations. Parameter names alone never establish its identity.
            if let Err(reason) = program.runtime_callable_type_identity(crate::type_identity::TypeIdentityRequest::ordinary(reference)) {
                formation.diagnostics().push(Diagnostic::error(format!("{owner}: {reason}")));
                return;
            }
            let signature = &program.runtime_callable_contract(*contract).signature;
            let mut parameters = program.state_signature_type_parameters(signature).to_vec();
            parameters.extend_from_slice(scope.type_parameters);
            let mut lifetimes = signature.lifetime_parameters.clone();
            lifetimes.extend_from_slice(scope.lifetime_parameters);
            let callable_scope = TypeParameterScope::authored(signature.symbol, &parameters, &lifetimes);
            for parameter in program.state_signature_parameters(signature) {
                validate_type_node(program, formation, parameter.type_reference, owner, callable_scope);
            }
            if signature.return_type.is_valid() {
                validate_type_node(program, formation, signature.return_type, owner, callable_scope);
            }
        }
        TypeReferenceNode::ConstExpression(expression) => formation.diagnostics().push(Diagnostic::error(format!(
            "{owner} uses proof-static index expression `{}` as a runtime type; computed const expressions are valid only inside an indexed-domain argument pack",
            program.expression_table.display_name(*expression)
        ))),
        TypeReferenceNode::Unit => {}
    }
}
