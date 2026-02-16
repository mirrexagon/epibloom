import { useState, useEffect, useMemo } from "react";
import { useScript } from "../hooks/useScript";
import { useStateObjects } from "../hooks/useStateObjects";
import { Chat } from "../types/api";

export function ChatView() {
    const [chatIds, setChatIds] = useState<string[]>([]);
    const [selectedChatId, setSelectedChatId] = useState<string | null>(null);

    // Script hooks for chat actions
    const listScript = useScript<string[]>("chat", "chat_action");
    const newChatScript = useScript<string>("chat", "chat_action");
    const pokeScript = useScript<void>("chat", "chat_action");

    // State hook for selected chat - memoize to prevent infinite re-renders
    const stateObjectNames = useMemo(
        () => (selectedChatId ? [`chat:${selectedChatId}`] : []),
        [selectedChatId]
    );
    const { objects } = useStateObjects(stateObjectNames);

    // Get the chat data from state objects
    const chatData = objects.find(
        (obj) => obj.name === `chat:${selectedChatId}`,
    )?.value as Chat | undefined;

    // Load chat list on mount
    useEffect(() => {
        const loadChats = async () => {
            const result = await listScript.runScript({ action: "list" });
            if (result) {
                setChatIds(result);
            }
        };
        loadChats();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    const handleNewChat = async () => {
        const newChatId = await newChatScript.runScript({ action: "new" });
        if (newChatId) {
            setChatIds((prev) => [...prev, newChatId]);
            setSelectedChatId(newChatId);
        }
    };

    const handleSelectChat = async (chatId: string) => {
        setSelectedChatId(chatId);
        // Poke the chat to load it into the state system
        await pokeScript.runScript({ action: "poke", chat_id: chatId });
    };

    return (
        <div className="flex-1 flex overflow-hidden">
            {/* Sidebar */}
            <div className="w-64 bg-gray-800 border-r border-gray-700 flex flex-col">
                <div className="p-4 border-b border-gray-700">
                    <button
                        onClick={handleNewChat}
                        disabled={newChatScript.loading}
                        className="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 text-white rounded transition-colors"
                    >
                        {newChatScript.loading ? "Creating..." : "+ New Chat"}
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto">
                    {listScript.loading && (
                        <div className="p-4 text-gray-400 text-sm">
                            Loading chats...
                        </div>
                    )}
                    {listScript.error && (
                        <div className="p-4 text-red-400 text-sm">
                            Error: {listScript.error}
                        </div>
                    )}
                    {chatIds.length === 0 && !listScript.loading && (
                        <div className="p-4 text-gray-500 text-sm">
                            No chats yet. Create one!
                        </div>
                    )}
                    <div className="space-y-1 p-2">
                        {chatIds.map((chatId) => (
                            <button
                                key={chatId}
                                onClick={() => handleSelectChat(chatId)}
                                className={`w-full text-left px-3 py-2 rounded transition-colors ${
                                    selectedChatId === chatId
                                        ? "bg-gray-700 text-white"
                                        : "text-gray-300 hover:bg-gray-750"
                                }`}
                            >
                                <div className="font-mono text-xs truncate">
                                    {chatId}
                                </div>
                            </button>
                        ))}
                    </div>
                </div>
            </div>

            {/* Main panel */}
            <div className="flex-1 flex flex-col overflow-hidden bg-gray-900">
                {!selectedChatId ? (
                    <div className="flex-1 flex items-center justify-center text-gray-500">
                        <div className="text-center">
                            <div className="text-6xl mb-4">💬</div>
                            <div className="text-xl">
                                Select a chat or create a new one
                            </div>
                        </div>
                    </div>
                ) : (
                    <div className="flex-1 overflow-y-auto p-4">
                        <div className="mb-4 text-gray-400 text-sm font-mono">
                            Chat ID: {selectedChatId}
                        </div>
                        {chatData ? (
                            <pre className="bg-gray-800 p-4 rounded text-gray-300 text-sm overflow-x-auto">
                                {JSON.stringify(chatData, null, 2)}
                            </pre>
                        ) : (
                            <div className="text-gray-500 text-sm">
                                Loading chat data...
                            </div>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}
