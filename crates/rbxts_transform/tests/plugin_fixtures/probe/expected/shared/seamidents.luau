-- Compiled with roblox-ts v3.0.0
local TS = require(game:GetService("ReplicatedStorage"):WaitForChild("include"):WaitForChild("RuntimeLib"))
-- The names seam locals avoid are TypeScript 7.1's identifier set (`SourceFile::has_identifier`): JSDoc is not in it,
-- and every string literal is.
--[[
	`impl` only in JSDoc: {@link impl}
	@param impl a JSDoc parameter name
]]
local impl = TS.import(script, game:GetService("ReplicatedStorage"), "shared", "runtime").impl
local function documented(n)
	return n
end
-- so the import local is `impl`
local swapped = impl("b", 2)
-- a string literal anywhere takes its text: `constant_22_1`
local text = "constant_22"
local constant_22_1 = { 2 }
local named = constant_22_1
return {
	documented = documented,
	swapped = swapped,
	text = text,
	named = named,
}
