-- Compiled with roblox-ts v3.0.0
-- The declarations the probes use; `zz_late.ts` holds more, in a later file of the program.
local Key = {
	C = "c",
	A = "a",
	B = "b",
}
local Base
do
	Base = setmetatable({}, {
		__tostring = function()
			return "Base"
		end,
	})
	Base.__index = Base
	function Base.new(...)
		local self = setmetatable({}, Base)
		return self:constructor(...) or self
	end
	function Base:constructor()
		self.b2 = 1
		self.b1 = 2
	end
end
local function mkOne()
	return {
		o2 = 1,
		o1 = 2,
	}
end
local function mkTwo()
	return {
		o1 = 1,
		o2 = 2,
		o3 = 3,
	}
end
return {
	mkOne = mkOne,
	mkTwo = mkTwo,
	Key = Key,
	Base = Base,
}
