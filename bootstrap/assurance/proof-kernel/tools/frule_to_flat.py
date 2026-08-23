#!/usr/bin/env python3
# frule_to_flat.py — rewrite a checker.gamma Term from the (Frule …) WRAPPER form of
# user-function rules into the FLAT form that checker_typed.gamma uses, so the same
# user-function proofs can be cross-checked against the type-erased typed checker.
#
#   checker.gamma:        (Fapp arg (Frule ca ba) (Frule cb bb))   ; rules wrapped
#   checker_typed.gamma:  (Fapp arg ca ba cb bb)                   ; rules flattened
#
# Everything else (Fbundle / Apply / Con / Reccall / Par / Su / Pl / … and all the
# proof-term constructors) is identical in both surfaces and passes through verbatim.
# The rewrite is bottom-up so nested Fapp nodes (a rule body that calls another user
# function, e.g. mult whose step calls add) are flattened too. Deterministic; total.
import sys

def tokenize(src):
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if c in '()':
            out.append(c); i += 1
        elif c.isspace():
            i += 1
        else:
            j = i
            while j < n and not src[j].isspace() and src[j] not in '()':
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
            pos += 1
            return lst
        return t
    forms = []
    while pos < len(toks):
        forms.append(node())
    return forms

def rewrite(node):
    if isinstance(node, str):
        return node
    node = [rewrite(x) for x in node]          # bottom-up: inner Fapp flattened first
    if len(node) == 4 and node[0] == 'Fapp':
        ra, rb = node[2], node[3]
        if (isinstance(ra, list) and len(ra) == 3 and ra[0] == 'Frule'
                and isinstance(rb, list) and len(rb) == 3 and rb[0] == 'Frule'):
            return ['Fapp', node[1], ra[1], ra[2], rb[1], rb[2]]
    return node

def serialize(node):
    if isinstance(node, str):
        return node
    return '(' + ' '.join(serialize(x) for x in node) + ')'

def main():
    for form in parse(tokenize(sys.stdin.read())):
        print(serialize(rewrite(form)))

if __name__ == '__main__':
    main()
