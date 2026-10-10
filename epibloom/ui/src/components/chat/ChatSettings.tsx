import { useState } from "react";
import { Chat } from "../../types/api";
import { ChatActions } from "../../hooks/useChatActions";

/** Editor for a chat's name, system prompt, and generation settings. */
export function ChatSettings({
    chat,
    busy,
    actions,
    onClose,
}: {
    chat: Chat;
    busy: boolean;
    actions: ChatActions;
    onClose: () => void;
}) {
    const [name, setName] = useState(chat.name);
    const [system, setSystem] = useState(chat.system);
    const [model, setModel] = useState(chat.model);
    const [maxTokens, setMaxTokens] = useState(String(chat.max_tokens));
    const [saving, setSaving] = useState(false);

    const maxTokensNumber = Number(maxTokens);
    const maxTokensValid =
        Number.isInteger(maxTokensNumber) && maxTokensNumber > 0;

    const save = async () => {
        setSaving(true);
        try {
            let ok = true;
            if (name !== chat.name) {
                ok = (await actions.setName(chat.id, name)) && ok;
            }
            if (system !== chat.system) {
                ok = (await actions.setSystem(chat.id, system)) && ok;
            }
            if (model !== chat.model || maxTokensNumber !== chat.max_tokens) {
                ok =
                    (await actions.setSettings(chat.id, {
                        model,
                        max_tokens: maxTokensNumber,
                    })) && ok;
            }
            if (ok) onClose();
        } finally {
            setSaving(false);
        }
    };

    const inputClass =
        "w-full bg-neutral-950 border border-neutral-700 px-2 py-1 text-base text-neutral-100 focus:outline-none focus:border-neutral-500";

    return (
        <div className="border-b border-neutral-800 p-4 space-y-3 bg-neutral-950">
            <Field label="Name">
                <input
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    className={inputClass}
                />
            </Field>
            <Field label="System prompt">
                <textarea
                    value={system}
                    onChange={(e) => setSystem(e.target.value)}
                    rows={Math.min(
                        16,
                        Math.max(4, system.split("\n").length + 1),
                    )}
                    className={inputClass}
                />
            </Field>
            <div className="flex gap-4">
                <Field label="Model" className="flex-1">
                    <input
                        value={model}
                        onChange={(e) => setModel(e.target.value)}
                        className={inputClass}
                    />
                </Field>
                <Field label="Max tokens" className="w-40">
                    <input
                        value={maxTokens}
                        onChange={(e) => setMaxTokens(e.target.value)}
                        inputMode="numeric"
                        className={`${inputClass} ${maxTokensValid ? "" : "border-red-500"}`}
                    />
                </Field>
            </div>
            <div className="flex items-center gap-2 text-sm">
                <button
                    onClick={save}
                    disabled={busy || saving || !maxTokensValid}
                    className="px-3 py-1.5 bg-neutral-200 text-black font-semibold hover:bg-white disabled:opacity-40"
                >
                    Save
                </button>
                <button
                    onClick={onClose}
                    className="px-3 py-1.5 border border-neutral-700 text-neutral-300 hover:bg-neutral-900"
                >
                    Cancel
                </button>
                {busy && (
                    <span className="text-neutral-500">
                        Can't save while generating.
                    </span>
                )}
            </div>
        </div>
    );
}

function Field({
    label,
    className = "",
    children,
}: {
    label: string;
    className?: string;
    children: React.ReactNode;
}) {
    return (
        <label className={`block ${className}`}>
            <span className="block text-sm uppercase tracking-wider text-neutral-500 mb-1">
                {label}
            </span>
            {children}
        </label>
    );
}
