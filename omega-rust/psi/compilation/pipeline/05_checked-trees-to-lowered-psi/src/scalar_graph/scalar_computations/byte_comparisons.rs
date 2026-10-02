//! Observe contents only after the structural operand has completed. The same
//! comparison template serves existing storage and freshly prepared views;
//! descriptor preparation remains inside its selected scalar branch.
use super::{Expansion, LoweringError, QualifiedScalarType, Site, parameters, unsupported};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, LoweredElementPathSegment,
};
use crate::expression_preparation::bindings::ScalarBindings;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedStorageRoot,
    CheckedStructuralPredicatePathSegment as Predicate, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan as Source, CheckedUnitStructuralPathSegment as Path,
};

impl Expansion<'_> {
    pub(super) fn byte_literal_comparison(
        &mut self,
        expression: checked_trees::expression::ExpressionHandle,
        subject: &CheckedUnitStructuralArgumentPlan,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
    ) -> Result<usize, LoweringError> {
        let (operand, bytes, negated) = crate::expression_preparation::source_custody::computation_calls::byte_comparisons::operands(self.checked, expression)?;
        let mut effects = Vec::new();
        let private_bindings;
        let (bindings, root, path) = if let Some(range) = super::subslices::range(subject) {
            let slot = self
                .subslices
                .iter()
                .find(|slot| slot.expression == range.expression)
                .ok_or(LoweringError::Unsupported(
                    "byte comparison operand has no descriptor reservation",
                ))?;
            effects = super::subslices::effects(
                self.checked,
                site.state,
                site.statement,
                checked_trees::CheckedSubsliceSite::Expression {
                    expression: operand,
                },
                subject,
                slot,
                site.bindings,
            )?;
            private_bindings = ScalarBindings::new([]).with_structural_parameters(&[(
                0,
                terminal_psi::StructuralParameterDeclaration {
                    place: slot.place,
                    position: 0,
                    is_self: false,
                    structural_type: slot.structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
            )]);
            (
                &private_bindings,
                CheckedStorageRoot::Parameter { index: 0 },
                Vec::new(),
            )
        } else {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                self.checked,
                site.state,
            )?;
            let root = match subject.source {
                Source::Parameter { parameter_index } => {
                    let (position, _) = self
                        .checked
                        .state_parameters(state)
                        .iter()
                        .enumerate()
                        .filter(|(_, parameter)| {
                            !parameter.relevance.is_erased()
                                && self
                                    .checked
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_none()
                        })
                        .nth(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "byte comparison lost its source parameter",
                        ))?;
                    CheckedStorageRoot::Parameter {
                        index: u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported("byte comparison parameter overflow")
                        })?,
                    }
                }
                Source::StructuralLocal { symbol } => {
                    let (_, local) = self
                        .checked
                        .state_local_declaration(state, symbol)
                        .ok_or(LoweringError::Unsupported("byte comparison lost its local"))?;
                    if validation::reference_shell(&self.checked.typed, local.type_reference)
                        .is_some()
                    {
                        CheckedStorageRoot::ViewLocal { symbol }
                    } else {
                        CheckedStorageRoot::OwnedLocal { symbol }
                    }
                }
                _ => return unsupported("byte comparison has no observable operand"),
            };
            let path = subject
                .path
                .iter()
                .map(|segment| match segment {
                    Path::Field(field) => Ok(Predicate::Field(field.clone())),
                    Path::FixedIndex(index) => Ok(Predicate::FixedIndex(*index)),
                    Path::RuntimeIndex(index) => Ok(Predicate::RuntimeIndex(index.clone())),
                    _ => unsupported("byte comparison has no scalar observation path"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            (site.bindings, root, path)
        };
        let mut comparison = CheckedBooleanExpression::byte_literal_equality(root, path, bytes)
            .ok_or(LoweringError::Unsupported(
                "byte comparison literal exceeds its index carrier",
            ))?;
        if negated {
            comparison = CheckedBooleanExpression::Not(Box::new(comparison));
        }
        let mut value =
            bindings.expression(&CheckedScalarExpression::Boolean(Box::new(comparison)))?;
        // The template asks several questions of one already evaluated operand.
        // Capture its selectors before the length guard, then carry their exact
        // SSA identities through the selected byte observations. This also keeps
        // mutable scalar fields used as selectors from being read repeatedly.
        let mut captures = OperandSelectors::new(input_types.len());
        captures.capture(&mut value);
        let mut operand_types = input_types.to_vec();
        operand_types.extend(
            captures
                .values
                .iter()
                .map(|value| QualifiedScalarType::from(value.scalar_type())),
        );
        let mut comparison = self.binding(
            &operand_types,
            input_types.len(),
            target,
            LoweredScalarBinding::Expression(value),
        );
        for (ordinal, value) in captures.values.into_iter().enumerate().rev() {
            comparison = self.direct_value(
                &operand_types[..input_types.len() + ordinal],
                comparison,
                value,
            )?;
        }
        if effects.is_empty() {
            return Ok(comparison);
        }
        Ok(self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: input_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: effects,
            terminator: LoweredScalarBranchTerminator::Jump {
                trivial_affine_discards: Vec::new(),
                target: comparison,
                arguments: parameters(input_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
            },
        }))
    }

    pub(super) fn byte_predicate(
        &mut self,
        source_expression: checked_trees::expression::ExpressionHandle,
        predicate: terminal_psi::ByteSequencePredicate,
        subject: &CheckedUnitStructuralArgumentPlan,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
    ) -> Result<usize, LoweringError> {
        let (authored_predicate, operand) = crate::expression_preparation::source_custody::computation_calls::byte_predicates::operands(
            self.checked,
            source_expression,
        )?;
        if authored_predicate != predicate {
            return unsupported("byte predicate changed its declared predicate");
        }
        let mut effects = Vec::new();
        let private_bindings;
        let (bindings, root, path) = if let Some(range) = super::subslices::range(subject) {
            let slot = self
                .subslices
                .iter()
                .find(|slot| slot.expression == range.expression)
                .ok_or(LoweringError::Unsupported(
                    "byte predicate operand has no descriptor reservation",
                ))?;
            effects = super::subslices::effects(
                self.checked,
                site.state,
                site.statement,
                checked_trees::CheckedSubsliceSite::Expression {
                    expression: operand,
                },
                subject,
                slot,
                site.bindings,
            )?;
            private_bindings = ScalarBindings::new([]).with_structural_parameters(&[(
                0,
                terminal_psi::StructuralParameterDeclaration {
                    place: slot.place,
                    position: 0,
                    is_self: false,
                    structural_type: slot.structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
            )]);
            (
                &private_bindings,
                CheckedStorageRoot::Parameter { index: 0 },
                Vec::new(),
            )
        } else {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                self.checked,
                site.state,
            )?;
            let root = match subject.source {
                Source::Parameter { parameter_index } => {
                    let (position, _) = self
                        .checked
                        .state_parameters(state)
                        .iter()
                        .enumerate()
                        .filter(|(_, parameter)| {
                            !parameter.relevance.is_erased()
                                && self
                                    .checked
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_none()
                        })
                        .nth(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "byte predicate lost its source parameter",
                        ))?;
                    CheckedStorageRoot::Parameter {
                        index: u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported("byte predicate parameter overflow")
                        })?,
                    }
                }
                Source::StructuralLocal { symbol } => {
                    let (_, local) = self
                        .checked
                        .state_local_declaration(state, symbol)
                        .ok_or(LoweringError::Unsupported("byte predicate lost its local"))?;
                    if validation::reference_shell(&self.checked.typed, local.type_reference)
                        .is_some()
                    {
                        CheckedStorageRoot::ViewLocal { symbol }
                    } else {
                        CheckedStorageRoot::OwnedLocal { symbol }
                    }
                }
                _ => return unsupported("byte predicate has no observable operand"),
            };
            let path = subject
                .path
                .iter()
                .map(|segment| match segment {
                    Path::Field(field) => Ok(Predicate::Field(field.clone())),
                    Path::FixedIndex(index) => Ok(Predicate::FixedIndex(*index)),
                    Path::RuntimeIndex(index) => Ok(Predicate::RuntimeIndex(index.clone())),
                    _ => unsupported("byte predicate has no scalar observation path"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            (site.bindings, root, path)
        };
        let evaluation = CheckedBooleanExpression::ByteSequencePredicate {
            predicate,
            root,
            path,
        };
        let mut value =
            bindings.expression(&CheckedScalarExpression::Boolean(Box::new(evaluation)))?;
        // The evaluated view can carry runtime selectors the predicate itself
        // depends on; capture their SSA identities before the verdict leaf so
        // each selector is read once.
        let mut captures = OperandSelectors::new(input_types.len());
        captures.capture(&mut value);
        let mut operand_types = input_types.to_vec();
        operand_types.extend(
            captures
                .values
                .iter()
                .map(|value| QualifiedScalarType::from(value.scalar_type())),
        );
        let mut evaluation = self.binding(
            &operand_types,
            input_types.len(),
            target,
            LoweredScalarBinding::Expression(value),
        );
        for (ordinal, value) in captures.values.into_iter().enumerate().rev() {
            evaluation = self.direct_value(
                &operand_types[..input_types.len() + ordinal],
                evaluation,
                value,
            )?;
        }
        if effects.is_empty() {
            return Ok(evaluation);
        }
        Ok(self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: input_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: effects,
            terminator: LoweredScalarBranchTerminator::Jump {
                trivial_affine_discards: Vec::new(),
                target: evaluation,
                arguments: parameters(input_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
            },
        }))
    }
}

/// Selectors are operands of the one content query, not independent operands
/// of the length and each byte observation generated from that query.
struct OperandSelectors {
    first_parameter: usize,
    values: Vec<LoweredDirectExpression>,
    paths: Vec<(
        Vec<LoweredElementPathSegment>,
        Vec<LoweredElementPathSegment>,
    )>,
}
impl OperandSelectors {
    fn new(first_parameter: usize) -> Self {
        Self {
            first_parameter,
            values: Vec::new(),
            paths: Vec::new(),
        }
    }
    fn bind(&mut self, value: LoweredDirectExpression) -> LoweredDirectExpression {
        let position = self.first_parameter + self.values.len();
        let scalar_type = value.scalar_type();
        self.values.push(value);
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
    }
    fn capture(&mut self, expression: &mut LoweredDirectExpression) {
        match expression {
            LoweredDirectExpression::Boolean { expression } => self.boolean(expression),
            LoweredDirectExpression::ByteSequenceFieldLength { path, .. }
            | LoweredDirectExpression::ByteSequenceFieldRead { path, .. } => {
                if let Some((_, prepared)) =
                    self.paths.iter().find(|(original, _)| original == path)
                {
                    *path = prepared.clone();
                    return;
                }
                let original = path.clone();
                for segment in path.iter_mut() {
                    if let LoweredElementPathSegment::RuntimeIndex { selector, extent } = segment {
                        *selector = self.bind(selector.clone());
                        *extent = extent.take().map(|value| self.bind(value));
                    }
                }
                self.paths.push((original, path.clone()));
            }
            _ => {}
        }
    }
    fn boolean(&mut self, expression: &mut LoweredBooleanReturnExpression) {
        match expression {
            LoweredBooleanReturnExpression::Not { operand } => self.boolean(operand),
            LoweredBooleanReturnExpression::And { left, right }
            | LoweredBooleanReturnExpression::Or { left, right }
            | LoweredBooleanReturnExpression::Equal { left, right } => {
                self.boolean(left);
                self.boolean(right);
            }
            LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
                self.capture(left);
                self.capture(right);
            }
            _ => {}
        }
    }
}
