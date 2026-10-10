import { useState } from "react";
import { StateTab } from "./components/StateTab";
import { AnthropicTracesTab } from "./components/AnthropicTracesTab";
import { ChatTab } from "./components/chat/ChatTab";
import { ScriptRunnerProvider } from "./components/ScriptRunnerProvider";

type Tab = "chat" | "state" | "anthropic-traces";

const TABS: { id: Tab; label: string }[] = [
    { id: "chat", label: "Chat" },
    { id: "state", label: "State" },
    { id: "anthropic-traces", label: "Anthropic LLM Traces" },
];

function App() {
    const [activeTab, setActiveTab] = useState<Tab>("chat");

    return (
        <ScriptRunnerProvider>
            <div className="h-screen flex flex-col bg-black text-neutral-100">
                <header className="flex items-stretch border-b border-neutral-800 px-4">
                    <h1 className="py-3 pr-6 text-xl font-semibold tracking-tight">
                        Epibloom
                    </h1>
                    <nav className="flex">
                        {TABS.map((tab) => (
                            <button
                                key={tab.id}
                                onClick={() => setActiveTab(tab.id)}
                                className={`px-4 text-base border-b -mb-px transition-colors ${
                                    activeTab === tab.id
                                        ? "bg-neutral-800 border-white text-white"
                                        : "border-transparent text-neutral-500 hover:bg-neutral-900 hover:text-neutral-200"
                                }`}
                            >
                                {tab.label}
                            </button>
                        ))}
                    </nav>
                </header>

                <div className="flex-1 overflow-hidden">
                    {activeTab === "state" && <StateTab />}
                    {activeTab === "chat" && <ChatTab />}
                    {activeTab === "anthropic-traces" && <AnthropicTracesTab />}
                </div>
            </div>
        </ScriptRunnerProvider>
    );
}

export default App;
