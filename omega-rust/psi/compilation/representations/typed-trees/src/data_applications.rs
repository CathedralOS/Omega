//! Applied data keeps its authored members and the actual substituted types.
//! This is source metadata formation, never constructor or argument admission.

use crate::TypedTrees;
use crate::data::{DataMember, TypeParameterKind};
use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};

impl TypedTrees {
    /// Retain views for the new source references before constructor landing.
    /// Views do not publish declarations or establish their generic premises.
    pub fn prepare_data_application_views(&mut self, first: usize) -> Result<(), String> {
        let references: Vec<_> = self
            .type_reference_table
            .type_reference_rows_from(first)
            .filter_map(|(reference, node)| match node {
                TypeReferenceNode::Generic { base_symbol, .. }
                    if self
                        .unique_data_definition_by_symbol(*base_symbol)
                        .is_some() =>
                {
                    Some(reference)
                }
                _ => None,
            })
            .collect();
        let mut preparation = ApplicationPreparation::new(self);
        for reference in references {
            preparation.reference(reference)?;
        }
        Ok(())
    }

    /// Form one already-resolved application, including its argument views.
    /// `true` means its types are closed; callers still owe every source
    /// requirement and must judge the actual value or call they construct.
    pub fn prepare_data_application_view(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<bool, String> {
        ApplicationPreparation::new(self).reference(reference)
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
        &mut self,
        argument: TypeReferenceHandle,
        spelling: &str,
    ) -> Result<(), String> {
        if !matches!(
            self.type_reference_table.type_reference(argument),
            TypeReferenceNode::ConstExpression(_)
        ) {
            return Err("a settled const argument is not an expression to evaluate".into());
        }
        let mut extents = Vec::new();
        let mut applications = Vec::new();
        for (reference, node) in self.type_reference_table.type_reference_rows() {
            match node {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Expression(extent),
                } if *extent == argument => extents.push((reference, *element_type)),
                TypeReferenceNode::Generic { arguments, .. }
                    if self.data_application_definition(reference).is_some()
                        && !self.data_application_is_closed(reference) =>
                {
                    for child in self.type_reference_table.type_reference_handles(*arguments) {
                        if self.mentions(*child, argument, 0)? {
                            applications.push(reference);
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        self.type_reference_table.substitute_node(
            argument,
            TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: crate::name::Identifier::generated(spelling),
            },
        );
        if !extents.is_empty() {
            let length = self
                .type_reference_table
                .anonymous_numeric_literal_value(argument)
                .ok_or("an applied array length is not a closed nonnegative integer")?;
            for (reference, element_type) in extents {
                self.type_reference_table.substitute_node(
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
            self.forget_data_application_definition(*reference);
        }
        for reference in applications {
            self.prepare_data_application_view(reference)?;
        }
        Ok(())
    }

    /// Whether the type `reference` is or contains the reference `argument`.
    /// A malformed or excessively deep dependency cannot establish that an
    /// application view is independent of the argument being settled.
    fn mentions(
        &self,
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
        if !self.type_reference_table.contains_type_reference(reference) {
            return Err("data application has a stale type dependency".into());
        }
        if depth > 64 {
            return Err("data application has an excessively deep type dependency".into());
        }
        let mentions = |child: &TypeReferenceHandle| self.mentions(*child, argument, depth + 1);
        match self.type_reference_table.type_reference(reference) {
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
            TypeReferenceNode::Generic { arguments, .. } => self
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
                let signature = &self
                    .declared_callable_contract(*contract)
                    .map_err(str::to_owned)?
                    .signature;
                crate::references::signature_type_inputs(self, signature)
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
        let Some(template) = self.source.unique_data_definition_by_symbol(owner).cloned() else {
            // Applications of machines and other namespaces retain their own
            // source records; this operation does not reinterpret them as data.
            return Ok(false);
        };
        if !self.source.symbols.symbols().nodes().is_valid(owner)
            || self.source.symbols.get(owner).kind != SymbolKind::Data
        {
            return Err("data application owner has a non-data symbol".into());
        }
        let parameters = self.source.data_type_parameters(&template).to_vec();
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
                    TypeParameterKind::Type
                        | TypeParameterKind::Const { .. }
                        | TypeParameterKind::Implementation
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
                || self.source.symbols.get(parameter.symbol).kind
                    != if parameter.kind == TypeParameterKind::Implementation {
                        SymbolKind::ConformanceParameter
                    } else {
                        SymbolKind::TypeParameter
                    }
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
            // Keep the authored application untouched, including a literal's
            // carrier. Its member types need the canonical integer value so
            // array extents substitute identically for both literal encodings.
            let substituted = if matches!(parameter.kind, TypeParameterKind::Const { .. }) {
                self.source
                    .static_const_literal_from_type_reference(argument)
                    .and_then(|literal| {
                        literal
                            .value_i64()
                            .map(i128::from)
                            .or_else(|| literal.value_u64().map(i128::from))
                    })
                    .map(|value| {
                        self.source
                            .type_reference_table
                            .insert(TypeReferenceNode::Named {
                                symbol: SymbolHandle::invalid(),
                                name: crate::name::Identifier::generated(value.to_string()),
                            })
                    })
                    .unwrap_or(argument)
            } else {
                argument
            };
            substitutions.push((parameter.symbol, substituted));
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
        let mut closed = true;
        for (parameter, (_, argument)) in parameters.iter().zip(&substitutions) {
            closed &= match parameter.kind {
                TypeParameterKind::Type => self.reference(*argument)?,
                TypeParameterKind::Const { .. } => {
                    self.source
                        .static_const_literal_from_type_reference(*argument)
                        .is_some()
                        || crate::canonical_const_argument_value(self.source, *argument).is_some()
                }
                TypeParameterKind::Implementation => {
                    // This closes the retained value tuple, not its schema
                    // constraints, member refinements or conformance verdict.
                    // A data declaration cannot masquerade as a static record.
                    crate::domain::index_argument_is_closed(self.source, *argument)
                        && self
                            .source
                            .applied_requirement_scope(owner, *argument)
                            .is_some()
                        && self
                            .source
                            .require_owned_application_operands(&[*argument], owner)
                            .is_ok()
                }
                _ => false,
            };
        }
        let mut definition = template.clone();
        definition.type_parameters = HandleSpan::empty();
        definition.generic_instance = Some(reference);
        for bound in &mut definition.conformance_bounds {
            for argument in &mut bound.arguments {
                *argument = self
                    .source
                    .type_reference_table
                    .instantiate(*argument, &substitutions)
                    .map_err(str::to_owned)?;
            }
        }
        definition.predicate_subject_type = self
            .source
            .type_reference_table
            .instantiate(template.predicate_subject_type, &substitutions)
            .map_err(str::to_owned)?;
        let mut members = self.source.data_members(&template).to_vec();
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
        closed &= children
            .iter()
            .all(|child| crate::domain::index_argument_is_closed(self.source, *child));
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
mod tests {
    use super::*;
    use crate::data::{DataDefinition, DataField, DataVariant, TypeParameter};
    use crate::name::Identifier;
    use symbols::{SymbolNameRef, SymbolTableBuilder};

    /// `data Chain<T> { case Link(head: T, tail: Chain<T>); }` with the
    /// references `Chain<T>` (the member's own type) and `Chain<u8>`.
    fn chain() -> (TypedTrees, TypeReferenceHandle, TypeReferenceHandle) {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            symbols::builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Data, SymbolNameRef::Static("Chain"))]),
        );
        let data = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let formal = builder
            .insert_children(
                data,
                [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
            )
            .start();
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let named = |program: &mut TypedTrees, symbol, name: &'static str| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
        };
        let applied = |program: &mut TypedTrees, argument| {
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles([argument]);
            program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol: data,
                    base_name: Identifier::generated("Chain"),
                    arguments,
                    lifetime_arguments: Vec::new(),
                })
        };
        let parameter = named(&mut program, formal, "T");
        let byte = named(
            &mut program,
            children.handle_at(symbols::BuiltinTypeAtom::U8.ordinal() as u32),
            "u8",
        );
        let open = applied(&mut program, parameter);
        let closed = applied(&mut program, byte);
        let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: formal,
            name: Identifier::generated("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        }]);
        let payload = program.tables.data_payload_fields.insert_many([
            DataField {
                name: Identifier::generated("head"),
                type_reference: parameter,
                ..Default::default()
            },
            DataField {
                name: Identifier::generated("tail"),
                type_reference: open,
                ..Default::default()
            },
        ]);
        let members = program
            .tables
            .data_members
            .insert_many([DataMember::Variant(DataVariant {
                name: Identifier::generated("Link"),
                payload,
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: data,
            name: Identifier::generated("Chain"),
            type_parameters,
            members,
            ..Default::default()
        });
        (program, open, closed)
    }

    #[test]
    fn data_that_stores_its_own_application_keeps_one_view_of_it() {
        let (mut program, open, closed) = chain();
        program.prepare_data_application_views(0).unwrap();
        for (reference, is_closed) in [(open, false), (closed, true)] {
            let view = program
                .data_application_definition(reference)
                .expect("the application retains its view");
            let DataMember::Variant(link) = &program.data_members(view)[0] else {
                panic!("the Link case")
            };
            // The `tail` member of `Chain<T>` is that same reference; the
            // one of `Chain<u8>` is an equal application with its own view.
            let tail = program.data_payload_fields(link)[1].type_reference;
            if reference == open {
                assert_eq!(tail, reference);
            }
            assert!(program.data_application_definition(tail).is_some());
            assert_eq!(
                program.prepare_data_application_view(reference),
                Ok(is_closed)
            );
        }
    }

    #[test]
    fn canonical_const_carriers_check_inactive_payloads_and_cycles() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::Data, SymbolNameRef::Static("Choice")),
                (SymbolKind::Data, SymbolNameRef::Static("Linear")),
            ]),
        );
        let choice = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let linear = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
        let cases = builder.insert_children(
            choice,
            [
                (SymbolKind::Variant, SymbolNameRef::Static("Empty")),
                (SymbolKind::Variant, SymbolNameRef::Static("Value")),
            ],
        );
        let field = builder
            .insert_children(
                cases.handle_at(1),
                [(SymbolKind::Field, SymbolNameRef::Static("value"))],
            )
            .start();
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let mut named = |symbol, name: &str| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: name.into(),
                })
        };
        let integer = named(
            children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32),
            "u64",
        );
        let choice_type = named(choice, "Choice");
        let linear_type = named(linear, "Linear");
        let payload = program.data_payload_fields.insert_many([DataField {
            symbol: field,
            name: "value".into(),
            type_reference: integer,
            ..Default::default()
        }]);
        let members = program.data_members.insert_many([
            DataMember::Variant(DataVariant {
                symbol: cases.handle_at(0),
                name: "Empty".into(),
                ..Default::default()
            }),
            DataMember::Variant(DataVariant {
                symbol: cases.handle_at(1),
                name: "Value".into(),
                payload,
                ..Default::default()
            }),
        ]);
        program.push_data_definition(DataDefinition {
            symbol: choice,
            name: "Choice".into(),
            members,
            properties: crate::data::DataProperties {
                multiplicity: language_semantics::Multiplicity::Unrestricted,
                ..Default::default()
            },
            ..Default::default()
        });
        program.push_data_definition(DataDefinition {
            symbol: linear,
            name: "Linear".into(),
            ..Default::default()
        });
        assert!(crate::canonical_const_carrier_is_supported(
            &program,
            choice_type
        ));
        let borrowed = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: integer,
                access: language_semantics::ReferenceAccess::Shared,
                lifetime: None,
            });
        // Even an Empty value cannot make the other case's reference or
        // linear storage eligible for a canonical constant.
        for ineligible in [borrowed, linear_type] {
            program
                .data_payload_fields
                .get_mut(payload.start())
                .type_reference = ineligible;
            assert!(!crate::canonical_const_carrier_is_supported(
                &program,
                choice_type
            ));
        }
        program
            .data_payload_fields
            .get_mut(payload.start())
            .type_reference = integer;
        let array = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: choice_type,
                length: FixedArrayLength::Literal(3),
            });
        assert!(crate::canonical_const_carrier_is_supported(&program, array));
        program.type_reference_table.substitute_node(
            array,
            TypeReferenceNode::FixedArray {
                element_type: array,
                length: FixedArrayLength::Literal(3),
            },
        );
        assert!(!crate::canonical_const_carrier_is_supported(
            &program, array
        ));
        assert!(!crate::canonical_const_carrier_is_supported(
            &program,
            TypeReferenceHandle::invalid()
        ));
    }

    #[test]
    fn a_type_that_contains_itself_with_no_declaration_between_is_refused() {
        let (mut program, _, closed) = chain();
        let looped = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: closed,
            });
        program.type_reference_table.substitute_node(
            looped,
            TypeReferenceNode::Slice {
                element_type: looped,
            },
        );
        assert_eq!(
            program.prepare_data_application_view(looped),
            Err("data application has a cyclic type-reference graph".into())
        );
    }
}
