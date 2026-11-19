use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::declaration_bodies;
use crate::{TypeError, TypedStage};
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) fn infer<'a>(
    units: &[ResolvedSourceFile<'a>],
    catalog: &mut DeclarationCatalog<'a>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut inferred = Vec::new();
    let mut indices = HashMap::new();
    for unit in units {
        for (n, node) in unit
            .file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::Const && node.children.len() == 2)
        {
            let id = unit.declaration(n)?;
            indices.insert(id, inferred.len());
            inferred.push((unit, id, node.children[1]));
        }
    }
    let mut pending = Vec::new();
    let mut users = vec![Vec::new(); inferred.len()];
    let mut ready = VecDeque::new();
    for (i, &(unit, _, expr)) in inferred.iter().enumerate() {
        let mut deps = HashSet::new();
        let mut walk = vec![expr];
        while let Some(n) = walk.pop() {
            if let Some(Symbol::Declaration(id)) = unit.symbol(n)
                && let Some(&dep) = indices.get(id)
                && deps.insert(dep)
            {
                users[dep].push(i);
            }
            walk.extend(&unit.file.nodes[n].children);
        }
        pending.push(deps.len());
        if deps.is_empty() {
            ready.push_back(i);
        }
    }
    while let Some(i) = ready.pop_front() {
        let (unit, id, expr) = inferred[i];
        let value = declaration_bodies::infer_constant(unit, catalog, expr, output)?;
        catalog.constants.insert(id, value);
        for &user in &users[i] {
            pending[user] -= 1;
            if pending[user] == 0 {
                ready.push_back(user);
            }
        }
    }
    if let Some(i) = pending.iter().position(|&count| count != 0) {
        let (unit, _, expr) = inferred[i];
        return Err(TypeError::at(&unit.file, expr, "cannot infer a constant through a dependency cycle"));
    }
    Ok(())
}
