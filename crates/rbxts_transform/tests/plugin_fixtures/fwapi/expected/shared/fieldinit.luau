-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- import_named in field initializers where no constructor parameter shadows the reused import's name: reused, as
-- Flamework's TS transformer does (a local of the constructor's body comes after the initializers roblox-ts moves
-- into the constructor, and a static initializer stays outside it)
local u = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "impa").u
local S
do
	S = setmetatable({}, {
		__tostring = function()
			return "S"
		end,
	})
	S.__index = S
	function S.new(...)
		local self = setmetatable({}, S)
		return self:constructor(...) or self
	end
	function S:constructor(v)
		self.x = u
		local u = v
		print(u)
	end
end
local T
do
	T = setmetatable({}, {
		__tostring = function()
			return "T"
		end,
	})
	T.__index = T
	function T.new(...)
		local self = setmetatable({}, T)
		return self:constructor(...) or self
	end
	function T:constructor()
		self.y = u
	end
end
local U
do
	U = setmetatable({}, {
		__tostring = function()
			return "U"
		end,
	})
	U.__index = U
	function U.new(...)
		local self = setmetatable({}, U)
		return self:constructor(...) or self
	end
	function U:constructor(u)
		print(u)
	end
	U.z = u
end
return {
	S = S,
	T = T,
	U = U,
}
