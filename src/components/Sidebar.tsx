import { useAppStore } from "@/stores/useAppStore";
import { NavItem } from "@/types";
import {
  Images,
  Star,
  Search,
  Download,
  BarChart3,
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

const navItems: { key: NavItem; label: string; icon: React.ReactNode }[] = [
  { key: "library", label: "图库", icon: <Images size={18} /> },
  { key: "scoring", label: "评分", icon: <Star size={18} /> },
  { key: "insights", label: "洞察", icon: <BarChart3 size={18} /> },
  { key: "search", label: "智能检索", icon: <Search size={18} /> },
  { key: "export", label: "精选导出", icon: <Download size={18} /> },
];

export default function Sidebar() {
  const currentView = useAppStore((s) => s.currentView);
  const setCurrentView = useAppStore((s) => s.setCurrentView);

  return (
    <aside className="w-[210px] min-w-[190px] bg-sidebar border-r border-gray-200 flex flex-col select-none">
      <div className="px-4 py-3">
        <h1 className="text-sm font-semibold text-gray-800 tracking-tight">
          PhotoCurate
        </h1>
      </div>
      <nav className="flex-1 px-2 space-y-1">
        {navItems.map((item) => (
          <button
            key={item.key}
            onClick={() => setCurrentView(item.key)}
            className={cn(
              "w-full flex items-center gap-2.5 px-3 py-2 rounded-md text-[13px] font-medium transition-colors",
              currentView === item.key
                ? "bg-blue-50 text-blue-600"
                : "text-gray-600 hover:bg-gray-100 hover:text-gray-900"
            )}
          >
            {item.icon}
            {item.label}
          </button>
        ))}
      </nav>
    </aside>
  );
}
