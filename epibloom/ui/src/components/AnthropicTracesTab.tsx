import { useState, useMemo, useEffect, useRef } from "react";
import { useStateObjects } from "../hooks/useStateObjects";
import { AnthropicTraceStateObject, AnthropicMessage, AnthropicResponse } from "../types/api";

export function AnthropicTracesTab() {
    const [selectedTraceId, setSelectedTraceId] = useState<string | null>(null);
    const [autoSelectNew, setAutoSelectNew] = useState(true);
    const prevTraceIdsRef = useRef<string[]>([]);

    // Subscribe to the index to get all state object names
    const indexObjectNames = useMemo(() => [""], []);
    const {
        objects: indexObjects,
        connected,
        error,
    } = useStateObjects(indexObjectNames);

    // Extract trace IDs from the index (in index order, assumed append-order)
    const traceIds = useMemo(() => {
        const stateObjectNames =
            (indexObjects[0]?.value as string[] | undefined) || [];
        return stateObjectNames
            .filter((name) => name.startsWith("llm-trace-anthropic:"))
            .map((name) => {
                let id = name.replace("llm-trace-anthropic:", "");
                if (id.endsWith(":persist")) {
                    id = id.slice(0, -8);
                }
                return id;
            });
    }, [indexObjects]);

    // Reverse so the newest (last-appended) trace appears at the top of the sidebar
    const orderedTraceIds = useMemo(() => [...traceIds].reverse(), [traceIds]);

    // Subscribe to only the selected trace
    const selectedTraceObjectNames = useMemo(
        () =>
            selectedTraceId
                ? [`llm-trace-anthropic:${selectedTraceId}:persist`]
                : [],
        [selectedTraceId],
    );
    const { objects: selectedTraceObjects } = useStateObjects(
        selectedTraceObjectNames,
    );

    // Parse selected trace data
    const selectedTrace = useMemo(() => {
        if (!selectedTraceId || selectedTraceObjects.length === 0) return null;
        const obj = selectedTraceObjects[0];
        const data = obj.value as AnthropicTraceStateObject;
        return { id: selectedTraceId, ...data };
    }, [selectedTraceId, selectedTraceObjects]);

    // Auto-select new traces if enabled
    useEffect(() => {
        if (!autoSelectNew) return;

        const prevTraceIds = prevTraceIdsRef.current;
        const newTraceIds = traceIds.filter((id) => !prevTraceIds.includes(id));

        if (newTraceIds.length > 0) {
            // Select the most recently added new trace
            setSelectedTraceId(newTraceIds[newTraceIds.length - 1]);
        }

        prevTraceIdsRef.current = [...traceIds];
    }, [traceIds, autoSelectNew]);

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
                        <span className="ml-2">
                            ({traceIds.length} traces)
                        </span>
                    </div>
                    {error && (
                        <div className="text-sm text-red-400 mt-1">{error}</div>
                    )}
                    <div className="mt-3">
                        <label className="flex items-center cursor-pointer text-sm">
                            <input
                                type="checkbox"
                                checked={autoSelectNew}
                                onChange={(e) =>
                                    setAutoSelectNew(e.target.checked)
                                }
                                className="mr-2 cursor-pointer"
                            />
                            <span className="text-gray-300">
                                Auto-select new traces
                            </span>
                        </label>
                    </div>
                </div>
                <div className="flex-1 overflow-y-auto">
                    {orderedTraceIds.map((id) => (
                        <button
                            key={id}
                            onClick={() => setSelectedTraceId(id)}
                            className={`w-full text-left px-4 py-3 border-b border-gray-800 hover:bg-gray-800 transition-colors ${
                                selectedTraceId === id
                                    ? "bg-gray-800 border-l-4 border-blue-500"
                                    : ""
                            }`}
                        >
                            <span className="font-mono text-xs text-gray-400 break-all">
                                {id}
                            </span>
                        </button>
                    ))}
                    {orderedTraceIds.length === 0 && (
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
                                <h3 className="text-lg font-semibold">
                                    Trace Details
                                </h3>
                                {selectedTrace.error ? (
                                    <span className="text-sm text-red-400">
                                        ● Error
                                    </span>
                                ) : !selectedTrace.end_time ? (
                                    <span className="text-sm text-yellow-400">
                                        ● Running
                                    </span>
                                ) : null}
                            </div>
                            <div className="mt-2 space-y-1 text-sm text-gray-400">
                                <div>
                                    <span className="font-semibold">ID:</span>{" "}
                                    <span className="font-mono">
                                        {selectedTrace.id}
                                    </span>
                                </div>
                                <div>
                                    <span className="font-semibold">
                                        Model:
                                    </span>{" "}
                                    {selectedTrace.request.model}
                                </div>
                                <div>
                                    <span className="font-semibold">
                                        Started:
                                    </span>{" "}
                                    {new Date(
                                        selectedTrace.start_time,
                                    ).toLocaleString()}
                                </div>
                                {selectedTrace.end_time && (
                                    <div>
                                        <span className="font-semibold">
                                            Ended:
                                        </span>{" "}
                                        {new Date(
                                            selectedTrace.end_time,
                                        ).toLocaleString()}
                                    </div>
                                )}
                                <div>
                                    <span className="font-semibold">
                                        Max Tokens:
                                    </span>{" "}
                                    {selectedTrace.request.max_tokens}
                                </div>
                            </div>
                            {selectedTrace.error && (
                                <div className="mt-3 p-3 bg-red-900/20 border border-red-700 rounded">
                                    <div className="text-sm font-semibold text-red-400 mb-1">
                                        Error
                                    </div>
                                    <div className="text-sm text-gray-300 whitespace-pre-wrap font-mono">
                                        {selectedTrace.error}
                                    </div>
                                </div>
                            )}
                        </div>
                        <div className="flex-1 overflow-auto p-4 space-y-4">
                            {/* Request Section */}
                            <div>
                                <h4 className="text-sm font-semibold text-gray-400 mb-3">Request</h4>
                                <div className="space-y-4">
                                    {/* Show system prompt if present */}
                                    {selectedTrace.request.system && (
                                        <MessageView
                                            message={{
                                                role: "system",
                                                content: selectedTrace.request.system,
                                            }}
                                        />
                                    )}
                                    {/* Show regular messages */}
                                    {selectedTrace.request.messages.map(
                                        (message, index) => (
                                            <MessageView
                                                key={index}
                                                message={message}
                                            />
                                        ),
                                    )}
                                </div>
                            </div>

                            {/* Response Section */}
                            {selectedTrace.response && (
                                <div className="pt-4 border-t border-gray-700">
                                    <h4 className="text-sm font-semibold text-gray-400 mb-3">Response</h4>
                                    <div className="space-y-4">
                                        <ResponseView response={selectedTrace.response} />
                                    </div>
                                </div>
                            )}
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
                isSystem
                    ? "bg-gray-800/50"
                    : isUser
                      ? "bg-blue-900/30"
                      : "bg-gray-800"
            }`}
        >
            <div className="flex items-center justify-between mb-2">
                <span
                    className={`font-semibold ${
                        isSystem
                            ? "text-gray-400"
                            : isUser
                              ? "text-blue-400"
                              : "text-green-400"
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
                        {message.usage.input_tokens} in /{" "}
                        {message.usage.output_tokens} out
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
                                <span className="font-semibold">
                                    Cache read:
                                </span>{" "}
                                {message.usage.cache_read_input_tokens} tokens
                            </div>
                        )}
                </div>
            )}
        </div>
    );
}

function ResponseView({ response }: { response: AnthropicResponse }) {
    // Extract text and thinking content from response
    let textContent = "";
    let thinkingContent = "";

    if (response.content) {
        for (const block of response.content) {
            if (block.type === "text") {
                textContent += block.text;
            } else if (block.type === "thinking") {
                thinkingContent += block.thinking;
            }
        }
    }

    return (
        <div className="rounded-lg p-4 bg-green-900/20 border border-green-900/30">
            <div className="flex items-center justify-between mb-2">
                <span className="font-semibold text-green-400">
                    Assistant
                </span>
                {response.id && (
                    <span className="text-xs text-gray-500 font-mono">
                        {response.id}
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

            {response.usage && (
                <div className="mt-3 pt-3 border-t border-gray-700 text-xs text-gray-400 space-y-1">
                    <div>
                        <span className="font-semibold">Tokens:</span>{" "}
                        {response.usage.input_tokens} in /{" "}
                        {response.usage.output_tokens} out
                    </div>
                    {response.stop_reason && (
                        <div>
                            <span className="font-semibold">Stop reason:</span>{" "}
                            {response.stop_reason}
                        </div>
                    )}
                    {response.usage.cache_read_input_tokens !== undefined &&
                        response.usage.cache_read_input_tokens > 0 && (
                            <div>
                                <span className="font-semibold">
                                    Cache read:
                                </span>{" "}
                                {response.usage.cache_read_input_tokens} tokens
                            </div>
                        )}
                </div>
            )}
        </div>
    );
}
