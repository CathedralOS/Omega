/*
We represent typed proof propositions, mathematical definitions, byte predicates,
and termination ranking evidence (`evidence`) for `TypedTrees`.
*/

pub mod evidence {
    pub mod byte_predicates {
        use crate::source_state::TypedTrees;
        use crate::source_state::domain::DomainDefinition;
        use crate::source_state::expression::ExpressionHandle;
        use crate::source_state::types::TypeReferenceNode;
        use symbols::{BuiltinTypeAtom, SymbolHandle};

        pub use semantic_vocabulary::byte_predicates::ByteSequencePredicate;

        pub fn byte_predicate_call(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<(ByteSequencePredicate, ExpressionHandle)> {
            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            let call = program.expression_table.call_node(expression)?;
            // Optional handles have one exact absent sentinel. A partial or
            // stale handle is present malformed custody, not an omitted input.
            if !program
                .symbols
                .symbols()
                .nodes()
                .is_valid(call.target_symbol)
                || (call.receiver != ExpressionHandle::invalid()
                    && !program.expression_table.expression_is_valid(call.receiver))
            {
                return None;
            }
            if !program.call_has_no_runtime_receiver_for_target(call)
                || call.runtime_call.is_some()
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.requirement_application != Default::default()
                || call.exact_coercion_target != Default::default()
                || call.build_selection != Default::default()
                || call.static_machine_parameter != Default::default()
                || call.static_requirement_dispatch.is_some()
                || call.quotient_operation.is_some()
                || call.private_layout_operation.is_some()
                || call.installed_interrupt_operation.is_some()
                || call.layout_query.is_some()
                || call.native_code_layout.is_some()
                || call.operational_acknowledgement != Default::default()
            {
                return None;
            }
            let (declaration, predicate) =
                byte_observation_declaration(program, call.target_symbol)?;
            let entry = program
                .machine_states(program.machine_by_symbol(declaration)?)
                .first()?
                .symbol;
            let names_target =
                |symbol| symbol == call.target_symbol || symbol == declaration || symbol == entry;
            if let Some(binding) = call.authored_call_binding
                && (binding.selected != call.target_symbol || binding.original != binding.selected)
            {
                return None;
            }
            if let Some(occurrence) = call.authored_call_selection {
                let selected = program.authored_declaration_selections().get(occurrence)?;
                if !matches!(selected.target(), crate::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(resolved)
                    if names_target(resolved.selected_symbol()))
                {
                    return None;
                }
            }
            if call.authored_machine_application != Default::default() {
                if !program
                    .expression_table
                    .static_machine_application_is_valid(call.authored_machine_application)
                {
                    return None;
                }
                let application = program
                    .expression_table
                    .static_machine_application(call.authored_machine_application);
                if !application.arguments.is_empty()
                    || !application.selected_machine_arguments.is_empty()
                    || !application.input_selections.is_empty()
                    || !application.bindings.is_empty()
                    || !application.lifetime_bindings.is_empty()
                    || application.conformance_selection.is_some()
                    || application.callable_contract_source != Default::default()
                    || (application.registration_operation != SymbolHandle::invalid()
                        && !names_target(application.registration_operation))
                {
                    return None;
                }
            }
            let arguments = program.expression_table.expression_handles(call.arguments);
            let [argument] = arguments else {
                return None;
            };
            if !program.expression_table.expression_is_valid(*argument) {
                return None;
            }
            Some((predicate, *argument))
        }

        /// Catalog custody and the complete canonical header nominate the
        /// observation. Callers still owe argument and storage formation.
        pub fn byte_observation_declaration(
            program: &TypedTrees,
            target: SymbolHandle,
        ) -> Option<(SymbolHandle, ByteSequencePredicate)> {
            let declaration = program
                .machine_by_symbol(target)
                .or_else(|| program.machine_holding_state(target))?;
            let (
                language_core::sealed_intrinsics::CompilerPrimitive::ByteObservation(predicate),
                _,
            ) = program
                .compiler_primitive_schema(declaration.symbol)
                .ok()??
            else {
                return None;
            };
            Some((declaration.symbol, predicate))
        }

        pub fn domain_byte_predicate(
            program: &TypedTrees,
            domain_symbol: SymbolHandle,
        ) -> Option<ByteSequencePredicate> {
            let domain = program.domain_definition(domain_symbol)?;
            domain_byte_predicate_fact(program, domain).map(|(_, predicate)| predicate)
        }

        /// The exact fact read by the byte-slice checker. Keep the authored
        /// subject and call occurrence when an applied theory supplies the carrier.
        pub fn byte_slice_domain_predicate(
            program: &TypedTrees,
            theory: &DomainDefinition,
        ) -> Option<(ExpressionHandle, ByteSequencePredicate)> {
            byte_carrier_domain_predicate(program, theory, ByteCarrier::Slice)
        }

        /// The one primitive byte predicate of a domain declared on `[u8]` or
        /// on a fixed `[u8; N]`, with the call that states it. A fixed array's
        /// predicate is a clause over its bytes; a slice's is judged from the
        /// bytes each store supplies.
        pub fn byte_domain_predicate(
            program: &TypedTrees,
            theory: &DomainDefinition,
        ) -> Option<(ExpressionHandle, ByteSequencePredicate)> {
            byte_carrier_domain_predicate(program, theory, ByteCarrier::SliceOrFixedArray)
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum ByteCarrier {
            Slice,
            SliceOrFixedArray,
        }

        fn byte_carrier_domain_predicate(
            program: &TypedTrees,
            theory: &DomainDefinition,
            carrier: ByteCarrier,
        ) -> Option<(ExpressionHandle, ByteSequencePredicate)> {
            let types = &program.type_reference_table;
            if !types.contains_type_reference(theory.target_type) {
                return None;
            }
            let element_type = match types.type_reference(theory.target_type) {
                TypeReferenceNode::Slice { element_type } => element_type,
                TypeReferenceNode::FixedArray { element_type, .. }
                    if carrier == ByteCarrier::SliceOrFixedArray =>
                {
                    element_type
                }
                _ => return None,
            };
            if !types.contains_type_reference(*element_type) {
                return None;
            }
            let TypeReferenceNode::Named { symbol, .. } = types.type_reference(*element_type)
            else {
                return None;
            };
            if program.symbols.builtin_type_atom(*symbol) != Some(BuiltinTypeAtom::U8) {
                return None;
            }
            let declaration = program.domain_definition(theory.symbol)?;
            domain_byte_predicate_fact(program, declaration)
        }

        fn domain_byte_predicate_fact(
            program: &TypedTrees,
            domain: &DomainDefinition,
        ) -> Option<(ExpressionHandle, ByteSequencePredicate)> {
            if !program
                .symbols
                .symbols()
                .nodes()
                .is_valid(domain.subject_symbol)
                || domain.alias.is_some()
                || !domain.index_arguments.is_empty()
                || domain.issuance() != crate::source_state::domain::DomainIssuance::Unrestricted
                || !crate::source_state::domain::index_parameters(program, domain).is_empty()
            {
                return None;
            }
            let Some([crate::source_state::domain::ProofFact::Expression(expression)]) =
                program.proof_facts.span_or_empty(domain.facts).as_array()
            else {
                return None;
            };
            let (predicate, argument) = byte_predicate_call(program, *expression)?;
            // The domain subject is a logical value, not an implicit loan.
            // Its declaration must explicitly form the shared whole view the
            // selected observer's ordinary formal receives.
            if !matches!(program.expression_table.expression(argument),
                crate::source_state::expression::ExpressionNode::Borrow(borrow)
                    if borrow.access == language_core::ReferenceAccess::Shared)
            {
                return None;
            }
            let subject = whole_byte_observation_subject(program, argument)?;
            crate::source_state::domain::domain_subject_reference(program, domain, subject)
                .then_some((*expression, predicate))
        }

        /// A shared loan of an entire range observes the same bytes as its
        /// backing. Partial ranges remain distinct subjects, including UTF-8
        /// windows whose endpoints need not be character boundaries.
        pub fn whole_byte_observation_subject(
            program: &TypedTrees,
            argument: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            use crate::source_state::expression::ExpressionNode;
            let table = &program.expression_table;
            let ExpressionNode::Borrow(borrow) = table.expression(argument) else {
                return Some(argument);
            };
            if borrow.access != language_core::ReferenceAccess::Shared {
                return None;
            }
            let ExpressionNode::Indexed(indexed) = table.expression(borrow.target) else {
                return Some(borrow.target);
            };
            let ExpressionNode::Range(range) = table.expression(indexed.index) else {
                return Some(borrow.target);
            };
            (!range.start.is_valid() && !range.end.is_valid() && !range.end_inclusive)
                .then_some(indexed.collection)
        }

        /// The exact primitive predicate of a declared byte-slice domain.
        /// Its subject is the domain's immutable carrier, not a callable
        /// formal reconstructed from the predicate's spelling.
        pub fn primitive_byte_slice_domain(
            program: &TypedTrees,
            domain_symbol: SymbolHandle,
        ) -> Option<ByteSequencePredicate> {
            let domain = program.domain_definition(domain_symbol)?;
            let table = &program.type_reference_table;
            if !table.contains_type_reference(domain.target_type) {
                return None;
            }
            let TypeReferenceNode::Slice { element_type } =
                table.type_reference(domain.target_type)
            else {
                return None;
            };
            if !table.contains_type_reference(*element_type) {
                return None;
            }
            let TypeReferenceNode::Named { symbol, .. } = table.type_reference(*element_type)
            else {
                return None;
            };
            if !program.symbols.symbols().nodes().is_valid(*symbol)
                || program.symbols.builtin_type_atom(*symbol) != Some(symbols::BuiltinTypeAtom::U8)
            {
                return None;
            }
            domain_byte_predicate(program, domain_symbol)
        }

        pub fn literal_bytes(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<std::borrow::Cow<'_, [u8]>> {
            match program.expression_table.expression(expression) {
                crate::source_state::expression::ExpressionNode::String(bytes) => {
                    Some(std::borrow::Cow::Borrowed(bytes))
                }
                crate::source_state::expression::ExpressionNode::ArrayLiteral(elements) => {
                    let bytes = program
                        .expression_table
                        .expression_handles(*elements)
                        .iter()
                        .map(|element| {
                            let crate::source_state::expression::ExpressionNode::Integer(integer) =
                                program.expression_table.expression(*element)
                            else {
                                return None;
                            };
                            u8::try_from(integer.value_u64()?).ok()
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some(std::borrow::Cow::Owned(bytes))
                }
                _ => None,
            }
        }
    }
    pub mod mathematical {
        use crate::source_state::data::TypeParameter;
        use crate::source_state::expression::ExpressionHandle;
        use crate::source_state::name::Identifier;
        use crate::source_state::types::TypeReferenceHandle;
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct MathematicalDefinition {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub is_public: bool,
            pub binders: HandleSpan<TypeParameter>,
            pub parameters: HandleSpan<MathematicalParameter>,
            pub result: MathematicalTypeHandle,
            pub body: MathematicalBody,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct MathematicalParameter {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub relevance: language_core::BindingRelevance,
            pub ty: MathematicalTypeHandle,
        }

        pub type MathematicalTypeHandle = Handle<MathematicalType>;

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub enum MathematicalType {
            Ordinary(TypeReferenceHandle),
            Arrow {
                binder: Option<Identifier>,
                domain: MathematicalTypeHandle,
                codomain: MathematicalTypeHandle,
            },
            Application {
                callee: MathematicalTypeHandle,
                arguments: HandleSpan<ExpressionHandle>,
            },
        }

        impl Default for MathematicalType {
            fn default() -> Self {
                Self::Ordinary(TypeReferenceHandle::invalid())
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub enum MathematicalBody {
            #[default]
            Assumption,
            Definition(ExpressionHandle),
            DependentArrow {
                binders: HandleSpan<MathematicalParameter>,
                body: ExpressionHandle,
            },
        }
    }
    pub mod proposition {
        use crate::source_state::data::DataProperties;
        use crate::source_state::expression::ExpressionHandle;
        use crate::source_state::name::Identifier;
        use crate::source_state::type_identity::TypeIdentityRequest;
        use crate::source_state::types::TypeReferenceHandle;
        use crate::source_symbols::JoinedPath;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        pub(crate) mod constructor_values {
            use crate::source_state::data::DataMember;
            use crate::source_state::expression::{ExpressionHandle, TableStructLiteralField};
            use symbols::SymbolHandle;

            pub(crate) fn render(
                program: &crate::source_state::TypedTrees,
                owner: SymbolHandle,
                case: Option<SymbolHandle>,
                fields: &[TableStructLiteralField],
                render_value: impl Fn(ExpressionHandle) -> String,
            ) -> Option<String> {
                let mut owners = program
                    .data_definitions()
                    .iter()
                    .filter(|data| data.symbol == owner);
                let data = owners.next()?;
                if owners.next().is_some() {
                    return None;
                }
                let members = program.data_members(data);
                let selected_case = if let Some(case) = case {
                    let mut cases = members.iter().filter_map(|member| match member {
                        DataMember::Variant(variant) if variant.symbol == case => Some(variant),
                        _ => None,
                    });
                    let selected = cases.next()?;
                    if cases.next().is_some() {
                        return None;
                    }
                    Some(selected)
                } else {
                    None
                };
                let owner = program.normalized_hermetic_symbol_identity(owner).ok()?;
                let case = selected_case.map_or_else(
                    || "record".to_owned(),
                    |case| member_identity(case.identity, case.name.as_str()),
                );
                let mut values = Vec::with_capacity(fields.len());
                for field in fields {
                    let common = members.iter().filter_map(|member| match member {
                        DataMember::Field(declared) if declared.symbol == field.field_symbol => {
                            Some(("common", declared))
                        }
                        _ => None,
                    });
                    let payload = selected_case
                        .into_iter()
                        .flat_map(|case| program.data_payload_fields(case))
                        .filter(|declared| declared.symbol == field.field_symbol)
                        .map(|declared| ("payload", declared));
                    let mut declarations = common.chain(payload);
                    let (scope, declared) = declarations.next()?;
                    if declarations.next().is_some() {
                        return None;
                    }
                    let identity = format!(
                        "{scope}:{}",
                        member_identity(declared.identity, declared.name.as_str())
                    );
                    values.push((identity, render_value(field.value)));
                }
                values.sort_by(|left, right| left.0.cmp(&right.0));
                if values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                    return None;
                }
                let mut label = "constructor:".to_owned();
                append(&mut label, &owner);
                append(&mut label, &case);
                for (field, value) in values {
                    append(&mut label, &field);
                    append(&mut label, &value);
                }
                Some(label)
            }

            fn member_identity(identity: Option<u64>, name: &str) -> String {
                identity.map_or_else(
                    || format!("name:{name}"),
                    |identity| format!("schema:{identity}"),
                )
            }

            fn append(label: &mut String, component: &str) {
                use std::fmt::Write;
                let _ = write!(label, "{}:{component}", component.len());
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct PropositionDefinition {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub is_public: bool,
            pub binders: HandleSpan<PropositionBinder>,
            pub parameters: HandleSpan<crate::source_state::signature::StateParameter>,
            pub transparent_formula_source_span: Option<source_text::SourceSpan>,
            pub body: PropositionBody,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct PropositionBinder {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub kind: PropositionBinderKind,
            pub bounds: DataProperties,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub enum PropositionBinderKind {
            #[default]
            Type,
            Const {
                type_reference: TypeReferenceHandle,
            },
            Machine,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub enum PropositionBody {
            #[default]
            Primitive,
            Witness {
                evidence: TypeReferenceHandle,
            },
            Transparent {
                proposition: PropositionFormula,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub enum PropositionFormula {
            Application(PropositionApplication),
            BooleanExpression(ExpressionHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct PropositionApplication {
            pub proposition: SymbolHandle,
            pub name: Identifier,
            pub binder_arguments: Box<[PropositionBinderArgument]>,
            pub arguments: HandleSpan<ExpressionHandle>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
        pub struct PropositionBinderArgument {
            pub kind: PropositionBinderArgumentKind,
            pub path: Box<[Identifier]>,
            pub const_literal: Option<numeric_literals::IntegerLiteral>,
            pub evidence_projection: Option<crate::source_state::expression::EvidenceProjection>,
            pub symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, record_storage::Record)]
        pub enum PropositionBinderArgumentKind {
            Type,
            Const,
            Machine,
        }

        impl PropositionBinderArgument {
            pub fn display_name(&self) -> String {
                display_binder_argument(self)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum NormalizedPropositionFormula {
            Proposition {
                label: String,
                classification: PropositionEvidenceClassification,
            },
            Boolean {
                label: String,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum PropositionEvidenceClassification {
            FactOnly,
            Witness {
                evidence: String,
                interface: Option<NormalizedEvidenceInterfaceIdentity>,
            },
        }

        fn normalize_evidence_interface(
            program: &crate::source_state::TypedTrees,
            evidence: TypeReferenceHandle,
            binders: arena::RootRows<'_, PropositionBinder>,
            binder_labels: &[String],
            binder_identities: &[Option<String>],
        ) -> Option<(String, Option<NormalizedEvidenceInterfaceIdentity>)> {
            let display_substitutions = binders
                .iter()
                .zip(binder_labels)
                .map(|(binder, replacement)| (binder.symbol, replacement.clone()))
                .collect::<Vec<_>>();
            let label = render_evidence_type(program, evidence, &display_substitutions);
            let (trait_symbol, arguments) =
                match program.type_reference_table.type_reference(evidence) {
                    crate::source_state::types::TypeReferenceNode::Named { symbol, .. } => {
                        (*symbol, &[][..])
                    }
                    crate::source_state::types::TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } => (*base_symbol, program.type_reference_handles(*arguments)),
                    _ => return Some((label, None)),
                };
            if !trait_symbol.is_valid()
                || program.symbols.get(trait_symbol).kind != symbols::SymbolKind::Trait
            {
                return Some((label, None));
            }
            let unresolved_binders = binders
                .iter()
                .zip(binder_identities)
                .filter_map(|(binder, identity)| identity.is_none().then_some(binder.symbol))
                .collect::<Vec<_>>();
            if arguments.iter().any(|argument| {
                type_reference_mentions_any(program, *argument, &unresolved_binders)
            }) {
                return Some((label, None));
            }
            let exact_substitutions = binders
                .iter()
                .zip(binder_identities)
                .filter_map(|(binder, replacement)| {
                    replacement.clone().map(|value| (binder.symbol, value))
                })
                .collect::<Vec<_>>();
            let arguments = arguments
                .iter()
                .map(|argument| {
                    program.type_identity(TypeIdentityRequest {
                        binders: &exact_substitutions,
                        ..TypeIdentityRequest::ordinary(*argument)
                    })
                })
                .collect::<Vec<_>>();
            let root_arguments = arguments
                .iter()
                .map(|argument| program.type_identity_text(*argument).unwrap_or_default())
                .collect::<Vec<_>>();
            Some((
                label,
                Some(NormalizedEvidenceInterfaceIdentity {
                    trait_symbol,
                    arguments,
                    requirements: normalized_evidence_requirements(
                        program,
                        trait_symbol,
                        &root_arguments,
                    )
                    .ok()?,
                }),
            ))
        }

        pub(crate) fn normalized_evidence_requirements(
            program: &crate::source_state::TypedTrees,
            root: SymbolHandle,
            root_arguments: &[String],
        ) -> Result<
            Vec<NormalizedEvidenceRequirementIdentity>,
            crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError,
        > {
            fn collect(
                program: &crate::source_state::TypedTrees,
                trait_symbol: SymbolHandle,
                trait_arguments: &[String],
                visited: &mut Vec<(SymbolHandle, Vec<String>)>,
                requirements: &mut Vec<NormalizedEvidenceRequirementIdentity>,
            ) -> Result<
                (),
                crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError,
            > {
                let visit = (trait_symbol, trait_arguments.to_vec());
                if visited.contains(&visit) {
                    return Ok(());
                }
                visited.push(visit);
                let definition = program.trait_definition_by_symbol(trait_symbol)
                    .ok_or(crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError::UnresolvedContractFact)?;
                if program.trait_type_parameters(definition).len() != trait_arguments.len() {
                    return Err(crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError::UnresolvedContractFact);
                }
                for requirement in program.trait_machine_signatures(definition) {
                    if !requirements.iter().any(|candidate| {
                        candidate.declaring_trait == definition.symbol
                            && candidate.declaring_trait_arguments == trait_arguments
                            && candidate.requirement == requirement.symbol
                    }) {
                        requirements.push(NormalizedEvidenceRequirementIdentity {
                            declaring_trait: definition.symbol,
                            declaring_trait_arguments: trait_arguments.to_vec(),
                            requirement: requirement.symbol,
                        });
                    }
                }
                let substitutions = program
                    .trait_type_parameters(definition)
                    .iter()
                    .zip(trait_arguments)
                    .map(|(parameter, argument)| (parameter.symbol, argument.clone()))
                    .collect::<Vec<_>>();
                for parent in program.trait_requirements(definition) {
                    let parent_arguments = program
                        .type_reference_handles(parent.arguments)
                        .iter()
                        .map(|argument| {
                            program.runtime_callable_type_identity(TypeIdentityRequest {
                                binders: &substitutions,
                                ..TypeIdentityRequest::ordinary(*argument)
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    collect(
                        program,
                        parent.symbol,
                        &parent_arguments,
                        visited,
                        requirements,
                    )?;
                }
                Ok(())
            }

            let mut requirements = Vec::new();
            collect(
                program,
                root,
                root_arguments,
                &mut Vec::new(),
                &mut requirements,
            )?;
            requirements.sort_by_key(|requirement| {
                (
                    requirement.declaring_trait.arena_index(),
                    requirement.requirement.arena_index(),
                    requirement.declaring_trait_arguments.clone(),
                )
            });
            Ok(requirements)
        }

        impl crate::source_state::TypedTrees {
            pub fn type_reference_mentions_any(
                &self,
                reference: TypeReferenceHandle,
                symbols: &[SymbolHandle],
            ) -> bool {
                type_reference_mentions_any(self, reference, symbols)
            }
        }

        fn type_reference_mentions_any(
            program: &crate::source_state::TypedTrees,
            type_reference: TypeReferenceHandle,
            symbols: &[SymbolHandle],
        ) -> bool {
            let mut references = FreeInputMentions::new(program, symbols);
            crate::source_state::references::visit_type_reference(
                program,
                &mut references,
                type_reference,
            );
            references.mentioned
        }

        struct FreeInputMentions<'program, 'query> {
            program: &'program crate::source_state::TypedTrees,
            symbols: &'query [SymbolHandle],
            mentioned: bool,
            scopes: Vec<Vec<SymbolHandle>>,
            pending_binders: Vec<SymbolHandle>,
            types: std::collections::HashSet<(TypeReferenceHandle, Vec<SymbolHandle>)>,
            expressions: std::collections::HashSet<(ExpressionHandle, Vec<SymbolHandle>)>,
        }

        impl<'program, 'query> FreeInputMentions<'program, 'query> {
            fn new(
                program: &'program crate::source_state::TypedTrees,
                symbols: &'query [SymbolHandle],
            ) -> Self {
                Self {
                    program,
                    symbols,
                    mentioned: false,
                    scopes: Vec::new(),
                    pending_binders: Vec::new(),
                    types: std::collections::HashSet::new(),
                    expressions: std::collections::HashSet::new(),
                }
            }
            fn context(&self) -> Vec<SymbolHandle> {
                let mut bound = self.scopes.iter().flatten().copied().collect::<Vec<_>>();
                bound.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                bound.dedup();
                bound
            }
        }

        impl Extend<SymbolHandle> for FreeInputMentions<'_, '_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
                self.mentioned |= symbols.into_iter().any(|symbol| {
                    self.symbols.contains(&symbol)
                        && !self.scopes.iter().flatten().any(|bound| *bound == symbol)
                });
            }
        }

        impl crate::source_state::references::TypedReferences for FreeInputMentions<'_, '_> {
            fn callable_signature(
                &mut self,
                signature: &crate::source_state::signature::StateSignature,
            ) {
                self.pending_binders = self
                    .program
                    .state_signature_type_parameters(signature)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(
                        self.program
                            .state_signature_parameters(signature)
                            .iter()
                            .map(|parameter| parameter.symbol),
                    )
                    .collect();
                if signature.symbol.is_valid() {
                    self.pending_binders.push(signature.symbol);
                    if let Some(children) = self.program.symbols.child_handles(signature.symbol) {
                        self.pending_binders.extend(children);
                    }
                }
            }
            fn enter_lexical_scope(&mut self, _: SymbolHandle) {
                self.scopes.push(std::mem::take(&mut self.pending_binders));
            }
            fn leave_lexical_scope(&mut self) {
                self.scopes.pop();
            }
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                !self.mentioned && self.types.insert((reference, self.context()))
            }
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                !self.mentioned && self.expressions.insert((expression, self.context()))
            }
        }

        fn exact_binder_argument_identity(
            program: &crate::source_state::TypedTrees,
            argument: &PropositionBinderArgument,
        ) -> Option<String> {
            if argument.const_literal.is_some() || !argument.symbol.is_valid() {
                return None;
            }
            if matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::TypeParameter
                    | symbols::SymbolKind::MachineParameter
                    | symbols::SymbolKind::PropositionParameter
                    | symbols::SymbolKind::PropositionMachineParameter
            ) {
                return None;
            }
            let identity = program.symbols.display_path(argument.symbol, "::");
            (!identity.is_empty()).then_some(identity)
        }

        fn binder_argument_identities_for_labels(
            program: &crate::source_state::TypedTrees,
            application: &PropositionApplication,
            binder_labels: &[String],
        ) -> Option<Vec<Option<String>>> {
            if application.binder_arguments.len() == binder_labels.len() {
                return Some(
                    application
                        .binder_arguments
                        .iter()
                        .map(|argument| exact_binder_argument_identity(program, argument))
                        .collect(),
                );
            }
            application
                .binder_arguments
                .is_empty()
                .then(|| vec![None; binder_labels.len()])
        }

        fn render_evidence_type(
            program: &crate::source_state::TypedTrees,
            type_reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, String)],
        ) -> String {
            use crate::source_state::types::TypeReferenceNode;

            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference {
                    referee, access, ..
                } => format!(
                    "{}{}",
                    access.borrow_prefix(),
                    render_evidence_type(program, *referee, substitutions)
                ),
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => format!(
                    "{}[{}]",
                    render_evidence_type(program, *base_type, substitutions),
                    match constraints.count() {
                        1 => "1 constraint".to_owned(),
                        count => format!("{count} constraints"),
                    }
                ),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => format!(
                    "[{}; {length}]",
                    render_evidence_type(program, *element_type, substitutions)
                ),
                TypeReferenceNode::Slice { element_type } => format!(
                    "[{}]",
                    render_evidence_type(program, *element_type, substitutions)
                ),
                TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                    ..
                } => format!(
                    "{base_name}<{}>",
                    program
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|argument| render_evidence_type(program, *argument, substitutions))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                TypeReferenceNode::Named { symbol, name } => substitutions
                    .iter()
                    .find(|(candidate, _)| candidate == symbol)
                    .map(|(_, replacement)| replacement.clone())
                    .unwrap_or_else(|| name.to_string()),
                TypeReferenceNode::Callable { .. } => {
                    program.type_identity_request_text(TypeIdentityRequest {
                        binders: substitutions,
                        ..TypeIdentityRequest::ordinary(type_reference)
                    })
                }
                TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {
                    program.display_type_reference(type_reference)
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct NormalizedEvidenceInterfaceIdentity {
            pub trait_symbol: SymbolHandle,
            pub arguments: Vec<crate::source_state::type_identity::NormalizedTypeIdentity>,
            pub requirements: Vec<NormalizedEvidenceRequirementIdentity>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct NormalizedEvidenceRequirementIdentity {
            pub declaring_trait: SymbolHandle,
            pub declaring_trait_arguments: Vec<String>,
            pub requirement: SymbolHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct NormalizedPropositionApplicationIdentity {
            pub declaration: SymbolHandle,
            pub name: String,
            pub binder_arguments: Vec<NormalizedPropositionBinderArgument>,
            pub arguments: Vec<String>,
            pub classification: PropositionEvidenceClassification,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct NormalizedPropositionBinderArgument {
            pub kind: PropositionBinderArgumentKind,
            pub identity: String,
        }

        impl NormalizedPropositionFormula {
            pub fn identity_label(&self) -> String {
                match self {
                    Self::Proposition {
                        label,
                        classification: PropositionEvidenceClassification::FactOnly,
                    } => format!("proposition:fact:{label}"),
                    Self::Proposition {
                        label,
                        classification: PropositionEvidenceClassification::Witness { evidence, .. },
                    } => format!("proposition:witness:{evidence}:{label}"),
                    Self::Boolean { label } => format!("boolean:{label}"),
                }
            }
        }

        pub type ProofLabelResult<T> =
            Result<T, crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError>;

        #[derive(Clone, Copy)]
        pub enum ProofSubstitutions<'a> {
            None,
            BySymbol(&'a [(SymbolHandle, String)]),
            ByParameter(&'a [(SymbolHandle, String, String)]),
        }

        #[derive(Clone, Copy)]
        pub struct PropositionLabels<'a> {
            pub binder_labels: &'a [String],
            pub argument_labels: &'a [String],
        }

        #[derive(Clone, Debug, Default, PartialEq, Eq)]
        pub struct OwnedPropositionLabels {
            pub binder_labels: Vec<String>,
            pub argument_labels: Vec<String>,
        }

        impl OwnedPropositionLabels {
            pub fn borrow(&self) -> PropositionLabels<'_> {
                PropositionLabels {
                    binder_labels: &self.binder_labels,
                    argument_labels: &self.argument_labels,
                }
            }
        }

        impl crate::source_state::TypedTrees {
            pub fn try_normalize_nominal_proposition_application(
                &self,
                application: &PropositionApplication,
                labels: Option<PropositionLabels<'_>>,
            ) -> ProofLabelResult<Option<NormalizedPropositionApplicationIdentity>> {
                let rendered = self.proposition_labels(application)?;
                let labels = labels.unwrap_or_else(|| rendered.borrow());
                let (binder_labels, argument_labels) =
                    (labels.binder_labels, labels.argument_labels);
                let Some(binder_identities) =
                    binder_argument_identities_for_labels(self, application, binder_labels)
                else {
                    return Ok(None);
                };
                self.normalize_nominal_proposition_application_inner(
                    application,
                    binder_labels,
                    argument_labels,
                    &binder_identities,
                    &mut Vec::new(),
                )
            }

            fn normalize_nominal_proposition_application_inner(
                &self,
                application: &PropositionApplication,
                binder_labels: &[String],
                argument_labels: &[String],
                binder_identities: &[Option<String>],
                visiting: &mut Vec<SymbolHandle>,
            ) -> ProofLabelResult<Option<NormalizedPropositionApplicationIdentity>> {
                if visiting.contains(&application.proposition) {
                    return Ok(None);
                }
                let Some(declaration) =
                    self.proposition_definition_by_symbol(application.proposition)
                else {
                    return Ok(None);
                };
                let binders = self.proposition_binders(declaration);
                let parameters = self.proposition_parameters(declaration);
                if binders.len() != binder_labels.len()
                    || binders.len() != binder_identities.len()
                    || parameters.len() != argument_labels.len()
                {
                    return Ok(None);
                }
                let substitutions = binders
                    .iter()
                    .zip(binder_labels)
                    .map(|(binder, label)| (binder.symbol, label.clone()))
                    .chain(
                        parameters
                            .iter()
                            .zip(argument_labels)
                            .map(|(parameter, label)| (parameter.symbol, label.clone())),
                    )
                    .collect::<Vec<_>>();
                let endpoint = |classification| NormalizedPropositionApplicationIdentity {
                    declaration: declaration.symbol,
                    name: declaration.name.as_str().to_owned(),
                    binder_arguments: binders
                        .iter()
                        .zip(binder_labels)
                        .map(|(binder, identity)| NormalizedPropositionBinderArgument {
                            kind: match binder.kind {
                                PropositionBinderKind::Type => PropositionBinderArgumentKind::Type,
                                PropositionBinderKind::Const { .. } => {
                                    PropositionBinderArgumentKind::Const
                                }
                                PropositionBinderKind::Machine => {
                                    PropositionBinderArgumentKind::Machine
                                }
                            },
                            identity: identity.clone(),
                        })
                        .collect(),
                    arguments: argument_labels.to_vec(),
                    classification,
                };
                match &declaration.body {
                    PropositionBody::Primitive => {
                        Ok(Some(endpoint(PropositionEvidenceClassification::FactOnly)))
                    }
                    PropositionBody::Witness { evidence } => {
                        let Some((evidence, interface)) = normalize_evidence_interface(
                            self,
                            *evidence,
                            binders,
                            binder_labels,
                            binder_identities,
                        ) else {
                            return Ok(None);
                        };
                        Ok(Some(endpoint(PropositionEvidenceClassification::Witness {
                            evidence,
                            interface,
                        })))
                    }
                    PropositionBody::Transparent {
                        proposition: PropositionFormula::BooleanExpression(_),
                    } => Ok(None),
                    PropositionBody::Transparent {
                        proposition: PropositionFormula::Application(expansion),
                    } => {
                        visiting.push(application.proposition);
                        let normalized = (|| {
                            let (expanded_binders, expanded_binder_identities, expanded_arguments) =
                                expanded_application_parts(
                                    self,
                                    expansion,
                                    &substitutions,
                                    binders,
                                    binder_identities,
                                )?;
                            self.normalize_nominal_proposition_application_inner(
                                expansion,
                                &expanded_binders,
                                &expanded_arguments,
                                &expanded_binder_identities,
                                visiting,
                            )
                        })();
                        visiting.pop();
                        normalized
                    }
                }
            }

            pub fn try_render_proof_expression(
                &self,
                expression: ExpressionHandle,
                substitutions: ProofSubstitutions<'_>,
            ) -> ProofLabelResult<String> {
                match substitutions {
                    ProofSubstitutions::None => render_expression(self, expression, &[], &[]),
                    ProofSubstitutions::BySymbol(substitutions) => {
                        render_expression(self, expression, substitutions, &[])
                    }
                    ProofSubstitutions::ByParameter(substitutions) => {
                        let symbol_substitutions = substitutions
                            .iter()
                            .map(|(symbol, _, replacement)| (*symbol, replacement.clone()))
                            .collect::<Vec<_>>();
                        let name_substitutions = substitutions
                            .iter()
                            .map(|(_, name, replacement)| (name.clone(), replacement.clone()))
                            .collect::<Vec<_>>();
                        render_expression(
                            self,
                            expression,
                            &symbol_substitutions,
                            &name_substitutions,
                        )
                    }
                }
            }

            pub fn proposition_labels(
                &self,
                application: &PropositionApplication,
            ) -> ProofLabelResult<OwnedPropositionLabels> {
                Ok(OwnedPropositionLabels {
                    binder_labels: application
                        .binder_arguments
                        .iter()
                        .map(display_binder_argument)
                        .collect(),
                    argument_labels: self
                        .expression_table
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|argument| {
                            self.try_render_proof_expression(*argument, ProofSubstitutions::None)
                        })
                        .collect::<ProofLabelResult<Vec<_>>>()?,
                })
            }

            pub fn normalize_proposition_application(
                &self,
                application: &PropositionApplication,
                labels: Option<PropositionLabels<'_>>,
            ) -> Option<NormalizedPropositionFormula> {
                self.try_normalize_proposition_application(application, labels)
                    .ok()
                    .flatten()
            }

            pub fn try_normalize_proposition_application(
                &self,
                application: &PropositionApplication,
                labels: Option<PropositionLabels<'_>>,
            ) -> ProofLabelResult<Option<NormalizedPropositionFormula>> {
                let rendered = self.proposition_labels(application)?;
                let labels = labels.unwrap_or_else(|| rendered.borrow());
                let (binder_labels, argument_labels) =
                    (labels.binder_labels, labels.argument_labels);
                let Some(binder_identities) =
                    binder_argument_identities_for_labels(self, application, binder_labels)
                else {
                    return Ok(None);
                };
                let mut visiting = Vec::new();
                self.normalize_proposition_application_inner(
                    application,
                    binder_labels,
                    argument_labels,
                    &binder_identities,
                    &mut visiting,
                )
            }

            fn normalize_proposition_application_inner(
                &self,
                application: &PropositionApplication,
                binder_labels: &[String],
                argument_labels: &[String],
                binder_identities: &[Option<String>],
                visiting: &mut Vec<SymbolHandle>,
            ) -> ProofLabelResult<Option<NormalizedPropositionFormula>> {
                if visiting.contains(&application.proposition) {
                    return Ok(None);
                }
                let declaration = self.proposition_definition_by_symbol(application.proposition);
                if declaration.is_none()
                    && self.symbols.get(application.proposition).kind
                        == symbols::SymbolKind::PropositionParameter
                {
                    let Some(parameter_count) = self
                        .data_type_parameters
                        .iter()
                        .map(|(_, parameter)| parameter)
                        .find_map(|parameter| match &parameter.kind {
                            crate::source_state::data::TypeParameterKind::Proposition {
                                contract,
                            } if parameter.symbol == application.proposition => {
                                Some(contract.parameters.len())
                            }
                            _ => None,
                        })
                    else {
                        return Ok(None);
                    };
                    if !binder_labels.is_empty() || parameter_count != argument_labels.len() {
                        return Ok(None);
                    }
                    return Ok(Some(NormalizedPropositionFormula::Proposition {
                        label: canonical_application_label(
                            application.name.as_str(),
                            binder_labels,
                            argument_labels,
                        ),
                        classification: PropositionEvidenceClassification::FactOnly,
                    }));
                }
                let Some(declaration) = declaration else {
                    return Ok(None);
                };
                let binders = self.proposition_binders(declaration);
                let parameters = self.proposition_parameters(declaration);
                if binders.len() != binder_labels.len()
                    || binders.len() != binder_identities.len()
                    || parameters.len() != argument_labels.len()
                {
                    return Ok(None);
                }

                let mut substitutions = Vec::new();
                for (binder, label) in binders.iter().zip(binder_labels) {
                    substitutions.push((binder.symbol, label.clone()));
                }
                for (parameter, label) in parameters.iter().zip(argument_labels) {
                    substitutions.push((parameter.symbol, label.clone()));
                }

                match &declaration.body {
                    PropositionBody::Primitive => {
                        Ok(Some(NormalizedPropositionFormula::Proposition {
                            label: canonical_application_label(
                                declaration.name.as_str(),
                                binder_labels,
                                argument_labels,
                            ),
                            classification: PropositionEvidenceClassification::FactOnly,
                        }))
                    }
                    PropositionBody::Witness { evidence } => {
                        let Some((evidence, interface)) = normalize_evidence_interface(
                            self,
                            *evidence,
                            binders,
                            binder_labels,
                            binder_identities,
                        ) else {
                            return Ok(None);
                        };
                        Ok(Some(NormalizedPropositionFormula::Proposition {
                            label: canonical_application_label(
                                declaration.name.as_str(),
                                binder_labels,
                                argument_labels,
                            ),
                            classification: PropositionEvidenceClassification::Witness {
                                evidence,
                                interface,
                            },
                        }))
                    }
                    PropositionBody::Transparent { proposition } => {
                        visiting.push(application.proposition);
                        let normalized = (|| match proposition {
                            PropositionFormula::BooleanExpression(expression) => {
                                Ok(Some(NormalizedPropositionFormula::Boolean {
                                    label: render_expression(
                                        self,
                                        *expression,
                                        &substitutions,
                                        &[],
                                    )?,
                                }))
                            }
                            PropositionFormula::Application(expansion) => {
                                let (
                                    expanded_binders,
                                    expanded_binder_identities,
                                    expanded_arguments,
                                ) = expanded_application_parts(
                                    self,
                                    expansion,
                                    &substitutions,
                                    binders,
                                    binder_identities,
                                )?;
                                self.normalize_proposition_application_inner(
                                    expansion,
                                    &expanded_binders,
                                    &expanded_arguments,
                                    &expanded_binder_identities,
                                    visiting,
                                )
                            }
                        })();
                        visiting.pop();
                        normalized
                    }
                }
            }
        }

        fn expanded_application_parts(
            program: &crate::source_state::TypedTrees,
            expansion: &PropositionApplication,
            substitutions: &[(SymbolHandle, String)],
            binders: arena::RootRows<'_, PropositionBinder>,
            binder_identities: &[Option<String>],
        ) -> ProofLabelResult<(Vec<String>, Vec<Option<String>>, Vec<String>)> {
            let expanded_binders = expansion
                .binder_arguments
                .iter()
                .map(|argument| {
                    substitutions
                        .iter()
                        .find(|(symbol, _)| *symbol == argument.symbol)
                        .map(|(_, label)| label.clone())
                        .unwrap_or_else(|| display_binder_argument(argument))
                })
                .collect::<Vec<_>>();
            let expanded_binder_identities = expansion
                .binder_arguments
                .iter()
                .map(|argument| {
                    binders
                        .iter()
                        .zip(binder_identities)
                        .find(|(binder, _)| binder.symbol == argument.symbol)
                        .and_then(|(_, identity)| identity.clone())
                        .or_else(|| exact_binder_argument_identity(program, argument))
                })
                .collect::<Vec<_>>();
            let expanded_arguments = program
                .expression_table
                .expression_handles(expansion.arguments)
                .iter()
                .map(|argument| render_expression(program, *argument, substitutions, &[]))
                .collect::<ProofLabelResult<Vec<_>>>()?;
            Ok((
                expanded_binders,
                expanded_binder_identities,
                expanded_arguments,
            ))
        }

        fn canonical_application_label(
            name: &str,
            binders: &[String],
            arguments: &[String],
        ) -> String {
            let binder_suffix = if binders.is_empty() {
                String::new()
            } else {
                format!("<{}>", binders.join(","))
            };
            format!("{name}{binder_suffix}({})", arguments.join(","))
        }

        fn display_binder_argument(argument: &PropositionBinderArgument) -> String {
            if let Some(literal) = &argument.const_literal {
                return literal.to_string();
            }
            if let Some(projection) = &argument.evidence_projection {
                return format!("{}.{}", projection.term, projection.member);
            }
            argument.path.joined_path()
        }

        // Display helpers accept total callbacks. Discard their complete output
        // when a child cannot be normalized, preserving the original failure.
        fn with_fallible_children<T>(
            render: impl Fn(ExpressionHandle) -> ProofLabelResult<String>,
            display: impl FnOnce(&dyn Fn(ExpressionHandle) -> String) -> T,
        ) -> ProofLabelResult<T> {
            let failure = std::cell::Cell::new(None);
            let result = display(&|expression| match render(expression) {
                Ok(label) => label,
                Err(error) => {
                    if failure.get().is_none() {
                        failure.set(Some(error));
                    }
                    String::new()
                }
            });
            match failure.into_inner() {
                Some(error) => Err(error),
                None => Ok(result),
            }
        }

        fn render_expression(
            program: &crate::source_state::TypedTrees,
            expression: ExpressionHandle,
            substitutions: &[(SymbolHandle, String)],
            name_substitutions: &[(String, String)],
        ) -> ProofLabelResult<String> {
            use crate::source_state::expression::ExpressionNode;
            if !program.expression_table.expression_is_valid(expression) {
                return Err(crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError::UnresolvedContractFact);
            }
            let render = |expression| {
                render_expression(program, expression, substitutions, name_substitutions)
            };
            Ok(match program.expression_table.expression(expression) {
                // A contract fact holds no block value.
                ExpressionNode::Block(_) => {
                    return Err(crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError::UnresolvedContractFact);
                }
                ExpressionNode::MachineValue(value) => {
                    return program.runtime_machine_value_identity(value, substitutions);
                }
                ExpressionNode::Match(dispatch) => with_fallible_children(render, |render| {
                    let arms = program
                        .expression_table
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| {
                            let pattern = crate::source_state::expression::display_match_pattern(
                                &program.expression_table,
                                arm.pattern,
                                render,
                            );
                            format!("{pattern} => {}", render(arm.value))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("match {} {{ {arms} }}", render(dispatch.subject))
                })?,
                ExpressionNode::SavedSubject(saved) => format!(
                    "<saved subject #{}:{}>",
                    saved.capture.arena_index(),
                    saved.capture.generation()
                ),
                ExpressionNode::Atomic(atomic) => {
                    let inputs = atomic.inputs().map(render).collect::<ProofLabelResult<Vec<_>>>()?;
                    format!("atomic[{:?}]({})", atomic.ordering, inputs.join(", "))
                }
                ExpressionNode::ArrayLiteral(values) => format!(
                    "[{}]",
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .map(|value| render(*value))
                        .collect::<ProofLabelResult<Vec<_>>>()?
                        .join(", ")
                ),
                ExpressionNode::Binary(binary) => format!(
                    "{} {} {}",
                    render(binary.left)?,
                    binary.operator.display_name(),
                    render(binary.right)?
                ),
                ExpressionNode::Boolean(value) => value.to_string(),
                ExpressionNode::Cast(cast) => format!(
                    "({} as {})",
                    render(cast.value)?,
                    crate::source_state::expression::display_name_path(
                        program
                            .expression_table
                            .name_path_members(cast.target_label),
                        "::",
                    )
                ),
                ExpressionNode::Call(call) => with_fallible_children(render, |render| {
                    crate::source_state::expression::display_call_label(&program.expression_table, call, render)
                })?,
                ExpressionNode::Float(value) => value.to_string(),
                ExpressionNode::UndeterminedNaN(format) => {
                    format!("<undetermined NaN {}>", format.name())
                }
                ExpressionNode::Indexed(indexed) => {
                    format!(
                        "{}[{}]",
                        render(indexed.collection)?,
                        render(indexed.index)?
                    )
                }
                ExpressionNode::Integer(value) => value.to_string(),
                ExpressionNode::Member(member) => {
                    format!("{}.{}", render(member.receiver)?, member.member)
                }
                ExpressionNode::Borrow(inner) => format!(
                    "{}{}",
                    inner.access.borrow_prefix(),
                    render(inner.target)?
                ),
                ExpressionNode::Name(path) => {
                    if let Some((_, replacement)) = substitutions
                        .iter()
                        .find(|(symbol, _)| *symbol == path.symbol || *symbol == path.head_symbol)
                    {
                        return Ok(replacement.clone());
                    }
                    let members = program.expression_table.name_path_members(path.members);
                    if let Some(first) = members.first()
                        && let Some((_, replacement)) = name_substitutions
                            .iter()
                            .find(|(name, _)| name == first.as_str())
                    {
                        return Ok(replacement.clone());
                    }
                    let selected = program
                        .expression_table
                        .name_path_member_symbols(path.member_symbols);
                    if selected.len() == members.len()
                        && selected.len() >= 2
                        && selected.first() == Some(&path.head_symbol)
                        && selected.last() == Some(&path.symbol)
                        && selected.iter().all(|symbol| symbol.is_valid())
                        && selected.windows(2).all(|pair| {
                            program
                                .symbols
                                .child_handles(pair[0])
                                .is_some_and(|mut children| children.any(|child| child == pair[1]))
                        })
                        && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant
                    {
                        let label = with_fallible_children(render, |render| {
                            constructor_values::render(
                                program,
                                program.symbols.get(path.symbol).parent,
                                Some(path.symbol),
                                &[],
                                render,
                            )
                        })?;
                        return label.ok_or(crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError::UnresolvedContractFact);
                    }
                    crate::source_state::expression::display_name_path(members, "::")
                }
                ExpressionNode::Range(range) => {
                    match (range.start.is_valid(), range.end.is_valid()) {
                        (true, true) => format!("{}..{}", render(range.start)?, render(range.end)?),
                        (true, false) => format!("{}..", render(range.start)?),
                        (false, true) => format!("..{}", render(range.end)?),
                        (false, false) => "..".to_owned(),
                    }
                }
                ExpressionNode::String(value) => format!("{value:?}"),
                ExpressionNode::StructLiteral(literal) => {
                    with_fallible_children(render, |render| {
                        constructor_values::render(
                            program,
                            literal.type_symbol,
                            literal.case_symbol,
                            program.expression_table.struct_fields(literal.fields),
                            render,
                        )
                    })?
                    .ok_or(crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError::UnresolvedContractFact)?
                }
                ExpressionNode::TypeExpression(reference) => {
                    program.display_type_reference(*reference)
                }
                ExpressionNode::ZeroValue(_) => program.expression_table.display_name(expression),
                ExpressionNode::Unary(unary) => {
                    format!(
                        "{}{}",
                        unary.operator.display_name(),
                        render(unary.operand)?
                    )
                }
            })
        }
        #[cfg(test)]
        mod proof_label_tests {
            use super::*;
            use crate::source_state::callable_values::RuntimeCallBinding;
            use crate::source_state::expression::{ExpressionNode, TableCallExpression};
            use crate::source_state::runtime_callable_identity::RuntimeCallableIdentityError;
            use crate::source_state::type_identity::package_policy_reading_tests::fixture::Fixture;
            use crate::source_state::types::TypeReferenceNode;

            #[test]
            fn callable_labels_keep_selection_identity_and_refuse_unresolved_children() {
                let mut fixture = Fixture::new();
                let expression = fixture.machine_value();
                let ExpressionNode::MachineValue(value) = fixture
                    .program
                    .expression_table
                    .expression(expression)
                    .clone()
                else {
                    panic!("fixture must retain a machine selection");
                };
                let expected = fixture
                    .program
                    .runtime_machine_value_identity(&value, &[])
                    .unwrap();
                assert_eq!(
                    fixture
                        .program
                        .try_render_proof_expression(expression, ProofSubstitutions::None,),
                    Ok(expected)
                );
                let amount = fixture.integer(9);
                assert_eq!(
                    fixture
                        .program
                        .try_render_proof_expression(amount, ProofSubstitutions::None,),
                    Ok(fixture.program.expression_table.display_name(amount))
                );

                let TypeReferenceNode::Callable { contract } = *fixture
                    .program
                    .type_reference_table
                    .type_reference(value.type_reference)
                else {
                    panic!("fixture must retain its exact callable contract");
                };
                let mut unresolved = value;
                unresolved.selected.symbol = SymbolHandle::invalid();
                let unresolved = fixture.expression(ExpressionNode::MachineValue(unresolved));
                let call = fixture.expression(ExpressionNode::Call(TableCallExpression {
                    runtime_call: Some(RuntimeCallBinding {
                        callee: unresolved,
                        contract,
                    }),
                    ..Default::default()
                }));
                let error = RuntimeCallableIdentityError::UnresolvedSymbol;
                assert_eq!(
                    fixture
                        .program
                        .try_render_proof_expression(call, ProofSubstitutions::None,),
                    Err(error)
                );
                let arguments = fixture
                    .program
                    .expression_table
                    .insert_expression_handles([call]);
                let application = PropositionApplication {
                    proposition: SymbolHandle::invalid(),
                    name: Identifier::generated("invalid_selection"),
                    binder_arguments: Box::new([]),
                    arguments,
                };
                let supplied_arguments = ["caller supplied label".to_owned()];
                for labels in [
                    None,
                    Some(PropositionLabels {
                        binder_labels: &[],
                        argument_labels: &supplied_arguments,
                    }),
                ] {
                    assert_eq!(
                        fixture
                            .program
                            .try_normalize_proposition_application(&application, labels,),
                        Err(error)
                    );
                    assert_eq!(
                        fixture
                            .program
                            .try_normalize_nominal_proposition_application(&application, labels,),
                        Err(error)
                    );
                }
                assert!(
                    fixture
                        .program
                        .normalize_proposition_application(&application, None)
                        .is_none()
                );
            }
        }
    }

    pub mod ranking {
        use crate::source_state::TypedTrees;
        use crate::source_state::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use crate::source_state::machine::Machine;
        use crate::source_state::state::State;
        use crate::source_symbols::JoinedPath;

        #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
        pub struct RankingExpressionCustody {
            pub machine: symbols::SymbolHandle,
            pub subjects: Vec<ExpressionHandle>,
            pub view: source_declarations::RankingViewSelection,
            pub view_arguments: Vec<ExpressionHandle>,
            pub rank_range: Option<ExpressionHandle>,
        }

        pub fn resolve_machine_witness_subjects(
            program: &TypedTrees,
            machine: &Machine,
        ) -> Option<Vec<ExpressionHandle>> {
            let witness = machine.termination_plan.implementation_witness.as_ref()?;
            if let Some(custody) = program.ranking_expression_custody_for(machine.symbol) {
                return (custody.subjects.len() == witness.subjects.len())
                    .then(|| custody.subjects.clone());
            }
            resolve_machine_witness_expressions(program, machine, &witness.subjects)
        }

        pub fn resolve_machine_witness_expressions(
            program: &TypedTrees,
            machine: &Machine,
            rendered: &[String],
        ) -> Option<Vec<ExpressionHandle>> {
            let root_state = program.machine_states(machine).first()?;
            resolve_witness_expressions(program, root_state, rendered)
        }

        pub fn resolve_witness_expressions(
            program: &TypedTrees,
            state: &State,
            rendered: &[String],
        ) -> Option<Vec<ExpressionHandle>> {
            rendered
                .iter()
                .map(|expected| {
                    if expected == "value" {
                        return None;
                    }
                    program
                        .expression_table
                        .iter_expressions()
                        .filter(|(handle, _)| {
                            witness_expression_text(program, *handle) == *expected
                        })
                        .find_map(|(handle, _)| {
                            expression_belongs_to_state(program, state, handle).then_some(handle)
                        })
                        .or_else(|| {
                            program
                                .expression_table
                                .iter_expressions()
                                .find_map(|(handle, _)| {
                                    (witness_expression_text(program, handle) == *expected)
                                        .then_some(handle)
                                })
                        })
                })
                .collect()
        }

        pub fn witness_expression_text(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> String {
            match program.expression_table.expression(expression) {
                ExpressionNode::Integer(literal) => literal.to_string(),
                ExpressionNode::Name(path) => program
                    .expression_table
                    .name_path_members(path.members)
                    .joined_path_with("."),
                ExpressionNode::Member(member) => format!(
                    "{}.{}",
                    witness_expression_text(program, member.receiver),
                    member.member.as_str()
                ),
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::Subtract) =>
                {
                    format!(
                        "{} - {}",
                        witness_expression_text(program, binary.left),
                        witness_expression_text(program, binary.right)
                    )
                }
                _ => "value".to_string(),
            }
        }

        fn expression_belongs_to_state(
            program: &TypedTrees,
            state: &State,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(root) = ranked_expression_root(program, expression) else {
                return false;
            };
            program.state_parameter_by_symbol(state, root).is_some()
        }

        fn ranked_expression_root(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<symbols::SymbolHandle> {
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) => Some(path.symbol),
                ExpressionNode::Member(member) => ranked_expression_root(program, member.receiver),
                ExpressionNode::Cast(cast) => ranked_expression_root(program, cast.value),
                _ => None,
            }
        }
    }
}
