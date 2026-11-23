import { useEffect, useRef } from "react";
import { toast } from "sonner";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Notice, Toaster } from "@/components/ui/sonner";
import { selectNotices, type Notice as NoticeState } from "@/store";

/** A notice keeps 16 px from the window's right edge and from its foot. */
const EDGE = 16;
/** The reader ends with its 64 px foot, which a notice never covers. */
const READER_FOOT = 64;

/**
 * The store's notices, shown through Sonner, one toast per notice keyed by
 * its cause. A notice the store closes leaves the window; one Sonner closes
 * (by its time running out, or a swipe) leaves the store. Errors stay until
 * dismissed or until their failure resolves; a success closes after Sonner's
 * default time, held while hovered or focused.
 */
export function Notices() {
  const { send, navigate } = useAtlasStore();
  const notices = useAtlas(selectNotices);
  const screen = useAtlas((state) => state.navigation.screen);
  /** Each notice as it was last handed to Sonner, by cause. */
  const shown = useRef(new Map<string, NoticeState>());

  useEffect(() => {
    const showing = shown.current;
    return () => {
      for (const id of showing.keys()) toast.dismiss(id);
      showing.clear();
    };
  }, []);

  useEffect(() => {
    const showing = shown.current;
    const present = (notice: NoticeState, held: boolean) => {
      // Sonner closed it; a notice of the same cause shown since stays.
      const closed = () => {
        if (showing.get(notice.id) === notice) {
          navigate({ type: "DismissNotice", id: notice.id });
        }
      };
      const { action } = notice;
      toast.custom(
        () => (
          <Notice
            kind={notice.kind}
            title={notice.title}
            description={notice.description}
            action={
              action && {
                label: action.label,
                onClick: () => {
                  send(action.request).catch(failureInStore);
                },
              }
            }
            onDismiss={() => navigate({ type: "DismissNotice", id: notice.id })}
            onFocus={() => {
              if (notice.kind === "success" && !held) present(notice, true);
            }}
            onBlur={(event) => {
              if (
                held &&
                !event.currentTarget.contains(event.relatedTarget as Node)
              ) {
                present(notice, false);
              }
            }}
          />
        ),
        {
          id: notice.id,
          duration: notice.kind === "error" || held ? Infinity : undefined,
          onDismiss: closed,
          onAutoClose: closed,
        },
      );
    };

    for (const id of showing.keys()) {
      if (!notices.some((notice) => notice.id === id)) {
        showing.delete(id);
        toast.dismiss(id);
      }
    }
    // Sonner puts each new toast first, so notices that arrive together are
    // handed over newest first and read oldest first, as the store has them.
    for (const notice of notices.toReversed()) {
      if (showing.get(notice.id) === notice) continue;
      showing.set(notice.id, notice);
      present(notice, false);
    }
  }, [notices, send, navigate]);

  const bottom = screen === "reader" ? EDGE + READER_FOOT : EDGE;
  return (
    <Toaster
      offset={{ right: EDGE, bottom }}
      mobileOffset={{ right: EDGE, bottom }}
    />
  );
}
