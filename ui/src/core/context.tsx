import { createContext, useContext, type ReactNode } from "react";

import type { CoreClient } from "./client";

const CoreContext = createContext<CoreClient | null>(null);

export function CoreProvider({
  client,
  children,
}: {
  client: CoreClient;
  children: ReactNode;
}) {
  return <CoreContext value={client}>{children}</CoreContext>;
}

export function useCore(): CoreClient {
  const client = useContext(CoreContext);
  if (client === null) {
    throw new Error("useCore needs a CoreProvider");
  }
  return client;
}
