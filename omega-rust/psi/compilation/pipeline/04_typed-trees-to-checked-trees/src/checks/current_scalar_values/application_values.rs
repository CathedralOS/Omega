//! Read a dependent application recipe under completed caller operands. A
//! structural operand supplies captured projections, never a fresh observation
//! of a same-named callee parameter or the caller's later storage version.

use super::{CurrentScalarValues, field_storage_path};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};

impl CurrentScalarValues<'_> {
    pub(crate) fn application_value_with_captured(
        &self,
        expression: ExpressionHandle,
        arguments: &[(SymbolHandle, Option<ExpressionHandle>)],
        captured: &[(SymbolHandle, Option<ScopedArithmeticValue>)],
    ) -> Option<ScopedArithmeticExpression> {
        if arguments.is_empty() && captured.is_empty() {
            return self.current(expression);
        }
        let mut bindings = Vec::new();
        let mut pending = vec![expression];
        let mut remaining = 4096usize;
        let mut owner = arguments
            .first()
            .map_or(self.state.symbol, |(parameter, _)| {
                self.program.symbols.get(*parameter).parent
            });
        while let Some(occurrence) = pending.pop() {
            remaining = remaining.checked_sub(1)?;
            match self.program.expression_table.expression(occurrence) {
                ExpressionNode::Name(path) => {
                    let value = if let Some((_, value)) =
                        captured.iter().find(|(formal, _)| *formal == path.symbol)
                    {
                        owner = self.program.symbols.get(path.symbol).parent;
                        value.clone()?
                    } else if let Some((_, actual)) =
                        arguments.iter().find(|(formal, _)| *formal == path.symbol)
                    {
                        owner = self.program.symbols.get(path.symbol).parent;
                        ScopedArithmeticValue::Term(self.current((*actual)?)?)
                    } else {
                        self.current_symbol(path.symbol)?
                    };
                    bindings.push(ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Symbol(path.symbol),
                        value,
                    });
                }
                ExpressionNode::Member(member) => {
                    let (root, mut path) = field_storage_path(self.program, member.receiver)?;
                    path.push(member.member_symbol);
                    let Some((_, actual)) = arguments.iter().find(|(formal, _)| *formal == root)
                    else {
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(occurrence),
                            value: ScopedArithmeticValue::Term(self.current(occurrence)?),
                        });
                        continue;
                    };
                    let actual = (*actual)?;
                    owner = self.program.symbols.get(root).parent;
                    let receiver = validation::parameter_expression_result_type_reference(
                        self.program,
                        owner,
                        &[],
                        member.receiver,
                    )?;
                    let value = if validation::collection_operations::collection_measure_member(
                        self.program,
                        member,
                        validation::collection_operations::MeasureReceiver::Declared(receiver),
                    ) == Some(
                        language_semantics::declaration_selection::CollectionMeasure::Length,
                    ) {
                        let (_, receiver_path) = field_storage_path(self.program, member.receiver)?;
                        if receiver_path.is_empty() {
                            self.collection_length(actual, 0)?
                        } else {
                            self.captured_field_lengths(actual, 0)
                                .into_iter()
                                .find(|field| field.path == receiver_path)?
                                .value
                        }
                    } else {
                        self.captured_scalar_fields(actual, 0)
                            .into_iter()
                            .find(|field| {
                                field.path
                                    == path
                                        .iter()
                                        .map(|symbol| super::ScalarProjection::Field(*symbol))
                                        .collect::<Vec<_>>()
                            })?
                            .value
                    };
                    bindings.push(ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Occurrence(occurrence),
                        value,
                    });
                }
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
                ExpressionNode::Binary(binary) => {
                    if !validation::has_builtin_parameter_bound_expression_meaning(
                        self.program,
                        owner,
                        &[],
                        occurrence,
                    ) {
                        return None;
                    }
                    pending.push(binary.left);
                    pending.push(binary.right);
                }
                _ => return None,
            }
        }
        Some(ScopedArithmeticExpression {
            owner,
            expression,
            bindings,
        })
    }
}
