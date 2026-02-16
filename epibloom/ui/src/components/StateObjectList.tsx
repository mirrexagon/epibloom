import { useState } from "react";
import { StateObjectData } from "../types/api";

interface StateObjectListProps {
    objects: StateObjectData[];
    selectedObjectName: string | null;
    onSelectObject: (objectName: string) => void;
    onAddObject: (objectName: string) => void;
    onRemoveObject: (objectName: string) => void;
    connected: boolean;
    error: string | null;
}

export function StateObjectList({
    objects,
    selectedObjectName,
    onSelectObject,
    onAddObject,
    onRemoveObject,
    connected,
    error,
}: StateObjectListProps) {
    const [newObjectName, setNewObjectName] = useState("");

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        if (newObjectName.trim()) {
            onAddObject(newObjectName.trim());
            setNewObjectName("");
        }
    };

    return (
        <div className="flex flex-col h-full bg-gray-800">
            {/* Header */}
            <div className="border-b border-gray-700 bg-gray-800 p-3">
                <div className="flex items-center justify-between mb-2">
                    <h3 className="text-sm font-semibold text-white">
                        State Objects
                    </h3>
                    <div
                        className={`w-2 h-2 rounded-full ${
                            connected ? "bg-green-500" : "bg-red-500"
                        }`}
                        title={connected ? "Connected" : "Disconnected"}
                    />
                </div>
                {error && (
                    <div className="text-xs text-red-400 mt-1">{error}</div>
                )}

                {/* Add object form */}
                <form onSubmit={handleSubmit} className="mt-2">
                    <div className="flex gap-1">
                        <input
                            type="text"
                            value={newObjectName}
                            onChange={(e) => setNewObjectName(e.target.value)}
                            placeholder="object:name"
                            className="flex-1 px-2 py-1 text-xs bg-gray-700 border border-gray-600 rounded text-white placeholder-gray-400 focus:outline-none focus:border-blue-500"
                        />
                        <button
                            type="submit"
                            className="px-2 py-1 text-xs bg-blue-600 hover:bg-blue-700 text-white rounded"
                        >
                            Add
                        </button>
                    </div>
                </form>
            </div>

            {/* Object list */}
            <div className="flex-1 overflow-y-auto">
                {objects.length === 0 ? (
                    <div className="p-3 text-xs text-gray-400 text-center">
                        No state objects subscribed
                    </div>
                ) : (
                    <div className="divide-y divide-gray-700">
                        {objects.map((obj) => (
                            <div
                                key={obj.name}
                                className={`flex items-center justify-between p-3 hover:bg-gray-700 cursor-pointer ${
                                    selectedObjectName === obj.name
                                        ? "bg-blue-900 border-l-4 border-blue-500"
                                        : ""
                                }`}
                                onClick={() => onSelectObject(obj.name)}
                            >
                                <div className="flex-1 min-w-0">
                                    <div className="font-medium text-xs truncate text-gray-100">
                                        {obj.name}
                                    </div>
                                </div>
                                <button
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        onRemoveObject(obj.name);
                                    }}
                                    className="ml-2 text-xs text-gray-400 hover:text-red-400"
                                    title="Remove"
                                >
                                    ✕
                                </button>
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}
