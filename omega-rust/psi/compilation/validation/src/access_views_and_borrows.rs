//! Source access-call metadata. Executable access and write-only custody is
//! judged by the lowered checker, not by a second typed-body walk.

pub mod access_views {
    use crate::CheckingInput;
    pub use typed_trees::access_atomic_observing_results;
    use typed_trees::{
        expression::ExpressionHandle, machine::Machine, state::State, statement::StatementNode,
    };
    pub(crate) fn statement_call_target(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        call: &typed_trees::statement::TableCall,
    ) -> Option<symbols::SymbolHandle> {
        let members = crate::machine_calls::calls::signature_queries::call_receiver_member_names(
            program, call,
        );
        crate::machine_calls::calls::accessor_target(
            program,
            machine,
            state,
            &members,
            call.target_symbol,
            call.target.as_str(),
        )
    }
    pub(crate) fn statement_expression_roots(
        program: &dyn CheckingInput,
        statement: &StatementNode,
        include_assignment_targets: bool,
    ) -> Vec<ExpressionHandle> {
        use typed_trees::statement::StatementRootRole as Role;
        program
            .tables
            .expression_roots(statement)
            .into_iter()
            .filter(|(role, _)| match role {
                Role::AssignmentTarget => include_assignment_targets,
                _ => true,
            })
            .map(|(_, root)| root)
            .collect()
    }
}
pub use access_views::access_atomic_observing_results;
