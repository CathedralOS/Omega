//! Constructor payloads use the ordinary operand graph before tag observation.
//! The same dynamic field computations retain their calls, traps and selected
//! operators; knowing the constructed case does not erase their evaluation.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    ExpressionNode, PrimitiveType,
};
use checked_trees::{
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction,
    CheckedScalarComputationStructuralArgument,
};

impl Builder<'_, '_> {
    pub(super) fn case_membership(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let machine = crate::lookup::machine_by_symbol(self.program, self.machine)?;
        let state = self
            .program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == self.state)?;
        if !validation::has_exact_case_membership_meaning(
            self.program,
            machine,
            Some(state),
            expression,
            binary,
        ) {
            return None;
        }
        // Dispatch aliases and authored observations resolve to the same place.
        // A transition arm spells its case as a payload-free constructor;
        // an authored test spells it as a name.
        let observed =
            crate::execution::terminal_unit::control::case_observation_alias::observed_place(
                self.program,
                state,
                binary.left,
            )
            .unwrap_or((binary.left, self.statement_index));
        if let Some((_, subject)) = self.readable_case_subject(observed.0, observed.1) {
            let case = match self.program.expression_table.expression(binary.right) {
                ExpressionNode::Name(selected) => selected.symbol,
                ExpressionNode::StructLiteral(literal)
                    if self
                        .program
                        .expression_table
                        .struct_fields(literal.fields)
                        .is_empty() =>
                {
                    literal.case_symbol?
                }
                _ => return None,
            };
            return Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::CaseMembership {
                    source_expression: expression,
                    subject: CheckedScalarComputationStructuralArgument::Place(subject),
                    case,
                },
            ));
        }
        let ExpressionNode::Name(selected) = self.program.expression_table.expression(binary.right)
        else {
            return None;
        };
        let case = selected.symbol;
        let constructor = self.case_construction(binary.left)?;
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::CaseMembership {
                source_expression: expression,
                subject: CheckedScalarComputationStructuralArgument::Case(constructor),
                case,
            },
        ))
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
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            statement_index,
            subject,
        )?;
        let facts::PlaceRoot::Symbol(root) =
            crate::flow::normalized_event_place_root(self.program, place.root)
        else {
            return None;
        };
        let parameter = self.authored_parameters.iter().find(|parameter| {
            crate::flow::normalized_event_place_root(
                self.program,
                facts::PlaceRoot::Symbol(parameter.symbol),
            ) == facts::PlaceRoot::Symbol(root)
        });
        let symbol = if let Some(parameter) = parameter {
            parameter.symbol
        } else {
            let machine = crate::lookup::machine_by_symbol(self.program, self.machine)?;
            let state = self
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == self.state)?;
            let (_, local) = self
                .program
                .state_local_declaration(state, root)
                .filter(|(ordinal, _)| *ordinal < statement_index)?;
            if validation::reference_shell(self.program, local.type_reference).is_some()
                || !(validation::has_plain_owned_contents_with_numeric_constraints(
                    self.program,
                    local.type_reference,
                ) || validation::has_owned_or_shared_view_contents(
                    self.program,
                    local.type_reference,
                ))
            {
                return None;
            }
            local.symbol
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
                // Keep storage identity until the receiver resolves its roster.
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                },
                path,
                type_identity: self
                    .program
                    .normalized_type_identity(projected)
                    .as_str()
                    .to_owned(),
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
