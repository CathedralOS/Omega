#!/usr/bin/env python3
"""Focused OMGCOMP1 fixtures for the OMGRSWC12 TokenStream relation."""

from __future__ import annotations

import argparse
import hashlib
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "compiler"))
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402

PACKAGE_KEY = "77" * 32
HEADER = struct.Struct("<8sHHHH44I")

CANONICAL = """module app;
data SourceId [copy] { value: u32 in Trapping; }
data Span [copy] { start: u64 in Trapping; end: u64 in Trapping; }
data SourceSpan [copy] { source: SourceId; span: Span; }
data NumericBase [copy] {
    case Binary; case Octal; case Decimal; case Hexadecimal;
}
data KeywordKind [copy] {
    case As; case CallingConvention; case Capability; case Contains; case Data;
    case Else; case Enum; case False; case Foreign; case Host; case If; case Let;
    case Library; case Loop; case Machine; case Match; case Owns; case Platform;
    case Pub; case Return; case SelfType; case SelfValue; case State; case Struct;
    case Target; case Transition; case True; case Use; case When; case While;
}
data PunctuationKind [copy] {
    case Ampersand; case AndAnd; case Apostrophe; case Arrow; case Asterisk;
    case Caret; case Colon; case ColonColon; case Comma; case Dot; case DotDot;
    case DotDotEqual; case Equal; case EqualEqual; case Exclamation;
    case ExclamationEqual; case Greater; case GreaterEqual; case GreaterGreater;
    case Hash; case LeftBrace; case LeftBracket; case LeftParen; case Less;
    case LessEqual; case LessLess; case Minus; case Percent; case Pipe; case PipePipe;
    case Plus; case PlusEqual; case MinusEqual; case AsteriskEqual; case SlashEqual;
    case PercentEqual; case RightBrace; case RightBracket; case RightParen;
    case Semicolon; case Slash; case Tilde;
}
data TokenKind [copy] {
    case Identifier;
    case Integer(base: NumericBase, empty_digits: bool, has_suffix: bool);
    case Float(has_exponent: bool, empty_exponent: bool, has_suffix: bool);
    case StringLiteral;
    case Keyword(keyword: KeywordKind);
    case Punctuation(punctuation: PunctuationKind);
    case Whitespace; case LineComment; case BlockComment;
}
data Token [copy] {
    kind: TokenKind;
    source_span: SourceSpan;
    decoded_start: u64 in Trapping;
    decoded_length: u64 in Trapping;
}
data LexDiagnosticCode [copy] {
    case None; case InvalidUtf8; case UnsupportedPunctuation;
    case UnterminatedBlockComment; case UnterminatedStringLiteral;
    case UnterminatedStringEscape; case UnsupportedEscape;
    case UnterminatedHexEscape; case InvalidHexEscapeDigit;
    case UnterminatedUnicodeEscape; case ExpectedUnicodeEscapeBrace;
    case EmptyUnicodeEscape; case InvalidUnicodeEscapeDigit;
    case InvalidUnicodeEscapeValue; case InvalidUnicodeScalar;
    case InvalidRawStringDelimiter; case UnterminatedRawString;
    case SourceCapacityExceeded; case TokenCapacityExceeded;
    case DecodedCapacityExceeded;
}
data LexDiagnostic [copy] { code: LexDiagnosticCode; source_span: SourceSpan; }
data TokenObservation [copy] {
    tag: u8; first: u8; second: u8; third: u8;
    source: u32 in Trapping;
    start: u64 in Trapping; end: u64 in Trapping;
    decoded_start: u64 in Trapping; decoded_length: u64 in Trapping;
}
data TokenStream {
    tokens: [Token; 16384] in Trapping;
    observations: [TokenObservation; 16384] in Trapping;
    token_count: u64 [0..=16384];
    decoded: [u8; 65536] in Trapping;
    decoded_length: u64 [0..=65536];
    diagnostic: LexDiagnostic;
    accepted: bool;
    last_retained: bool;
}
machine TokenStream::push(
    &mut self,
    source: SourceId,
    kind: TokenKind,
    start: u64 in Trapping,
    end: u64 in Trapping,
    decoded_start: u64 in Trapping,
    token_decoded_length: u64 in Trapping,
    observation_tag: u8,
    observation_first: u8,
    observation_second: u8,
    observation_third: u8
) {
    self.last_retained = false;
    transition self.token_count < 16384 { true -> retain() _ -> full() }
    state retain(&mut self) {
        self.tokens[self.token_count].kind = kind;
        self.tokens[self.token_count].source_span.source = source;
        self.tokens[self.token_count].source_span.span.start = start;
        self.tokens[self.token_count].source_span.span.end = end;
        self.tokens[self.token_count].decoded_start = decoded_start;
        self.tokens[self.token_count].decoded_length = token_decoded_length;
        self.observations[self.token_count].tag = observation_tag;
        self.observations[self.token_count].first = observation_first;
        self.observations[self.token_count].second = observation_second;
        self.observations[self.token_count].third = observation_third;
        self.observations[self.token_count].source = source.value;
        self.observations[self.token_count].start = start;
        self.observations[self.token_count].end = end;
        self.observations[self.token_count].decoded_start = decoded_start;
        self.observations[self.token_count].decoded_length = token_decoded_length;
        self.token_count = self.token_count + 1;
        self.last_retained = true;
    }
    state full(&mut self) { self.last_retained = false; }
}
machine TokenStream::read_kind(&self, index: u64 in Trapping) -> u8 {
    transition index < self.token_count { true -> present() _ -> absent() }
    state present(&self) {
        transition self.tokens[index].kind {
            TokenKind::Float { has_exponent, empty_exponent, has_suffix } -> exponent(has_exponent, empty_exponent, has_suffix)
            _ -> absent()
        }
    }
    state exponent(&self, has_exponent: bool, empty_exponent: bool, has_suffix: bool) {
        transition has_exponent { true -> empty(empty_exponent, has_suffix) _ -> absent() }
    }
    state empty(&self, empty_exponent: bool, has_suffix: bool) {
        transition empty_exponent { true -> absent() _ -> suffix(has_suffix) }
    }
    state suffix(&self, has_suffix: bool) {
        transition has_suffix { true -> retained_tag() _ -> absent() }
    }
    state retained_tag(&self) { self.observations[index].tag }
    state absent(&self) { 0 }
}
data Main { stream: TokenStream; }
machine Main::run(&mut self) -> u8 {
    self.stream.push(
        SourceId { value: 4 },
        TokenKind::Float { has_exponent: true, empty_exponent: false, has_suffix: true },
        5, 6, 7, 8, 70, 1, 2, 3
    );
    self.stream.read_kind(0)
}
"""


def encode(source: str, *, owner: str = "Main", machine: str = "run") -> bytes:
    packed = bundle.encode([bundle.Entry("tokens.omg", source.encode("ascii"))])
    manifest = {
        "target": "linux_x86_64",
        "packages": [{"key": PACKAGE_KEY,
                      "sources": [{"label": "tokens.omg", "module": "app"}]}],
        "aliases": [],
        "root": {"package": PACKAGE_KEY, "source": "tokens.omg",
                 "owner": owner, "machine": machine},
    }
    return compilation.encode_manifest(manifest, packed)


def renamed() -> str:
    result = CANONICAL
    replacements = {
        "TokenObservation": "Observation", "LexDiagnosticCode": "IssueCode",
        "PunctuationKind": "MarkKind", "KeywordKind": "WordKind",
        "NumericBase": "Radix", "TokenKind": "ItemKind",
        "LexDiagnostic": "Issue", "SourceSpan": "OriginSpan",
        "TokenStream": "Ledger", "SourceId": "OriginId", "Token": "Item",
        "Span": "Extent", "Main": "Driver", "read_kind": "inspect",
        "push": "retain", "run": "start_here", "tokens": "items",
        "observations": "notes", "token_count": "used",
        "last_retained": "kept", "decoded_length": "cooked_length",
        "decoded_start": "cooked_start", "diagnostic": "issue",
        "source_span": "origin_span", "source": "origin", "index": "position",
    }
    for old in sorted(replacements, key=len, reverse=True):
        result = result.replace(old, replacements[old])
    return result


def field_reordered() -> str:
    old = """    kind: TokenKind;
    source_span: SourceSpan;
    decoded_start: u64 in Trapping;
    decoded_length: u64 in Trapping;
"""
    new = """    decoded_length: u64 in Trapping;
    kind: TokenKind;
    decoded_start: u64 in Trapping;
    source_span: SourceSpan;
"""
    result = CANONICAL.replace(old, new)
    if result == CANONICAL:
        raise AssertionError("Token field reorder failed")
    return result


def declaration_reordered() -> str:
    # Move Main ahead of the two stream machines without changing any body.
    main = CANONICAL.index("data Main {")
    push = CANONICAL.index("machine TokenStream::push")
    return CANONICAL[:push] + CANONICAL[main:] + CANONICAL[push:main]


def matrix(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    positives = {
        "canonical": (CANONICAL, "Main", "run"),
        "renamed": (renamed(), "Driver", "start_here"),
        "field-reordered": (field_reordered(), "Main", "run"),
        "declaration-reordered": (declaration_reordered(), "Main", "run"),
        "commented-inert": (CANONICAL.replace("data Main { stream: TokenStream; }",
            "data Main { stream: TokenStream; /* irrelevant */ spare: bool; }"),
            "Main", "run"),
    }
    with (output / "positives.tsv").open("w", encoding="ascii") as manifest:
        for name, (source, owner, machine) in positives.items():
            path = output / f"{name}.omgc"
            path.write_bytes(encode(source, owner=owner, machine=machine))
            (output / f"{name}.omg").write_text(source, encoding="ascii")
            manifest.write(f"{name}\t{path}\n")

    negatives = {
        "missing-sum-store": CANONICAL.replace(
            "        self.tokens[self.token_count].kind = kind;\n", ""),
        "missing-source-store": CANONICAL.replace(
            "        self.tokens[self.token_count].source_span.source = source;\n", ""),
        "duplicate-observation-store": CANONICAL.replace(
            ".second = observation_second;", ".first = observation_second;"),
        "computed-index": CANONICAL.replace(
            "self.tokens[self.token_count].kind", "self.tokens[self.token_count + 0].kind"),
        "wrong-guard": CANONICAL.replace("self.token_count < 16384", "self.token_count < 16383", 1),
        "wrong-increment": CANONICAL.replace("self.token_count + 1", "self.token_count + 2"),
        "missing-array-policy": CANONICAL.replace(
            "[Token; 16384] in Trapping", "[Token; 16384]"),
        "wrong-float-payload": CANONICAL.replace(
            "has_exponent: true, empty_exponent: false, has_suffix: true",
            "has_exponent: true, empty_exponent: true, has_suffix: true", 1),
        "wrong-observation-tag": CANONICAL.replace("5, 6, 7, 8, 70, 1, 2, 3", "5, 6, 7, 8, 69, 1, 2, 3"),
        "array-too-large": CANONICAL.replace("[Token; 16384]", "[Token; 16385]", 1),
    }
    with (output / "negatives.tsv").open("w", encoding="ascii") as manifest:
        for name, source in negatives.items():
            path = output / f"{name}.omgc"
            path.write_bytes(encode(source))
            manifest.write(f"{name}\t251\t{path}\n")
        owner_fields = "\n".join(
            f"    reserve_{index}: [u8; 65536] in Trapping;"
            for index in range(7))
        owner_source = CANONICAL.replace(
            "    last_retained: bool;\n}",
            "    last_retained: bool;\n" + owner_fields + "\n}", 1)
        owner_exhausted = output / "owner-exhausted.omgc"
        owner_exhausted.write_bytes(encode(owner_source))
        manifest.write(f"owner-exhausted\t252\t{owner_exhausted}\n")
        exhausted = output / "input-exhausted.omgc"
        exhausted.write_bytes(b"\0" * 267282)
        manifest.write(f"input-exhausted\t252\t{exhausted}\n")


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    comp = encode(CANONICAL)
    (output / "canonical.omg").write_text(CANONICAL, encoding="ascii")
    (output / "canonical.omgc").write_bytes(comp)
    (output / "identity.txt").write_text(
        f"omgc_bytes={len(comp)}\nomgc_sha256={hashlib.sha256(comp).hexdigest()}\n",
        encoding="ascii")


def inspect(compilation_path: Path, witness_path: Path) -> None:
    comp = compilation_path.read_bytes(); witness = witness_path.read_bytes()
    if len(witness) != 7_520:
        raise ValueError(f"OMGRSWC12 length {len(witness)} != 7520")
    values = HEADER.unpack_from(witness)
    if values[:5] != (b"OMGRSWC\0", 12, 0, 0, 192):
        raise ValueError("wrong OMGRSWC12 identity")
    expected = (7520, len(comp), 1, 22, 8, 29, 5, 105, 8, 3, 11,
                14, 11, 2, 15, 20, 11, 2, 0, 1, 2, 3, 4, 5, 6, 7,
                0, 1, 2, 3, 4, 78, 0, 1, 16384, 65536, 1,
                1_638_456, 0, 2_097_152, 0, 0, 0, 0)
    if values[5:] != expected:
        raise ValueError("wrong OMGRSWC12 header semantics")
    decoded = compilation.decode(comp)
    if struct.unpack_from("<I", witness, 192)[0] != 0 or \
       struct.unpack_from("<I", witness, 204)[0] != len(decoded.bundle_entries[0].content):
        raise ValueError("wrong OMGRSWC12 unit extent")
    tables = ((212, 22, 32), (916, 8, 32), (1172, 29, 24),
              (1868, 5, 32), (2028, 105, 28), (4968, 8, 24),
              (5160, 3, 56), (5328, 11, 24), (5592, 14, 40),
              (6152, 11, 24), (6416, 2, 36), (6488, 15, 40),
              (7168, 11, 32))
    for start, count, width in tables:
        ids = [struct.unpack_from("<I", witness, start + row * width)[0]
               for row in range(count)]
        if ids != list(range(count)):
            raise ValueError(f"non-dense OMGRSWC12 table at {start}")
    if any(struct.unpack_from("<I", witness, 1868 + row * 32 + 28)[0] != 1
           for row in range(5)):
        raise ValueError("OMGRSWC12 pure-sum copy policy was not preserved")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    for command in ("build", "matrix"):
        item = sub.add_parser(command); item.add_argument("output", type=Path)
    item = sub.add_parser("inspect"); item.add_argument("compilation", type=Path); item.add_argument("witness", type=Path)
    args = parser.parse_args()
    if args.command == "build": build(args.output)
    elif args.command == "matrix": matrix(args.output)
    else: inspect(args.compilation, args.witness)


if __name__ == "__main__":
    main()
