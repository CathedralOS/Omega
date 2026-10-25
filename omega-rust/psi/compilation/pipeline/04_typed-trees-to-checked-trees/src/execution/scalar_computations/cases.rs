//! Constructor payloads use the ordinary operand graph before tag observation.
//! The same dynamic field computations retain their calls, traps and selected
//! operators; knowing the constructed case does not erase their evaluation.
use super::{Builder, CheckedScalarComputationHandle, ExpressionHandle, PrimitiveType};
use checked_trees::{
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction,
    CheckedScalarComputationStructuralArgument,
};

impl Builder<'_, '_> {
    pub(super) fn case_membership(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return None;
        };
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_in_machine(machine, self.state)?;
        let case = validation::applied_case_membership_variant(
            self.program,
            machine,
            Some(state),
            expression,
            binary,
        )?;
        if let Some((_, subject)) = self.readable_case_subject(binary.left, self.statement_index) {
            let receiver_value = if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression: anchor } = subject.source {
                let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                    self.program.expression_table.expression(anchor)
                else {
                    return None;
                };
                let source = self.program.saved_value_source(saved)?;
                let reference = validation::expression_result_type_reference(
                    self.program, machine, state, source,
                )?;
                let value = self.structural_value(source, reference)?;
                self.retain_structural_value_root(value, reference)?;
                value
            } else {
                checked_trees::CheckedStructuralValueHandle::invalid()
            };
            return Some(self.publish_case_membership(
                expression,
                expression,
                receiver_value,
                CheckedScalarComputationStructuralArgument::Place(subject),
                case,
            ));
        }
        self.computed_case_membership(expression, binary.left, case)
    }

    /// Resolve the same readable place for membership and intrinsic
    /// enum equality. This route contains only place reads: calls and computed
    /// selectors must already have ordinary captured results before observation.
    pub(super) fn readable_case_subject(
        &self,
        subject: ExpressionHandle,
        statement_index: usize,
    ) -> Option<(
        typed_trees::types::TypeReferenceHandle,
        checked_trees::CheckedUnitStructuralArgumentPlan,
    )> {
        if self.uses_captured_dispatch_value(subject) {
            return None;
        }
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            statement_index,
            subject,
        )?;
        let source = if let Some(expression) = place.root.as_expression() {
            // A saved subject denotes the dispatch's already completed value,
            // not its producer. Read that home without evaluating the producer
            // again or transferring the saved owner's custody into a tag test.
            let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                self.program.expression_table.expression(expression)
            else {
                return None;
            };
            self.program.saved_value_source(saved)?;
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression,
            }
        } else {
            let root = place.root.as_symbol()?;
            let parameter = self
                .authored_parameters
                .iter()
                .find(|parameter| parameter.symbol == root);
            let symbol = if let Some(parameter) = parameter {
                parameter.symbol
            } else {
                let machine = self.program.machine_by_symbol(self.machine)?;
                let state = self.program.state_in_machine(machine, self.state)?;
                let (_, local) = self
                    .program
                    .state_local_declaration(state, root)
                    .filter(|(ordinal, _)| *ordinal < statement_index)?;
                let inspected =
                    validation::unwrapped_type_reference(self.program, local.type_reference)?;
                // A discriminant read borrows the established owner in place. Its
                // payload need not be copyable or predicate-free; source custody
                // checks the tag observation and the later selected extraction.
                // Owned-value classifiers incorrectly rejected routed payloads.
                if !validation::has_contents(
                    self.program,
                    inspected,
                    &[],
                    validation::ContentsRequirement::BorrowedStorage,
                ) {
                    return None;
                }
                local.symbol
            };
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        };
        let (projected, path) = crate::execution::terminal_unit::calls::projected_observed_path(
            self.program,
            self.machine,
            self.state,
            statement_index,
            &place,
            &|selector| self.local_selector(selector),
        )?;
        Some((
            projected,
            checked_trees::CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                // Keep storage identity until the receiver resolves its roster.
                source,
                path,
                type_identity: self.program.normalized_type_identity(projected).to_owned(),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        ))
    }

    /// A runtime selector naming an immutable integer scalar local of this
    /// state: the temp symbol resolution hoists a computed subject index
    /// (`self.grid[self.r * 6 + self.c]`) into. It selects at the local's
    /// dense `Local` position, the coordinate every scalar read of that local
    /// in this computation uses.
    pub(super) fn local_selector(
        &self,
        selector: ExpressionHandle,
    ) -> Option<checked_trees::CheckedRuntimeIndex> {
        let (
            checked_trees::CheckedScalarExpression::Local {
                position,
                primitive_type,
            },
            _,
        ) = crate::values::scalar::scalar_lowering::lower_scalar_expression(
            self.program,
            self.operators,
            selector,
            self.parameters,
            self.authored_parameters,
            self.parameter_types,
            self.locals,
            self.exact_integer_casts,
        )?
        else {
            return None;
        };
        matches!(
            primitive_type,
            PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
                | PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
        )
        .then_some(checked_trees::CheckedRuntimeIndex::Local {
            position: u32::try_from(position).ok()?,
        })
    }

    pub(super) fn dense_structural_parameter_index(&self, authored_position: usize) -> Option<u32> {
        crate::execution::terminal_unit::types::dense_structural_parameter_index(
            self.program,
            self.authored_parameters,
            authored_position,
        )
    }

    /// Membership and stored values share the same authored field computations.
    pub(super) fn case_construction(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarCaseConstruction> {
        let constructor = validation::scalar_case_constructor(self.program, expression)?;
        let mut fields = Vec::with_capacity(constructor.fields.len());
        for (symbol, source, primitive) in constructor.fields {
            let value = self.expression(source, primitive)?;
            self.plans.nodes.get_mut(value).authored_root = source;
            fields.push(CheckedScalarCaseComputationField { symbol, value });
        }
        let fields = self.plans.case_fields.insert_many(fields);
        Some(CheckedScalarCaseConstruction {
            expression,
            type_reference: constructor.type_reference,
            case: constructor.case,
            fields,
        })
    }
}
