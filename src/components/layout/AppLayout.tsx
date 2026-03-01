import { TitleBar } from "./TitleBar";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { ChatView } from "../chat/ChatView";
import { ModelBrowser } from "../models/ModelBrowser";
import { useUIStore } from "../../stores/uiStore";

export function AppLayout() {
  const activePage = useUIStore((s) => s.activePage);

  return (
    <div className="h-screen flex flex-col">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex-1 flex flex-col overflow-hidden">
          {activePage === "chat" && <ChatView />}
          {activePage === "models" && <ModelBrowser />}
          {activePage === "monitor" && <PlaceholderPage name="Resource Monitor" />}
          {activePage === "settings" && <PlaceholderPage name="Settings" />}
        </main>
      </div>
      <StatusBar />
    </div>
  );
}

function PlaceholderPage({ name }: { name: string }) {
  return (
    <div className="flex-1 flex items-center justify-center text-muted-foreground">
      <div className="text-center">
        <h2 className="text-xl font-semibold mb-2">{name}</h2>
        <p className="text-sm">Coming soon</p>
      </div>
    </div>
  );
}
