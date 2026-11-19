//! Read the declared service, pause and crash bounds at their source clauses so inference can compare them with the behavior of checked bodies.

use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{PauseEffects, ServiceApplication, TypeError};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

#[derive(Clone, Copy)]
pub(crate) struct DeclaredPauseBounds {
    pub(super) suspend: Option<bool>,
    pub(super) block: Option<bool>,
}
impl DeclaredPauseBounds {
    pub(crate) fn interface(published: bool) -> Self {
        Self {
            block: published.then_some(false),
            suspend: published.then_some(false),
        }
    }
    pub fn declared(self) -> PauseEffects {
        PauseEffects {
            suspend: self.suspend == Some(true),
            block: self.block == Some(true),
        }
    }
    pub(crate) fn read(
        unit: &ResolvedSourceFile<'_>,
        n: usize,
    ) -> Result<Self, TypeError> {
        let f = unit.file;
        let published = f.nodes[n].words().any(|w| w.text == "public")
            || f.nodes[n]
                .parent
                .is_some_and(|p| f.nodes[p].kind == NodeKind::Trait && f.nodes[p].words().any(|w| w.text == "boundary"));
        let flag = |kind| -> Result<Option<bool>, TypeError> {
            let Some(c) = f.child(n, kind) else {
                return Ok(published.then_some(false));
            };
            if !f.nodes[c].children.is_empty() {
                return Err(TypeError::unsupported(f, c));
            }
            match f.nodes[c].words().nth(1).map(|w| w.text) {
                Some("false") => Ok(Some(false)),
                None | Some("true") => Ok(Some(true)),
                _ => Err(TypeError::unsupported(f, c)),
            }
        };
        Ok(Self {
            suspend: flag(NodeKind::Suspends)?,
            block: flag(NodeKind::Blocks)?,
        })
    }
}
pub(crate) fn crashes(
    unit: &ResolvedSourceFile<'_>,
    n: usize,
) -> Result<Option<Vec<crate::CrashCause>>, TypeError> {
    let f = unit.file;
    let mut causes = Vec::new();
    let mut specified = false;
    let mut empty = false;
    for &c in &f.nodes[n].children {
        if f.nodes[c].kind != NodeKind::Crashes {
            continue;
        }
        specified = true;
        let (&head, guards) = f.nodes[c]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(f, c))?;
        if !guards.is_empty() && matches!(unit.symbol(head),Some(Symbol::Builtin(s)) if s=="none") {
            return Err(TypeError::at(f, c, "crashes none cannot carry a route guard"));
        }
        match unit.symbol(head) {
            Some(Symbol::Builtin(s)) if s == "none" => empty = true,
            Some(Symbol::Builtin(s)) if s == "Trap" => causes.push(crate::CrashCause::Trap),
            Some(Symbol::Builtin(s)) if s == "Abort" => causes.push(crate::CrashCause::Abort),
            _ => return Err(TypeError::at(f, head, "crash clause requires Trap, Abort or none")),
        }
    }
    if empty && !causes.is_empty() {
        return Err(TypeError::at(f, n, "crashes none conflicts with a declared cause"));
    }
    causes.sort_by_key(|cause| matches!(cause, crate::CrashCause::Abort));
    causes.dedup();
    Ok((specified
        || f.nodes[n].words().any(|w| w.text == "public")
        || f.nodes[n]
            .parent
            .is_some_and(|p| f.nodes[p].kind == NodeKind::Trait && f.nodes[p].words().any(|w| w.text == "boundary")))
    .then_some(causes))
}
pub(crate) fn empty_clause(
    unit: &ResolvedSourceFile<'_>,
    n: usize,
) -> bool {
    matches!(unit.file.nodes[n].children.as_slice(),[i] if matches!(unit.symbol(*i),Some(Symbol::Builtin(s)) if s=="none"))
}
pub(crate) fn reach(
    unit: &ResolvedSourceFile<'_>,
    types: &TypeCatalog<'_>,
    n: usize,
) -> Result<Option<Vec<ServiceApplication>>, TypeError> {
    let f = unit.file;
    let Some(clause) = f.child(n, NodeKind::Reaches) else {
        return Ok(None);
    };
    if empty_clause(unit, clause) {
        return Ok(Some(Vec::new()));
    }
    let mut row = Vec::new();
    for &entry in &f.nodes[clause].children {
        let Some(Symbol::Declaration(id)) = unit.symbol(entry) else {
            return Err(TypeError::unsupported(f, entry));
        };
        if id.kind != NodeKind::Trait {
            return Err(TypeError::at(f, entry, "reach requires a service trait"));
        }
        let args = f
            .child(entry, NodeKind::TypeArgs)
            .into_iter()
            .flat_map(|section| &f.nodes[section].children)
            .enumerate()
            .map(|(index, &arg)| unit.argument(arg, types, id, index))
            .collect::<Result<Vec<_>, _>>()?;
        if args.len() != types.arities[id] {
            return Err(TypeError::at(f, entry, "incomplete service application"));
        }
        let service = ServiceApplication {
            service: id.clone(),
            arguments: args,
        };
        if !row.contains(&service) {
            row.push(service);
        }
    }
    Ok(Some(row))
}

pub(crate) fn markers(
    unit: &ResolvedSourceFile<'_>,
    mut n: usize,
) -> Result<PauseEffects, TypeError> {
    use NodeKind::*;
    let f = unit.file;
    let mut markers = PauseEffects::default();
    while let Some(parent) = f.nodes[n].parent.filter(|&p| f.nodes[p].kind == CallMarker) {
        match f.nodes[parent].words().next().map(|w| w.text) {
            Some("block") if !markers.block && !markers.suspend => markers.block = true,
            Some("suspend") if !markers.suspend => markers.suspend = true,
            _ => return Err(TypeError::at(f, parent, "repeated or reversed call acknowledgement")),
        }
        n = parent;
    }
    if markers.suspend {
        let legal = f.nodes[n].parent.is_some_and(|p| match f.nodes[p].kind {
            Tail | ExpressionStatement | Discard => true,
            Let => f.nodes[p].children.last() == Some(&n) && f.nodes[f.nodes[p].children[0]].kind == BindingPattern,
            Transition => f.nodes[p].children.first() == Some(&n),
            Arm => f.nodes[p].children.last() == Some(&n) && f.nodes[p].parent.is_some_and(|d| f.nodes[d].kind == Transition),
            _ => false,
        });
        if !legal {
            return Err(TypeError::at(f, n, "suspend acknowledgement requires a complete suspension position"));
        }
    }
    Ok(markers)
}
