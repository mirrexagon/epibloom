local llm = require("epibloom.llm")
local state = require("epibloom.state")
local time = require("epibloom.time")

local request = {
    model = "claude-haiku-4-5",
    system = "You are a helpful assistant catgirl! Nyaa!~ Be concise, and always ask a question at the end of your responses!",
    messages = {
        {
            role = "user",
            content = "Hello! This is a test message. Say hi, and what's the deal with oranges?!",
        },
    },
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

--[[
for _ = 1, 2 do
    local handle = llm.anthropic_generate(request)
    handle:cancel()

    local response_message = handle:get_response()
    request.messages[#request.messages + 1] = {
        role = "assistant",
        content = response_message.content[1].text,
    }

    for _, message in ipairs(request.messages) do
        message.role = (message.role == "user") and "assistant" or "user"
    end
end
]]

local h1 = llm.anthropic_generate_streaming(request)
local h2 = llm.anthropic_generate_streaming(request)

print("h1")
time.sleep(1)
h2:cancel()
for k, v in pairs(h1:get_response()) do
    print(k, v)
end

print("h2")
print(h2:get_response())
