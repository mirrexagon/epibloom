import { useEffect, useState } from "react";
import { StateEvent, StateObjectData } from "../types/api";
import { applyPatch } from "fast-json-patch";

interface UseStateObjectsResult {
    objects: StateObjectData[];
    error: string | null;
    connected: boolean;
}

export function useStateObjects(objectNames: string[]): UseStateObjectsResult {
    const [objects, setObjects] = useState<StateObjectData[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [connected, setConnected] = useState(false);

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
                    // Initial value or full update
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
                    // Apply JSON patch
                    setObjects((prev) =>
                        prev.map((obj) => {
                            if (obj.name === stateEvent.object) {
                                try {
                                    const patched = applyPatch(
                                        obj.value,
                                        stateEvent.patch,
                                        /* validate */ false,
                                        /* mutate */ false,
                                    ).newDocument;
                                    return { ...obj, value: patched };
                                } catch (patchError) {
                                    console.error(
                                        "Error applying patch:",
                                        patchError,
                                    );
                                    return obj;
                                }
                            }
                            return obj;
                        }),
                    );
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
            eventSource.close();
        };
    }, [objectNames]);

    return { objects, error, connected };
}
