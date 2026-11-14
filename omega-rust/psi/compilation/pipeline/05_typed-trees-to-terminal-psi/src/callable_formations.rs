//! An installed continuation owes its calling shape before its body can be
//! judged. Keeping that judgment separate lets a known source mismatch reject
//! even when the body contains a construct the lowering route does not serve.
//! These checks introduce no storage, facts, effect permission or evidence.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::callable_signature_shapes::{
    BoundCallableShape, CallableShapeMismatch, CallableSignatureShape,
    compare_callable_signature_shapes,
};
use typed_trees::product_operand::ProductEntryDeclaration;
use typed_trees::references::{TypedReferences, visit_type_reference};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use terminal_check_tables::compilation::{
    CallableFormationCause, CallableFormationDefect, CallableFormationFailure,
};

pub fn check_bound_callable_shapes(
    typed: &TypedTrees,
    bindings: &[BoundCallableShape],
) -> Result<Vec<CallableFormationFailure>, CallableFormationDefect> {
    let mut failures = Vec::new();
    for binding in bindings {
        let defect = |detail: &str| CallableFormationDefect {
            slot: binding.slot.clone(),
            detail: detail.to_owned(),
        };
        let schema = binding
            .semantic_schema
            .rejoin(
                &typed.symbols,
                typed.traits().iter().map(|owner| owner.symbol),
            )
            .map_err(defect)?;
        if !typed
            .trait_definition_by_symbol(schema)
            .is_some_and(|schema| schema.is_boundary)
        {
            return Err(defect(
                "bound continuation schema is not a boundary declaration",
            ));
        }
        let requirement = binding
            .requirement
            .rejoin(
                &typed.symbols,
                typed.traits().iter().flat_map(|owner| {
                    typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .map(|signature| signature.symbol)
                }),
            )
            .map_err(defect)?;
        let required = typed
            .trait_definition_by_symbol(schema)
            .and_then(|owner| typed.trait_machine_signature_by_symbol(owner, requirement))
            .ok_or_else(|| defect("bound continuation requirement has no declared signature"))?;
        let parameters = typed.state_signature_parameters(required);
        let mut projected = Vec::with_capacity(binding.visible_parameters.len());
        let mut previous = None;
        for occurrence in &binding.visible_parameters {
            let parameter = parameters
                .get(occurrence.ordinal)
                .filter(|parameter| parameter.symbol == occurrence.symbol)
                .ok_or_else(|| {
                    defect("bound continuation projection lost its exact parameter occurrence")
                })?;
            if previous.is_some_and(|previous| occurrence.ordinal <= previous) {
                return Err(defect(
                    "bound continuation projection does not preserve declaration order",
                ));
            }
            previous = Some(occurrence.ordinal);
            projected.push(parameter.clone());
        }
        let declaration =
            ProductEntryDeclaration::from_declared_owner(typed, binding.implementation)
                .ok_or_else(|| {
                    defect("bound continuation has no exact complete product declaration")
                })?;
        let failure = |cause, detail| CallableFormationFailure {
            slot: binding.slot.clone(),
            implementation: binding.implementation,
            source_span: typed
                .symbols
                .symbol_source_span(binding.implementation)
                .or(binding.source_span),
            cause,
            detail,
        };
        let name = declaration.name;
        let Some(actual) = declaration.entry else {
            failures.push(failure(
                CallableFormationCause::NoEntry,
                format!("entry machine `{name}` has no executable entry state"),
            ));
            continue;
        };
        // Access views retain their independent establishment obligations. Their
        // exclusion from the visible arrival parameters creates no backing.
        let visible: Vec<_> = actual
            .parameters
            .iter()
            .filter(|parameter| {
                typed
                    .access_view_plan_for_input_type_reference(parameter.type_reference)
                    .is_none()
            })
            .cloned()
            .collect();
        let mut types = TypeResolution::new(typed);
        for parameter in projected.iter().chain(&visible) {
            types.resolve(parameter.type_reference).map_err(defect)?;
        }
        let mismatches = compare_callable_signature_shapes(
            CallableSignatureShape {
                generic_arity: typed.state_signature_type_parameters(required).len(),
                parameters: &projected,
                return_type: required.return_type,
            },
            CallableSignatureShape {
                generic_arity: declaration.type_parameters.len(),
                parameters: &visible,
                return_type: actual.return_type,
            },
            |actual, required| types.matches(actual, required).map_err(defect),
        )?;
        for mismatch in mismatches {
            let detail = match mismatch {
                CallableShapeMismatch::GenericArity { .. } => format!(
                    "entry machine `{name}` is generic; a root slot must bind one exact machine"
                ),
                CallableShapeMismatch::ParameterArity { actual, .. } => format!(
                    "{}, but `{name}` declares {actual} visible parameters",
                    binding.arrival_description
                ),
                CallableShapeMismatch::ParameterMode { position } => format!(
                    "root slot `{}` requires visible parameter {position} of entry machine `{name}` to have the required calling mode",
                    binding.slot
                ),
                CallableShapeMismatch::ParameterType { position } => format!(
                    "root slot `{}` requires visible parameter {position} of entry machine `{name}` to have exact type `{}`, got `{}`",
                    binding.slot,
                    typed.display_type_reference_with_constraints(
                        projected[position].type_reference
                    ),
                    typed.display_type_reference_with_constraints(visible[position].type_reference)
                ),
                CallableShapeMismatch::ReturnType => format!(
                    "entry machine `{name}` returns a value, but `ProgramEntry` has no result"
                ),
            };
            failures.push(failure(CallableFormationCause::Shape(mismatch), detail));
        }
        let machine = typed
            .machine(binding.implementation)
            .ok_or_else(|| defect("bound continuation lost its machine declaration"))?;
        let entry = typed
            .machine_states(machine)
            .first()
            .ok_or_else(|| defect("bound continuation lost its entry declaration"))?;
        for (owner, contracts) in [
            (machine.symbol, typed.machine_contracts(machine)),
            (entry.symbol, typed.state_contracts(entry)),
        ] {
            for (contract, clause) in contracts.iter().enumerate() {
                if clause.kind == SignatureContractKind::Requires && clause.binding.is_some() {
                    failures.push(failure(CallableFormationCause::UnsuppliedEvidence { owner, contract }, format!("ProgramEntry cannot supply erased requires evidence to entry machine `{name}`")));
                }
            }
        }
    }
    Ok(failures)
}

// The type table owns one arena for this immutable binding check. Validate
// each handle's generation before indexing its dense slot, so a foreign handle
// cannot reuse another type's visit mark. Shared children are resolved once
// across parameters, the result, and the later identity comparisons.
struct TypeResolution<'a> {
    typed: &'a TypedTrees,
    visited: Vec<bool>,
    error: Option<&'static str>,
}
impl Extend<SymbolHandle> for TypeResolution<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        if symbols
            .into_iter()
            .any(|symbol| !self.typed.symbols.symbols().nodes().is_valid(symbol))
        {
            self.error = Some("callable type names an unresolved declaration");
        }
    }
}
impl TypeResolution<'_> {
    fn child(&mut self, child: TypeReferenceHandle) {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(child)
        {
            self.error = Some("callable type contains an unresolved child type");
        }
    }
    fn arguments(&mut self, children: &[TypeReferenceHandle], expected: usize) {
        if children.len() != expected {
            self.error = Some("callable type has a foreign argument span");
        }
        for child in children {
            self.child(*child);
        }
    }
}
impl TypedReferences for TypeResolution<'_> {
    fn declaration_description_dependency(&mut self, declaration: SymbolHandle) {
        self.extend([declaration]);
    }

    fn type_alias_application(
        &mut self,
        _: &TypedTrees,
        application: &typed_trees::type_aliases::TypeAliasApplication,
    ) -> bool {
        self.arguments(
            self.typed
                .type_reference_table
                .type_reference_handles(application.arguments),
            application.arguments.len(),
        );
        self.error.is_none()
    }

    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            self.error = Some("callable type has a foreign reference");
            return false;
        }
        let slot = reference.arena_index() as usize;
        if self.visited.len() <= slot {
            self.visited.resize(slot + 1, false);
        }
        if self.visited[slot] {
            return false;
        }
        self.visited[slot] = true;
        let node = self.typed.type_reference_table.type_reference(reference);
        match node {
            TypeReferenceNode::Callable { contract } => {
                if !self.typed.runtime_callable_contracts.is_valid(*contract) {
                    self.error = Some("callable type has no current signature contract");
                    return false;
                }
                let retained = self.typed.runtime_callable_contract(*contract);
                self.extend([retained.lexical_owner, retained.signature.symbol]);
                if let Err(reason) = self
                    .typed
                    .runtime_callable_contract_identity(*contract, &[])
                {
                    self.error = Some(reason.reason());
                    return false;
                }
                // The shared type visitor descends through the complete
                // signature in its formal scope, including contract operands,
                // static arguments, progress and invocation declarations.
            }
            TypeReferenceNode::Named { symbol, .. } => self.extend([*symbol]),
            TypeReferenceNode::DynamicTrait {
                symbol, arguments, ..
            } => {
                self.extend([*symbol]);
                self.arguments(
                    self.typed
                        .type_reference_table
                        .type_reference_handles(*arguments),
                    arguments.len(),
                );
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                self.extend([*base_symbol]);
                self.arguments(
                    self.typed
                        .type_reference_table
                        .type_reference_handles(*arguments),
                    arguments.len(),
                );
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                self.child(*base_type);
                let clauses = self.typed.type_reference_table.constraints(*constraints);
                if clauses.len() != constraints.len() {
                    self.error = Some("callable type has a foreign constraint span");
                }
                for constraint in clauses {
                    match constraint {
                        typed_trees::types::TypeConstraintNode::Domain(domain) => {
                            self.extend([domain.symbol]);
                            for argument in &domain.arguments {
                                self.child(*argument);
                            }
                        }
                        typed_trees::types::TypeConstraintNode::Named(_) => {
                            self.error =
                                Some("callable type constraint has no resolved domain identity")
                        }
                        typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::Reference { referee, .. } => self.child(*referee),
            TypeReferenceNode::Slice { element_type } => self.child(*element_type),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                self.child(*element_type);
                match length {
                    typed_trees::types::FixedArrayLength::Expression(reference) => {
                        self.child(*reference)
                    }
                    typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. } => {
                        self.extend([*symbol])
                    }
                    typed_trees::types::FixedArrayLength::ConstCall { machine, .. } => {
                        self.extend([*machine])
                    }
                    typed_trees::types::FixedArrayLength::Literal(_) => {}
                }
            }
            TypeReferenceNode::ConstExpression(_) => {
                self.error = Some("callable type retains an unevaluated constant expression");
                return false;
            }
            TypeReferenceNode::Unit => {}
        }
        self.error.is_none()
    }
}

impl<'a> TypeResolution<'a> {
    fn new(typed: &'a TypedTrees) -> Self {
        Self {
            typed,
            visited: Default::default(),
            error: None,
        }
    }

    fn resolve(&mut self, reference: TypeReferenceHandle) -> Result<(), &'static str> {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("callable parameter has no resolved type");
        }
        visit_type_reference(self.typed, self, reference);
        self.error.map_or(Ok(()), Err)
    }

    fn matches(
        &mut self,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
    ) -> Result<bool, &'static str> {
        // Parameters were resolved above. Only the absent result lane denotes
        // no result; a missing parameter type never reaches this comparison.
        if !actual.is_valid() || !required.is_valid() {
            return Ok(actual.is_valid() == required.is_valid());
        }
        self.resolve(actual)?;
        self.resolve(required)?;
        Ok(self.typed.normalized_type_identity(actual)
            == self.typed.normalized_type_identity(required))
    }
}
