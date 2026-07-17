use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::data::{
    DataDefinition, DataDefinitionStorage, DataField, DataMember, DataProperties, DataVariant,
    TypeParameter, TypeParameterKind,
};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    // `Versioned` is the permanent builtin era-tagged container (frozen
    // decision 14): every `Versioned<T>` reference folds to the synthesized
    // container of a versioned data type, so a user definition of the same
    // name could never be referenced -- reject it instead of shadowing.
    if data_definition.name.as_str() == "Versioned" {
        return Err(Diagnostic::error(
            "`Versioned` is the builtin era-tagged container type (chapter 21) and cannot be redeclared",
        ));
    }
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, data_definition.type_parameters)?;
    let members = lower_data_members(lowerer, syntax_trees, data_definition.members)?;
    // R2 rung 2 slice 1 (ch12 gating): lower the default-domain facts and
    // classify AT ZERO. Zero-satisfying facts are admitted -- the value is
    // born established (the zero-constructible tier); they stay INERT until
    // rung 3 wires entailment hypotheses and write obligations ATOMICALLY.
    // A GATED type (zero violates the domain) refuses until rung 2b lands
    // construction-mandatory fields; a fact the folder cannot evaluate at
    // zero refuses as unsupported (v1 fence). Never a silent drop.
    let where_facts =
        crate::domain::lower_proof_facts(lowerer, syntax_trees, data_definition.where_facts)?;
    let mut zero_gated = false;
    for fact in lowerer.symbol_resolved_trees.proof_facts(where_facts) {
        let omega_symbol_resolved_trees::domain::ProofFact::Expression(expression) = fact else {
            return Err(Diagnostic::error(format!(
                "data `{}`: a domain-membership fact in the default-domain `where` \
                 clause is not supported yet (R2) -- spell arithmetic facts over the \
                 fields",
                data_definition.name.as_str()
            )));
        };
        match zero_fold(
            &lowerer.symbol_resolved_trees.tables.bodies.expressions,
            *expression,
        ) {
            Some(value) if value != 0 => {}
            // R2 rung 2b: zero violates the domain -- the type is GATED
            // (admitted; its literals must PROVE the domain, and rung 3's
            // access gate covers zeroed storage).
            Some(_) => zero_gated = true,
            None => {
                return Err(Diagnostic::error(format!(
                    "data `{}`: a default-domain `where` fact is outside the v1 \
                     zero-foldable fragment (field names, integer literals, + - *, \
                     comparisons, && ) -- simplify the fact for now (R2)",
                    data_definition.name.as_str()
                )));
            }
        }
    }

    Ok(DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&data_definition.name),
        storage: DataDefinitionStorage {
            type_parameters,
            properties: DataProperties {
                copy: data_definition.properties.copy,
                zero_init: data_definition.properties.zero_init,
                send: data_definition.properties.send,
                // STR3: `[copy]` -> Unrestricted, ordinary data -> Affine
                // (`[linear]` has no spelling yet).
                multiplicity: if data_definition.properties.copy {
                    omega_core::semantics::Multiplicity::Unrestricted
                } else {
                    omega_core::semantics::Multiplicity::Affine
                },
            },
            where_facts,
            zero_gated,
            members,
        },
    })
}

/// R2 rung 2 slice 1: fold one default-domain fact AT THE ZERO VALUE --
/// every field name reads 0, literals read themselves, `+ - *` fold,
/// comparisons and `&&`/`||` yield 1/0. `None` = outside the fragment.
fn zero_fold(
    expressions: &omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<i128> {
    use omega_symbol_resolved_trees::expression::{BinaryOperator, ExpressionNode};
    match expressions.expression(expression) {
        ExpressionNode::Name(_) => Some(0),
        ExpressionNode::Integer(literal) => literal.text().parse::<i128>().ok(),
        ExpressionNode::Binary(binary) => {
            let left = zero_fold(expressions, binary.left)?;
            let right = zero_fold(expressions, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                BinaryOperator::Less => Some(i128::from(left < right)),
                BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                BinaryOperator::Greater => Some(i128::from(left > right)),
                BinaryOperator::Equal => Some(i128::from(left == right)),
                BinaryOperator::NotEqual => Some(i128::from(left != right)),
                BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Lower each `version vN { ... }` block of a data declaration into its own
/// historical-shape data definition named `Data::vN`. The synthesized
/// definitions must be pushed immediately after their parent (in member
/// order): the positional symbol-assignment pass pairs them with the root
/// symbols seeded at the same relative positions.
///
/// Declared-history contradictions caught here:
/// - a version name that is not a `v<number>` selector,
/// - duplicate version names on one data declaration,
/// - a version block nested inside another version block.
pub(crate) fn lower_data_version_definitions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<Vec<DataDefinition>, Diagnostic> {
    let mut versions = Vec::new();
    let mut seen_names: Vec<&str> = Vec::new();

    for member in syntax_trees.items.data_members(data_definition.members) {
        let syntax::item::DataMember::Version(version) = member else {
            continue;
        };

        let version_name = version.name.as_str();
        if !syntax::item::is_version_selector(version_name) {
            return Err(Diagnostic::error(format!(
                "data `{}` declares version `{version_name}`, but version names must be `v` followed by digits (`v1`, `v2`, ...)",
                data_definition.name
            )));
        }
        if seen_names.contains(&version_name) {
            return Err(Diagnostic::error(format!(
                "data `{}` declares duplicate version `{version_name}`",
                data_definition.name
            )));
        }
        seen_names.push(version_name);

        for nested in syntax_trees.items.data_members(version.members) {
            if matches!(nested, syntax::item::DataMember::Version(_)) {
                return Err(Diagnostic::error(format!(
                    "data `{}` version `{version_name}` cannot declare a nested version block",
                    data_definition.name
                )));
            }
        }

        let members = lower_data_members(lowerer, syntax_trees, version.members)?;
        versions.push(DataDefinition {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&omega_syntax_trees::identifier::Identifier::generated(
                version.shape_name(data_definition.name.as_str()),
            )),
            storage: DataDefinitionStorage {
                type_parameters: HandleSpan::empty(),
                where_facts: omega_core::arena::HandleSpan::empty(),
                zero_gated: false,
                // Historical shapes carry no declared properties; a property
                // describes the CURRENT shape only.
                properties: DataProperties::default(),
                members,
            },
        });
    }

    Ok(versions)
}

/// Synthesize the builtin `Versioned<Counter>` container definition for a data
/// declaration with `version vN { ... }` blocks (frozen decision 14): an
/// ordinary data definition `{ era: u32, __payload_vN: Counter::vN ...,
/// __payload_current: Counter }`. Era numbering follows decision 10 (declared
/// blocks count up from 0 in declaration order; the current shape's era is the
/// block count) -- see `omega_core::versioning`.
///
/// The container has NO source constructor (chapter 21: it is minted only by
/// boundary machinery), so the payload fields are unreachable except through
/// version match arm bindings, and `era` is kept read-only by validation.
///
/// INTERIM: the payload fields are laid out as a plain struct rather than the
/// frozen union-of-eras (static max size). Nothing can construct a container
/// yet, so the overlap is unobservable; the union layout lands with the first
/// boundary decoder.
pub(crate) fn lower_versioned_container_definition(
    lowerer: &mut Lowerer,
    data_definition: &syntax::item::DataDefinition,
    version_names: &[String],
) -> DataDefinition {
    use omega_core::versioning;
    use omega_symbol_resolved_trees::types::TypeReference;
    use omega_syntax_trees::identifier::Identifier;

    let data_name = data_definition.name.as_str();
    let mut members = Vec::new();

    members.push(DataMember::Field(DataField {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&Identifier::generated(
            versioning::VERSIONED_ERA_FIELD.to_owned(),
        )),
        type_reference: TypeReference::Named {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&Identifier::generated("u32".to_owned())),
        },
    }));

    for version_name in version_names {
        members.push(DataMember::Field(DataField {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&Identifier::generated(
                versioning::versioned_payload_field_name(version_name),
            )),
            type_reference: TypeReference::Named {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&Identifier::generated(format!(
                    "{data_name}::{version_name}"
                ))),
            },
            }));
    }

    members.push(DataMember::Field(DataField {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&Identifier::generated(
            versioning::VERSIONED_CURRENT_PAYLOAD_FIELD.to_owned(),
        )),
        type_reference: TypeReference::Named {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&Identifier::generated(data_name.to_owned())),
        },
    }));

    let mut span = HandleSpan::empty();
    for member in members {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .data_members
            .append_to_span(&mut span, member);
    }

    DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&Identifier::generated(
            versioning::versioned_container_name(data_name),
        )),
        storage: DataDefinitionStorage {
            type_parameters: HandleSpan::empty(),
            where_facts: omega_core::arena::HandleSpan::empty(),
                zero_gated: false,
            // The container itself is plain runtime state: zero-initialized
            // like any other field-bearing data (a zeroed container reads as
            // the oldest declared era with a zeroed payload).
            properties: DataProperties::default(),
            members: span,
        },
    }
}

pub(crate) fn lower_type_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_parameters: HandleSpan<syntax::item::TypeParameter>,
) -> Result<HandleSpan<TypeParameter>, Diagnostic> {
    let mut lowered = Vec::new();
    for parameter in syntax_trees.items.type_parameters(type_parameters) {
        let kind = match &parameter.kind {
            syntax::item::TypeParameterKind::Type => TypeParameterKind::Type,
            syntax::item::TypeParameterKind::Const { type_reference } => TypeParameterKind::Const {
                type_reference: lower_type_reference_handle(
                    lowerer,
                    syntax_trees,
                    *type_reference,
                )?,
            },
        };
        lowered.push(TypeParameter {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&parameter.name),
            kind,
            bounds: DataProperties {
                copy: parameter.bounds.copy,
                zero_init: parameter.bounds.zero_init,
                send: parameter.bounds.send,
                multiplicity: if parameter.bounds.copy {
                    omega_core::semantics::Multiplicity::Unrestricted
                } else {
                    omega_core::semantics::Multiplicity::Affine
                },
            },
        });
    }

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .data_type_parameters
        .insert_many(lowered))
}

fn lower_data_members(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    members: HandleSpan<syntax::item::DataMember>,
) -> Result<HandleSpan<DataMember>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.data_members(members) {
        if matches!(member, syntax::item::DataMember::Version(_)) {
            continue;
        }
        let member = lower_data_member(lowerer, syntax_trees, member)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .data_members
            .append_to_span(&mut span, member);
    }

    Ok(span)
}

fn lower_data_member(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    member: &syntax::item::DataMember,
) -> Result<DataMember, Diagnostic> {
    match member {
        syntax::item::DataMember::Field(field) => Ok(DataMember::Field(DataField {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&field.name),
            type_reference: lower_type_reference_handle(
                lowerer,
                syntax_trees,
                field.type_reference,
            )?,
        })),
        syntax::item::DataMember::Variant(variant) => {
            let mut payload = HandleSpan::empty();
            for field in syntax_trees.items.data_payload_fields(variant.payload) {
                let lowered = DataField {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::lower_name(&field.name),
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        field.type_reference,
                    )?,
                };
                lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .data_payload_fields
                    .append_to_span(&mut payload, lowered);
            }
            Ok(DataMember::Variant(DataVariant {
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&variant.name),
                payload,
            }))
        }
        syntax::item::DataMember::Version(_) => unreachable!("data versions are metadata"),
    }
}
