import type { ModelList } from "@/core/generated/ModelList";

export interface ModelsSlice {
  /** Null until the core has listed them. */
  list: ModelList | null;
}

export const initialModels: ModelsSlice = { list: null };
