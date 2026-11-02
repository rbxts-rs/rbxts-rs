-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- imports of modules no program file imports: a package and a declaration file outside the source root
local createElement_1 = TS.import(script, game:GetService("ReplicatedStorage"), "include", "node_modules", "@rbxts", "roact", "src", "ts").createElement
local extra_1 = TS.import(script, game:GetService("ReplicatedStorage"), "lib", "extra").extra
local r = createElement_1
local x = extra_1
return {
	r = r,
	x = x,
}
