import { useEffect, useState } from "react";
import { CheckCircle2, DownloadCloud, RefreshCw, RotateCw, X } from "lucide-react";
import {
  checkForAppUpdate,
  getAppVersion,
  installAppUpdate,
} from "@/hooks/useInvoke";
import type { AppUpdateInfo } from "@/types";

type UpdateState = "idle" | "checking" | "ready" | "downloading" | "restarting" | "current" | "error";

function formatDate(value: string | null) {
  if (!value) {
    return null;
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return null;
  }
  return date.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export default function UpdateStatus() {
  const [state, setState] = useState<UpdateState>("checking");
  const [version, setVersion] = useState<string>("--");
  const [update, setUpdate] = useState<AppUpdateInfo | null>(null);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState<number | null>(null);
  const [message, setMessage] = useState<string>("正在检查...");

  useEffect(() => {
    getAppVersion().then(setVersion).catch(() => setVersion("--"));
  }, []);

  useEffect(() => {
    let cancelled = false;
    async function checkOnStart() {
      await handleCheck({ quiet: true, cancelled: () => cancelled });
    }
    checkOnStart().catch(console.error);
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleCheck(options?: { quiet?: boolean; cancelled?: () => boolean }) {
    if (!options?.quiet) {
      setState("checking");
      setMessage("正在检查...");
    }
    try {
      const nextUpdate = await checkForAppUpdate();
      if (options?.cancelled?.()) {
        return;
      }
      if (nextUpdate) {
        setUpdate(nextUpdate);
        setState("ready");
        setMessage(`新版本 ${nextUpdate.version}`);
      } else {
        setUpdate(null);
        setState("current");
        setMessage("已是最新版本");
      }
    } catch (error) {
      setState("error");
      setMessage(typeof error === "string" ? error : "检查失败，请稍后重试");
    }
  }

  async function handleInstall() {
    setState("downloading");
    setDownloadedBytes(0);
    setTotalBytes(null);
    setMessage("正在下载更新...");

    try {
      await installAppUpdate((event) => {
        if (event.event === "Started") {
          setTotalBytes(event.data.contentLength ?? null);
        } else if (event.event === "Progress") {
          setDownloadedBytes((current) => current + event.data.chunkLength);
        } else if (event.event === "Finished") {
          setState("restarting");
          setMessage("安装完成，正在重启...");
        }
      });
    } catch (error) {
      setState("error");
      setMessage(typeof error === "string" ? error : "更新失败，请重新尝试");
    }
  }

  const isBusy = state === "checking" || state === "downloading" || state === "restarting";
  const progress = totalBytes ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100)) : 0;
  const releaseDate = formatDate(update?.date ?? null);

  return (
    <div className="border-t border-gray-200 px-3 py-3">
      <div className="mb-2 flex items-center justify-between text-[11px] text-gray-500">
        <span>当前版本</span>
        <span className="font-medium text-gray-700">v{version}</span>
      </div>

      {state === "ready" && (
        <div className="space-y-2">
          <button
            type="button"
            onClick={handleInstall}
            className="flex h-9 w-full items-center justify-center gap-1.5 rounded-full bg-blue-500 px-4 text-[13px] font-semibold text-white shadow-sm transition hover:bg-blue-600"
          >
            <RotateCw size={14} />
            更新
          </button>
          <div className="rounded-md bg-blue-50 px-2.5 py-2 text-[11px] leading-4 text-blue-700">
            <p className="font-semibold">{message}</p>
            {releaseDate && <p className="text-blue-500">{releaseDate}</p>}
            {update?.body && <p className="mt-1 line-clamp-2 whitespace-pre-line">{update.body}</p>}
          </div>
        </div>
      )}

      {state !== "ready" && (
        <button
          type="button"
          onClick={() => handleCheck()}
          disabled={isBusy}
          className="flex h-8 w-full items-center justify-center gap-1.5 rounded-md border border-gray-200 bg-white px-2 text-[12px] font-medium text-gray-700 transition hover:border-gray-300 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-70"
        >
          {state === "checking" && <RefreshCw size={14} className="animate-spin" />}
          {state === "current" && <CheckCircle2 size={14} className="text-green-600" />}
          {state === "error" && <X size={14} className="text-red-500" />}
          {(state === "idle" || state === "downloading" || state === "restarting") && (
            <DownloadCloud size={14} />
          )}
          <span>{message}</span>
        </button>
      )}

      {state === "downloading" && (
        <div className="mt-2">
          <div className="h-1.5 overflow-hidden rounded-full bg-gray-200">
            <div
              className="h-full rounded-full bg-blue-600 transition-all"
              style={{ width: totalBytes ? `${progress}%` : "35%" }}
            />
          </div>
          <p className="mt-1 text-[11px] text-gray-500">
            {totalBytes ? `${progress}%` : "正在连接下载源..."}
          </p>
        </div>
      )}
    </div>
  );
}
