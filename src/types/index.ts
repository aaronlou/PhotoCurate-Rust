export interface Photo {
  id: string;
  file_path: string;
  file_name: string;
  file_size: number;
  date_created: string | null;
  date_modified: string;
  camera_make: string | null;
  camera_model: string | null;
  lens_model: string | null;
  focal_length: number | null;
  aperture: number | null;
  shutter_speed: number | null;
  iso: number | null;
  width: number | null;
  height: number | null;
  aesthetic_score: number | null;
  has_been_scored: boolean;
  score_date: string | null;
  has_embedding: boolean;
  embedding_version: number | null;
  thumbnail_path: string | null;
  directory_id: string | null;
  has_been_exported: boolean;
  export_date: string | null;
}

export interface Directory {
  id: string;
  path: string;
  is_monitoring: boolean;
  date_added: string;
  photo_count?: number;
}

export interface AISettings {
  id: string;
  provider: "gemini";
  api_key: string;
  has_api_key: boolean;
  key_storage: "keychain" | "database" | "memory";
  scoring_provider: ScoringProvider;
  scoring_model: string;
  scoring_base_url: string;
  scoring_api_key: string;
  has_scoring_api_key: boolean;
  scoring_key_storage: "keychain" | "database" | "memory";
  ollama_base_url: string;
  ollama_embed_model: string;
  ollama_vision_model: string;
}

export type ScoringProvider = "gemini" | "qwen_vl" | "openai_compatible_vision";

export interface ScoreResult {
  score: number;
  review: string;
}

export interface SearchResult {
  photo: Photo;
  similarity: number;
}

export interface ExportFailure {
  id: string;
  file_name: string;
  error: string;
}

export interface ExportResult {
  exported_count: number;
  failed_count: number;
  failed_photos: ExportFailure[];
}

export interface IndexingProgress {
  current: number;
  total: number;
  status: "started" | "indexing" | "complete" | "unavailable" | "failed";
}

export type ViewMode = "grid" | "list";
export type NavItem = "library" | "scoring" | "search" | "export";
export type PhotoSortOrder = "date_desc" | "score_desc" | "score_asc";
