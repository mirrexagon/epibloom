import { useMemo } from "react";
import { useScriptRunner } from "./useScriptRunner";
import { ChatRole } from "../types/api";

/** Script context for quick chat actions (edits, cancelling, etc). */
const CHAT_CONTEXT = "chat";

/**
 * Script context for generating in a chat. Generation occupies the context
 * until it finishes, so it gets its own context per chat; this way edits and
 * cancellation (on `CHAT_CONTEXT`) and generation in other chats aren't queued
 * behind it.
 */
const generateContext = (chatId: string) => `chat-generate:${chatId}`;

const SCRIPT = "chat_action";

export interface ChatActions {
    newChat(name?: string): Promise<string | undefined>;
    setName(chatId: string, name: string): Promise<boolean>;
    setSystem(chatId: string, system: string): Promise<boolean>;
    setSettings(
        chatId: string,
        settings: { model?: string; max_tokens?: number },
    ): Promise<boolean>;
    appendMessage(
        chatId: string,
        role: ChatRole,
        text: string,
    ): Promise<boolean>;
    setMessageContent(
        chatId: string,
        messageIndex: number,
        alternateIndex: number,
        text: string,
        note: string,
    ): Promise<boolean>;
    selectAlternate(
        chatId: string,
        messageIndex: number,
        alternateIndex: number,
    ): Promise<boolean>;
    deleteAlternate(
        chatId: string,
        messageIndex: number,
        alternateIndex: number,
    ): Promise<boolean>;
    /** Resolves when generation has finished. */
    generate(chatId: string): Promise<boolean>;
    cancelGenerate(chatId: string): Promise<boolean>;
}

/** Convert plain text to message content (a single text block). */
export function textToContent(text: string): string[][] {
    return text === "" ? [] : [[text]];
}

/** Convert message content to plain text. */
export function contentToText(content: string[][]): string {
    return content.map((block) => block.join("")).join("\n\n");
}

/**
 * Wrappers around the `chat_action` script. Errors are passed to `onError`
 * rather than thrown; each action resolves to whether it succeeded.
 */
export function useChatActions(onError: (error: unknown) => void): ChatActions {
    const run = useScriptRunner();

    return useMemo(() => {
        const act = async (
            input: Record<string, unknown>,
            context = CHAT_CONTEXT,
        ) => {
            try {
                return { ok: true, result: await run(context, SCRIPT, input) };
            } catch (error) {
                onError(error);
                return { ok: false, result: undefined };
            }
        };

        return {
            newChat: async (name) => {
                const { result } = await act({ action: "new", name });
                return (result as { chat_id: string } | undefined)?.chat_id;
            },
            setName: async (chat_id, name) =>
                (await act({ action: "set_name", chat_id, name })).ok,
            setSystem: async (chat_id, system) =>
                (await act({ action: "set_system", chat_id, system })).ok,
            setSettings: async (chat_id, settings) =>
                (await act({ action: "set_settings", chat_id, ...settings }))
                    .ok,
            appendMessage: async (chat_id, role, text) =>
                (
                    await act({
                        action: "append_message",
                        chat_id,
                        role,
                        content: textToContent(text),
                    })
                ).ok,
            setMessageContent: async (
                chat_id,
                message_index,
                alternate_index,
                text,
                note,
            ) =>
                (
                    await act({
                        action: "set_message_content",
                        chat_id,
                        message_index,
                        alternate_index,
                        content: textToContent(text),
                        note,
                    })
                ).ok,
            selectAlternate: async (chat_id, message_index, alternate_index) =>
                (
                    await act({
                        action: "select_alternate",
                        chat_id,
                        message_index,
                        alternate_index,
                    })
                ).ok,
            deleteAlternate: async (chat_id, message_index, alternate_index) =>
                (
                    await act({
                        action: "delete_alternate",
                        chat_id,
                        message_index,
                        alternate_index,
                    })
                ).ok,
            generate: async (chat_id) =>
                (
                    await act(
                        { action: "generate", chat_id },
                        generateContext(chat_id),
                    )
                ).ok,
            cancelGenerate: async (chat_id) =>
                (await act({ action: "cancel_generate", chat_id })).ok,
        };
    }, [run, onError]);
}
