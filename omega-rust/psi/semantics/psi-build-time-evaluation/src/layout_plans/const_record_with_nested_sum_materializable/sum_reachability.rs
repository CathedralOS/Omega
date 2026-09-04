//! Bounded, non-recursive conventional-sum reachability.

use super::*;

#[derive(Default)]
pub(super) struct RecordSumProfile {
    pub(super) direct: bool,
    pub(super) array: bool,
    pub(super) deeper: bool,
}

pub(super) fn record_sum_profile(
    typed: &TypedTrees,
    data: &DataDefinition,
    reachability: &mut SumReachability<'_>,
) -> Result<RecordSumProfile, MaterializationDiagnostic> {
    let mut profile = RecordSumProfile::default();
    for member in typed.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        match typed
            .type_reference_table
            .type_reference(field.type_reference)
        {
            TypeReferenceNode::Named { .. } => {
                let Some(named) = exact_named_data(typed, field.type_reference)? else {
                    continue;
                };
                match DataDefinition::shape_kind_from_members(typed.data_members(named)) {
                    DataShapeKind::Enum => profile.direct = true,
                    DataShapeKind::Record => {
                        if reachability.type_contains_sum(field.type_reference)? {
                            profile.deeper = true;
                        }
                    }
                    DataShapeKind::Mixed => {
                        return Err(MaterializationDiagnostic(format!(
                            "field `{}` uses a mixed common-field/case shape",
                            field.name
                        )));
                    }
                    DataShapeKind::Empty => {}
                }
            }
            TypeReferenceNode::FixedArray { .. }
                if reachability.type_contains_sum(field.type_reference)? =>
            {
                profile.array = true;
            }
            _ => {}
        }
    }
    Ok(profile)
}

pub(super) fn reject_sum_array_type(
    typed: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    path: &str,
    reachability: &mut SumReachability<'_>,
) -> Result<(), MaterializationDiagnostic> {
    if matches!(
        typed.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::FixedArray { .. }
    ) && reachability.type_contains_sum(type_reference)?
    {
        return Err(MaterializationDiagnostic(format!(
            "{path} uses an array containing sums, outside the nested-record path rung"
        )));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ReachabilityState {
    Visiting,
    Done(bool),
}

struct ReachabilityFrame<'a> {
    data: &'a DataDefinition,
    next_member: usize,
    found: bool,
}

pub(in crate::layout_plans) struct SumReachability<'a> {
    typed: &'a TypedTrees,
    states: std::collections::HashMap<(u32, u32), ReachabilityState>,
    traversed_edges: usize,
}

impl<'a> SumReachability<'a> {
    const MAX_RECORDS: usize = 4096;
    pub(super) const MAX_EDGES: usize = 16384;

    pub(in crate::layout_plans) fn new(typed: &'a TypedTrees) -> Self {
        Self {
            typed,
            states: std::collections::HashMap::new(),
            traversed_edges: 0,
        }
    }

    pub(in crate::layout_plans) fn type_contains_sum(
        &mut self,
        mut type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Result<bool, MaterializationDiagnostic> {
        let mut array_depth = 0usize;
        while let TypeReferenceNode::FixedArray { element_type, .. } = self
            .typed
            .type_reference_table
            .type_reference(type_reference)
        {
            array_depth += 1;
            if array_depth > 64 {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable nested-record path exceeds bounded fixed-array depth"
                        .into(),
                ));
            }
            type_reference = *element_type;
        }
        let Some(data) = exact_named_data(self.typed, type_reference)? else {
            return Ok(false);
        };
        match DataDefinition::shape_kind_from_members(self.typed.data_members(data)) {
            DataShapeKind::Enum | DataShapeKind::Mixed => Ok(true),
            DataShapeKind::Empty => Ok(false),
            DataShapeKind::Record => self.record_contains_sum(data),
        }
    }

    fn record_contains_sum(
        &mut self,
        root: &'a DataDefinition,
    ) -> Result<bool, MaterializationDiagnostic> {
        let root_identity = symbol_identity(root.symbol)?;
        if let Some(state) = self.states.get(&root_identity) {
            return match state {
                ReachabilityState::Done(found) => Ok(*found),
                ReachabilityState::Visiting => Err(MaterializationDiagnostic(format!(
                    "ConstMaterializable nested-record path is recursive through `{}`",
                    root.name
                ))),
            };
        }
        self.insert_state(root_identity, ReachabilityState::Visiting)?;
        let mut stack = Vec::new();
        stack.try_reserve(1).map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record traversal stack exceeds compiler resources"
                    .into(),
            )
        })?;
        stack.push(ReachabilityFrame {
            data: root,
            next_member: 0,
            found: false,
        });

        loop {
            let Some(frame) = stack.last_mut() else {
                unreachable!("root reachability frame returns when completed")
            };
            let members = self.typed.data_members(frame.data);
            // `found` is the eventual answer, not permission to skip later
            // fields: a later branch can still expose a cycle, malformed
            // nominal identity, or resource-bound failure.
            if frame.next_member == members.len() {
                let completed = stack.pop().expect("active reachability frame");
                let identity = symbol_identity(completed.data.symbol)?;
                self.states
                    .insert(identity, ReachabilityState::Done(completed.found));
                if let Some(parent) = stack.last_mut() {
                    parent.found |= completed.found;
                    continue;
                }
                return Ok(completed.found);
            }
            let member = &members[frame.next_member];
            frame.next_member += 1;
            let DataMember::Field(field) = member else {
                frame.found = true;
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            self.traversed_edges = self.traversed_edges.checked_add(1).ok_or_else(|| {
                MaterializationDiagnostic(
                    "ConstMaterializable nested-record traversal edge count overflows".into(),
                )
            })?;
            if self.traversed_edges > Self::MAX_EDGES {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable nested-record path exceeds bounded schema traversal edges"
                        .into(),
                ));
            }
            let mut child_type = field.type_reference;
            let mut array_depth = 0usize;
            while let TypeReferenceNode::FixedArray { element_type, .. } =
                self.typed.type_reference_table.type_reference(child_type)
            {
                array_depth += 1;
                if array_depth > 64 {
                    return Err(MaterializationDiagnostic(
                        "ConstMaterializable nested-record path exceeds bounded fixed-array depth"
                            .into(),
                    ));
                }
                child_type = *element_type;
            }
            let Some(child) = exact_named_data(self.typed, child_type)? else {
                continue;
            };
            match DataDefinition::shape_kind_from_members(self.typed.data_members(child)) {
                DataShapeKind::Enum | DataShapeKind::Mixed => frame.found = true,
                DataShapeKind::Empty => {}
                DataShapeKind::Record => {
                    let identity = symbol_identity(child.symbol)?;
                    match self.states.get(&identity).copied() {
                        Some(ReachabilityState::Done(found)) => frame.found |= found,
                        Some(ReachabilityState::Visiting) => {
                            return Err(MaterializationDiagnostic(format!(
                                "ConstMaterializable nested-record path is recursive through `{}`",
                                child.name
                            )));
                        }
                        None => {
                            self.insert_state(identity, ReachabilityState::Visiting)?;
                            stack.try_reserve(1).map_err(|_| {
                                MaterializationDiagnostic(
                                    "ConstMaterializable nested-record traversal stack exceeds compiler resources"
                                        .into(),
                                )
                            })?;
                            stack.push(ReachabilityFrame {
                                data: child,
                                next_member: 0,
                                found: false,
                            });
                        }
                    }
                }
            }
        }
    }

    fn insert_state(
        &mut self,
        identity: (u32, u32),
        state: ReachabilityState,
    ) -> Result<(), MaterializationDiagnostic> {
        if self.states.len() >= Self::MAX_RECORDS {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path exceeds bounded schema traversal records"
                    .into(),
            ));
        }
        self.states.try_reserve(1).map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable nested-record visited map exceeds compiler resources".into(),
            )
        })?;
        self.states.insert(identity, state);
        Ok(())
    }
}

fn symbol_identity(
    symbol: psi_symbols::SymbolHandle,
) -> Result<(u32, u32), MaterializationDiagnostic> {
    if !symbol.is_valid() {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable nested-record path encountered an invalid nominal identity".into(),
        ));
    }
    Ok((symbol.arena_index(), symbol.generation()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn reachability_validates_siblings_after_an_already_found_sum() {
        let tokens = Lexer::new(
            r#"
            data Choice [copy] { case Empty; }
            data Trap [copy] { choice: Choice; later: u8; }
            data Root [copy] { trap: Trap; }
            "#,
        )
        .tokenize()
        .expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let trap = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Trap")
            .expect("Trap definition");
        let trap_symbol = trap.symbol;
        let trap_name = trap.name.clone();
        let later_type = typed
            .data_members(trap)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "later" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .expect("later field");
        let root = typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Root")
            .expect("Root definition");
        let trap_type = typed
            .data_members(root)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "trap" => {
                    Some(field.type_reference)
                }
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
            .expect("trap field");
        assert!(matches!(
            SumReachability::new(&typed).type_contains_sum(trap_type),
            Ok(true)
        ));

        let mut recursive = typed.clone();
        recursive.type_reference_table.substitute_node(
            later_type,
            TypeReferenceNode::Named {
                symbol: trap_symbol,
                name: trap_name.clone(),
            },
        );
        assert!(
            SumReachability::new(&recursive)
                .type_contains_sum(trap_type)
                .is_err(),
            "a later recursive sibling must not hide behind an earlier sum"
        );

        let mut malformed = typed;
        malformed.type_reference_table.substitute_node(
            later_type,
            TypeReferenceNode::Named {
                symbol: psi_symbols::SymbolHandle::invalid(),
                name: trap_name,
            },
        );
        assert!(
            SumReachability::new(&malformed)
                .type_contains_sum(trap_type)
                .is_err(),
            "a later malformed nominal branch must not hide behind an earlier sum"
        );
    }
}
