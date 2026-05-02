--- LLM generation built-in module.
local record llm
    --- The role of a message author.
    enum Role
        "system"
        "user"
        "assistant"
    end

    --- A message in the conversation sent to the LLM.
    record Message
        role: Role
        content: string
        thinking: string | nil
    end

    --- Configuration for the Anthropic backend.
    --- Omitted fields use server-side defaults.
    record AnthropicConfig
        max_tokens: integer
        model: string
        stop_sequences: {string}
        thinking_budget: integer | nil
    end

    --- Configuration for the Featherless backend.
    --- Omitted fields use server-side defaults.
    record FeatherlessConfig
        max_tokens: integer
        model: string
        temperature: number | nil
        top_p: number | nil
        top_k: integer | nil
        min_p: number | nil
        stop_sequences: {string}
    end

    --- Which LLM backend to use for generation.
    enum BackendChoice
        "anthropic"
        "featherless"
    end

    enum GenerateChunkType
        "thinking"
        "message"
        "error"
    end

    --- A single chunk received during streaming LLM generation.
    --- `delta` is populated for "thinking" and "message" chunks.
    --- `message` is populated for "error" chunks.
    record GenerateChunk
        type: GenerateChunkType
        delta: string
        message: string
    end

    --- Handle to an in-progress LLM generation.
    --- Calling the handle returns the next GenerateChunk, or nil when done.
    --- Typical usage:
    ---   local chunk = handle()
    ---   while chunk ~= nil do ... chunk = handle() end
    record GenerateHandle
        --- The UUID of the trace created for this generation.
        trace_id: string
        metamethod __call: function(GenerateHandle): (GenerateChunk | nil)
        --- Block until generation completes, discarding any remaining chunks.
        --- Returns (success, error_message).
        wait: function(GenerateHandle): (boolean, string | nil)
    end

    --- Start LLM generation with the given messages and backend.
    --- config should be an AnthropicConfig or FeatherlessConfig matching the chosen
    --- backend; if omitted, server-side defaults are used.
    generate: function(messages: {Message}, backend: BackendChoice, config?: any): GenerateHandle
end

return llm
