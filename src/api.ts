import { invoke } from "@tauri-apps/api/core";
import type {
  ImportPreview,
  ImportReport,
  ListQuery,
  NewSave,
  Save,
  SavePatch,
  TagCount,
  VaultStats,
} from "./types";

export const listSaves = (query: ListQuery) =>
  invoke<Save[]>("list_saves", { query });

export const getSave = (id: number) => invoke<Save>("get_save", { id });

export const addSave = (save: NewSave) => invoke<Save>("add_save", { save });

export const updateSave = (id: number, patch: SavePatch) =>
  invoke<Save>("update_save", { id, patch });

export const setFavorite = (id: number, favorite: boolean) =>
  invoke<Save>("set_favorite", { id, favorite });

export const setTags = (id: number, tags: string[]) =>
  invoke<Save>("set_tags", { id, tags });

export const deleteSave = (id: number) => invoke<void>("delete_save", { id });

export const listTags = () => invoke<TagCount[]>("list_tags");

export const vaultStats = () => invoke<VaultStats>("vault_stats");

export const vaultPath = () => invoke<string>("vault_path");

export const checkSaveNow = (id: number) =>
  invoke<Save>("check_save_now", { id });

export const recheckAll = () => invoke<number>("recheck_all");

export const logsPath = () => invoke<string>("logs_path");

export const openLogsDir = () => invoke<void>("open_logs_dir");

export const openVaultDir = () => invoke<void>("open_vault_dir");

export const captureEndpoint = () => invoke<string>("capture_endpoint");

export const previewImport = (path: string) =>
  invoke<ImportPreview>("preview_import", { path });

export const runImport = (path: string) =>
  invoke<ImportReport>("run_import", { path });

export const launchMenubarApp = () => invoke<void>("launch_menubar_app");

export const showMainWindow = () => invoke<void>("show_main_window");

export const hideQuickWindow = () => invoke<void>("hide_quick_window");
