//! Read a source expression under its declaration's actual input namespace.

use super::{
    Program, Slot,
    contracts::{ClauseScope, MathematicalTerms, ResultTerm},
};
use crate::{
    SourceExpressionAssumptions, SourceExpressionContext, SourceExpressionGoal,
    SourceExpressionValue, SourceNode,
    body::{
        Body, Borrowed, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Target,
        Termination, ValueType,
    },
    canonical::{self, Repeated},
    expression_formation::WitnessGoal,
};
use language_semantics::const_value::CanonicalConstIdentity;
use semantic_vocabulary::{MachineId, ScalarTerm, ScalarType, ValueId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::{
    data::{DataMember, TypeParameter, TypeParameterKind},
    domain::ProofFact,
    expression::ExpressionHandle,
    signature::{SignatureContractKind, StateParameter},
    types::TypeReferenceHandle,
};

struct Input {
    symbol: SymbolHandle,
    carrier: TypeReferenceHandle,
    parameter: Option<StateParameter>,
    erased: bool,
}

struct SourceFrame {
    inputs: Vec<Input>,
    generics: Vec<TypeParameter>,
    facts: Vec<ExpressionHandle>,
}

impl<'t> Program<'t> {
    fn expression_frame(&self, context: SourceExpressionContext<'_>) -> Checked<SourceFrame> {
        let id = self.next_identity()?;
        let typed = self.typed;
        let owner = context.lexical_owner;
        if !typed.symbols.symbols().nodes().is_valid(owner) {
            return Err(super::services::unavailable(
                id,
                "source expression has a stale lexical owner",
            ));
        }
        let mut frame = SourceFrame {
            inputs: Vec::new(),
            generics: Vec::new(),
            facts: Vec::new(),
        };
        let mut contracts = Vec::new();
        if let Some(signature) = context.signature {
            if !typed.symbols.symbols().nodes().is_valid(signature.symbol) {
                return Err(super::services::unavailable(
                    id,
                    "source expression lost its exact signature declaration",
                ));
            }
            frame
                .generics
                .extend_from_slice(typed.state_signature_type_parameters(signature));
            frame.inputs.extend(
                super::span(
                    &typed.state_parameters,
                    signature.parameters,
                    id,
                    "source signature parameter span",
                )?
                .iter()
                .cloned()
                .map(input_parameter),
            );
            contracts.extend_from_slice(super::span(
                &typed.signature_contracts,
                signature.contracts,
                id,
                "source signature clause span",
            )?);
        } else if let Some(machine) = typed.machine_by_symbol(owner) {
            let states = super::span(
                &typed.machine_states,
                machine.states,
                id,
                "source machine state span",
            )?;
            let state = match context.state {
                Some(symbol) => states.iter().find(|state| state.symbol == symbol),
                None => states.first(),
            }
            .ok_or_else(|| {
                super::services::unavailable(id, "source expression lost its exact state")
            })?;
            frame.generics = typed.machine_callable_parameters(machine);
            frame.inputs.extend(
                super::span(
                    &typed.state_parameters,
                    state.parameters,
                    id,
                    "source parameter span",
                )?
                .iter()
                .cloned()
                .map(input_parameter),
            );
            if states
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
            {
                contracts.extend_from_slice(super::span(
                    &typed.signature_contracts,
                    machine.contracts,
                    id,
                    "source machine clause span",
                )?);
            }
            contracts.extend_from_slice(super::span(
                &typed.signature_contracts,
                state.contracts,
                id,
                "source state clause span",
            )?);
        } else if let Some(data) = typed.data_definition_by_symbol(owner) {
            frame
                .generics
                .extend_from_slice(typed.data_type_parameters(data));
            // Concrete instances can keep template symbols in their predicates
            // after their own generic list has been consumed. Retain that exact
            // declaration namespace; supplied operands still come from the
            // authenticated application context, never inferred body values.
            let mut declaration = data;
            let mut applications = Vec::new();
            while let Some(application) = declaration.generic_instance {
                if !typed
                    .type_reference_table
                    .contains_type_reference(application)
                    || applications.contains(&declaration.symbol)
                {
                    return Err(super::services::unavailable(
                        id,
                        "source data instance has a stale or cyclic template application",
                    ));
                }
                applications.push(declaration.symbol);
                let typed_trees::types::TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } = typed.type_reference_table.type_reference(application)
                else {
                    return Err(super::services::unavailable(
                        id,
                        "source data instance lost its exact generic application",
                    ));
                };
                if !typed.symbols.symbols().nodes().is_valid(*base_symbol) {
                    return Err(super::services::unavailable(
                        id,
                        "source data template declaration is stale",
                    ));
                }
                let template = typed
                    .unique_data_definition_by_symbol(*base_symbol)
                    .ok_or_else(|| {
                        super::services::unavailable(
                            id,
                            "source data instance has no unique template declaration",
                        )
                    })?;
                let parameters = typed
                    .data_type_parameters
                    .span(template.type_parameters)
                    .ok_or_else(|| {
                        super::services::unavailable(
                            id,
                            "source data template parameter span is stale",
                        )
                    })?;
                let operands = typed
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if operands.len() != arguments.len() as usize
                    || parameters.len() != operands.len()
                    || operands.iter().any(|operand| {
                        !typed.type_reference_table.contains_type_reference(*operand)
                    })
                {
                    return Err(super::services::unavailable(
                        id,
                        "source data instance does not retain its complete template tuple",
                    ));
                }
                for parameter in parameters {
                    if !frame
                        .generics
                        .iter()
                        .any(|known| known.symbol == parameter.symbol)
                    {
                        frame.generics.push(parameter.clone());
                    }
                }
                declaration = template;
            }
            for member in typed.data_members(data) {
                if let DataMember::Field(field) = member {
                    frame.inputs.push(Input {
                        symbol: field.symbol,
                        carrier: field.type_reference,
                        parameter: None,
                        erased: field.relevance.is_erased(),
                    });
                }
            }
            frame.facts = self.expression_facts(
                id,
                super::span(
                    &typed.proof_facts,
                    data.requires_facts,
                    id,
                    "source data predicate span",
                )?,
            )?;
        } else if let Some(domain) = typed.domain_definition(owner) {
            frame
                .generics
                .extend_from_slice(typed.domain_type_parameters(domain));
            if domain.subject_symbol.is_valid() {
                frame.inputs.push(Input {
                    symbol: domain.subject_symbol,
                    carrier: domain.target_type,
                    parameter: None,
                    erased: false,
                });
            }
            frame.facts = self.expression_facts(
                id,
                super::span(
                    &typed.proof_facts,
                    domain.facts,
                    id,
                    "source domain predicate span",
                )?,
            )?;
        } else if let Some(alias) = typed
            .type_aliases()
            .iter()
            .find(|alias| alias.symbol == owner)
        {
            frame.generics.extend_from_slice(
                typed
                    .data_type_parameters
                    .span(alias.type_parameters)
                    .ok_or_else(|| {
                        super::services::unavailable(id, "source alias parameter span is stale")
                    })?,
            );
        } else if let Some(operator) = typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == owner)
        {
            frame
                .generics
                .extend_from_slice(typed.operator_type_parameters(operator));
            frame.inputs.extend(
                super::span(
                    &typed.state_parameters,
                    operator.parameters,
                    id,
                    "source operator parameter span",
                )?
                .iter()
                .cloned()
                .map(input_parameter),
            );
            contracts.extend_from_slice(super::span(
                &typed.signature_contracts,
                operator.contracts,
                id,
                "source operator clause span",
            )?);
        } else if let Some((_, signature)) = typed.trait_machine_signature(owner) {
            frame
                .generics
                .extend_from_slice(typed.state_signature_type_parameters(signature));
            frame.inputs.extend(
                super::span(
                    &typed.state_parameters,
                    signature.parameters,
                    id,
                    "source requirement parameter span",
                )?
                .iter()
                .cloned()
                .map(input_parameter),
            );
            contracts.extend_from_slice(super::span(
                &typed.signature_contracts,
                signature.contracts,
                id,
                "source requirement clause span",
            )?);
        } else if let Some(contract) = typed.retained_static_machine_contract(owner) {
            let signature = contract.signature();
            frame
                .generics
                .extend_from_slice(typed.state_signature_type_parameters(signature));
            frame.inputs.extend(
                super::span(
                    &typed.state_parameters,
                    signature.parameters,
                    id,
                    "source static parameter span",
                )?
                .iter()
                .cloned()
                .map(input_parameter),
            );
            contracts.extend_from_slice(super::span(
                &typed.signature_contracts,
                signature.contracts,
                id,
                "source static clause span",
            )?);
        } else {
            return Err(super::services::unavailable(
                id,
                "source expression declaration has no represented input namespace",
            ));
        }
        for contract in contracts {
            if contract.kind == SignatureContractKind::Requires {
                frame.facts.extend(self.expression_facts(
                    id,
                    super::span(
                        &typed.proof_facts,
                        contract.facts,
                        id,
                        "source input fact span",
                    )?,
                )?);
            }
        }
        // Lexical captures stay attached to their actual declaration. No
        // synthetic source binder is introduced for the proof witness.
        let mut ancestor = typed.symbols.get(owner).parent;
        let mut visited = Vec::new();
        while ancestor.is_valid() {
            if visited.contains(&ancestor) || !typed.symbols.symbols().nodes().is_valid(ancestor) {
                return Err(super::services::unavailable(
                    id,
                    "source expression has a stale or cyclic lexical scope",
                ));
            }
            visited.push(ancestor);
            let parameters = if let Some(machine) = typed.machine_by_symbol(ancestor) {
                typed.machine_callable_parameters(machine)
            } else if let Some(data) = typed.data_definition_by_symbol(ancestor) {
                typed.data_type_parameters(data).to_vec()
            } else if let Some(domain) = typed.domain_definition(ancestor) {
                typed.domain_type_parameters(domain).to_vec()
            } else {
                Vec::new()
            };
            for parameter in parameters {
                if !frame
                    .generics
                    .iter()
                    .any(|known| known.symbol == parameter.symbol)
                {
                    frame.generics.push(parameter);
                }
            }
            ancestor = typed.symbols.get(ancestor).parent;
        }
        for (position, input) in frame.inputs.iter().enumerate() {
            if !typed.symbols.symbols().nodes().is_valid(input.symbol)
                || !typed
                    .type_reference_table
                    .contains_type_reference(input.carrier)
                || frame.inputs[..position]
                    .iter()
                    .any(|prior| prior.symbol == input.symbol)
            {
                return Err(super::services::unavailable(
                    id,
                    "source expression input has a stale or repeated declaration or carrier",
                ));
            }
        }
        for parameter in &frame.generics {
            if !typed.symbols.symbols().nodes().is_valid(parameter.symbol) {
                return Err(super::services::unavailable(
                    id,
                    "source expression static binder is stale",
                ));
            }
        }
        Ok(frame)
    }

    fn expression_facts(
        &self,
        id: MachineId,
        facts: &[ProofFact],
    ) -> Checked<Vec<ExpressionHandle>> {
        facts.iter().map(|fact| self.fact(id, fact)).collect()
    }

    pub(crate) fn source_expression_premises(
        &self,
        context: SourceExpressionContext<'_>,
    ) -> Checked<Vec<ExpressionHandle>> {
        let frame = self.expression_frame(context)?;
        match context.assumptions {
            SourceExpressionAssumptions::Published => Ok(frame.facts),
            SourceExpressionAssumptions::Explicit(facts) => {
                if facts.iter().enumerate().any(|(position, fact)| {
                    !frame.facts.contains(fact) || facts[..position].contains(fact)
                }) {
                    return Err(super::services::unavailable(
                        self.next_identity()?,
                        "source expression premise is not an exact declared input fact",
                    ));
                }
                Ok(frame
                    .facts
                    .into_iter()
                    .filter(|fact| facts.contains(fact))
                    .collect())
            }
        }
    }

    pub(crate) fn source_expression_witness(
        &mut self,
        expression: ExpressionHandle,
        goal: WitnessGoal,
        context: SourceExpressionContext<'t>,
        premises: &[ExpressionHandle],
    ) -> Checked<(MachineId, Body)> {
        let id = self.next_identity()?;
        let typed = self.typed;
        if !typed.expression_table.expression_is_valid(expression) {
            return Err(super::services::unavailable(
                id,
                "source expression handle is stale",
            ));
        }
        let frame = self.expression_frame(context)?;
        self.source_expression_types(id, context, &frame.generics)?;
        self.lifetime_owner = context.lexical_owner;
        super::source_zero::validate_source_zero_projections(
            typed,
            context.lexical_owner,
            context.projections,
            id,
        )?;
        for projection in context.projections {
            let value_type = self.value_type(id, projection.carrier)?;
            let source = crate::SourceOrigin::typed(
                context.lexical_owner,
                SourceNode::Expression(projection.expression),
                typed.expression_table.source_span(projection.expression),
            );
            let value = scalar_constant(
                typed,
                id,
                projection.carrier,
                value_type,
                &projection.value,
                source,
            )?;
            self.source_expression_values
                .push((projection.expression, value));
        }
        let mut slots = Vec::new();
        let mut erased = Vec::new();
        let mut borrowed = Vec::new();
        for input in frame.inputs {
            let position = slots.len();
            let (value_type, is_erased, reference) = if let Some(parameter) = &input.parameter {
                if parameter.is_const {
                    (self.value_type(id, input.carrier)?, input.erased, None)
                } else {
                    self.formal(id, parameter, None)?
                }
            } else {
                (self.value_type(id, input.carrier)?, input.erased, None)
            };
            if is_erased {
                erased.push(position);
            }
            if let Some(reference) = &reference {
                borrowed.push(Borrowed {
                    position,
                    ..reference.clone()
                });
            }
            slots.push(Slot {
                symbol: input.symbol,
                value_type,
                assignable: false,
                homed: false,
                erased: is_erased,
                borrowed: reference.map(|reference| reference.access),
            });
        }
        for parameter in frame.generics {
            match parameter.kind {
                TypeParameterKind::Type => {}
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    if !slots.iter().any(|slot| slot.symbol == parameter.symbol) {
                        slots.push(Slot {
                            symbol: parameter.symbol,
                            value_type: ValueType::Scalar(
                                self.refinement_scalar_type(id, type_reference)?,
                            ),
                            assignable: false,
                            homed: false,
                            erased: false,
                            borrowed: None,
                        });
                    }
                }
                _ => {
                    return Err(super::services::unavailable(
                        id,
                        "source expression retains an unrepresented static binder category",
                    ));
                }
            }
        }
        let parameters = slots.iter().map(|slot| slot.value_type).collect::<Vec<_>>();
        let result = match goal {
            WitnessGoal::Expression(SourceExpressionGoal::Formed { expected }) => {
                Some(self.value_type(id, expected)?)
            }
            WitnessGoal::Expression(SourceExpressionGoal::Establish)
            | WitnessGoal::PremiseFormation => Some(ValueType::Scalar(ScalarType::Boolean)),
        };
        let values = crate::body::formal_values(
            slots
                .iter()
                .map(|slot| (slot.value_type, slot.erased, slot.borrowed.is_some())),
            result,
        );
        let clause_scope = ClauseScope {
            slots: &slots,
            values: &values,
            bound: &[],
            owner: context.state.unwrap_or(context.lexical_owner),
            result: ResultTerm::Absent("result in a source input premise"),
            subject: None,
            formal_fields: true,
            mathematical: MathematicalTerms::Served,
        };
        let mut requires = Vec::new();
        for premise in premises {
            if matches!(
                typed.expression_table.expression(*premise),
                typed_trees::expression::ExpressionNode::Boolean(true)
            ) {
                continue;
            }
            requires.push(self.clause(id, &clause_scope, *premise)?);
        }
        let mut statements = Vec::new();
        let source = crate::SourceOrigin::typed(
            context.lexical_owner,
            SourceNode::Expression(expression),
            typed.expression_table.source_span(expression),
        );
        for (position, binding) in context.bindings.iter().enumerate() {
            if matches!(goal, WitnessGoal::PremiseFormation) {
                break;
            }
            if context.bindings[..position]
                .iter()
                .any(|known| known.symbol == binding.symbol)
            {
                return Err(super::services::unavailable(
                    id,
                    "source expression repeats a supplied formal",
                ));
            }
            let declared = self.source_binding_carrier(id, binding, context)?;
            let value_type = self.value_type(id, binding.carrier)?;
            if self.value_type(id, declared)? != value_type {
                return Err(super::services::unavailable(
                    id,
                    "source expression supplied operand changed its formal carrier",
                ));
            }
            let value = match binding.value {
                SourceExpressionValue::Constant(constant) => {
                    scalar_constant(typed, id, binding.carrier, value_type, constant, source)?
                }
                SourceExpressionValue::Expression(actual) => {
                    self.expression(id, &slots, actual, value_type.scalar())?
                }
                SourceExpressionValue::Symbol(symbol) => {
                    let slot = slots.iter().rposition(|slot| slot.symbol == symbol).ok_or_else(|| super::services::unavailable(id, "source expression supplied symbol is outside its actual input namespace"))?;
                    if slots[slot].erased || slots[slot].borrowed.is_some() {
                        return Err(super::services::unavailable(
                            id,
                            "source expression supplied symbol needs retained erased or borrowed substitution",
                        ));
                    }
                    Expression {
                        kind: ExpressionKind::Slot(slot),
                        result: Some(slots[slot].value_type),
                        source,
                    }
                }
            };
            if value.result != Some(value_type) {
                return Err(super::services::unavailable(
                    id,
                    "source expression actual has another carrier",
                ));
            }
            // Applied operands live after the caller's formal namespace.
            // Reusing a declaration symbol in a recursive application shadows
            // it only in the supplied goal; entry premises keep the old value.
            let slot = slots.len();
            slots.push(Slot {
                symbol: binding.symbol,
                value_type,
                assignable: false,
                homed: false,
                erased: false,
                borrowed: None,
            });
            statements.push(Statement::Bind { slot, value });
        }
        let signature = Signature {
            parameters,
            erased,
            borrowed,
            result,
            requires: canonical::clauses(requires, Repeated::Merge, id)?,
            ensures: Vec::new(),
            crashes: CrashCeiling::DeclaredNone,
            termination: Termination::Silent,
        };
        let formals = slots[..signature.parameters.len()]
            .iter()
            .map(|slot| typed.symbols.display_path(slot.symbol, "::"))
            .collect();
        self.symbols.push(context.lexical_owner);
        self.signatures.push(signature);
        self.names.push(crate::MachineNames {
            machine: "source expression".into(),
            formals,
            ..Default::default()
        });
        self.scopes.push(self.scope.clone());
        self.value_parameters.push(0);
        self.value_arguments.push(Vec::new());
        self.fixed_inputs.push(id);
        let value = self.expression(id, &slots, expression, result.and_then(ValueType::scalar))?;
        if value.result != result {
            return Err(super::services::unavailable(
                id,
                "source expression does not have its expected carrier",
            ));
        }
        if matches!(
            goal,
            WitnessGoal::Expression(SourceExpressionGoal::Establish)
        ) {
            let scalar_count = slots[..self
                .signatures
                .get(id)
                .expect("signature was inserted")
                .parameters
                .len()]
                .iter()
                .filter(|slot| {
                    slot.value_type.declared().is_some() && !slot.erased && slot.borrowed.is_none()
                })
                .count();
            let result_value = ValueId::new(scalar_count as u64 + 1).ok_or_else(|| {
                super::services::unavailable(id, "source proof result identity overflow")
            })?;
            self.signatures
                .get_mut(id)
                .expect("signature was inserted")
                .ensures
                .push(canonical::equal(
                    ScalarTerm::Value {
                        id: result_value,
                        scalar_type: ScalarType::Boolean,
                    },
                    ScalarTerm::Boolean(true),
                    id,
                )?);
        }
        statements.push(Statement::Arm {
            guard: None,
            target: Target::Value(value),
            source,
        });
        Ok((
            id,
            Body {
                slots: slots.into_iter().map(|slot| slot.value_type).collect(),
                statements,
            },
        ))
    }

    fn source_expression_types(
        &mut self,
        id: MachineId,
        context: SourceExpressionContext<'_>,
        generics: &[TypeParameter],
    ) -> Checked<()> {
        for (ordinal, parameter) in generics
            .iter()
            .filter(|parameter| parameter.kind == TypeParameterKind::Type)
            .enumerate()
        {
            let ordinal = u32::try_from(ordinal).map_err(|_| {
                super::services::unavailable(id, "source type namespace exceeds its ordinal width")
            })?;
            self.scope.push((
                parameter.symbol,
                ValueType::Rigid(terminal_check_tables::values::RigidType(ordinal)),
            ));
        }
        self.apply_source_type_frames(id, context.types, context.scope_starts, &[])?;
        Ok(())
    }

    fn source_binding_carrier(
        &self,
        id: MachineId,
        binding: &crate::SourceExpressionBinding<'_>,
        context: SourceExpressionContext<'_>,
    ) -> Checked<TypeReferenceHandle> {
        let symbol = binding.symbol;
        if !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(super::services::unavailable(
                id,
                "source expression supplied formal is stale",
            ));
        }
        let frame = self.expression_frame(context)?;
        if binding.signature.is_none()
            && let Some(input) = frame.inputs.iter().find(|input| input.symbol == symbol)
        {
            return Ok(input.carrier);
        }
        let foreign;
        let parameters = if let Some(signature) = binding.signature {
            if !self
                .typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(signature.symbol)
            {
                return Err(super::services::unavailable(
                    id,
                    "source operand lost its exact supplying signature",
                ));
            }
            foreign = self
                .typed
                .data_type_parameters
                .span(signature.type_parameters)
                .ok_or_else(|| {
                    super::services::unavailable(
                        id,
                        "source supplying signature has a stale formal span",
                    )
                })?;
            foreign
        } else {
            &frame.generics
        };
        let mut matching = parameters
            .iter()
            .filter(|parameter| parameter.symbol == symbol);
        let parameter = matching.next().ok_or_else(|| {
            super::services::unavailable(
                id,
                "source expression supplied formal is outside its retained declaration",
            )
        })?;
        if matching.next().is_some() {
            return Err(super::services::unavailable(
                id,
                "source supplying declaration repeats a formal identity",
            ));
        }
        match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => Ok(type_reference),
            _ => Err(super::services::unavailable(
                id,
                "source expression supplied formal is not a value parameter",
            )),
        }
    }
}

fn input_parameter(parameter: StateParameter) -> Input {
    Input {
        symbol: parameter.symbol,
        carrier: parameter.type_reference,
        erased: parameter.relevance.is_erased(),
        parameter: Some(parameter),
    }
}

fn scalar_constant(
    _typed: &typed_trees::TypedTrees,
    id: MachineId,
    _carrier: TypeReferenceHandle,
    value_type: ValueType,
    identity: &CanonicalConstIdentity,
    source: crate::SourceOrigin,
) -> Checked<Expression> {
    let scalar = value_type.scalar().ok_or_else(|| {
        super::services::unavailable(
            id,
            "source constant has no scalar carrier; no structural invariant is assumed",
        )
    })?;
    let term = super::refinement::canonical_scalar_constant(id, scalar, identity)?;
    let kind = match term {
        ScalarTerm::Boolean(value) => ExpressionKind::Boolean(value),
        ScalarTerm::Integer { value, .. } => ExpressionKind::Integer(value),
        _ => {
            return Err(super::services::unavailable(
                id,
                "source scalar decoding did not retain a literal value",
            ));
        }
    };
    Ok(Expression {
        kind,
        result: Some(value_type),
        source,
    })
}

pub(super) fn expression_literal(id: MachineId, expression: &Expression) -> Checked<ScalarTerm> {
    match (
        &expression.kind,
        expression.result.and_then(ValueType::scalar),
    ) {
        (ExpressionKind::Boolean(value), Some(ScalarType::Boolean)) => {
            Ok(ScalarTerm::Boolean(*value))
        }
        (ExpressionKind::Integer(value), Some(ScalarType::Integer(scalar_type))) => {
            Ok(ScalarTerm::Integer {
                scalar_type,
                value: *value,
            })
        }
        _ => Err(super::services::unavailable(
            id,
            "source constant is not a mathematical scalar literal",
        )),
    }
}
