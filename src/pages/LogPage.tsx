import { Fragment, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Fingerprint } from "lucide-react";
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

export function LogPage() {
  const { t } = useTranslation();
  const api = useApiAdapter();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<UsageLogFilter>({ page: 1, page_size: 100 });
  const [errorsOnly, setErrorsOnly] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);

  useEvent("new-usage-log", () => {
    queryClient.invalidateQueries({ queryKey: ["usageLogs"] });
  });

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

        <div className="overflow-x-hidden rounded-md border">
          <table className="w-full table-fixed text-sm">
            <colgroup>
              <col className="w-40" />
              <col className="w-28" />
              <col className="w-24" />
              <col />
              <col className="w-28" />
              <col className="w-16" />
              <col className="w-16" />
              <col className="w-20" />
              <col className="w-24" />
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
                  <td className="whitespace-nowrap px-3 py-2"><div className="h-4 w-28 animate-pulse rounded bg-muted" /></td>
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
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-xl font-semibold">{t("log.title")}</h1>
        <div className="flex items-center gap-2 text-sm">
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

      <div className="overflow-x-hidden rounded-md border">
        <table className="w-full table-fixed text-sm">
          <colgroup>
            <col className="w-40" />
            <col className="w-20" />
            <col className="w-14" />
            <col className="w-32" />
            <col className="w-20" />
            <col className="w-36" />
            <col className="w-16" />
            <col className="w-20" />
            <col className="w-24" />
          </colgroup>
          <thead>
            <tr className="border-b bg-muted/50">
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium">{t("log.time")}</th>
              <th className="truncate px-3 py-2 text-left font-medium">{t("log.channel")}</th>
              <th className="truncate px-3 py-2 text-left font-medium">{t("log.token")}</th>
              <th className="truncate px-3 py-2 text-left font-medium">{t("log.model")}</th>
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium">{t("log.duration")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.promptTokens")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("log.completionTokens")}</th>
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium">{t("log.status")}</th>
              <th className="whitespace-nowrap px-3 py-2 text-left font-medium">{t("log.fingerprint")}</th>
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
