//! Substitute a data declaration’s type and constant arguments into its member types.
//! Typing forms the first views. Specialization and compile-time evaluation use
//! the same operation when they create types or settle a computed constant.
//! Each view stays associated with its original source type occurrence.

use crate::source_state::TypedTrees;
use crate::source_state::data::{DataMember, MachineParameterContract, TypeParameterKind};
use crate::source_state::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};

/// Each written lifetime is bound where its operand was supplied. A substituted
/// lifetime can already carry that binder's identity under a different name.
pub fn check_supplied_lifetimes(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    owner: SymbolHandle,
    arguments: &[crate::source_state::name::Identifier],
) -> Result<(), String> {
    use crate::source_state::type_identity::{
        lifetime_parameter_identity, lifetime_parameter_name,
    };

    let retained = program.type_reference_table.operand_lifetimes(reference);
    for (slot, argument) in arguments.iter().enumerate() {
        let region = retained
            .get(slot)
            .copied()
            .filter(|region| region.declaration.is_valid())
            .or_else(|| lifetime_parameter_identity(program, owner, argument.as_str()));
        if region
            .and_then(|id| lifetime_parameter_name(program, id))
            .is_none()
        {
            return Err(format!(
                "lifetime argument `'{}` must name a lifetime in its supplying declaration",
                argument.as_str().trim_start_matches('\'')
            ));
        }
    }
    Ok(())
}

/// A type argument must denote a type, including beneath a structural
/// wrapper. Its named head must receive its own declaration's full tuple.
pub fn check_type_argument(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    mut owner: SymbolHandle,
) -> Result<(), String> {
    let table = &program.type_reference_table;
    let mut remaining = table.type_reference_count();
    let (symbol, supplied_lifetimes, supplied_parameters) = loop {
        if remaining == 0 || !table.contains_type_reference(reference) {
            return Err("type argument contains a missing or cyclic type reference".into());
        }
        remaining -= 1;
        let supplier = table.operand_lifetime_owner(reference);
        if supplier.is_valid() {
            owner = supplier;
        }
        match table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee, lifetime, ..
            } => {
                check_supplied_lifetimes(program, reference, owner, lifetime.as_slice())?;
                reference = *referee;
            }
            TypeReferenceNode::Constrained {
                base_type: referee, ..
            }
            | TypeReferenceNode::FixedArray {
                element_type: referee,
                ..
            }
            | TypeReferenceNode::Slice {
                element_type: referee,
            } => reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => break (*symbol, 0, 0),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => {
                check_supplied_lifetimes(program, reference, owner, lifetime_arguments)?;
                break (*base_symbol, lifetime_arguments.len(), arguments.len());
            }
            _ => return Ok(()),
        }
    };
    if !program.symbols.symbols().nodes().is_valid(symbol) {
        return Ok(());
    }
    let expected = if let Some(data) = program.unique_data_definition_by_symbol(symbol) {
        (
            data.lifetime_parameters.len(),
            program.data_type_parameters(data).len(),
        )
    } else if program.symbols.builtin_type_atom(symbol).is_some()
        || program
            .data_type_parameter_by_symbol(symbol)
            .is_some_and(|parameter| parameter.kind == TypeParameterKind::Type)
    {
        (0, 0)
    } else {
        return Ok(());
    };
    if expected != (supplied_lifetimes, supplied_parameters) {
        return Err(format!(
            "type argument `{}` has the wrong arity (expected {} lifetime and {} ordinary arguments, found {} and {})",
            program.symbols.name(symbol),
            expected.0,
            expected.1,
            supplied_lifetimes,
            supplied_parameters
        ));
    }
    Ok(())
}

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
            TypeReferenceNode::Named { symbol, name } => {
                if !self.source.symbols.symbols().nodes().is_valid(symbol) {
                    return Err(format!("unknown type `{name}`"));
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
            TypeReferenceNode::FixedArray { element_type, .. } => {
                let element_closed = self.reference(element_type)?;
                Ok(element_closed && self.source.closed_fixed_array_length(reference).is_some())
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                // Instantiating the member's type substitutes its carrier and
                // written indices. Retain the domain at that same application
                // before a field or selected case can use its membership.
                let closed = self.reference(base_type)?;
                let mut changed = None;
                for position in 0..constraints.len() {
                    let rows = self.source.type_reference_table.constraints(constraints);
                    let TypeConstraintNode::Domain(domain) = &rows[position] else {
                        continue;
                    };
                    if domain.subject != DomainConstraintSubject::Declared
                        || self
                            .source
                            .domain_definition(domain.symbol)
                            .is_none_or(|definition| definition.type_parameters.is_empty())
                    {
                        continue;
                    }
                    let previous = domain.semantic_id;
                    let mut applied = domain.clone();
                    // An index can itself be qualified by a domain on this
                    // declaration's type parameter. Prepare its substituted
                    // carrier before interning the enclosing domain coordinate.
                    for &argument in &applied.arguments {
                        self.reference(argument)?;
                    }
                    self.source
                        .rebind_domain_constraint(base_type, &mut applied)?;
                    if previous != applied.semantic_id {
                        let rows = changed.get_or_insert_with(|| {
                            self.source
                                .type_reference_table
                                .constraints(constraints)
                                .to_vec()
                        });
                        rows[position] = TypeConstraintNode::Domain(applied);
                    }
                }
                if let Some(changed) = changed {
                    let constraints = self.source.type_reference_table.insert_constraints(changed);
                    self.source
                        .type_reference_table
                        .set_constraint_span(reference, constraints);
                }
                Ok(closed)
            }
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

    /// A selected implementation closes a data argument only at its declared trait application.
    /// The trait's type arguments are interpreted under this data application's telescope.
    fn closed_implementation(
        &self,
        application: TypeReferenceHandle,
        data: &crate::source_state::data::DataDefinition,
        parameter: &crate::source_state::data::TypeParameter,
        argument: TypeReferenceHandle,
    ) -> Result<bool, String> {
        use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationArguments};

        let program = &*self.source;
        if !crate::source_state::domain::index_argument_is_closed(program, application) {
            return Ok(false);
        }
        let (requirement, arguments, lifetimes) = match parameter.kind {
            TypeParameterKind::Implementation => {
                let mut bounds = data
                    .conformance_bounds
                    .iter()
                    .filter(|bound| bound.binder == parameter.symbol);
                let (Some(bound), None) = (bounds.next(), bounds.next()) else {
                    return Err(
                        "data implementation parameter must have one declared requirement".into(),
                    );
                };
                (
                    bound.trait_symbol,
                    bound.arguments.as_slice(),
                    bound.lifetime_arguments.as_slice(),
                )
            }
            TypeParameterKind::Const { type_reference } => {
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..], &[][..]),
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        lifetime_arguments,
                        ..
                    } => (
                        *base_symbol,
                        program.type_reference_handles(*arguments),
                        lifetime_arguments.as_slice(),
                    ),
                    _ => return Ok(false),
                }
            }
            _ => return Ok(false),
        };
        let requirement = program
            .trait_definition_by_symbol(requirement)
            .ok_or("data implementation parameter lost its declared requirement")?;
        let expected = AppliedTraitScope::new(
            program,
            requirement,
            TraitApplicationArguments {
                lexical_owner: data.symbol,
                enclosing_data_applications: &[application],
                arguments,
                lifetimes,
            },
        )
        .ok_or("data implementation parameter lost its requirement arguments")?;
        let symbol = program.type_reference_table.type_symbol(argument);
        if program.conformance_by_symbol(symbol).is_none() {
            // A computed const record can still acquire its value during evaluation.
            return if matches!(parameter.kind, TypeParameterKind::Const { .. }) {
                Ok(false)
            } else {
                Err("data implementation argument must name a selected implementation".into())
            };
        }
        let actual = program
            .applied_requirement_scope(data.symbol, argument)
            .ok_or("data implementation argument lacks a complete selected application")?;
        if !expected.same_application(program, &actual) {
            return Err(format!(
                "implementation `{}` does not supply the declared `{}` application of `{}`",
                program.display_type_reference(argument),
                requirement.name.as_str(),
                data.name.as_str()
            ));
        }
        Ok(true)
    }

    /// Whether a static machine argument names one declared machine.
    fn selects_machine(&self, argument: TypeReferenceHandle) -> bool {
        let TypeReferenceNode::Named { symbol, .. } =
            self.source.type_reference_table.type_reference(argument)
        else {
            return false;
        };
        self.source.symbols.symbols().nodes().is_valid(*symbol)
            && self.source.symbols.get(*symbol).kind == SymbolKind::Machine
            && self.source.machine_by_symbol(*symbol).is_some()
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
        // Even an argument erased by the member types must have its declared
        // kind. Check the tuple before deferring an open application.
        for (parameter, &argument) in parameters.iter().zip(&arguments) {
            if !self
                .source
                .type_reference_table
                .contains_type_reference(argument)
            {
                return Err("data application refers to a missing type reference".into());
            }
            if parameter.kind == TypeParameterKind::Type {
                if self.source.type_argument_names_value(argument) {
                    return Err(format!(
                        "data type argument for `{}` of `{}` requires a type but was supplied a value",
                        parameter.name.as_str(),
                        self.source.symbols.name(owner),
                    ));
                }
                check_type_argument(
                    self.source,
                    argument,
                    self.source
                        .type_reference_table
                        .operand_lifetime_owner(reference),
                )?;
            }
        }
        if parameters.is_empty() {
            return Ok(true);
        }
        // Lifetime arguments were matched to the declaration above; they
        // stay on this occurrence while the other parameters close its fields.
        if parameters.iter().any(|parameter| {
                !matches!(
                    parameter.kind,
                    TypeParameterKind::Type
                        | TypeParameterKind::Implementation
                        | TypeParameterKind::Const { .. }
                        | TypeParameterKind::Value { .. }
                        | TypeParameterKind::Machine {
                            contract: MachineParameterContract::Structural(_)
                        }
                )
            })
        {
            return Ok(false);
        }
        let mut substitutions = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let symbol_kind = match parameter.kind {
                TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
                TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                _ => SymbolKind::TypeParameter,
            };
            if !self
                .source
                .symbols
                .symbols()
                .nodes()
                .is_valid(parameter.symbol)
                || self.source.symbols.get(parameter.symbol).kind != symbol_kind
                || self.source.symbols.get(parameter.symbol).parent != owner
                || substitutions
                    .iter()
                    .any(|(known, _)| *known == parameter.symbol)
            {
                return Err(
                    "data application has a foreign or duplicated declaration binder".into(),
                );
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
        let mut closed = true;
        for (parameter, (_, argument)) in parameters.iter().zip(&substitutions) {
            closed &= match parameter.kind {
                TypeParameterKind::Type => self.reference(*argument)?,
                TypeParameterKind::Implementation => {
                    self.closed_implementation(reference, &definition, parameter, *argument)?
                }
                TypeParameterKind::Const { type_reference }
                    if self
                        .source
                        .static_record_schema(type_reference, owner)
                        .is_some() =>
                {
                    self.closed_implementation(reference, &definition, parameter, *argument)?
                }
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    self.source
                        .static_const_literal_from_type_reference(*argument)
                        .is_some()
                        || crate::source_state::canonical_const_argument_value(
                            self.source,
                            *argument,
                        )
                        .is_some()
                }
                // A static machine argument names the machine it selects. That
                // the machine has the parameter's whole contract is judged
                // where the application is read.
                TypeParameterKind::Machine { .. } => self.selects_machine(*argument),
                _ => false,
            };
        }
        definition.type_parameters = HandleSpan::empty();
        definition.generic_instance = Some(reference);
        definition.predicate_subject_type = self
            .source
            .tables
            .type_reference_table
            .instantiate(
                definition.predicate_subject_type,
                &substitutions,
                &self.source.tables.expression_table,
            )
            .map_err(str::to_owned)?;
        // A quotient application is the quotient of its carrier at these
        // arguments, by its relation and conformance at them.
        if let Some(quotient) = &mut definition.quotient {
            let table = &mut self.source.tables.type_reference_table;
            quotient.carrier = table
                .instantiate(
                    quotient.carrier,
                    &substitutions,
                    &self.source.tables.expression_table,
                )
                .map_err(str::to_owned)?;
            let mut span = |span: HandleSpan<TypeReferenceHandle>| {
                let applied = table
                    .type_reference_handles(span)
                    .to_vec()
                    .into_iter()
                    .map(|argument| {
                        table.instantiate(
                            argument,
                            &substitutions,
                            &self.source.tables.expression_table,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(str::to_owned)?;
                Ok::<_, String>(table.insert_type_reference_handles(applied))
            };
            quotient.relation_arguments = span(quotient.relation_arguments)?;
            if let Some(selection) = &mut quotient.equivalence {
                selection.relation_arguments = span(selection.relation_arguments)?;
                selection.trait_arguments = span(selection.trait_arguments)?;
                selection.conformance_arguments = span(selection.conformance_arguments)?;
            }
            // The carrier at these arguments is read like any other type.
            let carrier = quotient.carrier;
            self.reference(carrier)?;
        }
        let mut children = Vec::new();
        for member in &mut members {
            match member {
                DataMember::Field(field) => {
                    field.type_reference = self
                        .source
                        .tables
                        .type_reference_table
                        .instantiate(
                            field.type_reference,
                            &substitutions,
                            &self.source.tables.expression_table,
                        )
                        .map_err(str::to_owned)?;
                    children.push(field.type_reference);
                }
                DataMember::Variant(variant) => {
                    let mut fields = self.source.data_payload_fields(variant).to_vec();
                    for field in &mut fields {
                        field.type_reference = self
                            .source
                            .tables
                            .type_reference_table
                            .instantiate(
                                field.type_reference,
                                &substitutions,
                                &self.source.tables.expression_table,
                            )
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
