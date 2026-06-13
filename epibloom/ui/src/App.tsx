import { useState } from "react";
import { StateTab } from "./components/StateTab";
import { AnthropicTracesTab } from "./components/AnthropicTracesTab";

type Tab = "state" | "anthropic-traces";

function App() {
    const [activeTab, setActiveTab] = useState<Tab>("state");

    return (
        <div className="h-screen flex flex-col bg-gray-900">
            {/* App header */}
            <header className="bg-gray-800 text-white p-4 shadow-lg">
                <h1 className="text-2xl font-bold">🌸 Epibloom</h1>
            </header>

            {/* Tab bar */}
            <div className="bg-gray-800 border-b border-gray-700">
                <div className="flex">
                    <button
                        onClick={() => setActiveTab("state")}
                        className={`px-6 py-3 font-medium transition-colors ${
                            activeTab === "state"
                                ? "bg-gray-900 text-white border-b-2 border-blue-500"
                                : "text-gray-400 hover:text-white hover:bg-gray-700"
                        }`}
                    >
                        State
                    </button>
                    <button
                        onClick={() => setActiveTab("anthropic-traces")}
                        className={`px-6 py-3 font-medium transition-colors ${
                            activeTab === "anthropic-traces"
                                ? "bg-gray-900 text-white border-b-2 border-blue-500"
                                : "text-gray-400 hover:text-white hover:bg-gray-700"
                        }`}
                    >
                        Anthropic LLM Traces
                    </button>
                </div>
            </div>

            {/* Tab content */}
            <div className="flex-1 overflow-hidden">
                {activeTab === "state" && <StateTab />}
                {activeTab === "anthropic-traces" && <AnthropicTracesTab />}
            </div>
        </div>
    );
}

export default App;
