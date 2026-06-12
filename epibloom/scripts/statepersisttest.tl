local state = require("epibloom.state")
local llm = require("epibloom.llm")

local o = state.get("persist:absolutetest")

print(o:get())

o:update(function(_: any): any
    return math.random()
end)

local request = {
    model = "claude-haiku-4-5",
    messages = {
        { role = "user", content = "Hello! This is a test message. Say hi, and talk for a while about oranges!" }
    }
}

local function print_table(t, indent)
    indent = indent or 0

    local indent_string = string.rep("  ", indent)

    for k, v in pairs(t) do
        local indent = indent + 1
        if type(v) == "table" then
            print(string.format("%s%s = {", indent_string, k))
            print_table(v, indent)
        else
            print(string.format("%s%s = %s", indent_string, k, v))
        end
    end

    print(indent_string .. "}")
end

--print_table(assert(llm.anthropic_list_models()))
--print_table(assert(llm.anthropic_count_tokens(request)))

request.max_tokens = 1024

--print_table(assert(llm.anthropic_generate(request)))

for event in llm.anthropic_generate_streaming(request) do
    print("NEW EVENT")
    print_table(event)
end
