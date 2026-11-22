import type * as React from "react";
import { Toaster as Sonner, type ToasterProps } from "sonner";

import { useTheme } from "@/theme/provider";

const Toaster = ({ ...props }: ToasterProps) => {
  const { theme } = useTheme();

  return <Sonner theme={theme} className="toaster group" {...props} />;
};

type NoticeProps = Omit<React.ComponentProps<"div">, "title" | "role"> & {
  kind: "error" | "success";
  title: string;
  description: string;
  action?: { label: string; onClick: () => void } | null;
  onDismiss: () => void;
};

/** One notice's body, rendered inside a Sonner toast. */
function Notice(props: NoticeProps) {
  void props;
  return null;
}

export { Notice, Toaster };
