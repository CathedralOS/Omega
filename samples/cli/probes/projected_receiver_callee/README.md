# projected_receiver_callee

A ranked callee entered through a projected receiver: `Cursor::step(&mut engine.cursor, start)` borrows the `cursor` field as the callee's whole receiver; the callee's ranked backedge keeps that same projected referent as its subject, and the caller observes the write through `engine.cursor`. Interval return `u64 [0..=8]` spelled on both members.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
