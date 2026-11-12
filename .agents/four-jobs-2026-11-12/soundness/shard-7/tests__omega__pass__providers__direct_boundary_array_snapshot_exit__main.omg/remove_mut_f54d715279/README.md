# Direct boundary array snapshots

Complete the package review with `omega update --project .` and
`omega update --project . --resume`, then run `omega run --both main.omg`.
Both engines must exit 70.

An ordinary helper also forwards its owned array parameter to the same
trait requirement. `ThingTake::take` is a one-member boundary trait with an
empty reach. It receives an owned array between two scalar arguments. Its
provider observes the array before the final operand
mutates the caller's original buffer; the caller observes that later mutation.
The boundary declaration changes provider selection, not argument evaluation
order or snapshot ownership.
