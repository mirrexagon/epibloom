import { useEffect, useState } from "react";
import { Message, TraceEvent } from "../types/api";

interface UseTraceResult {
    messages: Message[];
    error: string | null;
    connected: boolean;
}

export function useTrace(traceId: string | null): UseTraceResult {
    const [messages, setMessages] = useState<Message[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [connected, setConnected] = useState(false);

    useEffect(() => {
        if (!traceId) {
            setMessages([]);
            setError(null);
            setConnected(false);
            return;
        }

        const eventSource = new EventSource(`/api/trace/${traceId}`);

        eventSource.onopen = () => {
            setConnected(true);
            setError(null);
        };

        eventSource.onmessage = (event) => {
            try {
                const traceEvent: TraceEvent = JSON.parse(event.data);

                setMessages((prev) => {
                    const newMessages = [...prev];

                    switch (traceEvent.type) {
                        case "message_start":
                            newMessages[traceEvent.position] = {
                                role: traceEvent.role,
                                content: "",
                                thinking:
                                    traceEvent.role === "assistant"
                                        ? ""
                                        : undefined,
                                position: traceEvent.position,
                            };
                            break;

                        case "thinking_delta":
                            if (newMessages.length > 0) {
                                const lastIdx = newMessages.length - 1;
                                newMessages[lastIdx] = {
                                    ...newMessages[lastIdx],
                                    thinking:
                                        (newMessages[lastIdx].thinking || "") +
                                        traceEvent.delta,
                                };
                            }
                            break;

                        case "content_delta":
                            if (newMessages.length > 0) {
                                const lastIdx = newMessages.length - 1;
                                newMessages[lastIdx] = {
                                    ...newMessages[lastIdx],
                                    content:
                                        newMessages[lastIdx].content +
                                        traceEvent.delta,
                                };
                            }
                            break;

                        case "message_end":
                            // Message is complete, nothing special to do
                            break;

                        case "error":
                            setError(traceEvent.message);
                            console.error(
                                `Trace stream reported error: ${traceEvent.message}`,
                            );
                            break;
                    }

                    return newMessages;
                });
            } catch (err) {
                console.error("Error parsing trace event:", err);
                setError(err instanceof Error ? err.message : "Unknown error");
            }
        };

        eventSource.onerror = () => {
            setConnected(false);
            console.error("Trace stream connection error");
            setError("Connection to trace lost. Retrying...");
        };

        return () => {
            eventSource.close();
            setMessages([]);
        };
    }, [traceId]);

    return { messages, error, connected };
}
