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
      aria-label="正在初始化龙骨工作台运行时快照"
      aria-busy="true"
      className="flex h-dvh w-full flex-col overflow-clip bg-background text-foreground"
    >
      {/* Top Workbench Command Bar Skeleton */}
      <header className="shrink-0 border-b border-border bg-card/65 px-5 py-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <Skeleton className="size-9 shrink-0 rounded-lg" />
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center gap-2">
                <Skeleton className="h-4 w-40" />
                <Skeleton className="h-5 w-16 rounded-4xl" />
                <Skeleton className="h-5 w-28 rounded-4xl" />
              </div>
              <Skeleton className="h-4 w-64" />
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            <Skeleton className="h-8 w-44 rounded-lg" />
            <Skeleton className="h-7 w-22 rounded-lg" />
          </div>
        </div>
      </header>

      {/* Main Two-Column Workspace Skeleton */}
      <main className="grid min-h-0 flex-1 grid-cols-12 gap-4 p-4">
        <div className="col-span-7 flex min-h-0 min-w-0 flex-col gap-4">
          <Card size="sm" className="shrink-0">
            <CardHeader className="border-b">
              <Skeleton className="h-4 w-28" />
              <Skeleton className="h-3.5 w-64" />
              <CardAction>
                <Skeleton className="h-6 w-20 rounded-md" />
              </CardAction>
            </CardHeader>
            <CardContent className="flex flex-col gap-2.5">
              <div className="flex items-center gap-2 border-b border-border pb-2">
                <Skeleton className="h-6 w-24 rounded-md" />
                <Skeleton className="h-6 w-28 rounded-md" />
                <Skeleton className="h-6 w-28 rounded-md" />
              </div>
              <Skeleton className="h-24 w-full rounded-lg" />
            </CardContent>
          </Card>

          <Card size="sm" className="flex min-h-0 flex-1 flex-col">
            <CardHeader className="border-b">
              <Skeleton className="h-4 w-36" />
              <Skeleton className="h-3.5 w-60" />
              <CardAction>
                <Skeleton className="h-5 w-20 rounded-4xl" />
              </CardAction>
            </CardHeader>
            <CardContent className="flex min-h-0 flex-1 flex-col gap-2.5 pt-3">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-9 w-full" />
              <Skeleton className="h-9 w-11/12" />
            </CardContent>
            <CardFooter className="justify-between py-2">
              <Skeleton className="h-3.5 w-40" />
              <Skeleton className="h-3.5 w-28" />
            </CardFooter>
          </Card>
        </div>

        <Card size="sm" className="col-span-5 flex min-h-0 min-w-0 flex-col">
          <CardHeader className="border-b">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-3.5 w-56" />
            <CardAction>
              <Skeleton className="h-5 w-16 rounded-4xl" />
            </CardAction>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col gap-2.5 pt-3">
            <Skeleton className="h-14 w-full" />
            <Skeleton className="h-14 w-full" />
            <Skeleton className="h-14 w-full" />
          </CardContent>
        </Card>
      </main>

      {/* Bottom Status Bar Skeleton */}
      <footer className="flex h-7 shrink-0 items-center justify-between gap-3 border-t border-border bg-card/55 px-5">
        <Skeleton className="h-3 w-64" />
        <Skeleton className="h-3 w-52" />
      </footer>
    </div>
  );
}
