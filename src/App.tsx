import { useEffect } from "react";
import { useAppStore } from "@/stores/useAppStore";
import Sidebar from "@/components/Sidebar";
import LibraryView from "@/views/LibraryView";
import ScoringView from "@/views/ScoringView";
import SearchView from "@/views/SearchView";
import ExportView from "@/views/ExportView";
import { getDirectories, getPhotos } from "@/hooks/useInvoke";

function App() {
  const currentView = useAppStore((s) => s.currentView);
  const setDirectories = useAppStore((s) => s.setDirectories);
  const setPhotos = useAppStore((s) => s.setPhotos);

  useEffect(() => {
    // Initial data load
    getDirectories().then(setDirectories).catch(console.error);
    getPhotos().then(setPhotos).catch(console.error);
  }, [setDirectories, setPhotos]);

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
