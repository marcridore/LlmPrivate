import { create } from "zustand";

type Theme = "light" | "dark";
type Page = "chat" | "documents" | "models" | "monitor" | "agents" | "settings";

interface UIState {
  theme: Theme;
  activePage: Page;
  sidebarCollapsed: boolean;

  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
  setActivePage: (page: Page) => void;
  toggleSidebar: () => void;
}

export const useUIStore = create<UIState>((set) => ({
  theme: "dark",
  activePage: "chat",
  sidebarCollapsed: false,

  setTheme: (theme) => set({ theme }),
  toggleTheme: () =>
    set((s) => ({ theme: s.theme === "dark" ? "light" : "dark" })),
  setActivePage: (activePage) => set({ activePage }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
}));
