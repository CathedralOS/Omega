# Carry an empty-slot check across helper calls

After checking that a pool slot is empty, the program passes the pool and index through helper calls and fills that same slot. The compiler must remember the check across those calls. This is a smaller witness for Cathedral's paging migration and retains all 8192 slots. The invalid twin removes only the guard.

The runtime checks distinct 47 and 59 payloads, an occupied-slot fallback of 83, a still-Empty neighboring slot and 91/97 surrounding canaries. Storing a Token makes the pool responsible for its cleanup. It supplies no physical-storage authority. Its drop hook changes only the dying Token, so this witness does not establish a cleanup count or order.
