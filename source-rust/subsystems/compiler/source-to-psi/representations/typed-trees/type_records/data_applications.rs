//! Substitute a data declaration’s type and constant arguments into its member types.
//! Typing forms the first views. Specialization and compile-time evaluation use
//! the same operation when they create types or settle a computed constant.
//! Each view stays associated with its original source type occurrence.

use crate::source_state::TypedTrees;
use crate::source_state::data::{DataMember, MachineParameterContract, TypeParameterKind};
use crate::source_state::types::{
    DomainConstraintSubject, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
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
    let TypeReferenceNode::ConstExpression(settled_expression) =
        *program.type_reference_table.type_reference(argument)
    else {
        return Err("a settled const argument is not an expression to evaluate".into());
    };

    let mut domain_sites = Vec::new();
    let mut extents = Vec::new();
    let mut applications = Vec::new();
    for (reference, node) in program.type_reference_table.type_reference_rows() {
        match node {
            TypeReferenceNode::Constrained { constraints, .. } => {
                let authored = program.type_reference_table.constraints(*constraints);
                if authored.iter().any(|constraint| matches!(constraint,
                    TypeConstraintNode::Domain(domain) if domain.subject == DomainConstraintSubject::Declared && domain.arguments.contains(&argument)
                )) {
                    domain_sites.push(reference);
                }
            }
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
    finalize_const_expression_operators(program, settled_expression);
    program.type_reference_table.substitute_node(
        argument,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: crate::source_state::name::Identifier::generated(spelling),
        },
    );
    settle_domain_indices(program, argument, &domain_sites)?;
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

/// Complete any `CheckedOperator` selections inside a const expression whose
/// value has just been settled in place, so later selection audits do not see
/// an orphaned late-bound operator on the replaced expression.
fn finalize_const_expression_operators(
    program: &mut TypedTrees,
    root: crate::source_state::expression::ExpressionHandle,
) {
    use crate::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Late,
        AuthoredDeclarationSelectionTarget as Target,
    };
    use crate::source_state::expression::ExpressionNode;
    let mut pending = vec![root];
    let mut occurrences = Vec::new();
    while let Some(handle) = pending.pop() {
        if !program.expression_table.expression_is_valid(handle) {
            continue;
        }
        match program.expression_table.expression(handle) {
            ExpressionNode::Binary(binary) => {
                for occurrence in program
                    .expression_table
                    .authored_selection_occurrences(handle)
                {
                    if program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|row| {
                            row.kind() == Kind::Operator
                                && row.target() == Target::LateBound(Late::CheckedOperator)
                        })
                    {
                        occurrences.push(occurrence);
                    }
                }
                pending.push(binary.left);
                pending.push(binary.right);
            }
            ExpressionNode::Unary(unary) => {
                for occurrence in program
                    .expression_table
                    .authored_selection_occurrences(handle)
                {
                    if program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|row| {
                            row.kind() == Kind::Operator
                                && row.target() == Target::LateBound(Late::CheckedOperator)
                        })
                    {
                        occurrences.push(occurrence);
                    }
                }
                pending.push(unary.operand);
            }
            _ => {}
        }
    }
    for occurrence in occurrences {
        let _ = program.finalize_authored_intrinsic_selection(
            occurrence,
            Late::CheckedOperator,
            Intrinsic::BuiltinOperator,
        );
    }
}

fn rebind_domain(
    program: &mut TypedTrees,
    carrier: TypeReferenceHandle,
    domain: &mut crate::source_state::types::DomainConstraint,
) -> Result<(), String> {
    let identity = program
        .retain_domain_application(domain.symbol, carrier, &domain.arguments)
        .map_err(|error| error.message)?;
    domain.semantic_id = identity;
    domain.semantic_roles.denotation_dimension =
        domain.semantic_roles.denotation_dimension.map(|_| identity);
    domain.semantic_roles.arithmetic_policy =
        domain.semantic_roles.arithmetic_policy.map(|_| identity);
    Ok(())
}

/// Replace the former application coordinate wherever the same type occurrence
/// supplied it. Another occurrence of a similar expression keeps its own request.
fn settle_domain_indices(
    program: &mut TypedTrees,
    argument: TypeReferenceHandle,
    domain_sites: &[TypeReferenceHandle],
) -> Result<(), String> {
    let mut domain_changes = Vec::new();
    // The same constraint span can occur on two carriers. Retain the value
    // application for each site before replacing that site's constraint span.
    for &site in domain_sites {
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = *program.type_reference_table.type_reference(site)
        else {
            return Err("an evaluated domain index lost its constrained type".into());
        };
        let carrier = base_type;
        let mut constraints = program
            .type_reference_table
            .constraints(constraints)
            .to_vec();
        for constraint in &mut constraints {
            let TypeConstraintNode::Domain(domain) = constraint else {
                continue;
            };
            if domain.subject != DomainConstraintSubject::Declared
                || !domain.arguments.contains(&argument)
            {
                continue;
            }
            let previous = domain.semantic_id;
            rebind_domain(program, carrier, domain)?;
            domain_changes.push((
                domain.symbol,
                previous,
                domain.arguments.clone(),
                domain.semantic_id,
            ));
        }
        let constraints = program.type_reference_table.insert_constraints(constraints);
        program
            .type_reference_table
            .set_constraint_span(site, constraints);
    }
    if !domain_changes.is_empty() {
        // A parameter's synthesized membership contract and a cast can retain the
        // same authored index handles. A separate occurrence needs its own evaluation.
        let updated = |symbol, previous, arguments: &[TypeReferenceHandle]| {
            domain_changes
                .iter()
                .find_map(|(declaration, old, original, next)| {
                    (*declaration == symbol && *old == previous && original == arguments)
                        .then_some(*next)
                })
        };
        let facts = program
            .proof_facts
            .iter()
            .filter_map(|(handle, fact)| {
                let membership = fact.as_membership()?;
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments);
                updated(
                    membership.domain_symbol,
                    membership.semantic_domain,
                    arguments,
                )
                .map(|identity| (handle, identity))
            })
            .collect::<Vec<_>>();
        for (handle, identity) in facts {
            if let crate::source_state::domain::ProofFact::Membership(membership) =
                program.proof_facts.get_mut(handle)
            {
                membership.semantic_domain = identity;
            }
        }
        let casts = program
            .expression_table
            .expression_entries()
            .filter_map(|(handle, expression)| {
                let crate::source_state::expression::ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments);
                updated(
                    cast.semantic_domain_symbol,
                    cast.semantic_domain_id,
                    arguments,
                )
                .map(|identity| (handle, identity))
            })
            .collect::<Vec<_>>();
        for (handle, identity) in casts {
            if let crate::source_state::expression::ExpressionNode::Cast(cast) =
                program.expression_table.expression_mut(handle)
            {
                cast.semantic_domain_id = identity;
            }
        }
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
                    rebind_domain(self.source, base_type, &mut applied)?;
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
                self.reference(base_type)
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
        if lifetimes != 0
            || parameters.iter().any(|parameter| {
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
        // Only a newly formed view needs canonical integers in its member types.
        for (parameter, (_, argument)) in parameters.iter().zip(&mut substitutions) {
            if parameter.kind.is_const_or_value()
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
            .type_reference_table
            .instantiate(definition.predicate_subject_type, &substitutions)
            .map_err(str::to_owned)?;
        // A quotient application is the quotient of its carrier at these
        // arguments, by its relation and conformance at them.
        if let Some(quotient) = &mut definition.quotient {
            let table = &mut self.source.type_reference_table;
            quotient.carrier = table
                .instantiate(quotient.carrier, &substitutions)
                .map_err(str::to_owned)?;
            let mut span = |span: HandleSpan<TypeReferenceHandle>| {
                let applied = table
                    .type_reference_handles(span)
                    .to_vec()
                    .into_iter()
                    .map(|argument| table.instantiate(argument, &substitutions))
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
