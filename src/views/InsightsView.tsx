import { useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  BarChart3,
  CheckCircle2,
  ClipboardList,
  Layers3,
  RefreshCw,
  Sparkles,
  Target,
  TrendingUp,
  WandSparkles,
} from "lucide-react";
import { useAppStore } from "@/stores/useAppStore";
import { getLibraryInsights } from "@/hooks/useInvoke";
import { useI18n } from "@/lib/i18n";
import type { DimensionInsight, LibraryInsights, ScoreBucket, TextInsight } from "@/types";

function formatScore(score: number | null | undefined) {
  return score == null ? "—" : Math.round(score).toString();
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

function formatLoadError(error: unknown, tauriMissingMessage: string) {
  const message = typeof error === "string" ? error : String(error);
  if (message.includes("invoke")) {
    return tauriMissingMessage;
  }
  return message;
}

export default function InsightsView() {
  const { t } = useI18n();
  const setCurrentView = useAppStore((s) => s.setCurrentView);
  const [insights, setInsights] = useState<LibraryInsights | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadInsights = async () => {
    setIsLoading(true);
    setError(null);
    try {
      setInsights(await getLibraryInsights());
    } catch (e) {
      setError(formatLoadError(e, t("insights.tauriMissing")));
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
              {t("insights.title")}
            </h2>
            <p className="mt-1 text-sm text-gray-500">
              {t("insights.subtitle")}
            </p>
          </div>
          <button
            onClick={loadInsights}
            disabled={isLoading}
            className="flex items-center gap-2 rounded-md border border-gray-300 bg-white px-3 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50"
          >
            <RefreshCw size={14} className={isLoading ? "animate-spin" : ""} />
            {t("insights.refresh")}
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
            {t("insights.loading")}
          </div>
        ) : insights ? (
          <div className="space-y-5">
            <section className="grid grid-cols-4 gap-3">
              <MetricCard label={t("insights.evaluatedPhotos")} value={`${insights.evaluated_photos}`} detail={t("insights.totalPhotos", { count: insights.total_photos })} />
              <MetricCard label={t("insights.averageScore")} value={formatScore(insights.average_score)} detail={t("insights.medianScore", { score: formatScore(insights.median_score) })} />
              <MetricCard label={t("insights.highScoreRate")} value={formatPercent(insights.high_score_rate)} detail={t("insights.highScoreDetail", { count: insights.high_score_count })} />
              <MetricCard
                label={t("insights.scoreOnly")}
                value={`${insights.score_only_photos}`}
                detail={t("insights.scoreOnlyDetail")}
              />
            </section>

            {insights.evaluated_photos === 0 ? (
              <EmptyState onStartAnalysis={() => setCurrentView("scoring")} />
            ) : (
              <>
                {(insights.score_only_photos > 0 || insights.evaluated_photos < insights.total_photos) && (
                  <AnalysisPrompt
                    missingCount={Math.max(0, insights.total_photos - insights.evaluated_photos)}
                    scoreOnlyCount={insights.score_only_photos}
                    onStartAnalysis={() => setCurrentView("scoring")}
                  />
                )}

                <InsightReportPreview insights={insights} />

                <section className="grid grid-cols-[1.25fr_0.75fr] gap-5">
                  <div className="rounded-lg border border-gray-200 bg-white p-5">
                    <div className="mb-4 flex items-center justify-between">
                      <div>
                        <h3 className="text-sm font-semibold text-gray-800">{t("insights.scoreDistribution")}</h3>
                        <p className="mt-1 text-xs text-gray-400">{t("insights.scoreDistributionSubtitle")}</p>
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
                    <h3 className="text-sm font-semibold text-gray-800">{t("insights.coachNotes")}</h3>
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
                      <h3 className="text-sm font-semibold text-gray-800">{t("insights.dimensionPerformance")}</h3>
                      <p className="mt-1 text-xs text-gray-400">
                        {t("insights.strongest", { name: strongestDimension?.name ?? "—" })}
                        <span className="mx-1 text-gray-300">/</span>
                        {t("insights.needsWork", { name: weakestDimension?.name ?? "—" })}
                      </p>
                    </div>
                    <DimensionBars dimensions={insights.dimension_averages} />
                  </div>

                  <div className="grid grid-cols-2 gap-5">
                    <TextPanel title={t("insights.stableStrengths")} icon={<CheckCircle2 size={16} />} items={insights.top_strengths} tone="green" />
                    <TextPanel title={t("insights.recurringWeaknesses")} icon={<Target size={16} />} items={insights.recurring_weaknesses} tone="amber" />
                  </div>
                </section>

                <section className="grid grid-cols-[0.9fr_1.1fr] gap-5">
                  <TextPanel title={t("insights.practiceDirections")} icon={<Sparkles size={16} />} items={insights.suggested_practices} tone="blue" />
                  <div className="rounded-lg border border-gray-200 bg-white p-5">
                    <h3 className="text-sm font-semibold text-gray-800">{t("insights.topPhotos")}</h3>
                    <div className="mt-4 space-y-3">
                      {insights.top_photos.length === 0 ? (
                        <p className="text-sm text-gray-400">{t("insights.noTopPhotos")}</p>
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
                    <h3 className="text-sm font-semibold text-gray-800">{t("insights.commonTags")}</h3>
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

function EmptyState({ onStartAnalysis }: { onStartAnalysis: () => void }) {
  const { t } = useI18n();

  return (
    <div className="rounded-lg border border-dashed border-gray-300 bg-white p-8 text-center">
      <Sparkles size={26} className="mx-auto text-blue-500" />
      <h3 className="mt-3 text-sm font-semibold text-gray-800">{t("insights.emptyTitle")}</h3>
      <p className="mx-auto mt-2 max-w-md text-sm leading-6 text-gray-500">
        {t("insights.emptyDescription")}
      </p>
      <button
        type="button"
        onClick={onStartAnalysis}
        className="mx-auto mt-4 flex items-center gap-1.5 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
      >
        <WandSparkles size={15} />
        {t("insights.startAnalysis")}
      </button>
    </div>
  );
}

function AnalysisPrompt({
  missingCount,
  scoreOnlyCount,
  onStartAnalysis,
}: {
  missingCount: number;
  scoreOnlyCount: number;
  onStartAnalysis: () => void;
}) {
  const { t } = useI18n();

  return (
    <section className="flex items-center justify-between gap-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
      <div className="flex items-start gap-2">
        <WandSparkles size={16} className="mt-0.5 shrink-0 text-amber-700" />
        <div>
          <p className="text-sm font-medium text-amber-900">{t("insights.analysisPromptTitle")}</p>
          <p className="mt-1 text-xs text-amber-700">
            {t("insights.analysisPromptMissing", { count: missingCount })}
            {scoreOnlyCount > 0 && t("insights.analysisPromptScoreOnly", { count: scoreOnlyCount })}
          </p>
        </div>
      </div>
      <button
        type="button"
        onClick={onStartAnalysis}
        className="shrink-0 rounded-md bg-amber-600 px-3 py-2 text-xs font-medium text-white hover:bg-amber-700"
      >
        {t("insights.goScoring")}
      </button>
    </section>
  );
}

function InsightReportPreview({ insights }: { insights: LibraryInsights }) {
  const { t } = useI18n();
  const reportItems = [
    {
      icon: <Layers3 size={15} />,
      label: t("insights.artworkProfile"),
      value: t("insights.frequentTags", { count: insights.top_tags.length }),
      detail: t("insights.artworkProfileDetail"),
    },
    {
      icon: <TrendingUp size={15} />,
      label: t("insights.growthTrend"),
      value: insights.recent_trend
        ? `${insights.recent_trend.delta >= 0 ? "+" : ""}${insights.recent_trend.delta.toFixed(1)}`
        : t("insights.pending"),
      detail: t("insights.growthTrendDetail"),
    },
    {
      icon: <ClipboardList size={15} />,
      label: t("insights.practicePlan"),
      value: t("insights.directionCount", { count: insights.suggested_practices.length }),
      detail: t("insights.practicePlanDetail"),
    },
  ];

  return (
    <section className="rounded-lg border border-blue-100 bg-white p-5">
      <div className="flex items-start justify-between gap-5">
        <div className="max-w-2xl">
          <div className="flex items-center gap-2">
            <Sparkles size={16} className="text-blue-600" />
            <h3 className="text-sm font-semibold text-gray-800">{t("insights.deepReport")}</h3>
            <span className="rounded-full bg-blue-50 px-2 py-0.5 text-[11px] font-medium text-blue-700">
              PhotoCurate AI
            </span>
          </div>
          <p className="mt-2 text-sm leading-6 text-gray-500">
            {t("insights.deepReportDescription")}
          </p>
        </div>
        <button
          type="button"
          disabled
          className="shrink-0 rounded-md bg-gray-200 px-3 py-2 text-xs font-medium text-gray-500"
        >
          {t("insights.comingSoon")}
        </button>
      </div>
      <div className="mt-4 grid grid-cols-3 gap-3">
        {reportItems.map((item) => (
          <div key={item.label} className="rounded-md border border-gray-200 bg-gray-50 p-3">
            <div className="flex items-center gap-1.5 text-xs font-medium text-gray-500">
              <span className="text-blue-600">{item.icon}</span>
              {item.label}
            </div>
            <p className="mt-2 text-lg font-semibold tracking-tight text-gray-900">{item.value}</p>
            <p className="mt-1 text-[11px] leading-5 text-gray-500">{item.detail}</p>
          </div>
        ))}
      </div>
    </section>
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
  const { t } = useI18n();

  if (dimensions.length === 0) {
    return <p className="text-sm text-gray-400">{t("insights.noDimensionScores")}</p>;
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
  const { t } = useI18n();
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
          <p className="text-sm text-gray-400">{t("insights.noAggregatedInfo")}</p>
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
