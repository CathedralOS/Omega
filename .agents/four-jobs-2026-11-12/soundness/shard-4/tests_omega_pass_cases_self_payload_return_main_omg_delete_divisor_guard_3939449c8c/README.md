# self_payload_return

Two-case transition where BOTH arms return their own case payload: each case edge binds its payload and the arm returns it — one structural split.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
