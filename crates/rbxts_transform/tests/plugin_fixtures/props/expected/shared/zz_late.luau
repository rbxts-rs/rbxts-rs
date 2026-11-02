-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
local Base = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "decls").Base
local Mid
do
	local super = Base
	Mid = setmetatable({}, {
		__tostring = function()
			return "Mid"
		end,
		__index = super,
	})
	Mid.__index = Mid
	function Mid.new(...)
		local self = setmetatable({}, Mid)
		return self:constructor(...) or self
	end
	function Mid:constructor(...)
		super.constructor(self, ...)
		self.m = 1
		self.b1 = 3
	end
end
return {
	Mid = Mid,
}
