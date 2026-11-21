//! Source inputs used to prepare constants and computed type arguments.
use arena::Handle;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::source_state::{
    TypedTrees, constant::ConstDeclaration, types::TypeReferenceHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedValue {
    Constant(SymbolHandle),
    StaticArgument {
        owner: SymbolHandle,
        site: TypeReferenceHandle,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TargetInputs {
    /// This value has no admitted deployment input.
    #[default]
    None,
    /// The initializer or extent remains open until its deployment supplies the inputs.
    Deferred,
    /// These deployments actually admitted an evaluation of this value.
    Observed(Vec<PreparedTarget>),
}

/// A deployment selected by a completed invocation and the immutable geometry it received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTarget {
    pub deployment: String,
    pub layout: layout_plans::LayoutTargetFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueInputs {
    source: u64,
    pub value: PreparedValue,
    /// Named constant inputs of the prepared computation, including called source.
    pub constants: Vec<SymbolHandle>,
    pub targets: TargetInputs,
}

impl ValueInputs {
    pub fn new(
        program: &TypedTrees,
        value: PreparedValue,
        constants: Vec<SymbolHandle>,
        targets: TargetInputs,
    ) -> Self {
        Self {
            source: program.identity.get(),
            value,
            constants,
            targets,
        }
    }
}

pub(super) fn constant_inputs(
    program: &TypedTrees,
    values: &[ValueInputs],
) -> Result<Vec<Handle<ConstDeclaration>>, Vec<Diagnostic>> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let mut wanted = Vec::new();
    for row in values {
        if row.source != program.identity.get() {
            return Err(vec![Diagnostic::error(
                "prepared value inputs belong to another source program",
            )]);
        }
        match row.value {
            PreparedValue::Constant(symbol) => wanted.push(symbol),
            PreparedValue::StaticArgument { owner, site } => {
                if !program.symbols.symbols().nodes().is_valid(owner)
                    || !program.type_reference_table.contains_type_reference(site)
                {
                    return Err(vec![Diagnostic::error(
                        "prepared static argument lost its declaration or type occurrence",
                    )]);
                }
            }
        }
        wanted.extend_from_slice(&row.constants);
    }
    let key = |symbol: &SymbolHandle| (symbol.arena_index(), symbol.generation());
    wanted.sort_unstable_by_key(key);
    wanted.dedup();
    let mut declarations = program
        .roots
        .const_declarations
        .handles()
        .map(|handle| {
            (
                key(&program.tables.const_declarations.get(handle).symbol),
                handle,
            )
        })
        .collect::<Vec<_>>();
    declarations.sort_unstable_by_key(|&(key, _)| key);
    wanted
        .into_iter()
        .map(|symbol| {
            declarations
                .binary_search_by_key(&key(&symbol), |&(key, _)| key)
                .map(|index| declarations[index].1)
                .map_err(|_| {
                    vec![Diagnostic::error(
                        "source preparation lost a demanded constant declaration",
                    )]
                })
        })
        .collect()
}
