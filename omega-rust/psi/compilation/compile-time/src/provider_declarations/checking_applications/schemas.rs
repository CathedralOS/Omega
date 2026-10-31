//! ABI-free whole callable rosters, retaining exact applied receiver meaning.
use super::super::ProviderPlanningContext;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

pub(super) type Key = language_semantics::declaration_selection::provider_choices::ApplicationKey;

#[derive(Clone)]
pub(super) struct Schema {
    pub key: Key,
    pub receivers: Vec<Key>,
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
        let mut receivers = Vec::new();
        for scope in scopes.iter().filter(|scope| scope.declaration.is_boundary) {
            receivers.push(trait_key(program, scope)?);
        }
        Some(Self {
            key: trait_key(program, &root)?,
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
            receivers: Vec::new(),
        })
    }
    pub fn includes(&self, other: &Self) -> bool {
        self.key == other.key || self.receivers.contains(&other.key)
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
