import { invoke } from "@tauri-apps/api/core";
import { HealthStatus } from "../types/health";
import { Project } from "../types/project";
import { CommandEnvelope, CommandReceipt, DiffReport } from "../types/command";

export const editorApi = {
  async getHealth(): Promise<HealthStatus> {
    return invoke<HealthStatus>("app_health_check");
  },

  async getCurrentProject(): Promise<Project> {
    return invoke<Project>("project_get_current");
  },

  async newProject(name: string): Promise<Project> {
    return invoke<Project>("project_new", { name });
  },

  async executeCommand(envelope: CommandEnvelope): Promise<CommandReceipt> {
    return invoke<CommandReceipt>("project_execute_command", { envelope });
  },

  async dryRun(envelope: CommandEnvelope): Promise<DiffReport> {
    return invoke<DiffReport>("project_dry_run", { envelope });
  },

  async saveProject(path?: string): Promise<string> {
    return invoke<string>("project_save", { path });
  },

  async openProject(path: string): Promise<Project> {
    return invoke<Project>("project_open", { path });
  },

  async checkRecovery(path: string): Promise<unknown> {
    return invoke("project_check_recovery", { path });
  },

  async undo(): Promise<string | null> {
    return invoke<string | null>("project_undo");
  },

  async redo(): Promise<string | null> {
    return invoke<string | null>("project_redo");
  },

  async exportProject(outputPath: string, profile?: unknown): Promise<{ output_path: string; duration_secs: number; file_size_bytes: number; render_time_ms: number }> {
    return invoke("project_export", { outputPath, profile });
  },

  async importMedia(filePath: string): Promise<unknown> {
    return invoke("media_import", { filePath });
  },
};
