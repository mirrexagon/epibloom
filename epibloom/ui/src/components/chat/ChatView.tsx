import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { Chat, ChatStatus } from "../../types/api";
import { ChatActions } from "../../hooks/useChatActions";
import { ChatMessageHandlers, ChatMessageView } from "./ChatMessageView";
import { ChatComposer } from "./ChatComposer";
import { ChatSettings } from "./ChatSettings";
import { OpenTrace } from "./chatUtils";

/** How close (px) to the bottom counts as "at the bottom" for auto-scrolling. */
const STICK_TO_BOTTOM_THRESHOLD = 80;

/** A single chat: header, settings, messages, and composer. Remount (via `key`) when switching chats. */
export function ChatView({
    chat,
    status,
    actions,
    openTrace,
    flashMessageIndex,
    onShowTrace,
}: {
    chat: Chat;
    status: ChatStatus;
    actions: ChatActions;
    openTrace: OpenTrace | null;
    flashMessageIndex: number | null;
    onShowTrace: (trace: OpenTrace) => void;
}) {
    const [settingsOpen, setSettingsOpen] = useState(false);

    const busy = status.generating;
    const lastSlot = chat.messages[chat.messages.length - 1];
    const lastRole = lastSlot?.alternates[lastSlot.selected - 1]?.role;

    // Auto-scroll to the bottom as messages arrive, unless the user has
    // scrolled up.
    const scrollRef = useRef<HTMLDivElement>(null);
    const stickToBottomRef = useRef(true);
    useLayoutEffect(() => {
        const el = scrollRef.current;
        if (el && stickToBottomRef.current) {
            el.scrollTop = el.scrollHeight;
        }
    }, [chat.messages]);

    const handlers: ChatMessageHandlers = useMemo(
        () => ({
            selectAlternate: (m, a) => actions.selectAlternate(chat.id, m, a),
            save: (m, a, text, note) =>
                actions.setMessageContent(chat.id, m, a, text, note),
            delete: (m, a) => actions.deleteAlternate(chat.id, m, a),
            showTrace: (trace) => onShowTrace({ chatId: chat.id, trace }),
            newSwipe: () => {
                stickToBottomRef.current = true;
                actions.generate(chat.id);
            },
        }),
        [actions, chat.id, onShowTrace],
    );

    return (
        <div className="h-full flex flex-col overflow-hidden">
            <div className="px-4 py-3 border-b border-neutral-800 flex items-center gap-4">
                <div className="min-w-0">
                    <h2 className="text-lg font-semibold truncate">
                        {chat.name}
                    </h2>
                    <div className="text-sm text-neutral-500 truncate">
                        {chat.model} · max {chat.max_tokens} tokens ·{" "}
                        {chat.messages.length} message
                        {chat.messages.length === 1 ? "" : "s"}
                    </div>
                </div>
                <button
                    onClick={() => setSettingsOpen((open) => !open)}
                    className={`ml-auto px-3 py-1.5 text-sm border transition-colors ${
                        settingsOpen
                            ? "border-white text-white bg-neutral-800"
                            : "border-neutral-700 text-neutral-300 hover:bg-neutral-900"
                    }`}
                >
                    Settings
                </button>
            </div>

            {settingsOpen && (
                <ChatSettings
                    chat={chat}
                    busy={busy}
                    actions={actions}
                    onClose={() => setSettingsOpen(false)}
                />
            )}

            <div
                ref={scrollRef}
                onScroll={(e) => {
                    const el = e.currentTarget;
                    stickToBottomRef.current =
                        el.scrollHeight - el.scrollTop - el.clientHeight <
                        STICK_TO_BOTTOM_THRESHOLD;
                }}
                className="flex-1 overflow-y-auto p-4 space-y-3"
            >
                {chat.system && (
                    <button
                        onClick={() => setSettingsOpen(true)}
                        title="Edit system prompt"
                        className="w-full text-left border border-neutral-800 p-3 hover:border-neutral-600"
                    >
                        <div className="text-sm font-semibold uppercase tracking-wider text-neutral-400 mb-2">
                            System
                        </div>
                        <div className="text-base text-neutral-400 whitespace-pre-wrap break-words line-clamp-4">
                            {chat.system}
                        </div>
                    </button>
                )}

                {chat.messages.map((slot, i) => {
                    const messageIndex = i + 1;
                    return (
                        <ChatMessageView
                            key={messageIndex}
                            slot={slot}
                            messageIndex={messageIndex}
                            isLast={messageIndex === chat.messages.length}
                            generatingAlternate={
                                busy && status.message_index === messageIndex
                                    ? (status.alternate_index ?? null)
                                    : null
                            }
                            chatBusy={busy}
                            traceOpen={
                                openTrace !== null &&
                                slot.alternates[slot.selected - 1]?.trace ===
                                    openTrace.trace
                            }
                            flash={flashMessageIndex === messageIndex}
                            handlers={handlers}
                        />
                    );
                })}

                {chat.messages.length === 0 && (
                    <div className="py-12 text-center text-base text-neutral-500">
                        No messages yet. Write one below to get started.
                    </div>
                )}
            </div>

            <ChatComposer
                busy={busy}
                cancelRequested={status.cancel_requested}
                lastRole={lastRole}
                onSend={async (text) => {
                    stickToBottomRef.current = true;
                    if (!(await actions.appendMessage(chat.id, "user", text)))
                        return false;
                    actions.generate(chat.id);
                    return true;
                }}
                onAdd={(text, role) => {
                    stickToBottomRef.current = true;
                    return actions.appendMessage(chat.id, role, text);
                }}
                onGenerate={() => {
                    stickToBottomRef.current = true;
                    actions.generate(chat.id);
                }}
                onStop={() => actions.cancelGenerate(chat.id)}
            />
        </div>
    );
}
