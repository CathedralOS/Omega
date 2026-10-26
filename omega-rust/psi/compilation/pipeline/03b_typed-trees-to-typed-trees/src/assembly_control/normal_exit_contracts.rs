//! Preserve declared region-exit goals across extraction into an ordinary call.
//!
//! Local transfers cannot return from the helper; normalization::exit creates a
//! normal completion only at the lexical body's falling-through edge. A final
//! conditional also has that not-taken completion. Only explicit Ensures at
//! that boundary are candidates, and only their captured-reference subjects
//! escape: internal scalar registers have separate versions, not caller homes.
//! These become ordinary helper Ensures, checked on EVERY normal helper exit;
//! the label goal itself and all incoming-edge obligations remain checked too.
//! No requirement, last observed value, or syntactic write scan proves a goal.

use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContract, SignatureContractKind};
use typed_trees::state::State;
use typed_trees::statement::{AssemblyControlFlow, AssemblyExit, AssemblyFactKind};

pub(super) fn append(
    program: &mut TypedTrees,
    source: &TypedTrees,
    state: &State,
    graph: &AssemblyControlFlow,
    body_start: usize,
    mapping: &[(SymbolHandle, SymbolHandle)],
    captured_references: &[SymbolHandle],
    helper: &mut Machine,
) {
    let blocks = source.statement_table.assembly_blocks(graph.blocks);
    let Some((exit_index, block)) = blocks.iter().enumerate().next_back() else {
        return;
    };
    if !matches!(
        block.exit,
        AssemblyExit::Fallthrough | AssemblyExit::ZeroFlag(_, _)
    ) {
        return;
    }
    let window = HandleSpan::from_parts(
        state
            .statement_nodes
            .handle_at((body_start + block.start as usize + 1) as u32),
        block.count - 1,
    );
    for statement in window.handles() {
        let Some(fact) = source
            .statement_table
            .statement(statement)
            .as_assembly_fact()
        else {
            continue;
        };
        if fact.kind != AssemblyFactKind::Ensures {
            continue;
        }
        let copied = program
            .tables
            .copy_statement_nodes_deep_from(&source.tables, HandleSpan::from_parts(statement, 1));
        program.tables.remap_statement_symbols(copied, mapping);
        let Some(fact) = program
            .statement_table
            .statement(copied.handle_at(0))
            .as_assembly_fact()
        else {
            continue;
        };
        let fact = program.proof_facts.get(fact.fact).clone();
        let ProofFact::Expression(expression) = &fact else {
            continue;
        };
        let mut expressions = Vec::new();
        validation::expression_traversal::collect_expression_tree(
            program,
            *expression,
            &mut expressions,
        );
        let mut observes_capture = false;
        let mut escapes_frame = false;
        for expression in expressions {
            // Saved-subject definitions have their own custody; ordinary name
            // remapping deliberately does not move those capture identities.
            // Keep their source assertion, but do not export an ambient snapshot.
            if matches!(
                program.expression_table.expression(expression),
                ExpressionNode::SavedSubject(_)
            ) {
                escapes_frame = true;
            }
            if let ExpressionNode::Name(path) = program.expression_table.expression(expression) {
                if !path.head_symbol.is_valid() {
                    escapes_frame = true;
                    continue;
                }
                if captured_references.contains(&path.head_symbol) {
                    observes_capture = true;
                } else if matches!(
                    program.symbols.get(path.head_symbol).kind,
                    SymbolKind::Local | SymbolKind::Parameter
                ) {
                    escapes_frame = true;
                }
            }
        }
        if !observes_capture || escapes_frame {
            continue;
        }
        // In the one-block conditional case, extraction runs the prefix before
        // the helper invocation. Its exit goal is therefore an ordinary entry
        // obligation too, proved at that exact call after the prefix executed.
        let kinds: &[SignatureContractKind] = if exit_index == 0 {
            &[
                SignatureContractKind::Requires,
                SignatureContractKind::Ensures,
            ]
        } else {
            &[SignatureContractKind::Ensures]
        };
        for kind in kinds {
            let facts = program.proof_facts.insert_many([fact.clone()]);
            program.push_machine_contract(
                helper,
                SignatureContract {
                    kind: kind.clone(),
                    keyword_source_span: None,
                    binding: None,
                    facts,
                    token_count: 0,
                },
            );
        }
    }
}
