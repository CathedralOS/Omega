//! Independent validation of boundary-decided predicate materialization.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the named instruction's
//! ordering `MaterializeBoolean*` kind and emitted `[def result]` shape,
//! the flag units it reads resolving through the shared condition-state
//! walk to one compare's published definitions, the compare's resolved
//! operand poles, and — for a decided predicate — the target's own
//! materialize row the rewritten instruction is built from — then
//! rebuilds the instruction the contract demands and requires the
//! proposal to carry it at the materialization's position. Reinserting
//! the source boolean must restore the complete source by content: every
//! other instruction, register, call, and function included. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.
