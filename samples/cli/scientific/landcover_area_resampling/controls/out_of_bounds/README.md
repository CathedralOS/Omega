This is a complete intended-invalid Omega program. It writes class index 4 into
an array of length exactly 4. The documented collection rule requires
`0 <= index < live_length`; see
[Bounds and lowering](../../../../../../wiki/spec/language/counts_and_addresses.md#bounds-and-lowering).

The separately run check at base 95344cda99e5495abf785d38e7b78289ab02cbed exited 1
with `error: cannot prove index 4 is within length 4`. The control passed its
intended-rejection criterion. A parse, entry-binding or package refusal would
not satisfy that criterion. The diagnostic's index is rendered with backticks;
`expected.txt` pins the exact index and extent to distinguish this rule from unrelated rejection.
