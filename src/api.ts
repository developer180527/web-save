import { invoke } from "@tauri-apps/api/core";
import type {
  ExtensionStatus,
  ImportPreview,
  ImportReport,
  ListQuery,
  NewSave,
  Save,
  SavePatch,
  SavedSearch,
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

export const setRead = (id: number, isRead: boolean) =>
  invoke<Save>("set_read", { id, isRead });

export const setUrl = (id: number, url: string) =>
  invoke<Save>("set_url", { id, url });

export const bulkSetFavorite = (ids: number[], favorite: boolean) =>
  invoke<void>("bulk_set_favorite", { ids, favorite });

export const bulkSetRead = (ids: number[], isRead: boolean) =>
  invoke<void>("bulk_set_read", { ids, isRead });

export const bulkDelete = (ids: number[]) =>
  invoke<void>("bulk_delete", { ids });

export const bulkAddTag = (ids: number[], tag: string) =>
  invoke<void>("bulk_add_tag", { ids, tag });

export const listSavedSearches = () =>
  invoke<SavedSearch[]>("list_saved_searches");

export const addSavedSearch = (name: string, query: ListQuery) =>
  invoke<SavedSearch>("add_saved_search", { name, query });

export const deleteSavedSearch = (id: number) =>
  invoke<void>("delete_saved_search", { id });

export const deleteSave = (id: number) => invoke<void>("delete_save", { id });

export const listTags = () => invoke<TagCount[]>("list_tags");

export const getArchive = (id: number) =>
  invoke<string | null>("get_archive", { id });

export const vaultStats = () => invoke<VaultStats>("vault_stats");

export const extensionStatus = () =>
  invoke<ExtensionStatus>("extension_status");

export const vaultPath = () => invoke<string>("vault_path");

export const appVersion = () => invoke<string>("app_version");

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
