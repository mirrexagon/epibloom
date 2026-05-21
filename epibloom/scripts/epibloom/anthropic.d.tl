--- Anthropic (Claude) LLM built-in module.
local record anthropic
    --- The role of a message author.
    enum Role
        "system"
        "user"
        "assistant"
    end

    --- Cache-control type string. Only "ephemeral" is supported.
    enum CacheControlType
        "ephemeral"
    end

    --- Cache-control breakpoint attached to a content block.
    record CacheControl
        type: CacheControlType
    end

    --- A single text content block within a message.
    --- Optionally carries a cache_control breakpoint.
    record TextBlock
        type: string
        text: string
        cache_control: CacheControl | nil
    end

    --- A message in the conversation sent to Claude.
    --- `content` may be a plain string (shorthand for a single text block with
    --- no cache_control) or an explicit list of TextBlocks.
    record Message
        role: Role
        content: string | {TextBlock}
        --- Thinking text for assistant turns that used extended thinking.
        thinking: string | nil
    end

    --- Per-call configuration. Omitted fields use server-side defaults.
    record Config
        max_tokens: integer
        model: string
        stop_sequences: {string}
        thinking_budget: integer | nil
    end

    enum ChunkType
        "thinking"
        "message"
        "error"
    end

    --- A single chunk received during streaming generation.
    --- `delta` is set for "thinking" and "message" chunks.
    --- `message` is set for "error" chunks.
    record GenerateChunk
        type: ChunkType
        delta: string
        message: string
    end

    --- Handle to an in-progress generation.
    --- Calling the handle returns the next GenerateChunk, or nil when done.
    ---   local chunk = handle()
    ---   while chunk ~= nil do ... chunk = handle() end
    record GenerateHandle
        --- UUID of the trace created for this generation.
        trace_id: string
        metamethod __call: function(GenerateHandle): GenerateChunk | nil
        --- Block until generation completes, discarding remaining chunks.
        --- Returns (success, error_message).
        wait: function(GenerateHandle): (boolean, string | nil)
    end

    --- Start generation with the given messages.
    --- `config` is optional; server-side defaults are used if omitted.
    generate: function(messages: {Message}, config?: Config): GenerateHandle
end

return anthropic
