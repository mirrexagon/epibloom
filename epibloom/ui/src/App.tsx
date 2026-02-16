import { useState } from "react";
import { DebugDrawer } from "./components/DebugDrawer";
import { ChatView } from "./components/ChatView";

function App() {
    const [debugDrawerOpen, setDebugDrawerOpen] = useState(true);
    const [debugDrawerHeight, setDebugDrawerHeight] = useState(400);

    return (
        <div className="h-screen flex flex-col bg-gray-900">
            {/* App header */}
            <header className="bg-gray-800 text-white p-4 shadow-lg">
                <h1 className="text-2xl font-bold">🌸 Epibloom</h1>
            </header>

            {/* Main content area */}
            <div className="flex-1 flex flex-col overflow-hidden">
                {/* Primary panel - Chat UI */}
                <ChatView />

                {/* Debug drawer */}
                <DebugDrawer
                    isOpen={debugDrawerOpen}
                    onToggle={() => setDebugDrawerOpen(!debugDrawerOpen)}
                    height={debugDrawerHeight}
                    onHeightChange={setDebugDrawerHeight}
                />
            </div>
        </div>
    );
}

export default App;
