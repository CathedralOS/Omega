"""Regenerate independent pinned DFT fixtures; never called by validation."""
import json, math
from pathlib import Path
fixtures=[]
def add(name,n,length,window,overlap,values,fs=128,gain=1000,offset=0):
    data=[fs,gain,offset,n,overlap,window,length]+values
    x=[v*.001*gain*.001+offset*.001 for v in values]
    w=[1 if window==0 else .5-.5*math.cos(2*math.pi*j/n) for j in range(n)]
    spectra=[]; powers=[]
    for start in range(0,length-n+1,n-overlap):
        y=[x[start+j]*w[j] for j in range(n)]; energy=sum(v*v for v in w)
        p=[]
        for k in range(n//2+1):
            z=sum(y[j]*complex(math.cos(-2*math.pi*k*j/n),math.sin(-2*math.pi*k*j/n)) for j in range(n))
            p.append(abs(z)**2*(1 if k in (0,n//2) else 2)/(fs*energy))
        assert abs(sum(p)*fs/n-sum(v*v for v in y)/energy)<1e-10
        spectra.append(p);powers.append(sum(v*v for v in y)/energy)
    density=[sum(s[k] for s in spectra)/len(spectra) for k in range(n//2+1)]
    fixtures.append(dict(name=name,input=' '.join(map(str,data))+'\n',segments=len(spectra),frequency=[fs*k/n for k in range(n//2+1)],density=density,power=sum(powers)/len(powers)))
for n in [8,16,32,64]:
    add(f'tone{n}',n,n,0,0,[round(2000*math.cos(2*math.pi*2*j/n)) for j in range(n)])
add('hann_overlap',16,48,1,8,[((j*17+3)%31-15)*100 for j in range(48)],gain=1250,offset=-250)
add('sampling_scaled',8,8,0,0,[2000,0,-2000,0,2000,0,-2000,0],fs=256)
add('gain_scaled',8,8,0,0,[2000,0,-2000,0,2000,0,-2000,0],gain=2000)
add('offset_cancel',8,8,0,0,[2000]*8,gain=1500,offset=-3000)
add('dc',8,16,0,0,[2000]*16,gain=1500,offset=500)
add('nyquist',8,16,0,4,[1000 if j%2==0 else -1000 for j in range(16)])
add('maximum_overlap',8,12,1,7,[((j*7)%13-6)*100 for j in range(12)])
add('large_frequency',64,64,0,0,[1000]*64,fs=1000000)
add('maximum_calibration',64,64,0,0,[1000000]*64,fs=1,gain=10000,offset=1000000)
add('zero',8,8,1,0,[0]*8)
add('trailing',16,37,1,7,[((j*13)%23-11)*100 for j in range(37)],fs=100)
Path(__file__).with_name('fixtures.json').write_text(json.dumps(fixtures,indent=2)+'\n')
