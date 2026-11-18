/*
    The one constructor for synthetic `let __hoist_N` temps.

    Every hoist pushes `let __hoist_N = <initial>;` ahead of the rewritten
    statement and reads the temp back through a fresh one-member `Name`.
    A missing annotation sets `type_is_inferred`. During statement typing,
    `infer_initializer_type` determines that local's type from its initializer.
*/

use super::place_operand_capture::{SelectorCapture, capture_reference_initializer_selectors};
use crate::resolution::lowering::expression::expression_table;
use crate::resolution::lowering::lowerer::Lowerer;
use crate::source_resolved::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::statement::{LocalData, LocalDataStorage, Statement};
use crate::source_resolved::types::TypeReference;
use arena::HandleSpan;
use symbols::SymbolHandle;

/*
    Emits `let __hoist_N = <initial>;` with an inferred type and returns a
    `Name` referencing it.
*/
pub(crate) fn hoist_into_temp(
    lowerer: &mut Lowerer,
    initial: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    mint_hoist_temp(lowerer, hoisted, None, None, false, |_, _| initial)
}

/*
    Capture an operand whose consumer determines whether a primitive reference
    supplies its value. Resolution cannot select the operator yet; retain this
    origin on the local rather than guessing from its initializer's spelling.
*/
pub(crate) fn hoist_operand_into_temp(
    lowerer: &mut Lowerer,
    initial: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    mint_hoist_temp(lowerer, hoisted, None, None, true, |_, _| initial)
}

/*
    Returns a `Name` for the temporary that holds this evaluation.
    Emit a local when the subject has no previous temporary.

    `shared_subject` is the match subject's exact syntax handle, which dispatch preparation
    shares across the lowered arms. The first arm mints
    the temp and records it; sibling arms reference that same temp, so
    exhaustiveness still groups them, and their own lowered subjects are left
    orphaned. `initial` runs only when minting and before the temp's name is
    drawn, so any temp it hoists for its own operands numbers first and
    precedes this one in `hoisted`.
*/
pub(crate) fn mint_hoist_temp(
    lowerer: &mut Lowerer,
    hoisted: &mut Vec<Statement>,
    annotation: Option<TypeReference>,
    shared_subject: Option<syntax_trees::expression::ExpressionHandle>,
    is_operand_capture: bool,
    initial: impl FnOnce(&mut Lowerer, &mut Vec<Statement>) -> ExpressionHandle,
) -> ExpressionHandle {
    let shared_temp = shared_subject.and_then(|subject| lowerer.match_subject_temp(subject));
    let (name, source_span) = match shared_temp {
        Some((existing, source_span)) => (DiagnosticName::generated(existing), source_span),
        None => {
            let initial_value = initial(lowerer, hoisted);
            let source_span = expression_table(lowerer).source_span(initial_value);
            capture_reference_initializer_selectors(
                lowerer,
                initial_value,
                SelectorCapture::Every,
                hoisted,
            );
            let fresh = lowerer.next_hoist_name();
            if let Some(subject) = shared_subject {
                lowerer.record_match_subject_temp(subject, fresh.clone(), source_span);
            }
            let name = DiagnosticName::generated(fresh);
            let type_is_inferred = annotation.is_none();
            hoisted.push(Statement::LocalData(LocalData {
                symbol: SymbolHandle::invalid(),
                name: name.clone(),
                storage: LocalDataStorage {
                    role: language_core::LocalBindingRole::Value,
                    type_reference: annotation.unwrap_or_default(),
                    initial_value,
                    is_mutable: false,
                    type_is_inferred,
                    is_operand_capture,
                    relevance: language_core::BindingRelevance::Relevant,
                    pattern_fields: Default::default(),
                },
            }));
            (name, source_span)
        }
    };
    let reference = reference_temp(lowerer, name);
    expression_table(lowerer).set_source_span(reference, source_span);
    reference
}

/*
    A fresh source occurrence of an already established temporary. Its symbol
    is resolved normally; the syntax occurrence selects the capture, not text
    similarity between two independently authored expressions.
*/
pub(in crate::resolution::lowering) fn reference_temp(
    lowerer: &mut Lowerer,
    name: DiagnosticName,
) -> ExpressionHandle {
    let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, name);
    let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
    expressions.insert(ExpressionNode::Name(TableNamePath::new(
        members,
        member_symbols,
        SymbolHandle::invalid(),
        SymbolHandle::invalid(),
    )))
}
