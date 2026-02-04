import { useEffect, useState } from 'react';
import { Message, TraceEvent } from '../types/api';

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
            case 'user_message_start':
              // Start a new user message
              newMessages[traceEvent.position] = {
                role: 'user',
                text: '',
                position: traceEvent.position,
              };
              break;

            case 'user_message_delta':
              // Append to the current user message (should be the last one)
              if (newMessages.length > 0) {
                const lastIdx = newMessages.length - 1;
                newMessages[lastIdx] = {
                  ...newMessages[lastIdx],
                  text: newMessages[lastIdx].text + traceEvent.delta,
                };
              }
              break;

            case 'user_message_end':
              // Message is complete, nothing special to do
              break;

            case 'assistant_message_start':
              // Start a new assistant message
              newMessages[traceEvent.position] = {
                role: 'assistant',
                text: '',
                thinking: '',
                position: traceEvent.position,
              };
              break;

            case 'assistant_thinking_delta':
              // Append to thinking
              if (newMessages.length > 0) {
                const lastIdx = newMessages.length - 1;
                newMessages[lastIdx] = {
                  ...newMessages[lastIdx],
                  thinking: (newMessages[lastIdx].thinking || '') + traceEvent.delta,
                };
              }
              break;

            case 'assistant_message_delta':
              // Append to message text
              if (newMessages.length > 0) {
                const lastIdx = newMessages.length - 1;
                newMessages[lastIdx] = {
                  ...newMessages[lastIdx],
                  text: newMessages[lastIdx].text + traceEvent.delta,
                };
              }
              break;

            case 'assistant_message_end':
              // Message is complete
              break;

            case 'error':
              setError(traceEvent.message);
              break;
          }

          return newMessages;
        });
      } catch (err) {
        console.error('Error parsing trace event:', err);
        setError(err instanceof Error ? err.message : 'Unknown error');
      }
    };

    eventSource.onerror = () => {
      setConnected(false);
      setError('Connection to trace lost. Retrying...');
    };

    return () => {
      eventSource.close();
      setMessages([]);
    };
  }, [traceId]);

  return { messages, error, connected };
}
