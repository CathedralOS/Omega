"""Freeze independent NumPy pivoted complex solves and SciPy matrix exponentials.
Run before the first sample execution; validation only reads the frozen results.
"""
import hashlib,json,math
from pathlib import Path
import numpy as np
from scipy.linalg import expm
ROOT=Path(__file__).resolve().parent
DATA=ROOT/'data'

def freeze():
    cases=[]
    def valid(name,n,h,dt,steps,potential,amplitude,mode=None):
        text=f'{n} {h:.16f} {dt:.16f} {steps}\n'+''.join(f'{v:.16f} {z.real:.16f} {z.imag:.16f}\n' for v,z in zip(potential,amplitude))
        (DATA/f'{name}.stdin').write_text(text)
        # Reference uses exactly the decimal values frozen for the program.
        tokens=list(map(float,text.split()));h,dt=tokens[1:3]
        v=np.array(tokens[4::3]);psi=np.array(tokens[5::3])+1j*np.array(tokens[6::3])
        psi=psi/np.sqrt(h*np.vdot(psi,psi).real)
        H=np.diag(1/h**2+v)
        for j in range(n):H[j,(j+1)%n]=-0.5/h**2;H[j,(j-1)%n]=-0.5/h**2
        A=np.eye(n)+0.5j*dt*H;B=np.eye(n)-0.5j*dt*H
        exact_step=expm(-1j*dt*H)
        cn=psi.copy();exact=psi.copy();checkpoints=[]
        for k in range(steps+1):
            row={'step':k,'amplitude':[[float(z.real),float(z.imag)] for z in cn], 'norm':float(h*np.vdot(cn,cn).real),'energy':float(h*np.vdot(cn,H@cn).real),'probability':list(map(float,h*abs(cn)**2)),'exponential':[[float(z.real),float(z.imag)] for z in exact]}
            if mode is not None:
                eigenvalue=(1-math.cos(2*math.pi*mode/n))/h**2+v[0]
                analytic=psi*np.exp(-1j*k*dt*eigenvalue)
                analytic_cn=psi*((1-0.5j*dt*eigenvalue)/(1+0.5j*dt*eigenvalue))**k
                assert max(abs(analytic-exact))<1e-12 * max(1.,max(abs(psi)))
                assert max(abs(analytic_cn-cn))<1e-12 * max(1.,max(abs(psi)))
                row['analytic']=[[float(z.real),float(z.imag)] for z in analytic]
            checkpoints.append(row)
            cn=np.linalg.solve(A,B@cn);exact=exact_step@exact
        cases.append({'name':name,'exit':0,'cells':n,'spacing':h,'timestep':dt,'steps':steps,'checkpoints':checkpoints,'mode':mode})
    for dt,steps,label in [(.04,32,'coarse'),(.02,64,'half'),(.01,128,'quarter')]:
        valid('eigen_'+label,8,1.,dt,steps,[0.]*8,np.array([complex((-1)**j) for j in range(8)]),4)
    for n,steps in [(16,8),(32,2)]:
        psi=np.array([np.exp(-((j-n/3)/2)**2)*np.exp(.7j*j) for j in range(n)])
        valid(f'wavepacket_{n}',n,.5,.01,steps,[0.]*n,psi)
        barrier=[4. if n//2<=j<n//2+3 else 0. for j in range(n)]
        valid(f'barrier_{n}',n,.5,.01,steps,barrier,psi)
        if n==16:
            for dt,count,label in [(.005,16,'half'),(.0025,32,'quarter')]:
                valid('wavepacket_'+label,n,.5,dt,count,[0.]*n,psi)
                valid('barrier_'+label,n,.5,dt,count,barrier,psi)
    valid('zero_steps',8,1.,.01,0,[0.]*8,np.ones(8,dtype=complex))
    valid('max_steps',8,1.,.001,256,[-.25]*8,np.array([complex((-1)**j) for j in range(8)]),4)
    valid('wide_spacing_timestep',8,11.,2.,2,[.5]*8,np.ones(8,dtype=complex),0)
    valid('small_spacing',8,1e-8,1e-16,2,[0.]*8,np.ones(8,dtype=complex),0)
    valid('large_potential',8,1.,.0001,2,[1000000.]*8,np.ones(8,dtype=complex),0)
    base=(DATA/'zero_steps.stdin').read_text()
    def invalid(name,text,exit_code=2):
        (DATA/f'{name}.stdin').write_text(text);cases.append({'name':name,'exit':exit_code})
    invalid('empty','');invalid('short',base[:25]);invalid('extra',base+'1\n')
    for name,header in [('cells_low','7 1 .01 0'),('cells_high','33 1 .01 0'),('fractional_cells','8.5 1 .01 0'),('spacing_zero','8 0 .01 0'),('spacing_negative','8 -1 .01 0'),('timestep_zero','8 1 0 0'),('timestep_negative','8 1 -.01 0'),('steps_high','8 1 .01 257'),('steps_negative','8 1 .01 -1'),('fractional_steps','8 1 .01 1.5')]:invalid(name,header+'\n'+base.split('\n',1)[1])
    invalid('conditioning_extreme','8 0.00000001 1 1\n'+'0 1 0\n'*8,3)
    invalid('fractional_rounded','8.00000000000000000000001 1 .01 0\n'+base.split('\n',1)[1])
    invalid('zero_norm','8 1 .01 1\n'+'0 0 0\n'*8)
    for word in ['nan','inf','-inf','1e309']:
        invalid('nonfinite_'+word.replace('-','negative').replace('1e309','overflow'),base.replace('1.0000000000000000',word,1))
    invalid('potential_nan',base.replace('0.0000000000000000','nan',1))
    invalid('amplitude_inf',base.replace('0.0000000000000000 1.0000000000000000','0 inf',1))
    invalid('bytes',' '*4096+'\n');invalid('long_token','9'*25+'\n');invalid('double_decimal','8 1.2.3 .01 0\n')
    (DATA/'oracles.json').write_text(json.dumps({'references':['numpy.linalg.solve: pivoted dense complex LAPACK solve','scipy.linalg.expm: matrix exponential independent of CN'],'cases':cases},indent=2)+'\n')
    files=[ROOT/'reference.py',*sorted(DATA.glob('*.stdin')),DATA/'oracles.json']
    (DATA/'FROZEN.sha256').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(ROOT))+'\n' for p in files))
    print(f'Frozen {len(cases)} cases before sample execution')
if __name__=='__main__':freeze()
