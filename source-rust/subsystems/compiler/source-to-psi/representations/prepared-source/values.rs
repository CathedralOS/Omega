//! Source inputs used to prepare constants and computed type arguments.
use arena::Handle;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::source_state::{
    TypedTrees, constant::ConstDeclaration, expression::ExpressionHandle,
    types::TypeReferenceHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedValue {
    Constant(SymbolHandle),
    StaticArgument {
        owner: SymbolHandle,
        site: TypeReferenceHandle,
        expression: ExpressionHandle,
        carrier: TypeReferenceHandle,
    },
}

/// Whether a value still waits for the deployment that will evaluate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetInputs {
    /// Preparation settled this value without waiting for a deployment.
    None,
    /// The initializer or extent remains open until its deployment supplies the inputs.
    Deferred,
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
            PreparedValue::StaticArgument {
                owner,
                site,
                expression,
                carrier,
            } => {
                if !program.symbols.symbols().nodes().is_valid(owner)
                    || !program.type_reference_table.contains_type_reference(site)
                    || !program.expression_table.expression_is_valid(expression)
                    || !program
                        .type_reference_table
                        .contains_type_reference(carrier)
                {
                    return Err(vec![Diagnostic::error(
                        "prepared static argument lost its declaration, expression or carrier",
                    )]);
                }
                if row.targets == TargetInputs::Deferred {
                    let recipe = program.constant_recipe_for_const_application(site);
                    let contexts = program.constant_recipes.is_valid(recipe).then(|| {
                        program
                            .constant_expression_contexts
                            .span_or_empty(program.constant_recipes.get(recipe).contexts)
                    });
                    if !matches!(contexts.and_then(|contexts| contexts.as_array::<1>()),
                        Some([context]) if context.owner == owner && context.result_type == carrier &&
                            context.evaluation_expression == expression)
                    {
                        return Err(vec![Diagnostic::error(
                            "deferred static argument lost its original evaluation context",
                        )]);
                    }
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
