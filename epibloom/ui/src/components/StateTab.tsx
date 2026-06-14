import { useState, useMemo } from "react";
import { useStateObjects } from "../hooks/useStateObjects";

export function StateTab() {
    const [selectedObject, setSelectedObject] = useState<string | null>(null);
    
    // Subscribe to the index (empty string name) to get the list of all state objects
    const indexObjectNames = useMemo(() => [""], []);
    const { objects: indexObjects, connected: indexConnected, error: indexError } = 
        useStateObjects(indexObjectNames);
    
    // Subscribe to the selected object if any
    const selectedObjectNames = useMemo(
        () => (selectedObject !== null ? [selectedObject] : []),
        [selectedObject]
    );
    const { objects: selectedObjects } = useStateObjects(selectedObjectNames);

    const stateObjectNames = (indexObjects[0]?.value as string[] | undefined) || [];
    const selectedObjectData = selectedObjects[0];

    return (
        <div className="h-full flex bg-gray-900 text-white">
            {/* Left sidebar - list of state objects */}
            <div className="w-80 border-r border-gray-700 flex flex-col">
                <div className="p-4 border-b border-gray-700">
                    <h2 className="text-lg font-semibold">State Objects</h2>
                    <div className="text-sm text-gray-400 mt-1">
                        {indexConnected ? (
                            <span className="text-green-400">● Connected</span>
                        ) : (
                            <span className="text-red-400">● Disconnected</span>
                        )}
                    </div>
                    {indexError && (
                        <div className="text-sm text-red-400 mt-1">{indexError}</div>
                    )}
                </div>
                <div className="flex-1 overflow-y-auto">
                    {stateObjectNames.map((name) => (
                        <button
                            key={name}
                            onClick={() => setSelectedObject(name)}
                            className={`w-full text-left px-4 py-2 border-b border-gray-800 hover:bg-gray-800 transition-colors ${
                                selectedObject === name ? "bg-gray-800 text-blue-400" : ""
                            }`}
                        >
                            <div className="font-mono text-sm break-all">
                                {name === "" ? "(index)" : name}
                            </div>
                        </button>
                    ))}
                </div>
            </div>

            {/* Right panel - selected object details */}
            <div className="flex-1 flex flex-col overflow-hidden">
                {selectedObject !== null ? (
                    <>
                        <div className="p-4 border-b border-gray-700">
                            <h3 className="text-lg font-semibold font-mono break-all">
                                {selectedObject === "" ? "(index)" : selectedObject}
                            </h3>
                        </div>
                        <div className="flex-1 overflow-auto p-4">
                            {selectedObjectData ? (
                                <pre className="text-sm bg-gray-800 p-4 rounded overflow-x-auto">
                                    {JSON.stringify(selectedObjectData.value, null, 2)}
                                </pre>
                            ) : (
                                <div className="text-gray-400">Loading...</div>
                            )}
                        </div>
                    </>
                ) : (
                    <div className="flex items-center justify-center h-full text-gray-400">
                        Select a state object to view its contents
                    </div>
                )}
            </div>
        </div>
    );
}
