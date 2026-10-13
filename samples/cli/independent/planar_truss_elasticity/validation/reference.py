"""Independent high-precision dense reference; never imports application code.

Element matrices use endpoint 4x4 scatter; free equations are solved with
Decimal partial pivoting at 70 digits. Analytic cases certify this oracle.
Running this file prints JSON; it never changes the frozen oracle.
"""
from decimal import Decimal as D, localcontext
import json

def reference(nodes, bars):
    with localcontext() as context:
        context.prec = 70
        size = 2*len(nodes)
        matrix = [[D(0) for _ in range(size)] for _ in range(size)]
        elements=[]
        for a,b,ea in bars:
            dx=D(nodes[b][0])-D(nodes[a][0]); dy=D(nodes[b][1])-D(nodes[a][1])
            length=(dx*dx+dy*dy).sqrt(); direction=[-dx/length,-dy/length,dx/length,dy/length]
            indices=[2*a,2*a+1,2*b,2*b+1]
            for i,row in enumerate(indices):
                for j,column in enumerate(indices): matrix[row][column]+=D(ea)/length*direction[i]*direction[j]
            elements.append((indices,direction,D(ea)/length))
        load=[D(node[4+axis]) for node in nodes for axis in range(2)]
        free=[2*i+axis for i,node in enumerate(nodes) for axis in range(2) if not node[2+axis]]
        augmented=[[matrix[i][j] for j in free]+[load[i]] for i in free]
        for pivot in range(len(free)):
            selected=max(range(pivot,len(free)),key=lambda i:abs(augmented[i][pivot]))
            augmented[pivot],augmented[selected]=augmented[selected],augmented[pivot]
            assert augmented[pivot][pivot] != 0
            for row in range(pivot+1,len(free)):
                factor=augmented[row][pivot]/augmented[pivot][pivot]
                for column in range(pivot,len(free)+1): augmented[row][column]-=factor*augmented[pivot][column]
        displacement=[D(0)]*size
        for row in reversed(range(len(free))):
            displacement[free[row]]=(augmented[row][-1]-sum(augmented[row][j]*displacement[free[j]] for j in range(row+1,len(free))))/augmented[row][row]
        internal=[sum(matrix[i][j]*displacement[j] for j in range(size)) for i in range(size)]
        reaction=[internal[i]-load[i] if i not in free else D(0) for i in range(size)]
        extensions=[sum(v*displacement[i] for i,v in zip(indices,direction)) for indices,direction,k in elements]
        forces=[k*extension for (_,_,k),extension in zip(elements,extensions)]
        energy=sum(force*extension/2 for force,extension in zip(forces,extensions))
        work=sum(a*b for a,b in zip(load,displacement))
        assert abs(2*energy-work)<D('1e-55')*max(D(1),abs(work))
        return {"displacement":[float(x) for x in displacement],"reaction":[float(x) for x in reaction],"extension":[float(x) for x in extensions],"force":[float(x) for x in forces],"energy":float(energy),"work":float(work)}

if __name__ == '__main__':
    print(json.dumps(reference([[0,0,1,1,0,0],[2,0,0,1,10,0]],[[0,1,100]]),indent=2))
