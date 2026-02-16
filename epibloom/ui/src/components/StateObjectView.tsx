import { StateObjectData } from "../types/api";

interface StateObjectViewProps {
    object: StateObjectData | null;
}

export function StateObjectView({ object }: StateObjectViewProps) {
    if (!object) {
        return (
            <div className="flex items-center justify-center h-full text-gray-400 bg-gray-900">
                Select a state object to view its data
            </div>
        );
    }

    return (
        <div className="flex flex-col h-full bg-gray-900">
            {/* Header */}
            <div className="border-b border-gray-700 bg-gray-800 p-3">
                <h3 className="text-sm font-semibold text-white">
                    {object.name}
                </h3>
            </div>

            {/* JSON viewer */}
            <div className="flex-1 overflow-auto p-3">
                <pre className="text-xs text-gray-300 font-mono">
                    {JSON.stringify(object.value, null, 2)}
                </pre>
            </div>
        </div>
    );
}
