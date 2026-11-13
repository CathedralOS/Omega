//! Enumerate owned linear claims from declaration metadata. A reference or
//! code-only callable does not own the claims of its parameter or referee.

use crate::{
    TypedTrees,
    data::{DataDefinition, DataMember},
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimPathSegment {
    Field { symbol: SymbolHandle },
    Case { variant: SymbolHandle },
    FixedIndex { index: usize },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearClaim {
    pub path: Vec<ClaimPathSegment>,
    pub type_reference: TypeReferenceHandle,
    pub conditional: bool,
}

pub fn linear_claim_frontier(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<Vec<LinearClaim>> {
    let mut output = Vec::new();
    Query {
        program,
        active: Vec::new(),
    }
    .append(reference, &[], &mut Vec::new(), &mut output, false)?;
    Ok(output)
}
pub fn type_carries_linear_obligation(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<bool> {
    let mut output = Vec::new();
    Query {
        program,
        active: Vec::new(),
    }
    .append(reference, &[], &mut Vec::new(), &mut output, true)?;
    Ok(!output.is_empty())
}

struct Query<'a> {
    program: &'a TypedTrees,
    active: Vec<(SymbolHandle, Vec<(SymbolHandle, TypeReferenceHandle)>)>,
}
impl Query<'_> {
    fn append(
        &mut self,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        path: &mut Vec<ClaimPathSegment>,
        output: &mut Vec<LinearClaim>,
        first_only: bool,
    ) -> Result<()> {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("linear claim query has a stale or missing type coordinate");
        }
        if let Some(storage) = self.program.layout_storage_arguments(reference) {
            return self.append(
                storage.semantic_type,
                substitutions,
                path,
                output,
                first_only,
            );
        }
        if self.program.primitive_type_reference(reference).is_some() {
            return Ok(());
        }
        match self
            .program
            .type_reference_table
            .type_reference(reference)
            .clone()
        {
            TypeReferenceNode::Callable { contract } => {
                if !self.program.runtime_callable_contracts.is_valid(contract) {
                    return Err("linear claim query has a stale callable contract");
                }
                Ok(())
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => Ok(()),
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.append(base_type, substitutions, path, output, first_only)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                let length = self.program.closed_fixed_array_length(reference);
                if length == Some(0) {
                    return Ok(());
                }
                let mut element = Vec::new();
                self.append(
                    element_type,
                    substitutions,
                    &mut Vec::new(),
                    &mut element,
                    first_only,
                )?;
                if element.is_empty() {
                    return Ok(());
                }
                let length = length.ok_or(
                    "linear claim query needs the exact array extent of a claim-bearing element",
                )?;
                let count = if first_only { 1 } else { length };
                let prefix_conditional = path
                    .iter()
                    .any(|segment| matches!(segment, ClaimPathSegment::Case { .. }));
                for index in 0..count {
                    for claim in &element {
                        let mut claim_path = Vec::with_capacity(path.len() + 1 + claim.path.len());
                        claim_path.extend_from_slice(path);
                        claim_path.push(ClaimPathSegment::FixedIndex { index });
                        claim_path.extend_from_slice(&claim.path);
                        output.push(LinearClaim {
                            path: claim_path,
                            type_reference: claim.type_reference,
                            conditional: prefix_conditional || claim.conditional,
                        });
                        if first_only {
                            return Ok(());
                        }
                    }
                }
                Ok(())
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some((actual, enclosing)) = replacement(substitutions, symbol, reference) {
                    return self.append(actual, enclosing, path, output, first_only);
                }
                if let Some(parameter) = self.program.first_data_type_parameter_by_symbol(symbol) {
                    if parameter.bounds.multiplicity == Multiplicity::Linear {
                        push_claim(output, path, reference);
                    }
                    return Ok(());
                }
                let data = self
                    .program
                    .unique_data_definition_by_symbol(symbol)
                    .ok_or("linear claim query has no unique exact data declaration")?;
                self.append_data(reference, data, substitutions, path, output, first_only)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                if let Some(data) = self.program.closed_data_definition(reference) {
                    return self.append_data(
                        reference,
                        data,
                        substitutions,
                        path,
                        output,
                        first_only,
                    );
                }
                let data = self
                    .program
                    .unique_data_definition_by_symbol(base_symbol)
                    .ok_or("linear claim query has no exact generic data declaration")?;
                let parameters = self.program.data_type_parameters(data);
                let arguments = self
                    .program
                    .type_reference_table
                    .type_reference_handles(arguments);
                if parameters.len() != arguments.len() {
                    return Err("linear claim query has an incomplete generic argument telescope");
                }
                let mut applied = substitutions.to_vec();
                applied.extend(parameters.iter().zip(arguments).filter_map(
                    |(parameter, actual)| {
                        matches!(parameter.kind, crate::data::TypeParameterKind::Type)
                            .then_some((parameter.symbol, *actual))
                    },
                ));
                self.append_data(reference, data, &applied, path, output, first_only)
            }
        }
    }
    fn append_data(
        &mut self,
        reference: TypeReferenceHandle,
        data: &DataDefinition,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        path: &mut Vec<ClaimPathSegment>,
        output: &mut Vec<LinearClaim>,
        first_only: bool,
    ) -> Result<()> {
        if data.properties.multiplicity == Multiplicity::Linear {
            push_claim(output, path, reference);
            return Ok(());
        }
        let mut environment = Vec::new();
        for (position, (formal, actual)) in substitutions.iter().enumerate().rev() {
            if environment.iter().any(|(retained, _)| retained == formal) {
                continue;
            }
            let mut actual = *actual;
            let mut enclosing = &substitutions[..position];
            if !self
                .program
                .type_reference_table
                .contains_type_reference(actual)
            {
                return Err("linear claim query has a stale generic actual");
            }
            while let TypeReferenceNode::Named { symbol, .. } =
                self.program.type_reference_table.type_reference(actual)
            {
                let Some((next, outer)) = replacement(enclosing, *symbol, actual) else {
                    break;
                };
                actual = next;
                enclosing = outer;
                if !self
                    .program
                    .type_reference_table
                    .contains_type_reference(actual)
                {
                    return Err("linear claim query has a stale enclosing generic actual");
                }
            }
            environment.push((*formal, actual));
        }
        environment.sort_unstable_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
        if self
            .active
            .iter()
            .any(|(symbol, scope)| *symbol == data.symbol && *scope == environment)
        {
            return Err("linear claim query encountered recursive owned storage");
        }
        self.active.push((data.symbol, environment));
        let result = (|| {
            for member in self.program.data_members(data) {
                match member {
                    DataMember::Field(field) => {
                        path.push(ClaimPathSegment::Field {
                            symbol: field.symbol,
                        });
                        self.append(
                            field.type_reference,
                            substitutions,
                            path,
                            output,
                            first_only,
                        )?;
                        path.pop();
                    }
                    DataMember::Variant(variant) => {
                        path.push(ClaimPathSegment::Case {
                            variant: variant.symbol,
                        });
                        for field in self.program.data_payload_fields(variant) {
                            path.push(ClaimPathSegment::Field {
                                symbol: field.symbol,
                            });
                            self.append(
                                field.type_reference,
                                substitutions,
                                path,
                                output,
                                first_only,
                            )?;
                            path.pop();
                            if first_only && !output.is_empty() {
                                break;
                            }
                        }
                        path.pop();
                    }
                }
                if first_only && !output.is_empty() {
                    break;
                }
            }
            Ok(())
        })();
        self.active.pop();
        result
    }
}
fn replacement(
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    symbol: SymbolHandle,
    occurrence: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, &[(SymbolHandle, TypeReferenceHandle)])> {
    substitutions
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, (formal, actual))| {
            (*formal == symbol && *actual != occurrence)
                .then_some((*actual, &substitutions[..position]))
        })
}
fn push_claim(
    output: &mut Vec<LinearClaim>,
    path: &[ClaimPathSegment],
    reference: TypeReferenceHandle,
) {
    output.push(LinearClaim {
        path: path.to_vec(),
        type_reference: reference,
        conditional: path
            .iter()
            .any(|segment| matches!(segment, ClaimPathSegment::Case { .. })),
    });
}
