-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- import_named where a parameter shadows the reused import's name: rbxts-rs imports it again under a fresh name
-- (`u_1`), where stock reuses the name, which reads the parameter there. A deliberate difference (a stock bug fixed):
-- expected/ is rbxts-rs's output, not stock's
local u_1 = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impa").u
local function f(u)
	return { u, u_1 }
end
local g = function(u)
	return { u, u_1 }
end
local function h(_param)
	local u = _param.u
	return { u, u_1 }
end
return {
	f = f,
	h = h,
	g = g,
}
