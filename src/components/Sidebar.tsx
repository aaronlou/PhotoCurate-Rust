import { useAppStore } from "@/stores/useAppStore";
import UpdateStatus from "@/components/UpdateStatus";
import { useI18n, type LocalePreference, type TranslationKey } from "@/lib/i18n";
import { NavItem } from "@/types";
import {
  Images,
  WandSparkles,
  Search,
  Download,
  BarChart3,
  Sparkles,
  Languages,
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

const workflowItems: { key: NavItem; labelKey: TranslationKey; icon: React.ReactNode }[] = [
  { key: "library", labelKey: "nav.library", icon: <Images size={18} /> },
  { key: "insights", labelKey: "nav.insights", icon: <BarChart3 size={18} /> },
  { key: "search", labelKey: "nav.search", icon: <Search size={18} /> },
  { key: "export", labelKey: "nav.export", icon: <Download size={18} /> },
];

const automationItems: { key: NavItem; labelKey: TranslationKey; icon: React.ReactNode }[] = [
  { key: "scoring", labelKey: "nav.scoring", icon: <WandSparkles size={18} /> },
  { key: "ai_service", labelKey: "nav.aiService", icon: <Sparkles size={18} /> },
];

const isAppStoreBuild = import.meta.env.VITE_APP_STORE === "true";

export default function Sidebar() {
  const currentView = useAppStore((s) => s.currentView);
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const { t } = useI18n();

  const renderItem = (item: { key: NavItem; labelKey: TranslationKey; icon: React.ReactNode }) => (
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
      {t(item.labelKey)}
    </button>
  );

  return (
    <aside className="w-[210px] min-w-[190px] bg-sidebar border-r border-gray-200 flex flex-col select-none">
      <div className="px-4 py-3">
        <h1 className="text-sm font-semibold text-gray-800 tracking-tight">
          PhotoCurate
        </h1>
      </div>
      <nav className="flex-1 px-2">
        <div className="space-y-1">
          <p className="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-gray-400">
            {t("nav.workflow")}
          </p>
          {workflowItems.map(renderItem)}
        </div>
        <div className="mt-5 space-y-1 border-t border-gray-200 pt-3">
          <p className="px-3 pb-1 text-[10px] font-semibold uppercase tracking-wider text-gray-400">
            {t("nav.automation")}
          </p>
          {automationItems.map(renderItem)}
        </div>
      </nav>
      <LanguageSwitcher />
      {!isAppStoreBuild && <UpdateStatus />}
    </aside>
  );
}

function LanguageSwitcher() {
  const { t, localePreference, setLocalePreference } = useI18n();

  return (
    <div className="border-t border-gray-200 px-3 py-3">
      <label className="mb-1.5 flex items-center gap-1.5 text-[11px] font-medium text-gray-500">
        <Languages size={13} />
        {t("language.label")}
      </label>
      <select
        value={localePreference}
        onChange={(event) => setLocalePreference(event.target.value as LocalePreference)}
        aria-label={t("language.label")}
        className="h-8 w-full rounded-md border border-gray-200 bg-white px-2 text-[12px] font-medium text-gray-700 outline-none transition focus:border-blue-300 focus:ring-2 focus:ring-blue-100"
      >
        <option value="auto">{t("language.auto")}</option>
        <option value="zh-CN">{t("language.chinese")}</option>
        <option value="en-US">{t("language.english")}</option>
      </select>
    </div>
  );
}
