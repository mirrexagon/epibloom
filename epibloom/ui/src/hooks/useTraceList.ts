import { useEffect, useState } from 'react';
import { TraceMetadata, TraceStoreEvent } from '../types/api';

interface UseTraceListResult {
  traces: TraceMetadata[];
  error: string | null;
  connected: boolean;
  latestTrace: TraceMetadata | null;
}

export function useTraceList(): UseTraceListResult {
  const [traces, setTraces] = useState<TraceMetadata[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const [latestTrace, setLatestTrace] = useState<TraceMetadata | null>(null);

  useEffect(() => {
    const eventSource = new EventSource('/api/trace/');

    eventSource.onopen = () => {
      setConnected(true);
      setError(null);
    };

    eventSource.onmessage = (event) => {
      try {
        const storeEvent: TraceStoreEvent = JSON.parse(event.data);
        
        if (storeEvent.type === 'added') {
          setTraces((prev) => {
            // Check if trace already exists
            const exists = prev.some((t) => t.id === storeEvent.trace.id);
            if (exists) {
              return prev;
            }
            // Set this as the latest trace for auto-selection
            setLatestTrace(storeEvent.trace);
            // Add new trace, sorted by created date (newest first)
            return [storeEvent.trace, ...prev].sort(
              (a, b) => new Date(b.created).getTime() - new Date(a.created).getTime()
            );
          });
        }
      } catch (err) {
        console.error('Error parsing trace store event:', err);
        setError(err instanceof Error ? err.message : 'Unknown error');
      }
    };

    eventSource.onerror = () => {
      setConnected(false);
      setError('Connection to server lost. Retrying...');
    };

    return () => {
      eventSource.close();
    };
  }, []);

  return { traces, error, connected, latestTrace };
}
