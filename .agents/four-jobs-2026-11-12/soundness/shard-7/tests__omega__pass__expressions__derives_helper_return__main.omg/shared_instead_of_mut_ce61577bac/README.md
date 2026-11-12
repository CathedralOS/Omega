# derives_helper_return

A helper machine DERIVES a narrower `File` authority from a
caller-provided `Folder` at the `Filesystem` boundary and returns it;
the outer `open_main_log` only forwards its folder, so the authority
flow propagates `derives` up the call graph
(`Worker::open_main_log derives via Worker::open_log`).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
