/*
We compute and intern canonical, binder-aware type identities (`type_identity`)
across open indices, constraints, lifetimes, portable machine applications, and
callable signatures in `TypedTrees`.
*/

pub(crate) mod callable_identity {
    use super::constraint_identity::{atom, compound};
    use super::coordinates::CoordinateTag;
    use super::{NormalizedResultDispatchSet, TypeIdentityId};
    use crate::TypedTrees;
    use crate::machine::Machine;
    use crate::operator::OperatorDefinition;
    use crate::signature::StateSignature;
    use crate::trait_definition::TraitDefinition;

    type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

    #[derive(
        Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct CallableIdentityId(u32);

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct CallablePathId(u32);

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct CallableParametersId(u32);

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct CallableResultDispatchId(u32);

    macro_rules! interned_handle {
        ($name:ident) => {
            impl $name {
                pub fn is_valid(self) -> bool {
                    self.0 != 0
                }

                fn slot(self) -> Option<usize> {
                    (self.0 != 0).then(|| self.0 as usize - 1)
                }

                fn from_slot(slot: usize) -> Self {
                    Self(slot as u32 + 1)
                }
            }
        };
    }

    interned_handle!(CallableIdentityId);
    interned_handle!(CallablePathId);
    interned_handle!(CallableParametersId);
    interned_handle!(CallableResultDispatchId);

    pub(crate) struct CallableIdentityParts {
        pub(crate) path: String,
        pub(crate) parameters: String,
        pub(crate) result_dispatch: NormalizedResultDispatchSet,
    }

    struct BoundaryOperatorIdentityParts {
        path: String,
        parameter_types: Vec<TypeIdentityId>,
        result_type: TypeIdentityId,
        parameters: String,
        result_dispatch: NormalizedResultDispatchSet,
        text: String,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum DeclarationKey {
        Machine(usize),
        TraitRequirement(usize, usize),
        Operator(usize),
        BoundaryOperatorRequirement(usize),
    }

    #[derive(Clone, compiler_storage::Record)]
    struct CallableIdentityRecord {
        path: CallablePathId,
        parameters: CallableParametersId,
        result_dispatch: CallableResultDispatchId,
        text: Box<str>,
    }

    #[derive(Clone, Default, compiler_storage::Record)]
    pub struct CallableIdentityTable {
        paths: Vec<Box<str>>,
        path_slots: HashMap<Box<str>, CallablePathId>,
        parameters: Vec<Box<str>>,
        parameter_slots: HashMap<Box<str>, CallableParametersId>,
        result_dispatches: Vec<NormalizedResultDispatchSet>,
        result_dispatch_slots:
            std::collections::HashMap<NormalizedResultDispatchSet, CallableResultDispatchId>,
        records: Vec<CallableIdentityRecord>,
        record_slots: HashMap<
            (
                CallablePathId,
                CallableParametersId,
                CallableResultDispatchId,
            ),
            CallableIdentityId,
        >,
        boundary_operator_slots:
            HashMap<(CallablePathId, Vec<TypeIdentityId>, TypeIdentityId), CallableIdentityId>,
    }

    #[derive(Default)]
    pub struct CallableIdentityMemo {
        by_text: HashMap<Box<str>, CallableIdentityId>,
        by_declaration: HashMap<DeclarationKey, CallableIdentityId>,
        machines: Vec<Vec<u32>>,
        swept: bool,
    }

    impl Clone for CallableIdentityMemo {
        fn clone(&self) -> Self {
            Self {
                by_declaration: HashMap::default(),
                by_text: self.by_text.clone(),
                machines: self.machines.clone(),
                swept: self.swept,
            }
        }
    }

    impl CallableIdentityTable {
        fn intern_path(&mut self, path: &str) -> CallablePathId {
            super::intern_boxed_text(
                path,
                &mut self.paths,
                &mut self.path_slots,
                CallablePathId::from_slot,
            )
        }

        fn intern_parameters(&mut self, parameters: &str) -> CallableParametersId {
            super::intern_boxed_text(
                parameters,
                &mut self.parameters,
                &mut self.parameter_slots,
                CallableParametersId::from_slot,
            )
        }

        fn intern_result_dispatch(
            &mut self,
            result_dispatch: NormalizedResultDispatchSet,
        ) -> CallableResultDispatchId {
            super::intern_owned(
                result_dispatch,
                &mut self.result_dispatches,
                &mut self.result_dispatch_slots,
                CallableResultDispatchId::from_slot,
            )
        }

        fn intern(&mut self, parts: CallableIdentityParts) -> CallableIdentityId {
            let dispatch_text = parts.result_dispatch.identity();
            let path = self.intern_path(&parts.path);
            let parameters = self.intern_parameters(&parts.parameters);
            let result_dispatch = self.intern_result_dispatch(parts.result_dispatch);
            let slot = (path, parameters, result_dispatch);
            if let Some(interned) = self.record_slots.get(&slot) {
                return *interned;
            }
            let text: Box<str> = compound(
                CoordinateTag::NamedCallable,
                [
                    atom(CoordinateTag::Path, &parts.path),
                    atom(CoordinateTag::Parameters, &parts.parameters),
                    atom(CoordinateTag::ResultDispatch, &dispatch_text),
                ],
            )
            .into();
            let interned = CallableIdentityId::from_slot(self.records.len());
            self.records.push(CallableIdentityRecord {
                path,
                parameters,
                result_dispatch,
                text,
            });
            self.record_slots.insert(slot, interned);
            interned
        }

        fn intern_boundary_operator(
            &mut self,
            parts: BoundaryOperatorIdentityParts,
        ) -> CallableIdentityId {
            let path = self.intern_path(&parts.path);
            let key = (path, parts.parameter_types, parts.result_type);
            if let Some(identity) = self.boundary_operator_slots.get(&key) {
                return *identity;
            }
            let parameters = self.intern_parameters(&parts.parameters);
            let result_dispatch = self.intern_result_dispatch(parts.result_dispatch);
            let identity = CallableIdentityId::from_slot(self.records.len());
            let text: Box<str> = parts.text.into();
            self.records.push(CallableIdentityRecord {
                path,
                parameters,
                result_dispatch,
                text,
            });
            self.boundary_operator_slots.insert(key, identity);
            identity
        }

        fn record(&self, identity: CallableIdentityId) -> Option<&CallableIdentityRecord> {
            self.records.get(identity.slot()?)
        }
    }

    impl TypedTrees {
        pub fn machine_callable_identity(&self, machine: &Machine) -> CallableIdentityId {
            self.interned_callable_identity(
                DeclarationKey::Machine(machine as *const Machine as usize),
                || self.machine_callable_identity_parts(machine),
            )
        }

        pub fn trait_requirement_callable_identity(
            &self,
            trait_definition: &TraitDefinition,
            requirement: &StateSignature,
        ) -> CallableIdentityId {
            self.interned_callable_identity(
                DeclarationKey::TraitRequirement(
                    trait_definition as *const TraitDefinition as usize,
                    requirement as *const StateSignature as usize,
                ),
                || {
                    Some(
                        self.trait_requirement_callable_identity_parts(
                            trait_definition,
                            requirement,
                        ),
                    )
                },
            )
        }

        pub fn operator_callable_identity(
            &self,
            operator: &OperatorDefinition,
        ) -> CallableIdentityId {
            self.interned_callable_identity(
                DeclarationKey::Operator(operator as *const OperatorDefinition as usize),
                || Some(self.operator_callable_identity_parts(operator)),
            )
        }

        pub fn boundary_operator_requirement_callable_identity(
            &self,
            operator: &OperatorDefinition,
        ) -> CallableIdentityId {
            if !operator.is_boundary {
                return Default::default();
            }
            if let Some(requirement) = self.machine(operator.symbol) {
                return self.machine_callable_identity(requirement);
            }
            let declaration = DeclarationKey::BoundaryOperatorRequirement(
                operator as *const OperatorDefinition as usize,
            );
            let revision = self.structural_fingerprint();
            if let Some(identity) =
                self.read_callable_memo(|memo| memo.by_declaration.get(&declaration).copied())
            {
                return identity;
            }
            let parts = self.boundary_operator_identity_parts(operator);
            let identity = self.callable_identities.with_mut(|table| {
                parts
                    .map(|parts| table.intern_boundary_operator(parts))
                    .unwrap_or_default()
            });
            self.memoize_callable_identity(revision, declaration, identity);
            identity
        }

        fn boundary_operator_identity_parts(
            &self,
            operator: &OperatorDefinition,
        ) -> Option<BoundaryOperatorIdentityParts> {
            let parameter_types = self
                .operator_parameters(operator)
                .iter()
                .map(|parameter| self.normalized_type_identity(parameter.type_reference))
                .collect::<Vec<_>>();
            let parameters = parameter_types
                .iter()
                .map(|identity| self.type_identity_text(*identity))
                .collect::<Option<Vec<_>>>()?
                .join(",");
            let result_type = if operator.return_type.is_valid() {
                let identity = self.normalized_type_identity(operator.return_type);
                identity.is_valid().then_some(identity)?
            } else {
                Default::default()
            };
            Some(BoundaryOperatorIdentityParts {
                path: self.operator_path_string(operator.name),
                parameter_types,
                result_type,
                parameters,
                result_dispatch: self.normalized_result_dispatch_set(operator.return_type),
                text: crate::operator::boundary_operator_requirement_identity(self, operator),
            })
        }

        fn interned_callable_identity(
            &self,
            declaration: DeclarationKey,
            derive: impl FnOnce() -> Option<CallableIdentityParts>,
        ) -> CallableIdentityId {
            let revision = self.structural_fingerprint();
            if let Some(interned) =
                self.read_callable_memo(|memo| memo.by_declaration.get(&declaration).copied())
            {
                return interned;
            }
            let parts = derive();
            let interned = self
                .callable_identities
                .with_mut(|table| parts.map(|parts| table.intern(parts)).unwrap_or_default());
            self.memoize_callable_identity(revision, declaration, interned);
            interned
        }

        fn memoize_callable_identity(
            &self,
            revision: u64,
            declaration: DeclarationKey,
            identity: CallableIdentityId,
        ) {
            let text = self.read_callable_identities(|table| {
                table.record(identity).map(|record| record.text.clone())
            });
            self.callable_identity_memo
                .with_mut(revision, CallableIdentityMemo::default, |memo| {
                    memo.by_declaration.insert(declaration, identity);
                    if let Some(text) = text {
                        memo.by_text.insert(text, identity);
                    }
                });
        }

        pub fn callable_identity_path(&self, identity: CallableIdentityId) -> CallablePathId {
            self.read_callable_identities(|table| {
                table
                    .record(identity)
                    .map(|record| record.path)
                    .unwrap_or_default()
            })
        }

        pub fn callable_identity_parameters(
            &self,
            identity: CallableIdentityId,
        ) -> CallableParametersId {
            self.read_callable_identities(|table| {
                table
                    .record(identity)
                    .map(|record| record.parameters)
                    .unwrap_or_default()
            })
        }

        pub fn callable_identities_share_family(
            &self,
            left: CallableIdentityId,
            right: CallableIdentityId,
        ) -> bool {
            self.read_callable_identities(|table| match (table.record(left), table.record(right)) {
                (Some(left), Some(right)) => {
                    left.path == right.path && left.parameters == right.parameters
                }
                _ => false,
            })
        }

        pub fn callable_identities_share_path(
            &self,
            left: CallableIdentityId,
            right: CallableIdentityId,
        ) -> bool {
            self.read_callable_identities(|table| match (table.record(left), table.record(right)) {
                (Some(left), Some(right)) => left.path == right.path,
                _ => false,
            })
        }

        pub fn callable_identity_result_dispatch(
            &self,
            identity: CallableIdentityId,
        ) -> CallableResultDispatchId {
            self.read_callable_identities(|table| {
                table
                    .record(identity)
                    .map(|record| record.result_dispatch)
                    .unwrap_or_default()
            })
        }

        pub fn callable_identity_text(&self, identity: CallableIdentityId) -> String {
            self.read_callable_identities(|table| {
                table
                    .record(identity)
                    .map(|record| record.text.as_ref().to_owned())
                    .unwrap_or_default()
            })
        }

        pub fn machine_callable_identity_text(&self, machine: &Machine) -> Option<String> {
            let identity = self.machine_callable_identity(machine);
            identity
                .is_valid()
                .then(|| self.callable_identity_text(identity))
        }

        pub fn trait_requirement_callable_identity_text(
            &self,
            trait_definition: &TraitDefinition,
            requirement: &StateSignature,
        ) -> String {
            self.callable_identity_text(
                self.trait_requirement_callable_identity(trait_definition, requirement),
            )
        }

        pub fn callable_path_text(&self, path: CallablePathId) -> String {
            self.read_callable_identities(|table| {
                path.slot()
                    .and_then(|slot| table.paths.get(slot))
                    .map(|path| path.as_ref().to_owned())
                    .unwrap_or_default()
            })
        }

        pub fn callable_parameters_text(&self, parameters: CallableParametersId) -> String {
            self.read_callable_identities(|table| {
                parameters
                    .slot()
                    .and_then(|slot| table.parameters.get(slot))
                    .map(|parameters| parameters.as_ref().to_owned())
                    .unwrap_or_default()
            })
        }

        pub fn callable_result_dispatch_text(&self, dispatch: CallableResultDispatchId) -> String {
            self.read_callable_identities(|table| {
                dispatch
                    .slot()
                    .and_then(|slot| table.result_dispatches.get(slot))
                    .map(NormalizedResultDispatchSet::identity)
                    .unwrap_or_default()
            })
        }

        pub fn callable_result_dispatch_is_empty(
            &self,
            dispatch: CallableResultDispatchId,
        ) -> bool {
            self.read_callable_identities(|table| {
                dispatch
                    .slot()
                    .and_then(|slot| table.result_dispatches.get(slot))
                    .is_none_or(NormalizedResultDispatchSet::is_empty)
            })
        }

        pub fn callable_path_by_text(&self, path: &str) -> CallablePathId {
            self.callable_identities
                .with_mut(|table| table.intern_path(path))
        }

        pub fn callable_result_dispatch_identity(
            &self,
            result_dispatch: &NormalizedResultDispatchSet,
        ) -> CallableResultDispatchId {
            self.callable_identities
                .with_mut(|table| table.intern_result_dispatch(result_dispatch.clone()))
        }

        pub fn callable_identity_by_text(&self, text: &str) -> CallableIdentityId {
            self.sweep_callable_identities();
            self.read_callable_memo(|memo| memo.by_text.get(text).copied().unwrap_or_default())
        }

        pub fn machine_by_callable_identity(
            &self,
            identity: CallableIdentityId,
            declarations: &crate::references::DeclarationSelection,
        ) -> Option<&Machine> {
            if !identity.is_valid() {
                return None;
            }
            self.sweep_callable_identities();
            let position = self.read_callable_memo(|memo| {
                let mut positions =
                    memo.machines
                        .get(identity.slot()?)?
                        .iter()
                        .copied()
                        .filter(|position| {
                            self.machines()
                                .get(*position as usize)
                                .is_some_and(|machine| declarations.contains(machine.symbol))
                        });
                let position = positions.next()?;
                positions.next().is_none().then_some(position)
            })?;
            self.machines().get(position as usize)
        }

        pub fn machines_by_callable_identity(&self, identity: CallableIdentityId) -> Vec<&Machine> {
            let Some(slot) = identity.slot() else {
                return Vec::new();
            };
            self.sweep_callable_identities();
            let positions = self
                .read_callable_memo(|memo| memo.machines.get(slot).cloned().unwrap_or_default());
            let machines = self.machines();
            positions
                .into_iter()
                .filter_map(|position| machines.get(position as usize))
                .collect()
        }

        fn read_callable_identities<R>(&self, read: impl FnOnce(&CallableIdentityTable) -> R) -> R {
            self.callable_identities.with(read)
        }

        fn read_callable_memo<R>(&self, read: impl FnOnce(&CallableIdentityMemo) -> R) -> R {
            self.callable_identity_memo.with(
                self.structural_fingerprint(),
                CallableIdentityMemo::default,
                read,
            )
        }

        fn sweep_callable_identities(&self) {
            let revision = self.structural_fingerprint();
            if self.read_callable_memo(|memo| memo.swept) {
                return;
            }
            let mut derived = Vec::with_capacity(self.machines().len());
            for (position, machine) in self.machines().iter().enumerate() {
                if machine.target.is_some() {
                    continue;
                }
                let Some(parts) = self.machine_callable_identity_parts(machine) else {
                    continue;
                };
                derived.push((
                    position as u32,
                    DeclarationKey::Machine(machine as *const Machine as usize),
                    parts,
                ));
            }
            let boundary_operators = self
                .all_operators()
                .filter(|operator| operator.is_boundary && self.machine(operator.symbol).is_none())
                .filter_map(|operator| {
                    Some((
                        DeclarationKey::BoundaryOperatorRequirement(
                            operator as *const OperatorDefinition as usize,
                        ),
                        self.boundary_operator_identity_parts(operator)?,
                    ))
                })
                .collect::<Vec<_>>();
            let (machines, boundary_operators) = self.callable_identities.with_mut(|table| {
                let machines = derived
                    .into_iter()
                    .map(|(position, declaration, parts)| {
                        let identity = table.intern(parts);
                        let text = table
                            .record(identity)
                            .expect("interned callable record")
                            .text
                            .clone();
                        (position, declaration, identity, text)
                    })
                    .collect::<Vec<_>>();
                let boundary_operators = boundary_operators
                    .into_iter()
                    .map(|(declaration, parts)| {
                        let identity = table.intern_boundary_operator(parts);
                        let text = table
                            .record(identity)
                            .expect("interned callable record")
                            .text
                            .clone();
                        (declaration, identity, text)
                    })
                    .collect::<Vec<_>>();
                (machines, boundary_operators)
            });
            self.callable_identity_memo
                .with_mut(revision, CallableIdentityMemo::default, |memo| {
                    if memo.swept {
                        return;
                    }
                    for (position, declaration, identity, text) in machines {
                        memo.by_declaration.insert(declaration, identity);
                        memo.by_text.insert(text, identity);
                        let Some(slot) = identity.slot() else {
                            continue;
                        };
                        memo.machines
                            .resize_with(memo.machines.len().max(slot + 1), Vec::new);
                        let positions = &mut memo.machines[slot];
                        if !positions.contains(&position) {
                            positions.push(position);
                        }
                    }
                    for (declaration, identity, text) in boundary_operators {
                        memo.by_declaration.insert(declaration, identity);
                        memo.by_text.insert(text, identity);
                    }
                    memo.swept = true;
                });
        }
    }
}
pub(crate) mod constraint_identity {
    use crate::TypedTrees;
    use crate::type_identity::substitution;
    use crate::type_system::type_identity::identity_context::{
        TypeIdentityContext, TypeIdentityQualification, normalize_type_reference,
    };
    use crate::types::{
        DomainConstraint, DomainConstraintSubject, FixedArrayLength, TypeConstraintNode,
        TypeReferenceHandle, TypeReferenceNode,
    };
    use arena::HandleSpan;
    use language_semantics::type_identity::vocabulary::{
        ArrayLengthForm, ConstraintForm, DomainFamily, IndexForm, Label, Spelled,
    };

    pub(crate) fn normalize_constrained_base(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        context: &TypeIdentityContext<'_>,
    ) -> (String, Vec<NormalizedConstraint>) {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let (base, mut all_constraints) =
                    normalize_constrained_base(program, *base_type, context);
                all_constraints.extend(normalized_constraints(program, *constraints, context));
                (base, all_constraints)
            }
            _ => (
                normalize_type_reference(program, type_reference, context),
                Vec::new(),
            ),
        }
    }

    pub(crate) fn normalized_constraints(
        program: &TypedTrees,
        constraints: HandleSpan<TypeConstraintNode>,
        context: &TypeIdentityContext<'_>,
    ) -> Vec<NormalizedConstraint> {
        program
            .type_reference_table
            .constraints(constraints)
            .iter()
            .map(|constraint| match constraint {
                TypeConstraintNode::Named(name) => {
                    NormalizedConstraint::Named(name.as_str().to_owned())
                }

                TypeConstraintNode::ArithmeticDomain(domain) => {
                    NormalizedConstraint::Arithmetic(domain.name().to_owned())
                }
                TypeConstraintNode::Domain(domain) => match domain.subject {
                    DomainConstraintSubject::Declared => NormalizedConstraint::DeclaredDomain(
                        normalized_declared_domain_identity(program, domain, context),
                    ),
                    DomainConstraintSubject::Carry(_)
                    | DomainConstraintSubject::Value(_)
                    | DomainConstraintSubject::OmegaLayout { .. } => {
                        NormalizedConstraint::CompilerDomain(normalized_compiler_domain_identity(
                            program, domain, context,
                        ))
                    }
                },
            })
            .collect()
    }

    fn normalized_declared_domain_identity(
        program: &TypedTrees,
        domain: &DomainConstraint,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        let ordinary_name = declared_domain_identity(program, domain);
        if context.qualification == TypeIdentityQualification::Ordinary
            && domain.arguments.is_empty()
        {
            return ordinary_name;
        }
        compound(
            ConstraintForm::DeclaredDomain,
            std::iter::once(atom(
                Label::Name,
                &context.name(program, domain.symbol, &ordinary_name),
            ))
            .chain(
                domain
                    .arguments
                    .iter()
                    .map(|argument| normalize_type_reference(program, *argument, context)),
            ),
        )
    }

    pub(crate) fn normalized_compiler_domain_identity(
        program: &TypedTrees,
        domain: &DomainConstraint,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        match domain.subject {
            DomainConstraintSubject::Declared => {
                normalized_declared_domain_identity(program, domain, context)
            }
            DomainConstraintSubject::Carry(permission) => compound(
                ConstraintForm::CompilerDomain,
                [
                    atom(Label::Family, DomainFamily::Carry.spelling()),
                    atom(
                        Label::Permission,
                        match permission {
                            language_semantics::CarryPermission::AcrossSuspend => "across-suspend",
                            language_semantics::CarryPermission::AnyCpu => "any-cpu",
                            language_semantics::CarryPermission::AnyThread => "any-thread",
                            language_semantics::CarryPermission::MovableAddress => {
                                "movable-address"
                            }
                        },
                    ),
                ],
            ),
            DomainConstraintSubject::Value(value_domain) => compound(
                ConstraintForm::CompilerDomain,
                [
                    atom(Label::Family, DomainFamily::Value.spelling()),
                    atom(
                        Label::Domain,
                        match value_domain {
                            language_semantics::value_domain::ValueDomain::Finite => "finite",
                        },
                    ),
                ],
            ),
            DomainConstraintSubject::OmegaLayout { grammar } => compound(
                ConstraintForm::CompilerDomain,
                [
                    atom(Label::Family, DomainFamily::OmegaLayout.spelling()),
                    atom(
                        Label::Grammar,
                        match grammar {
                            crate::types::OmegaLayoutGrammar::Derived => "derived",
                        },
                    ),
                ]
                .into_iter()
                .chain(domain.arguments.iter().map(|argument| {
                    compound(
                        Label::Schema,
                        [normalize_type_reference(program, *argument, context)],
                    )
                })),
            ),
        }
    }

    fn declared_domain_identity(program: &TypedTrees, domain: &DomainConstraint) -> String {
        if domain.semantic_id.is_valid()
            && let Some(name) = program.semantic_domains.name(domain.semantic_id)
        {
            return name.to_owned();
        }
        domain.name.as_str().to_owned()
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum NormalizedConstraint {
        Arithmetic(String),
        CompilerDomain(String),
        DeclaredDomain(String),
        Named(String),
    }

    impl NormalizedConstraint {
        pub(crate) fn encode(self) -> String {
            match self {
                Self::Arithmetic(name) => {
                    compound(ConstraintForm::ArithmeticDomain, [atom(Label::Name, &name)])
                }
                Self::CompilerDomain(identity) => identity,
                Self::DeclaredDomain(name) => {
                    compound(ConstraintForm::DeclaredDomain, [atom(Label::Name, &name)])
                }
                Self::Named(name) => {
                    compound(ConstraintForm::NamedConstraint, [atom(Label::Name, &name)])
                }
            }
        }
    }

    pub(crate) fn normalize_array_length(
        program: &TypedTrees,
        length: &FixedArrayLength,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        match length {
            FixedArrayLength::Literal(value) if context.invalid_application.is_some() => {
                atom(IndexForm::Integer, &value.to_string())
            }
            FixedArrayLength::ConstParameter { symbol, name }
                if context.invalid_application.is_some() =>
            {
                substitution::index_name(program, *symbol, name.as_str(), context)
            }
            FixedArrayLength::Expression(value) => {
                let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(*value)
                else {
                    return atom(
                        ArrayLengthForm::InvalidExpression,
                        "missing constant recipe",
                    );
                };
                compound(
                    ArrayLengthForm::ConstExpression,
                    [super::identity_context::normalize_index_expression(
                        program,
                        *expression,
                        context,
                    )],
                )
            }
            FixedArrayLength::Literal(value) => atom(ArrayLengthForm::Literal, &value.to_string()),
            FixedArrayLength::ConstParameter { symbol, name } => {
                substitution::array_length(program, *symbol, name.as_str(), context).unwrap_or_else(
                    || {
                        atom(
                            ArrayLengthForm::ConstParameter,
                            &context.name(program, *symbol, name.as_str()),
                        )
                    },
                )
            }
            FixedArrayLength::ConstCall { machine, name, .. } => atom(
                ArrayLengthForm::ConstCall,
                &context.name(program, *machine, name.as_str()),
            ),
        }
    }

    pub(crate) fn bare(tag: impl Spelled) -> String {
        tag.spelling().to_owned()
    }

    pub(crate) fn atom(tag: impl Spelled, value: &str) -> String {
        let tag = tag.spelling();
        let mut output = String::with_capacity(tag.len() + value.len() + 2);
        output.push_str(tag);
        output.push('(');
        if value
            .bytes()
            .any(|byte| matches!(byte, b'\\' | b'(' | b')' | b','))
        {
            for character in value.chars() {
                if matches!(character, '\\' | '(' | ')' | ',') {
                    output.push('\\');
                }
                output.push(character);
            }
        } else {
            output.push_str(value);
        }
        output.push(')');
        output
    }

    pub(crate) fn byte_atom(tag: impl Spelled, value: &[u8]) -> String {
        let tag = tag.spelling();
        let mut output = String::with_capacity(tag.len() + value.len().saturating_mul(2) + 24);
        output.push_str(tag);
        output.push('(');
        output.push_str(&value.len().to_string());
        output.push(':');
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for &byte in value {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output.push(')');
        output
    }

    pub(crate) fn compound(tag: impl Spelled, parts: impl IntoIterator<Item = String>) -> String {
        let tag = tag.spelling();
        let parts = parts.into_iter().collect::<Vec<_>>();
        let mut output =
            String::with_capacity(tag.len() + parts.iter().map(String::len).sum::<usize>() + 2);
        output.push_str(tag);
        output.push('(');
        for (index, part) in parts.into_iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str(&part);
        }
        output.push(')');
        output
    }

    #[cfg(test)]
    pub(crate) mod byte_atom_tests {
        use super::byte_atom;
        use language_semantics::type_identity::vocabulary::{OwnerForm, Spelled};

        fn original_byte_atom(tag: &str, value: &[u8]) -> String {
            let mut output = String::with_capacity(tag.len() + value.len().saturating_mul(2) + 24);
            output.push_str(tag);
            output.push('(');
            output.push_str(&value.len().to_string());
            output.push(':');
            for byte in value {
                output.push_str(&format!("{byte:02x}"));
            }
            output.push(')');
            output
        }

        #[test]
        fn byte_atoms_match_original_identity_encoding() {
            for tag in [OwnerForm::Package, OwnerForm::ToolchainSource] {
                for length in [0, 1, 2, 15, 16, 31, 32, 33, 63, 64, 255, 256, 4097] {
                    let bytes = (0..length)
                        .map(|position| (position as u8).wrapping_mul(73))
                        .collect::<Vec<_>>();
                    assert_eq!(
                        byte_atom(tag, &bytes),
                        original_byte_atom(tag.spelling(), &bytes)
                    );
                }
                for byte in 0..=u8::MAX {
                    assert_eq!(
                        byte_atom(tag, &[byte]),
                        original_byte_atom(tag.spelling(), &[byte])
                    );
                }
            }
            assert_eq!(
                byte_atom(OwnerForm::Package, &[0, 15, 16, 255]),
                "package-owner(4:000f10ff)"
            );
        }
    }
}
pub(crate) mod coordinates {
    use language_semantics::type_identity::vocabulary::Spelled;

    #[derive(Clone, Copy)]
    pub(crate) enum CoordinateTag {
        NamedCallable,
        SpecializedCallable,
        CapturedCallable,
        NestedMachine,
        ClosedRequirement,
        LocalTraitApplication,
        LocalTraitContract,
        Refinements,
        TraitApplication,
        Parameter,
        Path,
        Parameters,
        ResultDispatch,
        Owner,
        Name,
        Template,
        Declaration,
        Arguments,
        LocalArguments,
        RuntimeValueCarrier,
        Lifetimes,
        Mutable,
        Const,
    }

    impl Spelled for CoordinateTag {
        fn spelling(self) -> &'static str {
            match self {
                Self::NamedCallable => "named-callable",
                Self::SpecializedCallable => "specialized-callable",
                Self::CapturedCallable => "captured-callable",
                Self::NestedMachine => "nested-machine",
                Self::ClosedRequirement => "closed-requirement",
                Self::LocalTraitApplication => "local-trait-application",
                Self::LocalTraitContract => "local-trait-contract",
                Self::Refinements => "refinements",
                Self::TraitApplication => "trait-application",
                Self::Parameter => "parameter",
                Self::Path => "path",
                Self::Parameters => "parameters",
                Self::ResultDispatch => "result-dispatch",
                Self::Owner => "owner",
                Self::Name => "name",
                Self::Template => "template",
                Self::Declaration => "declaration",
                Self::Arguments => "arguments",
                Self::LocalArguments => "local-arguments",
                Self::RuntimeValueCarrier => "runtime-value-carrier",
                Self::Lifetimes => "lifetimes",
                Self::Mutable => "mutable",
                Self::Const => "const",
            }
        }
    }
}
pub(crate) mod derived_declarations {
    use super::constraint_identity::{atom, compound};
    use super::identity_context::{
        TypeIdentityContext, TypeIdentityQualification, normalize_type_reference,
    };
    use crate::TypedTrees;
    use crate::types::TypeReferenceNode;
    use language_semantics::type_identity::vocabulary::{Label, NameForm};
    use symbols::SymbolHandle;

    pub(crate) fn nominal_name(
        program: &TypedTrees,
        symbol: SymbolHandle,
        context: &TypeIdentityContext<'_>,
        hermetic: bool,
    ) -> Result<Option<String>, &'static str> {
        let qualified = TypeIdentityContext {
            storage_value_indices: false,
            qualification: TypeIdentityQualification::PackageQualified,
            ..*context
        };
        for view in &program.access_view_plans {
            let field = view.fields.iter().find(|field| {
                field.accessor_data_symbol == symbol
                    || field.accessor_targets.iter().any(|target| {
                        target.machine_symbol == symbol || target.state_symbol == symbol
                    })
            });
            if view.data_symbol != symbol && field.is_none() {
                continue;
            }
            let origin = program
                .data_definition_by_symbol(view.data_symbol)
                .and_then(|definition| definition.generic_instance)
                .ok_or("derived placed declaration has no retained applied origin")?;
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } = program.type_reference_table.type_reference(origin)
            else {
                return Err("derived placed origin is not an exact generic application");
            };
            let [policy, schema] = program.type_reference_handles(*arguments) else {
                return Err("derived placed origin lost its policy/schema operands");
            };
            if !view.template_symbol.is_valid()
                || *base_symbol != view.template_symbol
                || *base_symbol == view.data_symbol
                || !lifetime_arguments.is_empty()
                || !matches!(program.type_reference_table.type_reference(*policy), TypeReferenceNode::Named { symbol, .. } if *symbol == view.policy_symbol)
                || !matches!(program.type_reference_table.type_reference(*schema), TypeReferenceNode::Named { symbol, .. } if *symbol == view.schema_symbol)
                || [view.policy_symbol, view.schema_symbol].contains(&view.data_symbol)
            {
                return Err("derived placed origin changed its exact template/policy/schema join");
            }
            if hermetic
                && [view.template_symbol, view.policy_symbol, view.schema_symbol]
                    .iter()
                    .any(|symbol| {
                        program.symbols.symbol_package_identity(*symbol).is_none()
                            && !program
                                .symbols
                                .symbol_source_origin(*symbol)
                                .is_some_and(source::SourceOrigin::is_toolchain)
                    })
            {
                return Err("derived placed declaration contains an unowned nominal operand");
            }
            let owner = normalize_type_reference(program, origin, &qualified);
            if view.data_symbol == symbol {
                return Ok(Some(compound(NameForm::DerivedApplication, [owner])));
            }
            let field = field.ok_or("derived placed declaration lost its field producer")?;
            let selector = match field.member_identity {
                Some(identity) => atom(Label::Member, &identity.to_string()),
                None => atom(Label::Field, &field.field_name),
            };
            let accessor = compound(NameForm::PlacedField, [owner, selector]);
            if field.accessor_data_symbol == symbol {
                return Ok(Some(accessor));
            }
            for target in &field.accessor_targets {
                let role = if target.machine_symbol == symbol {
                    "machine"
                } else if target.state_symbol == symbol {
                    "state"
                } else {
                    continue;
                };
                return Ok(Some(compound(
                    NameForm::PlacedOperation,
                    [
                        accessor,
                        atom(Label::Operation, &target.operation),
                        atom(Label::Role, role),
                    ],
                )));
            }
            return Err("derived placed callable lost its exact operation producer");
        }
        Ok(None)
    }
}
pub(crate) mod identity_context {

    use super::LifetimeParameterIdentity;
    use crate::TypedTrees;
    use crate::expression::{ExpressionHandle, ExpressionNode};
    use crate::type_identity::substitution;
    use crate::type_system::type_identity::constraint_identity::{
        NormalizedConstraint, atom, bare, byte_atom, compound, normalize_array_length,
        normalize_constrained_base, normalized_constraints,
    };
    use crate::type_system::type_identity::open_index_identity::{
        index_binary_operator_name, normalize_selected_index_operation,
        open_index_operation_selection,
    };
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};
    use language_semantics::type_identity::vocabulary::{
        IndexForm, Label, NameForm, OwnerForm, RegionForm, Spelled, TypeForm,
    };
    use std::cell::Cell;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;

    pub(crate) mod match_dispatch {

        use super::{TypeIdentityContext, atom, bare, compound, normalize_index_expression};
        use crate::TypedTrees;
        use crate::expression::{
            ExpressionHandle, MatchFieldAction, MatchPattern, TableMatchExpression,
            TableSavedSubject,
        };
        use language_semantics::type_identity::vocabulary::{
            FieldActionForm, IndexForm, Label, PatternForm,
        };

        fn unavailable(context: &TypeIdentityContext<'_>) -> String {
            if let Some(invalid) = context.invalid_index_argument {
                invalid.set(true);
            }
            if let Some(invalid) = context.invalid_application {
                invalid.set(true);
            }
            if let Some(invalid) = context.missing_exact_nominal_owner {
                invalid.set(true);
            }
            bare(IndexForm::UnboundMatchSubject)
        }

        pub(crate) fn normalize_match(
            program: &TypedTrees,
            expression: ExpressionHandle,
            dispatch: &TableMatchExpression,
            context: &TypeIdentityContext<'_>,
        ) -> String {
            let table = &program.expression_table;
            if !table.expression_is_valid(dispatch.subject) {
                return unavailable(context);
            }
            let subject = normalize_index_expression(program, dispatch.subject, context);
            let mut subjects = context.match_subjects.to_vec();
            if dispatch.capture.is_valid() {
                if !table.contains_value_capture(dispatch.capture)
                    || table.value_capture(dispatch.capture).subject != dispatch.subject
                    || table.value_capture(dispatch.capture).definition != expression
                    || subjects.contains(&dispatch.capture)
                {
                    return unavailable(context);
                }
                subjects.push(dispatch.capture);
            } else if table
                .match_arms(dispatch.arms)
                .iter()
                .any(|arm| matches!(arm.pattern, MatchPattern::Destructure(_)))
            {
                return unavailable(context);
            }
            let nested = TypeIdentityContext {
                match_subjects: &subjects,
                ..*context
            };
            compound(
                IndexForm::Match,
                std::iter::once(subject).chain(table.match_arms(dispatch.arms).iter().map(|arm| {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => {
                            normalize_index_expression(program, value, &nested)
                        }
                        MatchPattern::Wildcard => bare(PatternForm::Wildcard),
                        MatchPattern::Destructure(pattern) => {
                            let fields = table.match_fields(pattern.fields).iter().map(|field| {
                                let action = match &field.action {
                                    MatchFieldAction::Bind(_) => bare(
                                        if field.component == language_core::PatternComponent::Case
                                        {
                                            FieldActionForm::BindCase
                                        } else {
                                            FieldActionForm::Bind
                                        },
                                    ),
                                    MatchFieldAction::Equal(value) => compound(
                                        FieldActionForm::Equal,
                                        [normalize_index_expression(program, *value, &nested)],
                                    ),
                                    MatchFieldAction::Waive => bare(
                                        if field.component == language_core::PatternComponent::Case
                                        {
                                            FieldActionForm::WaiveCase
                                        } else {
                                            FieldActionForm::Waive
                                        },
                                    ),
                                };
                                compound(
                                    Label::Field,
                                    [
                                        atom(
                                            Label::Name,
                                            &context.name(
                                                program,
                                                field.field_symbol,
                                                field.member.as_str(),
                                            ),
                                        ),
                                        action,
                                    ],
                                )
                            });
                            let selectors = table
                                .match_proof_selectors(pattern.proof_selectors)
                                .iter()
                                .enumerate()
                                .map(|(ordinal, selector)| {
                                    compound(
                                        Label::ProofSelector,
                                        [
                                            atom(Label::Field, selector.output_field.as_str()),
                                            atom(Label::Binding, &ordinal.to_string()),
                                        ],
                                    )
                                });
                            compound(
                                PatternForm::Destructure,
                                [
                                    normalize_index_expression(program, pattern.head, &nested),
                                    compound(Label::Fields, fields),
                                    atom(Label::Rest, if pattern.has_rest { "yes" } else { "no" }),
                                    if pattern.guard.is_valid() {
                                        normalize_index_expression(program, pattern.guard, &nested)
                                    } else {
                                        bare(Label::NoGuard)
                                    },
                                    compound(Label::ProofSelectors, selectors),
                                ],
                            )
                        }
                    };
                    compound(
                        Label::Arm,
                        [
                            pattern,
                            normalize_index_expression(program, arm.value, &nested),
                        ],
                    )
                })),
            )
        }

        pub(crate) fn normalize_saved_subject(
            program: &TypedTrees,
            saved: &TableSavedSubject,
            context: &TypeIdentityContext<'_>,
        ) -> String {
            if !program
                .expression_table
                .contains_value_capture(saved.capture)
                || !program
                    .expression_table
                    .expression_is_valid(saved.pattern_head)
            {
                return unavailable(context);
            }
            let Some(depth) = context
                .match_subjects
                .iter()
                .rev()
                .position(|capture| *capture == saved.capture)
            else {
                return unavailable(context);
            };
            let identity = compound(
                IndexForm::SavedSubject,
                [
                    atom(Label::Depth, &depth.to_string()),
                    normalize_index_expression(program, saved.pattern_head, context),
                ],
            );
            if saved.component == language_core::PatternComponent::Case {
                compound(IndexForm::CaseComponent, [identity])
            } else {
                identity
            }
        }
    }

    #[cfg(test)]
    pub(crate) mod argument_roles_tests {
        use crate::TypedTrees;
        use crate::data::{DataDefinition, TypeParameter, TypeParameterKind};
        use crate::expression::ExpressionNode;
        use crate::name::Identifier;
        use crate::trait_definition::Conformance;
        use crate::type_identity::TypeIdentityRequest;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};
        use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

        fn named(
            program: &mut TypedTrees,
            symbol: SymbolHandle,
            name: &str,
        ) -> TypeReferenceHandle {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
        }

        fn application(
            program: &mut TypedTrees,
            symbol: SymbolHandle,
            name: &str,
            arguments: Vec<TypeReferenceHandle>,
        ) -> TypeReferenceHandle {
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles(arguments);
            program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    base_name: Identifier::generated(name),
                    lifetime_arguments: Vec::new(),
                    arguments,
                })
        }

        #[test]
        fn resolved_region_substitution_is_scoped_and_simultaneous() {
            use crate::machine::Machine;
            use crate::type_identity::{
                LifetimeParameterIdentity, applied_type_identity_in_region_scope,
            };
            let mut builder = SymbolTableBuilder::default();
            let required =
                builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("required"));
            let actual = builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("actual"));
            let captured =
                builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("captured"));
            let scalar = builder.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u32"));
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            for (symbol, name) in [
                (required, "required"),
                (actual, "actual"),
                (captured, "captured"),
            ] {
                program.push_machine(Machine {
                    symbol,
                    name: Identifier::generated(name),
                    lifetime_parameters: vec![
                        Identifier::generated("slot"),
                        Identifier::generated("later"),
                    ],
                    ..Default::default()
                });
            }
            let scalar = named(&mut program, scalar, "u32");
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: scalar,
                    access: language_semantics::ReferenceAccess::Shared,
                    lifetime: Some(Identifier::generated("slot")),
                });
            let later = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: scalar,
                    access: language_semantics::ReferenceAccess::Shared,
                    lifetime: Some(Identifier::generated("later")),
                });
            let region = |declaration| LifetimeParameterIdentity {
                declaration,
                ordinal: 0,
            };
            let identity =
                |owner, rows: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)]| {
                    applied_type_identity_in_region_scope(
                        &program,
                        reference,
                        owner,
                        owner,
                        &[],
                        rows,
                    )
                    .unwrap()
                };
            assert_ne!(identity(actual, &[]), identity(required, &[]));

            use crate::type_identity::{
                DeclaredStaticOperandIdentityRequest, IndexIdentityArgument,
            };
            let declared = |owner, reference| {
                program
                    .declared_static_operand_identity(DeclaredStaticOperandIdentityRequest {
                        operand: IndexIdentityArgument::TypeArgument(reference),
                        lexical_owner: owner,
                        binders: &[],
                        lifetime_binders: &[
                            (region(owner), 0),
                            (
                                LifetimeParameterIdentity {
                                    declaration: owner,
                                    ordinal: 1,
                                },
                                1,
                            ),
                        ],
                    })
                    .unwrap()
            };
            assert_eq!(declared(required, reference), declared(actual, reference));
            assert_ne!(declared(required, reference), declared(required, later));
            assert_ne!(identity(actual, &[]), identity(captured, &[]));
            assert_eq!(
                identity(actual, &[(region(actual), region(required))]),
                identity(required, &[])
            );
            assert_eq!(
                identity(actual, &[(region(captured), region(required))]),
                identity(actual, &[])
            );
            assert_eq!(
                identity(
                    actual,
                    &[
                        (region(actual), region(required)),
                        (region(required), region(captured))
                    ]
                ),
                identity(required, &[])
            );
            assert_eq!(
                identity(actual, &[(region(actual), region(captured))]),
                identity(captured, &[])
            );
            assert_ne!(identity(actual, &[]), identity(required, &[]));
        }

        #[test]
        fn ordinary_nested_implementation_const_roles_preserve_values_and_policy_identity() {
            let mut symbols = SymbolTableBuilder::default();
            let u64_symbol =
                symbols.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u64"));
            let payload = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Payload"));
            let policy =
                symbols.insert_root(SymbolKind::Conformance, SymbolNameRef::Static("Policy"));
            let other_policy = symbols.insert_root(
                SymbolKind::Conformance,
                SymbolNameRef::Static("OtherPolicy"),
            );
            let storage = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Storage"));
            let indexed = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Indexed"));
            let policy_slot = symbols
                .insert_children(
                    policy,
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
                )
                .handles()
                .next()
                .unwrap();
            let other_slot = symbols
                .insert_children(
                    other_policy,
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
                )
                .handles()
                .next()
                .unwrap();
            let storage_slots = symbols
                .insert_children(
                    storage,
                    [
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Policy")),
                    ],
                )
                .handles()
                .collect::<Vec<_>>();
            let value_slot = symbols
                .insert_children(
                    indexed,
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("Subject"))],
                )
                .handles()
                .next()
                .unwrap();
            let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
            let subjects = symbols
                .insert_children(
                    caller,
                    [
                        (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                        (SymbolKind::Parameter, SymbolNameRef::Static("right")),
                    ],
                )
                .handles()
                .collect::<Vec<_>>();
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let u64_type = named(&mut program, u64_symbol, "u64");
            let payload_type = named(&mut program, payload, "Payload");
            program.push_data_definition(DataDefinition {
                symbol: payload,
                name: Identifier::generated("Payload"),
                ..DataDefinition::default()
            });
            for (symbol, slot, name) in [
                (policy, policy_slot, "Policy"),
                (other_policy, other_slot, "OtherPolicy"),
            ] {
                let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
                    symbol: slot,
                    name: Identifier::generated("N"),
                    kind: TypeParameterKind::Const {
                        type_reference: u64_type,
                    },
                    ..TypeParameter::default()
                }]);
                program.push_conformance(Conformance {
                    symbol,
                    alias: Some(Identifier::generated(name)),
                    type_parameters,
                    ..Conformance::default()
                });
                assert!(matches!(
                    program.declaration_type_parameters(symbol)[0].kind,
                    TypeParameterKind::Const { .. }
                ));
            }
            let type_parameters = program.data_type_parameters.insert_many([
                TypeParameter {
                    symbol: storage_slots[0],
                    name: Identifier::generated("T"),
                    kind: TypeParameterKind::Type,
                    ..TypeParameter::default()
                },
                TypeParameter {
                    symbol: storage_slots[1],
                    name: Identifier::generated("Policy"),
                    kind: TypeParameterKind::Implementation,
                    ..TypeParameter::default()
                },
            ]);
            program.push_data_definition(DataDefinition {
                symbol: storage,
                name: Identifier::generated("Storage"),
                type_parameters,
                ..DataDefinition::default()
            });

            let remembered_seven = named(&mut program, SymbolHandle::invalid(), "7");
            let integer = program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_value(7).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::U64,
                    domain: ArithmeticDomain::Exact,
                }),
            ));
            let authored_seven = program
                .type_reference_table
                .insert(TypeReferenceNode::ConstExpression(integer));
            let eight = named(&mut program, SymbolHandle::invalid(), "8");
            let remembered_policy =
                application(&mut program, policy, "Policy", vec![remembered_seven]);
            let authored_policy = application(&mut program, policy, "Policy", vec![authored_seven]);
            let wrong_value_policy = application(&mut program, policy, "Policy", vec![eight]);
            let wrong_declaration_policy = application(
                &mut program,
                other_policy,
                "OtherPolicy",
                vec![remembered_seven],
            );
            let remembered = application(
                &mut program,
                storage,
                "Storage",
                vec![payload_type, remembered_policy],
            );
            let authored = application(
                &mut program,
                storage,
                "Storage",
                vec![payload_type, authored_policy],
            );
            let wrong_value = application(
                &mut program,
                storage,
                "Storage",
                vec![payload_type, wrong_value_policy],
            );
            let wrong_declaration = application(
                &mut program,
                storage,
                "Storage",
                vec![payload_type, wrong_declaration_policy],
            );
            let ordinary = |program: &TypedTrees, reference| {
                program.type_identity(TypeIdentityRequest::ordinary(reference))
            };
            assert_ne!(
                ordinary(&program, remembered),
                ordinary(&program, wrong_value)
            );
            assert_ne!(
                ordinary(&program, remembered),
                ordinary(&program, wrong_declaration)
            );

            let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
                symbol: value_slot,
                name: Identifier::generated("Subject"),
                kind: TypeParameterKind::Value {
                    type_reference: u64_type,
                },
                ..TypeParameter::default()
            }]);
            program.push_data_definition(DataDefinition {
                symbol: indexed,
                name: Identifier::generated("Indexed"),
                type_parameters,
                ..DataDefinition::default()
            });
            let left = named(&mut program, subjects[0], "left");
            let right = named(&mut program, subjects[1], "right");
            let left = application(&mut program, indexed, "Indexed", vec![left]);
            let right = application(&mut program, indexed, "Indexed", vec![right]);
            assert_ne!(ordinary(&program, left), ordinary(&program, right));
            assert_eq!(
                program.storage_type_identity(TypeIdentityRequest::ordinary(left)),
                program.storage_type_identity(TypeIdentityRequest::ordinary(right))
            );

            let borrow = |program: &mut TypedTrees, referee| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Reference {
                        referee,
                        access: language_semantics::ReferenceAccess::Shared,
                        lifetime: None,
                    })
            };
            let remembered_borrow = borrow(&mut program, remembered);
            let authored_borrow = borrow(&mut program, authored);
            let actual =
                program.type_identity_request_text(TypeIdentityRequest::ordinary(remembered));
            let expected =
                program.type_identity_request_text(TypeIdentityRequest::ordinary(authored));
            eprintln!("specialized carrier: {actual}\nauthored carrier: {expected}");
            let same_carrier = ordinary(&program, remembered) == ordinary(&program, authored);
            let same_borrow =
                ordinary(&program, remembered_borrow) == ordinary(&program, authored_borrow);
            assert!(
                same_carrier && same_borrow,
                "same declaration-owned Const value must preserve ordinary and borrowed carrier identity\nspecialized: {actual}\nauthored: {expected}"
            );
        }
    }

    #[derive(Default)]
    pub(crate) struct TypeIdentityContext<'binders> {
        pub(crate) portable_machine_applications:
            Option<&'binders super::portable_machine_applications::PortableMachineApplications>,
        pub(crate) storage_value_indices: bool,
        pub(crate) declared_carrier: bool,
        pub(crate) application_scopes: &'binders [super::AppliedTypeIdentityScope<'binders>],
        pub(crate) application_owner: SymbolHandle,
        pub(crate) application_argument_owner: SymbolHandle,
        pub(crate) resolved_lifetimes:
            &'binders [(LifetimeParameterIdentity, LifetimeParameterIdentity)],
        pub(crate) lifetime_binders: &'binders [(LifetimeParameterIdentity, usize)],
        pub(crate) invalid_application: Option<&'binders Cell<bool>>,
        pub(crate) encountered_regions: Option<&'binders Cell<bool>>,
        // A description can retain elision without claiming a concrete region.
        // Existing checked identities point this observation at their error flag.
        pub(crate) encountered_elided_regions: Option<&'binders Cell<bool>>,
        pub(crate) binders: &'binders [(SymbolHandle, String)],
        pub(crate) substitutions: &'binders [(SymbolHandle, TypeReferenceHandle)],
        pub(crate) substitution_scope_starts: &'binders [usize],
        pub(crate) substitution_semantics: SubstitutionSemantics,
        pub(crate) active_const_substitutions: &'binders [SymbolHandle],
        pub(crate) index_substitutions:
            &'binders [(SymbolHandle, super::IndexIdentityArgument<'binders>)],
        pub(crate) invalid_index_argument: Option<&'binders Cell<bool>>,
        pub(crate) match_subjects:
            &'binders [arena::Handle<crate::expression::CompletedValueCapture>],
        pub(crate) exact_toolchain_sources: &'binders [(source::SourceId, [u8; 32])],
        pub(crate) missing_exact_nominal_owner: Option<&'binders Cell<bool>>,
        pub(crate) qualification: TypeIdentityQualification,
        pub(crate) static_context: crate::expression::ExpressionHandle,
    }

    #[derive(Clone, Copy, Default)]
    pub(crate) enum SubstitutionSemantics {
        #[default]
        Transitive,
        Simultaneous,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TypeIdentityQualification {
        #[default]
        Ordinary,
        PackageQualified,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum PackageQualifiedNominalOwner {
        Package([u8; 32]),
        ToolchainSource([u8; 32]),
        Toolchain,
        Unresolved,
    }

    impl PackageQualifiedNominalOwner {
        fn encode(self) -> String {
            match self {
                Self::Package(digest) => byte_atom(OwnerForm::Package, &digest),
                Self::ToolchainSource(digest) => byte_atom(OwnerForm::ToolchainSource, &digest),
                Self::Toolchain => bare(OwnerForm::Toolchain),
                Self::Unresolved => bare(OwnerForm::Unresolved),
            }
        }
    }

    impl TypeIdentityContext<'_> {
        pub(crate) fn enter_type_operand(
            &self,
            program: &TypedTrees,
            reference: TypeReferenceHandle,
        ) -> Self {
            let operand_owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            if self.invalid_application.is_some()
                && operand_owner.is_valid()
                && !program
                    .type_reference_table
                    .has_resolved_operand_lifetimes(reference)
                && !self.declared_carrier
            {
                Self {
                    application_owner: operand_owner,
                    application_scopes: self
                        .application_scopes
                        .iter()
                        .rposition(|scope| {
                            program.lifetime_declaration_owner(scope.declaration)
                                == program.lifetime_declaration_owner(operand_owner)
                        })
                        .map_or(&[][..], |ordinal| &self.application_scopes[..=ordinal]),
                    ..*self
                }
            } else {
                Self { ..*self }
            }
        }

        /*
            Type equality names the value an implementation constructs, so two
            constructors of one value compare equal. A portable or
            package-qualified identity keeps the written constructor: it names
            the declaration an artifact depends on, and equal values must not
            lose the dependencies of the one actually used.
        */
        pub(crate) fn identifies_constructed_values(&self) -> bool {
            self.portable_machine_applications.is_none()
                && self.qualification == TypeIdentityQualification::Ordinary
        }

        pub(crate) fn applied_argument(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(super::IndexIdentityArgument<'_>, Self)> {
            if !symbol.is_valid() {
                return None;
            }
            for (ordinal, scope) in self.application_scopes.iter().enumerate().rev() {
                if let Some((_, argument)) =
                    scope.arguments.iter().find(|(binder, _)| *binder == symbol)
                {
                    return Some((
                        *argument,
                        Self {
                            application_scopes: &self.application_scopes[..ordinal],
                            application_owner: if self.application_argument_owner.is_valid() {
                                self.application_argument_owner
                            } else {
                                self.application_owner
                            },
                            ..*self
                        },
                    ));
                }
            }
            None
        }

        pub(crate) fn lifetime(&self, program: &TypedTrees, name: &str) -> String {
            if let Some((region, _)) = super::applied_lifetime_parameter_identity_with_supplier(
                program,
                self.application_owner,
                if self.application_argument_owner.is_valid() {
                    self.application_argument_owner
                } else {
                    self.application_owner
                },
                name,
                self.application_scopes,
            ) {
                return self.encode_lifetime(program, region);
            }
            if let Some(invalid) = self.invalid_application {
                invalid.set(true);
            }
            bare(RegionForm::Unresolved)
        }

        pub(crate) fn replacement_context(&self) -> Self {
            Self {
                substitutions: match self.substitution_semantics {
                    SubstitutionSemantics::Transitive => self.substitutions,
                    SubstitutionSemantics::Simultaneous => &[],
                },
                ..*self
            }
        }

        pub(crate) fn replacement_context_at(&self, replacement_ordinal: usize) -> Self {
            let Some((scope_ordinal, start)) = self
                .substitution_scope_starts
                .iter()
                .enumerate()
                .rev()
                .find(|(_, start)| **start <= replacement_ordinal)
            else {
                return self.replacement_context();
            };
            Self {
                substitutions: &self.substitutions[..*start],
                substitution_scope_starts: &self.substitution_scope_starts[..scope_ordinal],
                ..*self
            }
        }

        fn resolved_lifetime(
            &self,
            program: &TypedTrees,
            region: LifetimeParameterIdentity,
        ) -> String {
            let Some((region, _)) = super::apply_resolved_lifetime_parameter_identity(
                program,
                region,
                if self.application_argument_owner.is_valid() {
                    self.application_argument_owner
                } else {
                    self.application_owner
                },
                self.application_scopes,
            ) else {
                if let Some(invalid) = self.invalid_application {
                    invalid.set(true);
                }
                return bare(RegionForm::Unresolved);
            };
            self.encode_lifetime(program, region)
        }

        fn encode_lifetime(
            &self,
            program: &TypedTrees,
            region: LifetimeParameterIdentity,
        ) -> String {
            let region = self
                .resolved_lifetimes
                .iter()
                .find(|(parameter, _)| *parameter == region)
                .map_or(region, |(_, argument)| *argument);
            if let Some((_, binder)) = self
                .lifetime_binders
                .iter()
                .find(|(parameter, _)| *parameter == region)
            {
                return compound(
                    RegionForm::Region,
                    [
                        atom(Label::Owner, "schema"),
                        atom(Label::Ordinal, &binder.to_string()),
                    ],
                );
            }
            if self.qualification == TypeIdentityQualification::PackageQualified {
                let identity = if self.portable_machine_applications.is_some() {
                    super::lifetime_parameters::portable_lifetime_parameter_identity_in_context(
                        program, region, self,
                    )
                } else {
                    super::lifetime_parameters::portable_lifetime_parameter_identity(
                        program, region,
                    )
                };
                if let Some(identity) = identity {
                    return identity;
                }
                if let Some(invalid) = self.invalid_application {
                    invalid.set(true);
                }
                return bare(RegionForm::Unresolved);
            }
            compound(
                RegionForm::Region,
                [
                    atom(
                        Label::Owner,
                        &format!(
                            "{}:{}",
                            region.declaration.arena_index(),
                            region.declaration.generation()
                        ),
                    ),
                    atom(Label::Ordinal, &region.ordinal.to_string()),
                ],
            )
        }

        fn lifetime_slot(
            &self,
            program: &TypedTrees,
            name: &crate::name::Identifier,
            region: Option<LifetimeParameterIdentity>,
        ) -> String {
            if let Some(region) = region.filter(|region| region.declaration.is_valid()) {
                self.resolved_lifetime(program, region)
            } else {
                self.lifetime(program, name.as_str())
            }
        }

        pub(crate) fn name(
            &self,
            program: &TypedTrees,
            symbol: SymbolHandle,
            fallback: &str,
        ) -> String {
            if let Some((_, replacement)) = self
                .binders
                .iter()
                .find(|(candidate, _)| *candidate == symbol)
            {
                return replacement.clone();
            }
            if !symbol.is_valid() && !fallback.contains("::") {
                let mut replacements = self.binders.iter().filter(|(candidate, _)| {
                    candidate.is_valid() && program.symbols.name(*candidate) == fallback
                });
                if let Some((_, replacement)) = replacements.next() {
                    if replacements.any(|(_, other)| other != replacement) {
                        return substitution::rejected(self);
                    }
                    return replacement.clone();
                }
            }
            if symbol.is_valid() {
                let path = program.symbols.display_path(symbol, "::");
                if !path.is_empty() {
                    return self.qualify_non_binder_name(program, symbol, path);
                }
            }
            self.qualify_non_binder_name(program, symbol, fallback.to_owned())
        }

        pub(crate) fn qualify_non_binder_name(
            &self,
            program: &TypedTrees,
            symbol: SymbolHandle,
            path: String,
        ) -> String {
            if let Some(projection) = self.portable_machine_applications
                && let Some(identity) = projection.nominal_name(program, symbol, self)
            {
                return identity;
            }
            match super::derived_declarations::nominal_name(program, symbol, self, false) {
                Ok(Some(identity)) => return identity,
                Err(reason) => {
                    if let Some(missing) = self.missing_exact_nominal_owner {
                        missing.set(true);
                    }
                    return atom(NameForm::InvalidDerivedOrigin, reason);
                }
                Ok(None) => {}
            }
            if self.qualification == TypeIdentityQualification::Ordinary
                && (program.symbols.builtin_type_atom(symbol).is_some()
                    || (program.symbols.symbol_package_identity(symbol).is_none()
                        && !program
                            .symbols
                            .symbol_source_origin(symbol)
                            .is_some_and(source::SourceOrigin::is_toolchain)))
            {
                return path;
            }
            if let Some(builtin_atom) = program.symbols.builtin_type_atom(symbol) {
                return compound(
                    NameForm::CompilerType,
                    [atom(Label::Atom, builtin_atom.identity())],
                );
            }
            let owner = if let Some(package) = program.symbols.symbol_package_identity(symbol) {
                PackageQualifiedNominalOwner::Package(package.digest())
            } else if program
                .symbols
                .symbol_source_origin(symbol)
                .is_some_and(source::SourceOrigin::is_toolchain)
            {
                program
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .and_then(|span| {
                        self.exact_toolchain_sources
                            .iter()
                            .find(|(source_id, _)| *source_id == span.source_id)
                    })
                    .map(|(_, digest)| PackageQualifiedNominalOwner::ToolchainSource(*digest))
                    .unwrap_or_else(|| {
                        if let Some(missing) = self.missing_exact_nominal_owner {
                            missing.set(true);
                        }
                        PackageQualifiedNominalOwner::Toolchain
                    })
            } else {
                if let Some(missing) = self.missing_exact_nominal_owner {
                    missing.set(true);
                }
                PackageQualifiedNominalOwner::Unresolved
            };
            package_qualified_nominal_name(owner, &path)
        }
    }

    pub(crate) fn package_qualified_nominal_name(
        owner: PackageQualifiedNominalOwner,
        path: &str,
    ) -> String {
        compound(NameForm::Nominal, [owner.encode(), atom(Label::Path, path)])
    }

    fn generic_type_identity(
        program: &TypedTrees,
        symbol: SymbolHandle,
        name: &str,
        lifetimes: &[crate::name::Identifier],
        regions: &[LifetimeParameterIdentity],
        arguments: impl Iterator<Item = String>,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        let mut arguments = arguments.peekable();
        if context.declared_carrier && arguments.peek().is_none() {
            return compound(
                TypeForm::Named,
                [normalize_const_or_nominal_name(
                    program,
                    symbol,
                    name,
                    Label::Name,
                    context,
                )],
            );
        }
        compound(
            TypeForm::Generic,
            std::iter::once(atom(Label::Name, &context.name(program, symbol, name)))
                .chain(
                    context
                        .invalid_application
                        .filter(|_| !context.declared_carrier)
                        .into_iter()
                        .flat_map(|_| {
                            lifetimes.iter().enumerate().map(|(ordinal, lifetime)| {
                                context.lifetime_slot(
                                    program,
                                    lifetime,
                                    regions.get(ordinal).copied(),
                                )
                            })
                        }),
                )
                .chain(arguments),
        )
    }

    fn generic_reference_identity<'program>(
        program: &'program TypedTrees,
        mut symbol: SymbolHandle,
        mut name: &'program crate::name::Identifier,
        lifetimes: &[crate::name::Identifier],
        regions: &[LifetimeParameterIdentity],
        mut arguments: arena::HandleSpan<TypeReferenceHandle>,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        if arguments.is_empty()
            && let Some(definition) = program.unique_data_definition_by_symbol(symbol)
            && program.data_type_parameters(definition).is_empty()
            && let Some(origin) = definition.generic_instance
            && program.type_reference_table.contains_type_reference(origin)
            && let TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments: origin_arguments,
                lifetime_arguments: origin_lifetimes,
            } = program.type_reference_table.type_reference(origin)
            && *base_symbol != symbol
        {
            if let Some(encountered) = context.encountered_regions
                && (!origin_lifetimes.is_empty()
                    || program
                        .data_definition_by_symbol(*base_symbol)
                        .is_some_and(|declaration| !declaration.lifetime_parameters.is_empty()))
            {
                encountered.set(true);
            }
            symbol = *base_symbol;
            name = base_name;
            arguments = *origin_arguments;
        }
        let constructed = context.identifies_constructed_values();
        let lifetime_is_relevant =
            |ordinal| !constructed || program.application_lifetime_is_relevant(symbol, ordinal);
        let argument_is_relevant =
            |ordinal| !constructed || program.application_argument_is_relevant(symbol, ordinal);
        let lifetimes = lifetimes
            .iter()
            .enumerate()
            .filter(|(ordinal, _)| lifetime_is_relevant(*ordinal))
            .map(|(_, lifetime)| lifetime.clone())
            .collect::<Vec<_>>();
        let regions = regions
            .iter()
            .enumerate()
            .filter(|(ordinal, _)| lifetime_is_relevant(*ordinal))
            .map(|(_, region)| *region)
            .collect::<Vec<_>>();
        let (lifetimes, regions) = (lifetimes.as_slice(), regions.as_slice());
        let arguments = program
            .type_reference_handles(arguments)
            .iter()
            .enumerate()
            .filter(|(ordinal, _)| argument_is_relevant(*ordinal))
            .collect::<Vec<_>>();
        let parameters = program.declaration_type_parameters(symbol);
        if constructed {
            symbol = program.implementation_value_head(symbol);
        }
        if arguments.is_empty()
            && (lifetimes.is_empty()
                || context.invalid_application.is_none()
                || context.declared_carrier)
        {
            return compound(
                TypeForm::Named,
                [normalize_const_or_nominal_name(
                    program,
                    symbol,
                    name.as_str(),
                    Label::Name,
                    context,
                )],
            );
        }
        generic_type_identity(
            program,
            symbol,
            name.as_str(),
            lifetimes,
            regions,
            arguments.into_iter().map(|(ordinal, argument)| {
                if context.storage_value_indices
                    && parameters.get(ordinal).is_some_and(|parameter| {
                        super::storage_identity::independent_value_index(program, symbol, parameter)
                    })
                {
                    return bare(TypeForm::Unit);
                }
                if context.invalid_application.is_some()
                    && parameters.get(ordinal).is_some_and(|parameter| {
                        matches!(
                            parameter.kind,
                            crate::data::TypeParameterKind::Machine { .. }
                        )
                    })
                {
                    return match program.static_operand_from_type_reference(*argument) {
                        Ok(selected) => substitution::index_argument(
                            program,
                            super::IndexIdentityArgument::StaticArgument(&selected),
                            context,
                        ),
                        Err(_) => substitution::rejected(context),
                    };
                }
                let is_index = parameters
                    .get(ordinal)
                    .is_some_and(|parameter| parameter.kind.is_const_or_value());
                if is_index {
                    substitution::index_argument(
                        program,
                        super::IndexIdentityArgument::TypeReference(*argument),
                        context,
                    )
                } else {
                    normalize_type_reference(program, *argument, context)
                }
            }),
            context,
        )
    }

    pub(crate) fn static_nominal_argument_identity(
        program: &TypedTrees,
        argument: &crate::expression::StaticMachineArgument,
        context: &TypeIdentityContext<'_>,
    ) -> Option<String> {
        if argument
            .application
            .as_ref()
            .is_some_and(|application| application.member.is_valid())
        {
            return None;
        }
        if context.invalid_application.is_none()
            || !argument.symbol.is_valid()
            || !matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::Data
                    | symbols::SymbolKind::BuiltinType
                    | symbols::SymbolKind::TypeParameter
                    | symbols::SymbolKind::Conformance
                    | symbols::SymbolKind::ConformanceParameter
            )
        {
            return None;
        }
        if let Some((replacement, outer)) = context.applied_argument(argument.symbol) {
            return Some(substitution::index_argument(program, replacement, &outer));
        }
        if argument.application.is_none()
            && program
                .data_type_parameter_by_symbol(argument.symbol)
                .is_some_and(|binder| binder.kind.is_const_or_value())
        {
            return None;
        }
        let constructed = context.identifies_constructed_values();
        let head = if constructed {
            program.implementation_value_head(argument.symbol)
        } else {
            argument.symbol
        };
        if let Some(application) = &argument.application {
            let lifetimes = application
                .lifetime_arguments
                .iter()
                .enumerate()
                .filter(|(ordinal, _)| {
                    !constructed
                        || program.application_lifetime_is_relevant(argument.symbol, *ordinal)
                })
                .map(|(_, lifetime)| lifetime.clone())
                .collect::<Vec<_>>();
            let arguments = application
                .arguments
                .iter()
                .enumerate()
                .filter(|(ordinal, _)| {
                    !constructed
                        || program.application_argument_is_relevant(argument.symbol, *ordinal)
                })
                .map(|(_, argument)| {
                    substitution::index_argument(
                        program,
                        super::IndexIdentityArgument::StaticArgument(argument),
                        context,
                    )
                })
                .collect::<Vec<_>>();
            let binders_dropped = lifetimes.len() != application.lifetime_arguments.len()
                || arguments.len() != application.arguments.len();
            if (head != argument.symbol || binders_dropped)
                && lifetimes.is_empty()
                && arguments.is_empty()
            {
                return Some(compound(
                    TypeForm::Named,
                    [normalize_const_or_nominal_name(
                        program,
                        head,
                        "",
                        Label::Name,
                        context,
                    )],
                ));
            }
            return Some(generic_type_identity(
                program,
                head,
                "",
                &lifetimes,
                &[],
                arguments.into_iter(),
                context,
            ));
        }
        if let Some(origin) = program
            .data_definition_by_symbol(argument.symbol)
            .and_then(|data| data.generic_instance)
        {
            return Some(normalize_type_reference(program, origin, context));
        }
        Some(compound(
            TypeForm::Named,
            [normalize_const_or_nominal_name(
                program,
                head,
                "",
                Label::Name,
                context,
            )],
        ))
    }

    pub(crate) fn normalize_type_reference(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        if let Some(encountered) = context.encountered_regions {
            let has_regions = match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { .. } => true,
                TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    ..
                } => {
                    !lifetime_arguments.is_empty()
                        || program
                            .data_definition_by_symbol(*base_symbol)
                            .is_some_and(|declaration| !declaration.lifetime_parameters.is_empty())
                        || program
                            .trait_definition_by_symbol(*base_symbol)
                            .is_some_and(|declaration| !declaration.lifetime_parameters.is_empty())
                }
                TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::DynamicTrait { symbol, .. } => {
                    program
                        .data_definition_by_symbol(*symbol)
                        .is_some_and(|declaration| !declaration.lifetime_parameters.is_empty())
                        || program
                            .trait_definition_by_symbol(*symbol)
                            .is_some_and(|declaration| !declaration.lifetime_parameters.is_empty())
                }
                _ => false,
            };
            if has_regions {
                encountered.set(true);
            }
        }
        let scoped_context = context.enter_type_operand(program, type_reference);
        let context = &scoped_context;
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
            && let Some((argument, outer)) = context.applied_argument(*symbol)
        {
            return substitution::index_argument(program, argument, &outer);
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
            && let Some((
                _,
                super::IndexIdentityArgument::TypeReference(reference)
                | super::IndexIdentityArgument::TypeArgument(reference),
            )) = context
                .index_substitutions
                .iter()
                .find(|(binder, _)| binder == symbol)
        {
            return normalize_type_reference(
                program,
                *reference,
                &TypeIdentityContext {
                    index_substitutions: &[],
                    ..*context
                },
            );
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
            && let Some(generic_instance) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            && generic_instance != type_reference
        {
            return normalize_type_reference(program, generic_instance, context);
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
            && let Some((ordinal, (_, replacement))) = context
                .substitutions
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (candidate, _))| candidate == symbol)
            && *replacement != type_reference
        {
            return normalize_type_reference(
                program,
                *replacement,
                &context.replacement_context_at(ordinal),
            );
        }
        if context.static_context.is_valid() {
            let refined =
                program.statically_refined_type_reference(context.static_context, type_reference);
            if refined != type_reference
                && crate::domain::index_argument_is_closed(program, refined)
            {
                return normalize_type_reference(program, refined, context);
            }
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => compound(
                match access {
                    language_core::ReferenceAccess::Shared => TypeForm::Reference,
                    language_core::ReferenceAccess::Mutable => TypeForm::MutableReference,
                    language_core::ReferenceAccess::WriteOnly => TypeForm::WriteOnlyReference,
                },
                std::iter::once(normalize_type_reference(program, *referee, context)).chain(
                    context
                        .invalid_application
                        .filter(|_| !context.declared_carrier)
                        .map(|_| {
                            lifetime.as_ref().map_or_else(
                                || {
                                    if context.qualification
                                        == TypeIdentityQualification::PackageQualified
                                        && let Some(elided) = context.encountered_elided_regions
                                    {
                                        elided.set(true);
                                    }
                                    bare(RegionForm::Elided)
                                },
                                |lifetime| {
                                    context.lifetime_slot(
                                        program,
                                        lifetime,
                                        program
                                            .type_reference_table
                                            .operand_lifetimes(type_reference)
                                            .first()
                                            .copied(),
                                    )
                                },
                            )
                        }),
                ),
            ),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if context.declared_carrier {
                    return normalize_type_reference(program, *base_type, context);
                }
                let (base, mut all_constraints) =
                    normalize_constrained_base(program, *base_type, context);
                all_constraints.extend(normalized_constraints(program, *constraints, context));
                all_constraints.sort();
                all_constraints.dedup();
                compound(
                    TypeForm::Constrained,
                    std::iter::once(base).chain(
                        all_constraints
                            .into_iter()
                            .map(NormalizedConstraint::encode),
                    ),
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => compound(
                TypeForm::Array,
                [
                    normalize_type_reference(program, *element_type, context),
                    normalize_array_length(program, length, context),
                ],
            ),
            TypeReferenceNode::Slice { element_type } => compound(
                TypeForm::Slice,
                [normalize_type_reference(program, *element_type, context)],
            ),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
                ..
            } => generic_reference_identity(
                program,
                *base_symbol,
                base_name,
                lifetime_arguments,
                program
                    .type_reference_table
                    .operand_lifetimes(type_reference),
                *arguments,
                context,
            ),
            TypeReferenceNode::ConstExpression(expression) => compound(
                TypeForm::IndexExpression,
                [normalize_index_expression(program, *expression, context)],
            ),
            TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                arguments,
            } => compound(
                TypeForm::DynamicTrait,
                std::iter::once(atom(
                    Label::Name,
                    &context.name(program, *symbol, name.as_str()),
                ))
                .chain(
                    program
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|argument| normalize_type_reference(program, *argument, context)),
                ),
            ),
            TypeReferenceNode::Named { symbol, name } => compound(
                TypeForm::Named,
                [normalize_const_or_nominal_name(
                    program,
                    if context.identifies_constructed_values() {
                        program.implementation_value_head(*symbol)
                    } else {
                        *symbol
                    },
                    name.as_str(),
                    Label::Name,
                    context,
                )],
            ),
            TypeReferenceNode::Unit => bare(TypeForm::Unit),
        }
    }

    pub(crate) fn normalize_index_expression(
        program: &TypedTrees,
        expression: ExpressionHandle,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if path.static_application.is_valid() {
                    return match program
                        .expression_table
                        .static_member(path.static_application)
                    {
                        Some(argument) => substitution::index_argument(
                            program,
                            crate::type_identity::IndexIdentityArgument::StaticArgument(argument),
                            context,
                        ),
                        None => substitution::rejected(context),
                    };
                }
                let members = program.expression_table.name_path_members(path.members);
                let fallback = members.joined_path();
                let symbol = if path.symbol.is_valid() {
                    path.symbol
                } else if members.len() == 1 {
                    path.head_symbol
                } else {
                    SymbolHandle::invalid()
                };
                let identity = substitution::index(program, symbol, &fallback, context)
                    .unwrap_or_else(|| {
                        substitution::index_name(program, symbol, fallback.as_str(), context)
                    });
                if program.pattern_component(expression)
                    == Some(language_core::PatternComponent::Case)
                {
                    compound(IndexForm::CaseComponent, [identity])
                } else {
                    identity
                }
            }
            ExpressionNode::Binary(binary) => {
                if open_index_operation_selection(program, expression).is_none() {
                    return compound(
                        index_binary_operator_name(binary.operator),
                        [
                            normalize_index_expression(program, binary.left, context),
                            normalize_index_expression(program, binary.right, context),
                        ],
                    );
                }
                normalize_selected_index_operation(program, expression, context)
            }
            ExpressionNode::Integer(value) => substitution::index_integer(value, context),
            ExpressionNode::Unary(unary) => compound(
                match unary.operator {
                    crate::expression::UnaryOperator::BitwiseNot => IndexForm::BitwiseNot,
                    crate::expression::UnaryOperator::LogicalNot => IndexForm::LogicalNot,
                },
                [normalize_index_expression(program, unary.operand, context)],
            ),
            ExpressionNode::Boolean(value) => atom(IndexForm::Boolean, &value.to_string()),
            ExpressionNode::Float(value) => atom(IndexForm::Float, &value.to_string()),
            ExpressionNode::UndeterminedNaN(format) => {
                atom(IndexForm::UndeterminedNan, format.name())
            }
            ExpressionNode::String(value) => byte_atom(IndexForm::String, value),
            ExpressionNode::ArrayLiteral(elements) => compound(
                IndexForm::ArrayLiteral,
                program
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .map(|element| normalize_index_expression(program, *element, context)),
            ),
            ExpressionNode::Match(dispatch) => {
                match_dispatch::normalize_match(program, expression, dispatch, context)
            }
            ExpressionNode::SavedSubject(saved) => {
                match_dispatch::normalize_saved_subject(program, saved, context)
            }
            ExpressionNode::Atomic(atomic) => compound(
                IndexForm::Atomic,
                [
                    normalize_index_expression(program, atomic.value, context),
                    normalize_index_expression(program, atomic.result, context),
                    atom(Label::Ordering, &format!("{:?}", atomic.ordering)),
                    atom(
                        Label::ResultCustody,
                        &format!("{:?}", atomic.result_custody),
                    ),
                ],
            ),
            ExpressionNode::Cast(cast) => compound(
                IndexForm::Cast,
                [
                    normalize_index_expression(program, cast.value, context),
                    normalize_type_reference(program, cast.target_type, context),
                    atom(Label::Domain, cast.domain.name()),
                ],
            ),
            ExpressionNode::Call(call) => compound(
                IndexForm::Call,
                std::iter::once(atom(
                    Label::Target,
                    &context.name(program, call.target_symbol, call.target.as_str()),
                ))
                .chain({
                    let arguments =
                        crate::references::StaticArgumentCall::Expression(expression, call)
                            .arguments(program);
                    (!arguments.is_empty()).then(|| {
                        compound(
                            Label::StaticArguments,
                            arguments.iter().map(|argument| {
                                substitution::index_argument(
                                    program,
                                    super::IndexIdentityArgument::StaticArgument(argument),
                                    context,
                                )
                            }),
                        )
                    })
                })
                .chain(
                    call.receiver
                        .is_valid()
                        .then(|| normalize_index_expression(program, call.receiver, context)),
                )
                .chain(
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| normalize_index_expression(program, *argument, context)),
                ),
            ),
            ExpressionNode::Indexed(indexed) => compound(
                IndexForm::Indexed,
                [
                    normalize_index_expression(program, indexed.collection, context),
                    normalize_index_expression(program, indexed.index, context),
                ],
            ),
            ExpressionNode::Member(member) => compound(
                IndexForm::Member,
                std::iter::once(normalize_index_expression(
                    program,
                    member.receiver,
                    context,
                ))
                .chain(std::iter::once(atom(Label::Member, member.member.as_str())))
                .chain(
                    member
                        .case_variant
                        .iter()
                        .map(|variant| atom(Label::Case, variant.as_str())),
                ),
            ),
            ExpressionNode::Borrow(borrow) => compound(
                match borrow.access {
                    language_core::ReferenceAccess::Shared => IndexForm::Borrow,
                    language_core::ReferenceAccess::Mutable => IndexForm::MutableBorrow,
                    language_core::ReferenceAccess::WriteOnly => IndexForm::WriteOnlyBorrow,
                },
                [normalize_index_expression(program, borrow.target, context)],
            ),
            ExpressionNode::Range(range) => compound(
                if range.end_inclusive {
                    IndexForm::InclusiveRange
                } else {
                    IndexForm::ExclusiveRange
                },
                [
                    normalize_index_expression(program, range.start, context),
                    normalize_index_expression(program, range.end, context),
                ],
            ),
            ExpressionNode::StructLiteral(literal) => compound(
                IndexForm::StructLiteral,
                std::iter::once(atom(
                    Label::Type,
                    &context.name(program, literal.type_symbol, literal.type_name.as_str()),
                ))
                .chain(
                    literal
                        .case_name
                        .as_ref()
                        .map(|case| atom(Label::Case, case.as_str())),
                )
                .chain(
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .map(|field| {
                            compound(
                                Label::Field,
                                [
                                    atom(Label::Name, field.name.as_str()),
                                    normalize_index_expression(program, field.value, context),
                                ],
                            )
                        }),
                ),
            ),
            ExpressionNode::TypeExpression(type_reference) => compound(
                IndexForm::TypeExpression,
                [normalize_type_reference(program, *type_reference, context)],
            ),
            ExpressionNode::ZeroValue(type_reference) => compound(
                IndexForm::ZeroValue,
                [normalize_type_reference(program, *type_reference, context)],
            ),
        }
    }

    pub(crate) fn normalize_const_or_nominal_name(
        program: &TypedTrees,
        symbol: SymbolHandle,
        spelling: &str,
        nominal_tag: impl Spelled,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        if !symbol.is_valid() {
            if let Some(value) =
                language_semantics::const_value::CanonicalConstValue::from_atom(spelling)
            {
                return compound(
                    IndexForm::CanonicalConst,
                    [
                        atom(Label::Type, &value.type_name),
                        atom(Label::Encoding, &value.encoding),
                    ],
                );
            }
            if let Ok(value) = spelling.parse::<i128>() {
                return atom(IndexForm::IntegerConst, &value.to_string());
            }
        }
        atom(nominal_tag, &context.name(program, symbol, spelling))
    }
}
pub(crate) mod lifetime_parameters {
    use super::constraint_identity::{atom, compound};
    use crate::TypedTrees;
    use language_semantics::type_identity::vocabulary::{Label, RegionForm};
    use symbols::SymbolHandle;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct LifetimeParameterIdentity {
        pub declaration: SymbolHandle,
        pub ordinal: usize,
    }

    pub fn portable_lifetime_parameter_identity(
        program: &TypedTrees,
        region: LifetimeParameterIdentity,
    ) -> Option<String> {
        portable_lifetime_parameter_identity_in_context(
            program,
            region,
            &super::identity_context::TypeIdentityContext {
                qualification: super::TypeIdentityQualification::PackageQualified,
                ..Default::default()
            },
        )
    }

    pub(crate) fn portable_lifetime_parameter_identity_in_context(
        program: &TypedTrees,
        region: LifetimeParameterIdentity,
        context: &super::identity_context::TypeIdentityContext<'_>,
    ) -> Option<String> {
        lifetime_parameter_name(program, region)?;
        let owner = region.declaration;
        let (kind, declaration) = if let Some(machine) = program.machine_by_symbol(owner) {
            ("machine", program.machine_callable_identity_text(machine)?)
        } else if let Some((declaration, requirement)) = program.trait_machine_signature(owner) {
            (
                "requirement",
                program.trait_requirement_callable_identity_text(declaration, requirement),
            )
        } else if program.conformance_by_symbol(owner).is_some() {
            ("implementation", program.symbols.display_path(owner, "::"))
        } else if let Some(declaration) = program.trait_definition_by_symbol(owner) {
            ("trait", program.trait_declaration_path(declaration))
        } else if program
            .type_aliases()
            .iter()
            .any(|alias| alias.symbol == owner)
        {
            (
                "type-alias-parameter",
                program.symbols.display_path(owner, "::"),
            )
        } else if program.data_definition_by_symbol(owner).is_some() {
            ("data", program.symbols.display_path(owner, "::"))
        } else {
            return None;
        };
        if declaration.is_empty() {
            return None;
        }
        let coordinate = context.qualify_non_binder_name(program, owner, declaration);
        Some(compound(
            RegionForm::Region,
            [
                atom(Label::Kind, kind),
                atom(Label::Declaration, &coordinate),
                atom(Label::Ordinal, &region.ordinal.to_string()),
            ],
        ))
    }

    pub fn applied_lifetime_parameter_identity(
        program: &TypedTrees,
        lexical_owner: SymbolHandle,
        name: &str,
        scopes: &[super::AppliedTypeIdentityScope<'_>],
    ) -> Option<LifetimeParameterIdentity> {
        applied_lifetime_parameter_identity_with_supplier(
            program,
            lexical_owner,
            lexical_owner,
            name,
            scopes,
        )
        .map(|(identity, _)| identity)
    }

    pub fn applied_lifetime_parameter_identity_with_supplier(
        program: &TypedTrees,
        lexical_owner: SymbolHandle,
        argument_owner: SymbolHandle,
        name: &str,
        scopes: &[super::AppliedTypeIdentityScope<'_>],
    ) -> Option<(LifetimeParameterIdentity, bool)> {
        let lexical_declaration = program.lifetime_declaration_owner(lexical_owner);
        if let Some(region) = lifetime_parameter_identity(program, lexical_owner, name)
            && region.declaration == lexical_declaration
            && !scopes.iter().any(|scope| {
                program.lifetime_declaration_owner(scope.declaration) == lexical_declaration
            })
        {
            return Some((region, false));
        }
        for (ordinal, scope) in scopes.iter().enumerate().rev() {
            if let Some((_, argument)) = scope
                .lifetimes
                .iter()
                .find(|(parameter, _)| parameter == name)
            {
                return applied_lifetime_parameter_identity(
                    program,
                    argument_owner,
                    argument,
                    &scopes[..ordinal],
                )
                .map(|identity| (identity, true));
            }
            if let Some(region) = lifetime_parameter_identity(program, scope.declaration, name)
                && region.declaration == program.lifetime_declaration_owner(scope.declaration)
            {
                return Some((region, false));
            }
        }
        lifetime_parameter_identity(program, lexical_owner, name).map(|identity| (identity, false))
    }

    pub fn apply_resolved_lifetime_parameter_identity(
        program: &TypedTrees,
        region: LifetimeParameterIdentity,
        argument_owner: SymbolHandle,
        scopes: &[super::AppliedTypeIdentityScope<'_>],
    ) -> Option<(LifetimeParameterIdentity, bool)> {
        let parameter = lifetime_parameter_name(program, region)?;
        for (ordinal, scope) in scopes.iter().enumerate().rev() {
            if program.lifetime_declaration_owner(scope.declaration) != region.declaration {
                continue;
            }
            let Some((_, argument)) = scope
                .lifetimes
                .iter()
                .find(|(name, _)| name == parameter.as_str())
            else {
                return Some((region, false));
            };
            return applied_lifetime_parameter_identity(
                program,
                argument_owner,
                argument,
                &scopes[..ordinal],
            )
            .map(|actual| (actual, true));
        }
        Some((region, false))
    }

    pub fn lifetime_parameter_identity(
        program: &TypedTrees,
        lexical_owner: SymbolHandle,
        name: &str,
    ) -> Option<LifetimeParameterIdentity> {
        let mut owner = program.lifetime_declaration_owner(lexical_owner);
        while owner.is_valid() {
            let parameters = declaration_lifetime_parameters(program, owner);
            if let Some(ordinal) = parameters.and_then(|parameters| {
                parameters
                    .iter()
                    .position(|parameter| parameter.as_str() == name)
            }) {
                return Some(LifetimeParameterIdentity {
                    declaration: owner,
                    ordinal,
                });
            }
            owner = program.lifetime_declaration_owner(program.declaration_scope_parent(owner));
        }
        None
    }

    pub fn lifetime_parameter_name(
        program: &TypedTrees,
        region: LifetimeParameterIdentity,
    ) -> Option<&crate::name::Identifier> {
        declaration_lifetime_parameters(program, region.declaration)?.get(region.ordinal)
    }

    pub fn declaration_lifetime_parameters(
        program: &TypedTrees,
        owner: SymbolHandle,
    ) -> Option<&[crate::name::Identifier]> {
        program
            .machine_by_symbol(owner)
            .map(|owner| owner.lifetime_parameters.as_slice())
            .or_else(|| {
                program
                    .type_aliases()
                    .iter()
                    .find(|alias| alias.symbol == owner)
                    .map(|alias| alias.lifetime_parameters.as_slice())
            })
            .or_else(|| {
                program
                    .conformance_by_symbol(owner)
                    .map(|owner| owner.lifetime_parameters.as_slice())
            })
            .or_else(|| {
                program
                    .trait_definition_by_symbol(owner)
                    .map(|owner| owner.lifetime_parameters.as_slice())
            })
            .or_else(|| {
                program
                    .data_definition_by_symbol(owner)
                    .map(|owner| owner.lifetime_parameters.as_slice())
            })
            .or_else(|| {
                program
                    .trait_machine_signature(owner)
                    .map(|(_, signature)| signature.lifetime_parameters.as_slice())
            })
            .or_else(|| {
                program
                    .retained_static_machine_contract(owner)
                    .map(|contract| contract.signature().lifetime_parameters.as_slice())
            })
    }
}
pub(crate) mod normalized_identities {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TypeIdentityId(u32);

    /*
        Written under the table's canonical numbering when one is installed
        (`TypeIdentityTable::renumber_canonically`), so the bytes do not depend
        on which checking thread first minted an identity.
    */
    impl compiler_storage::Record for TypeIdentityId {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            output.record(|output| {
                let number = match self.slot() {
                    None => 0,
                    Some(slot) => u32::try_from(output.renumbered::<TypeIdentityId>(slot)? + 1)
                        .map_err(|_| output.error("type identity number overflow"))?,
                };
                compiler_storage::Record::write(&number, output)
            })
        }

        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            input.record(|input| Ok(Self(compiler_storage::Record::read(input)?)))
        }
    }

    impl TypeIdentityId {
        pub fn is_valid(self) -> bool {
            self.0 != 0
        }

        pub fn storage_index(self) -> Option<usize> {
            self.slot()
        }

        fn slot(self) -> Option<usize> {
            (self.0 != 0).then(|| self.0 as usize - 1)
        }

        fn from_slot(slot: usize) -> Self {
            Self(slot as u32 + 1)
        }
    }

    pub type NormalizedTypeIdentity = TypeIdentityId;

    #[derive(Clone, Default, PartialEq, Eq, Debug)]
    pub struct TypeIdentityTable {
        texts: arena::OwnedTextInterner,
    }

    /*
        Parallel checking passes mint identities into one shared table in the
        order their threads first ask, so the same program numbers them
        differently from run to run. A record of the program is written under
        a canonical numbering instead: the table's texts in sorted order, and
        every `TypeIdentityId` by its text's place there. Equal programs then
        write equal bytes, which exact-key reuse of checked slices needs.
        Reading needs no change; the table comes back in canonical order.
    */
    impl compiler_storage::Record for TypeIdentityTable {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            output.record(|output| {
                let installed = output
                    .renumbering_owner::<TypeIdentityId>()
                    .map(|(owner, numbers)| (owner, numbers.to_vec()));
                match installed {
                    None => compiler_storage::Record::write(&self.texts, output),
                    Some((owner, _)) if owner != self.owner() => Err(output
                        .error("a second type identity table under another table's numbering")),
                    Some((_, numbers)) => self.texts.write_renumbered(output, &numbers),
                }
            })
        }

        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            input.record(|input| {
                Ok(Self {
                    texts: compiler_storage::Record::read(input)?,
                })
            })
        }
    }

    impl TypeIdentityTable {
        fn owner(&self) -> *const () {
            std::ptr::from_ref(self).cast()
        }

        /*
            Installs this table's canonical numbering on `output`; see the
            `Record` implementation. Every identity in the written value must
            come from this table, and none may be minted while it is written.
        */
        pub fn renumber_canonically(&self, output: &mut compiler_storage::RecordWriter) {
            output.renumber::<TypeIdentityId>(self.owner(), self.texts.sorted_numbering().into());
        }

        pub fn intern(&self, text: &str) -> TypeIdentityId {
            TypeIdentityId::from_slot(self.texts.intern(text))
        }

        pub fn text(&self, identity: TypeIdentityId) -> Option<&str> {
            self.texts.text(identity.slot()?)
        }

        pub fn by_text(&self, text: &str) -> TypeIdentityId {
            self.texts
                .slot(text)
                .map(TypeIdentityId::from_slot)
                .unwrap_or_default()
        }
    }

    #[cfg(test)]
    mod canonical_numbering_tests {
        use super::{TypeIdentityId, TypeIdentityTable};
        use compiler_storage::RecordLimits;
        use std::collections::BTreeMap;

        /* A program's identities as checking leaves them: a table, the
        identities its facts name, and a map ordered by identity. */
        #[derive(Default, compiler_storage::Record)]
        struct Checked {
            identities: TypeIdentityTable,
            named: Vec<TypeIdentityId>,
            by_identity: BTreeMap<TypeIdentityId, u32>,
        }

        const NAMED: [&str; 3] = ["b", "a", "c"];

        fn checked(minted: &[&str]) -> Checked {
            let identities = TypeIdentityTable::default();
            for text in minted {
                identities.intern(text);
            }
            let named: Vec<_> = NAMED.iter().map(|text| identities.by_text(text)).collect();
            let by_identity = named
                .iter()
                .zip(0..)
                .map(|(id, place)| (*id, place))
                .collect();
            Checked {
                identities,
                named,
                by_identity,
            }
        }

        fn encoded(checked: &Checked) -> Result<Vec<u8>, compiler_storage::RecordError> {
            compiler_storage::encode_with(checked, RecordLimits::default(), |output| {
                checked.identities.renumber_canonically(output)
            })
        }

        #[test]
        fn a_program_whose_threads_minted_in_another_order_writes_equal_bytes() {
            let first = checked(&["a", "b", "c"]);
            let raced = checked(&["c", "a", "b"]);
            assert_ne!(
                first.named, raced.named,
                "the two orders mint other numbers"
            );
            let bytes = encoded(&raced).unwrap();
            assert_eq!(encoded(&first).unwrap(), bytes);
            let read: Checked = compiler_storage::decode(&bytes, RecordLimits::default()).unwrap();
            for (identity, text) in read.named.iter().zip(NAMED) {
                assert_eq!(read.identities.text(*identity), Some(text));
            }
            let places: Vec<u32> = NAMED
                .iter()
                .map(|text| read.by_identity[&read.identities.by_text(text)])
                .collect();
            assert_eq!(places, [0, 1, 2]);
        }

        #[test]
        fn a_second_table_under_the_first_tables_numbering_is_refused() {
            #[derive(compiler_storage::Record)]
            struct Two {
                first: TypeIdentityTable,
                second: TypeIdentityTable,
            }
            let two = Two {
                first: checked(&["a"]).identities,
                second: checked(&["a", "b"]).identities,
            };
            let written = compiler_storage::encode_with(&two, RecordLimits::default(), |output| {
                two.first.renumber_canonically(output)
            });
            assert!(written.is_err());
        }

        #[test]
        fn an_identity_minted_after_the_numbering_was_taken_is_refused() {
            let checked = checked(&["a", "b", "c"]);
            let mut output = compiler_storage::RecordWriter::new(RecordLimits::default());
            checked.identities.renumber_canonically(&mut output);
            let late = checked.identities.intern("d");
            assert!(compiler_storage::Record::write(&late, &mut output).is_err());
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
    pub struct NormalizedResultDispatchSet {
        pub(crate) terms: Vec<NormalizedDomainTerm>,
    }

    impl NormalizedResultDispatchSet {
        pub fn terms(&self) -> &[NormalizedDomainTerm] {
            &self.terms
        }

        pub fn is_empty(&self) -> bool {
            self.terms.is_empty()
        }

        pub fn identity(&self) -> String {
            self.terms
                .iter()
                .map(|term| match term {
                    NormalizedDomainTerm::Arithmetic(name) => format!("arithmetic:{name}"),
                    NormalizedDomainTerm::Compiler(identity) => format!("compiler:{identity}"),
                    NormalizedDomainTerm::Declared(name) => format!("declared:{name}"),
                })
                .collect::<Vec<_>>()
                .join("&")
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum NormalizedDomainTerm {
        Arithmetic(String),
        Compiler(String),
        Declared(String),
    }
}
pub(crate) mod open_index_identity {
    use crate::TypedTrees;
    use crate::expression::{BinaryOperator, ExpressionHandle, OpenIndexOperationSelection};
    use crate::type_system::type_identity::constraint_identity::{atom, compound};
    use crate::type_system::type_identity::identity_context::{
        TypeIdentityContext, TypeIdentityQualification, normalize_index_expression,
    };
    use language_semantics::type_identity::vocabulary::{IndexForm, Label};

    const MAX_EXPANDED_SUMMANDS: usize = 64;

    pub(crate) fn open_index_operation_selection(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<&OpenIndexOperationSelection> {
        program.expression_table.open_index_operation(expression)
    }

    fn provider_identity(
        program: &TypedTrees,
        selection: &OpenIndexOperationSelection,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        super::substitution::index_argument(
            program,
            selection.application.as_ref().map_or(
                super::IndexIdentityArgument::Subject(selection.provider),
                super::IndexIdentityArgument::StaticArgument,
            ),
            context,
        )
    }

    pub(crate) fn open_index_algebra_identity(
        program: &TypedTrees,
        selection: &OpenIndexOperationSelection,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        compound(
            Label::OpenIndexAlgebra,
            [
                atom(
                    Label::Provider,
                    &provider_identity(program, selection, context),
                ),
                atom(
                    Label::Trait,
                    &context.name(
                        program,
                        selection.algebra_trait,
                        "<unresolved-algebra-trait>",
                    ),
                ),
                atom(Label::Requirement, &selection.algebra_requirement),
            ],
        )
    }

    pub(crate) fn open_index_operation_identity(
        program: &TypedTrees,
        selection: &OpenIndexOperationSelection,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        let contract = selection.operation_contract_identity(program);
        if context.qualification == TypeIdentityQualification::Ordinary {
            return contract;
        }
        compound(
            Label::OpenIndexOperation,
            [
                atom(
                    Label::Symbol,
                    &context.name(program, selection.operator, &contract),
                ),
                atom(Label::Contract, &contract),
            ],
        )
    }

    fn same_open_index_ac_authority(
        program: &TypedTrees,
        left: &OpenIndexOperationSelection,
        right: &OpenIndexOperationSelection,
        context: &TypeIdentityContext<'_>,
    ) -> bool {
        left.operator == right.operator
            && open_index_algebra_identity(program, left, context)
                == open_index_algebra_identity(program, right, context)
            && program.normalized_type_identity(left.operand_type)
                == program.normalized_type_identity(right.operand_type)
            && left.algebra_trait == right.algebra_trait
            && left.algebra_requirement == right.algebra_requirement
            && left.commutativity_licensed == right.commutativity_licensed
            && left.associativity_licensed == right.associativity_licensed
            && left.distributes_over == right.distributes_over
            && left.left_identity_literal == right.left_identity_literal
            && left.right_identity_literal == right.right_identity_literal
    }

    fn distributes_over(
        program: &TypedTrees,
        product: &OpenIndexOperationSelection,
        sum: &OpenIndexOperationSelection,
        context: &TypeIdentityContext<'_>,
    ) -> bool {
        product.algebra_trait == sum.algebra_trait
            && product.distributes_over.contains(&sum.algebra_requirement)
            && !sum.distributes_over.contains(&product.algebra_requirement)
            && program.normalized_type_identity(product.operand_type)
                == program.normalized_type_identity(sum.operand_type)
            && provider_identity(program, product, context)
                == provider_identity(program, sum, context)
    }

    #[derive(Clone)]
    enum IndexTerm<'program> {
        Leaf(String),
        Operation {
            operator: BinaryOperator,
            selection: &'program OpenIndexOperationSelection,
            operands: Vec<IndexTerm<'program>>,
        },
    }

    impl<'program> IndexTerm<'program> {
        fn of(
            program: &'program TypedTrees,
            expression: ExpressionHandle,
            context: &TypeIdentityContext<'_>,
        ) -> Self {
            match (
                program.expression_table.binary_node(expression),
                open_index_operation_selection(program, expression),
            ) {
                (Some(binary), Some(selection)) => Self::operation(
                    program,
                    binary.operator,
                    selection,
                    vec![
                        Self::of(program, binary.left, context),
                        Self::of(program, binary.right, context),
                    ],
                    context,
                ),
                _ => Self::Leaf(normalize_index_expression(program, expression, context)),
            }
        }

        fn operation(
            program: &'program TypedTrees,
            operator: BinaryOperator,
            selection: &'program OpenIndexOperationSelection,
            operands: Vec<Self>,
            context: &TypeIdentityContext<'_>,
        ) -> Self {
            let mut operands: Vec<Self> = if selection.associativity_licensed {
                operands
                    .into_iter()
                    .flat_map(|operand| match operand {
                        Self::Operation {
                            operator: inner,
                            selection: inner_selection,
                            operands: inner_operands,
                        } if inner == operator
                            && same_open_index_ac_authority(
                                program,
                                selection,
                                inner_selection,
                                context,
                            ) =>
                        {
                            inner_operands
                        }
                        operand => vec![operand],
                    })
                    .collect()
            } else {
                operands
            };
            operands = without_licensed_identities(program, selection, operands, context);
            if let [single] = operands.as_slice() {
                return single.clone();
            }
            if selection.commutativity_licensed {
                operands.sort_by_cached_key(|operand| operand.render(program, context));
            }
            let term = Self::Operation {
                operator,
                selection,
                operands,
            };
            term.expanded(program, context).unwrap_or(term)
        }

        fn expanded(
            &self,
            program: &'program TypedTrees,
            context: &TypeIdentityContext<'_>,
        ) -> Option<Self> {
            let Self::Operation {
                operator,
                selection,
                operands,
            } = self
            else {
                return None;
            };
            if selection.distributes_over.is_empty() {
                return None;
            }
            let licensed_sum = |operand: &Self| match operand {
                Self::Operation {
                    selection: sum,
                    operands,
                    ..
                } if distributes_over(program, selection, sum, context) => Some(operands.len()),
                _ => None,
            };
            let sum_position = operands
                .iter()
                .enumerate()
                .position(|(position, operand)| {
                    (selection.commutativity_licensed || position + 1 == operands.len())
                        && licensed_sum(operand).is_some()
                })?;
            let Self::Operation {
                operator: sum_operator,
                selection: sum_selection,
                operands: summands,
            } = &operands[sum_position]
            else {
                return None;
            };
            let produced = if selection.commutativity_licensed {
                operands.iter().fold(1usize, |count, operand| {
                    count.saturating_mul(licensed_sum(operand).unwrap_or(1))
                })
            } else {
                summands.len()
            };
            if produced > MAX_EXPANDED_SUMMANDS {
                return None;
            }
            let products = summands
                .iter()
                .map(|summand| {
                    let mut factors = operands
                        .iter()
                        .enumerate()
                        .filter(|(position, _)| *position != sum_position)
                        .map(|(_, factor)| factor.clone())
                        .collect::<Vec<_>>();
                    factors.push(summand.clone());
                    Self::operation(program, *operator, selection, factors, context)
                })
                .collect();
            Some(Self::operation(
                program,
                *sum_operator,
                sum_selection,
                products,
                context,
            ))
        }

        fn render(&self, program: &TypedTrees, context: &TypeIdentityContext<'_>) -> String {
            match self {
                Self::Leaf(text) => text.clone(),
                Self::Operation {
                    operator,
                    selection,
                    operands,
                } => compound(
                    index_binary_operator_name(*operator),
                    [
                        atom(
                            Label::Operation,
                            &open_index_operation_identity(program, selection, context),
                        ),
                        atom(
                            Label::Algebra,
                            &open_index_algebra_identity(program, selection, context),
                        ),
                    ]
                    .into_iter()
                    .chain(
                        operands
                            .iter()
                            .map(|operand| operand.render(program, context)),
                    ),
                ),
            }
        }
    }

    fn without_licensed_identities<'program>(
        program: &'program TypedTrees,
        selection: &'program OpenIndexOperationSelection,
        operands: Vec<IndexTerm<'program>>,
        context: &TypeIdentityContext<'_>,
    ) -> Vec<IndexTerm<'program>> {
        let len = operands.len();
        let matches = operands
            .iter()
            .enumerate()
            .map(|(index, operand)| {
                let rendered = operand.render(program, context);
                let left = literal_matches(&selection.left_identity_literal, &rendered);
                let right = literal_matches(&selection.right_identity_literal, &rendered);
                let removable = identity_operand_is_removable(index, len, left, right, selection);
                (rendered, removable)
            })
            .collect::<Vec<_>>();
        let mut shared = None;
        for (rendered, removable) in &matches {
            if !removable {
                continue;
            }
            match &shared {
                None => shared = Some(rendered.clone()),
                Some(existing) if existing == rendered => {}
                Some(_) => return operands,
            }
        }
        let mut kept = Vec::new();
        for (operand, (_, removable)) in operands.into_iter().zip(matches) {
            if removable {
                continue;
            }
            kept.push(operand);
        }
        if kept.is_empty()
            && let Some(text) = shared
        {
            return vec![IndexTerm::Leaf(text)];
        }
        kept
    }

    fn literal_matches(literal: &str, rendered: &str) -> bool {
        !literal.is_empty() && rendered == atom(IndexForm::Integer, literal)
    }

    fn identity_operand_is_removable(
        index: usize,
        len: usize,
        left: bool,
        right: bool,
        selection: &OpenIndexOperationSelection,
    ) -> bool {
        if !left && !right {
            return false;
        }
        let leading = index == 0;
        let trailing = index + 1 == len;
        let commutative = selection.commutativity_licensed;
        (right && (!leading || commutative || left))
            || (left && (!trailing || commutative || right))
    }

    pub(crate) fn normalize_selected_index_operation(
        program: &TypedTrees,
        expression: ExpressionHandle,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        IndexTerm::of(program, expression, context).render(program, context)
    }

    pub(crate) fn index_binary_operator_name(operator: BinaryOperator) -> IndexForm {
        match operator {
            BinaryOperator::Add => IndexForm::Add,
            BinaryOperator::And => IndexForm::And,
            BinaryOperator::BitwiseAnd => IndexForm::BitwiseAnd,
            BinaryOperator::BitwiseOr => IndexForm::BitwiseOr,
            BinaryOperator::BitwiseXor => IndexForm::BitwiseXor,
            BinaryOperator::Divide => IndexForm::Divide,
            BinaryOperator::Equal => IndexForm::Equal,
            BinaryOperator::Greater => IndexForm::Greater,
            BinaryOperator::GreaterOrEqual => IndexForm::GreaterOrEqual,
            BinaryOperator::Less => IndexForm::Less,
            BinaryOperator::LessOrEqual => IndexForm::LessOrEqual,
            BinaryOperator::Modulo => IndexForm::Modulo,
            BinaryOperator::Multiply => IndexForm::Multiply,
            BinaryOperator::NotEqual => IndexForm::NotEqual,
            BinaryOperator::Or => IndexForm::Or,
            BinaryOperator::ShiftLeft => IndexForm::ShiftLeft,
            BinaryOperator::ShiftRight => IndexForm::ShiftRight,
            BinaryOperator::Subtract => IndexForm::Subtract,
            BinaryOperator::CaseMembership => IndexForm::CaseMembership,
        }
    }
}
#[cfg(test)]
pub(crate) mod package_policy_reading_tests {
    pub(crate) mod fixture {
        use crate::TypedTrees;
        use crate::data::{DataDefinition, TypeParameter, TypeParameterKind};
        use crate::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, NamePath, StaticMachineArgument,
            TableBinaryExpression, TableCallExpression,
        };
        use crate::name::Identifier;
        use crate::type_identity::ExactOwnerTypeIdentityRequest;
        use crate::types::{
            FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };
        use crate::{AccessFieldPlan, AccessViewPlan, AccessorTarget};
        use access_plans::{
            AccessFields, AccessPlan, BoundaryReach, FieldAccess, validate_access_plan,
        };
        use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};
        use semantic_vocabulary::PackageKeyIdentity;
        use source::{SourceId, SourceMap, SourceOrigin, SourceSpan, Span};
        use std::path::PathBuf;
        use std::sync::Arc;
        use symbols::{
            BuiltinTypeAtom, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder,
        };

        const DECLARATIONS: &[(u8, SymbolKind, &str)] = &[
            (1, SymbolKind::Data, "Payload"),
            (1, SymbolKind::Data, "Container"),
            (2, SymbolKind::Data, "Record"),
            (2, SymbolKind::Data, "Count"),
            (2, SymbolKind::Const, "LIMIT"),
            (2, SymbolKind::Field, "left"),
            (3, SymbolKind::Trait, "Hooks"),
            (3, SymbolKind::Machine, "size"),
            (3, SymbolKind::Const, "WIDTH"),
            (3, SymbolKind::Field, "right"),
            (4, SymbolKind::Machine, "Selected"),
            (4, SymbolKind::Domain, "Aligned"),
            (4, SymbolKind::Field, "spare"),
            (0, SymbolKind::Data, "Source"),
            (1, SymbolKind::Data, "Window"),
            (2, SymbolKind::Data, "Policy"),
            (3, SymbolKind::Data, "Schema"),
            (1, SymbolKind::Data, "Placed"),
            (1, SymbolKind::Data, "Accessor"),
            (1, SymbolKind::Data, "Cursor"),
            (3, SymbolKind::Machine, "read_machine"),
            (3, SymbolKind::State, "read_state"),
        ];

        const PARAMETERS: [&str; 3] = ["T", "N", "V"];

        const TOOLCHAIN_DIGEST: [u8; 32] = [9; 32];

        pub(crate) struct Fixture {
            pub(crate) program: TypedTrees,
            toolchain: [(SourceId, [u8; 32]); 1],
            symbols: Vec<(&'static str, SymbolHandle)>,
            pub(crate) u64_type: TypeReferenceHandle,
        }

        impl Fixture {
            pub(crate) fn new() -> Self {
                let names = DECLARATIONS
                    .iter()
                    .map(|(_, _, name)| *name)
                    .chain(PARAMETERS)
                    .collect::<Vec<_>>();
                let mut text = String::new();
                let mut spans = Vec::new();
                for name in &names {
                    let start = text.len();
                    text.push_str(name);
                    spans.push((*name, Span::new(start, text.len())));
                    text.push(' ');
                }
                let mut sources = SourceMap::default();
                let package_sources = [1u8, 2, 3, 4].map(|package| {
                    let file = sources.add_with_metadata(
                        PathBuf::from(format!("package{package}.omg")),
                        text.clone(),
                        PathBuf::from("package"),
                        PackageKeyIdentity::from_digest([package; 32]),
                        SourceOrigin::User,
                    );
                    (package, file.source_id)
                });
                let toolchain_source = sources
                    .add_with_metadata(
                        PathBuf::from("toolchain.omg"),
                        text.clone(),
                        PathBuf::from("toolchain"),
                        None,
                        SourceOrigin::Toolchain,
                    )
                    .source_id;
                let span_of = |name: &str, package: u8| {
                    let source = if package == 0 {
                        toolchain_source
                    } else {
                        package_sources
                            .iter()
                            .find(|(candidate, _)| *candidate == package)
                            .expect("a declared package has a source")
                            .1
                    };
                    let span = spans
                        .iter()
                        .find(|(candidate, _)| *candidate == name)
                        .expect("every name is in the source text")
                        .1;
                    SymbolNameRef::Source(SourceSpan::new(source, span))
                };

                let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                let mut children = SymbolTableBuilder::child_handles(
                    builder.insert_children(
                        root,
                        BuiltinTypeAtom::ALL
                            .iter()
                            .map(|atom| {
                                (
                                    SymbolKind::BuiltinType,
                                    SymbolNameRef::Static(atom.symbol_name()),
                                )
                            })
                            .chain(
                                DECLARATIONS
                                    .iter()
                                    .map(|(package, kind, name)| (*kind, span_of(name, *package))),
                            ),
                    ),
                )
                .collect::<Vec<_>>();
                let declared = children.split_off(BuiltinTypeAtom::ALL.len());
                let u64_symbol = children[BuiltinTypeAtom::U64.ordinal()];
                let mut symbols = DECLARATIONS
                    .iter()
                    .map(|(_, _, name)| *name)
                    .zip(declared)
                    .collect::<Vec<_>>();
                let container = symbols
                    .iter()
                    .find(|(name, _)| *name == "Container")
                    .expect("Container is declared")
                    .1;
                let parameters = SymbolTableBuilder::child_handles(
                    builder.insert_children(
                        container,
                        PARAMETERS
                            .iter()
                            .map(|name| (SymbolKind::TypeParameter, span_of(name, 1))),
                    ),
                )
                .collect::<Vec<_>>();
                symbols.extend(PARAMETERS.iter().copied().zip(parameters));

                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..TypedTrees::default()
                };
                let u64_type = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: u64_symbol,
                        name: Identifier::generated("u64"),
                    });
                let kinds = [
                    TypeParameterKind::Type,
                    TypeParameterKind::Const {
                        type_reference: u64_type,
                    },
                    TypeParameterKind::Value {
                        type_reference: u64_type,
                    },
                ];
                let type_parameters =
                    program
                        .data_type_parameters
                        .insert_many(PARAMETERS.iter().zip(&kinds).map(|(name, kind)| {
                            TypeParameter {
                                symbol: symbols
                                    .iter()
                                    .find(|(candidate, _)| candidate == name)
                                    .expect("a parameter is declared")
                                    .1,
                                name: Identifier::generated(*name),
                                kind: kind.clone(),
                                ..TypeParameter::default()
                            }
                        }));
                program.push_data_definition(DataDefinition {
                    symbol: container,
                    name: Identifier::generated("Container"),
                    type_parameters,
                    ..DataDefinition::default()
                });
                let toolchain = [(toolchain_source, TOOLCHAIN_DIGEST)];
                let mut fixture = Self {
                    program,
                    toolchain,
                    symbols,
                    u64_type,
                };
                fixture.install_placed_view();
                fixture
            }

            fn install_placed_view(&mut self) {
                let (policy, schema) = (self.named("Policy"), self.named("Schema"));
                let origin = self.generic("Window", vec![policy, schema]);
                let entry = |field: &str, offset| LayoutFieldEntryReport {
                    field: field.into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset },
                };
                let layout = LayoutPlanReport {
                    schema_report_fingerprint: 1,
                    entries: vec![entry("words", 0), entry("flags", 8)],
                    offsets: None,
                    size: Some(16),
                    align: 8,
                };
                let access = AccessFields::inaccessible(&layout).expect("schema slots");
                let keys = [
                    access.key_at(0).expect("first field"),
                    access.key_at(1).expect("second field"),
                ];
                let placement = validate_access_plan(AccessPlan {
                    layout,
                    access,
                    reach: BoundaryReach::default(),
                })
                .expect("an inaccessible placement validates");
                let field =
                    |key, name: &str, member_identity, accessor: &str, targets| AccessFieldPlan {
                        access_key: key,
                        view_field_symbol: SymbolHandle::invalid(),
                        field_name: name.to_owned(),
                        member_identity,
                        field_symbol: SymbolHandle::invalid(),
                        accessor_name: accessor.to_owned(),
                        accessor_type: self.u64_type,
                        accessor_data_symbol: self.symbol(accessor),
                        accessor_targets: targets,
                        value_type: self.u64_type,
                        access: FieldAccess::Inaccessible,
                        atomic_resident: None,
                    };
                let read = AccessorTarget {
                    operation: "read".to_owned(),
                    machine_symbol: self.symbol("read_machine"),
                    state_symbol: self.symbol("read_state"),
                };
                let fields = vec![
                    field(keys[0], "words", Some(7), "Accessor", vec![read]),
                    field(keys[1], "flags", None, "Cursor", Vec::new()),
                ];
                self.program.access_view_plans.push(AccessViewPlan {
                    template_symbol: self.symbol("Window"),
                    data_name: "Placed".to_owned(),
                    data_symbol: self.symbol("Placed"),
                    policy_name: "Policy".to_owned(),
                    policy_symbol: self.symbol("Policy"),
                    policy_plan_machine_symbol: SymbolHandle::invalid(),
                    schema_name: "Schema".to_owned(),
                    schema_symbol: self.symbol("Schema"),
                    placement,
                    fields,
                });
                self.program.push_data_definition(DataDefinition {
                    symbol: self.symbol("Placed"),
                    name: Identifier::generated("Placed"),
                    generic_instance: Some(origin),
                    ..DataDefinition::default()
                });
            }

            pub(crate) fn symbol(&self, name: &str) -> SymbolHandle {
                self.symbols
                    .iter()
                    .find(|(candidate, _)| *candidate == name)
                    .unwrap_or_else(|| panic!("`{name}` is not declared"))
                    .1
            }

            pub(crate) fn type_node(&mut self, node: TypeReferenceNode) -> TypeReferenceHandle {
                self.program.type_reference_table.insert(node)
            }

            pub(crate) fn named(&mut self, name: &str) -> TypeReferenceHandle {
                let symbol = self.symbol(name);
                self.type_node(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
            }

            pub(crate) fn spelled(&mut self, spelling: &str) -> TypeReferenceHandle {
                self.type_node(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::generated(spelling),
                })
            }

            pub(crate) fn generic(
                &mut self,
                base: &str,
                arguments: Vec<TypeReferenceHandle>,
            ) -> TypeReferenceHandle {
                let base_symbol = self.symbol(base);
                let arguments = self
                    .program
                    .type_reference_table
                    .insert_type_reference_handles(arguments);
                self.type_node(TypeReferenceNode::Generic {
                    base_symbol,
                    base_name: Identifier::generated(base),
                    lifetime_arguments: Vec::new(),
                    arguments,
                })
            }

            pub(crate) fn constrained(
                &mut self,
                constraints: Vec<TypeConstraintNode>,
            ) -> TypeReferenceHandle {
                let constraints = self
                    .program
                    .type_reference_table
                    .insert_constraints(constraints);
                self.type_node(TypeReferenceNode::Constrained {
                    base_type: self.u64_type,
                    constraints,
                })
            }

            pub(crate) fn array(&mut self, length: FixedArrayLength) -> TypeReferenceHandle {
                self.type_node(TypeReferenceNode::FixedArray {
                    element_type: self.u64_type,
                    length,
                })
            }

            pub(crate) fn index(&mut self, expression: ExpressionHandle) -> TypeReferenceHandle {
                self.type_node(TypeReferenceNode::ConstExpression(expression))
            }

            pub(crate) fn expression(&mut self, node: ExpressionNode) -> ExpressionHandle {
                self.program.expression_table.insert(node)
            }

            pub(crate) fn integer(&mut self, value: i64) -> ExpressionHandle {
                self.expression(ExpressionNode::Integer(
                    IntegerLiteral::from_value(value).with_landing(IntegerLanding {
                        landed_type: LandedIntegerType::U64,
                        domain: ArithmeticDomain::Exact,
                    }),
                ))
            }

            pub(crate) fn name(&mut self, name: &str) -> ExpressionHandle {
                let symbol = self.symbol(name);
                self.program
                    .expression_table
                    .insert_tree(&crate::expression::Expression::Name(NamePath::resolved(
                        vec![Identifier::generated(name)],
                        symbol,
                        symbol,
                    )))
            }

            pub(crate) fn binary(
                &mut self,
                left: ExpressionHandle,
                operator: BinaryOperator,
                right: ExpressionHandle,
            ) -> ExpressionHandle {
                self.expression(ExpressionNode::Binary(TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    left,
                    operator,
                    right,
                }))
            }

            pub(crate) fn call(
                &mut self,
                receiver: ExpressionHandle,
                arguments: Vec<ExpressionHandle>,
                machine_arguments: Vec<StaticMachineArgument>,
            ) -> ExpressionHandle {
                self.call_to("size", receiver, arguments, machine_arguments)
            }

            pub(crate) fn call_to(
                &mut self,
                target: &str,
                receiver: ExpressionHandle,
                arguments: Vec<ExpressionHandle>,
                machine_arguments: Vec<StaticMachineArgument>,
            ) -> ExpressionHandle {
                let arguments = self
                    .program
                    .expression_table
                    .insert_expression_handles(arguments);
                self.expression(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    build_selection: Default::default(),
                    authored_machine_application: Default::default(),
                    exact_coercion_target: Default::default(),
                    requirement_application: Default::default(),
                    receiver,
                    target_symbol: self.symbol(target),
                    static_machine_parameter: Default::default(),
                    target: Identifier::generated(target),
                    static_requirement_dispatch: None,
                    machine_arguments: machine_arguments.into_boxed_slice(),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    arguments,
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }))
            }

            pub(crate) fn render(
                &self,
                reference: TypeReferenceHandle,
                binders: &[(SymbolHandle, String)],
                substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            ) -> String {
                let identity = self
                    .program
                    .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                        type_reference: reference,
                        binders,
                        substitutions,
                        exact_toolchain_sources: &self.toolchain,
                    })
                    .expect("every nominal of a witness has an exact owner");
                self.program
                    .type_identity_text(identity)
                    .expect("an interned identity has text")
            }
        }
    }
    pub(crate) mod kinds {
        use crate::expression::{BinaryOperator, ExpressionNode, MatchFieldAction, MatchPattern};
        use crate::types::{
            DomainConstraintSubject, FixedArrayLength, TypeConstraintNode, TypeReferenceNode,
        };

        macro_rules! node_kinds {
    ($kinds:ident, $kind_of:ident for $node:ty { $($kind:ident => $pattern:pat,)+ }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum $kinds {
            $($kind,)+
        }

        impl $kinds {
            pub(crate) const ALL: &'static [Self] = &[$(Self::$kind,)+];
        }

        pub(crate) fn $kind_of(node: &$node) -> $kinds {
            match node {
                $($pattern => $kinds::$kind,)+
            }
        }
    };
}

        node_kinds!(TypeKind, type_kind for TypeReferenceNode {
            Reference => TypeReferenceNode::Reference { .. },
            Constrained => TypeReferenceNode::Constrained { .. },
            FixedArray => TypeReferenceNode::FixedArray { .. },
            Slice => TypeReferenceNode::Slice { .. },
            Generic => TypeReferenceNode::Generic { .. },
            ConstExpression => TypeReferenceNode::ConstExpression(_),
            DynamicTrait => TypeReferenceNode::DynamicTrait { .. },
            Named => TypeReferenceNode::Named { .. },
            Unit => TypeReferenceNode::Unit,
        });

        node_kinds!(ConstraintKind, constraint_kind for TypeConstraintNode {
            Named => TypeConstraintNode::Named(_),
            ArithmeticDomain => TypeConstraintNode::ArithmeticDomain(_),
            Domain => TypeConstraintNode::Domain(_),
        });

        node_kinds!(DomainKind, domain_kind for DomainConstraintSubject {
            Declared => DomainConstraintSubject::Declared,
            Carry => DomainConstraintSubject::Carry(_),
            Value => DomainConstraintSubject::Value(_),
            OmegaLayout => DomainConstraintSubject::OmegaLayout { .. },
        });

        node_kinds!(LengthKind, length_kind for FixedArrayLength {
            Literal => FixedArrayLength::Literal(_),
            Expression => FixedArrayLength::Expression(_),
            ConstParameter => FixedArrayLength::ConstParameter { .. },
            ConstCall => FixedArrayLength::ConstCall { .. },
        });

        node_kinds!(ExpressionKind, expression_kind for ExpressionNode {
            TypeExpression => ExpressionNode::TypeExpression(_),
            Match => ExpressionNode::Match(_),
            SavedSubject => ExpressionNode::SavedSubject(_),
            ArrayLiteral => ExpressionNode::ArrayLiteral(_),
            Atomic => ExpressionNode::Atomic(_),
            Binary => ExpressionNode::Binary(_),
            Boolean => ExpressionNode::Boolean(_),
            Cast => ExpressionNode::Cast(_),
            Call => ExpressionNode::Call(_),
            Float => ExpressionNode::Float(_),
            UndeterminedNan => ExpressionNode::UndeterminedNaN(_),
            Indexed => ExpressionNode::Indexed(_),
            Integer => ExpressionNode::Integer(_),
            Member => ExpressionNode::Member(_),
            Borrow => ExpressionNode::Borrow(_),
            Name => ExpressionNode::Name(_),
            Range => ExpressionNode::Range(_),
            StructLiteral => ExpressionNode::StructLiteral(_),
            String => ExpressionNode::String(_),
            Unary => ExpressionNode::Unary(_),
            ZeroValue => ExpressionNode::ZeroValue(_),
        });

        node_kinds!(OperatorKind, operator_kind for BinaryOperator {
            Add => BinaryOperator::Add,
            And => BinaryOperator::And,
            BitwiseAnd => BinaryOperator::BitwiseAnd,
            BitwiseOr => BinaryOperator::BitwiseOr,
            BitwiseXor => BinaryOperator::BitwiseXor,
            Divide => BinaryOperator::Divide,
            Equal => BinaryOperator::Equal,
            Greater => BinaryOperator::Greater,
            GreaterOrEqual => BinaryOperator::GreaterOrEqual,
            Less => BinaryOperator::Less,
            LessOrEqual => BinaryOperator::LessOrEqual,
            Modulo => BinaryOperator::Modulo,
            Multiply => BinaryOperator::Multiply,
            NotEqual => BinaryOperator::NotEqual,
            Or => BinaryOperator::Or,
            ShiftLeft => BinaryOperator::ShiftLeft,
            ShiftRight => BinaryOperator::ShiftRight,
            Subtract => BinaryOperator::Subtract,
            CaseMembership => BinaryOperator::CaseMembership,
        });

        node_kinds!(PatternKind, pattern_kind for MatchPattern {
            Value => MatchPattern::Value(_),
            Destructure => MatchPattern::Destructure(_),
            Wildcard => MatchPattern::Wildcard,
        });

        node_kinds!(ActionKind, action_kind for MatchFieldAction {
            Bind => MatchFieldAction::Bind(_),
            Equal => MatchFieldAction::Equal(_),
            Waive => MatchFieldAction::Waive,
        });
    }
    pub(crate) mod witnesses {

        use super::fixture::Fixture;
        use crate::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern,
            NamePath, StaticMachineArgument, StaticSymbolApplication, TableAtomicExpression,
            TableBorrowExpression, TableDestructurePattern, TableIndexedExpression, TableMatchArm,
            TableMatchExpression, TableMatchField, TableMemberExpression, TableRangeExpression,
            TableSavedSubject, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
            UnaryOperator,
        };
        use crate::name::Identifier;
        use crate::statement::OutcomeProofSelector;
        use crate::types::{
            DomainConstraint, DomainConstraintSubject, FixedArrayLength, OmegaLayoutGrammar,
            TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };
        use language_core::atomic::{
            AtomicExpressionResultCustody, AtomicOrderingPlan, MemoryOrdering,
        };
        use language_semantics::CarryPermission;
        use language_semantics::const_value::CanonicalConstValue;
        use language_semantics::value_domain::ValueDomain;
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral};
        use source::SourceSpan;
        use symbols::SymbolHandle;

        pub(crate) struct Witness {
            pub(crate) form: &'static str,
            pub(crate) reference: TypeReferenceHandle,
            pub(crate) binders: Vec<(SymbolHandle, String)>,
            pub(crate) substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
            pub(crate) owners: &'static [u8],
        }

        pub(crate) fn witness(
            form: &'static str,
            reference: TypeReferenceHandle,
            owners: &'static [u8],
        ) -> Witness {
            Witness {
                form,
                reference,
                binders: Vec::new(),
                substitutions: Vec::new(),
                owners,
            }
        }

        pub(crate) fn type_witnesses(fixture: &mut Fixture) -> Vec<Witness> {
            let n = fixture.symbol("N");
            let mut witnesses = Vec::new();

            witnesses.push(witness(
                "unit",
                fixture.type_node(TypeReferenceNode::Unit),
                &[],
            ));
            witnesses.push(witness("builtin", fixture.u64_type, &[]));
            let payload = fixture.named("Payload");
            witnesses.push(witness("nominal", payload, &[1]));
            let source = fixture.named("Source");
            witnesses.push(witness("toolchain nominal", source, &[]));
            for access in [
                language_core::ReferenceAccess::Shared,
                language_core::ReferenceAccess::Mutable,
                language_core::ReferenceAccess::WriteOnly,
            ] {
                let reference = fixture.type_node(TypeReferenceNode::Reference {
                    referee: payload,
                    access,
                    lifetime: None,
                });
                witnesses.push(witness("reference", reference, &[1]));
            }
            let record = fixture.named("Record");
            let slice = fixture.type_node(TypeReferenceNode::Slice {
                element_type: record,
            });
            witnesses.push(witness("slice", slice, &[2]));

            witnesses.push(witness(
                "array of a literal extent",
                fixture.array(FixedArrayLength::Literal(4)),
                &[],
            ));
            let bound = fixture.array(FixedArrayLength::ConstParameter {
                symbol: n,
                name: Identifier::generated("N"),
            });
            witnesses.push(Witness {
                binders: vec![(n, "type-parameter:1".to_owned())],
                ..witness("array of a bound extent", bound, &[])
            });
            let five = fixture.spelled("5");
            witnesses.push(Witness {
                substitutions: vec![(n, five)],
                ..witness("array of a substituted extent", bound, &[])
            });
            let width = fixture.symbol("WIDTH");
            witnesses.push(witness(
                "array of a declared extent",
                fixture.array(FixedArrayLength::ConstParameter {
                    symbol: width,
                    name: Identifier::generated("WIDTH"),
                }),
                &[3],
            ));
            let sum = {
                let width = fixture.name("WIDTH");
                let one = fixture.integer(1);
                fixture.binary(width, BinaryOperator::Add, one)
            };
            let extent = fixture.index(sum);
            witnesses.push(witness(
                "array of an expression extent",
                fixture.array(FixedArrayLength::Expression(extent)),
                &[3],
            ));
            witnesses.push(witness(
                "array of a call extent",
                fixture.array(FixedArrayLength::ConstCall {
                    machine: fixture.symbol("size"),
                    name: Identifier::generated("size"),
                    source_span: SourceSpan::default(),
                }),
                &[3],
            ));

            let limit = fixture.named("LIMIT");
            let applied = fixture.generic("Container", vec![record, five, limit]);
            witnesses.push(witness(
                "generic with type and const operands",
                applied,
                &[1, 2],
            ));
            let open = {
                let width = fixture.name("WIDTH");
                let one = fixture.integer(1);
                let sum = fixture.binary(width, BinaryOperator::Add, one);
                fixture.index(sum)
            };
            let two = {
                let two = fixture.integer(2);
                fixture.index(two)
            };
            let computed = fixture.generic("Container", vec![payload, open, two]);
            witnesses.push(witness("generic with computed operands", computed, &[1, 3]));
            let canonical = fixture.spelled(&CanonicalConstValue::boolean(true).atom());
            let constant = fixture.generic("Container", vec![payload, canonical, five]);
            witnesses.push(witness("generic with a canonical constant", constant, &[1]));
            let lifetime_only = fixture.generic("Container", Vec::new());
            witnesses.push(witness("generic without operands", lifetime_only, &[1]));

            let hooks = {
                let two = fixture.integer(7);
                let seven = fixture.index(two);
                let arguments = fixture
                    .program
                    .type_reference_table
                    .insert_type_reference_handles(vec![payload, five, seven]);
                fixture.type_node(TypeReferenceNode::DynamicTrait {
                    symbol: fixture.symbol("Hooks"),
                    name: Identifier::generated("Hooks"),
                    arguments,
                })
            };
            witnesses.push(witness("dynamic trait", hooks, &[1, 3]));
            witnesses.push(witness("integer constant", five, &[]));
            witnesses.push(witness("canonical constant", canonical, &[]));

            witnesses
        }

        pub(crate) fn constraint_witnesses(fixture: &mut Fixture) -> Vec<Witness> {
            let payload = fixture.named("Payload");
            let eight = fixture.spelled("8");
            let domain = |subject, symbol: SymbolHandle, name: &str, arguments| DomainConstraint {
                name: Identifier::generated(name),
                arguments,
                subject,
                symbol,
                ..DomainConstraint::default()
            };
            let aligned = fixture.symbol("Aligned");
            let mut witnesses = Vec::new();
            fn add(
                witnesses: &mut Vec<Witness>,
                fixture: &mut Fixture,
                form: &'static str,
                constraints: Vec<TypeConstraintNode>,
                owners: &'static [u8],
            ) {
                let reference = fixture.constrained(constraints);
                witnesses.push(witness(form, reference, owners));
            }

            add(
                &mut witnesses,
                fixture,
                "named constraint",
                vec![TypeConstraintNode::Named(Identifier::generated("Even"))],
                &[],
            );
            add(
                &mut witnesses,
                fixture,
                "arithmetic domain",
                vec![TypeConstraintNode::ArithmeticDomain(
                    ArithmeticDomain::Wrapping,
                )],
                &[],
            );
            add(
                &mut witnesses,
                fixture,
                "declared domain",
                vec![TypeConstraintNode::Domain(domain(
                    DomainConstraintSubject::Declared,
                    aligned,
                    "Aligned",
                    Vec::new(),
                ))],
                &[4],
            );
            add(
                &mut witnesses,
                fixture,
                "declared domain with operands",
                vec![TypeConstraintNode::Domain(domain(
                    DomainConstraintSubject::Declared,
                    aligned,
                    "Aligned",
                    vec![payload, eight],
                ))],
                &[1, 4],
            );
            for permission in CarryPermission::ALL {
                add(
                    &mut witnesses,
                    fixture,
                    "carry domain",
                    vec![TypeConstraintNode::Domain(domain(
                        DomainConstraintSubject::Carry(permission),
                        SymbolHandle::invalid(),
                        "carry",
                        Vec::new(),
                    ))],
                    &[],
                );
            }
            add(
                &mut witnesses,
                fixture,
                "value domain",
                vec![TypeConstraintNode::Domain(domain(
                    DomainConstraintSubject::Value(ValueDomain::Finite),
                    SymbolHandle::invalid(),
                    "finite",
                    Vec::new(),
                ))],
                &[],
            );
            add(
                &mut witnesses,
                fixture,
                "layout domain",
                vec![TypeConstraintNode::Domain(domain(
                    DomainConstraintSubject::OmegaLayout {
                        grammar: OmegaLayoutGrammar::Derived,
                    },
                    SymbolHandle::invalid(),
                    "layout",
                    vec![payload],
                ))],
                &[1],
            );
            add(
                &mut witnesses,
                fixture,
                "every kind of constraint at once",
                vec![
                    TypeConstraintNode::Named(Identifier::generated("Even")),
                    TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Saturating),
                    TypeConstraintNode::Domain(domain(
                        DomainConstraintSubject::Declared,
                        aligned,
                        "Aligned",
                        vec![payload],
                    )),
                ],
                &[1, 4],
            );
            witnesses
        }

        pub(crate) fn expression_witnesses(fixture: &mut Fixture) -> Vec<Witness> {
            let mut witnesses = Vec::new();
            let mut add = |fixture: &mut Fixture,
                           form: &'static str,
                           expression: ExpressionHandle,
                           owners: &'static [u8]| {
                let reference = fixture.index(expression);
                witnesses.push(witness(form, reference, owners));
            };

            let name = fixture.name("LIMIT");
            add(fixture, "declared name", name, &[2]);
            let value = fixture.integer(7);
            add(fixture, "integer", value, &[]);
            let spelled =
                fixture
                    .program
                    .expression_table
                    .insert_tree(&crate::expression::Expression::Name(NamePath::unresolved(
                        vec![Identifier::generated("true")],
                    )));
            add(fixture, "spelled Boolean name", spelled, &[]);
            let boolean = fixture.expression(ExpressionNode::Boolean(true));
            add(fixture, "Boolean", boolean, &[]);
            let float = fixture.expression(ExpressionNode::Float(FloatLiteral::new(1.5)));
            add(fixture, "float", float, &[]);
            let nan = fixture.expression(ExpressionNode::UndeterminedNaN(FloatFormat::F64));
            add(fixture, "undetermined NaN", nan, &[]);
            let string = fixture.expression(ExpressionNode::String(source::SourceBytes::from(
                &b"bytes"[..],
            )));
            add(fixture, "string", string, &[]);
            for operator in [UnaryOperator::BitwiseNot, UnaryOperator::LogicalNot] {
                let operand = fixture.name("WIDTH");
                let unary = fixture.expression(ExpressionNode::Unary(TableUnaryExpression {
                    operator,
                    operand,
                }));
                add(fixture, "unary", unary, &[3]);
            }
            for operator in [
                BinaryOperator::Add,
                BinaryOperator::And,
                BinaryOperator::BitwiseAnd,
                BinaryOperator::BitwiseOr,
                BinaryOperator::BitwiseXor,
                BinaryOperator::Divide,
                BinaryOperator::Equal,
                BinaryOperator::Greater,
                BinaryOperator::GreaterOrEqual,
                BinaryOperator::Less,
                BinaryOperator::LessOrEqual,
                BinaryOperator::Modulo,
                BinaryOperator::Multiply,
                BinaryOperator::NotEqual,
                BinaryOperator::Or,
                BinaryOperator::ShiftLeft,
                BinaryOperator::ShiftRight,
                BinaryOperator::Subtract,
                BinaryOperator::CaseMembership,
            ] {
                let (left, right) = (fixture.integer(1), fixture.name("LIMIT"));
                let binary = fixture.binary(left, operator, right);
                add(fixture, "binary", binary, &[2]);
            }

            let (left, right) = (fixture.name("LIMIT"), fixture.name("WIDTH"));
            let licensed = fixture.binary(left, BinaryOperator::Add, right);
            let selection = crate::expression::OpenIndexOperationSelection {
                expression: licensed,
                spelling: language_core::operator_spelling::OperatorSpelling::Add,
                operator: fixture.symbol("size"),
                operand_type: fixture.u64_type,
                provider: fixture.symbol("Selected"),
                application: None,
                algebra_trait: fixture.symbol("Hooks"),
                algebra_requirement: "add".to_owned(),
                algebra_alias: None,
                commutativity_licensed: true,
                associativity_licensed: true,
                distributes_over: Vec::new(),
                left_identity_literal: String::new(),
                right_identity_literal: String::new(),
            };
            fixture
                .program
                .expression_table
                .index_expression_authority_mut(licensed)
                .operation = Some(selection);
            add(
                fixture,
                "binary with a selected algebra",
                licensed,
                &[2, 3, 4],
            );

            let target = fixture.named("Count");
            let cast = {
                let value = crate::expression::Expression::Integer(IntegerLiteral::from_value(3));
                fixture
                    .program
                    .expression_table
                    .insert_tree(&crate::expression::Expression::Cast(Box::new(
                        crate::expression::CastExpression {
                            value,
                            target_type: target,
                            target_label: NamePath::unresolved(Vec::new()),
                            domain: ArithmeticDomain::Wrapping,
                            form: language_core::cast_form::CastForm::Value,
                        },
                    )))
            };
            add(fixture, "cast", cast, &[2]);

            let (receiver, argument) = (fixture.name("LIMIT"), fixture.integer(3));
            let plain = fixture.call(receiver, vec![argument], Vec::new());
            add(fixture, "call", plain, &[2, 3]);
            let selected = StaticMachineArgument {
                symbol: fixture.symbol("Selected"),
                application: Some(Box::new(StaticSymbolApplication {
                    member: arena::Handle::invalid(),
                    lifetime_arguments: Box::new([Identifier::generated("a")]),
                    arguments: Box::new([StaticMachineArgument {
                        const_literal: Some(IntegerLiteral::from_value(7)),
                        ..StaticMachineArgument::default()
                    }]),
                })),
                ..StaticMachineArgument::default()
            };
            let operand = fixture.named("Payload");
            let applied = fixture.call(
                ExpressionHandle::invalid(),
                Vec::new(),
                vec![
                    selected,
                    StaticMachineArgument {
                        type_reference: operand,
                        ..StaticMachineArgument::default()
                    },
                ],
            );
            add(fixture, "call with static operands", applied, &[1, 3, 4]);

            let (collection, index) = (fixture.name("LIMIT"), fixture.integer(0));
            let indexed = fixture.expression(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));
            add(fixture, "indexed", indexed, &[2]);
            let receiver = fixture.name("LIMIT");
            let member = fixture.expression(ExpressionNode::Member(TableMemberExpression {
                receiver,
                member_symbol: SymbolHandle::invalid(),
                member: Identifier::generated("tag"),
                case_variant: Some(Identifier::generated("On")),
            }));
            add(fixture, "member", member, &[2]);
            for access in [
                language_core::ReferenceAccess::Shared,
                language_core::ReferenceAccess::Mutable,
                language_core::ReferenceAccess::WriteOnly,
            ] {
                let target = fixture.name("LIMIT");
                let borrow = fixture.expression(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access,
                }));
                add(fixture, "borrow", borrow, &[2]);
            }
            for end_inclusive in [true, false] {
                let (start, end) = (fixture.integer(0), fixture.name("WIDTH"));
                let range = fixture.expression(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive,
                }));
                add(fixture, "range", range, &[3]);
            }
            let (first, second) = (fixture.integer(1), fixture.name("LIMIT"));
            let elements = fixture
                .program
                .expression_table
                .insert_expression_handles([first, second]);
            let array = fixture.expression(ExpressionNode::ArrayLiteral(elements));
            add(fixture, "array literal", array, &[2]);
            let empty = fixture
                .program
                .expression_table
                .insert_expression_handles([]);
            let empty = fixture.expression(ExpressionNode::ArrayLiteral(empty));
            add(fixture, "empty array literal", empty, &[]);
            let value = fixture.name("WIDTH");
            let fields =
                fixture
                    .program
                    .expression_table
                    .insert_struct_fields([TableStructLiteralField {
                        name: Identifier::generated("width"),
                        field_symbol: SymbolHandle::invalid(),
                        value,
                    }]);
            let literal = fixture.expression(ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: Default::default(),
                type_name: Identifier::generated("Record"),
                type_symbol: fixture.symbol("Record"),
                case_name: Some(Identifier::generated("On")),
                case_symbol: None,
                fields,
            }));
            add(fixture, "struct literal", literal, &[2, 3]);
            let operand = fixture.named("Payload");
            let type_expression = fixture.expression(ExpressionNode::TypeExpression(operand));
            add(fixture, "type expression", type_expression, &[1]);
            let zero = fixture.expression(ExpressionNode::ZeroValue(operand));
            add(fixture, "zero value", zero, &[1]);
            let (value, result) = (fixture.name("LIMIT"), fixture.name("WIDTH"));
            let atomic = fixture.expression(ExpressionNode::Atomic(TableAtomicExpression {
                value,
                result,
                ordering: AtomicOrderingPlan::Load(MemoryOrdering::Receive),
                result_custody: AtomicExpressionResultCustody::Scalar,
            }));
            add(fixture, "atomic", atomic, &[2, 3]);

            for (form, name) in [
                ("derived view", "Placed"),
                ("derived accessor by member identity", "Accessor"),
                ("derived accessor by field name", "Cursor"),
                ("derived operation state", "read_state"),
            ] {
                let derived = fixture.name(name);
                add(fixture, form, derived, &[1, 2, 3]);
            }
            let receiver = fixture.name("LIMIT");
            let operation = fixture.call_to("read_machine", receiver, Vec::new(), Vec::new());
            add(fixture, "derived operation machine", operation, &[1, 2, 3]);

            let (subject, wanted, chosen, otherwise) = (
                fixture.name("LIMIT"),
                fixture.integer(0),
                fixture.name("WIDTH"),
                fixture.integer(1),
            );
            let arms = fixture.program.expression_table.insert_match_arms([
                TableMatchArm {
                    pattern: MatchPattern::Value(wanted),
                    value: chosen,
                    source_span: SourceSpan::default(),
                },
                TableMatchArm {
                    pattern: MatchPattern::Wildcard,
                    value: otherwise,
                    source_span: SourceSpan::default(),
                },
            ]);
            let dispatch = fixture.expression(ExpressionNode::Match(TableMatchExpression {
                subject,
                arms,
                ..TableMatchExpression::default()
            }));
            add(fixture, "match on values", dispatch, &[2, 3]);

            let subject = fixture.name("LIMIT");
            let capture = fixture
                .program
                .expression_table
                .insert_match_subject(subject);
            let head = fixture.name("Record");
            let saved = fixture.expression(ExpressionNode::SavedSubject(TableSavedSubject {
                component: Default::default(),
                capture,
                pattern_head: head,
            }));
            let wanted = fixture.name("WIDTH");
            let guard = fixture.name("Count");
            let (left, right, spare) = (
                fixture.symbol("left"),
                fixture.symbol("right"),
                fixture.symbol("spare"),
            );
            let fields = fixture.program.expression_table.insert_match_fields([
                TableMatchField {
                    component: Default::default(),
                    member: Identifier::generated("left"),
                    field_symbol: left,
                    action: MatchFieldAction::Bind(Identifier::generated("bound")),
                },
                TableMatchField {
                    component: Default::default(),
                    member: Identifier::generated("right"),
                    field_symbol: right,
                    action: MatchFieldAction::Equal(wanted),
                },
                TableMatchField {
                    component: Default::default(),
                    member: Identifier::generated("spare"),
                    field_symbol: spare,
                    action: MatchFieldAction::Waive,
                },
            ]);
            let selectors = fixture
                .program
                .expression_table
                .insert_match_proof_selectors([OutcomeProofSelector {
                    output_field: Identifier::generated("left"),
                    binding: Identifier::generated("bound"),
                }]);
            let unguarded = fixture.program.expression_table.insert_match_arms([
                TableMatchArm {
                    pattern: MatchPattern::Destructure(TableDestructurePattern {
                        application_type: Default::default(),
                        head,
                        fields,
                        has_rest: true,
                        guard,
                        condition: ExpressionHandle::invalid(),
                        proof_selectors: selectors,
                    }),
                    value: saved,
                    source_span: SourceSpan::default(),
                },
                TableMatchArm {
                    pattern: MatchPattern::Destructure(TableDestructurePattern {
                        application_type: Default::default(),
                        head,
                        fields: Default::default(),
                        has_rest: false,
                        guard: ExpressionHandle::invalid(),
                        condition: ExpressionHandle::invalid(),
                        proof_selectors: Default::default(),
                    }),
                    value: head,
                    source_span: SourceSpan::default(),
                },
            ]);
            let destructuring = fixture.expression(ExpressionNode::Match(TableMatchExpression {
                subject,
                capture,
                arms: unguarded,
            }));
            add(
                fixture,
                "match that destructures",
                destructuring,
                &[2, 3, 4],
            );
            witnesses
        }
    }

    use self::fixture::Fixture;
    use self::kinds::{
        ActionKind, ConstraintKind, DomainKind, ExpressionKind, LengthKind, OperatorKind,
        PatternKind, TypeKind, action_kind, constraint_kind, domain_kind, expression_kind,
        length_kind, operator_kind, pattern_kind, type_kind,
    };
    use self::witnesses::{Witness, constraint_witnesses, expression_witnesses, type_witnesses};
    use crate::expression::{ExpressionNode, MatchPattern};
    use crate::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
    use language_semantics::type_identity::{
        TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
    };
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct Owners {
        seen: BTreeSet<[u8; 32]>,
        embedded: Vec<String>,
        depth: usize,
    }

    impl TypeIdentityPackageOwnerVisitor for Owners {
        fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
            self.depth += 1;
            Ok(())
        }
        fn leave(&mut self) {
            self.depth -= 1;
        }
        fn reserve(&mut self, _: usize) -> Result<(), TypeIdentityVisitError> {
            Ok(())
        }
        fn package_owner(&mut self, digest: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
            self.seen.insert(digest);
            Ok(())
        }
        fn embedded_name(&mut self, name: &str) -> Result<(), TypeIdentityVisitError> {
            self.embedded.push(name.to_owned());
            Ok(())
        }
    }

    fn read(identity: &str) -> (Result<(), TypeIdentityVisitError>, Owners) {
        let mut owners = Owners::default();
        let result = visit_type_identity_package_owners(identity, &mut owners);
        assert_eq!(owners.depth, 0, "every entered node is left: {identity}");
        (result, owners)
    }

    fn check(fixture: &Fixture, witnesses: &[Witness]) {
        let mut failures = Vec::new();
        for witness in witnesses {
            let text = fixture.render(witness.reference, &witness.binders, &witness.substitutions);
            let (result, owners) = read(&text);
            let expected = witness
                .owners
                .iter()
                .map(|package| [*package; 32])
                .collect::<BTreeSet<_>>();
            if result != Ok(()) || owners.seen != expected {
                failures.push(format!(
                    "{}: {result:?}, {} of {} package owners visited\n    {text}",
                    witness.form,
                    owners.seen.len(),
                    expected.len(),
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} forms do not read back:\n{}",
            failures.len(),
            witnesses.len(),
            failures.join("\n"),
        );
    }

    #[test]
    fn every_type_form_the_renderer_writes_reads_back() {
        let mut fixture = Fixture::new();
        let witnesses = type_witnesses(&mut fixture);
        check(&fixture, &witnesses);
    }

    #[test]
    fn every_constraint_form_the_renderer_writes_reads_back() {
        let mut fixture = Fixture::new();
        let witnesses = constraint_witnesses(&mut fixture);
        check(&fixture, &witnesses);
    }

    #[test]
    fn every_index_form_the_renderer_writes_reads_back() {
        let mut fixture = Fixture::new();
        let witnesses = expression_witnesses(&mut fixture);
        check(&fixture, &witnesses);
    }

    #[test]
    fn witnesses_reach_every_node_kind() {
        let mut fixture = Fixture::new();
        let mut witnesses = type_witnesses(&mut fixture);
        witnesses.extend(constraint_witnesses(&mut fixture));
        witnesses.extend(expression_witnesses(&mut fixture));
        check(&fixture, &witnesses);

        let table = &fixture.program.type_reference_table;
        let mut types = BTreeSet::new();
        let mut constraints = BTreeSet::new();
        let mut domains = BTreeSet::new();
        let mut lengths = BTreeSet::new();
        for (_, node) in table.type_reference_rows() {
            types.insert(type_kind(node));
            match node {
                TypeReferenceNode::Constrained {
                    constraints: span, ..
                } => {
                    for constraint in table.constraints(*span) {
                        constraints.insert(constraint_kind(constraint));
                        if let TypeConstraintNode::Domain(domain) = constraint {
                            domains.insert(domain_kind(&domain.subject));
                        }
                    }
                }
                TypeReferenceNode::FixedArray { length, .. } => {
                    lengths.insert(length_kind(length));
                }
                _ => {}
            }
        }
        assert_eq!(types, TypeKind::ALL.iter().copied().collect());
        assert_eq!(constraints, ConstraintKind::ALL.iter().copied().collect());
        assert_eq!(domains, DomainKind::ALL.iter().copied().collect());
        assert_eq!(lengths, LengthKind::ALL.iter().copied().collect());

        let table = &fixture.program.expression_table;
        let mut expressions = BTreeSet::new();
        let mut operators = BTreeSet::new();
        let mut patterns = BTreeSet::new();
        let mut actions = BTreeSet::new();
        for (_, node) in table.iter_expressions() {
            expressions.insert(expression_kind(node));
            match node {
                ExpressionNode::Binary(binary) => {
                    operators.insert(operator_kind(&binary.operator));
                }
                ExpressionNode::Match(dispatch) => {
                    for arm in table.match_arms(dispatch.arms) {
                        patterns.insert(pattern_kind(&arm.pattern));
                        if let MatchPattern::Destructure(pattern) = &arm.pattern {
                            for field in table.match_fields(pattern.fields) {
                                actions.insert(action_kind(&field.action));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        assert_eq!(expressions, ExpressionKind::ALL.iter().copied().collect());
        assert_eq!(operators, OperatorKind::ALL.iter().copied().collect());
        assert_eq!(patterns, PatternKind::ALL.iter().copied().collect());
        assert_eq!(actions, ActionKind::ALL.iter().copied().collect());
    }

    #[test]
    fn an_unflagged_marker_is_refused_with_its_reason() {
        let mut fixture = Fixture::new();
        let not_a_recipe = fixture.named("Payload");
        let array = fixture.array(FixedArrayLength::Expression(not_a_recipe));
        let text = fixture.render(array, &[], &[]);
        assert_eq!(
            read(&text).0,
            Err(TypeIdentityVisitError::Unadmitted(
                language_semantics::type_identity::vocabulary::Unadmitted::InvalidIdentity
            )),
            "{text}"
        );
    }
}
pub(crate) mod portable_machine_applications {
    use super::IndexIdentityArgument;
    pub(crate) mod captures {
        use super::{
            CoordinateTag, IndexIdentityArgument, PortableMachineApplications, TypeIdentityContext,
            atom, compound, conformances, static_binding_operand,
        };
        use crate::CapturedMachineApplication;
        use crate::TypedTrees;
        use crate::expression::{StaticMachineArgument, StaticSymbolApplication};

        impl PortableMachineApplications {
            pub(crate) fn capture(
                &self,
                program: &TypedTrees,
                capture: &CapturedMachineApplication,
                context: &TypeIdentityContext<'_>,
            ) -> Result<String, String> {
                let original = program
                    .machine_by_symbol(capture.template)
                    .ok_or("captured member lost its original declaration")?;
                let instance = program
                    .machine_by_symbol(capture.instance)
                    .ok_or("captured member lost its executable declaration")?;
                let closed = &capture.conformance_application;
                let implementation = program
                    .conformance_by_symbol(closed.declaration)
                    .ok_or("captured member lost its selecting implementation")?;
                if capture.instance == capture.template
                    || program.declaration_scope_parent(capture.template) != implementation.symbol
                    || !implementation.rows.iter().any(|row| {
                        row.source.owns_realization() && row.realization_machine == capture.template
                    })
                {
                    return Err(
                        "captured member does not belong to its selecting implementation".into(),
                    );
                }
                if !capture.binding_application.is_valid()
                    || !capture.supplying_application.is_valid()
                {
                    return Err("captured member lost its binding or supplying application".into());
                }
                let binding = program.static_machine_application(capture.binding_application);
                let supplying = program.static_machine_application(capture.supplying_application);
                if binding.binding_declaration != implementation.symbol
                    || binding.lexical_owner != supplying.lexical_owner
                {
                    return Err(
                        "captured member bindings changed implementation or supplying owner".into(),
                    );
                }
                let context = TypeIdentityContext {
                    application_owner: binding.lexical_owner,
                    application_argument_owner: binding.lexical_owner,
                    ..*context
                };
                let parameters = program.conformance_type_parameters(implementation);
                let arguments =
                    program.static_machine_application_arguments(capture.binding_application);
                let bindings =
                    program.static_machine_application_bindings(capture.binding_application);
                if parameters.len() != bindings.len() || parameters.len() != arguments.len() {
                    return Err(
                        "captured member has an incomplete implementation argument list".into(),
                    );
                }
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    let mut matching = bindings
                        .iter()
                        .filter(|binding| binding.parameter == parameter.symbol);
                    let selected = matching
                        .next()
                        .ok_or("captured member is missing an implementation parameter binding")?;
                    if matching.next().is_some() {
                        return Err(
                            "captured member repeats an implementation parameter binding".into(),
                        );
                    }
                    let (_, operand) = static_binding_operand(&parameter.kind, &selected.value)?;
                    if super::super::substitution::index_argument(program, operand, &context)
                        != super::super::substitution::index_argument(
                            program,
                            IndexIdentityArgument::StaticArgument(argument),
                            &context,
                        )
                    {
                        return Err(format!(
                            "captured member binding for `{}` differs from its selected argument",
                            parameter.name
                        ));
                    }
                }
                let lifetimes = program
                    .static_application_lifetimes_for_declaration(
                        capture.binding_application,
                        implementation.symbol,
                    )
                    .map_err(str::to_owned)?;
                if lifetimes.len() != implementation.lifetime_parameters.len() {
                    return Err(
                        "captured member has an incomplete implementation lifetime list".into(),
                    );
                }
                let mut lifetime_arguments = Vec::with_capacity(lifetimes.len());
                for parameter in &implementation.lifetime_parameters {
                    let argument = lifetimes
                        .iter()
                        .find(|binding| binding.parameter == *parameter)
                        .ok_or("captured member lost an implementation lifetime binding")?;
                    lifetime_arguments.push(argument.argument.clone());
                }
                let selected = StaticMachineArgument {
                    symbol: implementation.symbol,
                    application: Some(Box::new(StaticSymbolApplication {
                        member: Default::default(),
                        arguments: arguments.into(),
                        lifetime_arguments: lifetime_arguments.into(),
                    })),
                    ..Default::default()
                };
                let implementation_identity =
                    conformances::project(program, closed, &selected, &context)?;

                let source_parameters = program
                    .machine_type_parameters(original)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(original.conformance_bounds.iter().map(|bound| bound.binder))
                    .collect::<Vec<_>>();
                let instance_parameters = program
                    .machine_type_parameters(instance)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(instance.conformance_bounds.iter().map(|bound| bound.binder))
                    .collect::<Vec<_>>();
                if source_parameters.len() != capture.parameters.len()
                    || instance_parameters.len() != capture.parameters.len()
                    || capture
                        .parameters
                        .iter()
                        .zip(source_parameters.iter().zip(&instance_parameters))
                        .any(
                            |((source, realized), (expected_source, expected_instance))| {
                                source != expected_source || realized != expected_instance
                            },
                        )
                {
                    return Err("captured member changed its own parameter correspondence".into());
                }
                let states = program
                    .captured_machine_states
                    .span_or_empty(capture.states);
                if states.len() != program.machine_states(original).len()
                    || states.len() != program.machine_states(instance).len()
                    || states.iter().enumerate().any(|(ordinal, state)| {
                        states[..ordinal].iter().any(|earlier| {
                            earlier.template == state.template || earlier.instance == state.instance
                        }) || !program.instantiated_machine_state_matches(
                            capture.instance,
                            capture.template,
                            state.template,
                            state.instance,
                        )
                    })
                {
                    return Err("captured member changed its explicit entry correspondence".into());
                }
                let origin = self.origin(
                    program,
                    capture.instance,
                    capture.template,
                    &capture.normalized_template_identity,
                    &context,
                )?;
                Ok(compound(
                    CoordinateTag::CapturedCallable,
                    [
                        origin,
                        atom(CoordinateTag::Declaration, &implementation_identity),
                    ],
                ))
            }
        }
    }
    pub(crate) mod conformances {
        use super::super::IndexIdentityArgument;
        use super::super::constraint_identity::{atom, compound};
        use super::super::coordinates::CoordinateTag;
        use super::super::identity_context::TypeIdentityContext;
        use super::super::substitution::index_argument;
        use crate::TypedTrees;
        use crate::expression::{StaticMachineArgument, StaticSymbolApplication};
        use crate::name::Identifier;
        use crate::{ClosedConformanceApplication, ClosedConformanceConstArgument};
        use language_semantics::type_identity::vocabulary::Label;

        pub(crate) fn project(
            program: &TypedTrees,
            closed: &ClosedConformanceApplication,
            supplied: &StaticMachineArgument,
            context: &TypeIdentityContext<'_>,
        ) -> Result<String, String> {
            let declaration = program
                .conformance_by_symbol(closed.declaration)
                .ok_or("specialized callable lost its selected implementation declaration")?;
            if declaration.trait_symbol != closed.trait_definition {
                return Err("selected implementation changed its declared trait".into());
            }
            if declaration.rows.len() != closed.rows.len()
                || declaration
                    .rows
                    .iter()
                    .zip(&closed.rows)
                    .any(|(declared, selected)| {
                        declared.declaring_trait != selected.declaring_trait
                            || declared.requirement != selected.requirement
                            || declared.realization_machine != selected.realization_machine
                            || declared.realization_state != selected.realization_state
                    })
            {
                return Err(
                    "selected implementation methods differ from its exact declaration rows".into(),
                );
            }
            let mut selected = StaticMachineArgument {
                symbol: closed.declaration,
                application: Some(Box::new(StaticSymbolApplication {
                    member: Default::default(),
                    lifetime_arguments: closed
                        .lifetime_arguments
                        .iter()
                        .map(Identifier::generated)
                        .collect(),
                    arguments: closed.arguments.clone(),
                })),
                ..Default::default()
            };
            let mut supplied = supplied.clone();
            if program.conformance_type_parameters(declaration).is_empty()
                && declaration.lifetime_parameters.is_empty()
            {
                for argument in [&mut supplied, &mut selected] {
                    if argument.application.as_ref().is_some_and(|application| {
                        !application.member.is_valid()
                            && application.arguments.is_empty()
                            && application.lifetime_arguments.is_empty()
                    }) {
                        argument.application = None;
                    }
                }
            }
            let own_arguments = index_argument(
                program,
                IndexIdentityArgument::StaticArgument(&selected),
                context,
            );
            if own_arguments
                != index_argument(
                    program,
                    IndexIdentityArgument::StaticArgument(&supplied),
                    context,
                )
            {
                return Err(
                    "specialized callable conformance binding differs from its checked application"
                        .into(),
                );
            }
            let scope = closed
                .trait_scope
                .scope(program)
                .ok_or("selected implementation lost its prepared trait application")?;
            if scope.declaration.symbol != closed.trait_definition {
                return Err("selected implementation's prepared scope names another trait".into());
            }
            validate_selected_scope(program, declaration, closed, &scope, context)?;
            let trait_application = scope.with_identity_scopes(program, |scopes| {
                let mut arguments = Vec::new();
                for parameter in program.trait_type_parameters(scope.declaration) {
                    let (reference, outer) = scope
                        .parameter_application(parameter.symbol)
                        .ok_or("selected implementation lost a target trait argument")?;
                    let context = TypeIdentityContext {
                        application_owner: scope.argument_owner,
                        application_argument_owner: scope.argument_owner,
                        application_scopes: &scopes[..outer],
                        ..*context
                    };
                    let operand = if parameter.kind.is_const_or_value() {
                        IndexIdentityArgument::TypeReference(reference)
                    } else {
                        IndexIdentityArgument::TypeArgument(reference)
                    };
                    arguments.push(index_argument(program, operand, &context));
                }
                let context = TypeIdentityContext {
                    application_owner: scope.argument_owner,
                    application_argument_owner: scope.argument_owner,
                    application_scopes: scopes,
                    ..*context
                };
                let lifetimes = scope
                    .declaration
                    .lifetime_parameters
                    .iter()
                    .map(|parameter| context.lifetime(program, parameter.as_str()));
                Ok::<_, String>(compound(
                    CoordinateTag::TraitApplication,
                    [
                        context.name(program, closed.trait_definition, ""),
                        compound(CoordinateTag::Arguments, arguments),
                        compound(CoordinateTag::Lifetimes, lifetimes),
                    ],
                ))
            })?;
            let type_identity = |reference| {
                index_argument(
                    program,
                    IndexIdentityArgument::TypeArgument(reference),
                    context,
                )
            };
            let constants = closed.const_arguments.iter().map(|argument| {
                let fields = match argument {
                    ClosedConformanceConstArgument::Evaluated {
                        parameter_carrier,
                        declared_carrier,
                        value,
                    } => vec![
                        atom(Label::Kind, "evaluated"),
                        type_identity(*parameter_carrier),
                        type_identity(*declared_carrier),
                        atom(CoordinateTag::Const, &value.encoding),
                    ],
                    ClosedConformanceConstArgument::CallerExpression {
                        parameter_carrier,
                        expression,
                    } => vec![
                        atom(Label::Kind, "expression"),
                        type_identity(*parameter_carrier),
                        index_argument(
                            program,
                            IndexIdentityArgument::TypeReference(*expression),
                            context,
                        ),
                    ],
                    ClosedConformanceConstArgument::CallerBinder {
                        parameter_carrier,
                        binder,
                        binder_carrier,
                    } => vec![
                        atom(Label::Kind, "binder"),
                        type_identity(*parameter_carrier),
                        type_identity(*binder_carrier),
                        index_argument(program, IndexIdentityArgument::Subject(*binder), context),
                    ],
                };
                compound(CoordinateTag::Const, fields)
            });
            let callable = |symbol| {
                index_argument(
                    program,
                    IndexIdentityArgument::StaticArgument(&StaticMachineArgument {
                        symbol,
                        ..Default::default()
                    }),
                    context,
                )
            };
            let mut rows = Vec::with_capacity(closed.rows.len());
            for row in &closed.rows {
                let (owner, requirement) = program
                    .trait_machine_signature(row.requirement)
                    .ok_or("selected implementation lost a checked requirement declaration")?;
                if owner.symbol != row.declaring_trait {
                    return Err(
                        "selected implementation requirement belongs to another trait".into(),
                    );
                }
                let machine = program
                    .machine_by_symbol(row.realization_machine)
                    .ok_or("selected implementation lost its checked method realization")?;
                if !program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == row.realization_state)
                {
                    return Err(
                        "selected implementation method and entry name different declarations"
                            .into(),
                    );
                }
                rows.push(compound(
                    CoordinateTag::Declaration,
                    [
                        context.name(program, row.declaring_trait, ""),
                        context.qualify_non_binder_name(
                            program,
                            row.requirement,
                            program.trait_requirement_callable_identity_text(owner, requirement),
                        ),
                        callable(row.realization_machine),
                        callable(row.realization_state),
                    ],
                ));
            }
            Ok(compound(
                CoordinateTag::Declaration,
                [
                    own_arguments,
                    trait_application,
                    compound(CoordinateTag::Const, constants),
                    compound(
                        CoordinateTag::Arguments,
                        closed
                            .machine_arguments
                            .iter()
                            .map(|symbol| callable(*symbol)),
                    ),
                    compound(CoordinateTag::Declaration, rows),
                ],
            ))
        }

        fn validate_selected_scope(
            program: &TypedTrees,
            declaration: &crate::trait_definition::Conformance,
            closed: &ClosedConformanceApplication,
            scope: &crate::trait_definition::AppliedTraitScope<'_>,
            context: &TypeIdentityContext<'_>,
        ) -> Result<(), String> {
            if scope.argument_owner != context.application_owner {
                return Err(
                    "selected implementation scope belongs to another supplying declaration".into(),
                );
            }
            let parameters = program.conformance_type_parameters(declaration);
            let references = scope.with_identity_scopes(program, |scopes| {
                let own = scopes
                    .first()
                    .filter(|own| own.declaration == declaration.symbol)
                    .ok_or("selected implementation scope lost its own argument frame")?;
                if own.arguments.len() != parameters.len()
                    || closed.arguments.len() != parameters.len()
                    || own.lifetimes.len() != declaration.lifetime_parameters.len()
                    || closed.lifetime_arguments.len() != declaration.lifetime_parameters.len()
                {
                    return Err(
                        "selected implementation scope has an incomplete own telescope".into(),
                    );
                }
                for ((formal, actual), (parameter, supplied)) in own.lifetimes.iter().zip(
                    declaration
                        .lifetime_parameters
                        .iter()
                        .zip(&closed.lifetime_arguments),
                ) {
                    if formal != parameter.as_str() || actual != supplied {
                        return Err(
                            "selected implementation scope changed an own lifetime argument".into(),
                        );
                    }
                }
                let mut references = Vec::with_capacity(parameters.len());
                for (((binder, actual), parameter), supplied) in own
                    .arguments
                    .iter()
                    .zip(parameters)
                    .zip(closed.arguments.iter())
                {
                    if *binder != parameter.symbol {
                        return Err(
                            "selected implementation scope changed an own parameter declaration"
                                .into(),
                        );
                    }
                    let reference = match actual {
                        IndexIdentityArgument::TypeArgument(reference)
                        | IndexIdentityArgument::TypeReference(reference) => *reference,
                        _ => {
                            return Err(
                                "selected implementation scope lost its prepared type operand"
                                    .into(),
                            );
                        }
                    };
                    let actual = program
                        .static_operand_from_type_reference(reference)
                        .map_err(str::to_owned)?;
                    if index_argument(
                        program,
                        IndexIdentityArgument::StaticArgument(&actual),
                        context,
                    ) != index_argument(
                        program,
                        IndexIdentityArgument::StaticArgument(supplied),
                        context,
                    ) {
                        return Err(
                            "selected implementation scope changed an own static argument".into(),
                        );
                    }
                    references.push(reference);
                }
                Ok::<_, String>(references)
            })?;
            let lifetimes = closed
                .lifetime_arguments
                .iter()
                .map(Identifier::generated)
                .collect::<Vec<_>>();
            let expected =
                crate::trait_definition::AppliedTraitScope::from_conformance_application(
                    program,
                    declaration,
                    crate::trait_definition::TraitApplicationArguments {
                        lexical_owner: context.application_owner,
                        enclosing_data_applications: &[],
                        arguments: &references,
                        lifetimes: &lifetimes,
                    },
                )
                .ok_or(
                    "selected implementation declaration has no matching prepared application",
                )?;
            if !scope.same_application(program, &expected) {
                return Err("selected implementation's target trait arguments differ from its declaration application".into());
            }
            Ok(())
        }
    }

    use super::constraint_identity::{atom, compound};
    use super::coordinates::CoordinateTag;
    use super::identity_context::{TypeIdentityContext, TypeIdentityQualification};
    use crate::TypedTrees;
    use crate::callable_applications::MachineInstantiation;
    use crate::data::TypeParameterKind;
    use crate::static_applications::StaticMachineBindingValue;
    use language_semantics::type_identity::vocabulary::{Label, NameForm};
    use std::cell::{Cell, RefCell};
    use symbols::SymbolHandle;

    pub fn portable_machine_application_identity(
        program: &TypedTrees,
        symbol: SymbolHandle,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<Option<String>, String> {
        let projection = PortableMachineApplications::default();
        let invalid = Cell::new(false);
        let context = TypeIdentityContext {
            portable_machine_applications: Some(&projection),
            qualification: TypeIdentityQualification::PackageQualified,
            exact_toolchain_sources,
            missing_exact_nominal_owner: Some(&invalid),
            invalid_application: Some(&invalid),
            encountered_elided_regions: Some(&invalid),
            invalid_index_argument: Some(&invalid),
            ..Default::default()
        };
        let result = projection.identity(program, symbol, &context)?;
        if let Some(reason) = projection.error.into_inner() {
            return Err(reason);
        }
        if invalid.get() {
            return Err(
                "specialization argument lost its exact owner, static operand, or lifetime scope"
                    .into(),
            );
        }
        Ok(result)
    }

    #[derive(Default)]
    pub(crate) struct PortableMachineApplications {
        active: RefCell<Vec<SymbolHandle>>,
        error: RefCell<Option<String>>,
    }

    impl PortableMachineApplications {
        pub(crate) fn nominal_name(
            &self,
            program: &TypedTrees,
            symbol: SymbolHandle,
            context: &TypeIdentityContext<'_>,
        ) -> Option<String> {
            match self.identity(program, symbol, context) {
                Ok(identity) => identity,
                Err(reason) => {
                    self.error
                        .borrow_mut()
                        .get_or_insert_with(|| reason.clone());
                    Some(atom(NameForm::InvalidDerivedOrigin, &reason))
                }
            }
        }

        fn identity(
            &self,
            program: &TypedTrees,
            symbol: SymbolHandle,
            context: &TypeIdentityContext<'_>,
        ) -> Result<Option<String>, String> {
            let Some(machine) = program
                .machine_by_symbol(symbol)
                .or_else(|| program.machine_holding_state(symbol))
            else {
                return Ok(None);
            };
            let Some(receipt) = program
                .machine_instantiation(machine.symbol)
                .map_err(|reason| format!("callable `{}`: {reason}", machine.name))?
            else {
                return Ok(None);
            };
            if self.active.borrow().contains(&machine.symbol) {
                return Err("specialized callable has a cyclic static argument identity".into());
            }
            self.active.borrow_mut().push(machine.symbol);
            let result = match receipt {
                MachineInstantiation::Specialization(specialization) => {
                    self.application(program, specialization, context)
                }
                MachineInstantiation::Capture(capture) => self.capture(program, capture, context),
            };
            self.active.borrow_mut().pop();
            let identity =
                result.map_err(|reason| format!("callable `{}`: {reason}", machine.name))?;
            if symbol == machine.symbol {
                return Ok(Some(identity));
            }
            let states = program.machine_states(machine);
            if states.first().is_some_and(|entry| entry.symbol == symbol) {
                Ok(Some(identity))
            } else {
                let original = program
                    .machine_by_symbol(receipt.template())
                    .ok_or("specialized callable lost its original nested declaration owner")?;
                let mut matches = program.machine_states(original).iter().filter(|state| {
                    program.instantiated_machine_state_matches(
                        machine.symbol,
                        receipt.template(),
                        state.symbol,
                        symbol,
                    )
                });
                let declaration = matches.next().ok_or(
                    "specialized nested callable does not rejoin its original declaration",
                )?;
                if matches.next().is_some() {
                    return Err(
                        "specialized nested callable has ambiguous original declarations".into(),
                    );
                }
                Ok(Some(compound(
                    CoordinateTag::NestedMachine,
                    [
                        identity,
                        atom(CoordinateTag::Name, declaration.name.as_str()),
                    ],
                )))
            }
        }

        fn origin(
            &self,
            program: &TypedTrees,
            instance: SymbolHandle,
            template: SymbolHandle,
            normalized_template_identity: &str,
            context: &TypeIdentityContext<'_>,
        ) -> Result<String, String> {
            if instance != template
                && let Some(origin) = self.identity(program, template, context)?
            {
                return Ok(origin);
            }
            if normalized_template_identity.is_empty() {
                return Err("callable application lost its original overload identity".into());
            }
            let declaration_context = TypeIdentityContext {
                portable_machine_applications: None,
                ..*context
            };
            Ok(compound(
                CoordinateTag::NamedCallable,
                [
                    declaration_context.name(program, template, ""),
                    atom(CoordinateTag::Template, normalized_template_identity),
                ],
            ))
        }

        fn application(
            &self,
            program: &TypedTrees,
            receipt: &crate::MachineSpecialization,
            context: &TypeIdentityContext<'_>,
        ) -> Result<String, String> {
            if !receipt.binding_application.is_valid()
                || receipt.normalized_template_identity.is_empty()
            {
                return Err(
                    "specialized callable lost its original declaration or binding application"
                        .into(),
                );
            }
            let application = program.static_machine_application(receipt.binding_application);
            if application.binding_declaration != receipt.template {
                return Err("specialized callable bindings belong to another declaration".into());
            }
            let template = program
                .machine_by_symbol(receipt.template)
                .ok_or("specialized callable lost its original machine declaration")?;
            let parameters = program
                .data_type_parameters
                .span(receipt.template_parameters)
                .ok_or("specialized callable lost its original static parameters")?;
            let bindings = program.static_machine_application_bindings(receipt.binding_application);
            if parameters.len() != bindings.len() {
                return Err("specialized callable has an incomplete static argument list".into());
            }
            let declaration = self.origin(
                program,
                receipt.instance,
                receipt.template,
                &receipt.normalized_template_identity,
                context,
            )?;
            let argument_context = TypeIdentityContext {
                application_owner: application.lexical_owner,
                application_argument_owner: application.lexical_owner,
                ..*context
            };
            let mut arguments = Vec::with_capacity(parameters.len());
            let mut conformances = receipt.conformance_applications.iter();
            let mut conformance_declarations = receipt.conformance_arguments.iter();
            for parameter in parameters {
                let mut matches = bindings
                    .iter()
                    .filter(|binding| binding.parameter == parameter.symbol);
                let binding = matches
                    .next()
                    .ok_or("specialized callable is missing an original parameter binding")?;
                if matches.next().is_some() {
                    return Err("specialized callable repeats an original parameter binding".into());
                }
                let (kind, operand) = static_binding_operand(&parameter.kind, &binding.value)?;
                let mut fields = vec![atom(Label::Kind, kind)];
                if let StaticMachineBindingValue::Conformance(argument) = &binding.value {
                    let closed = conformances
                        .next()
                        .ok_or("specialized callable lost a checked conformance application")?;
                    if conformance_declarations.next() != Some(&closed.declaration) {
                        return Err(
                            "specialized callable has inconsistent checked conformance selection"
                                .into(),
                        );
                    }
                    fields.push(conformances::project(
                        program,
                        closed,
                        argument,
                        &argument_context,
                    )?);
                } else {
                    fields.push(super::substitution::index_argument(
                        program,
                        operand,
                        &argument_context,
                    ));
                }
                arguments.push(compound(CoordinateTag::Parameter, fields));
            }
            if conformances.next().is_some() || conformance_declarations.next().is_some() {
                return Err(
                    "specialized callable retains a conformance without an original parameter"
                        .into(),
                );
            }
            let bindings = program
                .static_application_lifetimes_for_declaration(
                    receipt.binding_application,
                    receipt.template,
                )
                .map_err(str::to_owned)?;
            if bindings.len() != template.lifetime_parameters.len() {
                return Err("specialized callable has an incomplete lifetime argument list".into());
            }
            let mut lifetimes = Vec::with_capacity(bindings.len());
            for parameter in &template.lifetime_parameters {
                let binding = bindings
                    .iter()
                    .find(|binding| &binding.parameter == parameter)
                    .ok_or("specialized callable lost an original lifetime argument")?;
                lifetimes.push(argument_context.lifetime(program, binding.argument.as_str()));
            }
            Ok(compound(
                CoordinateTag::SpecializedCallable,
                [
                    declaration,
                    compound(CoordinateTag::Arguments, arguments),
                    compound(CoordinateTag::Lifetimes, lifetimes),
                ],
            ))
        }
    }

    fn static_binding_operand<'a>(
        kind: &TypeParameterKind,
        value: &'a StaticMachineBindingValue,
    ) -> Result<(&'static str, IndexIdentityArgument<'a>), String> {
        Ok(match (kind, value) {
            (TypeParameterKind::Type, StaticMachineBindingValue::Type(reference)) => {
                ("type", IndexIdentityArgument::TypeArgument(*reference))
            }
            (TypeParameterKind::Const { .. }, StaticMachineBindingValue::Const(reference)) => {
                ("const", IndexIdentityArgument::TypeReference(*reference))
            }
            (TypeParameterKind::Value { .. }, StaticMachineBindingValue::Const(reference)) => {
                ("value", IndexIdentityArgument::TypeReference(*reference))
            }
            (
                TypeParameterKind::Value { .. },
                StaticMachineBindingValue::RuntimeValue {
                    type_reference,
                    subject,
                    expression,
                },
            ) if !subject.is_valid() && !expression.is_valid() => (
                "runtime-carrier",
                IndexIdentityArgument::TypeArgument(*type_reference),
            ),
            (TypeParameterKind::Machine { .. }, StaticMachineBindingValue::Machine(argument)) => {
                ("machine", IndexIdentityArgument::StaticArgument(argument))
            }
            (
                TypeParameterKind::Implementation,
                StaticMachineBindingValue::Conformance(argument),
            ) => (
                "conformance",
                IndexIdentityArgument::StaticArgument(argument),
            ),
            _ => {
                return Err(
                    "specialized callable argument differs from its original parameter kind".into(),
                );
            }
        })
    }
}
pub(crate) mod result_dispatch_terms {
    use crate::TypedTrees;
    use crate::type_system::type_identity::NormalizedDomainTerm;
    use crate::type_system::type_identity::constraint_identity::normalized_compiler_domain_identity;
    use crate::type_system::type_identity::identity_context::TypeIdentityContext;
    use crate::types::{
        DomainConstraint, DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle,
        TypeReferenceNode,
    };
    use symbols::SymbolHandle;

    pub(crate) fn collect_result_dispatch_terms(
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        terms: &mut Vec<NormalizedDomainTerm>,
        alias_stack: &mut Vec<SymbolHandle>,
    ) {
        if !type_reference.is_valid() {
            return;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                collect_result_dispatch_terms(program, *referee, terms, alias_stack);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                collect_result_dispatch_terms(program, *base_type, terms, alias_stack);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::ArithmeticDomain(domain) => {
                            terms.push(NormalizedDomainTerm::Arithmetic(domain.name().to_owned()))
                        }
                        TypeConstraintNode::Domain(domain) => {
                            if domain.subject == DomainConstraintSubject::Declared {
                                collect_declared_result_dispatch_terms(
                                    program,
                                    domain,
                                    terms,
                                    alias_stack,
                                );
                            } else {
                                terms.push(NormalizedDomainTerm::Compiler(
                                    normalized_compiler_domain_identity(
                                        program,
                                        domain,
                                        &TypeIdentityContext::default(),
                                    ),
                                ));
                            }
                        }
                        TypeConstraintNode::Named(_) => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named { .. }
            | TypeReferenceNode::Unit => {}
        }
    }

    fn collect_declared_result_dispatch_terms(
        program: &TypedTrees,
        constraint: &DomainConstraint,
        terms: &mut Vec<NormalizedDomainTerm>,
        alias_stack: &mut Vec<SymbolHandle>,
    ) {
        let definition = constraint
            .symbol
            .is_valid()
            .then(|| program.domain_definition(constraint.symbol));
        if let Some(Some(definition)) = definition {
            if let Some(alias) = definition.alias.as_ref() {
                if alias_stack.contains(&definition.symbol) {
                    return;
                }
                alias_stack.push(definition.symbol);
                for constituent in &alias.constituents {
                    let Some(constituent_definition) =
                        program.domain_definition(constituent.domain_symbol)
                    else {
                        continue;
                    };
                    let constituent_constraint = DomainConstraint {
                        name: constituent_definition.name.clone(),
                        arguments: Vec::new(),
                        subject: crate::types::DomainConstraintSubject::Declared,
                        symbol: constituent_definition.symbol,
                        semantic_id: constituent_definition.semantic_id,
                        classification: constituent_definition.classification,
                        predicate_presence: constituent_definition.predicate_presence,
                        semantic_roles: constituent_definition.semantic_roles,
                        establishment_routes: constituent_definition.establishment_routes.clone(),
                        source_selection: None,
                    };
                    collect_declared_result_dispatch_terms(
                        program,
                        &constituent_constraint,
                        terms,
                        alias_stack,
                    );
                }
                alias_stack.pop();
                return;
            }
            if definition.predicate_presence.is_present()
                && definition.semantic_roles.is_empty()
                && definition.establishment_routes.is_empty()
            {
                return;
            }
            terms.push(NormalizedDomainTerm::Declared(declared_domain_name(
                program,
                definition.semantic_id,
                definition.name.as_str(),
            )));
            return;
        }

        if constraint.predicate_presence.is_present()
            && constraint.semantic_roles.is_empty()
            && constraint.establishment_routes.is_empty()
        {
            return;
        }
        terms.push(NormalizedDomainTerm::Declared(declared_domain_name(
            program,
            constraint.semantic_id,
            constraint.name.as_str(),
        )));
    }

    fn declared_domain_name(
        program: &TypedTrees,
        semantic_id: language_semantics::SemanticDomainId,
        fallback: &str,
    ) -> String {
        semantic_id
            .is_valid()
            .then(|| program.semantic_domains.name(semantic_id))
            .flatten()
            .unwrap_or(fallback)
            .to_owned()
    }
}
pub(crate) mod static_operand_identity {
    use super::{
        IndexIdentityArgument, NormalizedTypeIdentity, SubstitutionSemantics, TypeIdentityContext,
        program_intern_type_identity, substitution,
    };
    use crate::{TypedTrees, types::TypeReferenceHandle};
    use std::cell::Cell;
    use symbols::SymbolHandle;

    #[derive(Clone, Copy)]
    pub struct StaticOperandIdentityRequest<'a> {
        pub operand: IndexIdentityArgument<'a>,
        pub lexical_owner: SymbolHandle,
        pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
    }

    pub struct DeclaredStaticOperandIdentityRequest<'a> {
        pub operand: IndexIdentityArgument<'a>,
        pub lexical_owner: SymbolHandle,
        pub binders: &'a [(SymbolHandle, String)],
        pub lifetime_binders: &'a [(super::LifetimeParameterIdentity, usize)],
    }

    impl TypedTrees {
        pub fn declared_static_operand_identity(
            &self,
            request: DeclaredStaticOperandIdentityRequest<'_>,
        ) -> Option<NormalizedTypeIdentity> {
            let invalid = Cell::new(false);
            let identity = substitution::index_argument(
                self,
                request.operand,
                &TypeIdentityContext {
                    application_owner: request.lexical_owner,
                    application_argument_owner: request.lexical_owner,
                    binders: request.binders,
                    lifetime_binders: request.lifetime_binders,
                    qualification: super::TypeIdentityQualification::PackageQualified,
                    invalid_application: Some(&invalid),
                    encountered_elided_regions: Some(&invalid),
                    invalid_index_argument: Some(&invalid),
                    ..Default::default()
                },
            );
            (!invalid.get()).then(|| program_intern_type_identity(self, identity))
        }

        pub fn static_operand_identity(
            &self,
            request: StaticOperandIdentityRequest<'_>,
        ) -> Option<NormalizedTypeIdentity> {
            self.static_operand_identity_with_semantics(request, SubstitutionSemantics::Transitive)
        }

        pub fn static_operand_identity_with_simultaneous_substitutions(
            &self,
            request: StaticOperandIdentityRequest<'_>,
        ) -> Option<NormalizedTypeIdentity> {
            self.static_operand_identity_with_semantics(
                request,
                SubstitutionSemantics::Simultaneous,
            )
        }

        fn static_operand_identity_with_semantics(
            &self,
            request: StaticOperandIdentityRequest<'_>,
            substitution_semantics: SubstitutionSemantics,
        ) -> Option<NormalizedTypeIdentity> {
            let invalid = Cell::new(false);
            let identity = substitution::index_argument(
                self,
                request.operand,
                &TypeIdentityContext {
                    application_owner: request.lexical_owner,
                    substitutions: request.substitutions,
                    substitution_semantics,
                    invalid_application: Some(&invalid),
                    encountered_elided_regions: Some(&invalid),
                    invalid_index_argument: Some(&invalid),
                    ..TypeIdentityContext::default()
                },
            );
            (!invalid.get()).then(|| program_intern_type_identity(self, identity))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{expression::ExpressionNode, name::Identifier, types::TypeReferenceNode};
        use numerics::literals::{IntegerLiteral, IntegerRadix};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn receiving_static_indices_do_not_resubstitute_sibling_bindings() {
            let mut symbols = SymbolTableBuilder::default();
            let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Owner"));
            let parameters = symbols
                .insert_children(
                    owner,
                    [
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("N")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("M")),
                    ],
                )
                .handles()
                .collect::<Vec<_>>();
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let references = parameters
                .iter()
                .map(|symbol| {
                    let name = Identifier::from(program.symbols.name(*symbol));
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol: *symbol,
                            name,
                        })
                })
                .collect::<Vec<_>>();
            let integer = program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "7").unwrap(),
            ));
            let seven = program
                .type_reference_table
                .insert(TypeReferenceNode::ConstExpression(integer));
            let bindings = [(parameters[0], references[1]), (parameters[1], seven)];
            let request = StaticOperandIdentityRequest {
                operand: IndexIdentityArgument::TypeReference(references[0]),
                lexical_owner: owner,
                substitutions: &bindings,
            };
            let direct = |reference| {
                program.static_operand_identity(StaticOperandIdentityRequest {
                    operand: IndexIdentityArgument::TypeReference(reference),
                    substitutions: &[],
                    ..request
                })
            };
            let receiving =
                program.static_operand_identity_with_simultaneous_substitutions(request);
            assert!(receiving.is_some());
            assert_eq!(receiving, direct(references[1]));
            assert_eq!(program.static_operand_identity(request), direct(seven));
            assert_ne!(receiving, direct(seven));
            let permutation = [
                (parameters[0], references[1]),
                (parameters[1], references[0]),
            ];
            let request = StaticOperandIdentityRequest {
                substitutions: &permutation,
                ..request
            };
            assert_eq!(
                program.static_operand_identity_with_simultaneous_substitutions(request),
                direct(references[1]),
            );
            assert!(program.static_operand_identity(request).is_none());
        }
    }
}
pub(crate) mod storage_identity {
    use crate::TypedTrees;
    use crate::data::{TypeParameter, TypeParameterKind};
    use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
    use symbols::SymbolHandle;

    pub(crate) fn independent_value_index(
        program: &TypedTrees,
        owner: SymbolHandle,
        parameter: &TypeParameter,
    ) -> bool {
        if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
            return false;
        }
        let Some(data) = program.unique_data_definition_by_symbol(owner) else {
            return false;
        };
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || data.quotient.is_some()
        {
            return false;
        }
        program
            .data_member_fields(program.data_members(data))
            .all(|field| independent_type(program, field.type_reference, parameter.symbol, 0))
    }

    fn independent_type(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        parameter: SymbolHandle,
        depth: usize,
    ) -> bool {
        if depth > 128 || !program.contains_type_reference(reference) {
            return false;
        }
        let child = |reference| independent_type(program, reference, parameter, depth + 1);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                *symbol != parameter
                    && program
                        .data_definition_by_symbol(*symbol)
                        .and_then(|data| data.generic_instance)
                        .is_none_or(child)
            }
            TypeReferenceNode::Reference { referee, .. } => child(*referee),
            TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(_),
            } => child(*element_type),
            TypeReferenceNode::Generic { arguments, .. } => program
                .type_reference_handles(*arguments)
                .iter()
                .all(|argument| child(*argument)),
            TypeReferenceNode::Unit => true,
            _ => false,
        }
    }
}
pub(crate) mod substitution {
    use super::IndexIdentityArgument;
    use crate::type_system::type_identity::constraint_identity::atom;
    use crate::type_system::type_identity::constraint_identity::bare;
    use crate::type_system::type_identity::constraint_identity::compound;
    use crate::type_system::type_identity::identity_context::TypeIdentityContext;
    use crate::type_system::type_identity::identity_context::normalize_const_or_nominal_name;
    use crate::type_system::type_identity::identity_context::normalize_index_expression;
    use crate::type_system::type_identity::identity_context::normalize_type_reference;
    use crate::types::FixedArrayLength;
    use crate::{
        TypedTrees,
        expression::ExpressionNode,
        types::{TypeReferenceHandle, TypeReferenceNode},
    };
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    use language_semantics::type_identity::vocabulary::{ArrayLengthForm, IndexForm, Label};
    use symbols::SymbolHandle;

    pub(crate) fn index_argument(
        program: &TypedTrees,
        argument: IndexIdentityArgument<'_>,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        match argument {
            IndexIdentityArgument::TypeArgument(reference) => {
                if !program.contains_type_reference(reference) {
                    return rejected(context);
                }
                normalize_type_reference(program, reference, context)
            }
            IndexIdentityArgument::Expression(expression) => {
                if !program.expression_table.expression_is_valid(expression) {
                    return rejected(context);
                }
                normalize_index_expression(program, expression, context)
            }
            IndexIdentityArgument::TypeReference(reference) => {
                if !program.contains_type_reference(reference) {
                    return rejected(context);
                }
                index_reference(program, reference, context)
            }
            IndexIdentityArgument::Subject(symbol) => {
                if !symbol.is_valid() {
                    return rejected(context);
                }
                index_name(program, symbol, program.symbols.name(symbol), context)
            }
            IndexIdentityArgument::FieldProjection { receiver, field } => {
                if !program.expression_table.expression_is_valid(receiver) || !field.is_valid() {
                    return rejected(context);
                }
                let declaration = program.symbols.get(field);
                let case = declaration.parent;
                if declaration.kind != symbols::SymbolKind::Field
                    || !case.is_valid()
                    || program.symbols.get(case).kind != symbols::SymbolKind::Variant
                {
                    return rejected(context);
                }
                compound(
                    IndexForm::Member,
                    [
                        normalize_index_expression(program, receiver, context),
                        atom(Label::Member, program.symbols.name(field)),
                        atom(Label::Case, program.symbols.name(case)),
                    ],
                )
            }
            IndexIdentityArgument::StaticArgument(argument) => {
                if argument
                    .application
                    .as_ref()
                    .is_some_and(|application| application.member.is_valid())
                {
                    return rejected(context);
                }
                if argument.type_reference.is_valid() {
                    if !program.contains_type_reference(argument.type_reference) {
                        return rejected(context);
                    }
                    if context.invalid_application.is_some()
                        && let Ok(selected) =
                            program.static_operand_from_type_reference(argument.type_reference)
                        && selected.symbol.is_valid()
                        && matches!(
                            program.symbols.get(selected.symbol).kind,
                            symbols::SymbolKind::Machine | symbols::SymbolKind::State
                        )
                    {
                        return index_argument(
                            program,
                            IndexIdentityArgument::StaticArgument(&selected),
                            context,
                        );
                    }
                    if matches!(
                        program.type_reference_node(argument.type_reference),
                        TypeReferenceNode::ConstExpression(_)
                    ) {
                        return index_reference(program, argument.type_reference, context);
                    }
                    return normalize_type_reference(program, argument.type_reference, context);
                }
                if let Some(literal) = &argument.const_literal {
                    return index_integer(literal, context);
                }
                if !argument.symbol.is_valid()
                    && argument.application.is_none()
                    && argument.evidence_projection.is_none()
                    && let [spelling] = argument.path.as_ref()
                    && CanonicalConstValue::from_atom(spelling.as_str())
                        .and_then(|value| value.decode_encoding())
                        .is_some()
                {
                    return index_name(program, argument.symbol, spelling.as_str(), context);
                }
                if let Some(identity) =
                    static_callable_argument_identity(program, argument, context)
                {
                    return identity;
                }
                if let Some(identity) = super::identity_context::static_nominal_argument_identity(
                    program, argument, context,
                ) {
                    return identity;
                }
                if !argument.symbol.is_valid() || argument.evidence_projection.is_some() {
                    return rejected(context);
                }
                let owner = index_name(program, argument.symbol, "", context);
                let Some(application) = &argument.application else {
                    return owner;
                };
                compound(
                    IndexForm::StaticApplication,
                    std::iter::once(owner)
                        .chain(application.lifetime_arguments.iter().map(|lifetime| {
                            if context.invalid_application.is_some() {
                                context.lifetime(program, lifetime.as_str())
                            } else {
                                atom(Label::Lifetime, lifetime.as_str())
                            }
                        }))
                        .chain(application.arguments.iter().map(|argument| {
                            index_argument(
                                program,
                                IndexIdentityArgument::StaticArgument(argument),
                                context,
                            )
                        })),
                )
            }
        }
    }

    fn static_callable_argument_identity(
        program: &TypedTrees,
        argument: &crate::expression::StaticMachineArgument,
        context: &TypeIdentityContext<'_>,
    ) -> Option<String> {
        if context.invalid_application.is_none()
            || !argument.symbol.is_valid()
            || !matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::Machine | symbols::SymbolKind::State
            )
        {
            return None;
        }
        let machine = match program.symbols.get(argument.symbol).kind {
            symbols::SymbolKind::Machine => program.machine_by_symbol(argument.symbol),
            symbols::SymbolKind::State => program.machine_holding_state(argument.symbol),
            _ => unreachable!("callable kinds checked above"),
        };
        let Some(machine) = machine else {
            return Some(rejected(context));
        };
        let states = program.machine_states(machine);
        let state = if argument.symbol == machine.symbol {
            states.first()
        } else {
            let mut matching = states
                .iter()
                .filter(|state| state.symbol == argument.symbol);
            let selected = matching.next();
            if matching.next().is_some() {
                return Some(rejected(context));
            }
            selected
        };
        let Some(state) = state.filter(|state| state.symbol.is_valid()) else {
            return Some(rejected(context));
        };
        if let Some(projection) = context.portable_machine_applications
            && let Some(identity) = projection.nominal_name(program, argument.symbol, context)
        {
            let mut operands = vec![atom(IndexForm::ConstName, &identity)];
            if let Some(application) = &argument.application {
                operands.extend(
                    application
                        .lifetime_arguments
                        .iter()
                        .map(|lifetime| context.lifetime(program, lifetime.as_str())),
                );
                operands.extend(application.arguments.iter().map(|argument| {
                    index_argument(
                        program,
                        IndexIdentityArgument::StaticArgument(argument),
                        context,
                    )
                }));
            }
            return Some(compound(IndexForm::StaticApplication, operands));
        }
        let Some(overload) = program.machine_callable_identity_text(machine) else {
            return Some(rejected(context));
        };
        let mut operands = vec![
            atom(
                IndexForm::ConstName,
                &context.qualify_non_binder_name(
                    program,
                    machine.symbol,
                    program.symbols.display_path(machine.symbol, "::"),
                ),
            ),
            atom(
                IndexForm::ConstName,
                &context.qualify_non_binder_name(
                    program,
                    state.symbol,
                    program.symbols.display_path(state.symbol, "::"),
                ),
            ),
            atom(IndexForm::ConstName, &overload),
        ];
        if let Some(application) = &argument.application {
            operands.extend(
                application
                    .lifetime_arguments
                    .iter()
                    .map(|lifetime| context.lifetime(program, lifetime.as_str())),
            );
            operands.extend(application.arguments.iter().map(|argument| {
                index_argument(
                    program,
                    IndexIdentityArgument::StaticArgument(argument),
                    context,
                )
            }));
        }
        Some(compound(IndexForm::StaticApplication, operands))
    }

    pub(crate) fn index_integer(
        literal: &numerics::literals::IntegerLiteral,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        literal.value_bignum().map_or_else(
            || rejected(context),
            |value| atom(IndexForm::Integer, &value.to_string()),
        )
    }

    pub(crate) fn index_name(
        program: &TypedTrees,
        symbol: SymbolHandle,
        spelling: &str,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        if let Some(identity) = index(program, symbol, spelling, context) {
            return identity;
        }
        if let Some(value) = integer(program, symbol, spelling) {
            return atom(IndexForm::Integer, &value.to_string());
        }
        if let Some(value) = boolean(program, symbol, spelling) {
            return atom(IndexForm::Boolean, &value.to_string());
        }
        if let Some(declaration) = program.const_declaration_by_symbol(symbol)
            && let Some(encoding) = declaration
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
            && let Some(
                DecodedCanonicalConstValue::Array { .. }
                | DecodedCanonicalConstValue::Record { .. }
                | DecodedCanonicalConstValue::Variant { .. },
            ) = CanonicalConstValue::new("", encoding, "").decode_encoding()
        {
            return compound(
                IndexForm::CanonicalConst,
                [
                    atom(
                        Label::Type,
                        &program.normalized_type_identity_text(declaration.declared_type),
                    ),
                    atom(Label::Encoding, encoding),
                ],
            );
        }
        normalize_const_or_nominal_name(program, symbol, spelling, IndexForm::ConstName, context)
    }

    fn selected(
        context: &TypeIdentityContext<'_>,
        symbol: SymbolHandle,
    ) -> Option<(usize, TypeReferenceHandle)> {
        if symbol.is_valid() {
            return context
                .substitutions
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (candidate, _))| *candidate == symbol)
                .map(|(index, (_, reference))| (index, *reference));
        }
        None
    }

    macro_rules! open_selected_substitution {
        ($context:expr, $symbol:expr, $active:ident, $reference:ident, $nested:ident) => {
            let (ordinal, $reference) = match selected($context, $symbol) {
                Some(selected_reference) => selected_reference,
                None => return None,
            };
            if $context.active_const_substitutions.len() >= 64
                || $context.active_const_substitutions.contains(&$symbol)
            {
                return Some(rejected($context));
            }
            let mut $active = $context.active_const_substitutions.to_vec();
            $active.push($symbol);
            let $nested = TypeIdentityContext {
                active_const_substitutions: &$active,
                ..$context.replacement_context_at(ordinal)
            };
        };
    }

    pub(crate) fn index(
        program: &TypedTrees,
        symbol: SymbolHandle,
        _spelling: &str,
        context: &TypeIdentityContext<'_>,
    ) -> Option<String> {
        if let Some((argument, outer)) = context.applied_argument(symbol) {
            return Some(index_argument(program, argument, &outer));
        }
        if symbol.is_valid()
            && let Some((_, replacement)) = context
                .index_substitutions
                .iter()
                .find(|(candidate, _)| *candidate == symbol)
        {
            return Some(index_argument(
                program,
                *replacement,
                &TypeIdentityContext {
                    index_substitutions: &[],
                    ..*context
                },
            ));
        }
        open_selected_substitution!(context, symbol, active, reference, nested);
        Some(index_reference(program, reference, &nested))
    }

    fn index_reference(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        context: &TypeIdentityContext<'_>,
    ) -> String {
        if context.invalid_application.is_some()
            && let Ok(selected) = program.static_operand_from_type_reference(reference)
            && selected.symbol.is_valid()
            && matches!(
                program.symbols.get(selected.symbol).kind,
                symbols::SymbolKind::Machine | symbols::SymbolKind::State
            )
        {
            return index_argument(
                program,
                IndexIdentityArgument::StaticArgument(&selected),
                context,
            );
        }
        match program.type_reference_table.type_reference(reference) {
            // A named implementation is a declaration, not a constant. Written
            // directly as a trait argument it is identified as a nominal, so an
            // implementation that arrives here through a binder keeps that same
            // identity. Otherwise one map compared with itself would differ
            // only in whether it was forwarded.
            TypeReferenceNode::Named { symbol, .. }
                if program.conformance_by_symbol(*symbol).is_some() =>
            {
                normalize_type_reference(program, reference, context)
            }
            TypeReferenceNode::Named { symbol, name } => {
                index_name(program, *symbol, name.as_str(), context)
            }
            TypeReferenceNode::ConstExpression(expression) => {
                normalize_index_expression(program, *expression, context)
            }
            _ => normalize_type_reference(program, reference, context),
        }
    }

    pub(crate) fn array_length(
        program: &TypedTrees,
        symbol: SymbolHandle,
        _spelling: &str,
        context: &TypeIdentityContext<'_>,
    ) -> Option<String> {
        open_selected_substitution!(context, symbol, active, reference, nested);
        Some(
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, name } => {
                    if let Some(identity) = array_length(program, *symbol, name.as_str(), &nested) {
                        return Some(identity);
                    }
                    match integer(program, *symbol, name.as_str())
                        .and_then(|value| usize::try_from(value).ok())
                    {
                        Some(value) => atom(ArrayLengthForm::Literal, &value.to_string()),
                        None => atom(
                            ArrayLengthForm::ConstParameter,
                            &nested.name(program, *symbol, name.as_str()),
                        ),
                    }
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    match program.expression_table.expression(*expression) {
                        ExpressionNode::Integer(value) => {
                            atom(ArrayLengthForm::Literal, &value.to_string())
                        }
                        _ => compound(
                            ArrayLengthForm::ConstExpression,
                            [normalize_index_expression(program, *expression, &nested)],
                        ),
                    }
                }
                _ => normalize_type_reference(program, reference, &nested),
            },
        )
    }

    pub(crate) fn rejected(context: &TypeIdentityContext<'_>) -> String {
        if let Some(invalid) = context.invalid_application {
            invalid.set(true);
        }
        if let Some(invalid) = context.invalid_index_argument {
            invalid.set(true);
        }
        if let Some(missing) = context.missing_exact_nominal_owner {
            missing.set(true);
        }
        bare(IndexForm::UnsupportedConstSubstitution)
    }

    fn boolean(program: &TypedTrees, symbol: SymbolHandle, spelling: &str) -> Option<bool> {
        let value = if symbol.is_valid() {
            let declaration = program.const_declaration_by_symbol(symbol)?;
            let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
            CanonicalConstValue::new("", encoding.to_owned(), "").decode_encoding()?
        } else {
            if let Some(value) = language_semantics::const_value::boolean_literal_spelling(spelling)
            {
                return Some(value);
            }
            CanonicalConstValue::from_atom(spelling)?.decode_encoding()?
        };
        match value {
            DecodedCanonicalConstValue::Boolean(value) => Some(value),
            _ => None,
        }
    }

    fn integer(program: &TypedTrees, symbol: SymbolHandle, spelling: &str) -> Option<i128> {
        if !symbol.is_valid() {
            if let Ok(value) = spelling.parse() {
                return Some(value);
            }
            let value = CanonicalConstValue::from_atom(spelling)?;
            return match value.identity().decode_encoding()? {
                DecodedCanonicalConstValue::Integer { value, .. } => Some(value),
                _ => None,
            };
        }
        let declaration = program.const_declaration_by_symbol(symbol)?;
        let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
        match CanonicalConstValue::new("", encoding.to_owned(), "").decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => Some(value),
            _ => None,
        }
    }

    impl TypedTrees {
        pub fn closed_array_extent(
            &self,
            length: &FixedArrayLength,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Option<usize> {
            let (mut symbol, mut spelling) = match length {
                FixedArrayLength::Literal(length) => return Some(*length),
                FixedArrayLength::ConstParameter { symbol, name } if symbol.is_valid() => {
                    (*symbol, name.as_str())
                }
                _ => return None,
            };
            let mut active = Vec::new();
            loop {
                let replacement = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| symbol.is_valid() && *parameter == symbol);
                let Some((_, reference)) = replacement else {
                    return usize::try_from(integer(self, symbol, spelling)?).ok();
                };
                if active.contains(&symbol) || !self.contains_type_reference(*reference) {
                    return None;
                }
                active.push(symbol);
                match self.type_reference_table.type_reference(*reference) {
                    TypeReferenceNode::Named {
                        symbol: selected,
                        name,
                    } => {
                        symbol = *selected;
                        spelling = name.as_str();
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        if !self.expression_table.expression_is_valid(*expression) {
                            return None;
                        }
                        match self.expression_table.expression(*expression) {
                            ExpressionNode::Integer(literal) => {
                                return usize::try_from(literal.value_u64()?).ok();
                            }
                            ExpressionNode::Name(path)
                                if !path.static_application.is_valid()
                                    && path.symbol.is_valid() =>
                            {
                                symbol = path.symbol;
                                spelling = "";
                            }
                            _ => return None,
                        }
                    }
                    _ => return None,
                }
            }
        }
    }
}

pub use callable_identity::{
    CallableIdentityId, CallableIdentityMemo, CallableIdentityTable, CallableParametersId,
    CallablePathId, CallableResultDispatchId,
};
pub use language_semantics::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
};
pub use lifetime_parameters::{
    LifetimeParameterIdentity, applied_lifetime_parameter_identity,
    applied_lifetime_parameter_identity_with_supplier, apply_resolved_lifetime_parameter_identity,
    declaration_lifetime_parameters, lifetime_parameter_identity, lifetime_parameter_name,
    portable_lifetime_parameter_identity,
};
pub use normalized_identities::{
    NormalizedDomainTerm, NormalizedResultDispatchSet, NormalizedTypeIdentity, TypeIdentityId,
    TypeIdentityTable,
};
pub use portable_machine_applications::portable_machine_application_identity;
pub use static_operand_identity::{
    DeclaredStaticOperandIdentityRequest, StaticOperandIdentityRequest,
};

use crate::TypedTrees;
use crate::data;
use crate::trait_definition::TraitDefinition;

/*
    Every program numbers type identities in its own table, and a program can
    hold another (its product declaration frontier). Under a canonical
    numbering, each program's fields are written under its own table's, and
    the enclosing one is restored after; see `TypeIdentityTable`'s `Record`.
*/
pub(crate) fn write_under_own_numbering(
    program: &TypedTrees,
    output: &mut compiler_storage::RecordWriter,
    write: impl FnOnce(&mut compiler_storage::RecordWriter) -> Result<(), compiler_storage::RecordError>,
) -> Result<(), compiler_storage::RecordError> {
    let Some(enclosing) = output.take_renumbering::<TypeIdentityId>() else {
        return write(output);
    };
    program.type_identities.renumber_canonically(output);
    let written = write(output);
    output.restore_renumbering::<TypeIdentityId>(Some(enclosing));
    written
}

fn program_intern_type_identity(program: &TypedTrees, identity: String) -> TypeIdentityId {
    program.type_identities.intern(&identity)
}
use crate::type_system::type_identity::constraint_identity::atom;
use crate::type_system::type_identity::constraint_identity::compound;
use crate::type_system::type_identity::coordinates::CoordinateTag;
use crate::type_system::type_identity::identity_context::SubstitutionSemantics;
use crate::type_system::type_identity::identity_context::TypeIdentityContext;
pub use crate::type_system::type_identity::identity_context::TypeIdentityQualification;
use crate::type_system::type_identity::identity_context::normalize_type_reference;
use crate::type_system::type_identity::result_dispatch_terms::collect_result_dispatch_terms;
use crate::types::TypeReferenceHandle;
use language_semantics::type_identity::vocabulary::{Label, TypeForm};
use std::cell::Cell;
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub enum IndexIdentityArgument<'argument> {
    TypeArgument(TypeReferenceHandle),
    TypeReference(TypeReferenceHandle),
    Expression(crate::expression::ExpressionHandle),
    Subject(SymbolHandle),
    FieldProjection {
        receiver: crate::expression::ExpressionHandle,
        field: SymbolHandle,
    },
    StaticArgument(&'argument crate::expression::StaticMachineArgument),
}

#[derive(Clone, Copy)]
pub struct AppliedTypeIdentityScope<'argument> {
    pub declaration: SymbolHandle,
    pub arguments: &'argument [(SymbolHandle, IndexIdentityArgument<'argument>)],
    pub lifetimes: &'argument [(String, String)],
}

pub fn applied_type_identity(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_type_identity(
        program,
        reference,
        lexical_owner,
        lexical_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
        &[],
    )
}

pub fn applied_type_identity_in_scope(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    receiving_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_type_identity(
        program,
        reference,
        receiving_owner,
        argument_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
        &[],
    )
}

pub fn applied_type_identity_in_region_scope(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    receiving_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    resolved_lifetimes: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_type_identity(
        program,
        reference,
        receiving_owner,
        argument_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
        resolved_lifetimes,
    )
}

pub fn applied_index_identity_in_region_scope(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    resolved_lifetimes: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_index_identity(
        program,
        argument,
        argument_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
        resolved_lifetimes,
    )
}

pub fn applied_declared_carrier_identity(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let identity = normalize_type_reference(
        program,
        reference,
        &TypeIdentityContext {
            declared_carrier: true,
            application_scopes: scopes,
            invalid_application: Some(&invalid),
            encountered_elided_regions: Some(&invalid),
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(program_intern_type_identity(program, identity))
}

pub fn applied_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_index_identity(
        program,
        argument,
        lexical_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
        &[],
    )
}

pub fn portable_applied_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_index_identity(
        program,
        argument,
        lexical_owner,
        scopes,
        TypeIdentityQualification::PackageQualified,
        &[],
    )
}

pub fn portable_applied_index_identity_in_region_scope(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    resolved_lifetimes: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_index_identity(
        program,
        argument,
        lexical_owner,
        scopes,
        TypeIdentityQualification::PackageQualified,
        resolved_lifetimes,
    )
}

fn qualified_applied_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    qualification: TypeIdentityQualification,
    resolved_lifetimes: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let identity = substitution::index_argument(
        program,
        argument,
        &TypeIdentityContext {
            application_scopes: scopes,
            resolved_lifetimes,
            application_owner: lexical_owner,
            qualification,
            invalid_application: Some(&invalid),
            encountered_elided_regions: Some(&invalid),
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(program_intern_type_identity(program, identity))
}

pub fn portable_applied_type_identity(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_type_identity(
        program,
        reference,
        lexical_owner,
        lexical_owner,
        scopes,
        TypeIdentityQualification::PackageQualified,
        &[],
    )
}

/// Descriptions retain declared lifetime relationships, including elision,
/// with the actual supplying scope and requested nominal qualification.
/// This produces descriptive data; portable/static clients keep their own
/// concrete-region requirement through the same normalization owner.
pub fn descriptive_type_identity_in_region_scope(
    program: &TypedTrees,
    request: TypeIdentityRequest<'_>,
    receiving_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    resolved_lifetimes: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let elided = Cell::new(false);
    let identity = normalize_type_reference(
        program,
        request.type_reference,
        &TypeIdentityContext {
            binders: request.binders,
            substitutions: request.substitutions,
            qualification: request.qualification,
            application_scopes: scopes,
            application_owner: receiving_owner,
            application_argument_owner: argument_owner,
            resolved_lifetimes,
            invalid_application: Some(&invalid),
            // Elision remains explicit descriptive information. Missing named
            // scopes and malformed applications still set `invalid`.
            encountered_elided_regions: Some(&elided),
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then(|| program_intern_type_identity(program, identity))
}

fn qualified_applied_type_identity(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    qualification: TypeIdentityQualification,
    resolved_lifetimes: &[(LifetimeParameterIdentity, LifetimeParameterIdentity)],
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let identity = normalize_type_reference(
        program,
        reference,
        &TypeIdentityContext {
            application_scopes: scopes,
            resolved_lifetimes,
            application_owner: lexical_owner,
            application_argument_owner: argument_owner,
            qualification,
            invalid_application: Some(&invalid),
            encountered_elided_regions: Some(&invalid),
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(program_intern_type_identity(program, identity))
}

pub fn applied_lifetime_identity(
    program: &TypedTrees,
    name: &str,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_lifetime_identity(
        program,
        name,
        lexical_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
    )
}

pub fn applied_type_lifetime_identity_in_scope(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    ordinal: usize,
    receiving_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    if !program.contains_type_reference(reference) {
        return None;
    }
    let crate::types::TypeReferenceNode::Generic {
        lifetime_arguments, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let name = lifetime_arguments.get(ordinal)?;
    let invalid = Cell::new(false);
    let context = TypeIdentityContext {
        application_scopes: scopes,
        application_owner: receiving_owner,
        application_argument_owner: argument_owner,
        invalid_application: Some(&invalid),
        encountered_elided_regions: Some(&invalid),
        ..TypeIdentityContext::default()
    }
    .enter_type_operand(program, reference);
    let identity = context.lifetime(program, name.as_str());
    (!invalid.get()).then(|| program_intern_type_identity(program, identity))
}

pub fn portable_applied_lifetime_identity(
    program: &TypedTrees,
    name: &str,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_lifetime_identity(
        program,
        name,
        lexical_owner,
        scopes,
        TypeIdentityQualification::PackageQualified,
    )
}

fn qualified_applied_lifetime_identity(
    program: &TypedTrees,
    name: &str,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    qualification: TypeIdentityQualification,
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let identity = TypeIdentityContext {
        application_scopes: scopes,
        application_owner: lexical_owner,
        qualification,
        invalid_application: Some(&invalid),
        encountered_elided_regions: Some(&invalid),
        ..TypeIdentityContext::default()
    }
    .lifetime(program, name);
    (!invalid.get()).then_some(program_intern_type_identity(program, identity))
}

pub(crate) fn local_trait_application_identity(
    program: &TypedTrees,
    declaration: SymbolHandle,
    arguments: &[NormalizedTypeIdentity],
    lifetimes: &[NormalizedTypeIdentity],
) -> Option<NormalizedTypeIdentity> {
    let render = |identities: &[NormalizedTypeIdentity]| {
        identities
            .iter()
            .map(|identity| program.type_identity_text(*identity))
            .collect::<Option<Vec<_>>>()
    };
    let identity = compound(
        CoordinateTag::LocalTraitApplication,
        [
            atom(
                CoordinateTag::Declaration,
                &format!("{}:{}", declaration.arena_index(), declaration.generation()),
            ),
            compound(CoordinateTag::Arguments, render(arguments)?),
            compound(CoordinateTag::Lifetimes, render(lifetimes)?),
        ],
    );
    Some(program_intern_type_identity(program, identity))
}

pub(crate) fn local_trait_contract_identity(
    program: &TypedTrees,
    base: NormalizedTypeIdentity,
    refinements: &[NormalizedTypeIdentity],
) -> Option<NormalizedTypeIdentity> {
    if refinements.is_empty() {
        return Some(base);
    }
    let mut refinements = refinements.to_vec();
    refinements.sort();
    refinements.dedup();
    let refinements = refinements
        .iter()
        .map(|identity| program.type_identity_text(*identity))
        .collect::<Option<Vec<_>>>()?;
    Some(program_intern_type_identity(
        program,
        compound(
            CoordinateTag::LocalTraitContract,
            [
                program.type_identity_text(base)?,
                compound(CoordinateTag::Refinements, refinements),
            ],
        ),
    ))
}

pub(crate) fn portable_trait_application_identity(
    program: &TypedTrees,
    declaration: SymbolHandle,
    arguments: &[NormalizedTypeIdentity],
    lifetimes: &[NormalizedTypeIdentity],
) -> Option<NormalizedTypeIdentity> {
    let path = program.symbols.display_path(declaration, "::");
    if !declaration.is_valid() || path.is_empty() {
        return None;
    }
    let coordinate = TypeIdentityContext {
        qualification: TypeIdentityQualification::PackageQualified,
        ..TypeIdentityContext::default()
    }
    .qualify_non_binder_name(program, declaration, path);
    let render = |identities: &[NormalizedTypeIdentity]| {
        identities
            .iter()
            .map(|identity| program.type_identity_text(*identity))
            .collect::<Option<Vec<_>>>()
    };
    Some(program_intern_type_identity(
        program,
        compound(
            CoordinateTag::TraitApplication,
            [
                atom(CoordinateTag::Declaration, &coordinate),
                compound(CoordinateTag::Arguments, render(arguments)?),
                compound(CoordinateTag::Lifetimes, render(lifetimes)?),
            ],
        ),
    ))
}

pub fn normalized_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
) -> Option<String> {
    let invalid = Cell::new(false);
    let identity = substitution::index_argument(
        program,
        argument,
        &TypeIdentityContext {
            index_substitutions: substitutions,
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(identity)
}

pub(crate) fn normalized_const_index_expression_identity(
    program: &TypedTrees,
    expression: crate::expression::ExpressionHandle,
) -> Option<String> {
    let invalid = Cell::new(false);
    let identity = identity_context::normalize_index_expression(
        program,
        expression,
        &identity_context::TypeIdentityContext {
            invalid_index_argument: Some(&invalid),
            ..identity_context::TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(identity)
}

pub fn normalized_expression_identity(
    program: &TypedTrees,
    expression: crate::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<String> {
    try_normalized_index_identity(
        program,
        IndexIdentityArgument::Expression(expression),
        binders,
        &[],
    )
}

pub fn try_normalized_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    binders: &[(SymbolHandle, String)],
    application_scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<String> {
    let invalid = Cell::new(false);
    let identity = substitution::index_argument(
        program,
        argument,
        &TypeIdentityContext {
            binders,
            application_scopes,
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(identity)
}

#[derive(Clone, Copy)]
pub struct TypeIdentityRequest<'a> {
    pub type_reference: TypeReferenceHandle,
    pub binders: &'a [(SymbolHandle, String)],
    pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
    pub qualification: TypeIdentityQualification,
}

impl TypeIdentityRequest<'_> {
    pub const fn ordinary(type_reference: TypeReferenceHandle) -> Self {
        Self {
            type_reference,
            binders: &[],
            substitutions: &[],
            qualification: TypeIdentityQualification::Ordinary,
        }
    }

    pub const fn package_qualified(type_reference: TypeReferenceHandle) -> Self {
        Self {
            type_reference,
            binders: &[],
            substitutions: &[],
            qualification: TypeIdentityQualification::PackageQualified,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ExactOwnerTypeIdentityRequest<'a> {
    pub type_reference: TypeReferenceHandle,
    pub binders: &'a [(SymbolHandle, String)],
    pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
}

impl TypedTrees {
    pub fn data_field_definition(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<&crate::data::DataDefinition> {
        let reference = self
            .type_reference_table
            .unconstrained_type_reference(reference);
        self.closed_data_definition(reference)
            .or_else(|| self.data_application_definition(reference))
    }

    pub fn closed_data_field_definition(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<&data::DataDefinition> {
        self.closed_data_definition(
            self.type_reference_table
                .unconstrained_type_reference(reference),
        )
    }

    pub fn closed_data_definition(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<&data::DataDefinition> {
        use crate::types::TypeReferenceNode;

        if !self.type_reference_table.contains_type_reference(reference) {
            return None;
        }
        if let Some(application) = self.data_application_definition(reference) {
            return self
                .data_application_is_closed(reference)
                .then_some(application);
        }
        let data = match self.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                self.unique_data_definition_by_symbol(*symbol)?
            }
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } => {
                let owner = self.unique_data_definition_by_symbol(*base_symbol)?;
                if lifetime_arguments.len() != owner.lifetime_parameters.len() {
                    return None;
                }
                let arguments = self.type_reference_table.type_reference_handles(*arguments);
                if arguments.is_empty() {
                    owner
                } else {
                    if arguments.len() != self.data_type_parameters(owner).len() {
                        return None;
                    }
                    let identity = self.normalized_type_identity(reference);
                    let mut instances = self.data_definitions().iter().filter(|data| {
                        let Some(origin) = data.generic_instance else {
                            return false;
                        };
                        self.type_reference_table.contains_type_reference(origin)
                            && matches!(self.type_reference_table.type_reference(origin),
                                TypeReferenceNode::Generic { base_symbol: origin_owner, lifetime_arguments, .. }
                                    if origin_owner == base_symbol && lifetime_arguments.is_empty())
                            && self.normalized_type_identity(origin) == identity
                            && data.lifetime_parameters == owner.lifetime_parameters
                    });
                    let instance = instances.next()?;
                    if instances.next().is_some() {
                        return None;
                    }
                    self.unique_data_definition_by_symbol(instance.symbol)?
                }
            }
            _ => return None,
        };
        data.type_parameters.is_empty().then_some(data)
    }

    pub fn closed_data_constructor_definition(
        &self,
        reference: TypeReferenceHandle,
        authored: SymbolHandle,
    ) -> Option<&data::DataDefinition> {
        self.closed_data_definition(reference)?;
        self.data_constructor_definition(reference, authored)
    }

    pub fn data_constructor_definition(
        &self,
        reference: TypeReferenceHandle,
        authored: SymbolHandle,
    ) -> Option<&data::DataDefinition> {
        if !self.type_reference_table.contains_type_reference(reference) {
            return None;
        }
        let reference = self
            .type_reference_table
            .unconstrained_type_reference(reference);
        if let Some(storage) = self.layout_storage_arguments(reference) {
            return self.data_constructor_definition(
                self.type_reference_table
                    .unconstrained_type_reference(storage.semantic_type),
                authored,
            );
        }
        let instance = self
            .data_application_definition(reference)
            .or_else(|| self.closed_data_definition(reference))?;
        if authored == instance.symbol {
            return Some(instance);
        }
        let identity = self.normalized_type_identity(reference);
        let declaration = |symbol| {
            let nominal = self.unique_data_definition_by_symbol(symbol)?;
            let Some(origin) = nominal.generic_instance else {
                return Some(symbol);
            };
            if !self.type_reference_table.contains_type_reference(origin) {
                return None;
            }
            let crate::types::TypeReferenceNode::Generic { base_symbol, .. } =
                self.type_reference_table.type_reference(origin)
            else {
                return None;
            };
            (self.normalized_type_identity(origin) == identity).then_some(*base_symbol)
        };
        (declaration(instance.symbol)? == declaration(authored)?).then_some(instance)
    }

    pub fn closed_data_constructor_case(
        &self,
        reference: TypeReferenceHandle,
        retained: SymbolHandle,
    ) -> Option<&data::DataVariant> {
        use data::DataMember;
        if !retained.is_valid() || self.symbols.get(retained).kind != symbols::SymbolKind::Variant {
            return None;
        }
        let owner = self.symbols.get(retained).parent;
        let instance = self.closed_data_constructor_definition(reference, owner)?;
        let source = self.unique_data_definition_by_symbol(owner)?;
        let (ordinal, original) = self.data_members(source).iter().enumerate().find_map(
            |(ordinal, member)| match member {
                DataMember::Variant(variant) if variant.symbol == retained => {
                    Some((ordinal, variant))
                }
                _ => None,
            },
        )?;
        let DataMember::Variant(selected) = self.data_members(instance).get(ordinal)? else {
            return None;
        };
        (original.name == selected.name && original.identity == selected.identity)
            .then_some(selected)
    }

    pub fn closed_data_constructor_field(
        &self,
        reference: TypeReferenceHandle,
        authored: SymbolHandle,
        retained: SymbolHandle,
    ) -> Option<&data::DataField> {
        self.closed_data_definition(reference)?;
        self.data_constructor_field(reference, authored, retained)
    }

    pub fn data_constructor_case(
        &self,
        reference: TypeReferenceHandle,
        authored: SymbolHandle,
        retained: SymbolHandle,
    ) -> Option<&data::DataVariant> {
        use data::DataMember;
        let instance = self.data_constructor_definition(reference, authored)?;
        if !retained.is_valid() || self.symbols.get(retained).kind != symbols::SymbolKind::Variant {
            return None;
        }
        let source_owner = self.symbols.get(retained).parent;
        self.data_constructor_definition(reference, source_owner)?;
        let source = self.unique_data_definition_by_symbol(source_owner)?;
        let (ordinal, original) = self.data_members(source).iter().enumerate().find_map(
            |(ordinal, member)| match member {
                DataMember::Variant(case) if case.symbol == retained => Some((ordinal, case)),
                _ => None,
            },
        )?;
        let DataMember::Variant(selected) = self.data_members(instance).get(ordinal)? else {
            return None;
        };
        (original.name == selected.name
            && original.identity == selected.identity
            && selected.symbol.is_valid())
        .then_some(selected)
    }

    pub fn data_constructor_field(
        &self,
        reference: TypeReferenceHandle,
        authored: SymbolHandle,
        retained: SymbolHandle,
    ) -> Option<&data::DataField> {
        use data::DataMember;
        let instance = self.data_constructor_definition(reference, authored)?;
        if !retained.is_valid() || self.symbols.get(retained).kind != symbols::SymbolKind::Field {
            return None;
        }
        let parent = self.symbols.get(retained).parent;
        let source_owner = if self.symbols.get(parent).kind == symbols::SymbolKind::Variant {
            self.symbols.get(parent).parent
        } else {
            parent
        };
        self.data_constructor_definition(reference, source_owner)?;
        let source = self.unique_data_definition_by_symbol(source_owner)?;
        let corresponding_field = |original: &data::DataField, selected: &data::DataField| {
            original.name == selected.name
                && original.identity == selected.identity
                && original.relevance == selected.relevance
                && selected.symbol.is_valid()
                && self.contains_type_reference(selected.type_reference)
        };
        for (ordinal, member) in self.data_members(source).iter().enumerate() {
            match member {
                DataMember::Field(original) if original.symbol == retained => {
                    let DataMember::Field(selected) = self.data_members(instance).get(ordinal)?
                    else {
                        return None;
                    };
                    return corresponding_field(original, selected).then_some(selected);
                }
                DataMember::Variant(original_case) => {
                    let Some((field_ordinal, original)) = self
                        .data_payload_fields(original_case)
                        .iter()
                        .enumerate()
                        .find(|(_, field)| field.symbol == retained)
                    else {
                        continue;
                    };
                    let DataMember::Variant(selected_case) =
                        self.data_members(instance).get(ordinal)?
                    else {
                        return None;
                    };
                    if original_case.name != selected_case.name
                        || original_case.identity != selected_case.identity
                    {
                        return None;
                    }
                    let selected = self.data_payload_fields(selected_case).get(field_ordinal)?;
                    return corresponding_field(original, selected).then_some(selected);
                }
                _ => {}
            }
        }
        None
    }

    pub fn closed_data_member_field(
        &self,
        reference: TypeReferenceHandle,
        retained: SymbolHandle,
        member_name: &str,
    ) -> Option<&data::DataField> {
        let instance = self.closed_data_definition(reference)?;
        let mut fields =
            self.data_members(instance)
                .iter()
                .enumerate()
                .filter_map(|(ordinal, member)| {
                    let data::DataMember::Field(field) = member else {
                        return None;
                    };
                    (field.name.as_str() == member_name).then_some((ordinal, field))
                });
        let (ordinal, field) = fields.next()?;
        if fields.next().is_some() {
            return None;
        }
        if !field.symbol.is_valid()
            || !self.contains_type_reference(field.type_reference)
            || self.symbols.get(field.symbol).parent != instance.symbol
        {
            return None;
        }
        if retained == field.symbol {
            return Some(field);
        }
        let origin = instance.generic_instance?;
        if !self.type_reference_table.contains_type_reference(origin) {
            return None;
        }
        let crate::types::TypeReferenceNode::Generic { base_symbol, .. } =
            self.type_reference_table.type_reference(origin)
        else {
            return None;
        };
        if self.closed_data_definition(origin)?.symbol != instance.symbol {
            return None;
        }
        let template = self.unique_data_definition_by_symbol(*base_symbol)?;
        let data::DataMember::Field(original) = self.data_members(template).get(ordinal)? else {
            return None;
        };
        (field.name == original.name
            && field.identity == original.identity
            && field.relevance == original.relevance
            && original.symbol.is_valid()
            && self.symbols.get(original.symbol).parent == template.symbol
            && (!retained.is_valid() || retained == field.symbol || retained == original.symbol))
            .then_some(field)
    }

    fn data_field_declaration_symbol(
        &self,
        mut symbol: SymbolHandle,
    ) -> Result<SymbolHandle, String> {
        if !symbol.is_valid() || self.symbols.get(symbol).kind != symbols::SymbolKind::Field {
            return Ok(symbol);
        }
        for _ in 0..=self.data_definitions().len() {
            let parent = self.symbols.get(symbol).parent;
            if !parent.is_valid() {
                return Err("a field declaration has no retained owner".to_owned());
            }
            let owner = if self.symbols.get(parent).kind == symbols::SymbolKind::Variant {
                self.symbols.get(parent).parent
            } else {
                parent
            };
            if !owner.is_valid() || self.symbols.get(owner).kind != symbols::SymbolKind::Data {
                return Ok(symbol);
            }
            let data = self
                .unique_data_definition_by_symbol(owner)
                .ok_or_else(|| "a field has no unique retained declaration owner".to_owned())?;
            let Some(origin) = data.generic_instance else {
                return Ok(symbol);
            };
            if !self.contains_type_reference(origin) {
                return Err("a specialized field lost its owner application".to_owned());
            }
            if self
                .closed_data_definition(origin)
                .is_none_or(|instance| instance.symbol != data.symbol)
            {
                return Err(
                    "a specialized field origin does not retain its exact data instance".to_owned(),
                );
            }
            let crate::types::TypeReferenceNode::Generic { base_symbol, .. } =
                self.type_reference_table.type_reference(origin)
            else {
                return Err("a specialized field owner is not a generic application".to_owned());
            };
            let template = self
                .unique_data_definition_by_symbol(*base_symbol)
                .ok_or_else(|| {
                    "a specialized field has no unique original declaration owner".to_owned()
                })?;
            let mut correspondence = None;
            for (ordinal, member) in self.data_members(data).iter().enumerate() {
                match member {
                    data::DataMember::Field(selected) if selected.symbol == symbol => {
                        if let Some(data::DataMember::Field(original)) =
                            self.data_members(template).get(ordinal)
                        {
                            correspondence = Some((selected, original, template.symbol));
                        }
                        break;
                    }
                    data::DataMember::Variant(selected_case) if selected_case.symbol == parent => {
                        if let Some(data::DataMember::Variant(original_case)) =
                            self.data_members(template).get(ordinal)
                            && selected_case.name == original_case.name
                            && selected_case.identity == original_case.identity
                            && let Some((field_ordinal, selected)) = self
                                .data_payload_fields(selected_case)
                                .iter()
                                .enumerate()
                                .find(|(_, field)| field.symbol == symbol)
                            && let Some(original) =
                                self.data_payload_fields(original_case).get(field_ordinal)
                        {
                            correspondence = Some((selected, original, original_case.symbol));
                        }
                        break;
                    }
                    _ => {}
                }
            }
            let Some((selected, original, original_parent)) = correspondence else {
                return Err("a specialized field lost its original member coordinate".to_owned());
            };
            if selected.name != original.name
                || selected.identity != original.identity
                || selected.relevance != original.relevance
                || !original.symbol.is_valid()
                || self.symbols.get(original.symbol).parent != original_parent
                || !self
                    .symbols
                    .same_symbol_source_package(symbol, original.symbol)
            {
                return Err(
                    "a specialized field changed its original declaration custody".to_owned(),
                );
            }
            symbol = original.symbol;
        }
        Err("a field declaration has a cyclic specialization origin".to_owned())
    }

    pub fn normalized_hermetic_symbol_identity(
        &self,
        symbol: SymbolHandle,
    ) -> Result<String, String> {
        let symbol = self.data_field_declaration_symbol(symbol)?;
        if !symbol.is_valid() {
            return Err("a hermetic identity contains an unresolved declaration".to_owned());
        }
        if let Some(identity) = derived_declarations::nominal_name(
            self,
            symbol,
            &TypeIdentityContext {
                qualification: TypeIdentityQualification::PackageQualified,
                ..Default::default()
            },
            true,
        )
        .map_err(str::to_owned)?
        {
            return Ok(identity);
        }
        let path = self.symbols.display_path(symbol, "::");
        if let Some(package) = self.symbols.symbol_package_identity(symbol) {
            let mut owner = String::with_capacity(64);
            use std::fmt::Write as _;
            for byte in package.digest() {
                let _ = write!(owner, "{byte:02x}");
            }
            return Ok(format!("package:{owner}::{path}"));
        }
        match self.symbols.symbol_source_origin(symbol) {
            Some(source::SourceOrigin::Toolchain) => Ok(format!("toolchain::{path}")),
            Some(source::SourceOrigin::ToolchainContract) => {
                Ok(format!("toolchain-contract::{path}"))
            }
            Some(origin) => Err(format!(
                "declaration `{path}` has non-hermetic source origin `{origin:?}`"
            )),
            None => Err(format!(
                "declaration `{path}` has no retained source/package provenance"
            )),
        }
    }

    pub fn normalized_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        self.type_identity(TypeIdentityRequest::ordinary(type_reference))
    }

    pub fn type_identity_text(&self, identity: TypeIdentityId) -> Option<String> {
        self.type_identities.text(identity).map(str::to_owned)
    }

    pub fn requirement_argument_identity_text(
        &self,
        argument: crate::RequirementLocalArgument,
        identity: NormalizedTypeIdentity,
    ) -> Option<String> {
        let text = self.type_identity_text(identity)?;
        Some(match argument {
            crate::RequirementLocalArgument::Static(_) => text,
            crate::RequirementLocalArgument::RuntimeValue { .. } => {
                atom(CoordinateTag::RuntimeValueCarrier, &text)
            }
        })
    }

    pub fn normalized_type_identity_text(&self, type_reference: TypeReferenceHandle) -> String {
        self.type_identity_text(self.normalized_type_identity(type_reference))
            .unwrap_or_default()
    }

    pub fn type_identity_by_text(&self, text: &str) -> TypeIdentityId {
        self.type_identities.by_text(text)
    }

    pub fn normalized_nominal_type_identity(
        &self,
        symbol: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        if !symbol.is_valid() || self.symbols.display_path(symbol, "::").is_empty() {
            return None;
        }
        derived_declarations::nominal_name(self, symbol, &TypeIdentityContext::default(), false)
            .ok()?;
        if let Some(origin) = self
            .data_definition_by_symbol(symbol)
            .and_then(|data| data.generic_instance)
        {
            return Some(self.normalized_type_identity(origin));
        }
        Some(program_intern_type_identity(
            self,
            compound(
                TypeForm::Named,
                [identity_context::normalize_const_or_nominal_name(
                    self,
                    symbol,
                    "",
                    Label::Name,
                    &TypeIdentityContext::default(),
                )],
            ),
        ))
    }

    pub fn package_qualified_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        self.type_identity(TypeIdentityRequest::package_qualified(type_reference))
    }

    pub fn package_qualified_type_identity_text(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> String {
        self.type_identity_text(self.package_qualified_type_identity(type_reference))
            .unwrap_or_default()
    }

    pub fn type_identity_request_text(&self, request: TypeIdentityRequest<'_>) -> String {
        self.type_identity_text(self.type_identity(request))
            .unwrap_or_default()
    }

    pub fn fixed_array_length_identity(
        &self,
        length: &crate::types::FixedArrayLength,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<NormalizedTypeIdentity> {
        self.fixed_array_length_identity_with_substitution_semantics(
            length,
            binders,
            substitutions,
            SubstitutionSemantics::Transitive,
            &[],
        )
    }

    pub fn fixed_array_length_identity_with_simultaneous_substitutions(
        &self,
        length: &crate::types::FixedArrayLength,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<NormalizedTypeIdentity> {
        self.fixed_array_length_identity_with_substitution_semantics(
            length,
            &[],
            substitutions,
            SubstitutionSemantics::Simultaneous,
            &[],
        )
    }

    pub fn scoped_fixed_array_length_identity(
        &self,
        length: &crate::types::FixedArrayLength,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
    ) -> Option<NormalizedTypeIdentity> {
        let mut subjects = match length {
            crate::types::FixedArrayLength::ConstParameter { symbol, .. } => vec![*symbol],
            crate::types::FixedArrayLength::Expression(reference) => {
                if !self.contains_type_reference(*reference) {
                    return None;
                }
                crate::references::collect_type_symbol_dependencies(self, *reference)
            }
            _ => Vec::new(),
        };
        for (_, argument) in substitutions {
            if !self.contains_type_reference(*argument) {
                return None;
            }
            subjects.extend(crate::references::collect_type_symbol_dependencies(
                self, *argument,
            ));
        }
        subjects.retain(|symbol| {
            symbol.is_valid()
                && matches!(
                    self.symbols.get(*symbol).kind,
                    symbols::SymbolKind::TypeParameter
                        | symbols::SymbolKind::Parameter
                        | symbols::SymbolKind::Local
                        | symbols::SymbolKind::MachineParameter
                        | symbols::SymbolKind::ConformanceParameter
                        | symbols::SymbolKind::PropositionParameter
                        | symbols::SymbolKind::PropositionMachineParameter
                )
        });
        subjects.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        subjects.dedup();
        let binders = subjects
            .into_iter()
            .map(|symbol| {
                (
                    symbol,
                    format!(
                        "array-index:{}:{}",
                        symbol.arena_index(),
                        symbol.generation()
                    ),
                )
            })
            .collect::<Vec<_>>();
        self.fixed_array_length_identity_with_substitution_semantics(
            length,
            &binders,
            substitutions,
            SubstitutionSemantics::Transitive,
            scope_starts,
        )
    }

    fn fixed_array_length_identity_with_substitution_semantics(
        &self,
        length: &crate::types::FixedArrayLength,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        substitution_semantics: SubstitutionSemantics,
        scope_starts: &[usize],
    ) -> Option<NormalizedTypeIdentity> {
        if let crate::types::FixedArrayLength::Expression(reference) = length
            && (!self.contains_type_reference(*reference)
                || !matches!(
                    self.type_reference_table.type_reference(*reference),
                    crate::types::TypeReferenceNode::ConstExpression(_)
                ))
        {
            return None;
        }
        let invalid = Cell::new(false);
        let identity = constraint_identity::normalize_array_length(
            self,
            length,
            &TypeIdentityContext {
                binders,
                substitutions,
                substitution_semantics,
                substitution_scope_starts: scope_starts,
                invalid_index_argument: Some(&invalid),
                ..Default::default()
            },
        );
        (!invalid.get()).then(|| program_intern_type_identity(self, identity))
    }

    pub fn type_identity(&self, request: TypeIdentityRequest<'_>) -> NormalizedTypeIdentity {
        self.type_identity_with_substitution_semantics(
            request,
            SubstitutionSemantics::Transitive,
            &[],
        )
    }

    pub fn type_identities_match_in_static_context(
        &self,
        expression: crate::expression::ExpressionHandle,
        actual: TypeIdentityRequest<'_>,
        required: TypeIdentityRequest<'_>,
    ) -> bool {
        if self.static_type_context_is_unreachable(expression) {
            return true;
        }
        let normalize = |request: TypeIdentityRequest<'_>, semantics| {
            normalize_type_reference(
                self,
                request.type_reference,
                &TypeIdentityContext {
                    binders: request.binders,
                    substitutions: request.substitutions,
                    qualification: request.qualification,
                    substitution_semantics: semantics,
                    static_context: expression,
                    ..TypeIdentityContext::default()
                },
            )
        };
        normalize(actual, SubstitutionSemantics::Transitive)
            == normalize(required, SubstitutionSemantics::Simultaneous)
    }

    pub fn storage_type_identity(
        &self,
        request: TypeIdentityRequest<'_>,
    ) -> NormalizedTypeIdentity {
        program_intern_type_identity(
            self,
            normalize_type_reference(
                self,
                request.type_reference,
                &TypeIdentityContext {
                    storage_value_indices: true,
                    binders: request.binders,
                    substitutions: request.substitutions,
                    qualification: request.qualification,
                    ..TypeIdentityContext::default()
                },
            ),
        )
    }

    pub fn type_identity_with_simultaneous_substitutions(
        &self,
        request: TypeIdentityRequest<'_>,
    ) -> NormalizedTypeIdentity {
        self.type_identity_with_substitution_semantics(
            request,
            SubstitutionSemantics::Simultaneous,
            &[],
        )
    }

    pub fn scoped_type_identity(
        &self,
        request: TypeIdentityRequest<'_>,
        scope_starts: &[usize],
    ) -> NormalizedTypeIdentity {
        self.type_identity_with_substitution_semantics(
            request,
            SubstitutionSemantics::Transitive,
            scope_starts,
        )
    }

    pub fn scoped_type_identity_with_index_arguments(
        &self,
        request: TypeIdentityRequest<'_>,
        scope_starts: &[usize],
        arguments: &[(SymbolHandle, IndexIdentityArgument<'_>)],
    ) -> Option<NormalizedTypeIdentity> {
        let invalid = Cell::new(false);
        let regions = Cell::new(false);
        let identity = normalize_type_reference(
            self,
            request.type_reference,
            &TypeIdentityContext {
                binders: request.binders,
                substitutions: request.substitutions,
                substitution_scope_starts: scope_starts,
                index_substitutions: arguments,
                invalid_index_argument: Some(&invalid),
                invalid_application: Some(&invalid),
                encountered_elided_regions: Some(&invalid),
                encountered_regions: Some(&regions),
                qualification: request.qualification,
                ..TypeIdentityContext::default()
            },
        );
        (!invalid.get() && !regions.get()).then_some(program_intern_type_identity(self, identity))
    }

    pub fn region_free_scoped_type_identity(
        &self,
        request: TypeIdentityRequest<'_>,
        scope_starts: &[usize],
    ) -> Option<NormalizedTypeIdentity> {
        if !self.contains_type_reference(request.type_reference) {
            return None;
        }
        let encountered_regions = Cell::new(false);
        let identity = normalize_type_reference(
            self,
            request.type_reference,
            &TypeIdentityContext {
                binders: request.binders,
                substitutions: request.substitutions,
                substitution_scope_starts: scope_starts,
                encountered_regions: Some(&encountered_regions),
                qualification: request.qualification,
                ..TypeIdentityContext::default()
            },
        );
        (!encountered_regions.get()).then(|| program_intern_type_identity(self, identity))
    }

    fn type_identity_with_substitution_semantics(
        &self,
        request: TypeIdentityRequest<'_>,
        substitution_semantics: SubstitutionSemantics,
        substitution_scope_starts: &[usize],
    ) -> NormalizedTypeIdentity {
        program_intern_type_identity(
            self,
            normalize_type_reference(
                self,
                request.type_reference,
                &TypeIdentityContext {
                    binders: request.binders,
                    substitutions: request.substitutions,
                    substitution_semantics,
                    substitution_scope_starts,
                    active_const_substitutions: &[],
                    exact_toolchain_sources: &[],
                    missing_exact_nominal_owner: None,
                    qualification: request.qualification,
                    ..TypeIdentityContext::default()
                },
            ),
        )
    }

    pub fn exact_owner_type_identity(
        &self,
        request: ExactOwnerTypeIdentityRequest<'_>,
    ) -> Option<NormalizedTypeIdentity> {
        let missing_exact_nominal_owner = Cell::new(false);
        let identity = program_intern_type_identity(
            self,
            normalize_type_reference(
                self,
                request.type_reference,
                &TypeIdentityContext {
                    binders: request.binders,
                    substitutions: request.substitutions,
                    substitution_semantics: SubstitutionSemantics::Transitive,
                    active_const_substitutions: &[],
                    exact_toolchain_sources: request.exact_toolchain_sources,
                    missing_exact_nominal_owner: Some(&missing_exact_nominal_owner),
                    qualification: TypeIdentityQualification::PackageQualified,
                    ..TypeIdentityContext::default()
                },
            ),
        );
        (!missing_exact_nominal_owner.get()).then_some(identity)
    }

    pub fn exact_owner_nominal_identity(
        &self,
        symbol: SymbolHandle,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Option<NormalizedTypeIdentity> {
        if !symbol.is_valid() {
            return None;
        }
        let path = self.symbols.display_path(symbol, "::");
        if path.is_empty() {
            return None;
        }
        let missing_exact_nominal_owner = Cell::new(false);
        let identity = program_intern_type_identity(
            self,
            TypeIdentityContext {
                binders: &[],
                substitutions: &[],
                substitution_semantics: SubstitutionSemantics::Transitive,
                active_const_substitutions: &[],
                exact_toolchain_sources,
                missing_exact_nominal_owner: Some(&missing_exact_nominal_owner),
                qualification: TypeIdentityQualification::PackageQualified,
                ..TypeIdentityContext::default()
            }
            .qualify_non_binder_name(self, symbol, path),
        );
        (!missing_exact_nominal_owner.get()).then_some(identity)
    }

    pub fn normalized_result_dispatch_set(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedResultDispatchSet {
        let mut terms = Vec::new();
        collect_result_dispatch_terms(self, type_reference, &mut terms, &mut Vec::new());
        terms.sort();
        terms.dedup();
        NormalizedResultDispatchSet { terms }
    }

    fn machine_callable_identity_parts(
        &self,
        machine: &crate::machine::Machine,
    ) -> Option<callable_identity::CallableIdentityParts> {
        let entry = self.machine_states(machine).first()?;
        let module = self.symbols.symbol_module(machine.symbol);
        let parent = self.symbols.get(machine.symbol).parent;
        let lexical_path =
            if parent.is_valid() && self.symbols.get(parent).kind == symbols::SymbolKind::Machine {
                let owner = self.machine_by_symbol(parent)?;
                compound(
                    CoordinateTag::NestedMachine,
                    [
                        atom(
                            CoordinateTag::Owner,
                            &self.machine_callable_identity_text(owner)?,
                        ),
                        atom(CoordinateTag::Name, machine.name.as_str()),
                    ],
                )
            } else if module.is_valid() {
                format!(
                    "{}::{}",
                    self.symbols.display_path(module, "::"),
                    machine.name
                )
            } else {
                machine.name.as_str().to_owned()
            };
        Some(self.named_callable_identity_parts(
            &lexical_path,
            machine.symbol,
            self.machine_type_parameters(machine),
            self.state_parameters(entry),
            entry.return_type,
        ))
    }

    pub fn machine_entry_result_type_identity(
        &self,
        machine: &crate::machine::Machine,
    ) -> Option<NormalizedTypeIdentity> {
        let entry = self.machine_states(machine).first()?;
        let binders = callable_type_binders(machine.symbol, self.machine_type_parameters(machine));
        Some(self.type_identity(TypeIdentityRequest {
            binders: &binders,
            ..TypeIdentityRequest::ordinary(entry.return_type)
        }))
    }

    pub fn attached_data_path(&self, machine: &crate::machine::Machine) -> Option<String> {
        let name = machine.attached_data.as_ref()?;
        Some(
            if self
                .symbols
                .symbol_module(machine.attached_data_symbol)
                .is_valid()
            {
                self.symbols
                    .display_path(machine.attached_data_symbol, "::")
            } else {
                name.as_str().to_owned()
            },
        )
    }

    pub fn trait_declaration_path(&self, trait_definition: &TraitDefinition) -> String {
        if self
            .symbols
            .symbol_module(trait_definition.symbol)
            .is_valid()
        {
            self.symbols.display_path(trait_definition.symbol, "::")
        } else {
            trait_definition.name.as_str().to_owned()
        }
    }

    fn trait_requirement_callable_identity_parts(
        &self,
        trait_definition: &TraitDefinition,
        requirement: &crate::signature::StateSignature,
    ) -> callable_identity::CallableIdentityParts {
        let mut type_parameters = self.trait_type_parameters(trait_definition).to_vec();
        type_parameters.extend_from_slice(self.state_signature_type_parameters(requirement));
        let mut identity = self.named_callable_identity_parts(
            &format!(
                "{}::{}",
                self.trait_declaration_path(trait_definition),
                requirement.name
            ),
            trait_definition.symbol,
            &type_parameters,
            self.state_signature_parameters(requirement),
            requirement.return_type,
        );
        if let Some(application) = self.closed_requirement_application(requirement.symbol) {
            let template =
                self.trait_machine_signature_by_symbol(trait_definition, application.template);
            let template_identity = template
                .map(|template| {
                    self.trait_requirement_callable_identity_text(trait_definition, template)
                })
                .unwrap_or_default();
            let outer = self
                .type_identity_text(application.portable_trait_application_identity)
                .unwrap_or_default();
            let local = application
                .portable_local_arguments
                .iter()
                .zip(application.local_arguments)
                .map(|(identity, argument)| {
                    self.requirement_argument_identity_text(*argument, *identity)
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            identity.parameters = compound(
                CoordinateTag::ClosedRequirement,
                [
                    atom(CoordinateTag::Template, &template_identity),
                    atom(CoordinateTag::TraitApplication, &outer),
                    compound(CoordinateTag::LocalArguments, local),
                    atom(CoordinateTag::Parameters, &identity.parameters),
                ],
            );
        }
        identity
    }

    fn operator_callable_identity_parts(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> callable_identity::CallableIdentityParts {
        let path = self.operator_path_string(operator.name);
        let flags = self
            .operator_parameters(operator)
            .iter()
            .map(|parameter| {
                format!(
                    "mutable={};const={}",
                    parameter.is_mutable, parameter.is_const
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        callable_identity::CallableIdentityParts {
            path,
            parameters: format!(
                "operands({});flags({flags})",
                crate::operator::operator_operand_signature(self, operator)
            ),
            result_dispatch: self.normalized_result_dispatch_set(operator.return_type),
        }
    }

    fn named_callable_identity_parts(
        &self,
        path: &str,
        owner_symbol: SymbolHandle,
        type_parameters: &[data::TypeParameter],
        parameters: &[crate::signature::StateParameter],
        return_type: TypeReferenceHandle,
    ) -> callable_identity::CallableIdentityParts {
        let binders = callable_type_binders(owner_symbol, type_parameters);
        let parameters = parameters
            .iter()
            .map(|parameter| {
                compound(
                    CoordinateTag::Parameter,
                    [
                        atom(
                            CoordinateTag::Mutable,
                            if parameter.is_mutable { "yes" } else { "no" },
                        ),
                        atom(
                            CoordinateTag::Const,
                            if parameter.is_const { "yes" } else { "no" },
                        ),
                        self.type_identity_request_text(TypeIdentityRequest {
                            binders: &binders,
                            ..TypeIdentityRequest::ordinary(parameter.type_reference)
                        }),
                    ],
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        callable_identity::CallableIdentityParts {
            path: TypeIdentityContext::default().qualify_non_binder_name(
                self,
                owner_symbol,
                path.to_owned(),
            ),
            parameters,
            result_dispatch: self.normalized_result_dispatch_set(return_type),
        }
    }
}

fn callable_type_binders(
    owner: SymbolHandle,
    parameters: &[data::TypeParameter],
) -> Vec<(SymbolHandle, String)> {
    let mut binders = Vec::with_capacity(parameters.len() + 1);
    if owner.is_valid() {
        binders.push((owner, "$Self".to_owned()));
    }
    binders.extend(
        parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol.is_valid())
            .map(|(index, parameter)| (parameter.symbol, format!("$T{index}"))),
    );
    binders
}

fn intern_boxed_text<I: Copy, S: std::hash::BuildHasher>(
    text: &str,
    texts: &mut Vec<Box<str>>,
    slots: &mut std::collections::HashMap<Box<str>, I, S>,
    mint: impl Fn(usize) -> I,
) -> I {
    if let Some(interned) = slots.get(text) {
        return *interned;
    }
    let interned = mint(texts.len());
    let text: Box<str> = text.into();
    texts.push(text.clone());
    slots.insert(text, interned);
    interned
}

fn intern_owned<K, I: Copy, S: std::hash::BuildHasher>(
    value: K,
    values: &mut Vec<K>,
    slots: &mut std::collections::HashMap<K, I, S>,
    mint: impl Fn(usize) -> I,
) -> I
where
    K: Eq + std::hash::Hash + Clone,
{
    if let Some(interned) = slots.get(&value) {
        return *interned;
    }
    let interned = mint(values.len());
    values.push(value.clone());
    slots.insert(value, interned);
    interned
}
