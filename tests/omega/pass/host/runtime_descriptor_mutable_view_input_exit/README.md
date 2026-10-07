The compiler/corpus native and Terminal routes run this on macOS AArch64.
The public package route additionally needs reviewed acceptance:

```sh
cd tests/omega/pass/host/runtime_descriptor_mutable_view_input_exit
omega update
# Review pending decisions, then: omega update --resume
omega run --both main.omg < input.txt
```

The authored oracle is exit zero and `<abc` followed by LF, then `!` and LF.
The supplied input has four bytes, including LF. A forwarded call reads into array window `1..6`,
preserving its unread final byte and the array's outside prefix/suffix. Direct
empty and EOF calls preserve the same original backing and fixed extent.

`byte_view_calling.omg` explicitly places the view's pointer/count pair in
X1/X2; a bare safe slice does not declare a foreign ABI. Its types come from
the canonical std calling vocabulary, not copied compiler-owned definitions.
The policy selects the public `ByteViewCallingPolicy` map's exact reference
row; its body machine needs no separate satisfaction edge or naming convention.
Package preparation requests a full std review, including unused foreign
filesystem/process interfaces. Those decisions remain
unapproved in development validation. Without acceptance, the public command
refuses std's dependency-owned program-entry binding before execution; corpus
agreement is not evidence of accepted-package CLI execution.
