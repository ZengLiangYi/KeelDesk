import {
  Card,
  CardAction,
  CardContent,
  CardFooter,
  CardHeader,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export function WorkbenchSkeleton() {
  return (
    <div
      role="progressbar"
      aria-label="正在初始化 KeelDesk 运行时快照"
      aria-busy="true"
      className="flex h-dvh w-full flex-col overflow-clip bg-background text-foreground"
    >
      {/* Top Workbench Command Bar Skeleton */}
      <header className="shrink-0 border-b border-border-subtle bg-surface-panel px-4 py-2.5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <Skeleton className="size-8 shrink-0 rounded-md" />
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center gap-2">
                <Skeleton className="h-4 w-40 rounded-xs" />
                <Skeleton className="h-5 w-16 rounded-xs" />
                <Skeleton className="h-5 w-28 rounded-xs" />
              </div>
              <Skeleton className="h-4 w-64 rounded-xs" />
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            <Skeleton className="h-7 w-40 rounded-sm" />
            <Skeleton className="h-7 w-22 rounded-sm" />
          </div>
        </div>
      </header>

      {/* Main Two-Column Workspace Skeleton */}
      <main className="grid min-h-0 flex-1 grid-cols-12 gap-3 p-3">
        <div className="col-span-7 flex min-h-0 min-w-0 flex-col gap-3">
          <Card size="sm" className="shrink-0">
            <CardHeader className="border-b">
              <Skeleton className="h-4 w-28 rounded-xs" />
              <Skeleton className="h-3.5 w-64 rounded-xs" />
              <CardAction>
                <Skeleton className="h-6 w-20 rounded-sm" />
              </CardAction>
            </CardHeader>
            <CardContent className="flex flex-col gap-2.5">
              <div className="flex items-center gap-2 border-b border-border-subtle pb-2">
                <Skeleton className="h-6 w-24 rounded-sm" />
                <Skeleton className="h-6 w-28 rounded-sm" />
                <Skeleton className="h-6 w-28 rounded-sm" />
              </div>
              <Skeleton className="h-24 w-full rounded-md" />
            </CardContent>
          </Card>

          <Card size="sm" className="flex min-h-0 flex-1 flex-col">
            <CardHeader className="border-b">
              <Skeleton className="h-4 w-36 rounded-xs" />
              <Skeleton className="h-3.5 w-60 rounded-xs" />
              <CardAction>
                <Skeleton className="h-7 w-44 rounded-sm" />
              </CardAction>
            </CardHeader>
            <CardContent className="flex min-h-0 flex-1 flex-col gap-2.5 pt-3">
              <Skeleton className="h-8 w-full rounded-sm" />
              <Skeleton className="h-9 w-full rounded-sm" />
              <Skeleton className="h-9 w-11/12 rounded-sm" />
            </CardContent>
            <CardFooter className="justify-between">
              <Skeleton className="h-3.5 w-40 rounded-xs" />
              <Skeleton className="h-3.5 w-28 rounded-xs" />
            </CardFooter>
          </Card>
        </div>

        <Card size="sm" className="col-span-5 flex min-h-0 min-w-0 flex-col">
          <CardHeader className="border-b">
            <Skeleton className="h-4 w-32 rounded-xs" />
            <Skeleton className="h-3.5 w-56 rounded-xs" />
            <CardAction>
              <Skeleton className="h-5 w-16 rounded-xs" />
            </CardAction>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col gap-2.5 pt-3">
            <Skeleton className="h-14 w-full rounded-sm" />
            <Skeleton className="h-14 w-full rounded-sm" />
            <Skeleton className="h-14 w-full rounded-sm" />
          </CardContent>
        </Card>
      </main>

      {/* Bottom Status Bar Skeleton */}
      <footer className="flex h-7 shrink-0 items-center justify-between gap-3 border-t border-border-subtle bg-surface-subtle px-4">
        <Skeleton className="h-3 w-64 rounded-xs" />
        <Skeleton className="h-3 w-52 rounded-xs" />
      </footer>
    </div>
  );
}
