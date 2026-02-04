import { useTrace } from '../hooks/useTrace';
import { Message } from './Message';

interface TraceViewProps {
  traceId: string | null;
  traceName: string | null;
}

export function TraceView({ traceId, traceName }: TraceViewProps) {
  const { messages, error, connected } = useTrace(traceId);

  if (!traceId) {
    return (
      <div className="flex items-center justify-center h-full text-gray-400 bg-gray-900">
        Select a trace to view its messages
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-gray-900">
      {/* Header */}
      <div className="border-b border-gray-700 bg-gray-800 p-4 flex items-center justify-between">
        <h2 className="text-xl font-semibold text-white">{traceName || 'Trace'}</h2>
        <div className="flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full ${
              connected ? 'bg-green-500' : 'bg-red-500'
            }`}
            title={connected ? 'Connected' : 'Disconnected'}
          />
          <span className="text-sm text-gray-300">
            {connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div className="bg-red-900 border border-red-700 text-red-200 px-4 py-3">
          <strong>Error:</strong> {error}
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4">
        {messages.length === 0 ? (
          <div className="text-gray-400 text-center">No messages yet</div>
        ) : (
          <div className="font-mono text-xs text-gray-300 space-y-3">
            {messages.map((message, idx) => (
              <Message key={`${message.position}-${idx}`} message={message} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
