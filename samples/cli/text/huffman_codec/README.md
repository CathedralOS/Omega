# Huffman codec

Encodes the fixed 16-byte input `"ABRACADABRAABAAA"` over the 5-symbol alphabet
`{'A', 'B', 'C', 'D', 'R'}` (`65`, `66`, `67`, `68`, `82`):

1. Counts symbol frequencies (`A=9`, `B=3`, `C=1`, `D=1`, `R=2`) after proving each
   input byte belongs to `u8::HuffmanSymbol`.
2. Constructs the 9-node Huffman tree bottom-up in a fixed `[TreeNode; 9]` array
   of mixed sum values (`TreeNode::Leaf { symbol }`,
   `TreeNode::Branch { left, right } where left < 9 && right < 9`, and
   `TreeNode::Empty`) by selecting the two lowest-weight active roots at each step.
3. Derives per-symbol code lengths (`[1, 2, 4, 4, 3]`) and prefix bit sequences by
   walking parent links from each leaf to the root (`nodes[8]`).
4. Packs the 16 symbols MSB-first into a 4-byte bitstream (`29` bits:
   `[138, 215, 139, 56]`).
5. Decodes the 29 bits back into a 16-byte output buffer by dispatching on
   `TreeNode::Branch` and `TreeNode::Leaf` from the root.
6. Self-verifies frequencies, code lengths, total bit count, packed bytes, and the
   16-byte decoded round-trip against hand-computed constants, then prints `PASS`.

`invalid/` is the companion intended-invalid control: attempting to look up and
emit `'Z'` (`90`), a symbol absent from `u8::HuffmanSymbol`, is rejected by
`omega --check` with `cannot prove requires contract for call lookup_symbol from Main::main: 90 in u8::HuffmanSymbol`.

After completing the sample's package review, run from the repository root:

```sh
omega run --both samples/cli/text/huffman_codec/main.omg < /dev/null
```
