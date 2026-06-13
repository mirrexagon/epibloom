local llm = require("epibloom.llm")
local log = require("epibloom.log")
local state = require("epibloom.state")
local uuid = require("epibloom.uuid")

local Chat = {}
chat.__index = Chat

function Chat.get(chat_id)
    local new_chat = {}

    if id then
        -- Check for existing chat.
    else
        -- Create new chat.
        local new_chat_id = uuid.new_v7()
    end

    return setmetatable(new_chat, Chat)
end

function Chat._get_state_object(chat_id)
    return state.get(string.format("chat:%s", chat_id))
end

function Chat._get_state_object(chat_id)
    return state.get(string.format("chat:%s", chat_id))
end

return Chat
