// AI Elements: Prompt Input (https://ui.shadcn.com/docs/ai-elements/radix/prompt-input).
// A message field with a send button.
import { ArrowUpIcon } from "lucide-react";
import { useState, type FormEvent, type ReactNode } from "react";

import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupTextarea,
} from "@/components/ui/input-group";

export function PromptInput({
  onSend,
  placeholder,
  disabled = false,
  addons,
}: {
  onSend: (text: string) => void;
  placeholder?: string;
  disabled?: boolean;
  /** Controls shown before the send button, such as the model selector. */
  addons?: ReactNode;
}) {
  const [text, setText] = useState("");
  const canSend = !disabled && text.trim() !== "";

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSend) return;
    onSend(text.trim());
    setText("");
  }

  return (
    <form onSubmit={submit}>
      <InputGroup>
        <InputGroupTextarea
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder={placeholder}
          className="p-3"
        />
        <InputGroupAddon align="block-end">
          {addons}
          <InputGroupButton
            type="submit"
            size="icon-sm"
            variant="default"
            className="ml-auto rounded-full"
            disabled={!canSend}
            aria-label="Send"
          >
            <ArrowUpIcon />
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
    </form>
  );
}
