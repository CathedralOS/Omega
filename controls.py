from pathlib import Path
import shutil,subprocess,json,hashlib,sys,concurrent.futures
root=Path('build/symbolic-join/controls');root.mkdir(exist_ok=True)
source=Path('build/symbolic-join/probe/main.omg').read_text();common=source[:source.index('machine probe')]
pair='''data Pair { observed: u64; expected: u64; }
machine probe(left: u64, right: u64, select: bool) -> u64 {
    let mut items: [u64; 2] = [3, 1];
    let mut called: bool = false; let mut trace: u64 = 0;
    let pair: Pair = match select {
        true -> Pair { observed: observe(&mut items[0], left, &mut called, &mut trace, 11), expected: left }
        false -> Pair { observed: observe(&mut items[0], right, &mut called, &mut trace, 13), expected: right }
    };
    items[1] = 4;
    let result: u64 = same(pair.observed, pair.expected);
    let expected: u64 = match select { true -> left false -> right };
    let expected_trace: u64 = match select { true -> 11 false -> 13 };
    transition items[0] == expected && items[1] == 4 && called && trace == expected_trace {
        true -> result false -> 777
    }
}
machine main() crashes Trap {
    let first: u64 = probe(7, 9, true); let second: u64 = probe(7, 9, false);
    transition first == 7 && second == 9 { true -> done() false -> bad() }
    machine done() {} machine bad() crashes Trap { crash Trap; }
}
'''
reverse='''machine observe_reverse<'storage>(source: &'storage mut u64, value: u64,
    called: &mut bool, trace: &mut u64, event: u64) -> &'storage mut u64
ensures value == source { source = value; called = !called; trace = event; source }
'''
uncorrelated='''machine probe(left: u64, right: u64, first: bool, second: bool) -> u64 {
    let mut items: [u64; 2] = [3, 1]; let mut called: bool = false; let mut trace: u64 = 0;
    let saved: u64 = match first {
        true -> observe(&mut items[0], left, &mut called, &mut trace, 11)
        false -> observe(&mut items[0], right, &mut called, &mut trace, 13)
    };
    items[1] = 4;
    let expected: u64 = match second { true -> left false -> right };
    same(saved, expected)
}
machine main() { let result: u64 = probe(7, 9, true, false); }
'''
programs={'correlated_pair':common+pair,'correlated_pair_invalid':common+pair.replace('expected: right }','expected: left }'),'reverse_guarantee':common+reverse+source[source.index('machine probe'):].replace('false -> observe(', 'false -> observe_reverse('),'uncorrelated_choices':common+uncorrelated,'stale_expected':source.replace('    items[1] = 4;','    items[1] = 4;\n    let mut expected: u64 = value;\n    expected = 3;').replace('same(saved, value)','same(saved, expected)')}
programs.update({
'unreadable_delivery': """// Expected exit: 0
machine forward<'storage>(value: &'storage write u64) -> &'storage write u64 { value }
machine invalid(value: &write u64, select: bool) -> u64 {
    match select { true -> forward(value) false -> forward(value) }
}
machine main() {}
""",
'wrong_primitive_delivery': """// Expected exit: 0
machine forward<'storage>(value: &'storage bool) -> &'storage bool { value }
machine invalid(value: &bool, select: bool) -> u64 {
    match select { true -> forward(value) false -> forward(value) }
}
machine main() {}
""",
'wrong_reference_delivery': """// Expected exit: 0
machine forward<'storage>(value: &'storage mut bool) -> &'storage mut bool { value }
machine invalid<'storage>(value: &'storage mut bool, select: bool) -> &'storage mut u64 {
    match select { true -> forward(value) false -> forward(value) }
}
machine main() {}
"""})
if sys.argv[1]=='prepare':
 for name,text in programs.items():
  dest=root/name;dest.mkdir(exist_ok=True);(dest/'main.omg').write_text(text);shutil.copy2('samples/cli/facts/saved_observation_snapshot/local/build.omg',dest/'build.omg')
else:
 cli=str(Path(sys.argv[1]).resolve());label=sys.argv[2]
 def run(name):
  p=root/name/'main.omg';bad=name not in ['correlated_pair','reverse_guarantee'];command=[cli,'--check',str(p)] if bad else [cli,'run','--both',str(p)];r=subprocess.run(command,input='',capture_output=True,text=True,timeout=180);row=dict(name=name,invalid=bad,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),command=command,exit=r.returncode,stdout=r.stdout,stderr=r.stderr);print(json.dumps(row),flush=True);return row
 with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:rows=list(pool.map(run,programs))
 (root/('results-'+label+'.json')).write_text(json.dumps(rows,indent=2)+'\n')
