//! Independent validation of dead-store elimination.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! removal's legality from the source records — the named instruction's
//! store shape against the bound constraint row, the write row's place and
//! byte extent (or the fixed bytes a resolved `index`/`length` collapses
//! the dynamic extent to), the forward walk that must find one covering
//! write on every reachable path or confine the path to clear blocks
//! forever, the crossed-edge and boundary-settlement audits, and the
//! dropped-row contract — then rebuilds the function the contract demands
//! and requires the proposal to equal it. Restoring the store and its
//! roster rows must reproduce the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.
use crate::rewrites::block_edges::terminator_successors;
use crate::rewrites::window_hazards::unrostered_reach;
