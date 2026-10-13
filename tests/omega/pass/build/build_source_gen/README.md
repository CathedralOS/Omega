# build_source_gen

Build-source round-trip: `BuildSource::resolve/open/read/close` pulls fixture bytes; `BuildOutput::create/write/include_source` stages generated.omg back as a compiled source (17th unit); LEARNING: generated sources parse under a restricted profile — `pub` rejected.

Compile-checked only: run a check target, not native.
