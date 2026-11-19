// AI Elements: Prompt Input (https://ui.shadcn.com/docs/ai-elements/radix/prompt-input).
// A message field with a send button: Enter sends, Shift+Enter breaks the
// line, and while a reply streams the button stops it.
import { ArrowUpIcon, SquareIcon } from "lucide-react";
import {
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";

import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupTextarea,
} from "@/components/ui/input-group";

export function PromptInput({
  label,
  onSend,
  onStop,
  placeholder,
  disabled = false,
  autoFocus = false,
  addons,
}: {
  /** The field's accessible name. */
  label: string;
  onSend: (text: string) => void;
  /** Set while a reply streams: the button stops it, and nothing is sent. */
  onStop?: () => void;
  placeholder?: string;
  disabled?: boolean;
  /** Takes focus when it appears. */
  autoFocus?: boolean;
  /** Controls shown before the send button, such as the model selector. */
  addons?: ReactNode;
}) {
  const [text, setText] = useState("");
  const field = useRef<HTMLTextAreaElement>(null);
  const streaming = onStop !== undefined;
  const canSend = !disabled && !streaming && text.trim() !== "";

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSend) return;
    onSend(text.trim());
    setText("");
  }

  function sendOnEnter(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key !== "Enter" || event.shiftKey) return;
    if (event.nativeEvent.isComposing) return;
    event.preventDefault();
    event.currentTarget.form?.requestSubmit();
  }

  return (
    <form onSubmit={submit}>
      <InputGroup className="bg-canvas">
        <InputGroupTextarea
          ref={field}
          aria-label={label}
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={sendOnEnter}
          placeholder={placeholder}
          autoFocus={autoFocus}
        />
        <InputGroupAddon align="block-end">
          {addons}
          {streaming ? (
            <InputGroupButton
              type="button"
              size="icon"
              variant="primary"
              className="ml-auto"
              aria-label="Stop"
              onClick={() => {
                onStop();
                field.current?.focus();
              }}
            >
              <SquareIcon />
            </InputGroupButton>
          ) : (
            <InputGroupButton
              type="submit"
              size="icon"
              variant="primary"
              className="ml-auto"
              disabled={!canSend}
              aria-label="Send"
            >
              <ArrowUpIcon />
            </InputGroupButton>
          )}
        </InputGroupAddon>
      </InputGroup>
    </form>
  );
}
