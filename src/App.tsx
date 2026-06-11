import { useEffect, useRef, useState, type PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SendIcon, XIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { commands } from "./bindings";
import "./App.css";

function getTauriWindow() {
  return "__TAURI_INTERNALS__" in window ? getCurrentWindow() : null;
}

async function closeWindow() {
  const appWindow = getTauriWindow();

  if (!appWindow) {
    return;
  }

  await appWindow.close();
}

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [isDragHandleActive, setIsDragHandleActive] = useState(false);
  const dragHandleHideTimer = useRef<number | null>(null);
  const dragHandlePollTimer = useRef<number | null>(null);
  const isWindowDragging = useRef(false);

  const stack = ["Tauri v2", "React", "TypeScript", "Tailwind v4", "shadcn/ui"];

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await commands.greet(name));
  }

  function clearDragHandleHideTimer() {
    if (dragHandleHideTimer.current === null) {
      return;
    }

    window.clearTimeout(dragHandleHideTimer.current);
    dragHandleHideTimer.current = null;
  }

  function clearDragHandlePollTimer() {
    if (dragHandlePollTimer.current === null) {
      return;
    }

    window.clearTimeout(dragHandlePollTimer.current);
    dragHandlePollTimer.current = null;
  }

  function hideDragHandleSoon(delay = 180) {
    clearDragHandleHideTimer();
    dragHandleHideTimer.current = window.setTimeout(() => {
      isWindowDragging.current = false;
      clearDragHandlePollTimer();
      setIsDragHandleActive(false);
      dragHandleHideTimer.current = null;
    }, delay);
  }

  function lockDragHandleVisible() {
    clearDragHandleHideTimer();
    setIsDragHandleActive(true);
  }

  async function pollDragMouseButton() {
    if (!isWindowDragging.current) {
      return;
    }

    try {
      if (!(await commands.isPrimaryMouseButtonDown())) {
        hideDragHandleSoon();
        return;
      }
    } catch {
      hideDragHandleSoon(900);
      return;
    }

    dragHandlePollTimer.current = window.setTimeout(pollDragMouseButton, 80);
  }

  function startDragMouseButtonPolling() {
    clearDragHandlePollTimer();
    dragHandlePollTimer.current = window.setTimeout(pollDragMouseButton, 80);
  }

  async function startWindowDrag(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return;
    }

    const appWindow = getTauriWindow();

    if (!appWindow) {
      return;
    }

    event.preventDefault();
    isWindowDragging.current = true;
    lockDragHandleVisible();
    startDragMouseButtonPolling();

    try {
      await appWindow.startDragging();
    } catch {
      isWindowDragging.current = false;
      hideDragHandleSoon(0);
      return;
    }
  }

  useEffect(() => {
    return () => {
      clearDragHandleHideTimer();
      clearDragHandlePollTimer();
    };
  }, []);

  return (
    <main className="app-shell dark">
      <header className="titlebar">
        <div
          className={`drag-handle-zone${isDragHandleActive ? " is-active" : ""}`}
          onPointerDown={(event) => void startWindowDrag(event)}
        >
          <div className="drag-handle" />
        </div>

        <div className="close-control-zone">
          <button
            type="button"
            className="close-control"
            aria-label="Close"
            onClick={() => void closeWindow()}
          >
            <XIcon aria-hidden="true" size={18} strokeWidth={1.8} />
          </button>
        </div>
      </header>

      <section className="container">
        <Card className="w-full max-w-xl border-border/70 bg-card/85 shadow-2xl backdrop-blur-xl">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <img className="brand-mark" src="/logo.svg" alt="" aria-hidden="true" />
              KeelDesk
            </CardTitle>
            <CardDescription>
              Tailwind and shadcn/ui are now wired into the desktop shell.
            </CardDescription>
            <CardAction>
              <Badge variant="secondary">pnpm</Badge>
            </CardAction>
          </CardHeader>
          <CardContent className="flex flex-col gap-5">
            <div className="flex flex-wrap gap-2">
              {stack.map((item) => (
                <Badge key={item} variant="outline">
                  {item}
                </Badge>
              ))}
            </div>

            <Separator />

            <form
              className="flex flex-col gap-3 sm:flex-row"
              onSubmit={(e) => {
                e.preventDefault();
                void greet();
              }}
            >
              <Input
                id="greet-input"
                value={name}
                onChange={(e) => setName(e.currentTarget.value)}
                placeholder="Enter a name..."
              />
              <Button type="submit">
                <SendIcon data-icon="inline-start" />
                Greet
              </Button>
            </form>

            <p className="min-h-6 text-sm text-muted-foreground">
              {greetMsg || "Generated Tauri command bindings are ready."}
            </p>
          </CardContent>
          <CardFooter className="text-sm text-muted-foreground">
            Custom transparent chrome stays outside the component system.
          </CardFooter>
        </Card>
      </section>
    </main>
  );
}

export default App;
