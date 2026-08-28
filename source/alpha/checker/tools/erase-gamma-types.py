#!/usr/bin/env python3
# erase-gamma-types.py — checker-owned mechanical type eraser: typed Gamma -> the
# untyped Gamma that interp.beta runs (the surface of checker.gamma). This lets the
# SAME artifact that typeck.beta accepts be fed to the behavioral checker diamond, so
# "the checker is type-safe" and "the checker is correct" become claims about ONE file.
#
# The transformation is purely syntactic and total:
#   * drop every top-level (data ...) declaration (constructors are dynamic in interp)
#   * rewrite (def NAME ((p1 T1) (p2 T2) ...) RET body...) -> (def NAME (p1 p2 ...) body...)
#     i.e. strip each parameter's type and the single return-type token.
# Everything else (match / if / let / calls / constructor uses) is identical in both
# surfaces and passes through verbatim. Deterministic: no clocks, no randomness.
import sys

def tokenize(src):
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == ';':                       # line comment to end of line
            while i < n and src[i] != '\n':
                i += 1
        elif c in '()':
            out.append(c); i += 1
        elif c.isspace():
            i += 1
        else:                              # atom
            j = i
            while j < n and not src[j].isspace() and src[j] not in '();':
                j += 1
            out.append(src[i:j]); i = j
    return out

def parse(toks):
    pos = 0
    def node():
        nonlocal pos
        t = toks[pos]; pos += 1
        if t == '(':
            lst = []
            while toks[pos] != ')':
                lst.append(node())
            pos += 1                       # consume ')'
            return lst
        return t                           # atom
    forms = []
    while pos < len(toks):
        forms.append(node())
    return forms

def serialize(node):
    if isinstance(node, str):
        return node
    return '(' + ' '.join(serialize(x) for x in node) + ')'

def erase_def(form):
    # form = ['def', NAME, PARAMLIST, RET, body0, body1, ...]
    name = form[1]
    params = form[2]                       # [[p1, T1], [p2, T2], ...]
    bare = [p[0] if isinstance(p, list) else p for p in params]
    body = form[4:]                        # drop RET (form[3])
    return ['def', name, bare] + body

def main():
    forms = parse(tokenize(sys.stdin.read()))
    for form in forms:
        if isinstance(form, list) and form and form[0] == 'data':
            continue                       # erase data declarations
        if isinstance(form, list) and form and form[0] == 'def':
            form = erase_def(form)
        print(serialize(form))

if __name__ == '__main__':
    main()
