//! `[erased]` noninterference for statement-call receivers: the receiver of a
//! `receiver.state(...)` call is the callee's `self` operand, so it is a
//! runtime place exactly like a direct read. A runtime call through an erased
//! local, parameter, attached field, or a projection crossing an erased field
//! rejects with the same diagnostic family as a runtime read; calls to proof
//! machines and calls inside proof machines are proof computation and keep
//! their erased receivers.
//!
//! A `builder.roots.bind(slot, implementation)` statement is not a call: its
//! receiver and its delegated `ProductEntryRef` operand are places read at
//! build evaluation, so an erased receiver or operand rejects identically.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolved source");
    lower_symbol_resolved_trees(&resolved).expect("typed source")
}

fn rejects(source: &str, fragment: &str) {
    let diagnostics = validation::validate_program(&typed(source))
        .expect_err("erased receiver on a runtime call must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}"
    );
}

fn accepts(source: &str) {
    validation::validate_program(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

#[test]
fn erased_local_receiver_rejects() {
    rejects(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) { self.count = 1; }
         machine inspect() -> u64 {
             let c [erased]: Counter = Counter { count: 0 };
             c.bump();
             0
         }",
        "erased local `c` has no runtime value",
    );
}

#[test]
fn erased_parameter_receiver_rejects() {
    rejects(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) { self.count = 1; }
         machine inspect(c [erased]: Counter) -> u64 {
             c.bump();
             0
         }",
        "erased parameter `c` has no runtime value",
    );
}

#[test]
fn erased_field_receiver_rejects() {
    rejects(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) { self.count = 1; }
         data Main { c [erased]: Counter; }
         machine Main::inspect(&mut self) { self.c.bump(); }",
        "erased field `c` has no runtime value",
    );
}

#[test]
fn receiver_projection_through_erased_field_rejects() {
    rejects(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) { self.count = 1; }
         data Inner { c: Counter; }
         data Holder { inner [erased]: Inner; }
         data Main { holder: Holder; }
         machine Main::inspect(&mut self) { self.holder.inner.c.bump(); }",
        "erased field `inner` has no runtime value",
    );
}

#[test]
fn proof_machine_receiver_accepts_erased() {
    // `P` is recursive data with no layout (proof-only), so `P::touch(self)`
    // is a proof machine and the erased local legally supplies its receiver.
    accepts(
        "data P { case Z; case S(prev: P); }
         machine P::touch(self) {}
         machine inspect() -> u64 {
             let p [erased]: P = P::Z {};
             p.touch();
             0
         }",
    );
}

#[test]
fn proof_context_receiver_accepts_erased() {
    // A free machine whose signature mentions proof-only data is a proof
    // machine: its whole body is proof context, so the erased receiver inside
    // is not a runtime use.
    accepts(
        "data P { case Z; case S(prev: P); }
         machine P::touch(self) {}
         machine lemma(witness: P) -> u64 {
             let p [erased]: P = P::Z {};
             p.touch();
             0
         }",
    );
}

#[test]
fn erased_root_binding_receiver_rejects() {
    // `builder.roots.bind(...)` lowers to a RootBinding statement whose
    // receiver is the `&mut Build` place read at build evaluation. It is not
    // a statement call, so only this walk can see an erased receiver.
    rejects(
        "data Build {} \
         machine build(builder [erased]: &mut Build) { \
             builder.roots.bind(macos_arm64::ProgramEntry, Main::main); \
         }",
        "erased parameter `builder` has no runtime value",
    );
}

#[test]
fn erased_parameter_root_binding_operand_rejects() {
    // A bare implementation name resolving to a binding stays on
    // `implementation_operand` and is evaluated as the described
    // `ProductEntryRef` place at build time; an erased binding cannot supply it.
    rejects(
        "data Build {} \
         data ProductEntryRef {} \
         machine build(builder: &mut Build, entry [erased]: ProductEntryRef) { \
             builder.roots.bind(macos_arm64::ProgramEntry, entry); \
         }",
        "erased parameter `entry` has no runtime value",
    );
}

#[test]
fn erased_local_root_binding_operand_rejects() {
    rejects(
        "data Build {} \
         data ProductEntryRef {} \
         machine build(builder: &mut Build, entry [erased]: ProductEntryRef) { \
             let e [erased]: ProductEntryRef = entry; \
             builder.roots.bind(macos_arm64::ProgramEntry, e); \
         }",
        "erased local `e` has no runtime value",
    );
}

#[test]
fn retained_root_binding_operands_still_accepted() {
    // The canonical build program binds a lexical implementation path, and a
    // retained delegated operand keeps its place read: neither is an erased
    // runtime use.
    accepts(
        "data Build {} \
         data ProductEntryRef {} \
         machine build(builder: &mut Build, entry: ProductEntryRef) { \
             builder.roots.bind(macos_arm64::ProgramEntry, entry); \
             builder.roots.bind(macos_arm64::ProgramEntry, Main::main); \
         }",
    );
}

#[test]
fn relevant_receiver_still_accepted() {
    accepts(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) { self.count = 1; }
         data Main { c: Counter; }
         machine Main::inspect(&mut self) { self.c.bump(); }",
    );
}
