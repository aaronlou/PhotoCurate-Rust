import { create } from "zustand";
import { Photo, Directory, AISettings, ViewMode, NavItem, PhotoSortOrder } from "@/types";
import { addDirectory, getDirectories, getPhotos } from "@/hooks/useInvoke";

interface AppState {
  currentView: NavItem;
  setCurrentView: (view: NavItem) => void;

  photos: Photo[];
  setPhotos: (photos: Photo[]) => void;
  refreshPhotos: (sortOrder?: PhotoSortOrder) => Promise<Photo[]>;
  selectedPhoto: Photo | null;
  setSelectedPhoto: (photo: Photo | null) => void;

  directories: Directory[];
  setDirectories: (dirs: Directory[]) => void;
  refreshDirectories: () => Promise<Directory[]>;
  loadLibrary: () => Promise<void>;
  addDirectoryAndRefresh: (path: string) => Promise<Directory>;

  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;

  photoSortOrder: PhotoSortOrder;
  setPhotoSortOrder: (order: PhotoSortOrder) => void;
  changePhotoSortOrder: (order: PhotoSortOrder) => Promise<void>;

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

function syncSelectedPhoto(photos: Photo[], selectedPhoto: Photo | null) {
  return {
    photos,
    selectedPhoto: selectedPhoto ? photos.find((photo) => photo.id === selectedPhoto.id) ?? null : null,
  };
}

export const useAppStore = create<AppState>((set, get) => ({
  currentView: "library",
  setCurrentView: (view) => set({ currentView: view }),

  photos: [],
  setPhotos: (photos) => set((state) => syncSelectedPhoto(photos, state.selectedPhoto)),
  refreshPhotos: async (sortOrder) => {
    const photos = await getPhotos(sortOrder ?? get().photoSortOrder);
    set((state) => syncSelectedPhoto(photos, state.selectedPhoto));
    return photos;
  },
  selectedPhoto: null,
  setSelectedPhoto: (photo) => set({ selectedPhoto: photo }),

  directories: [],
  setDirectories: (dirs) => set({ directories: dirs }),
  refreshDirectories: async () => {
    const directories = await getDirectories();
    set({ directories });
    return directories;
  },
  loadLibrary: async () => {
    const [directories, photos] = await Promise.all([
      getDirectories(),
      getPhotos(get().photoSortOrder),
    ]);
    set((state) => ({ directories, ...syncSelectedPhoto(photos, state.selectedPhoto) }));
  },
  addDirectoryAndRefresh: async (path) => {
    const directory = await addDirectory(path);
    const [directories, photos] = await Promise.all([
      getDirectories(),
      getPhotos(get().photoSortOrder),
    ]);
    set((state) => ({ directories, ...syncSelectedPhoto(photos, state.selectedPhoto) }));
    return directory;
  },

  viewMode: "grid",
  setViewMode: (mode) => set({ viewMode: mode }),

  photoSortOrder: "date_desc",
  setPhotoSortOrder: (order) => set({ photoSortOrder: order }),
  changePhotoSortOrder: async (order) => {
    set({ photoSortOrder: order });
    const photos = await getPhotos(order);
    set((state) => syncSelectedPhoto(photos, state.selectedPhoto));
  },

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
