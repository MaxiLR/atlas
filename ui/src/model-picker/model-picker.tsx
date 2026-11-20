import { CircleAlertIcon, RotateCwIcon } from "lucide-react";
import { useCallback, useEffect, useId, useState } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import {
  ModelSelector,
  modelTitle,
} from "@/components/ai-elements/model-selector";
import {
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";

const NOT_LISTED = "The model list could not be loaded.";

/** How this picker's own reading of the list stands. */
type Listing = "loading" | "listed" | "failed";

/**
 * The model in use, named by its display name, and the menu that picks
 * another. The list is read on mount; until it answers the control shows
 * the last known model or a neutral placeholder, and when it fails the
 * control keeps the model in use and its menu offers to read it again. With
 * a single model there is nothing to pick, so the name stands alone.
 */
export function ModelPicker() {
  const { send } = useAtlasStore();
  const list = useAtlas((state) => state.models.list);
  const [listing, setListing] = useState<Listing>("loading");
  const descriptionId = useId();

  const read = useCallback(() => {
    let live = true;
    send({ type: "ListModels" }).then(
      () => live && setListing("listed"),
      () => live && setListing("failed"),
    );
    return () => {
      live = false;
    };
  }, [send]);
  useEffect(read, [read]);

  const retry = () => {
    setListing("loading");
    read();
  };
  const select = (slug: string) => {
    send({ type: "SelectModel", slug }).catch(failureInStore);
  };

  const models = list?.models ?? [];
  const current = list?.current ?? null;
  const name =
    current === null
      ? null
      : (models.find(({ slug }) => slug === current)?.displayName ?? current);

  if (listing === "failed") {
    return (
      <>
        <ModelSelector
          models={[]}
          value={current}
          name={name}
          onValueChange={select}
          aria-describedby={descriptionId}
          marker={<CircleAlertIcon aria-hidden className="text-amber" />}
        >
          <p className="max-w-56 px-2 py-1.5 text-12 leading-snug text-muted-foreground">
            {NOT_LISTED}
          </p>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={retry}>
            <RotateCwIcon aria-hidden />
            Try again
          </DropdownMenuItem>
        </ModelSelector>
        <span id={descriptionId} className="sr-only">
          {NOT_LISTED}
        </span>
      </>
    );
  }

  if (list === null) {
    return (
      <ModelSelector
        models={[]}
        value={null}
        onValueChange={select}
        disabled
        aria-busy
      />
    );
  }

  if (models.length < 2) {
    return (
      <p
        title={modelTitle(name)}
        className="inline-flex h-7 max-w-56 min-w-0 items-center px-2.5 text-13 leading-none font-medium text-muted-foreground"
      >
        <span className="sr-only">Model: </span>
        <span className="truncate">{name ?? "None listed"}</span>
      </p>
    );
  }

  return (
    <ModelSelector
      models={models}
      value={current}
      onValueChange={select}
      aria-busy={listing === "loading" || undefined}
    />
  );
}
