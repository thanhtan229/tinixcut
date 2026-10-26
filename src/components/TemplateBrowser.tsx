import React, { useState, useEffect, useMemo } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { TemplateDefinition } from "../types/template";
import { Asset, TimelineItem } from "../types/project";
import { TemplateCard } from "./TemplateCard";
import { TemplateAuthoringDialog } from "./TemplateAuthoringDialog";
import { TemplatePackExportDialog } from "./TemplatePackExportDialog";
import { CapCutTemplateImportDialog } from "./CapCutTemplateImportDialog";
import {
  extractCategories,
  filterTemplates,
  loadFavoriteTemplateIds,
  loadRecentTemplateIds,
  recordRecentTemplateId,
  toggleFavoriteTemplateId,
} from "../utils/templateHelpers";
import { editorApi } from "../services/editorApi";

const DEFAULT_PAGE_SIZE = 12;

interface TemplateBrowserProps {
  selectedItem: TimelineItem | null;
  assets?: Asset[];
  onApplyTemplate: (definition: TemplateDefinition) => Promise<unknown> | void;
  onStatusChange?: (msg: string) => void;
  projectRevision: number;
  onProjectChanged: () => Promise<void> | void;
}

export const TemplateBrowser: React.FC<TemplateBrowserProps> = ({
  selectedItem,
  assets = [],
  onApplyTemplate,
  onStatusChange,
  projectRevision,
  onProjectChanged,
}) => {
  const [definitions, setDefinitions] = useState<TemplateDefinition[]>([]);
  const [builtinIds, setBuiltinIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Filters state
  const [search, setSearch] = useState<string>("");
  const [selectedCategory, setSelectedCategory] = useState<string>("All");
  const [sourceFilter, setSourceFilter] = useState<"all" | "builtin" | "user">("all");
  const [favoriteIds, setFavoriteIds] = useState<Set<string>>(new Set());
  const [onlyFavorites, setOnlyFavorites] = useState<boolean>(false);
  const [recentIds, setRecentIds] = useState<string[]>([]);
  const [onlyRecent, setOnlyRecent] = useState<boolean>(false);

  // Bounded mount / pagination state
  const [visibleCount, setVisibleCount] = useState<number>(DEFAULT_PAGE_SIZE);

  // Shared reduced-motion listener (Item 8)
  const [prefersReducedMotion, setPrefersReducedMotion] = useState<boolean>(false);

  useEffect(() => {
    try {
      const mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
      setPrefersReducedMotion(mediaQuery.matches);
      const listener = (event: MediaQueryListEvent) => {
        setPrefersReducedMotion(event.matches);
      };
      mediaQuery.addEventListener("change", listener);
      return () => mediaQuery.removeEventListener("change", listener);
    } catch {
      // fallback
    }
  }, []);

  // Modals state
  const [isAuthoringOpen, setIsAuthoringOpen] = useState<boolean>(false);
  const [isExportPackOpen, setIsExportPackOpen] = useState<boolean>(false);
  const [isCapCutImportOpen, setIsCapCutImportOpen] = useState<boolean>(false);

  // Load favorites & recents on mount
  useEffect(() => {
    setFavoriteIds(loadFavoriteTemplateIds());
    setRecentIds(loadRecentTemplateIds());
  }, []);

  // Reset pagination when search or filters change
  useEffect(() => {
    setVisibleCount(DEFAULT_PAGE_SIZE);
  }, [search, selectedCategory, sourceFilter, onlyFavorites, onlyRecent]);

  // Fetch catalog from backend
  const refreshCatalog = async () => {
    try {
      setIsLoading(true);
      setErrorMsg(null);
      const defs = await editorApi.templateListDefinitions();
      setDefinitions(defs);

      const builtinKnown = new Set([
        "headline-bold",
        "retro-neon",
        "clean-lower-third-v2",
        "quote-card-v2",
        "sticker-badge",
        "minimal-title-card",
        "cyber-frame",
        "social-follow-bar",
        "kinetic-stack",
        "warm-editorial",
        "gaming-alert",
        "breaking-news",
        "clean-lower-third",
        "bold-headline",
        "center-pop",
        "subtitle-card",
        "quote-card",
        "call-to-action",
      ]);
      setBuiltinIds(builtinKnown);
      setIsLoading(false);
    } catch (err) {
      setIsLoading(false);
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  useEffect(() => {
    refreshCatalog();
  }, []);

  const categories = useMemo(() => extractCategories(definitions), [definitions]);

  const filtered = useMemo(() => {
    return filterTemplates(definitions, {
      search,
      category: selectedCategory,
      source: sourceFilter,
      favoriteIds,
      onlyFavorites,
      recentIds,
      onlyRecent,
      builtinIds,
    });
  }, [
    definitions,
    search,
    selectedCategory,
    sourceFilter,
    favoriteIds,
    onlyFavorites,
    recentIds,
    onlyRecent,
    builtinIds,
  ]);

  const displayedDefinitions = useMemo(() => {
    return filtered.slice(0, visibleCount);
  }, [filtered, visibleCount]);

  const handleToggleFavorite = (id: string) => {
    const updated = toggleFavoriteTemplateId(id, favoriteIds);
    setFavoriteIds(updated);
  };

  // Item 4: Don't record recents or report applied before onApplyTemplate resolves successfully
  const handleApply = async (definition: TemplateDefinition) => {
    try {
      await onApplyTemplate(definition);
      const updatedRecents = recordRecentTemplateId(definition.id, recentIds);
      setRecentIds(updatedRecents);
      if (onStatusChange) {
        onStatusChange(`Applied template: ${definition.name} (v${definition.version})`);
      }
    } catch (err) {
      if (onStatusChange) {
        onStatusChange(`Failed to apply template: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  };

  const handleDeleteUserTemplate = async (id: string, version: number) => {
    try {
      const ok = await editorApi.templateDeleteUserDefinition(id, version);
      if (ok) {
        if (onStatusChange) onStatusChange(`Deleted template '${id}' v${version}`);
        await refreshCatalog();
      } else {
        if (onStatusChange) onStatusChange(`Template '${id}' v${version} not found in user library`);
      }
    } catch (err) {
      if (onStatusChange) {
        onStatusChange(`Delete failed: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  };

  const handleImportPack = async () => {
    try {
      const chosen = await open({
        title: "Import Tinix Template Pack",
        filters: [{ name: "Tinix Template Pack", extensions: ["tinixtpl"] }],
        multiple: false,
      });

      if (!chosen) return;
      const path = Array.isArray(chosen) ? chosen[0] : chosen;

      setIsLoading(true);
      const manifest = await editorApi.templateImportPack(path);
      setIsLoading(false);

      if (onStatusChange) {
        onStatusChange(`Imported pack '${manifest.name}' (${manifest.template_ids.length} templates)`);
      }
      await refreshCatalog();
    } catch (err) {
      setIsLoading(false);
      const msg = `Pack import failed: ${err instanceof Error ? err.message : String(err)}`;
      setErrorMsg(msg);
      if (onStatusChange) onStatusChange(msg);
    }
  };

  const isTextSelected = selectedItem?.kind === "Text";

  return (
    <aside className="left-panel template-browser-panel">
      {/* Panel Top Header */}
      <div className="panel-header" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <span>Templates</span>
        <div style={{ display: "flex", gap: "6px" }}>
          <button
            type="button"
            className="btn-action"
            onClick={() => setIsCapCutImportOpen(true)}
            title="Inspect and convert a CapCut draft"
            style={{
              padding: "2px 6px",
              fontSize: "10px",
              backgroundColor: "transparent",
              color: "var(--accent-primary)",
              border: "1px solid var(--border-color)",
              borderRadius: "3px",
              cursor: "pointer",
            }}
          >
            CapCut
          </button>
          <button
            type="button"
            className="btn-action"
            onClick={handleImportPack}
            title="Import .tinixtpl template pack"
            style={{
              padding: "2px 6px",
              fontSize: "10px",
              backgroundColor: "transparent",
              color: "var(--accent-primary)",
              border: "1px solid var(--border-color)",
              borderRadius: "3px",
              cursor: "pointer",
            }}
          >
            Import
          </button>
          <button
            type="button"
            className="btn-action"
            onClick={() => setIsExportPackOpen(true)}
            title="Export templates to .tinixtpl pack"
            style={{
              padding: "2px 6px",
              fontSize: "10px",
              backgroundColor: "transparent",
              color: "var(--accent-primary)",
              border: "1px solid var(--border-color)",
              borderRadius: "3px",
              cursor: "pointer",
            }}
          >
            Export
          </button>
          <button
            type="button"
            className="btn-action"
            onClick={() => setIsAuthoringOpen(true)}
            disabled={!isTextSelected}
            title={
              isTextSelected
                ? "Save selected text item as a reusable template"
                : "Select a text clip on timeline to save as template"
            }
            style={{
              padding: "2px 8px",
              fontSize: "10px",
              fontWeight: 600,
              backgroundColor: isTextSelected ? "var(--accent-primary)" : "var(--bg-surface)",
              color: isTextSelected ? "#ffffff" : "var(--text-muted)",
              border: "none",
              borderRadius: "3px",
              cursor: isTextSelected ? "pointer" : "not-allowed",
            }}
          >
            + Save As
          </button>
        </div>
      </div>

      <div className="panel-content" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        {/* Search bar */}
        <div style={{ position: "relative" }}>
          <input
            type="search"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search templates or tags..."
            aria-label="Search templates"
            style={{
              width: "100%",
              padding: "6px 8px",
              paddingRight: search ? "24px" : "8px",
              backgroundColor: "var(--bg-surface)",
              color: "var(--text-main)",
              border: "1px solid var(--border-color)",
              borderRadius: "4px",
              fontSize: "11px",
              outline: "none",
            }}
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              style={{
                position: "absolute",
                right: "6px",
                top: "50%",
                transform: "translateY(-50%)",
                background: "none",
                border: "none",
                color: "var(--text-muted)",
                cursor: "pointer",
                fontSize: "11px",
                padding: "2px",
              }}
              title="Clear search"
            >
              ✕
            </button>
          )}
        </div>

        {/* Source filters: All / Built-in / User / Favorites / Recent */}
        <div style={{ display: "flex", gap: "4px", flexWrap: "wrap", alignItems: "center" }}>
          <button
            type="button"
            className="btn-action"
            onClick={() => {
              setSourceFilter("all");
              setOnlyFavorites(false);
              setOnlyRecent(false);
            }}
            style={{
              padding: "3px 6px",
              fontSize: "10px",
              borderRadius: "3px",
              border: "none",
              backgroundColor:
                sourceFilter === "all" && !onlyFavorites && !onlyRecent
                  ? "var(--accent-primary)"
                  : "var(--bg-surface)",
              color:
                sourceFilter === "all" && !onlyFavorites && !onlyRecent
                  ? "#ffffff"
                  : "var(--text-muted)",
              cursor: "pointer",
            }}
          >
            All
          </button>
          <button
            type="button"
            className="btn-action"
            onClick={() => {
              setSourceFilter("builtin");
              setOnlyFavorites(false);
              setOnlyRecent(false);
            }}
            style={{
              padding: "3px 6px",
              fontSize: "10px",
              borderRadius: "3px",
              border: "none",
              backgroundColor:
                sourceFilter === "builtin" && !onlyFavorites && !onlyRecent
                  ? "var(--accent-primary)"
                  : "var(--bg-surface)",
              color:
                sourceFilter === "builtin" && !onlyFavorites && !onlyRecent
                  ? "#ffffff"
                  : "var(--text-muted)",
              cursor: "pointer",
            }}
          >
            Built-in
          </button>
          <button
            type="button"
            className="btn-action"
            onClick={() => {
              setSourceFilter("user");
              setOnlyFavorites(false);
              setOnlyRecent(false);
            }}
            style={{
              padding: "3px 6px",
              fontSize: "10px",
              borderRadius: "3px",
              border: "none",
              backgroundColor:
                sourceFilter === "user" && !onlyFavorites && !onlyRecent
                  ? "var(--accent-primary)"
                  : "var(--bg-surface)",
              color:
                sourceFilter === "user" && !onlyFavorites && !onlyRecent
                  ? "#ffffff"
                  : "var(--text-muted)",
              cursor: "pointer",
            }}
          >
            My Templates
          </button>
          <button
            type="button"
            className="btn-action"
            onClick={() => {
              setOnlyFavorites(!onlyFavorites);
              if (!onlyFavorites) setOnlyRecent(false);
            }}
            style={{
              padding: "3px 6px",
              fontSize: "10px",
              borderRadius: "3px",
              border: "none",
              backgroundColor: onlyFavorites ? "#f59e0b" : "var(--bg-surface)",
              color: onlyFavorites ? "#ffffff" : "var(--text-muted)",
              cursor: "pointer",
            }}
            title="Show favorite templates"
          >
            ★ Favorites
          </button>
          <button
            type="button"
            className="btn-action"
            onClick={() => {
              setOnlyRecent(!onlyRecent);
              if (!onlyRecent) setOnlyFavorites(false);
            }}
            style={{
              padding: "3px 6px",
              fontSize: "10px",
              borderRadius: "3px",
              border: "none",
              backgroundColor: onlyRecent ? "var(--accent-primary)" : "var(--bg-surface)",
              color: onlyRecent ? "#ffffff" : "var(--text-muted)",
              cursor: "pointer",
            }}
            title="Show recently applied templates"
          >
            🕒 Recent
          </button>
        </div>

        {/* Category chips */}
        <div
          style={{
            display: "flex",
            gap: "4px",
            overflowX: "auto",
            paddingBottom: "4px",
            scrollbarWidth: "none",
          }}
        >
          {categories.map((cat) => (
            <button
              key={cat}
              type="button"
              onClick={() => setSelectedCategory(cat)}
              style={{
                padding: "2px 8px",
                fontSize: "10px",
                borderRadius: "12px",
                border: "1px solid var(--border-color)",
                whiteSpace: "nowrap",
                backgroundColor:
                  selectedCategory.toLowerCase() === cat.toLowerCase()
                    ? "rgba(59, 130, 246, 0.2)"
                    : "transparent",
                color:
                  selectedCategory.toLowerCase() === cat.toLowerCase()
                    ? "var(--accent-primary)"
                    : "var(--text-muted)",
                cursor: "pointer",
              }}
            >
              {cat}
            </button>
          ))}
        </div>

        {/* Error message */}
        {errorMsg && (
          <div
            style={{
              padding: "8px",
              borderRadius: "4px",
              backgroundColor: "rgba(239, 68, 68, 0.15)",
              border: "1px solid rgba(239, 68, 68, 0.3)",
              color: "#ef4444",
              fontSize: "11px",
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
            }}
          >
            <span>{errorMsg}</span>
            <button
              type="button"
              onClick={refreshCatalog}
              style={{
                background: "none",
                border: "none",
                color: "#ef4444",
                cursor: "pointer",
                fontWeight: 600,
                fontSize: "11px",
              }}
            >
              Retry
            </button>
          </div>
        )}

        {/* Loading state */}
        {isLoading ? (
          <div style={{ textAlign: "center", padding: "32px 0", color: "var(--text-muted)", fontSize: "12px" }}>
            Loading template catalog...
          </div>
        ) : filtered.length === 0 ? (
          /* Empty state */
          <div
            style={{
              textAlign: "center",
              padding: "32px 12px",
              color: "var(--text-muted)",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: "8px",
            }}
          >
            <span style={{ fontSize: "24px" }}>🎨</span>
            <div style={{ fontWeight: 600, fontSize: "12px", color: "var(--text-main)" }}>
              No templates found
            </div>
            <p style={{ fontSize: "11px", margin: 0 }}>
              {search || selectedCategory !== "All" || onlyFavorites || onlyRecent
                ? "Try clearing your search query or filters."
                : "Create a template using Save As from any text clip."}
            </p>
            {(search || selectedCategory !== "All" || onlyFavorites || onlyRecent || sourceFilter !== "all") && (
              <button
                type="button"
                className="btn-action"
                onClick={() => {
                  setSearch("");
                  setSelectedCategory("All");
                  setSourceFilter("all");
                  setOnlyFavorites(false);
                  setOnlyRecent(false);
                }}
                style={{
                  padding: "4px 8px",
                  fontSize: "10px",
                  backgroundColor: "var(--bg-surface)",
                  color: "var(--accent-primary)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "3px",
                  cursor: "pointer",
                  marginTop: "4px",
                }}
              >
                Clear all filters
              </button>
            )}
          </div>
        ) : (
          /* Cards list / grid with bounded mount */
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr",
              gap: "8px",
              overflowY: "auto",
              paddingRight: "2px",
            }}
          >
            {displayedDefinitions.map((def) => {
              const isBuiltin = builtinIds.has(def.id);
              const isFav = favoriteIds.has(def.id);

              return (
                <TemplateCard
                  key={`${def.id}_v${def.version}`}
                  definition={def}
                  isBuiltin={isBuiltin}
                  isFavorite={isFav}
                  onToggleFavorite={handleToggleFavorite}
                  onApply={handleApply}
                  onDelete={isBuiltin ? undefined : handleDeleteUserTemplate}
                  selectedTextAvailable={isTextSelected}
                  prefersReducedMotion={prefersReducedMotion}
                />
              );
            })}

            {/* Pagination / Show more button */}
            {filtered.length > visibleCount && (
              <button
                type="button"
                className="btn-action"
                onClick={() => setVisibleCount((prev) => prev + DEFAULT_PAGE_SIZE)}
                style={{
                  padding: "6px 12px",
                  fontSize: "11px",
                  fontWeight: 600,
                  backgroundColor: "var(--bg-surface)",
                  color: "var(--accent-primary)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "4px",
                  cursor: "pointer",
                  margin: "8px 0",
                  textAlign: "center",
                }}
              >
                Show more ({visibleCount} of {filtered.length})
              </button>
            )}
          </div>
        )}
      </div>

      {/* Authoring Dialog */}
      <TemplateAuthoringDialog
        isOpen={isAuthoringOpen}
        selectedItem={selectedItem}
        assets={assets}
        onClose={() => setIsAuthoringOpen(false)}
        onSaved={async (newDef) => {
          if (onStatusChange) {
            onStatusChange(`Saved template '${newDef.name}' to user catalog`);
          }
          await refreshCatalog();
        }}
      />

      {/* Pack Export Dialog */}
      <TemplatePackExportDialog
        isOpen={isExportPackOpen}
        definitions={definitions}
        assets={assets}
        onClose={() => setIsExportPackOpen(false)}
        onExportSuccess={(packPath) => {
          if (onStatusChange) {
            onStatusChange(`Successfully exported template pack to ${packPath}`);
          }
        }}
      />

      <CapCutTemplateImportDialog
        isOpen={isCapCutImportOpen}
        selectedItem={selectedItem}
        projectRevision={projectRevision}
        onClose={() => setIsCapCutImportOpen(false)}
        onApplied={async (message) => {
          onStatusChange?.(message);
          await onProjectChanged();
        }}
        onSaved={refreshCatalog}
      />
    </aside>
  );
};
