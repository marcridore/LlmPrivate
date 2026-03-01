import { useEffect, useState } from "react";
import { useModelStore } from "../../stores/modelStore";
import { ModelCard } from "./ModelCard";
import { LocalModels } from "./LocalModels";
import { DownloadManager } from "./DownloadManager";

type Tab = "recommended" | "local" | "downloads";
type CategoryFilter = "all" | "general" | "code" | "small" | "large" | "vision";

export function ModelBrowser() {
  const [tab, setTab] = useState<Tab>("recommended");
  const [search, setSearch] = useState("");
  const [categoryFilter, setCategoryFilter] =
    useState<CategoryFilter>("all");

  const recommendedModels = useModelStore((s) => s.recommendedModels);
  const loadRecommendedModels = useModelStore((s) => s.loadRecommendedModels);
  const scanLocalModels = useModelStore((s) => s.scanLocalModels);
  const activeDownloads = useModelStore((s) => s.activeDownloads);

  useEffect(() => {
    loadRecommendedModels();
    scanLocalModels();
  }, [loadRecommendedModels, scanLocalModels]);

  const filteredModels = recommendedModels.filter((m) => {
    const matchesSearch =
      !search ||
      m.name.toLowerCase().includes(search.toLowerCase()) ||
      m.description.toLowerCase().includes(search.toLowerCase()) ||
      m.tags.some((t) => t.toLowerCase().includes(search.toLowerCase()));

    const matchesCategory =
      categoryFilter === "all" || m.category === categoryFilter;

    return matchesSearch && matchesCategory;
  });

  const downloadCount = activeDownloads.size;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="p-4 border-b border-border">
        <h1 className="text-lg font-bold mb-3">Models</h1>

        {/* Tabs */}
        <div className="flex gap-1 mb-3">
          {(
            [
              ["recommended", "Discover"],
              ["local", "Installed"],
              ["downloads", `Downloads${downloadCount > 0 ? ` (${downloadCount})` : ""}`],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              onClick={() => setTab(key)}
              className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
                tab === key
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-muted"
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        {/* Search + Filter (only on recommended tab) */}
        {tab === "recommended" && (
          <div className="flex gap-2">
            <input
              type="text"
              placeholder="Search models..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="flex-1 px-3 py-2 bg-muted rounded-md text-sm outline-none focus:ring-1 focus:ring-ring"
            />
            <select
              value={categoryFilter}
              onChange={(e) =>
                setCategoryFilter(e.target.value as CategoryFilter)
              }
              className="px-3 py-2 bg-muted rounded-md text-sm outline-none"
            >
              <option value="all">All Categories</option>
              <option value="small">Small (2-4B)</option>
              <option value="general">General (7-8B)</option>
              <option value="code">Code</option>
              <option value="vision">Vision</option>
              <option value="large">Large (16B+)</option>
            </select>
          </div>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {tab === "recommended" && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 max-w-4xl">
            {filteredModels.length === 0 && (
              <p className="text-muted-foreground text-sm col-span-2">
                No models match your search.
              </p>
            )}
            {filteredModels.map((model) => (
              <ModelCard key={model.id} model={model} />
            ))}
          </div>
        )}

        {tab === "local" && <LocalModels />}

        {tab === "downloads" && <DownloadManager />}
      </div>
    </div>
  );
}
