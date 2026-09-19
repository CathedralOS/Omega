//! Structural const values under exact declaration-site carrier selection.
//!
//! The header resolver selects each nominal type and constructor in its own
//! authored source, including nested field carriers. Compare those declarations
//! before erasing names into the existing canonical atom. Field order and leaf
//! landing still determine the encoded value; final resolution independently
//! checks the receiving parameter's nominal carrier.
//!
//! Structural encoding recursively lands anonymous scalar leaves at the declared
//! component type. A suffixed leaf has already landed: erasing that carrier or
//! arithmetic domain would equate a differently typed value with this component,
//! even when its numeric payload fits. Check the landing before encoding it.
use super::super::{
    CanonicalConstValue, ConstDefinition, DataDefinition, DataMember, ExpressionHandle,
    ExpressionNode, FixedArrayLength, HandleSpan, HashSet, Identifier, Item, SyntaxTrees,
    TypeReferenceHandle, TypeReferenceNode,
};

use crate::preparation::generic_data::ClosedArgumentIdentity;
use crate::preparation::generic_data::ConstFactValue;
use crate::preparation::generic_data::arguments::{
    closed_argument_identity, monomorphizable_argument_slugs,
};
use crate::preparation::generic_data::constant_selection::{
    ConstantSelection, GenericApplicationSubstitution, resolved_generic_argument,
};
use crate::preparation::generic_data::integer_literal_value;
use crate::preparation::generic_data::qualified_const_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::preparation::generic_data) enum CanonicalConstNode {
    Integer {
        type_name: String,
        value: i128,
    },
    Boolean(bool),
    Array {
        type_name: String,
        values: Vec<CanonicalConstNode>,
    },
    Record {
        type_name: String,
        fields: Vec<(String, CanonicalConstNode)>,
    },
    Variant {
        type_name: String,
        case_name: String,
        fields: Vec<(String, CanonicalConstNode)>,
    },
}

impl CanonicalConstNode {
    pub(in crate::preparation::generic_data) fn encoding(&self) -> String {
        match self {
            Self::Integer { type_name, value } => {
                framed("integer", [type_name.clone(), value.to_string()])
            }
            Self::Boolean(value) => framed("boolean", [if *value { "true" } else { "false" }]),
            Self::Array { type_name, values } => framed(
                "array",
                std::iter::once(type_name.as_str().to_owned())
                    .chain(values.iter().map(Self::encoding)),
            ),
            Self::Record { type_name, fields } => framed(
                "record",
                std::iter::once(type_name.clone()).chain(
                    fields
                        .iter()
                        .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                ),
            ),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } => framed(
                "variant",
                [type_name.clone(), case_name.clone()].into_iter().chain(
                    fields
                        .iter()
                        .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                ),
            ),
        }
    }

    pub(in crate::preparation::generic_data) fn display(&self) -> String {
        match self {
            Self::Integer { value, .. } => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::Array { values, .. } => format!(
                "[{}]",
                values
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Record { type_name, fields } => format!(
                "{type_name} {{ {} }}",
                fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } if fields.is_empty() => format!("{type_name}::{case_name}"),
            Self::Variant {
                type_name,
                case_name,
                fields,
            } => format!(
                "{type_name}::{case_name} {{ {} }}",
                fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

pub(in crate::preparation::generic_data) fn framed(
    tag: &str,
    pieces: impl IntoIterator<Item = impl AsRef<str>>,
) -> String {
    let mut encoded = tag.to_owned();
    for piece in pieces {
        let piece = piece.as_ref();
        encoded.push_str(&piece.len().to_string());
        encoded.push(':');
        encoded.push_str(piece);
    }
    encoded
}

pub(in crate::preparation::generic_data) fn canonicalize_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    parameter_type: TypeReferenceHandle,
) -> Result<CanonicalConstValue, String> {
    canonicalize_selected_const_definition(syntax, definition, parameter_type, None)
}

pub(in crate::preparation::generic_data) fn canonicalize_selected_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    parameter_type: TypeReferenceHandle,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstValue, String> {
    let declared = syntax_type_identity(syntax, definition.type_reference)?;
    let required = syntax_type_identity(syntax, parameter_type)?;
    if !same_carrier(syntax, definition.type_reference, parameter_type, selection)? {
        return Err(format!(
            "const `{}` declares type `{declared}`, but the parameter requires `{required}`",
            qualified_const_name(definition)
        ));
    }
    canonicalize_selected_index_expression(
        syntax,
        definition.type_reference,
        parameter_type,
        definition.value,
        selection,
    )
}

/// Direct structural atoms and named constants share index admissibility and
/// canonical representation. Only the named route additionally owns a const
/// declaration's copy/cleanup eligibility and declared-carrier equality.
pub(in crate::preparation::generic_data) fn canonicalize_selected_index_expression(
    syntax: &SyntaxTrees,
    value_type: TypeReferenceHandle,
    parameter_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstValue, String> {
    validate_selected_const_index_type(
        syntax,
        parameter_type,
        &mut Vec::new(),
        selection,
        &GenericApplicationSubstitution::new(),
    )?;
    let node = canonicalize_const_expression(
        syntax,
        value_type,
        expression,
        selection,
        &GenericApplicationSubstitution::new(),
    )?;
    let required = selected_type_label(syntax, parameter_type, selection)?;
    if required == "Rat" {
        validate_canonical_rat(&node)?;
    }
    Ok(CanonicalConstValue::new(
        required,
        node.encoding(),
        node.display(),
    ))
}

pub(in crate::preparation::generic_data) fn syntax_type_identity(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Result<String, String> {
    Ok(
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(name) => name.as_str().to_owned(),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => format!(
                "[{}; {length}]",
                syntax_type_identity(syntax, *element_type)?
            ),
            TypeReferenceNode::Constrained { base_type, .. } => {
                syntax_type_identity(syntax, *base_type)?
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let argument_handles = syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments);
                if argument_handles.is_empty() {
                    // A synthesized instance's lookup spelling carries its full
                    // name in `base_name`.
                    base_name.as_str().to_owned()
                } else {
                    let mut parts = Vec::with_capacity(argument_handles.len());
                    for argument in argument_handles {
                        parts.push(syntax_type_identity(syntax, *argument)?);
                    }
                    format!("{}<{}>", base_name.as_str(), parts.join(", "))
                }
            }
            TypeReferenceNode::Unit => "()".to_owned(),
            _ => {
                return Err(
                "structured const parameter types must be a canonical scalar, fixed array, or declared data value"
                    .to_owned(),
            );
            }
        },
    )
}

// Encoded labels are consistency claims. Nominal equality is established from
// the selected declarations before values lose their authored spelling; the
// receiving generic slot independently rejoins that declared carrier later.
fn selected_data<'syntax>(
    syntax: &'syntax SyntaxTrees,
    name: &Identifier,
    selection: Option<&ConstantSelection>,
) -> Result<&'syntax DataDefinition, String> {
    if let Some(selection) = selection {
        return selection.data(syntax, name);
    }
    let mut definitions = syntax.root_items().filter_map(|item| match item {
        Item::Data(definition) if definition.name.as_str() == name.as_str() => Some(definition),
        _ => None,
    });
    let definition = definitions
        .next()
        .ok_or_else(|| format!("`{name}` is not a declared canonical data type"))?;
    if definitions.next().is_some() {
        return Err(format!("`{name}` has ambiguous nominal declarations"));
    }
    Ok(definition)
}

/// Select a synthesized closed instance by its exact display name. Synthesized
/// declarations are pushed after the header symbol pass, so name law lookup
/// cannot reach them; their `generic_instance` evidence is what distinguishes
/// one from an authored declaration that happens to share the spelling.
fn selected_synthesized_instance<'syntax>(
    syntax: &'syntax SyntaxTrees,
    name: &Identifier,
) -> Result<&'syntax DataDefinition, String> {
    let mut candidates = syntax.root_items().filter_map(|item| match item {
        Item::Data(definition)
            if definition.generic_instance.is_some()
                && definition.name.as_str() == name.as_str() =>
        {
            Some(definition)
        }
        _ => None,
    });
    let instance = candidates
        .next()
        .ok_or_else(|| format!("`{name}` does not select one declared canonical data type"))?;
    if candidates.next().is_some() {
        return Err(format!("`{name}` has ambiguous nominal declarations"));
    }
    Ok(instance)
}

fn selected_constructor<'syntax>(
    syntax: &'syntax SyntaxTrees,
    name: &Identifier,
    selection: Option<&ConstantSelection>,
) -> Result<(&'syntax DataDefinition, Option<Identifier>), String> {
    if let Some(selection) = selection {
        return selection.constructor(syntax, name);
    }
    let record = selected_data(syntax, name, None).ok();
    let case = name.as_str().rsplit_once("::").and_then(|(owner, case)| {
        let owner = Identifier::new(owner, name.source_span());
        selected_data(syntax, &owner, None)
            .ok()
            .filter(|definition| {
                syntax.items.data_members(definition.members).iter().any(|member| {
                    matches!(member, DataMember::Variant(variant) if variant.name.as_str() == case)
                })
            })
            .map(|definition| (definition, Identifier::new(case, name.source_span())))
    });
    match (record, case) {
        (Some(definition), None) => Ok((definition, None)),
        (None, Some((definition, case))) => Ok((definition, Some(case))),
        _ => Err(format!(
            "`{name}` does not select one declared constructor carrier"
        )),
    }
}

fn selected_type_label(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    selection: Option<&ConstantSelection>,
) -> Result<String, String> {
    match syntax.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name)
            if !matches!(
                name.as_str(),
                "bool"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "addr"
                    | "f32"
                    | "f64"
                    | "string"
            ) =>
        {
            // A rewritten instance spelling labels through its retained
            // authored application so the label is the synthesized name the
            // instance itself will own.
            let origin = syntax.type_references.generic_application_origin(reference);
            if origin.is_valid() && origin != reference {
                return selected_type_label(syntax, origin, selection);
            }
            Ok(selected_data(syntax, name, selection)?
                .name
                .as_str()
                .to_owned())
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let argument_handles = syntax.type_references.type_reference_handles(*arguments);
            if argument_handles.is_empty() {
                // A synthesized instance's lookup spelling selects the
                // instance declaration itself.
                return Ok(selected_synthesized_instance(syntax, base_name)?
                    .name
                    .as_str()
                    .to_owned());
            }
            let definition = selected_data(syntax, base_name, selection)?;
            generic_application_label(syntax, definition, argument_handles, selection)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Ok(format!(
            "[{}; {length}]",
            selected_type_label(syntax, *element_type, selection)?
        )),
        TypeReferenceNode::Constrained { base_type, .. } => {
            selected_type_label(syntax, *base_type, selection)
        }
        _ => syntax_type_identity(syntax, reference),
    }
}

/// The canonical carrier label of one closed generic application: the exact
/// name the synthesized instance owns (`synthetic_name`), matching the
/// downstream `definition.name` label contract. When the instance already
/// exists its declaration name is authoritative — the deduplicated `__n`
/// spellings can only be reproduced that way. Before synthesis completes, the
/// same leaf-plus-slugs construction predicts it.
fn generic_application_label(
    syntax: &SyntaxTrees,
    definition: &DataDefinition,
    argument_handles: &[TypeReferenceHandle],
    selection: Option<&ConstantSelection>,
) -> Result<String, String> {
    if let Some(instance) = synthesized_instance(syntax, selection, definition, argument_handles) {
        return Ok(instance.name.as_str().to_owned());
    }
    let argument_names =
        if let Some(names) = monomorphizable_argument_slugs(syntax, argument_handles) {
            names
        } else {
            // Declaration evaluation precedes instance synthesis. A nested closed
            // application still has its authored Generic node here, so it has no
            // synthesized slug yet. Require an exact closed identity before using
            // the same recursive label that later instance publication will use.
            let parameters = syntax.items.type_parameters(definition.type_parameters);
            if parameters.len() != argument_handles.len() {
                return Err(
                    "generic const carrier is not a closed monomorphizable application".to_owned(),
                );
            }
            parameters
                .iter()
                .zip(argument_handles)
                .map(|(parameter, argument)| {
                    if let Some(slug) =
                        crate::preparation::generic_data::type_reference_slug(syntax, *argument)
                    {
                        return Ok(slug);
                    }
                    closed_argument_identity(
                        syntax,
                        selection,
                        *argument,
                        matches!(
                            parameter.kind,
                            syntax_trees::item::TypeParameterKind::Const { .. }
                        ),
                    )
                    .ok_or("generic const carrier is not a closed monomorphizable application")?;
                    let label = selected_type_label(syntax, *argument, selection)?;
                    if let TypeReferenceNode::Generic { base_name, .. } =
                        syntax.type_references.type_reference(*argument)
                        && let Some(selection) = selection
                        && let Some(path) = selection.data_lookup_path(syntax, base_name)
                        && let Some((prefix, _)) = path.rsplit_once("::")
                    {
                        Ok(format!("{prefix}::{label}"))
                    } else {
                        Ok(label)
                    }
                })
                .collect::<Result<Vec<_>, String>>()?
        };
    Ok(format!(
        "{}<{}>",
        definition.name.as_str(),
        argument_names.join(", ")
    ))
}

/// The already-synthesized instance for one selected template and closed
/// argument tuple, found by the same `Instance` identity deduplication uses —
/// never by rendered-name equality.
fn synthesized_instance<'syntax>(
    syntax: &'syntax SyntaxTrees,
    selection: Option<&ConstantSelection>,
    template: &DataDefinition,
    argument_handles: &[TypeReferenceHandle],
) -> Option<&'syntax DataDefinition> {
    let parameters = syntax.items.type_parameters(template.type_parameters);
    if parameters.len() != argument_handles.len() {
        return None;
    }
    let identity = parameters
        .iter()
        .zip(argument_handles)
        .map(|(parameter, argument)| {
            closed_argument_identity(
                syntax,
                selection,
                *argument,
                matches!(
                    parameter.kind,
                    syntax_trees::item::TypeParameterKind::Const { .. }
                ),
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let template_handle = syntax.root_item_handles().iter().copied().find(|handle| {
        matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, template))
    })?;
    syntax.root_items().find_map(|item| match item {
        Item::Data(data) => {
            let origin = data.generic_instance?;
            match closed_argument_identity(syntax, selection, origin, false) {
                Some(ClosedArgumentIdentity::Instance(declaration, identities))
                    if declaration == template_handle && identities == identity =>
                {
                    Some(data)
                }
                _ => None,
            }
        }
        _ => None,
    })
}

fn same_carrier(
    syntax: &SyntaxTrees,
    declared: TypeReferenceHandle,
    required: TypeReferenceHandle,
    selection: Option<&ConstantSelection>,
) -> Result<bool, String> {
    // One shared reference node is trivially one carrier; the structural
    // comparison below exists for separately spelled references.
    if declared == required {
        return Ok(true);
    }
    // A retained generic-application origin — on either a surviving `Generic`
    // spelling or the `Named` instance spelling synthesis rewrote it to —
    // identifies the carrier by its selected template and closed argument
    // tuple, never by which spelling reached this comparison.
    if syntax
        .type_references
        .generic_application_origin(declared)
        .is_valid()
        || syntax
            .type_references
            .generic_application_origin(required)
            .is_valid()
    {
        return Ok(
            closed_argument_identity(syntax, selection, declared, false).is_some()
                && closed_argument_identity(syntax, selection, declared, false)
                    == closed_argument_identity(syntax, selection, required, false),
        );
    }
    match (
        syntax.type_references.type_reference(declared),
        syntax.type_references.type_reference(required),
    ) {
        (TypeReferenceNode::Named(left), TypeReferenceNode::Named(right)) => {
            let primitive = |name: &str| {
                matches!(
                    name,
                    "bool"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "addr"
                        | "f32"
                        | "f64"
                        | "string"
                )
            };
            if primitive(left.as_str()) || primitive(right.as_str()) {
                return Ok(left.as_str() == right.as_str());
            }
            // These references borrow the same immutable syntax owner. Its
            // exact declaration entries distinguish derived declarations too;
            // a shared template source span does not establish their identity.
            Ok(std::ptr::eq(
                selected_data(syntax, left, selection)?,
                selected_data(syntax, right, selection)?,
            ))
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: left,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                length: right_length,
            },
        ) => Ok(left_length == right_length && same_carrier(syntax, *left, *right, selection)?),
        (TypeReferenceNode::Constrained { base_type, .. }, _) => {
            same_carrier(syntax, *base_type, required, selection)
        }
        (_, TypeReferenceNode::Constrained { base_type, .. }) => {
            same_carrier(syntax, declared, *base_type, selection)
        }
        // Generic applications compare by their selected template declaration
        // and closed argument tuple — the same identity the instance
        // synthesizer uses — never by their rendered spelling.
        (TypeReferenceNode::Generic { .. }, _) | (_, TypeReferenceNode::Generic { .. }) => {
            match (
                closed_argument_identity(syntax, selection, declared, false),
                closed_argument_identity(syntax, selection, required, false),
            ) {
                (Some(declared_identity), Some(required_identity)) => {
                    Ok(declared_identity == required_identity)
                }
                _ => Ok(false),
            }
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => Ok(true),
        _ => Ok(false),
    }
}

pub(in crate::preparation::generic_data) fn validate_const_index_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    _visiting: &mut HashSet<String>,
) -> Result<(), String> {
    validate_selected_const_index_type(
        syntax,
        type_reference,
        &mut Vec::new(),
        None,
        &GenericApplicationSubstitution::new(),
    )
}

fn validate_selected_const_index_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<source::SourceSpan>,
    selection: Option<&ConstantSelection>,
    substitution: &GenericApplicationSubstitution,
) -> Result<(), String> {
    // A carrier spelled as a bare parameter reselects the enclosing
    // application's already-closed argument handle.
    if let TypeReferenceNode::Named(name) =
        syntax.tables.type_references.type_reference(type_reference)
        && let Some(argument) = substitution.get(name.as_str())
    {
        return validate_selected_const_index_type(
            syntax,
            *argument,
            visiting,
            selection,
            substitution,
        );
    }
    // A rewritten `Named` instance spelling validates through its retained
    // authored application rather than the synthesized name.
    let origin = syntax
        .type_references
        .generic_application_origin(type_reference);
    if origin.is_valid() && origin != type_reference {
        return validate_selected_const_index_type(
            syntax,
            origin,
            visiting,
            selection,
            substitution,
        );
    }
    let no_bindings = GenericApplicationSubstitution::new();
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => {
            if matches!(
                name.as_str(),
                "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                    | "addr"
            ) {
                return Ok(());
            }
            if matches!(name.as_str(), "f32" | "f64" | "string") {
                return Err(format!(
                    "`{name}` is not eligible as a const index: runtime floating/text identity is not canonical structural data"
                ));
            }
            let definition = selected_data(syntax, name, selection)?;
            let declaration = definition.name.source_span();
            if visiting.contains(&declaration) {
                return Ok(());
            }
            if !definition.type_parameters.is_empty() || !definition.lifetime_parameters.is_empty() {
                return Err("generic data const carriers require closed template normalization".to_owned());
            }
            visiting.push(declaration);
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!(
                    "boundary-opaque data `{name}` is not eligible as a const index"
                ));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` is not eligible as a structural const index until quotient-backed canonical representatives land"
                ));
            }
            if !definition.where_facts.is_empty() {
                return Err(format!(
                    "data `{name}` has default-domain facts whose index-site proof is not implemented; it is not yet eligible as a const index"
                ));
            }
            // A nongeneric declaration's members never see an enclosing
            // application's parameters: a member spelling `T` there selects
            // whatever `T` resolves to in this declaration's own scope.
            for member in syntax.tables.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => validate_selected_const_index_type(
                        syntax,
                        field.type_reference,
                        visiting,
                        selection,
                        &no_bindings,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in syntax.tables.items.data_payload_fields(variant.payload) {
                            validate_selected_const_index_type(syntax, field.type_reference, visiting, selection, &no_bindings)?;
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
            visiting.pop();
            Ok(())
        }
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            // A closed application is eligible exactly when its selected
            // template is: parameters bind to the authored argument handles
            // and each member validates under those bindings.
            let argument_handles = syntax
                .tables
                .type_references
                .type_reference_handles(*arguments);
            if argument_handles.is_empty() {
                // A synthesized instance's spelling selects the substituted
                // declaration; its members are already closed.
                let definition = selected_synthesized_instance(syntax, base_name)?;
                let declaration = definition.name.source_span();
                if visiting.contains(&declaration) {
                    return Ok(());
                }
                visiting.push(declaration);
                for member in syntax.tables.items.data_members(definition.members) {
                    match member {
                        DataMember::Field(field) => validate_selected_const_index_type(
                            syntax,
                            field.type_reference,
                            visiting,
                            selection,
                            &no_bindings,
                        )?,
                        DataMember::Variant(variant) => {
                            for field in
                                syntax.tables.items.data_payload_fields(variant.payload)
                            {
                                validate_selected_const_index_type(
                                    syntax,
                                    field.type_reference,
                                    visiting,
                                    selection,
                                    &no_bindings,
                                )?;
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
                visiting.pop();
                return Ok(());
            }
            let definition = selected_data(syntax, base_name, selection)?;
            let parameters = syntax.items.type_parameters(definition.type_parameters);
            if !lifetime_arguments.is_empty()
                || !definition.lifetime_parameters.is_empty()
                || argument_handles.len() != arguments.len()
                || parameters.len() != definition.type_parameters.len()
                || parameters.len() != argument_handles.len()
            {
                return Err(
                    "generic const carriers require closed lifetime-free applications"
                        .to_owned(),
                );
            }
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!(
                    "boundary-opaque data `{base_name}` is not eligible as a const index"
                ));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{base_name}` is not eligible as a structural const index until quotient-backed canonical representatives land"
                ));
            }
            if !definition.where_facts.is_empty() {
                return Err(format!(
                    "data `{base_name}` has default-domain facts whose index-site proof is not implemented; it is not yet eligible as a const index"
                ));
            }
            let declaration = definition.name.source_span();
            if visiting.contains(&declaration) {
                return Ok(());
            }
            visiting.push(declaration);
            let mut next_substitution = GenericApplicationSubstitution::new();
            for (parameter, argument) in parameters.iter().zip(argument_handles.iter()) {
                next_substitution.insert(
                    parameter.name.as_str().to_owned(),
                    resolved_generic_argument(syntax, *argument, substitution)?,
                );
            }
            for member in syntax.tables.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => validate_selected_const_index_type(
                        syntax,
                        field.type_reference,
                        visiting,
                        selection,
                        &next_substitution,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in syntax.tables.items.data_payload_fields(variant.payload) {
                            validate_selected_const_index_type(syntax, field.type_reference, visiting, selection, &next_substitution)?;
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
            visiting.pop();
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            match length {
                FixedArrayLength::Literal(_) => {}
                FixedArrayLength::ConstParameter(name) => {
                    let Some(argument) = substitution.get(name.as_str()) else {
                        return Err(
                            "const index types require finite structural values with decidable equality and one canonical form"
                                .to_owned(),
                        );
                    };
                    let TypeReferenceNode::Named(value) = syntax
                        .tables
                        .type_references
                        .type_reference(*argument)
                    else {
                        return Err(
                            "const index types require finite structural values with decidable equality and one canonical form"
                                .to_owned(),
                        );
                    };
                    if value.as_str().parse::<usize>().is_err() {
                        return Err(
                            "const index types require finite structural values with decidable equality and one canonical form"
                                .to_owned(),
                        );
                    }
                }
                FixedArrayLength::ConstCall(_) => {
                    return Err(
                        "const index types require finite structural values with decidable equality and one canonical form"
                            .to_owned(),
                    );
                }
            }
            validate_selected_const_index_type(
                syntax,
                *element_type,
                visiting,
                selection,
                substitution,
            )
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_selected_const_index_type(syntax, *base_type, visiting, selection, substitution)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::SelfType => Err(
            "const index types require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
    }
}

pub(in crate::preparation::generic_data) fn canonicalize_const_expression(
    syntax: &SyntaxTrees,
    expected_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    selection: Option<&ConstantSelection>,
    substitution: &GenericApplicationSubstitution,
) -> Result<CanonicalConstNode, String> {
    // A carrier spelled as a bare parameter reselects the enclosing
    // application's already-closed argument handle.
    if let TypeReferenceNode::Named(name) =
        syntax.tables.type_references.type_reference(expected_type)
        && let Some(argument) = substitution.get(name.as_str())
    {
        return canonicalize_const_expression(
            syntax,
            *argument,
            expression,
            selection,
            substitution,
        );
    }
    // A rewritten `Named` instance spelling canonicalizes through its
    // retained authored application rather than the synthesized name.
    let origin = syntax
        .type_references
        .generic_application_origin(expected_type);
    if origin.is_valid() && origin != expected_type {
        return canonicalize_const_expression(syntax, origin, expression, selection, substitution);
    }
    match syntax.tables.type_references.type_reference(expected_type) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            canonicalize_const_expression(syntax, *base_type, expression, selection, substitution)
        }
        TypeReferenceNode::Named(type_name)
            if matches!(
                type_name.as_str(),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "addr"
            ) =>
        {
            let ExpressionNode::Integer(literal) = syntax.expressions.expression(expression) else {
                return Err(format!("expected an integer literal for `{type_name}`"));
            };
            if literal.landing().is_some_and(|landing| {
                landing.landed_type.name() != type_name.as_str()
                    || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
            }) {
                return Err(format!(
                    "integer literal landing conflicts with declared const component `{type_name}`"
                ));
            }
            let value = integer_literal_value(literal)
                .ok_or_else(|| "integer literal exceeds the const-value envelope".to_owned())?;
            validate_syntax_integer_range(type_name.as_str(), value)?;
            Ok(CanonicalConstNode::Integer {
                type_name: type_name.as_str().to_owned(),
                value,
            })
        }
        TypeReferenceNode::Named(type_name) if type_name.as_str() == "bool" => {
            let ExpressionNode::Boolean(value) = syntax.expressions.expression(expression) else {
                return Err("expected a boolean literal for `bool`".to_owned());
            };
            Ok(CanonicalConstNode::Boolean(*value))
        }
        TypeReferenceNode::Named(type_name) => {
            canonicalize_data_const_expression(syntax, type_name, expression, selection)
        }
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let argument_handles = syntax
                .tables
                .type_references
                .type_reference_handles(*arguments)
                .to_vec();
            if argument_handles.is_empty() {
                // A synthesized instance's lookup spelling already selects the
                // fully substituted declaration.
                let definition = selected_synthesized_instance(syntax, base_name)?;
                return canonicalize_selected_data_const_expression(
                    syntax,
                    definition,
                    definition.name.as_str(),
                    expression,
                    selection,
                    &GenericApplicationSubstitution::new(),
                );
            }
            let definition = selected_data(syntax, base_name, selection)?;
            let parameters = syntax.items.type_parameters(definition.type_parameters);
            if !lifetime_arguments.is_empty()
                || !definition.lifetime_parameters.is_empty()
                || argument_handles.len() != arguments.len()
                || parameters.len() != definition.type_parameters.len()
                || parameters.len() != argument_handles.len()
            {
                return Err(
                    "generic const carrier is not a closed monomorphizable application".to_owned(),
                );
            }
            let mut next_substitution = GenericApplicationSubstitution::new();
            let mut resolved_arguments = Vec::with_capacity(argument_handles.len());
            for (parameter, argument) in parameters.iter().zip(argument_handles.iter()) {
                let resolved = resolved_generic_argument(syntax, *argument, substitution)?;
                next_substitution.insert(parameter.name.as_str().to_owned(), resolved);
                resolved_arguments.push(resolved);
            }
            // The label must render the resolved arguments: an authored `T`
            // contributes its argument's slug to the synthesized name.
            let label =
                generic_application_label(syntax, definition, &resolved_arguments, selection)?;
            // Constructor normalization may already have selected the closed
            // instance. Rejoin it through the exact template/argument tuple,
            // not its rendered label, before comparing constructor ownership.
            // Before synthesis the same operation uses the template and its
            // explicit substitutions; neither route accepts another tuple.
            if let Some(instance) =
                synthesized_instance(syntax, selection, definition, &resolved_arguments)
            {
                return canonicalize_selected_data_const_expression(
                    syntax,
                    instance,
                    &label,
                    expression,
                    selection,
                    &GenericApplicationSubstitution::new(),
                );
            }
            canonicalize_selected_data_const_expression(
                syntax,
                definition,
                &label,
                expression,
                selection,
                &next_substitution,
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let length =
                match length {
                    FixedArrayLength::Literal(length) => *length,
                    FixedArrayLength::ConstParameter(name) => {
                        let Some(argument) = substitution.get(name.as_str()) else {
                            return Err("const value expression has an ineligible parameter type"
                                .to_owned());
                        };
                        let TypeReferenceNode::Named(value) =
                            syntax.tables.type_references.type_reference(*argument)
                        else {
                            return Err("const value expression has an ineligible parameter type"
                                .to_owned());
                        };
                        value.as_str().parse::<usize>().map_err(|_| {
                            "const value expression has an ineligible parameter type".to_owned()
                        })?
                    }
                    FixedArrayLength::ConstCall(_) => {
                        return Err(
                            "const value expression has an ineligible parameter type".to_owned()
                        );
                    }
                };
            let ExpressionNode::ArrayLiteral(values) = syntax.expressions.expression(expression)
            else {
                return Err("expected an array literal for fixed-array const value".to_owned());
            };
            let values = syntax.expressions.expression_handles(*values);
            if values.len() != length {
                return Err(format!(
                    "fixed-array const value requires {length} elements but has {}",
                    values.len()
                ));
            }
            // Element carriers live in the owning declaration's scope, so the
            // enclosing bindings still apply to their spellings.
            let values = values
                .iter()
                .map(|value| {
                    canonicalize_const_expression(
                        syntax,
                        *element_type,
                        *value,
                        selection,
                        substitution,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CanonicalConstNode::Array {
                type_name: substituted_type_label(syntax, expected_type, substitution, selection)?,
                values,
            })
        }
        TypeReferenceNode::Unit => Err(
            "unit const values do not yet have a source literal; use an empty declared record"
                .to_owned(),
        ),
        _ => Err("const value expression has an ineligible parameter type".to_owned()),
    }
}

/// The canonical carrier label after the enclosing application's bindings have
/// been applied: a member spelled `T` or `Pair<T>` labels as its resolved
/// closed carrier, never its template spelling.
fn substituted_type_label(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    substitution: &GenericApplicationSubstitution,
    selection: Option<&ConstantSelection>,
) -> Result<String, String> {
    match syntax.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name) if substitution.contains_key(name.as_str()) => {
            selected_type_label(syntax, substitution[name.as_str()], selection)
        }
        TypeReferenceNode::Named(_)
            if syntax
                .type_references
                .generic_application_origin(reference)
                .is_valid() =>
        {
            selected_type_label(
                syntax,
                syntax.type_references.generic_application_origin(reference),
                selection,
            )
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let argument_handles = syntax
                .type_references
                .type_reference_handles(*arguments)
                .to_vec();
            if argument_handles.is_empty() {
                return selected_type_label(syntax, reference, selection);
            }
            let definition = selected_data(syntax, base_name, selection)?;
            let resolved = argument_handles
                .iter()
                .map(|argument| resolved_generic_argument(syntax, *argument, substitution))
                .collect::<Result<Vec<_>, _>>()?;
            generic_application_label(syntax, definition, &resolved, selection)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let length = match length {
                FixedArrayLength::Literal(length) => *length,
                FixedArrayLength::ConstParameter(name) => {
                    let Some(argument) = substitution.get(name.as_str()) else {
                        return selected_type_label(syntax, reference, selection);
                    };
                    let TypeReferenceNode::Named(value) =
                        syntax.tables.type_references.type_reference(*argument)
                    else {
                        return selected_type_label(syntax, reference, selection);
                    };
                    match value.as_str().parse::<usize>() {
                        Ok(length) => length,
                        Err(_) => return selected_type_label(syntax, reference, selection),
                    }
                }
                FixedArrayLength::ConstCall(_) => {
                    return selected_type_label(syntax, reference, selection);
                }
            };
            Ok(format!(
                "[{}; {length}]",
                substituted_type_label(syntax, *element_type, substitution, selection)?
            ))
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            substituted_type_label(syntax, *base_type, substitution, selection)
        }
        _ => selected_type_label(syntax, reference, selection),
    }
}

pub(in crate::preparation::generic_data) fn canonicalize_data_const_expression(
    syntax: &SyntaxTrees,
    type_name: &Identifier,
    expression: ExpressionHandle,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstNode, String> {
    let definition = selected_data(syntax, type_name, selection)?;
    // A directly named declaration's members never see an enclosing
    // application's parameters.
    canonicalize_selected_data_const_expression(
        syntax,
        definition,
        definition.name.as_str(),
        expression,
        selection,
        &GenericApplicationSubstitution::new(),
    )
}

fn canonicalize_selected_data_const_expression(
    syntax: &SyntaxTrees,
    definition: &DataDefinition,
    type_name: &str,
    expression: ExpressionHandle,
    selection: Option<&ConstantSelection>,
    substitution: &GenericApplicationSubstitution,
) -> Result<CanonicalConstNode, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::StructLiteral(literal) => {
            let (constructed, case_name) =
                selected_constructor(syntax, &literal.constructor_name, selection)?;
            // A generic application's constructor selects its template, which
            // is the declaration this expression canonicalizes against. When
            // `definition` is a synthesized instance the same constructor
            // legitimately names the instance's template.
            if !std::ptr::eq(constructed, definition) {
                let instance_template = definition
                    .generic_instance
                    .and_then(
                        |origin| match syntax.type_references.type_reference(origin) {
                            TypeReferenceNode::Generic { base_name, .. } => Some(base_name.clone()),
                            _ => None,
                        },
                    )
                    .and_then(|base_name| selected_data(syntax, &base_name, selection).ok());
                if !instance_template.is_some_and(|template| std::ptr::eq(template, constructed)) {
                    return Err(format!(
                        "const constructor `{}` selects a different nominal carrier than `{type_name}`",
                        literal.constructor_name
                    ));
                }
            }
            if let Some(case_name) = &case_name {
                let variant = syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.name.as_str() == case_name.as_str() =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| format!("`{type_name}` has no case `{}`", case_name.as_str()))?;
                let declared_fields = syntax
                    .tables
                    .items
                    .data_payload_fields(variant.payload)
                    .iter()
                    .collect::<Vec<_>>();
                let fields = canonicalize_named_fields(
                    syntax,
                    &declared_fields,
                    literal.fields,
                    selection,
                    substitution,
                )?;
                Ok(CanonicalConstNode::Variant {
                    type_name: type_name.to_owned(),
                    case_name: case_name.as_str().to_owned(),
                    fields,
                })
            } else {
                if syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
                {
                    return Err(format!(
                        "`{type_name}` is case data; its const value must name one case"
                    ));
                }
                let declared_fields = syntax
                    .tables
                    .items
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let fields = canonicalize_named_fields(
                    syntax,
                    &declared_fields,
                    literal.fields,
                    selection,
                    substitution,
                )?;
                Ok(CanonicalConstNode::Record {
                    type_name: type_name.to_owned(),
                    fields,
                })
            }
        }
        ExpressionNode::Name(path) => {
            let path = syntax.expressions.identifier_path_members(*path);
            let Some((case_name, owner)) = path.split_last() else {
                return Err(format!("expected a `{type_name}` structural literal"));
            };
            let Some(first) = owner.first() else {
                return Err(format!("expected a `{type_name}` case owner"));
            };
            let head = Identifier::new(
                owner
                    .iter()
                    .map(Identifier::as_str)
                    .collect::<Vec<_>>()
                    .join("::"),
                first.source_span(),
            );
            let selected = selected_data(syntax, &head, selection)?;
            // A payloadless case spelled through the template still selects the
            // same carrier as the synthesized instance it materializes into.
            let same_carrier = std::ptr::eq(selected, definition)
                || definition
                    .generic_instance
                    .and_then(
                        |origin| match syntax.type_references.type_reference(origin) {
                            TypeReferenceNode::Generic { base_name, .. } => Some(base_name.clone()),
                            _ => None,
                        },
                    )
                    .and_then(|base_name| selected_data(syntax, &base_name, selection).ok())
                    .is_some_and(|template| std::ptr::eq(template, selected));
            if !same_carrier {
                return Err(format!(
                    "expected a `{type_name}` value, got `{}`",
                    head.as_str()
                ));
            }
            let variant = syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                })
                .ok_or_else(|| format!("`{type_name}` has no case `{case_name}`"))?;
            if !variant.payload.is_empty() {
                return Err(format!(
                    "case `{type_name}::{case_name}` requires named payload fields"
                ));
            }
            Ok(CanonicalConstNode::Variant {
                type_name: type_name.to_owned(),
                case_name: case_name.as_str().to_owned(),
                fields: Vec::new(),
            })
        }
        _ => Err(format!("expected a `{type_name}` structural literal")),
    }
}

pub(in crate::preparation::generic_data) fn canonicalize_named_fields(
    syntax: &SyntaxTrees,
    declared_fields: &[&syntax_trees::item::DataField],
    literal_fields: HandleSpan<syntax_trees::expression::TableStructLiteralField>,
    selection: Option<&ConstantSelection>,
    substitution: &GenericApplicationSubstitution,
) -> Result<Vec<(String, CanonicalConstNode)>, String> {
    let authored = syntax.expressions.struct_fields(literal_fields);
    let mut canonical = Vec::with_capacity(declared_fields.len());
    for declared in declared_fields {
        let matches = authored
            .iter()
            .filter(|field| field.name.as_str() == declared.name.as_str())
            .collect::<Vec<_>>();
        let [field] = matches.as_slice() else {
            return Err(if matches.is_empty() {
                format!("missing const field `{}`", declared.name.as_str())
            } else {
                format!("duplicate const field `{}`", declared.name.as_str())
            });
        };
        canonical.push((
            declared.name.as_str().to_owned(),
            canonicalize_const_expression(
                syntax,
                declared.type_reference,
                field.value,
                selection,
                substitution,
            )?,
        ));
    }
    for field in authored {
        if !declared_fields
            .iter()
            .any(|declared| declared.name.as_str() == field.name.as_str())
        {
            return Err(format!("unknown const field `{}`", field.name.as_str()));
        }
    }
    Ok(canonical)
}

pub(in crate::preparation::generic_data) fn validate_syntax_integer_range(
    type_name: &str,
    value: i128,
) -> Result<(), String> {
    let (minimum, maximum) = match type_name {
        "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
        "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
        "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
        "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
        "u8" => (0, i128::from(u8::MAX)),
        "u16" => (0, i128::from(u16::MAX)),
        "u32" => (0, i128::from(u32::MAX)),
        "u64" | "addr" => (0, i128::from(u64::MAX)),
        _ => return Err(format!("`{type_name}` is not an integer const type")),
    };
    if value < minimum || value > maximum {
        Err(format!("const value `{value}` does not fit `{type_name}`"))
    } else {
        Ok(())
    }
}

pub(in crate::preparation::generic_data) fn validate_canonical_rat(
    value: &CanonicalConstNode,
) -> Result<(), String> {
    let CanonicalConstNode::Record { fields, .. } = value else {
        return Err("`Rat` index value must be a structural record".to_owned());
    };
    let numerator = fields
        .iter()
        .find(|(name, _)| name == "num")
        .map(|(_, value)| value)
        .ok_or_else(|| "`Rat` index value is missing `num`".to_owned())?;
    let denominator = fields
        .iter()
        .find(|(name, _)| name == "den")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat` index value is missing `den`".to_owned())?;
    let CanonicalConstNode::Record { fields, .. } = numerator else {
        return Err("`Rat.num` must be an `IntPair` record".to_owned());
    };
    let negative = fields
        .iter()
        .find(|(name, _)| name == "neg")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat.num` is missing `neg`".to_owned())?;
    let positive = fields
        .iter()
        .find(|(name, _)| name == "pos")
        .map(|(_, value)| nat_value(value))
        .transpose()?
        .ok_or_else(|| "`Rat.num` is missing `pos`".to_owned())?;
    if denominator == 0 {
        return Err("`Rat` index denominator must be positive".to_owned());
    }
    if negative != 0 && positive != 0 {
        return Err(
            "`Rat` index signed coordinates must be cancelled (at least one of `num.neg` and `num.pos` must be zero)"
                .to_owned(),
        );
    }
    let magnitude = negative.max(positive);
    if gcd_usize(magnitude, denominator) != 1 {
        return Err(
            "`Rat` index numerator magnitude and denominator must be gcd-reduced".to_owned(),
        );
    }
    Ok(())
}

pub(in crate::preparation::generic_data) fn nat_value(
    value: &CanonicalConstNode,
) -> Result<usize, String> {
    match value {
        CanonicalConstNode::Variant {
            type_name,
            case_name,
            fields,
        } if type_name == "Nat" && case_name == "Zero" && fields.is_empty() => Ok(0),
        CanonicalConstNode::Variant {
            type_name,
            case_name,
            fields,
        } if type_name == "Nat" && case_name == "Succ" => {
            let previous = fields
                .iter()
                .find(|(name, _)| name == "prev")
                .map(|(_, value)| nat_value(value))
                .transpose()?
                .ok_or_else(|| "`Nat::Succ` is missing `prev`".to_owned())?;
            previous
                .checked_add(1)
                .ok_or_else(|| "`Nat` const value is too large".to_owned())
        }
        _ => Err("`Rat` canonicality requires structural core `Nat` fields".to_owned()),
    }
}

pub(in crate::preparation::generic_data) fn gcd_usize(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(in crate::preparation::generic_data) fn const_integer_in_envelope(value: i128) -> Option<i128> {
    (value >= i128::from(i64::MIN) && value <= i128::from(u64::MAX)).then_some(value)
}

pub(in crate::preparation::generic_data) fn checked_fact_integer(
    value: Option<i128>,
    operation: &str,
) -> Result<ConstFactValue, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(ConstFactValue::Integer)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}
