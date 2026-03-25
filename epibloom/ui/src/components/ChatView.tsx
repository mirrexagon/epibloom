import { useState, useEffect, useLayoutEffect, useMemo, useRef, useCallback, memo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import { useScript } from "../hooks/useScript";
import { useStateObjects } from "../hooks/useStateObjects";
import { Chat, ChatMessage } from "../types/api";
import { ChatFlowchart, ChatFlowchartHandle } from "./ChatFlowchart";

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

    // TODO: Check if the sort here is actually sorting by timestamp, or if only
    // a subset of the bits of the UUID should be considered.
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
        currentId = node.parent_id ?? null;
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
// SiblingNavigator
// ---------------------------------------------------------------------------

interface SiblingNavigatorProps {
    messages: Record<string, ChatMessage>;
    node: ChatMessage;
    isUser: boolean;
    /** Called with the pivot node and the first-leaf id of the newly selected sibling. */
    onNavigate: (node: ChatMessage, leafId: string) => void;
}

const SiblingNavigator = memo(function SiblingNavigator({
    messages,
    node,
    isUser,
    onNavigate,
}: SiblingNavigatorProps) {
    const siblings = getChildren(messages, node.parent_id ?? null);
    if (siblings.length <= 1) return null;

    const currentIndex = siblings.findIndex((s) => s.id === node.id);

    const goTo = (idx: number) => {
        if (idx >= 0 && idx < siblings.length) {
            onNavigate(node, getFirstLeaf(messages, siblings[idx].id));
        }
    };

    return (
        <div
            className={`flex items-center gap-1.5 text-xs text-gray-400 mb-1 ${
                isUser ? "justify-end" : "justify-start"
            }`}
        >
            <button
                onClick={(e) => {
                    e.stopPropagation();
                    goTo(currentIndex - 1);
                }}
                disabled={currentIndex === 0}
                className="px-1 hover:text-gray-200 disabled:opacity-30 disabled:cursor-not-allowed"
            >
                ←
            </button>
            <span>
                {currentIndex + 1} / {siblings.length}
            </span>
            <button
                onClick={(e) => {
                    e.stopPropagation();
                    goTo(currentIndex + 1);
                }}
                disabled={currentIndex === siblings.length - 1}
                className="px-1 hover:text-gray-200 disabled:opacity-30 disabled:cursor-not-allowed"
            >
                →
            </button>
        </div>
    );
});

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
                    code: (codeProps: any) => {
                        const {
                            inline,
                            className: cls,
                            children,
                            ...props
                        } = codeProps;
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
    /** Called with the message id when the bubble is clicked. */
    onBubbleClick?: (messageId: string) => void;
    /** Highlight this bubble as the active branch point. */
    isSelected?: boolean;
}

const MessageBubble = memo(function MessageBubble({
    message,
    onSelectTrace,
    onBubbleClick,
    isSelected = false,
}: MessageBubbleProps) {
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
                onClick={() => onBubbleClick?.(message.id)}
                className={`max-w-[70%] rounded-lg px-4 py-2 cursor-pointer transition-all ${
                    isUser
                        ? "bg-blue-600 text-white"
                        : isSystem
                          ? "bg-gray-700 text-gray-300 text-sm italic"
                          : "bg-gray-700 text-gray-100"
                } ${isSelected ? "ring-2 ring-yellow-400" : "hover:ring-1 hover:ring-gray-500"}`}
            >
                {thinkingText && (
                    <div className="mb-2">
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                setShowThinking(!showThinking);
                            }}
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
                        onClick={(e) => {
                            e.stopPropagation();
                            onSelectTrace?.(message.trace_id!);
                        }}
                        className="text-xs opacity-50 hover:opacity-100 mt-1 font-mono transition-opacity hover:underline cursor-pointer text-left"
                    >
                        trace: {message.trace_id}
                    </button>
                )}
            </div>
        </div>
    );
});

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
    const [showFlowchart, setShowFlowchart] = useState(true);
    const [flowchartWidth, setFlowchartWidth] = useState(320);

    // selectedLeafId: the "cursor" — the node currently at the bottom of the
    // viewed path.  In branch mode this can be a middle node.
    const [selectedLeafId, setSelectedLeafId] = useState<string | null>(null);
    // committedLeafId: the "real" deepest leaf of the active thread.  Branch
    // mode is active when selectedLeafId !== committedLeafId.
    const [committedLeafId, setCommittedLeafId] = useState<string | null>(null);

    const [inputMessage, setInputMessage] = useState("");

    // Ref to the flowchart so we can imperatively re-centre it on navigation.
    const flowchartRef = useRef<ChatFlowchartHandle>(null);

    // Map from message id → its DOM container element, for scroll-to.
    const messageRefs = useRef<Map<string, HTMLElement>>(new Map());

    // When set, the next scroll effect will scroll to this message id.
    const scrollToMessageId = useRef<string | null>(null);
    // Incremented every time handleSelectNode is called so the scroll effect
    // fires even when fullPath hasn't changed (same-branch ancestor click).
    const [scrollVersion, setScrollVersion] = useState(0);

    // Ref to the scrollable messages container, used for scroll-anchor maths.
    const scrollContainerRef = useRef<HTMLDivElement>(null);

    // When set, the next useLayoutEffect will adjust scrollTop so the given
    // message stays at the recorded pixel offset from the container top.
    const pendingScrollAnchor = useRef<{
        messageId: string;
        offsetFromContainerTop: number;
    } | null>(null);

    // When true, the normal scroll-to-bottom useEffect skips once (used when
    // sibling navigation is handling scroll via the anchor instead).
    const suppressDefaultScroll = useRef(false);

    // pendingNewChildOf: when set, we are waiting for a *new* child of this
    // node to appear in chatData (e.g. after a generate call).  The ref stores
    // the node id and the set of child ids that existed *before* the call so we
    // can detect the freshly-added child.
    const pendingNewChildOf = useRef<{
        nodeId: string;
        existingChildIds: Set<string>;
    } | null>(null);

    // Script hooks for chat actions
    // TODO: Each message generation should use its own script context, so multiple branches can be generating at once.
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

    // Helper: set both cursor and committed leaf atomically (used for primary
    // navigation flows — initial load, send, sibling nav, regenerate).
    const setLeafIds = useCallback((id: string | null) => {
        setSelectedLeafId(id);
        setCommittedLeafId(id);
    }, []);

    // When chatData arrives for a newly opened chat and selectedLeafId is null,
    // navigate to the deepest first-child leaf.
    useEffect(() => {
        if (!chatData || selectedLeafId !== null) return;
        const messages = chatData.messages;
        const rootNodes = getChildren(messages, null);
        if (rootNodes.length === 0) return;
        setLeafIds(getFirstLeaf(messages, rootNodes[0].id));
    }, [chatData, selectedLeafId, setLeafIds]);

    // Watch for a newly-added child of `pendingNewChildOf`.  When found,
    // advance the cursor (and committed leaf) to that new child.
    useEffect(() => {
        if (!chatData || !pendingNewChildOf.current) return;
        const { nodeId, existingChildIds } = pendingNewChildOf.current;
        const children = getChildren(chatData.messages, nodeId);
        const newChild = children.find((c) => !existingChildIds.has(c.id));
        if (newChild) {
            setLeafIds(newChild.id);
            pendingNewChildOf.current = null;
        }
    }, [chatData, setLeafIds]);

    // Escape key: cancel branch mode.
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape" && selectedLeafId !== committedLeafId) {
                suppressDefaultScroll.current = true;
                setSelectedLeafId(committedLeafId);
            }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [selectedLeafId, committedLeafId]);

    // Compute the ordered list of messages to display (root → selectedLeaf).
    const currentPath = useMemo(() => {
        if (!chatData || !selectedLeafId) return [];
        if (!chatData.messages[selectedLeafId]) return [];
        return getPath(chatData.messages, selectedLeafId);
    }, [chatData, selectedLeafId]);

    // In branch mode the path is extended to the committed leaf so that dimmed
    // bubbles below the branch point are still visible.
    const fullPath = useMemo(() => {
        if (!chatData || !committedLeafId) return currentPath;
        if (selectedLeafId === committedLeafId) return currentPath;
        if (!chatData.messages[committedLeafId]) return currentPath;
        return getPath(chatData.messages, committedLeafId);
    }, [chatData, committedLeafId, currentPath, selectedLeafId]);

    // Index of the selectedLeafId in fullPath — everything after it is dimmed.
    const branchCutIndex = useMemo(() => {
        if (!selectedLeafId) return fullPath.length - 1;
        return fullPath.findIndex((m) => m.id === selectedLeafId);
    }, [fullPath, selectedLeafId]);

    const isBranchMode =
        selectedLeafId !== null &&
        committedLeafId !== null &&
        selectedLeafId !== committedLeafId;

    // Scroll-anchor correction — runs synchronously before the browser paints
    // so the user never sees the scroll jump when switching sibling branches.
    // Only active when pendingScrollAnchor has been set by handleSiblingNavigate.
    useLayoutEffect(() => {
        const anchor = pendingScrollAnchor.current;
        if (!anchor) return;
        pendingScrollAnchor.current = null;

        const container = scrollContainerRef.current;
        const el = messageRefs.current.get(anchor.messageId);
        if (!el || !container) return;

        const containerRect = container.getBoundingClientRect();
        const elRect = el.getBoundingClientRect();
        const currentOffset = elRect.top - containerRect.top;
        // Adjust scrollTop by however much the element moved from where it was.
        container.scrollTop += currentOffset - anchor.offsetFromContainerTop;
    }, [fullPath]);

    // Unified scroll effect.
    //
    // When a tree node was clicked (scrollToMessageId is set), scroll to that
    // specific message — even if fullPath didn't change (ancestor of current
    // selection in a linear section).  Otherwise scroll to the bottom as usual.
    //
    // Depends on both fullPath (content changes) and scrollVersion (tree clicks
    // that don't change the path still need to trigger a scroll).
    useEffect(() => {
        // Sibling navigation handles its own scroll via the layout-effect anchor;
        // skip the default behaviour for that one render cycle.
        if (suppressDefaultScroll.current) {
            suppressDefaultScroll.current = false;
            return;
        }
        const target = scrollToMessageId.current;
        if (target) {
            scrollToMessageId.current = null;
            const el = messageRefs.current.get(target);
            if (el) {
                el.scrollIntoView({ behavior: "smooth", block: "center" });
                return;
            }
            // Target not in the current path — fall through to scroll-to-bottom.
        }
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [scrollVersion, fullPath]);

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
            setLeafIds(null);
        }
    };

    const handleSelectChat = async (chatId: string) => {
        setSelectedChatId(chatId);
        setLeafIds(null);
        // Poke the chat to load it into the state system
        await pokeScript.runScript({ action: "poke", chat_id: chatId });
    };

    // Called by SiblingNavigator when the user clicks ← or →.
    // Before changing the leaf we record where the parent message currently sits
    // so the useLayoutEffect can restore that position after the re-render.
    const handleSiblingNavigate = useCallback(
        (node: ChatMessage, leafId: string) => {
            // The "anchor" is the parent message — the last shared message above
            // the branch point that stays visible in both siblings.
            const anchorId = node.parent_id ?? node.id;
            const container = scrollContainerRef.current;
            const el = messageRefs.current.get(anchorId);
            if (el && container) {
                const containerRect = container.getBoundingClientRect();
                const elRect = el.getBoundingClientRect();
                pendingScrollAnchor.current = {
                    messageId: anchorId,
                    offsetFromContainerTop: elRect.top - containerRect.top,
                };
                suppressDefaultScroll.current = true;
            }

            setLeafIds(leafId);
            flowchartRef.current?.centerOnNode(leafId);
        },
        [setLeafIds],
    );

    /** Click on a bubble: set branch point, or cancel if already there. */
    const handleBubbleClick = useCallback(
        (messageId: string) => {
            // Don't scroll — the user is just clicking in place.
            suppressDefaultScroll.current = true;
            if (selectedLeafId === messageId) {
                // Same bubble clicked again → cancel branch mode
                setSelectedLeafId(committedLeafId);
            } else {
                setSelectedLeafId(messageId);
            }
        },
        [selectedLeafId, committedLeafId],
    );

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
            // Move cursor and committed leaf to the new user message.
            setLeafIds(newUserMsgId);

            // Prepare to navigate to the assistant reply when it arrives.
            pendingNewChildOf.current = {
                nodeId: newUserMsgId,
                existingChildIds: new Set<string>(),
            };

            // Fire-and-forget — streaming content arrives via state SSE.
            generateScript.runScript({
                action: "generate",
                chat_id: selectedChatId,
                parent_message_id: newUserMsgId,
            });
        }
    };

    /** Regenerate: create a new assistant response.
     *  - If cursor is on a user message, generate a child of it.
     *  - If cursor is on an assistant message, generate a sibling (child of its parent).
     */
    const handleRegenerate = async () => {
        if (!selectedChatId || !selectedLeafId || !chatData) return;
        const msg = chatData.messages[selectedLeafId];
        if (!msg) return;

        let targetUserMsgId: string;
        if (msg.role === "user") {
            targetUserMsgId = selectedLeafId;
        } else if (msg.role === "assistant" && msg.parent_id) {
            targetUserMsgId = msg.parent_id;
        } else {
            return;
        }

        // Snapshot existing children so we can detect the new one.
        const existingChildren = getChildren(
            chatData.messages,
            targetUserMsgId,
        );
        pendingNewChildOf.current = {
            nodeId: targetUserMsgId,
            existingChildIds: new Set(existingChildren.map((c) => c.id)),
        };

        // Fire-and-forget.
        generateScript.runScript({
            action: "generate",
            chat_id: selectedChatId,
            parent_message_id: targetUserMsgId,
        });
    };

    // ---------------------------------------------------------------------------
    // Flowchart node selection — navigate to the first-leaf under the clicked node
    // and scroll the chat to the clicked message.
    // ---------------------------------------------------------------------------
    const handleSelectNode = useCallback(
        (nodeId: string) => {
            if (!chatData) return;
            setLeafIds(getFirstLeaf(chatData.messages, nodeId));
            // Scroll the chat to the clicked node.  scrollVersion is incremented
            // so the effect fires even when fullPath doesn't change (e.g. clicking
            // an ancestor in the same linear branch).
            scrollToMessageId.current = nodeId;
            setScrollVersion((v) => v + 1);
        },
        [chatData, setLeafIds],
    );

    // ---------------------------------------------------------------------------
    // Input bar derived state
    // ---------------------------------------------------------------------------
    const selectedMsg = selectedLeafId
        ? chatData?.messages[selectedLeafId]
        : null;
    const selectedMsgRole = selectedMsg?.role ?? null;
    const textInputEnabled = selectedMsgRole !== "user";
    // Show Regenerate when cursor is on an assistant or user message node.
    const showRegenerate = selectedLeafId !== null && selectedMsg !== undefined;

    // ---------------------------------------------------------------------------
    // Render
    // ---------------------------------------------------------------------------
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

            {/* Chat column */}
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
                        {/* Chat header */}
                        <div className="flex items-center justify-end px-4 py-1.5 border-b border-gray-700 bg-gray-800">
                            <button
                                onClick={() => setShowFlowchart((v) => !v)}
                                title={
                                    showFlowchart
                                        ? "Hide tree view"
                                        : "Show tree view"
                                }
                                className={`text-sm px-2 py-0.5 rounded transition-colors ${
                                    showFlowchart
                                        ? "bg-blue-700 text-white"
                                        : "bg-gray-700 text-gray-300 hover:bg-gray-600"
                                }`}
                            >
                                ⎇ Tree
                            </button>
                        </div>

                        {/* Messages area */}
                        <div ref={scrollContainerRef} className="flex-1 overflow-y-auto p-4">
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
                                    {fullPath.map((message, index) => {
                                        const isDimmed =
                                            branchCutIndex >= 0 &&
                                            index > branchCutIndex;
                                        const isSelectedBubble =
                                            isBranchMode &&
                                            message.id === selectedLeafId;
                                        return (
                                            <div
                                                key={message.id}
                                                ref={(el) => {
                                                    if (el) messageRefs.current.set(message.id, el);
                                                    else messageRefs.current.delete(message.id);
                                                }}
                                                className={`transition-opacity ${isDimmed ? "opacity-50" : ""}`}
                                            >
                                                <SiblingNavigator
                                                    messages={chatData.messages}
                                                    node={message}
                                                    isUser={
                                                        message.role === "user"
                                                    }
                                                    onNavigate={handleSiblingNavigate}
                                                />
                                                <MessageBubble
                                                    message={message}
                                                    onSelectTrace={
                                                        onSelectTrace
                                                    }
                                                    onBubbleClick={handleBubbleClick}
                                                    isSelected={
                                                        isSelectedBubble
                                                    }
                                                />
                                            </div>
                                        );
                                    })}
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
                            {isBranchMode && (
                                <div className="mb-2 text-xs text-yellow-400 flex items-center gap-1">
                                    <span>⎇ Branching from here</span>
                                    <button
                                        onClick={() => {
                                            suppressDefaultScroll.current = true;
                                            setSelectedLeafId(committedLeafId);
                                        }}
                                        className="ml-2 underline hover:text-yellow-200"
                                    >
                                        Cancel
                                    </button>
                                </div>
                            )}
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
                                    placeholder={
                                        textInputEnabled
                                            ? "Type a message..."
                                            : "Select a response above to continue…"
                                    }
                                    className="flex-1 px-4 py-2 bg-gray-700 text-white rounded border border-gray-600 focus:outline-none focus:border-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                                    disabled={
                                        !textInputEnabled ||
                                        addUserMessageScript.loading
                                    }
                                />
                                {textInputEnabled && (
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
                                )}
                                {showRegenerate && (
                                    <button
                                        type="button"
                                        onClick={handleRegenerate}
                                        disabled={generateScript.loading}
                                        className="px-4 py-2 bg-gray-600 hover:bg-gray-500 disabled:bg-gray-700 disabled:cursor-not-allowed text-white rounded transition-colors whitespace-nowrap"
                                        title="Generate another response"
                                    >
                                        ↺ Regenerate
                                    </button>
                                )}
                            </form>
                        </div>
                    </>
                )}
            </div>

            {/* Flowchart panel */}
            {showFlowchart && selectedChatId && chatData && (
                <>
                    {/* Resize handle */}
                    <div
                        className="w-1 flex-shrink-0 cursor-col-resize bg-gray-700 hover:bg-blue-500 active:bg-blue-400 transition-colors"
                        onMouseDown={(e) => {
                            e.preventDefault();
                            const startX = e.clientX;
                            const startWidth = flowchartWidth;
                            const onMove = (ev: MouseEvent) => {
                                const delta = startX - ev.clientX;
                                setFlowchartWidth(
                                    Math.max(
                                        200,
                                        Math.min(800, startWidth + delta),
                                    ),
                                );
                            };
                            const onUp = () => {
                                window.removeEventListener("mousemove", onMove);
                                window.removeEventListener("mouseup", onUp);
                            };
                            window.addEventListener("mousemove", onMove);
                            window.addEventListener("mouseup", onUp);
                        }}
                    />
                    <div
                        className="flex-shrink-0 border-l border-gray-700 bg-gray-900 flex flex-col"
                        style={{ width: flowchartWidth }}
                    >
                        <div className="px-3 py-1.5 border-b border-gray-700 text-xs text-gray-400 font-medium tracking-wide">
                            Tree
                        </div>
                        <div className="flex-1 overflow-hidden">
                            <ChatFlowchart
                                ref={flowchartRef}
                                messages={chatData.messages}
                                selectedLeafId={selectedLeafId}
                                onSelectNode={handleSelectNode}
                            />
                        </div>
                    </div>
                </>
            )}
        </div>
    );
}
