--- LLM built-in module.
local record llm
    --- Handle to an in-progress generation.
    --- Calling the handle returns the next streaming event, or nil when done.
    ---   local event = handle()
    ---   while event ~= nil do ... event = handle() end
    record GenerateHandle
        --- UUID of the llm-trace state object created for this generation.
        --- The full state object name is "llm-trace:<trace_id>".
        trace_id: string
        metamethod __call: function(GenerateHandle): table | nil
        --- Block until generation completes, discarding remaining events.
        --- Returns (success, error_message).
        wait: function(GenerateHandle): (boolean, string | nil)
    end

    --- Start generation with the given messages.
    --- `config` is optional; server-side defaults are used if omitted.
    generate: function(request: table): GenerateHandle
end

return llm
