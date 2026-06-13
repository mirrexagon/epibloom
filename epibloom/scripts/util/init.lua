local table_util = require("util.table")

local util = {
    table = table_util,
}

-- Prints a formatted string.
function util.printf(s, ...)
    print(string.format(s, ...))
end

-- Like printf, but throws an error with the formatted string
-- as the error message.
function util.errorf(s, ...)
    error(string.format(s, ...))
end

return util
