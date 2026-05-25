import { useEffect } from "react";
import { useAppStore } from "@/stores/useAppStore";
import Sidebar from "@/components/Sidebar";
import LibraryView from "@/views/LibraryView";
import ScoringView from "@/views/ScoringView";
import SearchView from "@/views/SearchView";
import ExportView from "@/views/ExportView";
import { getDirectories, getPhotos } from "@/hooks/useInvoke";
import { listen } from "@tauri-apps/api/event";
import type { IndexingProgress } from "@/types";

function App() {
  const currentView = useAppStore((s) => s.currentView);
  const setDirectories = useAppStore((s) => s.setDirectories);
  const setPhotos = useAppStore((s) => s.setPhotos);
  const photoSortOrder = useAppStore((s) => s.photoSortOrder);
  const setIsIndexing = useAppStore((s) => s.setIsIndexing);
  const setIndexProgress = useAppStore((s) => s.setIndexProgress);
  const setIsScoring = useAppStore((s) => s.setIsScoring);
  const setScoreProgress = useAppStore((s) => s.setScoreProgress);

  useEffect(() => {
    getDirectories().then(setDirectories).catch(console.error);
    getPhotos(photoSortOrder).then(setPhotos).catch(console.error);
  }, [setDirectories, setPhotos, photoSortOrder]);

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
          getPhotos(photoSortOrder).then(setPhotos).catch(console.error);
        }
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setIsIndexing, setIndexProgress, setPhotos, photoSortOrder]);

  useEffect(() => {
    const unlisten = listen<IndexingProgress>("scoring-progress", (event) => {
      const { current, total, status } = event.payload;
      if (status === "indexing") {
        setScoreProgress({ current, total });
      } else if (status === "complete") {
        setIsScoring(false);
        setScoreProgress(null);
        getPhotos(photoSortOrder).then(setPhotos).catch(console.error);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setIsScoring, setScoreProgress, setPhotos, photoSortOrder]);

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