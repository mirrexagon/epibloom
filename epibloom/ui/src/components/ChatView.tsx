import { useState, useEffect, useMemo, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import { useScript } from "../hooks/useScript";
import { useStateObjects } from "../hooks/useStateObjects";
import { Chat, ChatMessage } from "../types/api";

// ---------------------------------------------------------------------------
// Tree helpers
// ---------------------------------------------------------------------------

/** Returns the direct children of parentId, sorted by UUIDv7 (insertion order). */
function getChildren(
    messages: Record<string, ChatMessage>,
    parentId: string | null | undefined,
): ChatMessage[] {
    // When there is no parent ID, it is just missing from the object, due to
    // Lua->JSON serialization quirks.
    if (parentId === null) {
        parentId = undefined;
    }

    return Object.values(messages)
        .filter((m) => m.parent_id === parentId)
        .sort((a, b) => a.id.localeCompare(b.id));
}

/** Walks parent_id links from leafId to the root and returns a root-first array. */
function getPath(
    messages: Record<string, ChatMessage>,
    leafId: string,
): ChatMessage[] {
    const path: ChatMessage[] = [];
    let currentId: string | null = leafId;
    while (currentId !== null) {
        const node = messages[currentId];
        if (!node) break;
        path.unshift(node);
        currentId = node.parent_id;
    }
    return path;
}

/** Follows the first child repeatedly from nodeId until reaching a leaf; returns that leaf's id. */
function getFirstLeaf(
    messages: Record<string, ChatMessage>,
    nodeId: string,
): string {
    let currentId = nodeId;
    let children = getChildren(messages, currentId);
    while (children.length > 0) {
        currentId = children[0].id;
        children = getChildren(messages, currentId);
    }
    return currentId;
}

// ---------------------------------------------------------------------------
// MarkdownContent
// ---------------------------------------------------------------------------

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
                        <h1 className="text-xl font-bold mt-3 mb-1">
                            {children}
                        </h1>
                    ),
                    h2: ({ children }) => (
                        <h2 className="text-lg font-bold mt-3 mb-1">
                            {children}
                        </h2>
                    ),
                    h3: ({ children }) => (
                        <h3 className="text-base font-bold mt-2 mb-1">
                            {children}
                        </h3>
                    ),
                    // Paragraph — top margin only after the first child
                    p: ({ children }) => (
                        <p className="mt-2 first:mt-0 leading-relaxed">
                            {children}
                        </p>
                    ),
                    // Lists
                    ul: ({ children }) => (
                        <ul className="mt-2 ml-4 list-disc space-y-0.5">
                            {children}
                        </ul>
                    ),
                    ol: ({ children }) => (
                        <ol className="mt-2 ml-4 list-decimal space-y-0.5">
                            {children}
                        </ol>
                    ),
                    li: ({ children }) => (
                        <li className="leading-relaxed">{children}</li>
                    ),
                    // Inline code
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    code: ({
                        inline,
                        className: cls,
                        children,
                        ...props
                    }: any) => {
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
                    em: ({ children }) => (
                        <em className="italic">{children}</em>
                    ),
                }}
            >
                {text}
            </ReactMarkdown>
        </div>
    );
}

// ---------------------------------------------------------------------------
// MessageBubble
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ChatView
// ---------------------------------------------------------------------------

interface ChatViewProps {
    onSelectTrace?: (traceId: string) => void;
    onSelectStateObject?: (name: string) => void;
}

export function ChatView({
    onSelectTrace,
    onSelectStateObject,
}: ChatViewProps) {
    const [chatIds, setChatIds] = useState<string[]>([]);
    const [selectedChatId, setSelectedChatId] = useState<string | null>(null);
    const [selectedLeafId, setSelectedLeafId] = useState<string | null>(null);
    const [inputMessage, setInputMessage] = useState("");

    // Script hooks for chat actions
    // TODO: Each chat should use its own script context
    const listScript = useScript<string[]>("chat", "chat_action");
    const newChatScript = useScript<string>("chat", "chat_action");
    const pokeScript = useScript<void>("chat", "chat_action");
    const addUserMessageScript = useScript<string>("chat", "chat_action");
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

    // When chatData arrives for a newly opened chat and selectedLeafId is null,
    // navigate to the deepest first-child leaf.
    useEffect(() => {
        if (!chatData || selectedLeafId !== null) return;
        const messages = chatData.messages;
        const rootNodes = getChildren(messages, null);
        if (rootNodes.length === 0) return;
        setSelectedLeafId(getFirstLeaf(messages, rootNodes[0].id));
    }, [chatData, selectedLeafId]);

    // Auto-navigate: when the selected leaf is a user-message node that gains
    // its first child (the streaming assistant reply), move the cursor to it.
    useEffect(() => {
        if (!chatData || !selectedLeafId) return;
        const msg = chatData.messages[selectedLeafId];
        if (!msg || msg.role !== "user") return;
        const children = getChildren(chatData.messages, selectedLeafId);
        if (children.length > 0) {
            setSelectedLeafId(children[0].id);
        }
    }, [chatData, selectedLeafId]);

    // Compute the ordered list of messages to display (root → selectedLeaf).
    const currentPath = useMemo(() => {
        if (!chatData || !selectedLeafId) return [];
        if (!chatData.messages[selectedLeafId]) return [];
        return getPath(chatData.messages, selectedLeafId);
    }, [chatData, selectedLeafId]);

    // Auto-scroll to bottom when the current path changes.
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [currentPath]);

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

    const handleNewChat = async () => {
        const newChatId = await newChatScript.runScript({ action: "new" });
        if (newChatId) {
            setChatIds((prev) => [...prev, newChatId]);
            setSelectedChatId(newChatId);
            setSelectedLeafId(null);
        }
    };

    const handleSelectChat = async (chatId: string) => {
        setSelectedChatId(chatId);
        setSelectedLeafId(null);
        // Poke the chat to load it into the state system
        await pokeScript.runScript({ action: "poke", chat_id: chatId });
    };

    const handleSendMessage = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!selectedChatId || !inputMessage.trim()) return;

        const messageToSend = inputMessage.trim();
        setInputMessage(""); // Clear input immediately

        // Add the user message as a child of the current leaf (or as the root
        // if there are no messages yet).
        const newUserMsgId = await addUserMessageScript.runScript({
            action: "add_user_message",
            chat_id: selectedChatId,
            parent_id: selectedLeafId, // null → first message in the chat
            content: messageToSend,
        });

        if (newUserMsgId) {
            // Move cursor to the user message; the auto-navigate effect will
            // advance it to the assistant reply once streaming begins.
            setSelectedLeafId(newUserMsgId);

            // Fire-and-forget — streaming content arrives via state SSE.
            generateScript.runScript({
                action: "generate",
                chat_id: selectedChatId,
                parent_message_id: newUserMsgId,
            });
        }
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
                            <button
                                onClick={() =>
                                    onSelectStateObject?.(
                                        `chat:${selectedChatId}`,
                                    )
                                }
                                className="mb-4 text-gray-400 hover:text-blue-400 text-xs font-mono transition-colors cursor-pointer hover:underline text-left"
                            >
                                Chat ID: {selectedChatId}
                            </button>
                            {chatData ? (
                                <div className="space-y-4">
                                    {currentPath.map((message) => (
                                        <MessageBubble
                                            key={message.id}
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
                                    disabled={addUserMessageScript.loading}
                                />
                                <button
                                    type="submit"
                                    disabled={
                                        !inputMessage.trim() ||
                                        addUserMessageScript.loading
                                    }
                                    className="px-6 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white rounded transition-colors"
                                >
                                    {addUserMessageScript.loading
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
