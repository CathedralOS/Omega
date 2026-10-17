# Reference arrival handoff

An exclusive reference moves into another local binding. Writing through the
receiving binding changes the original backing; the following read must see 41.
Expected exit: 0.

The compiler represents the handoff as an owned block arrival, then lends that
arrival's referent to the store. Its home belongs to the destination block:
lowering must not require the predecessor's operation identity to borrow it.

Run `omega run --both samples/cli/facts/reference_arrival_handoff/main.omg`.
