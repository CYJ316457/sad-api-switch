import { Fragment, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Fingerprint, RotateCcw } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { useEvent } from "@/lib/events";
import { formatTokenCount, formatTokenCountFixed } from "@/lib/numberFormat";
import { useApiAdapter } from "@/lib/useApiAdapter";
import type { UsageLogFilter } from "@/types";

interface UsageLogMeta {
  requested_model?: string;
  resolved_model?: string;
  cache_read_tokens?: number;
  cache_write_tokens?: number;
  client_fingerprint?: string;
  client_user_agent?: string;
  attempt_path?: Array<{
    entry_id?: string;
    channel?: string;
    model?: string;
    status_code?: number;
    success?: boolean;
    error?: string | null;
  }>;
  stream_end_reason?: string;
}

type LogColumnKey =
  | "time"
  | "channel"
  | "token"
  | "model"
  | "duration"
  | "prompt"
  | "completion"
  | "status"
  | "fingerprint";

type LogColumnWidths = Record<LogColumnKey, number>;

const LOG_COLUMN_STORAGE_KEY = "api-switch.log-column-widths";
const DEFAULT_COLUMN_WIDTHS: LogColumnWidths = {
  time: 160,
  channel: 80,
  token: 56,
  model: 96,
  duration: 64,
  prompt: 176,
  completion: 64,
  status: 64,
  fingerprint: 96,
};
const MIN_COLUMN_WIDTHS: LogColumnWidths = {
  time: 140,
  channel: 72,
  token: 52,
  model: 72,
  duration: 64,
  prompt: 120,
  completion: 64,
  status: 56,
  fingerprint: 72,
};

function parseUsageLogMeta(other: string | null | undefined): UsageLogMeta | null {
  if (!other) return null;
  try {
    const parsed = JSON.parse(other);
    return parsed && typeof parsed === "object" ? (parsed as UsageLogMeta) : null;
  } catch {
    return null;
  }
}

function formatAttemptPath(meta: UsageLogMeta | null): string[] {
  return (meta?.attempt_path || [])
    .map((attempt) => {
      const title = [attempt.channel, attempt.model].filter(Boolean).join(" / ");
      const status = attempt.status_code ? ` [${attempt.status_code}]` : "";
      return `${title || "unknown"}${status}`;
    })
    .filter(Boolean);
}

function cacheRate(cacheReadTokens: number, promptTokens: number): string {
  if (cacheReadTokens <= 0 || promptTokens <= 0) return "0";
  return ((cacheReadTokens / promptTokens) * 100).toFixed(1);
}

function shortTokenName(log: { token_name?: string; access_key_name?: string }): string {
  const value = log.token_name || log.access_key_name || "";
  return value.length > 5 ? value.slice(0, 5) : value;
}

function readStoredColumnWidths(): LogColumnWidths {
  if (typeof window === "undefined") return DEFAULT_COLUMN_WIDTHS;
  try {
    const raw = window.localStorage.getItem(LOG_COLUMN_STORAGE_KEY);
    if (!raw) return DEFAULT_COLUMN_WIDTHS;
    const parsed = JSON.parse(raw) as Partial<LogColumnWidths>;
    return {
      ...DEFAULT_COLUMN_WIDTHS,
      ...Object.fromEntries(
        Object.entries(parsed || {}).map(([key, value]) => [
          key,
          typeof value === "number" && Number.isFinite(value)
            ? Math.max(MIN_COLUMN_WIDTHS[key as LogColumnKey], Math.round(value))
            : DEFAULT_COLUMN_WIDTHS[key as LogColumnKey],
        ]),
      ),
    } as LogColumnWidths;
  } catch {
    return DEFAULT_COLUMN_WIDTHS;
  }
}

function ColumnHeader({
  children,
  width,
  onResizeStart,
  className = "",
  canResize = true,
}: {
  children: React.ReactNode;
  width: number;
  onResizeStart: (event: React.MouseEvent<HTMLDivElement>) => void;
  className?: string;
  canResize?: boolean;
}) {
  return (
    <th className={`relative px-3 py-2 text-left font-medium ${className}`.trim()}>
      <div className="truncate pr-2">{children}</div>
      {canResize ? (
        <div
          className="absolute right-0 top-0 h-full w-2 cursor-col-resize select-none touch-none"
          onMouseDown={onResizeStart}
          title="拖拽调整列宽"
        >
          <div className="mx-auto h-full w-px bg-border/70 transition-colors hover:bg-foreground/40" />
        </div>
      ) : null}
      <div
        className="pointer-events-none absolute bottom-0 left-0 h-0.5 bg-border/60"
        style={{ width }}
      />
    </th>
  );
}

export function LogPage() {
  const { t } = useTranslation();
  const api = useApiAdapter();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<UsageLogFilter>({ page: 1, page_size: 100 });
  const [errorsOnly, setErrorsOnly] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [columnWidths, setColumnWidths] = useState<LogColumnWidths>(() =>
    readStoredColumnWidths(),
  );
  const [resizing, setResizing] = useState<{
    key: LogColumnKey;
    startX: number;
    startWidth: number;
  } | null>(null);

  useEvent("new-usage-log", () => {
    queryClient.invalidateQueries({ queryKey: ["usageLogs"] });
  });

  useEffect(() => {
    window.localStorage.setItem(LOG_COLUMN_STORAGE_KEY, JSON.stringify(columnWidths));
  }, [columnWidths]);

  useEffect(() => {
    if (!resizing) return;

    const onMouseMove = (event: MouseEvent) => {
      const delta = event.clientX - resizing.startX;
      const nextWidth = Math.max(
        MIN_COLUMN_WIDTHS[resizing.key],
        Math.round(resizing.startWidth + delta),
      );
      setColumnWidths((current) => {
        if (current[resizing.key] === nextWidth) return current;
        return { ...current, [resizing.key]: nextWidth };
      });
    };

    const onMouseUp = () => {
      setResizing(null);
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";

    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
    };
  }, [resizing]);

  const startResize =
    (key: LogColumnKey) => (event: React.MouseEvent<HTMLDivElement>) => {
      event.preventDefault();
      event.stopPropagation();
      setResizing({ key, startX: event.clientX, startWidth: columnWidths[key] });
    };

  const resetColumnWidths = () => {
    setColumnWidths(DEFAULT_COLUMN_WIDTHS);
  };

  const tableMinWidth = useMemo(
    () => Object.values(columnWidths).reduce((sum, width) => sum + width, 0),
    [columnWidths],
  );

  const { data: result, isLoading } = useQuery({
    queryKey: ["usageLogs", filter],
    queryFn: () => api.usage.getLogs(filter),
  });

  const todayStart = new Date();
  todayStart.setHours(0, 0, 0, 0);
  const todayEnd = new Date(todayStart);
  todayEnd.setDate(todayEnd.getDate() + 1);
  const todayFilter = {
    start_time: Math.floor(todayStart.getTime() / 1000),
    end_time: Math.floor(todayEnd.getTime() / 1000) - 1,
  };
  const { data: todayStats } = useQuery({
    queryKey: ["usageLogs", "todayStats", todayFilter.start_time, todayFilter.end_time],
    queryFn: () => api.usage.getDashboardStats(todayFilter),
  });

  const logs = result?.items || [];
  const logMetas = logs.map((log) => parseUsageLogMeta(log.other));
  const totalCacheRead = logs.reduce(
    (sum, _log, index) => sum + (logMetas[index]?.cache_read_tokens ?? 0),
    0,
  );
  const loadedPromptTokens = logs.reduce((sum, log) => sum + log.prompt_tokens, 0);

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="mb-6 flex items-center justify-between">
          <div className="h-6 w-32 animate-pulse rounded bg-muted" />
          <div className="flex items-center gap-2">
            <div className="h-4 w-12 animate-pulse rounded bg-muted" />
            <div className="h-4 w-12 animate-pulse rounded bg-muted" />
          </div>
        </div>

        <div className="mb-4 grid grid-cols-2 gap-4 xl:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Card key={i}>
              <CardContent className="p-4">
                <div className="mb-2 h-4 w-24 animate-pulse rounded bg-muted" />
                <div className="h-8 w-16 animate-pulse rounded bg-muted" />
              </CardContent>
            </Card>
          ))}
        </div>

        <div className="overflow-x-auto rounded-md border">
          <table className="w-full table-fixed text-sm" style={{ minWidth: `${tableMinWidth}px` }}>
            <colgroup>
              <col style={{ width: `${columnWidths.time}px` }} />
              <col style={{ width: `${columnWidths.channel}px` }} />
              <col style={{ width: `${columnWidths.token}px` }} />
              <col style={{ width: `${columnWidths.model}px` }} />
              <col style={{ width: `${columnWidths.duration}px` }} />
              <col style={{ width: `${columnWidths.prompt}px` }} />
              <col style={{ width: `${columnWidths.completion}px` }} />
              <col style={{ width: `${columnWidths.status}px` }} />
              <col style={{ width: `${columnWidths.fingerprint}px` }} />
            </colgroup>
            <thead>
              <tr className="border-b bg-muted/50">
                <th className="whitespace-nowrap px-3 py-2 text-left font-medium"><div className="h-4 w-20 animate-pulse rounded bg-muted" /></th>
                <th className="truncate px-3 py-2 text-left font-medium"><div className="h-4 w-16 animate-pulse rounded bg-muted" /></th>
                <th className="truncate px-3 py-2 text-left font-medium"><div className="h-4 w-12 animate-pulse rounded bg-muted" /></th>
                <th className="truncate px-3 py-2 text-left font-medium"><div className="h-4 w-24 animate-pulse rounded bg-muted" /></th>
                <th className="whitespace-nowrap px-3 py-2 text-left font-medium"><div className="h-4 w-16 animate-pulse rounded bg-muted" /></th>
                <th className="px-3 py-2 text-right font-medium"><div className="ml-auto h-4 w-12 animate-pulse rounded bg-muted" /></th>
                <th className="px-3 py-2 text-right font-medium"><div className="ml-auto h-4 w-12 animate-pulse rounded bg-muted" /></th>
                <th className="whitespace-nowrap px-3 py-2 text-left font-medium"><div className="h-4 w-14 animate-pulse rounded bg-muted" /></th>
                <th className="whitespace-nowrap px-3 py-2 text-left font-medium"><div className="h-4 w-14 animate-pulse rounded bg-muted" /></th>
              </tr>
            </thead>
            <tbody>
              {Array.from({ length: 5 }).map((_, i) => (
                <tr key={i} className="border-b">
                  <td className="whitespace-nowrap px-3 py-2"><div className="h-4 w-32 animate-pulse rounded bg-muted" /></td>
                  <td className="min-w-0 px-3 py-2"><div className="h-4 w-20 animate-pulse rounded bg-muted" /></td>
                  <td className="min-w-0 px-3 py-2"><div className="h-4 w-16 animate-pulse rounded bg-muted" /></td>
                  <td className="min-w-0 px-3 py-2 font-mono text-xs"><div className="h-4 w-24 animate-pulse rounded bg-muted" /></td>
                  <td className="whitespace-nowrap px-3 py-2"><div className="h-4 w-20 animate-pulse rounded bg-muted" /></td>
                  <td className="px-3 py-2 text-right"><div className="ml-auto h-4 w-10 animate-pulse rounded bg-muted" /></td>
                  <td className="px-3 py-2 text-right"><div className="ml-auto h-4 w-10 animate-pulse rounded bg-muted" /></td>
                  <td className="whitespace-nowrap px-3 py-2"><div className="h-4 w-12 animate-pulse rounded bg-muted" /></td>
                  <td className="whitespace-nowrap px-3 py-2"><div className="h-4 w-12 animate-pulse rounded bg-muted" /></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    );
  }

  const toggleErrorsOnly = (checked: boolean) => {
    setErrorsOnly(checked);
    setFilter((f) => ({
      ...f,
      success: checked ? false : undefined,
      page: 1,
    }));
  };

  return (
    <div className="p-6">
      <div className="mb-6 flex items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">{t("log.title")}</h1>
        <div className="flex items-center gap-2 text-sm">
          <button
            type="button"
            className="inline-flex h-8 w-8 items-center justify-center rounded-md border bg-background text-muted-foreground transition-colors hover:text-foreground"
            onClick={resetColumnWidths}
            title="恢复默认列宽"
          >
            <RotateCcw className="h-3.5 w-3.5" />
          </button>
          <span className="text-muted-foreground">{t("log.all")}</span>
          <Switch checked={errorsOnly} onCheckedChange={toggleErrorsOnly} />
          <span className="text-muted-foreground">{t("log.failed")}</span>
        </div>
      </div>

      <div className="mb-4 grid gap-4 md:grid-cols-4">
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.recentLogs")}</div>
            <div className="mt-1 text-2xl font-semibold">{todayStats?.total_requests ?? 0}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.promptTokens")}</div>
            <div className="mt-1 text-2xl font-semibold">{formatTokenCount(todayStats?.total_prompt_tokens ?? 0)}</div>
            {totalCacheRead > 0 ? (
              <div className="mt-1 text-xs text-muted-foreground">
                {t("log.cacheRead")} {formatTokenCount(totalCacheRead)} {t("log.cacheRate")} {cacheRate(totalCacheRead, loadedPromptTokens)}%
              </div>
            ) : null}
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.completionTokens")}</div>
            <div className="mt-1 text-2xl font-semibold">{formatTokenCount(todayStats?.total_completion_tokens ?? 0)}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-muted-foreground">{t("log.successRate")}</div>
            <div className="mt-1 text-2xl font-semibold">
              {todayStats && todayStats.total_requests > 0
                ? `${(todayStats.success_rate * 100).toFixed(1)}%`
                : "0%"}
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="overflow-x-auto rounded-md border">
        <table className="w-full table-fixed text-sm" style={{ minWidth: `${tableMinWidth}px` }}>
          <colgroup>
            <col style={{ width: `${columnWidths.time}px` }} />
            <col style={{ width: `${columnWidths.channel}px` }} />
            <col style={{ width: `${columnWidths.token}px` }} />
            <col style={{ width: `${columnWidths.model}px` }} />
            <col style={{ width: `${columnWidths.duration}px` }} />
            <col style={{ width: `${columnWidths.prompt}px` }} />
            <col style={{ width: `${columnWidths.completion}px` }} />
            <col style={{ width: `${columnWidths.status}px` }} />
            <col style={{ width: `${columnWidths.fingerprint}px` }} />
          </colgroup>
          <thead>
            <tr className="border-b bg-muted/50">
              <ColumnHeader width={columnWidths.time} onResizeStart={startResize("time")} className="whitespace-nowrap">{t("log.time")}</ColumnHeader>
              <ColumnHeader width={columnWidths.channel} onResizeStart={startResize("channel")} className="truncate">{t("log.channel")}</ColumnHeader>
              <ColumnHeader width={columnWidths.token} onResizeStart={startResize("token")} className="truncate">{t("log.token")}</ColumnHeader>
              <ColumnHeader width={columnWidths.model} onResizeStart={startResize("model")} className="truncate">{t("log.model")}</ColumnHeader>
              <ColumnHeader width={columnWidths.duration} onResizeStart={startResize("duration")} className="whitespace-nowrap">{t("log.duration")}</ColumnHeader>
              <ColumnHeader width={columnWidths.prompt} onResizeStart={startResize("prompt")} className="text-right">{t("log.promptTokens")}</ColumnHeader>
              <ColumnHeader width={columnWidths.completion} onResizeStart={startResize("completion")} className="text-right">{t("log.completionTokens")}</ColumnHeader>
              <ColumnHeader width={columnWidths.status} onResizeStart={startResize("status")} className="whitespace-nowrap">{t("log.status")}</ColumnHeader>
              <ColumnHeader width={columnWidths.fingerprint} onResizeStart={startResize("fingerprint")} className="whitespace-nowrap">{t("log.fingerprint")}</ColumnHeader>
            </tr>
          </thead>
          <tbody>
            {logs.map((log, index) => {
              const isExpanded = expandedId === log.id;
              const meta = logMetas[index] ?? parseUsageLogMeta(log.other);
              const resolvedModel = meta?.resolved_model || log.model;
              const requestedModel = meta?.requested_model || log.requested_model;
              const attemptPath = formatAttemptPath(meta);
              const cacheReadTokens = meta?.cache_read_tokens ?? 0;
              const cacheWriteTokens = meta?.cache_write_tokens ?? 0;
              const fingerprint = meta?.client_fingerprint ?? "";
              const fingerprintTitle =
                meta?.client_user_agent || fingerprint || t("log.unknown");

              return (
                <Fragment key={log.id}>
                  <tr
                    className="cursor-pointer border-b hover:bg-muted/30"
                    onClick={() => setExpandedId(isExpanded ? null : log.id)}
                  >
                    <td className="whitespace-nowrap px-3 py-2">
                      <div>{new Date(log.created_at * 1000).toLocaleString()}</div>
                    </td>
                    <td className="min-w-0 px-2 py-2">
                      <div className="truncate" title={log.channel_name}>
                        {log.channel_name}
                      </div>
                    </td>
                    <td className="min-w-0 px-2 py-2">
                      <div
                        className="truncate"
                        title={log.token_name || log.access_key_name || undefined}
                      >
                        {shortTokenName(log) || (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </div>
                    </td>
                    <td className="min-w-0 px-3 py-2 font-mono text-xs">
                      <div className="truncate" title={resolvedModel}>
                        {resolvedModel}
                      </div>
                    </td>
                    <td className="whitespace-nowrap px-2 py-2">
                      <div
                        className="truncate"
                        title={`${log.use_time || Math.ceil(log.latency_ms / 1000)}s${
                          log.is_stream && log.first_token_ms > 0
                            ? ` / ${(log.first_token_ms / 1000).toFixed(1)}s`
                            : ""
                        }  ${log.is_stream ? t("log.streamShort") : t("log.nonStreamShort")}`}
                      >
                        {`${log.use_time || Math.ceil(log.latency_ms / 1000)}s${
                          log.is_stream && log.first_token_ms > 0
                            ? `/${(log.first_token_ms / 1000).toFixed(1)}s`
                            : ""
                        }`}
                      </div>
                    </td>
                    <td className="px-3 py-2 text-right leading-tight">
                      <div>{formatTokenCountFixed(log.prompt_tokens)}</div>
                      {cacheReadTokens > 0 ? (
                        <>
                          <div className="text-[11px] text-muted-foreground">
                            {t("log.cacheRead")} {formatTokenCount(cacheReadTokens)}
                          </div>
                          <div className="text-[11px] text-muted-foreground">
                            {t("log.cacheRate")} {cacheRate(cacheReadTokens, log.prompt_tokens)}%
                          </div>
                        </>
                      ) : cacheWriteTokens > 0 ? (
                        <div className="text-[11px] text-muted-foreground">
                          {t("log.cacheWrite")} {formatTokenCount(cacheWriteTokens)}
                        </div>
                      ) : null}
                    </td>
                    <td className="px-3 py-2 text-right">
                      {formatTokenCount(log.completion_tokens)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2">
                      <span className={log.success ? "text-green-600" : "text-red-500"}>
                        {log.success ? t("log.success") : t("log.failed")}
                      </span>
                    </td>
                    <td className="whitespace-nowrap px-3 py-2">
                      <div
                        className="inline-flex items-center gap-1 text-xs"
                        title={fingerprintTitle}
                      >
                        <Fingerprint className="h-3 w-3 shrink-0 text-muted-foreground" />
                        <span className="truncate">{fingerprint || "-"}</span>
                      </div>
                    </td>
                  </tr>
                  {isExpanded ? (
                    <tr className="border-b bg-muted/20">
                      <td colSpan={9} className="px-4 py-3">
                        <div className="max-w-3xl space-y-2 text-xs">
                          {meta ? (
                            <div className="grid gap-1 rounded bg-background/60 p-2 text-muted-foreground">
                              <div>
                                <span className="font-medium">{t("log.requestedModel")}:</span>{" "}
                                {requestedModel || "-"}
                              </div>
                              <div>
                                <span className="font-medium">{t("log.resolvedModel")}:</span>{" "}
                                {resolvedModel || "-"}
                              </div>
                              {fingerprint ? (
                                <div>
                                  <span className="font-medium">{t("log.fingerprint")}:</span>{" "}
                                  {fingerprint}
                                </div>
                              ) : null}
                              {attemptPath.length ? (
                                <div>
                                  <span className="font-medium">{t("log.attemptPath")}:</span>{" "}
                                  {attemptPath.join(" -> ")}
                                </div>
                              ) : null}
                              {meta.stream_end_reason ? (
                                <div>
                                  <span className="font-medium">
                                    {t("log.streamEndReason")}:
                                  </span>{" "}
                                  {meta.stream_end_reason}
                                </div>
                              ) : null}
                            </div>
                          ) : log.other ? (
                            <div>
                              <div className="mb-1 font-medium text-muted-foreground">Meta</div>
                              <pre className="whitespace-pre-wrap break-all text-muted-foreground">
                                {log.other}
                              </pre>
                            </div>
                          ) : null}
                          {log.content ? (
                            <div>
                              <div className="mb-1 font-medium text-muted-foreground">
                                {t("log.details")}
                              </div>
                              <pre className="whitespace-pre-wrap break-all">{log.content}</pre>
                            </div>
                          ) : null}
                          {log.error_message ? (
                            <div>
                              <div className="mb-1 font-medium text-red-500">
                                {t("log.error")}
                              </div>
                              <pre className="whitespace-pre-wrap break-all text-red-500">
                                {log.error_message}
                              </pre>
                            </div>
                          ) : null}
                          {!log.content && !log.error_message && !log.other ? (
                            <span className="text-muted-foreground">{t("log.noError")}</span>
                          ) : null}
                        </div>
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>

      {!logs.length && !isLoading && (
        <div className="flex h-32 items-center justify-center text-muted-foreground">
          {t("common.noData")}
        </div>
      )}
    </div>
  );
}
