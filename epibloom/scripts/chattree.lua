local fs = require("epibloom.filesystem")
local log = require("epibloom.log")
local trace_store = require("epibloom.trace")
local uuid = require("epibloom.uuid")

local chattree = {}
chattree.__index = chattree

function chattree.new()
    local o = setmetatable({}, chattree)

    o.id = uuid.new()
    o.chat = {
        messages = {},
    }

    return o
end

function chattree.load(chat_id)
    local chat = fs.load_json(string.format("chats/%s", chat_id))
    return setmetatable({ id = chat_id, chat = chat }, chattree)
end

function chattree:set_message(message_position, message)
    assert(message_position <= (#self.chat.messages + 1), "tried to add message past the end of the message list")
    self.chat.messages[message_position] = message
end

function chattree:generate(...)
    local trace = trace_store:new()

    for i, message in ipairs(self.chat.messages) do
        trace[i] = message
    end
end

function chattree:save()
    fs.save_json(string.format("chats/%s", self.id), self.chat)
end

return chattree
