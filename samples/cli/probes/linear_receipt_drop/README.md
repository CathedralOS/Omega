# Linear Receipt Drop

The program issues a linear receipt with code 70 and reads it through a shared
borrow. Calling `drop(receipt)` transfers the receipt to the selected
`drop_receipt` machine, which consumes it by unpacking its code.

```sh
mbx run -p omega -- --check samples/cli/probes/linear_receipt_drop/main.omg
mbx run -p omega -- packages update --project samples/cli/probes/linear_receipt_drop
```
