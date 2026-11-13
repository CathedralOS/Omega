//! Scoped source expression inputs and declaration result queries. These rows
//! describe authored typing; they do not provide executable proof evidence.

use crate::{
    TypedTrees,
    callable_values::RuntimeCallableContractHandle,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, TableMemberExpression},
    types::TypeReferenceHandle,
};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferredType {
    TypeReference(TypeReferenceHandle),
    Nominal(SymbolHandle),
    CollectionView(TypeReferenceHandle),
    CompilerString,
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub name: String,
    pub symbol: SymbolHandle,
    pub type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, Default)]
pub struct TypeEnvironment {
    pub owner: SymbolHandle,
    pub runtime_contract: Option<RuntimeCallableContractHandle>,
    pub trait_scope: Option<crate::trait_definition::RetainedTraitScope>,
    pub self_type: Option<InferredType>,
    pub result_type: Option<TypeReferenceHandle>,
    pub bindings: Vec<Binding>,
}

/// A callable signature begins a different formal namespace. Its inputs are
/// queried separately under the exact retained row, never under an outer field
/// or parameter's telescope.
#[derive(Default)]
pub struct ScopedTypeInputs {
    pub types: Vec<TypeReferenceHandle>,
    pub expressions: Vec<ExpressionHandle>,
    pub callables: Vec<RuntimeCallableContractHandle>,
}

struct Inputs<'program> {
    program: &'program TypedTrees,
    inputs: ScopedTypeInputs,
    error: Option<&'static str>,
}

impl Extend<SymbolHandle> for Inputs<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}

impl crate::references::TypedReferences for Inputs<'_> {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            self.error = Some("source type expression has a stale type coordinate");
            return false;
        }
        if self.inputs.types.contains(&reference) {
            return false;
        }
        self.inputs.types.push(reference);
        true
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            self.error = Some("source type expression has a stale operand coordinate");
            return false;
        }
        if self.inputs.expressions.contains(&expression) {
            return false;
        }
        self.inputs.expressions.push(expression);
        true
    }
    fn callable_contract(&mut self, contract: RuntimeCallableContractHandle) -> bool {
        if let Err(error) = self
            .program
            .runtime_callable_contract_identity(contract, &[])
        {
            self.error = Some(error.reason());
            return false;
        }
        if !self.inputs.callables.contains(&contract) {
            self.inputs.callables.push(contract);
        }
        false
    }
}

impl<'program> Inputs<'program> {
    fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            inputs: ScopedTypeInputs::default(),
            error: None,
        }
    }
    fn finish(self) -> Result<ScopedTypeInputs, &'static str> {
        match self.error {
            Some(reason) => Err(reason),
            None => Ok(self.inputs),
        }
    }
}

pub fn type_inputs(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<ScopedTypeInputs, &'static str> {
    let mut inputs = Inputs::new(program);
    crate::references::visit_type_reference(program, &mut inputs, reference);
    inputs.finish()
}

pub fn callable_inputs(
    program: &TypedTrees,
    contract: RuntimeCallableContractHandle,
) -> Result<ScopedTypeInputs, &'static str> {
    program
        .runtime_callable_contract_identity(contract, &[])
        .map_err(|error| error.reason())?;
    let mut inputs = Inputs::new(program);
    crate::references::visit_signature(
        program,
        &mut inputs,
        &program.runtime_callable_contract(contract).signature,
    );
    inputs.finish()
}

pub trait ExpressionTypes {
    fn typed(&self) -> &TypedTrees;
    fn declaration_type(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle>;
    fn member_type(
        &self,
        receiver: InferredType,
        member: &TableMemberExpression,
    ) -> Option<TypeReferenceHandle>;
    fn collection_measure_type(
        &self,
        receiver: InferredType,
        member: &TableMemberExpression,
    ) -> Option<TypeReferenceHandle>;
    fn computation_type(
        &self,
        environment: &TypeEnvironment,
        expression: ExpressionHandle,
        leaf_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> Option<TypeReferenceHandle>;
}

pub fn infer_expression_type(
    context: &impl ExpressionTypes,
    expression: ExpressionHandle,
    environment: &TypeEnvironment,
    visited: &mut Vec<ExpressionHandle>,
) -> Option<InferredType> {
    let program = context.typed();
    if !program.expression_table.expression_is_valid(expression) || visited.contains(&expression) {
        return None;
    }
    // Closed runtime calls and constructions retain their own exact contract.
    // A failed source coordinate cannot fall back to nominal inference.
    match program.declared_callable_expression_type_reference(expression) {
        Ok(Some(reference)) => return Some(InferredType::TypeReference(reference)),
        Ok(None) => {}
        Err(_) => return None,
    }
    visited.push(expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::MachineValue(_) => None,
        ExpressionNode::SavedSubject(saved) => program
            .saved_value_source(saved)
            .and_then(|subject| infer_expression_type(context, subject, environment, visited)),
        ExpressionNode::Match(dispatch) => {
            let mut result = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let candidate =
                    infer_expression_type(context, arm.value, environment, &mut visited.clone())?;
                if result.is_some_and(|previous| previous != candidate) {
                    return None;
                }
                result = Some(candidate);
            }
            result
        }
        ExpressionNode::Atomic(atomic) => {
            infer_expression_type(context, atomic.value, environment, visited)
        }
        ExpressionNode::Borrow(inner) => {
            infer_expression_type(context, inner.target, environment, visited)
        }
        ExpressionNode::Cast(cast) => Some(InferredType::TypeReference(cast.target_type)),
        ExpressionNode::Name(path) => {
            if path.symbol.is_valid()
                && let Some(reference) =
                    crate::source_operator_types::named_value_type_reference(program, path)
                        .or_else(|| context.declaration_type(path.symbol))
            {
                return Some(InferredType::TypeReference(reference));
            }
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if !path.symbol.is_valid() && !path.head_symbol.is_valid() && name.is_self_receiver() {
                return environment.self_type;
            }
            if program.expression_table.names_reserved_result(expression) {
                return environment.result_type.map(InferredType::TypeReference);
            }
            let mut matches = environment
                .bindings
                .iter()
                .filter(|binding| {
                    (path.head_symbol.is_valid() && binding.symbol == path.head_symbol)
                        || (path.symbol.is_valid() && binding.symbol == path.symbol)
                        || (!path.head_symbol.is_valid()
                            && !path.symbol.is_valid()
                            && binding.name == name.as_str())
                })
                .map(|binding| binding.type_reference);
            let first = matches.next()?;
            matches
                .all(|reference| reference == first)
                .then_some(InferredType::TypeReference(first))
        }
        ExpressionNode::Member(member) => {
            let receiver = infer_expression_type(context, member.receiver, environment, visited)?;
            context
                .member_type(receiver, member)
                .or_else(|| context.collection_measure_type(receiver, member))
                .map(InferredType::TypeReference)
        }
        ExpressionNode::Call(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_) => context
            .computation_type(
                environment,
                expression,
                &|leaf| match infer_expression_type(
                    context,
                    leaf,
                    environment,
                    &mut visited.clone(),
                )? {
                    InferredType::TypeReference(reference) => Some(reference),
                    _ => None,
                },
            )
            .map(InferredType::TypeReference),
        ExpressionNode::Indexed(indexed) => {
            let collection =
                infer_expression_type(context, indexed.collection, environment, visited)?;
            let element = collection_element_type(program, collection)?;
            Some(
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    InferredType::CollectionView(element)
                } else {
                    InferredType::TypeReference(element)
                },
            )
        }
        ExpressionNode::StructLiteral(literal) => {
            if literal.application_type.is_valid() {
                Some(InferredType::TypeReference(literal.application_type))
            } else {
                literal
                    .type_symbol
                    .is_valid()
                    .then_some(InferredType::Nominal(literal.type_symbol))
            }
        }
        ExpressionNode::String(_) => Some(InferredType::CompilerString),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

pub fn collection_element_type(
    program: &TypedTrees,
    inferred: InferredType,
) -> Option<TypeReferenceHandle> {
    let mut reference = match inferred {
        InferredType::TypeReference(reference) => reference,
        InferredType::CollectionView(element) => return Some(element),
        InferredType::Nominal(_) | InferredType::CompilerString => return None,
    };
    let mut visited = Vec::new();
    loop {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || visited.contains(&reference)
        {
            return None;
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            crate::types::TypeReferenceNode::Reference { referee, .. }
            | crate::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            crate::types::TypeReferenceNode::FixedArray { element_type, .. }
            | crate::types::TypeReferenceNode::Slice { element_type } => {
                return Some(*element_type);
            }
            _ => return None,
        }
    }
}

/// Signature expression scopes retain both the lexical declaration and the
/// exact anonymous row; copied rows may legitimately reuse formal symbols.
pub fn callable_environments(
    program: &TypedTrees,
) -> Result<Vec<(Vec<ExpressionHandle>, TypeEnvironment)>, &'static str> {
    let mut environments = Vec::new();
    for (contract, row) in program.runtime_callable_contracts.iter() {
        let signature = &row.signature;
        let parameters = program.state_signature_parameters(signature);
        let mut bindings = program
            .data_type_parameters
            .span_or_empty(signature.type_parameters)
            .iter()
            .filter_map(|parameter| match parameter.kind {
                crate::data::TypeParameterKind::Const { type_reference }
                | crate::data::TypeParameterKind::Value { type_reference } => Some(Binding {
                    name: parameter.name.to_string(),
                    symbol: parameter.symbol,
                    type_reference,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        let environment = |bindings: Vec<Binding>, result_type| TypeEnvironment {
            owner: row.lexical_owner,
            runtime_contract: Some(contract),
            result_type,
            bindings,
            ..Default::default()
        };
        for parameter in parameters {
            let inputs = type_inputs(program, parameter.type_reference)?;
            if !inputs.expressions.is_empty() {
                environments.push((inputs.expressions, environment(bindings.clone(), None)));
            }
            bindings.push(Binding {
                name: parameter.name.to_string(),
                symbol: parameter.symbol,
                type_reference: parameter.type_reference,
            });
        }
        let inputs = callable_inputs(program, contract)?;
        if !inputs.expressions.is_empty() {
            environments.push((
                inputs.expressions,
                environment(
                    bindings,
                    signature
                        .return_type
                        .is_valid()
                        .then_some(signature.return_type),
                ),
            ));
        }
    }
    Ok(environments)
}

pub fn is_boolean_fact_expression(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::SavedSubject(_) | ExpressionNode::Match(_) => false,
        ExpressionNode::Atomic(atomic) => is_boolean_fact_expression(program, atomic.value),
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::CaseMembership => true,
            BinaryOperator::And
            | BinaryOperator::Equal
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::NotEqual
            | BinaryOperator::Or => true,
            BinaryOperator::Add
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Multiply
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::Subtract => false,
        },
        ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_) => true,
        ExpressionNode::Range(_) => false,
        ExpressionNode::Borrow(inner) => is_boolean_fact_expression(program, inner.target),
        ExpressionNode::Unary(unary) => is_boolean_fact_expression(program, unary.operand),
        ExpressionNode::MachineValue(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        callable_values::RuntimeCallableContract,
        domain::ProofFact,
        name::Identifier,
        signature::{SignatureContract, SignatureContractKind, StateParameter, StateSignature},
        types::TypeReferenceNode,
    };
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn anonymous_signature_inputs_keep_their_exact_formal_scope() {
        let mut symbols = SymbolTableBuilder::default();
        let owner = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("scope"));
        let signature = symbols
            .insert_children(
                owner,
                [(
                    SymbolKind::CallableContract,
                    SymbolNameRef::Static("contract"),
                )],
            )
            .handle_at(0);
        let formal = symbols
            .insert_children(
                signature,
                [(SymbolKind::Parameter, SymbolNameRef::Static("value"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let fact = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let facts = program
            .proof_facts
            .insert_many([ProofFact::Expression(fact)]);
        let parameters = program.state_parameters.insert_many([StateParameter {
            symbol: formal,
            name: Identifier::generated("value"),
            type_reference: unit,
            ..Default::default()
        }]);
        let contracts = program.signature_contracts.insert_many([SignatureContract {
            kind: SignatureContractKind::Requires,
            facts,
            ..Default::default()
        }]);
        let contract = program
            .runtime_callable_contracts
            .insert(RuntimeCallableContract {
                signature: StateSignature {
                    symbol: signature,
                    parameters,
                    contracts,
                    return_type: unit,
                    ..Default::default()
                },
                lexical_owner: owner,
                resolved_source: None,
            });
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Callable { contract });
        let outside = type_inputs(&program, reference).unwrap();
        assert_eq!(outside.callables, vec![contract]);
        assert!(!outside.expressions.contains(&fact));
        let environments = callable_environments(&program).unwrap();
        let (_, environment) = environments
            .iter()
            .find(|(roots, _)| roots.contains(&fact))
            .unwrap();
        assert_eq!(environment.runtime_contract, Some(contract));
        assert_eq!(environment.owner, owner);
        assert_eq!(environment.bindings.len(), 1);
        assert_eq!(environment.bindings[0].symbol, formal);
        let stale = RuntimeCallableContractHandle::from_parts(
            contract.arena_index(),
            contract.generation() + 1,
        );
        let broken = program
            .type_reference_table
            .insert(TypeReferenceNode::Callable { contract: stale });
        assert!(type_inputs(&program, broken).is_err());
    }
}
