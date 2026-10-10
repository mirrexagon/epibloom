import { useState } from "react";
import { ChatRole } from "../../types/api";

export function ChatComposer({
    busy,
    cancelRequested,
    lastRole,
    onSend,
    onAdd,
    onGenerate,
    onStop,
}: {
    /** Whether the chat is generating. */
    busy: boolean;
    cancelRequested: boolean;
    /** Role of the last message's selected alternate, if any. */
    lastRole: ChatRole | undefined;
    /** Append a user message and generate a response. */
    onSend: (text: string) => Promise<boolean>;
    /** Append a message without generating. */
    onAdd: (text: string, role: ChatRole) => Promise<boolean>;
    /** Generate a response (or a new swipe, if the last message is from the assistant). */
    onGenerate: () => void;
    onStop: () => void;
}) {
    const [text, setText] = useState("");
    const [submitting, setSubmitting] = useState(false);

    const hasText = text.trim() !== "";
    const disabled = busy || submitting;

    const primaryLabel = hasText
        ? "Send"
        : lastRole === "assistant"
          ? "New swipe"
          : "Generate";
    const primaryDisabled = disabled || (!hasText && lastRole === undefined);

    const submit = async (action: () => Promise<boolean>) => {
        setSubmitting(true);
        try {
            if (await action()) setText("");
        } finally {
            setSubmitting(false);
        }
    };

    const primary = () => {
        if (primaryDisabled) return;
        if (hasText) {
            submit(() => onSend(text));
        } else {
            onGenerate();
        }
    };

    return (
        <div className="border-t border-neutral-800 p-3 space-y-2">
            <textarea
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                    if (
                        e.key === "Enter" &&
                        !e.shiftKey &&
                        !e.nativeEvent.isComposing
                    ) {
                        e.preventDefault();
                        primary();
                    }
                }}
                placeholder="Message… (Enter to send, Shift+Enter for a new line; send with no text to generate)"
                rows={Math.min(12, Math.max(3, text.split("\n").length))}
                className="w-full bg-neutral-950 border border-neutral-700 p-2 text-base text-neutral-100 placeholder:text-neutral-600 focus:outline-none focus:border-neutral-500 resize-none"
            />
            <div className="flex items-center gap-2 text-sm">
                <button
                    disabled={disabled || !hasText}
                    onClick={() => submit(() => onAdd(text, "user"))}
                    className="px-3 py-1.5 border border-neutral-700 text-neutral-300 hover:bg-neutral-900 disabled:opacity-40 disabled:hover:bg-transparent"
                    title="Append as a user message without generating a response"
                >
                    Add as user
                </button>
                <button
                    disabled={disabled || !hasText}
                    onClick={() => submit(() => onAdd(text, "assistant"))}
                    className="px-3 py-1.5 border border-neutral-700 text-neutral-300 hover:bg-neutral-900 disabled:opacity-40 disabled:hover:bg-transparent"
                    title="Append as an assistant message"
                >
                    Add as assistant
                </button>
                <div className="ml-auto">
                    {busy ? (
                        <button
                            onClick={onStop}
                            disabled={cancelRequested}
                            className="px-4 py-1.5 bg-red-500 text-black font-semibold hover:bg-red-400 disabled:opacity-50"
                        >
                            {cancelRequested ? "Stopping…" : "Stop"}
                        </button>
                    ) : (
                        <button
                            onClick={primary}
                            disabled={primaryDisabled}
                            className="px-4 py-1.5 bg-neutral-200 text-black font-semibold hover:bg-white disabled:opacity-40 disabled:hover:bg-neutral-200"
                        >
                            {primaryLabel}
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}
