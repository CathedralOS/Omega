//! Non-authoritative `RA`/`RR` derivation for direct terminal quotient requests.
//!
//! The plan retains exact quotient TYPE identity as well as relation symbol so
//! two quotients over one carrier cannot collapse. It grants no execution
//! authority and deliberately refuses nested/adapted result flow.

use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{TypeParameter, TypeParameterKind};
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{
    ExpressionHandle, ExpressionNode, QuotientOperationKind, QuotientOperationRequest,
    StaticMachineArgument, TableCallExpression,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind};
use psi_typed_trees::state::State;
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactQuotientRelation {
    pub(super) quotient_type: TypeReferenceHandle,
    pub(super) quotient_symbol: SymbolHandle,
    pub(super) relation_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputRelation {
    Quotient(ExactQuotientRelation),
    /// Non-quotient operands remain part of the pointwise relation through
    /// exact equality. They must never disappear into an implicit `true`.
    ExactEquality(TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectTerminalRelationPlan {
    /// One entry per authored runtime argument. Quotient positions use their
    /// exact selected relation; ordinary positions use exact typed equality.
    pub(super) input_relations: Vec<InputRelation>,
    pub(super) result_relation: ExactQuotientRelation,
    pub(super) representative: RepresentativeTelescope,
    pub(super) define_correspondence: Option<DefineRuntimeCorrespondence>,
    pub(super) public_precondition: Option<RepresentativePreconditionPartition>,
    pub(super) representative_precondition: Option<RepresentativePreconditionPartition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ImmutableAliasFallthroughRoot {
    pub(super) request_expression: ExpressionHandle,
    pub(super) alias_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepresentativeRuntimeParameter {
    pub(super) symbol: SymbolHandle,
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) is_mutable: bool,
    pub(super) is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepresentativeTelescope {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
    pub(super) parameters: Vec<RepresentativeRuntimeParameter>,
    pub(super) return_type: TypeReferenceHandle,
    pub(super) machine_contracts: HandleSpan<SignatureContract>,
    pub(super) state_contracts: HandleSpan<SignatureContract>,
    pub(super) static_application: RepresentativeStaticApplication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DefineRuntimePosition {
    pub(super) public_parameter: SymbolHandle,
    pub(super) representative_parameter: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DefineRuntimeCorrespondence {
    pub(super) positions: Vec<DefineRuntimePosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepresentativeContractOwner {
    Machine,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepresentativeContractFactLocation {
    pub(super) owner: RepresentativeContractOwner,
    pub(super) contract_position: usize,
    pub(super) fact_position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepresentativePreconditionPartition {
    /// Exact `requires` facts whose expression depends on at least one
    /// quotient-bearing representative position. This is the future `P`
    /// surface; retaining it proves no implication or invariance law.
    pub(super) dependent: Vec<RepresentativeContractFactLocation>,
    /// Exact `requires` facts independent of quotient-bearing positions.
    pub(super) fixed: Vec<RepresentativeContractFactLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepresentativeStaticBindingKind {
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepresentativeStaticBinding {
    pub(super) parameter: SymbolHandle,
    pub(super) kind: RepresentativeStaticBindingKind,
    pub(super) argument: StaticMachineArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepresentativeStaticApplication {
    pub(super) lifetime_arguments: Vec<Identifier>,
    pub(super) bindings: Vec<RepresentativeStaticBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RelationPlanError {
    UnresolvedArgumentType(usize),
    UnresolvedInputRelationApplication(usize),
    ResultIsNotQuotient,
    UnresolvedResultRelationApplication,
    RepresentativeEntryDoesNotResolveExactly,
    RepresentativeResultTypeIsUnresolved,
    RepresentativeStaticArityMismatch,
    RepresentativeStaticArgumentCategoryMismatch(usize),
    RepresentativeStaticArgumentIsOpen(usize),
    RepresentativeLifetimeApplicationRequiresElision,
    RepresentativePropositionApplicationUnsupported(usize),
    DefineOwnerRequiresSubstitution,
    DefineRuntimeArityMismatch,
    DefineParameterIdentityNotUnique,
    DefineArgumentIsNotPublicParameter(usize),
    DefineArgumentOrderMismatch(usize),
    DefineParameterModeMismatch(usize),
    DefineParameterTypeMismatch(usize),
    DefineResultTypeMismatch,
    PreconditionDependencyUnresolved,
}

impl fmt::Display for RelationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedArgumentType(position) => write!(
                formatter,
                "argument {position} has no exact declared type; adapted lift arguments require later expression typing"
            ),
            Self::UnresolvedInputRelationApplication(position) => write!(
                formatter,
                "argument {position}'s quotient relation has an open binder application that requires the representative-operation telescope"
            ),
            Self::ResultIsNotQuotient => formatter
                .write_str("the enclosing state's exact result type is not a formed quotient"),
            Self::UnresolvedResultRelationApplication => formatter.write_str(
                "the result quotient relation has an open binder application that requires the representative-operation result telescope",
            ),
            Self::RepresentativeEntryDoesNotResolveExactly => formatter.write_str(
                "the retained representative entry symbol does not resolve to exactly one machine state",
            ),
            Self::RepresentativeResultTypeIsUnresolved => formatter.write_str(
                "the representative operation has no exact result type",
            ),
            Self::RepresentativeStaticArityMismatch => formatter.write_str(
                "the representative static application does not exactly match its declaration parameter arity",
            ),
            Self::RepresentativeStaticArgumentCategoryMismatch(position) => write!(
                formatter,
                "representative static argument {position} has the wrong declaration category"
            ),
            Self::RepresentativeStaticArgumentIsOpen(position) => write!(
                formatter,
                "representative static argument {position} is not one closed application"
            ),
            Self::RepresentativeLifetimeApplicationRequiresElision => formatter.write_str(
                "representative lifetime arguments require the ordinary call-site elision judgment",
            ),
            Self::RepresentativePropositionApplicationUnsupported(position) => write!(
                formatter,
                "representative proposition argument {position} has no closed application boundary yet"
            ),
            Self::DefineOwnerRequiresSubstitution => formatter.write_str(
                "the quotient-facing definition is generic and requires exact owner-telescope substitution",
            ),
            Self::DefineRuntimeArityMismatch => formatter.write_str(
                "the public, authored-call, and representative runtime telescopes have different arity",
            ),
            Self::DefineParameterIdentityNotUnique => formatter.write_str(
                "the public or representative runtime telescope repeats one parameter identity",
            ),
            Self::DefineArgumentIsNotPublicParameter(position) => write!(
                formatter,
                "define argument {position} is not one exact direct public parameter"
            ),
            Self::DefineArgumentOrderMismatch(position) => write!(
                formatter,
                "define argument {position} does not name the public parameter at the same position"
            ),
            Self::DefineParameterModeMismatch(position) => write!(
                formatter,
                "define parameter {position} changes mutable/borrow mode"
            ),
            Self::DefineParameterTypeMismatch(position) => write!(
                formatter,
                "define parameter {position} does not map its exact quotient carrier or ordinary type to the representative parameter"
            ),
            Self::DefineResultTypeMismatch => formatter.write_str(
                "the exact quotient result carrier does not match the representative result",
            ),
            Self::PreconditionDependencyUnresolved => formatter.write_str(
                "a quotient-facing or representative precondition contains an unresolved value identity and cannot be partitioned by quotient-bearing position",
            ),
        }
    }
}

pub(super) fn derive_direct_terminal_plan(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    request: &QuotientOperationRequest,
) -> Result<DirectTerminalRelationPlan, RelationPlanError> {
    let mut input_relations = Vec::new();
    for (position, argument) in program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        let argument_type =
            crate::places::declared_place_type_raw(program, machine, Some(state), *argument)
                .ok_or(RelationPlanError::UnresolvedArgumentType(position))?;
        input_relations.push(match exact_quotient_relation(program, argument_type) {
            ExactRelationLookup::NotQuotient => InputRelation::ExactEquality(argument_type),
            ExactRelationLookup::Exact(relation) => InputRelation::Quotient(relation),
            ExactRelationLookup::OpenApplication => {
                return Err(RelationPlanError::UnresolvedInputRelationApplication(
                    position,
                ));
            }
        });
    }
    let result_relation = match exact_quotient_relation(program, state.return_type) {
        ExactRelationLookup::NotQuotient => return Err(RelationPlanError::ResultIsNotQuotient),
        ExactRelationLookup::Exact(relation) => relation,
        ExactRelationLookup::OpenApplication => {
            return Err(RelationPlanError::UnresolvedResultRelationApplication);
        }
    };
    let representative = derive_representative_telescope(program, request)?;
    let define_correspondence = (request.kind == QuotientOperationKind::Define)
        .then(|| {
            derive_define_runtime_correspondence(
                program,
                machine,
                state,
                call,
                &input_relations,
                result_relation,
                &representative,
            )
        })
        .transpose()?;
    let representative_precondition = define_correspondence
        .as_ref()
        .map(|_| {
            derive_representative_precondition_partition(program, &input_relations, &representative)
        })
        .transpose()?;
    let public_precondition = define_correspondence
        .as_ref()
        .map(|_| derive_public_precondition_partition(program, machine, state, &input_relations))
        .transpose()?;
    Ok(DirectTerminalRelationPlan {
        input_relations,
        result_relation,
        representative,
        define_correspondence,
        public_precondition,
        representative_precondition,
    })
}

/// Recognize only the straight-line immutable alias form of one unchanged
/// state-fallthrough result. This deliberately excludes transitions,
/// assignments, side statements, mutable locals, and type drift.
pub(super) fn immutable_alias_fallthrough_root(
    program: &TypedTrees,
    state: &State,
) -> Option<ImmutableAliasFallthroughRoot> {
    if !state.return_type.is_valid() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let (psi_typed_trees::statement::StatementNode::Expression(result), prefix) =
        statements.split_last()?
    else {
        return None;
    };
    let mut expected_symbol = exact_local_name_symbol(program, *result)?;
    let mut seen = Vec::new();
    for (position, statement) in prefix.iter().enumerate().rev() {
        let psi_typed_trees::statement::StatementNode::LocalData(local) = statement else {
            return None;
        };
        if local.is_mutable
            || local.symbol != expected_symbol
            || seen.contains(&local.symbol)
            || !local.type_reference.is_valid()
            || program.normalized_type_identity(local.type_reference)
                != program.normalized_type_identity(state.return_type)
        {
            return None;
        }
        seen.push(local.symbol);
        match program.expression_table.expression(local.initial_value) {
            ExpressionNode::Call(call) if call.quotient_operation.is_some() && position == 0 => {
                return Some(ImmutableAliasFallthroughRoot {
                    request_expression: local.initial_value,
                    alias_count: seen.len(),
                });
            }
            ExpressionNode::Name(_) => {
                expected_symbol = exact_local_name_symbol(program, local.initial_value)?;
                if seen.contains(&expected_symbol) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    None
}

fn exact_local_name_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (path.symbol.is_valid()
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1)
        .then_some(path.symbol)
}

fn derive_public_precondition_partition(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    input_relations: &[InputRelation],
) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
    let public_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .collect::<Vec<_>>();
    if public_parameters.len() != input_relations.len() {
        return Err(RelationPlanError::DefineRuntimeArityMismatch);
    }
    let varying_parameters = input_relations
        .iter()
        .zip(public_parameters)
        .filter_map(|(relation, parameter)| {
            matches!(relation, InputRelation::Quotient(_)).then_some(parameter.symbol)
        })
        .collect::<Vec<_>>();
    derive_precondition_partition(
        program,
        machine.contracts,
        state.contracts,
        &varying_parameters,
    )
}

fn derive_representative_precondition_partition(
    program: &TypedTrees,
    input_relations: &[InputRelation],
    representative: &RepresentativeTelescope,
) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
    let varying_parameters = input_relations
        .iter()
        .zip(&representative.parameters)
        .filter_map(|(relation, parameter)| {
            matches!(relation, InputRelation::Quotient(_)).then_some(parameter.symbol)
        })
        .collect::<Vec<_>>();
    derive_precondition_partition(
        program,
        representative.machine_contracts,
        representative.state_contracts,
        &varying_parameters,
    )
}

fn derive_precondition_partition(
    program: &TypedTrees,
    machine_contracts: HandleSpan<SignatureContract>,
    state_contracts: HandleSpan<SignatureContract>,
    varying_parameters: &[SymbolHandle],
) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
    let mut partition = RepresentativePreconditionPartition {
        dependent: Vec::new(),
        fixed: Vec::new(),
    };
    for (owner, contracts) in [
        (
            RepresentativeContractOwner::Machine,
            program.signature_contracts.span_or_empty(machine_contracts),
        ),
        (
            RepresentativeContractOwner::State,
            program.signature_contracts.span_or_empty(state_contracts),
        ),
    ] {
        for (contract_position, contract) in contracts.iter().enumerate() {
            if contract.kind != SignatureContractKind::Requires {
                continue;
            }
            for (fact_position, fact) in program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .enumerate()
            {
                let location = RepresentativeContractFactLocation {
                    owner,
                    contract_position,
                    fact_position,
                };
                if proof_fact_depends_on_any(program, fact, varying_parameters)? {
                    partition.dependent.push(location);
                } else {
                    partition.fixed.push(location);
                }
            }
        }
    }
    Ok(partition)
}

fn proof_fact_depends_on_any(
    program: &TypedTrees,
    fact: &ProofFact,
    parameters: &[SymbolHandle],
) -> Result<bool, RelationPlanError> {
    match fact {
        ProofFact::Expression(expression) => {
            expression_depends_on_any(program, *expression, parameters)
        }
        ProofFact::Membership(membership) => {
            expression_depends_on_any(program, membership.value, parameters)
        }
        ProofFact::Proposition(application) => program
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .try_fold(false, |depends, expression| {
                let expression_depends =
                    expression_depends_on_any(program, *expression, parameters)?;
                Ok(depends || expression_depends)
            }),
    }
}

fn expression_depends_on_any(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[SymbolHandle],
) -> Result<bool, RelationPlanError> {
    let depends = |expression| expression_depends_on_any(program, expression, parameters);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            let value = depends(atomic.value)?;
            let result = if atomic.result.is_valid() {
                depends(atomic.result)?
            } else {
                false
            };
            Ok(value || result)
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .try_fold(false, |found, expression| {
                let expression_depends = depends(*expression)?;
                Ok(found || expression_depends)
            }),
        ExpressionNode::Binary(binary) => {
            let left = depends(binary.left)?;
            let right = depends(binary.right)?;
            Ok(left || right)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => Ok(false),
        ExpressionNode::Cast(cast) => depends(cast.value),
        ExpressionNode::Call(call) => {
            let receiver_depends = if call.receiver.is_valid() {
                depends(call.receiver)?
            } else {
                false
            };
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .try_fold(receiver_depends, |found, expression| {
                    let expression_depends = depends(*expression)?;
                    Ok(found || expression_depends)
                })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = depends(indexed.collection)?;
            let index = depends(indexed.index)?;
            Ok(collection || index)
        }
        ExpressionNode::Member(member) => depends(member.receiver),
        ExpressionNode::Mutable(inner) => depends(*inner),
        ExpressionNode::Unary(unary) => depends(unary.operand),
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid() && !path.head_symbol.is_valid() {
                return Err(RelationPlanError::PreconditionDependencyUnresolved);
            }
            Ok(parameters.contains(&path.symbol) || parameters.contains(&path.head_symbol))
        }
        ExpressionNode::Range(range) => {
            let start = if range.start.is_valid() {
                depends(range.start)?
            } else {
                false
            };
            let end = if range.end.is_valid() {
                depends(range.end)?
            } else {
                false
            };
            Ok(start || end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .try_fold(false, |found, field| {
                let field_depends = depends(field.value)?;
                Ok(found || field_depends)
            }),
    }
}

fn derive_define_runtime_correspondence(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    input_relations: &[InputRelation],
    result_relation: ExactQuotientRelation,
    representative: &RepresentativeTelescope,
) -> Result<DefineRuntimeCorrespondence, RelationPlanError> {
    if !program.machine_type_parameters(machine).is_empty() {
        return Err(RelationPlanError::DefineOwnerRequiresSubstitution);
    }
    let public_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .collect::<Vec<_>>();
    let arguments = program.expression_table.expression_handles(call.arguments);
    if public_parameters.len() != arguments.len()
        || arguments.len() != representative.parameters.len()
        || input_relations.len() != arguments.len()
    {
        return Err(RelationPlanError::DefineRuntimeArityMismatch);
    }
    if has_duplicate_parameter_symbols(public_parameters.iter().map(|parameter| parameter.symbol))
        || has_duplicate_parameter_symbols(
            representative
                .parameters
                .iter()
                .map(|parameter| parameter.symbol),
        )
    {
        return Err(RelationPlanError::DefineParameterIdentityNotUnique);
    }

    let mut positions = Vec::with_capacity(arguments.len());
    for (position, (((public, argument), relation), representative_parameter)) in public_parameters
        .iter()
        .zip(arguments)
        .zip(input_relations)
        .zip(&representative.parameters)
        .enumerate()
    {
        let argument_symbol = direct_public_parameter_symbol(program, *argument).ok_or(
            RelationPlanError::DefineArgumentIsNotPublicParameter(position),
        )?;
        if argument_symbol != public.symbol {
            return Err(RelationPlanError::DefineArgumentOrderMismatch(position));
        }
        if public.is_mutable != representative_parameter.is_mutable {
            return Err(RelationPlanError::DefineParameterModeMismatch(position));
        }
        if !input_relation_matches_public_type(program, *relation, public.type_reference)
            || !input_relation_matches_representative_type(
                program,
                *relation,
                representative_parameter.type_reference,
                &representative.static_application.bindings,
            )
        {
            return Err(RelationPlanError::DefineParameterTypeMismatch(position));
        }
        positions.push(DefineRuntimePosition {
            public_parameter: public.symbol,
            representative_parameter: representative_parameter.symbol,
        });
    }
    if !quotient_carrier_matches_type(
        program,
        result_relation,
        representative.return_type,
        &representative.static_application.bindings,
    ) {
        return Err(RelationPlanError::DefineResultTypeMismatch);
    }
    Ok(DefineRuntimeCorrespondence { positions })
}

fn has_duplicate_parameter_symbols(symbols: impl IntoIterator<Item = SymbolHandle>) -> bool {
    let mut seen = Vec::new();
    for symbol in symbols {
        if seen.contains(&symbol) {
            return true;
        }
        seen.push(symbol);
    }
    false
}

fn direct_public_parameter_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => *inner,
        _ => expression,
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1
        && path.symbol.is_valid())
    .then_some(path.symbol)
}

fn input_relation_matches_representative_type(
    program: &TypedTrees,
    relation: InputRelation,
    representative_type: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    match relation {
        InputRelation::ExactEquality(public_type) => {
            substituted_type_matches(program, representative_type, public_type, substitutions)
        }
        InputRelation::Quotient(relation) => {
            quotient_carrier_matches_type(program, relation, representative_type, substitutions)
        }
    }
}

fn input_relation_matches_public_type(
    program: &TypedTrees,
    relation: InputRelation,
    public_type: TypeReferenceHandle,
) -> bool {
    let relation_type = match relation {
        InputRelation::Quotient(relation) => relation.quotient_type,
        InputRelation::ExactEquality(type_reference) => type_reference,
    };
    program.normalized_type_identity(relation_type) == program.normalized_type_identity(public_type)
}

fn quotient_carrier_matches_type(
    program: &TypedTrees,
    relation: ExactQuotientRelation,
    representative_type: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    if !matches!(
        program
            .type_reference_table
            .type_reference(relation.quotient_type),
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
    ) {
        // Borrow/reference carrier substitution needs an exact shell-preserving
        // rewrite; do not erase that mode by unwrapping here.
        return false;
    }
    let Some(quotient) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == relation.quotient_symbol)
    else {
        return false;
    };
    let Some(metadata) = quotient.quotient.as_ref() else {
        return false;
    };
    let Some(carrier_symbol) = super::base_data_symbol(program, metadata.carrier) else {
        return false;
    };
    let Some(carrier) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == carrier_symbol)
    else {
        return false;
    };
    quotient.properties.multiplicity == carrier.properties.multiplicity
        && substituted_type_matches(
            program,
            representative_type,
            metadata.carrier,
            substitutions,
        )
}

/// Compare one declaration type against a concrete use without mutating the
/// checked type arena. Only the exact, closed static application retained on
/// the representative telescope may replace a declaration binder.
fn substituted_type_matches(
    program: &TypedTrees,
    template: TypeReferenceHandle,
    concrete: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    let template_node = program.type_reference_table.type_reference(template);
    let concrete_node = program.type_reference_table.type_reference(concrete);
    match (template_node, concrete_node) {
        (
            TypeReferenceNode::Named { symbol, .. },
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
        ) => substitutions
            .iter()
            .find(|binding| binding.parameter == *symbol)
            .map_or_else(
                || {
                    program.normalized_type_identity(template)
                        == program.normalized_type_identity(concrete)
                },
                |binding| {
                    binding.kind != RepresentativeStaticBindingKind::Const
                        && static_argument_matches_type(
                            program,
                            &binding.argument,
                            concrete,
                            substitutions,
                        )
                },
            ),
        (
            TypeReferenceNode::Reference {
                referee: template_referee,
                is_mutable: template_mutable,
                ..
            },
            TypeReferenceNode::Reference {
                referee: concrete_referee,
                is_mutable: concrete_mutable,
                ..
            },
        ) => {
            template_mutable == concrete_mutable
                && substituted_type_matches(
                    program,
                    *template_referee,
                    *concrete_referee,
                    substitutions,
                )
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: template_element,
                length: template_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: concrete_element,
                length: concrete_length,
            },
        ) => {
            substituted_type_matches(program, *template_element, *concrete_element, substitutions)
                && substituted_array_length_matches(template_length, concrete_length, substitutions)
        }
        (
            TypeReferenceNode::Slice {
                element_type: template_element,
            },
            TypeReferenceNode::Slice {
                element_type: concrete_element,
            },
        ) => substituted_type_matches(program, *template_element, *concrete_element, substitutions),
        (
            TypeReferenceNode::Generic {
                base_symbol: template_base,
                lifetime_arguments: template_lifetimes,
                arguments: template_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: concrete_base,
                lifetime_arguments: concrete_lifetimes,
                arguments: concrete_arguments,
                ..
            },
        ) => {
            if let Some(binding) = substitutions
                .iter()
                .find(|binding| binding.parameter == *template_base)
            {
                return binding.kind != RepresentativeStaticBindingKind::Const
                    && static_argument_matches_type(
                        program,
                        &binding.argument,
                        concrete,
                        substitutions,
                    );
            }
            let template_arguments = program
                .type_reference_table
                .type_reference_handles(*template_arguments);
            let concrete_arguments = program
                .type_reference_table
                .type_reference_handles(*concrete_arguments);
            template_base == concrete_base
                && template_lifetimes == concrete_lifetimes
                && template_arguments.len() == concrete_arguments.len()
                && template_arguments
                    .iter()
                    .zip(concrete_arguments)
                    .all(|(template, concrete)| {
                        substituted_type_matches(program, *template, *concrete, substitutions)
                    })
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        // Constrained/const-expression/dynamic-trait identities can contain
        // more than a closed type/const/machine binder. Until their own exact
        // substitution judgments exist, only an already-identical type passes.
        _ => {
            program.normalized_type_identity(template) == program.normalized_type_identity(concrete)
        }
    }
}

fn substituted_array_length_matches(
    template: &FixedArrayLength,
    concrete: &FixedArrayLength,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    match (template, concrete) {
        (FixedArrayLength::Literal(template), FixedArrayLength::Literal(concrete)) => {
            template == concrete
        }
        (FixedArrayLength::ConstParameter { symbol, .. }, FixedArrayLength::Literal(concrete)) => {
            substitutions
                .iter()
                .find(|binding| {
                    binding.parameter == *symbol
                        && binding.kind == RepresentativeStaticBindingKind::Const
                })
                .and_then(|binding| binding.argument.const_literal.as_ref())
                .and_then(|literal| literal.value_u64())
                .and_then(|literal| usize::try_from(literal).ok())
                == Some(*concrete)
        }
        (FixedArrayLength::ConstParameter { symbol, .. }, _) => {
            !substitutions
                .iter()
                .any(|binding| binding.parameter == *symbol)
                && template == concrete
        }
        _ => template == concrete,
    }
}

fn static_argument_matches_type(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    concrete: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
        return false;
    }
    let concrete_node = program.type_reference_table.type_reference(concrete);
    let Some(application) = argument.application.as_ref() else {
        let TypeReferenceNode::Named { symbol, name } = concrete_node else {
            return false;
        };
        return if argument.symbol.is_valid() {
            argument.symbol == *symbol
        } else {
            !symbol.is_valid()
                && argument.path.len() == 1
                && argument.path[0].as_str() == name.as_str()
        };
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = concrete_node
    else {
        return false;
    };
    if argument.symbol != *base_symbol
        || application.lifetime_arguments.as_ref() != lifetime_arguments.as_slice()
    {
        return false;
    }
    let concrete_arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    application.arguments.len() == concrete_arguments.len()
        && application
            .arguments
            .iter()
            .zip(concrete_arguments)
            .all(|(argument, concrete)| {
                static_argument_matches_type(program, argument, *concrete, substitutions)
            })
}

fn derive_representative_telescope(
    program: &TypedTrees,
    request: &QuotientOperationRequest,
) -> Result<RepresentativeTelescope, RelationPlanError> {
    let (machine, state) =
        representative_machine_state(program, request.representative_operation.symbol)?;
    let static_application = derive_exact_representative_static_application(program, request)?;
    if !state.return_type.is_valid() {
        return Err(RelationPlanError::RepresentativeResultTypeIsUnresolved);
    }
    let parameters = program
        .state_parameters(state)
        .iter()
        // This is only the RUNTIME telescope. Exact static/const argument
        // correspondence remains a later obligation over the retained static
        // application; filtering here does not discharge it.
        .filter(|parameter| !parameter.is_const)
        .map(|parameter| RepresentativeRuntimeParameter {
            symbol: parameter.symbol,
            type_reference: parameter.type_reference,
            is_mutable: parameter.is_mutable,
            is_self: parameter.is_self,
        })
        .collect();
    Ok(RepresentativeTelescope {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        parameters,
        return_type: state.return_type,
        machine_contracts: machine.contracts,
        state_contracts: state.contracts,
        static_application,
    })
}

fn representative_machine_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
) -> Result<(&Machine, &State), RelationPlanError> {
    let mut matches = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == state_symbol)
            .map(move |state| (machine, state))
    });
    let Some((machine, state)) = matches.next() else {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    };
    if matches.next().is_some() {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    }
    Ok((machine, state))
}

fn derive_exact_representative_static_application(
    program: &TypedTrees,
    request: &QuotientOperationRequest,
) -> Result<RepresentativeStaticApplication, RelationPlanError> {
    let (machine, _) =
        representative_machine_state(program, request.representative_operation.symbol)?;
    validate_static_application(
        program,
        &machine.lifetime_parameters,
        program.machine_type_parameters(machine),
        &request.representative_operation,
    )
}

fn validate_static_application(
    program: &TypedTrees,
    lifetime_parameters: &[Identifier],
    parameters: &[TypeParameter],
    selected: &StaticMachineArgument,
) -> Result<RepresentativeStaticApplication, RelationPlanError> {
    if !lifetime_parameters.is_empty() {
        return Err(RelationPlanError::RepresentativeLifetimeApplicationRequiresElision);
    }
    let empty_lifetimes: &[Identifier] = &[];
    let empty_arguments: &[StaticMachineArgument] = &[];
    let (lifetime_arguments, arguments) = selected
        .application
        .as_ref()
        .map(|application| {
            (
                application.lifetime_arguments.as_ref(),
                application.arguments.as_ref(),
            )
        })
        .unwrap_or((empty_lifetimes, empty_arguments));
    if !lifetime_arguments.is_empty() || arguments.len() != parameters.len() {
        return Err(RelationPlanError::RepresentativeStaticArityMismatch);
    }

    let mut bindings = Vec::with_capacity(arguments.len());
    for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        let kind = match &parameter.kind {
            TypeParameterKind::Type => {
                validate_closed_type_argument(program, argument, position)?;
                RepresentativeStaticBindingKind::Type
            }
            TypeParameterKind::Const { .. } => {
                if argument.const_literal.is_none()
                    || argument.symbol.is_valid()
                    || argument.application.is_some()
                    || argument.evidence_projection.is_some()
                {
                    return Err(
                        RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position),
                    );
                }
                RepresentativeStaticBindingKind::Const
            }
            TypeParameterKind::Machine { .. } => {
                if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
                    return Err(
                        RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position),
                    );
                }
                let (machine, _) =
                    representative_machine_state(program, argument.symbol).map_err(|_| {
                        RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position)
                    })?;
                validate_static_application(
                    program,
                    &machine.lifetime_parameters,
                    program.machine_type_parameters(machine),
                    argument,
                )?;
                RepresentativeStaticBindingKind::Machine
            }
            TypeParameterKind::Proposition { .. } => {
                return Err(
                    RelationPlanError::RepresentativePropositionApplicationUnsupported(position),
                );
            }
        };
        bindings.push(RepresentativeStaticBinding {
            parameter: parameter.symbol,
            kind,
            argument: argument.clone(),
        });
    }
    Ok(RepresentativeStaticApplication {
        lifetime_arguments: lifetime_arguments.to_vec(),
        bindings,
    })
}

fn validate_closed_type_argument(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    position: usize,
) -> Result<(), RelationPlanError> {
    if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
        return Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position));
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == argument.symbol)
    else {
        let primitive = argument.application.is_none()
            && argument.path.len() == 1
            && PrimitiveType::from_name(argument.path[0].as_str()).is_some();
        if primitive {
            return Ok(());
        }
        return Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position));
    };
    let nested = validate_static_application(
        program,
        &data.lifetime_parameters,
        program.data_type_parameters(data),
        argument,
    );
    match nested {
        Ok(_) => Ok(()),
        Err(RelationPlanError::RepresentativeStaticArityMismatch) => Err(
            RelationPlanError::RepresentativeStaticArgumentIsOpen(position),
        ),
        Err(error) => Err(error),
    }
}

enum ExactRelationLookup {
    NotQuotient,
    Exact(ExactQuotientRelation),
    OpenApplication,
}

fn exact_quotient_relation(
    program: &TypedTrees,
    quotient_type: TypeReferenceHandle,
) -> ExactRelationLookup {
    let Some(quotient) = super::quotient_for_type(program, quotient_type) else {
        return ExactRelationLookup::NotQuotient;
    };
    let Some(metadata) = quotient.quotient.as_ref() else {
        return ExactRelationLookup::NotQuotient;
    };
    let Some(relation) = program
        .propositions()
        .iter()
        .find(|relation| relation.symbol == metadata.relation_symbol)
    else {
        return ExactRelationLookup::OpenApplication;
    };
    if !program.proposition_binders(relation).is_empty() {
        // The quotient declaration retains the relation declaration identity,
        // but not the closed application needed for heterogeneous families.
        // That application must come from the fully instantiated
        // representative operation telescope; guessing it from the quotient
        // type would collapse independently quantified I/J/K binders.
        return ExactRelationLookup::OpenApplication;
    }
    ExactRelationLookup::Exact(ExactQuotientRelation {
        quotient_type,
        quotient_symbol: quotient.symbol,
        relation_symbol: metadata.relation_symbol,
    })
}

impl DirectTerminalRelationPlan {
    pub(super) fn render_ra(&self, program: &TypedTrees) -> String {
        let positions = self
            .input_relations
            .iter()
            .enumerate()
            .map(|(position, relation)| {
                let relation = match relation {
                    InputRelation::Quotient(relation) => {
                        relation_name(program, relation.relation_symbol)
                    }
                    InputRelation::ExactEquality(type_reference) => format!(
                        "==<{}>",
                        program.display_type_reference_with_constraints(*type_reference)
                    ),
                };
                format!("{position}:{relation}")
            })
            .collect::<Vec<_>>();
        format!("RA=[{}]", positions.join(", "))
    }

    pub(super) fn render_rr(&self, program: &TypedTrees) -> String {
        format!(
            "RR={}",
            relation_name(program, self.result_relation.relation_symbol)
        )
    }

    pub(super) fn render_representative_telescope(&self, program: &TypedTrees) -> String {
        let parameters = self
            .representative
            .parameters
            .iter()
            .map(|parameter| {
                let receiver = if parameter.is_self { "self:" } else { "" };
                format!(
                    "{receiver}{}",
                    program.display_type_reference_with_constraints(parameter.type_reference)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "F#{}({parameters})->{}",
            self.representative.state_symbol.arena_index(),
            program.display_type_reference_with_constraints(self.representative.return_type),
        )
    }

    pub(super) fn render_define_correspondence(&self) -> Option<String> {
        self.define_correspondence.as_ref().map(|correspondence| {
            format!(
                "define-runtime=[{}]",
                correspondence
                    .positions
                    .iter()
                    .enumerate()
                    .map(|(position, _)| position.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }

    pub(super) fn render_representative_precondition(&self) -> Option<String> {
        self.representative_precondition.as_ref().map(|partition| {
            format!(
                "P=[dependent:{}, fixed:{}]",
                partition.dependent.len(),
                partition.fixed.len()
            )
        })
    }

    pub(super) fn render_public_precondition(&self) -> Option<String> {
        self.public_precondition.as_ref().map(|partition| {
            format!(
                "Q=[dependent:{}, fixed:{}]",
                partition.dependent.len(),
                partition.fixed.len()
            )
        })
    }
}

fn relation_name(program: &TypedTrees, symbol: SymbolHandle) -> String {
    program
        .propositions()
        .iter()
        .find(|proposition| proposition.symbol == symbol)
        .map(|proposition| proposition.name.as_str().to_owned())
        .unwrap_or_else(|| format!("relation#{symbol:?}"))
}

#[cfg(test)]
mod tests {
    use super::{
        ExactQuotientRelation, InputRelation, RelationPlanError,
        RepresentativeContractFactLocation, RepresentativeContractOwner,
        RepresentativeRuntimeParameter, RepresentativeStaticApplication,
        RepresentativeStaticBindingKind, RepresentativeTelescope, derive_direct_terminal_plan,
        derive_exact_representative_static_application, derive_public_precondition_partition,
        derive_representative_precondition_partition, derive_representative_telescope,
        immutable_alias_fallthrough_root, substituted_type_matches,
    };
    use psi_arena::HandleSpan;
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::data::{
        DataDefinition, MachineParameterContract, QuotientDefinition, TypeParameter,
        TypeParameterKind,
    };
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, QuotientOperationKind,
        QuotientOperationRequest, StaticMachineArgument, StaticSymbolApplication,
        TableBinaryExpression, TableCallExpression, TableNamePath,
    };
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::proposition::{
        PropositionBinder, PropositionBinderKind, PropositionDefinition,
    };
    use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
    use psi_typed_trees::state::State;
    use psi_typed_trees::statement::{StatementNode, TableLocalData};
    use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn quotient_type(
        program: &mut TypedTrees,
        quotient_symbol: SymbolHandle,
        quotient_name: &'static str,
        relation_symbol: SymbolHandle,
        relation_name: &'static str,
    ) -> TypeReferenceHandle {
        let carrier_symbol = symbol(500);
        if !program
            .data_definitions()
            .iter()
            .any(|definition| definition.symbol == carrier_symbol)
        {
            program.push_data_definition(DataDefinition {
                symbol: carrier_symbol,
                name: Identifier::generated_static("Carrier"),
                ..Default::default()
            });
        }
        let carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: carrier_symbol,
                name: Identifier::generated_static("Carrier"),
            });
        if !program
            .propositions()
            .iter()
            .any(|proposition| proposition.symbol == relation_symbol)
        {
            program.push_proposition(PropositionDefinition {
                symbol: relation_symbol,
                name: Identifier::generated_static(relation_name),
                ..Default::default()
            });
        }
        program.push_data_definition(DataDefinition {
            symbol: quotient_symbol,
            name: Identifier::generated_static(quotient_name),
            quotient: Some(QuotientDefinition {
                carrier,
                relation: vec![Identifier::generated_static(relation_name)],
                relation_symbol,
                equivalence: None,
            }),
            ..Default::default()
        });
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: quotient_symbol,
                name: Identifier::generated_static(quotient_name),
            })
    }

    fn carrier_type(program: &mut TypedTrees) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(500),
                name: Identifier::generated_static("Carrier"),
            })
    }

    fn named_argument(
        program: &mut TypedTrees,
        name: &'static str,
        name_symbol: SymbolHandle,
    ) -> psi_typed_trees::expression::ExpressionHandle {
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated_static(name));
        program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                head_symbol: name_symbol,
                symbol: name_symbol,
                ..Default::default()
            }))
    }

    fn call_with_arguments(
        arguments: HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
    ) -> TableCallExpression {
        TableCallExpression {
            receiver: ExpressionHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            target: Identifier::generated_static("lift"),
            machine_arguments: Box::default(),
            quotient_operation: None,
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }
    }

    fn static_argument(name: &'static str) -> StaticMachineArgument {
        StaticMachineArgument {
            path: vec![Identifier::generated_static(name)].into_boxed_slice(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: SymbolHandle::invalid(),
        }
    }

    fn request_with_representative(symbol: SymbolHandle) -> QuotientOperationRequest {
        let mut representative_operation = static_argument("representative");
        representative_operation.symbol = symbol;
        QuotientOperationRequest {
            kind: QuotientOperationKind::Lift,
            representative_operation,
            respect_conformance: static_argument("ExactRespect"),
        }
    }

    fn push_representative(
        program: &mut TypedTrees,
        parameters: &[(TypeReferenceHandle, bool, bool)],
        return_type: TypeReferenceHandle,
    ) -> QuotientOperationRequest {
        let mut machine = Machine {
            symbol: symbol(90),
            name: Identifier::generated_static("representative"),
            ..Default::default()
        };
        let mut state = State {
            symbol: symbol(91),
            name: Identifier::generated_static("entry"),
            return_type,
            ..Default::default()
        };
        for (position, (type_reference, is_self, is_const)) in parameters.iter().enumerate() {
            program.push_state_parameter(
                &mut state,
                StateParameter {
                    symbol: symbol(100 + u32::try_from(position).expect("test position")),
                    name: Identifier::generated(format!("p{position}")),
                    type_reference: *type_reference,
                    is_self: *is_self,
                    is_const: *is_const,
                    ..Default::default()
                },
            );
        }
        program.push_machine_contract(&mut machine, Default::default());
        program.push_state_contract(&mut state, Default::default());
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        request_with_representative(symbol(91))
    }

    fn push_generic_representative_application(
        program: &mut TypedTrees,
    ) -> QuotientOperationRequest {
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let type_symbol = symbol(600);
        program.push_data_definition(DataDefinition {
            symbol: type_symbol,
            name: Identifier::generated_static("StaticType"),
            ..Default::default()
        });

        let mut selected_machine = Machine {
            symbol: symbol(610),
            name: Identifier::generated_static("selected"),
            ..Default::default()
        };
        program.push_machine_state(
            &mut selected_machine,
            State {
                symbol: symbol(611),
                return_type: unit,
                ..Default::default()
            },
        );
        program.push_machine(selected_machine);

        let mut representative = Machine {
            symbol: symbol(620),
            name: Identifier::generated_static("generic_representative"),
            ..Default::default()
        };
        for parameter in [
            TypeParameter {
                symbol: symbol(622),
                name: Identifier::generated_static("T"),
                kind: TypeParameterKind::Type,
                ..Default::default()
            },
            TypeParameter {
                symbol: symbol(623),
                name: Identifier::generated_static("N"),
                kind: TypeParameterKind::Const {
                    type_reference: unit,
                },
                ..Default::default()
            },
            TypeParameter {
                symbol: symbol(624),
                name: Identifier::generated_static("F"),
                kind: TypeParameterKind::Machine {
                    contract: MachineParameterContract::default(),
                },
                ..Default::default()
            },
        ] {
            program.push_machine_type_parameter(&mut representative, parameter);
        }
        let representative_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(622),
                name: Identifier::generated_static("T"),
            });
        let mut representative_state = State {
            symbol: symbol(621),
            return_type: representative_type,
            ..Default::default()
        };
        program.push_state_parameter(
            &mut representative_state,
            StateParameter {
                symbol: symbol(625),
                name: Identifier::generated_static("value"),
                type_reference: representative_type,
                ..Default::default()
            },
        );
        program.push_machine_state(&mut representative, representative_state);
        program.push_machine(representative);

        let mut type_argument = static_argument("StaticType");
        type_argument.symbol = type_symbol;
        let const_argument = StaticMachineArgument {
            path: Box::default(),
            application: None,
            const_literal: Some(Default::default()),
            evidence_projection: None,
            symbol: SymbolHandle::invalid(),
        };
        let mut machine_argument = static_argument("selected");
        machine_argument.symbol = symbol(611);
        let mut request = request_with_representative(symbol(621));
        request.representative_operation.application = Some(Box::new(StaticSymbolApplication {
            lifetime_arguments: Box::default(),
            arguments: vec![type_argument, const_argument, machine_argument].into_boxed_slice(),
        }));
        request
    }

    #[test]
    fn direct_plan_retains_exact_input_and_result_quotient_identities() {
        let mut program = TypedTrees::default();
        let left_type = quotient_type(&mut program, symbol(1), "LeftQ", symbol(2), "LeftR");
        let right_type = quotient_type(&mut program, symbol(3), "RightQ", symbol(4), "RightR");
        let ordinary_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let left_symbol = symbol(5);
        let ordinary_symbol = symbol(6);
        let left = named_argument(&mut program, "left", left_symbol);
        let ordinary = named_argument(&mut program, "ordinary", ordinary_symbol);
        let arguments = program
            .expression_table
            .insert_expression_handles([left, ordinary]);
        let call = call_with_arguments(arguments);
        let machine = Machine::default();
        let mut state = State {
            return_type: right_type,
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: left_symbol,
                name: Identifier::generated_static("left"),
                type_reference: left_type,
                ..Default::default()
            },
        );
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: ordinary_symbol,
                name: Identifier::generated_static("ordinary"),
                type_reference: ordinary_type,
                ..Default::default()
            },
        );
        let representative_carrier = carrier_type(&mut program);
        let request = push_representative(
            &mut program,
            &[
                (representative_carrier, true, false),
                (ordinary_type, false, false),
                (ordinary_type, false, true),
            ],
            representative_carrier,
        );

        let plan = derive_direct_terminal_plan(&program, &machine, &state, &call, &request)
            .expect("direct named operands and quotient result derive an exact plan");

        assert_eq!(plan.input_relations.len(), 2);
        let InputRelation::Quotient(left_relation) = plan.input_relations[0] else {
            panic!("quotient input must retain its exact relation");
        };
        assert_eq!(left_relation.quotient_type, left_type);
        assert_eq!(left_relation.quotient_symbol, symbol(1));
        assert_eq!(left_relation.relation_symbol, symbol(2));
        assert_eq!(
            plan.input_relations[1],
            InputRelation::ExactEquality(ordinary_type)
        );
        assert_eq!(plan.result_relation.quotient_type, right_type);
        assert_eq!(plan.result_relation.quotient_symbol, symbol(3));
        assert_eq!(plan.result_relation.relation_symbol, symbol(4));
        assert_eq!(plan.representative.machine_symbol, symbol(90));
        assert_eq!(plan.representative.state_symbol, symbol(91));
        assert_eq!(plan.representative.parameters.len(), 2);
        assert!(plan.representative.parameters[0].is_self);
        assert!(!plan.representative.parameters[1].is_self);
        assert_eq!(plan.representative.return_type, representative_carrier);
        assert_eq!(plan.representative.machine_contracts.count(), 1);
        assert_eq!(plan.representative.state_contracts.count(), 1);
    }

    #[test]
    fn direct_plan_rejects_untyped_adapted_argument() {
        let mut program = TypedTrees::default();
        let result_type = quotient_type(&mut program, symbol(1), "ResultQ", symbol(2), "ResultR");
        let literal = program
            .expression_table
            .insert(ExpressionNode::Integer(Default::default()));
        let arguments = program
            .expression_table
            .insert_expression_handles([literal]);
        let call = call_with_arguments(arguments);
        let state = State {
            return_type: result_type,
            ..Default::default()
        };

        assert_eq!(
            derive_direct_terminal_plan(
                &program,
                &Machine::default(),
                &state,
                &call,
                &request_with_representative(SymbolHandle::invalid()),
            ),
            Err(RelationPlanError::UnresolvedArgumentType(0))
        );
    }

    #[test]
    fn direct_plan_rejects_nonquotient_result() {
        let mut program = TypedTrees::default();
        let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let arguments = program
            .expression_table
            .insert_expression_handles(std::iter::empty());
        let call = call_with_arguments(arguments);
        let state = State {
            return_type: result_type,
            ..Default::default()
        };

        assert_eq!(
            derive_direct_terminal_plan(
                &program,
                &Machine::default(),
                &state,
                &call,
                &request_with_representative(SymbolHandle::invalid()),
            ),
            Err(RelationPlanError::ResultIsNotQuotient)
        );
    }

    #[test]
    fn direct_plan_rejects_open_relation_application_without_operation_telescope() {
        let mut program = TypedTrees::default();
        let mut relation = PropositionDefinition {
            symbol: symbol(2),
            name: Identifier::generated_static("IndexedR"),
            ..Default::default()
        };
        program.push_proposition_binder(
            &mut relation,
            PropositionBinder {
                symbol: symbol(3),
                name: Identifier::generated_static("I"),
                kind: PropositionBinderKind::Machine,
                ..Default::default()
            },
        );
        program.push_proposition(relation);
        let quotient_type =
            quotient_type(&mut program, symbol(1), "IndexedQ", symbol(2), "IndexedR");
        let value_symbol = symbol(4);
        let value = named_argument(&mut program, "value", value_symbol);
        let arguments = program.expression_table.insert_expression_handles([value]);
        let call = call_with_arguments(arguments);
        let mut state = State {
            return_type: quotient_type,
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: value_symbol,
                name: Identifier::generated_static("value"),
                type_reference: quotient_type,
                ..Default::default()
            },
        );

        assert_eq!(
            derive_direct_terminal_plan(
                &program,
                &Machine::default(),
                &state,
                &call,
                &request_with_representative(SymbolHandle::invalid()),
            ),
            Err(RelationPlanError::UnresolvedInputRelationApplication(0))
        );
    }

    #[test]
    fn representative_telescope_rejects_duplicate_state_identity_within_one_machine() {
        let mut program = TypedTrees::default();
        let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let mut machine = Machine {
            symbol: symbol(90),
            ..Default::default()
        };
        for _ in 0..2 {
            program.push_machine_state(
                &mut machine,
                State {
                    symbol: symbol(91),
                    return_type: result_type,
                    ..Default::default()
                },
            );
        }
        program.push_machine(machine);
        let request = request_with_representative(symbol(91));

        assert_eq!(
            derive_representative_telescope(&program, &request),
            Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly)
        );
    }

    #[test]
    fn representative_telescope_retains_closed_static_application_for_substitution() {
        let mut program = TypedTrees::default();
        let request = push_generic_representative_application(&mut program);

        let application = derive_exact_representative_static_application(&program, &request)
            .expect("closed type/const/machine application must retain exact bindings");
        assert_eq!(application.bindings.len(), 3);
        assert_eq!(application.bindings[0].parameter, symbol(622));
        assert_eq!(
            application.bindings[0].kind,
            RepresentativeStaticBindingKind::Type
        );
        assert_eq!(application.bindings[1].parameter, symbol(623));
        assert_eq!(
            application.bindings[1].kind,
            RepresentativeStaticBindingKind::Const
        );
        assert_eq!(application.bindings[2].parameter, symbol(624));
        assert_eq!(
            application.bindings[2].kind,
            RepresentativeStaticBindingKind::Machine
        );

        let telescope = derive_representative_telescope(&program, &request)
            .expect("a closed static application is retained on the telescope");
        assert_eq!(telescope.static_application, application);
    }

    #[test]
    fn immutable_telescope_substitution_covers_type_const_and_machine_binders() {
        let mut program = TypedTrees::default();
        let request = push_generic_representative_application(&mut program);
        let bindings = derive_representative_telescope(&program, &request)
            .expect("closed application")
            .static_application
            .bindings;

        let type_template = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(622),
                name: Identifier::generated_static("T"),
            });
        let type_concrete = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(600),
                name: Identifier::generated_static("StaticType"),
            });
        assert!(substituted_type_matches(
            &program,
            type_template,
            type_concrete,
            &bindings,
        ));

        let machine_template = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(624),
                name: Identifier::generated_static("F"),
            });
        let machine_concrete = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(611),
                name: Identifier::generated_static("selected"),
            });
        assert!(substituted_type_matches(
            &program,
            machine_template,
            machine_concrete,
            &bindings,
        ));

        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let array_template = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unit,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(623),
                    name: Identifier::generated_static("N"),
                },
            });
        let array_concrete = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unit,
                length: FixedArrayLength::Literal(0),
            });
        assert!(substituted_type_matches(
            &program,
            array_template,
            array_concrete,
            &bindings,
        ));
    }

    #[test]
    fn immutable_telescope_substitution_rejects_type_and_const_near_misses() {
        let mut program = TypedTrees::default();
        let request = push_generic_representative_application(&mut program);
        let bindings = derive_representative_telescope(&program, &request)
            .expect("closed application")
            .static_application
            .bindings;
        let type_template = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(622),
                name: Identifier::generated_static("T"),
            });
        program.push_data_definition(DataDefinition {
            symbol: symbol(601),
            name: Identifier::generated_static("OtherType"),
            ..Default::default()
        });
        let other_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(601),
                name: Identifier::generated_static("OtherType"),
            });
        assert!(!substituted_type_matches(
            &program,
            type_template,
            other_type,
            &bindings,
        ));

        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let array_template = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unit,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(623),
                    name: Identifier::generated_static("N"),
                },
            });
        let wrong_length = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unit,
                length: FixedArrayLength::Literal(1),
            });
        assert!(!substituted_type_matches(
            &program,
            array_template,
            wrong_length,
            &bindings,
        ));
        let stale_length = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unit,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(623),
                    name: Identifier::generated_static("N"),
                },
            });
        assert!(!substituted_type_matches(
            &program,
            array_template,
            stale_length,
            &bindings,
        ));
    }

    #[test]
    fn representative_precondition_partition_tracks_exact_dependent_fact_locations() {
        let mut program = TypedTrees::default();
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let quotient_parameter = symbol(700);
        let fixed_parameter = symbol(701);
        let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
        let fixed_name = named_argument(&mut program, "fixed", fixed_parameter);
        let mixed =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: quotient_name,
                    operator: BinaryOperator::Equal,
                    right: fixed_name,
                }));
        let machine_facts = program.proof_facts.insert_many([
            ProofFact::Expression(quotient_name),
            ProofFact::Expression(mixed),
        ]);
        let state_facts = program
            .proof_facts
            .insert_many([ProofFact::Expression(fixed_name)]);
        let machine_contracts = program.signature_contracts.insert_many([
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: machine_facts,
                ..Default::default()
            },
            SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts: state_facts,
                ..Default::default()
            },
        ]);
        let state_contracts = program.signature_contracts.insert_many([SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: state_facts,
            ..Default::default()
        }]);
        let telescope = RepresentativeTelescope {
            machine_symbol: symbol(710),
            state_symbol: symbol(711),
            parameters: vec![
                RepresentativeRuntimeParameter {
                    symbol: quotient_parameter,
                    type_reference: unit,
                    is_mutable: false,
                    is_self: false,
                },
                RepresentativeRuntimeParameter {
                    symbol: fixed_parameter,
                    type_reference: unit,
                    is_mutable: false,
                    is_self: false,
                },
            ],
            return_type: unit,
            machine_contracts,
            state_contracts,
            static_application: RepresentativeStaticApplication {
                lifetime_arguments: Vec::new(),
                bindings: Vec::new(),
            },
        };
        let relations = [
            InputRelation::Quotient(ExactQuotientRelation {
                quotient_type: unit,
                quotient_symbol: symbol(720),
                relation_symbol: symbol(721),
            }),
            InputRelation::ExactEquality(unit),
        ];

        let partition =
            derive_representative_precondition_partition(&program, &relations, &telescope)
                .expect("all value identities are exact");
        assert_eq!(
            partition.dependent,
            vec![
                RepresentativeContractFactLocation {
                    owner: RepresentativeContractOwner::Machine,
                    contract_position: 0,
                    fact_position: 0,
                },
                RepresentativeContractFactLocation {
                    owner: RepresentativeContractOwner::Machine,
                    contract_position: 0,
                    fact_position: 1,
                },
            ]
        );
        assert_eq!(
            partition.fixed,
            vec![RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::State,
                contract_position: 0,
                fact_position: 0,
            }]
        );
    }

    #[test]
    fn representative_precondition_partition_rejects_unresolved_value_identity() {
        let mut program = TypedTrees::default();
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let quotient_parameter = symbol(700);
        let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
        let unresolved = named_argument(&mut program, "unknown", SymbolHandle::invalid());
        let mixed =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: quotient_name,
                    operator: BinaryOperator::Equal,
                    right: unresolved,
                }));
        let facts = program
            .proof_facts
            .insert_many([ProofFact::Expression(mixed)]);
        let machine_contracts = program.signature_contracts.insert_many([SignatureContract {
            kind: SignatureContractKind::Requires,
            facts,
            ..Default::default()
        }]);
        let telescope = RepresentativeTelescope {
            machine_symbol: symbol(710),
            state_symbol: symbol(711),
            parameters: vec![RepresentativeRuntimeParameter {
                symbol: quotient_parameter,
                type_reference: unit,
                is_mutable: false,
                is_self: false,
            }],
            return_type: unit,
            machine_contracts,
            state_contracts: HandleSpan::empty(),
            static_application: RepresentativeStaticApplication {
                lifetime_arguments: Vec::new(),
                bindings: Vec::new(),
            },
        };

        assert_eq!(
            derive_representative_precondition_partition(
                &program,
                &[InputRelation::Quotient(ExactQuotientRelation {
                    quotient_type: unit,
                    quotient_symbol: symbol(720),
                    relation_symbol: symbol(721),
                })],
                &telescope,
            ),
            Err(RelationPlanError::PreconditionDependencyUnresolved)
        );
    }

    #[test]
    fn public_precondition_partition_distinguishes_q_from_fixed_ordinary_facts() {
        let mut program = TypedTrees::default();
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let quotient_parameter = symbol(730);
        let ordinary_parameter = symbol(731);
        let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
        let ordinary_name = named_argument(&mut program, "ordinary", ordinary_parameter);
        let machine_facts = program
            .proof_facts
            .insert_many([ProofFact::Expression(quotient_name)]);
        let state_facts = program
            .proof_facts
            .insert_many([ProofFact::Expression(ordinary_name)]);
        let mut machine = Machine::default();
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: machine_facts,
                ..Default::default()
            },
        );
        let mut state = State::default();
        for (parameter, name) in [
            (quotient_parameter, "quotient"),
            (ordinary_parameter, "ordinary"),
        ] {
            program.push_state_parameter(
                &mut state,
                StateParameter {
                    symbol: parameter,
                    name: Identifier::generated_static(name),
                    type_reference: unit,
                    ..Default::default()
                },
            );
        }
        program.push_state_contract(
            &mut state,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: state_facts,
                ..Default::default()
            },
        );
        let relations = [
            InputRelation::Quotient(ExactQuotientRelation {
                quotient_type: unit,
                quotient_symbol: symbol(732),
                relation_symbol: symbol(733),
            }),
            InputRelation::ExactEquality(unit),
        ];

        let partition =
            derive_public_precondition_partition(&program, &machine, &state, &relations)
                .expect("public parameter identities are exact");
        assert_eq!(
            partition.dependent,
            vec![RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 0,
            }]
        );
        assert_eq!(
            partition.fixed,
            vec![RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::State,
                contract_position: 0,
                fact_position: 0,
            }]
        );
    }

    #[test]
    fn define_correspondence_applies_closed_representative_type_substitution() {
        let mut program = TypedTrees::default();
        let mut request = push_generic_representative_application(&mut program);
        request.kind = QuotientOperationKind::Define;
        let carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(600),
                name: Identifier::generated_static("StaticType"),
            });
        program.push_proposition(PropositionDefinition {
            symbol: symbol(2),
            name: Identifier::generated_static("ExactR"),
            ..Default::default()
        });
        program.push_data_definition(DataDefinition {
            symbol: symbol(1),
            name: Identifier::generated_static("ExactQ"),
            quotient: Some(QuotientDefinition {
                carrier,
                relation: vec![Identifier::generated_static("ExactR")],
                relation_symbol: symbol(2),
                equivalence: None,
            }),
            ..Default::default()
        });
        let quotient = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(1),
                name: Identifier::generated_static("ExactQ"),
            });
        let public_symbol = symbol(3);
        let argument = named_argument(&mut program, "value", public_symbol);
        let arguments = program
            .expression_table
            .insert_expression_handles([argument]);
        let call = call_with_arguments(arguments);
        let mut state = State {
            return_type: quotient,
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: public_symbol,
                name: Identifier::generated_static("value"),
                type_reference: quotient,
                ..Default::default()
            },
        );

        let plan =
            derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
                .expect("closed T := StaticType must instantiate the runtime telescope");
        assert_eq!(
            plan.representative_precondition,
            Some(super::RepresentativePreconditionPartition {
                dependent: Vec::new(),
                fixed: Vec::new(),
            })
        );
        assert_eq!(
            plan.public_precondition,
            Some(super::RepresentativePreconditionPartition {
                dependent: Vec::new(),
                fixed: Vec::new(),
            })
        );
        assert_eq!(
            plan.define_correspondence
                .expect("define correspondence")
                .positions,
            vec![super::DefineRuntimePosition {
                public_parameter: public_symbol,
                representative_parameter: symbol(625),
            }]
        );
    }

    #[test]
    fn representative_static_application_rejects_const_category_near_miss() {
        let mut program = TypedTrees::default();
        let mut request = push_generic_representative_application(&mut program);
        let application = request
            .representative_operation
            .application
            .as_mut()
            .expect("generic application");
        application.arguments[1] = application.arguments[0].clone();

        assert_eq!(
            derive_exact_representative_static_application(&program, &request),
            Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(1))
        );
    }

    #[test]
    fn define_runtime_correspondence_rejects_reordered_public_parameters() {
        let mut program = TypedTrees::default();
        let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
        let carrier_type = carrier_type(&mut program);
        let left_symbol = symbol(3);
        let right_symbol = symbol(4);
        let left = named_argument(&mut program, "left", left_symbol);
        let right = named_argument(&mut program, "right", right_symbol);
        let arguments = program
            .expression_table
            .insert_expression_handles([right, left]);
        let call = call_with_arguments(arguments);
        let mut state = State {
            return_type: quotient_type,
            ..Default::default()
        };
        for (parameter_symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
            program.push_state_parameter(
                &mut state,
                StateParameter {
                    symbol: parameter_symbol,
                    name: Identifier::generated_static(name),
                    type_reference: quotient_type,
                    ..Default::default()
                },
            );
        }
        let mut request = push_representative(
            &mut program,
            &[(carrier_type, false, false), (carrier_type, false, false)],
            carrier_type,
        );
        request.kind = QuotientOperationKind::Define;

        assert_eq!(
            derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request,),
            Err(RelationPlanError::DefineArgumentOrderMismatch(0))
        );
    }

    #[test]
    fn derived_direct_terminal_plan_remains_non_executable() {
        let mut program = TypedTrees::default();
        let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
        let value_symbol = symbol(3);
        let value = named_argument(&mut program, "value", value_symbol);
        let arguments = program.expression_table.insert_expression_handles([value]);
        let mut call = call_with_arguments(arguments);
        let carrier_type = carrier_type(&mut program);
        // Attached and free operations share the normalized positional form:
        // the representative receiver occupies position zero without forcing
        // the public wrapper parameter to be spelled `self`.
        let mut request =
            push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
        request.kind = QuotientOperationKind::Define;
        call.quotient_operation = Some(request);
        let call = program.expression_table.insert(ExpressionNode::Call(call));
        let mut state = State {
            return_type: quotient_type,
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: value_symbol,
                name: Identifier::generated_static("value"),
                type_reference: quotient_type,
                ..Default::default()
            },
        );
        program
            .statement_table
            .push_statement(&mut state.statement_nodes, StatementNode::Expression(call));
        let mut machine = Machine::default();
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        let mut diagnostics = Vec::new();

        super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("compiler-derived direct-terminal relations RA=[0:")
        );
        assert!(diagnostics[0].message.contains("RR="));
        assert!(diagnostics[0].message.contains("define-runtime=[0]"));
        assert!(diagnostics[0].message.contains("Q=[dependent:0, fixed:0]"));
        assert!(diagnostics[0].message.contains("P=[dependent:0, fixed:0]"));
        assert!(
            diagnostics[0]
                .message
                .contains("one unchanged state-fallthrough result root")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("executable quotient operations are not admitted")
        );
    }

    #[test]
    fn immutable_alias_fallthrough_requires_an_exact_immutable_chain() {
        let mut program = TypedTrees::default();
        let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
        let arguments = program
            .expression_table
            .insert_expression_handles(std::iter::empty());
        let mut call = call_with_arguments(arguments);
        call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
        let request = program.expression_table.insert(ExpressionNode::Call(call));
        let first_symbol = symbol(10);
        let second_symbol = symbol(11);
        let first_name = named_argument(&mut program, "first", first_symbol);
        let second_name = named_argument(&mut program, "second", second_symbol);
        let mut state = State {
            return_type: quotient_type,
            ..Default::default()
        };
        for local in [
            TableLocalData {
                symbol: first_symbol,
                name: Identifier::generated_static("first"),
                type_reference: quotient_type,
                initial_value: request,
                is_mutable: false,
            },
            TableLocalData {
                symbol: second_symbol,
                name: Identifier::generated_static("second"),
                type_reference: quotient_type,
                initial_value: first_name,
                is_mutable: false,
            },
        ] {
            program
                .statement_table
                .push_statement(&mut state.statement_nodes, StatementNode::LocalData(local));
        }
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(second_name),
        );

        assert_eq!(
            immutable_alias_fallthrough_root(&program, &state),
            Some(super::ImmutableAliasFallthroughRoot {
                request_expression: request,
                alias_count: 2,
            })
        );

        let drifted_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        if let StatementNode::LocalData(first) = &mut program
            .statement_table
            .statements_mut(state.statement_nodes)[0]
        {
            first.type_reference = drifted_type;
        }
        assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);

        if let StatementNode::LocalData(first) = &mut program
            .statement_table
            .statements_mut(state.statement_nodes)[0]
        {
            first.type_reference = quotient_type;
            first.is_mutable = true;
        }
        assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);
    }

    #[test]
    fn derived_immutable_alias_fallthrough_remains_non_executable() {
        let mut program = TypedTrees::default();
        let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
        let value_symbol = symbol(3);
        let value = named_argument(&mut program, "value", value_symbol);
        let arguments = program.expression_table.insert_expression_handles([value]);
        let mut call = call_with_arguments(arguments);
        let carrier_type = carrier_type(&mut program);
        let mut request =
            push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
        request.kind = QuotientOperationKind::Define;
        call.quotient_operation = Some(request);
        let request = program.expression_table.insert(ExpressionNode::Call(call));
        let result_symbol = symbol(4);
        let result = named_argument(&mut program, "result", result_symbol);
        let mut state = State {
            return_type: quotient_type,
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: value_symbol,
                name: Identifier::generated_static("value"),
                type_reference: quotient_type,
                ..Default::default()
            },
        );
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: result_symbol,
                name: Identifier::generated_static("result"),
                type_reference: quotient_type,
                initial_value: request,
                is_mutable: false,
            }),
        );
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(result),
        );
        let mut machine = Machine::default();
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        let mut diagnostics = Vec::new();

        super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("compiler-derived immutable-alias fallthrough relations")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("through 1 exact immutable alias")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("executable quotient operations are not admitted")
        );
    }

    #[test]
    fn nonterminal_expression_request_cannot_claim_direct_result_flow() {
        let mut program = TypedTrees::default();
        let arguments = program
            .expression_table
            .insert_expression_handles(std::iter::empty());
        let mut call = call_with_arguments(arguments);
        call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
        let request = program.expression_table.insert(ExpressionNode::Call(call));
        let terminal = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let mut state = State::default();
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(request),
        );
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(terminal),
        );
        let mut machine = Machine::default();
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        let mut diagnostics = Vec::new();

        super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("retains its exact representative operation and named conformance")
        );
        assert!(
            !diagnostics[0]
                .message
                .contains("compiler-derived direct-terminal relations")
        );
        assert!(
            !diagnostics[0]
                .message
                .contains("unchanged state-fallthrough result root")
        );
    }
}
