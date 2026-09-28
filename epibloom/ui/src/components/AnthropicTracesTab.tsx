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
        <div className="h-full flex">
            {/* Left sidebar - list of traces */}
            <div className="w-96 border-r border-neutral-800 flex flex-col">
                <div className="px-4 py-3 border-b border-neutral-800">
                    <h2 className="text-lg font-semibold">Traces</h2>
                    <div className="text-sm mt-1">
                        {connected ? (
                            <span className="text-green-400">● Connected</span>
                        ) : (
                            <span className="text-red-400">● Disconnected</span>
                        )}
                        <span className="ml-2 text-neutral-500">({traceIds.length} traces)</span>
                    </div>
                    {error && (
                        <div className="text-sm text-red-400 mt-1">{error}</div>
                    )}
                    <label className="mt-2 flex items-center cursor-pointer text-sm text-neutral-400">
                        <input
                            type="checkbox"
                            checked={autoSelectNew}
                            onChange={(e) => setAutoSelectNew(e.target.checked)}
                            className="mr-2 cursor-pointer accent-white"
                        />
                        Auto-select new traces
                    </label>
                </div>
                <div className="flex-1 overflow-y-auto">
                    {orderedTraceIds.map((id) => (
                        <button
                            key={id}
                            onClick={() => setSelectedTraceId(id)}
                            className={`w-full text-left px-4 py-2 font-mono text-sm break-all transition-colors ${
                                selectedTraceId === id
                                    ? "bg-neutral-900 text-white"
                                    : "text-neutral-400 hover:bg-neutral-950 hover:text-neutral-200"
                            }`}
                        >
                            {id}
                        </button>
                    ))}
                    {orderedTraceIds.length === 0 && (
                        <div className="p-4 text-center text-base text-neutral-500">
                            No traces found
                        </div>
                    )}
                </div>
            </div>

            {/* Right panel - selected trace details */}
            <div className="flex-1 flex flex-col overflow-hidden">
                {selectedTrace ? (
                    <>
                        <div className="px-4 py-3 border-b border-neutral-800">
                            <div className="flex items-center justify-between">
                                <h3 className="text-lg font-semibold font-mono break-all">
                                    {selectedTrace.id}
                                </h3>
                                {selectedTrace.error ? (
                                    <span className="text-sm text-red-400">
                                        Error
                                    </span>
                                ) : !selectedTrace.end_time ? (
                                    <span className="text-sm text-neutral-400 animate-pulse">
                                        Running…
                                    </span>
                                ) : null}
                            </div>
                            <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-0.5 text-sm">
                                <MetaRow label="Model" value={selectedTrace.request.model} />
                                <MetaRow
                                    label="Started"
                                    value={new Date(selectedTrace.start_time).toLocaleString()}
                                />
                                {selectedTrace.end_time && (
                                    <MetaRow
                                        label="Ended"
                                        value={new Date(selectedTrace.end_time).toLocaleString()}
                                    />
                                )}
                                <MetaRow
                                    label="Max tokens"
                                    value={selectedTrace.request.max_tokens}
                                />
                            </dl>
                            {selectedTrace.error && (
                                <div className="mt-3 border-l-2 border-red-400 pl-3 text-sm text-neutral-200 whitespace-pre-wrap font-mono">
                                    {selectedTrace.error}
                                </div>
                            )}
                        </div>
                        <div className="flex-1 overflow-auto p-4 space-y-6">
                            <section>
                                <SectionHeading>Request</SectionHeading>
                                <div className="space-y-3">
                                    {selectedTrace.request.system && (
                                        <MessageView
                                            message={{
                                                role: "system",
                                                content: selectedTrace.request.system,
                                            }}
                                        />
                                    )}
                                    {selectedTrace.request.messages.map(
                                        (message, index) => (
                                            <MessageView
                                                key={index}
                                                message={message}
                                            />
                                        ),
                                    )}
                                </div>
                            </section>

                            {selectedTrace.response && (
                                <section>
                                    <SectionHeading>Response</SectionHeading>
                                    <MessageView
                                        message={selectedTrace.response}
                                        highlight
                                    />
                                </section>
                            )}
                        </div>
                    </>
                ) : (
                    <div className="flex items-center justify-center h-full text-base text-neutral-500">
                        Select a trace to view its details
                    </div>
                )}
            </div>
        </div>
    );
}

function MetaRow({ label, value }: { label: string; value: React.ReactNode }) {
    return (
        <>
            <dt className="text-neutral-500">{label}</dt>
            <dd className="text-neutral-300">{value}</dd>
        </>
    );
}

function SectionHeading({ children }: { children: React.ReactNode }) {
    return (
        <h4 className="text-sm uppercase tracking-wider text-neutral-500 mb-2">
            {children}
        </h4>
    );
}

const ROLE_STYLES: Record<
    string,
    { label: string; labelClass: string; borderClass: string; highlightBorderClass: string }
> = {
    system: {
        label: "System",
        labelClass: "text-neutral-400",
        borderClass: "border-neutral-800",
        highlightBorderClass: "border-neutral-500",
    },
    user: {
        label: "User",
        labelClass: "text-blue-400",
        borderClass: "border-blue-900",
        highlightBorderClass: "border-blue-600",
    },
    assistant: {
        label: "Assistant",
        labelClass: "text-green-400",
        borderClass: "border-green-900",
        highlightBorderClass: "border-green-600",
    },
};

function MessageView({
    message,
    highlight = false,
}: {
    message: AnthropicMessage | AnthropicResponse;
    highlight?: boolean;
}) {
    // Extract text and thinking content
    let textContent = "";
    let thinkingContent = "";

    if (typeof message.content === "string") {
        textContent = message.content;
    } else {
        for (const block of message.content ?? []) {
            if (block.type === "text") {
                textContent += block.text;
            } else if (block.type === "thinking") {
                thinkingContent += block.thinking;
            }
        }
    }

    const isSystem = message.role === "system";
    const style = ROLE_STYLES[message.role] ?? ROLE_STYLES.system;

    return (
        <div
            className={`border p-3 ${
                highlight ? style.highlightBorderClass : style.borderClass
            }`}
        >
            <div className="flex items-baseline justify-between mb-2">
                <span
                    className={`text-sm font-semibold uppercase tracking-wider ${style.labelClass}`}
                >
                    {ROLE_STYLES[message.role]?.label ?? message.role}
                </span>
                {message.id && (
                    <span className="text-sm text-neutral-600 font-mono">
                        {message.id}
                    </span>
                )}
            </div>

            {thinkingContent && (
                <div className="mb-3 border-l-2 border-neutral-700 pl-3">
                    <div className="text-sm text-neutral-500 mb-1">Thinking</div>
                    <div className="text-base text-neutral-400 italic whitespace-pre-wrap">
                        {thinkingContent}
                    </div>
                </div>
            )}

            <div
                className={`text-base whitespace-pre-wrap ${
                    isSystem ? "text-neutral-400" : "text-neutral-100"
                }`}
            >
                {textContent}
            </div>

            {message.usage && (
                <div className="mt-3 pt-2 border-t border-neutral-800 text-sm text-neutral-500 flex flex-wrap gap-x-4">
                    <span>
                        {message.usage.input_tokens} in /{" "}
                        {message.usage.output_tokens} out
                    </span>
                    {message.usage.cache_read_input_tokens !== undefined &&
                        message.usage.cache_read_input_tokens > 0 && (
                            <span>
                                {message.usage.cache_read_input_tokens} cached
                            </span>
                        )}
                    {message.stop_reason && (
                        <span>stop: {message.stop_reason}</span>
                    )}
                </div>
            )}
        </div>
    );
}
