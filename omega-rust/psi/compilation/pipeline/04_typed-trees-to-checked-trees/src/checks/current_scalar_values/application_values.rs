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
    /// Owned records and readable borrowed records expose typed entry fields.
    /// A borrowed root requires its own stable reference binding; it does not
    /// freeze the referent. These current rows still expire on overlapping
    /// writes, while a selected entry value is saved independently by ranking.
    /// Only the formal's authenticated boundary is crossed here: reference
    /// leaves inside the record need separate load evidence and are not walked.
    pub(super) fn seed_parameter_fields(&mut self) {
        for parameter in self.program.state_parameters(self.state) {
            if parameter.is_const || parameter.relevance.is_erased() {
                continue;
            }
            let reference = if let Some(shell) =
                validation::reference_shell(self.program, parameter.type_reference)
            {
                if !shell.access.is_readable()
                    || !validation::state_reference_parameter_binding_is_stable(
                        self.program,
                        self.machine,
                        self.state,
                        parameter.symbol,
                    )
                {
                    continue;
                }
                shell.referee
            } else {
                parameter.type_reference
            };
            let mut pending = vec![(reference, Vec::new(), 0usize)];
            while let Some((reference, path, depth)) = pending.pop() {
                if depth >= 64 {
                    continue;
                }
                let declared = self
                    .program
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                // Atomic/placed carriers require their observation operation;
                // equal scalar representation supplies no ordinary field read.
                if self
                    .program
                    .placed_view_plan_for_type_reference(declared)
                    .is_some()
                    || self
                        .program
                        .placed_view_field_plan_for_type_reference(declared)
                        .is_some()
                    || matches!(self.program.type_reference_table.type_reference(declared),
                        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                            if self.program.symbols.builtin_type_atom(*symbol)
                                .is_some_and(|atom| atom.is_atomic()))
                {
                    continue;
                }
                let identity = format!(
                    "state:{:?}:parameter:{:?}:field:{path:?}",
                    self.state.symbol, parameter.symbol
                );
                if let Some(value) =
                    super::fixed_length(self.program, reference, || format!("{identity}:length"))
                {
                    if !path.is_empty() {
                        let coordinate = path
                            .iter()
                            .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                            .collect::<Vec<_>>();
                        let Some(name) = validation::frame_length_coordinate_path(
                            self.program,
                            parameter.symbol,
                            &coordinate,
                        ) else {
                            continue;
                        };
                        self.lengths.push(super::Length {
                            subject: super::LengthSubject::Field {
                                owner: parameter.symbol,
                                path,
                            },
                            name,
                            stable: false,
                            type_reference: reference,
                            value: Some(value),
                        });
                    }
                    continue;
                }
                if let Some(carrier) = super::integer_carrier(self.program, reference) {
                    if !path.is_empty() {
                        self.fields
                            .push(super::scalar_fields::ScalarField::for_owner(
                                self.program,
                                parameter.symbol,
                                super::scalar_fields::CapturedScalarField {
                                    path,
                                    carrier,
                                    value: ScopedArithmeticValue::Atom {
                                        identity,
                                        unsigned: super::unsigned(carrier),
                                    },
                                },
                            ));
                    }
                    continue;
                }
                let reference = self
                    .program
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                if matches!(
                    self.program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ) {
                    continue;
                }
                let Some(data) =
                    self.program
                        .data_application_definition(reference)
                        .or_else(|| {
                            self.program
                                .type_reference_table
                                .type_head_symbol(reference)
                                .and_then(|symbol| self.program.data_definition_by_symbol(symbol))
                        })
                else {
                    continue;
                };
                for member in self.program.data_members(data) {
                    if let typed_trees::data::DataMember::Field(field) = member
                        && !field.relevance.is_erased()
                    {
                        let mut child = path.clone();
                        child.push(field.symbol);
                        pending.push((field.type_reference, child, depth + 1));
                    }
                }
            }
        }
    }

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
                            .find(|field| field.path == path)?
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
