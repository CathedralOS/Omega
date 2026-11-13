/*
We represent checked operator applications and float comparison selections
(`operators`) alongside checked proof obligations, contracts, float meanings,
and lemmas (`proof`) in `CheckedTrees`.
*/

pub mod operators {
    use crate::{CheckedValueOrigin, CrashCause};
    use typed_trees::operator::declaration_by_symbol;
    use typed_trees::typed_trees::ClosedConformanceApplicationCommitment;
    pub(crate) mod application_operands {
        use super::{
            CheckedOperatorFacts, CheckedOperatorOccurrence, CheckedOperatorResolutionStatus,
        };
        use crate::{CheckedBoundaryOperatorApplicationArgument, CheckedValueOrigin};
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::{PrimitiveType, TypeReferenceNode};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct CheckedBoundaryApplicationOperands {
            pub requirement_operator: SymbolHandle,
            pub operands: Vec<(ExpressionHandle, Option<PrimitiveType>)>,
        }

        impl CheckedOperatorFacts {
            pub fn boundary_application_operands(
                &self,
                program: &TypedTrees,
                expression: ExpressionHandle,
                origin: CheckedValueOrigin,
            ) -> Option<CheckedBoundaryApplicationOperands> {
                if !program.expression_table.expression_is_valid(expression) {
                    return None;
                }
                let spelled = self
                    .uses
                    .iter()
                    .map(|(_, operator_use)| operator_use)
                    .filter(|operator_use| {
                        operator_use.expression == expression
                            && operator_use.origin == origin
                            && operator_use.occurrence == CheckedOperatorOccurrence::Expression
                            && operator_use.status == CheckedOperatorResolutionStatus::Resolved
                            && operator_use.selected_operator_symbol.is_valid()
                    })
                    .collect::<Vec<_>>();
                let named = self
                    .named_uses
                    .iter()
                    .map(|(_, named_use)| named_use)
                    .filter(|named_use| {
                        named_use.expression == expression
                            && named_use.origin == origin
                            && named_use.selected_operator_symbol.is_valid()
                    })
                    .collect::<Vec<_>>();
                let (requirement_operator, spelled_use) =
                    match (spelled.as_slice(), named.as_slice()) {
                        ([operator_use], []) => {
                            (operator_use.selected_operator_symbol, Some(*operator_use))
                        }
                        ([], [named_use]) => (named_use.selected_operator_symbol, None),
                        _ => return None,
                    };
                let operator =
                    typed_trees::operator::declaration_by_symbol(program, requirement_operator)?;
                if !operator.is_boundary {
                    return None;
                }
                let operand_expressions = match program.expression_table.expression(expression) {
                    ExpressionNode::Call(call) => program
                        .expression_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                    _ => spelled_use?.operands(program)?,
                };
                let parameters = program.operator_parameters(operator);
                if parameters.len() != operand_expressions.len()
                    || parameters.iter().any(|parameter| parameter.is_const)
                {
                    return None;
                }
                let binders = program.operator_type_parameters(operator);
                let type_arguments = self
                    .boundary_applications
                    .iter()
                    .filter(|demand| {
                        demand.requirement_symbol == requirement_operator
                            && matches!(
                                demand.site,
                                crate::CheckedBoundaryOperatorApplicationUseSite::Expression {
                                    expression: site,
                                    origin: site_origin,
                                } if site == expression && site_origin == origin
                            )
                    })
                    .flat_map(|demand| &demand.arguments)
                    .filter_map(|argument| match argument {
                        CheckedBoundaryOperatorApplicationArgument::Type {
                            binder_owner,
                            binder_symbol,
                            type_reference,
                            ..
                        } if *binder_owner == requirement_operator => {
                            Some((*binder_symbol, *type_reference))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let operands = operand_expressions
                    .into_iter()
                    .zip(parameters)
                    .map(|(operand, parameter)| {
                        let declared = program.primitive_type_reference(parameter.type_reference);
                        let closed = || match program
                            .type_reference_table
                            .type_reference(parameter.type_reference)
                        {
                            TypeReferenceNode::Named { symbol, .. }
                                if binders.iter().any(|binder| binder.symbol == *symbol) =>
                            {
                                let mut closing =
                                    type_arguments.iter().filter(|(binder, _)| binder == symbol);
                                let (_, type_reference) = closing.next()?;
                                if closing.next().is_some() {
                                    return None;
                                }
                                program.primitive_type_reference(*type_reference)
                            }
                            _ => None,
                        };
                        (operand, declared.or_else(closed))
                    })
                    .collect();
                Some(CheckedBoundaryApplicationOperands {
                    requirement_operator,
                    operands,
                })
            }
        }

        pub fn checked_operator_application_matches_realization(
            program: &TypedTrees,
            demand: &crate::CheckedBoundaryOperatorApplicationDemand,
            realization: &typed_trees::operator::ClosedOperatorRealizationApplication,
        ) -> bool {
            demand.requirement_symbol == realization.requirement_symbol
                && demand.arguments.len() == realization.arguments.len()
                && demand
                    .arguments
                    .iter()
                    .zip(&realization.arguments)
                    .enumerate()
                    .all(|(ordinal, (demand_argument, realization))| {
                        match (demand_argument, realization) {
                            (
                                crate::CheckedBoundaryOperatorApplicationArgument::Type {
                                    binder_owner,
                                    binder_ordinal,
                                    binder_symbol,
                                    type_reference,
                                },
                                typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                                    binder_symbol: realization_binder,
                                    type_reference: realization_type,
                                },
                            ) => {
                                *binder_owner == demand.requirement_symbol
                                    && usize::try_from(*binder_ordinal).ok() == Some(ordinal)
                                    && *binder_symbol == *realization_binder
                                    && program.package_qualified_type_identity(*type_reference)
                                        == program
                                            .package_qualified_type_identity(*realization_type)
                            }
                            (
                                crate::CheckedBoundaryOperatorApplicationArgument::Const {
                                    binder_owner,
                                    binder_ordinal,
                                    binder_symbol,
                                    declared_carrier,
                                    value,
                                },
                                typed_trees::operator::ClosedOperatorApplicationArgument::Const {
                                    binder_symbol: realization_binder,
                                    declared_carrier: realization_carrier,
                                    value: realization_value,
                                },
                            ) => {
                                *binder_owner == demand.requirement_symbol
                                    && usize::try_from(*binder_ordinal).ok() == Some(ordinal)
                                    && *binder_symbol == *realization_binder
                                    && program.package_qualified_type_identity(*declared_carrier)
                                        == program
                                            .package_qualified_type_identity(*realization_carrier)
                                    && value == realization_value
                            }
                            _ => false,
                        }
                    })
        }
    }
    pub(crate) mod comparisons {
        use super::{
            CheckedOperatorFacts, CheckedOperatorOccurrence, CheckedOperatorResolutionStatus,
            CheckedOperatorUseFact, Handle, OperatorSpelling,
        };
        use typed_trees::expression::BinaryOperator;
        impl CheckedOperatorFacts {
            fn resolved_boundary_binary_operator<'a>(
                &'a self,
                program: &'a typed_trees::TypedTrees,
                handle: Handle<CheckedOperatorUseFact>,
            ) -> Option<(
                &'a CheckedOperatorUseFact,
                &'a typed_trees::operator::OperatorDefinition,
            )> {
                if !self.uses.is_valid(handle) {
                    return None;
                }
                let selected = self.uses.get(handle);
                if selected.status != CheckedOperatorResolutionStatus::Resolved
                    || selected.operands(program)?.len() != 2
                {
                    return None;
                }
                let operator = typed_trees::operator::declaration_by_symbol(
                    program,
                    selected.selected_operator_symbol,
                )?;
                if !operator.is_boundary || !self.selected_candidate(selected)?.is_boundary {
                    return None;
                }
                Some((selected, operator))
            }

            pub fn selected_float_comparison(
                &self,
                program: &typed_trees::TypedTrees,
                handle: Handle<CheckedOperatorUseFact>,
            ) -> Option<(
                semantic_vocabulary::IeeeFloatComparisonOperation,
                typed_trees::types::PrimitiveType,
            )> {
                use semantic_vocabulary::IeeeFloatComparisonOperation as Comparison;
                let (selected, operator) =
                    self.resolved_boundary_binary_operator(program, handle)?;
                let (operation, format) =
                    typed_trees::operator::primitive_float_binary_semantics(program, operator)?;
                if operator.spelling != Some(selected.spelling) {
                    return None;
                }
                occurrence_carries_operation(program, selected, operation)?;
                let comparison = match operation {
                    BinaryOperator::Equal => Comparison::Equal,
                    BinaryOperator::NotEqual => Comparison::NotEqual,
                    BinaryOperator::Less => Comparison::Less,
                    BinaryOperator::LessOrEqual => Comparison::LessOrEqual,
                    BinaryOperator::Greater => Comparison::Greater,
                    BinaryOperator::GreaterOrEqual => Comparison::GreaterOrEqual,
                    _ => return None,
                };
                let primitive = match format {
                    numerics::literals::FloatFormat::F32 => typed_trees::types::PrimitiveType::F32,
                    numerics::literals::FloatFormat::F64 => typed_trees::types::PrimitiveType::F64,
                };
                Some((comparison, primitive))
            }

            pub fn selected_integer_comparison(
                &self,
                program: &typed_trees::TypedTrees,
                handle: Handle<CheckedOperatorUseFact>,
            ) -> Option<(BinaryOperator, typed_trees::types::PrimitiveType)> {
                use typed_trees::types::PrimitiveType;
                let (selected, operator) =
                    self.resolved_boundary_binary_operator(program, handle)?;
                if operator.spelling != Some(selected.spelling) {
                    return None;
                }
                let operation = match selected.spelling {
                    OperatorSpelling::Equal => BinaryOperator::Equal,
                    OperatorSpelling::NotEqual => BinaryOperator::NotEqual,
                    OperatorSpelling::Less => BinaryOperator::Less,
                    OperatorSpelling::LessEqual => BinaryOperator::LessOrEqual,
                    OperatorSpelling::Greater => BinaryOperator::Greater,
                    OperatorSpelling::GreaterEqual => BinaryOperator::GreaterOrEqual,
                    _ => return None,
                };
                occurrence_carries_operation(program, selected, operation)?;
                let [left, right] = program.operator_parameters(operator) else {
                    return None;
                };
                let primitive = program.primitive_type_reference(left.type_reference)?;
                if program.primitive_type_reference(right.type_reference) != Some(primitive)
                    || matches!(
                        primitive,
                        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
                    )
                    || program.primitive_type_reference(operator.return_type)
                        != Some(PrimitiveType::Bool)
                {
                    return None;
                }
                Some((operation, primitive))
            }
        }

        fn occurrence_carries_operation(
            program: &typed_trees::TypedTrees,
            selected: &CheckedOperatorUseFact,
            operation: BinaryOperator,
        ) -> Option<()> {
            match selected.occurrence {
                CheckedOperatorOccurrence::Expression => {
                    let typed_trees::expression::ExpressionNode::Binary(binary) =
                        program.expression_table.expression(selected.expression)
                    else {
                        return None;
                    };
                    (binary.operator == operation).then_some(())
                }
                CheckedOperatorOccurrence::MatchEquality { .. }
                    if operation == BinaryOperator::Equal =>
                {
                    Some(())
                }
                _ => None,
            }
        }
    }
    pub(crate) mod selected_float_comparison_execution {
        use super::{CheckedOperatorFacts, CheckedOperatorUseFact, CheckedOperatorUseHandle};
        use semantic_vocabulary::IeeeFloatComparisonOperation;
        use typed_trees::TypedTrees;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::types::PrimitiveType;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedSelectedFloatComparisonExecution {
            operator_use: CheckedOperatorUseHandle,
            selected: CheckedOperatorUseFact,
            operands: [ExpressionHandle; 2],
            comparison: IeeeFloatComparisonOperation,
            primitive: PrimitiveType,
        }

        impl Default for CheckedSelectedFloatComparisonExecution {
            fn default() -> Self {
                Self {
                    operator_use: CheckedOperatorUseHandle::invalid(),
                    selected: CheckedOperatorUseFact::default(),
                    operands: [ExpressionHandle::invalid(); 2],
                    comparison: IeeeFloatComparisonOperation::Equal,
                    primitive: PrimitiveType::F32,
                }
            }
        }

        impl CheckedSelectedFloatComparisonExecution {
            pub fn from_selected_provider(
                program: &TypedTrees,
                facts: &CheckedOperatorFacts,
                operator_use: CheckedOperatorUseHandle,
                comparison: IeeeFloatComparisonOperation,
                primitive: PrimitiveType,
            ) -> Option<Self> {
                let selected = *facts.uses.get(operator_use);
                let operands = selected.operands(program)?.try_into().ok()?;
                let execution = Self {
                    operator_use,
                    selected,
                    operands,
                    comparison,
                    primitive,
                };
                execution.validated_primitive(program, facts)?;
                Some(execution)
            }

            pub fn operator_use(&self) -> CheckedOperatorUseHandle {
                self.operator_use
            }

            pub fn validated_primitive(
                &self,
                program: &TypedTrees,
                facts: &CheckedOperatorFacts,
            ) -> Option<PrimitiveType> {
                if !facts.uses.is_valid(self.operator_use)
                    || facts.uses.get(self.operator_use) != &self.selected
                    || self.selected.operands(program)?.as_slice() != self.operands
                    || facts.selected_float_comparison(program, self.operator_use)
                        != Some((self.comparison, self.primitive))
                {
                    return None;
                }
                let site = self.selected.application_site();
                let mut applications = facts
                    .boundary_applications
                    .iter()
                    .filter(|application| application.site == site);
                let application = applications.next()?;
                if applications.next().is_some()
                    || application.requirement_symbol != self.selected.selected_operator_symbol
                    || !application.arguments.is_empty()
                    || facts
                        .symbolic_boundary_applications
                        .iter()
                        .any(|application| application.site == site)
                {
                    return None;
                }
                Some(self.primitive)
            }
        }
    }
    pub use application_operands::{
        CheckedBoundaryApplicationOperands, checked_operator_application_matches_realization,
    };
    pub use selected_float_comparison_execution::CheckedSelectedFloatComparisonExecution;
    pub type CheckedOperatorUseHandle = arena::Handle<CheckedOperatorUseFact>;
    use arena::{Arena, Handle, HandleSpan};
    use language_core::operator_spelling::OperatorSpelling;
    use language_semantics::const_value::CanonicalConstIdentity;
    use symbols::SymbolHandle;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::SignatureContract;
    use typed_trees::statement::StatementHandle;
    use typed_trees::type_identity::CallableIdentityId;
    use typed_trees::types::TypeReferenceHandle;

    pub use numerics::arithmetic::ArithmeticPolicyAdapter as CheckedArithmeticPolicyAdapter;

    typed_trees::digest_commitment! {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        CheckedProviderPlanCommitment
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedOperatorResolutionStatus {
        #[default]
        Missing,
        Resolved,
        Ambiguous,
        DomainPending,
        BuiltinFallback,
        Inadmissible,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOperatorUseFact {
        pub expression: ExpressionHandle,
        pub origin: CheckedValueOrigin,
        pub is_generic_template: bool,
        pub occurrence: CheckedOperatorOccurrence,
        pub spelling: OperatorSpelling,
        pub policy_adapter: CheckedArithmeticPolicyAdapter,
        pub selected_operator_symbol: SymbolHandle,
        pub candidates: HandleSpan<CheckedOperatorCandidateFact>,
        pub candidate_count: usize,
        pub status: CheckedOperatorResolutionStatus,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedOperatorOccurrence {
        #[default]
        Expression,
        MatchEquality {
            source_arm: Handle<typed_trees::expression::TableMatchArm>,
        },
    }

    impl CheckedOperatorUseFact {
        pub fn resolved_through_builtin_fallback(&self, spelling: OperatorSpelling) -> bool {
            self.spelling == spelling
                && !self.selected_operator_symbol.is_valid()
                && self.candidate_count == 0
                && matches!(
                    self.status,
                    CheckedOperatorResolutionStatus::Missing
                        | CheckedOperatorResolutionStatus::BuiltinFallback
                )
        }

        pub fn application_site(&self) -> CheckedBoundaryOperatorApplicationUseSite {
            match self.occurrence {
                CheckedOperatorOccurrence::Expression => {
                    CheckedBoundaryOperatorApplicationUseSite::Expression {
                        expression: self.expression,
                        origin: self.origin,
                    }
                }
                CheckedOperatorOccurrence::MatchEquality { source_arm } => {
                    CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
                        expression: self.expression,
                        source_arm,
                        origin: self.origin,
                    }
                }
            }
        }

        pub fn operands(&self, program: &typed_trees::TypedTrees) -> Option<Vec<ExpressionHandle>> {
            use typed_trees::expression::{ExpressionNode, MatchPattern};
            if !program
                .expression_table
                .expression_is_valid(self.expression)
            {
                return None;
            }
            match self.occurrence {
                CheckedOperatorOccurrence::Expression => {
                    match program.expression_table.expression(self.expression) {
                        ExpressionNode::Binary(binary) => Some(vec![binary.left, binary.right]),
                        ExpressionNode::Indexed(indexed) => {
                            let mut operands = vec![indexed.collection];
                            match program.expression_table.expression(indexed.index) {
                                ExpressionNode::Range(range) => {
                                    operands.extend([range.start, range.end])
                                }
                                _ => operands.push(indexed.index),
                            }
                            Some(operands)
                        }
                        _ => None,
                    }
                }
                CheckedOperatorOccurrence::MatchEquality { source_arm } => {
                    let dispatch = program.expression_table.match_node(self.expression)?;
                    if self.spelling != OperatorSpelling::Equal
                        || source_arm.generation() != dispatch.arms.start().generation()
                    {
                        return None;
                    }
                    let ordinal = source_arm
                        .arena_index()
                        .checked_sub(dispatch.arms.start().arena_index())?;
                    let arm = program
                        .expression_table
                        .match_arms(dispatch.arms)
                        .get(ordinal as usize)?;
                    let MatchPattern::Value(pattern) = arm.pattern else {
                        return None;
                    };
                    Some(vec![dispatch.subject, pattern])
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNamedOperatorUseFact {
        pub expression: ExpressionHandle,
        pub origin: CheckedValueOrigin,
        pub is_generic_template: bool,
        pub selected_operator_symbol: SymbolHandle,
        pub policy_adapter: CheckedArithmeticPolicyAdapter,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedNamedRequirementUseFact {
        pub expression: ExpressionHandle,
        pub origin: CheckedValueOrigin,
        pub requirement_symbol: SymbolHandle,
        pub policy_adapter: CheckedArithmeticPolicyAdapter,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedProviderRequirementKind {
        Operator { is_product_boundary: bool },
        DirectCall { diagnostic_name: String },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedProviderRequirement {
        pub symbol: SymbolHandle,
        pub kind: CheckedProviderRequirementKind,
        pub schema_identity: String,
        pub package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    }

    impl Default for CheckedProviderRequirementKind {
        fn default() -> Self {
            Self::Operator {
                is_product_boundary: false,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CheckedCrashQualifiedUse {
        Spelled(CheckedOperatorUseHandle),
        Named(Handle<CheckedNamedOperatorUseFact>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedBoundaryOperatorApplicationDemand {
        pub site: CheckedBoundaryOperatorApplicationUseSite,
        pub requirement_symbol: SymbolHandle,
        pub requirement_identity: CallableIdentityId,
        pub arguments: Vec<CheckedBoundaryOperatorApplicationArgument>,
        pub body_candidates: HandleSpan<CheckedBoundaryBodyCandidate>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedBoundaryBodyCandidate {
        pub declaration: Handle<CheckedBoundaryOperatorDeclaration>,
        pub is_product: bool,
        pub selection: SymbolHandle,
        pub selection_identity: CallableIdentityId,
        pub target: Option<String>,
        pub machine: SymbolHandle,
        pub entry: SymbolHandle,
        pub contract: [u8; 32],
        pub specialization: Option<(SymbolHandle, [u8; 32])>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedBoundaryOperatorDeclaration {
        pub requirement: SymbolHandle,
        pub machine: SymbolHandle,
        pub selection: language_semantics::BoundaryBodySelection,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedSymbolicBoundaryOperatorApplicationDemand {
        pub site: CheckedBoundaryOperatorApplicationUseSite,
        pub requirement_symbol: SymbolHandle,
        pub machine_symbol: SymbolHandle,
        pub arguments: Vec<CheckedSymbolicBoundaryOperatorApplicationArgument>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedSymbolicBoundaryOperatorApplicationArgument {
        TypeBinder {
            binder_owner: SymbolHandle,
            binder_ordinal: u32,
            binder_symbol: SymbolHandle,
            machine_binder_ordinal: u32,
            machine_binder_symbol: SymbolHandle,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBoundaryOperatorApplicationUseSite {
        Expression {
            expression: ExpressionHandle,
            origin: CheckedValueOrigin,
        },
        MatchEquality {
            expression: ExpressionHandle,
            source_arm: Handle<typed_trees::expression::TableMatchArm>,
            origin: CheckedValueOrigin,
        },
        Statement(StatementHandle),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedBoundaryOperatorApplicationArgument {
        Type {
            binder_owner: SymbolHandle,
            binder_ordinal: u32,
            binder_symbol: SymbolHandle,
            type_reference: TypeReferenceHandle,
        },
        Const {
            binder_owner: SymbolHandle,
            binder_ordinal: u32,
            binder_symbol: SymbolHandle,
            declared_carrier: TypeReferenceHandle,
            value: CanonicalConstIdentity,
        },
    }

    impl Default for CheckedOperatorUseFact {
        fn default() -> Self {
            Self {
                expression: ExpressionHandle::invalid(),
                origin: CheckedValueOrigin::default(),
                is_generic_template: false,
                occurrence: CheckedOperatorOccurrence::Expression,
                spelling: OperatorSpelling::Index,
                policy_adapter: CheckedArithmeticPolicyAdapter::None,
                selected_operator_symbol: SymbolHandle::invalid(),
                candidates: HandleSpan::empty(),
                candidate_count: 0,
                status: CheckedOperatorResolutionStatus::Missing,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOperatorCandidateFact {
        pub operator_symbol: SymbolHandle,
        pub domain_symbol: SymbolHandle,
        pub conformance_symbol: SymbolHandle,
        pub trait_requirement_symbol: SymbolHandle,
        pub realization_machine_symbol: SymbolHandle,
        pub realization_state_symbol: SymbolHandle,
        pub conformance_application_report_fingerprint: u64,
        pub conformance_application_commitment: ClosedConformanceApplicationCommitment,
        pub receiver_type: TypeReferenceHandle,
        pub return_type: TypeReferenceHandle,
        pub contracts: HandleSpan<SignatureContract>,
        pub type_parameter_count: usize,
        pub parameter_count: usize,
        pub contract_count: usize,
        pub is_boundary: bool,
    }

    impl CheckedOperatorCandidateFact {
        pub const fn root(operator_symbol: SymbolHandle) -> Self {
            Self {
                operator_symbol,
                domain_symbol: SymbolHandle::invalid(),
                conformance_symbol: SymbolHandle::invalid(),
                trait_requirement_symbol: SymbolHandle::invalid(),
                realization_machine_symbol: SymbolHandle::invalid(),
                realization_state_symbol: SymbolHandle::invalid(),
                conformance_application_report_fingerprint: 0,
                conformance_application_commitment:
                    ClosedConformanceApplicationCommitment::from_digest([0; 32]),
                receiver_type: TypeReferenceHandle::invalid(),
                return_type: TypeReferenceHandle::invalid(),
                contracts: HandleSpan::empty(),
                type_parameter_count: 0,
                parameter_count: 0,
                contract_count: 0,
                is_boundary: false,
            }
        }

        pub const fn domain(operator_symbol: SymbolHandle, domain_symbol: SymbolHandle) -> Self {
            Self {
                domain_symbol,
                ..Self::root(operator_symbol)
            }
        }

        pub const fn is_domain_owned(self) -> bool {
            self.domain_symbol.is_valid()
        }

        pub const fn trait_backed(
            requirement_symbol: SymbolHandle,
            conformance_symbol: SymbolHandle,
            realization_machine_symbol: SymbolHandle,
            realization_state_symbol: SymbolHandle,
            conformance_application_report_fingerprint: u64,
            conformance_application_commitment: ClosedConformanceApplicationCommitment,
        ) -> Self {
            Self {
                operator_symbol: requirement_symbol,
                domain_symbol: SymbolHandle::invalid(),
                conformance_symbol,
                trait_requirement_symbol: requirement_symbol,
                realization_machine_symbol,
                realization_state_symbol,
                conformance_application_report_fingerprint,
                conformance_application_commitment,
                receiver_type: TypeReferenceHandle::invalid(),
                return_type: TypeReferenceHandle::invalid(),
                contracts: HandleSpan::empty(),
                type_parameter_count: 0,
                parameter_count: 0,
                contract_count: 0,
                is_boundary: false,
            }
        }

        pub const fn is_trait_backed(self) -> bool {
            self.conformance_symbol.is_valid() && self.trait_requirement_symbol.is_valid()
        }

        pub const fn with_signature(
            mut self,
            receiver_type: TypeReferenceHandle,
            return_type: TypeReferenceHandle,
            contracts: HandleSpan<SignatureContract>,
            type_parameter_count: usize,
            parameter_count: usize,
            is_boundary: bool,
        ) -> Self {
            self.receiver_type = receiver_type;
            self.return_type = return_type;
            self.contracts = contracts;
            self.type_parameter_count = type_parameter_count;
            self.parameter_count = parameter_count;
            self.contract_count = contracts.len();
            self.is_boundary = is_boundary;
            self
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CheckedOperatorResolutionIssue<'facts> {
        pub operator_use: &'facts CheckedOperatorUseFact,
        pub candidates: &'facts [CheckedOperatorCandidateFact],
    }

    impl CheckedOperatorResolutionIssue<'_> {
        pub const fn status(&self) -> CheckedOperatorResolutionStatus {
            self.operator_use.status
        }

        pub const fn is_ambiguous(&self) -> bool {
            matches!(
                self.operator_use.status,
                CheckedOperatorResolutionStatus::Ambiguous
            )
        }

        pub const fn is_inadmissible(&self) -> bool {
            matches!(
                self.operator_use.status,
                CheckedOperatorResolutionStatus::Inadmissible
            )
        }

        pub const fn spelling(&self) -> OperatorSpelling {
            self.operator_use.spelling
        }

        pub const fn candidate_count(&self) -> usize {
            self.operator_use.candidate_count
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CheckedOperatorContractUse<'facts> {
        pub operator_use: &'facts CheckedOperatorUseFact,
        pub candidate: &'facts CheckedOperatorCandidateFact,
    }

    impl CheckedOperatorContractUse<'_> {
        pub const fn contract_count(&self) -> usize {
            self.candidate.contract_count
        }

        pub const fn contracts(&self) -> HandleSpan<SignatureContract> {
            self.candidate.contracts
        }

        pub const fn operator_symbol(&self) -> SymbolHandle {
            self.candidate.operator_symbol
        }

        pub const fn domain_symbol(&self) -> SymbolHandle {
            self.candidate.domain_symbol
        }

        pub const fn is_boundary(&self) -> bool {
            self.candidate.is_boundary
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOperatorCrashBucket {
        cause: CrashCause,
        unconditional: bool,
        facts: Vec<Handle<ProofFact>>,
    }

    impl CheckedOperatorCrashBucket {
        pub fn new(cause: CrashCause, unconditional: bool, facts: Vec<Handle<ProofFact>>) -> Self {
            Self {
                cause,
                unconditional,
                facts,
            }
        }

        pub const fn cause(&self) -> CrashCause {
            self.cause
        }

        pub const fn is_unconditional(&self) -> bool {
            self.unconditional
        }

        pub fn facts(&self) -> &[Handle<ProofFact>] {
            &self.facts
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOperatorCrashContract {
        operator_symbol: SymbolHandle,
        buckets: Vec<CheckedOperatorCrashBucket>,
    }

    impl CheckedOperatorCrashContract {
        pub fn new(
            operator_symbol: SymbolHandle,
            buckets: Vec<CheckedOperatorCrashBucket>,
        ) -> Self {
            Self {
                operator_symbol,
                buckets,
            }
        }

        pub const fn operator_symbol(&self) -> SymbolHandle {
            self.operator_symbol
        }

        pub fn buckets(&self) -> &[CheckedOperatorCrashBucket] {
            &self.buckets
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ExactIntegerCastFact {
        pub expression: ExpressionHandle,
        pub source_type: typed_trees::types::PrimitiveType,
        pub target_type: typed_trees::types::PrimitiveType,
        pub minimum: numerics::bignum::BigInt,
        pub maximum: numerics::bignum::BigInt,
    }

    impl Default for ExactIntegerCastFact {
        fn default() -> Self {
            Self {
                expression: ExpressionHandle::invalid(),
                source_type: typed_trees::types::PrimitiveType::Bool,
                target_type: typed_trees::types::PrimitiveType::Bool,
                minimum: numerics::bignum::BigInt::zero(),
                maximum: numerics::bignum::BigInt::zero(),
            }
        }
    }

    #[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOperatorFacts {
        pub uses: Arena<CheckedOperatorUseFact>,
        pub provider_requirements: Arena<CheckedProviderRequirement>,
        pub exact_integer_casts: Vec<ExactIntegerCastFact>,
        pub decided_formations: Vec<ExpressionHandle>,
        pub trap_free_expressions: Vec<ExpressionHandle>,
        pub selected_float_comparisons: Arena<CheckedSelectedFloatComparisonExecution>,
        pub named_uses: Arena<CheckedNamedOperatorUseFact>,
        pub named_requirement_uses: Arena<CheckedNamedRequirementUseFact>,
        pub candidates: Arena<CheckedOperatorCandidateFact>,
        pub operator_crash_contracts: Vec<CheckedOperatorCrashContract>,
        pub operator_realization_contracts: Vec<CheckedOperatorRealizationContract>,
        pub boundary_applications: Vec<CheckedBoundaryOperatorApplicationDemand>,
        pub boundary_body_candidates: Arena<CheckedBoundaryBodyCandidate>,
        pub boundary_declarations: Arena<CheckedBoundaryOperatorDeclaration>,
        pub symbolic_boundary_applications: Vec<CheckedSymbolicBoundaryOperatorApplicationDemand>,
        expression_uses: arena::OwnedIndex<ExpressionUseIndex>,
    }

    #[derive(Clone, Default)]
    struct ExpressionUseIndex {
        expressions: symbols::SymbolKeyMap<ExpressionHandle, ExpressionUseRoster>,
        handles: Arena<Handle<CheckedOperatorUseFact>>,
    }

    #[derive(Clone, Copy, Default)]
    struct ExpressionUseRoster {
        first_expression: Handle<CheckedOperatorUseFact>,
        all: HandleSpan<Handle<CheckedOperatorUseFact>>,
    }

    impl ExpressionUseIndex {
        fn build(uses: &Arena<CheckedOperatorUseFact>) -> Self {
            let mut ordered = uses
                .iter()
                .map(|(handle, fact)| (fact.expression, handle))
                .collect::<Vec<_>>();
            ordered
                .sort_by_key(|(expression, _)| (expression.arena_index(), expression.generation()));
            let mut index = Self::default();
            for group in ordered.chunk_by(|left, right| left.0 == right.0) {
                let first_expression = group
                    .iter()
                    .find_map(|(_, handle)| {
                        (uses.get(*handle).occurrence == CheckedOperatorOccurrence::Expression)
                            .then_some(*handle)
                    })
                    .unwrap_or_default();
                let all = index
                    .handles
                    .insert_many(group.iter().map(|(_, handle)| *handle));
                index.expressions.insert(
                    group[0].0,
                    ExpressionUseRoster {
                        first_expression,
                        all,
                    },
                );
            }
            index
        }
    }

    impl std::fmt::Debug for CheckedOperatorFacts {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("CheckedOperatorFacts")
                .field("uses", &self.uses)
                .field("provider_requirements", &self.provider_requirements)
                .field("exact_integer_casts", &self.exact_integer_casts)
                .field("decided_formations", &self.decided_formations)
                .field("trap_free_expressions", &self.trap_free_expressions)
                .field(
                    "selected_float_comparisons",
                    &self.selected_float_comparisons,
                )
                .field("named_uses", &self.named_uses)
                .field("named_requirement_uses", &self.named_requirement_uses)
                .field("candidates", &self.candidates)
                .field("operator_crash_contracts", &self.operator_crash_contracts)
                .field(
                    "operator_realization_contracts",
                    &self.operator_realization_contracts,
                )
                .field("boundary_applications", &self.boundary_applications)
                .field(
                    "symbolic_boundary_applications",
                    &self.symbolic_boundary_applications,
                )
                .finish()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedOperatorRealizationContract {
        machine_symbol: SymbolHandle,
        operator_symbol: SymbolHandle,
    }

    impl CheckedOperatorRealizationContract {
        pub fn new(machine_symbol: SymbolHandle, operator_symbol: SymbolHandle) -> Self {
            Self {
                machine_symbol,
                operator_symbol,
            }
        }

        pub const fn machine_symbol(&self) -> SymbolHandle {
            self.machine_symbol
        }

        pub const fn operator_symbol(&self) -> SymbolHandle {
            self.operator_symbol
        }
    }

    impl CheckedOperatorFacts {
        pub fn project_body_candidates<'facts, Body>(
            &'facts self,
            application: &CheckedBoundaryOperatorApplicationDemand,
            payload: impl Fn(&'facts CheckedBoundaryBodyCandidate) -> Body,
        ) -> Result<
            Vec<language_semantics::CheckedBodyCandidate<'facts, Body>>,
            language_semantics::boundary_provider_review::BoundaryApplicationEvidenceError,
        > {
            use language_semantics::boundary_provider_review::BoundaryApplicationEvidenceError as Error;
            let bodies = self
                .boundary_body_candidates
                .span(application.body_candidates)
                .ok_or(Error::MissingBodySpan)?;
            let mut candidates = Vec::with_capacity(bodies.len());
            for body in bodies {
                if !self.boundary_declarations.is_valid(body.declaration) {
                    return Err(Error::MissingDeclaration);
                }
                let declaration = self.boundary_declarations.get(body.declaration);
                if declaration.requirement != application.requirement_symbol {
                    return Err(Error::DifferentRequirement);
                }
                candidates.push(language_semantics::CheckedBodyCandidate {
                    body: payload(body),
                    declaration: &declaration.selection,
                    target: body.target.as_deref(),
                    is_product: body.is_product,
                });
            }
            Ok(candidates)
        }

        pub fn formation_is_decided(&self, expression: ExpressionHandle) -> bool {
            self.decided_formations
                .binary_search_by_key(&handle_position(expression), |defined| {
                    handle_position(*defined)
                })
                .is_ok()
        }

        pub fn is_trap_free(&self, expression: ExpressionHandle) -> bool {
            self.trap_free_expressions
                .binary_search_by_key(&handle_position(expression), |source| {
                    handle_position(*source)
                })
                .is_ok()
        }

        pub fn crash_qualified_uses<'facts>(
            &'facts self,
            program: &'facts typed_trees::TypedTrees,
        ) -> impl Iterator<Item = CheckedCrashQualifiedUse> + 'facts {
            let publishes_crash = |contracts: HandleSpan<SignatureContract>| {
                program
                    .signature_contracts
                    .span_or_empty(contracts)
                    .iter()
                    .any(|contract| contract.is_crashes())
            };
            self.uses
                .iter()
                .filter_map(move |(handle, operator_use)| {
                    self.selected_candidate(operator_use)
                        .is_some_and(|selected| publishes_crash(selected.contracts))
                        .then_some(CheckedCrashQualifiedUse::Spelled(handle))
                })
                .chain(
                    self.named_uses
                        .iter()
                        .filter_map(move |(handle, named_use)| {
                            declaration_by_symbol(program, named_use.selected_operator_symbol)
                                .is_some_and(|operator| publishes_crash(operator.contracts))
                                .then_some(CheckedCrashQualifiedUse::Named(handle))
                        }),
                )
        }

        pub fn has_crash_qualified_uses(&self, program: &typed_trees::TypedTrees) -> bool {
            self.crash_qualified_uses(program).next().is_some()
        }

        pub fn with_roots(
            uses: Arena<CheckedOperatorUseFact>,
            named_uses: Arena<CheckedNamedOperatorUseFact>,
            candidates: Arena<CheckedOperatorCandidateFact>,
        ) -> Self {
            Self {
                uses,
                provider_requirements: Arena::new(),
                exact_integer_casts: Vec::new(),
                decided_formations: Vec::new(),
                trap_free_expressions: Vec::new(),
                selected_float_comparisons: Arena::new(),
                named_uses,
                named_requirement_uses: Arena::new(),
                candidates,
                operator_crash_contracts: Vec::new(),
                operator_realization_contracts: Vec::new(),
                boundary_applications: Vec::new(),
                boundary_body_candidates: Arena::new(),
                boundary_declarations: Arena::new(),
                symbolic_boundary_applications: Vec::new(),
                expression_uses: arena::OwnedIndex::default(),
            }
        }

        pub fn with_operator_crash_contracts(
            mut self,
            operator_crash_contracts: Vec<CheckedOperatorCrashContract>,
        ) -> Self {
            self.operator_crash_contracts = operator_crash_contracts;
            self
        }

        pub fn with_operator_realization_contracts(
            mut self,
            operator_realization_contracts: Vec<CheckedOperatorRealizationContract>,
        ) -> Self {
            self.operator_realization_contracts = operator_realization_contracts;
            self
        }

        pub fn expression_use(
            &self,
            expression: ExpressionHandle,
        ) -> Option<&CheckedOperatorUseFact> {
            let handle = self.expression_uses.with(
                self.uses.revision(),
                || ExpressionUseIndex::build(&self.uses),
                |index| {
                    index
                        .expressions
                        .get(&expression)
                        .map(|roster| roster.first_expression)
                },
            )?;
            handle.is_valid().then(|| self.uses.get(handle))
        }

        pub fn for_each_expression_use<'facts>(
            &'facts self,
            expression: ExpressionHandle,
            mut visit: impl FnMut(Handle<CheckedOperatorUseFact>, &'facts CheckedOperatorUseFact),
        ) {
            self.expression_uses.with(
                self.uses.revision(),
                || ExpressionUseIndex::build(&self.uses),
                |index| {
                    if let Some(roster) = index.expressions.get(&expression) {
                        for handle in index.handles.span_or_empty(roster.all) {
                            visit(*handle, self.uses.get(*handle));
                        }
                    }
                },
            );
        }

        pub fn uses_with_status(
            &self,
            status: CheckedOperatorResolutionStatus,
        ) -> impl Iterator<Item = &CheckedOperatorUseFact> {
            self.uses.iter().filter_map(move |(_, operator_use)| {
                (operator_use.status == status).then_some(operator_use)
            })
        }

        pub fn resolved_uses(&self) -> impl Iterator<Item = &CheckedOperatorUseFact> {
            self.uses_with_status(CheckedOperatorResolutionStatus::Resolved)
        }

        pub fn resolved_expression_selections(
            &self,
        ) -> impl Iterator<
            Item = (
                ExpressionHandle,
                CheckedValueOrigin,
                SymbolHandle,
                CheckedArithmeticPolicyAdapter,
            ),
        > {
            self.resolved_uses()
                .filter(|fact| fact.occurrence == CheckedOperatorOccurrence::Expression)
                .map(|fact| {
                    (
                        fact.expression,
                        fact.origin,
                        fact.selected_operator_symbol,
                        fact.policy_adapter,
                    )
                })
                .chain(self.named_uses().map(|fact| {
                    (
                        fact.expression,
                        fact.origin,
                        fact.selected_operator_symbol,
                        fact.policy_adapter,
                    )
                }))
                .chain(self.named_requirement_uses().map(|fact| {
                    (
                        fact.expression,
                        fact.origin,
                        fact.requirement_symbol,
                        fact.policy_adapter,
                    )
                }))
        }

        pub fn for_each_resolved_expression_selection<'facts>(
            &'facts self,
            expression: ExpressionHandle,
            mut visit: impl FnMut(
                CheckedValueOrigin,
                SymbolHandle,
                CheckedArithmeticPolicyAdapter,
                Option<&'facts CheckedOperatorUseFact>,
            ),
        ) {
            self.for_each_expression_use(expression, |_, fact| {
                if fact.status == CheckedOperatorResolutionStatus::Resolved
                    && fact.occurrence == CheckedOperatorOccurrence::Expression
                {
                    visit(
                        fact.origin,
                        fact.selected_operator_symbol,
                        fact.policy_adapter,
                        Some(fact),
                    );
                }
            });
            for fact in self.named_uses() {
                if fact.expression == expression {
                    visit(
                        fact.origin,
                        fact.selected_operator_symbol,
                        fact.policy_adapter,
                        None,
                    );
                }
            }
            for fact in self.named_requirement_uses() {
                if fact.expression == expression {
                    visit(
                        fact.origin,
                        fact.requirement_symbol,
                        fact.policy_adapter,
                        None,
                    );
                }
            }
        }

        pub fn named_uses(&self) -> impl Iterator<Item = &CheckedNamedOperatorUseFact> {
            self.named_uses.iter().map(|(_, operator_use)| operator_use)
        }

        pub fn with_named_requirement_uses(
            mut self,
            named_requirement_uses: Arena<CheckedNamedRequirementUseFact>,
        ) -> Self {
            self.named_requirement_uses = named_requirement_uses;
            self
        }

        pub fn named_requirement_uses(
            &self,
        ) -> impl Iterator<Item = &CheckedNamedRequirementUseFact> {
            self.named_requirement_uses
                .iter()
                .map(|(_, requirement_use)| requirement_use)
        }

        pub fn resolution_issues(
            &self,
        ) -> impl Iterator<Item = CheckedOperatorResolutionIssue<'_>> {
            self.uses
                .iter()
                .filter(|&(_, operator_use)| {
                    matches!(
                        operator_use.status,
                        CheckedOperatorResolutionStatus::Missing
                            | CheckedOperatorResolutionStatus::Ambiguous
                            | CheckedOperatorResolutionStatus::Inadmissible
                    )
                })
                .map(|(_, operator_use)| CheckedOperatorResolutionIssue {
                    operator_use,
                    candidates: self.candidates(operator_use),
                })
        }

        pub fn selected_candidate(
            &self,
            operator_use: &CheckedOperatorUseFact,
        ) -> Option<&CheckedOperatorCandidateFact> {
            if !operator_use.selected_operator_symbol.is_valid() {
                return None;
            }
            self.candidates(operator_use).iter().find(|candidate| {
                candidate.operator_symbol == operator_use.selected_operator_symbol
            })
        }

        pub fn selected_trait_candidate_in_machine(
            &self,
            expression: ExpressionHandle,
            machine_symbol: SymbolHandle,
        ) -> Option<&CheckedOperatorCandidateFact> {
            let mut found = None;
            self.for_each_expression_use(expression, |_, operator_use| {
                if found.is_none()
                    && operator_use.occurrence == CheckedOperatorOccurrence::Expression
                    && operator_use.origin.machine_symbol() == Some(machine_symbol)
                    && operator_use.status == CheckedOperatorResolutionStatus::Resolved
                    && let Some(candidate) = self
                        .selected_candidate(operator_use)
                        .filter(|candidate| candidate.is_trait_backed())
                {
                    found = Some(candidate);
                }
            });
            found
        }

        pub fn resolved_contract_uses(
            &self,
        ) -> impl Iterator<Item = CheckedOperatorContractUse<'_>> {
            self.resolved_uses().filter_map(|operator_use| {
                let [candidate] = self.candidates(operator_use) else {
                    return None;
                };
                (candidate.contract_count > 0).then_some(CheckedOperatorContractUse {
                    operator_use,
                    candidate,
                })
            })
        }

        pub fn candidates(
            &self,
            operator_use: &CheckedOperatorUseFact,
        ) -> &[CheckedOperatorCandidateFact] {
            self.candidates.span_or_empty(operator_use.candidates)
        }
    }

    pub fn handle_position(expression: ExpressionHandle) -> (u32, u32) {
        (expression.arena_index(), expression.generation())
    }
}

pub mod proof {
    pub(crate) mod contracts {
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;

        use crate::CheckedValueOrigin;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum ContractProofFactKind {
            #[default]
            Requires,
            Ensures,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum ContractProofFactOwner {
            #[default]
            Unknown,
            Machine {
                machine_symbol: SymbolHandle,
            },
            MachineState {
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
            },
            StateSignature {
                owner_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
            },
            RuntimeCallableContract {
                contract: typed_trees::callable_values::RuntimeCallableContractHandle,
            },
            CallSignature {
                owner_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                application: Handle<CallContractSignature>,
            },
            OperatorDeclaration {
                operator_symbol: SymbolHandle,
            },
            OperatorUse {
                expression: ExpressionHandle,
                origin: CheckedValueOrigin,
                operator_symbol: SymbolHandle,
            },
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct BoundaryQualificationAuthorization {
            pub requirement_symbol: SymbolHandle,
            pub signature_symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractProofFact {
            pub kind: ContractProofFactKind,
            pub owner: ContractProofFactOwner,
            pub fact: Handle<typed_trees::domain::ProofFact>,
            pub evidence_term: Option<Handle<CheckedEvidenceTerm>>,
            pub nominal_argument_roots: Option<arena::HandleSpan<OutcomeOccurrenceRoot>>,
            pub qualification_authorization: Option<BoundaryQualificationAuthorization>,
            pub inherited_scope: Option<Handle<InheritedContractScope>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct InheritedContractScope {
            pub conformance_machine: SymbolHandle,
            pub declaring_trait: SymbolHandle,
            pub requirement: SymbolHandle,
            pub trait_arguments: Vec<typed_trees::types::TypeReferenceHandle>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeSpecificGuaranteeFact {
            pub machine_symbol: SymbolHandle,
            pub result_data: SymbolHandle,
            pub result_case: SymbolHandle,
            pub public_selector: Option<String>,
            pub fact: Handle<typed_trees::domain::ProofFact>,
            pub fact_is_truth: bool,
            pub argument_roots: Vec<OutcomeOccurrenceRoot>,
            pub evidence_term: Option<Handle<CheckedEvidenceTerm>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeSpecificArmFact {
            pub caller_machine_symbol: SymbolHandle,
            pub caller_state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub result_call_statement_index: usize,
            pub result_call_expression: ExpressionHandle,
            pub result_call_target: SymbolHandle,
            pub result_data: SymbolHandle,
            pub result_case: SymbolHandle,
            pub result_expression: ExpressionHandle,
            pub rows: Vec<OutcomeSpecificArmRowFact>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeSpecificArmRowFact {
            pub guarantee: Handle<OutcomeSpecificGuaranteeFact>,
            pub instantiated_proposition: Option<crate::CheckedPropositionApplication>,
            pub instantiated_label: Option<String>,
            pub captured_argument_identities: Option<Vec<Option<String>>>,
            pub argument_roots: Vec<OutcomeOccurrenceRoot>,
            pub validity: OutcomeSpecificValidityFact,
            pub selected_term: Option<Handle<CheckedEvidenceTerm>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeSpecificValidityFact {
            pub result_occurrence: ExpressionHandle,
            pub referenced_occurrences: Vec<OutcomeReferencedOccurrence>,
            pub evidence_interface_scope: Option<OutcomeSpecificEvidenceInterfaceScopeFact>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeSpecificEvidenceInterfaceScopeFact {
            pub interface: crate::CheckedEvidenceInterfaceIdentity,
            pub evidence_type: typed_trees::types::TypeReferenceHandle,
            pub reference_regions: Vec<typed_trees::types::TypeReferenceHandle>,
            pub retained_occurrences: Vec<OutcomeReferencedOccurrence>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeReferencedOccurrence {
            pub occurrence: ExpressionHandle,
            pub root: OutcomeOccurrenceRoot,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum OutcomeOccurrenceRoot {
            CalleeResult,
            CalleeParameter {
                source_position: u32,
            },
            CalleeParameterProjection {
                source_position: u32,
                fields: Vec<SymbolHandle>,
            },
            ClosedValue,
            #[default]
            Other,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedEvidenceTerm {
            pub name: String,
            pub owner: ContractProofFactOwner,
            pub kind: ContractProofFactKind,
            pub lane_position: usize,
            pub proposition: crate::CheckedPropositionApplication,
            pub evidence_type: String,
            pub evidence_interface: Option<crate::CheckedEvidenceInterfaceIdentity>,
        }

        impl Default for CheckedEvidenceTerm {
            fn default() -> Self {
                Self {
                    name: String::new(),
                    owner: ContractProofFactOwner::Unknown,
                    kind: ContractProofFactKind::Requires,
                    lane_position: 0,
                    proposition: crate::CheckedPropositionApplication {
                        declaration: SymbolHandle::invalid(),
                        binder_arguments: Vec::new(),
                        arguments: Vec::new(),
                        evidence_interface: None,
                    },
                    evidence_type: String::new(),
                    evidence_interface: None,
                }
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractProofFactRef {
            pub fact: Handle<ContractProofFact>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractEvidenceArgument {
            pub source: Handle<CheckedEvidenceTerm>,
            pub parameter: Handle<CheckedEvidenceTerm>,
            pub lane_position: usize,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractExpressionEvidenceArgumentFact {
            pub source: Handle<CheckedEvidenceTerm>,
            pub parameter: Handle<CheckedEvidenceTerm>,
            pub lane_position: usize,
            pub instantiated_proposition: crate::CheckedPropositionApplication,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractExpressionEvidenceCallFact {
            pub owner: ContractProofFactOwner,
            pub fact: Handle<typed_trees::domain::ProofFact>,
            pub expression: ExpressionHandle,
            pub runtime_contract:
                Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
            pub target_machine_symbol: SymbolHandle,
            pub target_state_symbol: SymbolHandle,
            pub evidence_arguments: Vec<ContractExpressionEvidenceArgumentFact>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractExpressionStaticConformanceApplicationFact {
            pub owner: ContractProofFactOwner,
            pub fact: Handle<typed_trees::domain::ProofFact>,
            pub expression: ExpressionHandle,
            pub static_argument_position: usize,
            pub application: typed_trees::typed_trees::ClosedConformanceApplication,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum EvidenceAssignmentSource {
            Forwarded {
                term: Handle<CheckedEvidenceTerm>,
            },
            ProducerConformance {
                conformance: SymbolHandle,
                evidence_trait: SymbolHandle,
                rows: Vec<crate::DynamicConformanceRowFact>,
            },
        }

        impl Default for EvidenceAssignmentSource {
            fn default() -> Self {
                Self::Forwarded {
                    term: Handle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct EvidenceForwardingFact {
            pub machine_symbol: SymbolHandle,
            pub state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub output: Handle<CheckedEvidenceTerm>,
            pub source: EvidenceAssignmentSource,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofOutputCallFact {
            pub caller_machine_symbol: SymbolHandle,
            pub caller_state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub source_statement_index: usize,
            pub runtime_call: Option<ProofOutputRuntimeCallFact>,
            pub target_machine_symbol: SymbolHandle,
            pub target_state_symbol: SymbolHandle,
            pub static_requirement_dispatch: Option<StaticRequirementDispatchFact>,
            pub evidence_arguments: Vec<ProofOutputEvidenceArgumentFact>,
            pub outputs: Vec<ProofOutputFact>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StaticRequirementDispatchFact {
            pub selection_application:
                typed_trees::static_applications::StaticMachineApplicationHandle,
            pub evidence_parameter: SymbolHandle,
            pub application_report_fingerprint: u64,
            pub application_commitment:
                typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
            pub declaring_trait: SymbolHandle,
            pub requirement: SymbolHandle,
            pub contract_signature: SymbolHandle,
            pub realization_machine: SymbolHandle,
            pub realization_state: SymbolHandle,
        }

        impl From<&typed_trees::typed_trees::StaticRequirementDispatch> for StaticRequirementDispatchFact {
            fn from(dispatch: &typed_trees::typed_trees::StaticRequirementDispatch) -> Self {
                Self {
                    selection_application: dispatch.selection_application,
                    evidence_parameter: dispatch.evidence_parameter,
                    application_report_fingerprint: dispatch.application_report_fingerprint,
                    application_commitment: dispatch.application_commitment,
                    declaring_trait: dispatch.declaring_trait,
                    requirement: dispatch.requirement,
                    contract_signature: dispatch.contract_signature,
                    realization_machine: dispatch.realization_machine,
                    realization_state: dispatch.realization_state,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofOutputEvidenceArgumentFact {
            pub input_position: usize,
            pub callee_input: Handle<CheckedEvidenceTerm>,
            pub source: Handle<CheckedEvidenceTerm>,
            pub instantiated_proposition: crate::CheckedPropositionApplication,
            pub instantiated_label: String,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofOutputRuntimeCallFact {
            pub statement_index: usize,
            pub call_ordinal: usize,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofOutputFact {
            pub output_position: usize,
            pub callee_output: Handle<CheckedEvidenceTerm>,
            pub instantiated_proposition: crate::CheckedPropositionApplication,
            pub instantiated_label: String,
            pub output: Option<Handle<CheckedEvidenceTerm>>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CallContractSignatureSource {
            #[default]
            Invalid,
            Requirement {
                declaring_trait: SymbolHandle,
            },
            Runtime {
                contract: typed_trees::callable_values::RuntimeCallableContractHandle,
                lexical_owner: SymbolHandle,
                callee: ExpressionHandle,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CallContractSignature {
            pub caller_machine_symbol: SymbolHandle,
            pub caller_state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub source_statement: typed_trees::statement::StatementHandle,
            pub source_expression: ExpressionHandle,
            pub source_target: SymbolHandle,
            pub source: CallContractSignatureSource,
            pub signature: typed_trees::signature::StateSignature,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractCallFact {
            pub caller_machine_symbol: SymbolHandle,
            pub caller_state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
            pub target_machine_symbol: SymbolHandle,
            pub target_state_symbol: SymbolHandle,
            pub signature: Handle<CallContractSignature>,
            pub requires: HandleSpan<ContractProofFactRef>,
            pub ensures: HandleSpan<ContractProofFactRef>,
            pub evidence_arguments: HandleSpan<ContractEvidenceArgument>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractExitFact {
            pub machine_symbol: SymbolHandle,
            pub state_symbol: SymbolHandle,
            pub statement_index: usize,
            pub transition_target: typed_trees::statement::TransitionTargetHandle,
            pub ensures: HandleSpan<ContractProofFactRef>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ContractOperatorUseFact {
            pub expression: ExpressionHandle,
            pub origin: CheckedValueOrigin,
            pub operator_symbol: SymbolHandle,
            pub requires: HandleSpan<ContractProofFactRef>,
            pub ensures: HandleSpan<ContractProofFactRef>,
        }
    }
    pub(crate) mod float_meaning {
        use numerics::float_projection::FloatProjectionOperation;
        use numerics::float_semantics_catalog::FloatSemanticOperation;
        use typed_trees::types::PrimitiveType;

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub struct CheckedProofValueId(pub u32);

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub struct CheckedProofPropositionId(pub u32);

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub struct CheckedFloatProjectionInputId(pub u32);

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub struct CheckedFloatMeaningProjectionOccurrenceId(pub u32);

        pub type CheckedFloatProjectionContractIdentity =
            numerics::float_projection::FloatProjectionContractIdentity;

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub enum CheckedProofOnlyValueType {
            FloatMeaning,
        }

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub struct CheckedProofValueDeclaration {
            pub id: CheckedProofValueId,
            pub value_type: CheckedProofOnlyValueType,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedFloatProjectionInput {
            pub id: CheckedFloatProjectionInputId,
            pub primitive: PrimitiveType,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDirectMachineFloatParameter {
            pub owner_machine: symbols::SymbolHandle,
            pub parameter: symbols::SymbolHandle,
            pub fallback: CheckedFloatProjectionInput,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDirectMachineFloatResult {
            pub owner_machine: symbols::SymbolHandle,
            pub fallback: CheckedFloatProjectionInput,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDirectBlockFloatParameter {
            pub owner_machine: symbols::SymbolHandle,
            pub owner_state: symbols::SymbolHandle,
            pub parameter: symbols::SymbolHandle,
            pub fallback: CheckedFloatProjectionInput,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDirectStructuralFloatLeaf {
            pub owner_machine: symbols::SymbolHandle,
            pub field: crate::CheckedStructuralParameterField,
            pub fallback: CheckedFloatProjectionInput,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedFloatUseSite {
            pub owner_machine: symbols::SymbolHandle,
            pub owner_state: symbols::SymbolHandle,
            pub statement_index: usize,
            pub call_ordinal: usize,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDirectCallFloatResult {
            pub use_site: CheckedFloatUseSite,
            pub fallback: CheckedFloatProjectionInput,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedDirectOperationFloatResult {
            pub use_site: CheckedFloatUseSite,
            pub use_expression: typed_trees::expression::ExpressionHandle,
            pub fallback: CheckedFloatProjectionInput,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedFloatProjectionSource {
            TransitionalInput(CheckedFloatProjectionInput),
            DirectMachineParameter(CheckedDirectMachineFloatParameter),
            DirectMachineResult(CheckedDirectMachineFloatResult),
            DirectBlockParameter(CheckedDirectBlockFloatParameter),
            DirectStructuralLeaf(CheckedDirectStructuralFloatLeaf),
            DirectCallResult(CheckedDirectCallFloatResult),
            DirectOperationResult(CheckedDirectOperationFloatResult),
            ExactBinary32Literal(u32),
            ExactBinary64Literal(u64),
        }

        impl CheckedFloatProjectionSource {
            pub const fn primitive(&self) -> PrimitiveType {
                match self {
                    Self::TransitionalInput(input) => input.primitive,
                    Self::DirectMachineParameter(parameter) => parameter.fallback.primitive,
                    Self::DirectMachineResult(result) => result.fallback.primitive,
                    Self::DirectBlockParameter(parameter) => parameter.fallback.primitive,
                    Self::DirectStructuralLeaf(leaf) => leaf.fallback.primitive,
                    Self::DirectCallResult(result) => result.fallback.primitive,
                    Self::DirectOperationResult(result) => result.fallback.primitive,
                    Self::ExactBinary32Literal(_) => PrimitiveType::F32,
                    Self::ExactBinary64Literal(_) => PrimitiveType::F64,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedFloatMeaningProjection {
            pub result: CheckedProofValueDeclaration,
            pub source: CheckedFloatProjectionSource,
            pub operation: FloatProjectionOperation,
            pub contract: CheckedFloatProjectionContractIdentity,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedFloatMeaningProjectionOccurrence {
            pub id: CheckedFloatMeaningProjectionOccurrenceId,
            pub value: CheckedProofValueId,
            pub source_span: source::SourceSpan,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedFloatSemanticApplicationOperand {
            Format(semantic_vocabulary::IeeeFloatFormat),
            Meaning(CheckedProofValueId),
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedFloatSemanticApplication {
            pub result: CheckedProofValueId,
            pub contract: numerics::float_semantics_catalog::FloatSemanticContractIdentity,
            pub format: semantic_vocabulary::IeeeFloatFormat,
            pub operands: Vec<CheckedFloatSemanticApplicationOperand>,
        }

        impl CheckedFloatSemanticApplication {
            pub fn validate(&self) -> Result<(), CheckedFloatSemanticApplicationError> {
                use numerics::float_semantics_catalog::FloatSemanticValueKind;
                let row = FloatSemanticOperation::for_contract_identity(&self.contract)
                    .ok_or(CheckedFloatSemanticApplicationError::ContractIdentityMismatch)?;
                if row.result != FloatSemanticValueKind::Meaning {
                    return Err(CheckedFloatSemanticApplicationError::ResultKindMismatch);
                }
                if self.operands.len() != row.parameters.len() {
                    return Err(CheckedFloatSemanticApplicationError::OperandCountMismatch);
                }
                for (operand_index, (operand, kind)) in
                    self.operands.iter().zip(row.parameters.iter()).enumerate()
                {
                    let operand_index = u32::try_from(operand_index).unwrap_or(u32::MAX);
                    match (operand, kind) {
                        (
                            CheckedFloatSemanticApplicationOperand::Format(format),
                            FloatSemanticValueKind::Format,
                        ) if *format == self.format => {}
                        (
                            CheckedFloatSemanticApplicationOperand::Format(_),
                            FloatSemanticValueKind::Format,
                        ) => {
                            return Err(CheckedFloatSemanticApplicationError::FormatMismatch {
                                operand: operand_index,
                            });
                        }
                        (
                            CheckedFloatSemanticApplicationOperand::Meaning(value),
                            FloatSemanticValueKind::Meaning,
                        ) if value.0 < self.result.0 => {}
                        (
                            CheckedFloatSemanticApplicationOperand::Meaning(_),
                            FloatSemanticValueKind::Meaning,
                        ) => {
                            return Err(CheckedFloatSemanticApplicationError::OperandRow {
                                operand: operand_index,
                            });
                        }
                        _ => {
                            return Err(
                                CheckedFloatSemanticApplicationError::OperandKindMismatch {
                                    operand: operand_index,
                                },
                            );
                        }
                    }
                }
                Ok(())
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CheckedFloatSemanticApplicationError {
            ContractIdentityMismatch,
            ResultKindMismatch,
            OperandCountMismatch,
            OperandKindMismatch { operand: u32 },
            OperandRow { operand: u32 },
            FormatMismatch { operand: u32 },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedFloatMeaningEqualityProposition {
            pub id: CheckedProofPropositionId,
            pub left: CheckedProofValueId,
            pub right: CheckedProofValueId,
            pub source_expression: typed_trees::expression::ExpressionHandle,
            pub use_site: Option<CheckedFloatUseSite>,
        }

        impl CheckedFloatMeaningProjection {
            pub fn validate(&self) -> Result<(), CheckedFloatMeaningProjectionError> {
                if self.result.value_type != CheckedProofOnlyValueType::FloatMeaning {
                    return Err(CheckedFloatMeaningProjectionError::ResultTypeMismatch);
                }
                let expected = match self.operation {
                    FloatProjectionOperation::Meaning32 => PrimitiveType::F32,
                    FloatProjectionOperation::Meaning64 => PrimitiveType::F64,
                };
                if self.source.primitive() != expected {
                    return Err(CheckedFloatMeaningProjectionError::SourceFormatMismatch);
                }
                match &self.source {
                    CheckedFloatProjectionSource::DirectMachineParameter(parameter)
                        if !parameter.owner_machine.is_valid()
                            || !parameter.parameter.is_valid() =>
                    {
                        return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
                    }
                    CheckedFloatProjectionSource::DirectMachineResult(result)
                        if !result.owner_machine.is_valid() =>
                    {
                        return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
                    }
                    CheckedFloatProjectionSource::DirectBlockParameter(parameter)
                        if !parameter.owner_machine.is_valid()
                            || !parameter.owner_state.is_valid()
                            || !parameter.parameter.is_valid() =>
                    {
                        return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
                    }
                    CheckedFloatProjectionSource::DirectStructuralLeaf(leaf)
                        if !leaf.owner_machine.is_valid() || leaf.field.path.is_empty() =>
                    {
                        return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
                    }
                    CheckedFloatProjectionSource::DirectCallResult(result)
                        if !result.use_site.owner_machine.is_valid()
                            || !result.use_site.owner_state.is_valid() =>
                    {
                        return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
                    }
                    CheckedFloatProjectionSource::DirectOperationResult(result)
                        if !result.use_site.owner_machine.is_valid()
                            || !result.use_site.owner_state.is_valid()
                            || !result.use_expression.is_valid() =>
                    {
                        return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
                    }
                    _ => {}
                }
                if self.contract != self.operation.contract_identity() {
                    return Err(CheckedFloatMeaningProjectionError::ContractIdentityMismatch);
                }
                Ok(())
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CheckedFloatMeaningProjectionError {
            ResultTypeMismatch,
            SourceFormatMismatch,
            InvalidSourceProvenance,
            ContractIdentityMismatch,
        }
    }
    pub(crate) mod lemmas {
        use arena::Arena;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum ProofLemmaKind {
            #[default]
            IndexInBounds,
            NonEmptyHasFirst,
            WindowLength,
            WindowSubrange,
            TailLengthDecreases,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct ProofLemmaFact {
            pub kind: ProofLemmaKind,
            pub collection_symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct QuantifiedRangeFact {
            pub collection_symbol: SymbolHandle,
            pub predicate_symbol: SymbolHandle,
            pub start: QuantifiedBoundFact,
            pub end: QuantifiedBoundFact,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum QuantifiedBoundFact {
            #[default]
            Zero,
            Literal(i64),
            Length,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub struct LemmaFacts {
            pub lemmas: Arena<ProofLemmaFact>,
            pub quantified: Arena<QuantifiedRangeFact>,
        }

        impl LemmaFacts {
            pub fn with_roots(
                lemmas: Arena<ProofLemmaFact>,
                quantified: Arena<QuantifiedRangeFact>,
            ) -> Self {
                Self { lemmas, quantified }
            }
        }
    }
    pub(crate) mod mathematical {
        use proof_admission::{Declaration, TermArena};
        use symbols::SymbolHandle;
        use typed_trees::type_identity::TypeIdentityId;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedMathematicalDeclaration {
            pub symbol: SymbolHandle,
            pub name: String,
            pub is_public: bool,
            pub binders: Vec<CheckedMathematicalBinder>,
            pub parameters: Vec<CheckedMathematicalParameter>,
            pub result: String,
            pub body: CheckedMathematicalBody,
        }

        impl CheckedMathematicalDeclaration {
            pub fn level_arity(&self) -> usize {
                self.binders
                    .iter()
                    .filter(|binder| matches!(binder.kind, CheckedMathematicalBinderKind::Level))
                    .count()
            }

            pub fn is_assumption(&self) -> bool {
                matches!(self.body, CheckedMathematicalBody::Assumption)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedMathematicalBinder {
            pub name: String,
            pub kind: CheckedMathematicalBinderKind,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedMathematicalBinderKind {
            Level,
            Type { universe: Option<String> },
            Const { type_identity: TypeIdentityId },
            Value { type_identity: TypeIdentityId },
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedMathematicalParameter {
            pub name: String,
            pub relevance: language_core::BindingRelevance,
            pub type_spelling: String,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedMathematicalBody {
            Definition { term_spelling: String },
            Assumption,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedMathematicalSignature {
            pub arena: TermArena,
            pub declarations: Vec<Declaration>,
            pub authored: Vec<u32>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedSourceEvidenceCarrier {
            pub level_arity: u32,
            pub type_identity: TypeIdentityId,
            pub ty: proof_admission::TermHandle,
            pub domains: Vec<proof_admission::TermHandle>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedIndexedTheorem {
            pub level_arity: u32,
            pub machine: SymbolHandle,
            pub expression: typed_trees::expression::ExpressionHandle,
            pub ensure_ordinal: u32,
            pub evidence_source_position: u32,
            pub evidence_formal: SymbolHandle,
            pub type_identity: TypeIdentityId,
            pub term: proof_admission::TermHandle,
            pub ty: proof_admission::TermHandle,
            pub assumption_closure: Vec<u32>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedIndexedCitation {
            pub caller: SymbolHandle,
            pub theorem: SymbolHandle,
        }

        /// A formed sealed Id statement, retained separately from evidence of truth.
        /// The exact source carrier and checked kernel telescope survive erasure;
        /// publication abstracts these subjects without allocating runtime copies.
        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedIdentityFormation {
            pub level_arity: u32,
            pub expression: typed_trees::expression::ExpressionHandle,
            pub owner: SymbolHandle,
            pub type_identity: TypeIdentityId,
            pub term: proof_admission::TermHandle,
            pub ty: proof_admission::TermHandle,
            pub domains: Vec<proof_admission::TermHandle>,
            /// Source occurrences in the kernel telescope's outermost-first order.
            pub captures: Vec<(typed_trees::expression::ExpressionHandle, TypeIdentityId)>,
        }

        /// Fully formed publication plan. Checking resolves source identities and
        /// capture positions and abstracts the checked Id values once. Lowering only
        /// interns this metadata and encodes the shared mathematical judgment.
        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedIdentityFormationPlan {
            pub rows: Vec<CheckedIdentityPublication>,
            pub arena: TermArena,
            pub declarations: Vec<Declaration>,
            pub term: proof_admission::TermHandle,
            pub ty: proof_admission::TermHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedIdentityPublication {
            pub owner: CheckedIdentityOwner,
            pub source_carrier: String,
            pub subject_carriers: Vec<String>,
            pub subject_origins: Vec<(u32, u32)>,
            pub declaration: u32,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedIdentityOwner {
            Hermetic(String),
            InvocationLocal(String),
        }
    }
    pub(crate) mod obligations {
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum ProofFactKind {
            #[default]
            BoundedAssignment,
            BoundedCallArgument,
            BoundedInitializer,
            BoundedStateReturn,
            BoundedValue,
            BoundedTransitionArgument,
            GuardedTransition,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofObligationFact {
            pub kind: ProofFactKind,
            pub machine_symbol: SymbolHandle,
            pub state_symbol: SymbolHandle,
            pub owner: ProofObligationOwner,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum ProofObligationOwner {
            #[default]
            Unknown,
            MachineState {
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
            },
            MachineOwnedData {
                machine_symbol: SymbolHandle,
                data_symbol: SymbolHandle,
            },
            StateParameter {
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
            },
            StateReturn {
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
            },
            CallParameter {
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                target_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
            },
            TransitionParameter {
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                parameter_symbol: SymbolHandle,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedIntegerAssignmentDecision {
            pub atomic_resident: bool,
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub statement_index: usize,
            pub target: typed_trees::expression::ExpressionHandle,
            pub value: typed_trees::expression::ExpressionHandle,
            pub target_type: typed_trees::types::TypeReferenceHandle,
            pub minimum: numerics::bignum::BigInt,
            pub maximum: numerics::bignum::BigInt,
        }
    }
    pub(crate) mod propositions {
        use symbols::SymbolHandle;
        use typed_trees::proposition::NormalizedEvidenceInterfaceIdentity;
        use typed_trees::proposition::NormalizedPropositionApplicationIdentity;
        use typed_trees::proposition::PropositionBinderArgumentKind::Const;
        use typed_trees::proposition::PropositionBinderArgumentKind::Machine;
        use typed_trees::proposition::PropositionBinderArgumentKind::Type;
        use typed_trees::proposition::PropositionEvidenceClassification::Witness;
        use typed_trees::type_identity::TypeIdentityId;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedPropositionVocabulary {
            pub declarations: Vec<CheckedPropositionDeclaration>,
            pub applications: Vec<CheckedPropositionApplication>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedPropositionDeclaration {
            pub symbol: SymbolHandle,
            pub name: String,
            pub is_public: bool,
            pub binders: Vec<CheckedPropositionBinder>,
            pub parameter_types: Vec<String>,
            pub evidence: CheckedPropositionEvidence,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedPropositionBinder {
            pub name: String,
            pub kind: CheckedPropositionBinderKind,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedPropositionBinderKind {
            Type,
            Const { type_identity: TypeIdentityId },
            Machine,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedPropositionEvidence {
            FactOnly,
            Witness { evidence_type: String },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedPropositionApplication {
            pub declaration: SymbolHandle,
            pub binder_arguments: Vec<CheckedPropositionBinderArgument>,
            pub arguments: Vec<String>,
            pub evidence_interface: Option<CheckedEvidenceInterfaceIdentity>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedEvidenceInterfaceIdentity {
            pub trait_symbol: SymbolHandle,
            pub arguments: Vec<typed_trees::type_identity::TypeIdentityId>,
            pub requirements: Vec<CheckedEvidenceRequirementIdentity>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedEvidenceRequirementIdentity {
            pub declaring_trait: SymbolHandle,
            pub declaring_trait_arguments: Vec<String>,
            pub requirement: SymbolHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedPropositionBinderArgument {
            pub kind: CheckedPropositionBinderArgumentKind,
            pub diagnostic_label: String,
            pub evidence_projection: Option<CheckedEvidenceProjection>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedEvidenceProjection {
            pub term: arena::Handle<crate::CheckedEvidenceTerm>,
            pub declaring_trait: SymbolHandle,
            pub declaring_trait_arguments: Vec<String>,
            pub requirement: SymbolHandle,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CheckedPropositionBinderArgumentKind {
            Type,
            Const,
            Machine,
        }

        impl CheckedPropositionApplication {
            pub fn from_normalized(normalized: NormalizedPropositionApplicationIdentity) -> Self {
                let evidence_interface = match &normalized.classification {
                    typed_trees::proposition::PropositionEvidenceClassification::FactOnly => None,
                    Witness { interface, .. } => interface
                        .as_ref()
                        .map(CheckedEvidenceInterfaceIdentity::from_normalized),
                };
                CheckedPropositionApplication {
                    declaration: normalized.declaration,
                    binder_arguments: normalized
                        .binder_arguments
                        .into_iter()
                        .map(|argument| CheckedPropositionBinderArgument {
                            kind: match argument.kind {
                                Type => CheckedPropositionBinderArgumentKind::Type,
                                Const => CheckedPropositionBinderArgumentKind::Const,
                                Machine => CheckedPropositionBinderArgumentKind::Machine,
                            },
                            diagnostic_label: argument.identity,
                            evidence_projection: None,
                        })
                        .collect(),
                    arguments: normalized.arguments,
                    evidence_interface,
                }
            }
        }
        impl CheckedEvidenceInterfaceIdentity {
            pub fn from_normalized(interface: &NormalizedEvidenceInterfaceIdentity) -> Self {
                CheckedEvidenceInterfaceIdentity {
                    trait_symbol: interface.trait_symbol,
                    arguments: interface.arguments.clone(),
                    requirements: interface
                        .requirements
                        .iter()
                        .map(|requirement| CheckedEvidenceRequirementIdentity {
                            declaring_trait: requirement.declaring_trait,
                            declaring_trait_arguments: requirement
                                .declaring_trait_arguments
                                .clone(),
                            requirement: requirement.requirement,
                        })
                        .collect(),
                }
            }
        }
    }
    pub(crate) mod roots {
        use arena::{Arena, Handle};

        use symbols::SymbolHandle;

        use crate::keyed_rows::KeyedRows;
        use crate::{
            CallContractSignature, CheckedEvidenceTerm, ContractCallFact, ContractEvidenceArgument,
            ContractExitFact, ContractExpressionEvidenceCallFact,
            ContractExpressionStaticConformanceApplicationFact, ContractOperatorUseFact,
            ContractProofFact, ContractProofFactRef, EvidenceForwardingFact,
            InheritedContractScope, OutcomeSpecificArmFact, OutcomeSpecificGuaranteeFact,
            ProofObligationFact, ProofOutputCallFact,
        };

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProofFacts {
            pub obligations: Arena<ProofObligationFact>,
            pub integer_assignments: Arena<crate::CheckedIntegerAssignmentDecision>,
            pub contract_facts: Arena<ContractProofFact>,
            pub nominal_argument_roots: Arena<crate::OutcomeOccurrenceRoot>,
            pub inherited_contract_scopes: Arena<InheritedContractScope>,
            pub outcome_specific_guarantees: Arena<OutcomeSpecificGuaranteeFact>,
            pub outcome_specific_arms: Arena<OutcomeSpecificArmFact>,
            pub evidence_terms: Arena<CheckedEvidenceTerm>,
            pub contract_evidence_arguments: Arena<ContractEvidenceArgument>,
            pub evidence_forwardings: Arena<EvidenceForwardingFact>,
            pub proof_output_calls: Arena<ProofOutputCallFact>,
            pub contract_fact_refs: Arena<ContractProofFactRef>,
            pub contract_calls: Arena<ContractCallFact>,
            pub call_contract_signatures: Arena<CallContractSignature>,
            pub contract_expression_evidence_calls: Vec<ContractExpressionEvidenceCallFact>,
            pub contract_expression_static_conformance_applications:
                Vec<ContractExpressionStaticConformanceApplicationFact>,
            pub contract_exits: Arena<ContractExitFact>,
            pub contract_operator_uses: Arena<ContractOperatorUseFact>,
            pub float_meaning_projections: Vec<crate::CheckedFloatMeaningProjection>,
            pub float_meaning_projection_occurrences:
                Vec<crate::CheckedFloatMeaningProjectionOccurrence>,
            pub float_meaning_equalities: Vec<crate::CheckedFloatMeaningEqualityProposition>,
            pub float_semantic_applications: Vec<crate::CheckedFloatSemanticApplication>,
            pub proposition_vocabulary: crate::CheckedPropositionVocabulary,
            pub mathematical_declarations: Vec<crate::CheckedMathematicalDeclaration>,
            pub mathematical_signature: Option<crate::CheckedMathematicalSignature>,
            /// Typed Id formation supplies no identity witness or executable Boolean.
            pub identity_formations: Vec<crate::CheckedIdentityFormation>,
            pub identity_formation_plan: crate::CheckedIdentityFormationPlan,
            /// Independently retained source carrier formation, sharing that signature.
            pub source_evidence_carriers: Vec<crate::CheckedSourceEvidenceCarrier>,
            pub mathematical_safety_assumptions: Vec<u32>,
            pub indexed_theorems: Vec<crate::CheckedIndexedTheorem>,
            pub indexed_citations: Vec<crate::CheckedIndexedCitation>,
            pub call_index: KeyedRows<(SymbolHandle, SymbolHandle, usize, usize), ContractCallFact>,
            pub exit_index: KeyedRows<
                (
                    SymbolHandle,
                    SymbolHandle,
                    typed_trees::statement::TransitionTargetHandle,
                ),
                ContractExitFact,
            >,
            pub outcome_arm_index:
                KeyedRows<(SymbolHandle, SymbolHandle, usize), OutcomeSpecificArmFact>,
            pub proof_output_index:
                KeyedRows<(SymbolHandle, SymbolHandle, usize), ProofOutputCallFact>,
        }

        impl ProofFacts {
            pub fn state_requires_fact(
                &self,
                program: &typed_trees::TypedTrees,
                machine: SymbolHandle,
                state: SymbolHandle,
                fact: Handle<typed_trees::domain::ProofFact>,
            ) -> Option<&ContractProofFact> {
                let (_, declaration) = program.machine_state(machine, state)?;
                self.declared_contract_fact(
                    program,
                    program.state_contracts(declaration),
                    crate::ContractProofFactOwner::MachineState {
                        machine_symbol: machine,
                        state_symbol: state,
                    },
                    crate::ContractProofFactKind::Requires,
                    fact,
                )
            }

            pub fn machine_requires_fact(
                &self,
                program: &typed_trees::TypedTrees,
                machine: SymbolHandle,
                fact: Handle<typed_trees::domain::ProofFact>,
            ) -> Option<&ContractProofFact> {
                let declaration = program.machine_by_symbol(machine)?;
                self.declared_contract_fact(
                    program,
                    program.machine_contracts(declaration),
                    crate::ContractProofFactOwner::Machine {
                        machine_symbol: machine,
                    },
                    crate::ContractProofFactKind::Requires,
                    fact,
                )
            }

            pub fn machine_ensures_fact(
                &self,
                program: &typed_trees::TypedTrees,
                machine: SymbolHandle,
                fact: Handle<typed_trees::domain::ProofFact>,
            ) -> Option<&ContractProofFact> {
                let declaration = program.machine_by_symbol(machine)?;
                self.declared_contract_fact(
                    program,
                    program.machine_contracts(declaration),
                    crate::ContractProofFactOwner::Machine {
                        machine_symbol: machine,
                    },
                    crate::ContractProofFactKind::Ensures,
                    fact,
                )
            }

            fn declared_contract_fact(
                &self,
                program: &typed_trees::TypedTrees,
                contracts: &[typed_trees::signature::SignatureContract],
                owner: crate::ContractProofFactOwner,
                kind: crate::ContractProofFactKind,
                fact: Handle<typed_trees::domain::ProofFact>,
            ) -> Option<&ContractProofFact> {
                if !program.proof_facts.is_valid(fact) {
                    return None;
                }
                let signature_kind = match kind {
                    crate::ContractProofFactKind::Requires => {
                        typed_trees::signature::SignatureContractKind::Requires
                    }
                    crate::ContractProofFactKind::Ensures => {
                        typed_trees::signature::SignatureContractKind::Ensures
                    }
                };
                let mut authored = None;
                let mut lane_position = 0usize;
                for contract in contracts
                    .iter()
                    .filter(|contract| contract.kind == signature_kind)
                {
                    for candidate in contract.facts.handles() {
                        if candidate == fact {
                            if authored.is_some() {
                                return None;
                            }
                            authored = Some((contract, lane_position));
                        }
                        if contract.binding.is_some() {
                            lane_position = lane_position.checked_add(1)?;
                        }
                    }
                }
                let (contract, expected_lane) = authored?;
                let mut recorded = self
                    .contract_facts
                    .iter()
                    .map(|(_, row)| row)
                    .filter(|row| row.owner == owner && row.kind == kind && row.fact == fact);
                let row = recorded.next()?;
                if recorded.next().is_some() {
                    return None;
                }
                if let Some(binding) = &contract.binding {
                    let term = row.evidence_term?;
                    if !self.evidence_terms.is_valid(term) {
                        return None;
                    }
                    let term = self.evidence_terms.get(term);
                    let proposition = program.proof_facts.get(fact).as_proposition()?;
                    let normalized =
                        program.normalize_nominal_proposition_application(proposition, None)?;
                    let expected =
                        crate::CheckedPropositionApplication::from_normalized(normalized);
                    if term.owner != owner
                        || term.kind != kind
                        || term.lane_position != expected_lane
                        || term.name != binding.as_str()
                        || term.proposition != expected
                        || term.evidence_interface != expected.evidence_interface
                        || term.evidence_interface.is_none()
                    {
                        return None;
                    }
                } else if row.evidence_term.is_some() {
                    return None;
                }
                Some(row)
            }

            pub fn contract_call_at(
                &self,
                key: &(SymbolHandle, SymbolHandle, usize, usize),
            ) -> Option<&ContractCallFact> {
                let handle = self.call_index.with_rows(
                    &self.contract_calls,
                    |call| {
                        (
                            call.caller_machine_symbol,
                            call.caller_state_symbol,
                            call.statement_index,
                            call.call_ordinal,
                        )
                    },
                    key,
                    |rows| rows.last().copied(),
                )?;
                Some(self.contract_calls.get(handle))
            }

            pub fn unique_contract_call_at(
                &self,
                key: &(SymbolHandle, SymbolHandle, usize, usize),
            ) -> Option<&ContractCallFact> {
                let handle = self.call_index.with_rows(
                    &self.contract_calls,
                    |call| {
                        (
                            call.caller_machine_symbol,
                            call.caller_state_symbol,
                            call.statement_index,
                            call.call_ordinal,
                        )
                    },
                    key,
                    |rows| match rows {
                        [single] => Some(*single),
                        _ => None,
                    },
                )?;
                Some(self.contract_calls.get(handle))
            }

            pub fn contract_exits_at(
                &self,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                transition_target: typed_trees::statement::TransitionTargetHandle,
            ) -> Vec<Handle<ContractExitFact>> {
                self.exit_index.with_rows(
                    &self.contract_exits,
                    |exit| {
                        (
                            exit.machine_symbol,
                            exit.state_symbol,
                            exit.transition_target,
                        )
                    },
                    &(machine_symbol, state_symbol, transition_target),
                    |rows| rows.to_vec(),
                )
            }

            pub fn has_outcome_specific_arm(
                &self,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                statement_index: usize,
            ) -> bool {
                self.outcome_arm_index.with_rows(
                    &self.outcome_specific_arms,
                    |arm| {
                        (
                            arm.caller_machine_symbol,
                            arm.caller_state_symbol,
                            arm.statement_index,
                        )
                    },
                    &(machine_symbol, state_symbol, statement_index),
                    |rows| !rows.is_empty(),
                )
            }

            pub fn has_proof_only_output_call(
                &self,
                machine_symbol: SymbolHandle,
                state_symbol: SymbolHandle,
                statement_index: usize,
            ) -> bool {
                self.proof_output_index.with_rows(
                    &self.proof_output_calls,
                    |invocation| {
                        (
                            invocation.caller_machine_symbol,
                            invocation.caller_state_symbol,
                            invocation.statement_index,
                        )
                    },
                    &(machine_symbol, state_symbol, statement_index),
                    |rows| {
                        rows.iter().any(|handle| {
                            self.proof_output_calls.get(*handle).runtime_call.is_none()
                        })
                    },
                )
            }
        }

        impl ProofFacts {
            pub fn integer_assignment_at(
                &self,
                machine: SymbolHandle,
                state: SymbolHandle,
                statement_index: usize,
                target: typed_trees::expression::ExpressionHandle,
                value: typed_trees::expression::ExpressionHandle,
            ) -> Handle<crate::CheckedIntegerAssignmentDecision> {
                let mut matches = self.integer_assignments.iter().filter(|(_, decision)| {
                    decision.machine == machine
                        && decision.state == state
                        && decision.statement_index == statement_index
                        && decision.target == target
                        && decision.value == value
                });
                let Some((handle, _)) = matches.next() else {
                    return Handle::invalid();
                };
                if matches.next().is_some() {
                    Handle::invalid()
                } else {
                    handle
                }
            }
        }
    }

    pub use contracts::{
        BoundaryQualificationAuthorization, CallContractSignature, CallContractSignatureSource,
        CheckedEvidenceTerm, ContractCallFact, ContractEvidenceArgument, ContractExitFact,
        ContractExpressionEvidenceArgumentFact, ContractExpressionEvidenceCallFact,
        ContractExpressionStaticConformanceApplicationFact, ContractOperatorUseFact,
        ContractProofFact, ContractProofFactKind, ContractProofFactOwner, ContractProofFactRef,
        EvidenceAssignmentSource, EvidenceForwardingFact, InheritedContractScope,
        OutcomeOccurrenceRoot, OutcomeReferencedOccurrence, OutcomeSpecificArmFact,
        OutcomeSpecificArmRowFact, OutcomeSpecificEvidenceInterfaceScopeFact,
        OutcomeSpecificGuaranteeFact, OutcomeSpecificValidityFact, ProofOutputCallFact,
        ProofOutputEvidenceArgumentFact, ProofOutputFact, ProofOutputRuntimeCallFact,
        StaticRequirementDispatchFact,
    };
    pub use float_meaning::{
        CheckedDirectBlockFloatParameter, CheckedDirectCallFloatResult,
        CheckedDirectMachineFloatParameter, CheckedDirectMachineFloatResult,
        CheckedDirectOperationFloatResult, CheckedDirectStructuralFloatLeaf,
        CheckedFloatMeaningEqualityProposition, CheckedFloatMeaningProjection,
        CheckedFloatMeaningProjectionError, CheckedFloatMeaningProjectionOccurrence,
        CheckedFloatMeaningProjectionOccurrenceId, CheckedFloatProjectionContractIdentity,
        CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
        CheckedFloatSemanticApplication, CheckedFloatSemanticApplicationError,
        CheckedFloatSemanticApplicationOperand, CheckedFloatUseSite, CheckedProofOnlyValueType,
        CheckedProofPropositionId, CheckedProofValueDeclaration, CheckedProofValueId,
    };
    pub use lemmas::{
        LemmaFacts, ProofLemmaFact, ProofLemmaKind, QuantifiedBoundFact, QuantifiedRangeFact,
    };
    pub use mathematical::{
        CheckedIdentityFormation, CheckedIdentityFormationPlan, CheckedIdentityOwner,
        CheckedIdentityPublication, CheckedIndexedCitation, CheckedIndexedTheorem,
        CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
        CheckedMathematicalDeclaration, CheckedMathematicalParameter, CheckedMathematicalSignature,
        CheckedSourceEvidenceCarrier,
    };
    pub use obligations::{
        CheckedIntegerAssignmentDecision, ProofFactKind, ProofObligationFact, ProofObligationOwner,
    };
    pub use propositions::{
        CheckedEvidenceInterfaceIdentity, CheckedEvidenceProjection,
        CheckedEvidenceRequirementIdentity, CheckedPropositionApplication,
        CheckedPropositionBinder, CheckedPropositionBinderArgument,
        CheckedPropositionBinderArgumentKind, CheckedPropositionBinderKind,
        CheckedPropositionDeclaration, CheckedPropositionEvidence, CheckedPropositionVocabulary,
    };
    pub use roots::ProofFacts;
}
