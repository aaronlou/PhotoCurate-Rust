import { useEffect, useMemo, useState } from "react";
import { AlertCircle, BarChart3, CheckCircle2, RefreshCw, Sparkles, Target, TrendingUp } from "lucide-react";
import { getLibraryInsights } from "@/hooks/useInvoke";
import type { DimensionInsight, LibraryInsights, ScoreBucket, TextInsight } from "@/types";

function formatScore(score: number | null | undefined) {
  return score == null ? "—" : Math.round(score).toString();
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

function formatLoadError(error: unknown) {
  const message = typeof error === "string" ? error : String(error);
  if (message.includes("invoke")) {
    return "当前浏览器预览缺少 Tauri 运行环境，请在桌面应用中查看洞察数据。";
  }
  return message;
}

export default function InsightsView() {
  const [insights, setInsights] = useState<LibraryInsights | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadInsights = async () => {
    setIsLoading(true);
    setError(null);
    try {
      setInsights(await getLibraryInsights());
    } catch (e) {
      setError(formatLoadError(e));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadInsights();
  }, []);

  const strongestDimension = useMemo(() => {
    return insights?.dimension_averages[0] ?? null;
  }, [insights]);

  const weakestDimension = useMemo(() => {
    if (!insights || insights.dimension_averages.length === 0) return null;
    return insights.dimension_averages.reduce((lowest, dimension) =>
      dimension.average_score < lowest.average_score ? dimension : lowest
    );
  }, [insights]);

  return (
    <div className="flex h-full flex-col bg-[#f7f8fa]">
      <header className="border-b border-gray-200 bg-white px-6 py-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="flex items-center gap-2 text-lg font-semibold text-gray-800">
              <BarChart3 size={20} className="text-blue-600" />
              作品洞察
            </h2>
            <p className="mt-1 text-sm text-gray-500">
              从评分、点评和维度表现中提炼你的作品模式
            </p>
          </div>
          <button
            onClick={loadInsights}
            disabled={isLoading}
            className="flex items-center gap-2 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50"
          >
            <RefreshCw size={14} className={isLoading ? "animate-spin" : ""} />
            刷新
          </button>
        </div>
      </header>

      <main className="flex-1 overflow-auto p-6">
        {error && (
          <div className="mb-4 flex items-center gap-2 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-600">
            <AlertCircle size={16} />
            {error}
          </div>
        )}

        {isLoading && !insights ? (
          <div className="rounded-lg border border-gray-200 bg-white p-6 text-sm text-gray-500">
            正在生成洞察...
          </div>
        ) : insights ? (
          <div className="space-y-5">
            <section className="grid grid-cols-4 gap-3">
              <MetricCard label="已评价照片" value={`${insights.evaluated_photos}`} detail={`总计 ${insights.total_photos} 张`} />
              <MetricCard label="平均分" value={formatScore(insights.average_score)} detail={`中位数 ${formatScore(insights.median_score)}`} />
              <MetricCard label="高分比例" value={formatPercent(insights.high_score_rate)} detail={`${insights.high_score_count} 张 >= 80`} />
              <MetricCard
                label="旧评分待补全"
                value={`${insights.score_only_photos}`}
                detail="可在评分页补生成评价"
              />
            </section>

            {insights.evaluated_photos === 0 ? (
              <EmptyState />
            ) : (
              <>
                <section className="grid grid-cols-[1.25fr_0.75fr] gap-5">
                  <div className="rounded-lg border border-gray-200 bg-white p-5">
                    <div className="mb-4 flex items-center justify-between">
                      <div>
                        <h3 className="text-sm font-semibold text-gray-800">评分分布</h3>
                        <p className="mt-1 text-xs text-gray-400">观察作品库质量层级，而不是只看单张高分</p>
                      </div>
                      {insights.recent_trend && (
                        <div className={`flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium ${
                          insights.recent_trend.delta >= 0 ? "bg-green-50 text-green-700" : "bg-amber-50 text-amber-700"
                        }`}>
                          <TrendingUp size={13} />
                          {insights.recent_trend.delta >= 0 ? "+" : ""}
                          {insights.recent_trend.delta.toFixed(1)}
                        </div>
                      )}
                    </div>
                    <ScoreDistribution buckets={insights.score_distribution} />
                  </div>

                  <div className="rounded-lg border border-gray-200 bg-white p-5">
                    <h3 className="text-sm font-semibold text-gray-800">教练笔记</h3>
                    <div className="mt-4 space-y-3">
                      {insights.coach_notes.map((note) => (
                        <div key={note} className="flex gap-2 rounded-md bg-gray-50 p-3 text-sm leading-6 text-gray-700">
                          <Sparkles size={15} className="mt-0.5 shrink-0 text-blue-600" />
                          <span>{note}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                </section>

                <section className="grid grid-cols-[0.9fr_1.1fr] gap-5">
                  <div className="rounded-lg border border-gray-200 bg-white p-5">
                    <div className="mb-4">
                      <h3 className="text-sm font-semibold text-gray-800">维度表现</h3>
                      <p className="mt-1 text-xs text-gray-400">
                        最强：{strongestDimension?.name ?? "—"}
                        <span className="mx-1 text-gray-300">/</span>
                        待加强：{weakestDimension?.name ?? "—"}
                      </p>
                    </div>
                    <DimensionBars dimensions={insights.dimension_averages} />
                  </div>

                  <div className="grid grid-cols-2 gap-5">
                    <TextPanel title="稳定优势" icon={<CheckCircle2 size={16} />} items={insights.top_strengths} tone="green" />
                    <TextPanel title="反复短板" icon={<Target size={16} />} items={insights.recurring_weaknesses} tone="amber" />
                  </div>
                </section>

                <section className="grid grid-cols-[0.9fr_1.1fr] gap-5">
                  <TextPanel title="练习方向" icon={<Sparkles size={16} />} items={insights.suggested_practices} tone="blue" />
                  <div className="rounded-lg border border-gray-200 bg-white p-5">
                    <h3 className="text-sm font-semibold text-gray-800">代表高分作品</h3>
                    <div className="mt-4 space-y-3">
                      {insights.top_photos.length === 0 ? (
                        <p className="text-sm text-gray-400">还没有可展示的高分作品。</p>
                      ) : (
                        insights.top_photos.map((photo) => (
                          <div key={photo.id} className="flex items-start justify-between gap-4 border-b border-gray-100 pb-3 last:border-0 last:pb-0">
                            <div className="min-w-0">
                              <p className="truncate text-sm font-medium text-gray-800">{photo.file_name}</p>
                              <p className="mt-1 text-xs leading-5 text-gray-500">{photo.summary}</p>
                            </div>
                            <span className="rounded-md bg-blue-50 px-2 py-1 text-sm font-semibold text-blue-700">
                              {Math.round(photo.score)}
                            </span>
                          </div>
                        ))
                      )}
                    </div>
                  </div>
                </section>

                {insights.top_tags.length > 0 && (
                  <section className="rounded-lg border border-gray-200 bg-white p-5">
                    <h3 className="text-sm font-semibold text-gray-800">常见标签</h3>
                    <div className="mt-4 flex flex-wrap gap-2">
                      {insights.top_tags.map((tag) => (
                        <span key={tag.label} className="rounded-full bg-gray-100 px-3 py-1 text-xs font-medium text-gray-600">
                          {tag.label} · {tag.count}
                        </span>
                      ))}
                    </div>
                  </section>
                )}
              </>
            )}
          </div>
        ) : null}
      </main>
    </div>
  );
}

function MetricCard({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4">
      <p className="text-xs font-medium text-gray-500">{label}</p>
      <p className="mt-2 text-2xl font-semibold tracking-tight text-gray-900">{value}</p>
      <p className="mt-1 text-xs text-gray-400">{detail}</p>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="rounded-lg border border-dashed border-gray-300 bg-white p-8 text-center">
      <Sparkles size={26} className="mx-auto text-blue-500" />
      <h3 className="mt-3 text-sm font-semibold text-gray-800">还没有足够的评价数据</h3>
      <p className="mx-auto mt-2 max-w-md text-sm leading-6 text-gray-500">
        先到评分页为照片生成结构化评价，洞察页会开始汇总你的优势、短板、维度表现和练习方向。
      </p>
    </div>
  );
}

function ScoreDistribution({ buckets }: { buckets: ScoreBucket[] }) {
  const maxCount = Math.max(1, ...buckets.map((bucket) => bucket.count));

  return (
    <div className="space-y-3">
      {buckets.map((bucket) => (
        <div key={bucket.label} className="grid grid-cols-[58px_1fr_36px] items-center gap-3">
          <span className="text-xs font-medium text-gray-500">{bucket.label}</span>
          <div className="h-2.5 overflow-hidden rounded-full bg-gray-100">
            <div
              className="h-full rounded-full bg-blue-600"
              style={{ width: `${(bucket.count / maxCount) * 100}%` }}
            />
          </div>
          <span className="text-right text-xs font-medium text-gray-600">{bucket.count}</span>
        </div>
      ))}
    </div>
  );
}

function DimensionBars({ dimensions }: { dimensions: DimensionInsight[] }) {
  if (dimensions.length === 0) {
    return <p className="text-sm text-gray-400">评价里还没有维度评分。</p>;
  }

  return (
    <div className="space-y-3">
      {dimensions.map((dimension) => (
        <div key={dimension.name}>
          <div className="mb-1 flex items-center justify-between text-xs">
            <span className="font-medium text-gray-600">{dimension.name}</span>
            <span className="text-gray-500">{Math.round(dimension.average_score)}</span>
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-gray-100">
            <div
              className="h-full rounded-full bg-gray-800"
              style={{ width: `${Math.max(0, Math.min(100, dimension.average_score))}%` }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

function TextPanel({
  title,
  icon,
  items,
  tone,
}: {
  title: string;
  icon: React.ReactNode;
  items: TextInsight[];
  tone: "green" | "amber" | "blue";
}) {
  const toneClass = {
    green: "bg-green-50 text-green-700",
    amber: "bg-amber-50 text-amber-700",
    blue: "bg-blue-50 text-blue-700",
  }[tone];

  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5">
      <h3 className="flex items-center gap-2 text-sm font-semibold text-gray-800">
        <span className={toneClass}>{icon}</span>
        {title}
      </h3>
      <div className="mt-4 space-y-2">
        {items.length === 0 ? (
          <p className="text-sm text-gray-400">暂时没有可聚合的信息。</p>
        ) : (
          items.map((item) => (
            <div key={item.label} className="flex items-center justify-between gap-3 rounded-md bg-gray-50 px-3 py-2">
              <span className="min-w-0 truncate text-sm text-gray-700">{item.label}</span>
              <span className="shrink-0 rounded-full bg-white px-2 py-0.5 text-xs font-medium text-gray-500">
                {item.count}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
