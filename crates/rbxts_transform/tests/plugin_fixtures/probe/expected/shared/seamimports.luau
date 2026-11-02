-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- plugin imports, as Flamework's TS transformer adds them: one declaration per path, the paths in the order their
-- first import was asked for, each listing its imports in that order. The string literals `"u"` and `"t"` take those
-- names (TypeScript 7.1's identifier set), so the locals are `u_1`, `t_1` and `t_2` in print order: `./impa`'s `t` is
-- `t_1` though `./impb`'s was asked for first
local _impa = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impa")
local u_1 = _impa.u
local t_1 = _impa.t
local t_2 = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impb").t
local i1 = u_1
local i2 = t_2
local i3 = t_1
local function later()
	return { t_2, u_1 }
end
return {
	later = later,
	i1 = i1,
	i2 = i2,
	i3 = i3,
}
