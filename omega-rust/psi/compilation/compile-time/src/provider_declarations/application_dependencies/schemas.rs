//! ABI-free whole callable rosters, retaining exact applied receiver meaning.
use super::super::ProviderPlanningContext;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

pub(super) type Key = language_semantics::declaration_selection::provider_choices::ApplicationKey;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum CallingContext {
    Implicit,
    Authored(String),
    Unresolved,
}

#[derive(Clone)]
pub(super) struct Receiver {
    pub key: Key,
    pub requirements: Vec<SymbolHandle>,
    pub calling: CallingContext,
}
impl Receiver {
    /// Identical complete source inputs imply identical pure calling output.
    /// Different policy inputs do not prove different ABI output.
    pub fn correspondence(&self, other: &Self) -> Option<bool> {
        if self.key != other.key || self.requirements != other.requirements {
            return Some(false);
        }
        match (&self.calling, &other.calling) {
            (CallingContext::Implicit, CallingContext::Implicit) => Some(true),
            (CallingContext::Implicit, CallingContext::Authored(_))
            | (CallingContext::Authored(_), CallingContext::Implicit) => Some(false),
            (CallingContext::Authored(left), CallingContext::Authored(right)) if left == right => {
                Some(true)
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(super) struct Schema {
    pub key: Key,
    pub methods: Vec<SymbolHandle>,
    pub receivers: Vec<Receiver>,
}
impl Schema {
    pub fn trait_scope(program: &TypedTrees, root: AppliedTraitScope<'_>) -> Option<Self> {
        let mut scopes: Vec<AppliedTraitScope<'_>> = Vec::new();
        let mut ambiguous = false;
        root.visit(program, &mut |scope| {
            if let Some(known) = scopes
                .iter()
                .find(|known| known.declaration.symbol == scope.declaration.symbol)
            {
                ambiguous |= !known.same_application(program, scope);
            } else {
                scopes.push(scope.clone());
            }
        });
        if ambiguous {
            return None;
        }
        let calling = calling_context(program, &root);
        let mut methods = Vec::new();
        for scope in &scopes {
            for signature in program.trait_machine_signatures(scope.declaration) {
                if !methods.contains(&signature.symbol) {
                    methods.push(signature.symbol);
                }
            }
        }
        let mut receivers = Vec::new();
        for scope in scopes.iter().filter(|scope| scope.declaration.is_boundary) {
            let mut requirements = Vec::new();
            scope.visit(program, &mut |declaring| {
                for signature in program.trait_machine_signatures(declaring.declaration) {
                    if !requirements.contains(&signature.symbol) {
                        requirements.push(signature.symbol);
                    }
                }
            });
            requirements.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            receivers.push(Receiver {
                key: trait_key(program, scope)?,
                requirements,
                calling: calling.clone(),
            });
        }
        Some(Self {
            key: trait_key(program, &root)?,
            methods,
            receivers,
        })
    }
    pub fn exact(program: &TypedTrees, symbol: SymbolHandle) -> Option<Self> {
        let identity = match program.machine_by_symbol(symbol) {
            Some(machine)
                if machine.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement =>
            {
                if machine.spelling.is_some() {
                    program.machine_callable_identity_text(machine)?
                } else {
                    program.symbols.display_path(machine.symbol, "::")
                }
            }
            _ => {
                let operator = typed_trees::operator::declaration_by_symbol(program, symbol)
                    .or_else(|| {
                        program
                            .machine_token_bindings()
                            .iter()
                            .find(|row| row.symbol == symbol)
                    })?;
                if !operator.is_boundary {
                    return None;
                }
                typed_trees::operator::boundary_operator_requirement_identity(program, operator)
            }
        };
        Some(Self {
            key: (
                program.symbols.symbol_package_identity(symbol),
                identity,
                None,
            ),
            methods: vec![symbol],
            receivers: Vec::new(),
        })
    }
    pub fn supplies(&self, other: &Self) -> Option<bool> {
        let Some(root) = other.receivers.first() else {
            return Some(false);
        };
        let mut unknown = false;
        for view in &self.receivers {
            match view.correspondence(root) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => unknown = true,
            }
        }
        if unknown { None } else { Some(false) }
    }
    pub fn overlaps(&self, other: &Self) -> Option<bool> {
        let mut unknown = false;
        for left in &self.receivers {
            for right in &other.receivers {
                match left.correspondence(right) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => unknown = true,
                }
            }
        }
        if unknown { None } else { Some(false) }
    }
}

pub(super) fn trait_key(program: &TypedTrees, scope: &AppliedTraitScope<'_>) -> Option<Key> {
    Some((
        program
            .symbols
            .symbol_package_identity(scope.declaration.symbol),
        program.trait_declaration_path(scope.declaration),
        Some(program.type_identity_text(scope.portable_application_identity(program)?)?),
    ))
}

fn calling_context(program: &TypedTrees, root: &AppliedTraitScope<'_>) -> CallingContext {
    let vocabularies = validation::standard_calling_vocabularies(program);
    let relationships = program
        .trait_requirements(root.declaration)
        .iter()
        .filter(|relationship| {
            vocabularies
                .iter()
                .any(|(calling, _)| calling.symbol == relationship.symbol)
        })
        .collect::<Vec<_>>();
    match relationships.as_slice() {
        [] => CallingContext::Implicit,
        [relationship] => root
            .declaring_scope(program, relationship.symbol)
            .and_then(|scope| scope.portable_application_identity(program))
            .and_then(|identity| program.type_identity_text(identity))
            .map(CallingContext::Authored)
            .unwrap_or(CallingContext::Unresolved),
        _ => CallingContext::Unresolved,
    }
}

pub(super) fn provider_key(
    context: ProviderPlanningContext<'_>,
    symbol: SymbolHandle,
    application: Option<String>,
) -> Key {
    (
        context.typed.symbols.symbol_package_identity(symbol),
        context.typed.symbols.display_path(symbol, "::"),
        application,
    )
}
