import { useState } from "react";
import { Message as MessageType } from "../types/api";

interface MessageProps {
    message: MessageType;
}

export function Message({ message }: MessageProps) {
    const [showThinking, setShowThinking] = useState(false);

    const roleColors = {
        user: "bg-blue-900 border-blue-700",
        assistant: "bg-green-900 border-green-700",
        system: "bg-gray-700 border-gray-600",
    };

    const roleLabels = {
        user: "User",
        assistant: "Assistant",
        system: "System",
    };

    return (
        <div className={`border rounded-lg p-4 ${roleColors[message.role]}`}>
            <div className="flex items-center justify-between mb-2">
                <span className="font-semibold text-sm text-gray-100">
                    {roleLabels[message.role]}
                </span>
            </div>

            {message.thinking && (
                <div className="mb-2">
                    <button
                        onClick={() => setShowThinking(!showThinking)}
                        className="text-sm text-gray-300 hover:text-gray-100 underline"
                    >
                        {showThinking ? "▼" : "►"} Thinking
                    </button>
                    {showThinking && (
                        <pre className="mt-2 p-2 bg-gray-800 rounded text-xs whitespace-pre-wrap font-mono text-gray-200">
                            {message.thinking}
                        </pre>
                    )}
                </div>
            )}

            <div className="whitespace-pre-wrap text-gray-100">
                {message.text}
            </div>
        </div>
    );
}
