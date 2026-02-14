local arg = ...

package.loaded["chat"] = nil
local chat = require("chat")

local dispatch = {
    list = function()
        return chat.list_chats()
    end,

    new = function()
        local chat = chat.new()
        chat:save()
        return chat.id
    end,

    get = function(arg)
        return chat.get(arg.chat_id)
    end,

    test = function(arg)
        chat.set_message(
            arg.chat_id,
            1,
            { role = "user", content = "What is your best guess at the energy content of a glass of orange juice?" }
        )

        print("generate time")
        chat.generate(arg.chat_id, "anthropic")
    end,
}

if not dispatch[arg.action] then
    error(string.format("Invalid action: '%s'", arg.action))
end

return dispatch[arg.action](arg)
