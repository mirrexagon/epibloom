import { useMemo } from "react";
import { LlmTrace, LlmTraceIndexEntry } from "../types/api";
import { useStateObjects } from "./useStateObjects";

// ---------------------------------------------------------------------------
// useTrace — subscribe to a single llm-trace:<uuid> state object
// ---------------------------------------------------------------------------

interface UseTraceResult {
    trace: LlmTrace | null;
    error: string | null;
    connected: boolean;
}

const EMPTY_NAMES: string[] = [];

export function useTrace(traceId: string | null): UseTraceResult {
    const objectNames = useMemo(
        () => (traceId ? [`llm-trace:${traceId}`] : EMPTY_NAMES),
        [traceId],
    );

    const { objects, error, connected } = useStateObjects(objectNames);

    const trace = useMemo(() => {
        if (!traceId) return null;
        const obj = objects.find((o) => o.name === `llm-trace:${traceId}`);
        if (!obj || obj.value === null) return null;
        return obj.value as LlmTrace;
    }, [objects, traceId]);

    return { trace, error, connected };
}

// ---------------------------------------------------------------------------
// useTraceList — subscribe to the llm-trace-index state object
// ---------------------------------------------------------------------------

interface UseTraceListResult {
    traces: LlmTraceIndexEntry[];
    error: string | null;
    connected: boolean;
    latestTrace: LlmTraceIndexEntry | null;
}

const INDEX_NAMES = ["llm-trace-index"];

export function useTraceList(): UseTraceListResult {
    const { objects, error, connected } = useStateObjects(INDEX_NAMES);

    const traces = useMemo(() => {
        const obj = objects.find((o) => o.name === "llm-trace-index");
        if (!obj || !Array.isArray(obj.value)) return [];
        const entries = obj.value as LlmTraceIndexEntry[];
        // Sort newest-first by created timestamp.
        return [...entries].sort(
            (a, b) =>
                new Date(b.created).getTime() - new Date(a.created).getTime(),
        );
    }, [objects]);

    // The latest trace is the one with the most recent created timestamp.
    const latestTrace = traces.length > 0 ? traces[0] : null;

    return { traces, error, connected, latestTrace };
}
