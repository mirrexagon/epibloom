import { useState, useMemo, useEffect, useRef } from "react";
import { useStateObjects } from "../hooks/useStateObjects";
import { AnthropicTraceStateObject } from "../types/api";
import { AnthropicTraceView } from "./AnthropicTraceView";

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
                        <span className="ml-2 text-neutral-500">
                            ({traceIds.length} traces)
                        </span>
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
                    <AnthropicTraceView
                        id={selectedTrace.id}
                        trace={selectedTrace}
                    />
                ) : (
                    <div className="flex items-center justify-center h-full text-base text-neutral-500">
                        Select a trace to view its details
                    </div>
                )}
            </div>
        </div>
    );
}
