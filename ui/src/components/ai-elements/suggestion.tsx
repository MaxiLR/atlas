// AI Elements: Suggestion (https://ui.shadcn.com/docs/ai-elements/radix/suggestion).
// Short prompts sent to the agent in one click, stacked and wrapping.
import { Button } from "@/components/ui/button";

export function Suggestions({
  suggestions,
  onSend,
}: {
  suggestions: string[];
  onSend: (suggestion: string) => void;
}) {
  return (
    <div className="flex flex-col items-start gap-1">
      {suggestions.map((suggestion) => (
        <Button
          key={suggestion}
          className="h-auto min-h-7 max-w-full py-1.5 text-left leading-[1.35] font-normal whitespace-normal"
          onClick={() => onSend(suggestion)}
        >
          {suggestion}
        </Button>
      ))}
    </div>
  );
}
