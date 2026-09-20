import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  LogIn,
  LogOut,
  Brain,
  ScanEye,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Input } from "@/components/ui/input";
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
  thinking?: string[];
  image?: string;
}

const CHAT_MODELS = [
  "gpt-4o-mini",
  "gpt-4o",
  "gpt-4.1",
  "o3-mini",
  "o1",
  "claude-3.5-sonnet",
  "claude-3.7-sonnet",
  "claude-sonnet-4",
  "gemini-2.0-flash-001",
  "gemini-2.5-pro",
];
const SYSTEM_PROMPT = `You are a concise, helpful assistant embedded in a desktop overlay widget for Fanaperana (Prince Fanaperana), a software engineer. You already know the facts below — answer directly from them, and only call the fetch_url tool when the user asks for something not covered here or for live/updated info.

# About Fanaperana
- Software engineer, 6+ years, 46 shipped projects (11 in Rust, 23 public). Focus: Rust, TypeScript, WebAssembly, and agentic AI.
- Builds small, sharp tools: CLIs, TUIs, tiny languages/parsers, numerics, Tauri desktop apps, Wayland compositors, and local-first AI agents. Prefers zero-JS-by-default pages, tiny bundles, and keyboard-first UIs (Svelte 5, React, Vue).
- Hobbies: building tiny OSes/kernels, writing parsers and expression-oriented languages, infinite-canvas UIs and node editors, making terminals beautiful, research notes in LaTeX, motion design in After Effects.
- Contact: fanaperanaprince@gmail.com · github.com/Fanaperana · linkedin.com/in/prince-fanaperana. Remote, USA (EST). Open to opportunities.
- Languages by project count: TypeScript 17, Rust 11, PHP 4, Python 2, Svelte 2, Vue 2, Shell 2, JavaScript 2, MDX 1, TeX 1, Zig 1, Vim Script 1.

# The 46 projects
1. semtree — Universal incremental language infrastructure, faster than Tree-sitter; built-in formatter, linter, refactoring, IDE support. Pure Rust.
2. cinecode — Pure-Rust cinematic engine for programming documentaries: keyframe code animations, narrated walkthroughs, rendered video.
3. cinecode-docs — Documentation for CineCode (Diátaxis + Fumadocs).
4. pskey — Tiny transparent Tauri widget password manager; libsodium, Argon2id, challenge-response PIN.
5. adaptive-codegraph — Language-agnostic code graph indexer, search engine, and MCP server; add a language with .toml + .scm. Rust.
6. spdf — Fast spatial PDF parsing in Rust; column-aware text extraction, optional OCR, format conversion.
7. AVIL — Adaptive Verified Iteration Loop: a self-improving SDLC for agentic AI; research paper with formal model and evaluation.
8. cahier — Themeable PDF reader (React + Vite + Zustand, Turborepo, PDF.js, IndexedDB).
9. sentinel — His biggest project: an agentic AI coding assistant in Rust benchmarked against Claude Code, Hermes, OpenClaw. Local-first planner + tools loop.
10. continuum — Self-learning AI agent (same family as sentinel), focused on continual learning and self-improvement loops. Python.
11. MosaicFlow-Svelte — His best app so far: a node-based infinite canvas for visual information mapping and research. Tauri 2, Svelte 5, TypeScript.
12. canvaswm — Infinite-canvas Wayland compositor; zoomable 2D surface. Low-level Rust graphics.
13. rmd — Plugin-first rich markdown editor (CodeMirror 6 + Svelte); rich while reading, raw while editing.
14. hexglyph — HexGlyph-16 OrbitSigil: procedural visual alphabet mapping every u16 to a unique glyph (base-65536). Rust + TypeScript.
15. mineos — A Linux distribution that only runs Minecraft; boot-to-Minecraft in under 15 seconds.
16. zigos — Minimal x86_64 operating system written in Zig; learning kernel from the metal up.
17. minichess — Terminal chess engine interface in Rust powered by Stockfish; plays, analyzes, renders the board in a TUI.
18. simpless — E-commerce platform built with Laravel; Shopify-style storefront and admin.
19. laravel_vue_twilio — Healthcare-grade secure communication system for Parkview Mirro Center; Laravel + Vue + Twilio.
20. codegraph — Codebase graph analysis CLI with hybrid retrieval (Neo4j graph + vector similarity); tree-sitter multi-language parsing.
21. fuzzy-search-rs — Educational implementation of fuzzy-search algorithms (Levenshtein) in Rust.
22. fkbr — Cross-platform mouseless desktop app for keyboard-driven mouse control; Tauri, React, Rust.
23. framescript — Tiny scripting-language experiment; expression-oriented, frame-based execution. TypeScript.
24. genai — Generative-AI toolkit sandbox in TypeScript; prompts, chains, experiments.
25. monax — Keyframe-based code animation engine with a visual node editor.
26. kodex — Cinematic code animation studio; typewriter effects and smooth diffs, powered by Monaco Editor.
27. ae-reach — The ultimate After Effects toolset: 90+ tools in a single CEP panel for motion designers.
28. ae-curves-panel — After Effects CEP panel for advanced cubic-bezier easing curves.
29. quantrs — Quantitative experiments in Rust; fast numerics and market tooling.
30. Polymine — Hexagonal minesweeper with WebGL rendering and a Rust backend, packaged as a Tauri desktop app.
31. kadoo — UI-primitives experiment for task/idea capture; TypeScript, keyboard-first.
32. whisper-asr-cli-rs — Rust CLI wrapping Whisper for offline speech-to-text from the terminal.
33. landmine — Svelte minesweeper variant with a polished UI.
34. SandCode — Local-first code snippet manager (offline GitHub Gist alternative); Tauri + Vue.
35. rekan — Modern kanban reboot in TypeScript; keyboard-first (latest of the kan family).
36. ekan — Electron kanban; second iteration of the kan family.
37. blingnails — Private nail-salon management/showcase site; TypeScript client work.
38. kan — Original kanban board in TypeScript; lightweight, drag-and-drop (start of the kan family).
39. recog — Object-recognition playground in Python; OpenCV / ML for computer vision.
40. tauri-kiosk — Shell scripts to set up a minimal locked kiosk desktop on bare Ubuntu running a Tauri app.
41. fluid-converter — Mobile app (React Native + Expo) for fluid unit conversion with mixology-grade precision.
42. neovim-config — Personal Neovim configuration; custom keymaps, plugins, IDE-like setup.
43. custom-greenscreen-chat — Vue green-background text-bubble overlay for chroma-keying in video editing.
44. Make-The-Docs — Live markdown editor built with Laravel; split-pane live preview and theming.
45. make-the-doc-vue — Vue + Vite frontend for Make-The-Docs.
46. rockdiva — Production website for Rockdiva Nails (Laravel), powering rockdivanails.com.

The fetch_url tool reads public web pages when you need something beyond the above; his portfolio is https://fanaperana.github.io/portfolio/. The github_get tool does read-only GitHub REST GETs for his account (private and public repos, pull requests, commits, issues) — call it with a path like /user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member, /repos/OWNER/REPO/pulls?state=all, /repos/OWNER/REPO/commits?per_page=30, or /search/issues?q=author:USERNAME+is:pr. Use it whenever the user asks about their repositories, PRs, commits, or GitHub activity. Always use the full conversation history for context: treat follow-ups like "yes", "list them all", or "include private" as answers to your own previous question and act on them immediately — never re-ask something the user already answered. Keep replies concise.`;

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
  const [model, setModel] = useState(
    () => localStorage.getItem("copilot_model") ?? CHAT_MODELS[0]
  );
  const [autoSend, setAutoSend] = useState(
    () => localStorage.getItem("auto_send") !== "false"
  );
  const [githubToken, setGithubToken] = useState(
    () => localStorage.getItem("github_token") ?? ""
  );
  const [models, setModels] = useState<string[]>(CHAT_MODELS);
  const [showSettings, setShowSettings] = useState(!copilotToken);
  const [login, setLogin] = useState<{ userCode: string; uri: string } | null>(
    null
  );

  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [recording, setRecording] = useState(false);
  const [status, setStatus] = useState("");
  // Live "thinking" lines for the in-progress request (tool calls, reasoning).
  const [thinking, setThinking] = useState<string[]>([]);
  const thinkingRef = useRef<string[]>([]);
  // Pending screen-capture image (data URL) to attach to the next prompt.
  const [attachment, setAttachment] = useState<string | null>(null);
  // Frozen full-screen frame shown while the user drags a capture region.
  const [captureFrame, setCaptureFrame] = useState<string | null>(null);

  const listRef = useRef<HTMLDivElement>(null);
  const pollRef = useRef<number | null>(null);
  const stopFallbackRef = useRef<number | null>(null);
  // Live transcript for the in-progress dictation, plus refs so the (mount-only)
  // STT event listeners always see the latest state.
  const liveRef = useRef("");
  const inputRef = useRef(input);
  const autoSendRef = useRef(autoSend);
  const sendRef = useRef<(text: string) => void>(() => {});

  inputRef.current = input;
  autoSendRef.current = autoSend;

  useEffect(() => {
    localStorage.setItem("copilot_oauth_token", copilotToken);
  }, [copilotToken]);
  useEffect(() => {
    localStorage.setItem("github_token", githubToken);
  }, [githubToken]);
  useEffect(() => {
    localStorage.setItem("copilot_model", model);
  }, [model]);
  // Load the models the account can actually use so the dropdown never offers
  // an unsupported one.
  useEffect(() => {
    if (!copilotToken) return;
    invoke<string[]>("copilot_models", { token: copilotToken })
      .then((list) => {
        if (!list.length) return;
        setModels(list);
        setModel((m) => (list.includes(m) ? m : list[0]));
      })
      .catch(() => {});
  }, [copilotToken]);
  useEffect(() => {
    localStorage.setItem("auto_send", String(autoSend));
  }, [autoSend]);
  useEffect(() => {
    listRef.current?.scrollTo({ top: listRef.current.scrollHeight });
  }, [messages, busy]);
  useEffect(() => {
    return () => {
      if (pollRef.current) window.clearTimeout(pollRef.current);
    };
  }, []);

  // Subscribe to local speech-to-text events once.
  useEffect(() => {
    const subs = [
      listen<string>("stt-final", (e) => {
        const t = e.payload.trim();
        if (!t) return;
        liveRef.current = liveRef.current ? `${liveRef.current} ${t}` : t;
        setInput(liveRef.current);
      }),
      listen<string>("stt-partial", (e) => {
        setStatus(`Listening… ${e.payload}`);
      }),
      listen<string>("stt-status", (e) => {
        if (e.payload === "loading") setStatus("Loading speech model…");
        else if (e.payload === "listening") setStatus("Listening…");
        else if (e.payload === "stopped") {
          if (stopFallbackRef.current) {
            window.clearTimeout(stopFallbackRef.current);
            stopFallbackRef.current = null;
          }
          setRecording(false);
          setStatus("");
          const text = liveRef.current.trim();
          if (autoSendRef.current && text) {
            liveRef.current = "";
            sendRef.current(text);
          }
        }
      }),
      listen<string>("stt-error", (e) => {
        if (stopFallbackRef.current) {
          window.clearTimeout(stopFallbackRef.current);
          stopFallbackRef.current = null;
        }
        setRecording(false);
        setStatus(String(e.payload));
      }),
      listen<string>("copilot-thinking", (e) => {
        const line = e.payload.trim();
        if (!line) return;
        thinkingRef.current = [...thinkingRef.current, line];
        setThinking(thinkingRef.current);
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((un) => un()));
    };
  }, []);

  async function startLogin() {
    if (login) return;
    setStatus("");
    try {
      const info = await invoke<DeviceInfo>("copilot_login_start");
      setLogin({ userCode: info.user_code, uri: info.verification_uri });
      await openUrl(info.verification_uri).catch(() => {});

      // Poll with a 1s buffer over GitHub's interval to avoid `slow_down`
      // throttling, and give up after a few minutes instead of hanging forever.
      const stepMs = (Math.max(info.interval, 5) + 1) * 1000;
      const deadline = Date.now() + 5 * 60 * 1000;
      const poll = async () => {
        try {
          const token = await invoke<string | null>("copilot_login_poll", {
            deviceCode: info.device_code,
          });
          if (token) {
            pollRef.current = null;
            setCopilotToken(token);
            setLogin(null);
            setStatus("Signed in with GitHub Copilot.");
            return;
          }
          if (Date.now() > deadline) {
            pollRef.current = null;
            setLogin(null);
            setStatus("Login timed out. Please try signing in again.");
            return;
          }
          pollRef.current = window.setTimeout(poll, stepMs);
        } catch (e) {
          pollRef.current = null;
          setLogin(null);
          setStatus(String(e));
        }
      };
      pollRef.current = window.setTimeout(poll, stepMs);
    } catch (e) {
      setStatus(String(e));
    }
  }

  function signOut() {
    if (pollRef.current) window.clearTimeout(pollRef.current);
    pollRef.current = null;
    setLogin(null);
    setCopilotToken("");
    setStatus("");
  }

  async function send(text: string) {
    const prompt = text.trim();
    if ((!prompt && !attachment) || busy) return;
    if (!copilotToken.trim()) {
      setShowSettings(true);
      setStatus("Sign in with GitHub Copilot first.");
      return;
    }

    liveRef.current = "";
    const image = attachment ?? undefined;
    const history: Message[] = [
      ...messages,
      { role: "user", content: prompt, image },
    ];
    setMessages(history);
    setInput("");
    setAttachment(null);
    setBusy(true);
    setStatus("");
    thinkingRef.current = [];
    setThinking([]);

    try {
      const reply = await invoke<string>("ask_copilot", {
        token: copilotToken,
        githubToken,
        model,
        messages: [
          { role: "system", content: SYSTEM_PROMPT },
          ...history.map((m) =>
            m.image
              ? {
                  role: m.role,
                  content: [
                    { type: "text", text: m.content || "Describe this image." },
                    { type: "image_url", image_url: { url: m.image } },
                  ],
                }
              : { role: m.role, content: m.content }
          ),
        ],
      });
      const thoughts = thinkingRef.current;
      setMessages((m) => [
        ...m,
        {
          role: "assistant",
          content: reply,
          thinking: thoughts.length ? thoughts : undefined,
        },
      ]);
    } catch (e) {
      const err = String(e);
      // Some models the API advertises aren't reachable via chat completions.
      // Drop the offender and fall back to a working model automatically.
      if (/not accessible|not supported/i.test(err)) {
        const next = models.filter((m) => m !== model);
        setModels(next.length ? next : CHAT_MODELS);
        setModel((next[0] ?? CHAT_MODELS[0]));
        setStatus(`"${model}" isn't available for chat here — switched to ${next[0] ?? CHAT_MODELS[0]}. Try again.`);
      } else {
        setStatus(err);
      }
    } finally {
      setBusy(false);
    }
  }

  // Expand the widget into a fullscreen overlay and freeze the screen so the
  // user can drag a region. The overlay lives in this (content-protected)
  // window, so it never appears in a screen share.
  async function startCapture() {
    try {
      const frame = await invoke<string>("enter_capture");
      setCaptureFrame(frame);
    } catch (e) {
      setStatus(String(e));
    }
  }

  async function finishCapture(dataUrl: string) {
    setAttachment(dataUrl);
    setCaptureFrame(null);
    try {
      await invoke("exit_capture");
    } catch {}
  }

  async function cancelCapture() {
    setCaptureFrame(null);
    try {
      await invoke("exit_capture");
    } catch {}
  }

  async function toggleRecording() {
    if (busy) return;

    if (!recording) {
      // Continue appending onto whatever is already in the box.
      liveRef.current = inputRef.current;
      try {
        await invoke("start_stt");
        setRecording(true);
        setStatus("Listening…");
      } catch (e) {
        setStatus(String(e));
      }
      return;
    }

    setStatus("Finishing…");
    try {
      await invoke("stop_stt");
      // Safety net: if the backend never reports "stopped" (e.g. the last
      // decode is slow), don't leave the UI wedged on "Finishing…".
      if (stopFallbackRef.current) window.clearTimeout(stopFallbackRef.current);
      stopFallbackRef.current = window.setTimeout(() => {
        stopFallbackRef.current = null;
        setRecording(false);
        setStatus("");
      }, 4000);
    } catch (e) {
      setRecording(false);
      setStatus(String(e));
    }
  }

  sendRef.current = send;

  const appWindow = getCurrentWindow();

  return (
    <>
      {captureFrame && (
        <CaptureOverlay
          frame={captureFrame}
          onDone={finishCapture}
          onCancel={cancelCapture}
        />
      )}
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
          <span data-tauri-drag-region>Copilot Widget</span>
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
                  <LogIn className="size-3.5" /> Signed in
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
                <LogIn className="size-3.5" /> Sign in with GitHub
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
                  {models.map((m) => (
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
            <Label>GitHub token (read-only)</Label>
            <Input
              type="password"
              placeholder="ghp_… — for reading your repos, PRs, commits"
              value={githubToken}
              onChange={(e) => setGithubToken(e.target.value)}
              autoComplete="off"
            />
            <p className="text-[10px] text-muted-foreground">
              Optional. Create a token with read-only repo access at{" "}
              <button
                type="button"
                className="underline"
                onClick={() =>
                  openUrl(
                    "https://github.com/settings/tokens/new?scopes=repo,read:org&description=wg%20read-only"
                  )
                }
              >
                github.com/settings/tokens
              </button>
              . Stored locally; enables private repo/PR/commit access.
            </p>
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
              audio to ask Copilot.
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
                "max-w-[85%] whitespace-pre-wrap rounded-md px-2 py-1 text-xs leading-snug cursor-default select-text",
                m.role === "user"
                  ? "bg-primary text-primary-foreground"
                  : "bg-secondary text-secondary-foreground"
              )}
            >
              {m.thinking && m.thinking.length > 0 && (
                <details className="mb-1 rounded border border-border/60 bg-background/40 px-1.5 py-0.5">
                  <summary className="flex cursor-pointer items-center gap-1 text-[10px] text-muted-foreground select-none">
                    <Brain className="size-2.5" />
                    Thought for {m.thinking.length} step
                    {m.thinking.length > 1 ? "s" : ""}
                  </summary>
                  <div className="mt-1 space-y-0.5 text-[10px] text-muted-foreground">
                    {m.thinking.map((t, j) => (
                      <div key={j} className="whitespace-pre-wrap">
                        {t}
                      </div>
                    ))}
                  </div>
                </details>
              )}
              {m.image && (
                <img
                  src={m.image}
                  alt="attachment"
                  className="mb-1 max-h-40 rounded border border-border/60"
                />
              )}
              {m.content}
            </div>
          </div>
        ))}
        {busy && (
          <div className="flex items-start gap-1.5 text-muted-foreground">
            <div className="mt-0.5 flex size-4 items-center justify-center rounded-full bg-accent">
              <Sparkles className="size-2.5" />
            </div>
            {thinking.length > 0 ? (
              <div className="max-w-[85%] rounded-md border border-border/60 bg-background/40 px-2 py-1 text-[10px]">
                <div className="mb-0.5 flex items-center gap-1 text-muted-foreground">
                  <Brain className="size-2.5 animate-pulse" />
                  Thinking…
                </div>
                <div className="space-y-0.5 whitespace-pre-wrap">
                  {thinking.map((t, j) => (
                    <div key={j}>{t}</div>
                  ))}
                </div>
              </div>
            ) : (
              <Loader2 className="mt-0.5 size-3.5 animate-spin" />
            )}
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
        className="flex shrink-0 flex-col gap-1.5 border-t border-border p-2"
        onSubmit={(e) => {
          e.preventDefault();
          send(input);
        }}
      >
        {attachment && (
          <div className="relative w-fit">
            <img
              src={attachment}
              alt="pending capture"
              className="max-h-28 rounded border border-border"
            />
            <button
              type="button"
              title="Remove"
              className="absolute -top-1.5 -right-1.5 rounded-full border border-border bg-background p-0.5 text-muted-foreground hover:text-foreground"
              onClick={() => setAttachment(null)}
            >
              <X className="size-3" />
            </button>
          </div>
        )}
        <div className="flex items-end gap-1.5">
          <Button
            type="button"
            size="icon"
            variant="secondary"
            title="Capture a screen area"
            className="shrink-0"
            onClick={startCapture}
          >
            <ScanEye />
          </Button>
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
            placeholder="Ask Copilot…"
            rows={1}
            className="max-h-24 min-h-7 flex-1 cursor-default select-text"
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
            disabled={busy || (!input.trim() && !attachment)}
          >
            {busy ? <Loader2 className="animate-spin" /> : <SendHorizontal />}
          </Button>
        </div>
      </form>
      </div>
    </>
  );
}

/// Fullscreen frozen-frame overlay: drag a rectangle to crop a region.
function CaptureOverlay({
  frame,
  onDone,
  onCancel,
}: {
  frame: string;
  onDone: (dataUrl: string) => void;
  onCancel: () => void;
}) {
  const imgRef = useRef<HTMLImageElement>(null);
  const [drag, setDrag] = useState<{ sx: number; sy: number } | null>(null);
  const [rect, setRect] = useState<{
    x: number;
    y: number;
    w: number;
    h: number;
  } | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  function finish(x: number, y: number, w: number, h: number) {
    const img = imgRef.current;
    if (!img || w < 5 || h < 5) {
      onCancel();
      return;
    }
    try {
      // Map CSS (logical) pixels to the frame's native (physical) pixels.
      const scaleX = img.naturalWidth / window.innerWidth;
      const scaleY = img.naturalHeight / window.innerHeight;
      const cx = Math.round(x * scaleX);
      const cy = Math.round(y * scaleY);
      const cw = Math.max(1, Math.round(w * scaleX));
      const ch = Math.max(1, Math.round(h * scaleY));
      const canvas = document.createElement("canvas");
      canvas.width = cw;
      canvas.height = ch;
      const ctx = canvas.getContext("2d");
      if (!ctx) {
        onCancel();
        return;
      }
      ctx.drawImage(img, cx, cy, cw, ch, 0, 0, cw, ch);
      onDone(canvas.toDataURL("image/png"));
    } catch {
      onCancel();
    }
  }

  return (
    <div
      className="fixed inset-0 z-9999 cursor-default select-none"
      onMouseDown={(e) => {
        if (e.button !== 0) return;
        setDrag({ sx: e.clientX, sy: e.clientY });
        setRect({ x: e.clientX, y: e.clientY, w: 0, h: 0 });
      }}
      onMouseMove={(e) => {
        if (!drag) return;
        setRect({
          x: Math.min(drag.sx, e.clientX),
          y: Math.min(drag.sy, e.clientY),
          w: Math.abs(e.clientX - drag.sx),
          h: Math.abs(e.clientY - drag.sy),
        });
      }}
      onMouseUp={(e) => {
        if (!drag) return;
        const x = Math.min(drag.sx, e.clientX);
        const y = Math.min(drag.sy, e.clientY);
        const w = Math.abs(e.clientX - drag.sx);
        const h = Math.abs(e.clientY - drag.sy);
        setDrag(null);
        finish(x, y, w, h);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        onCancel();
      }}
    >
      <img
        ref={imgRef}
        src={frame}
        alt=""
        draggable={false}
        className="pointer-events-none absolute inset-0 h-full w-full object-fill"
      />
      {!rect && <div className="pointer-events-none absolute inset-0 bg-black/40" />}
      {rect && (
        <div
          className="pointer-events-none absolute border border-sky-400"
          style={{
            left: rect.x,
            top: rect.y,
            width: rect.w,
            height: rect.h,
            boxShadow: "0 0 0 100000px rgba(0,0,0,0.45)",
          }}
        />
      )}
      <div className="pointer-events-none absolute left-1/2 top-3 -translate-x-1/2 rounded-md bg-black/70 px-2.5 py-1 text-xs text-white">
        Drag to capture · Esc / right-click to cancel
      </div>
    </div>
  );
}

export default App;
