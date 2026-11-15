# Byte window dispatch

Expected exit: 0.

Run `omega run --both samples/cli/probes/byte_window_dispatch/main.omg`.
A runtime machine value chooses the header or trailer decoder for the same
packet. Each decoder returns a borrowed byte window; the caller checks its
length and reads the selected byte. Rebinding the decoder local changes which
window is returned without changing the packet's storage.

The compiler does not parse callable types (`machine(...)` as a type) yet, so
the probe stops at line 12 before its returned-window checks
([OWNER_CODE_CONCERNS.md](../../../../OWNER_CODE_CONCERNS.md)).

The expected header byte is 11 and trailer byte is 29. Either a wrong slice,
an incorrect selector, or a lost backing reference reaches `crash Trap`.
