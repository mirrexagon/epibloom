local state = require("epibloom.state")

local o = state.get("persist:absolutetest")

o:update(function(_: any): string
    return "test"
end)
