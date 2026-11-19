# Glossary

**Candidate** — The person learning. One per installation.

**Topic** — What the candidate asks to learn, in their own words.

**Course** — Everything Atlas holds for one topic and one candidate: the
candidate profile, the index, the generated sections, the glossary and the side
chats. The **library** is the list of courses on the home screen.

**Concept ladder** — Eight to twelve concepts of the topic in prerequisite
order, drafted by the agent before placement and not shown.

**Placement** — The questions that open a course: one self-rating and the goal,
then four to six multiple-choice questions asked one at a time, moving up the
concept ladder after a right answer and down after a wrong one or "Not sure". It
stops once two neighbouring rungs disagree.

**Candidate profile** — What Atlas believes the candidate knows: each rung of
the concept ladder marked known, shaky or unknown, the misconceptions picked,
and the goal. Never a single score. Check questions keep updating it, and it is
sent with every generation.

**Index** — The tree of sections of a course. It starts two levels deep
(chapters, then sections) and covers the whole topic at overview depth: the
**broad pass**.

**Section** — One index entry. Its content is generated when first opened,
stored, and reopened without regenerating. "Explain differently" adds a new
**version**; earlier versions are kept.

**Deep dive** — Child sections generated under a section by "Go deeper", to any
depth. A side branch: Next walks the broad pass, not the deep dives.

**Course map** — What the agent plans with the index and does not show: for
every section, the concepts it introduces and the concepts it relies on, and the
course's conventions. Deep dives extend it. Not the concept-map visual of the
catalog.

**Conventions** — The notation, terminology and running examples a course has
fixed. The first fixing holds for the whole course.

**Section record** — What a section leaves for later ones, written by the agent
with the section and never shown: its summary, the concepts it taught, the
conventions it fixed, the examples it used and what it promised a later section
would cover. One per version. Together with the conventions, the records are the
**course memory**: the core picks from it what each new section is sent.

**Next** — The control at the foot of a section that opens, and if needed
generates, the following section of the broad pass. The full index beside it
jumps anywhere.

**Check question** — One or two multiple-choice questions at the end of a
section. Answers update the candidate profile.

**Block** — One unit of generated section content: prose, or a visual from the
catalog.

**Catalog** — The fixed set of visual components the agent may compose. The one
free-form entry is the **figure**: a static SVG the model writes using only
Atlas's colour and type tokens, sanitised, with no scripts.

**Define** — Selecting text and asking for a short explanation shown in place,
written for the candidate profile and the surrounding sentence. One request, no
conversation. It also offers the Wikipedia summary of the term, with a link.

**Glossary** — The definitions a course has collected. A defined term keeps a
faint dotted underline and reopens without a request.

**Side chat** — A conversation in the right-hand panel with its own history,
usually **anchored** to a selected passage, which keeps a faint margin mark that
reopens it. A course holds many; one can start without a selection. A side chat
never changes a section and never enters the context of section generation.

**Plan login** — The candidate's Sign in with ChatGPT connection, which pays for
model calls with their ChatGPT plan.

**Usage meter** — The display of the plan's limit windows: the 5-hour and weekly
windows, the share used, the time to each reset and a pace mark.

**Window** — One usage limit of the plan over a stretch of time, as the provider
reports it: its length, the share used and when it resets.

**Pace mark** — The tick on a usage bar at the share of the window's time that
has passed. Usage past it means the window runs out before it resets at the
current rate.
