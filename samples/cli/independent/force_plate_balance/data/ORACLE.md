Frozen before the first Omega check/run. All values are integers in N and mm.
F=sum(loads); Qx=sum(load*x); Qy=sum(load*y). Standard torque is My=-Qx, Mx=Qy;
reports use first moments Qx,Qy for unambiguous CoP coordinates.
CoP=(Qx/F,Qy/F); emit unreduced ratios, exact rational comparison. Tolerance=0.
Equilibrium residuals=(F-applied_F,Qx-applied_Qx,Qy-applied_Qy).
Known independent hand calculations: equal 100 N -> (400,0,0); right-heavy
(50,150,150,50) -> (400,40000,0); support corner (0,50,300,50) ->
(400,60000,30000), exactly (150,75), inclusive; (1,2,3,4) -> (10,0,400).
Invalid priority: negative, saturated, zero force, equilibrium, outside support.
Outside support is a valid reconstructed frame with an unsafe support decision.
No binary floating arithmetic, uncertainty estimate, or in-progress library.
