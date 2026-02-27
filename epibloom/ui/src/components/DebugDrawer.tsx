import { useState, useCallback, useEffect, useRef } from "react";
import { TraceList } from "./TraceList";
import { TraceView } from "./TraceView";
import { StateObjectList } from "./StateObjectList";
import { StateObjectView } from "./StateObjectView";
import { TraceMetadata } from "../types/api";
import { useStateObjects } from "../hooks/useStateObjects";
import { useTraceList } from "../hooks/useTraceList";

type TabType = "traces" | "state";

interface DebugDrawerProps {
    isOpen: boolean;
    onToggle: () => void;
    height: number;
    onHeightChange: (height: number) => void;
    selectedTrace: TraceMetadata | null;
    onSelectTrace: (trace: TraceMetadata) => void;
    activeTab: TabType;
    onTabChange: (tab: TabType) => void;
    /** When set, the drawer adds this state object to the watch list and selects it. */
    requestedStateObject?: string | null;
}

export function DebugDrawer({
    isOpen,
    onToggle,
    height,
    onHeightChange,
    selectedTrace,
    onSelectTrace,
    activeTab,
    onTabChange,
    requestedStateObject,
}: DebugDrawerProps) {
    const {
        traces,
        error: traceListError,
        connected: traceListConnected,
        latestTrace,
    } = useTraceList();

    // Auto-select newly added traces
    useEffect(() => {
        if (latestTrace) {
            onSelectTrace(latestTrace);
        }
    }, [latestTrace, onSelectTrace]);

    const [selectedStateName, setSelectedStateName] = useState<string | null>(
        null,
    );
    const [watchedObjects, setWatchedObjects] = useState<string[]>([]);

    const {
        objects,
        error: stateError,
        connected: stateConnected,
    } = useStateObjects(watchedObjects);

    const [isDragging, setIsDragging] = useState(false);

    const handleAddStateObject = useCallback((objectName: string) => {
        setWatchedObjects((prev) => {
            if (prev.includes(objectName)) {
                return prev;
            }
            return [...prev, objectName];
        });
    }, []);

    // When requestedStateObject changes, add it to the watch list and select it.
    const prevRequestedStateObject = useRef<string | null | undefined>(undefined);
    useEffect(() => {
        if (
            requestedStateObject &&
            requestedStateObject !== prevRequestedStateObject.current
        ) {
            prevRequestedStateObject.current = requestedStateObject;
            handleAddStateObject(requestedStateObject);
            setSelectedStateName(requestedStateObject);
        }
    }, [requestedStateObject, handleAddStateObject]);

    const handleRemoveStateObject = useCallback(
        (objectName: string) => {
            setWatchedObjects((prev) =>
                prev.filter((name) => name !== objectName),
            );
            if (selectedStateName === objectName) {
                setSelectedStateName(null);
            }
        },
        [selectedStateName],
    );

    const handleMouseDown = (e: React.MouseEvent) => {
        e.preventDefault();
        setIsDragging(true);
    };

    const handleMouseMove = useCallback(
        (e: MouseEvent) => {
            if (isDragging) {
                const newHeight = window.innerHeight - e.clientY;
                const clampedHeight = Math.max(
                    100,
                    Math.min(newHeight, window.innerHeight - 100),
                );
                onHeightChange(clampedHeight);
            }
        },
        [isDragging, onHeightChange],
    );

    const handleMouseUp = useCallback(() => {
        setIsDragging(false);
    }, []);

    // Global mouse event listeners for dragging
    useEffect(() => {
        if (isDragging) {
            window.addEventListener("mousemove", handleMouseMove);
            window.addEventListener("mouseup", handleMouseUp);
            return () => {
                window.removeEventListener("mousemove", handleMouseMove);
                window.removeEventListener("mouseup", handleMouseUp);
            };
        }
    }, [isDragging, handleMouseMove, handleMouseUp]);

    const selectedObject = objects.find(
        (obj) => obj.name === selectedStateName,
    );

    if (!isOpen) {
        return (
            <div className="bg-gray-800 border-t border-gray-700">
                {/* Fake drag handle */}
                <div
                    className="h-1 bg-gray-700 hover:bg-blue-500 cursor-ns-resize transition-colors"
                    onMouseUp={onToggle}
                />

                <div className="w-full py-2 px-4 text-sm flex items-center justify-end gap-2">
                    <button
                        onClick={onToggle}
                        className="flex items-center gap-2 text-gray-300 hover:bg-gray-700 px-2 py-1 rounded transition-colors"
                    >
                        <span className="font-semibold">DEBUG</span>
                        <span className="text-gray-500">▲</span>
                    </button>
                </div>
            </div>
        );
    }

    return (
        <div
            className="bg-gray-800 border-t border-gray-700 flex flex-col"
            style={{ height: `${height}px` }}
        >
            {/* Drag handle */}
            <div
                className="h-1 bg-gray-700 hover:bg-blue-500 cursor-ns-resize transition-colors"
                onMouseDown={handleMouseDown}
            />

            {/* Header with tabs and controls */}
            <div className="border-b border-gray-700 bg-gray-800 px-4 py-2 flex items-center justify-between w-full">
                <div className="flex gap-1">
                    <button
                        onClick={() => onTabChange("traces")}
                        className={`px-3 py-1 text-sm font-medium rounded transition-colors ${
                            activeTab === "traces"
                                ? "bg-blue-600 text-white"
                                : "text-gray-400 hover:text-gray-200 hover:bg-gray-700"
                        }`}
                    >
                        Traces
                    </button>
                    <button
                        onClick={() => onTabChange("state")}
                        className={`px-3 py-1 text-sm font-medium rounded transition-colors ${
                            activeTab === "state"
                                ? "bg-blue-600 text-white"
                                : "text-gray-400 hover:text-gray-200 hover:bg-gray-700"
                        }`}
                    >
                        State
                    </button>
                </div>

                <button
                    onClick={onToggle}
                    className="flex items-center gap-2 text-sm hover:bg-gray-700 px-2 py-1 rounded transition-colors"
                >
                    <span className="font-semibold text-gray-300">DEBUG</span>
                    <span className="text-gray-500">▼</span>
                </button>
            </div>

            {/* Tab content */}
            <div className="flex-1 flex overflow-hidden">
                {activeTab === "traces" ? (
                    <>
                        <div className="w-64 flex-shrink-0 border-r border-gray-700">
                            <TraceList
                                traces={traces}
                                error={traceListError}
                                connected={traceListConnected}
                                selectedTraceId={selectedTrace?.id || null}
                                onSelectTrace={onSelectTrace}
                            />
                        </div>
                        <div className="flex-1">
                            <TraceView
                                traceId={selectedTrace?.id || null}
                                traceName={
                                    selectedTrace
                                        ? (traces.find(t => t.id === selectedTrace.id)?.name ?? selectedTrace.name)
                                        : null
                                }
                            />
                        </div>
                    </>
                ) : (
                    <>
                        <div className="w-64 flex-shrink-0 border-r border-gray-700">
                            <StateObjectList
                                objects={objects}
                                selectedObjectName={selectedStateName}
                                onSelectObject={setSelectedStateName}
                                onAddObject={handleAddStateObject}
                                onRemoveObject={handleRemoveStateObject}
                                connected={stateConnected}
                                error={stateError}
                            />
                        </div>
                        <div className="flex-1">
                            <StateObjectView object={selectedObject || null} />
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
