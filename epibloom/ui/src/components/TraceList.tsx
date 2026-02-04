import { useTraceList } from "../hooks/useTraceList";
import { TraceMetadata } from "../types/api";

interface TraceListProps {
    selectedTraceId: string | null;
    onSelectTrace: (trace: TraceMetadata) => void;
}

export function TraceList({ selectedTraceId, onSelectTrace }: TraceListProps) {
    const { traces, error, connected } = useTraceList();

    const formatDate = (dateString: string) => {
        const date = new Date(dateString);
        return date.toLocaleString();
    };

    return (
        <div className="flex flex-col h-full bg-gray-800 border-r border-gray-700">
            {/* Header */}
            <div className="border-b border-gray-700 bg-gray-800 p-4">
                <div className="flex items-center justify-between mb-2">
                    <h2 className="text-lg font-semibold text-white">Traces</h2>
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
            </div>

            {/* Trace list */}
            <div className="flex-1 overflow-y-auto">
                {traces.length === 0 ? (
                    <div className="p-4 text-sm text-gray-400 text-center">
                        {connected ? "No traces yet" : "Connecting..."}
                    </div>
                ) : (
                    <div className="divide-y divide-gray-700">
                        {traces.map((trace) => (
                            <button
                                key={trace.id}
                                onClick={() => onSelectTrace(trace)}
                                className={`w-full text-left p-4 hover:bg-gray-700 transition-colors ${
                                    selectedTraceId === trace.id
                                        ? "bg-blue-900 border-l-4 border-blue-500"
                                        : ""
                                }`}
                            >
                                <div className="font-medium text-sm mb-1 truncate text-gray-100">
                                    {trace.name}
                                </div>
                                <div className="text-xs text-gray-400">
                                    {formatDate(trace.created)}
                                </div>
                            </button>
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}
