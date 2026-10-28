-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- import_named per site: the user import reused where its name means the import, imported again as `u_1` where a
-- block-scoped local, a catch clause's variable or a destructured local shadows it, where stock reuses the name, which
-- reads the local there. A deliberate difference (a stock bug fixed): expected/ is rbxts-rs's output, not stock's
local u_1 = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impa").u
local u = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impa").u
local top = u
local function block(flag)
	if flag then
		local u = 1
		return { u, u_1 }
	end
	return { u, u }
end
local function caught()
	local _exitType, _returns = TS.try(function()
		error("x")
	end, function(u)
		return TS.TRY_RETURN, { { u, u_1 } }
	end)
	if _exitType then
		return unpack(_returns)
	end
	return nil
end
local function destructured(o)
	local u = o.u
	return { u, u_1 }
end
return {
	block = block,
	caught = caught,
	destructured = destructured,
	top = top,
}
