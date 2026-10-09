A rank-free finishing state tail-calls `left(0)`. The path `left(0) -> finish() -> left(0)` repeats without visiting the right-to-left strict edge. This state is recurrent and cannot receive the exemption for an acyclic suffix that drops its rank formal.

The frozen baseline rejects rank-range formation for the selected `0..=9` view. The general acyclic-exit repair must retain a language rejection for this recurrent state.
