//! Read-only applied declaration scopes over the frozen type table.
//!
//! Prepared applications retain exact fields when available. Generated nested
//! applications instead bind the declaration telescope in invocation-owned
//! scratch scopes. Readers share this mechanism rather than mutating frozen
//! type rows or treating an unprepared generic carrier as reference-free.

use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, TypeParameterKind};
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// In-flight formal→actual type-parameter bindings. Entries are pushed while
/// a generic application expands and truncated afterward, so a binding never
/// outlives the member walk that derived it.
pub(crate) type TypeBindings = Vec<(SymbolHandle, TypeReferenceHandle)>;

/// Nested member applications may not have materialized declarations: finite
/// preparation does not chase every generated field type. Inventory, projection
/// and value transport share simultaneous substitutions in query-local scopes,
/// without manufacturing typed rows. Siblings cannot inherit one another's
/// bindings; each application owns one scope shared by its children.
pub struct DeclarationScopes {
    scopes: Vec<DeclarationScope>,
}

struct DeclarationScope {
    bindings: TypeBindings,
    starts: Vec<usize>,
    parent: usize,
}

impl Default for DeclarationScopes {
    fn default() -> Self {
        Self {
            scopes: vec![DeclarationScope {
                bindings: Vec::new(),
                starts: Vec::new(),
                parent: 0,
            }],
        }
    }
}

impl DeclarationScopes {
    pub fn head(
        &self,
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
        mut scope: usize,
    ) -> (TypeReferenceHandle, usize) {
        // An actual belongs to the enclosing application, not to the telescope
        // that consumes it. Preserve that environment even for compound actuals
        // such as `&T`; substituting only a named head loses this distinction.
        loop {
            if !reference.is_valid() {
                return (reference, scope);
            }
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return (reference, scope);
            };
            let mut owner = scope;
            loop {
                let environment = &self.scopes[owner];
                let Some(start) = environment.starts.last().copied() else {
                    return (reference, scope);
                };
                if let Some((_, actual)) = environment.bindings[start..]
                    .iter()
                    .find(|(formal, _)| formal == symbol)
                {
                    reference = *actual;
                    scope = environment.parent;
                    break;
                }
                owner = environment.parent;
            }
        }
    }

    pub fn type_identity(
        &self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        scope: usize,
    ) -> typed_trees::type_identity::TypeIdentityId {
        let environment = &self.scopes[scope];
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &environment.bindings,
                ..TypeIdentityRequest::ordinary(reference)
            },
            &environment.starts,
        )
    }

    /// A repeated by-value declaration may expand only by descending into a
    /// finite actual argument, never by growing another application in a field.
    pub fn is_strict_argument(
        &self,
        program: &dyn crate::CheckingInput,
        parent: TypeReferenceHandle,
        parent_scope: usize,
        selected: typed_trees::type_identity::TypeIdentityId,
    ) -> bool {
        let (parent, parent_scope) = self.head(program, parent, parent_scope);
        let TypeReferenceNode::Generic { arguments, .. } =
            program.type_reference_table.type_reference(parent)
        else {
            return false;
        };
        let mut pending = program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .map(|reference| (*reference, parent_scope))
            .collect::<Vec<_>>();
        let mut visited = Vec::new();
        while let Some((reference, scope)) = pending.pop() {
            let (reference, scope) = self.head(program, reference, scope);
            if visited.contains(&(reference, scope)) {
                continue;
            }
            visited.push((reference, scope));
            if self.type_identity(program, reference, scope) == selected {
                return true;
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|reference| (*reference, scope)),
                ),
                TypeReferenceNode::Constrained { base_type, .. } => {
                    pending.push((*base_type, scope))
                }
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    pending.push((*element_type, scope))
                }
                _ => {}
            }
        }
        false
    }

    pub fn definition<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        scope: usize,
    ) -> Option<(&'program DataDefinition, usize)> {
        if let Some(definition) = program.closed_data_definition(reference) {
            return Some((definition, scope));
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        let definition = unique_data_definition(program, *base_symbol)?;
        let parameters = program.data_type_parameters(definition);
        if parameters.len() != arguments.len()
            || lifetime_arguments.len() != definition.lifetime_parameters.len()
            || parameters.iter().any(|parameter| {
                !parameter.symbol.is_valid() || !matches!(parameter.kind, TypeParameterKind::Type)
            })
            || arguments.iter().any(|argument| {
                !program
                    .type_reference_table
                    .contains_type_reference(*argument)
            })
        {
            return None;
        }
        let parent = scope;
        let mut bindings = self.scopes[parent].bindings.clone();
        let mut starts = self.scopes[parent].starts.clone();
        starts.push(bindings.len());
        bindings.extend(
            parameters
                .iter()
                .map(|parameter| parameter.symbol)
                .zip(arguments.iter().copied()),
        );
        let scope = self.scopes.len();
        self.scopes.push(DeclarationScope {
            bindings,
            starts,
            parent,
        });
        Some((definition, scope))
    }
}

/// Resolve a `Named` head through the active bindings. An unbound parameter
/// keeps its named leaf for the conservative nominal fallback. Bound handles
/// are caller-side actuals; they cannot mention this instantiation's
/// parameters, but a transitive binding chain still terminates within the
/// environment's own length.
pub(crate) fn substituted_head(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    for _ in 0..=bindings.len() {
        if !reference.is_valid() {
            return reference;
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return reference;
        };
        let Some(bound) = crate::substitutions::actual_for(bindings, *symbol) else {
            return reference;
        };
        reference = bound;
    }
    reference
}

/// The unique data definition behind a nominal symbol. Type parameters,
/// builtins, and ambiguous spellings leave the carrier opaque.
pub(crate) fn unique_data_definition(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&DataDefinition> {
    if !symbol.is_valid() {
        return None;
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = definitions.next()?;
    definitions.next().is_none().then_some(definition)
}

/// Push one generic application's formal→actual bindings and return its data
/// definition. Every parameter must be a runtime `Type` binder with a valid
/// symbol and a supplied argument; anything else stays uninspectable, and no
/// bindings are pushed. Callers record `bindings.len()` beforehand and
/// truncate afterward so the entries cannot leak into a sibling walk.
pub(crate) fn push_generic_application_bindings<'program>(
    program: &'program dyn crate::CheckingInput,
    base_symbol: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    bindings: &mut TypeBindings,
) -> Option<&'program DataDefinition> {
    let definition = unique_data_definition(program, base_symbol)?;
    let parameters = program.data_type_parameters(definition);
    if parameters.len() != arguments.len()
        || parameters.iter().any(|parameter| {
            !parameter.symbol.is_valid() || !matches!(parameter.kind, TypeParameterKind::Type)
        })
    {
        return None;
    }
    // Resolve each actual through the bindings already in scope so a nested
    // application records the outer instantiation, not a formal name that
    // only the sibling walk could read.
    let resolved: Vec<TypeReferenceHandle> = arguments
        .iter()
        .map(|argument| substituted_head(program, *argument, bindings))
        .collect();
    bindings.extend(
        parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .zip(resolved),
    );
    Some(definition)
}
