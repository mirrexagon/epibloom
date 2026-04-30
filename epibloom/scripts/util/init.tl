local table_util = require("util.table")

local record util
    table: table_util
end

-- Prints a formatted string.
function util.printf(s: string, ...)
    return print(string.format(s, ...))
end

-- Like printf, but throws an error with the formatted string
-- as the error message.
function util.errorf(s: string, ...)
    return error(string.format(s, ...))
end

return util
