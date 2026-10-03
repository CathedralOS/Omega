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
        // A transition arm (`Color::Red -> red()`) spells its case as a
        // payload-free constructor; an authored test spells it as a name.
        // Only a runtime-selected subject admits the constructor spelling
        // here: static subjects keep their pure observation.
        let observed =
            crate::execution::terminal_unit::control::case_observation_alias::observed_place(
                self.program,
                state,
                binary.left,
            )
            .unwrap_or((binary.left, self.statement_index));
        if let Some(subject) = self.runtime_element_case_subject(observed.0, observed.1) {
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
        if let ExpressionNode::Name(name) = self.program.expression_table.expression(binary.left)
            && let Some((_, local)) = self
                .program
                .state_local_declaration(state, name.symbol)
                .filter(|&(ordinal, _)| ordinal < self.statement_index)
        {
            // The observation reads the case tag through a shared borrow;
            // what the local's contents carry — plain fields or shared
            // views whose loans keep their lineage — selects nothing here.
            // A local that is itself a borrow names no storage of its own,
            // so only owned carriers classify.
            if local.is_mutable
                || name.head_symbol != name.symbol
                || self
                    .program
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
                || !(validation::has_plain_owned_contents_with_numeric_constraints(
                    self.program,
                    local.type_reference,
                ) || (validation::has_owned_or_shared_view_contents(
                    self.program,
                    local.type_reference,
                ) && !matches!(
                    self.program
                        .type_reference_table
                        .type_reference(local.type_reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                )))
            {
                return None;
            }
            return Some(self.insert(PrimitiveType::Bool, CheckedScalarComputationKind::CaseMembership {
                source_expression: expression,
                subject: CheckedScalarComputationStructuralArgument::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: local.symbol },
                    path: Vec::new(),
                    type_identity: self.program.normalized_type_identity(local.type_reference).as_str().to_owned(),
                    access: checked_trees::CheckedStructuralAccess::SharedBorrow,
                }),
                case,
            }));
        }
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

    /// The subject of a membership that selects an array element at run time
    /// (`self.items[self.w] is Color::Red`), directly or through the erased
    /// dispatch subject symbol resolution hoists it into
    /// (`control::case_observation_alias`). A static subject keeps the pure
    /// `StructuralCaseMembership` observation its canonical path keys; only a
    /// path through a runtime element takes this name-spelled place, the
    /// route e19d9775393 gave `self.cells[self.i].hp`. Its `RuntimeIndex`
    /// segment names the checked selector and carries no bound: Terminal
    /// re-proves `index < extent` against the array. The observation reads
    /// "some element", so it keys no case fact on a canonical place, while
    /// the dispatch it guards still selects exactly one arm by the observed
    /// tag.
    fn runtime_element_case_subject(
        &self,
        subject: ExpressionHandle,
        statement_index: usize,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            statement_index,
            subject,
        )?;
        // Admission follows the authored selector even when an immutable
        // selector later folds to a fixed index in the observation path.
        if !place
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
        {
            return None;
        }
        let (_, mut argument) = self.parameter_case_subject(subject, statement_index)?;
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            argument.source
        else {
            return None;
        };
        let position = self
            .authored_parameters
            .iter()
            .position(|parameter| parameter.symbol == symbol)?;
        argument.source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: self.dense_structural_parameter_index(position)?,
        };
        Some(argument)
    }

    /// Resolve the same readable parameter place for membership and intrinsic
    /// enum equality. This route contains only place reads: calls and computed
    /// selectors must already have ordinary captured results before observation.
    pub(super) fn parameter_case_subject(
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
        let authored_position = self.authored_parameters.iter().position(|parameter| {
            (parameter.is_self && root == self.machine)
                || crate::flow::normalized_event_place_root(
                    self.program,
                    facts::PlaceRoot::Symbol(parameter.symbol),
                ) == facts::PlaceRoot::Symbol(root)
        })?;
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
                // Formal symbols survive until the receiving stage has the
                // actual retained structural roster, including its receiver.
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: self.authored_parameters[authored_position].symbol,
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
