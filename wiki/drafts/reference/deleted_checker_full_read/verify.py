import os,re,sys
S='/tmp/claude-0/-home-user-Omega/1601736f-52e4-5ba1-a47f-e8466f18ac78/scratchpad/full'
rx=re.compile(r'READ\s+`?([^\s`]+)`?\s+(\d+)\s*[-–]\s*(\d+)')
full=[];part=[];missing=[];tot=0;cov=0
for i in range(1,138):
    sid=f'{i:03d}'
    spec=[l.rstrip('\n').split('\t') for l in open(f'{S}/slices/{sid}.txt') if not l.startswith('#') and l.strip()]
    need=sum(int(b)-int(a)+1 for _,a,b in spec); tot+=need
    rp=f'{S}/reports/{sid}.md'
    if not os.path.isfile(rp): missing.append(sid); continue
    text=open(rp,encoding='utf-8',errors='replace').read()
    reads={}
    for m in rx.finditer(text):
        reads.setdefault(os.path.basename(m.group(1)),[]).append((int(m.group(2)),int(m.group(3))))
    got=0; gaps=[]
    # files with same basename within a slice: fall back to pooled ranges per basename
    byname={}
    for p,a,b in spec: byname.setdefault(os.path.basename(p),[]).append((int(a),int(b)))
    for name,ranges in byname.items():
        rs=sorted(reads.get(name,[]))
        for a,b in ranges:
            covered=0; cur=a
            for x,y in rs:
                if y<cur or x>b: continue
                if x>cur: gaps.append((name,cur,x-1))
                cur=max(cur,min(y,b)+1)
            if cur<=b: gaps.append((name,cur,b))
            covered=(b-a+1)-sum(g[2]-g[1]+1 for g in gaps if g[0]==name and g[1]>=a and g[2]<=b)
            got+=max(0,covered)
    cov+=got
    done='## Surprises' in text and re.search(r'NOT READ', text)
    if got>=need and done: full.append(sid)
    else: part.append((sid,need,got,len(gaps),bool(done)))
print(f'lines assigned {tot}; lines logged as read {cov} ({100*cov/tot:.1f}%)')
print(f'fully logged and finished: {len(full)}; partial: {len(part)}; no report file: {len(missing)}')
print('partial:',' '.join(f'{s}({g}/{n}{"" if d else ",unfinished"})' for s,n,g,k,d in part))
print('missing:',' '.join(missing))
open(S+'/todo.txt','w').write(' '.join([p[0] for p in part]+missing))
