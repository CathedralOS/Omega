# CKIR16 conservative x86-64 backend contract

The inherited checked-IR backend accepts CKIR16 without changing historical
schema interpretation. Kind-8 values use aligned eight-byte frame slots,
eight-byte object fields, eight-byte call/edge scratch cells and `RAX` as the
scalar return register.

The conservative unsigned comparison is one qword compare:

```text
48 8b 85 disp32       mov rax,[rbp-disp]
48 3b 85 disp32       cmp rax,[rbp-disp]
0f 92 c0              setb al
0f b6 c0              movzx eax,al
89 85 disp32          mov [rbp-disp],eax
```

An unsigned qword `CMP` observes both halves and is sufficient; a split
high/low `SETB` sequence would duplicate the processor’s carry semantics.

Every dynamic kind-8 destination interval is checked before publication. The
backend loads each bound into `R9` with `49 b9 imm64`, compares with
`4c 39 c8`, and branches to the inherited trap with `JB` for the lower bound
and `JA` for the upper bound. Store preserves the destination pointer in
`R10`, so the range helper deliberately uses `R9`. Call arguments and edge
arguments are staged as qwords before the callee/target performs its range
check; returns check the declared machine result before `leave; ret`.

The focused gates are `delta-checked-ir-v16-reference.sh` and
`delta-checked-ir-v16-backend.sh`. Their no-view positives cross both the low
and high halves, exercise field storage, calls, true-edge narrowing, return and
record-constructor custody, compare native and self-hosted backend artifacts,
and tooth the exact qword comparison template. The backend gate also runs the
CKIR15 reference and CKIR14 backend regression chains.
