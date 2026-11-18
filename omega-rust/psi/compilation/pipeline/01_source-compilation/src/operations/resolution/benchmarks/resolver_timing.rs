//! Measure one resolver operation on an owned, already parsed forest.
#[path = "../../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use diagnostics::Diagnostic;
use source_compiler::resolution::{
    DeferredResolvedFrontier, ExtensionRequest, ResolutionRequest, resolve, resolve_deferred,
    resolve_deferred_extension,
};
use source_compiler::source_inputs::SourceMap;
use source_compiler::source_resolved::SymbolResolvedTrees;
use source_compiler::source_symbols::SourceScopedTopLevelBinding;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};
use syntax_trees::SyntaxTrees;

struct Package {
    syntax: SyntaxTrees,
    sources: Arc<SourceMap>,
    imports: Vec<SourceScopedTopLevelBinding>,
}

impl Package {
    fn parse(case: &str) -> Result<Self, String> {
        let mut sources = SourceMap::default();
        let mut syntax = SyntaxTrees::default();
        let mut add = |path: &str, text: &str| {
            let id = sources.add(path.into(), text.into()).source_id;
            let tokens = frontend::Lexer::new(text)
                .tokenize()
                .map_err(|e| e.message)?;
            frontend::parse(&mut syntax, id, &tokens).map_err(|e| e.message)?;
            Ok::<_, String>(id)
        };
        let mut imports = Vec::new();
        match case {
            "reach" => {
                add(
                    "pkg/main.omg",
                    include_str!(
                        "../../../../../../../../../tests/fixtures/pass/effects/optional_reach_declared_slack_exit/main.omg"
                    ),
                )?;
            }
            name if name.starts_with("machines-") => {
                let count = name["machines-".len()..]
                    .parse::<std::num::NonZeroUsize>()
                    .map_err(|error| format!("machine count: {error}"))?;
                let mut text = String::from("public machine main() -> u64 { 7u64 }\n");
                for ordinal in 1..count.get() {
                    writeln!(text, "machine spare_{ordinal}() -> u64 {{ 1u64 }}").unwrap();
                }
                add("pkg/main.omg", &text)?;
            }
            "tiny" => {
                add("pkg/main.omg", "public machine main() -> u64 { 7u64 }")?;
            }
            "imports" => {
                let kit = add(
                    "pkg/kit.omg",
                    include_str!(
                        "../../../../../../../../../tests/fixtures/pass/calls/ordinary_family_binding/kit.omg"
                    ),
                )?;
                let main = add(
                    "pkg/main.omg",
                    include_str!(
                        "../../../../../../../../../tests/fixtures/pass/calls/ordinary_family_binding/main.omg"
                    ),
                )?;
                imports.push(SourceScopedTopLevelBinding::logical_module_import(
                    main,
                    kit,
                    "kit::pick",
                    0,
                ));
            }
            "generics" => {
                add(
                    "pkg/main.omg",
                    include_str!(
                        "../../../../../../../../../tests/fixtures/run/generics/nominal_const_extent/main.omg"
                    ),
                )?;
            }
            _ => return Err(format!("unknown input {case}")),
        }
        Ok(Self {
            syntax,
            sources: Arc::new(sources),
            imports,
        })
    }

    fn request(&self) -> ResolutionRequest {
        ResolutionRequest {
            syntax: self.syntax.clone(),
            sources: Some(Arc::clone(&self.sources)),
            top_level_bindings: self.imports.clone(),
            retained_base: None,
        }
    }
}

fn result<T>(value: Result<T, Vec<Diagnostic>>) -> Result<T, String> {
    value.map_err(|errors| format!("{errors:?}"))
}

fn fingerprint(
    trees: &SymbolResolvedTrees,
    pending: impl Iterator<Item = symbols::SymbolHandle>,
) -> u64 {
    let mut hash = DefaultHasher::new();
    for machine in trees.machines.iter() {
        machine.service_reach_row.hash(&mut hash);
        trees
            .service_reach_rows
            .services(machine.service_reach_row)
            .hash(&mut hash);
        trees
            .service_reach_rows
            .is_abstract(machine.service_reach_row)
            .hash(&mut hash);
        machine.name.as_str().hash(&mut hash);
        machine.symbol.hash(&mut hash);
        trees
            .symbols
            .symbol_source_span(machine.symbol)
            .hash(&mut hash);
    }
    for (_, expr) in trees.tables.bodies.expressions.iter_expressions() {
        if let source_compiler::source_resolved::expression::ExpressionNode::Call(call) = expr {
            call.target.source_span().hash(&mut hash);
            call.target_symbol.hash(&mut hash);
        }
    }
    for (_, statement) in trees.tables.declarations.state_statements.iter() {
        if let source_compiler::source_resolved::statement::Statement::Call(call) = statement {
            call.target.source_span().hash(&mut hash);
            call.target_symbol.hash(&mut hash);
        }
    }
    for definition in trees.service_reaches.definitions().iter() {
        format!("{definition:?}").hash(&mut hash);
    }
    for row in &trees.authored_service_reach_rows {
        format!("{row:?}").hash(&mut hash);
    }
    for selection in trees.authored_declaration_selections().iter() {
        format!("{selection:?}").hash(&mut hash);
    }
    for owner in pending {
        owner.hash(&mut hash);
    }
    hash.finish()
}

fn frontier_fingerprint(frontier: &DeferredResolvedFrontier) -> u64 {
    fingerprint(frontier.trees(), frontier.pending_machines())
}

// The input's ownership matters. Each iteration prepares a fresh request, and
// the clock stops before we inspect or destroy the returned trees.
fn time<I, O>(
    repeats: u64,
    prepare: impl Fn() -> Result<I, String>,
    run: impl Fn(I) -> Result<O, Vec<Diagnostic>>,
    inspect: impl Fn(O) -> Result<u64, String>,
) -> Result<(), String> {
    let once = || {
        let input = black_box(prepare()?);
        let start = Instant::now();
        let output = black_box(run(input));
        let elapsed = start.elapsed();
        Ok::<_, String>((elapsed, inspect(result(output)?)?))
    };
    let (_, checksum) = once()?;
    let mut elapsed = Duration::ZERO;
    for _ in 0..repeats {
        let (sample, observed) = once()?;
        if observed != checksum {
            return Err("resolver output changed between repetitions".into());
        }
        elapsed += sample;
    }
    println!(
        "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{},\"work_units\":{repeats},\"checksum\":\"{checksum}\"}}",
        elapsed.as_nanos()
    );
    Ok(())
}

fn measure() -> Result<(), String> {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|s| s != "--bench")
        .collect();
    let [operation, case, repeats] = args.as_slice() else {
        return Err("usage: resolver_timing <full|deferred|body|extension> <tiny|imports|generics|reach|machines-N> <repeats>".into());
    };
    let repeats = repeats.parse::<u64>().map_err(|e| e.to_string())?;
    if repeats == 0 {
        return Err("repeats must be positive".into());
    }
    let pkg = Package::parse(case)?;
    match operation.as_str() {
        "full" => time(
            repeats,
            || Ok(pkg.request()),
            resolve,
            |trees| Ok(fingerprint(&trees, std::iter::empty())),
        ),
        "deferred" => time(
            repeats,
            || Ok(pkg.request()),
            resolve_deferred,
            |f| Ok(frontier_fingerprint(&f)),
        ),
        "body" => {
            let frontier = result(resolve_deferred(pkg.request()))?;
            let main = frontier
                .pending_machines()
                .find(|id| frontier.trees().symbols.name(*id) == "main")
                .ok_or("main body was not deferred")?;
            time(
                repeats,
                || Ok(frontier.clone()),
                |f| {
                    f.complete_machine_bodies(&[main])
                        .map(|c| c.into_resolved_frontier())
                },
                |f| {
                    if f.pending_machines().any(|id| id == main) {
                        return Err("main body is still pending".into());
                    }
                    Ok(frontier_fingerprint(&f))
                },
            )
        }
        "extension" => {
            let base = result(resolve_deferred(pkg.request()))?.shared_trees();
            let declarations = Arc::new(pkg.syntax.clone());
            let mut sources = pkg.sources.as_ref().clone();
            let text = "machine generated() { main(); }";
            let id = sources
                .add("pkg/generated.omg".into(), text.into())
                .source_id;
            let tokens = frontend::Lexer::new(text)
                .tokenize()
                .map_err(|e| e.message)?;
            let syntax =
                frontend::parse_syntax_trees_with_id(id, &tokens).map_err(|e| e.message)?;
            let sources = Arc::new(sources);
            time(
                repeats,
                || {
                    Ok(ExtensionRequest {
                        base: Arc::clone(&base),
                        base_declarations: Arc::clone(&declarations),
                        syntax: syntax.clone(),
                        sources: Arc::clone(&sources),
                        top_level_bindings: Vec::new(),
                    })
                },
                resolve_deferred_extension,
                |ext| {
                    // The public join consumes the extension. Inspection happens after timing.
                    let (_, frontier) = result(
                        ext.rebase_for_typed_continuation(base.authored_declaration_selections()),
                    )?;
                    let retained = frontier.trees().machines.iter().map(|m| m.symbol);
                    if !retained
                        .take(base.machines.len())
                        .eq(base.machines.iter().map(|m| m.symbol))
                    {
                        return Err("generated source changed a base machine identity".into());
                    }
                    if !frontier
                        .pending_machines()
                        .any(|id| frontier.trees().symbols.name(id) == "generated")
                    {
                        return Err("extension lost generated body".into());
                    }
                    Ok(frontier_fingerprint(&frontier))
                },
            )
        }
        name => Err(format!("unknown operation {name}")),
    }
}

fn main() -> Result<(), String> {
    execution_admission::ExecutionBudget::with_workers(1)
        .map_err(|e| e.to_string())?
        .run(measure)
}
