import { useEffect } from "react";
import { useAppStore } from "@/stores/useAppStore";
import Sidebar from "@/components/Sidebar";
import LibraryView from "@/views/LibraryView";
import ScoringView from "@/views/ScoringView";
import SearchView from "@/views/SearchView";
import ExportView from "@/views/ExportView";
import { listen } from "@tauri-apps/api/event";
import type { IndexingProgress } from "@/types";

function App() {
  const currentView = useAppStore((s) => s.currentView);
  const photoSortOrder = useAppStore((s) => s.photoSortOrder);
  const loadLibrary = useAppStore((s) => s.loadLibrary);
  const refreshPhotos = useAppStore((s) => s.refreshPhotos);
  const setIsIndexing = useAppStore((s) => s.setIsIndexing);
  const setIndexProgress = useAppStore((s) => s.setIndexProgress);
  const setIsScoring = useAppStore((s) => s.setIsScoring);
  const setScoreProgress = useAppStore((s) => s.setScoreProgress);

  useEffect(() => {
    loadLibrary().catch(console.error);
  }, [loadLibrary, photoSortOrder]);

  useEffect(() => {
    const unlisten = listen<IndexingProgress>("indexing-progress", (event) => {
      const { current, total, status } = event.payload;
      if (status === "indexing" || status === "started") {
        setIsIndexing(true);
        setIndexProgress({ current, total });
      } else {
        setIsIndexing(false);
        setIndexProgress(null);
        if (status === "complete") {
          refreshPhotos(photoSortOrder).catch(console.error);
        }
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setIsIndexing, setIndexProgress, refreshPhotos, photoSortOrder]);

  useEffect(() => {
    const unlisten = listen<IndexingProgress>("scoring-progress", (event) => {
      const { current, total, status } = event.payload;
      if (status === "indexing") {
        setScoreProgress({ current, total });
      } else if (status === "complete") {
        setIsScoring(false);
        setScoreProgress(null);
        refreshPhotos(photoSortOrder).catch(console.error);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setIsScoring, setScoreProgress, refreshPhotos, photoSortOrder]);

  return (
    <div className="flex h-screen w-screen bg-white">
      <Sidebar />
      <main className="flex-1 overflow-hidden">
        {currentView === "library" && <LibraryView />}
        {currentView === "scoring" && <ScoringView />}
        {currentView === "search" && <SearchView />}
        {currentView === "export" && <ExportView />}
      </main>
    </div>
  );
}

export default App;
