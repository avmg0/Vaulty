import * as React from "react";
import { cn } from "@/lib/utils";

const Card = ({ className, ...p }: React.ComponentProps<"div">) =>
  <div className={cn("bg-card text-card-foreground flex flex-col gap-4 rounded-xl border py-5 shadow-sm", className)} {...p} />;
const CardHeader = ({ className, ...p }: React.ComponentProps<"div">) =>
  <div className={cn("flex flex-col gap-1.5 px-5", className)} {...p} />;
const CardTitle = ({ className, ...p }: React.ComponentProps<"div">) =>
  <div className={cn("leading-none font-semibold", className)} {...p} />;
const CardDescription = ({ className, ...p }: React.ComponentProps<"div">) =>
  <div className={cn("text-muted-foreground text-sm", className)} {...p} />;
const CardContent = ({ className, ...p }: React.ComponentProps<"div">) =>
  <div className={cn("px-5", className)} {...p} />;

export { Card, CardHeader, CardTitle, CardDescription, CardContent };
