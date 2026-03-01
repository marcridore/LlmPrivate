import { useEffect } from "react";
import { AppLayout } from "./components/layout/AppLayout";
import { useUIStore } from "./stores/uiStore";

function App() {
  const theme = useUIStore((s) => s.theme);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "dark") {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
  }, [theme]);

  return <AppLayout />;
}

export default App;
