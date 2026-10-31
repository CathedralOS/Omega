//! Infer the checked core locator application from its selected payload.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode, TableStructLiteral},
    machine::Machine,
    name::Identifier,
    state::State,
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};
use validation::native_locators::NativeLocatorVocabulary;

pub(super) fn infer(
    program: &mut TypedTrees,
    machine: &Machine,
    state: &State,
    vocabulary: NativeLocatorVocabulary,
    value: ExpressionHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let existing = validation::expression_result_type_reference(program, machine, state, value);
    if let Some(reference) = existing {
        let application = program
            .type_reference_table
            .peeled_type_reference(reference);
        let base = match program.type_reference_table.type_reference(application) {
            TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
            TypeReferenceNode::Named { symbol, .. } => program
                .data_definition_by_symbol(*symbol)
                .and_then(|data| data.generic_instance)
                .map(|origin| program.type_reference_table.type_symbol(origin)),
            _ => None,
        };
        if base == Some(vocabulary.binding) {
            return Ok(reference);
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(application)
            && symbol.is_valid()
            && matches!(
                program.symbols.get(*symbol).kind,
                symbols::SymbolKind::TypeParameter
            )
        {
            // A generic helper retains its actual formal type. Its ordinary
            // specialization must close it before Terminal/native admission.
            return Ok(reference);
        }
    }
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value).clone()
    else {
        return Err(Diagnostic::error(
            "native binding needs an exact closed ForeignBinding value or an inferable core constructor",
        ));
    };
    if literal.type_symbol != vocabulary.binding {
        return Err(Diagnostic::error(
            "native binding locator must use the exact compiler-owned ForeignBinding carrier",
        ));
    }
    let case = selected_case(program, vocabulary.binding, &literal)?;
    let zero = atom(program, "0".into(), SymbolHandle::invalid());
    let widths = match case.as_str() {
        "Syscall" => [zero; 3],
        "DllImport" => {
            let import = field(program, &literal, "import")?;
            import_widths(program, machine, state, vocabulary, import, zero)?
        }
        _ => {
            return Err(Diagnostic::error(
                "native binding constructor selected a case outside its core schema",
            ));
        }
    };
    application(program, vocabulary.binding, value, widths)
}

fn import_widths(
    program: &mut TypedTrees,
    machine: &Machine,
    state: &State,
    vocabulary: NativeLocatorVocabulary,
    value: ExpressionHandle,
    zero: TypeReferenceHandle,
) -> Result<[TypeReferenceHandle; 3], Diagnostic> {
    if let Some(reference) =
        validation::expression_result_type_reference(program, machine, state, value)
    {
        let reference = program
            .type_reference_table
            .peeled_type_reference(reference);
        if let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program.type_reference_table.type_reference(reference)
            && *base_symbol == vocabulary.dll_import
        {
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if let [object, symbol, version] = arguments {
                return Ok([*object, *symbol, *version]);
            }
        }
    }
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value).clone()
    else {
        return Err(Diagnostic::error(
            "native import needs its exact DllImport application or an inferable core constructor",
        ));
    };
    if literal.type_symbol != vocabulary.dll_import {
        return Err(Diagnostic::error(
            "native import constructor has the wrong exact core owner",
        ));
    }
    let case = selected_case(program, vocabulary.dll_import, &literal)?;
    let coordinates: &[(&str, usize)] = match case.as_str() {
        "PeByName" => &[("library", 0), ("export", 1)],
        "PeByOrdinal" => &[("library", 0)],
        "ElfVersioned" => &[("object", 0), ("symbol", 1), ("version", 2)],
        "MachODylibSymbol" => &[("install_name", 0), ("symbol", 1)],
        _ => {
            return Err(Diagnostic::error(
                "native import case is outside its checked core schema",
            ));
        }
    };
    // The core locator contract fixes inactive coordinates to zero. Active
    // widths come from actual fixed-byte operands, never rendered names or
    // manually counted API arguments. Ordinary formation checks each payload.
    let mut widths = [zero; 3];
    for (name, position) in coordinates {
        let expression = field(program, &literal, name)?;
        widths[*position] = byte_extent(program, machine, state, expression)?;
    }
    application(program, vocabulary.dll_import, value, widths)?;
    Ok(widths)
}

fn selected_case(
    program: &TypedTrees,
    owner: SymbolHandle,
    literal: &TableStructLiteral,
) -> Result<String, Diagnostic> {
    let data = program
        .unique_data_definition_by_symbol(owner)
        .ok_or_else(|| Diagnostic::error("native constructor lost its exact core declaration"))?;
    program
        .data_members(data)
        .iter()
        .filter_map(|member| member.as_variant())
        .find(|case| literal.case_symbol == Some(case.symbol))
        .map(|case| case.name.as_str().to_owned())
        .ok_or_else(|| Diagnostic::error("native constructor has no exact selected core case"))
}

fn field(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    name: &str,
) -> Result<ExpressionHandle, Diagnostic> {
    let mut fields = program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .filter(|field| field.name.as_str() == name);
    let field = fields.next().ok_or_else(|| {
        Diagnostic::error(format!("native locator is missing coordinate `{name}`"))
    })?;
    if fields.next().is_some() {
        return Err(Diagnostic::error(format!(
            "native locator repeats coordinate `{name}`"
        )));
    }
    Ok(field.value)
}

fn byte_extent(
    program: &mut TypedTrees,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let length = match program.expression_table.expression(value) {
        ExpressionNode::String(bytes) => Some(FixedArrayLength::Literal(bytes.len())),
        ExpressionNode::ArrayLiteral(elements) => Some(FixedArrayLength::Literal(elements.len() as usize)),
        _ => validation::expression_result_type_reference(program, machine, state, value)
            .and_then(|reference| match program.type_reference_table.type_reference(program.type_reference_table.peeled_type_reference(reference)) {
                TypeReferenceNode::FixedArray { length, .. } => program.closed_fixed_array_length(reference).map(FixedArrayLength::Literal).or_else(|| Some(length.clone())),
                _ => None,
            }),
    }.ok_or_else(|| Diagnostic::error("native locator coordinate must supply owned fixed bytes; a borrowed or dynamically sized view cannot infer an extent"))?;
    match length {
        FixedArrayLength::Literal(value) => {
            Ok(atom(program, value.to_string(), SymbolHandle::invalid()))
        }
        FixedArrayLength::ConstParameter { name, symbol } => {
            Ok(atom(program, name.as_str().to_owned(), symbol))
        }
        FixedArrayLength::Expression(reference) => Ok(reference),
        FixedArrayLength::ConstCall { name, .. } => Err(Diagnostic::error(format!(
            "native coordinate extent `{name}()` must complete ordinary constant evaluation before its application can be inferred"
        ))),
    }
}

fn atom(program: &mut TypedTrees, spelling: String, symbol: SymbolHandle) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated(spelling),
        })
}

fn application(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    value: ExpressionHandle,
    widths: [TypeReferenceHandle; 3],
) -> Result<TypeReferenceHandle, Diagnostic> {
    let name = program
        .unique_data_definition_by_symbol(owner)
        .ok_or_else(|| Diagnostic::error("native constructor lost its checked owner"))?
        .name
        .clone();
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles(widths);
    let application = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: owner,
            base_name: name,
            lifetime_arguments: Default::default(),
            arguments,
        });
    program
        .expression_table
        .set_constructed_value_type_reference(value, application);
    program
        .expression_table
        .set_construction_required_type_reference(value, application);
    Ok(application)
}
