import React, { useState } from "react";
import { TemplateDefinition } from "../types/template";

interface TemplateCardProps {
  definition: TemplateDefinition;
  isBuiltin: boolean;
  isFavorite: boolean;
  onToggleFavorite: (id: string) => void;
  onApply: (definition: TemplateDefinition) => void;
  onDelete?: (id: string, version: number) => void;
  selectedTextAvailable?: boolean;
  prefersReducedMotion?: boolean;
}

export const TemplateCard: React.FC<TemplateCardProps> = ({
  definition,
  isBuiltin,
  isFavorite,
  onToggleFavorite,
  onApply,
  onDelete,
  selectedTextAvailable = false,
  prefersReducedMotion = false,
}) => {
  const [isHovered, setIsHovered] = useState(false);
  const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);

  // Extract representative visual properties from definition's layers
  const shapeLayer = definition.layers.find((l) => l.kind.type === "Shape");
  const textLayers = definition.layers.filter((l) => l.kind.type === "Text");

  const bgColor =
    shapeLayer && shapeLayer.kind.type === "Shape" && shapeLayer.kind.data.fill_color
      ? shapeLayer.kind.data.fill_color
      : "#1e1e24";

  const strokeColor =
    shapeLayer && shapeLayer.kind.type === "Shape" && shapeLayer.kind.data.stroke_color
      ? shapeLayer.kind.data.stroke_color
      : "transparent";

  const strokeWidth =
    shapeLayer && shapeLayer.kind.type === "Shape" && shapeLayer.kind.data.stroke_width
      ? Math.min(shapeLayer.kind.data.stroke_width, 4)
      : 0;

  // Active motion effect on hover if motion is allowed
  const shouldAnimate = isHovered && !prefersReducedMotion;

  return (
    <article
      className="template-card"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => {
        setIsHovered(false);
        setIsConfirmingDelete(false);
      }}
      tabIndex={0}
      onKeyDown={(e) => {
        // Prevent child button triggers from bubbling into card apply
        if (e.key === "Enter" && e.target === e.currentTarget) {
          e.preventDefault();
          onApply(definition);
        }
      }}
      style={{
        backgroundColor: "var(--bg-surface)",
        border: isHovered
          ? "1px solid var(--accent-primary)"
          : "1px solid var(--border-color)",
        borderRadius: "6px",
        padding: "10px",
        display: "flex",
        flexDirection: "column",
        gap: "8px",
        position: "relative",
        transition: "border-color 0.15s ease, transform 0.15s ease",
        transform: shouldAnimate ? "translateY(-2px)" : "none",
        outline: "none",
      }}
      aria-label={`Template ${definition.name}`}
    >
      {/* Thumbnail DOM representation */}
      <div
        className="template-thumbnail-box"
        style={{
          height: "90px",
          borderRadius: "4px",
          backgroundColor: bgColor,
          border: strokeWidth > 0 ? `${strokeWidth}px solid ${strokeColor}` : "1px solid rgba(255,255,255,0.06)",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          padding: "8px",
          position: "relative",
          overflow: "hidden",
          boxShadow: "inset 0 0 20px rgba(0,0,0,0.4)",
        }}
        title="Presentation preview only. Authoritative preview and export rendered by native composition engine."
      >
        {/* Subtle motion indicator / preview */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: "4px",
            width: "100%",
            textAlign: "center",
            transition: shouldAnimate ? "transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1)" : "none",
            transform: shouldAnimate ? "scale(1.06)" : "scale(1)",
          }}
        >
          {textLayers.length > 0 ? (
            textLayers.map((lyr, index) => {
              if (lyr.kind.type !== "Text") return null;
              const textData = lyr.kind.data;
              const textContent =
                textData.text_content ||
                (textData.is_primary ? definition.name : `Slot ${index + 1}`);
              const fontColor =
                textData.text_style?.font_color ||
                (index === 0 ? "#ffffff" : "var(--accent-primary)");
              const isTitle = index === 0;

              return (
                <div
                  key={lyr.layer_id}
                  style={{
                    color: fontColor,
                    fontSize: isTitle ? "12px" : "10px",
                    fontWeight: isTitle ? 700 : 500,
                    maxWidth: "90%",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    textShadow: "0 1px 3px rgba(0,0,0,0.8)",
                    letterSpacing: isTitle ? "0.5px" : "normal",
                  }}
                >
                  {textContent}
                </div>
              );
            })
          ) : (
            <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>
              {definition.name}
            </div>
          )}
        </div>

        {/* Presentation-only watermark badge */}
        <span
          style={{
            position: "absolute",
            bottom: "3px",
            right: "5px",
            fontSize: "9px",
            color: "rgba(255,255,255,0.4)",
            pointerEvents: "none",
          }}
        >
          DOM preview
        </span>
      </div>

      {/* Template Info */}
      <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: "4px",
          }}
        >
          <span
            style={{
              fontWeight: 600,
              fontSize: "12px",
              color: "var(--text-main)",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
            title={definition.name}
          >
            {definition.name}
          </span>
          <button
            type="button"
            className="btn-action"
            onClick={(e) => {
              e.stopPropagation();
              onToggleFavorite(definition.id);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.stopPropagation();
              }
            }}
            style={{
              background: "none",
              border: "none",
              cursor: "pointer",
              fontSize: "13px",
              color: isFavorite ? "#fbbf24" : "var(--text-muted)",
              padding: "0 2px",
            }}
            title={isFavorite ? "Remove from favorites" : "Add to favorites"}
            aria-label={isFavorite ? "Remove from favorites" : "Add to favorites"}
          >
            {isFavorite ? "★" : "☆"}
          </button>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "4px", flexWrap: "wrap" }}>
          <span
            style={{
              fontSize: "9px",
              padding: "1px 5px",
              borderRadius: "3px",
              backgroundColor: "rgba(255,255,255,0.06)",
              color: "var(--text-muted)",
            }}
          >
            {definition.category}
          </span>
          <span
            style={{
              fontSize: "9px",
              padding: "1px 4px",
              borderRadius: "3px",
              backgroundColor: isBuiltin
                ? "rgba(59, 130, 246, 0.15)"
                : "rgba(34, 197, 94, 0.15)",
              color: isBuiltin ? "var(--accent-primary)" : "var(--accent-success)",
            }}
          >
            {isBuiltin ? "Built-in" : "User"} v{definition.version}
          </span>
          {definition.layers.length > 1 && (
            <span style={{ fontSize: "9px", color: "var(--text-muted)" }}>
              {definition.layers.length} layers
            </span>
          )}
        </div>

        {definition.description && (
          <p
            style={{
              fontSize: "10px",
              color: "var(--text-muted)",
              margin: "2px 0 0 0",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
            title={definition.description}
          >
            {definition.description}
          </p>
        )}
      </div>

      {/* Card Actions */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          gap: "6px",
          marginTop: "auto",
        }}
      >
        <button
          type="button"
          className="btn-action"
          onClick={(e) => {
            e.stopPropagation();
            onApply(definition);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.stopPropagation();
            }
          }}
          style={{
            flex: 1,
            padding: "4px 8px",
            fontSize: "11px",
            fontWeight: 600,
            backgroundColor: "var(--accent-primary)",
            color: "#ffffff",
            border: "none",
            borderRadius: "4px",
            cursor: "pointer",
            textAlign: "center",
          }}
          title={
            selectedTextAvailable
              ? `Apply ${definition.name} to selected text clip`
              : `Insert ${definition.name} at playhead`
          }
        >
          {selectedTextAvailable ? "✓ Apply" : "+ Insert"}
        </button>

        {!isBuiltin && onDelete && (
          <>
            {isConfirmingDelete ? (
              <div style={{ display: "flex", gap: "4px" }}>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(definition.id, definition.version);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.stopPropagation();
                    }
                  }}
                  style={{
                    padding: "3px 6px",
                    fontSize: "10px",
                    backgroundColor: "#ef4444",
                    color: "#ffffff",
                    border: "none",
                    borderRadius: "3px",
                    cursor: "pointer",
                  }}
                  title="Confirm delete from user catalog"
                >
                  Delete!
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setIsConfirmingDelete(false);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.stopPropagation();
                    }
                  }}
                  style={{
                    padding: "3px 5px",
                    fontSize: "10px",
                    backgroundColor: "transparent",
                    color: "var(--text-muted)",
                    border: "1px solid var(--border-color)",
                    borderRadius: "3px",
                    cursor: "pointer",
                  }}
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                type="button"
                className="btn-action"
                onClick={(e) => {
                  e.stopPropagation();
                  setIsConfirmingDelete(true);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.stopPropagation();
                  }
                }}
                style={{
                  padding: "4px 6px",
                  fontSize: "10px",
                  background: "none",
                  border: "1px solid var(--border-color)",
                  color: "var(--text-muted)",
                  borderRadius: "4px",
                  cursor: "pointer",
                }}
                title="Delete template from user catalog"
                aria-label={`Delete ${definition.name}`}
              >
                🗑
              </button>
            )}
          </>
        )}
      </div>
    </article>
  );
};
