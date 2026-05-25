import { useState, useEffect, useRef } from "react";
import { useQuery, useInfiniteQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, Copy, Check, ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Checkbox } from "@/components/ui/checkbox";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";
import { useApiAdapter } from "@/lib/useApiAdapter";
import { toast } from "sonner";
import type { AccessKey, PaginatedResult } from "@/types";
import { cn } from "@/lib/utils";

export function TokenPage() {
  const { t } = useTranslation();
  const api = useApiAdapter();
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [newKeyName, setNewKeyName] = useState("");
  const [createdKey, setCreatedKey] = useState<AccessKey | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<AccessKey | null>(null);

  const {
    data: keysPages,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
    isLoading,
  } = useInfiniteQuery({
    queryKey: ["accessKeys", "paginated"],
    queryFn: ({ pageParam = 1 }) =>
      api.tokens.listPaginated({ page: pageParam, pageSize: 40 }) as Promise<PaginatedResult<AccessKey>>,
    getNextPageParam: (lastPage) =>
      lastPage.page * lastPage.page_size < lastPage.total ? lastPage.page + 1 : undefined,
    initialPageParam: 1,
    staleTime: 2000,
  });
  const keys = keysPages?.pages.flatMap(p => p.items) ?? [];

  const { data: entries = [] } = useQuery({
    queryKey: ["apiEntries", "tokenModels"],
    queryFn: () => api.pool.list(),
    staleTime: 5000,
  });

  const modelOptions = Array.from(
    new Set(entries.map((entry) => entry.model.trim()).filter(Boolean)),
  ).sort((a, b) => a.localeCompare(b));

  const sentinelRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = sentinelRef.current;
    if (!el || !hasNextPage || isFetchingNextPage) return;
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) fetchNextPage(); },
      { rootMargin: "200px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  const createMutation = useMutation({
    mutationFn: (name: string) => api.tokens.create(name),
    onSuccess: (key) => {
      queryClient.invalidateQueries({ queryKey: ["accessKeys", "paginated"] });
      setShowCreate(false);
      setCreatedKey(key);
      setNewKeyName("");
    },
    onError: (err) => {
      toast.error(`${t("token.add")} ${t("common.failed")}: ${err}`);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.tokens.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accessKeys", "paginated"] });
      setDeleteTarget(null);
    },
    onError: (err) => {
      toast.error(`${t("common.delete")} ${t("common.failed")}: ${err}`);
    },
  });

  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.tokens.toggle(id, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["accessKeys", "paginated"] }),
    onError: (err) => {
      toast.error(`${t("common.toggle")} ${t("common.failed")}: ${err}`);
    },
  });

  const updateModelsMutation = useMutation({
    mutationFn: ({ id, allowedModels }: { id: string; allowedModels: string[] | null }) =>
      api.tokens.updateModels(id, allowedModels),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["accessKeys", "paginated"] }),
    onError: (err) => {
      toast.error(`${t("token.availableModels")} ${t("common.failed")}: ${err}`);
    },
  });

  const copyKey = async (key: string, id: string) => {
    await navigator.clipboard.writeText(key);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 3000);
  };

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">{t("common.loading")}</div>;
  }

  const formatDate = (ts: number) => {
    const d = new Date(ts * 1000);
    return d.toLocaleString();
  };

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold">{t("token.title")}</h1>
        <Button size="sm" className="gap-1.5" onClick={() => setShowCreate(true)}>
          <Plus className="h-4 w-4" />
          {t("token.add")}
        </Button>
      </div>

      {keys?.length ? (
        <div className="border rounded-lg overflow-hidden">
          <table className="w-full table-fixed text-sm">
            <thead>
              <tr className="border-b bg-muted/50 text-left text-muted-foreground">
                <th className="px-4 py-2 font-medium w-16">{t("token.enabled")}</th>
                <th className="px-4 py-2 font-medium w-40">{t("token.name")}</th>
                <th className="px-4 py-2 font-medium w-56">{t("token.key")}</th>
                <th className="px-4 py-2 font-medium">{t("token.availableModels")}</th>
                <th className="px-4 py-2 font-medium w-44">{t("token.created")}</th>
                <th className="px-4 py-2 font-medium w-16">{t("common.action")}</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((key) => (
                <tr key={key.id} className="border-b last:border-b-0 hover:bg-muted/30">
                  <td className="px-4 py-3">
                    <Switch
                      checked={key.enabled}
                      onCheckedChange={(checked) =>
                        toggleMutation.mutate({ id: key.id, enabled: checked })
                      }
                    />
                  </td>
                  <td className="px-4 py-3 font-medium truncate" title={key.name}>{key.name}</td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-1 min-w-0">
                      <code className="text-xs bg-muted px-1.5 py-0.5 rounded font-mono truncate flex-1 min-w-0" title={key.key}>
                        {key.key}
                      </code>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-5 w-5 shrink-0 text-muted-foreground"
                        onClick={() => copyKey(key.key, key.id)}
                      >
                        {copiedId === key.id ? (
                          <Check className="h-3 w-3 text-green-600" />
                        ) : (
                          <Copy className="h-3 w-3" />
                        )}
                      </Button>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <ModelPermissionSelect
                      value={key.allowed_models}
                      options={modelOptions}
                      disabled={updateModelsMutation.isPending}
                      onChange={(allowedModels) =>
                        updateModelsMutation.mutate({ id: key.id, allowedModels })
                      }
                    />
                  </td>
                  <td className="px-4 py-3 text-muted-foreground text-xs">{formatDate(key.created_at)}</td>
<td className="px-4 py-3">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => setDeleteTarget(key)}
        >
          <Trash2 className="h-3.5 w-3.5 text-destructive" />
        </Button>
      </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div ref={sentinelRef} className="h-4" />
          {isFetchingNextPage && (
            <div className="flex justify-center py-4 text-sm text-muted-foreground">Loading...</div>
          )}
        </div>
      ) : (
        <div className="flex h-64 items-center justify-center text-muted-foreground">
          {t("common.noData")}
        </div>
      )}

      {/* Create Dialog */}
      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("token.add")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label>{t("token.name")}</Label>
              <Input
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
                placeholder={t("token.deviceNamePlaceholder")}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowCreate(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              onClick={() => createMutation.mutate(newKeyName)}
              disabled={!newKeyName || createMutation.isPending}
            >
              {t("common.add")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

 {/* Created Key Dialog */}
    <Dialog open={!!createdKey} onOpenChange={(v) => !v && setCreatedKey(null)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("token.add")}</DialogTitle>
          <DialogDescription>{t("token.keyWarning")}</DialogDescription>
        </DialogHeader>
        {createdKey && (
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <code className="flex-1 text-sm bg-muted p-3 rounded font-mono break-all">
                {createdKey.key}
              </code>
              <Button
                variant="outline"
                size="icon"
                onClick={() => copyKey(createdKey.key, createdKey.id)}
              >
                {copiedId === createdKey.id ? (
                  <Check className="h-4 w-4 text-green-600" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>
        )}
        <DialogFooter>
          <Button onClick={() => setCreatedKey(null)}>{t("common.close")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    {/* Delete Confirmation Dialog */}
    <Dialog open={!!deleteTarget} onOpenChange={(v) => !v && setDeleteTarget(null)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("common.deleteTitle")}</DialogTitle>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">{t("common.deleteWarning")}</p>
        <DialogFooter>
          <Button variant="outline" onClick={() => setDeleteTarget(null)}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="destructive"
            disabled={deleteMutation.isPending}
            onClick={() => {
              if (deleteTarget) {
                deleteMutation.mutate(deleteTarget.id);
              }
            }}
          >
            {t("common.delete")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
);
}

function ModelPermissionSelect({
  value,
  options,
  disabled,
  onChange,
}: {
  value: string[] | null;
  options: string[];
  disabled: boolean;
  onChange: (value: string[] | null) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const selected = value ?? options;
  const selectedSet = new Set(selected);
  const isAll = value === null;

  const toggleModel = (model: string, checked: boolean) => {
    const next = new Set(selected);
    if (checked) {
      next.add(model);
    } else {
      next.delete(model);
    }
    onChange(Array.from(next).sort((a, b) => a.localeCompare(b)));
  };

  const summary = isAll
    ? t("token.allModels")
    : selected.length === 0
      ? t("token.noModels")
      : selected.length <= 2
        ? selected.join(", ")
        : t("token.selectedModels", { count: selected.length });

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={disabled}
          className={cn("h-8 w-full justify-between gap-2", !isAll && selected.length === 0 && "text-destructive")}
        >
          <span className="truncate">{summary}</span>
          <ChevronDown className="h-3.5 w-3.5 shrink-0 opacity-60" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 p-3">
        <div className="flex items-center justify-between gap-2 border-b pb-2">
          <div className="text-sm font-medium">{t("token.availableModels")}</div>
          <div className="flex gap-1">
            <Button type="button" variant="ghost" size="sm" className="h-7 px-2" onClick={() => onChange(null)}>
              {t("token.selectAll")}
            </Button>
            <Button type="button" variant="ghost" size="sm" className="h-7 px-2" onClick={() => onChange([])}>
              {t("token.clearAll")}
            </Button>
          </div>
        </div>
        {options.length ? (
          <ScrollArea className="mt-2 h-64 pr-2">
            <div className="space-y-1">
              {options.map((model) => (
                <label
                  key={model}
                  className="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-muted"
                >
                  <Checkbox
                    checked={selectedSet.has(model)}
                    onCheckedChange={(checked) => toggleModel(model, checked === true)}
                  />
                  <span className="min-w-0 flex-1 truncate" title={model}>{model}</span>
                </label>
              ))}
            </div>
          </ScrollArea>
        ) : (
          <div className="py-8 text-center text-sm text-muted-foreground">{t("token.noModelOptions")}</div>
        )}
      </PopoverContent>
    </Popover>
  );
}
