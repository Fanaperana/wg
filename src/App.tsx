import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Settings2,
  Minus,
  X,
  Mic,
  Square,
  SendHorizontal,
  Loader2,
  Sparkles,
  User,
  Github,
  LogOut,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";

type Role = "user" | "assistant" | "system";
interface Message {
  role: Role;
  content: string;
}

const CHAT_MODELS = ["gpt-4o-mini", "gpt-4o", "o3-mini", "claude-3.5-sonnet"];
const TRANSCRIBE_MODEL = "whisper-1";
const SYSTEM_PROMPT =
  "You are a concise, helpful assistant embedded in a desktop overlay widget.";

interface DeviceInfo {
  device_code: string;
  user_code: string;
  verification_uri: string;
  interval: number;
}

function App() {
  const [copilotToken, setCopilotToken] = useState(
    () => localStorage.getItem("copilot_oauth_token") ?? ""
  );
  const [apiKey, setApiKey] = useState(
    () => localStorage.getItem("openai_api_key") ?? ""
  );
  const [model, setModel] = useState(
    () => localStorage.getItem("copilot_model") ?? CHAT_MODELS[0]
  );
  const [autoSend, setAutoSend] = useState(
    () => localStorage.getItem("auto_send") !== "false"
  );
  const [showSettings, setShowSettings] = useState(!copilotToken);
  const [login, setLogin] = useState<{ userCode: string; uri: string } | null>(
    null
  );

  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [recording, setRecording] = useState(false);
  const [status, setStatus] = useState("");

  const listRef = useRef<HTMLDivElement>(null);
  const pollRef = useRef<number | null>(null);

  useEffect(() => {
    localStorage.setItem("copilot_oauth_token", copilotToken);
  }, [copilotToken]);
  useEffect(() => {
    localStorage.setItem("openai_api_key", apiKey);
  }, [apiKey]);
  useEffect(() => {
    localStorage.setItem("copilot_model", model);
  }, [model]);
  useEffect(() => {
    localStorage.setItem("auto_send", String(autoSend));
  }, [autoSend]);
  useEffect(() => {
    listRef.current?.scrollTo({ top: listRef.current.scrollHeight });
  }, [messages, busy]);
  useEffect(() => {
    return () => {
      if (pollRef.current) window.clearInterval(pollRef.current);
    };
  }, []);

  async function startLogin() {
    if (login) return;
    setStatus("");
    try {
      const info = await invoke<DeviceInfo>("copilot_login_start");
      setLogin({ userCode: info.user_code, uri: info.verification_uri });
      await openUrl(info.verification_uri).catch(() => {});
      const poll = async () => {
        try {
          const token = await invoke<string | null>("copilot_login_poll", {
            deviceCode: info.device_code,
          });
          if (token) {
            if (pollRef.current) window.clearInterval(pollRef.current);
            pollRef.current = null;
            setCopilotToken(token);
            setLogin(null);
            setStatus("Signed in with GitHub Copilot.");
          }
        } catch (e) {
          if (pollRef.current) window.clearInterval(pollRef.current);
          pollRef.current = null;
          setLogin(null);
          setStatus(String(e));
        }
      };
      pollRef.current = window.setInterval(
        poll,
        Math.max(info.interval, 5) * 1000
      );
    } catch (e) {
      setStatus(String(e));
    }
  }

  function signOut() {
    if (pollRef.current) window.clearInterval(pollRef.current);
    pollRef.current = null;
    setLogin(null);
    setCopilotToken("");
    setStatus("");
  }

  async function send(text: string) {
    const prompt = text.trim();
    if (!prompt || busy) return;
    if (!copilotToken.trim()) {
      setShowSettings(true);
      setStatus("Sign in with GitHub Copilot first.");
      return;
    }

    const history: Message[] = [...messages, { role: "user", content: prompt }];
    setMessages(history);
    setInput("");
    setBusy(true);
    setStatus("");

    try {
      const reply = await invoke<string>("ask_copilot", {
        token: copilotToken,
        model,
        messages: [{ role: "system", content: SYSTEM_PROMPT }, ...history],
      });
      setMessages((m) => [...m, { role: "assistant", content: reply }]);
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggleRecording() {
    if (busy) return;
    if (!apiKey.trim()) {
      setShowSettings(true);
      setStatus("Voice needs an OpenAI API key (Copilot has no transcription).");
      return;
    }

    if (!recording) {
      try {
        await invoke("start_capture");
        setRecording(true);
        setStatus("Capturing system audio…");
      } catch (e) {
        setStatus(String(e));
      }
      return;
    }

    setRecording(false);
    setBusy(true);
    setStatus("Transcribing…");
    try {
      const text = await invoke<string>("stop_capture_and_transcribe", {
        apiKey,
        model: TRANSCRIBE_MODEL,
      });
      setStatus("");
      if (!text.trim()) {
        setStatus("Nothing was transcribed.");
      } else if (autoSend) {
        setBusy(false);
        await send(text);
        return;
      } else {
        setInput((prev) => (prev ? `${prev} ${text}` : text));
      }
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  }

  const appWindow = getCurrentWindow();

  return (
    <div className="flex h-screen flex-col overflow-hidden rounded-lg border border-border bg-background text-foreground backdrop-blur-xl">
      {/* Title bar */}
      <header
        data-tauri-drag-region
        className="flex h-8 shrink-0 items-center justify-between border-b border-border px-2"
      >
        <div
          data-tauri-drag-region
          className="flex items-center gap-1.5 text-xs font-semibold"
        >
          <Sparkles className="size-3.5 text-primary" />
          <span data-tauri-drag-region>ChatGPT Widget</span>
        </div>
        <div className="flex items-center gap-0.5">
          <Button
            variant="ghost"
            size="icon-sm"
            title="Settings"
            onClick={() => setShowSettings((s) => !s)}
          >
            <Settings2 />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            title="Minimize"
            onClick={() => appWindow.minimize()}
          >
            <Minus />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            title="Close"
            className="hover:bg-destructive hover:text-white"
            onClick={() => appWindow.close()}
          >
            <X />
          </Button>
        </div>
      </header>

      {/* Settings */}
      {showSettings && (
        <section className="shrink-0 space-y-2 border-b border-border bg-card/50 p-2">
          <div className="space-y-1">
            <Label>GitHub Copilot</Label>
            {copilotToken ? (
              <div className="flex items-center justify-between gap-2 rounded-md bg-secondary px-2 py-1.5 text-[11px]">
                <span className="flex items-center gap-1.5 text-secondary-foreground">
                  <Github className="size-3.5" /> Signed in
                </span>
                <Button variant="ghost" size="sm" onClick={signOut}>
                  <LogOut className="size-3.5" /> Sign out
                </Button>
              </div>
            ) : login ? (
              <div className="space-y-1 rounded-md bg-secondary px-2 py-1.5 text-[11px] text-secondary-foreground">
                <p>
                  Enter this code at{" "}
                  <button
                    type="button"
                    className="underline"
                    onClick={() => openUrl(login.uri)}
                  >
                    github.com/login/device
                  </button>
                  :
                </p>
                <p className="text-center text-base font-bold tracking-widest select-text">
                  {login.userCode}
                </p>
                <p className="flex items-center gap-1 text-muted-foreground">
                  <Loader2 className="size-3 animate-spin" /> Waiting for
                  authorization…
                </p>
              </div>
            ) : (
              <Button
                variant="secondary"
                size="sm"
                className="w-full"
                onClick={startLogin}
              >
                <Github className="size-3.5" /> Sign in with GitHub
              </Button>
            )}
          </div>
          <div className="flex items-end gap-2">
            <div className="flex-1 space-y-1">
              <Label>Model</Label>
              <Select value={model} onValueChange={setModel}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {CHAT_MODELS.map((m) => (
                    <SelectItem key={m} value={m}>
                      {m}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <label className="flex h-7 items-center gap-1.5 text-[11px] text-muted-foreground">
              <Switch checked={autoSend} onCheckedChange={setAutoSend} />
              Auto-send voice
            </label>
          </div>
          <div className="space-y-1">
            <Label htmlFor="api-key">OpenAI API key (optional, voice only)</Label>
            <Input
              id="api-key"
              type="password"
              value={apiKey}
              placeholder="sk-…"
              autoComplete="off"
              onChange={(e) => setApiKey(e.currentTarget.value)}
            />
          </div>
        </section>
      )}

      {/* Messages */}
      <div ref={listRef} className="flex-1 space-y-1.5 overflow-y-auto p-2">
        {messages.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-1.5 text-center text-muted-foreground">
            <Sparkles className="size-5 opacity-50" />
            <p className="text-[11px] leading-tight">
              Type a prompt or capture system
              <br />
              audio to ask ChatGPT.
            </p>
          </div>
        )}
        {messages.map((m, i) => (
          <div
            key={i}
            className={cn(
              "flex gap-1.5",
              m.role === "user" ? "flex-row-reverse" : "flex-row"
            )}
          >
            <div
              className={cn(
                "mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full",
                m.role === "user"
                  ? "bg-primary/20 text-primary"
                  : "bg-accent text-accent-foreground"
              )}
            >
              {m.role === "user" ? (
                <User className="size-2.5" />
              ) : (
                <Sparkles className="size-2.5" />
              )}
            </div>
            <div
              className={cn(
                "max-w-[85%] whitespace-pre-wrap rounded-md px-2 py-1 text-xs leading-snug select-text",
                m.role === "user"
                  ? "bg-primary text-primary-foreground"
                  : "bg-secondary text-secondary-foreground"
              )}
            >
              {m.content}
            </div>
          </div>
        ))}
        {busy && (
          <div className="flex items-center gap-1.5 text-muted-foreground">
            <div className="flex size-4 items-center justify-center rounded-full bg-accent">
              <Sparkles className="size-2.5" />
            </div>
            <Loader2 className="size-3.5 animate-spin" />
          </div>
        )}
      </div>

      {/* Status */}
      {status && (
        <div className="shrink-0 border-t border-border bg-card/50 px-2 py-1 text-[10px] text-muted-foreground">
          {status}
        </div>
      )}

      {/* Composer */}
      <form
        className="flex shrink-0 items-end gap-1.5 border-t border-border p-2"
        onSubmit={(e) => {
          e.preventDefault();
          send(input);
        }}
      >
        <Button
          type="button"
          size="icon"
          variant={recording ? "destructive" : "secondary"}
          title={recording ? "Stop capture" : "Capture system audio"}
          className="shrink-0"
          onClick={toggleRecording}
        >
          {recording ? <Square className="fill-current" /> : <Mic />}
        </Button>
        <Textarea
          value={input}
          placeholder="Ask ChatGPT…"
          rows={1}
          className="max-h-24 min-h-7 flex-1 select-text"
          onChange={(e) => setInput(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send(input);
            }
          }}
        />
        <Button
          type="submit"
          size="icon"
          className="shrink-0"
          disabled={busy || !input.trim()}
        >
          {busy ? <Loader2 className="animate-spin" /> : <SendHorizontal />}
        </Button>
      </form>
    </div>
  );
}

export default App;
