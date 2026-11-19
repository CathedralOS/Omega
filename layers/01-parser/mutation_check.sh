#!/usr/bin/env bash
# Breaks the parser one rule at a time and reports whether the test suite notices.
# Part of the layer gate: every mutation here must be caught.
#
#     bash layers/01-parser/mutation_check.sh        (from the repository root)
#
# Each mutation is one sed expression applied to one source file. The file is tested and then
# put back, so an interrupted run leaves at most one file changed; `git diff` shows which.
# "NOT APPLIED" means the source moved and the expression matched nothing: fix the expression.
set -uo pipefail

# A failing baseline would make every mutation appear caught. Always compare the committed goldens.
unset OMEGA_BLESS
cargo test -q -p omega-parser || exit 1

SRC=layers/01-parser/src
BACKUP=$(mktemp -d)

# name | file | sed expression
MUTATIONS=(
  "or binds tighter than and|grammar/expressions.rs|s/Punctuation::PipePipe => 1,/Punctuation::PipePipe => 3,/"
  "comparison binds tighter than shift|grammar/expressions.rs|s/Punctuation::Less | Punctuation::LessEqual | Punctuation::GreaterEqual => 4,/Punctuation::Less | Punctuation::LessEqual | Punctuation::GreaterEqual => 10,/"
  "membership binds like comparison|grammar/expressions.rs|s/binding_level: 5,/binding_level: 4,/"
  "operators group to the right|grammar/expressions.rs|s/binary_operator.binding_level + 1/binary_operator.binding_level/"
  "right shift allows a space|grammar/expressions.rs|s/self.nth_at(1, Punctuation::Greater) \&\& self.nth_touches_next(0)/self.nth_at(1, Punctuation::Greater)/"
  "cast entries are not read|grammar/expressions.rs|s/while self.at_cast_entry() {/while false {/"
  "record literal allowed everywhere|grammar/expressions.rs|s/context.get_allows_record_literal() \&\& self.at(Punctuation::LeftBrace)/self.at(Punctuation::LeftBrace)/"
  "membership allowed everywhere|grammar/expressions.rs|s/context.get_allows_membership() \&\& self.at_keyword(\"in\")/self.at_keyword(\"in\")/"
  "greater-than never stops|grammar/expressions.rs|s/if context.get_stops_at_greater() {/if false {/"
  "prefix words apply before anything|grammar/expressions.rs|s/        if self.nth_at_word(1) {$/        if true {/"
  "function arrow allowed after a cast|grammar/types.rs|s/type_position != TypePosition::Cast \&\& self.at(Punctuation::Arrow)/self.at(Punctuation::Arrow)/"
  "clauses inside angles read past the close|grammar/clauses.rs|s/let stops_at_greater = type_position == TypePosition::InsideAngles;/let stops_at_greater = false;/"
  "hypothesis is read as a plain requires|grammar/clauses.rs|s/if kind == NodeKind::Requires \&\& self.nth_at_keyword(1, \"machine\") \&\& self.nth_at_word(2) {/if false {/"
  "missing arm comma reported as a plain error|grammar/dispatch.rs|s/ParseErrorKind::MissingArmComma$/ParseErrorKind::UnexpectedToken/"
  "arms need no comma|grammar/dispatch.rs|s/if self.eat(Punctuation::Comma) {/if self.eat(Punctuation::Comma) || !self.at(Punctuation::RightBrace) {/"
  "every parenthesis starts a tuple pattern|grammar/dispatch.rs|s/self.at(Punctuation::LeftParenthesis) \&\& self.parenthesis_closes_a_pattern()/self.at(Punctuation::LeftParenthesis)/"
  "match is always a keyword|grammar/dispatch.rs|s/self.nth_can_start_expression(1) \&\& !self.nth_at_keyword(1, \"as\")/!self.nth_at_keyword(1, \"as\")/"
  "separator before a body is allowed|grammar/items.rs|s/if is_separator \&\& self.nth_at(1, Punctuation::LeftBrace) {/if false {/"
  "public is always a visibility|grammar/items.rs|s/^        !names_a_conformance$/        true/"
  "machine is always a declaration|grammar/items.rs|s/b\"machine\" if self.machine_name_follows(name_offset) => Some(ItemKind::Machine),/b\"machine\" => Some(ItemKind::Machine),/"
  "statements may follow a transition|grammar/statements.rs|s/if last_shape == StatementShape::Transition {/if false {/"
  "if is never reported|grammar/statements.rs|s/if is_if_statement {/if false {/"
  "let is always a statement|grammar/statements.rs|s/if self.at_keyword(\"let\") \&\& (self.nth_at_word(1) || self.nth_at(1, Punctuation::LeftParenthesis)) {/if self.at_keyword(\"let\") {/"
  "case is always a case|grammar/data.rs|s/if self.at_keyword(\"case\") \&\& self.nth_at_word(1) {/if self.at_keyword(\"case\") {/"
  "generic argument before a comma is a value|grammar/generics.rs|s/Some(Punctuation::Comma | Punctuation::Greater) => !is_inside_brackets,/Some(Punctuation::Comma | Punctuation::Greater) => false,/"
  "bracketed generic argument is never a type|grammar/generics.rs|s/Some(Punctuation::RightBracket | Punctuation::Semicolon) => is_inside_brackets,/Some(Punctuation::RightBracket | Punctuation::Semicolon) => false,/"
  "comparisons may chain|grammar/expressions.rs|s/if is_comparison && (left_side_is_comparison || operand_of == OperandOf::Comparison) {/if false {/"
  "a comparison may be the right side of another|grammar/expressions.rs|s/ || operand_of == OperandOf::Comparison) {/) {/"
  "every instruction needs a semicolon|grammar/assembly.rs|s/if !self.eat(Punctuation::Semicolon) {/self.expect(Punctuation::Semicolon, \"x\")?; if false {/"
  "nesting limit removed|parsing/syntax_parser.rs|s/const NESTING_LIMIT: u32 = 128;/const NESTING_LIMIT: u32 = 4000000;/"
  "a token is dropped from the tree|parsing/syntax_parser.rs|s/^        self.events.push(TreeEvent::Token);$/        if self.token_position % 97 != 96 { self.events.push(TreeEvent::Token); }/"
  "logical and starts a borrow|grammar/expressions.rs|s/Some(Punctuation::Ampersand) => {/Some(Punctuation::Ampersand | Punctuation::AmpersandAmpersand) => {/"
  "saturating left shift binds like xor|grammar/expressions.rs|s/| Punctuation::LessLessSaturating |/|/;s/Punctuation::Caret => 7,/Punctuation::LessLessSaturating | Punctuation::Caret => 7,/"
  "one extra nesting level allowed|parsing/syntax_parser.rs|s/const NESTING_LIMIT: u32 = 128;/const NESTING_LIMIT: u32 = 129;/"
  "fragment may leave trailing tokens|parsing/syntax_parser.rs|s/if !self.at_end() {/if false {/"
)

caught=0
missed=0

for mutation in "${MUTATIONS[@]}"; do
  IFS='|' read -r name file expression <<< "$mutation"
  cp "$SRC/$file" "$BACKUP/original.rs"
  sed -i "$expression" "$SRC/$file"

  if cmp -s "$SRC/$file" "$BACKUP/original.rs"; then
    echo "NOT APPLIED  $name  (the sed expression matched nothing in $file)"
    missed=$((missed + 1))
    continue
  fi

  # The linker sometimes cannot write its output because something else has the file open
  # (another build, or a virus scanner). That says nothing about the mutation, so try again.
  for attempt in 1 2 3; do
    cargo test -q -p omega-parser > "$BACKUP/out.txt" 2>&1 && test_status=0 || test_status=$?

    if ! grep -q "linking with" "$BACKUP/out.txt"; then
      break
    fi
  done

  if [ "$test_status" -eq 0 ]; then
    echo "MISSED       $name"
    missed=$((missed + 1))
  else
    if grep -q "could not compile" "$BACKUP/out.txt"; then
      # Not a verdict on the tests. Either the mutation is not valid Rust, or something else in
      # the workspace was mid-edit; the first error says which.
      echo "NO COMPILE   $name  ($(grep -m1 "^error" "$BACKUP/out.txt"))"
      missed=$((missed + 1))
    else
      echo "caught       $name"
      caught=$((caught + 1))
    fi
  fi

  cp "$BACKUP/original.rs" "$SRC/$file"
done

echo
echo "$caught caught, $missed missed or not applied, of ${#MUTATIONS[@]}"
rm -rf "$BACKUP"
(( missed == 0 ))
