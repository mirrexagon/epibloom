import { useMemo } from "react";
import { useStateObjects } from "../../hooks/useStateObjects";
import { AnthropicTraceStateObject } from "../../types/api";
import { AnthropicTraceView } from "../AnthropicTraceView";

/** Side panel showing the LLM trace for a chat message. */
export function TraceDrawer({
    traceName,
    title,
    onJumpToMessage,
    onClose,
}: {
    /** Name of the trace state object. */
    traceName: string;
    title: string;
    /** Not given if the message no longer exists. */
    onJumpToMessage?: () => void;
    onClose: () => void;
}) {
    const objectNames = useMemo(() => [traceName], [traceName]);
    const { objects } = useStateObjects(objectNames);
    const trace = objects.find((obj) => obj.name === traceName)?.value as
        AnthropicTraceStateObject | null | undefined;

    const traceId = traceName
        .replace(/^llm-trace-anthropic:/, "")
        .replace(/:persist$/, "");

    return (
        <div className="h-full flex flex-col overflow-hidden">
            <div className="px-4 py-2 border-b border-neutral-800 flex items-center gap-3 bg-neutral-950">
                <span className="text-sm uppercase tracking-wider text-neutral-400">
                    {title}
                </span>
                <button
                    onClick={onJumpToMessage}
                    disabled={!onJumpToMessage}
                    className="ml-auto disabled:opacity-40 px-2 py-1 text-sm border border-neutral-700 text-neutral-300 hover:bg-neutral-900"
                >
                    Jump to message
                </button>
                <button
                    onClick={onClose}
                    title="Close (Escape)"
                    aria-label="Close trace"
                    className="px-2 py-1 text-base leading-none text-neutral-400 hover:text-white"
                >
                    ✕
                </button>
            </div>
            <div className="flex-1 overflow-hidden">
                {trace ? (
                    <AnthropicTraceView id={traceId} trace={trace} />
                ) : (
                    <div className="p-4 text-base text-neutral-500">
                        {trace === null ? "Trace not found." : "Loading…"}
                    </div>
                )}
            </div>
        </div>
    );
}
