local table_util = {}

-- Checks whether t has the given value and returns its index if it does.
function table_util.find(t, value)
	for i, v in ipairs(t) do
		if v == value then
			return i
		end
	end

	return nil
end

return table_util
