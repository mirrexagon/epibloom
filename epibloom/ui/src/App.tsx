import { useState } from "react";
import { DebugDrawer } from "./components/DebugDrawer";
import { ChatView } from "./components/ChatView";
import { TraceMetadata } from "./types/api";

function App() {
    const [debugDrawerOpen, setDebugDrawerOpen] = useState(true);
    const [debugDrawerHeight, setDebugDrawerHeight] = useState(400);
    const [selectedTrace, setSelectedTrace] = useState<TraceMetadata | null>(
        null,
    );
    const [debugTab, setDebugTab] = useState<"traces" | "state">("traces");
    const [requestedStateObject, setRequestedStateObject] = useState<string | null>(null);

    const handleSelectTraceFromChat = (traceId: string) => {
        // Create a minimal TraceMetadata object - the full data will be loaded by TraceView
        setSelectedTrace({
            id: traceId,
            name: traceId,
            created: "",
            modified: "",
        });
        setDebugTab("traces");
        setDebugDrawerOpen(true);
    };

    const handleSelectStateObject = (name: string) => {
        setRequestedStateObject(name);
        setDebugTab("state");
        setDebugDrawerOpen(true);
    };

    return (
        <div className="h-screen flex flex-col bg-gray-900">
            {/* App header */}
            <header className="bg-gray-800 text-white p-4 shadow-lg">
                <h1 className="text-2xl font-bold">🌸 Epibloom</h1>
            </header>

            {/* Main content area */}
            <div className="flex-1 flex flex-col overflow-hidden">
                {/* Primary panel - Chat UI */}
                <ChatView
                    onSelectTrace={handleSelectTraceFromChat}
                    onSelectStateObject={handleSelectStateObject}
                />

                {/* Debug drawer */}
                <DebugDrawer
                    isOpen={debugDrawerOpen}
                    onToggle={() => setDebugDrawerOpen(!debugDrawerOpen)}
                    height={debugDrawerHeight}
                    onHeightChange={setDebugDrawerHeight}
                    selectedTrace={selectedTrace}
                    onSelectTrace={setSelectedTrace}
                    activeTab={debugTab}
                    onTabChange={setDebugTab}
                    requestedStateObject={requestedStateObject}
                />
            </div>
        </div>
    );
}

export default App;
