import * as React from "react";
import * as LabelPrimitive from "@radix-ui/react-label";
import { cn } from "@/lib/utils";

function Label({
  className,
  ...props
}: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      className={cn(
        "text-[10px] font-medium uppercase tracking-wide text-muted-foreground select-none",
        className
      )}
      {...props}
    />
  );
}

export { Label };
