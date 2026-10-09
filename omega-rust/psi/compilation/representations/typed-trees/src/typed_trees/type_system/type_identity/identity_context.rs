//! The identity context: package qualification and the normalization of
//! type references, index expressions and const or nominal names.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::type_identity::substitution;
use crate::typed_trees::type_system::type_identity::constraint_identity::{
    NormalizedConstraint, atom, byte_atom, compound, normalize_array_length,
    normalize_constrained_base, normalized_constraints,
};
use crate::typed_trees::type_system::type_identity::open_index_identity::{
    collect_open_index_operands, index_binary_operator_name, open_index_algebra_identity,
    open_index_operation_identity, open_index_operation_selection,
};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use std::cell::Cell;
use symbols::JoinedPath;
use symbols::SymbolHandle;

#[derive(Default)]
pub(crate) struct TypeIdentityContext<'binders> {
    pub(crate) application_scopes: &'binders [super::AppliedTypeIdentityScope<'binders>],
    pub(crate) application_owner: SymbolHandle,
    pub(crate) invalid_application: Option<&'binders Cell<bool>>,
    pub(crate) binders: &'binders [(SymbolHandle, String)],
    pub(crate) substitutions: &'binders [(SymbolHandle, TypeReferenceHandle)],
    pub(crate) substitution_semantics: SubstitutionSemantics,
    pub(crate) active_const_substitutions: &'binders [SymbolHandle],
    pub(crate) index_substitutions:
        &'binders [(SymbolHandle, super::IndexIdentityArgument<'binders>)],
    pub(crate) invalid_index_argument: Option<&'binders Cell<bool>>,
    pub(crate) exact_toolchain_sources: &'binders [(source::SourceId, [u8; 32])],
    pub(crate) missing_exact_nominal_owner: Option<&'binders Cell<bool>>,
    pub(crate) qualification: TypeIdentityQualification,
}

#[derive(Clone, Copy, Default)]
pub(crate) enum SubstitutionSemantics {
    #[default]
    Transitive,
    Simultaneous,
}

/// Whether a normalized identity retains the local declaration namespace or
/// the exact package owner. The toolchain namespace is distinct in both forms.
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
            Self::Package(digest) => byte_atom("package-owner", &digest),
            Self::ToolchainSource(digest) => byte_atom("toolchain-source-owner", &digest),
            Self::Toolchain => "toolchain-owner".to_owned(),
            Self::Unresolved => "unresolved-owner".to_owned(),
        }
    }
}

impl TypeIdentityContext<'_> {
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
                        ..*self
                    },
                ));
            }
        }
        None
    }

    fn lifetime(&self, program: &TypedTrees, name: &str) -> String {
        if let Some((ordinal, scope)) = self.application_scopes.iter().enumerate().next_back()
            && let Some((_, argument)) = scope
                .lifetimes
                .iter()
                .find(|(parameter, _)| parameter == name)
        {
            return Self {
                application_scopes: &self.application_scopes[..ordinal],
                ..*self
            }
            .lifetime(program, argument);
        }
        let owner = self
            .application_scopes
            .last()
            .map_or(self.application_owner, |scope| scope.declaration);
        let parameters = program
            .machine_by_symbol(owner)
            .map(|owner| &owner.lifetime_parameters)
            .or_else(|| {
                program
                    .conformance_by_symbol(owner)
                    .map(|owner| &owner.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .trait_definition_by_symbol(owner)
                    .map(|owner| &owner.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .data_definition_by_symbol(owner)
                    .map(|owner| &owner.lifetime_parameters)
            });
        if let Some(ordinal) = parameters.and_then(|parameters| {
            parameters
                .iter()
                .position(|parameter| parameter.as_str() == name)
        }) {
            return compound(
                "region",
                [
                    atom(
                        "owner",
                        &format!("{}:{}", owner.arena_index(), owner.generation()),
                    ),
                    atom("ordinal", &ordinal.to_string()),
                ],
            );
        }
        if let Some(invalid) = self.invalid_application {
            invalid.set(true);
        }
        "unresolved-region".to_owned()
    }

    /// A simultaneous application's actual belongs to the caller's scope.
    /// Its leaves must not be rebound by the callee's application again.
    pub(crate) fn replacement_context(&self) -> Self {
        Self {
            substitutions: match self.substitution_semantics {
                SubstitutionSemantics::Transitive => self.substitutions,
                SubstitutionSemantics::Simultaneous => &[],
            },
            ..*self
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
        // Open index expressions are parsed before their enclosing const
        // telescope is available, so a direct binder leaf can still carry an
        // invalid expression symbol. Binder-aware template identity recovers
        // that exact local by its symbol-table name; concrete qualified names
        // never match this one-segment route.
        if !symbol.is_valid()
            && !fallback.contains("::")
            && let Some((_, replacement)) = self.binders.iter().find(|(candidate, _)| {
                candidate.is_valid() && program.symbols.name(*candidate) == fallback
            })
        {
            return replacement.clone();
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
        // Toolchain declarations and authored declarations can have the same
        // visible path: lookup admits an authored `Source` beside Build's
        // toolchain `Source`. Erasing that namespace merges unrelated carriers
        // before structural retention. Source-free compiler atoms retain their
        // closed scalar identities; package qualification remains the stronger
        // identity used across independently compiled package graphs.
        if self.qualification == TypeIdentityQualification::Ordinary
            && (program.symbols.builtin_type_atom(symbol).is_some()
                || program.symbols.symbol_source_origin(symbol)
                    != Some(source::SourceOrigin::Toolchain))
        {
            return path;
        }
        if let Some(builtin_atom) = program.symbols.builtin_type_atom(symbol) {
            return compound("compiler-type", [atom("atom", builtin_atom.identity())]);
        }
        let owner = if let Some(package) = program.symbols.symbol_package_identity(symbol) {
            PackageQualifiedNominalOwner::Package(package.digest())
        } else if program.symbols.symbol_source_origin(symbol)
            == Some(source::SourceOrigin::Toolchain)
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
    compound("nominal", [owner.encode(), atom("path", path)])
}

fn generic_type_identity(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    lifetimes: &[crate::name::Identifier],
    arguments: impl Iterator<Item = String>,
    context: &TypeIdentityContext<'_>,
) -> String {
    compound(
        "generic",
        std::iter::once(atom("name", &context.name(program, symbol, name)))
            .chain(context.invalid_application.into_iter().flat_map(|_| {
                lifetimes
                    .iter()
                    .map(|lifetime| context.lifetime(program, lifetime.as_str()))
            }))
            .chain(arguments),
    )
}

/// A source static type application and a type-reference application denote
/// the same carrier. Read both through one grammar without allocating a type
/// node merely to compare a retained source operand.
pub(super) fn static_type_argument_identity(
    program: &TypedTrees,
    argument: &crate::expression::StaticMachineArgument,
    context: &TypeIdentityContext<'_>,
) -> Option<String> {
    if context.invalid_application.is_none()
        || !argument.symbol.is_valid()
        || !matches!(
            program.symbols.get(argument.symbol).kind,
            symbols::SymbolKind::Data
                | symbols::SymbolKind::BuiltinType
                | symbols::SymbolKind::TypeParameter
        )
    {
        return None;
    }
    if let Some((replacement, outer)) = context.applied_argument(argument.symbol) {
        return Some(substitution::index_argument(program, replacement, &outer));
    }
    if let Some(application) = &argument.application {
        return Some(generic_type_identity(
            program,
            argument.symbol,
            "",
            &application.lifetime_arguments,
            application.arguments.iter().map(|argument| {
                substitution::index_argument(
                    program,
                    super::IndexIdentityArgument::StaticArgument(argument),
                    context,
                )
            }),
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
        "named",
        [normalize_const_or_nominal_name(
            program,
            argument.symbol,
            "",
            "name",
            context,
        )],
    ))
}

pub(crate) fn normalize_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
        && let Some((argument, outer)) = context.applied_argument(*symbol)
    {
        return substitution::index_argument(program, argument, &outer);
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
        && let Some((_, replacement)) = context
            .substitutions
            .iter()
            .rev()
            .find(|(candidate, _)| candidate == symbol)
        && *replacement != type_reference
    {
        return normalize_type_reference(program, *replacement, &context.replacement_context());
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => compound(
            match access {
                language_core::ReferenceAccess::Shared => "ref",
                language_core::ReferenceAccess::Mutable => "ref-mut",
                language_core::ReferenceAccess::WriteOnly => "ref-write",
            },
            std::iter::once(normalize_type_reference(program, *referee, context)).chain(
                context.invalid_application.map(|_| {
                    lifetime.as_ref().map_or_else(
                        || "elided-region".to_owned(),
                        |lifetime| context.lifetime(program, lifetime.as_str()),
                    )
                }),
            ),
        ),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let (base, mut all_constraints) =
                normalize_constrained_base(program, *base_type, context);
            all_constraints.extend(normalized_constraints(program, *constraints, context));
            all_constraints.sort();
            all_constraints.dedup();
            compound(
                "constrained",
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
            "array",
            [
                normalize_type_reference(program, *element_type, context),
                normalize_array_length(program, length, context),
            ],
        ),
        TypeReferenceNode::Slice { element_type } => compound(
            "slice",
            [normalize_type_reference(program, *element_type, context)],
        ),
        // Region applications retain their lifetime arguments for loan checks,
        // but a lifetime-only application has the same runtime nominal carrier
        // as the bare declaration. An empty runtime argument list must not add
        // a generic wrapper after those regions have been erased.
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
            ..
        } if arguments.is_empty()
            && (lifetime_arguments.is_empty() || context.invalid_application.is_none()) =>
        {
            compound(
                "named",
                [normalize_const_or_nominal_name(
                    program,
                    *base_symbol,
                    base_name.as_str(),
                    "name",
                    context,
                )],
            )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
            ..
        } => generic_type_identity(
            program,
            *base_symbol,
            base_name.as_str(),
            lifetime_arguments,
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .enumerate()
                .map(|(ordinal, argument)| {
                    let is_index = context.invalid_application.is_some()
                        && program
                            .data_definition_by_symbol(*base_symbol)
                            .and_then(|data| program.data_type_parameters(data).get(ordinal))
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
        ),
        TypeReferenceNode::ConstExpression(expression) => compound(
            "index-expression",
            [normalize_index_expression(program, *expression, context)],
        ),
        TypeReferenceNode::DynamicTrait {
            symbol,
            name,
            arguments,
        } => compound(
            "dynamic-trait",
            std::iter::once(atom("name", &context.name(program, *symbol, name.as_str()))).chain(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| normalize_type_reference(program, *argument, context)),
            ),
        ),
        TypeReferenceNode::Named { symbol, name } => compound(
            "named",
            [normalize_const_or_nominal_name(
                program,
                *symbol,
                name.as_str(),
                "name",
                context,
            )],
        ),
        TypeReferenceNode::Unit => "unit".to_owned(),
    }
}

pub(crate) fn normalize_index_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let fallback = members.joined_path();
            let symbol = if path.symbol.is_valid() {
                path.symbol
            } else if members.len() == 1 {
                path.head_symbol
            } else {
                SymbolHandle::invalid()
            };
            if let Some(identity) = substitution::index(program, symbol, &fallback, context) {
                return identity;
            }
            substitution::index_name(program, symbol, fallback.as_str(), context)
        }
        ExpressionNode::Binary(binary) => {
            let Some(selection) = open_index_operation_selection(program, expression) else {
                return compound(
                    index_binary_operator_name(binary.operator),
                    [
                        normalize_index_expression(program, binary.left, context),
                        normalize_index_expression(program, binary.right, context),
                    ],
                );
            };
            let operands = collect_open_index_operands(
                program,
                expression,
                binary.operator,
                selection,
                context,
            );
            compound(
                index_binary_operator_name(binary.operator),
                std::iter::once(atom(
                    "operation",
                    &open_index_operation_identity(program, selection, context),
                ))
                .chain(std::iter::once(atom(
                    "algebra",
                    &open_index_algebra_identity(program, selection, context),
                )))
                .chain(operands),
            )
        }
        ExpressionNode::Integer(value) => substitution::index_integer(value, context),
        ExpressionNode::Unary(unary) => compound(
            match unary.operator {
                crate::expression::UnaryOperator::BitwiseNot => "bitwise-not",
                crate::expression::UnaryOperator::LogicalNot => "logical-not",
            },
            [normalize_index_expression(program, unary.operand, context)],
        ),
        // These shapes are rejected by PDI3 index validation. Keep their
        // provisional identity structural -- a kind tag over normalized
        // children -- and independent of diagnostic rendering so even a
        // rejected tree never makes display text an equality oracle, and two
        // differently shaped rejected expressions never share one identity.
        ExpressionNode::Boolean(value) => atom("boolean", &value.to_string()),
        ExpressionNode::Float(value) => atom("float", &value.to_string()),
        ExpressionNode::UndeterminedNaN(format) => atom("undetermined-nan", format.name()),
        ExpressionNode::String(value) => byte_atom("string", value),
        ExpressionNode::ArrayLiteral(elements) => compound(
            "array-literal",
            program
                .expression_table
                .expression_handles(*elements)
                .iter()
                .map(|element| normalize_index_expression(program, *element, context)),
        ),
        ExpressionNode::Match(match_expression) => compound(
            "match",
            std::iter::once(normalize_index_expression(
                program,
                match_expression.subject,
                context,
            ))
            .chain(
                program
                    .expression_table
                    .match_arms(match_expression.arms)
                    .iter()
                    .map(|arm| {
                        compound(
                            "arm",
                            [
                                match arm.pattern {
                                    crate::expression::MatchPattern::Value(pattern) => {
                                        normalize_index_expression(program, pattern, context)
                                    }
                                    crate::expression::MatchPattern::Wildcard => {
                                        "wildcard".to_owned()
                                    }
                                },
                                normalize_index_expression(program, arm.value, context),
                            ],
                        )
                    }),
            ),
        ),
        ExpressionNode::Atomic(atomic) => compound(
            "atomic",
            [
                normalize_index_expression(program, atomic.value, context),
                normalize_index_expression(program, atomic.result, context),
                atom("ordering", &format!("{:?}", atomic.ordering)),
                atom("result-custody", &format!("{:?}", atomic.result_custody)),
            ],
        ),
        ExpressionNode::Cast(cast) => compound(
            "cast",
            [
                normalize_index_expression(program, cast.value, context),
                normalize_type_reference(program, cast.target_type, context),
                atom("domain", cast.domain.name()),
            ],
        ),
        ExpressionNode::Call(call) => compound(
            "call",
            std::iter::once(atom(
                "target",
                &context.name(program, call.target_symbol, call.target.as_str()),
            ))
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
            "indexed",
            [
                normalize_index_expression(program, indexed.collection, context),
                normalize_index_expression(program, indexed.index, context),
            ],
        ),
        ExpressionNode::Member(member) => compound(
            "member",
            std::iter::once(normalize_index_expression(
                program,
                member.receiver,
                context,
            ))
            .chain(std::iter::once(atom("member", member.member.as_str())))
            .chain(
                member
                    .case_variant
                    .iter()
                    .map(|variant| atom("case", variant.as_str())),
            ),
        ),
        ExpressionNode::Borrow(borrow) => compound(
            match borrow.access {
                language_core::ReferenceAccess::Shared => "borrow",
                language_core::ReferenceAccess::Mutable => "borrow-mut",
                language_core::ReferenceAccess::WriteOnly => "borrow-write",
            },
            [normalize_index_expression(program, borrow.target, context)],
        ),
        ExpressionNode::Range(range) => compound(
            if range.end_inclusive {
                "range-inclusive"
            } else {
                "range-exclusive"
            },
            [
                normalize_index_expression(program, range.start, context),
                normalize_index_expression(program, range.end, context),
            ],
        ),
        ExpressionNode::StructLiteral(literal) => compound(
            "struct-literal",
            std::iter::once(atom(
                "type",
                &context.name(program, literal.type_symbol, literal.type_name.as_str()),
            ))
            .chain(
                literal
                    .case_name
                    .as_ref()
                    .map(|case| atom("case", case.as_str())),
            )
            .chain(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| {
                        compound(
                            "field",
                            [
                                atom("name", field.name.as_str()),
                                normalize_index_expression(program, field.value, context),
                            ],
                        )
                    }),
            ),
        ),
        ExpressionNode::ZeroValue(type_reference) => compound(
            "zero-value",
            [normalize_type_reference(program, *type_reference, context)],
        ),
    }
}

pub(crate) fn normalize_const_or_nominal_name(
    program: &TypedTrees,
    symbol: SymbolHandle,
    spelling: &str,
    nominal_tag: &str,
    context: &TypeIdentityContext<'_>,
) -> String {
    if !symbol.is_valid() {
        // The const evaluator alone can create this source-unspellable atom.
        // Decode its semantic fields and deliberately omit diagnostic display;
        // package review separately proves that the leaf occupies one exact
        // declared const slot before accepting this identity.
        if let Some(value) =
            language_semantics::const_value::CanonicalConstValue::from_atom(spelling)
        {
            return compound(
                "canonical-const",
                [
                    atom("type", &value.type_name),
                    atom("encoding", &value.encoding),
                ],
            );
        }
        if let Ok(value) = spelling.parse::<i128>() {
            return atom("integer-const", &value.to_string());
        }
    }
    atom(nominal_tag, &context.name(program, symbol, spelling))
}
