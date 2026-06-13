import { useState, useMemo } from "react";
import { useStateObjects } from "../hooks/useStateObjects";
import { AnthropicTraceStateObject, AnthropicMessage } from "../types/api";

export function AnthropicTracesTab() {
    const [selectedTraceId, setSelectedTraceId] = useState<string | null>(null);

    // Subscribe to the index to get all state object names
    const indexObjectNames = useMemo(() => [""], []);
    const { objects: indexObjects, connected, error } = useStateObjects(indexObjectNames);

    // Extract trace IDs from the index
    const traceIds = useMemo(() => {
        const stateObjectNames = (indexObjects[0]?.value as string[] | undefined) || [];
        return stateObjectNames
            .filter((name) => name.startsWith("llm-trace-anthropic:"))
            .map((name) => {
                // Strip prefix and :persist suffix if present
                let id = name.replace("llm-trace-anthropic:", "");
                if (id.endsWith(":persist")) {
                    id = id.slice(0, -8); // Remove ":persist"
                }
                return id;
            });
    }, [indexObjects]);

    // Subscribe to all traces - memoize the array to prevent infinite loops
    const traceObjectNames = useMemo(
        () => traceIds.map((id) => `llm-trace-anthropic:${id}:persist`),
        [traceIds]
    );
    const { objects: traceObjects } = useStateObjects(traceObjectNames);

    // Parse trace data
    const traces = useMemo(() => {
        return traceObjects.map((obj) => {
            const data = obj.value as AnthropicTraceStateObject;
            let id = obj.name.replace("llm-trace-anthropic:", "");
            if (id.endsWith(":persist")) {
                id = id.slice(0, -8); // Remove ":persist"
            }
            return { id, ...data };
        });
    }, [traceObjects]);

    // Sort traces by start time (newest first)
    const sortedTraces = useMemo(() => {
        return [...traces].sort((a, b) => {
            return new Date(b.start_time).getTime() - new Date(a.start_time).getTime();
        });
    }, [traces]);

    const selectedTrace = sortedTraces.find((t) => t.id === selectedTraceId);

    return (
        <div className="h-full flex bg-gray-900 text-white">
            {/* Left sidebar - list of traces */}
            <div className="w-80 border-r border-gray-700 flex flex-col">
                <div className="p-4 border-b border-gray-700">
                    <h2 className="text-lg font-semibold">Traces</h2>
                    <div className="text-sm text-gray-400 mt-1">
                        {connected ? (
                            <span className="text-green-400">● Connected</span>
                        ) : (
                            <span className="text-red-400">● Disconnected</span>
                        )}
                        <span className="ml-2">({sortedTraces.length} traces)</span>
                    </div>
                    {error && (
                        <div className="text-sm text-red-400 mt-1">{error}</div>
                    )}
                </div>
                <div className="flex-1 overflow-y-auto">
                    {sortedTraces.map((trace) => (
                        <button
                            key={trace.id}
                            onClick={() => setSelectedTraceId(trace.id)}
                            className={`w-full text-left px-4 py-3 border-b border-gray-800 hover:bg-gray-800 transition-colors ${
                                selectedTraceId === trace.id ? "bg-gray-800 border-l-4 border-blue-500" : ""
                            }`}
                        >
                            <div className="flex items-center justify-between mb-1">
                                <span className="font-mono text-xs text-gray-400">
                                    {trace.id.slice(0, 8)}...
                                </span>
                                {!trace.end_time && (
                                    <span className="text-xs text-yellow-400">● Running</span>
                                )}
                            </div>
                            <div className="text-sm text-gray-300">
                                {new Date(trace.start_time).toLocaleString()}
                            </div>
                            <div className="text-xs text-gray-500 mt-1">
                                {trace.trace.model} • {trace.trace.messages.length} messages
                            </div>
                        </button>
                    ))}
                    {sortedTraces.length === 0 && (
                        <div className="p-4 text-center text-gray-400">
                            No traces found
                        </div>
                    )}
                </div>
            </div>

            {/* Right panel - selected trace details */}
            <div className="flex-1 flex flex-col overflow-hidden">
                {selectedTrace ? (
                    <>
                        <div className="p-4 border-b border-gray-700">
                            <div className="flex items-center justify-between">
                                <h3 className="text-lg font-semibold">Trace Details</h3>
                                {!selectedTrace.end_time && (
                                    <span className="text-sm text-yellow-400">● Running</span>
                                )}
                            </div>
                            <div className="mt-2 space-y-1 text-sm text-gray-400">
                                <div>
                                    <span className="font-semibold">ID:</span>{" "}
                                    <span className="font-mono">{selectedTrace.id}</span>
                                </div>
                                <div>
                                    <span className="font-semibold">Model:</span>{" "}
                                    {selectedTrace.trace.model}
                                </div>
                                <div>
                                    <span className="font-semibold">Started:</span>{" "}
                                    {new Date(selectedTrace.start_time).toLocaleString()}
                                </div>
                                {selectedTrace.end_time && (
                                    <div>
                                        <span className="font-semibold">Ended:</span>{" "}
                                        {new Date(selectedTrace.end_time).toLocaleString()}
                                    </div>
                                )}
                                <div>
                                    <span className="font-semibold">Max Tokens:</span>{" "}
                                    {selectedTrace.trace.max_tokens}
                                </div>
                            </div>
                        </div>
                        <div className="flex-1 overflow-auto p-4 space-y-4">
                            {/* Show system prompt if present */}
                            {selectedTrace.trace.system && (
                                <MessageView
                                    message={{
                                        role: "system",
                                        content: selectedTrace.trace.system,
                                    }}
                                />
                            )}
                            {/* Show regular messages */}
                            {selectedTrace.trace.messages.map((message, index) => (
                                <MessageView key={index} message={message} />
                            ))}
                        </div>
                    </>
                ) : (
                    <div className="flex items-center justify-center h-full text-gray-400">
                        Select a trace to view its details
                    </div>
                )}
            </div>
        </div>
    );
}

function MessageView({ message }: { message: AnthropicMessage }) {
    const isUser = message.role === "user";
    const isSystem = message.role === "system";
    
    // Extract text and thinking content
    let textContent = "";
    let thinkingContent = "";
    
    if (typeof message.content === "string") {
        textContent = message.content;
    } else {
        for (const block of message.content) {
            if (block.type === "text") {
                textContent += block.text;
            } else if (block.type === "thinking") {
                thinkingContent += block.thinking;
            }
        }
    }

    return (
        <div
            className={`rounded-lg p-4 ${
                isSystem ? "bg-gray-800/50" : isUser ? "bg-blue-900/30" : "bg-gray-800"
            }`}
        >
            <div className="flex items-center justify-between mb-2">
                <span
                    className={`font-semibold ${
                        isSystem ? "text-gray-400" : isUser ? "text-blue-400" : "text-green-400"
                    }`}
                >
                    {isSystem ? "System" : isUser ? "User" : "Assistant"}
                </span>
                {message.id && (
                    <span className="text-xs text-gray-500 font-mono">
                        {message.id}
                    </span>
                )}
            </div>

            {thinkingContent && (
                <div className="mb-3 p-3 bg-purple-900/20 border border-purple-800 rounded">
                    <div className="text-xs text-purple-400 font-semibold mb-1">
                        THINKING
                    </div>
                    <div className="text-sm text-gray-300 whitespace-pre-wrap">
                        {thinkingContent}
                    </div>
                </div>
            )}

            <div className="text-gray-100 whitespace-pre-wrap">
                {textContent}
            </div>

            {message.usage && (
                <div className="mt-3 pt-3 border-t border-gray-700 text-xs text-gray-400 space-y-1">
                    <div>
                        <span className="font-semibold">Tokens:</span>{" "}
                        {message.usage.input_tokens} in / {message.usage.output_tokens} out
                    </div>
                    {message.stop_reason && (
                        <div>
                            <span className="font-semibold">Stop reason:</span>{" "}
                            {message.stop_reason}
                        </div>
                    )}
                    {message.usage.cache_read_input_tokens !== undefined &&
                        message.usage.cache_read_input_tokens > 0 && (
                            <div>
                                <span className="font-semibold">Cache read:</span>{" "}
                                {message.usage.cache_read_input_tokens} tokens
                            </div>
                        )}
                </div>
            )}
        </div>
    );
}
