-- Compiled with roblox-ts v3.0.0
-- file 7 of 8: the same probes in another order (from `indexFirst`), so each checker
-- meets the types in another order
local lists = {
	indexFirst = "a,b",
	symbols = "@s1,@s0,plain",
	returned = "o2,o1",
	chain = "c,x,y,w",
	both = "f2,f1,own,s2,s1",
	literalUnion = "b,a",
	interfaceUnion = "u2,u1",
	interfaceUnionReversed = "u2,u1",
	recordOfKeys = "a,b,c",
	recordOfEnum = "a,b,c",
	recordOfNumbers = "1,2,3",
	recordOfTemplate = "k1,k2",
	pick = "z,a,m",
	mappedKeys = "p,q,r",
	mappedRenamed = "ona,onb,onc",
	partial = "z,a,m,q",
	omit = "z,m,q",
	readonly = "x,y,w,c",
	spread = "a1,b1,a2,last",
	rest = "z,m,q",
	intersection = "f2,f1,s2,s1",
	intersectionReversed = "s2,s1,f2,f1",
	box = "v,k",
	boxUnion = "v,k",
	classes = "m,b1,b2",
	merged = "m0,m3,m2,m1",
}
return {
	lists = lists,
}
