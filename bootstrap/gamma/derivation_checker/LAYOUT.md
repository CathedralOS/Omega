# Inner layout admission

[Physical format and later semantic checks](FORMAT.md) | [Outer admission](REQUEST.md)

`admit_derivation_layout()` composes outer admission with physical traversal of
all three inner sections. It does not form a theory, resolve references, infer
sorts, validate proof rules, or compare the owner root. A physically decoded
layout is not evidence that any equality holds.

The outer outcome is returned unchanged on rejection or capacity refusal. On
outer success, traverse theory, proposition, and certificate in that order,
finishing one section before starting the next. Within a section:

1. Require its four magic bytes; a shorter section fails at its end. Compare
   magic bytes in order, selecting the first differing byte.
2. Require a whole number of four-byte payload words. A partial word fails at
   its first byte, before inspecting the preceding words' high bits.
3. Inspect every payload word's high byte in increasing offset order. A set
   high bit fails at that byte, before traversing grammar fields.
4. Traverse the format's records and fields, requiring each record and the
   section to end exactly. Unknown mode, term, and rule tags fail at their
   tag field. Surplus fields fail at the first unconsumed cursor.

Every coordinate is relative to the complete sealed request, including its
24-byte outer header. All inner physical failures use outer rejection tag 1
and code 6 `inner_layout`; limit and requested are zero. Earlier errors are
not replaced by later errors in another section.

## Extent checks

Before reading a fixed group of fields, require the group to fit its containing
extent; a short group fails at that containing end. Read a record length only
after checking its word fits. Compare its payload count with remaining bytes
divided by four before computing its end; an impossible extent fails at the
record's length field. Every nested end stays within its containing record.

For a table, check that its count is no greater than the remaining bytes divided
by four before entering the row loop. This lower bound accounts for one length
word per record, not a claim that every row's fields are valid. An impossible
count fails at the count field. Each iteration then consumes one complete,
checked record. Argument and premise vectors similarly check their counts
against the remaining whole-word extent before advancing; impossible vector
counts fail at their count field. Grammar traversal follows physical field
order after each such preflight.

The fixed groups, and therefore missing-field precedence, are explicit:

- A constructor/function signature first requires its result sort and argument
  count together, then its argument vector.
- A function next requires and checks the mode word **before** requiring its
  selected-argument word and then its clause table.
- A clause requires its constructor word, then its template table, then its
  body word. A proposition requires both root words together after its table.
- A term or proof record first requires and validates its tag word. Only a
  known tag permits checking its remaining fields. A variable then requires
  its slot word; an application requires symbol and argument count together,
  then the argument vector.
- A known proof rule next requires both conclusion references together.
  Reflexivity has no further fields; symmetry and unfolding require one word;
  transitivity requires both premises together; congruence requires its count
  word followed by the premise vector.
- Every other count, length, or sort-count field is a one-word group.

Thus a one-word term record with unknown tag 9 fails at the tag, not its end;
one with known application tag 1 fails at its end for the missing header.

The word scan checks even fields that later prove surplus or unreachable. No
record body, reference vector, or final proof prefix can hide a high-bit word.
Logical term depth never controls Gamma call depth: child references are
physical words here, not recursive requests to decode a tree.

## Private outcome and storage

On success, return tag 3 `Layout` with the same three section-end payload fields
as tag 0 `Framed`. This retains the original immutable input spans, not copied
term records or a semantic index. Only tag 3 permits a consumer to rely on
physical inner traversal; no production proof-accepting entry is supplied.

Scanning helpers use scalar cursors. A nonnegative result is the next cursor;
a negative result encodes the failure coordinate as `-(coordinate + 1)`.
Callers must branch on failure before reading, adding, or advancing. These
private scalars never become term identities or theory arithmetic.

The scans allocate no pairs and emit no bytes. The successful outer admission
allocates three pairs; layout success wraps its existing payload in one pair,
while a subsequent inner failure allocates four rejection pairs. Thus this
component uses at most seven pairs, without per-word or per-row allocation.
All row loops consume bounded input; nesting follows the fixed wire grammar,
not arbitrary input-provided recursion. The complete checker must separately
budget its future indexes, formation, comparisons, substitutions, and proof
state; this physical traversal does not establish that full profile.

## Deliberately deferred checks

Zero sort or reference values, empty proof/template tables, incorrect clause
cardinality, unbound slots, cyclic or forward references, wrong argument sorts,
and false rule conclusions can have a physical layout. They must fail the
later formation, [ground checking](GROUND.md), or derivation stage, and are not
accepted proofs here.
Unknown tags, illegal variable tags in ground records, wrong physical field
counts, and record escapes are already physical errors.

The diagnostic test entry must preserve this distinction, including positive
layout controls whose semantics are deliberately invalid. It reports exact
owned outcomes, not a compiler artifact or proof verdict. Outer evaluator
failures and host timeouts are not layout outcomes.
