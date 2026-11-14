/*
We represent symbol-resolved value expressions (`values::expression`), static
member projections, and symbol/table subtree copying in `SymbolResolvedTrees`.
*/

pub mod expression {

    pub(crate) mod display {
        use diagnostics::{bracketed_display_names, comma_join_display};

        use crate::expression::{
            BinaryOperator, ExpressionNode, ExpressionTable, StaticMachineArgument,
            TableBinaryExpression, TableCallExpression, TableCastExpression, TableUnaryExpression,
            UnaryOperator,
        };
        use crate::name::DiagnosticName;

        impl ExpressionNode {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                match self {
                    Self::Match(dispatch) => {
                        let arms = table
                            .match_arms(dispatch.arms)
                            .iter()
                            .map(|arm| {
                                let pattern = match arm.pattern {
                                    crate::expression::MatchPattern::Value(value) => {
                                        table.display_name(value)
                                    }
                                    crate::expression::MatchPattern::Destructure(pattern) => {
                                        display_destructure_pattern(table, pattern)
                                    }
                                    crate::expression::MatchPattern::Wildcard => "_".to_owned(),
                                };
                                format!("{pattern} => {}", table.display_name(arm.value))
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!(
                            "match {} {{ {arms} }}",
                            table.display_name(dispatch.subject)
                        )
                    }
                    Self::SavedSubject(saved) => {
                        format!("<saved subject #{}>", saved.capture.arena_index())
                    }
                    Self::ArrayLiteral(values) => {
                        bracketed_display_names(table.expression_handles(*values).iter(), |value| {
                            table.display_name(*value)
                        })
                    }
                    Self::Atomic(atomic) => format!(
                        "atomic[{:?}]({})",
                        atomic.ordering,
                        table.display_name(atomic.value)
                    ),
                    Self::Binary(binary) => binary.display_name(table),
                    Self::Boolean(value) => value.to_string(),
                    Self::Cast(cast) => cast.display_name(table),
                    Self::Call(call) => call.display_name(table),
                    Self::Float(value) => value.to_string(),
                    Self::UndeterminedNaN(format) => {
                        format!("<undetermined NaN {}>", format.name())
                    }
                    Self::Indexed(indexed) => {
                        format!(
                            "{}[{}]",
                            table.display_name(indexed.collection),
                            table.display_name(indexed.index)
                        )
                    }
                    Self::Integer(value) => value.to_string(),
                    Self::Membership(membership) => {
                        format!(
                            "{} in {}",
                            table.display_name(membership.value),
                            display_name_path(table.name_path_members(membership.domain), "::")
                        )
                    }
                    Self::Member(member) => {
                        format!("{}.{}", table.display_name(member.receiver), member.member)
                    }
                    Self::Borrow(expression) => format!(
                        "{}{}",
                        expression.access.borrow_prefix(),
                        table.display_name(expression.target)
                    ),
                    Self::Name(path) => {
                        if let Some(argument) = table.static_member(path.static_application) {
                            argument.display_name_in(table)
                        } else if path.static_application.is_valid() {
                            "<missing applied declaration>".to_owned()
                        } else {
                            display_name_path(table.name_path_members(path.members), "::")
                        }
                    }
                    Self::Range(range) => match (range.start.is_valid(), range.end.is_valid()) {
                        (true, true) => format!(
                            "{}..{}",
                            table.display_name(range.start),
                            table.display_name(range.end)
                        ),
                        (true, false) => format!("{}..", table.display_name(range.start)),
                        (false, true) => format!("..{}", table.display_name(range.end)),
                        (false, false) => "..".to_string(),
                    },
                    Self::StructLiteral(struct_literal) => struct_literal.type_name.to_string(),
                    Self::String(value) => source::display_literal_bytes(value),
                    Self::Unary(unary) => unary.display_name(table),
                    Self::TypeExpression(_) => "type".to_owned(),
                    Self::ZeroValue(_) => "zero_value<type>()".to_owned(),
                }
            }
        }

        fn display_destructure_pattern(
            table: &ExpressionTable,
            pattern: crate::expression::TableDestructurePattern,
        ) -> String {
            use crate::expression::MatchFieldAction;
            let mut fields = table
                .match_fields(pattern.fields)
                .iter()
                .map(|field| {
                    let member = field.member.as_str();
                    match &field.action {
                        MatchFieldAction::Bind(binding) if binding.as_str() == member => {
                            member.to_owned()
                        }
                        MatchFieldAction::Bind(binding) => {
                            format!("{member} as {}", binding.as_str())
                        }
                        MatchFieldAction::Equal(value) => {
                            format!("{member}: {}", table.display_name(*value))
                        }
                        MatchFieldAction::Waive => format!("{member} as _"),
                    }
                })
                .collect::<Vec<_>>();
            if pattern.has_rest {
                fields.push("..".to_owned());
            }
            let selectors = table.match_proof_selectors(pattern.proof_selectors);
            let proofs = if selectors.is_empty() {
                String::new()
            } else {
                format!(
                    "; {}",
                    selectors
                        .iter()
                        .map(|selector| format!(
                            "{}: {}",
                            selector.output_field.as_str(),
                            selector.binding.as_str()
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let guard = if pattern.guard.is_valid() {
                format!(" if {}", table.display_name(pattern.guard))
            } else {
                String::new()
            };
            format!(
                "{} {{ {}{proofs} }}{guard}",
                table.display_name(pattern.head),
                fields.join(", ")
            )
        }

        pub fn display_name_path(path: &[DiagnosticName], separator: &str) -> String {
            let byte_count = path.iter().map(|name| name.as_str().len()).sum::<usize>()
                + separator.len().saturating_mul(path.len().saturating_sub(1));
            let mut display_name = String::with_capacity(byte_count);

            for (index, name) in path.iter().enumerate() {
                if index > 0 {
                    display_name.push_str(separator);
                }

                display_name.push_str(name.as_str());
            }

            display_name
        }

        impl TableBinaryExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                format!(
                    "{} {}{} {}",
                    table.display_name(self.left),
                    self.operator.display_name(),
                    self.policy.operator_suffix(),
                    table.display_name(self.right)
                )
            }
        }

        impl TableUnaryExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                format!(
                    "{}{}",
                    self.operator.display_name(),
                    table.display_name(self.operand)
                )
            }
        }

        impl TableCastExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                let target_type =
                    display_name_path(table.name_path_members(self.target_label), "::");
                format!("{} as {}", table.display_name(self.value), target_type)
            }
        }

        impl TableCallExpression {
            pub fn display_name(&self, table: &ExpressionTable) -> String {
                let machine_arguments = if self.machine_arguments.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        self.machine_arguments
                            .iter()
                            .map(|argument| argument.display_name_in(table))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let arguments =
                    comma_join_display(table.expression_handles(self.arguments), |argument| {
                        table.display_name(*argument)
                    });

                if self.receiver.is_valid() {
                    format!(
                        "{}.{}{machine_arguments}({arguments})",
                        table.display_name(self.receiver),
                        self.target
                    )
                } else {
                    format!("{}{machine_arguments}({arguments})", self.target)
                }
            }
        }

        impl StaticMachineArgument {
            pub fn display_name_in(&self, table: &ExpressionTable) -> String {
                self.display_name_with_owner(Some(table))
            }

            fn display_name_with_owner(&self, table: Option<&ExpressionTable>) -> String {
                if self.type_reference.is_valid() {
                    return "<structural type>".to_owned();
                }
                if let Some(literal) = &self.const_literal {
                    return literal.to_string();
                }
                if let Some(projection) = &self.evidence_projection {
                    return format!("{}.{}", projection.term, projection.member);
                }
                let mut rendered = display_name_path(&self.path, "::");
                if let Some(application) = &self.application {
                    let mut arguments = application
                        .lifetime_arguments
                        .iter()
                        .map(|lifetime| format!("'{lifetime}"))
                        .collect::<Vec<_>>();
                    arguments.extend(
                        application
                            .arguments
                            .iter()
                            .map(|argument| argument.display_name_with_owner(table)),
                    );
                    rendered.push('<');
                    rendered.push_str(&arguments.join(", "));
                    rendered.push('>');
                    if application.member.is_valid() {
                        rendered.push_str("::");
                        match table.and_then(|table| table.static_member(application.member)) {
                            Some(member) => {
                                rendered.push_str(&member.display_name_with_owner(table))
                            }
                            None => rendered.push_str("<qualified member requires owning table>"),
                        }
                    }
                }
                rendered
            }
        }

        impl BinaryOperator {
            pub fn display_name(self) -> &'static str {
                match self {
                    Self::Add => "+",
                    Self::And => "&&",
                    Self::BitwiseAnd => "&",
                    Self::BitwiseOr => "|",
                    Self::BitwiseXor => "^",
                    Self::Divide => "/",
                    Self::Equal => "==",
                    Self::Greater => ">",
                    Self::GreaterOrEqual => ">=",
                    Self::Less => "<",
                    Self::LessOrEqual => "<=",
                    Self::Modulo => "%",
                    Self::Multiply => "*",
                    Self::NotEqual => "!=",
                    Self::Or => "||",
                    Self::ShiftLeft => "<<",
                    Self::ShiftRight => ">>",
                    Self::Subtract => "-",
                }
            }
        }

        impl UnaryOperator {
            pub fn display_name(self) -> &'static str {
                match self {
                    Self::BitwiseNot => "~",
                    Self::LogicalNot => "!",
                }
            }
        }
    }
    pub(crate) mod static_members {
        use super::{ExpressionTable, StaticMachineArgument};
        use symbols::SymbolHandle;

        impl ExpressionTable {
            pub fn copy_static_argument_from(
                &mut self,
                source: &Self,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                self.copy_static_argument_members(Some(source), argument)
            }

            pub fn copy_static_argument_from_self(
                &mut self,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                self.copy_static_argument_members(None, argument)
            }

            fn copy_static_argument_members(
                &mut self,
                source: Option<&Self>,
                argument: &StaticMachineArgument,
            ) -> StaticMachineArgument {
                let mut copied = argument.clone();
                if let Some(application) = &mut copied.application {
                    application.arguments = application
                        .arguments
                        .iter()
                        .map(|argument| self.copy_static_argument_members(source, argument))
                        .collect();
                    if application.member.is_valid() {
                        let member = source
                            .unwrap_or(self)
                            .static_member(application.member)
                            .expect(
                                "qualified static member belongs to its source expression table",
                            )
                            .clone();
                        let copied_member = self.copy_static_argument_members(source, &member);
                        application.member = self.insert_static_member(copied_member);
                    }
                }
                copied
            }

            pub fn remap_static_argument_symbols(
                &mut self,
                argument: &mut StaticMachineArgument,
                symbols: &[(SymbolHandle, SymbolHandle)],
            ) {
                argument.authored_selection_target =
                    argument.authored_selection_target.map(|target| {
                        symbols
                            .iter()
                            .find(|(source, _)| *source == target)
                            .map_or(target, |(_, replacement)| *replacement)
                    });
                if let Some((_, replacement)) = symbols
                    .iter()
                    .find(|(source, _)| *source == argument.symbol)
                {
                    argument.symbol = *replacement;
                }
                if let Some(application) = &mut argument.application {
                    for argument in application.arguments.iter_mut() {
                        self.remap_static_argument_symbols(argument, symbols);
                    }
                    if application.member.is_valid() {
                        let mut member = self
                            .static_member(application.member)
                            .expect(
                                "qualified static member belongs to its receiving expression table",
                            )
                            .clone();
                        self.remap_static_argument_symbols(&mut member, symbols);
                        *self.static_member_mut(application.member).expect(
                            "qualified static member remains in its receiving expression table",
                        ) = member;
                    }
                }
            }
        }
    }
    pub(crate) mod symbol_copies {
        use crate::SymbolResolvedTrees;
        use crate::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
        use crate::domain::ProofFact;
        use crate::expression::{
            ExpressionHandle, ExpressionNode, MatchPattern, StaticMachineArgument,
        };
        use crate::types::{
            FixedArrayLength, GenericApplicationOrigin, OperandLifetimeOwner,
            OperandLifetimeOwnerHandle, TypeConstraint, TypeReference,
        };
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;

        impl SymbolResolvedTrees {
            pub fn copy_expression_with_symbols(
                &mut self,
                source: ExpressionHandle,
                substitutions: &[(SymbolHandle, SymbolHandle)],
            ) -> ExpressionHandle {
                SymbolCopy {
                    program: self,
                    substitutions,
                    type_arguments: &[],
                    types: Vec::new(),
                    callables: Vec::new(),
                    lifetime_owners: Vec::new(),
                    visited_payloads: Vec::new(),
                }
                .expression(source)
            }

            pub fn copy_proof_fact_with_symbols(
                &mut self,
                source: Handle<ProofFact>,
                substitutions: &[(SymbolHandle, SymbolHandle)],
            ) -> Handle<ProofFact> {
                SymbolCopy {
                    program: self,
                    substitutions,
                    type_arguments: &[],
                    types: Vec::new(),
                    callables: Vec::new(),
                    lifetime_owners: Vec::new(),
                    visited_payloads: Vec::new(),
                }
                .proof_fact(source)
            }

            /// Copy a selected declaration's type operands into an application.
            /// Exact binder identities select replacements; retained source names
            /// continue to identify the declarations which supplied the type.
            pub fn copy_type_arguments(
                &mut self,
                source: HandleSpan<TypeReference>,
                type_arguments: &[(SymbolHandle, TypeReference)],
            ) -> HandleSpan<TypeReference> {
                let substitutions = type_arguments
                    .iter()
                    .filter_map(|(formal, actual)| {
                        if let TypeReference::Named { symbol, .. } = actual {
                            Some((*formal, *symbol))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                SymbolCopy {
                    program: self,
                    substitutions: &substitutions,
                    type_arguments,
                    types: Vec::new(),
                    callables: Vec::new(),
                    lifetime_owners: Vec::new(),
                    visited_payloads: Vec::new(),
                }
                .type_span(source)
            }

            /// Transport the complete callable type to another declaration's
            /// parameter. Local binders receive derived children of that exact
            /// parameter; supplied outer operands retain their own context.
            /// Copying only parameter/result types would lose the contract and
            /// sharing the original locals would capture another use's binders.
            pub fn copy_callable_signature(
                &mut self,
                source: &crate::signature::StateSignature,
                destination: SymbolHandle,
                type_arguments: &[(SymbolHandle, TypeReference)],
                selections: &[(SymbolHandle, SymbolHandle)],
            ) -> crate::signature::StateSignature {
                let mut substitutions = vec![(source.symbol, destination)];
                let mut cursor = 0;
                while cursor < substitutions.len() {
                    let (original, copied) = substitutions[cursor];
                    cursor += 1;
                    let children = self
                        .symbols
                        .child_handles(original)
                        .into_iter()
                        .flatten()
                        .map(|child| {
                            (
                                child,
                                self.symbols.get(child).kind,
                                self.symbols.name(child).to_owned(),
                            )
                        })
                        .collect::<Vec<_>>();
                    let copies = self.symbols.append_generated_children_from(
                        copied,
                        children
                            .iter()
                            .map(|(origin, kind, name)| (*origin, *kind, name.as_str())),
                    );
                    substitutions.extend(
                        children
                            .iter()
                            .zip(copies.handles())
                            .map(|((original, _, _), copied)| (*original, copied)),
                    );
                }
                substitutions.extend(
                    selections
                        .iter()
                        .copied()
                        .filter(|(original, _)| *original != source.symbol),
                );
                substitutions.extend(type_arguments.iter().filter_map(|(formal, actual)| {
                    if let TypeReference::Named { symbol, .. } = actual {
                        Some((*formal, *symbol))
                    } else {
                        None
                    }
                }));
                SymbolCopy {
                    program: self,
                    substitutions: &substitutions,
                    type_arguments,
                    types: Vec::new(),
                    callables: Vec::new(),
                    lifetime_owners: Vec::new(),
                    visited_payloads: Vec::new(),
                }
                .signature(source)
            }
        }

        struct SymbolCopy<'program, 'substitutions> {
            program: &'program mut SymbolResolvedTrees,
            substitutions: &'substitutions [(SymbolHandle, SymbolHandle)],
            type_arguments: &'substitutions [(SymbolHandle, TypeReference)],
            types: Vec<(Handle<TypeReference>, Handle<TypeReference>)>,
            callables: Vec<(
                crate::callable_values::RuntimeCallableContractHandle,
                crate::callable_values::RuntimeCallableContractHandle,
            )>,
            lifetime_owners: Vec<(OperandLifetimeOwnerHandle, OperandLifetimeOwnerHandle)>,
            visited_payloads: Vec<ExpressionHandle>,
        }

        impl SymbolCopy<'_, '_> {
            fn signature(
                &mut self,
                source: &crate::signature::StateSignature,
            ) -> crate::signature::StateSignature {
                let mut copied = source.clone();
                copied.symbol = self.symbol(source.symbol);
                let parameters = self
                    .program
                    .data_type_parameters(source.type_parameters)
                    .to_vec();
                let parameters = parameters
                    .into_iter()
                    .map(|parameter| self.parameter(parameter))
                    .collect::<Vec<_>>();
                copied.type_parameters = self
                    .program
                    .declarations_mut()
                    .data_type_parameters
                    .insert_many(parameters);
                for bound in &mut copied.conformance_bounds {
                    bound.binder = self.symbol(bound.binder);
                    bound.trait_symbol = self.symbol(bound.trait_symbol);
                    bound.arguments = self.type_span(bound.arguments);
                }
                copied.parameters = self.parameters(source.parameters);
                copied.return_type = source
                    .return_type
                    .clone()
                    .map(|reference| self.type_payload(reference));
                let invokes = self.program.signature_invokes(source.invokes).to_vec();
                copied.invokes = self
                    .program
                    .declarations_mut()
                    .signature_invokes
                    .insert_many(invokes);
                let contracts = self.program.signature_contracts(source.contracts).to_vec();
                let contracts = contracts
                    .into_iter()
                    .map(|mut contract| {
                        if let crate::signature::SignatureContractKind::EnsuresForResultCase {
                            result_data,
                            result_case,
                        } = &mut contract.kind
                        {
                            *result_data = self.symbol(*result_data);
                            *result_case = self.symbol(*result_case);
                        }
                        contract.facts = self.proof_span(contract.facts);
                        contract
                    })
                    .collect::<Vec<_>>();
                copied.contracts = self
                    .program
                    .declarations_mut()
                    .signature_contracts
                    .insert_many(contracts);
                let rows = self
                    .program
                    .authored_service_reach_rows_for(source.symbol)
                    .cloned()
                    .collect::<Vec<_>>();
                for mut row in rows {
                    row.owner = copied.symbol;
                    for target in &mut row.targets {
                        target.service = self.symbol(target.service);
                    }
                    // Runtime rows can share their formal namespace while their
                    // supplying scope changes. Preserve each distinct authored
                    // row without duplicating an unchanged declaration record.
                    if !self.program.authored_service_reach_rows.contains(&row) {
                        self.program.authored_service_reach_rows.push(row);
                    }
                }
                copied
            }

            fn parameters(
                &mut self,
                source: HandleSpan<crate::signature::StateParameter>,
            ) -> HandleSpan<crate::signature::StateParameter> {
                let parameters = self.program.state_parameters(source).to_vec();
                let parameters = parameters
                    .into_iter()
                    .map(|mut parameter| {
                        parameter.symbol = self.symbol(parameter.symbol);
                        parameter.type_reference = self.type_payload(parameter.type_reference);
                        parameter
                    })
                    .collect::<Vec<_>>();
                self.program
                    .declarations_mut()
                    .state_parameters
                    .insert_many(parameters)
            }

            fn parameter(&mut self, mut parameter: TypeParameter) -> TypeParameter {
                parameter.symbol = self.symbol(parameter.symbol);
                parameter.kind = match parameter.kind {
                    TypeParameterKind::Const { type_reference } => TypeParameterKind::Const {
                        type_reference: self.type_payload(type_reference),
                    },
                    TypeParameterKind::Value { type_reference } => TypeParameterKind::Value {
                        type_reference: self.type_payload(type_reference),
                    },
                    TypeParameterKind::Machine { contract } => TypeParameterKind::Machine {
                        contract: match contract {
                            MachineParameterContract::Structural(signature) => {
                                MachineParameterContract::Structural(self.signature(&signature))
                            }
                            MachineParameterContract::AuthoredNominal {
                                requirement,
                                trait_arguments,
                            } => MachineParameterContract::AuthoredNominal {
                                requirement,
                                trait_arguments: self.type_span(trait_arguments),
                            },
                            MachineParameterContract::Nominal {
                                trait_definition,
                                requirement,
                                authored_path,
                                trait_arguments,
                            } => MachineParameterContract::Nominal {
                                trait_definition: self.symbol(trait_definition),
                                requirement: self.symbol(requirement),
                                authored_path,
                                trait_arguments: self.type_span(trait_arguments),
                            },
                            other => other,
                        },
                    },
                    TypeParameterKind::Proposition { mut contract } => {
                        contract.parameters = self.parameters(contract.parameters);
                        TypeParameterKind::Proposition { contract }
                    }
                    other => other,
                };
                parameter
            }

            fn proof_fact(&mut self, source: Handle<ProofFact>) -> Handle<ProofFact> {
                if !source.is_valid() {
                    return source;
                }
                let (fact, source_span) = self.proof_fact_payload(source);
                let copied = self.program.declarations_mut().proof_facts.insert(fact);
                if let Some(source_span) = source_span {
                    self.program.set_proof_fact_source_span(copied, source_span);
                }
                copied
            }

            fn proof_fact_payload(
                &mut self,
                source: Handle<ProofFact>,
            ) -> (ProofFact, Option<source::SourceSpan>) {
                let source_span = self.program.proof_fact_source_span(source);
                let fact = self.program.declarations().proof_facts.get(source).clone();
                (self.fact_payload(fact), source_span)
            }

            fn fact_payload(&mut self, fact: ProofFact) -> ProofFact {
                match fact {
                    ProofFact::Expression(expression) => {
                        ProofFact::Expression(self.expression(expression))
                    }
                    ProofFact::Membership(mut membership) => {
                        membership.value = self.expression(membership.value);
                        let names = self.program.domain_path_members(membership.domain).to_vec();
                        membership.domain = self
                            .program
                            .declarations_mut()
                            .domain_path_members
                            .insert_many(names);
                        membership.domain_symbol = self.symbol(membership.domain_symbol);
                        membership.domain_arguments = self.type_span(membership.domain_arguments);
                        ProofFact::Membership(membership)
                    }
                }
            }

            fn symbol(&self, source: SymbolHandle) -> SymbolHandle {
                if !source.is_valid() {
                    return source;
                }
                self.substitutions
                    .iter()
                    .find(|(original, _)| *original == source)
                    .map_or(source, |(_, replacement)| *replacement)
            }

            fn expression(&mut self, source: ExpressionHandle) -> ExpressionHandle {
                if !source.is_valid() {
                    return source;
                }
                let copied = self.program.expression_table_mut().copy_from_self(source);
                self.expression_payloads(copied);
                copied
            }

            fn expression_payloads(&mut self, copied: ExpressionHandle) {
                if !copied.is_valid() {
                    return;
                }
                let position = copied.arena_index() as usize;
                if self.visited_payloads.get(position) == Some(&copied) {
                    return;
                }
                if self.visited_payloads.len() <= position {
                    self.visited_payloads
                        .resize(position + 1, Handle::invalid());
                }
                self.visited_payloads[position] = copied;
                let authored = self
                    .program
                    .expression_table()
                    .materialization_source(copied);
                self.expression_payloads(authored);
                let mut node = self.program.expression_table().expression(copied).clone();
                match &mut node {
                    ExpressionNode::Match(dispatch) => {
                        self.expression_payloads(dispatch.subject);
                        let arms = self
                            .program
                            .expression_table()
                            .match_arms(dispatch.arms)
                            .to_vec();
                        for (ordinal, mut arm) in arms.into_iter().enumerate() {
                            match arm.pattern {
                                MatchPattern::Value(pattern) => self.expression_payloads(pattern),
                                MatchPattern::Wildcard => {}
                                MatchPattern::Destructure(mut pattern) => {
                                    pattern.application_type =
                                        self.type_reference(pattern.application_type);
                                    arm.pattern = MatchPattern::Destructure(pattern);
                                    self.expression_payloads(pattern.head);
                                    self.expression_payloads(pattern.guard);
                                    let fields = self
                                        .program
                                        .expression_table()
                                        .match_fields(pattern.fields)
                                        .to_vec();
                                    for (ordinal, mut field) in fields.into_iter().enumerate() {
                                        field.field_symbol = self.symbol(field.field_symbol);
                                        if let super::MatchFieldAction::Equal(value) = field.action
                                        {
                                            self.expression_payloads(value);
                                        }
                                        *self.program.expression_table_mut().match_field_mut(
                                            pattern.fields.handle_at(ordinal as u32),
                                        ) = field;
                                    }
                                }
                            }
                            self.expression_payloads(arm.value);
                            self.program
                                .expression_table_mut()
                                .match_arm_mut(dispatch.arms.handle_at(ordinal as u32))
                                .pattern = arm.pattern;
                        }
                    }
                    ExpressionNode::SavedSubject(_) => {}
                    ExpressionNode::ArrayLiteral(elements) => {
                        self.expression_span_payloads(*elements)
                    }
                    ExpressionNode::Atomic(atomic) => {
                        self.expression_payloads(atomic.value);
                        self.expression_payloads(atomic.result);
                    }
                    ExpressionNode::Binary(binary) => {
                        self.expression_payloads(binary.left);
                        self.expression_payloads(binary.right);
                    }
                    ExpressionNode::Cast(cast) => {
                        self.expression_payloads(cast.value);
                        cast.target_type = self.type_reference(cast.target_type);
                        cast.semantic_domain_arguments =
                            self.type_span(cast.semantic_domain_arguments);
                        cast.semantic_domain_symbol = self.symbol(cast.semantic_domain_symbol);
                    }
                    ExpressionNode::Call(call) => {
                        self.expression_payloads(call.callee);
                        self.expression_payloads(call.receiver);
                        self.expression_span_payloads(call.arguments);
                        call.target_symbol = self.symbol(call.target_symbol);
                        call.requirement_application =
                            self.type_reference(call.requirement_application);
                        if let Some(binding) = &mut call.authored_call_binding {
                            binding.selected = self.symbol(binding.selected);
                        }
                        for argument in call.machine_arguments.iter_mut() {
                            self.static_argument(argument);
                        }
                    }
                    ExpressionNode::Indexed(indexed) => {
                        self.expression_payloads(indexed.collection);
                        self.expression_payloads(indexed.index);
                    }
                    ExpressionNode::Membership(membership) => {
                        self.expression_payloads(membership.value);
                        membership.domain_symbol = self.symbol(membership.domain_symbol);
                        membership.case_type_symbol = self.symbol(membership.case_type_symbol);
                        membership.case_symbol = self.symbol(membership.case_symbol);
                    }
                    ExpressionNode::Member(member) => {
                        self.expression_payloads(member.receiver);
                        member.member_symbol = self.symbol(member.member_symbol);
                    }
                    ExpressionNode::Borrow(borrow) => self.expression_payloads(borrow.target),
                    ExpressionNode::Name(path) => {
                        if let Some(mut application) = self
                            .program
                            .expression_table()
                            .static_member(path.static_application)
                            .cloned()
                        {
                            self.static_argument(&mut application);
                            *self
                                .program
                                .expression_table_mut()
                                .static_member_mut(path.static_application)
                                .expect("declaration application remains in its owning table") =
                                application;
                        }
                        path.head_symbol = self.symbol(path.head_symbol);
                        path.symbol = self.symbol(path.symbol);
                        path.reserved_result_of = self.symbol(path.reserved_result_of);
                        let symbols = self
                            .program
                            .expression_table()
                            .name_path_member_symbols(path.member_symbols)
                            .to_vec();
                        for (position, source) in (0..path.member_symbols.count()).zip(symbols) {
                            let replacement = self.symbol(source);
                            self.program
                                .expression_table_mut()
                                .set_name_path_member_symbol_at_offset(
                                    path.member_symbols,
                                    position,
                                    replacement,
                                );
                        }
                    }
                    ExpressionNode::Range(range) => {
                        self.expression_payloads(range.start);
                        self.expression_payloads(range.end);
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        literal.application_type = self.type_reference(literal.application_type);
                        literal.type_symbol = self.symbol(literal.type_symbol);
                        literal.case_symbol = literal.case_symbol.map(|symbol| self.symbol(symbol));
                        let fields = self
                            .program
                            .expression_table()
                            .struct_fields(literal.fields)
                            .to_vec();
                        for (position, mut field) in (0..literal.fields.count()).zip(fields) {
                            self.expression_payloads(field.value);
                            field.field_symbol = self.symbol(field.field_symbol);
                            self.program
                                .expression_table_mut()
                                .set_struct_field_at_offset(literal.fields, position, field);
                        }
                    }
                    ExpressionNode::Unary(unary) => self.expression_payloads(unary.operand),
                    ExpressionNode::TypeExpression(reference)
                    | ExpressionNode::ZeroValue(reference) => {
                        *reference = self.type_reference(*reference)
                    }
                    ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::String(_) => {}
                }
                *self.program.expression_table_mut().expression_mut(copied) = node;
            }

            fn expression_span_payloads(&mut self, expressions: HandleSpan<ExpressionHandle>) {
                let children = self
                    .program
                    .expression_table()
                    .expression_handles(expressions)
                    .to_vec();
                for child in children {
                    self.expression_payloads(child);
                }
            }

            fn static_argument(&mut self, argument: &mut StaticMachineArgument) {
                argument.type_reference = self.type_reference(argument.type_reference);
                argument.symbol = self.symbol(argument.symbol);
                argument.authored_selection_target = argument
                    .authored_selection_target
                    .map(|target| self.symbol(target));
                if let Some(application) = &mut argument.application {
                    for child in application.arguments.iter_mut() {
                        self.static_argument(child);
                    }
                    if application.member.is_valid() {
                        let mut member = self
                            .program
                            .expression_table()
                            .static_member(application.member)
                            .expect(
                                "copied static member belongs to the receiving expression table",
                            )
                            .clone();
                        self.static_argument(&mut member);
                        *self
                            .program
                            .expression_table_mut()
                            .static_member_mut(application.member)
                            .expect("copied static member remains present") = member;
                    }
                }
            }

            fn type_reference(&mut self, source: Handle<TypeReference>) -> Handle<TypeReference> {
                if !source.is_valid() {
                    return source;
                }
                if let Some((_, copied)) =
                    self.types.iter().find(|(original, _)| *original == source)
                {
                    return *copied;
                }
                let reference = self
                    .program
                    .declarations()
                    .child_type_references
                    .get(source)
                    .clone();
                let copied = self
                    .program
                    .declarations_mut()
                    .child_type_references
                    .insert(TypeReference::Unit);
                self.types.push((source, copied));
                let reference = self.type_payload(reference);
                *self
                    .program
                    .declarations_mut()
                    .child_type_references
                    .get_mut(copied) = reference;
                self.type_origins(source, copied);
                copied
            }

            fn type_origins(
                &mut self,
                source: Handle<TypeReference>,
                copied: Handle<TypeReference>,
            ) {
                let origins = self
                    .program
                    .tables
                    .types
                    .generic_application_origins
                    .iter()
                    .filter_map(|(_, origin)| (origin.instance == source).then_some(origin.clone()))
                    .collect::<Vec<_>>();
                for origin in origins {
                    let application = self.type_reference(origin.application);
                    self.program
                        .tables
                        .types
                        .generic_application_origins
                        .insert(GenericApplicationOrigin {
                            instance: copied,
                            application,
                        });
                }
            }

            fn type_span(
                &mut self,
                source: HandleSpan<TypeReference>,
            ) -> HandleSpan<TypeReference> {
                let references = self
                    .program
                    .declarations()
                    .child_type_references
                    .span_or_empty(source)
                    .to_vec();
                let copied = self
                    .program
                    .declarations_mut()
                    .child_type_references
                    .insert_many(references.iter().map(|_| TypeReference::Unit));
                for position in 0..source.count() {
                    let original = source.handle_at(position);
                    if !self
                        .types
                        .iter()
                        .any(|(candidate, _)| *candidate == original)
                    {
                        self.types.push((original, copied.handle_at(position)));
                    }
                }
                for (position, reference) in (0..source.count()).zip(references) {
                    let destination = copied.handle_at(position);
                    let reference = self.type_payload(reference);
                    *self
                        .program
                        .declarations_mut()
                        .child_type_references
                        .get_mut(destination) = reference;
                    self.type_origins(source.handle_at(position), destination);
                }
                copied
            }

            fn type_payload(&mut self, mut reference: TypeReference) -> TypeReference {
                if let TypeReference::Named { symbol, .. } = &reference
                    && let Some((_, actual)) = self
                        .type_arguments
                        .iter()
                        .find(|(formal, _)| formal == symbol)
                {
                    return actual.clone();
                }
                let owner = reference.supplied_operand_owner();
                if owner.is_valid() {
                    let declaration = self
                        .program
                        .tables
                        .types
                        .operand_lifetime_owners
                        .get(owner)
                        .declaration;
                    let copied = self.symbol(declaration);
                    if copied != declaration {
                        let owner = match self
                            .lifetime_owners
                            .iter()
                            .find(|(original, _)| *original == owner)
                        {
                            Some((_, copied)) => *copied,
                            None => {
                                let copied =
                                    self.program.tables.types.operand_lifetime_owners.insert(
                                        OperandLifetimeOwner {
                                            declaration: copied,
                                        },
                                    );
                                self.lifetime_owners.push((owner, copied));
                                copied
                            }
                        };
                        reference.retain_supplied_operand_owner(owner);
                    }
                }
                match &mut reference {
                    TypeReference::Reference(borrow) => {
                        borrow.referee = self.type_reference(borrow.referee)
                    }
                    TypeReference::Constrained(constrained) => {
                        constrained.base_type = self.type_reference(constrained.base_type);
                        let constraints = self
                            .program
                            .tables
                            .types
                            .constraints
                            .span_or_empty(constrained.constraints)
                            .to_vec();
                        let copied = constraints
                            .into_iter()
                            .map(|constraint| self.constraint(constraint))
                            .collect::<Vec<_>>();
                        constrained.constraints =
                            self.program.tables.types.constraints.insert_many(copied);
                    }
                    TypeReference::FixedArray(array) => {
                        array.element_type = self.type_reference(array.element_type);
                        match &mut array.length {
                            FixedArrayLength::ConstParameter { symbol, .. } => {
                                *symbol = self.symbol(*symbol)
                            }
                            FixedArrayLength::ConstCall { machine, .. } => {
                                *machine = self.symbol(*machine)
                            }
                            FixedArrayLength::Expression(value) => {
                                *value = self.type_reference(*value)
                            }
                            FixedArrayLength::Literal(_) => {}
                        }
                    }
                    TypeReference::Slice(slice) => {
                        slice.element_type = self.type_reference(slice.element_type)
                    }
                    TypeReference::Generic(application) => {
                        application.base_symbol = self.symbol(application.base_symbol);
                        application.arguments = self.type_span(application.arguments);
                    }
                    TypeReference::ConstExpression(expression) => {
                        expression.expression = self.expression(expression.expression);
                        expression.lexical_owner = self.symbol(expression.lexical_owner);
                    }
                    TypeReference::Callable { contract, .. } => {
                        *contract = self.callable_contract(*contract);
                    }
                    TypeReference::DynamicTrait {
                        symbol, arguments, ..
                    } => {
                        *symbol = self.symbol(*symbol);
                        *arguments = self.type_span(*arguments);
                    }
                    TypeReference::Named { symbol, .. } => *symbol = self.symbol(*symbol),
                    TypeReference::AliasUse {
                        expansion,
                        application,
                        ..
                    } => {
                        **expansion = self.type_payload((**expansion).clone());
                        **application = self.type_payload((**application).clone());
                    }
                    TypeReference::Unit => {}
                }
                reference
            }

            fn callable_contract(
                &mut self,
                source: crate::callable_values::RuntimeCallableContractHandle,
            ) -> crate::callable_values::RuntimeCallableContractHandle {
                if let Some((_, copied)) = self
                    .callables
                    .iter()
                    .find(|(original, _)| *original == source)
                {
                    return *copied;
                }
                let mut declaration = self
                    .program
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .get(source)
                    .clone();
                let copied = self
                    .program
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .append(Default::default());
                self.callables.push((source, copied));
                declaration.lexical_owner = self.symbol(declaration.lexical_owner);
                declaration.signature = self.signature(&declaration.signature);
                *self
                    .program
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .get_mut(copied) = declaration;
                copied
            }

            fn proof_span(&mut self, source: HandleSpan<ProofFact>) -> HandleSpan<ProofFact> {
                let facts = source
                    .handles()
                    .map(|handle| self.proof_fact_payload(handle))
                    .collect::<Vec<_>>();
                let (facts, spans): (Vec<_>, Vec<_>) = facts.into_iter().unzip();
                let copied = self
                    .program
                    .tables
                    .declarations
                    .proof_facts
                    .insert_many(facts);
                for (handle, source_span) in copied.handles().zip(spans) {
                    if let Some(source_span) = source_span {
                        self.program.set_proof_fact_source_span(handle, source_span);
                    }
                }
                copied
            }

            fn constraint(&mut self, mut constraint: TypeConstraint) -> TypeConstraint {
                match &mut constraint {
                    TypeConstraint::Domain(domain) => {
                        domain.arguments = self.type_span(domain.arguments)
                    }
                    TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                }
                constraint
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::expression::{StaticSymbolApplication, TableCallExpression, TableNamePath};
            use crate::name::DiagnosticName;
            use crate::types::{GenericTypeReference, GenericTypeReferenceStorage};
            use source::{SourceId, SourceSpan, Span};

            fn generic(base: SymbolHandle, arguments: HandleSpan<TypeReference>) -> TypeReference {
                TypeReference::Generic(GenericTypeReference {
                    storage: GenericTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        base_symbol: base,
                        base_name: DiagnosticName::generated("Carrier"),
                        lifetime_arguments: Vec::new(),
                        arguments,
                    },
                })
            }

            #[test]
            fn proof_membership_copies_domain_arguments_and_preserves_provenance() {
                let mut program = SymbolResolvedTrees::default();
                let original = SymbolHandle::from_arena_index(3);
                let replacement = SymbolHandle::from_arena_index(4);
                let domain = program.declarations_mut().domain_path_members.insert_many([
                    DiagnosticName::generated("contracts"),
                    DiagnosticName::generated("Resident"),
                ]);
                let arguments = program
                    .declarations_mut()
                    .child_type_references
                    .insert_many([TypeReference::Named {
                        supplied_operand_owner: Default::default(),
                        symbol: original,
                        name: DiagnosticName::generated("T"),
                    }]);
                let fact = program
                    .declarations_mut()
                    .proof_facts
                    .insert(ProofFact::Membership(crate::domain::ProofMembershipFact {
                        domain,
                        domain_symbol: original,
                        domain_arguments: arguments,
                        ..Default::default()
                    }));
                let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
                program.set_proof_fact_source_span(fact, source_span);
                let copied = program.copy_proof_fact_with_symbols(fact, &[(original, replacement)]);
                assert_eq!(program.proof_fact_source_span(copied), Some(source_span));
                let ProofFact::Membership(membership) =
                    program.declarations().proof_facts.get(copied)
                else {
                    panic!("copy remains membership");
                };
                assert_eq!(membership.domain_symbol, replacement);
                assert_ne!(membership.domain, domain);
                assert_eq!(
                    program.domain_path_members(membership.domain),
                    program.domain_path_members(domain)
                );
                assert_ne!(membership.domain_arguments, arguments);
                assert_eq!(membership.authored_domain_selection, None);
                let TypeReference::Named { symbol, .. } = program
                    .declarations()
                    .child_type_references
                    .get(membership.domain_arguments.handle_at(0))
                else {
                    panic!("domain argument remains named");
                };
                assert_eq!(*symbol, replacement);
                let ProofFact::Membership(original_fact) =
                    program.declarations().proof_facts.get(fact)
                else {
                    panic!("original remains membership");
                };
                assert_eq!(original_fact.domain_symbol, original);
                assert_eq!(original_fact.domain, domain);
                assert_eq!(original_fact.domain_arguments, arguments);
            }

            #[test]
            fn copies_span_origins_and_expression_payloads_without_mutating_originals() {
                let mut program = SymbolResolvedTrees::default();
                let original = SymbolHandle::from_arena_index(3);
                let replacement = SymbolHandle::from_arena_index(4);
                let member_symbols = program
                    .expression_table_mut()
                    .reserve_name_path_member_symbols(1);
                program
                    .expression_table_mut()
                    .set_name_path_member_symbol_at_offset(member_symbols, 0, original);
                let name =
                    program
                        .expression_table_mut()
                        .insert(ExpressionNode::Name(TableNamePath {
                            members: HandleSpan::empty(),
                            member_symbols,
                            head_symbol: original,
                            symbol: original,
                            static_application: arena::Handle::invalid(),
                            reserved_result_of: SymbolHandle::invalid(),
                        }));
                let arguments = program
                    .declarations_mut()
                    .child_type_references
                    .insert_many([TypeReference::ConstExpression(
                        crate::types::ConstExpressionTypeReference::new(name),
                    )]);
                let application = program
                    .declarations_mut()
                    .child_type_references
                    .insert(generic(original, HandleSpan::empty()));
                program
                    .tables
                    .types
                    .generic_application_origins
                    .insert(GenericApplicationOrigin {
                        instance: arguments.handle_at(0),
                        application,
                    });
                let carrier = program
                    .declarations_mut()
                    .child_type_references
                    .insert(generic(original, arguments));
                let source = program
                    .expression_table_mut()
                    .insert(ExpressionNode::ZeroValue(carrier));
                let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
                program
                    .expression_table_mut()
                    .set_source_span(source, source_span);
                let copied =
                    program.copy_expression_with_symbols(source, &[(original, replacement)]);
                assert_eq!(
                    program.tables.bodies.expressions.source_span(copied),
                    source_span
                );
                let ExpressionNode::ZeroValue(copied_carrier) =
                    *program.tables.bodies.expressions.expression(copied)
                else {
                    panic!("copy must retain zero construction");
                };
                assert_ne!(copied_carrier, carrier);
                let TypeReference::Generic(copied_type) = program
                    .declarations()
                    .child_type_references
                    .get(copied_carrier)
                else {
                    panic!("copy must retain generic application");
                };
                assert_eq!(copied_type.base_symbol, replacement);
                let copied_argument = copied_type.arguments.handle_at(0);
                let TypeReference::ConstExpression(copied_name) = program
                    .declarations()
                    .child_type_references
                    .get(copied_argument)
                else {
                    panic!("copy must retain static expression");
                };
                let ExpressionNode::Name(copied_path) = program
                    .expression_table()
                    .expression(copied_name.expression)
                else {
                    panic!("copy must retain name");
                };
                assert_eq!(copied_path.symbol, replacement);
                assert_eq!(
                    program
                        .expression_table()
                        .name_path_member_symbols(copied_path.member_symbols),
                    [replacement]
                );
                let origin = program
                    .tables
                    .types
                    .generic_application_origins
                    .iter()
                    .find_map(|(_, origin)| (origin.instance == copied_argument).then_some(origin))
                    .expect("copied span origin");
                let TypeReference::Generic(origin_application) = program
                    .declarations()
                    .child_type_references
                    .get(origin.application)
                else {
                    panic!("copy must retain origin application");
                };
                assert_eq!(origin_application.base_symbol, replacement);
                let ExpressionNode::Name(original_path) =
                    program.tables.bodies.expressions.expression(name)
                else {
                    panic!("original remains a name");
                };
                assert_eq!(original_path.symbol, original);
                let TypeReference::Generic(original_type) =
                    program.declarations().child_type_references.get(carrier)
                else {
                    panic!("original remains generic");
                };
                assert_eq!(original_type.base_symbol, original);
                assert_eq!(original_type.arguments, arguments);
            }

            #[test]
            fn rebases_nested_static_arguments_simultaneously_and_preserves_absence() {
                let mut program = SymbolResolvedTrees::default();
                let original = SymbolHandle::from_arena_index(3);
                let replacement = SymbolHandle::from_arena_index(4);
                let argument = StaticMachineArgument {
                    authored_selection: None,
                    authored_selection_target: None,
                    type_reference: Handle::invalid(),
                    path: Box::new([]),
                    application: None,
                    const_literal: None,
                    evidence_projection: None,
                    symbol: original,
                };
                let source = program.expression_table_mut().insert(ExpressionNode::Call(
                    TableCallExpression {
                        callee: ExpressionHandle::invalid(),
                        authored_call_binding: None,
                        receiver: ExpressionHandle::invalid(),
                        target_symbol: original,
                        target: DiagnosticName::generated("run"),
                        requirement_application: arena::Handle::invalid(),
                        machine_arguments: vec![StaticMachineArgument {
                            application: Some(Box::new(StaticSymbolApplication {
                                member: arena::Handle::invalid(),
                                lifetime_arguments: Box::new([]),
                                arguments: vec![argument.clone()].into_boxed_slice(),
                            })),
                            ..argument
                        }]
                        .into_boxed_slice(),
                        arguments: HandleSpan::empty(),
                        evidence_arguments: Box::new([]),
                        operational_acknowledgement: Default::default(),
                    },
                ));
                let copied = program.copy_expression_with_symbols(
                    source,
                    &[
                        (original, replacement),
                        (replacement, SymbolHandle::from_arena_index(5)),
                        (SymbolHandle::invalid(), original),
                    ],
                );
                let ExpressionNode::Call(call) =
                    program.tables.bodies.expressions.expression(copied)
                else {
                    panic!("copy remains a call");
                };
                assert_eq!(call.target_symbol, replacement);
                assert!(!call.callee.is_valid());
                assert!(!call.receiver.is_valid());
                assert!(!call.machine_arguments[0].type_reference.is_valid());
                assert_eq!(call.machine_arguments[0].symbol, replacement);
                assert_eq!(
                    call.machine_arguments[0]
                        .application
                        .as_ref()
                        .expect("nested application")
                        .arguments[0]
                        .symbol,
                    replacement
                );
                assert!(
                    !program
                        .copy_expression_with_symbols(ExpressionHandle::invalid(), &[])
                        .is_valid()
                );
            }
        }
    }
    pub(crate) mod table_copies {

        use crate::name::DiagnosticName;
        use crate::values::expression::{
            ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, TableAtomicExpression,
            TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
            TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMemberExpression,
            TableMembershipExpression, TableNamePath, TableRangeExpression, TableStructLiteral,
            TableStructLiteralField, TableUnaryExpression,
        };
        use arena::{Handle, HandleSpan};

        #[derive(Default)]
        struct CopyGraph {
            expressions: Vec<(ExpressionHandle, ExpressionHandle)>,
            subjects: Vec<(
                Handle<super::MatchSubjectCapture>,
                Handle<super::MatchSubjectCapture>,
            )>,
        }
        use symbols::SymbolHandle;

        #[derive(Clone, Copy)]
        pub(crate) enum ExpressionSource<'source> {
            Own,
            Table(&'source ExpressionTable),
        }

        impl<'source> ExpressionSource<'source> {
            pub(crate) fn table<'table>(
                self,
                destination: &'table ExpressionTable,
            ) -> &'table ExpressionTable
            where
                'source: 'table,
            {
                match self {
                    Self::Own => destination,
                    Self::Table(source) => source,
                }
            }

            pub(crate) fn copy_into(
                self,
                destination: &mut ExpressionTable,
                expression: ExpressionHandle,
            ) -> ExpressionHandle {
                destination.copy_subtree(self, expression)
            }
        }

        impl ExpressionTable {
            pub fn copy_from(
                &mut self,
                source: &ExpressionTable,
                expression: ExpressionHandle,
            ) -> ExpressionHandle {
                self.copy_subtree(ExpressionSource::Table(source), expression)
            }

            pub fn copy_from_self(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
                self.copy_subtree(ExpressionSource::Own, expression)
            }

            fn copy_subtree(
                &mut self,
                source: ExpressionSource<'_>,
                expression: ExpressionHandle,
            ) -> ExpressionHandle {
                self.copy_graph(source, expression, &mut CopyGraph::default())
            }

            fn copy_graph(
                &mut self,
                source: ExpressionSource<'_>,
                expression: ExpressionHandle,
                copies: &mut CopyGraph,
            ) -> ExpressionHandle {
                if !expression.is_valid() {
                    return expression;
                }
                let position = expression.arena_index() as usize;
                if let Some((original, copied)) = copies.expressions.get(position)
                    && *original == expression
                {
                    return *copied;
                }
                let origin = source.table(self);
                let source_span = origin.source_span(expression);
                let materialization_source = origin.materialization_source(expression);
                let occurrences = origin
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>();
                let authored_exposure = origin.authored_expression_exposure(expression);
                let compiler_partition = origin.compiler_selection_partition(expression);
                let copied = match origin.expression(expression).clone() {
                    ExpressionNode::Match(dispatch) => {
                        let subject = self.copy_graph(source, dispatch.subject, copies);
                        let capture = self.copy_match_subject(source, dispatch.capture, copies);
                        let source_arms = source.table(self).match_arms(dispatch.arms).to_vec();
                        let mut arms = Vec::with_capacity(source_arms.len());
                        for arm in source_arms {
                            let pattern = match arm.pattern {
                                MatchPattern::Value(value) => {
                                    MatchPattern::Value(self.copy_graph(source, value, copies))
                                }
                                MatchPattern::Wildcard => MatchPattern::Wildcard,
                                MatchPattern::Destructure(pattern) => MatchPattern::Destructure(
                                    self.copy_destructure_pattern(source, pattern, copies),
                                ),
                            };
                            let value = self.copy_graph(source, arm.value, copies);
                            arms.push(TableMatchArm {
                                pattern,
                                value,
                                source_span: arm.source_span,
                            });
                        }
                        let arms = self.insert_match_arms(arms);
                        self.insert(ExpressionNode::Match(TableMatchExpression {
                            subject,
                            capture,
                            arms,
                        }))
                    }
                    ExpressionNode::SavedSubject(saved) => {
                        let copied_capture =
                            copies.subjects.iter().find_map(|(original, copied)| {
                                (*original == saved.capture).then_some(*copied)
                            });
                        if copied_capture.is_none() && matches!(source, ExpressionSource::Own) {
                            self.insert(ExpressionNode::SavedSubject(saved))
                        } else {
                            let capture = self.copy_match_subject(source, saved.capture, copies);
                            let pattern_head = self.copy_graph(source, saved.pattern_head, copies);
                            self.insert(ExpressionNode::SavedSubject(super::TableSavedSubject {
                                component: saved.component,
                                capture,
                                pattern_head,
                            }))
                        }
                    }
                    ExpressionNode::ArrayLiteral(values) => {
                        let values = self.copy_expression_span(source, values, copies);
                        self.insert(ExpressionNode::ArrayLiteral(values))
                    }
                    ExpressionNode::Atomic(atomic) => {
                        let value = self.copy_graph(source, atomic.value, copies);
                        let result = self.copy_optional_subtree(source, atomic.result, copies);
                        self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                            value,
                            result,
                            ordering: atomic.ordering,
                            result_custody: atomic.result_custody,
                        }))
                    }
                    ExpressionNode::Binary(binary) => {
                        let left = self.copy_graph(source, binary.left, copies);
                        let right = self.copy_graph(source, binary.right, copies);
                        self.insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: binary.policy,
                            left,
                            operator: binary.operator,
                            right,
                        }))
                    }
                    ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
                    ExpressionNode::Cast(cast) => {
                        let value = self.copy_graph(source, cast.value, copies);
                        let target_label = self.copy_name_path_members(source, cast.target_label);
                        let semantic_domain =
                            self.copy_name_path_members(source, cast.semantic_domain);
                        self.insert(ExpressionNode::Cast(TableCastExpression {
                            value,
                            target_type: cast.target_type,
                            target_label,
                            domain: cast.domain,
                            semantic_domain,
                            semantic_domain_arguments: cast.semantic_domain_arguments,
                            semantic_domain_symbol: cast.semantic_domain_symbol,
                            form: cast.form,
                        }))
                    }
                    ExpressionNode::Call(call) => {
                        let callee = self.copy_optional_subtree(source, call.callee, copies);
                        let receiver = self.copy_optional_subtree(source, call.receiver, copies);
                        let arguments = self.copy_expression_span(source, call.arguments, copies);
                        let machine_arguments = call
                            .machine_arguments
                            .iter()
                            .map(|argument| match source {
                                ExpressionSource::Own => {
                                    self.copy_static_argument_from_self(argument)
                                }
                                ExpressionSource::Table(table) => {
                                    self.copy_static_argument_from(table, argument)
                                }
                            })
                            .collect();
                        self.insert(ExpressionNode::Call(TableCallExpression {
                            callee,
                            authored_call_binding: call.authored_call_binding,
                            receiver,
                            target_symbol: call.target_symbol,
                            target: call.target,
                            requirement_application: call.requirement_application,
                            machine_arguments,
                            arguments,
                            evidence_arguments: call.evidence_arguments,
                            operational_acknowledgement: call.operational_acknowledgement,
                        }))
                    }
                    ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value)),
                    ExpressionNode::UndeterminedNaN(format) => {
                        self.insert(ExpressionNode::UndeterminedNaN(format))
                    }
                    ExpressionNode::Indexed(indexed) => {
                        let collection = self.copy_graph(source, indexed.collection, copies);
                        let index = self.copy_graph(source, indexed.index, copies);
                        self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                            collection,
                            index,
                        }))
                    }
                    ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
                    ExpressionNode::Membership(membership) => {
                        let value = self.copy_graph(source, membership.value, copies);
                        let domain = self.copy_name_path_members(source, membership.domain);
                        self.insert(ExpressionNode::Membership(TableMembershipExpression {
                            is_pattern: membership.is_pattern,
                            value,
                            domain,
                            domain_symbol: membership.domain_symbol,
                            case_type_symbol: membership.case_type_symbol,
                            case_symbol: membership.case_symbol,
                        }))
                    }
                    ExpressionNode::Member(member) => {
                        let receiver = self.copy_graph(source, member.receiver, copies);
                        self.insert(ExpressionNode::Member(TableMemberExpression {
                            receiver,
                            member_symbol: member.member_symbol,
                            member: member.member,
                            case_variant: member.case_variant,
                        }))
                    }
                    ExpressionNode::Borrow(borrow) => {
                        let target = self.copy_graph(source, borrow.target, copies);
                        self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                            target,
                            access: borrow.access,
                        }))
                    }
                    ExpressionNode::Name(path) => {
                        let static_application = if path.static_application.is_valid() {
                            let argument = source.table(self).static_member(path.static_application)
                                .expect("declaration application belongs to its source expression table").clone();
                            let copied = match source {
                                ExpressionSource::Own => {
                                    self.copy_static_argument_from_self(&argument)
                                }
                                ExpressionSource::Table(table) => {
                                    self.copy_static_argument_from(table, &argument)
                                }
                            };
                            self.insert_static_member(copied)
                        } else {
                            arena::Handle::invalid()
                        };
                        let members = self.copy_name_path_members(source, path.members);
                        let member_symbols =
                            self.copy_name_path_member_symbols(source, path.member_symbols);
                        self.insert(ExpressionNode::Name(TableNamePath {
                            members,
                            member_symbols,
                            head_symbol: path.head_symbol,
                            symbol: path.symbol,
                            reserved_result_of: path.reserved_result_of,
                            static_application,
                        }))
                    }
                    ExpressionNode::Range(range) => {
                        let start = self.copy_optional_subtree(source, range.start, copies);
                        let end = self.copy_optional_subtree(source, range.end, copies);
                        self.insert(ExpressionNode::Range(TableRangeExpression {
                            start,
                            end,
                            end_inclusive: range.end_inclusive,
                        }))
                    }
                    ExpressionNode::StructLiteral(literal) => {
                        let fields =
                            self.copy_struct_literal_fields(source, literal.fields, copies);
                        self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                            application_type: literal.application_type,
                            type_name: literal.type_name,
                            type_symbol: literal.type_symbol,
                            case_name: literal.case_name,
                            case_symbol: literal.case_symbol,
                            fields,
                        }))
                    }
                    ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
                    ExpressionNode::Unary(unary) => {
                        let operand = self.copy_graph(source, unary.operand, copies);
                        self.insert(ExpressionNode::Unary(TableUnaryExpression {
                            operator: unary.operator,
                            operand,
                        }))
                    }
                    ExpressionNode::TypeExpression(type_reference) => {
                        self.insert(ExpressionNode::TypeExpression(type_reference))
                    }
                    ExpressionNode::ZeroValue(type_reference) => {
                        self.insert(ExpressionNode::ZeroValue(type_reference))
                    }
                };
                self.set_source_span(copied, source_span);
                if let Some(exposure) = authored_exposure {
                    self.set_authored_expression_exposure(copied, exposure);
                }
                if let Some(partition) = compiler_partition {
                    self.set_compiler_selection_partition(copied, partition);
                }
                self.attach_authored_selection_occurrences(copied, occurrences);
                if copies.expressions.len() <= position {
                    copies
                        .expressions
                        .resize(position + 1, (Handle::invalid(), Handle::invalid()));
                }
                copies.expressions[position] = (expression, copied);
                if materialization_source.is_valid() {
                    let authored = self.copy_graph(source, materialization_source, copies);
                    self.retain_materialization_source(copied, authored);
                }
                copied
            }

            fn copy_optional_subtree(
                &mut self,
                source: ExpressionSource<'_>,
                expression: ExpressionHandle,
                copies: &mut CopyGraph,
            ) -> ExpressionHandle {
                if expression.is_valid() {
                    self.copy_graph(source, expression, copies)
                } else {
                    ExpressionHandle::invalid()
                }
            }

            fn copy_expression_span(
                &mut self,
                source: ExpressionSource<'_>,
                expressions: HandleSpan<ExpressionHandle>,
                copies: &mut CopyGraph,
            ) -> HandleSpan<ExpressionHandle> {
                let expressions = source.table(self).expression_handles(expressions).to_vec();
                self.copy_expression_list(source, &expressions, copies)
            }

            fn copy_expression_list(
                &mut self,
                source: ExpressionSource<'_>,
                expressions: &[ExpressionHandle],
                copies: &mut CopyGraph,
            ) -> HandleSpan<ExpressionHandle> {
                let span = self.reserve_expression_handles(
                    expressions
                        .len()
                        .try_into()
                        .expect("expression handle span count overflow"),
                );

                for (offset, expression) in expressions.iter().enumerate() {
                    let expression = self.copy_graph(source, *expression, copies);
                    self.set_expression_handle_at_offset(
                        span,
                        offset
                            .try_into()
                            .expect("expression handle span count overflow"),
                        expression,
                    );
                }

                span
            }

            /// A record pattern's field roster: it names no expression, so
            /// the rows copy as they stand.
            pub(crate) fn copy_pattern_fields(
                &mut self,
                source: ExpressionSource<'_>,
                fields: HandleSpan<super::TableMatchField>,
            ) -> HandleSpan<super::TableMatchField> {
                let fields = source.table(self).match_fields(fields).to_vec();
                self.insert_match_fields(fields)
            }

            pub(crate) fn copy_transition_pattern(
                &mut self,
                source: ExpressionSource<'_>,
                retained: crate::statement::TransitionPattern,
            ) -> crate::statement::TransitionPattern {
                let copies = &mut CopyGraph::default();
                let subject = self.copy_graph(source, retained.subject, copies);
                let capture = self.copy_match_subject(source, retained.capture, copies);
                let pattern = self.copy_destructure_pattern(source, retained.pattern, copies);
                crate::statement::TransitionPattern {
                    is_structural: retained.is_structural,
                    subject,
                    capture,
                    pattern,
                }
            }

            fn copy_match_subject(
                &mut self,
                source: ExpressionSource<'_>,
                capture: Handle<super::MatchSubjectCapture>,
                copies: &mut CopyGraph,
            ) -> Handle<super::MatchSubjectCapture> {
                if !capture.is_valid() {
                    return capture;
                }
                let position = capture.arena_index() as usize;
                if let Some((original, copied)) = copies.subjects.get(position)
                    && *original == capture
                {
                    return *copied;
                }
                let subject = source.table(self).match_subject(capture).subject;
                let subject = self.copy_graph(source, subject, copies);
                let copied = match source.table(self).match_subject(capture).purpose {
                    super::SubjectCapturePurpose::Match => self.insert_match_subject(subject),
                    super::SubjectCapturePurpose::Transition => {
                        self.insert_transition_subject(subject)
                    }
                };
                if copies.subjects.len() <= position {
                    copies
                        .subjects
                        .resize(position + 1, (Handle::invalid(), Handle::invalid()));
                }
                copies.subjects[position] = (capture, copied);
                copied
            }

            fn copy_destructure_pattern(
                &mut self,
                source: ExpressionSource<'_>,
                pattern: super::TableDestructurePattern,
                copies: &mut CopyGraph,
            ) -> super::TableDestructurePattern {
                let head = self.copy_graph(source, pattern.head, copies);
                let mut fields = source.table(self).match_fields(pattern.fields).to_vec();
                for field in &mut fields {
                    if let super::MatchFieldAction::Equal(value) = &mut field.action {
                        *value = self.copy_graph(source, *value, copies);
                    }
                }
                let fields = self.insert_match_fields(fields);
                let guard = self.copy_graph(source, pattern.guard, copies);
                let condition = self.copy_graph(source, pattern.condition, copies);
                let selectors = source
                    .table(self)
                    .match_proof_selectors(pattern.proof_selectors)
                    .to_vec();
                let proof_selectors = self.insert_match_proof_selectors(selectors);
                super::TableDestructurePattern {
                    application_type: pattern.application_type,
                    head,
                    fields,
                    has_rest: pattern.has_rest,
                    guard,
                    condition,
                    proof_selectors,
                }
            }

            fn copy_name_path_members(
                &mut self,
                source: ExpressionSource<'_>,
                members: HandleSpan<DiagnosticName>,
            ) -> HandleSpan<DiagnosticName> {
                let members = source.table(self).name_path_members(members).to_vec();
                self.spans.name_path_members.insert_many(members)
            }

            fn copy_name_path_member_symbols(
                &mut self,
                source: ExpressionSource<'_>,
                member_symbols: HandleSpan<SymbolHandle>,
            ) -> HandleSpan<SymbolHandle> {
                let member_symbols = source
                    .table(self)
                    .name_path_member_symbols(member_symbols)
                    .to_vec();
                self.spans
                    .name_path_member_symbols
                    .insert_many(member_symbols)
            }

            fn copy_struct_literal_fields(
                &mut self,
                source: ExpressionSource<'_>,
                fields: HandleSpan<TableStructLiteralField>,
                copies: &mut CopyGraph,
            ) -> HandleSpan<TableStructLiteralField> {
                let fields = source.table(self).struct_fields(fields).to_vec();
                let span = self.reserve_struct_fields(
                    fields
                        .len()
                        .try_into()
                        .expect("struct literal field span count overflow"),
                );

                for (offset, field) in fields.into_iter().enumerate() {
                    let value = self.copy_graph(source, field.value, copies);
                    self.set_struct_field_at_offset(
                        span,
                        offset
                            .try_into()
                            .expect("struct literal field span count overflow"),
                        TableStructLiteralField {
                            name: field.name,
                            field_symbol: field.field_symbol,
                            value,
                        },
                    );
                }

                span
            }
        }
    }
    pub(crate) use table_copies::ExpressionSource;
    #[cfg(test)]
    pub(crate) mod tests {
        use super::{
            BinaryOperator, ExpressionNode, ExpressionTable, TableBinaryExpression, TableNamePath,
            TableStructLiteral, TableStructLiteralField,
        };
        use crate::name::DiagnosticName;
        use crate::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
            AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelections,
        };
        use arena::HandleSpan;
        use language_semantics::declaration_selection::CompilerDerivedSelectionPartition;
        use source::{SourceId, SourceSpan, Span};
        use symbols::SymbolHandle;

        fn authored_selection_occurrences() -> [AuthoredDeclarationSelectionOccurrenceId; 2] {
            let mut selections = AuthoredDeclarationSelections::default();
            let first = selections
                .record_resolved(
                    SourceSpan::default(),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::MemberAccess,
                    SymbolHandle::from_arena_index(31),
                )
                .expect("valid selected symbol");
            let second = selections
                .record_resolved(
                    SourceSpan::default(),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::Call,
                    SymbolHandle::from_arena_index(32),
                )
                .expect("valid selected symbol");
            [first, second]
        }

        #[test]
        fn expression_occurrences_support_multiple_ids_and_survive_resolved_copies() {
            let occurrences = authored_selection_occurrences();
            assert_eq!(occurrences[0].ordinal(), 0);

            let mut source = ExpressionTable::new();
            let expression = source.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(7),
            ));
            source.set_authored_expression_exposure(
                expression,
                AuthoredDeclarationSelectionExposure::PublicInterface,
            );
            let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
            source.set_source_span(expression, source_span);
            let partition = CompilerDerivedSelectionPartition::from_compiler_ordinal(9);
            source.set_compiler_selection_partition(expression, partition);
            source.attach_authored_selection_occurrences(expression, occurrences);

            assert_eq!(
                source
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>(),
                occurrences
            );

            let mut copied = ExpressionTable::new();
            let copied_expression = copied.copy_from(&source, expression);
            assert_eq!(
                copied
                    .authored_selection_occurrences(copied_expression)
                    .collect::<Vec<_>>(),
                occurrences
            );
            assert_eq!(
                copied.authored_expression_exposure(copied_expression),
                Some(AuthoredDeclarationSelectionExposure::PublicInterface)
            );
            assert_eq!(copied.source_span(copied_expression), source_span);
            assert_eq!(
                copied.compiler_selection_partition(copied_expression),
                Some(partition)
            );

            let self_copied_expression = source.copy_from_self(expression);
            assert_eq!(
                source
                    .authored_selection_occurrences(self_copied_expression)
                    .collect::<Vec<_>>(),
                occurrences
            );
            assert_eq!(
                source.authored_expression_exposure(self_copied_expression),
                Some(AuthoredDeclarationSelectionExposure::PublicInterface)
            );
            assert_eq!(source.source_span(self_copied_expression), source_span);
            assert_eq!(
                source.compiler_selection_partition(self_copied_expression),
                Some(partition)
            );
        }

        #[test]
        fn expression_table_copies_table_payloads_without_tree_roundtrip() {
            let room_symbol = SymbolHandle::from_arena_index(3);
            let field_symbol = SymbolHandle::from_arena_index(4);

            let mut source = ExpressionTable::new();
            let room_members = source.reserve_name_path_members(1);
            source.set_name_path_member_at_offset(
                room_members,
                0,
                DiagnosticName::generated("room"),
            );
            let room_member_symbols = source.reserve_name_path_member_symbols(1);
            source.set_name_path_member_symbol_at_offset(room_member_symbols, 0, room_symbol);
            let room = source.insert(ExpressionNode::Name(TableNamePath {
                members: room_members,
                member_symbols: room_member_symbols,
                head_symbol: room_symbol,
                symbol: room_symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }));

            let field_members = source.reserve_name_path_members(2);
            source.set_name_path_member_at_offset(
                field_members,
                0,
                DiagnosticName::generated("room"),
            );
            source.set_name_path_member_at_offset(
                field_members,
                1,
                DiagnosticName::generated("field"),
            );
            let field_member_symbols = source.reserve_name_path_member_symbols(2);
            source.set_name_path_member_symbol_at_offset(field_member_symbols, 0, room_symbol);
            source.set_name_path_member_symbol_at_offset(field_member_symbols, 1, field_symbol);
            let field = source.insert(ExpressionNode::Name(TableNamePath {
                members: field_members,
                member_symbols: field_member_symbols,
                head_symbol: room_symbol,
                symbol: field_symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }));

            let hall = source.insert(ExpressionNode::String((&b"Hall"[..]).into()));
            let open = source.insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: room,
                operator: BinaryOperator::Equal,
                right: field,
            }));
            let fields = source.insert_struct_fields([
                TableStructLiteralField {
                    name: DiagnosticName::generated("name"),
                    field_symbol: SymbolHandle::from_arena_index(6),
                    value: hall,
                },
                TableStructLiteralField {
                    name: DiagnosticName::generated("open"),
                    field_symbol: SymbolHandle::from_arena_index(7),
                    value: open,
                },
            ]);
            let root = source.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: Default::default(),
                type_name: DiagnosticName::generated("Room"),
                type_symbol: SymbolHandle::from_arena_index(5),
                case_name: None,
                case_symbol: None,
                fields,
            }));

            let mut copied = ExpressionTable::new();
            let copied_root = copied.copy_from(&source, root);

            assert_eq!(source.display_name(root), copied.display_name(copied_root));

            let ExpressionNode::StructLiteral(struct_literal) = copied.expression(copied_root)
            else {
                panic!("copied root should remain a struct literal");
            };
            assert_eq!(copied.struct_fields(struct_literal.fields).len(), 2);
            assert_eq!(
                struct_literal.type_symbol,
                SymbolHandle::from_arena_index(5)
            );
            assert_eq!(
                copied.struct_fields(struct_literal.fields)[0].field_symbol,
                SymbolHandle::from_arena_index(6)
            );

            let open_field = &copied.struct_fields(struct_literal.fields)[1];
            let ExpressionNode::Binary(binary) = copied.expression(open_field.value) else {
                panic!("copied field should keep its binary expression");
            };
            let ExpressionNode::Name(TableNamePath {
                members,
                head_symbol,
                symbol,
                ..
            }) = copied.expression(binary.right)
            else {
                panic!("copied binary rhs should keep its name path");
            };

            assert_eq!(*head_symbol, room_symbol);
            assert_eq!(*symbol, field_symbol);
            assert_eq!(copied.name_path_members(*members).len(), 2);
            assert_eq!(
                copied.name_path_member_symbols(match copied.expression(binary.right) {
                    ExpressionNode::Name(path) => path.member_symbols,
                    _ => unreachable!(),
                }),
                [room_symbol, field_symbol]
            );
        }
        #[test]
        fn match_arms_preserve_order_source_and_nonnumeric_values() {
            use super::{MatchPattern, TableMatchArm, TableMatchExpression};
            let mut table = ExpressionTable::new();
            let subject = table.insert(ExpressionNode::Boolean(true));
            let pattern = table.insert(ExpressionNode::Boolean(false));
            let first = table.insert(ExpressionNode::String("first".as_bytes().into()));
            let fallback = table.insert(ExpressionNode::String("fallback".as_bytes().into()));
            let source_span =
                source::SourceSpan::new(source::SourceId(7), source::Span::new(11, 29));
            let arms = table.insert_match_arms([
                TableMatchArm {
                    pattern: MatchPattern::Value(pattern),
                    value: first,
                    source_span,
                },
                TableMatchArm {
                    pattern: MatchPattern::Wildcard,
                    value: fallback,
                    source_span,
                },
            ]);
            let expression = table.insert(ExpressionNode::Match(TableMatchExpression {
                subject,
                arms,
                ..Default::default()
            }));
            table.set_source_span(expression, source_span);
            assert_eq!(
                table.match_arms(arms)[0].pattern,
                MatchPattern::Value(pattern)
            );
            assert_eq!(table.match_arms(arms)[1].pattern, MatchPattern::Wildcard);
            assert_eq!(table.match_arms(arms)[0].source_span, source_span);
            assert_eq!(
                table.display_name(expression),
                "match true { false => \"first\", _ => \"fallback\" }"
            );

            let mut copied = ExpressionTable::new();
            let copied_expression = copied.copy_from(&table, expression);
            assert_eq!(
                copied.display_name(copied_expression),
                table.display_name(expression)
            );
            assert_eq!(copied.source_span(copied_expression), source_span);
            let ExpressionNode::Match(dispatch) = copied.expression(copied_expression) else {
                panic!("expected match");
            };
            assert_eq!(copied.match_arms(dispatch.arms)[0].source_span, source_span);
            let own_copy = table.copy_from_self(expression);
            assert_eq!(
                table.display_name(own_copy),
                copied.display_name(copied_expression)
            );
            assert_eq!(table.source_span(own_copy), source_span);
        }

        struct RowFrontier {
            expressions: usize,
            occurrence_ids: usize,
            expression_handles: usize,
            name_path_members: usize,
            name_path_member_symbols: usize,
            struct_fields: usize,
            match_arms: usize,
        }

        impl RowFrontier {
            fn of(table: &ExpressionTable) -> Self {
                Self {
                    expressions: table.nodes.expressions.storage_slice().len(),
                    occurrence_ids: table
                        .spans
                        .authored_selection_occurrence_ids
                        .storage_slice()
                        .len(),
                    expression_handles: table.spans.expression_handles.storage_slice().len(),
                    name_path_members: table.spans.name_path_members.storage_slice().len(),
                    name_path_member_symbols: table
                        .spans
                        .name_path_member_symbols
                        .storage_slice()
                        .len(),
                    struct_fields: table.spans.struct_fields.storage_slice().len(),
                    match_arms: table.spans.match_arms.storage_slice().len(),
                }
            }
        }

        fn render_rows_past(table: &ExpressionTable, frontier: &RowFrontier) -> String {
            let mut lines = Vec::new();
            for (handle, node) in table.iter_expressions().skip(frontier.expressions) {
                let span = table.source_span(handle);
                let occurrences =
                    table.nodes.authored_selection_occurrences[super::source_span_index(handle)];
                lines.push(format!(
            "expression {handle:?} = {node:?} at {}:{}..{} exposure {:?} partition {:?} occurrences {occurrences:?}",
            span.source_id.0,
            span.span.start,
            span.span.end,
            table.authored_expression_exposure(handle),
            table.compiler_selection_partition(handle),
        ));
            }
            let occurrence_ids = table
                .spans
                .authored_selection_occurrence_ids
                .storage_slice()
                .iter()
                .map(|stored| stored.occurrence())
                .collect::<Vec<_>>();
            push_rows_past(
                &mut lines,
                "occurrence",
                &occurrence_ids,
                frontier.occurrence_ids,
            );
            push_rows_past(
                &mut lines,
                "handle",
                table.spans.expression_handles.storage_slice(),
                frontier.expression_handles,
            );
            push_rows_past(
                &mut lines,
                "member",
                table.spans.name_path_members.storage_slice(),
                frontier.name_path_members,
            );
            push_rows_past(
                &mut lines,
                "member symbol",
                table.spans.name_path_member_symbols.storage_slice(),
                frontier.name_path_member_symbols,
            );
            push_rows_past(
                &mut lines,
                "field",
                table.spans.struct_fields.storage_slice(),
                frontier.struct_fields,
            );
            push_rows_past(
                &mut lines,
                "arm",
                table.spans.match_arms.storage_slice(),
                frontier.match_arms,
            );
            shorten_handles(&lines.join("\n"))
        }

        fn push_rows_past<Row: std::fmt::Debug>(
            lines: &mut Vec<String>,
            label: &str,
            rows: &[Row],
            frontier: usize,
        ) {
            for (row_index, row) in rows.iter().enumerate().skip(frontier) {
                lines.push(format!("{label} #{} = {row:?}", row_index + 1));
            }
        }

        fn shorten_handles(rendered: &str) -> String {
            let handles = shorten_records(
                rendered,
                "Handle { arena_index: ",
                ", generation: ",
                |index, generation| {
                    if generation == "1" {
                        format!("#{index}")
                    } else {
                        format!("#{index}@{generation}")
                    }
                },
            );
            shorten_records(
                &handles,
                "HandleSpan { start: ",
                ", count: ",
                |start, count| format!("{start}+{count}"),
            )
        }

        fn shorten_records(
            rendered: &str,
            opening: &str,
            separator: &str,
            shorten: impl Fn(&str, &str) -> String,
        ) -> String {
            let mut shortened = String::with_capacity(rendered.len());
            let mut rest = rendered;
            while let Some(start) = rest.find(opening) {
                shortened.push_str(&rest[..start]);
                let fields = &rest[start + opening.len()..];
                let (first, after_first) = fields
                    .split_once(separator)
                    .expect("debug record keeps its field separator");
                let (second, after_record) = after_first
                    .split_once(" }")
                    .expect("debug record keeps its closing brace");
                shortened.push_str(&shorten(first, second));
                rest = after_record;
            }
            shortened.push_str(rest);
            shortened
        }

        fn insert_members(
            table: &mut ExpressionTable,
            members: &[&str],
        ) -> HandleSpan<DiagnosticName> {
            let span = table.reserve_name_path_members(members.len() as u32);
            for (offset, member) in members.iter().enumerate() {
                table.set_name_path_member_at_offset(
                    span,
                    offset as u32,
                    DiagnosticName::generated(*member),
                );
            }
            span
        }

        fn insert_name(
            table: &mut ExpressionTable,
            members: &[&str],
            member_symbols: &[SymbolHandle],
            symbol: SymbolHandle,
        ) -> super::ExpressionHandle {
            let members = insert_members(table, members);
            let symbol_span = table.reserve_name_path_member_symbols(member_symbols.len() as u32);
            for (offset, member_symbol) in member_symbols.iter().enumerate() {
                table.set_name_path_member_symbol_at_offset(
                    symbol_span,
                    offset as u32,
                    *member_symbol,
                );
            }
            table.insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols: symbol_span,
                head_symbol: member_symbols.first().copied().unwrap_or_default(),
                symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }))
        }

        fn insert_every_node_kind(table: &mut ExpressionTable) -> super::ExpressionHandle {
            use super::{
                MatchPattern, TableAtomicExpression, TableBorrowExpression, TableCallExpression,
                TableCastExpression, TableIndexedExpression, TableMatchArm, TableMatchExpression,
                TableMemberExpression, TableMembershipExpression, TableRangeExpression,
                TableUnaryExpression, UnaryOperator,
            };
            use language_core::atomic::{
                AtomicExpressionResultCustody, AtomicOrderingPlan, MemoryOrdering,
            };
            use numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral};

            let [root_occurrence, call_occurrence] = authored_selection_occurrences();
            let room_symbol = SymbolHandle::from_arena_index(3);
            let count_symbol = SymbolHandle::from_arena_index(4);
            let span = |start| SourceSpan::new(SourceId(7), Span::new(start, start + 5));

            let receiver = insert_name(
                table,
                &["self", "room"],
                &[room_symbol, room_symbol],
                room_symbol,
            );
            table.attach_authored_selection_occurrences(receiver, [root_occurrence]);
            table.set_authored_expression_exposure(
                receiver,
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
            );
            let one = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
            let half = table.insert(ExpressionNode::Float(FloatLiteral::new(2.5)));
            let array_values = table.insert_expression_handles([one, half]);
            let array = table.insert(ExpressionNode::ArrayLiteral(array_values));
            let flag = table.insert(ExpressionNode::Boolean(true));
            let not_flag = table.insert(ExpressionNode::Unary(TableUnaryExpression {
                operator: UnaryOperator::LogicalNot,
                operand: flag,
            }));
            let target_label = insert_members(table, &["u8"]);
            let semantic_domain = insert_members(table, &["Meters"]);
            let cast = table.insert(ExpressionNode::Cast(TableCastExpression {
                value: not_flag,
                target_type: arena::Handle::from_arena_index(5),
                target_label,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain,
                semantic_domain_arguments: arena::HandleSpan::empty(),
                semantic_domain_symbol: SymbolHandle::from_arena_index(6),
                form: language_core::cast_form::CastForm::Value,
            }));
            table.set_source_span(cast, span(40));
            table.set_compiler_selection_partition(
                cast,
                CompilerDerivedSelectionPartition::from_compiler_ordinal(9),
            );
            let arguments = table.insert_expression_handles([array, cast]);
            let call = table.insert(ExpressionNode::Call(TableCallExpression {
                // This nominal call carries its declaration in target_symbol.
                callee: super::ExpressionHandle::invalid(),
                authored_call_binding: None,
                receiver,
                target_symbol: SymbolHandle::from_arena_index(7),
                target: DiagnosticName::generated("measure"),
                requirement_application: arena::Handle::invalid(),
                machine_arguments: Box::default(),
                arguments,
                evidence_arguments: Box::from([DiagnosticName::generated("witness")]),
                operational_acknowledgement: Default::default(),
            }));
            table.set_source_span(call, span(30));
            table.attach_authored_selection_occurrences(call, [call_occurrence, root_occurrence]);
            table.set_authored_expression_exposure(
                call,
                AuthoredDeclarationSelectionExposure::PublicInterface,
            );

            let zero = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
            let range = table.insert(ExpressionNode::Range(TableRangeExpression {
                start: zero,
                end: super::ExpressionHandle::invalid(),
                end_inclusive: false,
            }));
            let hall = table.insert(ExpressionNode::String((&b"Hall"[..]).into()));
            let eleven = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(11)));
            let thirteen = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(13)));
            let proof = table.insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: eleven,
                operator: BinaryOperator::Add,
                right: thirteen,
            }));
            let grid = insert_name(table, &["grid"], &[room_symbol], room_symbol);
            let three = table.insert(ExpressionNode::Integer(IntegerLiteral::from_value(3)));
            let cell = table.insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection: grid,
                index: three,
            }));
            let fields = table.insert_struct_fields([
                TableStructLiteralField {
                    name: DiagnosticName::generated("name"),
                    field_symbol: SymbolHandle::from_arena_index(8),
                    value: hall,
                },
                TableStructLiteralField {
                    name: DiagnosticName::generated("proof"),
                    field_symbol: SymbolHandle::from_arena_index(9),
                    value: proof,
                },
                TableStructLiteralField {
                    name: DiagnosticName::generated("size"),
                    field_symbol: SymbolHandle::from_arena_index(10),
                    value: cell,
                },
            ]);
            let literal = table.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: Default::default(),
                type_name: DiagnosticName::generated("Room"),
                type_symbol: SymbolHandle::from_arena_index(11),
                case_name: Some(DiagnosticName::generated("Open")),
                case_symbol: Some(SymbolHandle::from_arena_index(12)),
                fields,
            }));
            table.set_source_span(literal, span(60));

            let counter = insert_name(table, &["cell"], &[], SymbolHandle::invalid());
            let count = table.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: counter,
                member_symbol: count_symbol,
                member: DiagnosticName::generated("count"),
                case_variant: Some(DiagnosticName::generated("Full")),
            }));
            let borrowed = table.insert(ExpressionNode::Borrow(TableBorrowExpression {
                target: count,
                access: language_core::ReferenceAccess::Mutable,
            }));
            let previous = insert_name(table, &["previous"], &[count_symbol], count_symbol);
            let atomic = table.insert(ExpressionNode::Atomic(TableAtomicExpression {
                value: borrowed,
                result: previous,
                ordering: AtomicOrderingPlan::Load(MemoryOrdering::GlobalOrder),
                result_custody: AtomicExpressionResultCustody::Scalar,
            }));
            let nan = table.insert(ExpressionNode::UndeterminedNaN(FloatFormat::F32));
            let domain = insert_members(table, &["Small"]);
            let membership = table.insert(ExpressionNode::Membership(TableMembershipExpression {
                is_pattern: false,
                value: nan,
                domain,
                domain_symbol: SymbolHandle::from_arena_index(14),
                case_type_symbol: SymbolHandle::invalid(),
                case_symbol: SymbolHandle::invalid(),
            }));
            let zero_value = table.insert(ExpressionNode::ZeroValue(
                arena::Handle::from_arena_index(13),
            ));

            let arms = table.insert_match_arms([
                TableMatchArm {
                    pattern: MatchPattern::Value(range),
                    value: literal,
                    source_span: span(70),
                },
                TableMatchArm {
                    pattern: MatchPattern::Value(atomic),
                    value: membership,
                    source_span: span(80),
                },
                TableMatchArm {
                    pattern: MatchPattern::Wildcard,
                    value: zero_value,
                    source_span: span(90),
                },
            ]);
            let root = table.insert(ExpressionNode::Match(TableMatchExpression {
                subject: call,
                arms,
                ..Default::default()
            }));
            table.set_source_span(root, span(20));
            table.attach_authored_selection_occurrences(root, [root_occurrence]);
            table.set_compiler_selection_partition(
                root,
                CompilerDerivedSelectionPartition::from_compiler_ordinal(4),
            );
            root
        }

        #[test]
        fn table_and_own_copies_append_identical_rows_in_child_first_order() {
            let mut source = ExpressionTable::new();
            let root = insert_every_node_kind(&mut source);
            let frontier = RowFrontier::of(&source);

            let mut across = source.clone();
            let across_root = across.copy_from(&source, root);
            let mut within = source.clone();
            let within_root = within.copy_from_self(root);

            assert_eq!(across_root, within_root);
            let rendered = render_rows_past(&across, &frontier);
            assert_eq!(rendered, render_rows_past(&within, &frontier));
            assert!(across == within, "both copies must leave identical tables");
            assert_eq!(rendered, EXPECTED_COPY_ROWS.trim());
        }

        const EXPECTED_COPY_ROWS: &str = r#"
expression #28 = Name(TableNamePath { members: #9+2, member_symbols: #5+2, head_symbol: #3, symbol: #3, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure Some(PrivateImplementation) partition None occurrences #5+1
expression #29 = Integer(IntegerLiteral { text: "1", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #30 = Float(FloatLiteral { text: "2.5", landing: None, presented: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #31 = ArrayLiteral(#7+2) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #32 = Boolean(true) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #33 = Unary(TableUnaryExpression { operator: LogicalNot, operand: #32 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #34 = Cast(TableCastExpression { value: #33, target_type: #5, target_label: #11+1, domain: Exact, semantic_domain: #12+1, semantic_domain_arguments: #0@0+0, semantic_domain_symbol: #6, form: Value }) at 7:40..45 exposure None partition Some(CompilerDerivedSelectionPartition(9)) occurrences #0@0+0
expression #35 = Call(TableCallExpression { callee: #0@0, authored_call_binding: None, receiver: #28, target_symbol: #7, target: "measure", requirement_application: #0@0, machine_arguments: [], arguments: #5+2, evidence_arguments: ["witness"], operational_acknowledgement: CallOperationalAcknowledgement { origin: Source, acknowledges_suspend: false, acknowledges_block: false } }) at 7:30..35 exposure Some(PublicInterface) partition None occurrences #6+2
expression #36 = Integer(IntegerLiteral { text: "0", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #37 = Range(TableRangeExpression { start: #36, end: #0@0, end_inclusive: false }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #38 = String([72, 97, 108, 108]) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #39 = Integer(IntegerLiteral { text: "11", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #40 = Integer(IntegerLiteral { text: "13", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #41 = Binary(TableBinaryExpression { left: #39, operator: Add, right: #40, policy: Exact }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #42 = Name(TableNamePath { members: #13+1, member_symbols: #7+1, head_symbol: #3, symbol: #3, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #43 = Integer(IntegerLiteral { text: "3", landing: None }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #44 = Indexed(TableIndexedExpression { collection: #42, index: #43 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #45 = StructLiteral(TableStructLiteral { application_type: #0@0, type_name: "Room", type_symbol: #11, case_name: Some("Open"), case_symbol: Some(#12), fields: #4+3 }) at 7:60..65 exposure None partition None occurrences #0@0+0
expression #46 = Name(TableNamePath { members: #14+1, member_symbols: #0@0+0, head_symbol: #0@0, symbol: #0@0, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #47 = Member(TableMemberExpression { receiver: #46, member_symbol: #4, member: "count", case_variant: Some("Full") }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #48 = Borrow(TableBorrowExpression { target: #47, access: Mutable }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #49 = Name(TableNamePath { members: #15+1, member_symbols: #8+1, head_symbol: #4, symbol: #4, static_application: #0@0, reserved_result_of: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #50 = Atomic(TableAtomicExpression { value: #48, result: #49, ordering: Load(GlobalOrder), result_custody: Scalar }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #51 = UndeterminedNaN(F32) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #52 = Membership(TableMembershipExpression { is_pattern: false, value: #51, domain: #16+1, domain_symbol: #14, case_type_symbol: #0@0, case_symbol: #0@0 }) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #53 = ZeroValue(#13) at 0:0..0 exposure None partition None occurrences #0@0+0
expression #54 = Match(TableMatchExpression { subject: #35, capture: #0@0, arms: #4+3 }) at 7:20..25 exposure None partition Some(CompilerDerivedSelectionPartition(4)) occurrences #8+1
occurrence #5 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #6 = AuthoredDeclarationSelectionOccurrenceId(1)
occurrence #7 = AuthoredDeclarationSelectionOccurrenceId(0)
occurrence #8 = AuthoredDeclarationSelectionOccurrenceId(0)
handle #5 = #31
handle #6 = #34
handle #7 = #29
handle #8 = #30
member #9 = "self"
member #10 = "room"
member #11 = "u8"
member #12 = "Meters"
member #13 = "grid"
member #14 = "cell"
member #15 = "previous"
member #16 = "Small"
member symbol #5 = #3
member symbol #6 = #3
member symbol #7 = #3
member symbol #8 = #4
field #4 = TableStructLiteralField { name: "name", field_symbol: #8, value: #38 }
field #5 = TableStructLiteralField { name: "proof", field_symbol: #9, value: #41 }
field #6 = TableStructLiteralField { name: "size", field_symbol: #10, value: #44 }
arm #4 = TableMatchArm { pattern: Value(#37), value: #45, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 70, end: 75 } } }
arm #5 = TableMatchArm { pattern: Value(#50), value: #52, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 80, end: 85 } } }
arm #6 = TableMatchArm { pattern: Wildcard, value: #53, source_span: SourceSpan { source_id: SourceId(7), span: Span { start: 90, end: 95 } } }
"#;

        #[test]
        fn materialized_value_copy_retains_its_distinct_authored_computation() {
            let mut source = ExpressionTable::new();
            let authored = source.insert(ExpressionNode::Boolean(false));
            let value = source.insert(ExpressionNode::Boolean(true));
            source.retain_materialization_source(value, authored);
            let own = source.copy_from_self(value);
            let mut destination = ExpressionTable::new();
            for _ in 0..8 {
                destination.insert(ExpressionNode::Boolean(true));
            }
            let copied = destination.copy_from(&source, value);
            for (table, copied) in [(&source, own), (&destination, copied)] {
                let authored = table.materialization_source(copied);
                assert!(authored.is_valid() && authored != copied);
                assert_eq!(table.expression(copied), &ExpressionNode::Boolean(true));
                assert_eq!(table.expression(authored), &ExpressionNode::Boolean(false));
            }
        }
    }

    pub use display::display_name_path;
    pub use numerics::literals::FloatLiteral;

    use crate::AuthoredDeclarationSelectionOccurrenceId;
    use crate::name::DiagnosticName;
    use arena::{Arena, Handle, HandleSpan};
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, CompilerDerivedSelectionPartition,
    };
    use numerics::literals::IntegerLiteral;
    use source::SourceSpan;
    use symbols::SymbolHandle;

    pub type ExpressionHandle = Handle<ExpressionNode>;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    struct StoredAuthoredSelectionOccurrenceId(Option<AuthoredDeclarationSelectionOccurrenceId>);

    impl StoredAuthoredSelectionOccurrenceId {
        fn new(occurrence: AuthoredDeclarationSelectionOccurrenceId) -> Self {
            Self(Some(occurrence))
        }

        fn occurrence(self) -> AuthoredDeclarationSelectionOccurrenceId {
            self.0
                .expect("expression occurrence spans cannot contain the arena dummy sentinel")
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ExpressionTable {
        nodes: ExpressionNodeStorage,
        spans: ExpressionSpanStorage,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    struct ExpressionNodeStorage {
        expressions: Arena<ExpressionNode>,
        source_spans: Vec<SourceSpan>,
        materialization_sources: Vec<ExpressionHandle>,

        authored_expression_exposures: Vec<Option<AuthoredDeclarationSelectionExposure>>,
        compiler_selection_partitions: Vec<Option<CompilerDerivedSelectionPartition>>,
        authored_selection_occurrences: Vec<HandleSpan<StoredAuthoredSelectionOccurrenceId>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    struct ExpressionSpanStorage {
        expression_handles: Arena<ExpressionHandle>,
        name_path_members: Arena<DiagnosticName>,
        name_path_member_symbols: Arena<SymbolHandle>,
        struct_fields: Arena<TableStructLiteralField>,
        match_arms: Arena<TableMatchArm>,
        match_fields: Arena<TableMatchField>,
        match_subjects: Arena<MatchSubjectCapture>,
        static_members: Arena<StaticMachineArgument>,
        match_proof_selectors: Arena<crate::statement::OutcomeProofSelector>,
        authored_selection_occurrence_ids: Arena<StoredAuthoredSelectionOccurrenceId>,
    }

    pub(crate) struct PreparedStaticArgumentRebase {
        arguments: Box<[StaticMachineArgument]>,
        members: Vec<(Handle<StaticMachineArgument>, StaticMachineArgument)>,
    }

    pub(crate) struct PreparedExpressionSelectionRebase {
        remapped: Vec<Option<Vec<AuthoredDeclarationSelectionOccurrenceId>>>,
        arguments: Vec<(
            ExpressionHandle,
            Option<Handle<StaticMachineArgument>>,
            PreparedStaticArgumentRebase,
        )>,
    }

    macro_rules! expression_node_accessors {
    ($($(#[$doc:meta])* $name:ident($variant:ident) -> $node_type:ty;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(&self, handle: ExpressionHandle) -> Option<&$node_type> {
                match self.expression(handle) {
                    ExpressionNode::$variant(node) => Some(node),
                    _ => None,
                }
            }
        )*
    };
}

    impl ExpressionTable {
        pub fn insert_static_member(
            &mut self,
            member: StaticMachineArgument,
        ) -> Handle<StaticMachineArgument> {
            self.spans.static_members.insert(member)
        }

        pub fn static_member(
            &self,
            member: Handle<StaticMachineArgument>,
        ) -> Option<&StaticMachineArgument> {
            self.spans
                .static_members
                .is_valid(member)
                .then(|| self.spans.static_members.get(member))
        }

        pub fn static_member_mut(
            &mut self,
            member: Handle<StaticMachineArgument>,
        ) -> Option<&mut StaticMachineArgument> {
            if self.spans.static_members.is_valid(member) {
                Some(self.spans.static_members.get_mut(member))
            } else {
                None
            }
        }

        pub fn insert_match_arms(
            &mut self,
            arms: impl IntoIterator<Item = TableMatchArm>,
        ) -> HandleSpan<TableMatchArm> {
            self.spans.match_arms.insert_many(arms)
        }

        pub fn match_arms(&self, arms: HandleSpan<TableMatchArm>) -> &[TableMatchArm] {
            self.spans.match_arms.span_or_empty(arms)
        }

        pub fn match_arm_mut(&mut self, arm: Handle<TableMatchArm>) -> &mut TableMatchArm {
            self.spans.match_arms.get_mut(arm)
        }

        pub fn insert_match_fields(
            &mut self,
            fields: impl IntoIterator<Item = TableMatchField>,
        ) -> HandleSpan<TableMatchField> {
            self.spans.match_fields.insert_many(fields)
        }

        pub fn match_fields(&self, fields: HandleSpan<TableMatchField>) -> &[TableMatchField] {
            self.spans.match_fields.span_or_empty(fields)
        }

        pub fn match_pattern_dependencies(
            &self,
            pattern: MatchPattern,
        ) -> impl Iterator<Item = ExpressionHandle> + '_ {
            let (head, fields, guard) = match pattern {
                MatchPattern::Value(value) => {
                    (value, HandleSpan::empty(), ExpressionHandle::invalid())
                }
                MatchPattern::Destructure(pattern) => {
                    (pattern.head, HandleSpan::empty(), pattern.condition)
                }
                MatchPattern::Wildcard => (
                    ExpressionHandle::invalid(),
                    HandleSpan::empty(),
                    ExpressionHandle::invalid(),
                ),
            };
            head.is_valid()
                .then_some(head)
                .into_iter()
                .chain(
                    self.match_fields(fields)
                        .iter()
                        .filter_map(|field| match &field.action {
                            MatchFieldAction::Equal(value) => Some(*value),
                            _ => None,
                        }),
                )
                .chain(guard.is_valid().then_some(guard))
        }

        pub fn match_field_mut(&mut self, field: Handle<TableMatchField>) -> &mut TableMatchField {
            self.spans.match_fields.get_mut(field)
        }

        pub fn insert_match_subject(
            &mut self,
            subject: ExpressionHandle,
        ) -> Handle<MatchSubjectCapture> {
            self.spans.match_subjects.insert(MatchSubjectCapture {
                purpose: SubjectCapturePurpose::Match,
                subject,
            })
        }

        pub fn insert_transition_subject(
            &mut self,
            subject: ExpressionHandle,
        ) -> Handle<MatchSubjectCapture> {
            self.spans.match_subjects.insert(MatchSubjectCapture {
                purpose: SubjectCapturePurpose::Transition,
                subject,
            })
        }

        pub fn match_subject(&self, capture: Handle<MatchSubjectCapture>) -> &MatchSubjectCapture {
            self.spans.match_subjects.get(capture)
        }

        pub fn insert_match_proof_selectors(
            &mut self,
            selectors: impl IntoIterator<Item = crate::statement::OutcomeProofSelector>,
        ) -> HandleSpan<crate::statement::OutcomeProofSelector> {
            self.spans.match_proof_selectors.insert_many(selectors)
        }

        pub fn match_proof_selectors(
            &self,
            selectors: HandleSpan<crate::statement::OutcomeProofSelector>,
        ) -> &[crate::statement::OutcomeProofSelector] {
            self.spans.match_proof_selectors.span_or_empty(selectors)
        }

        pub fn new() -> Self {
            Self {
                nodes: ExpressionNodeStorage {
                    expressions: Arena::new(),
                    source_spans: Vec::new(),
                    materialization_sources: Vec::new(),
                    authored_expression_exposures: Vec::new(),
                    compiler_selection_partitions: Vec::new(),
                    authored_selection_occurrences: Vec::new(),
                },
                spans: ExpressionSpanStorage {
                    expression_handles: Arena::new(),
                    name_path_members: Arena::new(),
                    name_path_member_symbols: Arena::new(),
                    struct_fields: Arena::new(),
                    match_arms: Arena::new(),
                    match_fields: Arena::new(),
                    match_subjects: Arena::new(),
                    static_members: Arena::new(),
                    match_proof_selectors: Arena::new(),
                    authored_selection_occurrence_ids: Arena::new(),
                },
            }
        }

        pub fn clear(&mut self) {
            self.nodes.expressions.reset_retain_capacity();
            self.nodes.source_spans.clear();
            self.nodes.materialization_sources.clear();
            self.nodes.authored_expression_exposures.clear();
            self.nodes.compiler_selection_partitions.clear();
            self.nodes.authored_selection_occurrences.clear();
            self.spans.expression_handles.reset_retain_capacity();
            self.spans.name_path_members.reset_retain_capacity();
            self.spans.name_path_member_symbols.reset_retain_capacity();
            self.spans.struct_fields.reset_retain_capacity();
            self.spans.match_arms.reset_retain_capacity();
            self.spans.match_fields.reset_retain_capacity();
            self.spans.match_subjects.reset_retain_capacity();
            self.spans.static_members.reset_retain_capacity();
            self.spans.match_proof_selectors.reset_retain_capacity();
            self.spans
                .authored_selection_occurrence_ids
                .reset_retain_capacity();
        }

        pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
            let handle = self.nodes.expressions.insert(expression);
            self.nodes.source_spans.push(SourceSpan::default());
            self.nodes
                .materialization_sources
                .push(ExpressionHandle::invalid());
            self.nodes.authored_expression_exposures.push(None);
            self.nodes.compiler_selection_partitions.push(None);
            self.nodes
                .authored_selection_occurrences
                .push(HandleSpan::empty());
            debug_assert_eq!(source_span_index(handle), self.nodes.source_spans.len() - 1);
            debug_assert_eq!(
                source_span_index(handle),
                self.nodes.authored_expression_exposures.len() - 1
            );
            debug_assert_eq!(
                source_span_index(handle),
                self.nodes.compiler_selection_partitions.len() - 1
            );
            debug_assert_eq!(
                source_span_index(handle),
                self.nodes.authored_selection_occurrences.len() - 1
            );
            handle
        }

        pub fn materialization_source(&self, handle: ExpressionHandle) -> ExpressionHandle {
            self.nodes
                .materialization_sources
                .get(source_span_index(handle))
                .copied()
                .unwrap_or_default()
        }

        pub fn retain_materialization_source(
            &mut self,
            value: ExpressionHandle,
            authored: ExpressionHandle,
        ) {
            assert!(value.is_valid() && authored.is_valid() && value != authored);
            assert!(!self.materialization_source(value).is_valid());
            self.nodes.materialization_sources[source_span_index(value)] = authored;
        }

        pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
            self.nodes.source_spans[source_span_index(handle)]
        }

        pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
            self.nodes.source_spans[source_span_index(handle)] = source_span;
        }

        pub fn authored_expression_exposure(
            &self,
            handle: ExpressionHandle,
        ) -> Option<AuthoredDeclarationSelectionExposure> {
            self.nodes.authored_expression_exposures[source_span_index(handle)]
        }

        pub fn set_authored_expression_exposure(
            &mut self,
            handle: ExpressionHandle,
            exposure: AuthoredDeclarationSelectionExposure,
        ) {
            let slot = &mut self.nodes.authored_expression_exposures[source_span_index(handle)];
            if let Some(existing) = *slot {
                assert_eq!(
                    existing, exposure,
                    "one expression handle cannot represent two authored visibility positions"
                );
            }
            *slot = Some(exposure);
        }

        pub fn compiler_selection_partition(
            &self,
            handle: ExpressionHandle,
        ) -> Option<CompilerDerivedSelectionPartition> {
            self.nodes.compiler_selection_partitions[source_span_index(handle)]
        }

        pub fn set_compiler_selection_partition(
            &mut self,
            handle: ExpressionHandle,
            partition: CompilerDerivedSelectionPartition,
        ) {
            let slot = &mut self.nodes.compiler_selection_partitions[source_span_index(handle)];
            if let Some(existing) = *slot {
                assert_eq!(
                    existing, partition,
                    "one expression handle cannot belong to two compiler selection partitions"
                );
            }
            *slot = Some(partition);
        }

        pub fn attach_authored_selection_occurrences(
            &mut self,
            handle: ExpressionHandle,
            occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
        ) {
            let mut combined = self
                .authored_selection_occurrences(handle)
                .collect::<Vec<_>>();

            for occurrence in occurrences {
                if !combined.contains(&occurrence) {
                    combined.push(occurrence);
                }
            }

            self.replace_authored_selection_occurrences(handle, combined);
        }

        pub fn replace_authored_selection_occurrences(
            &mut self,
            handle: ExpressionHandle,
            occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
        ) {
            let mut unique = Vec::new();
            for occurrence in occurrences {
                if !unique.contains(&occurrence) {
                    unique.push(occurrence);
                }
            }
            self.nodes.authored_selection_occurrences[source_span_index(handle)] =
                self.spans.authored_selection_occurrence_ids.insert_many(
                    unique
                        .into_iter()
                        .map(StoredAuthoredSelectionOccurrenceId::new),
                );
        }

        pub fn authored_selection_occurrences(
            &self,
            handle: ExpressionHandle,
        ) -> impl ExactSizeIterator<Item = AuthoredDeclarationSelectionOccurrenceId> + '_ {
            self.spans
                .authored_selection_occurrence_ids
                .span_or_empty(self.nodes.authored_selection_occurrences[source_span_index(handle)])
                .iter()
                .copied()
                .map(StoredAuthoredSelectionOccurrenceId::occurrence)
        }

        pub(crate) fn prepare_static_argument_rebase(
            &self,
            arguments: &[StaticMachineArgument],
            remap: impl Fn(
                AuthoredDeclarationSelectionOccurrenceId,
            ) -> Option<AuthoredDeclarationSelectionOccurrenceId>,
        ) -> Result<PreparedStaticArgumentRebase, ()> {
            fn argument(
                table: &ExpressionTable,
                value: &mut StaticMachineArgument,
                remap: &impl Fn(
                    AuthoredDeclarationSelectionOccurrenceId,
                ) -> Option<AuthoredDeclarationSelectionOccurrenceId>,
                members: &mut Vec<(Handle<StaticMachineArgument>, StaticMachineArgument)>,
                active: &mut Vec<Handle<StaticMachineArgument>>,
            ) -> Result<(), ()> {
                if let Some(occurrence) = value.authored_selection {
                    value.authored_selection = Some(remap(occurrence).ok_or(())?);
                }
                if let Some(application) = &mut value.application {
                    for child in &mut application.arguments {
                        argument(table, child, remap, members, active)?;
                    }
                    let handle = application.member;
                    if handle.is_valid() {
                        let key = |handle: Handle<StaticMachineArgument>| {
                            (handle.arena_index(), handle.generation())
                        };
                        if members
                            .binary_search_by_key(&key(handle), |(handle, _)| key(*handle))
                            .is_err()
                        {
                            if active.contains(&handle) {
                                return Err(());
                            }
                            active.push(handle);
                            let mut child = table.static_member(handle).ok_or(())?.clone();
                            argument(table, &mut child, remap, members, active)?;
                            active.pop();
                            let at =
                                members.partition_point(|(current, _)| key(*current) < key(handle));
                            members.insert(at, (handle, child));
                        }
                    }
                }
                Ok(())
            }
            let mut values = arguments.to_vec().into_boxed_slice();
            let mut members = Vec::new();
            for value in &mut values {
                argument(self, value, &remap, &mut members, &mut Vec::new())?;
            }
            Ok(PreparedStaticArgumentRebase {
                arguments: values,
                members,
            })
        }

        pub(crate) fn apply_static_argument_rebase(
            &mut self,
            prepared: PreparedStaticArgumentRebase,
        ) -> Box<[StaticMachineArgument]> {
            for (handle, member) in prepared.members {
                *self
                    .static_member_mut(handle)
                    .expect("prepared static member remains current") = member;
            }
            prepared.arguments
        }

        pub(crate) fn prepare_authored_selection_rebase(
            &self,
            expression_frontier: usize,
            rebase: language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
        ) -> Result<PreparedExpressionSelectionRebase, ()> {
            if expression_frontier > self.expression_count() {
                return Err(());
            }
            let remapped = self
                .iter_expressions()
                .enumerate()
                .map(|(index, (handle, _))| {
                    let occurrences = self
                        .authored_selection_occurrences(handle)
                        .map(|occurrence| {
                            if index < expression_frontier {
                                rebase.retain_base(occurrence)
                            } else {
                                rebase.rebase_appended(occurrence)
                            }
                            .ok_or(())
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((index >= expression_frontier).then_some(occurrences))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut arguments = Vec::new();
            for (index, (handle, node)) in self.iter_expressions().enumerate() {
                let (members, values) = match node {
                    ExpressionNode::Call(call) => (None, call.machine_arguments.as_ref()),
                    ExpressionNode::Name(name) if name.static_application.is_valid() => (
                        Some(name.static_application),
                        std::slice::from_ref(
                            self.static_member(name.static_application).ok_or(())?,
                        ),
                    ),
                    _ => continue,
                };
                let prepared = self.prepare_static_argument_rebase(values, |occurrence| {
                    if index < expression_frontier {
                        rebase.retain_base(occurrence)
                    } else {
                        rebase.rebase_appended(occurrence)
                    }
                })?;
                if index >= expression_frontier {
                    arguments.push((handle, members, prepared));
                }
            }
            Ok(PreparedExpressionSelectionRebase {
                remapped,
                arguments,
            })
        }

        pub(crate) fn apply_authored_selection_rebase(
            &mut self,
            prepared: PreparedExpressionSelectionRebase,
        ) {
            let PreparedExpressionSelectionRebase {
                remapped,
                arguments,
            } = prepared;
            for (handle, member, prepared) in arguments {
                let mut arguments = self.apply_static_argument_rebase(prepared);
                if let Some(member) = member {
                    *self
                        .static_member_mut(member)
                        .expect("prepared applied name remains current") = arguments[0].clone();
                } else {
                    let ExpressionNode::Call(call) = self.expression_mut(handle) else {
                        unreachable!("prepared call remains current")
                    };
                    call.machine_arguments = std::mem::take(&mut arguments);
                }
            }
            for (index, occurrences) in remapped.into_iter().enumerate() {
                let Some(occurrences) = occurrences else {
                    continue;
                };
                self.nodes.authored_selection_occurrences[index] =
                    self.spans.authored_selection_occurrence_ids.insert_many(
                        occurrences
                            .into_iter()
                            .map(StoredAuthoredSelectionOccurrenceId::new),
                    );
            }
        }

        pub fn insert_expression_handles(
            &mut self,
            expressions: impl IntoIterator<Item = ExpressionHandle>,
        ) -> HandleSpan<ExpressionHandle> {
            self.spans.expression_handles.insert_many(expressions)
        }

        pub fn reserve_expression_handles(&mut self, count: u32) -> HandleSpan<ExpressionHandle> {
            self.spans.expression_handles.insert_many(
                std::iter::repeat_with(ExpressionHandle::invalid)
                    .take(usize::try_from(count).expect("expression handle span count overflow")),
            )
        }

        pub fn set_expression_handle_at_offset(
            &mut self,
            expressions: HandleSpan<ExpressionHandle>,
            offset: u32,
            expression: ExpressionHandle,
        ) {
            *self
                .spans
                .expression_handles
                .get_mut(expressions.handle_at(offset)) = expression;
        }

        pub fn push_expression_handle(
            &mut self,
            span: &mut HandleSpan<ExpressionHandle>,
            expression: ExpressionHandle,
        ) {
            self.spans
                .expression_handles
                .append_to_span(span, expression);
        }

        pub fn insert_struct_fields(
            &mut self,
            fields: impl IntoIterator<Item = TableStructLiteralField>,
        ) -> HandleSpan<TableStructLiteralField> {
            self.spans.struct_fields.insert_many(fields)
        }

        pub fn reserve_struct_fields(&mut self, count: u32) -> HandleSpan<TableStructLiteralField> {
            self.spans.struct_fields.insert_many(
                std::iter::repeat_with(TableStructLiteralField::default).take(
                    usize::try_from(count).expect("struct literal field span count overflow"),
                ),
            )
        }

        pub fn set_struct_field_at_offset(
            &mut self,
            fields: HandleSpan<TableStructLiteralField>,
            offset: u32,
            field: TableStructLiteralField,
        ) {
            *self.spans.struct_fields.get_mut(fields.handle_at(offset)) = field;
        }

        pub fn push_struct_field(
            &mut self,
            span: &mut HandleSpan<TableStructLiteralField>,
            field: TableStructLiteralField,
        ) {
            self.spans.struct_fields.append_to_span(span, field);
        }

        pub fn push_name_path_member(
            &mut self,
            span: &mut HandleSpan<DiagnosticName>,
            member: DiagnosticName,
        ) {
            self.spans.name_path_members.append_to_span(span, member);
        }

        pub fn reserve_name_path_members(&mut self, count: u32) -> HandleSpan<DiagnosticName> {
            self.spans.name_path_members.insert_many(
                std::iter::repeat_with(DiagnosticName::default)
                    .take(usize::try_from(count).expect("name path member span count overflow")),
            )
        }

        pub fn set_name_path_member_at_offset(
            &mut self,
            members: HandleSpan<DiagnosticName>,
            offset: u32,
            member: DiagnosticName,
        ) {
            *self
                .spans
                .name_path_members
                .get_mut(members.handle_at(offset)) = member;
        }

        pub fn reserve_name_path_member_symbols(&mut self, count: u32) -> HandleSpan<SymbolHandle> {
            self.spans.name_path_member_symbols.insert_many(
                std::iter::repeat_with(SymbolHandle::invalid).take(
                    usize::try_from(count).expect("name path member symbol span count overflow"),
                ),
            )
        }

        pub fn set_name_path_member_symbol_at_offset(
            &mut self,
            member_symbols: HandleSpan<SymbolHandle>,
            offset: u32,
            member_symbol: SymbolHandle,
        ) {
            *self
                .spans
                .name_path_member_symbols
                .get_mut(member_symbols.handle_at(offset)) = member_symbol;
        }

        pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
            self.nodes.expressions.get(handle)
        }

        expression_node_accessors! {
            name_path_node(Name) -> TableNamePath;

            member_node(Member) -> TableMemberExpression;

            indexed_node(Indexed) -> TableIndexedExpression;

            cast_node(Cast) -> TableCastExpression;

            binary_node(Binary) -> TableBinaryExpression;

            call_node(Call) -> TableCallExpression;

            unary_node(Unary) -> TableUnaryExpression;

            integer_node(Integer) -> IntegerLiteral;

        }

        pub fn expression_is_valid(&self, handle: ExpressionHandle) -> bool {
            self.nodes.expressions.is_valid(handle)
        }

        pub fn expression_mut(&mut self, handle: ExpressionHandle) -> &mut ExpressionNode {
            self.nodes.expressions.get_mut(handle)
        }

        pub fn iter_expressions(
            &self,
        ) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
            self.nodes.expressions.iter()
        }

        pub fn expression_handles(
            &self,
            span: HandleSpan<ExpressionHandle>,
        ) -> &[ExpressionHandle] {
            self.spans.expression_handles.span_or_empty(span)
        }

        pub fn expression_handle_at_offset(
            &self,
            expressions: HandleSpan<ExpressionHandle>,
            offset: u32,
        ) -> &ExpressionHandle {
            self.spans
                .expression_handles
                .get(expressions.handle_at(offset))
        }

        pub fn struct_fields(
            &self,
            span: HandleSpan<TableStructLiteralField>,
        ) -> &[TableStructLiteralField] {
            self.spans.struct_fields.span_or_empty(span)
        }

        pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
            self.spans.name_path_members.span_or_empty(span)
        }

        pub fn name_path_member_symbols(&self, span: HandleSpan<SymbolHandle>) -> &[SymbolHandle] {
            self.spans.name_path_member_symbols.span_or_empty(span)
        }

        pub fn expression_count(&self) -> usize {
            self.nodes.expressions.len()
        }

        pub fn display_name(&self, handle: ExpressionHandle) -> String {
            self.expression(handle).display_name(self)
        }
    }

    fn source_span_index(handle: ExpressionHandle) -> usize {
        usize::try_from(handle.arena_index())
            .expect("expression index overflow")
            .checked_sub(1)
            .expect("invalid expression handle has no source span")
    }

    impl Default for ExpressionTable {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum ExpressionNode {
        Match(TableMatchExpression),
        SavedSubject(TableSavedSubject),
        ArrayLiteral(HandleSpan<ExpressionHandle>),
        Atomic(TableAtomicExpression),
        Binary(TableBinaryExpression),
        Boolean(bool),
        Cast(TableCastExpression),
        Call(TableCallExpression),
        Float(FloatLiteral),
        UndeterminedNaN(numerics::literals::FloatFormat),
        Indexed(TableIndexedExpression),
        Integer(IntegerLiteral),
        Membership(TableMembershipExpression),
        Member(TableMemberExpression),
        Borrow(TableBorrowExpression),
        Name(TableNamePath),
        Range(TableRangeExpression),
        StructLiteral(TableStructLiteral),
        String(source::SourceBytes),
        Unary(TableUnaryExpression),
        TypeExpression(arena::Handle<crate::types::TypeReference>),
        ZeroValue(arena::Handle<crate::types::TypeReference>),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableBorrowExpression {
        pub target: ExpressionHandle,
        pub access: language_core::ReferenceAccess,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableAtomicExpression {
        pub value: ExpressionHandle,
        pub result: ExpressionHandle,
        pub ordering: language_core::atomic::AtomicOrderingPlan,
        pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
    }

    impl Default for ExpressionNode {
        fn default() -> Self {
            Self::Integer(IntegerLiteral::zero())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableBinaryExpression {
        pub left: ExpressionHandle,
        pub operator: BinaryOperator,
        pub right: ExpressionHandle,
        /// The policy an operator suffix chose for this one operation
        /// (`+%`, `+|`, `+!`); `Exact` for the bare operator.
        pub policy: numerics::arithmetic::ArithmeticDomain,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableUnaryExpression {
        pub operator: UnaryOperator,
        pub operand: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableCastExpression {
        pub value: ExpressionHandle,
        pub target_type: arena::Handle<crate::types::TypeReference>,
        pub target_label: HandleSpan<DiagnosticName>,
        pub domain: numerics::arithmetic::ArithmeticDomain,
        pub semantic_domain: HandleSpan<DiagnosticName>,
        pub semantic_domain_arguments: HandleSpan<crate::types::TypeReference>,
        pub semantic_domain_symbol: SymbolHandle,
        pub form: language_core::cast_form::CastForm,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableIndexedExpression {
        pub collection: ExpressionHandle,
        pub index: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableRangeExpression {
        pub start: ExpressionHandle,
        pub end: ExpressionHandle,
        pub end_inclusive: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMembershipExpression {
        pub is_pattern: bool,
        pub value: ExpressionHandle,
        pub domain: HandleSpan<DiagnosticName>,
        pub domain_symbol: SymbolHandle,
        pub case_type_symbol: SymbolHandle,
        pub case_symbol: SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMemberExpression {
        pub receiver: ExpressionHandle,
        pub member_symbol: SymbolHandle,
        pub member: DiagnosticName,
        pub case_variant: Option<DiagnosticName>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableCallExpression {
        pub callee: ExpressionHandle,
        pub authored_call_binding:
            Option<language_semantics::declaration_selection::ResolvedAuthoredCallBinding>,
        pub receiver: ExpressionHandle,
        pub target_symbol: SymbolHandle,
        pub target: DiagnosticName,
        pub requirement_application: arena::Handle<crate::types::TypeReference>,
        pub machine_arguments: Box<[StaticMachineArgument]>,
        pub arguments: HandleSpan<ExpressionHandle>,
        pub evidence_arguments: Box<[DiagnosticName]>,
        pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct StaticMachineArgument {
        /// Exact ledger occurrence attached when resolution mints this input.
        pub authored_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
        pub authored_selection_target: Option<SymbolHandle>,
        pub type_reference: arena::Handle<crate::types::TypeReference>,
        pub path: Box<[DiagnosticName]>,
        pub application: Option<Box<StaticSymbolApplication>>,
        pub const_literal: Option<numerics::literals::IntegerLiteral>,
        pub evidence_projection: Option<EvidenceProjection>,
        pub symbol: SymbolHandle,
    }

    impl compiler_storage::Record for StaticMachineArgument {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            b"ARGSEL1".write(output)?;
            self.authored_selection.write(output)?;
            self.authored_selection_target.write(output)?;
            self.type_reference.write(output)?;
            self.path.write(output)?;
            self.application.write(output)?;
            self.const_literal.write(output)?;
            self.evidence_projection.write(output)?;
            self.symbol.write(output)
        }

        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            if <[u8; 7]>::read(input)? != *b"ARGSEL1" {
                return Err(input.error("static input selection storage has another version"));
            }
            Ok(Self {
                authored_selection: Option::read(input)?,
                authored_selection_target: Option::read(input)?,
                type_reference: Handle::read(input)?,
                path: Box::read(input)?,
                application: Option::read(input)?,
                const_literal: Option::read(input)?,
                evidence_projection: Option::read(input)?,
                symbol: SymbolHandle::read(input)?,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct StaticSymbolApplication {
        pub member: Handle<StaticMachineArgument>,
        pub lifetime_arguments: Box<[DiagnosticName]>,
        pub arguments: Box<[StaticMachineArgument]>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct EvidenceProjection {
        pub term: DiagnosticName,
        pub member: DiagnosticName,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableNamePath {
        pub members: HandleSpan<DiagnosticName>,
        pub member_symbols: HandleSpan<SymbolHandle>,
        pub head_symbol: SymbolHandle,
        pub symbol: SymbolHandle,
        pub static_application: arena::Handle<StaticMachineArgument>,
        /// The callable whose reserved `result` this name denotes, or invalid
        /// when it denotes no result. Stage 02 sets it once, where it decides a
        /// contract name is the reserved result occurrence
        /// (`wiki/spec/terminal-psi/calls_and_outcomes.md`: "The reserved
        /// result occurrence belongs to its contract owner, not any local or
        /// parameter spelled `result`"), so a later reader takes the result
        /// by this identity instead of comparing the spelling. `symbol` stays
        /// invalid, as before, so readers of the spelling route are unchanged.
        pub reserved_result_of: SymbolHandle,
    }

    impl TableNamePath {
        pub const fn new(
            members: HandleSpan<DiagnosticName>,
            member_symbols: HandleSpan<SymbolHandle>,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
        ) -> Self {
            Self {
                members,
                member_symbols,
                head_symbol,
                symbol,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct TableStructLiteral {
        pub application_type: Handle<crate::types::TypeReference>,
        pub type_name: DiagnosticName,
        pub type_symbol: SymbolHandle,
        pub case_name: Option<DiagnosticName>,
        pub case_symbol: Option<SymbolHandle>,
        pub fields: HandleSpan<TableStructLiteralField>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct TableStructLiteralField {
        pub name: DiagnosticName,
        pub field_symbol: SymbolHandle,
        pub value: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum BinaryOperator {
        Add,
        And,
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
        Divide,
        Equal,
        Greater,
        GreaterOrEqual,
        Less,
        LessOrEqual,
        Modulo,
        Multiply,
        NotEqual,
        Or,
        ShiftLeft,
        ShiftRight,
        Subtract,
    }

    impl BinaryOperator {
        pub fn is_comparison(self) -> bool {
            matches!(
                self,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            )
        }

        pub fn result_is_boolean(self) -> bool {
            self.is_comparison() || matches!(self, BinaryOperator::And | BinaryOperator::Or)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum UnaryOperator {
        BitwiseNot,
        LogicalNot,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMatchExpression {
        pub subject: ExpressionHandle,
        pub capture: Handle<MatchSubjectCapture>,
        pub arms: HandleSpan<TableMatchArm>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum SubjectCapturePurpose {
        #[default]
        Match,
        Transition,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct MatchSubjectCapture {
        pub purpose: SubjectCapturePurpose,
        pub subject: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableSavedSubject {
        pub component: language_core::PatternComponent,
        pub capture: Handle<MatchSubjectCapture>,
        pub pattern_head: ExpressionHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMatchArm {
        pub pattern: MatchPattern,
        pub value: ExpressionHandle,
        pub source_span: SourceSpan,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum MatchPattern {
        Value(ExpressionHandle),
        Destructure(TableDestructurePattern),
        #[default]
        Wildcard,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableDestructurePattern {
        pub application_type: Handle<crate::types::TypeReference>,
        pub head: ExpressionHandle,
        pub fields: HandleSpan<TableMatchField>,
        pub has_rest: bool,
        pub guard: ExpressionHandle,
        pub condition: ExpressionHandle,
        pub proof_selectors: HandleSpan<crate::statement::OutcomeProofSelector>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TableMatchField {
        pub component: language_core::PatternComponent,
        pub member: DiagnosticName,
        pub field_symbol: SymbolHandle,
        pub action: MatchFieldAction,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum MatchFieldAction {
        Bind(DiagnosticName),
        Equal(ExpressionHandle),
        #[default]
        Waive,
    }
}

#[cfg(test)]
mod static_input_record_tests {
    use crate::expression::StaticMachineArgument;
    use compiler_storage::{RecordLimits, decode, encode};
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelections,
    };
    use symbols::SymbolHandle;

    #[test]
    fn exact_static_input_attachment_round_trips_and_refuses_the_old_marker() {
        let selected = SymbolHandle::from_parts(3, 5);
        let mut ledger = AuthoredDeclarationSelections::default();
        let occurrence = ledger
            .record_resolved(
                source::SourceSpan::new(source::SourceId(0), source::Span::new(1, 2)),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::StaticArgument,
                selected,
            )
            .unwrap();
        let argument = StaticMachineArgument {
            authored_selection: Some(occurrence),
            authored_selection_target: Some(selected),
            symbol: selected,
            ..Default::default()
        };
        let mut bytes = encode(&argument, RecordLimits::default()).unwrap();
        assert_eq!(
            decode::<StaticMachineArgument>(&bytes, RecordLimits::default()).unwrap(),
            argument
        );
        bytes[8] ^= 1;
        assert!(decode::<StaticMachineArgument>(&bytes, RecordLimits::default()).is_err());
    }
}
