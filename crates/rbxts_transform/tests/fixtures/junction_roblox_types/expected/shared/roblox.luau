-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- Roblox types through linked packages (tests/fixtures.rs, `junction_roblox_types`): `node_modules`, `@rbxts` or
-- each package linked from elsewhere compiles as a copied install. `expected/` is stock roblox-ts's output with a
-- real `node_modules`.
local Workspace = TS.import(script, game:GetService("ReplicatedStorage"), "include", "node_modules", "@rbxts", "services").Workspace
local function nothing()
end
-- possibly-undefined call arguments of Roblox functions keep their parentheses, which truncate a call that returns
-- nothing or several values to one value
local function voidArgument()
	return tonumber((nothing()))
end
local function callbackArgument(callback)
	return tostring((callback()))
end
-- Roblox builtins (a library, a Roblox method) stay stable lookups for the evaluation order
local function builtinLibrary(codes)
	local i = 0
	local same = function(a, b)
		return a == b
	end
	local _original = i
	i += 1
	local _exp = string.char(codes[_original + 1])
	local _original_1 = i
	i += 1
	return same(_exp, string.char(codes[_original_1 + 1]))
end
local function nativeReceiver(values)
	local part = Instance.new("Part")
	local _self = part
	local _arg0 = function()
		part = Instance.new("Part")
		return 7
	end
	-- ▼ ReadonlyArray.map ▼
	local _newValue = table.create(#values)
	for _k, _v in values do
		_newValue[_k] = _arg0(_v, _k - 1, values)
	end
	-- ▲ ReadonlyArray.map ▲
	_self:SetAttribute("value", _newValue[1])
	return part
end
local gravity = Workspace.Gravity
return {
	voidArgument = voidArgument,
	callbackArgument = callbackArgument,
	builtinLibrary = builtinLibrary,
	nativeReceiver = nativeReceiver,
	gravity = gravity,
}
