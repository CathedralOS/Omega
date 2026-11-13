/*
Operator declaration validation, namespace resolution, pre-specialization and
pre-flow operator selections, and boundary operator applications
(`declarations::operators`).
*/
pub(crate) mod applications {
    use diagnostics::Diagnostic;
    use language_semantics::const_value::CanonicalConstIdentity;
    use symbols::SymbolHandle;
    use symbols::SymbolKind;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::operator::{ClosedOperatorApplicationArgument, OperatorDefinition};
    use typed_trees::statement::{StatementHandle, TableCall};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ValidatedBoundaryOperatorApplicationUseSite {
        Expression(ExpressionHandle),
        Statement(StatementHandle),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedBoundaryOperatorApplication {
        pub site: ValidatedBoundaryOperatorApplicationUseSite,
        pub requirement: SymbolHandle,
        pub arguments: Vec<ValidatedBoundaryOperatorApplicationArgument>,
    }

    pub fn canonical_closed_operator_realization_bytes(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        retained: &[typed_trees::operator::ClosedOperatorRealizationApplication],
    ) -> Result<Vec<u8>, &'static str> {
        typed_trees::operator::canonical_closed_operator_realization_bytes(
            program.typed_program(),
            machine_symbol,
            retained,
        )
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ValidatedBoundaryOperatorApplicationArgument {
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
        TypeBinder {
            binder_owner: SymbolHandle,
            binder_ordinal: u32,
            binder_symbol: SymbolHandle,
            machine_owner: SymbolHandle,
            machine_binder_ordinal: u32,
            machine_binder_symbol: SymbolHandle,
        },
    }

    pub fn validate_named_operator_application(
        program: &dyn crate::CheckingInput,
        operator: &OperatorDefinition,
        static_arguments: &[StaticMachineArgument],
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> Result<Option<Vec<ClosedOperatorApplicationArgument>>, Diagnostic> {
        let operator_name = program.operator_path_string(operator.name);
        let parameters = program.operator_type_parameters(operator);

        if !operator.lifetime_parameters.is_empty()
            || parameters.iter().any(|parameter| {
                !matches!(
                    parameter.kind,
                    TypeParameterKind::Type | TypeParameterKind::Const { .. }
                )
            })
        {
            if static_arguments.is_empty() {
                return Ok(None);
            }
            return Err(Diagnostic::error(format!(
                "named operator `{operator_name}` has a lifetime, machine, or proposition telescope whose explicit application is not yet supported"
            )));
        }

        if parameters.is_empty() {
            if static_arguments.is_empty() {
                return Ok(Some(Vec::new()));
            }
            return Err(Diagnostic::error(format!(
                "monomorphic named operator `{operator_name}` takes no static arguments, got {}",
                static_arguments.len()
            )));
        }

        let Some(inferred) = typed_trees::operator::closed_operator_application_for_operands(
            program,
            operator,
            operand_types,
        ) else {
            if static_arguments.is_empty() {
                return Ok(None);
            }
            return Err(Diagnostic::error(format!(
                "named operator `{operator_name}` cannot validate explicit static arguments because its operand application remains open or unresolved"
            )));
        };

        validate_closed_operator_application(program, operator, &inferred)?;

        if static_arguments.is_empty() {
            return Ok(Some(inferred));
        }
        if static_arguments.len() != inferred.len() {
            return Err(Diagnostic::error(format!(
                "named operator `{operator_name}` requires {} static argument(s), got {}",
                inferred.len(),
                static_arguments.len()
            )));
        }
        for ((parameter, inferred), supplied) in parameters
            .iter()
            .zip(&inferred)
            .zip(static_arguments.iter())
        {
            if !static_argument_matches(program, supplied, inferred) {
                let identity_kind = match inferred {
                    ClosedOperatorApplicationArgument::Type { .. } => "type",
                    ClosedOperatorApplicationArgument::Const { .. } => "value",
                };
                return Err(Diagnostic::error(format!(
                    "named operator `{operator_name}` static argument for parameter `{}` does not equal the {identity_kind} inferred from its operands",
                    parameter.name,
                )));
            }
        }
        Ok(Some(inferred))
    }

    pub fn landed_integer_literal_type_reference(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let atom = match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) => {
                typed_trees::types::builtin_type_atom(literal.landing()?.landed_type)
            }
            _ => return None,
        };
        let symbol = program
            .symbols
            .child_handles(program.symbols.root())?
            .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
        program
            .type_reference_table
            .find_named_type_reference(symbol)
    }

    pub fn validated_boundary_operator_application(
        site: ValidatedBoundaryOperatorApplicationUseSite,
        operator: &OperatorDefinition,
        bindings: Vec<ClosedOperatorApplicationArgument>,
    ) -> ValidatedBoundaryOperatorApplication {
        let arguments = bindings
            .into_iter()
            .enumerate()
            .map(|(ordinal, argument)| {
                let binder_ordinal =
                    u32::try_from(ordinal).expect("operator static telescope ordinal overflow");
                match argument {
                    ClosedOperatorApplicationArgument::Type {
                        binder_symbol,
                        type_reference,
                    } => ValidatedBoundaryOperatorApplicationArgument::Type {
                        binder_owner: operator.symbol,
                        binder_ordinal,
                        binder_symbol,
                        type_reference,
                    },
                    ClosedOperatorApplicationArgument::Const {
                        binder_symbol,
                        declared_carrier,
                        value,
                    } => ValidatedBoundaryOperatorApplicationArgument::Const {
                        binder_owner: operator.symbol,
                        binder_ordinal,
                        binder_symbol,
                        declared_carrier,
                        value,
                    },
                }
            })
            .collect();
        ValidatedBoundaryOperatorApplication {
            site,
            requirement: operator.symbol,
            arguments,
        }
    }

    pub fn validated_symbolic_boundary_operator_application(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        site: ValidatedBoundaryOperatorApplicationUseSite,
        operator: &OperatorDefinition,
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> Option<ValidatedBoundaryOperatorApplication> {
        let bindings = typed_trees::operator::symbolic_operator_type_application_for_operands(
            program,
            machine,
            operator,
            operand_types,
        )?;
        let arguments = bindings
            .into_iter()
            .enumerate()
            .map(|(ordinal, binding)| {
                Some(ValidatedBoundaryOperatorApplicationArgument::TypeBinder {
                    binder_owner: operator.symbol,
                    binder_ordinal: u32::try_from(ordinal).ok()?,
                    binder_symbol: binding.operator_binder_symbol,
                    machine_owner: machine.symbol,
                    machine_binder_ordinal: binding.machine_binder_ordinal,
                    machine_binder_symbol: binding.machine_binder_symbol,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ValidatedBoundaryOperatorApplication {
            site,
            requirement: operator.symbol,
            arguments,
        })
    }

    pub(crate) fn retain_validated_boundary_operator_application(
        applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
        application: ValidatedBoundaryOperatorApplication,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(existing) = applications
            .iter()
            .find(|existing| existing.site == application.site)
        else {
            applications.push(application);
            return;
        };
        if existing != &application {
            diagnostics.push(Diagnostic::error(
                "one operator use produced inconsistent validated static applications",
            ));
        }
    }

    pub(crate) fn validate_named_statement_operator_application(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement: StatementHandle,
        call: &TableCall,
        applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(operator) =
            typed_trees::operator::declaration_by_symbol(program, call.target_symbol)
                .filter(|operator| operator.spelling.is_none())
        else {
            return;
        };
        let mut operand_types = Vec::new();
        if let Some(receiver_type) = statement_value_receiver_type(program, machine, state, call) {
            operand_types.push(Some(receiver_type));
        }
        operand_types.extend(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    crate::value_custody::places::declared_place_type(
                        program,
                        machine,
                        Some(state),
                        *argument,
                    )
                    .or_else(|| landed_integer_literal_type_reference(program, *argument))
                }),
        );
        match validate_named_operator_application(
            program,
            operator,
            &call.machine_arguments,
            &operand_types,
        ) {
            Ok(Some(bindings)) if operator.is_boundary => {
                let application = validated_boundary_operator_application(
                    ValidatedBoundaryOperatorApplicationUseSite::Statement(statement),
                    operator,
                    bindings,
                );
                retain_validated_boundary_operator_application(
                    applications,
                    application,
                    diagnostics,
                );
            }
            Ok(_) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    fn statement_value_receiver_type(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &TableCall,
    ) -> Option<TypeReferenceHandle> {
        if !call.receiver_symbol.is_valid()
            || !matches!(
                program.symbols.get(call.receiver_symbol).kind,
                SymbolKind::Field | SymbolKind::Local | SymbolKind::Parameter
            )
        {
            return None;
        }
        typed_trees::source_names::declared_receiver_type_reference(
            program,
            machine,
            state,
            call.receiver_symbol,
        )
    }

    pub fn validate_closed_operator_application(
        program: &dyn crate::CheckingInput,
        operator: &OperatorDefinition,
        application: &[ClosedOperatorApplicationArgument],
    ) -> Result<(), Diagnostic> {
        let operator_name = program.operator_path_string(operator.name);
        let parameters = program.operator_type_parameters(operator);
        if parameters.len() != application.len() {
            return Err(Diagnostic::error(format!(
                "closed operator application for `{operator_name}` does not match its declaration telescope"
            )));
        }
        for (parameter, argument) in parameters.iter().zip(application) {
            match (&parameter.kind, argument) {
                (
                    TypeParameterKind::Type,
                    ClosedOperatorApplicationArgument::Type {
                        binder_symbol,
                        type_reference,
                    },
                ) if *binder_symbol == parameter.symbol && type_reference.is_valid() => {
                    let bounds = crate::proof_contracts::properties::declared_property_requirements(
                        &parameter.bounds,
                    );
                    let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
                    for property in bounds {
                        if crate::proof_contracts::properties::type_satisfies_declared_property(
                            program,
                            &[],
                            *type_reference,
                            property,
                        ) {
                            continue;
                        }
                        return Err(Diagnostic::error(format!(
                            "type parameter `{} [{}]` of operator `{operator_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                            parameter.name,
                            bound_labels.join(", "),
                            program.display_type_reference_with_constraints(*type_reference),
                        )));
                    }
                }
                (
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference },
                    ClosedOperatorApplicationArgument::Const {
                        binder_symbol,
                        declared_carrier,
                        value,
                    },
                ) if *binder_symbol == parameter.symbol && *declared_carrier == *type_reference => {
                    crate::value_custody::type_references::validate_exact_const_identity(
                    program,
                    *declared_carrier,
                    value,
                )
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "const parameter `{}` of operator `{operator_name}` has an invalid closed value: {reason}",
                        parameter.name
                    ))
                })?;
                }
                _ => {
                    return Err(Diagnostic::error(format!(
                        "closed operator application for `{operator_name}` does not rejoin parameter `{}` by category and symbol",
                        parameter.name
                    )));
                }
            }
        }
        Ok(())
    }

    fn static_argument_matches(
        program: &dyn crate::CheckingInput,
        supplied: &StaticMachineArgument,
        inferred: &ClosedOperatorApplicationArgument,
    ) -> bool {
        match inferred {
            ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                static_type_argument_matches(program, supplied, *type_reference)
            }
            ClosedOperatorApplicationArgument::Const {
                declared_carrier,
                value,
                ..
            } => static_const_argument_identity(program, supplied, *declared_carrier)
                .is_some_and(|supplied| supplied == *value),
        }
    }

    fn static_const_argument_identity(
        program: &dyn crate::CheckingInput,
        supplied: &StaticMachineArgument,
        declared_carrier: TypeReferenceHandle,
    ) -> Option<CanonicalConstIdentity> {
        if supplied.application.is_some()
            || supplied.evidence_projection.is_some()
            || supplied.symbol.is_valid()
            || !supplied.path.is_empty()
        {
            return None;
        }
        let literal = supplied.const_literal.as_ref()?;
        let value = literal
            .value_i64()
            .map(i128::from)
            .or_else(|| literal.value_u64().map(i128::from))?;
        let primitive = program
            .type_reference_table
            .primitive_type(declared_carrier)?;
        Some(CanonicalConstIdentity::integer(primitive.name(), value))
    }

    fn static_type_argument_matches(
        program: &dyn crate::CheckingInput,
        supplied: &StaticMachineArgument,
        inferred: TypeReferenceHandle,
    ) -> bool {
        typed_trees::source_operator_types::static_type_argument_matches(
            program.typed_program(),
            supplied,
            inferred,
        )
    }
}
pub(crate) mod contracts {
    use crate::CheckingInput;
    use checked_trees::{CheckedOperatorFacts, CheckedValueFacts};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::references::DeclarationSelection;

    pub fn derive_pre_flow_operator_selections(
        program: &dyn CheckingInput,
    ) -> CheckedOperatorFacts {
        let values = crate::collect_executable_value_origins(program, &DeclarationSelection::all());
        build_operator_facts(program, &values)
    }

    pub fn build_operator_facts(
        program: &dyn CheckingInput,
        values: &CheckedValueFacts,
    ) -> CheckedOperatorFacts {
        crate::build_operator_uses(program, values)
            .with_operator_crash_contracts(derive_checked_operator_crash_contracts(program))
            .with_operator_realization_contracts(derive_checked_operator_realization_contracts(
                program,
            ))
    }

    pub fn derive_checked_operator_realization_contracts(
        program: &dyn CheckingInput,
    ) -> Vec<checked_trees::CheckedOperatorRealizationContract> {
        let mut rows = Vec::new();
        for machine in program.machines() {
            if program.machine_states(machine).is_empty() {
                continue;
            }
            for conformance in program.machine_trait_conformances(machine) {
                if program
                    .trait_definition_by_symbol(conformance.symbol)
                    .is_some()
                {
                    continue;
                }
                let Some(operator) = typed_trees::operator::declaration_by_symbol(
                    program,
                    conformance.requirement_symbol,
                ) else {
                    continue;
                };
                rows.push(checked_trees::CheckedOperatorRealizationContract::new(
                    machine.symbol,
                    operator.symbol,
                ));
            }
        }
        rows.sort_by_key(|row| {
            (
                row.machine_symbol().arena_index(),
                row.machine_symbol().generation(),
                row.operator_symbol().arena_index(),
                row.operator_symbol().generation(),
            )
        });
        rows
    }

    pub fn derive_checked_operator_crash_contracts(
        program: &dyn CheckingInput,
    ) -> Vec<checked_trees::CheckedOperatorCrashContract> {
        use std::collections::BTreeMap;
        use typed_trees::{domain::ProofFact, signature::SignatureContractKind};

        #[derive(Default)]
        struct Bucket {
            unconditional: bool,
            facts: Vec<arena::Handle<ProofFact>>,
        }

        let operators = program.operators().iter().chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        );
        let mut rows = operators
            .map(|operator| {
                let mut buckets = BTreeMap::<checked_trees::CrashCause, Bucket>::new();
                for contract in program.operator_contracts(operator) {
                    let SignatureContractKind::Crashes { cause } = contract.kind else {
                        continue;
                    };
                    let cause = checked_trees::CrashCause::from(cause);
                    let bucket = buckets.entry(cause).or_default();
                    if contract.facts.is_empty() {
                        bucket.unconditional = true;
                        continue;
                    }
                    for fact in contract.facts.handles() {
                        let is_true = matches!(
                            program.proof_facts.get(fact),
                            ProofFact::Expression(expression)
                                if matches!(
                                    program.expression_table.expression(*expression),
                                    ExpressionNode::Boolean(true)
                                )
                        );
                        if is_true {
                            bucket.unconditional = true;
                        } else {
                            bucket.facts.push(fact);
                        }
                    }
                }
                let buckets = buckets
                    .into_iter()
                    .map(|(cause, bucket)| {
                        checked_trees::CheckedOperatorCrashBucket::new(
                            cause,
                            bucket.unconditional,
                            if bucket.unconditional {
                                Vec::new()
                            } else {
                                bucket.facts
                            },
                        )
                    })
                    .collect();
                checked_trees::CheckedOperatorCrashContract::new(operator.symbol, buckets)
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| {
            (
                row.operator_symbol().arena_index(),
                row.operator_symbol().generation(),
            )
        });
        rows
    }
}
pub(crate) mod namespaces {
    pub(crate) fn is_named_operator_namespace(
        program: &dyn crate::CheckingInput,
        receiver_symbols: &[symbols::SymbolHandle],
        namespace: &[&str],
        target_symbol: symbols::SymbolHandle,
        target: &str,
        argument_count: usize,
    ) -> bool {
        if namespace.is_empty()
            || receiver_symbols.iter().any(|symbol| {
                symbol.is_valid()
                    && !matches!(
                        program.symbols.get(*symbol).kind,
                        symbols::SymbolKind::Module
                            | symbols::SymbolKind::BuiltinType
                            | symbols::SymbolKind::Data
                            | symbols::SymbolKind::Domain
                            | symbols::SymbolKind::Machine
                            | symbols::SymbolKind::Trait
                    )
            })
        {
            return false;
        }
        typed_trees::operator::resolve_named_call(
            program,
            target_symbol,
            Some(namespace),
            target,
            argument_count,
            false,
        )
        .is_some_and(|operator| {
            program
                .operator_path_members(operator.name)
                .split_last()
                .is_some_and(|(member, prefix)| {
                    member.as_str() == target
                        && prefix.len() == namespace.len()
                        && prefix
                            .iter()
                            .zip(namespace)
                            .all(|(expected, actual)| expected.as_str() == *actual)
                })
        })
    }

    pub(crate) fn expression_call_has_operator_namespace(
        program: &dyn crate::CheckingInput,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        let table = &program.expression_table;
        let Some(path) = table.name_path_node(call.receiver) else {
            return false;
        };
        let receiver_symbols: Vec<_> = [path.head_symbol, path.symbol]
            .into_iter()
            .chain(
                table
                    .name_path_member_symbols(path.member_symbols)
                    .iter()
                    .copied(),
            )
            .collect();
        let namespace: Vec<_> = table
            .name_path_members(path.members)
            .iter()
            .map(|member| member.as_str())
            .collect();
        let target_symbol = typed_trees::operator::resolve_named_expression_call(program, call)
            .map(|operator| operator.symbol)
            .unwrap_or(call.target_symbol);
        is_named_operator_namespace(
            program,
            &receiver_symbols,
            &namespace,
            target_symbol,
            call.target.as_str(),
            table.expression_handles(call.arguments).len(),
        )
    }
}
pub(crate) mod receiver {
    use checked_trees::CheckedValueOrigin;
    use symbols::SymbolHandle;
    use typed_trees::{
        expression::{ExpressionHandle, TableCastExpression},
        machine::Machine,
        state::State,
        types::TypeReferenceHandle,
    };
    struct SourceTypes<'a>(&'a dyn crate::CheckingInput);
    impl typed_trees::source_operator_types::ContextualSourceTypes for SourceTypes<'_> {
        fn typed(&self) -> &typed_trees::TypedTrees {
            self.0.typed_program()
        }
        fn expression_result_type(
            &self,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            crate::expression_result_type_reference(self.0, machine, state, expression)
        }
        fn cast_result_type(&self, cast: &TableCastExpression) -> Option<TypeReferenceHandle> {
            crate::value_custody::expression_types::cast_result_type_reference(self.0, cast)
        }
        fn selected_operator_result_type(
            &self,
            symbol: SymbolHandle,
            result: TypeReferenceHandle,
            operands: &[Option<TypeReferenceHandle>],
        ) -> Option<TypeReferenceHandle> {
            crate::value_custody::expression_types::selected_operator_result_type_reference(
                self.0, symbol, result, operands,
            )
        }
        fn array_literal_type(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
            crate::declared_constant_array_type(self.0, expression)
        }
    }
    pub fn expression_type_reference_for_origin(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> Option<TypeReferenceHandle> {
        let CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } = origin
        else {
            return None;
        };
        typed_trees::source_operator_types::expression_type_reference_in_state(
            &SourceTypes(program),
            state_symbol,
            statement_index,
            expression,
        )
    }
}
pub(crate) mod requirement_uses {
    use super::uses::named_float_policy_adapter;
    use crate::frozen_program::expression_table;
    use checked_trees::{
        CheckedArithmeticPolicyAdapter, CheckedNamedRequirementUseFact, CheckedValueOrigin,
    };
    use numerics::float_semantics::FloatFormat;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use typed_trees::types::PrimitiveType;

    fn directly_callable_requirement(
        program: &dyn crate::CheckingInput,
        entry_symbol: symbols::SymbolHandle,
    ) -> Option<&typed_trees::machine::Machine> {
        if !entry_symbol.is_valid() {
            return None;
        }
        program.machines().iter().find(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && !machine.body_is_present
                && program.machine_type_parameters(machine).is_empty()
                && matches!(
                    program.machine_states(machine),
                    [entry] if entry.symbol == entry_symbol
                )
        })
    }

    pub(crate) fn named_requirement_use_fact(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        call: &TableCallExpression,
    ) -> Option<CheckedNamedRequirementUseFact> {
        let (requirement_symbol, return_type) =
            if let Some((owner, signature)) = program.trait_machine_signature(call.target_symbol) {
                let static_receiver = !call.receiver.is_valid()
                    || matches!(expression_table(program).expression(call.receiver),
                ExpressionNode::Name(path) if path.symbol == owner.symbol);
                if !owner.is_boundary
                    || !static_receiver
                    || !program.trait_type_parameters(owner).is_empty()
                    || !signature.type_parameters.is_empty()
                {
                    return None;
                }
                (signature.symbol, signature.return_type)
            } else {
                let requirement = directly_callable_requirement(program, call.target_symbol)?;
                let entry = program.machine_states(requirement).first()?;
                (requirement.symbol, entry.return_type)
            };
        let policy_adapter = match program.primitive_type_reference(return_type) {
            Some(PrimitiveType::F32) => {
                named_float_policy_adapter(program, call, origin, FloatFormat::BINARY32)
            }
            Some(PrimitiveType::F64) => {
                named_float_policy_adapter(program, call, origin, FloatFormat::BINARY64)
            }
            _ => CheckedArithmeticPolicyAdapter::None,
        };
        Some(CheckedNamedRequirementUseFact {
            expression,
            origin,
            requirement_symbol,
            policy_adapter,
        })
    }

    pub fn token_bound_machine_call_target(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<SymbolHandle> {
        if call.operational_acknowledgement.origin
            != language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
        {
            return None;
        }
        let target = if call.authored_machine_application.is_valid() {
            expression_table(program)
                .static_machine_application(call.authored_machine_application)
                .registration_operation
        } else {
            call.target_symbol
        };
        let (machine, _) = crate::find_machine_head(program, target)?;
        machine.spelling.is_some().then_some(machine.symbol)
    }
}
pub(crate) mod scoped_applications {
    use crate::frozen_program::expression_table;
    use checked_trees::{CheckedOperatorResolutionStatus, CheckedValueOrigin};
    use language_core::OperatorSpelling;
    use symbols::SymbolHandle;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::operator::{
        BoundTraitOperatorMeaning, OperatorOperandBindings, ScopedTraitOperatorMeaning,
        SelectedTraitOperatorMeaning, SpelledOperator,
    };
    use typed_trees::types::TypeReferenceHandle;

    pub struct OperatorApplicationScope<'scope> {
        pub owner: SymbolHandle,
        pub origin: Option<CheckedValueOrigin>,
        pub requires: &'scope [&'scope ProofFact],
        pub expression_type: &'scope dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    }

    pub enum ScopedOperatorApplication<'program> {
        Declared {
            declaration: SpelledOperator<'program>,
            bindings: OperatorOperandBindings,
        },
        BoundTrait(BoundTraitOperatorMeaning<'program>),
        Trait(ScopedTraitOperatorMeaning<'program>),
        SelectedTrait {
            meaning: SelectedTraitOperatorMeaning<'program>,
            bindings: OperatorOperandBindings,
        },
    }

    impl ScopedOperatorApplication<'_> {
        pub fn public_symbol(&self) -> SymbolHandle {
            match self {
                Self::Declared { declaration, .. } => declaration.operator.symbol,
                Self::BoundTrait(meaning) => meaning.requirement.symbol,
                Self::Trait(meaning) => meaning.requirement.symbol,
                Self::SelectedTrait { meaning, .. } => meaning.requirement.symbol,
            }
        }
    }

    pub struct ScopedOperatorResolution<'program> {
        pub candidates: Vec<ScopedOperatorApplication<'program>>,
        pub status: CheckedOperatorResolutionStatus,
        pub selected_symbol: SymbolHandle,
    }

    impl<'program> ScopedOperatorResolution<'program> {
        pub fn selected(&self) -> Option<&ScopedOperatorApplication<'program>> {
            (self.status == CheckedOperatorResolutionStatus::Resolved).then_some(())?;
            self.candidates
                .iter()
                .find(|candidate| candidate.public_symbol() == self.selected_symbol)
        }

        pub fn into_selected(self) -> Option<ScopedOperatorApplication<'program>> {
            (self.status == CheckedOperatorResolutionStatus::Resolved).then_some(())?;
            self.candidates
                .into_iter()
                .find(|candidate| candidate.public_symbol() == self.selected_symbol)
        }
    }

    pub fn scoped_operator_application<'program>(
        program: &'program dyn crate::CheckingInput,
        scope: &OperatorApplicationScope<'_>,
        expression: ExpressionHandle,
        spelling: OperatorSpelling,
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> ScopedOperatorResolution<'program> {
        if program.expression_table.binary_node(expression).is_some_and(|binary| {
        crate::proof_contracts::bound_expression_meaning::has_exact_typed_case_membership_meaning(
            program, expression, binary, operand_types.first().copied().flatten(),
        )
    }) {
        return ScopedOperatorResolution {
            candidates: Vec::new(),
            status: CheckedOperatorResolutionStatus::BuiltinFallback,
            selected_symbol: SymbolHandle::default(),
        };
    }
        if typed_trees::closed_numeric::has_anonymous_operator_meaning(program, expression)
            && expression_table(program)
                .binary_node(expression)
                .is_some_and(|binary| {
                    [binary.left, binary.right].into_iter().all(|operand| {
                        crate::value_custody::literals::has_anonymous_numeric_results(
                            program, operand,
                        )
                    })
                })
        {
            return ScopedOperatorResolution {
                candidates: Vec::new(),
                status: CheckedOperatorResolutionStatus::BuiltinFallback,
                selected_symbol: SymbolHandle::default(),
            };
        }
        let machine = program
            .machine_holding_state(scope.owner)
            .map_or(scope.owner, |machine| machine.symbol);
        let mut candidates = Vec::new();
        if scope.owner.is_valid() {
            let parent = program.symbols.get(scope.owner).parent;
            if let Some(definition) = program.trait_definition_by_symbol(parent)
                && program
                    .trait_machine_signature_by_symbol(definition, scope.owner)
                    .is_some()
            {
                candidates.extend(
                    typed_trees::operator::trait_scope_operator_meanings(
                        program,
                        definition,
                        spelling,
                        operand_types,
                    )
                    .into_iter()
                    .map(ScopedOperatorApplication::Trait),
                );
            }
        }
        if candidates.is_empty() {
            candidates.extend(
                typed_trees::operator::bound_trait_operator_meanings(
                    program,
                    machine,
                    spelling,
                    operand_types,
                )
                .into_iter()
                .map(ScopedOperatorApplication::BoundTrait),
            );
        }
        if candidates.is_empty() {
            candidates.extend(
                typed_trees::operator::selected_trait_operator_meanings(
                    program,
                    machine,
                    spelling,
                    operand_types,
                )
                .into_iter()
                .filter_map(|meaning| {
                    let bindings = typed_trees::operator::trait_operator_operand_bindings(
                        program,
                        meaning.trait_definition,
                        meaning.requirement,
                        operand_types,
                        None,
                    )?;
                    Some(ScopedOperatorApplication::SelectedTrait { meaning, bindings })
                }),
            );
        }
        if !candidates.is_empty() {
            let (status, selected_symbol) = if let [selected] = candidates.as_slice() {
                (
                    CheckedOperatorResolutionStatus::Resolved,
                    selected.public_symbol(),
                )
            } else {
                (
                    CheckedOperatorResolutionStatus::Ambiguous,
                    SymbolHandle::invalid(),
                )
            };
            return ScopedOperatorResolution {
                candidates,
                status,
                selected_symbol,
            };
        }
        let use_site = expression_table(program).source_span(expression);
        let declared = if matches!(spelling, OperatorSpelling::Index | OperatorSpelling::Range) {
            typed_trees::operator::resolve_indexed_spelling_for_operands(
                program,
                spelling,
                operand_types,
                use_site,
            )
        } else {
            typed_trees::operator::resolve_spelling_for_operands(
                program,
                spelling,
                operand_types,
                use_site,
            )
        };
        let operands = match expression_table(program).expression(expression) {
            typed_trees::expression::ExpressionNode::Binary(binary) => {
                vec![binary.left, binary.right]
            }
            typed_trees::expression::ExpressionNode::Indexed(indexed) => {
                if let Some(range) = expression_table(program).range_node(indexed.index) {
                    vec![indexed.collection, range.start, range.end]
                } else {
                    vec![indexed.collection, indexed.index]
                }
            }
            _ => Vec::new(),
        };
        let active = |domain| {
            operands
                .iter()
                .zip(operand_types)
                .any(|(operand, reference)| {
                    reference.is_some_and(|reference| {
                        typed_trees::domain::type_selects_semantic_domain(
                            program, reference, domain,
                        )
                    }) || super::selection::scoped_mint_selects_domain(
                        program,
                        scope.origin,
                        *operand,
                        domain,
                    ) || super::selection::signature_facts_select_domain(
                        program,
                        scope.requires,
                        *operand,
                        domain,
                    )
                })
        };
        let carriers = expression_table(program)
            .binary_node(expression)
            .is_some_and(|binary| {
                let [left, right] = operand_types else {
                    return false;
                };
                crate::value_custody::expression_types::builtin_binary_result(
                    program,
                    binary,
                    [*left, *right],
                    &mut |operand| (scope.expression_type)(operand),
                )
                .is_some()
                    || [binary.left, binary.right].into_iter().all(|operand| {
                        crate::value_custody::literals::has_anonymous_numeric_results(
                            program, operand,
                        )
                    })
            });
        let builtin_indexed = program.expression_table.indexed_node(expression).is_some()
            && matches!(spelling, OperatorSpelling::Index | OperatorSpelling::Range)
            && operand_types
                .first()
                .copied()
                .flatten()
                .and_then(|reference| crate::unwrapped_type_reference(program, reference))
                .is_some_and(|reference| {
                    matches!(
                        program.type_reference_table.type_reference(reference),
                        typed_trees::types::TypeReferenceNode::FixedArray { .. }
                            | typed_trees::types::TypeReferenceNode::Slice { .. }
                    )
                })
            && if spelling == OperatorSpelling::Index {
                typed_trees::operator::builtin_index_read_is_spelled(
                    program,
                    machine,
                    expression,
                    operand_types,
                )
            } else {
                typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    machine,
                    expression,
                    spelling,
                    operand_types,
                )
            };
        let builtin = builtin_indexed
            || (carriers
                && typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    scope.owner,
                    expression,
                    spelling,
                    operand_types,
                ));
        let domain_builtin = carriers
            && typed_trees::closed_numeric::has_builtin_anonymous_operands(
                program, expression, spelling,
            );
        let identities = declared
            .iter()
            .map(|candidate| {
                (
                    candidate.operator.symbol,
                    candidate
                        .domain
                        .map_or_else(SymbolHandle::invalid, |domain| domain.symbol),
                )
            })
            .collect::<Vec<_>>();
        let (status, selected_symbol) = super::selection::select_scoped_candidates(
            &identities,
            active,
            builtin
                || (declared.iter().any(|candidate| candidate.domain.is_some()) && domain_builtin),
        );
        for declaration in declared {
            if let Some(bindings) = typed_trees::operator::operator_operand_bindings(
                program,
                declaration.operator,
                operand_types,
            ) {
                candidates.push(ScopedOperatorApplication::Declared {
                    declaration,
                    bindings,
                });
            }
        }
        ScopedOperatorResolution {
            candidates,
            status,
            selected_symbol,
        }
    }

    pub(crate) fn occurrence_requires(
        program: &dyn crate::CheckingInput,
        origin: CheckedValueOrigin,
    ) -> Vec<&ProofFact> {
        let CheckedValueOrigin::StateStatement { state_symbol, .. } = origin else {
            return Vec::new();
        };
        program
            .machine_holding_state(state_symbol)
            .into_iter()
            .flat_map(|machine| program.machine_contracts(machine))
            .chain(
                program
                    .state_by_symbol(state_symbol)
                    .into_iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .collect()
    }
}
pub(crate) mod selection {
    use crate::frozen_program::expression_table;
    use checked_trees::{
        CheckedOperatorCandidateFact, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
        CheckedOperatorUseFact, CheckedValueOrigin,
    };
    use symbols::JoinedPath;
    use symbols::SymbolHandle;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    use super::receiver::expression_type_reference_for_origin;

    pub fn select_pending_domain_operator_meanings(
        program: &dyn crate::CheckingInput,
        operators: &mut CheckedOperatorFacts,
    ) {
        let pending: Vec<_> = operators
            .uses
            .iter()
            .filter(|(_, operator_use)| {
                operator_use.status == CheckedOperatorResolutionStatus::DomainPending
            })
            .map(|(handle, operator_use)| (handle, *operator_use))
            .collect();

        for (handle, operator_use) in pending {
            let candidates: Vec<CheckedOperatorCandidateFact> =
                operators.candidates(&operator_use).to_vec();
            let (status, selected_operator_symbol) =
                finalize_pending_use(program, &operator_use, &candidates);
            let operator_use = operators.uses.get_mut(handle);
            operator_use.status = status;
            operator_use.selected_operator_symbol = selected_operator_symbol;
        }
    }

    fn finalize_pending_use(
        program: &dyn crate::CheckingInput,
        operator_use: &CheckedOperatorUseFact,
        candidates: &[CheckedOperatorCandidateFact],
    ) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
        let identities = candidates
            .iter()
            .map(|candidate| (candidate.operator_symbol, candidate.domain_symbol))
            .collect::<Vec<_>>();
        select_scoped_candidates(
            &identities,
            |domain| domain_is_active_at_use(program, operator_use, domain),
            builtin_meaning_exists(program, operator_use),
        )
    }

    fn domain_is_active_at_use(
        program: &dyn crate::CheckingInput,
        operator_use: &CheckedOperatorUseFact,
        domain_symbol: SymbolHandle,
    ) -> bool {
        operator_use
            .operands(program)
            .unwrap_or_default()
            .into_iter()
            .any(|operand| {
                operand_selects_domain(program, operator_use.origin, operand, domain_symbol)
            })
    }

    fn operand_selects_domain(
        program: &dyn crate::CheckingInput,
        origin: CheckedValueOrigin,
        operand: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        expression_type_reference_for_origin(program, operand, origin).is_some_and(
            |type_reference| {
                typed_trees::domain::type_selects_semantic_domain(
                    program,
                    type_reference,
                    domain_symbol,
                )
            },
        ) || expression_mint_selects_domain(program, origin, operand, domain_symbol)
            || signature_selects_domain(program, origin, operand, domain_symbol)
    }

    fn expression_mint_selects_domain(
        program: &dyn crate::CheckingInput,
        origin: CheckedValueOrigin,
        expression: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        match expression_table(program).expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                expression_mint_selects_domain(program, origin, atomic.value, domain_symbol)
            }
            ExpressionNode::Borrow(inner) => {
                expression_mint_selects_domain(program, origin, inner.target, domain_symbol)
            }
            ExpressionNode::Cast(cast) => cast_selects_domain(program, cast, domain_symbol),
            ExpressionNode::Name(path) => local_initializer_selects_domain(
                program,
                origin,
                path.symbol,
                &expression_table(program).display_name(expression),
                domain_symbol,
            ),
            _ => false,
        }
    }

    fn cast_selects_domain(
        program: &dyn crate::CheckingInput,
        cast: &TableCastExpression,
        domain_symbol: SymbolHandle,
    ) -> bool {
        if cast.semantic_domain.is_empty() {
            return false;
        }
        let Some(domain) = typed_trees::domain::domain_by_symbol(program, domain_symbol)
            .filter(|domain| domain.semantic_roles.denotation_dimension.is_some())
        else {
            return false;
        };
        let authored = expression_table(program)
            .name_path_members(cast.semantic_domain)
            .joined_path();
        domain.name.as_str() == authored
            || domain
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|short| short == authored)
    }

    fn local_initializer_selects_domain(
        program: &dyn crate::CheckingInput,
        origin: CheckedValueOrigin,
        symbol: SymbolHandle,
        binding_name: &str,
        domain_symbol: SymbolHandle,
    ) -> bool {
        let CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } = origin
        else {
            return false;
        };
        let Some(state) = program.state_by_symbol(state_symbol) else {
            return false;
        };
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(statement_index)
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if (symbol.is_valid() && local.symbol == symbol)
                        || local.name.as_str() == binding_name =>
                {
                    Some(local.initial_value)
                }
                _ => None,
            })
            .is_some_and(|initializer| {
                expression_mint_selects_domain(program, origin, initializer, domain_symbol)
            })
    }

    fn signature_selects_domain(
        program: &dyn crate::CheckingInput,
        origin: CheckedValueOrigin,
        expression: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        let facts = super::scoped_applications::occurrence_requires(program, origin);
        signature_facts_select_domain(program, &facts, expression, domain_symbol)
    }

    pub(crate) fn signature_facts_select_domain(
        program: &dyn crate::CheckingInput,
        facts: &[&ProofFact],
        expression: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        let Some(binding) = direct_binding_symbol(program, expression) else {
            return false;
        };
        facts.iter().any(|fact| matches!(fact, ProofFact::Membership(membership)
        if membership.domain_symbol == domain_symbol
            && direct_binding_symbol(program, membership.value) == Some(binding)
            && program.domain_definition(domain_symbol).is_some_and(|domain| domain.semantic_roles.denotation_dimension.is_some())))
    }

    pub(crate) fn scoped_mint_selects_domain(
        program: &dyn crate::CheckingInput,
        origin: Option<CheckedValueOrigin>,
        expression: ExpressionHandle,
        domain: SymbolHandle,
    ) -> bool {
        if let Some(origin) = origin {
            return expression_mint_selects_domain(program, origin, expression, domain);
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Cast(cast) => cast_selects_domain(program, cast, domain),
            ExpressionNode::Borrow(borrow) => {
                scoped_mint_selects_domain(program, None, borrow.target, domain)
            }
            ExpressionNode::Atomic(atomic) => {
                scoped_mint_selects_domain(program, None, atomic.value, domain)
            }
            _ => false,
        }
    }

    pub(crate) fn select_scoped_candidates(
        candidates: &[(SymbolHandle, SymbolHandle)],
        active: impl Fn(SymbolHandle) -> bool,
        builtin: bool,
    ) -> (CheckedOperatorResolutionStatus, SymbolHandle) {
        let admissible = candidates
            .iter()
            .filter(|(_, domain)| domain.is_valid() && active(*domain))
            .collect::<Vec<_>>();
        match admissible.as_slice() {
            [selected] => return (CheckedOperatorResolutionStatus::Resolved, selected.0),
            [] => {}
            _ => {
                return (
                    CheckedOperatorResolutionStatus::Ambiguous,
                    SymbolHandle::invalid(),
                );
            }
        }
        let roots = candidates
            .iter()
            .filter(|(_, domain)| !domain.is_valid())
            .collect::<Vec<_>>();
        match roots.as_slice() {
            [selected] => (CheckedOperatorResolutionStatus::Resolved, selected.0),
            [] if builtin => (
                CheckedOperatorResolutionStatus::BuiltinFallback,
                SymbolHandle::invalid(),
            ),
            [] => (
                if candidates.is_empty() {
                    CheckedOperatorResolutionStatus::Missing
                } else {
                    CheckedOperatorResolutionStatus::Inadmissible
                },
                SymbolHandle::invalid(),
            ),
            _ => (
                CheckedOperatorResolutionStatus::Ambiguous,
                SymbolHandle::invalid(),
            ),
        }
    }

    fn direct_binding_symbol(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Atomic(atomic) => direct_binding_symbol(program, atomic.value),
            ExpressionNode::Borrow(inner) => direct_binding_symbol(program, inner.target),
            ExpressionNode::Name(path) => path
                .symbol
                .is_valid()
                .then_some(path.symbol)
                .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol)),
            _ => None,
        }
    }

    fn builtin_meaning_exists(
        program: &dyn crate::CheckingInput,
        operator_use: &CheckedOperatorUseFact,
    ) -> bool {
        if operator_use.occurrence != checked_trees::CheckedOperatorOccurrence::Expression {
            return false;
        }
        let Some(left_operand) = operator_use
            .operands(program)
            .unwrap_or_default()
            .first()
            .copied()
        else {
            return false;
        };
        expression_type_reference_for_origin(program, left_operand, operator_use.origin)
            .is_some_and(|type_reference| type_is_primitive(program, type_reference))
    }

    pub(crate) fn type_is_primitive(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        typed_trees::source_operator_types::type_is_primitive(program.typed_program(), reference)
    }
}
pub use scoped_applications::{
    OperatorApplicationScope, ScopedOperatorApplication, ScopedOperatorResolution,
    scoped_operator_application,
};
pub(crate) mod signatures {
    use typed_trees::operator::operator_operand_signature;

    pub(crate) fn operator_signature_key(
        program: &dyn crate::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
    ) -> Result<String, typed_trees::runtime_callable_identity::RuntimeCallableIdentityError> {
        operator_operand_signature(program, operator)
            .map(|operands| format!("{}({operands})", operator_name(program, operator)))
    }

    pub(crate) fn operator_operand_key(
        program: &dyn crate::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
    ) -> Result<String, typed_trees::runtime_callable_identity::RuntimeCallableIdentityError> {
        operator_operand_signature(program, operator)
    }

    pub(crate) fn operator_name(
        program: &dyn crate::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
    ) -> String {
        program.operator_path_string(operator.name)
    }
}
pub(crate) mod uses {
    use crate::frozen_program::expression_table;
    use std::collections::HashSet;

    use arena::Arena;
    use checked_trees::{
        CheckedArithmeticPolicyAdapter, CheckedNamedOperatorUseFact,
        CheckedNamedRequirementUseFact, CheckedOperatorCandidateFact, CheckedOperatorFacts,
        CheckedOperatorResolutionStatus, CheckedOperatorUseFact, CheckedValueFacts,
        CheckedValueOrigin,
    };
    use language_core::operator_spelling::OperatorSpelling;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::float_semantics::FloatFormat;
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
    };
    use typed_trees::operator::{
        SelectedTraitOperatorMeaning, SpelledOperator, resolve_spelling_for_operands,
        selected_trait_operator_meanings,
    };
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    use super::receiver::expression_type_reference_for_origin;
    use typed_trees::operator::binary_operator_spelling;

    pub fn collect_pre_specialization_operator_selections(
        program: &dyn crate::CheckingInput,
    ) -> Result<
        Vec<(
            ExpressionHandle,
            crate::selection_resolution::AuthoredSelectionResolutionTarget,
        )>,
        diagnostics::Diagnostic,
    > {
        use crate::selection_resolution::AuthoredSelectionResolutionTarget;
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;

        let mut selections = Vec::new();
        for operation in expression_table(program).selected_index_operations() {
            if let Some(target) =
                crate::selection_resolution::declaration_target(operation.operator)
            {
                selections.push((operation.expression, target));
            }
        }
        let mut diagnostic = None;
        program.for_each_machine_state_contract_frame(|machine, state, contracts, _| {
        let mut expressions = Vec::new();
        for contract in contracts {
            crate::queries::expression_traversal::collect_contract_facts(program, contract.facts, &mut expressions);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::queries::expression_traversal::collect_statement_expression_trees(program, statement, &mut expressions);
        }
        for expression in expressions {
            let Some(binary) = expression_table(program).binary_node(expression) else {
                continue;
            };
            let Some(spelling) = binary_operator_spelling(binary.operator) else {
                continue;
            };
            let operand_types = [binary.left, binary.right].map(|operand| {
                crate::expression_result_type_reference(program, machine, state, operand)
            });
            if operand_types.iter().all(Option::is_none) {
                continue;
            }
            let bound_meanings = typed_trees::operator::bound_trait_operator_meanings(
                program, machine.symbol, spelling, &operand_types,
            );
            let selected_meanings = selected_trait_operator_meanings(
                program, machine.symbol, spelling, &operand_types,
            );
            let target = match (bound_meanings.as_slice(), selected_meanings.as_slice()) {
                ([meaning], _) => AuthoredSelectionResolutionTarget::Declaration(meaning.requirement.symbol),
                ([], [meaning]) => AuthoredSelectionResolutionTarget::Declaration(meaning.requirement.symbol),
                ([], []) => {
                    if operand_types.iter().flatten().any(|reference| {
                        program.primitive_type_reference(*reference).is_none()
                    }) {
                        continue;
                    }
                    if !resolve_spelling_for_operands(
                        program, spelling, &operand_types,
                        expression_table(program).source_span(expression),
                    ).is_empty() || !typed_trees::closed_numeric::has_builtin_anonymous_operands(program, expression, spelling) {
                        continue;
                    }
                    AuthoredSelectionResolutionTarget::Intrinsic(AuthoredDeclarationSelectionIntrinsic::BuiltinOperator)
                }
                _ => {
                    diagnostic.get_or_insert_with(|| diagnostics::Diagnostic::error(format!(
                        "operator `{}` on machine `{}` has multiple explicitly selected implementation meanings",
                        spelling.symbol(), machine.name,
                    )));
                    continue;
                }
            };
            selections.push((expression, target));
        }
    });
        match diagnostic {
            Some(diagnostic) => Err(diagnostic),
            None => Ok(selections),
        }
    }

    #[derive(Default)]
    pub(crate) struct OperatorUseSink {
        seen: HashSet<(ExpressionHandle, CheckedValueOrigin), symbols::BuildSymbolHasher>,
        uses: Arena<CheckedOperatorUseFact>,
        named_uses: Arena<CheckedNamedOperatorUseFact>,
        named_requirement_uses: Arena<CheckedNamedRequirementUseFact>,
        candidates: Arena<CheckedOperatorCandidateFact>,
    }

    pub fn build_operator_uses(
        program: &dyn crate::CheckingInput,
        values: &CheckedValueFacts,
    ) -> CheckedOperatorFacts {
        let mut sink = OperatorUseSink::default();

        for (_, value) in values.values.iter() {
            if matches!(value.origin, CheckedValueOrigin::NestedExpression { .. }) {
                continue;
            }
            collect_expression_operator_use(program, value.expression, value.origin, &mut sink);
        }

        let OperatorUseSink {
            uses,
            named_uses,
            named_requirement_uses,
            candidates,
            ..
        } = sink;
        CheckedOperatorFacts::with_roots(uses, named_uses, candidates)
            .with_named_requirement_uses(named_requirement_uses)
    }

    fn collect_expression_operator_use(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        sink: &mut OperatorUseSink,
    ) {
        if !expression.is_valid() || !sink.seen.insert((expression, origin)) {
            return;
        }

        let node = expression_table(program).expression(expression);
        match node {
            ExpressionNode::Match(dispatch) => {
                collect_expression_operator_use(program, dispatch.subject, origin, sink);
                let subject_type =
                    expression_type_reference_for_origin(program, dispatch.subject, origin);
                let floating_subject = subject_type
                    .and_then(|reference| crate::unwrapped_type_reference(program, reference))
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .or_else(|| crate::match_subject_primitive_type(program, dispatch))
                    .is_some_and(|primitive| {
                        matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                    });
                for (ordinal, arm) in expression_table(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .enumerate()
                {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                        && floating_subject
                        && let Ok(ordinal) = u32::try_from(ordinal)
                    {
                        let source_arm = dispatch.arms.handle_at(ordinal);
                        let mut equality = operator_use_fact(
                            program,
                            expression,
                            origin,
                            OperatorSpelling::Equal,
                            &[
                                subject_type,
                                expression_type_reference_for_origin(program, pattern, origin),
                            ],
                            &mut sink.candidates,
                        );
                        equality.occurrence =
                            checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm };
                        sink.uses.append(equality);
                    }
                    for pattern in expression_table(program).match_pattern_operands(arm.pattern) {
                        collect_expression_operator_use(program, pattern, origin, sink);
                    }
                    collect_expression_operator_use(program, arm.value, origin, sink);
                }
                return;
            }
            ExpressionNode::Indexed(indexed) => {
                let spelling = indexed_operator_spelling(program, indexed.index);
                let operand_types = indexed_operand_types(program, indexed, origin);
                sink.uses.append(operator_use_fact(
                    program,
                    expression,
                    origin,
                    spelling,
                    &operand_types,
                    &mut sink.candidates,
                ));
            }
            ExpressionNode::Binary(binary) => {
                if let Some(spelling) = binary_operator_spelling(binary.operator)
                    && let operand_types = [
                        expression_type_reference_for_origin(program, binary.left, origin),
                        expression_type_reference_for_origin(program, binary.right, origin),
                    ]
                    && operand_types.iter().any(Option::is_some)
                    && let Some(fact) = binary_operator_use_fact(
                        program,
                        expression,
                        origin,
                        spelling,
                        &operand_types,
                        &mut sink.candidates,
                    )
                {
                    sink.uses.append(fact);
                }
            }
            ExpressionNode::Call(call) => {
                if let Some(named_use) = named_operator_use_fact(program, expression, origin, call)
                {
                    sink.named_uses.append(named_use);
                }
                if let Some(requirement_use) = super::requirement_uses::named_requirement_use_fact(
                    program, expression, origin, call,
                ) {
                    sink.named_requirement_uses.append(requirement_use);
                }
            }
            _ => {}
        }

        crate::queries::expression_traversal::executable_children(program, node, |child| {
            collect_expression_operator_use(program, child, origin, sink);
        });
    }

    fn named_operator_use_fact(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        call: &TableCallExpression,
    ) -> Option<CheckedNamedOperatorUseFact> {
        let operator = typed_trees::operator::resolve_named_expression_call(program, call)
            .or_else(|| {
                let operands: Vec<_> = expression_table(program)
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|operand| expression_type_reference_for_origin(program, *operand, origin))
                    .collect();
                let candidates =
                    typed_trees::operator::named_expression_call_candidates(program, call);
                let mut matching = candidates.into_iter().filter(|candidate| {
                    program
                        .machine_token_binding_by_symbol(candidate.symbol)
                        .is_some()
                        && candidate.spelling.is_some_and(|spelling| {
                            typed_trees::operator::resolve_spelling_for_operands(
                                program,
                                spelling,
                                &operands,
                                expression_table(program).source_span(expression),
                            )
                            .iter()
                            .any(|selected| selected.operator.symbol == candidate.symbol)
                        })
                });
                let selected = matching.next()?;
                matching.next().is_none().then_some(selected)
            })
            .filter(|operator| {
                let reference = expression_table(program).source_span(expression);
                program
                    .symbols
                    .source_reference_may_select_symbol(reference, operator.symbol)
            })?;
        let path = program.operator_path_members(operator.name);
        let format = match path {
            [namespace, requirement] => match namespace.as_str() {
                "F32" => Some(FloatFormat::BINARY32),
                "F64" => Some(FloatFormat::BINARY64),
                "Float" => typed_trees::operator::primitive_float_binary_semantics(
                    program, operator,
                )
                .map(|(_, format)| match format {
                    numerics::literals::FloatFormat::F32 => FloatFormat::BINARY32,
                    numerics::literals::FloatFormat::F64 => FloatFormat::BINARY64,
                }),
                "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64"
                    if matches!(requirement.as_str(), "from_f32" | "from_f64") =>
                {
                    None
                }
                _ => None,
            },
            _ => None,
        };
        let policy_adapter = match format {
            Some(format)
                if matches!(
                    (
                        format,
                        program.primitive_type_reference(operator.return_type)
                    ),
                    (FloatFormat::BINARY32, Some(PrimitiveType::F32))
                        | (FloatFormat::BINARY64, Some(PrimitiveType::F64))
                ) =>
            {
                named_float_policy_adapter(program, call, origin, format)
            }
            _ => CheckedArithmeticPolicyAdapter::None,
        };

        Some(CheckedNamedOperatorUseFact {
            expression,
            origin,
            is_generic_template: use_is_generic_template(program, origin),
            selected_operator_symbol: operator.symbol,
            policy_adapter,
        })
    }

    pub(crate) fn named_float_policy_adapter(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
        origin: CheckedValueOrigin,
        format: FloatFormat,
    ) -> CheckedArithmeticPolicyAdapter {
        if call.exact_coercion_target.is_valid() {
            return CheckedArithmeticPolicyAdapter::None;
        }
        let operands: Vec<_> = expression_table(program)
            .expression_handles(call.arguments)
            .iter()
            .map(|argument| expression_type_reference_for_origin(program, *argument, origin))
            .collect();
        crate::value_custody::expression_types::operand_arithmetic_domain(program, &operands)
            .map_or(CheckedArithmeticPolicyAdapter::None, |domain| {
                float_policy_adapter(format, domain)
            })
    }

    fn checked_candidate_span(
        program: &dyn crate::CheckingInput,
        candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
        candidates: &[SpelledOperator<'_>],
    ) -> (usize, arena::HandleSpan<CheckedOperatorCandidateFact>) {
        (
            candidates.len(),
            candidate_facts.insert_many(
                candidates
                    .iter()
                    .map(|candidate| checked_operator_candidate(program, candidate)),
            ),
        )
    }

    fn domain_or_unique_operator_status(
        candidates: &[SpelledOperator<'_>],
    ) -> Option<(CheckedOperatorResolutionStatus, SymbolHandle)> {
        if candidates
            .iter()
            .any(|candidate| candidate.domain.is_some())
        {
            Some((
                CheckedOperatorResolutionStatus::DomainPending,
                SymbolHandle::invalid(),
            ))
        } else if let [candidate] = candidates {
            Some((
                CheckedOperatorResolutionStatus::Resolved,
                candidate.operator.symbol,
            ))
        } else {
            None
        }
    }

    fn use_is_generic_template(
        program: &dyn crate::CheckingInput,
        origin: CheckedValueOrigin,
    ) -> bool {
        let CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin else {
            return false;
        };
        program
            .machine(machine_symbol)
            .is_some_and(|machine| !program.machine_type_parameters(machine).is_empty())
    }

    fn operator_use_record(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        spelling: OperatorSpelling,
        selected_operator_symbol: SymbolHandle,
        candidates: arena::HandleSpan<CheckedOperatorCandidateFact>,
        candidate_count: usize,
        status: CheckedOperatorResolutionStatus,
    ) -> CheckedOperatorUseFact {
        CheckedOperatorUseFact {
            expression,
            origin,
            is_generic_template: use_is_generic_template(program, origin),
            occurrence: Default::default(),
            spelling,
            policy_adapter: CheckedArithmeticPolicyAdapter::None,
            selected_operator_symbol,
            candidates,
            candidate_count,
            status,
        }
    }

    fn binary_operator_use_fact(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        spelling: OperatorSpelling,
        operand_types: &[Option<TypeReferenceHandle>],
        candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
    ) -> Option<CheckedOperatorUseFact> {
        let requires = super::scoped_applications::occurrence_requires(program, origin);
        let scope = super::OperatorApplicationScope {
            owner: origin_machine_symbol(origin).unwrap_or_else(SymbolHandle::invalid),
            origin: Some(origin),
            requires: &requires,
            expression_type: &|operand| {
                expression_type_reference_for_origin(program, operand, origin)
            },
        };
        let resolution = super::scoped_operator_application(
            program,
            &scope,
            expression,
            spelling,
            operand_types,
        );
        if resolution.candidates.is_empty() {
            return None;
        }
        let candidate_count = resolution.candidates.len();
        let candidate_span = candidate_facts.insert_many(
            resolution
                .candidates
                .iter()
                .map(|candidate| scoped_candidate_fact(program, candidate)),
        );
        Some(CheckedOperatorUseFact {
            expression,
            origin,
            is_generic_template: use_is_generic_template(program, origin),
            occurrence: Default::default(),
            spelling,
            policy_adapter: arithmetic_policy_adapter(program, spelling, operand_types),
            selected_operator_symbol: resolution.selected_symbol,
            candidates: candidate_span,
            candidate_count,
            status: resolution.status,
        })
    }

    fn scoped_candidate_fact(
        program: &dyn crate::CheckingInput,
        application: &super::ScopedOperatorApplication<'_>,
    ) -> CheckedOperatorCandidateFact {
        use super::ScopedOperatorApplication;
        let (candidate, definition, requirement) = match application {
            ScopedOperatorApplication::Declared { declaration, .. } => {
                return checked_operator_candidate(program, declaration);
            }
            ScopedOperatorApplication::SelectedTrait { meaning, .. } => (
                CheckedOperatorCandidateFact::trait_backed(
                    meaning.requirement.symbol,
                    meaning.application.declaration,
                    meaning.row.realization_machine,
                    meaning.row.realization_state,
                    meaning.application.report_fingerprint,
                    meaning.application.commitment,
                ),
                meaning.trait_definition,
                meaning.requirement,
            ),
            ScopedOperatorApplication::BoundTrait(meaning) => (
                CheckedOperatorCandidateFact::root(meaning.requirement.symbol),
                meaning.trait_definition,
                meaning.requirement,
            ),
            ScopedOperatorApplication::Trait(meaning) => (
                CheckedOperatorCandidateFact::root(meaning.requirement.symbol),
                meaning.trait_definition,
                meaning.requirement,
            ),
        };
        candidate.with_signature(
            program
                .state_signature_parameters(requirement)
                .first()
                .map_or_else(TypeReferenceHandle::invalid, |parameter| {
                    parameter.type_reference
                }),
            requirement.return_type,
            requirement.contracts,
            program.trait_type_parameters(definition).len()
                + program.state_signature_type_parameters(requirement).len(),
            program.state_signature_parameters(requirement).len(),
            definition.is_boundary,
        )
    }

    fn indexed_operator_spelling(
        program: &dyn crate::CheckingInput,
        index: ExpressionHandle,
    ) -> OperatorSpelling {
        if index.is_valid()
            && matches!(
                expression_table(program).expression(index),
                ExpressionNode::Range(_)
            )
        {
            OperatorSpelling::Range
        } else {
            OperatorSpelling::Index
        }
    }

    pub fn indexed_operand_types(
        program: &dyn crate::CheckingInput,
        indexed: &TableIndexedExpression,
        origin: CheckedValueOrigin,
    ) -> Vec<Option<TypeReferenceHandle>> {
        let mut operand_types = vec![expression_type_reference_for_origin(
            program,
            indexed.collection,
            origin,
        )];
        match expression_table(program).expression(indexed.index) {
            ExpressionNode::Range(range) => {
                operand_types.push(expression_type_reference_for_origin(
                    program,
                    range.start,
                    origin,
                ));
                operand_types.push(expression_type_reference_for_origin(
                    program, range.end, origin,
                ));
            }
            _ => operand_types.push(expression_type_reference_for_origin(
                program,
                indexed.index,
                origin,
            )),
        }
        operand_types
    }

    fn operator_use_fact(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        spelling: OperatorSpelling,
        operand_types: &[Option<TypeReferenceHandle>],
        candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
    ) -> CheckedOperatorUseFact {
        let trait_candidates =
            origin_machine_symbol(origin).map_or_else(Vec::new, |machine_symbol| {
                selected_trait_operator_meanings(program, machine_symbol, spelling, operand_types)
            });
        if !trait_candidates.is_empty() {
            return trait_operator_use_fact(
                program,
                expression,
                origin,
                spelling,
                trait_candidates,
                candidate_facts,
            );
        }
        let use_site = expression_table(program).source_span(expression);
        let candidates = if matches!(spelling, OperatorSpelling::Index | OperatorSpelling::Range) {
            typed_trees::operator::resolve_indexed_spelling_for_operands(
                program,
                spelling,
                operand_types,
                use_site,
            )
        } else {
            resolve_spelling_for_operands(program, spelling, operand_types, use_site)
        };
        let (candidate_count, candidate_span) =
            checked_candidate_span(program, candidate_facts, &candidates);
        let (status, selected_operator_symbol) =
            if let Some(status) = domain_or_unique_operator_status(&candidates) {
                status
            } else if candidates.is_empty() {
                let indexed_spelling = matches!(
                    expression_table(program).expression(expression),
                    ExpressionNode::Indexed(indexed)
                        if indexed_operator_spelling(program, indexed.index) == spelling
                );
                let builtin_geometry = operand_types
                    .first()
                    .copied()
                    .flatten()
                    .and_then(|reference| crate::unwrapped_type_reference(program, reference))
                    .is_some_and(|reference| {
                        matches!(
                            program.type_reference_table.type_reference(reference),
                            typed_trees::types::TypeReferenceNode::FixedArray { .. }
                                | typed_trees::types::TypeReferenceNode::Slice { .. }
                        )
                    });
                let builtin_indexed = indexed_spelling
                    && origin_machine_symbol(origin).is_some_and(|machine_symbol| {
                        builtin_geometry
                            && if spelling == OperatorSpelling::Index {
                                typed_trees::operator::builtin_index_read_is_spelled(
                                    program,
                                    machine_symbol,
                                    expression,
                                    operand_types,
                                )
                            } else {
                                typed_trees::operator::has_builtin_spelled_expression_meaning(
                                    program,
                                    machine_symbol,
                                    expression,
                                    spelling,
                                    operand_types,
                                )
                            }
                    });
                (
                    if builtin_indexed {
                        CheckedOperatorResolutionStatus::BuiltinFallback
                    } else {
                        CheckedOperatorResolutionStatus::Missing
                    },
                    SymbolHandle::invalid(),
                )
            } else {
                (
                    CheckedOperatorResolutionStatus::Ambiguous,
                    SymbolHandle::invalid(),
                )
            };

        operator_use_record(
            program,
            expression,
            origin,
            spelling,
            selected_operator_symbol,
            candidate_span,
            candidate_count,
            status,
        )
    }

    fn origin_machine_symbol(origin: CheckedValueOrigin) -> Option<SymbolHandle> {
        match origin {
            CheckedValueOrigin::MachineDecrease { machine_symbol, .. }
            | CheckedValueOrigin::MachineOwnedDataInitializer { machine_symbol, .. }
            | CheckedValueOrigin::StateStatement { machine_symbol, .. } => Some(machine_symbol),
            CheckedValueOrigin::NestedExpression { .. } => None,
        }
    }

    fn trait_operator_use_fact(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        spelling: OperatorSpelling,
        candidates: Vec<SelectedTraitOperatorMeaning<'_>>,
        candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
    ) -> CheckedOperatorUseFact {
        let candidate_count = candidates.len();
        let candidate_span = candidate_facts.insert_many(candidates.iter().map(|candidate| {
            CheckedOperatorCandidateFact::trait_backed(
                candidate.requirement.symbol,
                candidate.application.declaration,
                candidate.row.realization_machine,
                candidate.row.realization_state,
                candidate.application.report_fingerprint,
                candidate.application.commitment,
            )
            .with_signature(
                program
                    .state_signature_parameters(candidate.requirement)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or_else(TypeReferenceHandle::invalid),
                candidate.requirement.return_type,
                candidate.requirement.contracts,
                program
                    .trait_type_parameters(candidate.trait_definition)
                    .len()
                    + program
                        .state_signature_type_parameters(candidate.requirement)
                        .len(),
                program
                    .state_signature_parameters(candidate.requirement)
                    .len(),
                candidate.trait_definition.is_boundary,
            )
        }));
        let (status, selected_operator_symbol) = if let [candidate] = candidates.as_slice() {
            (
                CheckedOperatorResolutionStatus::Resolved,
                candidate.requirement.symbol,
            )
        } else {
            (
                CheckedOperatorResolutionStatus::Ambiguous,
                SymbolHandle::invalid(),
            )
        };

        operator_use_record(
            program,
            expression,
            origin,
            spelling,
            selected_operator_symbol,
            candidate_span,
            candidate_count,
            status,
        )
    }

    pub(crate) fn arithmetic_policy_adapter(
        program: &dyn crate::CheckingInput,
        spelling: OperatorSpelling,
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> CheckedArithmeticPolicyAdapter {
        if !matches!(
            spelling,
            OperatorSpelling::Add
                | OperatorSpelling::Subtract
                | OperatorSpelling::Multiply
                | OperatorSpelling::Divide
        ) {
            return CheckedArithmeticPolicyAdapter::None;
        }

        let mut format = None;
        for type_reference in operand_types.iter().flatten().copied() {
            let candidate_format = match program.primitive_type_reference(type_reference) {
                Some(PrimitiveType::F32) => Some(FloatFormat::BINARY32),
                Some(PrimitiveType::F64) => Some(FloatFormat::BINARY64),
                _ => None,
            };
            if let Some(candidate_format) = candidate_format {
                if format.is_some_and(|format| format != candidate_format) {
                    return CheckedArithmeticPolicyAdapter::None;
                }
                format = Some(candidate_format);
            }
        }
        let Some(format) = format else {
            return CheckedArithmeticPolicyAdapter::None;
        };
        crate::value_custody::expression_types::operand_arithmetic_domain(program, operand_types)
            .map_or(CheckedArithmeticPolicyAdapter::None, |domain| {
                float_policy_adapter(format, domain)
            })
    }

    fn float_policy_adapter(
        format: FloatFormat,
        domain: ArithmeticDomain,
    ) -> CheckedArithmeticPolicyAdapter {
        match domain {
            ArithmeticDomain::Saturating => {
                CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { format }
            }
            ArithmeticDomain::Trapping => {
                CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { format }
            }
            ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {
                CheckedArithmeticPolicyAdapter::None
            }
        }
    }

    pub fn checked_operator_candidate(
        program: &dyn crate::CheckingInput,
        candidate: &SpelledOperator<'_>,
    ) -> CheckedOperatorCandidateFact {
        let fact = if let Some(domain) = candidate.domain {
            CheckedOperatorCandidateFact::domain(candidate.operator.symbol, domain.symbol)
        } else {
            CheckedOperatorCandidateFact::root(candidate.operator.symbol)
        };
        fact.with_signature(
            program
                .operator_parameters(candidate.operator)
                .first()
                .map(|parameter| parameter.type_reference)
                .unwrap_or_else(TypeReferenceHandle::invalid),
            candidate.operator.return_type,
            candidate.operator.contracts,
            program.operator_type_parameters(candidate.operator).len(),
            program.operator_parameters(candidate.operator).len(),
            candidate.operator.is_boundary,
        )
    }
}

pub use contracts::{
    build_operator_facts, derive_checked_operator_crash_contracts,
    derive_pre_flow_operator_selections,
};
pub use receiver::expression_type_reference_for_origin;
pub use requirement_uses::token_bound_machine_call_target;
pub use selection::select_pending_domain_operator_meanings;
pub use uses::{
    build_operator_uses, checked_operator_candidate,
    collect_pre_specialization_operator_selections, indexed_operand_types,
};

pub(crate) use applications::retain_validated_boundary_operator_application;
pub(crate) use applications::validate_named_statement_operator_application;
pub use applications::{
    ValidatedBoundaryOperatorApplication, ValidatedBoundaryOperatorApplicationArgument,
    ValidatedBoundaryOperatorApplicationUseSite, canonical_closed_operator_realization_bytes,
    landed_integer_literal_type_reference, validate_closed_operator_application,
    validate_named_operator_application, validated_boundary_operator_application,
    validated_symbolic_boundary_operator_application,
};

use crate::declarations::symbols::TopLevelSymbols;
use crate::value_custody::type_references::{
    TypeParameterScope, TypeReferenceOwner, validate_type_reference_handle,
};
use diagnostics::Diagnostic;
use signatures::{operator_name, operator_operand_key, operator_signature_key};

pub fn comparison_operator_spelling(
    operator: typed_trees::expression::BinaryOperator,
) -> Option<language_core::OperatorSpelling> {
    use typed_trees::expression::BinaryOperator;
    match operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            typed_trees::operator::binary_operator_spelling(operator)
        }
        _ => None,
    }
}

pub(crate) fn validate_operator_declarations(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    validate_duplicate_operator_signatures(program, "root", program.operators(), diagnostics);
    validate_spelling_overlap(program, program.operators(), diagnostics);
    for operator in program.operators() {
        validate_operator_types(
            program,
            symbols,
            operator,
            diagnostics,
            formation_obligations,
        );
    }

    for domain in program.domain_definitions() {
        validate_duplicate_operator_signatures(
            program,
            domain.name.as_str(),
            program.domain_operators(domain),
            diagnostics,
        );
        validate_spelling_overlap(program, program.domain_operators(domain), diagnostics);
        for operator in program.domain_operators(domain) {
            validate_operator_types(
                program,
                symbols,
                operator,
                diagnostics,
                formation_obligations,
            );
        }
    }
}

fn validate_operator_types(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    operator: &typed_trees::operator::OperatorDefinition,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let name = operator_name(program, operator);
    for contract in program.operator_contracts(operator) {
        let typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind
        else {
            continue;
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if !matches!(fact, typed_trees::domain::ProofFact::Expression(_)) {
                diagnostics.push(Diagnostic::error(format!(
                    "operator `{name}` `crashes {cause:?}` routes must be Boolean expressions; domain memberships and proposition applications are proof facts, not runtime-refinable crash routes",
                )));
            }
        }
    }
    let type_parameters = program.operator_type_parameters(operator);
    for parameter in program.operator_parameters(operator) {
        validate_type_reference_handle(
            program,
            parameter.type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::OperatorParameter {
                operator: &name,
                parameter: parameter.name.as_str(),
                generic_depth: 0,
            },
            TypeParameterScope::authored(
                operator.symbol,
                type_parameters,
                &operator.lifetime_parameters,
            ),
            formation_obligations,
        );
    }
    if operator.return_type.is_valid() {
        validate_type_reference_handle(
            program,
            operator.return_type,
            symbols,
            diagnostics,
            TypeReferenceOwner::OperatorReturn {
                operator: &name,
                generic_depth: 0,
            },
            TypeParameterScope::authored(
                operator.symbol,
                type_parameters,
                &operator.lifetime_parameters,
            ),
            formation_obligations,
        );
    }
}

fn validate_spelling_overlap(
    program: &dyn crate::CheckingInput,
    operators: &[typed_trees::operator::OperatorDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let keys = operators
        .iter()
        .map(|operator| operator_operand_key(program, operator))
        .collect::<Vec<_>>();
    for (operator_index, operator) in operators.iter().enumerate() {
        let Some(spelling) = operator.spelling else {
            continue;
        };
        let operand_key = match &keys[operator_index] {
            Ok(identity) => identity,
            Err(error) => {
                diagnostics.push(Diagnostic::error(error.to_string()));
                continue;
            }
        };

        let overlaps = operators[..operator_index]
            .iter()
            .enumerate()
            .any(|(index, previous)| {
                previous.spelling == Some(spelling)
                    && keys[index]
                        .as_ref()
                        .is_ok_and(|previous| previous == operand_key)
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, operator.symbol)
            });

        if overlaps {
            diagnostics.push(Diagnostic::error(format!(
                "operator spelling `{}` has overlapping operand types `({operand_key})`; \
                 the same spelling with overlapping receiver/operand types is ambiguous",
                spelling.symbol()
            )));
        }
    }
}

fn validate_duplicate_operator_signatures(
    program: &dyn crate::CheckingInput,
    owner: &str,
    operators: &[typed_trees::operator::OperatorDefinition],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let keys = operators
        .iter()
        .map(|operator| operator_signature_key(program, operator))
        .collect::<Vec<_>>();
    for (operator_index, operator) in operators.iter().enumerate() {
        let signature = match &keys[operator_index] {
            Ok(identity) => identity,
            Err(error) => {
                diagnostics.push(Diagnostic::error(error.to_string()));
                continue;
            }
        };
        let previous = operators[..operator_index]
            .iter()
            .enumerate()
            .filter(|(index, previous)| {
                keys[*index]
                    .as_ref()
                    .is_ok_and(|previous| previous == signature)
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, operator.symbol)
            })
            .map(|(_, previous)| previous)
            .collect::<Vec<_>>();
        if previous.is_empty() {
            continue;
        }
        if operator.is_boundary
            && operator.spelling.is_none()
            && previous
                .iter()
                .all(|item| item.is_boundary && item.spelling.is_none())
        {
            let identity = program.operator_callable_identity(operator);
            let result_dispatch = program.callable_identity_result_dispatch(identity);
            if previous.iter().any(|previous| {
                program
                    .callable_identity_result_dispatch(program.operator_callable_identity(previous))
                    == result_dispatch
            }) {
                let dispatch = if program.callable_result_dispatch_is_empty(result_dispatch) {
                    "<empty>".to_owned()
                } else {
                    program.callable_result_dispatch_text(result_dispatch)
                };
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate named requirement overload `{}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
                    program.callable_path_text(program.callable_identity_path(identity)),
                    program.callable_parameters_text(program.callable_identity_parameters(identity)),
                )));
            }
            continue;
        }
        let name = operator_name(program, operator);
        if owner == "root" {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate operator declaration `{name}`"
            )));
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "domain `{owner}` has duplicate operator `{name}`"
            )));
        }
    }
}
