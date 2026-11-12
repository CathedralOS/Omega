Linux x86-64/glibc mallinfo2 returns an 80-byte record. The first snapshot crosses owned helper and state returns; malloc changes the allocator before the second snapshot. After free, the first still retains its authored fields and the second is a distinct payload. This challenges indirect-result storage reuse with distinct payloads.

Each snapshot must satisfy the relations glibc's `mallinfo2` guarantees for every process history: used plus free space equals the arena, `usmblks` is zero, the top chunk is one ordinary block, and releasable, fast-free and mapped figures stay within their totals. The second must report at least the live 128 bytes more used space than the first. These hold on both engines, although the interpreter calls `mallinfo2` in its provider worker, whose heap differs from a fresh process.

Run `omega run --both tests/omega/pass/providers/foreign_indirect_record_distinct_exit/main.omg`; authored exit is 70 on both engines. No other-host qualification is claimed.
