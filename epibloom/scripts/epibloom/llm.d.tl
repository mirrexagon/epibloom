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

    record MessageTokensCount
        input_tokens: number
    end

    --- Start generation with the given messages.
    anthropic_generate: function(request: table): GenerateHandle

    --- Get a count of input tokens in a generate request.
    anthropic_count_tokens: function(request: table): MessageTokensCount | nil, string | nil
end

return llm
