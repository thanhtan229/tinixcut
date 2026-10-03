import { Crop, ItemKind, RationalTime, TimeRange, TrackKind, Transform } from "./project";

export type CommandOperation =
  | {
      type: "TrackCreate";
      payload: {
        track_id?: string;
        name: string;
        kind: TrackKind;
      };
    }
  | {
      type: "TrackDelete";
      payload: {
        track_id: string;
      };
    }
  | {
      type: "ClipCreate";
      payload: {
        track_id: string;
        item_id?: string;
        name: string;
        kind: ItemKind;
        asset_id?: string;
        timeline_start: RationalTime;
        duration: RationalTime;
        source_start?: RationalTime;
      };
    }
  | {
      type: "ClipMove";
      payload: {
        item_id: string;
        target_track_id?: string;
        target_start: RationalTime;
      };
    }
  | {
      type: "ClipTrim";
      payload: {
        item_id: string;
        new_timeline_range: TimeRange;
        new_source_range: TimeRange;
      };
    }
  | {
      type: "ClipSplit";
      payload: {
        item_id: string;
        split_time: RationalTime;
        new_right_item_id?: string;
      };
    }
  | {
      type: "ClipDelete";
      payload: {
        item_id: string;
      };
    }
  | {
      type: "ClipDuplicate";
      payload: {
        item_id: string;
        target_start?: RationalTime;
        target_track_id?: string;
        new_item_id?: string;
      };
    }
  | {
      type: "ClipTransformSet";
      payload: {
        item_id: string;
        transform: Transform;
      };
    }
  | {
      type: "ClipCropSet";
      payload: {
        item_id: string;
        crop: Crop;
      };
    }
  | {
      type: "ClipOpacitySet";
      payload: {
        item_id: string;
        opacity: number;
      };
    }
  | {
      type: "ClipVolumeSet";
      payload: {
        item_id: string;
        volume: number;
      };
    }
  | {
      type: "ClipTextSet";
      payload: {
        item_id: string;
        text: string;
      };
    }
  | {
      type: "ClipAudioFadeSet";
      payload: {
        item_id: string;
        fade_in?: RationalTime;
        fade_out?: RationalTime;
      };
    }
  | {
      type: "AssetImport";
      payload: {
        asset_id?: string;
        name: string;
        path: string;
        media_kind: "Video" | "Audio" | "Image";
        file_size: number;
      };
    };

export interface OperationEnvelope {
  operation_id: string;
  operation: CommandOperation;
}

export interface CommandEnvelope {
  schema_version: number;
  project_id: string;
  base_revision: number;
  dry_run: boolean;
  transaction_id?: string;
  operations: OperationEnvelope[];
}

export interface OperationReceipt {
  operation_id: string;
  status: string;
  affected_item_id?: string;
  affected_track_id?: string;
}

export interface CommandReceipt {
  project_id: string;
  prior_revision: number;
  new_revision: number;
  transaction_id?: string;
  operations_applied: number;
  receipts: OperationReceipt[];
}

export interface DiffReport {
  project_id: string;
  base_revision: number;
  operations_evaluated: number;
  changes: unknown[];
  warnings: string[];
}
