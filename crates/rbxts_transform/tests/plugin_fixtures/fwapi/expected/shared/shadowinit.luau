-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- import_named in an instance field initializer, which roblox-ts moves into the constructor, where a constructor
-- parameter (a parameter property and a destructured one too) shadows the reused import's name: TypeScript resolves
-- the initializer's name to the import, but stock's reused name reads the parameter there. rbxts-rs imports it again
-- as `u_1`. A deliberate difference (a stock bug fixed): expected/ is rbxts-rs's output, not stock's
local u_1 = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impa").u
local P
do
	P = setmetatable({}, {
		__tostring = function()
			return "P"
		end,
	})
	P.__index = P
	function P.new(...)
		local self = setmetatable({}, P)
		return self:constructor(...) or self
	end
	function P:constructor(u)
		self.x = u_1
		print(u)
	end
end
local Q
do
	Q = setmetatable({}, {
		__tostring = function()
			return "Q"
		end,
	})
	Q.__index = Q
	function Q.new(...)
		local self = setmetatable({}, Q)
		return self:constructor(...) or self
	end
	function Q:constructor(u)
		self.u = u
		self.x = u_1
	end
end
local R
do
	R = setmetatable({}, {
		__tostring = function()
			return "R"
		end,
	})
	R.__index = R
	function R.new(...)
		local self = setmetatable({}, R)
		return self:constructor(...) or self
	end
	function R:constructor(_param)
		local u = _param.u
		self.x = function()
			return u_1
		end
		print(u)
	end
end
return {
	P = P,
	Q = Q,
	R = R,
}
