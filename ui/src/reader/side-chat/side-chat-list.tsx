import { useEffect } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { ModelPicker } from "@/model-picker/model-picker";
import { selectSideChats } from "@/store";

/**
 * The course's side chats, newest first, each by its passage or first
 * question and the section it was asked in; the model in use at the foot.
 */
export function SideChatList({
  courseId,
  openChatId,
}: {
  courseId: string;
  openChatId: string | null;
}) {
  const { send } = useAtlasStore();
  const chats = useAtlas((state) => selectSideChats(state, courseId));

  useEffect(() => {
    send({ type: "ListSideChats", courseId }).catch(failureInStore);
  }, [send, courseId]);

  return (
    <>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-3">
        {chats === null ? (
          <p role="status" className="px-2 text-13 text-muted-foreground">
            Loading side chats…
          </p>
        ) : chats.length === 0 ? (
          <div className="flex flex-col gap-1 px-2 text-13">
            <p className="text-body">No side chats yet.</p>
            <p className="text-muted-foreground">
              Start one with New side chat, above.
            </p>
          </div>
        ) : (
          <ul aria-label="Side chats" className="flex flex-col gap-0.5">
            {chats.map((chat) => (
              <li
                key={chat.id}
                aria-current={chat.id === openChatId ? "true" : undefined}
                className="flex flex-col gap-0.5 rounded-sm px-2 py-2 aria-[current=true]:bg-wash"
              >
                <span className="line-clamp-2 text-13 text-body">
                  {chat.opening}
                </span>
                {chat.sectionTitle !== null && (
                  <span className="truncate text-12 text-muted-foreground">
                    {chat.sectionTitle}
                  </span>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="shrink-0 border-t border-edge px-4 py-2.5">
        <ModelPicker />
      </div>
    </>
  );
}
