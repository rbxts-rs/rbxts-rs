-- Compiled with roblox-ts v3.0.0
-- A `default` that is not the last clause. roblox-ts lowered it as the end of the switch: every case after it was
-- dropped, so it ran the default where JavaScript runs a later case.
local function middle(x)
	local log = {}
	repeat
		local _fallthrough = false
		if x == 1 then
			table.insert(log, "one")
			_fallthrough = true
		end
		local _case
		if not _fallthrough then
			if x == 2 then
				_case = 1
			elseif x == 3 then
				_case = 2
			end
		end
		if _case == nil then
			table.insert(log, "default")
			_fallthrough = true
		end
		if _fallthrough or _case == 1 then
			table.insert(log, "two")
			break
		end
		if _case == 2 then
			table.insert(log, "three")
		end
	until true
	return log
end
local function first(x)
	local log = {}
	repeat
		local _fallthrough = false
		local _case
		if x == 1 then
			_case = 1
		elseif x == 2 then
			_case = 2
		end
		if _case == nil then
			table.insert(log, "default")
			break
		end
		if _case == 1 then
			_fallthrough = true
		end
		if _fallthrough or _case == 2 then
			table.insert(log, "one or two")
		end
	until true
	return log
end
-- later cases are tested in order, once each, and only until one matches
local function testedOnce(x)
	local log = {}
	local test = function(value)
		table.insert(log, `test {value}`)
		return value
	end
	local _exp = x
	repeat
		local _caseValue = test(1)
		if _exp == _caseValue then
			table.insert(log, "one")
			break
		end
		local _case
		local _caseValue_1 = test(2)
		if _exp == _caseValue_1 then
			_case = 1
		else
			table.insert(log, "prereq")
			local _ = #log
			if _exp == 3 then
				_case = 2
			end
		end
		if _case == nil then
			table.insert(log, "default")
			break
		end
		if _case == 1 then
			table.insert(log, "two")
			break
		end
		if _case == 2 then
			table.insert(log, "three")
		end
	until true
	return log
end
-- A variable a `default` in the middle declares is one local of the case block, shared with the cases after it, as
-- roblox-ts hoists a `case` clause's (`checkVariableHoist`); it was declared inside the default's `if`, so a later
-- case wrote a global. (A later case reached without the default meets the variable in its TDZ in JavaScript;
-- strict TypeScript lets it read the variable when its type includes `undefined`.)
local function sharedLet(x)
	local log = {}
	repeat
		local _fallthrough = false
		if x == 1 then
			table.insert(log, "one")
			break
		end
		local _case
		if x == 3 then
			_case = 1
		end
		local other
		if _case == nil then
			other = x * 10
			table.insert(log, `default {other}`)
			_fallthrough = true
		end
		if _fallthrough or _case == 1 then
			local _condition = other
			if _condition == nil then
				_condition = 0
			end
			other = _condition + 1
			table.insert(log, `three {other}`)
		end
	until true
	return log
end
local function sharedConst(x)
	local log = {}
	repeat
		local _fallthrough = false
		if x == 1 then
			table.insert(log, "one")
			_fallthrough = true
		end
		local _case
		if not _fallthrough then
			if x == 3 then
				_case = 1
			elseif x == 4 then
				_case = 2
			end
		end
		local doubled, later
		if _case == nil then
			doubled = x * 2
			table.insert(log, `default {doubled}`)
			_fallthrough = true
		end
		if _fallthrough or _case == 1 then
			local _condition = doubled
			if _condition == nil then
				_condition = 0
			end
			later = _condition + 1
			table.insert(log, `three {doubled} {later}`)
			break
		end
		if _case == 2 then
			table.insert(log, "four")
		end
	until true
	return log
end
local function sharedDestructuring(x)
	local log = {}
	repeat
		local _fallthrough = false
		if x == 1 then
			table.insert(log, "one")
			break
		end
		local _case
		if x == 3 then
			_case = 1
		end
		local first, second, size, name
		if _case == nil then
			local _binding = { x, x + 1 }
			first = _binding[1]
			second = _binding[2]
			local _binding_1 = {
				size = x * 3,
				name = `n{x}`,
			}
			size = _binding_1.size
			name = _binding_1.name
			_fallthrough = true
		end
		if _fallthrough or _case == 1 then
			local _condition = size
			if _condition == nil then
				_condition = 0
			end
			size = _condition + 1
			table.insert(log, `{first} {second} {size} {name}`)
		end
	until true
	return log
end
-- a variable the default alone uses stays inside its `if`
local function defaultOwn(x)
	local log = {}
	repeat
		local _case
		if x == 3 then
			_case = 1
		end
		if _case == nil then
			local own = x + 1
			table.insert(log, `default {own}`)
			break
		end
		if _case == 1 then
			local own3 = x + 3
			table.insert(log, `three {own3}`)
		end
	until true
	return log
end
return {
	middle = middle,
	first = first,
	testedOnce = testedOnce,
	sharedLet = sharedLet,
	sharedConst = sharedConst,
	sharedDestructuring = sharedDestructuring,
	defaultOwn = defaultOwn,
}
