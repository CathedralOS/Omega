/*
We represent typed declaration records (`declarations`), including domains, data
definitions, constants, measures, wire schemas, layout storage, visibility,
numeric/float intrinsics, and boundary completions in `TypedTrees`.
*/

pub mod boundary_completions {
    use crate::TypedTrees;
    use crate::types::PrimitiveType;
    use symbols::SymbolHandle;

    pub fn boundary_requirement_completion(
        program: &TypedTrees,
        requirement: SymbolHandle,
    ) -> semantic_vocabulary::BoundaryCompletion {
        if let Some(family) = program
            .target_machine_families
            .iter()
            .find(|family| family.canonical == requirement)
        {
            let mut members = family.members.iter();
            if let Some(first) = members.next() {
                let completion = machine_completion(program, first.machine);
                if members.all(|member| machine_completion(program, member.machine) == completion) {
                    return completion;
                }
            }
            return semantic_vocabulary::BoundaryCompletion::NormalReturn;
        }
        machine_completion(program, requirement)
    }

    fn machine_completion(
        program: &TypedTrees,
        requirement: SymbolHandle,
    ) -> semantic_vocabulary::BoundaryCompletion {
        if let Some(machine) = program.machine_by_symbol(requirement)
            && !machine.body_is_present
        {
            let mut external = program
                .machine_trait_conformances(machine)
                .iter()
                .filter_map(|edge| {
                    let declaration =
                        crate::machine::resolve_satisfied_declaration(program, machine, edge)?;
                    let completion = declared_completion(program, declaration.symbol());
                    matches!(
                        completion,
                        semantic_vocabulary::BoundaryCompletion::External(_)
                    )
                    .then_some(completion)
                });
            if let Some(completion) = external.next()
                && external.all(|other| other == completion)
            {
                return completion;
            }
        }
        declared_completion(program, requirement)
    }

    fn declared_completion(
        program: &TypedTrees,
        requirement: SymbolHandle,
    ) -> semantic_vocabulary::BoundaryCompletion {
        use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind};
        let external = program
            .traits()
            .iter()
            .any(|definition| exact_process_exit_requirement(program, definition, requirement));
        if external {
            BoundaryCompletion::External(ExternalCompletionKind::ProcessExit)
        } else {
            BoundaryCompletion::NormalReturn
        }
    }

    fn exact_process_exit_requirement(
        program: &TypedTrees,
        definition: &crate::trait_definition::TraitDefinition,
        requirement: SymbolHandle,
    ) -> bool {
        use language_core::sealed_intrinsics::{HostedOperation, SealedIntrinsic};
        if !requirement.is_valid()
            || !definition.is_boundary
            || !definition.is_public
            || !definition.lifetime_parameters.is_empty()
            || !program.trait_type_parameters(definition).is_empty()
            || program
                .symbols
                .sealed_member_intrinsic(definition.symbol, "exit_process")
                != Some(SealedIntrinsic::HostedRequirement(
                    HostedOperation::ProcessExit,
                ))
        {
            return false;
        }
        let signatures = program.trait_machine_signatures(definition);
        if signatures.len() != 1 {
            return false;
        }
        signatures.iter().any(|signature| {
            signature.symbol == requirement
                && signature.name.as_str() == "exit_process"
                && signature.lifetime_parameters.is_empty()
                && program
                    .state_signature_type_parameters(signature)
                    .is_empty()
                && signature.native_callback_parameters.is_empty()
                && !signature.suspends
                && !signature.blocks
                && signature.declares_no_crash_route
                && matches!(&signature.termination_guarantee,
                    language_semantics::TerminationGuarantee::Terminates { premises }
                    if premises.is_empty())
                && exact_process_exit_signature(
                    program,
                    program.state_signature_parameters(signature),
                    signature.return_type,
                )
        })
    }

    /// The canonical requirement names the exact bundled ordinary result
    /// declaration. This recognizes its signature, not an emptiness proof or
    /// a grant of process-exit authority. The lowered checker owns those
    /// independent judgments.
    fn exact_process_exit_signature(
        program: &TypedTrees,
        parameters: &[crate::signature::StateParameter],
        return_type: crate::types::TypeReferenceHandle,
    ) -> bool {
        exact_process_exit_inputs(program, parameters)
            && program
                .type_reference_table
                .contains_type_reference(return_type)
            && program
                .type_reference_table
                .unconstrained_named_symbol(return_type)
                .is_some_and(|symbol| {
                    // `type_symbol` also peels references and arrays. A pointer to
                    // Never, or an empty array of it, is not this nominal result.
                    program.data_definition_by_symbol(symbol).is_some()
                        && program.core_vocabulary(symbol)
                            == Some(language_core::core_vocabulary::CoreVocabulary::Never)
                })
    }

    fn exact_process_exit_inputs(
        program: &TypedTrees,
        parameters: &[crate::signature::StateParameter],
    ) -> bool {
        let [parameter] = parameters else {
            return false;
        };
        !parameter.relevance.is_erased()
            && !parameter.is_const
            && !parameter.is_mutable
            && program.primitive_type_reference(parameter.type_reference)
                == Some(PrimitiveType::Integer(
                    numerics::integer_carrier::FixedInteger::signed(32),
                ))
    }

    #[cfg(test)]
    mod process_exit_signature_tests {
        use super::*;
        use crate::data::DataDefinition;
        use crate::signature::StateParameter;
        use crate::types::TypeReferenceNode;
        use crate::types::{FixedArrayLength, TypeReferenceHandle};
        use source::{SourceMap, SourceOrigin, SourceSpan, Span};
        use std::{path::PathBuf, sync::Arc};
        use symbols::{
            BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols,
        };

        const NEVER_SOURCE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../source/library/core/never.omg"
        ));

        fn fixture(
            origin: SourceOrigin,
            text: &str,
        ) -> (TypedTrees, StateParameter, TypeReferenceHandle) {
            let mut sources = SourceMap::default();
            let source = sources
                .add_with_metadata(
                    PathBuf::from("core/never.omg"),
                    text.into(),
                    PathBuf::from("core"),
                    None,
                    origin,
                )
                .source_id;
            let start = text.find("Never").unwrap();
            let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let children = builder.insert_children(
                root,
                builtin_type_symbols().into_iter().chain([(
                    SymbolKind::Data,
                    SymbolNameRef::Source(SourceSpan::new(
                        source,
                        Span::new(start, start + "Never".len()),
                    )),
                )]),
            );
            let never = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
            let integer = children
                .handles()
                .nth(
                    BuiltinTypeAtom::Integer(numerics::integer_carrier::FixedInteger::signed(32))
                        .ordinal(),
                )
                .unwrap();
            let mut typed = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            typed.push_data_definition(DataDefinition {
                symbol: never,
                name: "Never".into(),
                ..Default::default()
            });
            let argument = typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: integer,
                name: "i32".into(),
            });
            let result = typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: never,
                name: "Never".into(),
            });
            (
                typed,
                StateParameter {
                    type_reference: argument,
                    ..Default::default()
                },
                result,
            )
        }

        #[test]
        fn canonical_result_recognizes_bundled_identity_without_granting_legacy_shapes() {
            let (mut typed, parameter, never) = fixture(SourceOrigin::Toolchain, NEVER_SOURCE);
            assert!(exact_process_exit_signature(
                &typed,
                std::slice::from_ref(&parameter),
                never
            ));
            let reference = typed
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: never,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: None,
                });
            let array = typed
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: never,
                    length: FixedArrayLength::Literal(0),
                });
            let unit = typed.type_reference_table.insert(TypeReferenceNode::Unit);
            for wrong in [
                reference,
                array,
                unit,
                parameter.type_reference,
                TypeReferenceHandle::invalid(),
            ] {
                assert!(!exact_process_exit_signature(
                    &typed,
                    std::slice::from_ref(&parameter),
                    wrong
                ));
            }
            let mut wrong_input = parameter;
            wrong_input.is_mutable = true;
            assert!(!exact_process_exit_signature(&typed, &[wrong_input], never));
        }

        #[test]
        fn a_same_named_user_or_edited_toolchain_type_is_not_the_canonical_result() {
            for (origin, text) in [
                (SourceOrigin::User, NEVER_SOURCE),
                (SourceOrigin::Toolchain, "public data Never {}"),
            ] {
                let (typed, parameter, result) = fixture(origin, text);
                assert!(!exact_process_exit_signature(&typed, &[parameter], result));
            }
        }
    }
}

pub mod calling_vocabulary {
    use crate::TypedTrees;
    use crate::data::TypeParameterKind;
    use crate::trait_definition::TraitDefinition;
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};
    use symbols::SymbolHandle;

    pub fn standard_calling_vocabularies(
        program: &TypedTrees,
    ) -> Vec<(&TraitDefinition, &TraitDefinition)> {
        let mut per_file: std::collections::BTreeMap<
            usize,
            (Option<&TraitDefinition>, Option<&TraitDefinition>),
        > = std::collections::BTreeMap::new();
        for definition in program.traits().iter() {
            if definition.is_boundary || definition.refines.is_some() {
                continue;
            }
            let Some(file) = program
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| program.symbols.source_file(span))
            else {
                continue;
            };
            let entry = per_file.entry(file.source_id.0).or_default();
            match definition.name.as_str() {
                "Calling" => entry.0 = Some(definition),
                "CallingPolicy" => entry.1 = Some(definition),
                _ => {}
            }
        }
        per_file
            .into_values()
            .filter_map(|vocabulary| match vocabulary {
                (Some(calling), Some(policy)) => {
                    has_calling_interface(program, calling, policy).then_some((calling, policy))
                }
                _ => None,
            })
            .collect()
    }

    fn has_calling_interface(
        program: &TypedTrees,
        calling: &TraitDefinition,
        policy: &TraitDefinition,
    ) -> bool {
        let Some([parameter]) = program.trait_type_parameters(calling).as_array() else {
            return false;
        };
        let [bound] = calling.conformance_bounds.as_slice() else {
            return false;
        };
        if parameter.kind != TypeParameterKind::Implementation
            || bound.binder != parameter.symbol
            || bound.trait_symbol != policy.symbol
            || !bound.lifetime_arguments.is_empty()
            || !bound.arguments.is_empty()
            || !program.trait_machine_signatures(calling).is_empty()
            || !program.trait_type_parameters(policy).is_empty()
        {
            return false;
        }
        let mut plans = program
            .trait_machine_signatures(policy)
            .iter()
            .filter(|signature| signature.name.as_str() == "plan");
        let Some(plan) = plans.next() else {
            return false;
        };
        if plans.next().is_some() {
            return false;
        }
        let [signature] = program.state_signature_parameters(plan) else {
            return false;
        };
        program.state_signature_type_parameters(plan).is_empty()
            && plan.native_callback_parameters.is_empty()
            && !signature.is_const
            && signature.relevance == language_core::BindingRelevance::Relevant
            && is_calling_interface_data(
                program,
                policy.symbol,
                signature.type_reference,
                "BoundarySignature",
            )
            && is_calling_interface_data(
                program,
                policy.symbol,
                plan.return_type,
                "BoundaryPlanResult",
            )
    }

    fn is_calling_interface_data(
        program: &TypedTrees,
        owner: SymbolHandle,
        type_reference: TypeReferenceHandle,
        name: &str,
    ) -> bool {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        let Some(definition) = program.unique_data_definition_by_symbol(*symbol) else {
            return false;
        };
        let declaring_source = |symbol| {
            program
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| program.symbols.source_file(span))
                .map(|source| source.source_id)
        };
        definition.name.as_str() == name
            && declaring_source(owner).is_some_and(|owner| declaring_source(*symbol) == Some(owner))
    }
}
pub mod constant {
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstDeclaration {
        pub symbol: SymbolHandle,
        pub is_public: bool,
        pub declared_type: crate::types::TypeReferenceHandle,
        pub initializer_source_span: source::SourceSpan,
        pub pending_target_observation: bool,
        pub evaluated_value: Option<language_core::constant_value::EvaluatedConstant>,
        pub authored_initializer: crate::expression::ExpressionHandle,
        pub materialized_initializer: crate::expression::ExpressionHandle,
        pub evaluation_entry: arena::Handle<crate::machine::Machine>,
        pub evaluation_expression: crate::expression::ExpressionHandle,
    }
}
pub mod data {
    use crate::name::Identifier;
    use crate::types::TypeReferenceHandle;
    use arena::HandleSpan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataDefinition {
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub is_public: bool,
        pub predicate_subject_symbol: SymbolHandle,
        pub predicate_subject_type: TypeReferenceHandle,
        pub supply_mode: language_semantics::DataSupplyMode,
        pub lifetime_parameters: Vec<Identifier>,
        pub type_parameters: HandleSpan<TypeParameter>,
        pub generic_instance: Option<TypeReferenceHandle>,
        pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
        pub properties: DataProperties,
        pub cleanup_selection: Option<crate::domain::CleanupSelection>,
        pub quotient: Option<QuotientDefinition>,
        pub requires_facts: HandleSpan<crate::domain::ProofFact>,
        pub zero_gated: bool,
        pub retired_identities: Vec<u64>,
        pub members: HandleSpan<DataMember>,
    }

    impl Default for DataDefinition {
        fn default() -> Self {
            Self {
                symbol: SymbolHandle::invalid(),
                name: Identifier::default(),
                is_public: false,
                predicate_subject_symbol: SymbolHandle::invalid(),
                predicate_subject_type: TypeReferenceHandle::invalid(),
                supply_mode: language_semantics::DataSupplyMode::CheckedShape,
                lifetime_parameters: Vec::new(),
                type_parameters: HandleSpan::empty(),
                conformance_bounds: Vec::new(),
                generic_instance: None,
                properties: DataProperties::default(),
                cleanup_selection: None,
                quotient: None,
                requires_facts: HandleSpan::empty(),
                zero_gated: false,
                retired_identities: Vec::new(),
                members: HandleSpan::empty(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct QuotientDefinition {
        pub carrier: TypeReferenceHandle,
        pub relation: Vec<Identifier>,
        pub relation_symbol: SymbolHandle,
        pub equivalence: Option<QuotientEquivalenceSelection>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct QuotientEquivalenceSelection {
        pub relation: Vec<Identifier>,
        pub relation_symbol: SymbolHandle,
        pub trait_name: Identifier,
        pub trait_symbol: SymbolHandle,
        pub trait_arguments: HandleSpan<TypeReferenceHandle>,
        pub conformance_name: Identifier,
        pub conformance_symbol: SymbolHandle,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataProperties {
        pub carry: Option<language_semantics::CarryPolicy>,
        pub multiplicity: language_semantics::Multiplicity,
    }

    impl DataDefinition {
        pub fn shape_kind_from_members<'a>(
            members: impl IntoIterator<Item = &'a DataMember>,
        ) -> DataShapeKind {
            let mut has_fields = false;
            let mut has_variants = false;

            for member in members {
                match member {
                    DataMember::Field(_) => has_fields = true,
                    DataMember::Variant(_) => has_variants = true,
                }
            }

            match (has_fields, has_variants) {
                (false, false) => DataShapeKind::Empty,
                (true, false) => DataShapeKind::Record,
                (false, true) => DataShapeKind::Enum,
                (true, true) => DataShapeKind::Mixed,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DataShapeKind {
        Empty,
        Enum,
        Mixed,
        Record,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum DataMember {
        Field(DataField),
        Variant(DataVariant),
    }

    impl DataMember {
        pub fn as_field(&self) -> Option<&DataField> {
            let DataMember::Field(field) = self else {
                return None;
            };
            Some(field)
        }

        pub fn as_variant(&self) -> Option<&DataVariant> {
            let DataMember::Variant(variant) = self else {
                return None;
            };
            Some(variant)
        }
    }

    impl Default for DataMember {
        fn default() -> Self {
            Self::Variant(DataVariant::default())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TypeParameter {
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub kind: TypeParameterKind,
        pub bounds: DataProperties,
    }

    pub fn type_parameter_demands_stronger_properties(
        requirement: &TypeParameter,
        provider: &TypeParameter,
    ) -> bool {
        (provider.bounds.multiplicity != language_semantics::Multiplicity::Affine
            && requirement.bounds.multiplicity != provider.bounds.multiplicity)
            || provider.bounds.carry.is_some() && provider.bounds.carry != requirement.bounds.carry
    }

    impl Default for TypeParameter {
        fn default() -> Self {
            Self {
                symbol: SymbolHandle::invalid(),
                name: Identifier::default(),
                kind: TypeParameterKind::Type,
                bounds: DataProperties::default(),
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum TypeParameterKind {
        #[default]
        Type,
        Implementation,
        Const {
            type_reference: TypeReferenceHandle,
        },
        Value {
            type_reference: TypeReferenceHandle,
        },
        Machine {
            contract: MachineParameterContract,
        },
        Proposition {
            contract: PropositionParameterSignature,
        },
    }

    impl TypeParameterKind {
        pub fn is_const_or_value(&self) -> bool {
            matches!(self, Self::Const { .. } | Self::Value { .. })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum MachineParameterContract {
        RequirementIdentity,
        Structural(crate::signature::StateSignature),
        Nominal {
            receiving_trait: SymbolHandle,
            receiving_arguments: Vec<crate::types::TypeReferenceHandle>,
            trait_definition: SymbolHandle,
            requirement: SymbolHandle,
            trait_arguments: Vec<crate::types::TypeReferenceHandle>,
            instantiated: Option<Box<crate::signature::StateSignature>>,
        },
    }

    impl Default for MachineParameterContract {
        fn default() -> Self {
            Self::Structural(crate::signature::StateSignature::default())
        }
    }

    impl MachineParameterContract {
        pub fn structural(&self) -> Option<&crate::signature::StateSignature> {
            match self {
                Self::Structural(signature) => Some(signature),
                Self::RequirementIdentity | Self::Nominal { .. } => None,
            }
        }

        pub fn structural_mut(&mut self) -> Option<&mut crate::signature::StateSignature> {
            match self {
                Self::Structural(signature) => Some(signature),
                Self::RequirementIdentity | Self::Nominal { .. } => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub enum MachineParameterContractView<'program> {
        Structural(&'program crate::signature::StateSignature),
        Nominal {
            trait_definition: &'program crate::trait_definition::TraitDefinition,
            requirement: &'program crate::signature::StateSignature,
        },
    }

    impl<'program> MachineParameterContractView<'program> {
        pub fn signature(self) -> &'program crate::signature::StateSignature {
            match self {
                Self::Structural(signature)
                | Self::Nominal {
                    requirement: signature,
                    ..
                } => signature,
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct PropositionParameterSignature {
        pub name: Identifier,
        pub parameters: HandleSpan<crate::signature::StateParameter>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataField {
        pub identity: Option<u64>,
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub relevance: language_core::BindingRelevance,
        pub type_reference: TypeReferenceHandle,
    }

    fn member_path_identity(identity: Option<u64>, name: &Identifier) -> String {
        identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| name.as_str().to_owned())
    }

    #[derive(Debug, Clone, Default, Eq, compiler_storage::Record)]
    pub struct DataMemberIdentity {
        pub ordinal: Option<u64>,
        pub name: Identifier,
    }

    impl DataMemberIdentity {
        pub fn of_member(ordinal: Option<u64>, name: Identifier) -> Self {
            Self { ordinal, name }
        }

        pub fn of_spelling(spelling: &str) -> Self {
            if let Some(ordinal) = spelling
                .strip_prefix('#')
                .and_then(|digits| digits.parse::<u64>().ok())
            {
                return Self {
                    ordinal: Some(ordinal),
                    name: Identifier::default(),
                };
            }
            Self {
                ordinal: None,
                name: Identifier::generated(spelling),
            }
        }

        pub fn is_valid(&self) -> bool {
            self.ordinal.is_some() || !self.name.as_str().is_empty()
        }

        pub fn spelling(&self) -> String {
            member_path_identity(self.ordinal, &self.name)
        }
    }

    impl PartialEq for DataMemberIdentity {
        fn eq(&self, other: &Self) -> bool {
            match (self.ordinal, other.ordinal) {
                (Some(own), Some(theirs)) => own == theirs,
                (None, None) => self.name == other.name,
                _ => false,
            }
        }
    }

    impl Ord for DataMemberIdentity {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.spelling().cmp(&other.spelling())
        }
    }

    impl PartialOrd for DataMemberIdentity {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl std::hash::Hash for DataMemberIdentity {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.spelling().hash(state);
        }
    }

    impl DataField {
        pub fn path_identity(&self) -> String {
            member_path_identity(self.identity, &self.name)
        }

        pub fn member_identity(&self) -> DataMemberIdentity {
            DataMemberIdentity::of_member(self.identity, self.name.clone())
        }
    }

    impl DataVariant {
        pub fn path_identity(&self) -> String {
            member_path_identity(self.identity, &self.name)
        }

        pub fn member_identity(&self) -> DataMemberIdentity {
            DataMemberIdentity::of_member(self.identity, self.name.clone())
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataVariant {
        pub identity: Option<u64>,
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub payload: HandleSpan<DataField>,
        pub requires_facts: HandleSpan<crate::domain::ProofFact>,
        pub retired_payload_identities: Vec<u64>,
    }
}
pub mod domain {
    use crate::TypedTrees;
    use crate::name::Identifier;
    use crate::type_identity::{IndexIdentityArgument, normalized_index_identity};
    use crate::types::TypeReferenceHandle;
    use crate::types::TypeReferenceNode::Named;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;

    /// The policy a domain selects with `access Policy`
    /// (`wiki/spec/language/domains.md`, "Domain-owned read and write
    /// authority"): the named implementation whose `plan` states who may
    /// introduce the domain and who may read or write what it governs.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainAccessPolicy {
        /// Evaluation must select the exact application, including each
        /// owner's type, constant, lifetime and member arguments.
        pub selection: crate::expression::StaticMachineArgument,
        /// Where the policy is named.
        pub source_span: source::SourceSpan,
        /// What Build-time evaluation concluded about the plan the selected
        /// policy returns for this domain. Only a demanded domain's plan is
        /// run.
        pub plan: AccessPlanJudgment,
    }

    /// The Build's conclusion about one domain's access plan. Only
    /// `Checked` shows anybody to be an issuer or judges a read or write;
    /// the other two leave every such use refused.
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum AccessPlanJudgment {
        /// Nobody has run the plan: the domain is not demanded, or its
        /// policy application is itself unserved.
        #[default]
        Unevaluated,
        Checked(CheckedDomainAccessPlan),
        /// The Build ran the plan and could not judge it: it failed to
        /// evaluate, described another schema, named a route no declaration
        /// answers, or stated restrictions no consumer reads yet. The domain
        /// still passes whole through formals, results and fields, as an
        /// unevaluated one does.
        Unjudged(String),
    }

    impl DomainAccessPolicy {
        pub fn implementation(&self) -> SymbolHandle {
            self.selection.symbol
        }
    }

    /// The judgment of one evaluated `AccessPolicy::plan` result
    /// (`wiki/spec/resources/access.md`, "Plans and admitted supply").
    ///
    /// The evaluator authenticates the returned schema, declarations and
    /// complete caller applications. The canonical operation judgment then
    /// checks the retained read/write rules on every actual subject and
    /// invocation. Observation demands and boundary reach remain unserved
    /// until their supply/lifecycle consumers can prove them.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CheckedDomainAccessPlan {
        /// The policy member whose body returned the plan.
        pub plan_machine: SymbolHandle,
        /// Keep the complete returned rules with the domain that selected
        /// them. These descriptions alone grant no operation permission.
        pub normalized: access_plans::AccessPlan,
        /// These issuer bindings name declarations only. A read/write rule
        /// must also match the function's complete static arguments.
        pub issuance: CheckedIssuance,
    }

    /// The plan's issuer choice. `Only` with no routes permits nobody; it is
    /// never read as `Unrestricted`.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum CheckedIssuance {
        Only(Vec<language_semantics::DomainEstablishmentRoute>),
        Unrestricted,
    }

    /// Who may introduce a domain, as far as its declaration decides it
    /// (`wiki/spec/resources/access.md`, "Plans and admitted supply").
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DomainIssuance<'domain> {
        /// No policy and no route list: qualification needs the predicates
        /// and nothing else.
        Unrestricted,
        /// Exactly these origins: written on the declaration with the
        /// retired `established by` clause, or returned by the selected
        /// policy's evaluated plan (`CheckedDomainAccessPlan`).
        Only(&'domain [language_semantics::DomainEstablishmentRoute]),
        /// The selected policy's plan states the issuance, and no checked
        /// plan is recorded: no origin can be shown to be authorized, so a
        /// reader treats this as admitting nobody it can name, never as
        /// unrestricted.
        Planned(&'domain DomainAccessPolicy),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainDefinition {
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub subject_name: Identifier,
        pub subject_symbol: SymbolHandle,
        pub type_parameters: HandleSpan<crate::data::TypeParameter>,
        pub target_type: TypeReferenceHandle,
        pub index_arguments: Vec<TypeReferenceHandle>,
        pub is_public: bool,
        pub cleanup_selection: Option<CleanupSelection>,
        pub alias: Option<DomainAliasDefinition>,
        pub classification: Option<language_semantics::DomainClassification>,
        pub predicate_presence: language_semantics::DomainPredicatePresence,
        pub facts: HandleSpan<ProofFact>,
        pub operators: HandleSpan<crate::operator::OperatorDefinition>,
        pub semantic_clause_token_count: usize,
        pub semantic_id: language_semantics::SemanticDomainId,
        pub semantic_roles: language_semantics::DomainSemanticRoles,
        pub establishment_routes: Vec<language_semantics::DomainEstablishmentRoute>,
        pub access_policy: Option<DomainAccessPolicy>,
    }

    impl DomainDefinition {
        /// Who may introduce this domain. The one question every reader of
        /// an `as +D` asks; a declaration writes a policy or a route list,
        /// never both.
        pub fn issuance(&self) -> DomainIssuance<'_> {
            match &self.access_policy {
                Some(policy) => match &policy.plan {
                    AccessPlanJudgment::Checked(plan) => match &plan.issuance {
                        CheckedIssuance::Only(routes) => DomainIssuance::Only(routes),
                        CheckedIssuance::Unrestricted => DomainIssuance::Unrestricted,
                    },
                    AccessPlanJudgment::Unevaluated | AccessPlanJudgment::Unjudged(_) => {
                        DomainIssuance::Planned(policy)
                    }
                },
                None if self.establishment_routes.is_empty() => DomainIssuance::Unrestricted,
                None => DomainIssuance::Only(&self.establishment_routes),
            }
        }
    }

    impl DomainDefinition {
        /// An empty theory has nothing about contents left to translate.
        /// Retained but unread clauses and other semantic definitions are not
        /// an empty theory merely because no predicate was produced for them.
        pub fn has_complete_empty_theory(&self) -> bool {
            self.predicate_presence == language_semantics::DomainPredicatePresence::Absent
                && self.facts.is_empty()
                && self.operators.is_empty()
                && self.semantic_clause_token_count == 0
                && self.alias.is_none()
                && self.classification.is_none()
                && self.semantic_roles.is_empty()
                && self.cleanup_selection.is_none()
        }

        /// Whether source lowering still lacks the complete selected plan.
        /// A checked plan permits forming operations; its retained rules are
        /// enforced by the canonical operation judgment before execution.
        pub fn access_unjudged(&self) -> bool {
            self.access_policy
                .as_ref()
                .is_some_and(|policy| !matches!(policy.plan, AccessPlanJudgment::Checked(_)))
        }
    }

    #[cfg(test)]
    #[test]
    fn unread_domain_clauses_are_not_a_complete_empty_theory() {
        let mut definition = DomainDefinition::default();
        assert!(definition.has_complete_empty_theory());
        definition.semantic_clause_token_count = 1;
        assert!(!definition.has_complete_empty_theory());
        definition.semantic_clause_token_count = 0;
        definition.predicate_presence = language_semantics::DomainPredicatePresence::Present;
        assert!(!definition.has_complete_empty_theory());
        definition.predicate_presence = language_semantics::DomainPredicatePresence::Absent;
        definition.alias = Some(DomainAliasDefinition {
            constituents: Vec::new(),
        });
        assert!(!definition.has_complete_empty_theory());
    }

    impl Default for DomainDefinition {
        fn default() -> Self {
            Self {
                symbol: SymbolHandle::invalid(),
                name: Identifier::default(),
                subject_name: Identifier::default(),
                subject_symbol: SymbolHandle::invalid(),
                type_parameters: HandleSpan::empty(),
                target_type: TypeReferenceHandle::invalid(),
                index_arguments: Vec::new(),
                is_public: false,
                cleanup_selection: None,
                alias: None,
                classification: None,
                predicate_presence: language_semantics::DomainPredicatePresence::Absent,
                facts: HandleSpan::empty(),
                operators: HandleSpan::empty(),
                semantic_clause_token_count: 0,
                semantic_id: language_semantics::SemanticDomainId::NULL,
                semantic_roles: language_semantics::DomainSemanticRoles::default(),
                establishment_routes: Vec::new(),
                access_policy: None,
            }
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainAliasDefinition {
        pub constituents: Vec<DomainAliasConstituent>,
    }

    #[derive(Debug, Clone, Default, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainAliasConstituent {
        pub domain: HandleSpan<Identifier>,
        pub domain_symbol: SymbolHandle,
    }

    pub fn indexed_domain_instance_name(
        program: &TypedTrees,
        domain: &DomainDefinition,
        parameters: arena::RootRows<'_, crate::data::TypeParameter>,
        arguments: &[TypeReferenceHandle],
    ) -> Result<String, Diagnostic> {
        indexed_domain_instance_name_with_substitutions(program, domain, parameters, arguments, &[])
    }

    pub fn indexed_domain_instance_name_with_substitutions(
        program: &TypedTrees,
        domain: &DomainDefinition,
        parameters: arena::RootRows<'_, crate::data::TypeParameter>,
        arguments: &[TypeReferenceHandle],
        substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
    ) -> Result<String, Diagnostic> {
        if parameters.len() != arguments.len() {
            return Err(Diagnostic::error(
                "indexed-domain argument arity does not match its declaration",
            ));
        }
        if arguments.is_empty() {
            return Ok(program
                .semantic_domains
                .name(domain.semantic_id)
                .unwrap_or(domain.name.as_str())
                .to_owned());
        }
        let mut identities = Vec::with_capacity(arguments.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            match parameter.kind {
                crate::data::TypeParameterKind::Type => identities.push(format!(
                    "type:{}",
                    substituted_type_identity_text(program, *argument, substitutions)?
                )),
                crate::data::TypeParameterKind::Const { type_reference } => {
                    let expected = const_index_type_name(program, type_reference)?;
                    identities.push(closed_domain_argument_identity(
                        program,
                        *argument,
                        &expected,
                        substitutions,
                    )?);
                }
                _ => {
                    return Err(Diagnostic::error(format!(
                        "domain family `{}` has an ineligible index binder `{}`",
                        domain.name, parameter.name
                    )));
                }
            }
        }
        let base = program
            .semantic_domains
            .name(domain.semantic_id)
            .unwrap_or(domain.name.as_str());
        Ok(format!("{base}<{}>", identities.join(",")))
    }

    fn substituted_type_identity_text(
        program: &TypedTrees,
        argument: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
    ) -> Result<String, Diagnostic> {
        normalized_index_identity(
            program,
            IndexIdentityArgument::TypeArgument(argument),
            substitutions,
        )
        .ok_or_else(|| {
            Diagnostic::error("indexed-domain argument has no valid substitution identity")
        })
    }

    pub fn has_generic_carrier(program: &TypedTrees, domain: &DomainDefinition) -> bool {
        let Some(parameter) = program.domain_type_parameters(domain).first() else {
            return false;
        };
        if !matches!(parameter.kind, crate::data::TypeParameterKind::Type) {
            return false;
        }
        let Named { symbol, .. } = program.type_reference_node(domain.target_type) else {
            return false;
        };
        symbol.is_valid() && *symbol == parameter.symbol
    }

    pub fn index_parameters<'program>(
        program: &'program TypedTrees,
        domain: &DomainDefinition,
    ) -> arena::RootRows<'program, crate::data::TypeParameter> {
        let parameters = program.domain_type_parameters(domain);
        if has_generic_carrier(program, domain) {
            parameters.slice(1..parameters.len())
        } else {
            parameters
        }
    }

    /// Whether the family's written index list names its exact index binders
    /// in order. Nontrivial index patterns need their own formation judgment;
    /// applying an argument tuple must not silently erase such a pattern.
    pub fn has_direct_index_telescope(program: &TypedTrees, domain: &DomainDefinition) -> bool {
        let parameters = index_parameters(program, domain);
        parameters.len() == domain.index_arguments.len()
            && parameters
                .iter()
                .zip(&domain.index_arguments)
                .all(|(parameter, argument)| {
                    matches!(program.type_reference_table.type_reference(*argument),
                    Named { symbol, .. } if symbol.is_valid() && *symbol == parameter.symbol)
                })
    }

    pub fn domain_accepts_carrier(
        program: &TypedTrees,
        domain: &DomainDefinition,
        carrier: TypeReferenceHandle,
        arguments: &[TypeReferenceHandle],
    ) -> bool {
        use crate::type_identity::TypeIdentityRequest;

        if !carrier.is_valid() || !domain.target_type.is_valid() {
            return false;
        }
        if has_generic_carrier(program, domain) {
            return true;
        }
        let substitutions = index_parameters(program, domain)
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>();
        let target = program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
            substitutions: &substitutions,
            ..TypeIdentityRequest::package_qualified(domain.target_type)
        });
        // `LayoutStorage<T, L>` is T's value in L's geometry, not a wrapper
        // around a stored T (plans.md, "Representation applications"), so
        // its data carrier is T: `RegisterStorage in Registers::Placed`
        // qualifies the laid `Registers`. The layout selection stays on the
        // application; nothing else about T is erased to match.
        program.package_qualified_type_identity(carrier) == target
            || program
                .layout_storage_arguments(carrier)
                .is_some_and(|laid| {
                    program.package_qualified_type_identity(laid.semantic_type) == target
                })
    }

    pub fn domain_by_symbol(
        program: &TypedTrees,
        symbol: SymbolHandle,
    ) -> Option<&DomainDefinition> {
        let position = program.domain_position_by_symbol(symbol)?;
        program.domain_definitions().get(position)
    }

    fn const_index_type_name(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
    ) -> Result<String, Diagnostic> {
        use crate::types::{FixedArrayLength, TypeReferenceNode};
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { .. } => Ok(program
                .primitive_type_reference(type_reference)
                .map_or_else(
                    || program.normalized_type_identity_text(type_reference),
                    |primitive| primitive.name(),
                )),
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(_),
                ..
            } => Ok(program.normalized_type_identity_text(type_reference)),
            TypeReferenceNode::Constrained { base_type, .. } => {
                const_index_type_name(program, *base_type)
            }
            TypeReferenceNode::Unit => Ok("()".to_owned()),
            _ => Err(Diagnostic::error(
                "indexed-domain const parameter types must have canonical structural identity",
            )),
        }
    }

    fn closed_domain_argument_identity(
        program: &TypedTrees,
        argument: TypeReferenceHandle,
        expected: &str,
        substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
    ) -> Result<String, Diagnostic> {
        let Named { symbol, name } = program.type_reference_table.type_reference(argument) else {
            if let crate::types::TypeReferenceNode::ConstExpression(expression) =
                program.type_reference_table.type_reference(argument)
            {
                if program.expression_table.binary_node(*expression).is_some()
                    && let Some(identity) =
                        crate::type_identity::normalized_const_index_expression_identity(
                            program,
                            *expression,
                        )
                    && let Some(value) = integer_atom_text(&identity)
                {
                    return Ok(format!("integer:{expected}:{value}"));
                }
                return Ok(format!(
                    "expression:{expected}:{}",
                    substituted_type_identity_text(program, argument, substitutions)?
                ));
            }
            return Err(Diagnostic::error(
                "indexed-domain arguments must be canonical const values, direct const binders, or supported open const expressions",
            ));
        };
        if let Some((_, replacement)) = substitutions.iter().find(|(binder, _)| binder == symbol) {
            return match replacement {
                IndexIdentityArgument::TypeReference(reference)
                | IndexIdentityArgument::TypeArgument(reference) => {
                    closed_domain_argument_identity(program, *reference, expected, &[])
                }
                IndexIdentityArgument::Subject(subject) if subject.is_valid() => Ok(format!(
                    "binder:{}:{expected}",
                    program.symbols.display_path(*subject, "::")
                )),
                _ => Err(Diagnostic::error(
                    "indexed-domain const argument requires a type reference or exact subject",
                )),
            };
        }
        if let Some(value) = crate::canonical_const_declaration_value(program, *symbol) {
            if value.type_name != expected {
                return Err(Diagnostic::error(format!(
                    "indexed-domain selected constant has canonical type `{}`, expected `{expected}`",
                    value.type_name
                )));
            }
            if let Some(retained) =
                language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                && retained.encoding != value.encoding
            {
                return Err(Diagnostic::error(
                    "indexed-domain constant differs from its selected declaration value",
                ));
            }
            return canonical_domain_const_identity(&value, expected);
        }
        if let Some(value) =
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
        {
            return canonical_domain_const_identity(&value, expected);
        }
        if name.as_str().parse::<i128>().is_ok() {
            return Ok(format!("integer:{expected}:{}", name.as_str()));
        }
        Ok(format!(
            "binder:{}:{}",
            if symbol.is_valid() {
                program.symbols.display_path(*symbol, "::")
            } else {
                name.as_str().to_owned()
            },
            expected
        ))
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProofFact {
        Expression(crate::expression::ExpressionHandle),
        Membership(ProofMembershipFact),
        Proposition(crate::proposition::PropositionApplication),
    }

    impl ProofFact {
        pub fn as_expression(&self) -> Option<&crate::expression::ExpressionHandle> {
            let ProofFact::Expression(expression) = self else {
                return None;
            };
            Some(expression)
        }

        pub fn as_membership(&self) -> Option<&ProofMembershipFact> {
            let ProofFact::Membership(membership) = self else {
                return None;
            };
            Some(membership)
        }
    }

    impl Default for ProofFact {
        fn default() -> Self {
            Self::Expression(crate::expression::ExpressionHandle::invalid())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProofMembershipFact {
        pub value: crate::expression::ExpressionHandle,
        pub domain: HandleSpan<Identifier>,
        pub domain_symbol: SymbolHandle,
        pub domain_arguments: HandleSpan<TypeReferenceHandle>,
        pub semantic_domain: language_semantics::SemanticDomainId,
        pub authored_domain_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    }

    impl Default for ProofMembershipFact {
        fn default() -> Self {
            Self {
                value: crate::expression::ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol: SymbolHandle::invalid(),
                domain_arguments: HandleSpan::empty(),
                semantic_domain: language_semantics::SemanticDomainId::NULL,
                authored_domain_selection: None,
            }
        }
    }

    pub fn domain_subject_reference(
        program: &TypedTrees,
        domain: &DomainDefinition,
        expression: crate::expression::ExpressionHandle,
    ) -> bool {
        if !domain.subject_symbol.is_valid()
            || !program.expression_table.expression_is_valid(expression)
        {
            return false;
        }
        let subject = program.symbols.get(domain.subject_symbol);
        let applied_subject = program
            .domain_application(domain.symbol, domain.semantic_id)
            .is_some_and(|application| {
                application
                    .theory
                    .as_ref()
                    .and_then(|theory| theory.as_ref().ok())
                    .is_some_and(|theory| theory.subject_symbol == domain.subject_symbol)
                    && program
                        .domain_definition(domain.symbol)
                        .is_some_and(|declared| {
                            program.symbols.authored_origin(domain.subject_symbol)
                                == declared.subject_symbol
                        })
            });
        if subject.kind != symbols::SymbolKind::Parameter
            || (subject.parent != domain.symbol && !applied_subject)
        {
            return false;
        }
        let crate::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(expression)
        else {
            return false;
        };
        path.symbol == domain.subject_symbol
            && path.head_symbol == domain.subject_symbol
            && path.members.count() == 1
            && program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                == [domain.subject_symbol]
    }

    pub fn subject_membership_facts<'a>(
        program: &'a TypedTrees,
        domain: &'a DomainDefinition,
    ) -> impl Iterator<Item = &'a ProofMembershipFact> + 'a {
        program
            .proof_facts(domain)
            .iter()
            .filter_map(|fact| match fact {
                ProofFact::Membership(membership) => {
                    domain_subject_reference(program, domain, membership.value)
                        .then_some(membership)
                }
                _ => None,
            })
    }

    pub fn type_selects_semantic_domain(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(type_reference) {
            crate::types::TypeReferenceNode::Reference { referee, .. } => {
                type_selects_semantic_domain(program, *referee, domain_symbol)
            }
            crate::types::TypeReferenceNode::Constrained { constraints, .. } => program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| match constraint {
                    crate::types::TypeConstraintNode::Domain(domain) => {
                        domain.symbol == domain_symbol
                            && domain.semantic_roles.denotation_dimension.is_some()
                    }
                    _ => false,
                }),
            _ => false,
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct CleanupSelection {
        pub application: crate::static_applications::StaticMachineApplicationHandle,
        pub authorization: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
        pub subject: TypeReferenceHandle,
        pub executable: SymbolHandle,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct AppliedDataCleanupSelection {
        pub declaration: SymbolHandle,
        pub selection: CleanupSelection,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct AppliedDomainApplication {
        pub declaration: SymbolHandle,
        pub identity: language_semantics::SemanticDomainId,
        /// The declaration's whole telescope, including a generic carrier.
        pub arguments: Vec<TypeReferenceHandle>,
        /// Whether the complete coordinate contains no remaining value/type
        /// binders. Open applications retain meaning but still need judgment.
        pub is_closed: bool,
        /// Typing retains coordinates; typed preparation applies the theory.
        /// Absence is pending work. A preparation failure stays on this exact
        /// application; neither state is an empty predicate or proof receipt.
        pub theory: Option<Result<DomainDefinition, String>>,
    }

    /// Rows stay in preparation order; the exact-key index is derived from
    /// those rows and never persisted as another application authority.
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct AppliedDomainApplications {
        rows: Vec<AppliedDomainApplication>,
        by_application:
            std::collections::HashMap<(SymbolHandle, language_semantics::SemanticDomainId), usize>,
    }

    impl AppliedDomainApplications {
        pub fn iter(&self) -> std::slice::Iter<'_, AppliedDomainApplication> {
            self.rows.iter()
        }

        pub(crate) fn len(&self) -> usize {
            self.rows.len()
        }

        pub(crate) fn get(&self, position: usize) -> Option<&AppliedDomainApplication> {
            self.rows.get(position)
        }

        pub(crate) fn find(
            &self,
            declaration: SymbolHandle,
            identity: language_semantics::SemanticDomainId,
        ) -> Option<&AppliedDomainApplication> {
            self.by_application
                .get(&(declaration, identity))
                .and_then(|position| self.rows.get(*position))
        }

        pub(crate) fn insert(&mut self, application: AppliedDomainApplication) -> bool {
            let key = (application.declaration, application.identity);
            let std::collections::hash_map::Entry::Vacant(entry) = self.by_application.entry(key)
            else {
                return false;
            };
            entry.insert(self.rows.len());
            self.rows.push(application);
            true
        }

        pub(crate) fn set_theory(
            &mut self,
            position: usize,
            theory: Result<DomainDefinition, String>,
        ) {
            self.rows[position].theory = Some(theory);
        }
    }

    impl compiler_storage::Record for AppliedDomainApplications {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            self.rows.write(output)
        }

        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            let rows = Vec::<AppliedDomainApplication>::read(input)?;
            let allocation = rows
                .len()
                .checked_mul(std::mem::size_of::<(
                    (SymbolHandle, language_semantics::SemanticDomainId),
                    usize,
                )>())
                .ok_or_else(|| input.error("domain application index allocation overflow"))?;
            input.allocate(allocation)?;
            let mut result = Self {
                by_application: std::collections::HashMap::with_capacity(rows.len()),
                rows,
            };
            for (position, row) in result.rows.iter().enumerate() {
                if result
                    .by_application
                    .insert((row.declaration, row.identity), position)
                    .is_some()
                {
                    return Err(input.error("duplicate exact domain application"));
                }
            }
            Ok(result)
        }
    }

    impl TypedTrees {
        pub fn domain_application(
            &self,
            symbol: SymbolHandle,
            identity: language_semantics::SemanticDomainId,
        ) -> Option<&AppliedDomainApplication> {
            self.applied_domain_applications.find(symbol, identity)
        }

        pub fn domain_arguments_for_instance(
            &self,
            symbol: SymbolHandle,
            identity: language_semantics::SemanticDomainId,
        ) -> Option<&[TypeReferenceHandle]> {
            let declared = self.domain_definition(symbol)?;
            if self.domain_type_parameters(declared).is_empty() && declared.semantic_id == identity
            {
                return Some(&[]);
            }
            self.domain_application(symbol, identity)
                .map(|application| application.arguments.as_slice())
        }

        pub fn domain_definition_for_instance(
            &self,
            symbol: SymbolHandle,
            identity: language_semantics::SemanticDomainId,
        ) -> Option<&DomainDefinition> {
            let declared = self.domain_definition(symbol)?;
            if self.domain_type_parameters(declared).is_empty() && declared.semantic_id == identity
            {
                return Some(declared);
            }
            self.domain_application(symbol, identity)
                .and_then(|application| application.theory.as_ref()?.as_ref().ok())
        }
    }

    pub fn index_argument_is_closed(
        program: &TypedTrees,
        reference: crate::types::TypeReferenceHandle,
    ) -> bool {
        index_argument_is_closed_with_substitutions(program, reference, &[])
    }

    pub fn index_argument_is_closed_with_substitutions(
        program: &TypedTrees,
        reference: crate::types::TypeReferenceHandle,
        substitutions: &[(symbols::SymbolHandle, crate::types::TypeReferenceHandle)],
    ) -> bool {
        struct Closure<'a> {
            program: &'a TypedTrees,
            substitutions: &'a [(symbols::SymbolHandle, crate::types::TypeReferenceHandle)],
            types: std::collections::HashSet<crate::types::TypeReferenceHandle>,
            expressions: std::collections::HashSet<crate::expression::ExpressionHandle>,
            open: bool,
        }
        impl Extend<symbols::SymbolHandle> for Closure<'_> {
            fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, symbols: T) {
                self.open |= symbols
                    .into_iter()
                    .filter(|symbol| symbol.is_valid())
                    .any(|symbol| {
                        if let Some((_, actual)) = self
                            .substitutions
                            .iter()
                            .find(|(formal, _)| *formal == symbol)
                        {
                            return !index_argument_is_closed(self.program, *actual);
                        }
                        matches!(
                            self.program.symbols.get(symbol).kind,
                            symbols::SymbolKind::TypeParameter
                                | symbols::SymbolKind::MachineParameter
                                | symbols::SymbolKind::ConformanceParameter
                                | symbols::SymbolKind::PropositionParameter
                                | symbols::SymbolKind::Parameter
                                | symbols::SymbolKind::Local
                        )
                    });
            }
        }
        impl crate::references::TypedReferences for Closure<'_> {
            fn type_reference(&mut self, reference: crate::types::TypeReferenceHandle) -> bool {
                self.types.insert(reference)
            }
            fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
                self.expressions.insert(expression)
            }
        }
        let mut closure = Closure {
            program,
            substitutions,
            types: Default::default(),
            expressions: Default::default(),
            open: false,
        };
        crate::references::visit_type_reference(program, &mut closure, reference);
        !closure.open
    }

    fn integer_atom_text(identity: &str) -> Option<&str> {
        let value = identity.strip_prefix("integer(")?.strip_suffix(')')?;
        let digits = value.strip_prefix('-').unwrap_or(value);
        (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())).then_some(value)
    }

    fn canonical_domain_const_identity(
        value: &language_semantics::const_value::CanonicalConstValue,
        expected: &str,
    ) -> Result<String, Diagnostic> {
        if value.type_name != expected {
            return Err(Diagnostic::error(format!(
                "indexed-domain argument has canonical type `{}`, expected `{expected}`",
                value.type_name
            )));
        }
        if let Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer {
            value,
            ..
        }) = value.decode_encoding()
        {
            return Ok(format!("integer:{expected}:{value}"));
        }
        Ok(format!(
            "const:{expected}:{}:{}",
            value.encoding.len(),
            value.encoding
        ))
    }
}
pub mod intrinsic_requirements {
    use crate::TypedTrees;
    use crate::signature::StateParameter;
    use crate::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IntrinsicRequirementKind {
        Operator,
        TopLevelRequirement,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IntrinsicRequirement<'typed> {
        pub kind: IntrinsicRequirementKind,
        pub symbol: symbols::SymbolHandle,
        pub call_target: symbols::SymbolHandle,
        pub namespace: String,
        pub name: String,
        pub parameters: &'typed [StateParameter],
        pub return_type: TypeReferenceHandle,
        pub package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub requirement_identity: String,
        operator: Option<&'typed crate::operator::OperatorDefinition>,
    }

    impl<'typed> IntrinsicRequirement<'typed> {
        pub fn from_operator(
            typed: &'typed TypedTrees,
            operator: &'typed crate::operator::OperatorDefinition,
        ) -> Option<Self> {
            if !operator.is_boundary {
                return None;
            }
            if let Some(requirement) = typed.machine(operator.symbol) {
                return Self::from_requirement(typed, requirement);
            }
            let [namespace, name] = typed.operator_path_members(operator.name) else {
                return None;
            };
            let requirement_identity =
                crate::operator::boundary_operator_requirement_identity(typed, operator);
            if requirement_identity.is_empty() {
                return None;
            }
            Some(Self {
                kind: IntrinsicRequirementKind::Operator,
                symbol: operator.symbol,
                call_target: operator.symbol,
                namespace: namespace.as_str().to_owned(),
                name: name.as_str().to_owned(),
                parameters: typed.operator_parameters(operator),
                return_type: operator.return_type,
                package_identity: typed.symbols.symbol_package_identity(operator.symbol),
                requirement_identity,
                operator: Some(operator),
            })
        }

        pub fn from_requirement(
            typed: &'typed TypedTrees,
            requirement: &'typed crate::machine::Machine,
        ) -> Option<Self> {
            if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
                || requirement.body_is_present
                || !typed.machine_type_parameters(requirement).is_empty()
            {
                return None;
            }
            let [entry] = typed.machine_states(requirement) else {
                return None;
            };
            let parameters = typed.state_parameters(entry);
            let (namespace, name) = requirement.name.as_str().rsplit_once("::")?;
            if namespace.is_empty() || name.is_empty() || namespace.contains("::") {
                return None;
            }
            let requirement_identity = typed.machine_callable_identity_text(requirement)?;
            if requirement_identity.is_empty() {
                return None;
            }
            Some(Self {
                kind: IntrinsicRequirementKind::TopLevelRequirement,
                symbol: requirement.symbol,
                call_target: entry.symbol,
                namespace: namespace.to_owned(),
                name: name.to_owned(),
                parameters,
                return_type: entry.return_type,
                package_identity: typed.symbols.symbol_package_identity(requirement.symbol),
                requirement_identity,
                operator: typed.machine_token_binding_by_symbol(requirement.symbol),
            })
        }

        pub fn by_symbol(typed: &'typed TypedTrees, symbol: symbols::SymbolHandle) -> Option<Self> {
            if !symbol.is_valid() {
                return None;
            }
            if let Some(operator) = typed.operator_definition_by_symbol(symbol) {
                return Self::from_operator(typed, operator);
            }
            typed
                .machines()
                .iter()
                .find(|machine| {
                    machine.symbol == symbol
                        || typed
                            .machine_states(machine)
                            .first()
                            .is_some_and(|entry| entry.symbol == symbol)
                })
                .and_then(|machine| Self::from_requirement(typed, machine))
        }

        pub fn contracts(
            &self,
            typed: &'typed TypedTrees,
        ) -> Option<&'typed [crate::signature::SignatureContract]> {
            match self.kind {
                IntrinsicRequirementKind::Operator => self
                    .operator
                    .map(|operator| typed.operator_contracts(operator)),
                IntrinsicRequirementKind::TopLevelRequirement => typed
                    .machine(self.symbol)
                    .map(|machine| typed.machine_contracts(machine)),
            }
        }

        pub fn display(&self) -> String {
            format!("{}::{}", self.namespace, self.name)
        }

        pub fn as_operator(&self) -> Option<&'typed crate::operator::OperatorDefinition> {
            self.operator
        }

        pub fn schema_identity(&self) -> std::borrow::Cow<'_, str> {
            if self.kind == IntrinsicRequirementKind::Operator || self.operator.is_some() {
                std::borrow::Cow::Borrowed(&self.requirement_identity)
            } else {
                std::borrow::Cow::Owned(self.display())
            }
        }

        pub fn method_name(&self) -> &str {
            match self.kind {
                IntrinsicRequirementKind::Operator => "realize",
                IntrinsicRequirementKind::TopLevelRequirement => &self.name,
            }
        }
    }
}
pub mod invocations {
    use crate::TypedTrees;
    use crate::machine::Machine;
    use crate::signature::{
        AuthoredInvocation, AuthoredInvocationTarget, StateParameter, StateSignature,
    };
    use flow_effects::InvocationTarget;

    pub fn declared_signature_invocations(
        program: &TypedTrees,
        signature: &StateSignature,
    ) -> Result<Vec<InvocationTarget>, String> {
        let parameters = program
            .state_signature_parameters(signature)
            .iter()
            .collect::<Vec<_>>();
        declared_targets(
            program,
            program.state_signature_invokes(signature),
            &parameters,
        )
    }

    pub fn declared_machine_invocations(
        program: &TypedTrees,
        machine: &Machine,
    ) -> Result<Vec<InvocationTarget>, String> {
        declared_targets(
            program,
            program.machine_invokes(machine),
            &machine_entry_parameters(program, machine),
        )
    }

    pub fn machine_entry_parameters<'a>(
        program: &'a TypedTrees,
        machine: &Machine,
    ) -> Vec<&'a StateParameter> {
        program
            .machine_states(machine)
            .first()
            .map(|state| program.state_parameters(state).iter().collect())
            .unwrap_or_default()
    }

    fn insert_target(targets: &mut Vec<InvocationTarget>, target: InvocationTarget) {
        if !targets.contains(&target) {
            targets.push(target);
            targets.sort_by_key(|target| match target {
                InvocationTarget::Parameter(index) => (0u8, *index),
                InvocationTarget::Service(symbol) => (1u8, symbol.arena_index()),
            });
        }
    }

    // Entry identities are not service reach: ordinary callback traits and
    // parameter bindings remain meaningful. An unresolved row must refuse the
    // projection instead of publishing a narrower contract than was authored.
    fn declared_targets(
        program: &TypedTrees,
        declarations: &[AuthoredInvocation],
        parameters: &[&StateParameter],
    ) -> Result<Vec<InvocationTarget>, String> {
        let mut targets = Vec::new();
        for declaration in declarations {
            let target = match declaration.target {
                AuthoredInvocationTarget::Unresolved | AuthoredInvocationTarget::Undeclared => None,
                AuthoredInvocationTarget::Parameter { ordinal, symbol } => parameters
                    .get(ordinal as usize)
                    .filter(|parameter| parameter.symbol == symbol)
                    .map(|_| InvocationTarget::Parameter(ordinal)),
                AuthoredInvocationTarget::Service(symbol) => program
                    .trait_definition_by_symbol(symbol)
                    .map(|_| InvocationTarget::Service(symbol)),
            }
            .ok_or_else(|| format!("invokes `{declaration}` has no exact retained target"))?;
            insert_target(&mut targets, target);
        }
        Ok(targets)
    }
}
pub mod layout_storage {
    use crate::{
        TypedTrees,
        types::{TypeReferenceHandle, TypeReferenceNode},
    };
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Copy)]
    pub struct LayoutStorageArguments {
        pub application: TypeReferenceHandle,
        pub semantic_type: TypeReferenceHandle,
        pub policy: TypeReferenceHandle,
        pub contract: SymbolHandle,
    }

    impl TypedTrees {
        pub fn field_layout_source(
            &self,
            mut reference: TypeReferenceHandle,
        ) -> crate::PlanLaidFieldLayoutSource {
            loop {
                if self.primitive_type_reference(reference).is_some() {
                    return crate::PlanLaidFieldLayoutSource::Scalar;
                }
                reference = self
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                if self.layout_storage_arguments(reference).is_some()
                    && self
                        .plan_laid_layout_for_type(reference)
                        .is_some_and(|plan| {
                            matches!(
                                plan.realizations,
                                crate::PlanLaidRealizations::TargetInputs(_)
                            )
                        })
                {
                    return crate::PlanLaidFieldLayoutSource::Explicit;
                }
                match self.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: crate::types::FixedArrayLength::Literal(_),
                    } => reference = *element_type,
                    _ => return crate::PlanLaidFieldLayoutSource::Default,
                }
            }
        }

        pub fn native_data_declaration_identity(
            &self,
            data: &crate::data::DataDefinition,
        ) -> Result<String, String> {
            self.normalized_hermetic_symbol_identity(data.symbol)
                .or_else(|reason| {
                    let origin = data.generic_instance.ok_or_else(|| reason.clone())?;
                    let TypeReferenceNode::Generic { base_symbol, .. } =
                        self.type_reference_table.type_reference(origin)
                    else {
                        return Err(reason);
                    };
                    if !base_symbol.is_valid() || *base_symbol == data.symbol {
                        return Err(reason);
                    }
                    self.normalized_hermetic_symbol_identity(*base_symbol)?;
                    let mut pending =
                        crate::references::collect_type_symbol_dependencies(self, origin);
                    let mut seen = std::collections::HashSet::new();
                    while let Some(symbol) = pending.pop() {
                        if !seen.insert(symbol) || self.symbols.builtin_type_atom(symbol).is_some()
                        {
                            continue;
                        }
                        let Err(reason) = self.normalized_hermetic_symbol_identity(symbol) else {
                            continue;
                        };
                        let instance = self
                            .data_definition_by_symbol(symbol)
                            .and_then(|data| data.generic_instance)
                            .ok_or_else(|| reason.clone())?;
                        let TypeReferenceNode::Generic { base_symbol, .. } =
                            self.type_reference_table.type_reference(instance)
                        else {
                            return Err(reason);
                        };
                        if !base_symbol.is_valid() || *base_symbol == symbol {
                            return Err(reason);
                        }
                        self.normalized_hermetic_symbol_identity(*base_symbol)?;
                        pending.extend(crate::references::collect_type_symbol_dependencies(
                            self, instance,
                        ));
                    }
                    Ok(format!(
                        "closed-data-instance::{}",
                        self.package_qualified_type_identity_text(origin)
                    ))
                })
        }
        /*
            Whether `symbol` is the opaque core layout data the sealed
            intrinsic catalog names as `data`.
        */
        pub fn core_layout_declaration(
            &self,
            symbol: SymbolHandle,
            data: language_core::sealed_intrinsics::SealedIntrinsic,
        ) -> bool {
            self.symbols.sealed_intrinsic(symbol) == Some(data)
                && self
                    .data_definition_by_symbol(symbol)
                    .is_some_and(|definition| {
                        definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
                    })
        }

        pub fn layout_storage_arguments(
            &self,
            reference: TypeReferenceHandle,
        ) -> Option<LayoutStorageArguments> {
            let mut application = self
                .type_reference_table
                .unconstrained_type_reference(reference);
            if let TypeReferenceNode::Named { symbol, .. } =
                self.type_reference_table.type_reference(application)
            {
                application = self.data_definition_by_symbol(*symbol)?.generic_instance?;
            }
            let TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } = self.type_reference_table.type_reference(application)
            else {
                return None;
            };
            if !self.core_layout_declaration(
                *base_symbol,
                language_core::sealed_intrinsics::SealedIntrinsic::LayoutStorageData,
            ) || !lifetime_arguments.is_empty()
            {
                return None;
            }
            let constructor = self.data_definition_by_symbol(*base_symbol)?;
            let [semantic, policy] = self.data_type_parameters(constructor).as_array()?;
            if semantic.kind != crate::data::TypeParameterKind::Type
                || policy.kind != crate::data::TypeParameterKind::Implementation
            {
                return None;
            }
            let [bound] = constructor.conformance_bounds.as_slice() else {
                return None;
            };
            if bound.binder != policy.symbol
                || !bound.lifetime_arguments.is_empty()
                || bound.arguments.len() != 1
            {
                return None;
            }
            let contract = self.trait_definition_by_symbol(bound.trait_symbol)?;
            if contract.is_boundary
                || contract.name.as_str() != "FixedLayout"
                || self.symbols.symbol_source_origin(contract.symbol)
                    != Some(source::SourceOrigin::Toolchain)
                || self.symbols.symbol_source_span(contract.symbol)?.source_id
                    != self
                        .symbols
                        .symbol_source_span(constructor.symbol)?
                        .source_id
                || self.type_reference_table.type_symbol(bound.arguments[0]) != semantic.symbol
            {
                return None;
            }
            let [semantic_type, policy] =
                self.type_reference_table.type_reference_handles(*arguments)
            else {
                return None;
            };
            Some(LayoutStorageArguments {
                application,
                semantic_type: *semantic_type,
                policy: *policy,
                contract: contract.symbol,
            })
        }

        pub fn plan_laid_layout_for_type(
            &self,
            reference: TypeReferenceHandle,
        ) -> Option<&crate::PlanLaidLayout> {
            let reference = self.type_reference_table.peeled_type_reference(reference);
            let identity = self.normalized_type_identity(reference);
            self.plan_laid_layouts.iter().find(|plan| {
                plan.application.is_valid()
                    && self.normalized_type_identity(plan.application) == identity
            })
        }
    }

    impl crate::PlanLaidLayout {
        pub fn native_semantic_identity(&self, program: &TypedTrees) -> Result<String, String> {
            use crate::type_identity::{IndexIdentityArgument, portable_applied_index_identity};
            let owner = program
                .data_implementation_applications
                .iter()
                .find_map(|(_, demand)| {
                    (program.normalized_type_identity(demand.source_application)
                        == program.normalized_type_identity(self.application))
                    .then_some(demand.lexical_owner)
                })
                .unwrap_or_else(|| {
                    program
                        .type_reference_table
                        .operand_lifetime_owner(self.semantic_type)
                });
            let identity = portable_applied_index_identity(
                program,
                IndexIdentityArgument::TypeArgument(self.semantic_type),
                owner,
                &[],
            )
            .ok_or("native layout lost its complete semantic argument identity")?;
            program
                .type_identity_text(identity)
                .ok_or("native layout lost its semantic argument coordinate".into())
        }

        pub fn native_policy_identity(&self, program: &TypedTrees) -> Result<String, String> {
            use crate::type_identity::{IndexIdentityArgument, portable_applied_index_identity};
            let storage = program
                .layout_storage_arguments(self.application)
                .ok_or("native layout lost its exact constructor application")?;
            let selected = program.static_operand_from_type_reference(storage.policy)?;
            let owner = program
                .data_implementation_applications
                .iter()
                .find_map(|(_, demand)| {
                    (program.normalized_type_identity(demand.source_application)
                        == program.normalized_type_identity(self.application))
                    .then_some(demand.lexical_owner)
                })
                .unwrap_or_else(|| {
                    program
                        .type_reference_table
                        .operand_lifetime_owner(storage.policy)
                });
            let identity = portable_applied_index_identity(
                program,
                IndexIdentityArgument::StaticArgument(&selected),
                owner,
                &[],
            )
            .ok_or("native layout has no complete selected implementation identity")?;
            program
                .type_identity_text(identity)
                .ok_or("native layout lost its selected implementation coordinate".into())
        }

        pub fn native_data_identity(
            &self,
            program: &TypedTrees,
            data: &crate::data::DataDefinition,
            policy: &str,
        ) -> Result<String, String> {
            program.native_data_declaration_identity(data).or_else(|_| {
                program
                    .normalized_hermetic_symbol_identity(self.schema_symbol)
                    .map(|schema| format!("closed-plan-laid-data::{policy}::{schema}"))
            })
        }
    }
}
pub mod measure {
    use crate::types::TypeReferenceHandle;
    use arena::HandleSpan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct MeasureDefinition {
        pub symbol: SymbolHandle,
        pub name: HandleSpan<crate::name::Identifier>,
        pub parameter: Option<crate::signature::StateParameter>,
        pub return_type: TypeReferenceHandle,
        pub lexicographic: bool,
        pub body: HandleSpan<crate::expression::ExpressionHandle>,
    }
}
pub mod numeric_intrinsics {
    use crate::TypedTrees;
    use crate::expression::BinaryOperator;
    use crate::operator::OperatorDefinition;
    use crate::signature::StateParameter;
    use crate::types::PrimitiveType;
    use crate::types::TypeReferenceHandle;
    use language_semantics::numeric_intrinsics::{
        NamedNumericIntrinsic, NumericIntrinsic, NumericResult, NumericType,
        PrimitiveFloatBinaryOperation, PrimitiveIntegerComparisonOperation,
    };

    pub const fn primitive_numeric_type(primitive: PrimitiveType) -> Option<NumericType> {
        Some(match primitive {
            PrimitiveType::Integer(carrier) => NumericType::Integer(carrier),
            PrimitiveType::F32 => NumericType::F32,
            PrimitiveType::F64 => NumericType::F64,
            PrimitiveType::Bool | PrimitiveType::Addr | PrimitiveType::Selector => return None,
        })
    }

    impl TypedTrees {
        pub fn numeric_intrinsic_execution(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<NumericIntrinsic> {
            if !symbol.is_valid() {
                return None;
            }
            let operator = self.operator_definition_by_symbol(symbol);
            if operator.is_some_and(|operator| !operator.is_boundary) {
                return None;
            }
            let machine = self.machine(symbol).or_else(|| {
                self.machine_by_entry_state_symbol(symbol)
                    .map(|(machine, _)| machine)
            });
            if let Some(machine) = machine {
                if machine.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
                    || machine.body_is_present
                    || !self.machine_type_parameters(machine).is_empty()
                {
                    return None;
                }
                let [entry] = self.machine_states(machine) else {
                    return None;
                };
                let (namespace, name) = machine.name.as_str().rsplit_once("::")?;
                if namespace.is_empty() || name.is_empty() || namespace.contains("::") {
                    return None;
                }
                return self.numeric_requirement_execution(
                    namespace,
                    name,
                    self.state_parameters(entry),
                    entry.return_type,
                    self.machine_token_binding_by_symbol(machine.symbol),
                );
            }
            let operator = operator?;
            let [namespace, name] = self.operator_path_members(operator.name) else {
                return None;
            };
            self.numeric_requirement_execution(
                namespace.as_str(),
                name.as_str(),
                self.operator_parameters(operator),
                operator.return_type,
                Some(operator),
            )
        }

        fn numeric_requirement_execution(
            &self,
            namespace: &str,
            name: &str,
            parameters: &[StateParameter],
            return_type: TypeReferenceHandle,
            operator: Option<&OperatorDefinition>,
        ) -> Option<NumericIntrinsic> {
            if let Some(execution) =
                operator.and_then(|operator| self.numeric_token_execution(operator))
            {
                return Some(execution);
            }
            let intrinsic = NamedNumericIntrinsic::resolve(
                namespace,
                name,
                self.type_reference_table.arithmetic_domain(return_type),
            )?;
            let result = match self.primitive_type_reference(return_type) {
                Some(PrimitiveType::Bool) => Some(NumericResult::Boolean),
                Some(primitive) => primitive_numeric_type(primitive).map(NumericResult::Primitive),
                None => (self.display_type_reference(return_type) == "FloatClass")
                    .then_some(NumericResult::FloatClass),
            };
            intrinsic
                .accepts(
                    parameters.iter().map(|parameter| {
                        self.primitive_type_reference(parameter.type_reference)
                            .and_then(primitive_numeric_type)
                    }),
                    result,
                )
                .then_some(intrinsic.execution)
        }

        fn numeric_token_execution(
            &self,
            operator: &OperatorDefinition,
        ) -> Option<NumericIntrinsic> {
            if let Some((operation, format)) =
                crate::operator::primitive_float_binary_semantics(self, operator)
            {
                use PrimitiveFloatBinaryOperation as Operation;
                let operation = match operation {
                    BinaryOperator::Add => Operation::Add,
                    BinaryOperator::Subtract => Operation::Subtract,
                    BinaryOperator::Multiply => Operation::Multiply,
                    BinaryOperator::Divide => Operation::Divide,
                    BinaryOperator::Equal => Operation::Equal,
                    BinaryOperator::NotEqual => Operation::NotEqual,
                    BinaryOperator::Less => Operation::Less,
                    BinaryOperator::LessOrEqual => Operation::LessOrEqual,
                    BinaryOperator::Greater => Operation::Greater,
                    BinaryOperator::GreaterOrEqual => Operation::GreaterOrEqual,
                    _ => return None,
                };
                return Some(NumericIntrinsic::FloatBinary { operation, format });
            }
            use PrimitiveIntegerComparisonOperation as Comparison;
            use language_core::operator_spelling::OperatorSpelling;
            let operation = match operator.spelling {
                Some(OperatorSpelling::Equal) => Comparison::Equal,
                Some(OperatorSpelling::NotEqual) => Comparison::NotEqual,
                Some(OperatorSpelling::Less) => Comparison::Less,
                Some(OperatorSpelling::LessEqual) => Comparison::LessOrEqual,
                Some(OperatorSpelling::Greater) => Comparison::Greater,
                Some(OperatorSpelling::GreaterEqual) => Comparison::GreaterOrEqual,
                _ => return None,
            };
            let [left, right] = self.operator_parameters(operator) else {
                return None;
            };
            let primitive = self.primitive_type_reference(left.type_reference)?;
            if self.primitive_type_reference(right.type_reference) != Some(primitive)
                || self.primitive_type_reference(operator.return_type) != Some(PrimitiveType::Bool)
            {
                return None;
            }
            let integer_type = primitive_numeric_type(primitive)?;
            (!integer_type.is_float()).then_some(NumericIntrinsic::IntegerComparison {
                operation,
                integer_type,
            })
        }
    }
}
pub use crate::operator_declarations::operator;
pub use crate::product_and_trait_declarations::{product_operand, trait_definition};
pub mod visibility {
    use symbols::{SymbolHandle, SymbolKind};

    use crate::TypedTrees;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DeclarationVisibility {
        kind: &'static str,
        is_public: bool,
    }

    impl DeclarationVisibility {
        pub const fn kind(self) -> &'static str {
            self.kind
        }

        pub const fn is_public(self) -> bool {
            self.is_public
        }
    }

    pub const fn requires_declaration_visibility(kind: SymbolKind) -> bool {
        matches!(
            kind,
            SymbolKind::Data
                | SymbolKind::TypeAlias
                | SymbolKind::Conformance
                | SymbolKind::Domain
                | SymbolKind::Field
                | SymbolKind::Variant
                | SymbolKind::Machine
                | SymbolKind::Operator
                | SymbolKind::Measure
                | SymbolKind::Proposition
                | SymbolKind::State
                | SymbolKind::Trait
                | SymbolKind::Const
                | SymbolKind::WireSchema
        )
    }

    pub fn owning_machine_is_retained(program: &TypedTrees, symbol: SymbolHandle) -> bool {
        let mut current = symbol;
        while current.is_valid() {
            let entry = program.symbols.get(current);
            if entry.kind == SymbolKind::Machine {
                return program.machine(current).is_some();
            }
            current = entry.parent;
        }
        true
    }

    pub fn authored_selection_visibility(
        program: &TypedTrees,
        symbol: SymbolHandle,
    ) -> Option<DeclarationVisibility> {
        if !symbol.is_valid() {
            return None;
        }
        if let Some(application) = program.closed_requirement_application(symbol) {
            if !application.application.is_valid() || application.template == symbol {
                return None;
            }
            let (owner, _) = program.trait_machine_signature(application.template)?;
            let source = program
                .expression_table
                .static_machine_application(application.application);
            if owner.symbol != application.declaring_trait
                || source.registration_operation != application.template
                || source.binding_declaration != application.template
            {
                return None;
            }
            return declaration_visibility(program, application.template);
        }
        let mut declaration = symbol;
        if program.symbols.get(declaration).kind == SymbolKind::State {
            declaration = program.symbols.get(declaration).parent;
        }
        if !declaration.is_valid() {
            return None;
        }
        if program.symbols.get(declaration).kind != SymbolKind::Machine {
            return declaration_visibility(program, symbol);
        }
        let origin_count =
            program.machine_specializations.len() + program.captured_machine_applications.len();
        for _ in 0..=origin_count {
            let mut origins = program
                .machine_specializations
                .for_instance(declaration)
                .map(|specialization| specialization.template)
                .chain(
                    program
                        .captured_machine_applications
                        .iter()
                        .filter(|(_, capture)| capture.instance == declaration)
                        .map(|(_, capture)| capture.template),
                );
            let Some(template) = origins.next() else {
                return authored_machine_visibility(program, declaration);
            };
            if origins.next().is_some() {
                return None;
            }
            if template == declaration {
                return authored_machine_visibility(program, declaration);
            }
            if !template.is_valid() || program.symbols.get(template).kind != SymbolKind::Machine {
                return None;
            }
            declaration = template;
        }
        None
    }

    fn authored_machine_visibility(
        program: &TypedTrees,
        machine: SymbolHandle,
    ) -> Option<DeclarationVisibility> {
        let owner = program.symbols.get(machine).declaration_owner;
        if owner.is_valid() && program.symbols.get(owner).kind == SymbolKind::Conformance {
            let (implementation, _) = program.owned_conformance_realization(machine)?;
            let visibility = declaration_visibility(program, implementation.symbol)?;
            return Some(DeclarationVisibility {
                kind: "machine",
                is_public: visibility.is_public(),
            });
        }
        declaration_visibility(program, machine)
    }

    pub fn declaration_visibility(
        program: &TypedTrees,
        symbol: SymbolHandle,
    ) -> Option<DeclarationVisibility> {
        let kind = program.symbols.get(symbol).kind;
        macro_rules! retained_visibility {
            ($table:ident, $kind:literal) => {
                consistent_visibility(
                    $kind,
                    program
                        .tables
                        .$table
                        .iter()
                        .filter(|(_, declaration)| declaration.symbol == symbol)
                        .map(|(_, declaration)| declaration.is_public),
                )
            };
        }
        let direct = match kind {
            SymbolKind::Data => retained_visibility!(data_definitions, "data"),
            SymbolKind::TypeAlias => retained_visibility!(type_aliases, "type alias"),
            SymbolKind::Conformance => retained_visibility!(conformances, "conformance"),
            SymbolKind::Domain => retained_visibility!(domain_definitions, "domain"),
            SymbolKind::Machine => retained_visibility!(machines, "machine"),
            SymbolKind::Operator => retained_visibility!(operators, "operator"),
            SymbolKind::Measure => program
                .tables
                .measures
                .iter()
                .any(|(_, declaration)| declaration.symbol == symbol)
                .then_some(DeclarationVisibility {
                    kind: "measure",
                    is_public: false,
                }),
            SymbolKind::Proposition => retained_visibility!(propositions, "proposition"),
            SymbolKind::Trait => retained_visibility!(traits, "trait"),
            SymbolKind::Const => retained_visibility!(const_declarations, "const"),
            SymbolKind::WireSchema => retained_visibility!(wire_schemas, "wire schema"),
            SymbolKind::Field | SymbolKind::Variant | SymbolKind::State => None,
            _ => return None,
        };
        if direct.is_some() {
            return direct;
        }

        if matches!(
            kind,
            SymbolKind::Field | SymbolKind::Variant | SymbolKind::State
        ) {
            let mut parent = program.symbols.get(symbol).parent;
            while parent.is_valid()
                && matches!(
                    program.symbols.get(parent).kind,
                    SymbolKind::ConformanceParameter
                        | SymbolKind::MachineParameter
                        | SymbolKind::PropositionMachineParameter
                )
            {
                parent = program.symbols.get(parent).parent;
            }
            if parent.is_valid() {
                return declaration_visibility(program, parent);
            }
        }
        None
    }

    fn consistent_visibility(
        kind: &'static str,
        mut copies: impl Iterator<Item = bool>,
    ) -> Option<DeclarationVisibility> {
        let is_public = copies.next()?;
        copies
            .all(|copy| copy == is_public)
            .then_some(DeclarationVisibility { kind, is_public })
    }

    #[cfg(test)]
    mod tests {
        use super::{consistent_visibility, declaration_visibility, owning_machine_is_retained};
        use crate::{TypedTrees, machine::Machine};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn projected_machine_visibility_does_not_restore_execution_membership() {
            for is_public in [false, true] {
                let mut builder = SymbolTableBuilder::default();
                let machine =
                    builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
                let state = builder
                    .insert_children(
                        machine,
                        [(SymbolKind::State, SymbolNameRef::Static("entry"))],
                    )
                    .start();
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                let declaration = Machine {
                    symbol: machine,
                    is_public,
                    ..Machine::default()
                };
                program.tables.machines.insert(declaration.clone());
                program.tables.machines.insert(declaration);
                let visibility = declaration_visibility(&program, state)
                    .expect("the exact backing owner retains visibility");
                assert_eq!(visibility.kind(), "machine");
                assert_eq!(visibility.is_public(), is_public);
                assert!(!owning_machine_is_retained(&program, state));
            }
        }

        #[test]
        fn transparent_alias_visibility_requires_exact_consistent_retained_headers() {
            use crate::type_aliases::TypeAliasDefinition;
            for is_public in [false, true] {
                let mut builder = SymbolTableBuilder::default();
                let symbol =
                    builder.insert_root(SymbolKind::TypeAlias, SymbolNameRef::Static("Alias"));
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                assert!(super::requires_declaration_visibility(
                    SymbolKind::TypeAlias
                ));
                assert!(declaration_visibility(&program, symbol).is_none());
                let header = TypeAliasDefinition {
                    symbol,
                    is_public,
                    ..Default::default()
                };
                program.tables.type_aliases.insert(header.clone());
                program.tables.type_aliases.insert(header);
                let visibility = super::authored_selection_visibility(&program, symbol).unwrap();
                assert_eq!(visibility.kind(), "type alias");
                assert_eq!(visibility.is_public(), is_public);
                program.tables.type_aliases.insert(TypeAliasDefinition {
                    symbol,
                    is_public: !is_public,
                    ..Default::default()
                });
                assert!(declaration_visibility(&program, symbol).is_none());
                assert!(super::authored_selection_visibility(&program, symbol).is_none());
            }
        }

        #[test]
        fn capture_and_invocation_selection_keep_authored_member_visibility() {
            use crate::{CapturedMachineApplication, MachineSpecialization};
            for is_public in [false, true] {
                let mut builder = SymbolTableBuilder::default();
                let template =
                    builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("member"));
                let captured =
                    builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("captured"));
                let invoked =
                    builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("invoked"));
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                for (symbol, is_public) in
                    [(template, is_public), (captured, false), (invoked, false)]
                {
                    program.push_machine(Machine {
                        symbol,
                        is_public,
                        ..Default::default()
                    });
                }
                program
                    .captured_machine_applications
                    .insert(CapturedMachineApplication {
                        template,
                        instance: captured,
                        ..Default::default()
                    });
                program.machine_specializations.push(MachineSpecialization {
                    template: captured,
                    instance: invoked,
                    ..Default::default()
                });
                for symbol in [captured, invoked] {
                    assert!(
                        !declaration_visibility(&program, symbol)
                            .unwrap()
                            .is_public()
                    );
                    assert_eq!(
                        super::authored_selection_visibility(&program, symbol)
                            .unwrap()
                            .is_public(),
                        is_public
                    );
                }
                program
                    .captured_machine_applications
                    .insert(CapturedMachineApplication {
                        template,
                        instance: invoked,
                        ..Default::default()
                    });
                assert!(super::authored_selection_visibility(&program, invoked).is_none());
            }
        }

        #[test]
        fn owned_member_selection_uses_map_visibility_without_exporting_helpers() {
            use crate::CapturedMachineApplication;
            use crate::trait_definition::{Conformance, ConformanceRow, ConformanceRowSource};

            for is_public in [false, true] {
                for source in [
                    ConformanceRowSource::Inline,
                    ConformanceRowSource::TraitDefault,
                    ConformanceRowSource::Synthesized,
                    ConformanceRowSource::Reference,
                ] {
                    let mut builder = SymbolTableBuilder::default();
                    let owner = builder
                        .insert_root(SymbolKind::Conformance, SymbolNameRef::Static("Selected"));
                    let member =
                        builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("member"));
                    let children = builder.insert_children(
                        member,
                        [
                            (SymbolKind::State, SymbolNameRef::Static("entry")),
                            (SymbolKind::Machine, SymbolNameRef::Static("helper")),
                        ],
                    );
                    let state = children.start();
                    let nested = children.handle_at(1);
                    let instance =
                        builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("captured"));
                    let mut program = TypedTrees {
                        symbols: builder.finish(),
                        ..Default::default()
                    };
                    if source.owns_realization() {
                        program
                            .symbols
                            .record_declaration_owner(member, owner)
                            .unwrap();
                    }
                    for symbol in [member, nested, instance] {
                        program.push_machine(Machine {
                            symbol,
                            ..Default::default()
                        });
                    }
                    let row = ConformanceRow {
                        declaring_trait: symbols::SymbolHandle::invalid(),
                        declaring_trait_name: Default::default(),
                        requirement: symbols::SymbolHandle::invalid(),
                        requirement_name: Default::default(),
                        realization_machine: member,
                        realization_state: state,
                        realization_name: Default::default(),
                        source,
                    };
                    program.push_conformance(Conformance {
                        symbol: owner,
                        is_public,
                        rows: vec![row.clone()],
                        ..Default::default()
                    });
                    program
                        .captured_machine_applications
                        .insert(CapturedMachineApplication {
                            template: member,
                            instance,
                            ..Default::default()
                        });
                    for symbol in [member, state, instance] {
                        assert!(
                            !declaration_visibility(&program, symbol)
                                .unwrap()
                                .is_public()
                        );
                        assert_eq!(
                            super::authored_selection_visibility(&program, symbol)
                                .unwrap()
                                .is_public(),
                            source.owns_realization() && is_public,
                        );
                    }
                    assert!(
                        !super::authored_selection_visibility(&program, nested)
                            .unwrap()
                            .is_public()
                    );

                    if source.owns_realization() {
                        program
                            .tables
                            .conformances
                            .get_mut(program.roots.conformances.start())
                            .rows
                            .push(row);
                    } else {
                        program
                            .symbols
                            .record_declaration_owner(member, owner)
                            .unwrap();
                    }
                    assert!(super::authored_selection_visibility(&program, member).is_none());
                }
            }
        }

        #[test]
        fn conflicting_retained_visibility_never_selects_a_public_copy() {
            assert!(consistent_visibility("machine", [].into_iter()).is_none());
            assert!(consistent_visibility("machine", [false, true].into_iter()).is_none());
            assert!(consistent_visibility("machine", [true, false].into_iter()).is_none());
        }
    }
}
pub mod wire {

    use crate::name::Identifier;
    use crate::types;
    use arena::HandleSpan;
    use symbols::SymbolHandle;
    use types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum WirePlacement {
        Varint { tag: u64 },
        LengthPrefixed { tag: u64 },
    }

    impl Default for WirePlacement {
        fn default() -> Self {
            Self::Varint { tag: 0 }
        }
    }

    impl WirePlacement {
        pub fn tag(self) -> u64 {
            match self {
                Self::Varint { tag } | Self::LengthPrefixed { tag } => tag,
            }
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireSchemaPlan {
        pub schema: SymbolHandle,
        pub placements: HandleSpan<WirePlacement>,
        pub encode_obligations: HandleSpan<WireEncodeObligation>,
        pub policy_verified: bool,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireEncodeObligation {
        pub field_number: u64,
        pub element: WireScalarEncoding,
        pub length: WireEncodeLengthObligation,
        pub work: WireEncodeWorkObligation,
        pub output_capacity: WireEncodeOutputCapacityObligation,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum WireEncodeLengthObligation {
        #[default]
        RuntimeElementCount,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum WireEncodeWorkObligation {
        #[default]
        TwoPassesPerElement,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum WireEncodeOutputCapacityObligation {
        #[default]
        ExactPackedPayload,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireSchema {
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub is_public: bool,
        pub encoding: Option<Identifier>,
        pub members: HandleSpan<WireMember>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum WireMember {
        Field(WireField),
        Reserved(WireReserved),
        Version(WireVersion),
    }

    impl Default for WireMember {
        fn default() -> Self {
            Self::Reserved(WireReserved::default())
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireField {
        pub number: u64,
        pub name: Identifier,
        pub relevance: language_core::BindingRelevance,
        pub type_reference: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireReserved {
        pub number: u64,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireVersion {
        pub name: Identifier,
        pub members: HandleSpan<WireMember>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct WireScalarEncoding {
        pub byte_size: usize,
        pub zigzag: bool,
    }
}

pub mod type_aliases {
    use crate::data::TypeParameter;
    use crate::machine::GenericConformanceBound;
    use crate::name::Identifier;
    use crate::types::TypeReferenceHandle;
    use arena::HandleSpan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TypeAliasDefinition {
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub is_public: bool,
        pub lifetime_parameters: Vec<Identifier>,
        pub type_parameters: HandleSpan<TypeParameter>,
        pub conformance_bounds: Vec<GenericConformanceBound>,
        pub expansion: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TypeAliasApplication {
        pub instance: TypeReferenceHandle,
        pub application: TypeReferenceHandle,
        pub declaration: SymbolHandle,
        pub declaration_source: source::SourceSpan,
        pub reference: source::SourceSpan,
        pub arguments: HandleSpan<TypeReferenceHandle>,
        pub lifetime_arguments: Vec<Identifier>,
        pub supplied_operand_owner: SymbolHandle,
    }
}
