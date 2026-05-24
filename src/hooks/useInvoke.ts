import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Photo, Directory, AISettings, SearchResult } from "@/types";

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
  return invoke("add_directory", { path });
}

export async function getDirectories(): Promise<Directory[]> {
  return invoke("get_directories");
}

export async function removeDirectory(id: string): Promise<void> {
  return invoke("remove_directory", { id });
}

// Photo commands
export async function getPhotos(): Promise<Photo[]> {
  return invoke("get_photos");
}

export async function getPhotoById(id: string): Promise<Photo | null> {
  return invoke("get_photo_by_id", { id });
}

export async function getThumbnailPath(photoId: string): Promise<string | null> {
  return invoke("get_thumbnail_path", { photoId });
}

// Scanning
export async function startScanning(directoryId: string): Promise<void> {
  return invoke("start_scanning", { directoryId });
}

// Scoring
export async function scorePhotos(photoIds: string[]): Promise<void> {
  return invoke("score_photos", { photoIds });
}

// Search Index
export async function buildSearchIndex(photoIds: string[]): Promise<void> {
  return invoke("build_search_index", { photoIds });
}

// Search
export async function naturalLanguageSearch(query: string): Promise<SearchResult[]> {
  return invoke("natural_language_search", { query });
}

// Export
export async function exportPhotos(photoIds: string[], destination: string): Promise<void> {
  return invoke("export_photos", { photoIds, destination });
}

// AI Settings
export async function getAiSettings(): Promise<AISettings | null> {
  return invoke("get_ai_settings");
}

export async function updateAiSettings(settings: Partial<AISettings>): Promise<AISettings> {
  return invoke("update_ai_settings", { settings });
}

export async function checkLocalModel(): Promise<boolean> {
  return invoke("check_local_model");
}

export async function validateApiKey(apiKey: string): Promise<{ valid: boolean; message: string }> {
  return invoke("validate_api_key", { apiKey });
}
