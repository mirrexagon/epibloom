import { useState } from "react";
import { Message as MessageType } from "../types/api";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";

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
                        <div className="mt-1 bg-gray-800 rounded text-xs overflow-hidden">
                            <SyntaxHighlighter
                                language="markdown"
                                style={oneDark}
                                customStyle={{
                                    margin: 0,
                                    padding: "0.5rem",
                                    background: "rgb(31, 41, 55)",
                                    fontSize: "0.75rem",
                                    whiteSpace: "pre-wrap",
                                    wordBreak: "break-word",
                                    overflowWrap: "break-word",
                                    maxWidth: "100%",
                                    overflow: "auto",
                                }}
                                codeTagProps={{
                                    style: {
                                        fontFamily: "ui-monospace, monospace",
                                        whiteSpace: "pre-wrap",
                                        wordBreak: "break-word",
                                    },
                                }}
                            >
                                {message.thinking}
                            </SyntaxHighlighter>
                        </div>
                    )}
                </div>
            )}

            <div className="text-gray-200 overflow-hidden">
                <SyntaxHighlighter
                    language="markdown"
                    style={oneDark}
                    customStyle={{
                        margin: 0,
                        padding: "0.5rem",
                        background: "transparent",
                        fontSize: "0.875rem",
                        whiteSpace: "pre-wrap",
                        wordBreak: "break-word",
                        overflowWrap: "break-word",
                        maxWidth: "100%",
                        overflow: "auto",
                    }}
                    codeTagProps={{
                        style: {
                            fontFamily: "ui-monospace, monospace",
                            whiteSpace: "pre-wrap",
                            wordBreak: "break-word",
                        },
                    }}
                >
                    {message.text}
                </SyntaxHighlighter>
            </div>
        </div>
    );
}
