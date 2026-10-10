import {
    AnthropicTraceStateObject,
    AnthropicMessage,
    AnthropicResponse,
} from "../types/api";
import { ROLE_STYLES, roleStyle } from "./roleStyles";

/** Header and request/response body for a single Anthropic LLM trace. Fills its parent's height. */
export function AnthropicTraceView({
    id,
    trace,
}: {
    id: string;
    trace: AnthropicTraceStateObject;
}) {
    return (
        <div className="h-full flex flex-col overflow-hidden">
            <div className="px-4 py-3 border-b border-neutral-800">
                <div className="flex items-center justify-between">
                    <h3 className="text-lg font-semibold font-mono break-all">
                        {id}
                    </h3>
                    {trace.error ? (
                        <span className="text-sm text-red-400">Error</span>
                    ) : !trace.end_time ? (
                        <span className="text-sm text-neutral-400 animate-pulse">
                            Running…
                        </span>
                    ) : null}
                </div>
                <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-0.5 text-sm">
                    <MetaRow label="Model" value={trace.request.model} />
                    <MetaRow
                        label="Started"
                        value={new Date(trace.start_time).toLocaleString()}
                    />
                    {trace.end_time && (
                        <MetaRow
                            label="Ended"
                            value={new Date(trace.end_time).toLocaleString()}
                        />
                    )}
                    <MetaRow
                        label="Max tokens"
                        value={trace.request.max_tokens}
                    />
                </dl>
                {trace.error && (
                    <div className="mt-3 border-l-2 border-red-400 pl-3 text-sm text-neutral-200 whitespace-pre-wrap font-mono">
                        {trace.error}
                    </div>
                )}
            </div>
            <div className="flex-1 overflow-auto p-4 space-y-6">
                <section>
                    <SectionHeading>Request</SectionHeading>
                    <div className="space-y-3">
                        {trace.request.system && (
                            <MessageView
                                message={{
                                    role: "system",
                                    content: trace.request.system,
                                }}
                            />
                        )}
                        {trace.request.messages.map((message, index) => (
                            <MessageView key={index} message={message} />
                        ))}
                    </div>
                </section>

                {trace.response && (
                    <section>
                        <SectionHeading>Response</SectionHeading>
                        <MessageView message={trace.response} highlight />
                    </section>
                )}
            </div>
        </div>
    );
}

function MetaRow({ label, value }: { label: string; value: React.ReactNode }) {
    return (
        <>
            <dt className="text-neutral-500">{label}</dt>
            <dd className="text-neutral-300">{value}</dd>
        </>
    );
}

function SectionHeading({ children }: { children: React.ReactNode }) {
    return (
        <h4 className="text-sm uppercase tracking-wider text-neutral-500 mb-2">
            {children}
        </h4>
    );
}

function MessageView({
    message,
    highlight = false,
}: {
    message: AnthropicMessage | AnthropicResponse;
    highlight?: boolean;
}) {
    // Extract text and thinking content
    let textContent = "";
    let thinkingContent = "";

    if (typeof message.content === "string") {
        textContent = message.content;
    } else {
        for (const block of message.content ?? []) {
            if (block.type === "text") {
                textContent += block.text;
            } else if (block.type === "thinking") {
                thinkingContent += block.thinking;
            }
        }
    }

    const isSystem = message.role === "system";
    const style = roleStyle(message.role);

    return (
        <div
            className={`border p-3 ${
                highlight ? style.highlightBorderClass : style.borderClass
            }`}
        >
            <div className="flex items-baseline justify-between mb-2">
                <span
                    className={`text-sm font-semibold uppercase tracking-wider ${style.labelClass}`}
                >
                    {ROLE_STYLES[message.role]?.label ?? message.role}
                </span>
                {message.id && (
                    <span className="text-sm text-neutral-600 font-mono">
                        {message.id}
                    </span>
                )}
            </div>

            {thinkingContent && (
                <div className="mb-3 border-l-2 border-neutral-700 pl-3">
                    <div className="text-sm text-neutral-500 mb-1">
                        Thinking
                    </div>
                    <div className="text-base text-neutral-400 italic whitespace-pre-wrap">
                        {thinkingContent}
                    </div>
                </div>
            )}

            <div
                className={`text-base whitespace-pre-wrap ${
                    isSystem ? "text-neutral-400" : "text-neutral-100"
                }`}
            >
                {textContent}
            </div>

            {message.usage && (
                <div className="mt-3 pt-2 border-t border-neutral-800 text-sm text-neutral-500 flex flex-wrap gap-x-4">
                    <span>
                        {message.usage.input_tokens} in /{" "}
                        {message.usage.output_tokens} out
                    </span>
                    {message.usage.cache_read_input_tokens !== undefined &&
                        message.usage.cache_read_input_tokens > 0 && (
                            <span>
                                {message.usage.cache_read_input_tokens} cached
                            </span>
                        )}
                    {message.stop_reason && (
                        <span>stop: {message.stop_reason}</span>
                    )}
                </div>
            )}
        </div>
    );
}
