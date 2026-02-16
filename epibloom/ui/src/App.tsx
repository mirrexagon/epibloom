import { useState } from "react";
import { DebugDrawer } from "./components/DebugDrawer";

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
                {/* Primary panel */}
                <div className="flex-1 flex items-center justify-center text-gray-500">
                    <div className="text-center">
                        <div className="text-6xl mb-4">🌸</div>
                        <div className="text-xl">Primary Panel</div>
                    </div>
                </div>

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
