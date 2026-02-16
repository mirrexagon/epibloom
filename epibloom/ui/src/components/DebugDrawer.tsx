import { useState, useCallback, useEffect } from "react";
import { TraceList } from "./TraceList";
import { TraceView } from "./TraceView";
import { StateObjectList } from "./StateObjectList";
import { StateObjectView } from "./StateObjectView";
import { TraceMetadata } from "../types/api";
import { useStateObjects } from "../hooks/useStateObjects";

type TabType = "traces" | "state";

interface DebugDrawerProps {
    isOpen: boolean;
    onToggle: () => void;
    height: number;
    onHeightChange: (height: number) => void;
}

export function DebugDrawer({
    isOpen,
    onToggle,
    height,
    onHeightChange,
}: DebugDrawerProps) {
    const [activeTab, setActiveTab] = useState<TabType>("traces");
    const [selectedTrace, setSelectedTrace] = useState<TraceMetadata | null>(
        null,
    );
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

    const handleSelectTrace = useCallback((trace: TraceMetadata) => {
        setSelectedTrace(trace);
    }, []);

    const handleAddStateObject = useCallback((objectName: string) => {
        setWatchedObjects((prev) => {
            if (prev.includes(objectName)) {
                return prev;
            }
            return [...prev, objectName];
        });
    }, []);

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

    // Effect for cursor style during dragging
    useEffect(() => {
        if (isDragging) {
            document.body.style.cursor = "ns-resize";
            document.body.style.userSelect = "none";
        } else {
            document.body.style.cursor = "";
            document.body.style.userSelect = "";
        }
    }, [isDragging]);

    const selectedObject = objects.find(
        (obj) => obj.name === selectedStateName,
    );

    if (!isOpen) {
        return (
            <div className="bg-gray-800 border-t border-gray-700">
                {/* Fake drag handle */}
                <div
                    className="h-1 bg-gray-700 hover:bg-blue-500 cursor-ns-resize transition-colors"
                    onMouseDown={onToggle}
                />

                <button
                    onClick={onToggle}
                    className="w-full py-2 px-4 text-sm text-gray-300 hover:bg-gray-700 flex items-center justify-end gap-2"
                >
                    <span className="font-semibold">DEBUG</span>
                    <span className="text-gray-500">▲</span>
                </button>
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
            <button
                onClick={onToggle}
                className="border-b border-gray-700 bg-gray-800 hover:bg-gray-700 px-4 py-2 flex items-center justify-between transition-colors w-full"
            >
                <div
                    className="flex gap-1"
                    onClick={(e) => e.stopPropagation()}
                >
                    <button
                        onClick={() => setActiveTab("traces")}
                        className={`px-3 py-1 text-sm font-medium rounded transition-colors ${
                            activeTab === "traces"
                                ? "bg-blue-600 text-white"
                                : "text-gray-400 hover:text-gray-200 hover:bg-gray-700"
                        }`}
                    >
                        Traces
                    </button>
                    <button
                        onClick={() => setActiveTab("state")}
                        className={`px-3 py-1 text-sm font-medium rounded transition-colors ${
                            activeTab === "state"
                                ? "bg-blue-600 text-white"
                                : "text-gray-400 hover:text-gray-200 hover:bg-gray-700"
                        }`}
                    >
                        State
                    </button>
                </div>

                <div className="flex items-center gap-2 text-sm">
                    <span className="font-semibold text-gray-300">DEBUG</span>
                    <span className="text-gray-500">▼</span>
                </div>
            </button>

            {/* Tab content */}
            <div className="flex-1 flex overflow-hidden">
                {activeTab === "traces" ? (
                    <>
                        <div className="w-64 flex-shrink-0 border-r border-gray-700">
                            <TraceList
                                selectedTraceId={selectedTrace?.id || null}
                                onSelectTrace={handleSelectTrace}
                            />
                        </div>
                        <div className="flex-1">
                            <TraceView
                                traceId={selectedTrace?.id || null}
                                traceName={selectedTrace?.name || null}
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
