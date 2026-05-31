import { useRef, useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { useApiAdapter, isTauriRuntime } from "@/lib/useApiAdapter";
import { toast } from "sonner";
import { DEFAULT_SETTINGS, type AppBackupPayload, type AppSettings } from "@/types";
import { SettingsEditor } from "@/features/settings/SettingsEditor";
import { Button } from "@/components/ui/button";
import { Download, Upload } from "lucide-react";

type SaveFilePickerHandle = {
  createWritable: () => Promise<{
    write: (data: string) => Promise<void>;
    close: () => Promise<void>;
  }>;
};

type WindowWithSaveFilePicker = Window & {
  showSaveFilePicker?: (options: {
    suggestedName?: string;
    types?: Array<{
      description: string;
      accept: Record<string, string[]>;
    }>;
  }) => Promise<SaveFilePickerHandle>;
};
async function saveTextFile(filename: string, text: string) {
  const picker = (window as WindowWithSaveFilePicker).showSaveFilePicker;
  if (picker) {
    const handle = await picker({
      suggestedName: filename,
      types: [
        {
          description: "JSON",
          accept: { "application/json": [".json"] },
        },
      ],
    });
    const writable = await handle.createWritable();
    await writable.write(text);
    await writable.close();
    return;
  }

  const blob = new Blob([text], { type: "application/json;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const adapter = useApiAdapter();
  const importInputRef = useRef<HTMLInputElement>(null);
  const [importing, setImporting] = useState(false);

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: adapter.settings.get,
  });

  const { data: groups = ["auto"] } = useQuery({
    queryKey: ["pool-groups"],
    queryFn: () => adapter.pool.getGroups(),
  });

  const { data: proxyStatus } = useQuery({
    queryKey: ["proxyStatus"],
    queryFn: adapter.proxy.getStatus,
  });

  const updateMutation = useMutation({
    mutationFn: adapter.settings.update,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      queryClient.invalidateQueries({ queryKey: ["proxyStatus"] });
      queryClient.invalidateQueries({ queryKey: ["pool-groups"] });
      queryClient.invalidateQueries({ queryKey: ["adminStatus"] });
    },
    onError: (err) => {
      toast.error(`设置保存失败: ${err}`, { duration: Infinity });
    },
  });

  const s = { ...DEFAULT_SETTINGS, ...settings };

  const refreshAfterBackupImport = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["settings"] }),
      queryClient.invalidateQueries({ queryKey: ["proxyStatus"] }),
      queryClient.invalidateQueries({ queryKey: ["pool-groups"] }),
      queryClient.invalidateQueries({ queryKey: ["adminStatus"] }),
      queryClient.invalidateQueries({ queryKey: ["channels"] }),
      queryClient.invalidateQueries({ queryKey: ["channels", "paginated"] }),
      queryClient.invalidateQueries({ queryKey: ["poolEntries"] }),
      queryClient.invalidateQueries({ queryKey: ["accessKeys"] }),
    ]);
  };

  const handleExportSettings = async () => {
    try {
      const payload = await adapter.backup.export();
      const stamp = new Date().toISOString().replace(/[:.]/g, "-");
      await saveTextFile(`api-switch-backup-${stamp}.json`, JSON.stringify(payload, null, 2));
      toast.success(t("settings.backup.exportSuccess"));
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") return;
      toast.error(`${t("settings.backup.exportFailed")}: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const importSettingsBackup = async (file: File) => {
    setImporting(true);
    try {
      const text = await file.text();
      const payload = JSON.parse(text) as AppBackupPayload;
      if (!payload || typeof payload !== "object" || !Array.isArray(payload.channels) || !Array.isArray(payload.api_entries) || !Array.isArray(payload.access_keys) || !payload.settings) {
        throw new Error("Invalid app backup file");
      }
      if (payload.settings.lan_share_enabled) {
        payload.settings.access_key_required = true;
      }
      await adapter.backup.import(payload);
      if (payload.settings.locale) {
        i18n.changeLanguage(payload.settings.locale);
        localStorage.setItem("api-switch-locale", payload.settings.locale);
      }
      if (payload.settings.active_group) {
        localStorage.setItem("api-switch-default-group", payload.settings.active_group);
      }
      await refreshAfterBackupImport();
      toast.success(t("settings.backup.importSuccess"));
    } catch (err) {
      toast.error(`${t("settings.backup.importFailed")}: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setImporting(false);
      if (importInputRef.current) importInputRef.current.value = "";
    }
  };

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    if (key === "locale") {
      i18n.changeLanguage(value as string);
      localStorage.setItem("api-switch-locale", value as string);
    }
    if (key === "active_group") {
      // Persist the remembered default group for the API Management page locally for faster UI restoration.
      localStorage.setItem("api-switch-default-group", value as string);
    }
    const next = { ...s, [key]: value };
    if (key === "lan_share_enabled" && value === true) {
      next.access_key_required = true;
    }
    updateMutation.mutate(next);
  };

  const toggleProxy = async (enabled: boolean) => {
    try {
      if (enabled) {
        await adapter.proxy.start();
      } else {
        await adapter.proxy.stop();
      }
        queryClient.invalidateQueries({ queryKey: ["proxyStatus"] });
        queryClient.invalidateQueries({ queryKey: ["adminStatus"] });
        queryClient.invalidateQueries({ queryKey: ["settings"] });
      } catch (err) {
      const action = enabled ? t("settings.proxy.start") : t("settings.proxy.stop");
      toast.error(`${action} ${t("common.failed")}: ${err}`, { duration: Infinity });
    }
  };

  return (
    <div className="p-6 max-w-2xl">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">{t("settings.title")}</h1>
        <div className="flex items-center gap-2">
          <input
            ref={importInputRef}
            type="file"
            accept="application/json,.json,.txt"
            className="hidden"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) void importSettingsBackup(file);
            }}
          />
          <Button
            size="sm"
            variant="outline"
            className="gap-1.5"
            onClick={() => importInputRef.current?.click()}
            disabled={importing}
          >
            <Upload className="h-4 w-4" />
            {importing ? t("settings.backup.importing") : t("settings.backup.import")}
          </Button>
          <Button size="sm" variant="outline" className="gap-1.5" onClick={handleExportSettings}>
            <Download className="h-4 w-4" />
            {t("settings.backup.export")}
          </Button>
        </div>
      </div>
      <SettingsEditor
        settings={s}
        proxyStatus={proxyStatus}
        appVersion={s.app_version}
        isWeb={!isTauriRuntime()}
        groups={groups}
        onChange={update}
        onProxyToggle={toggleProxy}
      />
    </div>
  );
}
