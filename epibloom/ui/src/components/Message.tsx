import { useState } from "react";
import { Message as MessageType } from "../types/api";

interface MessageProps {
    message: MessageType;
}

export function Message({ message }: MessageProps) {
    const [showThinking, setShowThinking] = useState(false);

    const roleStyles = {
        user: { text: "text-blue-400", border: "border-blue-900" },
        assistant: { text: "text-green-400", border: "border-green-900" },
        system: { text: "text-gray-400", border: "border-gray-600" },
    };

    const roleLabels = {
        user: "user",
        assistant: "assistant",
        system: "system",
    };

    return (
        <div
            className={`border rounded p-2 ${roleStyles[message.role].border}`}
        >
            <div className={`font-bold mb-1 ${roleStyles[message.role].text}`}>
                {roleLabels[message.role]}
            </div>

            {message.thinking && (
                <div className="mb-2">
                    <button
                        onClick={() => setShowThinking(!showThinking)}
                        className="text-xs text-gray-400 hover:text-gray-200 underline"
                    >
                        {showThinking ? "▼" : "►"} thinking
                    </button>
                    {showThinking && (
                        <pre className="mt-1 p-2 bg-gray-800 rounded text-xs whitespace-pre-wrap text-gray-300">
                            {message.thinking}
                        </pre>
                    )}
                </div>
            )}

            <div className="text-gray-200 whitespace-pre-wrap break-words">
                {message.text}
            </div>
        </div>
    );
}
