import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SendIcon } from "lucide-react";
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

async function runWindowCommand(
  command: "close" | "minimize" | "toggleMaximize",
) {
  const appWindow = getTauriWindow();

  if (!appWindow) {
    return;
  }

  await appWindow[command]();
}

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  const stack = ["Tauri v2", "React", "TypeScript", "Tailwind v4", "shadcn/ui"];

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await commands.greet(name));
  }

  return (
    <main className="app-shell dark">
      <header className="titlebar" data-tauri-drag-region="deep">
        <div className="traffic-light-controls" data-tauri-drag-region="false">
          <button
            type="button"
            className="window-control close"
            data-window-action="close"
            aria-label="Close"
            onClick={() => void runWindowCommand("close")}
          />
          <button
            type="button"
            className="window-control minimize"
            data-window-action="minimize"
            aria-label="Minimize"
            onClick={() => void runWindowCommand("minimize")}
          />
          <button
            type="button"
            className="window-control maximize"
            data-window-action="maximize"
            aria-label="Maximize"
            onClick={() => void runWindowCommand("toggleMaximize")}
          />
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
