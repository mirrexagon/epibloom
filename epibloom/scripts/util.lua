local util = {}

-- Prints a formatted string.
function util.printf(s, ...)
	print(string.format(s, ...))
end

-- Like printf, but throws an error with the formatted string
-- as the error message.
function util.errorf(s, ...)
	error(string.format(s, ...))
end

util.table = {}

-- Checks whether t has the given value and returns its index if it does.
function util.table.find(t, value)
	for i, v in ipairs(t) do
		if v == value then
			return i
		end
	end

	return nil
end

-- The predicate can return a second value, if it is not nil then that is used
-- as the new value.
function util.table.filter_map(t, predicate)
	local result = {}

	for _, v in ipairs(t) do
		local accepted, mapped_value = predicate(v)
		if accepted then
			if mapped_value == nil then
				mapped_value = v
			end
			table.insert(result, mapped_value)
		end
	end

	return result
end

return util
