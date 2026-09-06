//! Check source ownership before referent-oriented structural shape collection.

use super::*;

/// Temporary closed type arguments keep nested instantiations in their lexical
/// environment. Reusing a declaration's parameter symbol must not capture an
/// outer instantiation's argument or turn a stored reference into owned data.
#[derive(Clone, PartialEq, Eq)]
enum OwnedType {
    Scalar(PrimitiveType),
    Array(Box<OwnedType>, usize),
    Data(SymbolHandle, Vec<OwnedType>),
}

impl OwnedType {
    fn size(&self) -> usize {
        match self {
            Self::Scalar(_) => 1,
            Self::Array(element, _) => 1 + element.size(),
            Self::Data(_, arguments) => 1 + arguments.iter().map(Self::size).sum::<usize>(),
        }
    }
}

pub(in crate::flow::terminal_unit) fn has_plain_owned_contents(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    has_plain_owned_contents_with_substitutions(program, reference, &[])
}

pub(super) fn has_plain_owned_contents_with_substitutions(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let mut arguments = Vec::new();
    for (symbol, argument) in substitutions {
        let Some(argument) = resolve(program, *argument, &arguments) else {
            return false;
        };
        arguments.push((*symbol, argument));
    }
    resolve(program, reference, &arguments).is_some_and(|resolved| {
        check_contents(program, &resolved, &mut Vec::new(), &mut Vec::new())
    })
}

fn resolve(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    arguments: &[(SymbolHandle, OwnedType)],
) -> Option<OwnedType> {
    if !reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, argument)) = arguments
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                return Some(argument.clone());
            }
            if let Some(primitive) = program.primitive_type_reference(reference) {
                return Some(OwnedType::Scalar(primitive));
            }
            let data = definition(program, *symbol)?;
            program
                .data_type_parameters(data)
                .is_empty()
                .then_some(OwnedType::Data(*symbol, Vec::new()))
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments: source_arguments,
            ..
        } => {
            let data = definition(program, *base_symbol)?;
            let parameters = program.data_type_parameters(data);
            let source_arguments = program
                .type_reference_table
                .type_reference_handles(*source_arguments);
            if parameters.len() != source_arguments.len()
                || parameters.iter().any(|parameter| {
                    !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
            {
                return None;
            }
            let resolved = source_arguments
                .iter()
                .map(|reference| resolve(program, *reference, arguments))
                .collect::<Option<Vec<_>>>()?;
            Some(OwnedType::Data(*base_symbol, resolved))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length @ 1..),
        } => Some(OwnedType::Array(
            Box::new(resolve(program, *element_type, arguments)?),
            *length,
        )),
        // These require retained loans, qualifications, or a different carrier.
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}

fn definition(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    if !symbol.is_valid() {
        return None;
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == symbol);
    let data = definitions.next()?;
    definitions.next().is_none().then_some(data)
}

fn check_contents(
    program: &TypedTrees,
    resolved: &OwnedType,
    active: &mut Vec<OwnedType>,
    complete: &mut Vec<OwnedType>,
) -> bool {
    if complete.contains(resolved) {
        return true;
    }
    let OwnedType::Data(symbol, arguments) = resolved else {
        return match resolved {
            OwnedType::Scalar(_) => true,
            OwnedType::Array(element, _) => check_contents(program, element, active, complete),
            OwnedType::Data(..) => unreachable!(),
        };
    };
    let Some(data) = definition(program, *symbol) else {
        return false;
    };
    // A repeated declaration must consume finite type-argument structure, as
    // in Wrapper<Wrapper<Value>>. Expanding recursive by-value data is not a
    // finite owned carrier and must not make this classifier recurse forever.
    if data.properties.multiplicity == Multiplicity::Linear
        || active.iter().any(|ancestor| {
            matches!(ancestor, OwnedType::Data(owner, _)
            if owner == symbol && resolved.size() >= ancestor.size())
        })
        || program.machines().iter().any(|machine| {
            machine.attached_data_symbol == *symbol && machine.name.as_str().ends_with("::drop")
        })
    {
        return false;
    }
    let parameters = program.data_type_parameters(data);
    if parameters.len() != arguments.len() {
        return false;
    }
    let substitutions = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.symbol, argument.clone()))
        .collect::<Vec<_>>();
    active.push(resolved.clone());
    let mut check_field = |field: &typed_trees::data::DataField| {
        !field.relevance.is_erased()
            && resolve(program, field.type_reference, &substitutions)
                .is_some_and(|field| check_contents(program, &field, active, complete))
    };
    let supported = program
        .data_members(data)
        .iter()
        .all(|member| match member {
            DataMember::Field(field) => check_field(field),
            DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .all(&mut check_field),
        });
    active.pop();
    if supported {
        complete.push(resolved.clone());
    }
    supported
}
