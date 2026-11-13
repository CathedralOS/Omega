/*
We represent typed proof propositions, mathematical definitions, byte predicates,
and termination ranking evidence (`evidence`) alongside deterministic phase
inspection snapshots (`inspection`) for `TypedTrees`.
*/

pub mod evidence {
    pub mod byte_predicates {
        use crate::TypedTrees;
        use crate::expression::ExpressionHandle;
        use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
        use symbols::SymbolHandle;

        pub use language_semantics::byte_predicates::ByteSequencePredicate;

        pub fn byte_predicate_call(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<(ByteSequencePredicate, ExpressionHandle)> {
            let call = program.expression_table.call_node(expression)?;
            if call.target_symbol.is_valid() || call.receiver.is_valid() {
                return None;
            }
            let predicate = ByteSequencePredicate::from_name(call.target.as_str())?;
            let arguments = program.expression_table.expression_handles(call.arguments);
            let [argument] = arguments else {
                return None;
            };
            Some((predicate, *argument))
        }

        pub fn domain_byte_predicate(
            program: &TypedTrees,
            domain_symbol: SymbolHandle,
        ) -> Option<ByteSequencePredicate> {
            let domain = program.domain_definition(domain_symbol)?;
            if domain.alias.is_some()
                || !domain.index_arguments.is_empty()
                || !domain.establishment_routes.is_empty()
                || !crate::domain::index_parameters(program, domain).is_empty()
            {
                return None;
            }
            let [crate::domain::ProofFact::Expression(expression)] =
                program.proof_facts.span_or_empty(domain.facts)
            else {
                return None;
            };
            let (predicate, argument) = byte_predicate_call(program, *expression)?;
            crate::domain::domain_subject_reference(program, domain, argument).then_some(predicate)
        }

        pub fn type_reference_domain_predicates(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Vec<(String, Option<ByteSequencePredicate>)> {
            let mut predicates = Vec::new();
            let mut handle = type_reference;
            loop {
                if !handle.is_valid() {
                    return predicates;
                }
                handle = program
                    .type_reference_table
                    .dereferenced_type_reference(handle);
                match program.type_reference_table.type_reference(handle) {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        for constraint in program.type_reference_table.constraints(*constraints) {
                            let TypeConstraintNode::Domain(domain_constraint) = constraint else {
                                continue;
                            };
                            let predicate = domain_constraint
                                .symbol
                                .is_valid()
                                .then(|| domain_byte_predicate(program, domain_constraint.symbol))
                                .flatten();
                            predicates
                                .push((domain_constraint.name.as_str().to_owned(), predicate));
                        }
                        handle = *base_type;
                    }
                    _ => return predicates,
                }
            }
        }

        pub fn literal_bytes(
            program: &TypedTrees,
            expression: ExpressionHandle,
        ) -> Option<std::borrow::Cow<'_, [u8]>> {
            match program.expression_table.expression(expression) {
                crate::expression::ExpressionNode::String(bytes) => {
                    Some(std::borrow::Cow::Borrowed(bytes))
                }
                crate::expression::ExpressionNode::ArrayLiteral(elements) => {
                    let bytes = program
                        .expression_table
                        .expression_handles(*elements)
                        .iter()
                        .map(|element| {
                            let crate::expression::ExpressionNode::Integer(integer) =
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
        use crate::data::TypeParameter;
        use crate::expression::ExpressionHandle;
        use crate::name::Identifier;
        use crate::types::TypeReferenceHandle;
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct MathematicalDefinition {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub is_public: bool,
            pub binders: HandleSpan<TypeParameter>,
            pub parameters: HandleSpan<MathematicalParameter>,
            pub result: MathematicalTypeHandle,
            pub body: MathematicalBody,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct MathematicalParameter {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub relevance: language_core::BindingRelevance,
            pub ty: MathematicalTypeHandle,
        }

        pub type MathematicalTypeHandle = Handle<MathematicalType>;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum MathematicalBody {
            #[default]
            Assumption,
            Definition(ExpressionHandle),
        }
    }
    pub mod proof_only {
        use crate::TypedTrees;
        use crate::data::DataDefinition;
        use crate::name::Identifier;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use std::collections::HashMap;
        use symbols::SymbolHandle;

        /// One erased scalar lane: the field path below an erased binding (empty
        /// for a primitive or a shared reference to one) and the leaf's type.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ErasedScalarLane {
            pub fields: Vec<SymbolHandle>,
            pub primitive_type: crate::types::PrimitiveType,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ProofOnlyReason {
            Integer,
            Quotient { carrier: Identifier },
            Recursive,
            Contains { field: Identifier, held: Identifier },
        }

        #[derive(Debug, Default)]
        pub struct ProofOnlyClassification {
            reasons: HashMap<u32, ProofOnlyReason>,
        }

        impl ProofOnlyClassification {
            pub fn is_proof_only(&self, symbol: SymbolHandle) -> bool {
                self.reasons.contains_key(&symbol.arena_index())
            }

            pub fn reason(&self, symbol: SymbolHandle) -> Option<&ProofOnlyReason> {
                self.reasons.get(&symbol.arena_index())
            }

            pub fn describe(&self, name: &str, symbol: SymbolHandle) -> Option<String> {
                Some(match self.reasons.get(&symbol.arena_index())? {
                    ProofOnlyReason::Integer => {
                        format!(
                            "`{name}` is proof-only: mathematical integers have no runtime layout"
                        )
                    }
                    ProofOnlyReason::Quotient { carrier } => {
                        format!(
                            "`{name}` is proof-only: quotient carrier holds proof-only `{carrier}`"
                        )
                    }
                    ProofOnlyReason::Recursive => {
                        format!("`{name}` is proof-only: recursive data has no layout")
                    }
                    ProofOnlyReason::Contains { field, held } => {
                        format!("`{name}` is proof-only: field `{field}` holds proof-only `{held}`")
                    }
                })
            }

            pub fn proof_only_mention(
                &self,
                program: &TypedTrees,
                type_reference: TypeReferenceHandle,
            ) -> Option<Identifier> {
                proof_only_mention_in(&self.reasons, program, type_reference)
            }

            pub fn contract_term_carrier(
                &self,
                program: &TypedTrees,
                type_reference: TypeReferenceHandle,
            ) -> bool {
                if self.proof_only_mention(program, type_reference).is_some() {
                    return true;
                }
                // A record whose leaves all ride erased scalar lanes is carried
                // there, leaf by leaf, and owns no contract-term position.
                if !program.erased_scalar_lanes(type_reference).is_empty() {
                    return false;
                }
                let mut visiting = Vec::new();
                self.closed_contract_carrier(program, type_reference, &mut visiting)
            }

            fn closed_contract_carrier(
                &self,
                program: &TypedTrees,
                type_reference: TypeReferenceHandle,
                visiting: &mut Vec<SymbolHandle>,
            ) -> bool {
                if !type_reference.is_valid() {
                    return false;
                }
                if let TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } = program.type_reference_table.type_reference(type_reference)
                {
                    return matches!(length, crate::types::FixedArrayLength::Literal(_))
                        && self.contract_term_field(program, *element_type, visiting);
                }
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(type_reference)
                else {
                    return false;
                };
                if visiting.contains(symbol) {
                    return false;
                }
                let Some(definition) = program.data_definition_by_symbol(*symbol) else {
                    return false;
                };
                let shape =
                    DataDefinition::shape_kind_from_members(program.data_members(definition));
                if !program.closed_contract_nominal(definition)
                    || !matches!(
                        shape,
                        crate::data::DataShapeKind::Empty
                            | crate::data::DataShapeKind::Record
                            | crate::data::DataShapeKind::Enum
                            | crate::data::DataShapeKind::Mixed
                    )
                {
                    return false;
                }
                visiting.push(*symbol);
                let admitted = program
                    .data_member_fields(program.data_members(definition))
                    .all(|field| self.contract_term_field(program, field.type_reference, visiting));
                visiting.pop();
                admitted
            }

            fn contract_term_field(
                &self,
                program: &TypedTrees,
                type_reference: TypeReferenceHandle,
                visiting: &mut Vec<SymbolHandle>,
            ) -> bool {
                program.primitive_type_reference(type_reference).is_some()
                    || self.proof_only_mention(program, type_reference).is_some()
                    || self.closed_contract_carrier(program, type_reference, visiting)
            }
        }

        fn proof_only_mention_in(
            reasons: &HashMap<u32, ProofOnlyReason>,
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Option<Identifier> {
            if !type_reference.is_valid() {
                return None;
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { symbol, name } => reasons
                    .contains_key(&symbol.arena_index())
                    .then(|| name.clone()),
                TypeReferenceNode::Reference { referee, .. } => {
                    proof_only_mention_in(reasons, program, *referee)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    proof_only_mention_in(reasons, program, *base_type)
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    proof_only_mention_in(reasons, program, *element_type)
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    arguments,
                    ..
                } => {
                    if reasons.contains_key(&base_symbol.arena_index()) {
                        return Some(base_name.clone());
                    }
                    program
                        .type_reference_handles(*arguments)
                        .iter()
                        .find_map(|argument| proof_only_mention_in(reasons, program, *argument))
                }
                TypeReferenceNode::Callable { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => None,
            }
        }

        impl ProofOnlyClassification {
            pub fn is_proof_machine(
                &self,
                program: &TypedTrees,
                machine: &crate::machine::Machine,
            ) -> bool {
                let mut has_proof_value = false;
                for state in program.machine_states(machine) {
                    let parameters = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| !parameter.relevance.is_erased())
                        .map(|parameter| (parameter.type_reference, parameter.is_mutable));
                    let result = std::iter::once((state.return_type, false));
                    let locals = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .filter_map(|statement| match statement {
                            crate::statement::StatementNode::LocalData(local)
                                if !local.relevance.is_erased() =>
                            {
                                Some((local.type_reference, false))
                            }
                            _ => None,
                        });
                    for (type_reference, is_mutable_parameter) in
                        parameters.chain(result).chain(locals)
                    {
                        if self.proof_only_mention(program, type_reference).is_none() {
                            continue;
                        }
                        if is_mutable_parameter
                            || self.borrows_proof_only_data(program, type_reference)
                        {
                            return false;
                        }
                        has_proof_value = true;
                    }
                }
                has_proof_value
            }

            pub fn is_proof_signature(
                &self,
                program: &TypedTrees,
                signature: &crate::signature::StateSignature,
            ) -> bool {
                let mut has_proof_value = false;
                let parameters = program
                    .state_signature_parameters(signature)
                    .iter()
                    .filter(|parameter| !parameter.relevance.is_erased())
                    .map(|parameter| (parameter.type_reference, parameter.is_mutable));
                for (reference, mutable) in
                    parameters.chain(std::iter::once((signature.return_type, false)))
                {
                    if self.proof_only_mention(program, reference).is_none() {
                        continue;
                    }
                    if mutable || self.borrows_proof_only_data(program, reference) {
                        return false;
                    }
                    has_proof_value = true;
                }
                has_proof_value
            }

            fn borrows_proof_only_data(
                &self,
                program: &TypedTrees,
                type_reference: TypeReferenceHandle,
            ) -> bool {
                if !type_reference.is_valid() {
                    return false;
                }
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Reference { referee, .. } => {
                        self.proof_only_mention(program, *referee).is_some()
                    }
                    TypeReferenceNode::Slice { element_type } => {
                        self.proof_only_mention(program, *element_type).is_some()
                    }
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        self.borrows_proof_only_data(program, *base_type)
                    }
                    TypeReferenceNode::FixedArray { element_type, .. } => {
                        self.borrows_proof_only_data(program, *element_type)
                    }
                    TypeReferenceNode::Generic { arguments, .. } => program
                        .type_reference_handles(*arguments)
                        .iter()
                        .any(|argument| self.borrows_proof_only_data(program, *argument)),
                    TypeReferenceNode::Callable { .. }
                    | TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::ConstExpression(_)
                    | TypeReferenceNode::DynamicTrait { .. }
                    | TypeReferenceNode::Unit => false,
                }
            }
        }

        pub fn classify(program: &TypedTrees) -> ProofOnlyClassification {
            let definitions = program.data_definitions();
            let proof_integer = program
                .symbols
                .builtin_type_symbol(symbols::BuiltinType::Int);
            let index_by_symbol: HashMap<u32, usize> = definitions
                .iter()
                .enumerate()
                .map(|(index, definition)| (definition.symbol.arena_index(), index))
                .collect();

            let mut edges: Vec<Vec<(usize, Identifier, Identifier)>> =
                vec![Vec::new(); definitions.len()];
            let mut integer_containment = vec![None; definitions.len()];
            for (index, definition) in definitions.iter().enumerate() {
                for member in program.data_members(definition) {
                    let fields = program.data_member_fields(std::slice::from_ref(member));
                    for field in fields {
                        if field.relevance == language_core::BindingRelevance::Erased {
                            continue;
                        }
                        collect_inline_data_edges(
                            program,
                            field.type_reference,
                            &mut |target, held| {
                                if Some(target) == proof_integer {
                                    integer_containment[index] =
                                        Some((field.name.clone(), held.clone()));
                                }
                                if let Some(target) = index_by_symbol.get(&target.arena_index()) {
                                    edges[index].push((*target, field.name.clone(), held));
                                }
                            },
                        );
                    }
                }
            }

            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                for (index, definition) in definitions.iter().enumerate() {
                    eprintln!(
                        "CLASSIFY def[{index}] {} symbol={} edges={:?}",
                        definition.name,
                        definition.symbol.arena_index(),
                        edges[index]
                            .iter()
                            .map(|(target, field, held)| (
                                definitions[*target].name.as_str(),
                                field.as_str(),
                                held.as_str()
                            ))
                            .collect::<Vec<_>>()
                    );
                }
            }

            let mut reasons: HashMap<u32, ProofOnlyReason> = HashMap::new();
            if let Some(symbol) = proof_integer {
                reasons.insert(symbol.arena_index(), ProofOnlyReason::Integer);
            }
            for (index, contained) in integer_containment.into_iter().enumerate() {
                if let Some((field, held)) = contained {
                    reasons.insert(
                        definitions[index].symbol.arena_index(),
                        ProofOnlyReason::Contains { field, held },
                    );
                }
            }

            for (start, definition) in definitions.iter().enumerate() {
                if reaches(start, start, &edges) {
                    reasons.insert(definition.symbol.arena_index(), ProofOnlyReason::Recursive);
                }
            }

            loop {
                let mut changed = false;
                for (index, definition) in definitions.iter().enumerate() {
                    if reasons.contains_key(&definition.symbol.arena_index()) {
                        continue;
                    }
                    let contained = edges[index].iter().find(|(target, _, _)| {
                        reasons.contains_key(&definitions[*target].symbol.arena_index())
                    });
                    if let Some((_, field, held)) = contained {
                        reasons.insert(
                            definition.symbol.arena_index(),
                            ProofOnlyReason::Contains {
                                field: field.clone(),
                                held: held.clone(),
                            },
                        );
                        changed = true;
                        continue;
                    }
                    if let Some(quotient) = definition.quotient.as_ref()
                        && let Some(carrier) =
                            proof_only_mention_in(&reasons, program, quotient.carrier)
                    {
                        reasons.insert(
                            definition.symbol.arena_index(),
                            ProofOnlyReason::Quotient { carrier },
                        );
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }

            if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                for definition in definitions {
                    eprintln!(
                        "CLASSIFY reason {} symbol={} -> {:?}",
                        definition.name,
                        definition.symbol.arena_index(),
                        reasons.get(&definition.symbol.arena_index()),
                    );
                }
            }

            ProofOnlyClassification { reasons }
        }

        fn reaches(
            goal: usize,
            from: usize,
            edges: &[Vec<(usize, Identifier, Identifier)>],
        ) -> bool {
            let mut visited = vec![false; edges.len()];
            let mut stack: Vec<usize> = edges[from].iter().map(|(target, _, _)| *target).collect();
            while let Some(node) = stack.pop() {
                if node == goal {
                    return true;
                }
                if std::mem::replace(&mut visited[node], true) {
                    continue;
                }
                stack.extend(edges[node].iter().map(|(target, _, _)| *target));
            }
            false
        }

        fn collect_inline_data_edges(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
            edge: &mut impl FnMut(SymbolHandle, Identifier),
        ) {
            if !type_reference.is_valid() {
                return;
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { symbol, name } => {
                    if symbol.is_valid() {
                        edge(*symbol, name.clone());
                    }
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    collect_inline_data_edges(program, *base_type, edge)
                }
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    collect_inline_data_edges(program, *element_type, edge)
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    arguments,
                    ..
                } => {
                    if base_symbol.is_valid() {
                        edge(*base_symbol, base_name.clone());
                    }
                    for argument in program.type_reference_handles(*arguments) {
                        collect_inline_data_edges(program, *argument, edge);
                    }
                }
                TypeReferenceNode::Callable { .. }
                | TypeReferenceNode::Reference { .. }
                | TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::Unit => {}
            }
        }
    }
    pub mod proposition {
        use crate::data::DataProperties;
        use crate::expression::ExpressionHandle;
        use crate::name::Identifier;
        use crate::type_identity::TypeIdentityRequest;
        use crate::types::TypeReferenceHandle;
        use arena::HandleSpan;
        use symbols::JoinedPath;
        use symbols::SymbolHandle;

        pub(crate) mod constructor_values {
            use crate::data::DataMember;
            use crate::expression::{ExpressionHandle, TableStructLiteralField};
            use symbols::SymbolHandle;

            pub(crate) fn render(
                program: &crate::TypedTrees,
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
                let owner = program
                    .normalized_hermetic_symbol_identity(owner)
                    .unwrap_or_else(|_| program.symbols.display_path(owner, "::"));
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

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct PropositionDefinition {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub is_public: bool,
            pub binders: HandleSpan<PropositionBinder>,
            pub parameters: HandleSpan<crate::signature::StateParameter>,
            pub transparent_formula_source_span: Option<source::SourceSpan>,
            pub body: PropositionBody,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct PropositionBinder {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub kind: PropositionBinderKind,
            pub bounds: DataProperties,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum PropositionBinderKind {
            #[default]
            Type,
            Const {
                type_reference: TypeReferenceHandle,
            },
            Machine,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
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

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum PropositionFormula {
            Application(PropositionApplication),
            BooleanExpression(ExpressionHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct PropositionApplication {
            pub proposition: SymbolHandle,
            pub name: Identifier,
            pub binder_arguments: Box<[PropositionBinderArgument]>,
            pub arguments: HandleSpan<ExpressionHandle>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct PropositionBinderArgument {
            pub kind: PropositionBinderArgumentKind,
            pub path: Box<[Identifier]>,
            pub const_literal: Option<numerics::literals::IntegerLiteral>,
            pub evidence_projection: Option<crate::expression::EvidenceProjection>,
            pub symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
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
            program: &crate::TypedTrees,
            evidence: TypeReferenceHandle,
            binders: &[PropositionBinder],
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
                    crate::types::TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..]),
                    crate::types::TypeReferenceNode::Generic {
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
                    ),
                }),
            ))
        }

        fn normalized_evidence_requirements(
            program: &crate::TypedTrees,
            root: SymbolHandle,
            root_arguments: &[String],
        ) -> Vec<NormalizedEvidenceRequirementIdentity> {
            fn collect(
                program: &crate::TypedTrees,
                trait_symbol: SymbolHandle,
                trait_arguments: &[String],
                visited: &mut Vec<(SymbolHandle, Vec<String>)>,
                requirements: &mut Vec<NormalizedEvidenceRequirementIdentity>,
            ) {
                let visit = (trait_symbol, trait_arguments.to_vec());
                if visited.contains(&visit) {
                    return;
                }
                visited.push(visit);
                let Some(definition) = program.trait_definition_by_symbol(trait_symbol) else {
                    return;
                };
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
                            program.type_identity_request_text(TypeIdentityRequest {
                                binders: &substitutions,
                                ..TypeIdentityRequest::ordinary(*argument)
                            })
                        })
                        .collect::<Vec<_>>();
                    collect(
                        program,
                        parent.symbol,
                        &parent_arguments,
                        visited,
                        requirements,
                    );
                }
            }

            let mut requirements = Vec::new();
            collect(
                program,
                root,
                root_arguments,
                &mut Vec::new(),
                &mut requirements,
            );
            requirements.sort_by_key(|requirement| {
                (
                    requirement.declaring_trait.arena_index(),
                    requirement.requirement.arena_index(),
                    requirement.declaring_trait_arguments.clone(),
                )
            });
            requirements
        }

        impl crate::TypedTrees {
            pub fn type_reference_mentions_any(
                &self,
                reference: TypeReferenceHandle,
                symbols: &[SymbolHandle],
            ) -> bool {
                type_reference_mentions_any(self, reference, symbols)
            }
        }

        fn type_reference_mentions_any(
            program: &crate::TypedTrees,
            type_reference: TypeReferenceHandle,
            symbols: &[SymbolHandle],
        ) -> bool {
            let mut references = FreeInputMentions::new(program, symbols);
            crate::references::visit_type_reference(program, &mut references, type_reference);
            references.mentioned
        }

        struct FreeInputMentions<'program, 'query> {
            program: &'program crate::TypedTrees,
            symbols: &'query [SymbolHandle],
            mentioned: bool,
            scopes: Vec<Vec<SymbolHandle>>,
            pending_binders: Vec<SymbolHandle>,
            types: std::collections::HashSet<(TypeReferenceHandle, Vec<SymbolHandle>)>,
            expressions: std::collections::HashSet<(ExpressionHandle, Vec<SymbolHandle>)>,
        }

        impl<'program, 'query> FreeInputMentions<'program, 'query> {
            fn new(program: &'program crate::TypedTrees, symbols: &'query [SymbolHandle]) -> Self {
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

        impl crate::references::TypedReferences for FreeInputMentions<'_, '_> {
            fn callable_signature(&mut self, signature: &crate::signature::StateSignature) {
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
            program: &crate::TypedTrees,
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
            program: &crate::TypedTrees,
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
            program: &crate::TypedTrees,
            type_reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, String)],
        ) -> String {
            use crate::types::TypeReferenceNode;

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
                TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => program.display_type_reference(type_reference),
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct NormalizedEvidenceInterfaceIdentity {
            pub trait_symbol: SymbolHandle,
            pub arguments: Vec<crate::type_identity::NormalizedTypeIdentity>,
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

        impl crate::TypedTrees {
            pub fn normalize_nominal_proposition_application(
                &self,
                application: &PropositionApplication,
                labels: Option<PropositionLabels<'_>>,
            ) -> Option<NormalizedPropositionApplicationIdentity> {
                let rendered;
                let labels = match labels {
                    Some(labels) => labels,
                    None => {
                        rendered = self.proposition_labels(application);
                        rendered.borrow()
                    }
                };
                let (binder_labels, argument_labels) =
                    (labels.binder_labels, labels.argument_labels);
                let binder_identities =
                    binder_argument_identities_for_labels(self, application, binder_labels)?;
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
            ) -> Option<NormalizedPropositionApplicationIdentity> {
                if visiting.contains(&application.proposition) {
                    return None;
                }
                let declaration = self.proposition_definition_by_symbol(application.proposition)?;
                let binders = self.proposition_binders(declaration);
                let parameters = self.proposition_parameters(declaration);
                if binders.len() != binder_labels.len()
                    || binders.len() != binder_identities.len()
                    || parameters.len() != argument_labels.len()
                {
                    return None;
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
                        Some(endpoint(PropositionEvidenceClassification::FactOnly))
                    }
                    PropositionBody::Witness { evidence } => {
                        let (evidence, interface) = normalize_evidence_interface(
                            self,
                            *evidence,
                            binders,
                            binder_labels,
                            binder_identities,
                        )?;
                        Some(endpoint(PropositionEvidenceClassification::Witness {
                            evidence,
                            interface,
                        }))
                    }
                    PropositionBody::Transparent {
                        proposition: PropositionFormula::BooleanExpression(_),
                    } => None,
                    PropositionBody::Transparent {
                        proposition: PropositionFormula::Application(expansion),
                    } => {
                        visiting.push(application.proposition);
                        let (expanded_binders, expanded_binder_identities, expanded_arguments) =
                            expanded_application_parts(
                                self,
                                expansion,
                                &substitutions,
                                binders,
                                binder_identities,
                            );
                        let normalized = self.normalize_nominal_proposition_application_inner(
                            expansion,
                            &expanded_binders,
                            &expanded_arguments,
                            &expanded_binder_identities,
                            visiting,
                        );
                        visiting.pop();
                        normalized
                    }
                }
            }

            pub fn render_proof_constructor_value(
                &self,
                literal: &crate::expression::TableStructLiteral,
                render_value: impl Fn(ExpressionHandle) -> String,
            ) -> String {
                constructor_values::render(
                    self,
                    literal.type_symbol,
                    literal.case_symbol,
                    self.expression_table.struct_fields(literal.fields),
                    render_value,
                )
                .unwrap_or_else(|| format!("invalid-constructor:{}", literal.type_name))
            }

            pub fn render_proof_expression(
                &self,
                expression: ExpressionHandle,
                substitutions: ProofSubstitutions<'_>,
            ) -> String {
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
            ) -> OwnedPropositionLabels {
                OwnedPropositionLabels {
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
                            self.render_proof_expression(*argument, ProofSubstitutions::None)
                        })
                        .collect(),
                }
            }

            pub fn normalize_proposition_application(
                &self,
                application: &PropositionApplication,
                labels: Option<PropositionLabels<'_>>,
            ) -> Option<NormalizedPropositionFormula> {
                let rendered;
                let labels = match labels {
                    Some(labels) => labels,
                    None => {
                        rendered = self.proposition_labels(application);
                        rendered.borrow()
                    }
                };
                let (binder_labels, argument_labels) =
                    (labels.binder_labels, labels.argument_labels);
                let binder_identities =
                    binder_argument_identities_for_labels(self, application, binder_labels)?;
                let mut visiting = Vec::new();
                self.normalize_proposition_application_inner(
                    application,
                    binder_labels,
                    argument_labels,
                    &binder_identities,
                    &mut visiting,
                )
            }

            pub fn normalized_identity_label(
                &self,
                application: &PropositionApplication,
                binder_labels: &[String],
                argument_labels: &[String],
            ) -> Option<String> {
                self.normalize_proposition_application(
                    application,
                    Some(PropositionLabels {
                        binder_labels,
                        argument_labels,
                    }),
                )
                .map(|formula| formula.identity_label())
            }

            fn normalize_proposition_application_inner(
                &self,
                application: &PropositionApplication,
                binder_labels: &[String],
                argument_labels: &[String],
                binder_identities: &[Option<String>],
                visiting: &mut Vec<SymbolHandle>,
            ) -> Option<NormalizedPropositionFormula> {
                if visiting.contains(&application.proposition) {
                    return None;
                }
                let declaration = self.proposition_definition_by_symbol(application.proposition);
                if declaration.is_none()
                    && self.symbols.get(application.proposition).kind
                        == symbols::SymbolKind::PropositionParameter
                {
                    let parameter_count = self
                        .data_type_parameters
                        .iter()
                        .map(|(_, parameter)| parameter)
                        .find_map(|parameter| match &parameter.kind {
                            crate::data::TypeParameterKind::Proposition { contract }
                                if parameter.symbol == application.proposition =>
                            {
                                Some(contract.parameters.len())
                            }
                            _ => None,
                        })?;
                    if !binder_labels.is_empty() || parameter_count != argument_labels.len() {
                        return None;
                    }
                    return Some(NormalizedPropositionFormula::Proposition {
                        label: canonical_application_label(
                            application.name.as_str(),
                            binder_labels,
                            argument_labels,
                        ),
                        classification: PropositionEvidenceClassification::FactOnly,
                    });
                }
                let declaration = declaration?;
                let binders = self.proposition_binders(declaration);
                let parameters = self.proposition_parameters(declaration);
                if binders.len() != binder_labels.len()
                    || binders.len() != binder_identities.len()
                    || parameters.len() != argument_labels.len()
                {
                    return None;
                }

                let mut substitutions = Vec::new();
                for (binder, label) in binders.iter().zip(binder_labels) {
                    substitutions.push((binder.symbol, label.clone()));
                }
                for (parameter, label) in parameters.iter().zip(argument_labels) {
                    substitutions.push((parameter.symbol, label.clone()));
                }

                match &declaration.body {
                    PropositionBody::Primitive => Some(NormalizedPropositionFormula::Proposition {
                        label: canonical_application_label(
                            declaration.name.as_str(),
                            binder_labels,
                            argument_labels,
                        ),
                        classification: PropositionEvidenceClassification::FactOnly,
                    }),
                    PropositionBody::Witness { evidence } => {
                        let (evidence, interface) = normalize_evidence_interface(
                            self,
                            *evidence,
                            binders,
                            binder_labels,
                            binder_identities,
                        )?;
                        Some(NormalizedPropositionFormula::Proposition {
                            label: canonical_application_label(
                                declaration.name.as_str(),
                                binder_labels,
                                argument_labels,
                            ),
                            classification: PropositionEvidenceClassification::Witness {
                                evidence,
                                interface,
                            },
                        })
                    }
                    PropositionBody::Transparent { proposition } => {
                        visiting.push(application.proposition);
                        let normalized = match proposition {
                            PropositionFormula::BooleanExpression(expression) => {
                                Some(NormalizedPropositionFormula::Boolean {
                                    label: render_expression(
                                        self,
                                        *expression,
                                        &substitutions,
                                        &[],
                                    ),
                                })
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
                                );
                                self.normalize_proposition_application_inner(
                                    expansion,
                                    &expanded_binders,
                                    &expanded_arguments,
                                    &expanded_binder_identities,
                                    visiting,
                                )
                            }
                        };
                        visiting.pop();
                        normalized
                    }
                }
            }
        }

        fn expanded_application_parts(
            program: &crate::TypedTrees,
            expansion: &PropositionApplication,
            substitutions: &[(SymbolHandle, String)],
            binders: &[PropositionBinder],
            binder_identities: &[Option<String>],
        ) -> (Vec<String>, Vec<Option<String>>, Vec<String>) {
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
                .collect::<Vec<_>>();
            (
                expanded_binders,
                expanded_binder_identities,
                expanded_arguments,
            )
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

        fn render_expression(
            program: &crate::TypedTrees,
            expression: ExpressionHandle,
            substitutions: &[(SymbolHandle, String)],
            name_substitutions: &[(String, String)],
        ) -> String {
            use crate::expression::ExpressionNode;
            let render = |expression| {
                render_expression(program, expression, substitutions, name_substitutions)
            };
            match program.expression_table.expression(expression) {
                ExpressionNode::MachineValue(value) => {
                    crate::type_identity::normalized_expression_identity(
                        program,
                        expression,
                        substitutions,
                    )
                    .unwrap_or_else(|| format!("<invalid machine value {value:?}>"))
                }
                ExpressionNode::Match(dispatch) => {
                    let arms = program
                        .expression_table
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| {
                            let pattern = crate::expression::display_match_pattern(
                                &program.expression_table,
                                arm.pattern,
                                render,
                            );
                            format!("{pattern} => {}", render(arm.value))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("match {} {{ {arms} }}", render(dispatch.subject))
                }
                ExpressionNode::SavedSubject(saved) => format!(
                    "<saved subject #{}:{}>",
                    saved.capture.arena_index(),
                    saved.capture.generation()
                ),
                ExpressionNode::Atomic(atomic) => {
                    format!("atomic[{:?}]({})", atomic.ordering, render(atomic.value))
                }
                ExpressionNode::ArrayLiteral(values) => format!(
                    "[{}]",
                    program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .map(|value| render(*value))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                ExpressionNode::Binary(binary) => format!(
                    "{} {} {}",
                    render(binary.left),
                    binary.operator.display_name(),
                    render(binary.right)
                ),
                ExpressionNode::Boolean(value) => value.to_string(),
                ExpressionNode::Cast(cast) => format!(
                    "({} as {})",
                    render(cast.value),
                    crate::expression::display_name_path(
                        program
                            .expression_table
                            .name_path_members(cast.target_label),
                        "::",
                    )
                ),
                ExpressionNode::Call(call) => {
                    crate::expression::display_call_label(&program.expression_table, call, render)
                }
                ExpressionNode::Float(value) => value.to_string(),
                ExpressionNode::UndeterminedNaN(format) => {
                    format!("<undetermined NaN {}>", format.name())
                }
                ExpressionNode::Indexed(indexed) => {
                    format!("{}[{}]", render(indexed.collection), render(indexed.index))
                }
                ExpressionNode::Integer(value) => value.to_string(),
                ExpressionNode::Member(member) => {
                    format!("{}.{}", render(member.receiver), member.member)
                }
                ExpressionNode::Borrow(inner) => format!(
                    "{} {}",
                    match inner.access {
                        language_core::ReferenceAccess::Mutable => "mut",
                        language_core::ReferenceAccess::WriteOnly => "write",
                        language_core::ReferenceAccess::Shared => "shared",
                    },
                    render(inner.target)
                ),
                ExpressionNode::Name(path) => {
                    if let Some((_, replacement)) = substitutions
                        .iter()
                        .find(|(symbol, _)| *symbol == path.symbol || *symbol == path.head_symbol)
                    {
                        return replacement.clone();
                    }
                    let members = program.expression_table.name_path_members(path.members);
                    if let Some(first) = members.first()
                        && let Some((_, replacement)) = name_substitutions
                            .iter()
                            .find(|(name, _)| name == first.as_str())
                    {
                        return replacement.clone();
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
                        && let Some(label) = constructor_values::render(
                            program,
                            program.symbols.get(path.symbol).parent,
                            Some(path.symbol),
                            &[],
                            render,
                        )
                    {
                        return label;
                    }
                    crate::expression::display_name_path(members, "::")
                }
                ExpressionNode::Range(range) => {
                    match (range.start.is_valid(), range.end.is_valid()) {
                        (true, true) => format!("{}..{}", render(range.start), render(range.end)),
                        (true, false) => format!("{}..", render(range.start)),
                        (false, true) => format!("..{}", render(range.end)),
                        (false, false) => "..".to_owned(),
                    }
                }
                ExpressionNode::String(value) => format!("{value:?}"),
                ExpressionNode::StructLiteral(literal) => {
                    program.render_proof_constructor_value(literal, render)
                }
                ExpressionNode::TypeExpression(reference) => {
                    program.display_type_reference(*reference)
                }
                ExpressionNode::ZeroValue(_) => program.expression_table.display_name(expression),
                ExpressionNode::Unary(unary) => {
                    format!("{}{}", unary.operator.display_name(), render(unary.operand))
                }
            }
        }
    }
    pub mod ranking {
        use crate::TypedTrees;
        use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use crate::machine::Machine;
        use crate::state::State;
        use symbols::JoinedPath;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct RankingExpressionCustody {
            pub machine: symbols::SymbolHandle,
            pub subjects: Vec<ExpressionHandle>,
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

        pub fn resolve_machine_witness_view_arguments(
            program: &TypedTrees,
            machine: &Machine,
        ) -> Option<Vec<ExpressionHandle>> {
            let witness = machine.termination_plan.implementation_witness.as_ref()?;
            if let Some(custody) = program.ranking_expression_custody_for(machine.symbol) {
                return (custody.view_arguments.len() == witness.view_arguments.len())
                    .then(|| custody.view_arguments.clone());
            }
            resolve_machine_witness_expressions(program, machine, &witness.view_arguments)
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

pub mod inspection {
    pub mod callees {
        use crate::TypedTrees;
        use crate::expression::ExpressionHandle;
        use crate::state::State;
        use symbols::SymbolHandle;

        #[derive(Default)]
        pub struct ExplicitStateCallees {
            visited: Vec<ExpressionHandle>,
            called: Vec<SymbolHandle>,
        }

        impl ExplicitStateCallees {
            pub fn collect(&mut self, typed: &TypedTrees, state: &State) -> &[SymbolHandle] {
                self.called.clear();
                self.visited.clear();
                for statement in typed.statement_table.statements(state.statement_nodes) {
                    collect_statement_callees(
                        typed,
                        statement,
                        &mut self.visited,
                        &mut self.called,
                    );
                }
                &self.called
            }
        }

        fn note_callee(called: &mut Vec<symbols::SymbolHandle>, symbol: symbols::SymbolHandle) {
            if symbol.is_valid() && !called.contains(&symbol) {
                called.push(symbol);
            }
        }

        fn collect_statement_callees(
            typed: &TypedTrees,
            statement: &crate::statement::StatementNode,
            visited: &mut Vec<crate::expression::ExpressionHandle>,
            called: &mut Vec<symbols::SymbolHandle>,
        ) {
            use crate::statement::StatementNode;
            match statement {
                StatementNode::Assembly(_) => {}
                StatementNode::Call(call) => {
                    if let Some(binding) = call.runtime_call {
                        collect_expression_callees(typed, binding.callee, visited, called);
                    }
                    note_callee(called, call.target_symbol);
                    for argument in typed.statement_table.expression_handles(call.arguments) {
                        collect_expression_callees(typed, *argument, visited, called);
                    }
                }
                StatementNode::Assignment(assignment) => {
                    collect_expression_callees(typed, assignment.target, visited, called);
                    collect_expression_callees(typed, assignment.value, visited, called);
                }
                StatementNode::Expression(expression) => {
                    collect_expression_callees(typed, *expression, visited, called);
                }
                StatementNode::LocalData(local) => {
                    collect_expression_callees(typed, local.initial_value, visited, called);
                }
                StatementNode::AssemblyFact(fact) => {
                    for expression in typed.tables.proof_fact_expression_roots(fact.fact) {
                        collect_expression_callees(typed, expression, visited, called);
                    }
                }
                StatementNode::Transition(transition) => {
                    if let crate::statement::TransitionGuardNode::When(guard) = &transition.guard {
                        collect_expression_callees(typed, *guard, visited, called);
                    }
                    for target in transition.targets() {
                        collect_transition_target_callees(typed, target, visited, called);
                    }
                }
            }
        }

        fn collect_transition_target_callees(
            typed: &TypedTrees,
            target: crate::statement::TransitionTargetHandle,
            visited: &mut Vec<crate::expression::ExpressionHandle>,
            called: &mut Vec<symbols::SymbolHandle>,
        ) {
            use crate::statement::TransitionTargetNode;
            if !typed.statement_table.transition_target_is_valid(target) {
                return;
            }
            match typed.statement_table.transition_target(target) {
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    note_callee(called, path.symbol);
                    for argument in typed.statement_table.expression_handles(*arguments) {
                        collect_expression_callees(typed, *argument, visited, called);
                    }
                }
                TransitionTargetNode::Value(expression) => {
                    collect_expression_callees(typed, *expression, visited, called);
                }
                TransitionTargetNode::Terminal => {}
            }
        }

        fn collect_expression_callees(
            typed: &TypedTrees,
            expression: crate::expression::ExpressionHandle,
            visited: &mut Vec<crate::expression::ExpressionHandle>,
            called: &mut Vec<symbols::SymbolHandle>,
        ) {
            let edges = crate::SubtreeEdges {
                atomic_results: true,
                pattern_dependencies: true,
                ..crate::SubtreeEdges::VALUES
            };
            typed.subtree_any(expression, edges, |expression, node| {
                if visited.contains(&expression) {
                    return crate::SubtreeVisit::Prune;
                }
                visited.push(expression);
                if let crate::expression::ExpressionNode::Call(call) = node {
                    note_callee(called, call.target_symbol);
                }
                crate::SubtreeVisit::Descend
            });
        }
    }
    pub mod snapshot {
        pub(crate) mod declaration_snapshots {
            use super::to_strings;
            use crate::TypedTrees;
            use crate::data::{DataDefinition, DataMember};
            use crate::domain::{DomainDefinition, ProofFact};
            use crate::inspection::snapshot;
            use crate::mathematical::{MathematicalBody, MathematicalType};
            use crate::measure::MeasureDefinition;
            use crate::operator::OperatorDefinition;
            use crate::proposition::{
                PropositionBinderKind, PropositionBody, PropositionDefinition, PropositionFormula,
            };
            use serde::Serialize;
            use snapshot::machine_snapshots::state_parameter_snapshot;
            use snapshot::statement_and_expression_snapshots::{
                expression_snapshot, expression_span_snapshot,
            };
            use snapshot::type_snapshots::type_reference_snapshot;
            use snapshot::{
                ExpressionSnapshot, SourceSpanSnapshot, StateParameterSnapshot,
                TypeReferenceSnapshot,
            };

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct ConstDeclarationSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub is_public: bool,
                pub declared_type: TypeReferenceSnapshot,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub evaluated_value: Option<EvaluatedConstantSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub enum EvaluatedConstantSnapshot {
                Encoded(String),
                UndeterminedNaN { format: String },
            }

            impl From<&language_core::constant_value::EvaluatedConstant> for EvaluatedConstantSnapshot {
                fn from(value: &language_core::constant_value::EvaluatedConstant) -> Self {
                    match value {
                        language_core::constant_value::EvaluatedConstant::Encoded(encoding) => {
                            Self::Encoded(encoding.clone())
                        }
                        language_core::constant_value::EvaluatedConstant::UndeterminedNaN(
                            format,
                        ) => Self::UndeterminedNaN {
                            format: format.name().to_owned(),
                        },
                    }
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct OperatorDefinitionSnapshot {
                pub is_public: bool,
                pub is_boundary: bool,
                pub has_symbol: bool,
                pub name: Vec<String>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub lifetime_parameters: Vec<String>,
                pub type_parameters: Vec<String>,
                pub parameter_count: usize,
                pub has_return_type: bool,
                pub contract_count: usize,
                pub spelling: Option<&'static str>,
                pub token_count: usize,
            }

            pub(crate) fn operator_snapshot(
                program: &TypedTrees,
                operator: &OperatorDefinition,
            ) -> OperatorDefinitionSnapshot {
                OperatorDefinitionSnapshot {
                    is_public: operator.is_public,
                    is_boundary: operator.is_boundary,
                    has_symbol: operator.symbol.is_valid(),
                    name: to_strings(program.operator_path_members(operator.name).iter()),
                    lifetime_parameters: to_strings(operator.lifetime_parameters.iter()),
                    type_parameters: program
                        .data_type_parameters
                        .span_or_empty(operator.type_parameters)
                        .iter()
                        .map(|parameter| parameter.name.to_string())
                        .collect(),
                    parameter_count: program
                        .state_parameters
                        .span_or_empty(operator.parameters)
                        .len(),
                    has_return_type: operator.return_type.is_valid(),
                    contract_count: program
                        .signature_contracts
                        .span_or_empty(operator.contracts)
                        .len(),
                    spelling: operator.spelling.map(|spelling| spelling.symbol()),
                    token_count: operator.token_count,
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct PropositionSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub is_public: bool,
                pub binders: Vec<PropositionBinderSnapshot>,
                pub parameters: Vec<StateParameterSnapshot>,
                pub body: PropositionBodySnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct PropositionBinderSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub kind: &'static str,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub const_type: Option<TypeReferenceSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum PropositionBodySnapshot {
                Primitive,
                Witness { evidence: TypeReferenceSnapshot },
                Transparent { formula: PropositionFormulaSnapshot },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum PropositionFormulaSnapshot {
                Application {
                    proposition_symbol: u32,
                    name: String,
                    binder_arguments: Vec<Vec<String>>,
                    arguments: Vec<ExpressionSnapshot>,
                },
                BooleanExpression {
                    expression: ExpressionSnapshot,
                },
            }

            pub(crate) fn proposition_snapshot(
                program: &TypedTrees,
                proposition: &PropositionDefinition,
            ) -> PropositionSnapshot {
                PropositionSnapshot {
                    has_symbol: proposition.symbol.is_valid(),
                    name: proposition.name.to_string(),
                    is_public: proposition.is_public,
                    binders: program
                        .proposition_binders(proposition)
                        .iter()
                        .map(|binder| {
                            let (kind, const_type) = match binder.kind {
                                PropositionBinderKind::Type => ("type", None),
                                PropositionBinderKind::Const { type_reference } => (
                                    "const",
                                    Some(type_reference_snapshot(program, type_reference)),
                                ),
                                PropositionBinderKind::Machine => ("machine", None),
                            };
                            PropositionBinderSnapshot {
                                has_symbol: binder.symbol.is_valid(),
                                name: binder.name.to_string(),
                                kind,
                                const_type,
                            }
                        })
                        .collect(),
                    parameters: program
                        .proposition_parameters(proposition)
                        .iter()
                        .map(|parameter| state_parameter_snapshot(program, parameter))
                        .collect(),
                    body: match &proposition.body {
                        PropositionBody::Primitive => PropositionBodySnapshot::Primitive,
                        PropositionBody::Witness { evidence } => PropositionBodySnapshot::Witness {
                            evidence: type_reference_snapshot(program, *evidence),
                        },
                        PropositionBody::Transparent { proposition } => {
                            PropositionBodySnapshot::Transparent {
                                formula: match proposition {
                                    PropositionFormula::Application(application) => {
                                        proposition_formula_application_snapshot(
                                            program,
                                            application,
                                        )
                                    }
                                    PropositionFormula::BooleanExpression(expression) => {
                                        PropositionFormulaSnapshot::BooleanExpression {
                                            expression: expression_snapshot(program, *expression),
                                        }
                                    }
                                },
                            }
                        }
                    },
                }
            }

            fn proposition_formula_application_snapshot(
                program: &TypedTrees,
                application: &crate::proposition::PropositionApplication,
            ) -> PropositionFormulaSnapshot {
                PropositionFormulaSnapshot::Application {
                    proposition_symbol: application.proposition.arena_index(),
                    name: application.name.to_string(),
                    binder_arguments: application
                        .binder_arguments
                        .iter()
                        .map(|argument| vec![argument.display_name()])
                        .collect(),
                    arguments: program
                        .expression_table
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|argument| expression_snapshot(program, *argument))
                        .collect(),
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DataDefinitionSnapshot {
                pub name: String,
                pub is_public: bool,
                pub supply: String,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub lifetime_parameters: Vec<String>,
                pub type_parameters: Vec<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub predicate_subject: Option<DataPredicateSubjectSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub generic_instance: Option<TypeReferenceSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub conformance_bounds:
                    Vec<super::machine_snapshots::GenericConformanceBoundSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub cleanup_selection: Option<CleanupSelectionSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub quotient: Option<QuotientDefinitionSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub retired_identities: Vec<u64>,
                pub members: Vec<DataMemberSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DataPredicateSubjectSnapshot {
                pub carrier: TypeReferenceSnapshot,
                pub owned_by_data: bool,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct QuotientDefinitionSnapshot {
                pub carrier: TypeReferenceSnapshot,
                pub relation: Vec<String>,
                pub relation_symbol: u32,
                pub equivalence: Option<QuotientEquivalenceSelectionSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct QuotientEquivalenceSelectionSnapshot {
                pub relation: Vec<String>,
                pub relation_symbol: u32,
                pub trait_name: String,
                pub trait_symbol: u32,
                pub trait_arguments: Vec<TypeReferenceSnapshot>,
                pub conformance_name: String,
                pub conformance_symbol: u32,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum DataMemberSnapshot {
                Field {
                    identity: Option<u64>,
                    name: String,
                    relevance: &'static str,
                    type_reference: TypeReferenceSnapshot,
                },
                Variant {
                    identity: Option<u64>,
                    name: String,
                    payload: Vec<DataPayloadFieldSnapshot>,
                    retired_payload_identities: Vec<u64>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DataPayloadFieldSnapshot {
                pub identity: Option<u64>,
                pub name: String,
                pub relevance: &'static str,
                pub type_reference: TypeReferenceSnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct AppliedDomainSnapshot {
                pub semantic_id: u32,
                pub arguments: Vec<TypeReferenceSnapshot>,
                pub target_type: TypeReferenceSnapshot,
                pub facts: Vec<ProofFactSnapshot>,
                pub cleanup_selection: Option<CleanupSelectionSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DomainDefinitionSnapshot {
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub applied_instances: Vec<AppliedDomainSnapshot>,
                pub name: String,
                pub subject_name: String,
                pub has_subject_symbol: bool,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub cleanup_selection: Option<CleanupSelectionSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub type_parameters: Vec<DomainTypeParameterSnapshot>,
                pub target_type: TypeReferenceSnapshot,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub index_arguments: Vec<TypeReferenceSnapshot>,
                pub is_public: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub alias: Vec<DomainAliasConstituentSnapshot>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub classification: Option<&'static str>,
                pub predicate_presence: &'static str,
                pub semantic_id: u32,
                pub semantic_roles: DomainSemanticRolesSnapshot,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub establishment_routes: Vec<DomainEstablishmentRouteSnapshot>,
                pub facts: Vec<ProofFactSnapshot>,
                pub operators: Vec<OperatorDefinitionSnapshot>,
                pub semantic_clause_token_count: usize,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DomainTypeParameterSnapshot {
                pub name: String,
                pub kind: &'static str,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub const_type: Option<TypeReferenceSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DomainAliasConstituentSnapshot {
                pub domain: Vec<String>,
                pub domain_symbol: u32,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DomainSemanticRolesSnapshot {
                pub denotation_dimension: Option<u32>,
                pub arithmetic_policy: Option<u32>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DomainEstablishmentRouteSnapshot {
                pub kind: &'static str,
                pub source_symbol: u32,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub requirement_symbol: Option<u32>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum ProofFactSnapshot {
                Expression {
                    value: ExpressionSnapshot,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    source_span: Option<SourceSpanSnapshot>,
                },
                Membership {
                    value: ExpressionSnapshot,
                    domain: Vec<String>,
                    domain_symbol: u32,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    domain_arguments: Vec<TypeReferenceSnapshot>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    source_span: Option<SourceSpanSnapshot>,
                },
                Proposition {
                    proposition_symbol: u32,
                    name: String,
                    binder_arguments: Vec<Vec<String>>,
                    arguments: Vec<ExpressionSnapshot>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    source_span: Option<SourceSpanSnapshot>,
                },
            }

            pub(crate) fn data_definition_snapshot(
                program: &TypedTrees,
                data: &DataDefinition,
            ) -> DataDefinitionSnapshot {
                DataDefinitionSnapshot {
                    cleanup_selection: data
                        .cleanup_selection
                        .as_ref()
                        .map(|selection| cleanup_selection_snapshot(program, selection)),
                    name: data.name.to_string(),
                    is_public: data.is_public,
                    supply: match data.supply_mode {
                        language_semantics::DataSupplyMode::CheckedShape => "checked_shape",
                        language_semantics::DataSupplyMode::BoundaryOpaque => "boundary_opaque",
                    }
                    .to_owned(),
                    lifetime_parameters: to_strings(data.lifetime_parameters.iter()),
                    type_parameters: program
                        .data_type_parameters(data)
                        .iter()
                        .map(|parameter| parameter.name.to_string())
                        .collect(),
                    predicate_subject: (data.predicate_subject_symbol.is_valid()
                        && data.predicate_subject_type.is_valid())
                    .then(|| DataPredicateSubjectSnapshot {
                        carrier: type_reference_snapshot(program, data.predicate_subject_type),
                        owned_by_data: program.symbols.get(data.predicate_subject_symbol).parent
                            == data.symbol
                            && program.symbols.get(data.predicate_subject_symbol).kind
                                == symbols::SymbolKind::Parameter,
                    }),
                    conformance_bounds: data
                        .conformance_bounds
                        .iter()
                        .map(|bound| {
                            super::machine_snapshots::conformance_bound_snapshot(program, bound)
                        })
                        .collect(),
                    generic_instance: data
                        .generic_instance
                        .map(|origin| type_reference_snapshot(program, origin)),
                    quotient: data
                        .quotient
                        .as_ref()
                        .map(|quotient| QuotientDefinitionSnapshot {
                            carrier: type_reference_snapshot(program, quotient.carrier),
                            relation: to_strings(quotient.relation.iter()),
                            relation_symbol: quotient.relation_symbol.arena_index(),
                            equivalence: quotient.equivalence.as_ref().map(|selection| {
                                QuotientEquivalenceSelectionSnapshot {
                                    relation: to_strings(selection.relation.iter()),
                                    relation_symbol: selection.relation_symbol.arena_index(),
                                    trait_name: selection.trait_name.to_string(),
                                    trait_symbol: selection.trait_symbol.arena_index(),
                                    trait_arguments: program
                                        .type_reference_handles(selection.trait_arguments)
                                        .iter()
                                        .map(|argument| type_reference_snapshot(program, *argument))
                                        .collect(),
                                    conformance_name: selection.conformance_name.to_string(),
                                    conformance_symbol: selection.conformance_symbol.arena_index(),
                                }
                            }),
                        }),
                    retired_identities: data.retired_identities.clone(),
                    members: program
                        .data_members(data)
                        .iter()
                        .map(|member| data_member_snapshot(program, member))
                        .collect(),
                }
            }

            fn data_member_snapshot(
                program: &TypedTrees,
                member: &DataMember,
            ) -> DataMemberSnapshot {
                match member {
                    DataMember::Field(field) => DataMemberSnapshot::Field {
                        identity: field.identity,
                        name: field.name.to_string(),
                        relevance: snapshot_binding_relevance(field.relevance),
                        type_reference: type_reference_snapshot(program, field.type_reference),
                    },
                    DataMember::Variant(variant) => DataMemberSnapshot::Variant {
                        identity: variant.identity,
                        name: variant.name.to_string(),
                        payload: program
                            .data_payload_fields(variant)
                            .iter()
                            .map(|field| DataPayloadFieldSnapshot {
                                identity: field.identity,
                                name: field.name.to_string(),
                                relevance: snapshot_binding_relevance(field.relevance),
                                type_reference: type_reference_snapshot(
                                    program,
                                    field.type_reference,
                                ),
                            })
                            .collect(),
                        retired_payload_identities: variant.retired_payload_identities.clone(),
                    },
                }
            }

            pub(crate) fn snapshot_binding_relevance(
                relevance: language_core::BindingRelevance,
            ) -> &'static str {
                match relevance {
                    language_core::BindingRelevance::Relevant => "relevant",
                    language_core::BindingRelevance::Erased => "erased",
                }
            }

            pub(crate) fn domain_definition_snapshot(
                program: &TypedTrees,
                domain: &DomainDefinition,
            ) -> DomainDefinitionSnapshot {
                DomainDefinitionSnapshot {
                    applied_instances: program
                        .applied_domain_applications
                        .iter()
                        .filter(|application| application.declaration == domain.symbol)
                        .map(|application| AppliedDomainSnapshot {
                            semantic_id: application.theory.semantic_id.0,
                            arguments: application
                                .arguments
                                .iter()
                                .map(|argument| type_reference_snapshot(program, *argument))
                                .collect(),
                            target_type: type_reference_snapshot(
                                program,
                                application.theory.target_type,
                            ),
                            facts: domain_fact_snapshots(program, &application.theory),
                            cleanup_selection: application
                                .theory
                                .cleanup_selection
                                .as_ref()
                                .map(|selection| cleanup_selection_snapshot(program, selection)),
                        })
                        .collect(),
                    cleanup_selection: domain
                        .cleanup_selection
                        .as_ref()
                        .map(|selection| cleanup_selection_snapshot(program, selection)),
                    name: domain.name.to_string(),
                    subject_name: domain.subject_name.to_string(),
                    has_subject_symbol: domain.subject_symbol.is_valid(),
                    type_parameters: program
                        .domain_type_parameters(domain)
                        .iter()
                        .map(|parameter| DomainTypeParameterSnapshot {
                            name: parameter.name.to_string(),
                            kind: match parameter.kind {
                                crate::data::TypeParameterKind::Type => "type",
                                crate::data::TypeParameterKind::Implementation => "implementation",
                                crate::data::TypeParameterKind::Const { .. } => "const",
                                crate::data::TypeParameterKind::Value { .. } => "value",
                                crate::data::TypeParameterKind::Machine { .. } => "machine",
                                crate::data::TypeParameterKind::Proposition { .. } => "proposition",
                            },
                            const_type: match parameter.kind {
                                crate::data::TypeParameterKind::Const { type_reference }
                                | crate::data::TypeParameterKind::Value { type_reference } => {
                                    Some(type_reference_snapshot(program, type_reference))
                                }
                                crate::data::TypeParameterKind::Type
                                | crate::data::TypeParameterKind::Implementation
                                | crate::data::TypeParameterKind::Machine { .. }
                                | crate::data::TypeParameterKind::Proposition { .. } => None,
                            },
                        })
                        .collect(),
                    target_type: type_reference_snapshot(program, domain.target_type),
                    index_arguments: domain
                        .index_arguments
                        .iter()
                        .map(|argument| type_reference_snapshot(program, *argument))
                        .collect(),
                    is_public: domain.is_public,
                    alias: domain
                        .alias
                        .as_ref()
                        .map(|alias| {
                            alias
                                .constituents
                                .iter()
                                .map(|constituent| DomainAliasConstituentSnapshot {
                                    domain: to_strings(
                                        program.domain_path_members(constituent.domain).iter(),
                                    ),
                                    domain_symbol: constituent.domain_symbol.arena_index(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    classification: domain.classification.map(|value| value.as_str()),
                    predicate_presence: domain.predicate_presence.as_str(),
                    semantic_id: domain.semantic_id.0,
                    semantic_roles: semantic_roles_snapshot(domain.semantic_roles),
                    establishment_routes: domain
                        .establishment_routes
                        .iter()
                        .copied()
                        .map(establishment_route_snapshot)
                        .collect(),
                    facts: domain_fact_snapshots(program, domain),
                    operators: program
                        .domain_operators(domain)
                        .iter()
                        .map(|operator| operator_snapshot(program, operator))
                        .collect(),
                    semantic_clause_token_count: domain.semantic_clause_token_count,
                }
            }

            fn domain_fact_snapshots(
                program: &TypedTrees,
                domain: &DomainDefinition,
            ) -> Vec<ProofFactSnapshot> {
                fact_snapshots_for_span(program, domain.facts)
            }

            pub(crate) fn fact_snapshots_for_span(
                program: &TypedTrees,
                facts: arena::HandleSpan<ProofFact>,
            ) -> Vec<ProofFactSnapshot> {
                let facts_start = facts.start().arena_index();
                program
                    .proof_facts
                    .span_or_empty(facts)
                    .iter()
                    .enumerate()
                    .map(|(index, fact)| {
                        let source_span = program
                            .proof_fact_source_span(arena::Handle::from_arena_index(
                                facts_start + index as u32,
                            ))
                            .map(|span| super::source_span_snapshot(&span));
                        match fact {
                            ProofFact::Expression(expression) => ProofFactSnapshot::Expression {
                                value: expression_snapshot(program, *expression),
                                source_span,
                            },
                            ProofFact::Membership(membership) => ProofFactSnapshot::Membership {
                                value: expression_snapshot(program, membership.value),
                                domain: to_strings(
                                    program.domain_path_members(membership.domain).iter(),
                                ),
                                domain_symbol: membership.domain_symbol.arena_index(),
                                domain_arguments: program
                                    .type_reference_handles(membership.domain_arguments)
                                    .iter()
                                    .map(|argument| type_reference_snapshot(program, *argument))
                                    .collect(),
                                source_span,
                            },
                            ProofFact::Proposition(application) => ProofFactSnapshot::Proposition {
                                proposition_symbol: application.proposition.arena_index(),
                                name: application.name.to_string(),
                                binder_arguments: application
                                    .binder_arguments
                                    .iter()
                                    .map(|argument| vec![argument.display_name()])
                                    .collect(),
                                arguments: program
                                    .expression_table
                                    .expression_handles(application.arguments)
                                    .iter()
                                    .map(|argument| expression_snapshot(program, *argument))
                                    .collect(),
                                source_span,
                            },
                        }
                    })
                    .collect()
            }

            pub(crate) fn semantic_roles_snapshot(
                roles: language_semantics::DomainSemanticRoles,
            ) -> DomainSemanticRolesSnapshot {
                DomainSemanticRolesSnapshot {
                    denotation_dimension: roles.denotation_dimension.map(|semantic| semantic.0),
                    arithmetic_policy: roles.arithmetic_policy.map(|semantic| semantic.0),
                }
            }

            pub(crate) fn establishment_route_snapshot(
                route: language_semantics::DomainEstablishmentRoute,
            ) -> DomainEstablishmentRouteSnapshot {
                let requirement = route.requirement_symbol();
                DomainEstablishmentRouteSnapshot {
                    kind: route.kind_name(),
                    source_symbol: route.source_symbol().arena_index(),
                    requirement_symbol: requirement.is_valid().then(|| requirement.arena_index()),
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct MathematicalDefinitionSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub is_public: bool,
                pub binders: Vec<MathematicalBinderSnapshot>,
                pub parameters: Vec<MathematicalParameterSnapshot>,
                pub result: MathematicalTypeSnapshot,
                pub body: MathematicalBodySnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct MathematicalBinderSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub kind: &'static str,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub carrier: Option<TypeReferenceSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct MathematicalParameterSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub relevance: &'static str,
                pub ty: MathematicalTypeSnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum MathematicalTypeSnapshot {
                Ordinary {
                    reference: TypeReferenceSnapshot,
                },
                Arrow {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    binder: Option<String>,
                    domain: Box<MathematicalTypeSnapshot>,
                    codomain: Box<MathematicalTypeSnapshot>,
                },
                Application {
                    callee: Box<MathematicalTypeSnapshot>,
                    arguments: Vec<ExpressionSnapshot>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum MathematicalBodySnapshot {
                Assumption,
                Definition { term: ExpressionSnapshot },
            }

            fn mathematical_type_snapshot(
                program: &TypedTrees,
                handle: crate::mathematical::MathematicalTypeHandle,
            ) -> MathematicalTypeSnapshot {
                match program.mathematical_type(handle) {
                    MathematicalType::Ordinary(type_reference) => {
                        MathematicalTypeSnapshot::Ordinary {
                            reference: type_reference_snapshot(program, *type_reference),
                        }
                    }
                    MathematicalType::Arrow {
                        binder,
                        domain,
                        codomain,
                    } => MathematicalTypeSnapshot::Arrow {
                        binder: binder.as_ref().map(|binder| binder.to_string()),
                        domain: Box::new(mathematical_type_snapshot(program, *domain)),
                        codomain: Box::new(mathematical_type_snapshot(program, *codomain)),
                    },
                    MathematicalType::Application { callee, arguments } => {
                        MathematicalTypeSnapshot::Application {
                            callee: Box::new(mathematical_type_snapshot(program, *callee)),
                            arguments: program
                                .expression_table
                                .expression_handles(*arguments)
                                .iter()
                                .map(|argument| expression_snapshot(program, *argument))
                                .collect(),
                        }
                    }
                }
            }

            fn mathematical_binder_snapshot(
                program: &TypedTrees,
                parameter: &crate::data::TypeParameter,
            ) -> MathematicalBinderSnapshot {
                let (kind, carrier) = match &parameter.kind {
                    crate::data::TypeParameterKind::Type => ("type", None),
                    crate::data::TypeParameterKind::Implementation => ("implementation", None),
                    crate::data::TypeParameterKind::Const { type_reference }
                    | crate::data::TypeParameterKind::Value { type_reference } => {
                        let kind = match &parameter.kind {
                            crate::data::TypeParameterKind::Const { .. } => "const",
                            _ => "value",
                        };
                        (
                            kind,
                            Some(type_reference_snapshot(program, *type_reference)),
                        )
                    }
                    crate::data::TypeParameterKind::Machine { .. } => ("machine", None),
                    crate::data::TypeParameterKind::Proposition { .. } => ("proposition", None),
                };
                MathematicalBinderSnapshot {
                    has_symbol: parameter.symbol.is_valid(),
                    name: parameter.name.to_string(),
                    kind,
                    carrier,
                }
            }

            pub(crate) fn mathematical_definition_snapshot(
                program: &TypedTrees,
                definition: &crate::mathematical::MathematicalDefinition,
            ) -> MathematicalDefinitionSnapshot {
                MathematicalDefinitionSnapshot {
                    has_symbol: definition.symbol.is_valid(),
                    name: definition.name.to_string(),
                    is_public: definition.is_public,
                    binders: program
                        .data_type_parameters
                        .span_or_empty(definition.binders)
                        .iter()
                        .map(|binder| mathematical_binder_snapshot(program, binder))
                        .collect(),
                    parameters: program
                        .mathematical_parameters(definition.parameters)
                        .iter()
                        .map(|parameter| MathematicalParameterSnapshot {
                            has_symbol: parameter.symbol.is_valid(),
                            name: parameter.name.to_string(),
                            relevance: snapshot_binding_relevance(parameter.relevance),
                            ty: mathematical_type_snapshot(program, parameter.ty),
                        })
                        .collect(),
                    result: mathematical_type_snapshot(program, definition.result),
                    body: match &definition.body {
                        MathematicalBody::Assumption => MathematicalBodySnapshot::Assumption,
                        MathematicalBody::Definition(term) => {
                            MathematicalBodySnapshot::Definition {
                                term: expression_snapshot(program, *term),
                            }
                        }
                    },
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct MeasureDefinitionSnapshot {
                pub has_symbol: bool,
                pub name: Vec<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub parameter: Option<StateParameterSnapshot>,
                pub return_type: TypeReferenceSnapshot,
                pub lexicographic: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub body: Vec<ExpressionSnapshot>,
            }

            pub(crate) fn measure_snapshot(
                program: &TypedTrees,
                measure: &MeasureDefinition,
            ) -> MeasureDefinitionSnapshot {
                MeasureDefinitionSnapshot {
                    has_symbol: measure.symbol.is_valid(),
                    name: to_strings(program.measure_path_members(measure.name).iter()),
                    parameter: measure
                        .parameter
                        .as_ref()
                        .map(|parameter| state_parameter_snapshot(program, parameter)),
                    return_type: type_reference_snapshot(program, measure.return_type),
                    lexicographic: measure.lexicographic,
                    body: expression_span_snapshot(program, measure.body),
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct CleanupSelectionSnapshot {
                pub target: String,
                pub arguments: Vec<super::StaticArgumentSnapshot>,
                pub subject: TypeReferenceSnapshot,
                pub has_executable: bool,
                pub has_authorization: bool,
            }

            fn cleanup_selection_snapshot(
                program: &TypedTrees,
                selection: &crate::domain::CleanupSelection,
            ) -> CleanupSelectionSnapshot {
                let application = program.static_machine_application(selection.application);
                CleanupSelectionSnapshot {
                    target: program
                        .symbols
                        .display_path(application.registration_operation, "::"),
                    arguments: program
                        .static_machine_application_arguments(selection.application)
                        .iter()
                        .map(|argument| {
                            super::statement_and_expression_snapshots::snapshot_static_argument(
                                program, argument,
                            )
                        })
                        .collect(),
                    subject: type_reference_snapshot(program, selection.subject),
                    has_executable: selection.executable.is_valid(),
                    has_authorization: selection.authorization.is_some(),
                }
            }
        }
        pub(crate) mod machine_snapshots {
            use super::to_strings;
            use crate::TypedTrees;
            use crate::domain::ProofFact;
            use crate::inspection::snapshot;
            use crate::machine::{Machine, OwnedData};
            use crate::signature::SignatureContractKind;
            use crate::signature::{StateParameter, StateSignature};
            use crate::state::State;
            use crate::trait_definition::TraitDefinition;
            use serde::Serialize;
            use snapshot::declaration_snapshots;
            use snapshot::statement_and_expression_snapshots::{
                expression_snapshot_option, statement_snapshot,
            };
            use snapshot::type_snapshots::{
                type_reference_snapshot, type_reference_snapshot_option,
            };
            use snapshot::{
                ExpressionSnapshot, ProofFactSnapshot, StatementSnapshot, TypeReferenceSnapshot,
            };

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct ConformanceSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub is_public: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub lifetime_parameters: Vec<String>,
                pub type_parameters: Vec<String>,
                pub trait_name: String,
                pub trait_symbol: u32,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub trait_lifetime_arguments: Vec<u32>,
                pub arguments: Vec<TypeReferenceSnapshot>,
                pub rows: Vec<ConformanceRowSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct ConformanceRowSnapshot {
                pub declaring_trait: u32,
                pub requirement: u32,
                pub realization_machine: u32,
                pub realization_state: u32,
                pub source: &'static str,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct MachineSnapshot {
                pub name: String,
                pub attached_data: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub spelling: Option<&'static str>,
                pub is_public: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub lifetime_parameters: Vec<String>,
                pub type_parameters: Vec<String>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub declared_type_equations: Vec<DeclaredTypeEquationSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
                pub supply: MachineSupplySnapshot,
                pub body_is_present: bool,
                #[serde(skip_serializing_if = "is_false")]
                pub structural_type_equations_pending: bool,
                pub termination: TerminationInterfaceSnapshot,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub termination_witness: Option<RankingWitnessSnapshot>,
                pub invokes: Vec<String>,
                pub service_reach: Vec<String>,
                #[serde(skip_serializing_if = "is_false")]
                pub service_reach_is_installation_bound: bool,
                #[serde(skip_serializing_if = "is_zero_u64")]
                pub admitted_same_stack_bytes: u64,
                pub suspends: bool,
                pub blocks: bool,
                pub contracts: Vec<SignatureContractSnapshot>,
                pub owned_data: Vec<OwnedDataSnapshot>,
                pub states: Vec<StateSnapshot>,
            }

            fn is_false(value: &bool) -> bool {
                !*value
            }

            fn is_zero_u64(value: &u64) -> bool {
                *value == 0
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct GenericConformanceBoundSnapshot {
                pub binder: String,
                pub binder_symbol: u32,
                pub trait_name: String,
                pub trait_symbol: u32,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub lifetime_arguments: Vec<String>,
                pub arguments: Vec<TypeReferenceSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct DeclaredTypeEquationSnapshot {
                pub parameter: TypeReferenceSnapshot,
                pub definition: TypeReferenceSnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum MachineSupplySnapshot {
                CheckedBody,
                Requirement,
                TopLevelRequirement,
                Boundary,
                AdmissionClaim,
                ExternalRealization {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    binding: Option<u32>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    mechanism: Option<&'static str>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "interface", rename_all = "snake_case")]
            pub enum TerminationInterfaceSnapshot {
                InternalDerived,
                Published {
                    guarantee: TerminationGuaranteeSnapshot,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum TerminationGuaranteeSnapshot {
                NoGuarantee,
                Terminates {
                    premises: Vec<ProgressPremiseSnapshot>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct ProgressPremiseSnapshot {
                pub profile: u32,
                pub subject_root: u32,
                pub subject_projections: Vec<u32>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct RankingWitnessSnapshot {
                pub subjects: Vec<String>,
                pub ranking_view: u32,
                pub view_path: String,
                pub view_arguments: Vec<String>,
                pub rank_range: Option<RankRangeSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct RankRangeSnapshot {
                pub floor: String,
                pub ceiling: String,
                pub ceiling_inclusive: bool,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct OwnedDataSnapshot {
                pub name: String,
                pub type_reference: TypeReferenceSnapshot,
                pub initial_value: Option<ExpressionSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct TraitSnapshot {
                pub name: String,
                pub is_boundary: bool,
                pub is_public: bool,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub lifetime_parameters: Vec<String>,
                pub type_parameters: Vec<String>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
                pub requires: Vec<String>,
                pub machines: Vec<StateSignatureSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StateSnapshot {
                pub name: String,
                pub parameters: Vec<StateParameterSnapshot>,
                pub return_type: Option<TypeReferenceSnapshot>,
                pub contracts: Vec<SignatureContractSnapshot>,
                pub statements: Vec<StatementSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StateSignatureSnapshot {
                pub name: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub spelling: Option<&'static str>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub lifetime_parameters: Vec<String>,
                pub type_parameters: Vec<String>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub conformance_bounds: Vec<GenericConformanceBoundSnapshot>,
                pub is_default: bool,
                pub parameters: Vec<StateParameterSnapshot>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub native_callback_parameters: Vec<NativeCallbackParameterSnapshot>,
                pub return_type: Option<TypeReferenceSnapshot>,
                pub invokes: Vec<String>,
                pub service_reach: Vec<String>,
                pub service_reach_is_installation_bound: bool,
                pub suspends: bool,
                pub blocks: bool,
                pub contracts: Vec<SignatureContractSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub enum RuntimeCallableContractSnapshot {
                Contract {
                    lexical_owner: String,
                    resolved_source: Option<(u32, u32)>,
                    signature: Box<CallableSignatureSnapshot>,
                },
                Invalid {
                    handle: (u32, u32),
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct CallableSignatureSnapshot {
                pub signature: StateSignatureSnapshot,
                pub type_parameters: Vec<CallableTypeParameterSnapshot>,
                pub termination: TerminationInterfaceSnapshot,
                pub declares_no_crash_route: bool,
                pub contract_metadata: Vec<CallableContractMetadataSnapshot>,
                pub invocation_targets: Vec<String>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct CallableContractMetadataSnapshot {
                pub keyword_source_span: Option<super::SourceSpanSnapshot>,
                pub result_case: Option<(String, String)>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct CallableTypeParameterSnapshot {
                pub name: String,
                pub multiplicity: String,
                pub carry: Option<String>,
                pub kind: CallableTypeParameterKindSnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub enum CallableTypeParameterKindSnapshot {
                Type,
                Implementation,
                Const {
                    type_reference: TypeReferenceSnapshot,
                },
                Value {
                    type_reference: TypeReferenceSnapshot,
                },
                Machine {
                    contract: Box<CallableMachineParameterSnapshot>,
                },
                Proposition {
                    name: String,
                    parameters: Vec<StateParameterSnapshot>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub enum CallableMachineParameterSnapshot {
                RequirementIdentity,
                Structural {
                    signature: Box<CallableSignatureSnapshot>,
                },
                Nominal {
                    receiving_trait: String,
                    receiving_arguments: Vec<TypeReferenceSnapshot>,
                    trait_definition: String,
                    requirement: String,
                    trait_arguments: Vec<TypeReferenceSnapshot>,
                    instantiated: Option<Box<CallableSignatureSnapshot>>,
                },
            }

            pub(crate) fn runtime_callable_contract_snapshot(
                program: &TypedTrees,
                handle: crate::callable_values::RuntimeCallableContractHandle,
            ) -> RuntimeCallableContractSnapshot {
                if !program.runtime_callable_contracts.is_valid(handle) {
                    return RuntimeCallableContractSnapshot::Invalid {
                        handle: (handle.arena_index(), handle.generation()),
                    };
                }
                let contract = program.runtime_callable_contracts.get(handle);
                RuntimeCallableContractSnapshot::Contract {
                    lexical_owner: program.symbols.display_path(contract.lexical_owner, "::"),
                    resolved_source: contract.resolved_source,
                    signature: Box::new(callable_signature_snapshot(program, &contract.signature)),
                }
            }

            fn callable_signature_snapshot(
                program: &TypedTrees,
                signature: &StateSignature,
            ) -> CallableSignatureSnapshot {
                CallableSignatureSnapshot {
                    signature: state_signature_snapshot(program, signature),
                    type_parameters: program
                        .state_signature_type_parameters(signature)
                        .iter()
                        .map(|parameter| {
                            use crate::data::TypeParameterKind;
                            let kind = match &parameter.kind {
                                TypeParameterKind::Type => CallableTypeParameterKindSnapshot::Type,
                                TypeParameterKind::Implementation => {
                                    CallableTypeParameterKindSnapshot::Implementation
                                }
                                TypeParameterKind::Const { type_reference } => {
                                    CallableTypeParameterKindSnapshot::Const {
                                        type_reference: type_reference_snapshot(
                                            program,
                                            *type_reference,
                                        ),
                                    }
                                }
                                TypeParameterKind::Value { type_reference } => {
                                    CallableTypeParameterKindSnapshot::Value {
                                        type_reference: type_reference_snapshot(
                                            program,
                                            *type_reference,
                                        ),
                                    }
                                }
                                TypeParameterKind::Machine { contract } => {
                                    CallableTypeParameterKindSnapshot::Machine {
                                        contract: Box::new(callable_machine_parameter_snapshot(
                                            program, contract,
                                        )),
                                    }
                                }
                                TypeParameterKind::Proposition { contract } => {
                                    CallableTypeParameterKindSnapshot::Proposition {
                                        name: contract.name.to_string(),
                                        parameters: program
                                            .state_parameters
                                            .span_or_empty(contract.parameters)
                                            .iter()
                                            .map(|parameter| {
                                                state_parameter_snapshot(program, parameter)
                                            })
                                            .collect(),
                                    }
                                }
                            };
                            CallableTypeParameterSnapshot {
                                name: parameter.name.to_string(),
                                multiplicity: format!("{:?}", parameter.bounds.multiplicity),
                                carry: parameter.bounds.carry.map(|carry| format!("{carry:?}")),
                                kind,
                            }
                        })
                        .collect(),
                    termination: termination_interface_snapshot(
                        &language_semantics::TerminationInterface::Published(
                            signature.termination_guarantee.clone(),
                        ),
                    ),
                    declares_no_crash_route: signature.declares_no_crash_route,
                    contract_metadata: program
                        .state_signature_contracts(signature)
                        .iter()
                        .map(|contract| CallableContractMetadataSnapshot {
                            keyword_source_span: contract
                                .keyword_source_span
                                .as_ref()
                                .map(super::source_span_snapshot),
                            result_case: match contract.kind {
                                SignatureContractKind::EnsuresForResultCase {
                                    result_data,
                                    result_case,
                                } => Some((
                                    program.symbols.display_path(result_data, "::"),
                                    program.symbols.display_path(result_case, "::"),
                                )),
                                _ => None,
                            },
                        })
                        .collect(),
                    invocation_targets: program
                        .state_signature_invokes(signature)
                        .iter()
                        .map(|invocation| {
                            use crate::signature::AuthoredInvocationTarget;
                            match invocation.target {
                                AuthoredInvocationTarget::Unresolved => "unresolved".to_owned(),
                                AuthoredInvocationTarget::Parameter { ordinal, symbol } => format!(
                                    "parameter {ordinal}: {}",
                                    program.symbols.display_path(symbol, "::")
                                ),
                                AuthoredInvocationTarget::Service(symbol) => format!(
                                    "service: {}",
                                    program.symbols.display_path(symbol, "::")
                                ),
                            }
                        })
                        .collect(),
                }
            }

            fn callable_machine_parameter_snapshot(
                program: &TypedTrees,
                contract: &crate::data::MachineParameterContract,
            ) -> CallableMachineParameterSnapshot {
                use crate::data::MachineParameterContract;
                match contract {
                    MachineParameterContract::RequirementIdentity => {
                        CallableMachineParameterSnapshot::RequirementIdentity
                    }
                    MachineParameterContract::Structural(signature) => {
                        CallableMachineParameterSnapshot::Structural {
                            signature: Box::new(callable_signature_snapshot(program, signature)),
                        }
                    }
                    MachineParameterContract::Nominal {
                        receiving_trait,
                        receiving_arguments,
                        trait_definition,
                        requirement,
                        trait_arguments,
                        instantiated,
                    } => CallableMachineParameterSnapshot::Nominal {
                        receiving_trait: program.symbols.display_path(*receiving_trait, "::"),
                        receiving_arguments: receiving_arguments
                            .iter()
                            .map(|reference| type_reference_snapshot(program, *reference))
                            .collect(),
                        trait_definition: program.symbols.display_path(*trait_definition, "::"),
                        requirement: program.symbols.display_path(*requirement, "::"),
                        trait_arguments: trait_arguments
                            .iter()
                            .map(|reference| type_reference_snapshot(program, *reference))
                            .collect(),
                        instantiated: instantiated.as_ref().map(|signature| {
                            Box::new(callable_signature_snapshot(program, signature))
                        }),
                    },
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct NativeCallbackParameterSnapshot {
                pub name: String,
                pub binder: String,
                pub native_ordinal: u32,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct SignatureContractSnapshot {
                pub kind: &'static str,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub binding: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub crash_cause: Option<&'static str>,
                pub facts: Vec<ProofFactSnapshot>,
                pub token_count: usize,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StateParameterSnapshot {
                pub name: String,
                pub type_reference: TypeReferenceSnapshot,
                pub is_const: bool,
                pub is_mutable: bool,
                pub relevance: &'static str,
            }

            pub(crate) fn machine_snapshot(
                program: &TypedTrees,
                machine: &Machine,
            ) -> MachineSnapshot {
                MachineSnapshot {
                    name: machine.name.to_string(),
                    attached_data: machine.attached_data.as_ref().map(ToString::to_string),
                    spelling: machine.spelling.map(|spelling| spelling.symbol()),
                    is_public: machine.is_public,
                    lifetime_parameters: to_strings(machine.lifetime_parameters.iter()),
                    type_parameters: program
                        .machine_type_parameters(machine)
                        .iter()
                        .map(|parameter| parameter.name.to_string())
                        .collect(),
                    conformance_bounds: machine
                        .conformance_bounds
                        .iter()
                        .map(|bound| conformance_bound_snapshot(program, bound))
                        .collect(),
                    supply: machine_supply_snapshot(machine.supply_mode),
                    body_is_present: machine.body_is_present,
                    structural_type_equations_pending: machine.structural_type_equations_pending,
                    declared_type_equations: program
                        .declared_type_equations
                        .span_or_empty(machine.declared_type_equations)
                        .iter()
                        .map(|equation| DeclaredTypeEquationSnapshot {
                            parameter: type_reference_snapshot(program, equation.parameter),
                            definition: type_reference_snapshot(program, equation.definition),
                        })
                        .collect(),
                    termination: termination_interface_snapshot(
                        &machine.termination_plan.interface,
                    ),
                    termination_witness: machine
                        .termination_plan
                        .implementation_witness
                        .as_ref()
                        .map(|witness| RankingWitnessSnapshot {
                            subjects: witness.subjects.clone(),
                            ranking_view: witness.ranking_view.0,
                            view_path: witness.view_path.clone(),
                            view_arguments: witness.view_arguments.clone(),
                            rank_range: witness.rank_range.as_ref().map(|range| {
                                RankRangeSnapshot {
                                    floor: range.floor.clone(),
                                    ceiling: range.ceiling.clone(),
                                    ceiling_inclusive: range.ceiling_inclusive,
                                }
                            }),
                        }),
                    invokes: to_strings(program.machine_invokes(machine).iter()),
                    service_reach: service_reach_names(program, machine.service_reach_row),
                    service_reach_is_installation_bound: machine
                        .service_reach_is_installation_bound,
                    admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
                    suspends: machine.suspends,
                    blocks: machine.blocks,
                    contracts: program
                        .machine_contracts(machine)
                        .iter()
                        .map(|contract| signature_contract_snapshot(program, contract))
                        .collect(),
                    owned_data: program
                        .machine_owned_data(machine)
                        .iter()
                        .map(|owned| owned_data_snapshot(program, owned))
                        .collect(),
                    states: program
                        .machine_states(machine)
                        .iter()
                        .map(|state| state_snapshot(program, state))
                        .collect(),
                }
            }

            fn machine_supply_snapshot(
                supply: language_semantics::MachineSupplyMode,
            ) -> MachineSupplySnapshot {
                use language_semantics::MachineSupplyMode;
                match supply {
                    MachineSupplyMode::CheckedBody => MachineSupplySnapshot::CheckedBody,
                    MachineSupplyMode::Requirement => MachineSupplySnapshot::Requirement,
                    MachineSupplyMode::TopLevelRequirement => {
                        MachineSupplySnapshot::TopLevelRequirement
                    }
                    MachineSupplyMode::Boundary => MachineSupplySnapshot::Boundary,
                    MachineSupplyMode::AdmissionClaim => MachineSupplySnapshot::AdmissionClaim,
                    MachineSupplyMode::ExternalRealization { binding, mechanism } => {
                        MachineSupplySnapshot::ExternalRealization {
                            binding: binding.map(|binding| binding.0),
                            mechanism: mechanism.map(|mechanism| mechanism.as_str()),
                        }
                    }
                }
            }

            fn termination_interface_snapshot(
                interface: &language_semantics::TerminationInterface,
            ) -> TerminationInterfaceSnapshot {
                use language_semantics::{TerminationGuarantee, TerminationInterface};
                match interface {
                    TerminationInterface::InternalDerived => {
                        TerminationInterfaceSnapshot::InternalDerived
                    }
                    TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => {
                        TerminationInterfaceSnapshot::Published {
                            guarantee: TerminationGuaranteeSnapshot::NoGuarantee,
                        }
                    }
                    TerminationInterface::Published(TerminationGuarantee::Terminates {
                        premises,
                    }) => TerminationInterfaceSnapshot::Published {
                        guarantee: TerminationGuaranteeSnapshot::Terminates {
                            premises: premises
                                .iter()
                                .map(|premise| ProgressPremiseSnapshot {
                                    profile: premise.profile.0,
                                    subject_root: premise.subject.root.arena_index(),
                                    subject_projections: premise
                                        .subject
                                        .projections
                                        .iter()
                                        .map(|symbol| symbol.arena_index())
                                        .collect(),
                                })
                                .collect(),
                        },
                    },
                }
            }

            fn owned_data_snapshot(program: &TypedTrees, owned: &OwnedData) -> OwnedDataSnapshot {
                OwnedDataSnapshot {
                    name: owned.name.to_string(),
                    type_reference: type_reference_snapshot(program, owned.type_reference),
                    initial_value: expression_snapshot_option(program, owned.initial_value),
                }
            }

            pub(crate) fn trait_definition_snapshot(
                program: &TypedTrees,
                trait_definition: &TraitDefinition,
            ) -> TraitSnapshot {
                TraitSnapshot {
                    name: trait_definition.name.to_string(),
                    is_boundary: trait_definition.is_boundary,
                    is_public: trait_definition.is_public,
                    lifetime_parameters: to_strings(trait_definition.lifetime_parameters.iter()),
                    type_parameters: program
                        .trait_type_parameters(trait_definition)
                        .iter()
                        .map(|parameter| parameter.name.to_string())
                        .collect(),
                    conformance_bounds: trait_definition
                        .conformance_bounds
                        .iter()
                        .map(|bound| conformance_bound_snapshot(program, bound))
                        .collect(),
                    requires: program
                        .trait_requirements(trait_definition)
                        .iter()
                        .map(|requirement| {
                            let arguments = program.type_reference_handles(requirement.arguments);
                            if requirement.lifetime_arguments.is_empty() && arguments.is_empty() {
                                requirement.name.to_string()
                            } else {
                                let lifetime_arguments = requirement
                                    .lifetime_arguments
                                    .iter()
                                    .map(|lifetime| format!("'{lifetime}"));
                                let type_arguments = arguments
                                    .iter()
                                    .map(|argument| program.display_type_reference(*argument));
                                format!(
                                    "{}<{}>",
                                    requirement.name,
                                    lifetime_arguments
                                        .chain(type_arguments)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            }
                        })
                        .collect(),
                    machines: program
                        .trait_machine_signatures(trait_definition)
                        .iter()
                        .map(|signature| state_signature_snapshot(program, signature))
                        .collect(),
                }
            }

            pub(crate) fn conformance_snapshot(
                program: &TypedTrees,
                conformance: &crate::trait_definition::Conformance,
            ) -> ConformanceSnapshot {
                let rows = conformance
                    .rows
                    .iter()
                    .map(|row| ConformanceRowSnapshot {
                        declaring_trait: row.declaring_trait.arena_index(),
                        requirement: row.requirement.arena_index(),
                        realization_machine: row.realization_machine.arena_index(),
                        realization_state: row.realization_state.arena_index(),
                        source: match row.source {
                            crate::trait_definition::ConformanceRowSource::Inline => "inline",
                            crate::trait_definition::ConformanceRowSource::Reference => "reference",
                            crate::trait_definition::ConformanceRowSource::Synthesized => {
                                "synthesized"
                            }
                            crate::trait_definition::ConformanceRowSource::TraitDefault => {
                                "trait_default"
                            }
                        },
                    })
                    .collect();
                ConformanceSnapshot {
                    has_symbol: conformance.symbol.is_valid(),
                    name: conformance.alias.as_ref().map_or_else(
                        || program.symbols.display_path(conformance.symbol, "::"),
                        ToString::to_string,
                    ),
                    is_public: conformance.is_public,
                    lifetime_parameters: to_strings(conformance.lifetime_parameters.iter()),
                    type_parameters: program
                        .conformance_type_parameters(conformance)
                        .iter()
                        .map(|parameter| parameter.name.to_string())
                        .collect(),
                    trait_name: conformance.trait_name.to_string(),
                    trait_symbol: conformance.trait_symbol.arena_index(),
                    trait_lifetime_arguments: conformance.trait_lifetime_arguments.clone(),
                    arguments: program
                        .type_reference_handles(conformance.arguments)
                        .iter()
                        .map(|argument| type_reference_snapshot(program, *argument))
                        .collect(),
                    rows,
                }
            }

            fn state_snapshot(program: &TypedTrees, state: &State) -> StateSnapshot {
                StateSnapshot {
                    name: state.name.to_string(),
                    parameters: program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| state_parameter_snapshot(program, parameter))
                        .collect(),
                    return_type: type_reference_snapshot_option(program, state.return_type),
                    contracts: program
                        .state_contracts(state)
                        .iter()
                        .map(|contract| signature_contract_snapshot(program, contract))
                        .collect(),
                    statements: program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .map(|statement| statement_snapshot(program, statement))
                        .collect(),
                }
            }

            fn state_signature_snapshot(
                program: &TypedTrees,
                signature: &StateSignature,
            ) -> StateSignatureSnapshot {
                StateSignatureSnapshot {
                    name: signature.name.to_string(),
                    spelling: signature.spelling.map(|spelling| spelling.symbol()),
                    lifetime_parameters: to_strings(signature.lifetime_parameters.iter()),
                    type_parameters: program
                        .state_signature_type_parameters(signature)
                        .iter()
                        .map(|parameter| parameter.name.to_string())
                        .collect(),
                    conformance_bounds: signature
                        .conformance_bounds
                        .iter()
                        .map(|bound| conformance_bound_snapshot(program, bound))
                        .collect(),
                    is_default: signature.is_default,
                    parameters: program
                        .state_signature_parameters(signature)
                        .iter()
                        .map(|parameter| state_parameter_snapshot(program, parameter))
                        .collect(),
                    native_callback_parameters: signature
                        .native_callback_parameters
                        .iter()
                        .map(|parameter| NativeCallbackParameterSnapshot {
                            name: parameter.name.to_string(),
                            binder: parameter.binder.to_string(),
                            native_ordinal: parameter.native_ordinal,
                        })
                        .collect(),
                    return_type: type_reference_snapshot_option(program, signature.return_type),
                    invokes: to_strings(program.state_signature_invokes(signature).iter()),
                    service_reach: service_reach_names(program, signature.service_reach_row),
                    service_reach_is_installation_bound: signature
                        .service_reach_is_installation_bound,
                    suspends: signature.suspends,
                    blocks: signature.blocks,
                    contracts: program
                        .state_signature_contracts(signature)
                        .iter()
                        .map(|contract| signature_contract_snapshot(program, contract))
                        .collect(),
                }
            }

            fn service_reach_names(
                program: &TypedTrees,
                row: language_semantics::ServiceReachRowId,
            ) -> Vec<String> {
                program
                    .service_reach_rows
                    .services(row)
                    .iter()
                    .filter_map(|service| program.service_reaches.definition(*service))
                    .map(|definition| definition.name.clone())
                    .collect()
            }

            fn signature_contract_snapshot(
                program: &TypedTrees,
                contract: &crate::signature::SignatureContract,
            ) -> SignatureContractSnapshot {
                let (kind, crash_cause) = match &contract.kind {
                    SignatureContractKind::Requires => ("requires", None),
                    SignatureContractKind::Ensures => ("ensures", None),
                    SignatureContractKind::EnsuresForResultCase { .. } => {
                        ("ensures_for_result_case", None)
                    }
                    SignatureContractKind::Crashes { cause } => {
                        return SignatureContractSnapshot {
                            kind: "crashes",
                            binding: contract.binding.as_ref().map(ToString::to_string),
                            crash_cause: Some(match cause {
                                crate::signature::CrashCause::Trap => "Trap",
                                crate::signature::CrashCause::Abort => "Abort",
                            }),
                            facts: contract_fact_snapshots(program, contract.facts),
                            token_count: contract.token_count,
                        };
                    }
                };
                SignatureContractSnapshot {
                    kind,
                    binding: contract.binding.as_ref().map(ToString::to_string),
                    crash_cause,
                    facts: contract_fact_snapshots(program, contract.facts),
                    token_count: contract.token_count,
                }
            }

            fn contract_fact_snapshots(
                program: &TypedTrees,
                facts: arena::HandleSpan<ProofFact>,
            ) -> Vec<ProofFactSnapshot> {
                declaration_snapshots::fact_snapshots_for_span(program, facts)
            }

            pub(crate) fn state_parameter_snapshot(
                program: &TypedTrees,
                parameter: &StateParameter,
            ) -> StateParameterSnapshot {
                StateParameterSnapshot {
                    name: parameter.name.to_string(),
                    type_reference: type_reference_snapshot(program, parameter.type_reference),
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    relevance: super::declaration_snapshots::snapshot_binding_relevance(
                        parameter.relevance,
                    ),
                }
            }

            pub(crate) fn conformance_bound_snapshot(
                program: &TypedTrees,
                bound: &crate::machine::GenericConformanceBound,
            ) -> GenericConformanceBoundSnapshot {
                GenericConformanceBoundSnapshot {
                    binder: bound.binder_name.to_string(),
                    binder_symbol: bound.binder.arena_index(),
                    trait_name: bound.trait_name.to_string(),
                    trait_symbol: bound.trait_symbol.arena_index(),
                    lifetime_arguments: bound
                        .lifetime_arguments
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                    arguments: bound
                        .arguments
                        .iter()
                        .copied()
                        .map(|argument| type_reference_snapshot(program, argument))
                        .collect(),
                }
            }
        }
        pub(crate) mod statement_and_expression_snapshots {

            use super::to_strings;
            use crate::TypedTrees;
            use crate::expression::MatchFieldAction;
            use crate::expression::MatchPattern;
            use crate::expression::{ExpressionHandle, ExpressionNode};
            use crate::inspection::snapshot;
            use crate::statement::TransitionExit;
            use crate::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
            use serde::Serialize;
            use snapshot::TypeReferenceSnapshot;
            use snapshot::type_snapshots::{path_snapshot, type_reference_snapshot};

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(untagged)]
            pub enum StaticArgumentSnapshot {
                Type {
                    type_reference: Box<TypeReferenceSnapshot>,
                },
                Path(Vec<String>),
                Application {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    member: Option<Box<StaticArgumentSnapshot>>,
                    path: Vec<String>,
                    lifetime_arguments: Vec<String>,
                    arguments: Vec<StaticArgumentSnapshot>,
                },
                Const(String),
                EvidenceProjection {
                    term: String,
                    member: String,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct RuntimeCallSnapshot {
                pub callee: Box<ExpressionSnapshot>,
                pub contract: Box<super::machine_snapshots::RuntimeCallableContractSnapshot>,
            }

            fn runtime_call_snapshot(
                program: &TypedTrees,
                binding: crate::callable_values::RuntimeCallBinding,
            ) -> RuntimeCallSnapshot {
                RuntimeCallSnapshot {
                    callee: Box::new(expression_snapshot(program, binding.callee)),
                    contract: Box::new(
                        super::machine_snapshots::runtime_callable_contract_snapshot(
                            program,
                            binding.contract,
                        ),
                    ),
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StaticApplicationGraphSnapshot {
                pub root: (u32, u32),
                pub applications: Vec<StaticApplicationSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StaticApplicationSnapshot {
                pub handle: (u32, u32),
                pub callable_contract_source: Option<(u32, u32)>,
                pub registration_operation: String,
                pub lexical_owner: String,
                pub target: String,
                pub arguments: Vec<StaticArgumentSnapshot>,
                pub selected_machine_arguments: Vec<StaticArgumentSnapshot>,
                pub bindings: Vec<(String, StaticBindingSnapshot)>,
                pub lifetime_bindings: Vec<(String, String)>,
                pub lifetime_binding_declaration: String,
                pub binding_declaration: String,
                pub input_selections: Vec<StaticInputSelectionSnapshot>,
                pub conformance_selection: Option<StaticConformanceSelectionSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StaticConformanceSelectionSnapshot {
                pub ordinal: u32,
                pub application: Option<ClosedConformanceSelectionSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct ClosedConformanceSelectionSnapshot {
                pub declaration: String,
                pub callable_application: Option<(u32, u32)>,
                pub arguments: Vec<StaticArgumentSnapshot>,
                pub identity_encoding: Vec<u8>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StaticInputSelectionSnapshot {
                pub occurrence: Option<String>,
                pub argument_ordinal: u32,
                pub target: StaticInputTargetSnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub enum StaticInputTargetSnapshot {
                Invalid,
                Argument {
                    steps: Vec<String>,
                },
                Type {
                    reference: Box<TypeReferenceSnapshot>,
                    slot: String,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub enum StaticBindingSnapshot {
                Type(Box<TypeReferenceSnapshot>),
                Const(Box<TypeReferenceSnapshot>),
                RuntimeValue {
                    type_reference: Box<TypeReferenceSnapshot>,
                    subject: String,
                    expression: Box<ExpressionSnapshot>,
                },
                Machine(StaticArgumentSnapshot),
                Conformance(StaticArgumentSnapshot),
            }

            fn static_application_snapshot(
                program: &TypedTrees,
                root: crate::static_applications::StaticMachineApplicationHandle,
            ) -> StaticApplicationGraphSnapshot {
                let coordinate =
                    |handle: crate::static_applications::StaticMachineApplicationHandle| {
                        (handle.arena_index(), handle.generation())
                    };
                let mut pending = vec![root];
                let mut seen = Vec::new();
                let mut applications = Vec::new();
                while let Some(handle) = pending.pop() {
                    if !handle.is_valid() || seen.contains(&handle) {
                        continue;
                    }
                    seen.push(handle);
                    let application = program.static_machine_application(handle);
                    pending.push(application.callable_contract_source);
                    let conformance_selection =
                        application.conformance_selection.as_ref().map(|selection| {
                            StaticConformanceSelectionSnapshot {
                                ordinal: selection.operand_ordinal,
                                application: selection.application.as_ref().map(|selected| {
                                    pending.push(selected.callable_application);
                                    ClosedConformanceSelectionSnapshot {
                                        declaration: program
                                            .symbols
                                            .display_path(selected.declaration, "::"),
                                        callable_application: selected
                                            .callable_application
                                            .is_valid()
                                            .then(|| coordinate(selected.callable_application)),
                                        arguments: selected
                                            .arguments
                                            .iter()
                                            .map(|argument| {
                                                snapshot_static_argument(program, argument)
                                            })
                                            .collect(),
                                        identity_encoding: selected.identity_encoding.to_vec(),
                                    }
                                }),
                            }
                        });
                    applications.push(StaticApplicationSnapshot {
                        handle: coordinate(handle),
                        callable_contract_source: application
                            .callable_contract_source
                            .is_valid()
                            .then(|| coordinate(application.callable_contract_source)),
                        registration_operation: program
                            .symbols
                            .display_path(application.registration_operation, "::"),
                        lexical_owner: program
                            .symbols
                            .display_path(application.lexical_owner, "::"),
                        target: application.target.to_string(),
                        arguments: program
                            .static_machine_application_arguments(handle)
                            .iter()
                            .map(|argument| snapshot_static_argument(program, argument))
                            .collect(),
                        selected_machine_arguments: program
                            .expression_table
                            .static_machine_application_selected_machines(handle)
                            .iter()
                            .map(|argument| snapshot_static_argument(program, argument))
                            .collect(),
                        bindings: program
                            .static_machine_application_bindings(handle)
                            .iter()
                            .map(|binding| {
                                use crate::static_applications::StaticMachineBindingValue;
                                let value = match &binding.value {
                                    StaticMachineBindingValue::Type(reference) => {
                                        StaticBindingSnapshot::Type(Box::new(
                                            type_reference_snapshot(program, *reference),
                                        ))
                                    }
                                    StaticMachineBindingValue::Const(reference) => {
                                        StaticBindingSnapshot::Const(Box::new(
                                            type_reference_snapshot(program, *reference),
                                        ))
                                    }
                                    StaticMachineBindingValue::RuntimeValue {
                                        type_reference,
                                        subject,
                                        expression,
                                    } => StaticBindingSnapshot::RuntimeValue {
                                        type_reference: Box::new(type_reference_snapshot(
                                            program,
                                            *type_reference,
                                        )),
                                        subject: program.symbols.display_path(*subject, "::"),
                                        expression: Box::new(expression_snapshot(
                                            program,
                                            *expression,
                                        )),
                                    },
                                    StaticMachineBindingValue::Machine(argument) => {
                                        StaticBindingSnapshot::Machine(snapshot_static_argument(
                                            program, argument,
                                        ))
                                    }
                                    StaticMachineBindingValue::Conformance(argument) => {
                                        StaticBindingSnapshot::Conformance(
                                            snapshot_static_argument(program, argument),
                                        )
                                    }
                                };
                                (program.symbols.display_path(binding.parameter, "::"), value)
                            })
                            .collect(),
                        lifetime_bindings: program
                            .static_machine_application_lifetimes(handle)
                            .iter()
                            .map(|binding| {
                                (binding.parameter.to_string(), binding.argument.to_string())
                            })
                            .collect(),
                        lifetime_binding_declaration: program
                            .symbols
                            .display_path(application.lifetime_binding_declaration, "::"),
                        binding_declaration: program
                            .symbols
                            .display_path(application.binding_declaration, "::"),
                        input_selections: program
                            .product_input_selections
                            .span_or_empty(application.input_selections)
                            .iter()
                            .map(|selection| {
                                use crate::product_operand::ProductInputSelectionTarget;
                                StaticInputSelectionSnapshot {
                                    occurrence: selection
                                        .occurrence
                                        .map(|occurrence| format!("{occurrence:?}")),
                                    argument_ordinal: selection.argument_ordinal,
                                    target: match &selection.target {
                                        ProductInputSelectionTarget::Invalid => {
                                            StaticInputTargetSnapshot::Invalid
                                        }
                                        ProductInputSelectionTarget::Argument(steps) => {
                                            StaticInputTargetSnapshot::Argument {
                                                steps: program
                                                    .product_input_argument_steps
                                                    .span_or_empty(*steps)
                                                    .iter()
                                                    .map(|step| format!("{step:?}"))
                                                    .collect(),
                                            }
                                        }
                                        ProductInputSelectionTarget::Type { reference, slot } => {
                                            StaticInputTargetSnapshot::Type {
                                                reference: Box::new(type_reference_snapshot(
                                                    program, *reference,
                                                )),
                                                slot: format!("{slot:?}"),
                                            }
                                        }
                                    },
                                }
                            })
                            .collect(),
                        conformance_selection,
                    });
                }
                StaticApplicationGraphSnapshot {
                    root: coordinate(root),
                    applications,
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum StatementSnapshot {
                Assembly {
                    primitives: Vec<ExpressionSnapshot>,
                    scratch: Vec<ExpressionSnapshot>,
                    statement_count: u32,
                    blocks: Vec<AssemblyBlockSnapshot>,
                },
                AssemblyLabel {
                    label: String,
                },
                AssemblyFact {
                    contract_kind: &'static str,
                    fact: super::declaration_snapshots::ProofFactSnapshot,
                },
                Assignment {
                    target: ExpressionSnapshot,
                    value: ExpressionSnapshot,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    receiving_type: Option<TypeReferenceSnapshot>,
                },
                Call {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    runtime_call: Option<RuntimeCallSnapshot>,
                    receiver: Option<Vec<String>>,
                    target: String,
                    machine_arguments: Vec<StaticArgumentSnapshot>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    installed_registrar: Option<StaticArgumentSnapshot>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    installed_timer_slot: Option<StaticArgumentSnapshot>,
                    arguments: Vec<ExpressionSnapshot>,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    evidence_arguments: Vec<String>,
                    acknowledgement_synthesized: bool,
                    acknowledges_suspend: bool,
                    acknowledges_block: bool,
                },
                Expression {
                    value: ExpressionSnapshot,
                },
                LocalData {
                    role: String,
                    name: String,
                    type_reference: TypeReferenceSnapshot,
                    initial_value: Option<ExpressionSnapshot>,
                    type_is_inferred: bool,
                    relevance: &'static str,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    pattern_fields: Vec<PatternFieldSnapshot>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    pattern_subject: Option<String>,
                },
                Transition {
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    patterns: Vec<(bool, ExpressionSnapshot, MatchPatternSnapshot)>,
                    target: TransitionTargetSnapshot,
                    continuation: Option<TransitionTargetSnapshot>,
                    guard: TransitionGuardSnapshot,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    proof_selectors: Vec<(String, String)>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    crash_cause: Option<&'static str>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct AssemblyBlockSnapshot {
                pub label: String,
                pub start: u32,
                pub count: u32,
                pub jump: Option<usize>,
                pub zero_flag_when_set: Option<bool>,
                pub enclosing_transition: bool,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum TransitionGuardSnapshot {
                Always,
                When { value: ExpressionSnapshot },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum TransitionTargetSnapshot {
                Named {
                    path: Vec<String>,
                    arguments: Vec<ExpressionSnapshot>,
                    evidence_arguments: Vec<String>,
                },
                Value {
                    value: ExpressionSnapshot,
                },
                Terminal,
                Invalid {
                    handle: u32,
                },
                Unary {
                    operator: String,
                    operand: Box<ExpressionSnapshot>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct MatchArmSnapshot {
                pub pattern: MatchPatternSnapshot,
                pub value: ExpressionSnapshot,
                pub source_id: usize,
                pub source_start: usize,
                pub source_end: usize,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum MatchPatternSnapshot {
                Value {
                    value: Box<ExpressionSnapshot>,
                },
                Destructure {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    application_type: Option<Box<TypeReferenceSnapshot>>,
                    head: Box<ExpressionSnapshot>,
                    fields: Vec<MatchFieldSnapshot>,
                    has_rest: bool,
                    guard: Option<Box<ExpressionSnapshot>>,
                    proof_selectors: Vec<(String, String)>,
                },
                Wildcard,
            }

            /// One row of a record pattern's roster: the field as authored,
            /// the declared field it resolved to, and whether it is bound.
            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct PatternFieldSnapshot {
                pub member: String,
                pub field: Option<String>,
                pub binding: Option<String>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct MatchFieldSnapshot {
                #[serde(skip_serializing_if = "Option::is_none")]
                pub component: Option<&'static str>,
                pub member: String,
                pub action: MatchFieldActionSnapshot,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum MatchFieldActionSnapshot {
                Bind { binding: String },
                Equal { value: Box<ExpressionSnapshot> },
                Waive,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum ExpressionSnapshot {
                MachineValue {
                    type_reference: Box<TypeReferenceSnapshot>,
                    selected: StaticArgumentSnapshot,
                    selected_symbol: String,
                    application: StaticApplicationGraphSnapshot,
                    conformance: String,
                    requirement: String,
                },
                Match {
                    subject: Box<ExpressionSnapshot>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    capture: Option<(u32, u32)>,
                    arms: Vec<MatchArmSnapshot>,
                },
                SavedSubject {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    component: Option<&'static str>,
                    capture: (u32, u32),
                    pattern_head: Box<ExpressionSnapshot>,
                },
                ArrayLiteral {
                    values: Vec<ExpressionSnapshot>,
                },
                Atomic {
                    value: Box<ExpressionSnapshot>,
                    result: Option<Box<ExpressionSnapshot>>,
                    ordering: String,
                },
                Binary {
                    left: Box<ExpressionSnapshot>,
                    operator: String,
                    right: Box<ExpressionSnapshot>,
                },
                Boolean {
                    value: bool,
                },
                Cast {
                    value: Box<ExpressionSnapshot>,
                    target_type: Box<TypeReferenceSnapshot>,
                    result_type: Box<TypeReferenceSnapshot>,
                    semantic_domain: Vec<String>,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    semantic_domain_arguments: Vec<TypeReferenceSnapshot>,
                    semantic_domain_symbol: u32,
                    semantic_domain_id: u32,
                },
                Call {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    runtime_call: Option<RuntimeCallSnapshot>,
                    receiver: Option<Box<ExpressionSnapshot>>,
                    target: String,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    exact_coercion_target: Option<Box<TypeReferenceSnapshot>>,
                    machine_arguments: Vec<StaticArgumentSnapshot>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    private_layout_slot: Option<StaticArgumentSnapshot>,
                    installed_registrar: Option<StaticArgumentSnapshot>,
                    installed_timer_slot: Option<StaticArgumentSnapshot>,
                    arguments: Vec<ExpressionSnapshot>,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    evidence_arguments: Vec<String>,
                    acknowledgement_synthesized: bool,
                    acknowledges_suspend: bool,
                    acknowledges_block: bool,
                },
                UndeterminedNaN {
                    format: String,
                },
                Float {
                    value: String,
                },
                Indexed {
                    collection: Box<ExpressionSnapshot>,
                    index: Box<ExpressionSnapshot>,
                },
                Integer {
                    text: String,
                },
                Member {
                    receiver: Box<ExpressionSnapshot>,
                    member: String,
                },
                Borrow {
                    access: &'static str,
                    value: Box<ExpressionSnapshot>,
                },
                Name {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    component: Option<&'static str>,
                    path: Vec<String>,
                },
                Range {
                    start: Option<Box<ExpressionSnapshot>>,
                    end: Option<Box<ExpressionSnapshot>>,
                    end_inclusive: bool,
                },
                StructLiteral {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    application_type: Option<Box<TypeReferenceSnapshot>>,
                    type_name: String,
                    fields: Vec<StructLiteralFieldSnapshot>,
                },
                String {
                    bytes: Vec<u8>,
                },
                Invalid {
                    handle: u32,
                },
                Unary {
                    operator: String,
                    operand: Box<ExpressionSnapshot>,
                },
                TypeExpression {
                    type_reference: Box<TypeReferenceSnapshot>,
                },
                ZeroValue {
                    type_reference: Box<TypeReferenceSnapshot>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct StructLiteralFieldSnapshot {
                pub name: String,
                pub value: ExpressionSnapshot,
            }

            pub(crate) fn statement_snapshot(
                program: &TypedTrees,
                statement: &StatementNode,
            ) -> StatementSnapshot {
                match statement {
        StatementNode::Assembly(crate::statement::AssemblyControlPoint::Label(handle)) => {
            StatementSnapshot::AssemblyLabel {
                label: program
                    .statement_table
                    .assembly_block(*handle)
                    .label
                    .to_string(),
            }
        }
        StatementNode::Assembly(crate::statement::AssemblyControlPoint::Entry(handle)) => {
            let graph = program.statement_table.assembly(*handle);
            StatementSnapshot::Assembly {
                primitives: program
                    .statement_table
                    .expression_handles(graph.primitives)
                    .iter()
                    .map(|expression| expression_snapshot(program, *expression))
                    .collect(),
                scratch: graph
                    .scratch
                    .iter()
                    .copied()
                    .filter(|expression| expression.is_valid())
                    .map(|expression| expression_snapshot(program, expression))
                    .collect(),
                statement_count: graph.statement_count,
                blocks: program
                    .statement_table
                    .assembly_blocks(graph.blocks)
                    .iter()
                    .map(|block| AssemblyBlockSnapshot {
                        label: block.label.to_string(),
                        start: block.start,
                        count: block.count,
                        jump: match block.exit {
                            crate::statement::AssemblyExit::Jump(target)
                            | crate::statement::AssemblyExit::ZeroFlag(target, _) => {
                                graph.blocks.handles().position(|handle| handle == target)
                            }
                            _ => None,
                        },
                        zero_flag_when_set: match block.exit {
                            crate::statement::AssemblyExit::ZeroFlag(_, when_set) => Some(when_set),
                            _ => None,
                        },
                        enclosing_transition: matches!(
                            block.exit,
                            crate::statement::AssemblyExit::EnclosingTransition
                        ),
                    })
                    .collect(),
            }
        }
        StatementNode::AssemblyFact(fact) => StatementSnapshot::AssemblyFact {
            contract_kind: match fact.kind {
                crate::statement::AssemblyFactKind::Requires => "requires",
                crate::statement::AssemblyFactKind::Ensures => "ensures",
            },
            fact: super::declaration_snapshots::fact_snapshots_for_span(
                program,
                arena::HandleSpan::from_parts(fact.fact, 1),
            )
            .remove(0),
        },
        StatementNode::Assignment(assignment) => StatementSnapshot::Assignment {
            target: expression_snapshot(program, assignment.target),
            value: expression_snapshot(program, assignment.value),
            receiving_type: assignment
                .receiving_type
                .is_valid()
                .then(|| type_reference_snapshot(program, assignment.receiving_type)),
        },
        StatementNode::Call(call) => StatementSnapshot::Call {
            runtime_call: call.runtime_call.map(|binding| runtime_call_snapshot(program, binding)),
            receiver: (!call.receiver.is_empty())
                .then(|| path_snapshot(program.statement_table.name_path_members(call.receiver))),
            target: call.target.to_string(),
            machine_arguments: call
                .machine_arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
            installed_registrar: call
                .installed_interrupt_operation
                .as_ref()
                .map(|request| snapshot_static_argument(program, &request.registrar)),
            installed_timer_slot: call
                .installed_interrupt_operation
                .as_ref()
                .map(|request| snapshot_static_argument(program, &request.timer_slot)),
            arguments: statement_expression_span_snapshot(program, call.arguments),
            evidence_arguments: to_strings(call.evidence_arguments.iter()),
            acknowledgement_synthesized: call.operational_acknowledgement.origin
                == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
            acknowledges_suspend: call.operational_acknowledgement.acknowledges_suspend,
            acknowledges_block: call.operational_acknowledgement.acknowledges_block,
        },
        StatementNode::Expression(expression) => StatementSnapshot::Expression {
            value: expression_snapshot(program, *expression),
        },
        StatementNode::LocalData(local) => StatementSnapshot::LocalData {
            role: format!("{:?}", local.role),
            name: local.name.to_string(),
            type_reference: type_reference_snapshot(program, local.type_reference),
            initial_value: expression_snapshot_option(program, local.initial_value),
            type_is_inferred: local.type_is_inferred,
            relevance: super::declaration_snapshots::snapshot_binding_relevance(local.relevance),
            pattern_fields: program
                .expression_table
                .match_fields(local.pattern_fields)
                .iter()
                .map(|field| PatternFieldSnapshot {
                    member: field.member.as_str().to_owned(),
                    field: field
                        .field_symbol
                        .is_valid()
                        .then(|| program.symbols.name(field.field_symbol).to_owned()),
                    binding: match &field.action {
                        crate::expression::MatchFieldAction::Bind(binding) => {
                            Some(binding.as_str().to_owned())
                        }
                        crate::expression::MatchFieldAction::Equal(_)
                        | crate::expression::MatchFieldAction::Waive => None,
                    },
                })
                .collect(),
            pattern_subject: local
                .pattern_subject
                .is_valid()
                .then(|| program.symbols.name(local.pattern_subject).to_owned()),
        },
        StatementNode::Transition(transition) => StatementSnapshot::Transition {
            patterns: program
                .statement_table
                .transition_patterns(transition.patterns)
                .iter()
                .map(|retained| {
                    (
                        retained.is_structural,
                        expression_snapshot(program, retained.subject),
                        snapshot_match_pattern(
                            program,
                            MatchPattern::Destructure(retained.pattern),
                        ),
                    )
                })
                .collect(),
            target: transition_target_snapshot(program, transition.target),
            continuation: transition
                .continuation
                .is_valid()
                .then(|| transition_target_snapshot(program, transition.continuation)),
            guard: transition_guard_snapshot(program, transition.guard),
            proof_selectors: program
                .statement_table
                .outcome_proof_selectors(transition.proof_selectors)
                .iter()
                .map(|selector| {
                    (
                        selector.output_field.to_string(),
                        selector.binding.to_string(),
                    )
                })
                .collect(),
            crash_cause: match transition.exit {
                TransitionExit::Ordinary => None,
                TransitionExit::Crash(crate::signature::CrashCause::Trap) => Some("Trap"),
                TransitionExit::Crash(crate::signature::CrashCause::Abort) => Some("Abort"),
            },
        },
    }
            }

            fn transition_guard_snapshot(
                program: &TypedTrees,
                guard: TransitionGuardNode,
            ) -> TransitionGuardSnapshot {
                match guard {
                    TransitionGuardNode::Always => TransitionGuardSnapshot::Always,
                    TransitionGuardNode::When(value) => TransitionGuardSnapshot::When {
                        value: expression_snapshot(program, value),
                    },
                }
            }

            fn transition_target_snapshot(
                program: &TypedTrees,
                target: crate::statement::TransitionTargetHandle,
            ) -> TransitionTargetSnapshot {
                if !target.is_valid() {
                    return TransitionTargetSnapshot::Invalid {
                        handle: target.arena_index(),
                    };
                }

                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named {
                        path,
                        arguments,
                        evidence_arguments,
                        ..
                    } => TransitionTargetSnapshot::Named {
                        path: path_snapshot(
                            program.statement_table.name_path_members(path.members),
                        ),
                        arguments: statement_expression_span_snapshot(program, *arguments),
                        evidence_arguments: evidence_arguments
                            .iter()
                            .map(|name| name.as_str().to_owned())
                            .collect(),
                    },
                    TransitionTargetNode::Value(value) => TransitionTargetSnapshot::Value {
                        value: expression_snapshot(program, *value),
                    },
                    TransitionTargetNode::Terminal => TransitionTargetSnapshot::Terminal,
                }
            }

            pub(crate) fn expression_snapshot_option(
                program: &TypedTrees,
                expression: ExpressionHandle,
            ) -> Option<ExpressionSnapshot> {
                expression
                    .is_valid()
                    .then(|| expression_snapshot(program, expression))
            }

            pub(crate) fn expression_snapshot(
                program: &TypedTrees,
                expression: ExpressionHandle,
            ) -> ExpressionSnapshot {
                if !expression.is_valid() {
                    return ExpressionSnapshot::Invalid {
                        handle: expression.arena_index(),
                    };
                }

                match program.expression_table.expression(expression) {
        ExpressionNode::MachineValue(value) => ExpressionSnapshot::MachineValue {
            type_reference: Box::new(type_reference_snapshot(program, value.type_reference)),
            selected: snapshot_static_argument(program, &value.selected),
            selected_symbol: program.symbols.display_path(value.selected.symbol, "::"),
            application: static_application_snapshot(program, value.application),
            conformance: program.symbols.display_path(value.conformance, "::"),
            requirement: program.symbols.display_path(value.requirement, "::"),
        },
        ExpressionNode::Match(dispatch) => ExpressionSnapshot::Match {
            subject: Box::new(expression_snapshot(program, dispatch.subject)),
            capture: dispatch.capture.is_valid().then_some((
                dispatch.capture.arena_index(),
                dispatch.capture.generation(),
            )),
            arms: program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| MatchArmSnapshot {
                    source_id: arm.source_span.source_id.0,
                    source_start: arm.source_span.span.start,
                    source_end: arm.source_span.span.end,
                    pattern: snapshot_match_pattern(program, arm.pattern),
                    value: expression_snapshot(program, arm.value),
                })
                .collect(),
        },
        ExpressionNode::SavedSubject(saved) => ExpressionSnapshot::SavedSubject {
            component: (saved.component == language_core::PatternComponent::Case).then_some("case"),
            capture: (saved.capture.arena_index(), saved.capture.generation()),
            pattern_head: Box::new(expression_snapshot(program, saved.pattern_head)),
        },
        ExpressionNode::ArrayLiteral(values) => ExpressionSnapshot::ArrayLiteral {
            values: expression_span_snapshot(program, *values),
        },
        ExpressionNode::Binary(binary) => ExpressionSnapshot::Binary {
            left: Box::new(expression_snapshot(program, binary.left)),
            operator: binary.operator.display_name().to_owned(),
            right: Box::new(expression_snapshot(program, binary.right)),
        },
        ExpressionNode::Boolean(value) => ExpressionSnapshot::Boolean { value: *value },
        ExpressionNode::Cast(cast) => ExpressionSnapshot::Cast {
            value: Box::new(expression_snapshot(program, cast.value)),
            target_type: Box::new(type_reference_snapshot(program, cast.target_type)),
            result_type: Box::new(type_reference_snapshot(program, cast.result_type)),
            semantic_domain: path_snapshot(
                program
                    .expression_table
                    .name_path_members(cast.semantic_domain),
            ),
            semantic_domain_arguments: program
                .type_reference_handles(cast.semantic_domain_arguments)
                .iter()
                .map(|argument| type_reference_snapshot(program, *argument))
                .collect(),
            semantic_domain_symbol: cast.semantic_domain_symbol.arena_index(),
            semantic_domain_id: cast.semantic_domain_id.0,
        },
        ExpressionNode::Call(call) => ExpressionSnapshot::Call {
            runtime_call: call.runtime_call.map(|binding| runtime_call_snapshot(program, binding)),
            exact_coercion_target: call
                .exact_coercion_target
                .is_valid()
                .then(|| Box::new(type_reference_snapshot(program, call.exact_coercion_target))),
            receiver: call
                .receiver
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, call.receiver))),
            target: call.target.to_string(),
            machine_arguments: call
                .machine_arguments
                .iter()
                .map(|argument| snapshot_static_argument(program, argument))
                .collect(),
            private_layout_slot: call
                .private_layout_operation
                .as_ref()
                .map(|operation| snapshot_static_argument(program, &operation.selected_slot)),
            installed_registrar: call
                .installed_interrupt_operation
                .as_ref()
                .map(|operation| snapshot_static_argument(program, &operation.registrar)),
            installed_timer_slot: call
                .installed_interrupt_operation
                .as_ref()
                .map(|operation| snapshot_static_argument(program, &operation.timer_slot)),
            arguments: expression_span_snapshot(program, call.arguments),
            evidence_arguments: to_strings(call.evidence_arguments.iter()),
            acknowledgement_synthesized: call.operational_acknowledgement.origin
                == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
            acknowledges_suspend: call.operational_acknowledgement.acknowledges_suspend,
            acknowledges_block: call.operational_acknowledgement.acknowledges_block,
        },
        ExpressionNode::UndeterminedNaN(format) => ExpressionSnapshot::UndeterminedNaN {
            format: format.name().to_owned(),
        },
        ExpressionNode::Float(value) => ExpressionSnapshot::Float {
            value: value.to_string(),
        },
        ExpressionNode::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(expression_snapshot(program, indexed.collection)),
            index: Box::new(expression_snapshot(program, indexed.index)),
        },
        ExpressionNode::Integer(value) => ExpressionSnapshot::Integer {
            text: value.to_string(),
        },
        ExpressionNode::Member(member) => ExpressionSnapshot::Member {
            receiver: Box::new(expression_snapshot(program, member.receiver)),
            member: member.member.to_string(),
        },
        ExpressionNode::Borrow(value) => ExpressionSnapshot::Borrow {
            access: value.access.name(),
            value: Box::new(expression_snapshot(program, value.target)),
        },
        ExpressionNode::Name(path) => ExpressionSnapshot::Name {
            component: (program.pattern_component(expression)
                == Some(language_core::PatternComponent::Case))
            .then_some("case"),
            path: path_snapshot(program.expression_table.name_path_members(path.members)),
        },
        ExpressionNode::Range(range) => ExpressionSnapshot::Range {
            start: range
                .start
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, range.start))),
            end: range
                .end
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, range.end))),
            end_inclusive: range.end_inclusive,
        },
        ExpressionNode::StructLiteral(struct_literal) => ExpressionSnapshot::StructLiteral {
            application_type: struct_literal.application_type.is_valid().then(|| {
                Box::new(type_reference_snapshot(
                    program,
                    struct_literal.application_type,
                ))
            }),
            type_name: struct_literal.type_name.to_string(),
            fields: program
                .expression_table
                .struct_fields(struct_literal.fields)
                .iter()
                .map(|field| StructLiteralFieldSnapshot {
                    name: field.name.to_string(),
                    value: expression_snapshot(program, field.value),
                })
                .collect(),
        },
        ExpressionNode::Atomic(atomic) => ExpressionSnapshot::Atomic {
            value: Box::new(expression_snapshot(program, atomic.value)),
            result: atomic
                .result
                .is_valid()
                .then(|| Box::new(expression_snapshot(program, atomic.result))),
            ordering: format!("{:?}", atomic.ordering),
        },
        ExpressionNode::String(value) => ExpressionSnapshot::String {
            bytes: value.to_vec(),
        },
        ExpressionNode::Unary(unary) => ExpressionSnapshot::Unary {
            operator: unary.operator.display_name().to_owned(),
            operand: Box::new(expression_snapshot(program, unary.operand)),
        },
        ExpressionNode::TypeExpression(type_reference) => ExpressionSnapshot::TypeExpression {
            type_reference: Box::new(type_reference_snapshot(program, *type_reference)),
        },
        ExpressionNode::ZeroValue(type_reference) => ExpressionSnapshot::ZeroValue {
            type_reference: Box::new(type_reference_snapshot(program, *type_reference)),
        },
    }
            }

            pub(crate) fn snapshot_static_argument(
                program: &TypedTrees,
                argument: &crate::expression::StaticMachineArgument,
            ) -> StaticArgumentSnapshot {
                if argument.type_reference.is_valid() {
                    StaticArgumentSnapshot::Type {
                        type_reference: Box::new(type_reference_snapshot(
                            program,
                            argument.type_reference,
                        )),
                    }
                } else if let Some(literal) = &argument.const_literal {
                    StaticArgumentSnapshot::Const(literal.to_string())
                } else if let Some(projection) = &argument.evidence_projection {
                    StaticArgumentSnapshot::EvidenceProjection {
                        term: projection.term.as_str().to_owned(),
                        member: projection.member.as_str().to_owned(),
                    }
                } else if let Some(application) = &argument.application {
                    StaticArgumentSnapshot::Application {
                        member: program
                            .expression_table
                            .static_member(application.member)
                            .map(|member| Box::new(snapshot_static_argument(program, member))),
                        path: path_snapshot(&argument.path),
                        lifetime_arguments: path_snapshot(&application.lifetime_arguments),
                        arguments: application
                            .arguments
                            .iter()
                            .map(|argument| snapshot_static_argument(program, argument))
                            .collect(),
                    }
                } else {
                    StaticArgumentSnapshot::Path(path_snapshot(&argument.path))
                }
            }

            pub(crate) fn expression_span_snapshot(
                program: &TypedTrees,
                expressions: arena::HandleSpan<ExpressionHandle>,
            ) -> Vec<ExpressionSnapshot> {
                program
                    .expression_table
                    .expression_handles(expressions)
                    .iter()
                    .map(|expression| expression_snapshot(program, *expression))
                    .collect()
            }

            fn statement_expression_span_snapshot(
                program: &TypedTrees,
                expressions: arena::HandleSpan<ExpressionHandle>,
            ) -> Vec<ExpressionSnapshot> {
                program
                    .statement_table
                    .expression_handles(expressions)
                    .iter()
                    .map(|expression| expression_snapshot(program, *expression))
                    .collect()
            }

            pub(crate) fn snapshot_match_pattern(
                program: &TypedTrees,
                pattern: MatchPattern,
            ) -> MatchPatternSnapshot {
                match pattern {
                    MatchPattern::Value(value) => MatchPatternSnapshot::Value {
                        value: Box::new(expression_snapshot(program, value)),
                    },
                    MatchPattern::Destructure(pattern) => MatchPatternSnapshot::Destructure {
                        application_type: pattern.application_type.is_valid().then(|| {
                            Box::new(type_reference_snapshot(program, pattern.application_type))
                        }),
                        head: Box::new(expression_snapshot(program, pattern.head)),
                        fields: program
                            .expression_table
                            .match_fields(pattern.fields)
                            .iter()
                            .map(|field| MatchFieldSnapshot {
                                component: (field.component
                                    == language_core::PatternComponent::Case)
                                    .then_some("case"),
                                member: field.member.as_str().to_owned(),
                                action: match &field.action {
                                    MatchFieldAction::Bind(binding) => {
                                        MatchFieldActionSnapshot::Bind {
                                            binding: binding.as_str().to_owned(),
                                        }
                                    }
                                    MatchFieldAction::Equal(value) => {
                                        MatchFieldActionSnapshot::Equal {
                                            value: Box::new(expression_snapshot(program, *value)),
                                        }
                                    }
                                    MatchFieldAction::Waive => MatchFieldActionSnapshot::Waive,
                                },
                            })
                            .collect(),
                        has_rest: pattern.has_rest,
                        guard: pattern
                            .guard
                            .is_valid()
                            .then(|| Box::new(expression_snapshot(program, pattern.guard))),
                        proof_selectors: program
                            .expression_table
                            .match_proof_selectors(pattern.proof_selectors)
                            .iter()
                            .map(|selector| {
                                (
                                    selector.output_field.as_str().to_owned(),
                                    selector.binding.as_str().to_owned(),
                                )
                            })
                            .collect(),
                    },
                    MatchPattern::Wildcard => MatchPatternSnapshot::Wildcard,
                }
            }
        }
        pub(crate) mod type_snapshots {
            use super::to_strings;
            use crate::TypedTrees;
            use crate::inspection::snapshot;
            use crate::name::Identifier;
            use crate::types::{
                DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
            };
            use serde::Serialize;
            use snapshot::declaration_snapshots::{
                establishment_route_snapshot, semantic_roles_snapshot, snapshot_binding_relevance,
            };
            use snapshot::statement_and_expression_snapshots::expression_snapshot;
            use snapshot::{
                DomainEstablishmentRouteSnapshot, DomainSemanticRolesSnapshot, ExpressionSnapshot,
            };

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct WireSchemaSnapshot {
                pub has_symbol: bool,
                pub name: String,
                pub is_public: bool,
                pub encoding: Option<String>,
                pub members: Vec<WireMemberSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum WireMemberSnapshot {
                Field {
                    number: u64,
                    name: String,
                    relevance: &'static str,
                    type_reference: TypeReferenceSnapshot,
                },
                Reserved {
                    number: u64,
                },
                Version {
                    name: String,
                    members: Vec<WireMemberSnapshot>,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum TypeReferenceSnapshot {
                Callable {
                    contract: Box<super::machine_snapshots::RuntimeCallableContractSnapshot>,
                },
                Reference {
                    referee: Box<TypeReferenceSnapshot>,
                    access: &'static str,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    lifetime: Option<String>,
                },
                Constrained {
                    base_type: Box<TypeReferenceSnapshot>,
                    constraints: Vec<TypeConstraintSnapshot>,
                },
                FixedArray {
                    element_type: Box<TypeReferenceSnapshot>,
                    length: String,
                },
                Slice {
                    element_type: Box<TypeReferenceSnapshot>,
                },
                Generic {
                    base_name: String,
                    lifetime_arguments: Vec<String>,
                    arguments: Vec<TypeReferenceSnapshot>,
                },
                ConstExpression {
                    expression: ExpressionSnapshot,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    normalization: Option<OpenIndexNormalizationSnapshot>,
                },
                DynamicTrait {
                    name: String,
                    arguments: Vec<TypeReferenceSnapshot>,
                },
                Named {
                    name: String,
                },
                Unit,
                Invalid {
                    handle: u32,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct OpenIndexNormalizationSnapshot {
                pub index_type: Box<TypeReferenceSnapshot>,
                pub normalizer_version: u32,
                pub operations: Vec<OpenIndexOperationSnapshot>,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            pub struct OpenIndexOperationSnapshot {
                pub expression: ExpressionSnapshot,
                pub spelling: &'static str,
                pub operation_contract_identity: String,
                pub provider: String,
                pub algebra_trait: String,
                pub algebra_requirement: String,
                pub algebra_alias: Option<String>,
                pub commutativity_licensed: bool,
                pub associativity_licensed: bool,
                pub distributes_over: Vec<String>,
                #[serde(skip_serializing_if = "String::is_empty")]
                pub left_identity_literal: String,
                #[serde(skip_serializing_if = "String::is_empty")]
                pub right_identity_literal: String,
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum TypeConstraintSnapshot {
                Named {
                    name: String,
                },
                Domain {
                    name: String,
                    subject: DomainConstraintSubjectSnapshot,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    arguments: Vec<TypeReferenceSnapshot>,
                    symbol: u32,
                    semantic_id: u32,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    classification: Option<&'static str>,
                    predicate_presence: &'static str,
                    semantic_roles: DomainSemanticRolesSnapshot,
                    establishment_routes: Vec<DomainEstablishmentRouteSnapshot>,
                },

                ArithmeticDomain {
                    domain: String,
                },
            }

            #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
            #[serde(tag = "kind", rename_all = "snake_case")]
            pub enum DomainConstraintSubjectSnapshot {
                Declared,
                Carry { permission: &'static str },
                Value { domain: &'static str },
                OmegaLayout { grammar: &'static str },
            }

            pub(crate) fn wire_schema_snapshot(
                program: &TypedTrees,
                wire_schema: &crate::wire::WireSchema,
            ) -> WireSchemaSnapshot {
                WireSchemaSnapshot {
                    has_symbol: wire_schema.symbol.is_valid(),
                    name: wire_schema.name.to_string(),
                    is_public: wire_schema.is_public,
                    encoding: wire_schema
                        .encoding
                        .as_ref()
                        .map(|encoding| encoding.to_string()),
                    members: wire_member_snapshots(program, wire_schema.members),
                }
            }

            fn wire_member_snapshots(
                program: &TypedTrees,
                members: arena::HandleSpan<crate::wire::WireMember>,
            ) -> Vec<WireMemberSnapshot> {
                program
                    .wire_members(members)
                    .iter()
                    .map(|member| match member {
                        crate::wire::WireMember::Field(field) => WireMemberSnapshot::Field {
                            number: field.number,
                            name: field.name.to_string(),
                            relevance: snapshot_binding_relevance(field.relevance),
                            type_reference: type_reference_snapshot(program, field.type_reference),
                        },
                        crate::wire::WireMember::Reserved(reserved) => {
                            WireMemberSnapshot::Reserved {
                                number: reserved.number,
                            }
                        }
                        crate::wire::WireMember::Version(version) => WireMemberSnapshot::Version {
                            name: version.name.to_string(),
                            members: wire_member_snapshots(program, version.members),
                        },
                    })
                    .collect()
            }

            pub(crate) fn type_reference_snapshot_option(
                program: &TypedTrees,
                type_reference: TypeReferenceHandle,
            ) -> Option<TypeReferenceSnapshot> {
                type_reference
                    .is_valid()
                    .then(|| type_reference_snapshot(program, type_reference))
            }

            pub(crate) fn type_reference_snapshot(
                program: &TypedTrees,
                type_reference: TypeReferenceHandle,
            ) -> TypeReferenceSnapshot {
                if !type_reference.is_valid() {
                    return TypeReferenceSnapshot::Invalid {
                        handle: type_reference.arena_index(),
                    };
                }

                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Callable { contract } => TypeReferenceSnapshot::Callable {
                        contract: Box::new(
                            super::machine_snapshots::runtime_callable_contract_snapshot(
                                program, *contract,
                            ),
                        ),
                    },
                    TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    } => TypeReferenceSnapshot::Reference {
                        referee: Box::new(type_reference_snapshot(program, *referee)),
                        access: access.name(),
                        lifetime: lifetime.as_ref().map(ToString::to_string),
                    },
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => TypeReferenceSnapshot::Constrained {
                        base_type: Box::new(type_reference_snapshot(program, *base_type)),
                        constraints: program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .map(|constraint| type_constraint_snapshot(program, constraint))
                            .collect(),
                    },
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => TypeReferenceSnapshot::FixedArray {
                        element_type: Box::new(type_reference_snapshot(program, *element_type)),
                        length: length.to_string(),
                    },
                    TypeReferenceNode::Slice { element_type } => TypeReferenceSnapshot::Slice {
                        element_type: Box::new(type_reference_snapshot(program, *element_type)),
                    },
                    TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments,
                        arguments,
                        ..
                    } => TypeReferenceSnapshot::Generic {
                        base_name: base_name.to_string(),
                        lifetime_arguments: to_strings(lifetime_arguments.iter()),
                        arguments: program
                            .type_reference_handles(*arguments)
                            .iter()
                            .map(|argument| type_reference_snapshot(program, *argument))
                            .collect(),
                    },
                    TypeReferenceNode::ConstExpression(expression) => {
                        TypeReferenceSnapshot::ConstExpression {
                            expression: expression_snapshot(program, *expression),
                            normalization: program
                                .expression_table
                                .index_expression_authority(*expression)
                                .and_then(|authority| authority.normalization.as_ref())
                                .map(|normalization| OpenIndexNormalizationSnapshot {
                                    index_type: Box::new(type_reference_snapshot(
                                        program,
                                        normalization.index_type,
                                    )),
                                    normalizer_version: normalization.normalizer_version,
                                    operations: program
                                        .expression_table
                                        .open_index_operations(*expression)
                                        .map(|operation| OpenIndexOperationSnapshot {
                                            expression: expression_snapshot(
                                                program,
                                                operation.expression,
                                            ),
                                            spelling: operation.spelling.symbol(),
                                            operation_contract_identity: operation
                                                .operation_contract_identity(program),
                                            provider: program
                                                .symbols
                                                .display_path(operation.provider, "::"),
                                            algebra_trait: program
                                                .symbols
                                                .display_path(operation.algebra_trait, "::"),
                                            algebra_requirement: operation
                                                .algebra_requirement
                                                .clone(),
                                            algebra_alias: operation.algebra_alias.clone(),
                                            commutativity_licensed: operation
                                                .commutativity_licensed,
                                            associativity_licensed: operation
                                                .associativity_licensed,
                                            distributes_over: operation.distributes_over.clone(),
                                            left_identity_literal: operation
                                                .left_identity_literal
                                                .clone(),
                                            right_identity_literal: operation
                                                .right_identity_literal
                                                .clone(),
                                        })
                                        .collect(),
                                }),
                        }
                    }
                    TypeReferenceNode::DynamicTrait {
                        name, arguments, ..
                    } => TypeReferenceSnapshot::DynamicTrait {
                        name: name.to_string(),
                        arguments: program
                            .type_reference_handles(*arguments)
                            .iter()
                            .map(|argument| type_reference_snapshot(program, *argument))
                            .collect(),
                    },
                    TypeReferenceNode::Named { name, .. } => TypeReferenceSnapshot::Named {
                        name: name.to_string(),
                    },
                    TypeReferenceNode::Unit => TypeReferenceSnapshot::Unit,
                }
            }

            pub(crate) fn type_constraint_snapshot(
                program: &TypedTrees,
                constraint: &TypeConstraintNode,
            ) -> TypeConstraintSnapshot {
                match constraint {
                    TypeConstraintNode::Named(name) => TypeConstraintSnapshot::Named {
                        name: name.to_string(),
                    },
                    TypeConstraintNode::Domain(domain) => TypeConstraintSnapshot::Domain {
                        name: domain.name.to_string(),
                        subject: match domain.subject {
                            DomainConstraintSubject::Declared => {
                                DomainConstraintSubjectSnapshot::Declared
                            }
                            DomainConstraintSubject::Carry(permission) => {
                                DomainConstraintSubjectSnapshot::Carry {
                                    permission: permission.name(),
                                }
                            }
                            DomainConstraintSubject::Value(value_domain) => {
                                DomainConstraintSubjectSnapshot::Value {
                                    domain: value_domain.name(),
                                }
                            }
                            DomainConstraintSubject::OmegaLayout { grammar } => {
                                DomainConstraintSubjectSnapshot::OmegaLayout {
                                    grammar: grammar.as_str(),
                                }
                            }
                        },
                        arguments: domain
                            .arguments
                            .iter()
                            .map(|argument| type_reference_snapshot(program, *argument))
                            .collect(),
                        symbol: domain.symbol.arena_index(),
                        semantic_id: domain.semantic_id.0,
                        classification: domain.classification.map(|value| value.as_str()),
                        predicate_presence: domain.predicate_presence.as_str(),
                        semantic_roles: semantic_roles_snapshot(domain.semantic_roles),
                        establishment_routes: domain
                            .establishment_routes
                            .iter()
                            .copied()
                            .map(establishment_route_snapshot)
                            .collect(),
                    },

                    TypeConstraintNode::ArithmeticDomain(domain) => {
                        TypeConstraintSnapshot::ArithmeticDomain {
                            domain: domain.name().to_owned(),
                        }
                    }
                }
            }

            pub(crate) fn path_snapshot(path: &[Identifier]) -> Vec<String> {
                to_strings(path.iter())
            }
        }

        pub use declaration_snapshots::{
            CleanupSelectionSnapshot, ConstDeclarationSnapshot, DataDefinitionSnapshot,
            DataMemberSnapshot, DataPayloadFieldSnapshot, DomainAliasConstituentSnapshot,
            DomainDefinitionSnapshot, DomainEstablishmentRouteSnapshot,
            DomainSemanticRolesSnapshot, DomainTypeParameterSnapshot, EvaluatedConstantSnapshot,
            MathematicalBinderSnapshot, MathematicalBodySnapshot, MathematicalDefinitionSnapshot,
            MathematicalParameterSnapshot, MathematicalTypeSnapshot, MeasureDefinitionSnapshot,
            OperatorDefinitionSnapshot, ProofFactSnapshot, PropositionBinderSnapshot,
            PropositionBodySnapshot, PropositionFormulaSnapshot, PropositionSnapshot,
            QuotientDefinitionSnapshot, QuotientEquivalenceSelectionSnapshot,
        };
        pub use machine_snapshots::{
            ConformanceRowSnapshot, ConformanceSnapshot, GenericConformanceBoundSnapshot,
            MachineSnapshot, MachineSupplySnapshot, NativeCallbackParameterSnapshot,
            OwnedDataSnapshot, ProgressPremiseSnapshot, RankRangeSnapshot, RankingWitnessSnapshot,
            SignatureContractSnapshot, StateParameterSnapshot, StateSignatureSnapshot,
            StateSnapshot, TerminationGuaranteeSnapshot, TerminationInterfaceSnapshot,
            TraitSnapshot,
        };
        pub use statement_and_expression_snapshots::{
            ExpressionSnapshot, MatchArmSnapshot, MatchPatternSnapshot, StatementSnapshot,
            StaticArgumentSnapshot, StructLiteralFieldSnapshot, TransitionGuardSnapshot,
            TransitionTargetSnapshot,
        };
        pub use type_snapshots::{
            DomainConstraintSubjectSnapshot, OpenIndexNormalizationSnapshot,
            OpenIndexOperationSnapshot, TypeConstraintSnapshot, TypeReferenceSnapshot,
            WireMemberSnapshot, WireSchemaSnapshot,
        };

        use crate::TypedTrees;
        use crate::inspection::snapshot::declaration_snapshots::data_definition_snapshot;
        use crate::inspection::snapshot::declaration_snapshots::domain_definition_snapshot;
        use crate::inspection::snapshot::declaration_snapshots::mathematical_definition_snapshot;
        use crate::inspection::snapshot::declaration_snapshots::measure_snapshot;
        use crate::inspection::snapshot::declaration_snapshots::operator_snapshot;
        use crate::inspection::snapshot::declaration_snapshots::proposition_snapshot;
        use crate::inspection::snapshot::machine_snapshots::conformance_snapshot;
        use crate::inspection::snapshot::machine_snapshots::machine_snapshot;
        use crate::inspection::snapshot::machine_snapshots::trait_definition_snapshot;
        use crate::inspection::snapshot::statement_and_expression_snapshots::expression_snapshot;
        use crate::inspection::snapshot::type_snapshots::type_reference_snapshot;
        use crate::inspection::snapshot::type_snapshots::wire_schema_snapshot;
        use serde::Serialize;

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct TypedTreesSnapshot {
            pub roots: TypedRootsSnapshot,
            pub tables: TypedTableSnapshot,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub external_bindings: Vec<ExternalBindingSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub authored_declaration_selections: Vec<AuthoredDeclarationSelectionSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub evidence_forwardings: Vec<EvidenceForwardingSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub proof_output_calls: Vec<ProofOutputCallSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub service_reaches: Vec<ServiceReachDefinitionSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub authored_service_reach_rows: Vec<AuthoredServiceReachRowSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub machine_specializations: Vec<MachineSpecializationSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub ranking_expression_custody: Vec<RankingExpressionCustodySnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub pending_const_range_endpoints: Vec<u32>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct EvidenceForwardingSnapshot {
            pub machine_symbol: u32,
            pub state_symbol: u32,
            pub statement_index: usize,
            pub source_statement_index: usize,
            pub target: String,
            pub source: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub source_conformance: Option<u32>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct AuthoredDeclarationSelectionSnapshot {
            pub occurrence_id: u64,
            pub source_id: usize,
            pub source_start: usize,
            pub source_end: usize,
            pub exposure: &'static str,
            pub kind: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub compiler_partition: Option<u64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub top_level_static_argument_position: Option<usize>,
            pub target: AuthoredDeclarationSelectionTargetSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum AuthoredDeclarationSelectionTargetSnapshot {
            Resolved {
                selected_symbol: u32,
            },
            Intrinsic {
                intrinsic: &'static str,
                #[serde(skip_serializing_if = "Option::is_none")]
                detail: Option<String>,
            },
            LateBound {
                binding: &'static str,
            },
        }

        fn authored_declaration_selection_kind_name(
            kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
        ) -> &'static str {
            use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
            match kind {
                Kind::TypeReference => "type_reference",
                Kind::StaticPathSegment => "static_path_segment",
                Kind::MemberAccess => "member_access",
                Kind::StructLiteralType => "struct_literal_type",
                Kind::StructLiteralCase => "struct_literal_case",
                Kind::StructLiteralField => "struct_literal_field",
                Kind::CaseReference => "case_reference",
                Kind::CaseMembership => "case_membership",
                Kind::DomainMembership => "domain_membership",
                Kind::DomainIssuerAuthorization => "domain_issuer_authorization",
                Kind::DomainCleanupSelection => "domain_cleanup_selection",
                Kind::Call => "call",
                Kind::StaticArgument => "static_argument",
                Kind::Operator => "operator",
                Kind::Conformance => "conformance",
            }
        }

        fn authored_declaration_selection_late_binding_name(
            binding: language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding,
        ) -> &'static str {
            use language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding as Binding;
            match binding {
                Binding::CheckedStaticPathSegment => "checked_static_path_segment",
                Binding::CheckedMember => "checked_member",
                Binding::CheckedStructLiteralType => "checked_struct_literal_type",
                Binding::CheckedStructLiteralCase => "checked_struct_literal_case",
                Binding::CheckedStructLiteralField => "checked_struct_literal_field",
                Binding::CheckedCaseMembership => "checked_case_membership",
                Binding::CheckedDomainMembership => "checked_domain_membership",
                Binding::CheckedCall => "checked_call",
                Binding::CheckedStaticArgument => "checked_static_argument",
                Binding::CheckedOperator => "checked_operator",
                Binding::CheckedConformance => "checked_conformance",
            }
        }

        fn authored_declaration_selection_intrinsic_name(
            intrinsic: language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic,
        ) -> (&'static str, Option<String>) {
            use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;
            match intrinsic {
                Intrinsic::BuildNativeBinding => ("build_native_binding", None),
                Intrinsic::StorageAddress => ("storage_address", None),
                Intrinsic::BuiltinOperator => ("builtin_operator", None),
                Intrinsic::CarryPermission(permission) => {
                    ("carry_permission", Some(format!("{permission:?}")))
                }
                Intrinsic::CollectionLength => ("collection_length", None),
                Intrinsic::CollectionCapacity => ("collection_capacity", None),
                Intrinsic::ByteSequencePredicate(predicate) => {
                    ("byte_sequence_predicate", Some(format!("{predicate:?}")))
                }
                Intrinsic::BuildProductOperand {
                    operation,
                    position,
                    role,
                } => (
                    "build_product_operand",
                    Some(format!("{operation:?}:{position}:{role:?}")),
                ),
                Intrinsic::BuildProviderReadiness => ("build_provider_readiness", None),
                Intrinsic::BuildProviderSelection => ("build_provider_selection", None),
                Intrinsic::BuildAssumedBindingContract => ("build_assumed_binding_contract", None),
                Intrinsic::BuildRepresentationSelection => ("build_representation_selection", None),
                Intrinsic::BuildServiceExclusion => ("build_service_exclusion", None),
                Intrinsic::BuildServiceAudit => ("build_service_audit", None),
                Intrinsic::BuildTestGroupRegistration => ("build_test_group_registration", None),
                Intrinsic::BuildTestGroupService => ("build_test_group_service", None),
                Intrinsic::BuildOptimizationSelection => ("build_optimization_selection", None),
                Intrinsic::BuildOptimizationReportRequest => {
                    ("build_optimization_report_request", None)
                }
                Intrinsic::BuildBoundaryAcceptance => ("build_boundary_acceptance", None),
                Intrinsic::BuildWireCompatibilityRequest => {
                    ("build_wire_compatibility_request", None)
                }
                Intrinsic::WireCompatibilityFact(fact) => {
                    ("wire_compatibility_fact", Some(format!("{fact:?}")))
                }
                Intrinsic::BuildIncludedSourceHandoff => ("build_included_source_handoff", None),
                Intrinsic::BuildLogWriteLine => ("build_log_write_line", None),
                Intrinsic::WireEncode => ("wire_encode", None),
                Intrinsic::WireDecode => ("wire_decode", None),
                Intrinsic::InlineAssemblyOperation => ("inline_assembly_operation", None),
                Intrinsic::ProofView => ("proof_view", None),
                Intrinsic::QuotientDefine => ("quotient_define", None),
                Intrinsic::QuotientLift => ("quotient_lift", None),
            }
        }

        fn authored_declaration_selection_target_snapshot(
            target: language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget,
        ) -> AuthoredDeclarationSelectionTargetSnapshot {
            use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
            match target {
                Target::Resolved(selected) => {
                    AuthoredDeclarationSelectionTargetSnapshot::Resolved {
                        selected_symbol: selected.selected_symbol().arena_index(),
                    }
                }
                Target::Intrinsic(intrinsic) => {
                    let (intrinsic, detail) =
                        authored_declaration_selection_intrinsic_name(intrinsic);
                    AuthoredDeclarationSelectionTargetSnapshot::Intrinsic { intrinsic, detail }
                }
                Target::LateBound(binding) => {
                    AuthoredDeclarationSelectionTargetSnapshot::LateBound {
                        binding: authored_declaration_selection_late_binding_name(binding),
                    }
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct ProofOutputCallSnapshot {
            pub machine_symbol: u32,
            pub state_symbol: u32,
            pub statement_index: usize,
            pub source_statement_index: usize,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub runtime_call_statement_index: Option<usize>,
            pub bindings: Vec<(String, String)>,
            pub call: ExpressionSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct SourceSpanSnapshot {
            pub source_id: usize,
            pub start: usize,
            pub end: usize,
        }

        pub(crate) fn source_span_snapshot(span: &source::SourceSpan) -> SourceSpanSnapshot {
            SourceSpanSnapshot {
                source_id: span.source_id.0,
                start: span.span.start,
                end: span.span.end,
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct ServiceReachDefinitionSnapshot {
            pub id: u32,
            pub symbol: u32,
            pub name: String,
            pub parents: Vec<u32>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct AuthoredServiceReachTargetSnapshot {
            pub service: u32,
            pub span: SourceSpanSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct AuthoredServiceReachRowSnapshot {
            pub owner: u32,
            pub keyword_spans: Vec<SourceSpanSnapshot>,
            pub targets: Vec<AuthoredServiceReachTargetSnapshot>,
            pub installation_bound: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct MachineSpecializationSnapshot {
            pub template: u32,
            pub instance: u32,
            pub template_parameter_count: usize,
            pub type_arguments: Vec<String>,
            pub const_arguments: Vec<String>,
            pub type_argument_identities: Vec<String>,
            pub const_argument_identities: Vec<String>,
            pub machine_arguments: Vec<u32>,
            pub conformance_arguments: Vec<u32>,
            pub conformance_application_count: usize,
            pub operator_realization_count: usize,
            pub normalized_template_identity: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub accepted_template_commitment: Option<String>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct RankingExpressionCustodySnapshot {
            pub machine: u32,
            pub subjects: Vec<u32>,
            pub view_arguments: Vec<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub rank_range: Option<u32>,
        }

        impl TypedTreesSnapshot {
            pub fn from_typed_trees(program: &TypedTrees) -> Self {
                Self {
            roots: TypedRootsSnapshot {
                const_declarations: program
                    .const_declarations()
                    .iter()
                    .map(|declaration| ConstDeclarationSnapshot {
                        has_symbol: declaration.symbol.is_valid(),
                        name: program.symbols.display_path(declaration.symbol, "::"),
                        is_public: declaration.is_public,
                        declared_type: type_reference_snapshot(program, declaration.declared_type),
                        evaluated_value: declaration.evaluated_value.as_ref().map(Into::into),
                    })
                    .collect(),
                conformances: program
                    .conformances()
                    .iter()
                    .map(|conformance| conformance_snapshot(program, conformance))
                    .collect(),
                data_definitions: program
                    .data_definitions()
                    .iter()
                    .map(|data| data_definition_snapshot(program, data))
                    .collect(),
                domain_definitions: program
                    .domain_definitions()
                    .iter()
                    .map(|domain| domain_definition_snapshot(program, domain))
                    .collect(),
                machines: program
                    .machines()
                    .iter()
                    .map(|machine| machine_snapshot(program, machine))
                    .collect(),
                measures: program
                    .measures()
                    .iter()
                    .map(|measure| measure_snapshot(program, measure))
                    .collect(),
                mathematical_definitions: program
                    .mathematical_definitions()
                    .iter()
                    .map(|definition| mathematical_definition_snapshot(program, definition))
                    .collect(),
                operators: program
                    .operators()
                    .iter()
                    .map(|operator| operator_snapshot(program, operator))
                    .collect(),
                machine_token_bindings: program
                    .machine_token_bindings()
                    .iter()
                    .map(|operator| operator_snapshot(program, operator))
                    .collect(),
                propositions: program
                    .propositions()
                    .iter()
                    .map(|proposition| proposition_snapshot(program, proposition))
                    .collect(),
                traits: program
                    .traits()
                    .iter()
                    .map(|trait_definition| trait_definition_snapshot(program, trait_definition))
                    .collect(),
                wire_schemas: program
                    .wire_schemas()
                    .iter()
                    .map(|wire_schema| wire_schema_snapshot(program, wire_schema))
                    .collect(),
            },
            tables: TypedTableSnapshot {
                authored_declaration_selection_count: program
                    .authored_declaration_selections()
                    .len(),
                const_declaration_count: program.const_declarations.len(),
                data_definition_count: program.data_definitions.len(),
                data_application_row_count: program.data_application_definitions.len(),
                data_type_parameter_count: program.data_type_parameters.len(),
                data_member_count: program.data_members.len(),
                data_payload_field_count: program.data_payload_fields.len(),
                domain_definition_count: program.domain_definitions.len(),
                proof_fact_count: program.proof_facts.len(),
                proof_fact_source_span_count: program
                    .tables
                    .proof_fact_source_spans
                    .iter()
                    .filter(|span| span.is_some())
                    .count(),
                proposition_count: program.propositions.len(),
                proposition_binder_count: program.proposition_binders.len(),
                mathematical_definition_count: program.mathematical_definitions.len(),
                mathematical_parameter_count: program.mathematical_parameters.len(),
                mathematical_type_count: program.mathematical_types.len(),
                domain_path_member_count: program.domain_path_members.len(),
                operator_path_member_count: program.operator_path_members.len(),
                machine_count: program.machines.len(),
                measure_count: program.measures.len(),
                measure_path_member_count: program.measure_path_members.len(),
                operator_count: program.operators.len(),
                machine_owned_data_count: program.machine_owned_data.len(),
                machine_trait_conformance_count: program.machine_trait_conformances.len(),
                machine_state_count: program.machine_states.len(),
                state_parameter_count: program.state_parameters.len(),
                trait_count: program.traits.len(),
                conformance_count: program.conformances.len(),
                trait_requirement_count: program.trait_requirements.len(),
                trait_machine_signature_count: program.trait_machine_signatures.len(),
                signature_invoke_count: program.signature_invokes.len(),
                signature_contract_count: program.signature_contracts.len(),
                expression_count: program.expression_table.expression_count(),
                expression_struct_field_count: program.expression_table.struct_field_count(),
                statement_count: program.statement_table.statement_count(),
                transition_target_count: program.statement_table.transition_target_count(),
                type_reference_count: program.type_reference_table.type_reference_count(),
                type_constraint_count: program.type_reference_table.constraint_count(),
                wire_schema_count: program.wire_schemas.len(),
                wire_member_count: program.wire_members.len(),
                symbol_count: program.symbols.symbols().len(),
                service_reach_definition_count: program.service_reaches.definitions().len(),
                authored_service_reach_row_count: program.authored_service_reach_rows.len(),
                external_binding_count: program.external_bindings.identities().len(),
                plan_laid_layout_count: program.plan_laid_layouts.len(),
                access_view_plan_count: program.access_view_plans.len(),
                wire_placement_count: program.wire_placements.len(),
                wire_encode_obligation_count: program.wire_encode_obligations.len(),
                wire_schema_plan_count: program.wire_schema_plans.len(),
                machine_specialization_count: program.machine_specializations.len(),
                open_index_normalization_count: program.expression_table.open_index_normalization_count(),
                evidence_forwarding_count: program.evidence_forwardings.len(),
                proof_output_call_count: program.proof_output_calls.len(),
                ranking_expression_custody_count: program.ranking_expression_custody.len(),
                pending_const_range_endpoint_count: program.pending_const_range_endpoints.len(),
            },
            authored_declaration_selections: program
                .authored_declaration_selections()
                .iter()
                .map(|selection| AuthoredDeclarationSelectionSnapshot {
                    occurrence_id: selection.occurrence_id().ordinal(),
                    source_id: selection.source_span().source_id.0,
                    source_start: selection.source_span().span.start,
                    source_end: selection.source_span().span.end,
                    exposure: match selection.exposure() {
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation => {
                            "private_implementation"
                        }
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface => {
                            "public_interface"
                        }
                    },
                    kind: authored_declaration_selection_kind_name(selection.kind()),
                    top_level_static_argument_position: selection.top_level_static_argument_position(),
                    compiler_partition: selection
                        .compiler_partition()
                        .map(|partition| partition.ordinal()),
                    target: authored_declaration_selection_target_snapshot(selection.target()),
                })
                .collect(),
            external_bindings: program
                .external_bindings
                .identities()
                .map(|(identity, binding)| ExternalBindingSnapshot {
                    identity: identity.0,
                    binding: match binding {
                        language_semantics::ExternalBindingIdentity::Syscall { number } => {
                            ExternalBindingValueSnapshot::Syscall { number: *number }
                        }
                        language_semantics::ExternalBindingIdentity::CompilerIntrinsic => {
                            ExternalBindingValueSnapshot::CompilerIntrinsic
                        }
                        language_semantics::ExternalBindingIdentity::VtableSlot { index } => {
                            ExternalBindingValueSnapshot::VtableSlot { index: *index }
                        }
                        language_semantics::ExternalBindingIdentity::VtableField { field } => {
                            ExternalBindingValueSnapshot::VtableField {
                                field: field.clone(),
                            }
                        }
                        language_semantics::ExternalBindingIdentity::TableFunction { field } => {
                            ExternalBindingValueSnapshot::TableFunction {
                                field: field.clone(),
                            }
                        }
                    },
                })
                .collect(),
            evidence_forwardings: program
                .evidence_forwardings
                .iter()
                .map(|forwarding| EvidenceForwardingSnapshot {
                    machine_symbol: forwarding.machine_symbol.arena_index(),
                    state_symbol: forwarding.state_symbol.arena_index(),
                    statement_index: forwarding.statement_index,
                    source_statement_index: forwarding.source_statement_index,
                    target: forwarding.target.to_string(),
                    source: forwarding.source.to_string(),
                    source_conformance: forwarding
                        .source_conformance
                        .map(|symbol| symbol.arena_index()),
                })
                .collect(),
            proof_output_calls: program
                .proof_output_calls
                .iter()
                .map(|package| ProofOutputCallSnapshot {
                    machine_symbol: package.machine_symbol.arena_index(),
                    state_symbol: package.state_symbol.arena_index(),
                    statement_index: package.statement_index,
                    source_statement_index: package.source_statement_index,
                    runtime_call_statement_index: package.runtime_call_statement_index,
                    bindings: package
                        .bindings
                        .iter()
                        .map(|binding| {
                            (
                                binding.output_field.to_string(),
                                binding.binding.to_string(),
                            )
                        })
                        .collect(),
                    call: expression_snapshot(program, package.call),
                })
                .collect(),
            service_reaches: program
                .service_reaches
                .definitions()
                .iter()
                .enumerate()
                .map(|(index, definition)| ServiceReachDefinitionSnapshot {
                    id: u32::try_from(index + 1).expect("service reach table fits u32"),
                    symbol: definition.symbol.arena_index(),
                    name: definition.name.clone(),
                    parents: definition.parents.iter().map(|parent| parent.0).collect(),
                })
                .collect(),
            authored_service_reach_rows: program
                .authored_service_reach_rows
                .iter()
                .map(|row| AuthoredServiceReachRowSnapshot {
                    owner: row.owner.arena_index(),
                    keyword_spans: row
                        .keyword_source_spans
                        .iter()
                        .map(source_span_snapshot)
                        .collect(),
                    targets: row
                        .targets
                        .iter()
                        .map(|target| AuthoredServiceReachTargetSnapshot {
                            service: target.service.arena_index(),
                            span: source_span_snapshot(&target.source_span),
                        })
                        .collect(),
                    installation_bound: row.installation_bound,
                })
                .collect(),
            machine_specializations: program
                .machine_specializations
                .iter()
                .map(|specialization| MachineSpecializationSnapshot {
                    template: specialization.template.arena_index(),
                    instance: specialization.instance.arena_index(),
                    template_parameter_count: specialization.template_parameters.len(),
                    type_arguments: specialization.type_arguments.clone(),
                    const_arguments: specialization.const_arguments.clone(),
                    type_argument_identities: specialization
                        .type_argument_identities
                        .clone(),
                    const_argument_identities: specialization
                        .const_argument_identities
                        .clone(),
                    machine_arguments: specialization
                        .machine_arguments
                        .iter()
                        .map(|symbol| symbol.arena_index())
                        .collect(),
                    conformance_arguments: specialization
                        .conformance_arguments
                        .iter()
                        .map(|symbol| symbol.arena_index())
                        .collect(),
                    conformance_application_count: specialization
                        .conformance_applications
                        .len(),
                    operator_realization_count: specialization.operator_realizations.len(),
                    normalized_template_identity: specialization
                        .normalized_template_identity
                        .clone(),
                    accepted_template_commitment: specialization
                        .accepted_template_commitment
                        .clone(),
                })
                .collect(),
            ranking_expression_custody: program
                .ranking_expression_custody
                .iter()
                .map(|custody| RankingExpressionCustodySnapshot {
                    machine: custody.machine.arena_index(),
                    subjects: custody
                        .subjects
                        .iter()
                        .map(|handle| handle.arena_index())
                        .collect(),
                    view_arguments: custody
                        .view_arguments
                        .iter()
                        .map(|handle| handle.arena_index())
                        .collect(),
                    rank_range: custody.rank_range.map(|handle| handle.arena_index()),
                })
                .collect(),
            pending_const_range_endpoints: {
                let mut endpoints: Vec<u32> = program
                    .pending_const_range_endpoints
                    .iter()
                    .map(|handle| handle.arena_index())
                    .collect();
                endpoints.sort_unstable();
                endpoints
            },
        }
            }

            pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
                serde_json::to_string_pretty(self)
            }

            pub fn to_json(&self) -> Result<String, serde_json::Error> {
                serde_json::to_string(self)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct ExternalBindingSnapshot {
            pub identity: u32,
            #[serde(flatten)]
            pub binding: ExternalBindingValueSnapshot,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum ExternalBindingValueSnapshot {
            Syscall { number: i64 },
            CompilerIntrinsic,
            VtableSlot { index: i64 },
            VtableField { field: String },
            TableFunction { field: String },
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct TypedRootsSnapshot {
            pub const_declarations: Vec<ConstDeclarationSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub conformances: Vec<ConformanceSnapshot>,
            pub data_definitions: Vec<DataDefinitionSnapshot>,
            pub domain_definitions: Vec<DomainDefinitionSnapshot>,
            pub machines: Vec<MachineSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub measures: Vec<MeasureDefinitionSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub mathematical_definitions: Vec<MathematicalDefinitionSnapshot>,
            pub operators: Vec<OperatorDefinitionSnapshot>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            pub machine_token_bindings: Vec<OperatorDefinitionSnapshot>,
            pub propositions: Vec<PropositionSnapshot>,
            pub traits: Vec<TraitSnapshot>,
            pub wire_schemas: Vec<WireSchemaSnapshot>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct TypedTableSnapshot {
            pub authored_declaration_selection_count: usize,
            pub const_declaration_count: usize,
            pub data_definition_count: usize,
            pub data_application_row_count: usize,
            pub data_type_parameter_count: usize,
            pub data_member_count: usize,
            pub data_payload_field_count: usize,
            pub domain_definition_count: usize,
            pub proof_fact_count: usize,
            pub proof_fact_source_span_count: usize,
            pub proposition_count: usize,
            pub proposition_binder_count: usize,
            pub mathematical_definition_count: usize,
            pub mathematical_parameter_count: usize,
            pub mathematical_type_count: usize,
            pub domain_path_member_count: usize,
            pub operator_path_member_count: usize,
            pub machine_count: usize,
            pub measure_count: usize,
            pub measure_path_member_count: usize,
            pub operator_count: usize,
            pub machine_owned_data_count: usize,
            pub machine_trait_conformance_count: usize,
            pub machine_state_count: usize,
            pub state_parameter_count: usize,
            pub trait_count: usize,
            pub conformance_count: usize,
            pub trait_requirement_count: usize,
            pub trait_machine_signature_count: usize,
            pub signature_invoke_count: usize,
            pub signature_contract_count: usize,
            pub expression_count: usize,
            pub expression_struct_field_count: usize,
            pub statement_count: usize,
            pub transition_target_count: usize,
            pub type_reference_count: usize,
            pub type_constraint_count: usize,
            pub wire_schema_count: usize,
            pub wire_member_count: usize,
            pub symbol_count: usize,
            pub service_reach_definition_count: usize,
            pub authored_service_reach_row_count: usize,
            pub external_binding_count: usize,
            pub plan_laid_layout_count: usize,
            pub access_view_plan_count: usize,
            pub wire_placement_count: usize,
            pub wire_encode_obligation_count: usize,
            pub wire_schema_plan_count: usize,
            pub machine_specialization_count: usize,
            pub open_index_normalization_count: usize,
            pub evidence_forwarding_count: usize,
            pub proof_output_call_count: usize,
            pub ranking_expression_custody_count: usize,
            pub pending_const_range_endpoint_count: usize,
        }

        pub(crate) fn to_strings<T: ToString>(values: impl IntoIterator<Item = T>) -> Vec<String> {
            values.into_iter().map(|value| value.to_string()).collect()
        }
    }

    impl crate::TypedTrees {
        pub fn diagnostic_expression_name(
            &self,
            expression: crate::expression::ExpressionHandle,
        ) -> String {
            self.expression_table
                .display_name_resolving(expression, &|mut current| {
                    for _ in 0..self.expression_table.expression_count() {
                        let Some(path) = self.expression_table.name_path_node(current) else {
                            break;
                        };
                        if !path.symbol.is_valid() {
                            break;
                        }
                        let authored = self.symbols.authored_origin(path.symbol);
                        if self.symbols.get(authored).kind == symbols::SymbolKind::Local
                            && self.symbols.symbol_source_span(authored).is_some()
                        {
                            break;
                        }
                        let Some(state) =
                            self.state_by_symbol(self.symbols.get(path.symbol).parent)
                        else {
                            break;
                        };
                        let Some((_, local)) = self.state_local_declaration(state, path.symbol)
                        else {
                            break;
                        };
                        if local.is_mutable || !local.initial_value.is_valid() {
                            break;
                        }
                        current = local.initial_value;
                    }
                    current
                })
        }
    }
}
