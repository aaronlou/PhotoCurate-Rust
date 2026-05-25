import { create } from "zustand";
import { Photo, Directory, AISettings, ViewMode, NavItem, PhotoSortOrder } from "@/types";

interface AppState {
  currentView: NavItem;
  setCurrentView: (view: NavItem) => void;

  photos: Photo[];
  setPhotos: (photos: Photo[]) => void;
  selectedPhoto: Photo | null;
  setSelectedPhoto: (photo: Photo | null) => void;

  directories: Directory[];
  setDirectories: (dirs: Directory[]) => void;

  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;

  photoSortOrder: PhotoSortOrder;
  setPhotoSortOrder: (order: PhotoSortOrder) => void;

  aiSettings: AISettings | null;
  setAiSettings: (settings: AISettings | null) => void;

  isScanning: boolean;
  setIsScanning: (v: boolean) => void;
  scanProgress: { current: number; total: number } | null;
  setScanProgress: (p: { current: number; total: number } | null) => void;

  isScoring: boolean;
  setIsScoring: (v: boolean) => void;
  scoreProgress: { current: number; total: number } | null;
  setScoreProgress: (p: { current: number; total: number } | null) => void;

  isIndexing: boolean;
  setIsIndexing: (v: boolean) => void;
  indexProgress: { current: number; total: number } | null;
  setIndexProgress: (p: { current: number; total: number } | null) => void;

  searchQuery: string;
  setSearchQuery: (q: string) => void;
  searchResults: Photo[];
  setSearchResults: (photos: Photo[]) => void;
}

export const useAppStore = create<AppState>((set) => ({
  currentView: "library",
  setCurrentView: (view) => set({ currentView: view }),

  photos: [],
  setPhotos: (photos) => set({ photos }),
  selectedPhoto: null,
  setSelectedPhoto: (photo) => set({ selectedPhoto: photo }),

  directories: [],
  setDirectories: (dirs) => set({ directories: dirs }),

  viewMode: "browser",
  setViewMode: (mode) => set({ viewMode: mode }),

  photoSortOrder: "date_desc",
  setPhotoSortOrder: (order) => set({ photoSortOrder: order }),

  aiSettings: null,
  setAiSettings: (settings) => set({ aiSettings: settings }),

  isScanning: false,
  setIsScanning: (v) => set({ isScanning: v }),
  scanProgress: null,
  setScanProgress: (p) => set({ scanProgress: p }),

  isScoring: false,
  setIsScoring: (v) => set({ isScoring: v }),
  scoreProgress: null,
  setScoreProgress: (p) => set({ scoreProgress: p }),

  isIndexing: false,
  setIsIndexing: (v) => set({ isIndexing: v }),
  indexProgress: null,
  setIndexProgress: (p) => set({ indexProgress: p }),

  searchQuery: "",
  setSearchQuery: (q) => set({ searchQuery: q }),
  searchResults: [],
  setSearchResults: (photos) => set({ searchResults: photos }),
}));
