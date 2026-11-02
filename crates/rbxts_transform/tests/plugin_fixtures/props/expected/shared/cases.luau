-- Compiled with roblox-ts v3.0.0
-- Every type the probes list, one alias each.
-- bases in two files: own members, then the inherited ones by (file, position). `First`'s members sit, in their own
-- file, at offsets inside this declaration, which TypeScript 7 takes for own members (`isDeclarationContainedBy`
-- compares no files): `f2, f1, own, s2, s1`
local _object = table.clone(objA)
setmetatable(_object, nil)
for _k, _v in objB do
	_object[_k] = _v
end
_object.last = 1
local spread = _object
local _a = big.a
local _rest = {}
for _key, _value in big do
	if _key ~= "a" then
		_rest[_key] = _value
	end
end
local rest = _rest
return nil
