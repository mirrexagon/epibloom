import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useStateObjects } from "../../hooks/useStateObjects";
import { useChatActions } from "../../hooks/useChatActions";
import { ScriptError } from "../../hooks/useScriptRunner";
import { Chat, ChatStatus } from "../../types/api";
import { ChatView } from "./ChatView";
import { TraceDrawer } from "./TraceDrawer";
import { OpenTrace, chatMessageElementId, locateTrace } from "./chatUtils";

const SELECTED_CHAT_STORAGE_KEY = "epibloom.chat.selected";
const FLASH_DURATION_MS = 1200;
const DEFAULT_STATUS: ChatStatus = {
    generating: false,
    cancel_requested: false,
};

interface ErrorEntry {
    id: number;
    message: string;
    /** Full message (eg. with Lua stack traceback), if different. */
    details?: string;
}

export function ChatTab() {
    // ---- Data ----------

    // The index, to find chats.
    const indexObjectNames = useMemo(() => [""], []);
    const {
        objects: indexObjects,
        connected,
        error: connectionError,
    } = useStateObjects(indexObjectNames);

    // Chat IDs, newest first (IDs are UUIDv7, so they sort by creation time).
    // Joined into a string so the subscription below only changes when the set
    // of chats does, not whenever any other state object is created.
    const chatIdsKey = useMemo(() => {
        const names = (indexObjects[0]?.value as string[] | undefined) ?? [];
        return names
            .map((name) => name.match(/^chat:(.+):persist$/)?.[1])
            .filter((id): id is string => id !== undefined)
            .sort()
            .reverse()
            .join(",");
    }, [indexObjects]);

    // All chats and their statuses, on one connection. The list needs names
    // and generating status anyway, and the selected chat's data comes from
    // here too.
    const chatObjectNames = useMemo(
        () =>
            chatIdsKey === ""
                ? []
                : chatIdsKey
                      .split(",")
                      .flatMap((id) => [`chat:${id}:persist`, `chat:${id}`]),
        [chatIdsKey],
    );
    const { objects: chatObjects } = useStateObjects(chatObjectNames);

    const chats = useMemo(() => {
        const values = new Map(chatObjects.map((obj) => [obj.name, obj.value]));
        return chatIdsKey === ""
            ? []
            : chatIdsKey.split(",").flatMap((id) => {
                  const chat = values.get(`chat:${id}:persist`) as
                      Chat | null | undefined;
                  // Missing (still loading) or null (not actually a chat).
                  if (!chat) return [];
                  const status =
                      (values.get(`chat:${id}`) as
                          ChatStatus | null | undefined) ?? DEFAULT_STATUS;
                  return [{ chat, status }];
              });
    }, [chatObjects, chatIdsKey]);

    // ---- Selection ----------

    const [selectedChatId, setSelectedChatIdState] = useState<string | null>(
        () => localStorage.getItem(SELECTED_CHAT_STORAGE_KEY),
    );
    const [openTrace, setOpenTrace] = useState<OpenTrace | null>(null);

    const selectChat = useCallback((chatId: string | null) => {
        setSelectedChatIdState(chatId);
        setOpenTrace(null);
        if (chatId) {
            localStorage.setItem(SELECTED_CHAT_STORAGE_KEY, chatId);
        } else {
            localStorage.removeItem(SELECTED_CHAT_STORAGE_KEY);
        }
    }, []);

    const selected = chats.find(({ chat }) => chat.id === selectedChatId);
    const openTraceLocation =
        selected && openTrace?.chatId === selected.chat.id
            ? locateTrace(selected.chat, openTrace.trace)
            : null;

    // ---- Errors ----------

    const [errors, setErrors] = useState<ErrorEntry[]>([]);
    const nextErrorIdRef = useRef(0);
    const reportError = useCallback((error: unknown) => {
        console.error(error);
        const entry: ErrorEntry = {
            id: nextErrorIdRef.current++,
            message: error instanceof Error ? error.message : String(error),
            details:
                error instanceof ScriptError ? error.fullMessage : undefined,
        };
        setErrors((prev) => [...prev, entry]);
    }, []);

    const actions = useChatActions(reportError);

    // ---- Trace drawer ----------

    const [flashMessageIndex, setFlashMessageIndex] = useState<number | null>(
        null,
    );
    const flashTimerRef = useRef<ReturnType<typeof setTimeout>>();

    const jumpToMessage = useCallback((messageIndex: number) => {
        document
            .getElementById(chatMessageElementId(messageIndex))
            ?.scrollIntoView({ behavior: "smooth", block: "center" });
        setFlashMessageIndex(messageIndex);
        clearTimeout(flashTimerRef.current);
        flashTimerRef.current = setTimeout(
            () => setFlashMessageIndex(null),
            FLASH_DURATION_MS,
        );
    }, []);
    useEffect(() => () => clearTimeout(flashTimerRef.current), []);

    const showTrace = useCallback(
        (trace: OpenTrace) =>
            // Clicking "Trace" on the message whose trace is open closes it.
            setOpenTrace((current) =>
                current?.trace === trace.trace ? null : trace,
            ),
        [],
    );

    // Escape closes the trace drawer (unless typing somewhere).
    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent) => {
            const target = e.target as HTMLElement | null;
            if (target?.closest("input, textarea")) return;
            if (e.key === "Escape") setOpenTrace(null);
        };
        window.addEventListener("keydown", onKeyDown);
        return () => window.removeEventListener("keydown", onKeyDown);
    }, []);

    // ---- Render ----------

    return (
        <div className="h-full flex">
            {/* Chat list */}
            <div className="w-72 shrink-0 border-r border-neutral-800 flex flex-col">
                <div className="px-4 py-3 border-b border-neutral-800">
                    <div className="flex items-center justify-between">
                        <h2 className="text-lg font-semibold">Chats</h2>
                        <button
                            onClick={async () => {
                                const chatId = await actions.newChat();
                                if (chatId) selectChat(chatId);
                            }}
                            className="px-3 py-1 text-sm bg-neutral-200 text-black font-semibold hover:bg-white"
                        >
                            New chat
                        </button>
                    </div>
                    <div className="text-sm mt-1">
                        {connected ? (
                            <span className="text-green-400">● Connected</span>
                        ) : (
                            <span className="text-red-400">● Disconnected</span>
                        )}
                    </div>
                    {connectionError && (
                        <div className="text-sm text-red-400 mt-1">
                            {connectionError}
                        </div>
                    )}
                </div>
                <div className="flex-1 overflow-y-auto">
                    {chats.map(({ chat, status }) => (
                        <button
                            key={chat.id}
                            onClick={() => selectChat(chat.id)}
                            className={`w-full text-left px-4 py-2 transition-colors ${
                                selectedChatId === chat.id
                                    ? "bg-neutral-900 text-white"
                                    : "text-neutral-400 hover:bg-neutral-950 hover:text-neutral-200"
                            }`}
                        >
                            <div className="flex items-center gap-2">
                                <span className="truncate text-base">
                                    {chat.name}
                                </span>
                                {status.generating && (
                                    <span
                                        className="ml-auto shrink-0 w-2 h-2 rounded-full bg-green-400 animate-pulse"
                                        title="Generating"
                                    />
                                )}
                            </div>
                            <div className="text-sm text-neutral-600">
                                {chat.messages.length} message
                                {chat.messages.length === 1 ? "" : "s"} ·{" "}
                                {new Date(chat.modified).toLocaleString()}
                            </div>
                        </button>
                    ))}
                    {chats.length === 0 && (
                        <div className="p-4 text-center text-base text-neutral-500">
                            No chats yet
                        </div>
                    )}
                </div>
            </div>

            {/* Selected chat */}
            <div className="flex-1 min-w-0 flex flex-col overflow-hidden">
                {errors.length > 0 && (
                    <div className="border-b border-red-900 bg-red-950/40 max-h-48 overflow-y-auto">
                        {errors.map((entry) => (
                            <div
                                key={entry.id}
                                className="px-4 py-2 flex gap-3 text-sm"
                            >
                                <div className="flex-1 min-w-0 text-red-300">
                                    <div className="whitespace-pre-wrap break-words">
                                        {entry.message}
                                    </div>
                                    {entry.details &&
                                        entry.details !== entry.message && (
                                            <details className="mt-1 text-red-400/70">
                                                <summary className="cursor-pointer">
                                                    Details
                                                </summary>
                                                <pre className="mt-1 whitespace-pre-wrap break-words font-mono text-xs">
                                                    {entry.details}
                                                </pre>
                                            </details>
                                        )}
                                </div>
                                <button
                                    onClick={() =>
                                        setErrors((prev) =>
                                            prev.filter(
                                                (e) => e.id !== entry.id,
                                            ),
                                        )
                                    }
                                    aria-label="Dismiss error"
                                    className="self-start text-red-400 hover:text-white"
                                >
                                    ✕
                                </button>
                            </div>
                        ))}
                    </div>
                )}

                <div className="flex-1 overflow-hidden">
                    {selected ? (
                        <ChatView
                            key={selected.chat.id}
                            chat={selected.chat}
                            status={selected.status}
                            actions={actions}
                            openTrace={
                                openTrace?.chatId === selected.chat.id
                                    ? openTrace
                                    : null
                            }
                            flashMessageIndex={flashMessageIndex}
                            onShowTrace={showTrace}
                        />
                    ) : (
                        <div className="flex items-center justify-center h-full text-base text-neutral-500">
                            {selectedChatId &&
                            chatIdsKey.includes(selectedChatId)
                                ? "Loading…"
                                : "Select or create a chat"}
                        </div>
                    )}
                </div>
            </div>

            {/* Trace drawer */}
            {openTrace && (
                <div className="w-[40rem] max-w-[50%] shrink-0 border-l border-neutral-800">
                    <TraceDrawer
                        key={openTrace.trace}
                        traceName={openTrace.trace}
                        title={
                            openTraceLocation
                                ? `Trace · message ${openTraceLocation.messageIndex}, swipe ${openTraceLocation.alternateIndex}`
                                : "Trace · message deleted"
                        }
                        onJumpToMessage={
                            openTraceLocation
                                ? () =>
                                      jumpToMessage(
                                          openTraceLocation.messageIndex,
                                      )
                                : undefined
                        }
                        onClose={() => setOpenTrace(null)}
                    />
                </div>
            )}
        </div>
    );
}
