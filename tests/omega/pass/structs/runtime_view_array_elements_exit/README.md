# View-bearing array elements

After the package review from `omega update --project .` and
`omega update --project . --resume`, run `omega run --both main.omg`.
Both engines must exit 70.

Two completed records carry different shared UTF-8 labels and scalar weights
into an array member. Reading both labels and their neighboring weights must
observe the values authored before the transfer, preserving descriptor backing,
field identity and the selected array element.
