import { describeError } from "@/core/client";
import type { CoreError } from "@/core/generated/CoreError";
import type { ExportResult } from "@/core/generated/ExportResult";

import type { Notice } from "./notices";
import { unknownTag } from "./unknown-tag";

/** The cause of the notice saying how the last PDF export ended. */
export const PDF_NOTICE = "pdf";

function exportFailed(reason: string): Notice {
  return {
    id: PDF_NOTICE,
    kind: "error",
    title: "PDF export failed",
    description: reason,
    action: null,
  };
}

/** The core answered an export: where the PDF went, or why it did not. */
export function exportedNotice(result: ExportResult): Notice | null {
  switch (result.type) {
    case "Saved":
      return {
        id: PDF_NOTICE,
        kind: "success",
        title: "PDF saved",
        description: result.path.split(/[\\/]/).at(-1) ?? result.path,
        action: null,
      };
    case "Cancelled":
      return null;
    case "Failed":
      return exportFailed(result.reason);
    default:
      return unknownTag(result);
  }
}

/** The core refused an export: why. */
export function exportRefusedNotice(error: CoreError): Notice {
  return exportFailed(describeError(error));
}
