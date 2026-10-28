-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- import_named reuses a user import whose name the file also writes as a property or a type, which shadows no value,
-- as Flamework's TS transformer does: a property access, an object literal key, a shorthand property, a class field,
-- a destructured property renamed, a local type alias and a type parameter
local u = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impa").u
local o = {
	u = 1,
}
local s = {
	u = u,
}
local C
do
	C = setmetatable({}, {
		__tostring = function()
			return "C"
		end,
	})
	C.__index = C
	function C.new(...)
		local self = setmetatable({}, C)
		return self:constructor(...) or self
	end
	function C:constructor()
		self.u = 2
	end
	function C:m()
		return { self.u, u }
	end
end
local function renamed()
	local z = o.u
	return { z, u }
end
local function typed(value)
	local n = value
	return { n, u }
end
local function aliased()
	local n = 1
	return { n, u }
end
local y = { o.u, s.u, u }
return {
	renamed = renamed,
	typed = typed,
	aliased = aliased,
	o = o,
	s = s,
	C = C,
	y = y,
}
