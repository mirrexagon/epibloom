import { Chat } from "../../types/api";

export interface OpenTrace {
    chatId: string;
    /** Name of the trace state object. */
    trace: string;
}

/** Find the message alternate that was generated with the given trace. Indices are 1-based. */
export function locateTrace(
    chat: Chat,
    trace: string,
): { messageIndex: number; alternateIndex: number } | null {
    for (const [i, slot] of chat.messages.entries()) {
        const j = slot.alternates.findIndex(
            (alternate) => alternate.trace === trace,
        );
        if (j !== -1) return { messageIndex: i + 1, alternateIndex: j + 1 };
    }
    return null;
}

/** DOM element ID for a message slot, for scrolling to it. */
export function chatMessageElementId(messageIndex: number) {
    return `chat-message-${messageIndex}`;
}
