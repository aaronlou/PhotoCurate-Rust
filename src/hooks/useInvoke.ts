import { Channel, invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import type { Update } from "@tauri-apps/plugin-updater";
import { Photo, Directory, AISettings, SearchResult, ExportResult, PhotoSortOrder, ScoringProvider, LibraryInsights, AppUpdateDownloadEvent, AppUpdateInfo, ScoringRunResult } from "@/types";

const STATE_RETRY_ATTEMPTS = 20;
const STATE_RETRY_DELAY_MS = 250;
let pendingUpdate: Update | null = null;

async function loadUpdater() {
  if (import.meta.env.VITE_APP_STORE === "true") {
    throw new Error("Mac App Store 版本会通过 App Store 更新");
  }
  return import("@tauri-apps/plugin-updater");
}

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function isStateNotManagedError(error: unknown) {
  const message = typeof error === "string" ? error : String(error);
  return message.includes("state not managed") && message.includes("field `state`");
}

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  for (let attempt = 0; attempt < STATE_RETRY_ATTEMPTS; attempt += 1) {
    try {
      return await invoke<T>(command, args);
    } catch (error) {
      if (!isStateNotManagedError(error) || attempt === STATE_RETRY_ATTEMPTS - 1) {
        throw error;
      }
      await sleep(STATE_RETRY_DELAY_MS);
    }
  }

  return invoke<T>(command, args);
}

// Directory commands
export async function pickDirectory(): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
  });
  if (Array.isArray(result)) {
    return result[0] ?? null;
  }
  return result;
}

export async function addDirectory(path: string): Promise<Directory> {
  return invokeCommand("add_directory", { path });
}

export async function getDirectories(): Promise<Directory[]> {
  return invokeCommand("get_directories");
}

export async function removeDirectory(id: string): Promise<void> {
  return invokeCommand("remove_directory", { id });
}

// Photo commands
export async function getPhotos(sortOrder?: PhotoSortOrder): Promise<Photo[]> {
  return invokeCommand("get_photos", { sortOrder });
}

export async function getPhotoById(id: string): Promise<Photo | null> {
  return invokeCommand("get_photo_by_id", { id });
}

export async function getThumbnailPath(photoId: string): Promise<string | null> {
  return invokeCommand("get_thumbnail_path", { photoId });
}

// Scanning
export async function startScanning(directoryId: string): Promise<void> {
  return invokeCommand("start_scanning", { directoryId });
}

// Scoring
export async function scorePhotos(photoIds: string[], allowKeychainRead = false): Promise<ScoringRunResult> {
  return invokeCommand("score_photos", { photoIds, allowKeychainRead });
}

// Search Index
export async function buildSearchIndex(photoIds: string[], allowKeychainRead = false): Promise<number> {
  return invokeCommand("build_search_index", { photoIds, allowKeychainRead });
}

export async function cancelSearchIndexing(): Promise<void> {
  return invokeCommand("cancel_search_indexing");
}

// Search
export async function naturalLanguageSearch(query: string, allowKeychainRead = false): Promise<SearchResult[]> {
  return invokeCommand("natural_language_search", { query, allowKeychainRead });
}

// Export
export async function exportPhotos(
  photoIds: string[],
  destination: string,
  preserveStructure?: boolean
): Promise<ExportResult> {
  return invokeCommand("export_photos", { photoIds, destination, preserveStructure });
}

// Insights
export async function getLibraryInsights(): Promise<LibraryInsights> {
  return invokeCommand("get_library_insights");
}

// AI Settings
export async function getAiSettings(): Promise<AISettings | null> {
  return invokeCommand("get_ai_settings");
}

export async function updateAiSettings(settings: Partial<AISettings>): Promise<AISettings> {
  return invokeCommand("update_ai_settings", { settings });
}

export async function checkLocalModel(): Promise<boolean> {
  return invokeCommand("check_local_model");
}

export interface ValidateApiKeySettings {
  scoring_provider: ScoringProvider;
  scoring_model: string;
  scoring_base_url: string;
  scoring_api_key: string;
}

export async function validateApiKey(settings: ValidateApiKeySettings): Promise<{ valid: boolean; message: string }> {
  return invokeCommand("validate_api_key", { settings });
}

export async function getIndexStats(): Promise<{ count: number; dimension: number | null }> {
  const [count, dimension] = await invokeCommand<[number, number | null]>("get_index_stats");
  return { count, dimension };
}

export async function rebuildAllIndex(allowKeychainRead = false): Promise<number> {
  return invokeCommand("rebuild_all_index", { allowKeychainRead });
}

export async function getAppVersion(): Promise<string> {
  return getVersion();
}

export async function checkForAppUpdate(): Promise<AppUpdateInfo | null> {
  const { check } = await loadUpdater();
  pendingUpdate = await check();
  if (!pendingUpdate) {
    return null;
  }

  return {
    currentVersion: pendingUpdate.currentVersion,
    version: pendingUpdate.version,
    date: pendingUpdate.date ?? null,
    body: pendingUpdate.body ?? null,
  };
}

export async function installAppUpdate(
  onEvent: (event: AppUpdateDownloadEvent) => void
): Promise<void> {
  const { check } = await loadUpdater();
  if (!pendingUpdate) {
    pendingUpdate = await check();
  }
  if (!pendingUpdate) {
    throw new Error("当前已经是最新版本");
  }

  const update = pendingUpdate;
  const channel = new Channel<AppUpdateDownloadEvent>();
  channel.onmessage = onEvent;
  await update.downloadAndInstall((event) => channel.onmessage(event));
  await update.close();
  pendingUpdate = null;
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
