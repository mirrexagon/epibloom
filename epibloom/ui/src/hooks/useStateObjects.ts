import { useEffect, useRef, useState } from "react";
import { StateEvent, StateObjectData } from "../types/api";
import { applyPatch, Operation } from "fast-json-patch";
import { produce } from "immer";

// How long (ms) to accumulate patches before flushing them to React state.
// This caps the re-render rate at ~20/s regardless of LLM streaming speed.
const PATCH_BATCH_INTERVAL_MS = 50;

interface UseStateObjectsResult {
    objects: StateObjectData[];
    error: string | null;
    connected: boolean;
}

export function useStateObjects(objectNames: string[]): UseStateObjectsResult {
    const [objects, setObjects] = useState<StateObjectData[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [connected, setConnected] = useState(false);

    // Accumulated patches waiting to be applied, keyed by object name.
    const pendingPatchesRef = useRef<Map<string, Operation[]>>(new Map());
    const flushTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    // Schedule a flush if one isn't already pending.
    const scheduleFlush = (flushFn: () => void) => {
        if (flushTimerRef.current === null) {
            flushTimerRef.current = setTimeout(() => {
                flushTimerRef.current = null;
                flushFn();
            }, PATCH_BATCH_INTERVAL_MS);
        }
    };

    useEffect(() => {
        if (objectNames.length === 0) {
            setObjects([]);
            setConnected(false);
            return;
        }

        // Clear objects that are no longer being watched
        setObjects((prev) => prev.filter((obj) => objectNames.includes(obj.name)));

        const objectsParam = objectNames.join(",");
        const eventSource = new EventSource(
            `/api/state/?objects=${encodeURIComponent(objectsParam)}`,
        );

        eventSource.onopen = () => {
            setConnected(true);
            setError(null);
        };

        eventSource.onmessage = (event) => {
            try {
                const stateEvent: StateEvent = JSON.parse(event.data);

                if (stateEvent.type === "value") {
                    // Flush any pending patches for this object before
                    // replacing its value entirely, so they don't get applied
                    // on top of a stale document later.
                    pendingPatchesRef.current.delete(stateEvent.object);

                    // Initial value or full update — apply immediately.
                    setObjects((prev) => {
                        const existing = prev.find(
                            (obj) => obj.name === stateEvent.object,
                        );
                        if (existing) {
                            // Update existing
                            return prev.map((obj) =>
                                obj.name === stateEvent.object
                                    ? { ...obj, value: stateEvent.value }
                                    : obj,
                            );
                        } else {
                            // Add new
                            return [
                                ...prev,
                                {
                                    name: stateEvent.object,
                                    value: stateEvent.value,
                                },
                            ];
                        }
                    });
                } else if (stateEvent.type === "patch") {
                    // Accumulate the patch ops for this object.
                    const pending = pendingPatchesRef.current;
                    const existing = pending.get(stateEvent.object);
                    if (existing) {
                        existing.push(...stateEvent.patch);
                    } else {
                        pending.set(stateEvent.object, [...stateEvent.patch]);
                    }

                    // Schedule a single deferred flush that drains all
                    // accumulated patches in one setState call.
                    scheduleFlush(() => {
                        const toApply = pendingPatchesRef.current;
                        if (toApply.size === 0) return;

                        // Swap in a fresh map before touching state so any
                        // patches that arrive during the setState callback
                        // go into a new accumulator.
                        pendingPatchesRef.current = new Map();

                        setObjects((prev) =>
                            prev.map((obj) => {
                                const ops = toApply.get(obj.name);
                                if (!ops || ops.length === 0) return obj;
                                try {
                                    // produce() gives structural sharing:
                                    // only the objects along the mutated
                                    // path get new references; everything
                                    // else stays pointer-equal to the old
                                    // document, keeping React.memo effective.
                                    const patched = produce(
                                        obj.value,
                                        (draft) => {
                                            applyPatch(
                                                draft,
                                                ops,
                                                /* validate */ false,
                                                /* mutate */ true,
                                            );
                                        },
                                    );
                                    return { ...obj, value: patched };
                                } catch (patchError) {
                                    console.error(
                                        "Error applying patch batch:",
                                        patchError,
                                    );
                                    return obj;
                                }
                            }),
                        );
                    });
                }
            } catch (err) {
                console.error("Error parsing state event:", err);
                setError(err instanceof Error ? err.message : "Unknown error");
            }
        };

        eventSource.onerror = () => {
            setConnected(false);
            setError("Connection to server lost. Retrying...");
        };

        return () => {
            // Cancel any pending flush and discard accumulated patches so
            // they don't bleed into the next connection's state.
            if (flushTimerRef.current !== null) {
                clearTimeout(flushTimerRef.current);
                flushTimerRef.current = null;
            }
            pendingPatchesRef.current = new Map();
            eventSource.close();
        };
    }, [objectNames]);

    return { objects, error, connected };
}
