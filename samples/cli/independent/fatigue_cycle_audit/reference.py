#!/usr/bin/env python3
"""Independent list/deletion reference; never supplies expected output files."""
from fractions import Fraction
import json
from pathlib import Path

def reference(values, exponent, scale):
    distinct = [value for index, value in enumerate(values) if index == 0 or value != values[index-1]]
    turning = [value for index, value in enumerate(distinct)
               if index == 0 or index == len(distinct)-1
               or (value-distinct[index-1])*(distinct[index+1]-value) < 0]
    # Walk the full reversal list from the beginning, deleting closed pairs;
    # restart at the previous enclosing quadruple after each deletion.
    cycles = []
    index = 0
    while index + 3 < len(turning):
        a,b,c,d = turning[index:index+4]
        if abs(b-c) <= min(abs(a-b),abs(c-d)):
            cycles.append((abs(b-c),2))
            del turning[index+1:index+3]
            index = max(0,index-2)
        else:
            index += 1
    cycles.extend((abs(a-b),1) for a,b in zip(turning,turning[1:]))
    return cycles, sum((Fraction(weight*extent**exponent,2*scale)
                        for extent,weight in cycles),Fraction())

if __name__ == '__main__':
    root=Path(__file__).resolve().parent
    count=0
    for case in json.loads((root/'data/cases.json').read_text()):
        if case['exit']:
            continue
        exponent,scale,length,*values=map(int,(root/'data'/f"{case['name']}.stdin").read_text().split())
        assert length == len(values)
        cycles,damage=reference(values,exponent,scale)
        assert cycles == [tuple(cycle) for cycle in case['cycles']],case['name']
        assert damage == Fraction(*case['damage']),case['name']
        count+=1
    print(f'{count} frozen hand oracles independently confirmed')
