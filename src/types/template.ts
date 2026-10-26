import { PlacementMode, RationalTime, TextAnchor, TextStyle, Transform } from "./project";

export type TemplatePropertyPath =
  | "TextContent"
  | "FontColor"
  | "FontSize"
  | "FontFamily"
  | "BackgroundColor"
  | "PositionX"
  | "PositionY"
  | "ScaleX"
  | "ScaleY"
  | "Rotation"
  | "Opacity"
  | "FillColor"
  | "StrokeColor"
  | "StrokeWidth"
  | "CornerRadius"
  | "Width"
  | "Height"
  | "Visible";

export type TemplateParameterType =
  | { type: "Text" }
  | { type: "Color" }
  | { type: "Number"; min: number; max: number; step: number }
  | { type: "Boolean" }
  | { type: "Enum"; options: string[] };

export type TemplateParameterValue =
  | { kind: "Text"; value: string }
  | { kind: "Color"; value: string }
  | { kind: "Number"; value: number }
  | { kind: "Boolean"; value: boolean }
  | { kind: "Enum"; value: string };

export interface TemplateParameterDefinition {
  key: string;
  name: string;
  param_type: TemplateParameterType;
  default_value: TemplateParameterValue;
  description?: string | null;
}

export interface TemplateParameterBinding {
  parameter_key: string;
  target_layer_id: string;
  target_property: TemplatePropertyPath;
}

export interface TemplateOverride {
  target_layer_id: string;
  property: TemplatePropertyPath;
  value: TemplateParameterValue;
}

export type TemplateShapeKind =
  | "Rectangle"
  | "RoundedRectangle"
  | "Ellipse"
  | "Circle"
  | "Line";

export interface TemplateShapeLayerData {
  shape_kind: TemplateShapeKind;
  width: number;
  height: number;
  fill_color?: string | null;
  stroke_color?: string | null;
  stroke_width: number;
  corner_radius: number;
}

export interface TemplateTextLayerData {
  is_primary: boolean;
  slot_key?: string | null;
  text_content?: string | null;
  text_style?: TextStyle | null;
  text_rich?: import("./project").TextRichStyle | null;
  text_animation?: import("./project").TextAnimation | null;
}

export interface TemplateImageLayerData {
  asset_id?: string | null;
  asset_name?: string | null;
  asset_hash?: string | null;
  width: number;
  height: number;
  placement_mode: PlacementMode;
}

export type TemplateLayerKind =
  | { type: "Text"; data: TemplateTextLayerData }
  | { type: "Shape"; data: TemplateShapeLayerData }
  | { type: "Image"; data: TemplateImageLayerData };

export interface TemplateLayer {
  layer_id: string;
  name: string;
  z_index: number;
  visible: boolean;
  opacity: number;
  transform: Transform;
  anchor: TextAnchor;
  pivot: [number, number];
  motion?: import("./project").TextAnimation | null;
  kind: TemplateLayerKind;
}

export type TemplateResourceKind = "Font" | "Image" | "Thumbnail";

export interface TemplateResourceReference {
  resource_id: string;
  kind: TemplateResourceKind;
  file_name: string;
  sha256: string;
  byte_size: number;
  required: boolean;
}

export interface TemplateResourceManifest {
  resources: TemplateResourceReference[];
}

export interface ResponsivePolicy {
  reference_width: number;
  reference_height: number;
  supported_aspect_ratios: string[];
  auto_wrap: boolean;
  auto_fit: boolean;
  safe_margin: number;
}

export interface TemplateDefinition {
  id: string;
  version: number;
  name: string;
  category: string;
  description: string;
  tags: string[];
  parameters: TemplateParameterDefinition[];
  bindings: TemplateParameterBinding[];
  layers: TemplateLayer[];
  resources: TemplateResourceManifest;
  responsive_policy: ResponsivePolicy;
  default_duration: RationalTime;
}

export interface TemplateInstance {
  template_id: string;
  template_version: number;
  layers: TemplateLayer[];
  parameters: Record<string, TemplateParameterValue>;
  initial_parameters: Record<string, TemplateParameterValue>;
  overrides: TemplateOverride[];
  responsive_policy?: ResponsivePolicy | null;
}

export interface TemplatePackManifest {
  pack_id: string;
  pack_version: number;
  name: string;
  author?: string | null;
  description?: string | null;
  template_ids: string[];
  created_at: string;
}
