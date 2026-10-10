import { useState } from "react";
import { ChatMessageSlot } from "../../types/api";
import { contentToText } from "../../hooks/useChatActions";
import { roleStyle } from "../roleStyles";
import { chatMessageElementId } from "./chatUtils";

export interface ChatMessageHandlers {
    selectAlternate(messageIndex: number, alternateIndex: number): void;
    save(
        messageIndex: number,
        alternateIndex: number,
        text: string,
        note: string,
    ): Promise<boolean>;
    delete(messageIndex: number, alternateIndex: number): void;
    showTrace(trace: string): void;
    /** Generate a new alternate for the last message. */
    newSwipe(): void;
}

export function ChatMessageView({
    slot,
    messageIndex,
    isLast,
    generatingAlternate,
    chatBusy,
    traceOpen,
    flash,
    handlers,
}: {
    slot: ChatMessageSlot;
    /** 1-based. */
    messageIndex: number;
    isLast: boolean;
    /** If an alternate of this slot is currently being generated, its 1-based index. */
    generatingAlternate: number | null;
    /** Whether the chat is generating (and so can't be modified). */
    chatBusy: boolean;
    /** Whether the trace drawer is showing the trace of the selected alternate. */
    traceOpen: boolean;
    /** Briefly emphasise this message (eg. after jumping to it). */
    flash: boolean;
    handlers: ChatMessageHandlers;
}) {
    const alternateIndex = slot.selected;
    const message = slot.alternates[alternateIndex - 1];
    const alternateCount = slot.alternates.length;

    const [editing, setEditing] = useState(false);
    const [draftText, setDraftText] = useState("");
    const [draftNote, setDraftNote] = useState("");

    if (!message) return null;

    const style = roleStyle(message.role);
    const text = contentToText(message.content);
    const isGenerating = generatingAlternate === alternateIndex;
    const canSwipe = isLast && message.role === "assistant";

    const startEditing = () => {
        setDraftText(text);
        setDraftNote(message.note);
        setEditing(true);
    };

    const save = async () => {
        if (
            await handlers.save(
                messageIndex,
                alternateIndex,
                draftText,
                draftNote,
            )
        ) {
            setEditing(false);
        }
    };

    const borderClass = flash
        ? "border-white"
        : traceOpen
          ? style.highlightBorderClass
          : style.borderClass;

    return (
        <div
            id={chatMessageElementId(messageIndex)}
            className={`group border p-3 transition-colors ${borderClass} ${
                traceOpen ? "bg-neutral-950" : ""
            }`}
        >
            <div className="flex items-center gap-3 mb-2 text-sm">
                <span
                    className={`font-semibold uppercase tracking-wider ${style.labelClass}`}
                    title={`Created ${new Date(message.created).toLocaleString()}\nModified ${new Date(message.modified).toLocaleString()}`}
                >
                    {style.label}
                </span>

                {(alternateCount > 1 || canSwipe) && (
                    <span className="flex items-center gap-1 text-neutral-400">
                        <IconButton
                            label="Previous swipe"
                            disabled={chatBusy || alternateIndex <= 1}
                            onClick={() =>
                                handlers.selectAlternate(
                                    messageIndex,
                                    alternateIndex - 1,
                                )
                            }
                        >
                            ‹
                        </IconButton>
                        <span className="tabular-nums">
                            {alternateIndex}/{alternateCount}
                        </span>
                        <IconButton
                            label="Next swipe"
                            disabled={
                                chatBusy || alternateIndex >= alternateCount
                            }
                            onClick={() =>
                                handlers.selectAlternate(
                                    messageIndex,
                                    alternateIndex + 1,
                                )
                            }
                        >
                            ›
                        </IconButton>
                        {canSwipe && (
                            <IconButton
                                label="Generate a new swipe"
                                disabled={chatBusy}
                                onClick={handlers.newSwipe}
                            >
                                ↻
                            </IconButton>
                        )}
                    </span>
                )}

                {isGenerating && (
                    <span className="text-neutral-400 animate-pulse">
                        Generating…
                    </span>
                )}

                <span className="ml-auto flex items-center gap-3 text-neutral-500">
                    {message.trace && (
                        <TextButton
                            active={traceOpen}
                            onClick={() => handlers.showTrace(message.trace!)}
                        >
                            Trace
                        </TextButton>
                    )}
                    {!editing && (
                        <TextButton disabled={chatBusy} onClick={startEditing}>
                            Edit
                        </TextButton>
                    )}
                    <TextButton
                        disabled={chatBusy}
                        onClick={() => {
                            const what =
                                alternateCount > 1
                                    ? "this swipe"
                                    : "this message";
                            if (window.confirm(`Delete ${what}?`)) {
                                handlers.delete(messageIndex, alternateIndex);
                            }
                        }}
                    >
                        Delete
                    </TextButton>
                </span>
            </div>

            {editing ? (
                <div className="space-y-2">
                    <textarea
                        autoFocus
                        value={draftText}
                        onChange={(e) => setDraftText(e.target.value)}
                        onKeyDown={(e) => {
                            if (e.key === "Escape") setEditing(false);
                            if (e.key === "Enter" && (e.ctrlKey || e.metaKey))
                                save();
                        }}
                        rows={Math.min(
                            20,
                            Math.max(3, draftText.split("\n").length + 1),
                        )}
                        className="w-full bg-neutral-950 border border-neutral-700 p-2 text-base text-neutral-100 focus:outline-none focus:border-neutral-500"
                    />
                    <input
                        value={draftNote}
                        onChange={(e) => setDraftNote(e.target.value)}
                        placeholder="Note (not sent to the LLM)"
                        className="w-full bg-neutral-950 border border-neutral-700 px-2 py-1 text-sm text-neutral-300 focus:outline-none focus:border-neutral-500"
                    />
                    <div className="flex gap-2 text-sm">
                        <button
                            onClick={save}
                            disabled={chatBusy}
                            className="px-3 py-1 bg-neutral-200 text-black hover:bg-white disabled:opacity-40"
                        >
                            Save
                        </button>
                        <button
                            onClick={() => setEditing(false)}
                            className="px-3 py-1 border border-neutral-700 text-neutral-300 hover:bg-neutral-900"
                        >
                            Cancel
                        </button>
                        <span className="self-center text-neutral-600">
                            Ctrl+Enter to save, Escape to cancel
                        </span>
                    </div>
                </div>
            ) : (
                <>
                    <div className="text-base text-neutral-100 whitespace-pre-wrap break-words">
                        {text}
                        {isGenerating && (
                            <span className="inline-block w-2 h-4 ml-0.5 align-middle bg-neutral-400 animate-pulse" />
                        )}
                        {!text && !isGenerating && (
                            <span className="text-neutral-600 italic">
                                (empty)
                            </span>
                        )}
                    </div>
                    {message.note && (
                        <div className="mt-2 text-sm text-neutral-400 italic">
                            Note: {message.note}
                        </div>
                    )}
                </>
            )}

            {message.error && (
                <div className="mt-2 border-l-2 border-red-400 pl-3 text-sm text-red-300 whitespace-pre-wrap font-mono break-words">
                    {message.error}
                </div>
            )}
        </div>
    );
}

function IconButton({
    label,
    disabled,
    onClick,
    children,
}: {
    label: string;
    disabled?: boolean;
    onClick: () => void;
    children: React.ReactNode;
}) {
    return (
        <button
            title={label}
            aria-label={label}
            disabled={disabled}
            onClick={onClick}
            className="px-1.5 text-base leading-none hover:text-white disabled:opacity-30 disabled:hover:text-neutral-400"
        >
            {children}
        </button>
    );
}

function TextButton({
    active,
    disabled,
    onClick,
    children,
}: {
    active?: boolean;
    disabled?: boolean;
    onClick: () => void;
    children: React.ReactNode;
}) {
    return (
        <button
            disabled={disabled}
            onClick={onClick}
            className={`hover:text-neutral-200 disabled:opacity-30 disabled:hover:text-neutral-500 ${
                active ? "text-white underline underline-offset-4" : ""
            }`}
        >
            {children}
        </button>
    );
}
