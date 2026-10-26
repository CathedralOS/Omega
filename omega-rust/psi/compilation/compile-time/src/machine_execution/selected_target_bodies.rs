//! A realization capsule supplies exact target-family bodies as data.
//!
//! Omega chooses a retained member; this owner independently rejoins that row
//! against Psi's family catalog before preparing the private evaluation graph.
//! Source still selected the portable head. Its ordinary body, contracts,
//! effects, termination and declaration custody remain evaluator obligations.

use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionNode, name::Identifier};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBuildTimeTargetBody {
    head: SymbolHandle,
    member: SymbolHandle,
    target_scope: String,
}

impl SelectedBuildTimeTargetBody {
    pub fn prepare(
        program: &TypedTrees,
        head: SymbolHandle,
        member: SymbolHandle,
        target_scope: &str,
    ) -> Result<Self, String> {
        let row = Self {
            head,
            member,
            target_scope: target_scope.to_owned(),
        };
        row.validate(program)?;
        Ok(row)
    }

    fn validate(&self, program: &TypedTrees) -> Result<(), String> {
        let mut families = program
            .target_machine_families
            .iter()
            .filter(|family| family.canonical == self.head);
        let family = families
            .next()
            .ok_or("selected constant target body lost its exact family")?;
        if families.next().is_some()
            || self.target_scope.is_empty()
            || family
                .members
                .iter()
                .filter(|member| {
                    member.machine == self.member && member.target.as_str() == self.target_scope
                })
                .count()
                != 1
            || family
                .members
                .iter()
                .filter(|member| member.target.as_str() == self.target_scope)
                .count()
                != 1
        {
            return Err(
                "selected constant target body has ambiguous member or target custody".to_owned(),
            );
        }
        let head = program
            .machine_by_symbol(self.head)
            .ok_or("selected constant family lost its portable head")?;
        let member = program
            .machine_by_symbol(self.member)
            .ok_or("selected constant family lost its body")?;
        if program.symbols.symbol_package_identity(head.symbol)
            != program.symbols.symbol_package_identity(member.symbol)
        {
            return Err(
                "selected constant family body crosses declaration package custody".to_owned(),
            );
        }
        if !head.target_family_interface
            || member.target_family_interface
            || !member.supplies_checked_body()
            || program.machine_authored_target(member) != Some(self.target_scope.as_str())
        {
            return Err(
                "selected constant target body does not realize its retained declaration"
                    .to_owned(),
            );
        }
        let head_entry = program
            .machine_states(head)
            .first()
            .ok_or("selected constant family lost its head signature")?;
        let member_entry = program
            .machine_states(member)
            .first()
            .ok_or("selected constant family lost its body signature")?;
        let head_parameters = program.state_parameters(head_entry);
        let member_parameters = program.state_parameters(member_entry);
        if head_parameters.len() != member_parameters.len()
            || program.normalized_type_identity(head_entry.return_type)
                != program.normalized_type_identity(member_entry.return_type)
            || !head_parameters
                .iter()
                .zip(member_parameters)
                .all(|(head, member)| {
                    head.is_const == member.is_const
                        && head.is_mutable == member.is_mutable
                        && program.normalized_type_identity(head.type_reference)
                            == program.normalized_type_identity(member.type_reference)
                })
        {
            return Err(
                "selected constant target body differs from its portable signature".to_owned(),
            );
        }
        Ok(())
    }

    pub fn receipt(&self) -> typed_trees::constant_recipes::ConstantTargetBodySelection {
        typed_trees::constant_recipes::ConstantTargetBodySelection {
            head: self.head,
            member: self.member,
            target_scope: self.target_scope.clone(),
        }
    }
}

/// The caller owns a private evaluation copy. Rebind only live exact head
/// calls, preserving their complete actuals and original source selections.
/// This does not admit an invocation or select any member by spelling/host.
pub fn prepare_selected_target_family_calls(
    program: &mut TypedTrees,
    rows: &[SelectedBuildTimeTargetBody],
) -> Result<(), String> {
    for (index, row) in rows.iter().enumerate() {
        row.validate(program)?;
        if rows[..index].iter().any(|prior| prior.head == row.head) {
            return Err("constant family has competing selected bodies".to_owned());
        }
    }
    let mut bindings = std::collections::BTreeMap::new();
    for row in rows {
        let head = program
            .machine_by_symbol(row.head)
            .ok_or("constant family head disappeared")?;
        let head_entry = program
            .machine_states(head)
            .first()
            .ok_or("constant family signature disappeared")?;
        let member = program
            .machine_by_symbol(row.member)
            .ok_or("constant family body disappeared")?;
        let member_entry = program
            .machine_states(member)
            .first()
            .ok_or("constant family body entry disappeared")?;
        for symbol in [row.head, head_entry.symbol] {
            let key = (symbol.arena_index(), symbol.generation());
            let binding = (member_entry.symbol, member.name.as_str().to_owned());
            if bindings.insert(key, binding).is_some() {
                return Err("constant family call has competing exact bindings".to_owned());
            }
        }
    }
    let expressions = typed_trees::references::collect_program_references(program).expressions;
    let mut selections = program.authored_declaration_selections().clone();
    for expression in expressions {
        let Some(call) = program.expression_table.call_node(expression) else {
            continue;
        };
        let Some((symbol, name)) = bindings.get(&(
            call.target_symbol.arena_index(),
            call.target_symbol.generation(),
        )) else {
            continue;
        };
        let (symbol, name) = (*symbol, name.clone());
        for occurrence in program
            .expression_table
            .authored_selection_occurrences(expression)
        {
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
            };
            let selection = selections
                .get(occurrence)
                .ok_or("constant family call lost its source selection")?;
            if selection.target()
                == AuthoredDeclarationSelectionTarget::LateBound(
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                )
            {
                selections
                    .finalize_late_bound(
                        occurrence,
                        AuthoredDeclarationSelectionLateBinding::CheckedCall,
                        call.target_symbol,
                    )
                    .map_err(|reason| {
                        format!("constant family source selection could not settle: {reason:?}")
                    })?;
            }
        }

        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            unreachable!()
        };
        call.target_symbol = symbol;
        call.target = Identifier::generated(name);
    }
    program.retain_authored_declaration_selections(selections);
    Ok(())
}
