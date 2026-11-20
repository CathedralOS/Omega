//! Substitute a data declaration’s type and constant arguments into its member types.
//! Typing forms the first views. Specialization and compile-time evaluation use
//! the same operation when they create types or settle a computed constant.
//! Each view stays associated with its original source type occurrence.

use crate::source_state::TypedTrees;
use crate::source_state::data::{DataMember, TypeParameterKind};
use crate::source_state::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};

/// Retain views for the new source references before constructor landing.
/// Views do not publish declarations or establish their generic premises.
pub fn prepare_from(program: &mut TypedTrees, first: usize) -> Result<(), String> {
    let references: Vec<_> = program
        .type_reference_table
        .type_reference_rows_from(first)
        .filter_map(|(reference, node)| match node {
            TypeReferenceNode::Generic { base_symbol, .. }
                if program
                    .unique_data_definition_by_symbol(*base_symbol)
                    .is_some() =>
            {
                Some(reference)
            }
            _ => None,
        })
        .collect();
    let mut preparation = ApplicationPreparation::new(program);
    for reference in references {
        preparation.reference(reference)?;
    }
    Ok(())
}

/// Form one already-resolved application, including its argument views.
/// `true` means its types are closed; callers still owe every source
/// requirement and must judge the actual value or call they construct.
pub fn prepare(program: &mut TypedTrees, reference: TypeReferenceHandle) -> Result<bool, String> {
    ApplicationPreparation::new(program).reference(reference)
}

/// Gives `argument`, a const argument written as an expression to
/// evaluate (`Buffer<capacity()>`), the value an evaluator computed for
/// it, as the canonical spelling a literal of that value has.
///
/// generics.md, "Transparent type aliases": "A computed const value
/// retains its declared carrier and canonical value". So the argument
/// becomes the same node typing writes for a literal argument
/// (`Buffer<4>`), and every reader of either application reads one form.
///
/// Typing applied the data type before the value existed: an array
/// length taken from the argument is still that argument
/// (`FixedArrayLength::Expression`), and the views of the applications
/// that mention it are open. Both are settled here, in place, so a
/// handle typing already handed out keeps naming the same type. A value
/// that is no array length (`-1`) is the error it is for a literal.
pub fn settle_const_argument(
    program: &mut TypedTrees,
    argument: TypeReferenceHandle,
    spelling: &str,
) -> Result<(), String> {
    if !matches!(
        program.type_reference_table.type_reference(argument),
        TypeReferenceNode::ConstExpression(_)
    ) {
        return Err("a settled const argument is not an expression to evaluate".into());
    }
    let mut extents = Vec::new();
    let mut applications = Vec::new();
    for (reference, node) in program.type_reference_table.type_reference_rows() {
        match node {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Expression(extent),
            } if *extent == argument => extents.push((reference, *element_type)),
            TypeReferenceNode::Generic { arguments, .. }
                if program.data_application_definition(reference).is_some()
                    && !program.data_application_is_closed(reference) =>
            {
                for child in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    if mentions(program, *child, argument, 0)? {
                        applications.push(reference);
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    // A bad extent leaves the expression and every dependent view available
    // for a later settlement at the same source handle.
    let length = if extents.is_empty() {
        None
    } else {
        Some(spelling.parse::<usize>().map_err(|_| {
            "an applied array length is not a closed nonnegative integer".to_owned()
        })?)
    };
    program.type_reference_table.substitute_node(
        argument,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: crate::source_state::name::Identifier::generated(spelling),
        },
    );
    if let Some(length) = length {
        for (reference, element_type) in extents {
            program.type_reference_table.substitute_node(
                reference,
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
            );
        }
    }
    // Forget every stale view before forming any: a view formed while a
    // nested one is still stale would read that one's old verdict.
    for reference in &applications {
        program.discard_open_data_application(*reference);
    }
    for reference in applications {
        prepare(program, reference)?;
    }
    Ok(())
}

/// Whether the type `reference` is or contains the reference `argument`.
/// A malformed or excessively deep dependency cannot establish that an
/// application view is independent of the argument being settled.
fn mentions(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    argument: TypeReferenceHandle,
    depth: usize,
) -> Result<bool, String> {
    if reference == argument {
        return Ok(true);
    }
    if reference == TypeReferenceHandle::invalid() {
        return Ok(false);
    }
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err("data application has a stale type dependency".into());
    }
    if depth > 64 {
        return Err("data application has an excessively deep type dependency".into());
    }
    let mentions = |child: &TypeReferenceHandle| mentions(program, *child, argument, depth + 1);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => mentions(referee),
        TypeReferenceNode::Slice { element_type } => mentions(element_type),
        TypeReferenceNode::Constrained { base_type, .. } => mentions(base_type),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            if let FixedArrayLength::Expression(extent) = length
                && mentions(extent)?
            {
                return Ok(true);
            }
            mentions(element_type)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .try_fold(
                false,
                |found, child| {
                    if found { Ok(true) } else { mentions(child) }
                },
            ),
        TypeReferenceNode::Callable { contract } => {
            let signature = &program
                .declared_callable_contract(*contract)
                .map_err(str::to_owned)?
                .signature;
            crate::source_state::references::signature_type_inputs(program, signature)
                .types
                .iter()
                .try_fold(
                    false,
                    |found, child| {
                        if found { Ok(true) } else { mentions(child) }
                    },
                )
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => Ok(false),
    }
}

struct ApplicationPreparation<'source> {
    source: &'source mut TypedTrees,
    active_references: Vec<TypeReferenceHandle>,
    active_declarations: Vec<SymbolHandle>,
}

impl<'source> ApplicationPreparation<'source> {
    fn new(source: &'source mut TypedTrees) -> Self {
        Self {
            source,
            active_references: Vec::new(),
            active_declarations: Vec::new(),
        }
    }

    fn reference(&mut self, reference: TypeReferenceHandle) -> Result<bool, String> {
        if !self
            .source
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("data application refers to a missing type reference".into());
        }
        if self.active_references.contains(&reference) {
            // `data Seq<T> { case Cons(head: T, tail: Seq<T>); }` stores its
            // own application at the same arguments, and that member is this
            // very reference. Its view was retained before its members were
            // entered, so the member reads that view and is not expanded a
            // second time. Any other revisit is a type that contains itself
            // with no declaration in between, which no source type is.
            if self.source.data_application_definition(reference).is_some() {
                return Ok(self.source.data_application_is_closed(reference));
            }
            return Err("data application has a cyclic type-reference graph".into());
        }
        self.active_references.push(reference);
        let result = self.form(reference);
        self.active_references.pop();
        result
    }

    fn form(&mut self, reference: TypeReferenceHandle) -> Result<bool, String> {
        match self
            .source
            .type_reference_table
            .type_reference(reference)
            .clone()
        {
            TypeReferenceNode::Named { symbol, .. } => {
                if !self.source.symbols.symbols().nodes().is_valid(symbol) {
                    return Err("data application has an unresolved type argument".into());
                }
                if self.source.symbols.builtin_type_atom(symbol).is_some() {
                    return Ok(true);
                }
                Ok(self
                    .source
                    .unique_data_definition_by_symbol(symbol)
                    .is_some_and(|data| data.type_parameters.is_empty()))
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => self.application(
                reference,
                base_symbol,
                self.source
                    .type_reference_table
                    .type_reference_handles(arguments)
                    .to_vec(),
                lifetime_arguments.len(),
            ),
            TypeReferenceNode::Reference { referee, .. } => self.reference(referee),
            TypeReferenceNode::Slice { element_type } => self.reference(element_type),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => Ok(matches!(length, FixedArrayLength::Literal(_)) && self.reference(element_type)?),
            TypeReferenceNode::Constrained { base_type, .. } => self.reference(base_type),
            TypeReferenceNode::Callable { contract } => {
                self.source
                    .declared_callable_contract(contract)
                    .map_err(str::to_owned)?;
                self.source
                    .runtime_callable_contract_identity(contract, &[])
                    .map_err(|error| error.to_string())?;
                Ok(true)
            }
            TypeReferenceNode::Unit => Ok(true),
            TypeReferenceNode::ConstExpression(_) => Ok(false),
        }
    }

    fn application(
        &mut self,
        reference: TypeReferenceHandle,
        owner: SymbolHandle,
        arguments: Vec<TypeReferenceHandle>,
        lifetimes: usize,
    ) -> Result<bool, String> {
        let Some(template) = self.source.unique_data_definition_by_symbol(owner) else {
            // Applications of machines and other namespaces retain their own
            // source records; this operation does not reinterpret them as data.
            return Ok(false);
        };
        if !self.source.symbols.symbols().nodes().is_valid(owner)
            || self.source.symbols.get(owner).kind != SymbolKind::Data
        {
            return Err("data application owner has a non-data symbol".into());
        }
        let parameters = self.source.data_type_parameters(template);
        if lifetimes != template.lifetime_parameters.len() || parameters.len() != arguments.len() {
            return Err("data application does not retain its complete argument tuple".into());
        }
        if parameters.is_empty() {
            return Ok(true);
        }
        if lifetimes != 0
            || parameters.iter().any(|parameter| {
                !matches!(
                    parameter.kind,
                    TypeParameterKind::Type | TypeParameterKind::Const { .. }
                )
            })
        {
            return Ok(false);
        }
        let mut substitutions = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !self
                .source
                .symbols
                .symbols()
                .nodes()
                .is_valid(parameter.symbol)
                || self.source.symbols.get(parameter.symbol).kind != SymbolKind::TypeParameter
                || self.source.symbols.get(parameter.symbol).parent != owner
                || substitutions
                    .iter()
                    .any(|(known, _)| *known == parameter.symbol)
            {
                return Err(
                    "data application has a foreign or duplicated declaration binder".into(),
                );
            }
            if !self
                .source
                .type_reference_table
                .contains_type_reference(argument)
            {
                return Err("data application refers to a missing type reference".into());
            }
            substitutions.push((parameter.symbol, argument));
        }
        // Normalized source references remain immutable while this owner
        // appends their views. Reuse that exact association, not a second
        // whole-program scratch table for each receiving occurrence.
        if let Some(retained) = self.source.data_application_definition(reference) {
            if retained.symbol != owner || retained.generic_instance != Some(reference) {
                return Err("data application view belongs to another source occurrence".into());
            }
            return Ok(self.source.data_application_is_closed(reference));
        }
        let parameters = parameters.to_vec();
        let mut members = self.source.data_members(template).to_vec();
        let mut definition = template.clone();
        // Only a newly formed view needs canonical integers in its member types.
        for (parameter, (_, argument)) in parameters.iter().zip(&mut substitutions) {
            if matches!(parameter.kind, TypeParameterKind::Const { .. })
                && let Some(value) = self
                    .source
                    .static_const_literal_from_type_reference(*argument)
                    .and_then(|literal| {
                        literal
                            .value_i64()
                            .map(i128::from)
                            .or_else(|| literal.value_u64().map(i128::from))
                    })
            {
                *argument = self
                    .source
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: crate::source_state::name::Identifier::generated(value.to_string()),
                    });
            }
        }
        let mut closed = true;
        for (parameter, (_, argument)) in parameters.iter().zip(&substitutions) {
            closed &= match parameter.kind {
                TypeParameterKind::Type => self.reference(*argument)?,
                TypeParameterKind::Const { .. } => {
                    self.source
                        .static_const_literal_from_type_reference(*argument)
                        .is_some()
                        || crate::source_state::canonical_const_argument_value(
                            self.source,
                            *argument,
                        )
                        .is_some()
                }
                _ => false,
            };
        }
        definition.type_parameters = HandleSpan::empty();
        definition.generic_instance = Some(reference);
        definition.predicate_subject_type = self
            .source
            .type_reference_table
            .instantiate(definition.predicate_subject_type, &substitutions)
            .map_err(str::to_owned)?;
        let mut children = Vec::new();
        for member in &mut members {
            match member {
                DataMember::Field(field) => {
                    field.type_reference = self
                        .source
                        .type_reference_table
                        .instantiate(field.type_reference, &substitutions)
                        .map_err(str::to_owned)?;
                    children.push(field.type_reference);
                }
                DataMember::Variant(variant) => {
                    let mut fields = self.source.data_payload_fields(variant).to_vec();
                    for field in &mut fields {
                        field.type_reference = self
                            .source
                            .type_reference_table
                            .instantiate(field.type_reference, &substitutions)
                            .map_err(str::to_owned)?;
                        children.push(field.type_reference);
                    }
                    variant.payload = self.source.tables.data_payload_fields.insert_many(fields);
                }
            }
        }
        definition.members = self.source.tables.data_members.insert_many(members);
        // Keep the authored member symbols, requirements, storage properties
        // and source origins. Only the type telescope is applied here.
        closed &= children.iter().all(|child| {
            crate::source_state::domain::index_argument_is_closed(self.source, *child)
        });
        self.source
            .retain_data_application_definition(reference, definition, closed);
        if self.active_declarations.contains(&owner) {
            // Repeated declarations through their member definitions may
            // describe recursive storage. Retain this exact shallow view;
            // do not expand an unbounded family or grant it a finite layout.
            // Actual type arguments were prepared before entering the owner,
            // so finite nested applications still retain all their views.
            return Ok(closed);
        }
        self.active_declarations.push(owner);
        let prepared = (|| {
            for child in children {
                self.reference(child)?;
            }
            Ok(closed)
        })();
        self.active_declarations.pop();
        prepared
    }
}

#[cfg(test)]
mod tests;
