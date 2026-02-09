local chat_id = ...

local chattree = require("chattree")

local chat = chattree.load(chat_id)
return chat.chat
