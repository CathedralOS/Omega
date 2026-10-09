//! Replay of the in-block commuting run interchange: re-derive the window
//! the two named runs bound from the source, require the proposed program
//! to place exactly the later run, the interior, and the earlier run in
//! that order inside the window with the roster equal to the source's own
//! rows permuted into the new execution order, then restore the source by
//! content — splicing the source window back and writing the source
//! order's rows back into the window's roster positions must reproduce the
//! complete selected program bit-for-bit.
