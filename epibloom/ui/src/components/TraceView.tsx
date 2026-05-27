import { useTrace } from "../hooks/useTrace";
import { LlmTraceMessage } from "../types/api";

interface TraceViewProps {
    traceId: string | null;
    traceName: string | null;
}

export function TraceView({ traceId, traceName }: TraceViewProps) {
    const { trace, error, connected } = useTrace(traceId);

    if (!traceId) {
        return (
            <div className="flex items-center justify-center h-full text-gray-400 bg-gray-900">
                Select a trace to view its messages
            </div>
        );
    }

    return (
        <div className="flex flex-col h-full bg-gray-900">
            {/* Header */}
            <div className="border-b border-gray-700 bg-gray-800 p-4 flex items-center justify-between">
                <h2 className="text-xl font-semibold text-white">
                    {traceName || trace?.name || "Trace"}
                </h2>
                <div className="flex items-center gap-2">
                    <div
                        className={`w-2 h-2 rounded-full ${
                            connected ? "bg-green-500" : "bg-red-500"
                        }`}
                        title={connected ? "Connected" : "Disconnected"}
                    />
                    <span className="text-sm text-gray-300">
                        {connected ? "Connected" : "Disconnected"}
                    </span>
                </div>
            </div>

            {/* Error banner */}
            {error && (
                <div className="bg-red-900 border border-red-700 text-red-200 px-4 py-3">
                    <strong>Error:</strong> {error}
                </div>
            )}

            {/* Messages */}
            <div className="flex-1 overflow-y-auto p-4">
                {!trace || trace.messages.length === 0 ? (
                    <div className="text-gray-400 text-center">
                        No messages yet
                    </div>
                ) : (
                    <div className="font-mono text-xs text-gray-300 space-y-3">
                        {trace.messages.map((msg, idx) => (
                            <TraceMessageView key={idx} message={msg} />
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}

function TraceMessageView({ message }: { message: LlmTraceMessage }) {
    const content = message.content.join("");
    const thinking = message.thinking?.join("") ?? "";

    const roleColors: Record<string, string> = {
        system: "text-yellow-400",
        user: "text-blue-400",
        assistant: "text-green-400",
    };
    const roleColor = roleColors[message.role] ?? "text-gray-400";

    return (
        <div className="border border-gray-700 rounded p-3 space-y-2">
            {/* Role + status */}
            <div className="flex items-center gap-2">
                <span className={`font-bold uppercase text-xs ${roleColor}`}>
                    {message.role}
                </span>
                {message.generating && (
                    <span className="text-xs text-yellow-400 animate-pulse">
                        ● generating
                    </span>
                )}
                {message.error && (
                    <span className="text-xs text-red-400">
                        ✗ error
                    </span>
                )}
            </div>

            {/* Thinking */}
            {thinking && (
                <details className="text-gray-500">
                    <summary className="cursor-pointer text-xs text-gray-400 mb-1">
                        thinking ({thinking.length} chars)
                    </summary>
                    <pre className="whitespace-pre-wrap break-words text-xs">
                        {thinking}
                    </pre>
                </details>
            )}

            {/* Content */}
            {content && (
                <pre className="whitespace-pre-wrap break-words">{content}</pre>
            )}

            {/* Error */}
            {message.error && (
                <div className="text-red-400 text-xs mt-1">
                    {message.error}
                </div>
            )}
        </div>
    );
}
