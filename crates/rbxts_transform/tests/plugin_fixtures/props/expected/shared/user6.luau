-- Compiled with roblox-ts v3.0.0
-- file 6 of 8: the same probes in another order (reversed, from `spread`), so each checker
-- meets the types in another order
local lists = {
	spread = "a1,b1,a2,last",
	readonly = "x,y,w,c",
	omit = "z,m,q",
	partial = "z,a,m,q",
	mappedRenamed = "ona,onb,onc",
	mappedKeys = "p,q,r",
	pick = "z,a,m",
	recordOfTemplate = "k1,k2",
	recordOfNumbers = "1,2,3",
	recordOfEnum = "a,b,c",
	recordOfKeys = "a,b,c",
	interfaceUnionReversed = "u2,u1",
	interfaceUnion = "u2,u1",
	literalUnion = "b,a",
	both = "f2,f1,own,s2,s1",
	chain = "c,x,y,w",
	returned = "o2,o1",
	symbols = "@s1,@s0,plain",
	indexFirst = "a,b",
	merged = "m0,m3,m2,m1",
	classes = "m,b1,b2",
	boxUnion = "v,k",
	box = "v,k",
	intersectionReversed = "s2,s1,f2,f1",
	intersection = "f2,f1,s2,s1",
	rest = "z,m,q",
}
return {
	lists = lists,
}
