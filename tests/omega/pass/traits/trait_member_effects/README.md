# trait_member_effects

Member-level effects inside a boundary trait: `machine emit(value) reaches Sink;` / `machine last() -> i32 reaches Sink;` declare per-requirement reach ceilings; provider satisfies via `Build::select_provider<Sink, SinkProvider>`; entry needs `invokes Sink;` on its own header line + `boundary machine` + `pub data Main` + freestanding build to call bindings.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
