// AI Elements: Suggestion (https://ui.shadcn.com/docs/ai-elements/radix/suggestion).
// Short prompts sent to the agent in one click.
import { Button } from "@/components/ui/button";

export function Suggestions({
  suggestions,
  onSend,
}: {
  suggestions: string[];
  onSend: (suggestion: string) => void;
}) {
  return (
    <div className="scrollbar-none flex scroll-fade-x gap-2 overflow-x-auto overscroll-x-contain">
      {suggestions.map((suggestion) => (
        <Button
          key={suggestion}
          variant="outline"
          size="sm"
          className="shrink-0"
          onClick={() => onSend(suggestion)}
        >
          {suggestion}
        </Button>
      ))}
    </div>
  );
}
