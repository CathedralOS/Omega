"""Freeze weighted Kabsch/SVD independently of the quaternion implementation."""
import hashlib,json
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[1]
def digest(data):return hashlib.sha256(data).hexdigest()
def reference(source,target,weights):
    center_source=np.average(source,axis=0,weights=weights);center_target=np.average(target,axis=0,weights=weights)
    covariance=((source-center_source)*weights[:,None]).T@(target-center_target)
    left,singular,right=np.linalg.svd(covariance)
    correction=np.eye(3);correction[2,2]=np.linalg.det(right.T@left.T)
    rotation=right.T@correction@left.T;translation=center_target-rotation@center_source
    transformed=source@rotation.T+translation
    residual=(transformed-target)*np.sqrt(weights[:,None])
    rms=float(np.sqrt(np.sum(residual**2)/np.sum(weights)))
    return dict(rotation=rotation.tolist(),translation=translation.tolist(),transformed=transformed.tolist(),residual=residual.tolist(),norm=np.linalg.norm(residual,axis=1).tolist(),rms=rms,singular=singular.tolist())
def main():
    destination=ROOT/'data/oracle.json';assert not destination.exists(),'refuse to overwrite frozen oracle'
    source=np.array([[0,0,0],[2,0,0],[0,3,0],[0,0,4],[1,2,3],[-2,1,-1]],dtype=float)
    rotation=np.array([[0,-1,0],[1,0,0],[0,0,1]],dtype=float);translation=np.array([3,-2,5.]);weights=np.array([1,2,3,4,1.5,.5])
    fixtures={}
    def add(name,s,t,w,exact=None,status=0):
        payload=str(len(s))+' '+ ' '.join(format(float(x),'.8f').rstrip('0').rstrip('.') or '0' for row in zip(s,t,w) for x in [*row[0],*row[1],row[2]])+'\n'
        path=ROOT/'data'/f'{name}.txt';path.write_text(payload)
        expected=reference(s,t,w) if status==0 else None
        if exact:
            assert np.max(np.abs(np.array(expected['rotation'])-exact[0]))<1e-12
            assert np.max(np.abs(np.array(expected['translation'])-exact[1]))<1e-12
            assert expected['rms']<1e-12
        fixtures[name]=dict(input_sha256=digest(payload.encode()),exit=status,expected=expected,constructed_exact=bool(exact))
    target=source@rotation.T+translation
    add('exact',source,target,weights,(rotation,translation))
    add('minimum_planar',source[:3],target[:3],weights[:3],(rotation,translation))
    planar=source.copy();planar[:,2]=2
    add('planar',planar,planar@rotation.T+translation,weights,(rotation,translation))
    noise=np.array([[.01,-.02,.03],[-.03,.01,0],[.02,.01,-.01],[.01,0,-.02],[-.02,.03,.01],[.02,-.01,.02]])
    add('noisy',source,target+noise,weights)
    reflection=np.diag([-1.,1.,1.]);add('reflected',source,source@reflection+translation,weights)
    order=[4,1,5,2,0,3];add('reordered',source[order],(target+noise)[order],weights[order])
    rng=np.random.default_rng(211)
    for index in range(6):
        s=np.round(rng.uniform(-10,10,(16,3)),6);q=rng.normal(size=4);q/=np.linalg.norm(q);w,x,y,z=q
        rot=np.array([[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w)],[2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w)],[2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]])
        t=np.round(s@rot.T+np.array([1.3,-2.7,.8])+rng.normal(0,.01,(16,3)),6);ww=np.round(rng.uniform(.2,5,16),6)
        add(f'random{index}',s,t,ww)
    line=np.array([[0.,0,0],[1,2,3],[2,4,6]]);add('collinear',line,line+translation,np.ones(3),status=2)
    same=np.zeros((3,3));add('coincident',same,same+translation,np.ones(3),status=2)
    # Identifiability failure in the objective even though the source has rank 3.
    add('target_coincident',source,np.tile(translation,(6,1)),weights,status=2)
    base=(ROOT/'data/exact.txt').read_text()
    tokens=base.split()
    invalid={'empty':'','count2':'2 '+ ' '.join(tokens[1:15]),'count17':'17 '+ ' '.join(tokens[1:]),'fractional_count':'3.5 '+ ' '.join(tokens[1:22]),'missing':' '.join(tokens[:-1]),'extra':base.strip()+' 1','nan':base.replace('0 ','nan ',1),'infinite':base.replace('0 ','inf ',1),'exponent':base.replace('0 ','1e999 ',1),'bound':base.replace('0 ','1001 ',1),'negative_weight':' '.join(tokens[:7]+['-1']+tokens[8:]),'zero_weight':' '.join(tokens[:7]+['0']+tokens[8:]),'digit_limit':'1234567890123','decimal_limit':'3.123456789','byte_limit':' '*2048,'bare_sign':'-','bare_dot':'.','bad_character':'3 x'}
    for name,payload in invalid.items():
        (ROOT/'data'/f'{name}.txt').write_text(payload);fixtures[name]=dict(input_sha256=digest(payload.encode()),exit=64,expected=None)
    destination.write_text(json.dumps(dict(method='numpy weighted Kabsch/SVD; independent exact transforms',numpy=np.__version__,transform_residual_tolerance=1e-7,orthogonality_determinant_tolerance=1e-8,cases=fixtures),indent=2)+'\n')
    print('frozen',len(fixtures),'cases',digest(destination.read_bytes()))
if __name__=='__main__':main()
