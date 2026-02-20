import { useState, useEffect, useMemo, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import { useScript } from "../hooks/useScript";
import { useStateObjects } from "../hooks/useStateObjects";
import { Chat, ChatMessage } from "../types/api";

interface MarkdownContentProps {
    text: string;
    /** Extra CSS classes applied to the wrapping div */
    className?: string;
}

function MarkdownContent({ text, className = "" }: MarkdownContentProps) {
    return (
        <div className={`markdown-content ${className}`}>
            <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                    // Headings
                    h1: ({ children }) => (
                        <h1 className="text-xl font-bold mt-3 mb-1">{children}</h1>
                    ),
                    h2: ({ children }) => (
                        <h2 className="text-lg font-bold mt-3 mb-1">{children}</h2>
                    ),
                    h3: ({ children }) => (
                        <h3 className="text-base font-bold mt-2 mb-1">{children}</h3>
                    ),
                    // Paragraph — top margin only after the first child
                    p: ({ children }) => (
                        <p className="mt-2 first:mt-0 leading-relaxed">{children}</p>
                    ),
                    // Lists
                    ul: ({ children }) => (
                        <ul className="mt-2 ml-4 list-disc space-y-0.5">{children}</ul>
                    ),
                    ol: ({ children }) => (
                        <ol className="mt-2 ml-4 list-decimal space-y-0.5">{children}</ol>
                    ),
                    li: ({ children }) => <li className="leading-relaxed">{children}</li>,
                    // Inline code
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    code: ({ inline, className: cls, children, ...props }: any) => {
                        const match = /language-(\w+)/.exec(cls || "");
                        if (!inline && match) {
                            return (
                                <SyntaxHighlighter
                                    style={vscDarkPlus}
                                    language={match[1]}
                                    PreTag="div"
                                    className="rounded mt-2 text-sm"
                                    {...props}
                                >
                                    {String(children).replace(/\n$/, "")}
                                </SyntaxHighlighter>
                            );
                        }
                        if (!inline) {
                            // Fenced block with no language hint
                            return (
                                <SyntaxHighlighter
                                    style={vscDarkPlus}
                                    PreTag="div"
                                    className="rounded mt-2 text-sm"
                                    {...props}
                                >
                                    {String(children).replace(/\n$/, "")}
                                </SyntaxHighlighter>
                            );
                        }
                        return (
                            <code
                                className="bg-black/30 rounded px-1 py-0.5 font-mono text-sm"
                                {...props}
                            >
                                {children}
                            </code>
                        );
                    },
                    // Blockquote
                    blockquote: ({ children }) => (
                        <blockquote className="mt-2 border-l-4 border-current opacity-70 pl-3 italic">
                            {children}
                        </blockquote>
                    ),
                    // Horizontal rule
                    hr: () => <hr className="my-3 border-current opacity-30" />,
                    // Links
                    a: ({ href, children }) => (
                        <a
                            href={href}
                            target="_blank"
                            rel="noreferrer"
                            className="underline opacity-80 hover:opacity-100"
                        >
                            {children}
                        </a>
                    ),
                    // Strong / em
                    strong: ({ children }) => (
                        <strong className="font-bold">{children}</strong>
                    ),
                    em: ({ children }) => <em className="italic">{children}</em>,
                }}
            >
                {text}
            </ReactMarkdown>
        </div>
    );
}

interface MessageBubbleProps {
    message: ChatMessage;
    onSelectTrace?: (traceId: string) => void;
}

function MessageBubble({ message, onSelectTrace }: MessageBubbleProps) {
    const isUser = message.role === "user";
    const isSystem = message.role === "system";
    const [showThinking, setShowThinking] = useState(false);

    const thinkingText = message.thinking?.join("");
    const contentText = message.content.join("");

    return (
        <div
            className={`flex ${isUser ? "justify-end" : "justify-start"} ${isSystem ? "justify-center" : ""}`}
        >
            <div
                className={`max-w-[70%] rounded-lg px-4 py-2 ${
                    isUser
                        ? "bg-blue-600 text-white"
                        : isSystem
                          ? "bg-gray-700 text-gray-300 text-sm italic"
                          : "bg-gray-700 text-gray-100"
                }`}
            >
                {thinkingText && (
                    <div className="mb-2">
                        <button
                            onClick={() => setShowThinking(!showThinking)}
                            className="text-xs text-gray-400 hover:text-gray-200 underline"
                        >
                            {showThinking ? "▼" : "►"} thinking
                        </button>
                        {showThinking && (
                            <div className="mt-1 bg-gray-900 rounded p-2 text-xs text-gray-300">
                                <MarkdownContent text={thinkingText} />
                            </div>
                        )}
                    </div>
                )}

                <MarkdownContent text={contentText} />

                {message.error && (
                    <div className="mt-2 rounded bg-red-900/50 border border-red-700 px-3 py-2 text-xs text-red-300">
                        <span className="font-bold">Error:</span>{" "}
                        {message.error}
                    </div>
                )}

                {message.trace_id && (
                    <button
                        onClick={() => onSelectTrace?.(message.trace_id!)}
                        className="text-xs opacity-50 hover:opacity-100 mt-1 font-mono transition-opacity hover:underline cursor-pointer text-left"
                    >
                        trace: {message.trace_id}
                    </button>
                )}
            </div>
        </div>
    );
}

interface ChatViewProps {
    onSelectTrace?: (traceId: string) => void;
}

export function ChatView({ onSelectTrace }: ChatViewProps) {
    const [chatIds, setChatIds] = useState<string[]>([]);
    const [selectedChatId, setSelectedChatId] = useState<string | null>(null);
    const [inputMessage, setInputMessage] = useState("");

    // Script hooks for chat actions
    // TODO: Each chat should use its own script context
    const listScript = useScript<string[]>("chat", "chat_action");
    const newChatScript = useScript<string>("chat", "chat_action");
    const pokeScript = useScript<void>("chat", "chat_action");
    const appendMessageScript = useScript<void>("chat", "chat_action");
    const generateScript = useScript<void>("chat", "chat_action");

    // State hook for selected chat - memoize to prevent infinite re-renders
    const stateObjectNames = useMemo(
        () => (selectedChatId ? [`chat:${selectedChatId}`] : []),
        [selectedChatId],
    );
    const { objects } = useStateObjects(stateObjectNames);

    // Get the chat data from state objects
    const chatData = objects.find(
        (obj) => obj.name === `chat:${selectedChatId}`,
    )?.value as Chat | undefined;

    // Ref for auto-scrolling to bottom
    const messagesEndRef = useRef<HTMLDivElement>(null);

    // Load chat list on mount
    useEffect(() => {
        const loadChats = async () => {
            const result = await listScript.runScript({ action: "list" });
            if (result) {
                setChatIds(result);
            }
        };
        loadChats();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Auto-scroll to bottom when messages change
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [chatData?.messages]);

    const handleNewChat = async () => {
        const newChatId = await newChatScript.runScript({ action: "new" });
        if (newChatId) {
            setChatIds((prev) => [...prev, newChatId]);
            setSelectedChatId(newChatId);
        }
    };

    const handleSelectChat = async (chatId: string) => {
        setSelectedChatId(chatId);
        // Poke the chat to load it into the state system
        await pokeScript.runScript({ action: "poke", chat_id: chatId });
    };

    const handleSendMessage = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!selectedChatId || !inputMessage.trim()) return;

        const messageToSend = inputMessage.trim();
        setInputMessage(""); // Clear input immediately

        await appendMessageScript.runScript({
            action: "append_message",
            chat_id: selectedChatId,
            role: "user",
            content: messageToSend,
        });

        await generateScript.runScript({
            action: "generate",
            chat_id: selectedChatId,
        });
    };

    return (
        <div className="flex-1 flex overflow-hidden">
            {/* Sidebar */}
            <div className="w-64 bg-gray-800 border-r border-gray-700 flex flex-col">
                <div className="p-4 border-b border-gray-700">
                    <button
                        onClick={handleNewChat}
                        disabled={newChatScript.loading}
                        className="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 text-white rounded transition-colors"
                    >
                        {newChatScript.loading ? "Creating..." : "+ New Chat"}
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto">
                    {listScript.loading && (
                        <div className="p-4 text-gray-400 text-sm">
                            Loading chats...
                        </div>
                    )}
                    {listScript.error && (
                        <div className="p-4 text-red-400 text-sm">
                            Error: {listScript.error}
                        </div>
                    )}
                    {chatIds.length === 0 && !listScript.loading && (
                        <div className="p-4 text-gray-500 text-sm">
                            No chats yet. Create one!
                        </div>
                    )}
                    <div className="space-y-1 p-2">
                        {chatIds.map((chatId) => (
                            <button
                                key={chatId}
                                onClick={() => handleSelectChat(chatId)}
                                className={`w-full text-left px-3 py-2 rounded transition-colors ${
                                    selectedChatId === chatId
                                        ? "bg-gray-700 text-white"
                                        : "text-gray-300 hover:bg-gray-750"
                                }`}
                            >
                                <div className="font-mono text-xs truncate">
                                    {chatId}
                                </div>
                            </button>
                        ))}
                    </div>
                </div>
            </div>

            {/* Main panel */}
            <div className="flex-1 flex flex-col overflow-hidden bg-gray-900">
                {!selectedChatId ? (
                    <div className="flex-1 flex items-center justify-center text-gray-500">
                        <div className="text-center">
                            <div className="text-6xl mb-4">💬</div>
                            <div className="text-xl">
                                Select a chat or create a new one
                            </div>
                        </div>
                    </div>
                ) : (
                    <>
                        {/* Messages area */}
                        <div className="flex-1 overflow-y-auto p-4">
                            <div className="mb-4 text-gray-400 text-xs font-mono">
                                Chat ID: {selectedChatId}
                            </div>
                            {chatData ? (
                                <div className="space-y-4">
                                    {chatData.messages.map((message, index) => (
                                        <MessageBubble
                                            key={index}
                                            message={message}
                                            onSelectTrace={onSelectTrace}
                                        />
                                    ))}
                                    <div ref={messagesEndRef} />
                                </div>
                            ) : (
                                <div className="text-gray-500 text-sm">
                                    Loading chat data...
                                </div>
                            )}
                        </div>

                        {/* Input bar */}
                        <div className="border-t border-gray-700 bg-gray-800 p-4">
                            <form
                                onSubmit={handleSendMessage}
                                className="flex gap-2"
                            >
                                <input
                                    type="text"
                                    value={inputMessage}
                                    onChange={(e) =>
                                        setInputMessage(e.target.value)
                                    }
                                    placeholder="Type a message..."
                                    className="flex-1 px-4 py-2 bg-gray-700 text-white rounded border border-gray-600 focus:outline-none focus:border-blue-500"
                                    disabled={appendMessageScript.loading}
                                />
                                <button
                                    type="submit"
                                    disabled={
                                        !inputMessage.trim() ||
                                        appendMessageScript.loading
                                    }
                                    className="px-6 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white rounded transition-colors"
                                >
                                    {appendMessageScript.loading
                                        ? "Sending..."
                                        : "Send"}
                                </button>
                            </form>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
