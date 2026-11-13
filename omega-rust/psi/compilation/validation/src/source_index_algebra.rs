//! Source algebra descriptions use declaration scopes and the existing typed Id view.
use crate::CheckingInput;
use symbols::SymbolHandle;
use typed_trees::{
    expression::{BinaryOperator, ExpressionHandle},
    signature::{StateParameter, StateSignature},
    source_index_algebra::{DeclaredIdentity, IndexAlgebraLaws, LawSource},
    types::{PrimitiveType, TypeReferenceHandle},
};

pub fn builtin_identity_operand_references(
    program: &dyn CheckingInput,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<[TypeReferenceHandle; 2]> {
    let binary = program.expression_table.binary_node(expression)?;
    let spelling = match binary.operator {
        BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
        BinaryOperator::NotEqual => language_core::OperatorSpelling::NotEqual,
        _ => return None,
    };
    let types = [binary.left, binary.right].map(|operand| {
        crate::parameter_expression_result_type_reference(program, owner, parameters, operand)
    });
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program, owner, expression, spelling, &types,
    ) {
        return None;
    }
    let references = crate::value_custody::expression_types::builtin_scalar_operand_references(
        program, binary, types, None,
    )?;
    if references.iter().any(|reference| {
        crate::reference_shell(program, *reference).is_some()
            || !program
                .primitive_type_reference(*reference)
                .is_some_and(|primitive| {
                    primitive.is_fixed_integer()
                        || matches!(primitive, PrimitiveType::Bool | PrimitiveType::Addr)
                })
    }) || program.normalized_type_identity(references[0])
        != program.normalized_type_identity(references[1])
    {
        return None;
    }
    Some(references)
}

struct Source<'p>(&'p dyn CheckingInput);
impl LawSource for Source<'_> {
    fn identity(
        &self,
        law: &StateSignature,
        expression: ExpressionHandle,
    ) -> Result<Option<DeclaredIdentity>, &'static str> {
        match crate::mathematical_applications::identity_application_at(self.0, expression) {
            Ok(identity) => {
                return Ok(Some(DeclaredIdentity {
                    carrier: identity.carrier,
                    subjects: identity.subjects,
                }));
            }
            Err(reason)
                if self
                    .0
                    .expression_table
                    .call_node(expression)
                    .is_some_and(|call| {
                        crate::mathematical_applications::is_identity_call(self.0, call)
                    }) =>
            {
                return Err(reason);
            }
            Err(_) => {}
        }
        let Some(binary) = self.0.expression_table.binary_node(expression) else {
            return Ok(None);
        };
        if binary.operator != BinaryOperator::Equal {
            return Ok(None);
        }
        Ok(builtin_identity_operand_references(
            self.0,
            law.symbol,
            self.0.state_signature_parameters(law),
            expression,
        )
        .map(|references| DeclaredIdentity {
            carrier: references[0],
            subjects: [binary.left, binary.right],
        }))
    }
    fn conjunction(&self, law: &StateSignature, expression: ExpressionHandle) -> bool {
        self.0
            .expression_table
            .binary_node(expression)
            .is_some_and(|binary| binary.operator == BinaryOperator::And)
            && crate::has_builtin_parameter_bound_expression_meaning(
                self.0,
                law.symbol,
                self.0.state_signature_parameters(law),
                expression,
            )
    }
    fn expression_type(
        &self,
        law: &StateSignature,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        crate::parameter_expression_result_type_reference(
            self.0,
            law.symbol,
            self.0.state_signature_parameters(law),
            expression,
        )
    }
    fn integer(&self, expression: ExpressionHandle) -> Option<numerics::bignum::BigInt> {
        crate::value_custody::literals::anonymous_numeric_value(
            self.0,
            expression,
            &mut |expression| {
                crate::value_custody::literals::has_anonymous_operator_meaning(self.0, expression)
            },
        )?
        .value
        .to_integer_exact()
    }
}

pub fn bound_laws(
    program: &dyn CheckingInput,
    owner: SymbolHandle,
    bound: &typed_trees::machine::GenericConformanceBound,
    operation: SymbolHandle,
    operand_type: TypeReferenceHandle,
) -> Result<IndexAlgebraLaws, diagnostics::Diagnostic> {
    typed_trees::source_index_algebra::bound_laws(
        program.typed_program(),
        owner,
        bound,
        operation,
        operand_type,
        &Source(program),
    )
    .map_err(|reason| {
        diagnostics::Diagnostic::error(format!(
            "open index algebra has no complete source evidence: {reason}"
        ))
    })
}
