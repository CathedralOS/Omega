//! Load unchanged alloc source in an explicit retained test package plus genuine
//! bundled core, then retain its query/plan. This setup supplies no package-lock
//! admission or private policy selection authority.
//! No closed application, checked tree or catalogue row is inserted here.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use symbols::{SourceScopedTopLevelBinding, SymbolHandle};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};

pub(super) fn actual_vec_cell_source(
    library: &Path,
    query_text: &str,
) -> Result<(TypedTrees, ExpressionHandle, SymbolHandle), String> {
    let core_root = library
        .join("core")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let alloc_root = library
        .join("alloc")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xA1; 32]);
    let mut sources = source::SourceMap::default();
    let mut units = Vec::new();
    let mut paths = Vec::new();
    retained_units(&core_root, &mut paths)?;
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let file = sources.add_with_metadata(
            path,
            text,
            core_root.clone(),
            None,
            source::SourceOrigin::Toolchain,
        );
        units.push((file.source_id, Arc::clone(&file.source)));
    }
    let core_anchor = units
        .first()
        .ok_or("real core source selection is empty")?
        .0;
    let mut vec_source = None;
    for name in ["main.omg", "bump.omg", "vec.omg"] {
        let path = alloc_root.join(name);
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let file = sources.add_with_metadata(
            path,
            text,
            alloc_root.clone(),
            package,
            source::SourceOrigin::User,
        );
        let id = file.source_id;
        if name == "vec.omg" {
            vec_source = Some(id);
        }
        units.push((id, Arc::clone(&file.source)));
    }
    let file = sources.add_with_metadata(
        alloc_root.join("query.omg"),
        query_text.into(),
        alloc_root,
        package,
        source::SourceOrigin::User,
    );
    let requester = file.source_id;
    units.push((requester, Arc::clone(&file.source)));
    let mut syntax = syntax_trees::SyntaxTrees::new(requester);
    let mut bindings = Vec::new();
    let mut plan_source = None;
    for (id, text) in units {
        let previous = syntax.root_item_handles().len();
        let tokens = source_files_to_tokens::Lexer::new(&text)
            .tokenize()
            .map_err(|errors| format!("lex source {id:?}: {errors:?}"))?;
        tokens_to_syntax_trees::parse(&mut syntax, id, &tokens)
            .map_err(|errors| format!("parse source {id:?}: {errors:?}"))?;
        for root in &syntax.root_item_handles()[previous..] {
            // This fixture selects the authored attached declaration before
            // resolution. Display names can include its complete owner path;
            // later stages must keep this occurrence and its exact symbol.
            if Some(id) == vec_source
                && let syntax_trees::item::Item::Machine(machine) = syntax.root_item(*root)
                && machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|owner| owner.as_str(&syntax) == "VecCellPolicy")
                && syntax
                    .items
                    .state_handles(machine.states)
                    .first()
                    .is_some_and(|state| syntax.items.state(*state).name.as_str(&syntax) == "plan")
            {
                if plan_source.is_some() {
                    return Err(
                        "unchanged vector source has ambiguous authored plan declarations".into(),
                    );
                }
                plan_source = Some(machine.name.source_span());
            }
            let syntax_trees::item::Item::Use(import) = syntax.root_item(*root) else {
                continue;
            };
            let members = syntax.items.identifier_path_members(import.path);
            if members.len() >= 3
                && members[0].as_str(&syntax) == "omega"
                && members[1].as_str(&syntax) == "language"
                && members[2].as_str(&syntax) == "core"
            {
                let path = members
                    .iter()
                    .map(|member| member.as_str(&syntax))
                    .collect::<Vec<_>>()
                    .join("::");
                // The same loader binding selects declarations across this
                // exact admitted core package; its anchor creates no module.
                bindings.push(SourceScopedTopLevelBinding::logical_module_import(
                    id,
                    core_anchor,
                    &path,
                    3,
                ));
            }
        }
    }
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    request.top_level_bindings = bindings;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request)
        .map_err(|errors| format!("resolve actual alloc/core: {errors:?}"))?;
    let plan_source =
        plan_source.ok_or("unchanged vector source has no authored plan declaration")?;
    let mut plans = resolved
        .machines
        .iter()
        .filter(|machine| machine.name.source_span() == plan_source);
    let plan = plans
        .next()
        .ok_or("resolution lost the actual authored plan occurrence")?;
    if plans.next().is_some() {
        return Err("resolution made the actual authored plan occurrence ambiguous".into());
    }
    let (plan_symbol, plan_owner) = (plan.symbol, plan.attached_data_symbol);
    if resolved.data_definition_by_symbol(plan_owner).is_none() {
        return Err("authored vector plan has no exact declared data owner".into());
    }
    let retained_plan = symbols::RetainedDeclaration::capture(&resolved.symbols, plan_symbol)
        .map_err(|reason| format!("retain authored vector plan: {reason}"))?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|errors| format!("type actual alloc/core: {errors:?}"))?;
    let query = typed
        .realized_machine_named("describe_cell")
        .ok_or("actual query caller was not retained")?
        .symbol;
    let expression = {
        let mut calls =
            typed
                .expression_table
                .iter_expressions()
                .filter_map(|(expression, node)| {
                    let ExpressionNode::Call(call) = node else {
                        return None;
                    };
                    let request =
                        typed_trees::references::StaticArgumentCall::Expression(expression, call);
                    (request.lexical_owner(&typed) == query).then_some(expression)
                });
        let expression = calls
            .next()
            .ok_or("actual query caller has no retained call")?;
        if calls.next().is_some() {
            return Err("actual query caller has multiple retained requests".into());
        }
        expression
    };
    let declaration = typed
        .machine_by_symbol(plan_symbol)
        .ok_or("typing lost the actual authored vector plan declaration")?;
    if declaration.attached_data_symbol != plan_owner
        || typed.unique_data_definition_by_symbol(plan_owner).is_none()
    {
        return Err("typing changed the authored vector plan's attached owner".into());
    }
    let plan = retained_plan
        .rejoin(&typed.symbols, [declaration.symbol])
        .map_err(|reason| format!("rejoin authored vector plan after typing: {reason}"))?;
    Ok((typed, expression, plan))
}

fn retained_units(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            retained_units(&path, paths)?;
        } else if path.extension().is_some_and(|extension| extension == "omg")
            && path.file_name().is_some_and(|name| name != "build.omg")
        {
            paths.push(path);
        }
    }
    Ok(())
}
