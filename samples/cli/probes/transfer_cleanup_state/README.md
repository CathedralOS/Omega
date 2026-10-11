# transfer_cleanup_state

A resource-owning value moves into a target state through a transition
argument: the source hands `g` to `hold(...)`, transferring the cleanup
obligation; the receiving state owns it and lets it die. Move-into-target
on a transition edge is implicit in the ownership lane.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
