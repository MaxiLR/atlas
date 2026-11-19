import { useEffect } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";

/** The model in use, by its display name; nothing until the core lists them. */
export function ModelPicker() {
  const { send } = useAtlasStore();
  const list = useAtlas((state) => state.models.list);
  const listed = list !== null;

  useEffect(() => {
    if (!listed) send({ type: "ListModels" }).catch(failureInStore);
  }, [send, listed]);

  if (list === null || list.current === null) return null;
  const model = list.models.find(({ slug }) => slug === list.current);
  return (
    <p className="text-13 text-muted-foreground">
      <span className="sr-only">Model: </span>
      {model?.displayName ?? list.current}
    </p>
  );
}
